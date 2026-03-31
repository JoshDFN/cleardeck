// ============================================================================
// ⚠️  CRITICAL DEPLOYMENT WARNING ⚠️
// ============================================================================
// This canister holds REAL USER FUNDS (ICP/BTC). When upgrading:
//
// ✅ ALWAYS use: icp deploy <name> -e ic --mode upgrade
// ❌ NEVER use:  icp deploy <name> -e ic --mode reinstall
//
// --mode reinstall DESTROYS ALL STATE including user balances!
// The post_upgrade hook will PANIC if state restoration fails, rejecting
// the upgrade to protect user funds.
// ============================================================================

use candid::{CandidType, Deserialize, Principal, Nat};
use ic_cdk::management_canister::raw_rand;
use sha2::{Sha224, Sha256, Digest};
use std::cell::RefCell;
use std::collections::HashMap;
use icrc_ledger_types::icrc1::account::Account;
use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};
use icrc_ledger_types::icrc2::transfer_from::{TransferFromArgs, TransferFromError};
// ETH deposit: threshold ECDSA + EVM RPC
use k256::PublicKey as K256PublicKey;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey};
use sha3::Keccak256;
use alloy_consensus::TxEip1559;
use alloy_consensus::SignableTransaction;
use alloy_primitives::{Address, Bytes, TxKind, U256};

// ============================================================================
// CONSTANTS
// ============================================================================

const DEFAULT_ACTION_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUTS_BEFORE_SITOUT: u8 = 2;
const DEFAULT_TIME_BANK_SECS: u64 = 30;
const AUTO_DEAL_DELAY_NS: u64 = 3_000_000_000;
const RELOAD_TIMEOUT_SECS: u64 = 60;
const SITTING_OUT_KICK_SECS: u64 = 120; // Auto-kick sitting out players after 2 minutes

// ICP Ledger canister ID (mainnet)
const ICP_LEDGER_CANISTER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";
const ICP_TRANSFER_FEE: u64 = 10_000; // 0.0001 ICP

// ckBTC Ledger canister ID (mainnet)
const CKBTC_LEDGER_CANISTER: &str = "mxzaz-hqaaa-aaaar-qaada-cai";
const CKBTC_TRANSFER_FEE: u64 = 10; // 10 satoshis

// ckETH Ledger canister ID (mainnet)
const CKETH_LEDGER_CANISTER: &str = "ss2fx-dyaaa-aaaar-qacoq-cai";
const CKETH_TRANSFER_FEE: u64 = 2_000_000_000_000; // 0.000002 ETH (2000 Gwei)

// Rate limiting
const RATE_LIMIT_WINDOW_NS: u64 = 1_000_000_000; // 1 second
const MAX_ACTIONS_PER_WINDOW: u32 = 10;

// Withdrawal limits for ICP (in e8s - 1 ICP = 100_000_000 e8s)
const ICP_MAX_WITHDRAWAL_PER_TX: u64 = 10_000_000_000; // 100 ICP max per withdrawal
const ICP_MIN_WITHDRAWAL_AMOUNT: u64 = 100_000; // 0.001 ICP minimum (must cover fees)

// Withdrawal limits for BTC (in satoshis - 1 BTC = 100_000_000 satoshis)
const BTC_MAX_WITHDRAWAL_PER_TX: u64 = 10_000_000; // 0.1 BTC max per withdrawal
const BTC_MIN_WITHDRAWAL_AMOUNT: u64 = 11; // Just above 10 sat fee - receive at least 1 sat

// Withdrawal limits for ETH (in wei - 1 ETH = 1_000_000_000_000_000_000 wei)
const ETH_MAX_WITHDRAWAL_PER_TX: u64 = 1_000_000_000_000_000_000; // 1 ETH max per withdrawal
const ETH_MIN_WITHDRAWAL_AMOUNT: u64 = 10_000_000_000_000; // 0.00001 ETH minimum (must cover fees)

// Dogecoin canister (DFINITY official, mainnet beta)
const DOGECOIN_CANISTER: &str = "gordg-fyaaa-aaaan-aaadq-cai";
const DOGE_TRANSFER_FEE: u64 = 100_000; // 0.001 DOGE standard tx fee (in shibes)
const DOGE_MIN_CONFIRMATIONS: u32 = 6;

// Withdrawal limits for DOGE (in shibes - 1 DOGE = 100_000_000 shibes)
const DOGE_MAX_WITHDRAWAL_PER_TX: u64 = 100_000_000_000; // 1000 DOGE max per withdrawal
const DOGE_MIN_WITHDRAWAL_AMOUNT: u64 = 200_000; // 0.002 DOGE minimum (must cover fee)

const WITHDRAWAL_COOLDOWN_NS: u64 = 60_000_000_000; // 60 second cooldown between withdrawals

// Deposit verification rate limiting
const MAX_DEPOSIT_VERIFICATIONS_PER_MINUTE: u32 = 5;

// Heartbeat rate limiting (2 per second max to prevent DoS)
const MAX_HEARTBEATS_PER_SECOND: u32 = 2;

// Cleanup thresholds to prevent unbounded memory growth
const MAX_HAND_HISTORY_ENTRIES: usize = 100; // Keep last 100 hands in local history
const MAX_SHOWN_CARDS_HANDS: usize = 10; // Track shown cards for last 10 hands
const RATE_LIMIT_CLEANUP_AGE_NS: u64 = 60_000_000_000; // Clean up rate limit entries older than 1 minute
const CLEANUP_INTERVAL_NS: u64 = 30_000_000_000; // Run cleanup every 30 seconds

// ============================================================================
// TYPES - Core poker data structures
// ============================================================================

/// Currency type for the table - determines which ledger to use
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Currency {
    #[default]
    ICP,   // Uses ICP ledger, amounts in e8s (1 ICP = 100_000_000 e8s)
    BTC,   // Uses ckBTC ledger, amounts in satoshis (1 BTC = 100_000_000 sats)
    ETH,   // Uses ckETH ledger, amounts in wei (1 ETH = 1_000_000_000_000_000_000 wei)
    DOGE,  // Direct DOGE via Dogecoin canister, amounts in shibes (1 DOGE = 100_000_000 shibes)
}

impl Currency {
    pub fn ledger_canister(&self) -> Principal {
        match self {
            Currency::ICP => Principal::from_text(ICP_LEDGER_CANISTER).unwrap(),
            Currency::BTC => Principal::from_text(CKBTC_LEDGER_CANISTER).unwrap(),
            Currency::ETH => Principal::from_text(CKETH_LEDGER_CANISTER).unwrap(),
            Currency::DOGE => panic!("DOGE uses internal balance tracking, no external ledger"),
        }
    }

    pub fn has_external_ledger(&self) -> bool {
        !matches!(self, Currency::DOGE)
    }

    pub fn transfer_fee(&self) -> u64 {
        match self {
            Currency::ICP => ICP_TRANSFER_FEE,
            Currency::BTC => CKBTC_TRANSFER_FEE,
            Currency::ETH => CKETH_TRANSFER_FEE,
            Currency::DOGE => DOGE_TRANSFER_FEE,
        }
    }

    pub fn min_withdrawal(&self) -> u64 {
        match self {
            Currency::ICP => ICP_MIN_WITHDRAWAL_AMOUNT,
            Currency::BTC => BTC_MIN_WITHDRAWAL_AMOUNT,
            Currency::ETH => ETH_MIN_WITHDRAWAL_AMOUNT,
            Currency::DOGE => DOGE_MIN_WITHDRAWAL_AMOUNT,
        }
    }

    pub fn max_withdrawal(&self) -> u64 {
        match self {
            Currency::ICP => ICP_MAX_WITHDRAWAL_PER_TX,
            Currency::BTC => BTC_MAX_WITHDRAWAL_PER_TX,
            Currency::ETH => ETH_MAX_WITHDRAWAL_PER_TX,
            Currency::DOGE => DOGE_MAX_WITHDRAWAL_PER_TX,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::ICP => "ICP",
            Currency::BTC => "BTC",
            Currency::ETH => "ETH",
            Currency::DOGE => "DOGE",
        }
    }

    pub fn decimals(&self) -> u8 {
        match self {
            Currency::ICP | Currency::BTC | Currency::DOGE => 8,
            Currency::ETH => 18,
        }
    }

    /// Format an amount in smallest units (e8s/satoshis/wei) as a human-readable string
    pub fn format_amount(&self, smallest_units: u64) -> String {
        match self {
            Currency::ICP => {
                let decimal = smallest_units as f64 / 100_000_000.0;
                format!("{:.4} ICP", decimal)
            }
            Currency::BTC => {
                let decimal = smallest_units as f64 / 100_000_000.0;
                if decimal >= 0.001 {
                    format!("{:.4} BTC", decimal)
                } else {
                    format!("{} sats", smallest_units)
                }
            }
            Currency::ETH => {
                let decimal = smallest_units as f64 / 1_000_000_000_000_000_000.0;
                if decimal >= 0.001 {
                    format!("{:.6} ETH", decimal)
                } else {
                    let gwei = smallest_units as f64 / 1_000_000_000.0;
                    format!("{:.2} Gwei", gwei)
                }
            }
            Currency::DOGE => {
                let decimal = smallest_units as f64 / 100_000_000.0;
                if decimal >= 1.0 {
                    format!("{:.2} DOGE", decimal)
                } else {
                    format!("{} shibes", smallest_units)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    fn value(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard(Vec<u8>),
    Pair(u8, Vec<u8>),
    TwoPair(u8, u8, u8),
    ThreeOfAKind(u8, Vec<u8>),
    Straight(u8),
    Flush(Vec<u8>),
    FullHouse(u8, u8),
    FourOfAKind(u8, u8),
    StraightFlush(u8),
    RoyalFlush,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum PlayerAction {
    Fold,
    Check,
    Call,
    Bet(u64),
    Raise(u64),
    AllIn,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    WaitingForPlayers,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    HandComplete,
}

/// Last action taken by a player - displayed to other players
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum LastAction {
    Fold,
    Check,
    Call { amount: u64 },
    Bet { amount: u64 },
    Raise { amount: u64 },
    AllIn { amount: u64 },
    PostBlind { amount: u64 },
}

/// Record of the last action for display purposes
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct LastActionInfo {
    pub seat: u8,
    pub action: LastAction,
    pub timestamp: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum PlayerStatus {
    Active,
    SittingOut,
    Disconnected,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Player {
    pub principal: Principal,
    pub seat: u8,
    pub chips: u64,
    pub hole_cards: Option<(Card, Card)>,
    pub current_bet: u64,
    pub total_bet_this_hand: u64,
    pub has_folded: bool,
    pub has_acted_this_round: bool,
    pub is_all_in: bool,
    pub status: PlayerStatus,
    pub last_seen: u64,
    pub timeout_count: u8,
    pub time_bank_remaining: u64, // Extra time bank in seconds
    pub is_sitting_out_next_hand: bool, // Will sit out after current hand
    pub broke_at: Option<u64>, // Timestamp when player hit 0 chips (for reload timer)
    #[serde(default)] // For backwards compatibility
    pub sitting_out_since: Option<u64>, // Timestamp when player started sitting out (for auto-kick)
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ActionTimer {
    pub player_seat: u8,
    pub started_at: u64,
    pub expires_at: u64,
    pub using_time_bank: bool, // Whether player is using their time bank
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ShuffleProof {
    pub seed_hash: String,
    pub revealed_seed: Option<String>,
    pub timestamp: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableConfig {
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_players: u8,
    pub action_timeout_secs: u64,
    pub ante: u64, // Ante amount (0 for no ante)
    pub time_bank_secs: u64, // Time bank per player
    #[serde(default)] // Backwards compatibility - defaults to ICP
    pub currency: Currency, // ICP or BTC
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableState {
    pub id: u64,
    pub config: TableConfig,
    pub players: Vec<Option<Player>>,
    pub community_cards: Vec<Card>,
    pub deck: Vec<Card>,
    pub deck_index: usize,
    pub pot: u64,
    pub side_pots: Vec<SidePot>,
    pub current_bet: u64,
    pub min_raise: u64,
    pub phase: GamePhase,
    pub dealer_seat: u8,
    pub small_blind_seat: u8,
    pub big_blind_seat: u8,
    pub action_on: u8,
    pub action_timer: Option<ActionTimer>,
    pub shuffle_proof: Option<ShuffleProof>,
    pub hand_number: u64,
    pub last_aggressor: Option<u8>,
    pub bb_has_option: bool, // True if BB still has option to raise when limped to
    pub first_hand: bool, // Track if this is the first hand (for dealer button init)
    pub auto_deal_at: Option<u64>, // Timestamp for when to auto-deal next hand (nanoseconds)
    pub last_action: Option<LastActionInfo>, // Last action taken - for UI display
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandResult {
    pub winners: Vec<Winner>,
    pub hand_number: u64,
    pub community_cards: Vec<Card>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Winner {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
    pub hand_rank: Option<HandRank>,
    pub cards: Option<(Card, Card)>,
}

/// Player info for hand history (all players who went to showdown)
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ShowdownPlayer {
    pub seat: u8,
    pub principal: Principal,
    pub cards: Option<(Card, Card)>,
    pub hand_rank: Option<HandRank>,
    pub amount_won: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandHistory {
    pub hand_number: u64,
    pub shuffle_proof: ShuffleProof,
    pub actions: Vec<ActionRecord>,
    pub winners: Vec<Winner>,
    pub community_cards: Vec<Card>,
    #[serde(default)] // For backwards compatibility with old state that doesn't have this field
    pub showdown_players: Vec<ShowdownPlayer>, // All players who went to showdown (not just winners)
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ActionRecord {
    pub seat: u8,
    pub action: PlayerAction,
    pub timestamp: u64,
    #[serde(default)] // For backwards compatibility with old state
    pub phase: String, // "preflop", "flop", "turn", "river"
    #[serde(default)] // For backwards compatibility - tracks actual amount for Call/AllIn
    pub amount: u64,
}

/// A player's view of another player at the table
/// Hole cards are only visible if it's the viewer's own cards or at showdown
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct PlayerView {
    pub principal: Principal,
    pub seat: u8,
    pub chips: u64,
    pub hole_cards: Option<(Card, Card)>,  // None if not visible to viewer
    pub current_bet: u64,
    pub has_folded: bool,
    pub is_all_in: bool,
    pub status: PlayerStatus,
    pub is_self: bool,  // True if this is the viewer's own seat
    pub display_name: Option<String>,  // Custom display name set by player
}

/// Complete view of the table from a specific player's perspective
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableView {
    pub id: u64,
    pub config: TableConfig,
    pub players: Vec<Option<PlayerView>>,
    pub community_cards: Vec<Card>,
    pub pot: u64,
    pub side_pots: Vec<SidePot>, // Side pots for all-in situations
    pub current_bet: u64,
    pub min_raise: u64, // Minimum raise amount
    pub phase: GamePhase,
    pub dealer_seat: u8,
    pub small_blind_seat: u8,
    pub big_blind_seat: u8,
    pub action_on: u8,
    pub time_remaining_secs: Option<u64>,
    pub time_bank_remaining_secs: Option<u64>, // My remaining time bank
    pub using_time_bank: bool, // Whether current player is using time bank
    pub is_my_turn: bool,
    pub my_seat: Option<u8>,
    pub hand_number: u64,
    pub shuffle_proof: Option<ShuffleProof>,
    pub last_hand_winners: Vec<Winner>,  // Winners from the last completed hand
    pub call_amount: u64, // Amount needed to call (convenience field)
    pub can_check: bool, // Whether check is valid
    pub can_raise: bool, // Whether raise is valid
    pub min_bet: u64, // Minimum bet amount
    pub last_action: Option<LastActionInfo>, // Last action taken - for UI notification
}

// ============================================================================
// STATE
// ============================================================================

thread_local! {
    static TABLE: RefCell<Option<TableState>> = RefCell::new(None);
    static HAND_HISTORY: RefCell<Vec<HandHistory>> = RefCell::new(Vec::new());
    static LAST_HAND_WINNERS: RefCell<Vec<Winner>> = RefCell::new(Vec::new()); // Winners from the previous completed hand
    static CURRENT_ACTIONS: RefCell<Vec<ActionRecord>> = RefCell::new(Vec::new());
    static BALANCES: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
    static VERIFIED_DEPOSITS: RefCell<HashMap<u64, Principal>> = RefCell::new(HashMap::new());
    // Watermark: highest block index that was pruned from VERIFIED_DEPOSITS.
    // Any notify_deposit with block_index <= this value is rejected, preventing replay of pruned entries.
    static MIN_VERIFIED_BLOCK_INDEX: RefCell<u64> = RefCell::new(0);
    // Failed ckBTC sweeps: (caller, amount) pairs where sweep transfer failed after minter minted.
    // These funds are stuck in the user's subaccount and can be retried.
    static FAILED_CKBTC_SWEEPS: RefCell<Vec<(Principal, u64)>> = RefCell::new(Vec::new());
    // Pending deposits being verified - prevents double-crediting race condition
    static PENDING_DEPOSITS: RefCell<HashMap<u64, Principal>> = RefCell::new(HashMap::new());
    // Pending withdrawals - prevents reentrancy (used by WithdrawalGuard)
    static PENDING_WITHDRAWALS: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
    // Pending DOGE withdrawals - prevents reentrancy (used by DogeWithdrawalGuard)
    static PENDING_DOGE_WITHDRAWALS: RefCell<std::collections::HashSet<Principal>> = RefCell::new(std::collections::HashSet::new());
    // DEPRECATED: LEDGER_ID is now derived from TABLE_CONFIG.currency
    // Kept for backwards compatibility during migration
    static LEDGER_ID: RefCell<Principal> = RefCell::new(
        Principal::from_text(ICP_LEDGER_CANISTER)
            .expect("Invalid ICP ledger canister ID constant - this is a code bug")
    );
    static HISTORY_ID: RefCell<Option<Principal>> = RefCell::new(None);
    static STARTING_CHIPS: RefCell<HashMap<u8, u64>> = RefCell::new(HashMap::new());
    static TABLE_CONFIG: RefCell<Option<TableConfig>> = RefCell::new(None);
    // Controllers who can call admin functions
    static CONTROLLERS: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
    // Rate limiting: caller -> (last_action_time, count_in_window)
    static RATE_LIMITS: RefCell<HashMap<Principal, (u64, u32)>> = RefCell::new(HashMap::new());
    // Seed bytes for shuffle - only revealed when hand ends
    static CURRENT_SEED: RefCell<Option<Vec<u8>>> = RefCell::new(None);
    // Last withdrawal time per user - for cooldown enforcement
    static LAST_WITHDRAWAL: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
    // Deposit verification rate limiting: caller -> (window_start, count_in_window)
    static DEPOSIT_RATE_LIMITS: RefCell<HashMap<Principal, (u64, u32)>> = RefCell::new(HashMap::new());
    // Track which players voluntarily showed cards per hand (hand_number -> seat numbers)
    static SHOWN_CARDS: RefCell<HashMap<u64, Vec<u8>>> = RefCell::new(HashMap::new());
    // Display names set by players (principal -> name)
    static DISPLAY_NAMES: RefCell<HashMap<Principal, String>> = RefCell::new(HashMap::new());
    // Heartbeat rate limiting: caller -> (last_time, count_in_window)
    static HEARTBEAT_RATE_LIMITS: RefCell<HashMap<Principal, (u64, u32)>> = RefCell::new(HashMap::new());
    // Last cleanup timestamp to throttle cleanup operations
    static LAST_CLEANUP: RefCell<u64> = RefCell::new(0);
}

// ============================================================================
// ACCESS CONTROL
// ============================================================================

fn is_controller() -> bool {
    let caller = ic_cdk::api::msg_caller();
    // Check if caller is in the controller list OR is the canister controller
    CONTROLLERS.with(|c| {
        let controllers = c.borrow();
        if controllers.is_empty() {
            // If no controllers set, only allow the canister's actual controllers
            ic_cdk::api::is_controller(&caller)
        } else {
            controllers.contains(&caller) || ic_cdk::api::is_controller(&caller)
        }
    })
}

fn require_controller() -> Result<(), String> {
    if !is_controller() {
        return Err("Unauthorized: controller access required".to_string());
    }
    Ok(())
}

/// Get the currency configuration for this table
fn get_table_currency() -> Currency {
    TABLE_CONFIG.with(|c| {
        c.borrow().as_ref().map(|cfg| cfg.currency).unwrap_or(Currency::ICP)
    })
}

fn check_rate_limit() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();

    RATE_LIMITS.with(|r| {
        let mut limits = r.borrow_mut();
        let (last_time, count) = limits.get(&caller).copied().unwrap_or((0, 0));

        if now - last_time > RATE_LIMIT_WINDOW_NS {
            // New window
            limits.insert(caller, (now, 1));
            Ok(())
        } else if count >= MAX_ACTIONS_PER_WINDOW {
            Err("Rate limit exceeded. Please wait before trying again.".to_string())
        } else {
            limits.insert(caller, (last_time, count + 1));
            Ok(())
        }
    })
}

/// Periodic cleanup of unbounded maps to prevent memory exhaustion.
/// Called from check_timeouts to run at most once per CLEANUP_INTERVAL_NS.
fn periodic_cleanup() {
    let now = ic_cdk::api::time();

    // Check if enough time has passed since last cleanup
    let should_cleanup = LAST_CLEANUP.with(|l| {
        let last = *l.borrow();
        if now > last + CLEANUP_INTERVAL_NS {
            *l.borrow_mut() = now;
            true
        } else {
            false
        }
    });

    if !should_cleanup {
        return;
    }

    // Clean up rate limit maps - remove entries older than RATE_LIMIT_CLEANUP_AGE_NS
    let cutoff = now.saturating_sub(RATE_LIMIT_CLEANUP_AGE_NS);

    RATE_LIMITS.with(|r| {
        r.borrow_mut().retain(|_, (last_time, _)| *last_time > cutoff);
    });

    HEARTBEAT_RATE_LIMITS.with(|r| {
        r.borrow_mut().retain(|_, (last_time, _)| *last_time > cutoff);
    });

    DEPOSIT_RATE_LIMITS.with(|r| {
        r.borrow_mut().retain(|_, (window_start, _)| *window_start > cutoff);
    });

    // Clean up stale PENDING_DEPOSITS — entries stuck for > 5 minutes
    // This can happen if an async verification call times out
    let pending_cutoff = now.saturating_sub(5 * 60 * 1_000_000_000); // 5 minutes in ns
    PENDING_DEPOSITS.with(|p| {
        let before = p.borrow().len();
        // PENDING_DEPOSITS doesn't store timestamps, so we clear all if map is large
        // (indicates stuck entries since normal flow clears immediately)
        if before > 50 {
            ic_cdk::println!("WARNING: Clearing {} stale pending deposits", before);
            p.borrow_mut().clear();
        }
    });

    // Clean up stale PENDING_DOGE_WITHDRAWALS
    PENDING_DOGE_WITHDRAWALS.with(|pw| {
        if pw.borrow().len() > 20 {
            ic_cdk::println!("WARNING: Clearing stale DOGE withdrawal guards");
            pw.borrow_mut().clear();
        }
    });

    // Clean up SHOWN_CARDS - keep only recent hands
    let current_hand = TABLE.with(|t| {
        t.borrow().as_ref().map(|s| s.hand_number).unwrap_or(0)
    });
    if current_hand > MAX_SHOWN_CARDS_HANDS as u64 {
        let min_hand = current_hand - MAX_SHOWN_CARDS_HANDS as u64;
        SHOWN_CARDS.with(|s| {
            s.borrow_mut().retain(|hand_num, _| *hand_num >= min_hand);
        });
    }

    // Prune HAND_HISTORY to keep only recent entries
    HAND_HISTORY.with(|h| {
        let mut history = h.borrow_mut();
        if history.len() > MAX_HAND_HISTORY_ENTRIES {
            let excess = history.len() - MAX_HAND_HISTORY_ENTRIES;
            history.drain(0..excess);
        }
    });

    // Cap VERIFIED_DEPOSITS to prevent unbounded memory growth.
    // Keep the most recent 10,000 entries. Record the highest pruned block index
    // as a watermark so pruned entries cannot be replayed via notify_deposit().
    VERIFIED_DEPOSITS.with(|v| {
        let mut deposits = v.borrow_mut();
        if deposits.len() > 10_000 {
            let mut keys: Vec<u64> = deposits.keys().copied().collect();
            keys.sort();
            let to_remove = deposits.len() - 10_000;
            let mut highest_pruned: u64 = 0;
            for key in keys.into_iter().take(to_remove) {
                deposits.remove(&key);
                if key > highest_pruned {
                    highest_pruned = key;
                }
            }
            // Update watermark — only raise it, never lower
            MIN_VERIFIED_BLOCK_INDEX.with(|m| {
                let mut min = m.borrow_mut();
                if highest_pruned > *min {
                    *min = highest_pruned;
                }
            });
        }
    });

    // Prune DISPLAY_NAMES for principals with no balance and not seated
    let seated_principals: Vec<Principal> = TABLE.with(|t| {
        let table = t.borrow();
        match table.as_ref() {
            Some(state) => state.players.iter()
                .filter_map(|p| p.as_ref().map(|p| p.principal))
                .collect(),
            None => Vec::new(),
        }
    });
    DISPLAY_NAMES.with(|d| {
        let mut names = d.borrow_mut();
        if names.len() > 200 {
            BALANCES.with(|b| {
                let balances = b.borrow();
                names.retain(|principal, _| {
                    balances.get(principal).copied().unwrap_or(0) > 0
                        || seated_principals.contains(principal)
                });
            });
        }
    });

    // Prune LAST_WITHDRAWAL entries older than the cooldown period
    LAST_WITHDRAWAL.with(|l| {
        let mut withdrawals = l.borrow_mut();
        withdrawals.retain(|_, &mut last_time| now < last_time + WITHDRAWAL_COOLDOWN_NS * 2);
    });
}

// ============================================================================
// HISTORY CANISTER INTEGRATION
// ============================================================================

/// Types for history canister (must match history_canister types)
mod history_types {
    use super::*;

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub enum HistoryPlayerAction {
        Fold,
        Check,
        Call(u64),
        Bet(u64),
        Raise(u64),
        AllIn(u64),
        PostBlind(u64),
    }

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HistoryActionRecord {
        pub seat: u8,
        pub principal: Principal,
        pub action: HistoryPlayerAction,
        pub timestamp: u64,
        pub phase: String,
    }

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HistoryPlayerHandRecord {
        pub seat: u8,
        pub principal: Principal,
        pub starting_chips: u64,
        pub ending_chips: u64,
        pub hole_cards: Option<(Card, Card)>,
        pub final_hand_rank: Option<HandRank>,
        pub amount_won: u64,
        pub position: String,
    }

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HistoryShuffleProofRecord {
        pub seed_hash: String,
        pub revealed_seed: String,
        pub timestamp: u64,
    }

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HistoryWinnerRecord {
        pub seat: u8,
        pub principal: Principal,
        pub amount: u64,
        pub hand_rank: Option<HandRank>,
        pub pot_type: String,
    }

    #[derive(Clone, Debug, CandidType, Deserialize)]
    pub struct HandHistoryRecord {
        pub hand_id: u64,
        pub table_id: Principal,
        pub hand_number: u64,
        pub timestamp: u64,
        pub small_blind: u64,
        pub big_blind: u64,
        pub ante: u64,
        pub shuffle_proof: HistoryShuffleProofRecord,
        pub players: Vec<HistoryPlayerHandRecord>,
        pub dealer_seat: u8,
        pub flop: Option<(Card, Card, Card)>,
        pub turn: Option<Card>,
        pub river: Option<Card>,
        pub actions: Vec<HistoryActionRecord>,
        pub total_pot: u64,
        pub rake: u64,
        pub winners: Vec<HistoryWinnerRecord>,
        pub went_to_showdown: bool,
    }
}

use history_types::*;

// ============================================================================
// Security: inspect_message — reject anonymous callers at ingress to prevent
// cycle drain attacks. Runs before any update call is executed.
// ============================================================================
#[ic_cdk::inspect_message]
fn inspect_message() {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        ic_cdk::trap("Anonymous calls not allowed");
    }
    ic_cdk::api::call::accept_message();
}

/// Set the history canister ID (controller only)
/// Pass None to clear/disable history recording
#[ic_cdk::update]
fn set_history_canister(canister_id: Option<Principal>) -> Result<(), String> {
    require_controller()?;
    HISTORY_ID.with(|h| {
        *h.borrow_mut() = canister_id;
    });
    Ok(())
}

/// Get the history canister ID
#[ic_cdk::query]
fn get_history_canister() -> Option<Principal> {
    HISTORY_ID.with(|h| *h.borrow())
}

/// Set a custom display name (visible to all players)
/// Name must be 1-12 characters, alphanumeric with some symbols allowed
#[ic_cdk::update]
fn set_display_name(name: Option<String>) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();

    // Don't allow anonymous
    if caller == Principal::anonymous() {
        return Err("Anonymous users cannot set display names".to_string());
    }

    match name {
        Some(n) => {
            // Validate name
            let trimmed = n.trim();
            if trimmed.is_empty() {
                return Err("Name cannot be empty".to_string());
            }
            if trimmed.len() > 12 {
                return Err("Name must be 12 characters or less".to_string());
            }
            // Only allow ASCII alphanumeric and some safe symbols
            // Using is_ascii_* to prevent Unicode homoglyph attacks (e.g., Cyrillic 'а' looks like Latin 'a')
            if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ' ') {
                return Err("Name can only contain ASCII letters, numbers, spaces, underscores and hyphens".to_string());
            }

            DISPLAY_NAMES.with(|names| {
                names.borrow_mut().insert(caller, trimmed.to_string());
            });
        }
        None => {
            // Clear the display name
            DISPLAY_NAMES.with(|names| {
                names.borrow_mut().remove(&caller);
            });
        }
    }

    Ok(())
}

/// Get a player's display name
#[ic_cdk::query]
fn get_display_name(principal: Principal) -> Option<String> {
    DISPLAY_NAMES.with(|names| {
        names.borrow().get(&principal).cloned()
    })
}

/// Record a completed hand to the history canister (fire and forget)
fn record_hand_to_history(state: &TableState, winners: &[Winner], went_to_showdown: bool) {
    let history_id = match HISTORY_ID.with(|h| *h.borrow()) {
        Some(id) => id,
        None => return, // No history canister configured, skip recording
    };
    let table_id = ic_cdk::api::canister_self();

    // Get the shuffle proof
    let shuffle_proof = match &state.shuffle_proof {
        Some(proof) => {
            let revealed_seed = HAND_HISTORY.with(|h| {
                h.borrow().last()
                    .and_then(|hh| hh.shuffle_proof.revealed_seed.clone())
                    .unwrap_or_default()
            });
            HistoryShuffleProofRecord {
                seed_hash: proof.seed_hash.clone(),
                revealed_seed,
                timestamp: proof.timestamp,
            }
        }
        None => return, // No proof, don't record
    };

    // Build player records
    let players: Vec<HistoryPlayerHandRecord> = state.players.iter()
        .enumerate()
        .filter_map(|(i, p_opt)| {
            p_opt.as_ref().map(|p| {
                let starting = STARTING_CHIPS.with(|s| {
                    s.borrow().get(&(i as u8)).copied().unwrap_or(p.chips)
                });
                let amount_won = winners.iter()
                    .filter(|w| w.seat == i as u8)
                    .map(|w| w.amount)
                    .sum();

                // Determine position string
                let position = if i as u8 == state.dealer_seat {
                    "BTN".to_string()
                } else if i as u8 == state.small_blind_seat {
                    "SB".to_string()
                } else if i as u8 == state.big_blind_seat {
                    "BB".to_string()
                } else {
                    format!("Seat {}", i)
                };

                // Only include hole cards if shown (at showdown or voluntarily)
                let show_cards = went_to_showdown && !p.has_folded;

                HistoryPlayerHandRecord {
                    seat: i as u8,
                    principal: p.principal,
                    starting_chips: starting,
                    ending_chips: p.chips,
                    hole_cards: if show_cards { p.hole_cards } else { None },
                    final_hand_rank: if show_cards {
                        p.hole_cards.as_ref().map(|cards| evaluate_hand(cards, &state.community_cards))
                    } else {
                        None
                    },
                    amount_won,
                    position,
                }
            })
        })
        .collect();

    // Build action records with phase info
    let actions: Vec<HistoryActionRecord> = CURRENT_ACTIONS.with(|a| {
        a.borrow().iter().map(|action| {
            // Convert action to history format, using stored amount for Call/AllIn
            let hist_action = match &action.action {
                PlayerAction::Fold => HistoryPlayerAction::Fold,
                PlayerAction::Check => HistoryPlayerAction::Check,
                PlayerAction::Call => HistoryPlayerAction::Call(action.amount),
                PlayerAction::Bet(amt) => HistoryPlayerAction::Bet(*amt),
                PlayerAction::Raise(amt) => HistoryPlayerAction::Raise(*amt),
                PlayerAction::AllIn => HistoryPlayerAction::AllIn(action.amount),
            };

            HistoryActionRecord {
                seat: action.seat,
                principal: state.players.get(action.seat as usize)
                    .and_then(|p| p.as_ref())
                    .map(|p| p.principal)
                    .unwrap_or(Principal::anonymous()),
                action: hist_action,
                timestamp: action.timestamp,
                phase: action.phase.clone(),
            }
        }).collect()
    });

    // Build winner records
    let history_winners: Vec<HistoryWinnerRecord> = winners.iter().map(|w| {
        HistoryWinnerRecord {
            seat: w.seat,
            principal: w.principal,
            amount: w.amount,
            hand_rank: w.hand_rank.clone(),
            pot_type: "main".to_string(),
        }
    }).collect();

    // Build flop/turn/river
    let flop = if state.community_cards.len() >= 3 {
        Some((state.community_cards[0], state.community_cards[1], state.community_cards[2]))
    } else {
        None
    };
    let turn = state.community_cards.get(3).copied();
    let river = state.community_cards.get(4).copied();

    // Calculate total pot from what winners received
    let total_pot: u64 = winners.iter().map(|w| w.amount).sum();

    let record = HandHistoryRecord {
        hand_id: 0, // Will be assigned by history canister
        table_id,
        hand_number: state.hand_number,
        timestamp: state.shuffle_proof.as_ref().map(|p| p.timestamp).unwrap_or(0),
        small_blind: state.config.small_blind,
        big_blind: state.config.big_blind,
        ante: state.config.ante,
        shuffle_proof,
        players,
        dealer_seat: state.dealer_seat,
        flop,
        turn,
        river,
        actions,
        total_pot,
        rake: 0,
        winners: history_winners,
        went_to_showdown,
    };

    // Async call to history canister - best effort but log errors
    ic_cdk::futures::spawn(async move {
        let call_result = ic_cdk::call::Call::bounded_wait(history_id, "record_hand")
            .with_arg(record)
            .await;

        // Log errors for debugging but don't fail the hand
        match call_result {
            Ok(response) => {
                match response.candid::<(Result<u64, String>,)>() {
                    Ok((Ok(_hand_id),)) => {
                        // Success - history recorded
                    }
                    Ok((Err(e),)) => {
                        ic_cdk::println!("History canister rejected record: {}", e);
                    }
                    Err(e) => {
                        ic_cdk::println!("Failed to decode history response: {:?}", e);
                    }
                }
            }
            Err(e) => {
                ic_cdk::println!("Failed to call history canister: {:?}", e);
            }
        }
    });
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert GamePhase to string for action records
fn phase_to_string(phase: &GamePhase) -> String {
    match phase {
        GamePhase::WaitingForPlayers => "waiting".to_string(),
        GamePhase::PreFlop => "preflop".to_string(),
        GamePhase::Flop => "flop".to_string(),
        GamePhase::Turn => "turn".to_string(),
        GamePhase::River => "river".to_string(),
        GamePhase::Showdown => "showdown".to_string(),
        GamePhase::HandComplete => "complete".to_string(),
    }
}

/// Find the seat that is first clockwise from the dealer among a set of seats
/// In poker, pot remainders go to the first player clockwise from the button
fn first_clockwise_from_dealer(dealer_seat: u8, seats: &[u8], num_seats: usize) -> u8 {
    if seats.is_empty() {
        return 0;
    }
    if seats.len() == 1 {
        return seats[0];
    }

    // Start from the seat after the dealer and go clockwise
    for offset in 1..=num_seats {
        let check_seat = ((dealer_seat as usize + offset) % num_seats) as u8;
        if seats.contains(&check_seat) {
            return check_seat;
        }
    }

    // Fallback (shouldn't happen)
    seats[0]
}

// ============================================================================
// LEDGER INTEGRATION - Real Money Play
// ============================================================================

/// Get the canister's own principal (for receiving deposits)
fn canister_id() -> Principal {
    ic_cdk::api::canister_self()
}

/// Transfer tokens (ICP/ckBTC/ckETH) from canister to a player (for withdrawals/payouts)
/// Uses the table's configured currency. DOGE uses on-chain withdrawal, not this function.
async fn transfer_tokens(to: Principal, amount: u64) -> Result<u64, String> {
    use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};

    let currency = get_table_currency();

    if currency == Currency::DOGE {
        return Err("DOGE withdrawals use withdraw_doge endpoint, not ledger transfers".to_string());
    }

    let fee = currency.transfer_fee();

    if amount <= fee {
        return Err(format!("Amount too small to cover {} transfer fee", currency.symbol()));
    }

    let ledger_id = currency.ledger_canister();

    let transfer_args = TransferArg {
        from_subaccount: None,
        to: Account {
            owner: to,
            subaccount: None,
        },
        fee: Some(Nat::from(fee)),
        created_at_time: Some(ic_cdk::api::time()),
        memo: None,
        amount: Nat::from(amount - fee), // Deduct fee from amount
    };

    // Use ic_cdk::call which handles Candid encoding/decoding properly
    let result: Result<(Result<Nat, TransferError>,), _> =
        ic_cdk::call(ledger_id, "icrc1_transfer", (transfer_args,)).await;

    match result {
        Ok((Ok(block_index),)) => Ok(block_index.0.try_into().unwrap_or(0)),
        Ok((Err(e),)) => Err(format!("{} transfer failed: {:?}", currency.symbol(), e)),
        Err((code, msg)) => Err(format!("Call to {} ledger failed: {:?} - {}", currency.symbol(), code, msg)),
    }
}

/// Verify and credit a deposit by checking the ledger transaction
/// Players should first transfer ICP to the canister's account, then call this with the block index
#[ic_cdk::update]
async fn notify_deposit(block_index: u64) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot deposit".to_string());
    }
    let canister = canister_id();
    let now = ic_cdk::api::time();

    // Rate limit deposit verifications (5 per minute per user)
    let rate_limited = DEPOSIT_RATE_LIMITS.with(|r| {
        let mut limits = r.borrow_mut();
        let minute_ns: u64 = 60_000_000_000;

        if let Some((window_start, count)) = limits.get_mut(&caller) {
            if now > *window_start + minute_ns {
                // New window
                *window_start = now;
                *count = 1;
                false
            } else if *count >= MAX_DEPOSIT_VERIFICATIONS_PER_MINUTE {
                true // Rate limited
            } else {
                *count += 1;
                false
            }
        } else {
            limits.insert(caller, (now, 1));
            false
        }
    });

    if rate_limited {
        return Err("Too many deposit verification attempts. Please wait a minute.".to_string());
    }

    // Reject block indices at or below the pruning watermark.
    // These were previously verified but pruned for memory; replaying them would grant free funds.
    let below_watermark = MIN_VERIFIED_BLOCK_INDEX.with(|m| {
        block_index <= *m.borrow()
    });

    if below_watermark {
        return Err("This deposit has already been credited".to_string());
    }

    // Check if this block was already processed
    let already_processed = VERIFIED_DEPOSITS.with(|v| {
        v.borrow().contains_key(&block_index)
    });

    if already_processed {
        return Err("This deposit has already been credited".to_string());
    }

    // Check if this block is currently being verified (prevent race condition)
    let already_pending = PENDING_DEPOSITS.with(|p| {
        let mut pending = p.borrow_mut();
        if pending.contains_key(&block_index) {
            true
        } else {
            // Mark as pending before the async call
            pending.insert(block_index, caller);
            false
        }
    });

    if already_pending {
        return Err("This deposit is currently being verified".to_string());
    }

    // Helper to clear pending state on any exit path
    let clear_pending = || {
        PENDING_DEPOSITS.with(|p| {
            p.borrow_mut().remove(&block_index);
        });
    };

    // Query the ledger to verify the transfer
    let currency = get_table_currency();

    // DOGE uses direct deposit detection, not ledger-based notify_deposit
    if currency == Currency::DOGE {
        clear_pending();
        return Err("DOGE deposits are detected automatically. Use check_doge_deposit instead.".to_string());
    }

    let ledger_id = currency.ledger_canister();

    // For BTC (ckBTC) and ETH (ckETH), use ICRC-3 verification method
    if currency == Currency::BTC || currency == Currency::ETH {
        clear_pending();
        return verify_icrc_deposit(block_index, caller, canister, &currency).await;
    }

    // For ICP, use query_blocks (ICP ledger API)

    // ICP Ledger types for query_blocks
    #[derive(CandidType, Deserialize, Debug)]
    struct GetBlocksArgs {
        start: u64,
        length: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Tokens {
        e8s: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct AccountIdentifier {
        hash: Vec<u8>,
    }

    // TimeStamp is a record with timestamp_nanos field (defined first for use in Approve)
    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct TimeStamp {
        timestamp_nanos: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Transfer {
        from: AccountIdentifier,
        to: AccountIdentifier,
        amount: Tokens,
        fee: Tokens,
        spender: Option<Vec<u8>>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Mint {
        to: AccountIdentifier,
        amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Burn {
        from: AccountIdentifier,
        spender: Option<AccountIdentifier>,
        amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Approve {
        from: AccountIdentifier,
        spender: AccountIdentifier,
        allowance_e8s: i128,
        allowance: Tokens,
        fee: Tokens,
        expires_at: Option<TimeStamp>,
        expected_allowance: Option<Tokens>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    enum Operation {
        Transfer(Transfer),
        Mint(Mint),
        Burn(Burn),
        Approve(Approve),
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Transaction {
        memo: u64,
        icrc1_memo: Option<Vec<u8>>,
        operation: Option<Operation>,
        created_at_time: TimeStamp,
    }

    #[derive(CandidType, Deserialize, Debug)]
    struct Block {
        parent_hash: Option<Vec<u8>>,
        transaction: Transaction,
        timestamp: TimeStamp,
    }

    // Simplified response - we only need the blocks field
    // Using candid::Reserved for archived_blocks since Func types need special handling
    #[derive(CandidType, Deserialize, Debug)]
    struct QueryBlocksResponse {
        chain_length: u64,
        certificate: Option<Vec<u8>>,
        blocks: Vec<Block>,
        first_block_index: u64,
        // Use Reserved to skip deserializing the complex Func type in archived_blocks
        archived_blocks: candid::Reserved,
    }

    // Compute expected destination account identifier (this canister's default account)
    let expected_to = compute_account_identifier(&canister, None);

    let request = GetBlocksArgs {
        start: block_index,
        length: 1,
    };

    let call_result = ic_cdk::call::Call::bounded_wait(ledger_id, "query_blocks")
        .with_arg(request)
        .await;

    let response = match call_result {
        Ok(response) => match response.candid::<(QueryBlocksResponse,)>() {
            Ok((r,)) => r,
            Err(e) => {
                clear_pending();
                return Err(format!("Failed to decode ledger response: {:?}", e));
            }
        },
        Err(e) => {
            clear_pending();
            return Err(format!("Failed to query ledger: {:?}", e));
        }
    };

    // Helper function to verify and credit a transfer
    let verify_and_credit = |transfer: &Transfer| -> Result<u64, String> {
        // Verify the transfer was TO this canister
        if transfer.to.hash.len() != 32 {
            return Err("Invalid destination account".to_string());
        }
        let to_bytes: [u8; 32] = transfer.to.hash.clone().try_into()
            .map_err(|_| "Invalid destination account length")?;

        if to_bytes != expected_to {
            return Err("Transfer was not to this canister".to_string());
        }

        // Verify the sender is the caller by computing their expected account identifier.
        // Account identifiers are deterministic: SHA224(domain || principal || subaccount).
        let expected_from = compute_account_identifier(&caller, None);
        let from_bytes: [u8; 32] = transfer.from.hash.clone().try_into()
            .map_err(|_| "Invalid source account length".to_string())?;
        if from_bytes != expected_from {
            return Err("This transfer was not sent from your account. Only the sender can claim their deposit.".to_string());
        }

        let amount = transfer.amount.e8s;

        // Mark this deposit as processed
        VERIFIED_DEPOSITS.with(|v| {
            v.borrow_mut().insert(block_index, caller);
        });

        // Credit the player's escrow balance (with overflow protection)
        let new_balance = BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            let current = balances.get(&caller).copied().unwrap_or(0);
            let new_balance = current.saturating_add(amount);
            balances.insert(caller, new_balance);
            new_balance
        });

        Ok(new_balance)
    };

    // Check if we got the block directly
    if !response.blocks.is_empty() {
        let block = &response.blocks[0];
        let result = if let Some(Operation::Transfer(ref transfer)) = block.transaction.operation {
            verify_and_credit(transfer)
        } else {
            Err("Transaction is not a transfer".to_string())
        };
        clear_pending();
        return result;
    }

    // Note: archived_blocks handling removed - blocks at 33M+ should not be archived yet
    // If the block is archived, we'd need to query the archive canister separately
    clear_pending();
    Err("Transaction not found at this block index (may be archived)".to_string())
}


/// Deposit ICP using ICRC-2 transfer_from (seamless flow)
/// User must first approve this canister to spend their ICP via icrc2_approve on the ledger
/// Then call this function to pull the approved amount
#[ic_cdk::update]
async fn deposit(amount: u64) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot deposit".to_string());
    }
    let canister = canister_id();
    let currency = get_table_currency();

    if amount == 0 {
        return Err("Amount must be greater than 0".to_string());
    }

    // Minimum deposit to cover potential fees (currency-aware)
    let min_deposit = match currency {
        Currency::BTC => 1_000,                    // 1000 sats
        Currency::ETH => 10_000_000_000_000,       // 0.00001 ETH
        Currency::ICP => 20_000,                   // 0.0002 ICP
        Currency::DOGE => 1_000_000,               // 0.01 DOGE
    };
    if amount < min_deposit {
        return Err(format!(
            "Minimum deposit is {}",
            currency.format_amount(min_deposit)
        ));
    }

    let ledger_id = currency.ledger_canister();
    let transfer_fee = currency.transfer_fee();

    // Use the standard ICRC-2 types from icrc_ledger_types crate
    let transfer_from_args = TransferFromArgs {
        spender_subaccount: None,
        from: Account {
            owner: caller,
            subaccount: None,
        },
        to: Account {
            owner: canister,
            subaccount: None,
        },
        amount: Nat::from(amount),
        fee: Some(Nat::from(transfer_fee)),
        memo: None,
        created_at_time: Some(ic_cdk::api::time()),
    };

    // Use ic_cdk::call which properly handles Candid encoding/decoding
    let transfer_result: Result<(Result<Nat, TransferFromError>,), _> =
        ic_cdk::call(ledger_id, "icrc2_transfer_from", (transfer_from_args,)).await;

    let transfer_result = match transfer_result {
        Ok((result,)) => result,
        Err((code, msg)) => return Err(format!("Failed to call ledger: {:?} - {}", code, msg)),
    };

    match transfer_result {
        Ok(_block_index) => {
            // Credit the player's escrow balance
            let new_balance = BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                let new_balance = current.saturating_add(amount);
                balances.insert(caller, new_balance);
                new_balance
            });

            Ok(new_balance)
        }
        Err(e) => {
            let symbol = currency.symbol();
            match e {
                TransferFromError::InsufficientAllowance { allowance } => {
                    let allowance_u64: u64 = allowance.0.try_into().unwrap_or(0);
                    Err(format!("Insufficient allowance. You approved {} but tried to deposit {}. Please approve more {} first.",
                        currency.format_amount(allowance_u64),
                        currency.format_amount(amount),
                        symbol))
                }
                TransferFromError::InsufficientFunds { balance } => {
                    let balance_u64: u64 = balance.0.try_into().unwrap_or(0);
                    Err(format!("Insufficient {} in your wallet. Balance: {}", symbol, currency.format_amount(balance_u64)))
                }
                _ => Err(format!("{} transfer failed: {:?}", symbol, e))
            }
        }
    }
}

/// Deposit from an external wallet using subaccount-based deposit address
/// Each user gets a unique deposit address: (canister_id, sha256(user_principal))
/// The external wallet transfers directly to that address, then user calls this to claim.
/// SECURITY: Only the owner of the subaccount (derived from caller's principal) can claim.
#[ic_cdk::update]
async fn claim_external_deposit() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot claim deposits".to_string());
    }
    let canister = canister_id();
    let currency = get_table_currency();
    let ledger_id = currency.ledger_canister();
    let transfer_fee = currency.transfer_fee();

    // Compute the caller's unique deposit subaccount: sha256(principal)
    let subaccount = compute_deposit_subaccount(&caller);

    // Query the balance at the caller's deposit subaccount
    let balance_result: Result<(Nat,), _> = ic_cdk::call(
        ledger_id,
        "icrc1_balance_of",
        (Account {
            owner: canister,
            subaccount: Some(subaccount),
        },),
    ).await;

    let balance: u64 = match balance_result {
        Ok((bal,)) => bal.0.try_into().unwrap_or(0),
        Err((code, msg)) => return Err(format!("Failed to query balance: {:?} - {}", code, msg)),
    };

    if balance <= transfer_fee {
        return Err(format!(
            "No claimable balance. Send {} to your deposit address first. Use get_deposit_subaccount() to get your address.",
            currency.symbol()
        ));
    }

    // Sweep: transfer from the deposit subaccount to the canister's main account
    let sweep_amount = balance - transfer_fee; // Deduct fee for the internal transfer

    let transfer_args = TransferArg {
        from_subaccount: Some(subaccount),
        to: Account {
            owner: canister,
            subaccount: None,
        },
        amount: Nat::from(sweep_amount),
        fee: Some(Nat::from(transfer_fee)),
        memo: None,
        created_at_time: Some(ic_cdk::api::time()),
    };

    let transfer_result: Result<(Result<Nat, TransferError>,), _> =
        ic_cdk::call(ledger_id, "icrc1_transfer", (transfer_args,)).await;

    let transfer_result = match transfer_result {
        Ok((result,)) => result,
        Err((code, msg)) => return Err(format!("Failed to sweep deposit: {:?} - {}", code, msg)),
    };

    match transfer_result {
        Ok(_block_index) => {
            // Credit the caller's escrow balance
            let new_balance = BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                let new_balance = current.saturating_add(sweep_amount);
                balances.insert(caller, new_balance);
                new_balance
            });

            Ok(new_balance)
        }
        Err(e) => Err(format!("Sweep transfer failed: {:?}", e)),
    }
}

/// Get the caller's unique deposit subaccount address for external wallet deposits
/// External wallets (OISY, NNS, etc.) should transfer to: (canister_id, this_subaccount)
#[ic_cdk::query]
fn get_deposit_subaccount() -> Vec<u8> {
    let caller = ic_cdk::api::msg_caller();
    compute_deposit_subaccount(&caller).to_vec()
}

/// Compute a unique deposit subaccount for a principal: sha256("cleardeck-deposit:" || principal_bytes)
fn compute_deposit_subaccount(principal: &Principal) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"cleardeck-deposit:");
    hasher.update(principal.as_slice());
    hasher.finalize().into()
}

/// Verify a ckBTC/ckETH deposit using ICRC-3 get_transactions API
async fn verify_icrc_deposit(block_index: u64, caller: Principal, canister: Principal, currency: &Currency) -> Result<u64, String> {
    let ledger_id = currency.ledger_canister();

    // ICRC-3 types for get_transactions
    #[derive(CandidType, Deserialize, Debug)]
    struct GetTransactionsRequest {
        start: Nat,
        length: Nat,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Account {
        owner: Principal,
        subaccount: Option<Vec<u8>>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Burn {
        from: Account,
        memo: Option<Vec<u8>>,
        created_at_time: Option<u64>,
        amount: Nat,
        spender: Option<Account>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Mint {
        to: Account,
        memo: Option<Vec<u8>>,
        created_at_time: Option<u64>,
        amount: Nat,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Transfer {
        from: Account,
        to: Account,
        memo: Option<Vec<u8>>,
        created_at_time: Option<u64>,
        amount: Nat,
        fee: Option<Nat>,
        spender: Option<Account>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Approve {
        from: Account,
        spender: Account,
        memo: Option<Vec<u8>>,
        created_at_time: Option<u64>,
        amount: Nat,
        fee: Option<Nat>,
        expected_allowance: Option<Nat>,
        expires_at: Option<u64>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct Transaction {
        burn: Option<Burn>,
        mint: Option<Mint>,
        transfer: Option<Transfer>,
        approve: Option<Approve>,
        timestamp: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    struct TransactionWithId {
        id: Nat,
        transaction: Transaction,
    }

    #[derive(CandidType, Deserialize, Debug)]
    struct GetTransactionsResponse {
        log_length: Nat,
        first_index: Nat,
        transactions: Vec<TransactionWithId>,
        archived_transactions: candid::Reserved,
    }

    let request = GetTransactionsRequest {
        start: Nat::from(block_index),
        length: Nat::from(1u64),
    };

    let call_result = ic_cdk::call::Call::bounded_wait(ledger_id, "get_transactions")
        .with_arg(request)
        .await;

    let response = match call_result {
        Ok(response) => match response.candid::<(GetTransactionsResponse,)>() {
            Ok((r,)) => r,
            Err(e) => return Err(format!("Failed to decode {} ledger response: {:?}", currency.symbol(), e)),
        },
        Err(e) => return Err(format!("Failed to query {} ledger: {:?}", currency.symbol(), e)),
    };

    if response.transactions.is_empty() {
        return Err("Transaction not found. It may be archived or not yet finalized.".to_string());
    }

    let tx_with_id = &response.transactions[0];
    let tx = &tx_with_id.transaction;

    // Check if this is a transfer to our canister
    let transfer = tx.transfer.as_ref()
        .ok_or("Transaction is not a transfer")?;

    // Verify destination is our canister
    if transfer.to.owner != canister {
        return Err("This transaction was not sent to this table".to_string());
    }

    // Verify sender matches caller
    if transfer.from.owner != caller {
        return Err("This transaction was not sent by you".to_string());
    }

    let amount: u64 = transfer.amount.0.clone().try_into()
        .map_err(|_| "Deposit amount exceeds maximum".to_string())?;
    if amount == 0 {
        return Err("Invalid transaction amount".to_string());
    }

    // ATOMIC: Check if already verified AND mark as verified in one operation
    // This prevents race conditions where two concurrent calls could both pass the check
    let already_verified = VERIFIED_DEPOSITS.with(|v| {
        let mut deposits = v.borrow_mut();
        // use_entry pattern for atomic check-and-insert
        use std::collections::hash_map::Entry;
        match deposits.entry(block_index) {
            Entry::Occupied(_) => true,  // Already verified
            Entry::Vacant(e) => {
                e.insert(caller);  // Mark as verified atomically
                false
            }
        }
    });

    if already_verified {
        return Err("This deposit has already been credited".to_string());
    }

    // Credit the player's escrow balance
    let new_balance = BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        let new_balance = current.saturating_add(amount);
        balances.insert(caller, new_balance);
        new_balance
    });

    Ok(new_balance)
}

// ============================================================================
// Security: CallerGuard pattern — ensures PENDING_WITHDRAWALS is always
// cleaned up via Drop, even if the async callback traps.
// ============================================================================
struct WithdrawalGuard {
    principal: Principal,
}

impl WithdrawalGuard {
    fn new(principal: Principal) -> Result<Self, String> {
        PENDING_WITHDRAWALS.with(|pw| {
            if pw.borrow().contains_key(&principal) {
                return Err("A withdrawal is already in progress".to_string());
            }
            Ok(())
        })?;
        Ok(WithdrawalGuard { principal })
    }

    fn set_amount(&self, amount: u64) {
        PENDING_WITHDRAWALS.with(|pw| {
            pw.borrow_mut().insert(self.principal, amount);
        });
    }
}

impl Drop for WithdrawalGuard {
    fn drop(&mut self) {
        PENDING_WITHDRAWALS.with(|pw| {
            pw.borrow_mut().remove(&self.principal);
        });
    }
}

struct DogeWithdrawalGuard {
    principal: Principal,
}

impl DogeWithdrawalGuard {
    fn new(principal: Principal) -> Result<Self, String> {
        PENDING_DOGE_WITHDRAWALS.with(|pw| {
            if pw.borrow().contains(&principal) {
                return Err("A DOGE withdrawal is already in progress".to_string());
            }
            pw.borrow_mut().insert(principal);
            Ok(())
        })?;
        Ok(DogeWithdrawalGuard { principal })
    }
}

impl Drop for DogeWithdrawalGuard {
    fn drop(&mut self) {
        PENDING_DOGE_WITHDRAWALS.with(|pw| {
            pw.borrow_mut().remove(&self.principal);
        });
    }
}

/// Withdraw your balance from the table
#[ic_cdk::update]
async fn withdraw(amount: u64) -> Result<u64, String> {
    check_rate_limit()?;
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot withdraw".to_string());
    }
    let now = ic_cdk::api::time();
    let currency = get_table_currency();

    // Validate withdrawal amount limits (currency-aware)
    let min_withdrawal = currency.min_withdrawal();
    let max_withdrawal = currency.max_withdrawal();

    if amount < min_withdrawal {
        return Err(format!(
            "Minimum withdrawal is {}",
            currency.format_amount(min_withdrawal)
        ));
    }
    if amount > max_withdrawal {
        return Err(format!(
            "Maximum withdrawal per transaction is {}",
            currency.format_amount(max_withdrawal)
        ));
    }

    // Check withdrawal cooldown
    let last_withdrawal = LAST_WITHDRAWAL.with(|l| {
        l.borrow().get(&caller).copied()
    });
    if let Some(last_time) = last_withdrawal {
        if now < last_time + WITHDRAWAL_COOLDOWN_NS {
            let remaining_secs = (last_time + WITHDRAWAL_COOLDOWN_NS - now) / 1_000_000_000;
            return Err(format!("Please wait {} seconds before withdrawing again", remaining_secs));
        }
    }

    // Acquire withdrawal guard — prevents reentrancy via Drop trait
    let _guard = WithdrawalGuard::new(caller)?;

    // Check if player is in a hand (can't withdraw during play)
    let in_hand = TABLE.with(|t| {
        let table = t.borrow();
        if let Some(state) = table.as_ref() {
            if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
                // Check if this player is in the current hand
                return state.players.iter().flatten()
                    .any(|p| p.principal == caller && !p.has_folded);
            }
        }
        false
    });

    if in_hand {
        return Err("Cannot withdraw while in a hand".to_string());
    }

    // ATOMIC: Check balance and deduct in single critical section
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current_balance = balances.get(&caller).copied().unwrap_or(0);

        if amount > current_balance {
            let currency = get_table_currency();
            return Err(format!("Insufficient balance. Have: {}, requested: {}",
                currency.format_amount(current_balance),
                currency.format_amount(amount)));
        }

        // Deduct immediately while holding the lock
        balances.insert(caller, current_balance.saturating_sub(amount));
        Ok(())
    })?;

    // Mark pending amount (for tracking; guard handles cleanup via Drop)
    _guard.set_amount(amount);

    // Transfer to player's wallet
    let result = transfer_tokens(caller, amount).await;
    // Note: _guard is dropped automatically here (or on any exit path),
    // cleaning up PENDING_WITHDRAWALS even if the callback traps.

    match result {
        Ok(block) => {
            // Record successful withdrawal time for cooldown
            LAST_WITHDRAWAL.with(|l| {
                l.borrow_mut().insert(caller, now);
            });
            Ok(block)
        }
        Err(e) => {
            // Refund the escrow if transfer failed (with overflow protection)
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(amount));
            });
            Err(e)
        }
    }
}

/// Get your current escrow balance
#[ic_cdk::query]
fn get_balance() -> u64 {
    let caller = ic_cdk::api::msg_caller();
    BALANCES.with(|b| {
        b.borrow().get(&caller).copied().unwrap_or(0)
    })
}

/// Compute Account Identifier from principal and subaccount
/// This creates the 32-byte address format used by NNS and other wallets
fn compute_account_identifier(principal: &Principal, subaccount: Option<[u8; 32]>) -> [u8; 32] {
    // ICP Account Identifier = CRC32(hash) || hash
    // where hash = SHA224("\x0Aaccount-id" || principal || subaccount)
    let mut hasher = Sha224::new();
    hasher.update(b"\x0Aaccount-id");
    hasher.update(principal.as_slice());
    hasher.update(subaccount.unwrap_or([0u8; 32]));
    let hash = hasher.finalize(); // 28 bytes

    // Prepend CRC32 checksum (4 bytes) to get 32 bytes total
    let crc = crc32fast::hash(&hash);
    let mut result = [0u8; 32];
    result[0..4].copy_from_slice(&crc.to_be_bytes());
    result[4..32].copy_from_slice(&hash);
    result
}

/// Get the canister's account identifier for deposits (hex format for NNS wallet)
#[ic_cdk::query]
fn get_deposit_address() -> String {
    let account_id = compute_account_identifier(&canister_id(), None);
    hex::encode(account_id)
}

/// DEV ONLY: Get free test chips for local development
/// Disabled when dev_mode is false (production)
#[ic_cdk::update]
fn dev_faucet(_amount: u64) -> Result<u64, String> {
    // Dev faucet has been permanently disabled for production safety
    // Players must deposit real ICP via notify_deposit
    Err("Dev faucet is disabled. Please deposit ICP to fund your account.".to_string())
}

/// Buy into the table using your escrow balance
#[ic_cdk::update]
fn buy_in(seat: u8, amount: u64) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    // Check escrow balance
    let balance = BALANCES.with(|b| {
        b.borrow().get(&caller).copied().unwrap_or(0)
    });

    let currency = get_table_currency();
    if amount > balance {
        return Err(format!("Insufficient escrow balance. Have: {}, need: {}",
            currency.format_amount(balance),
            currency.format_amount(amount)));
    }

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;
        let currency = state.config.currency;

        // Validate buy-in amount
        if amount < state.config.min_buy_in {
            return Err(format!("Minimum buy-in is {}", currency.format_amount(state.config.min_buy_in)));
        }
        if amount > state.config.max_buy_in {
            return Err(format!("Maximum buy-in is {}", currency.format_amount(state.config.max_buy_in)));
        }

        // Check seat
        if seat as usize >= state.players.len() {
            return Err("Invalid seat".to_string());
        }
        if state.players[seat as usize].is_some() {
            return Err("Seat is taken".to_string());
        }

        // Check not already at table
        for p in state.players.iter().flatten() {
            if p.principal == caller {
                return Err("Already at table".to_string());
            }
        }

        // Deduct from escrow
        BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            balances.insert(caller, balance.saturating_sub(amount));
        });

        // BUGFIX: If joining mid-hand, player must sit out until next hand
        // This prevents them from corrupting action order and pot logic
        let joining_during_hand = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;
        let initial_status = if joining_during_hand {
            PlayerStatus::SittingOut
        } else {
            PlayerStatus::Active
        };

        // Add player to table with chips
        let now = ic_cdk::api::time();
        let time_bank = state.config.time_bank_secs;
        let sitting_out_since = if initial_status == PlayerStatus::SittingOut { Some(now) } else { None };
        state.players[seat as usize] = Some(Player {
            principal: caller,
            seat,
            chips: amount,  // Chips = buy-in amount in e8s
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: 0,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: false,
            status: initial_status,
            last_seen: now,
            timeout_count: 0,
            time_bank_remaining: time_bank,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since,
        });

        // Auto-start if we now have enough players and waiting for players
        if state.phase == GamePhase::WaitingForPlayers {
            let active_count = state.players.iter()
                .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
                .count();

            if active_count >= 2 && state.auto_deal_at.is_none() {
                // Schedule auto-deal to start the game
                state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
            }
        }

        Ok(())
    })
}

/// Reload chips from escrow (for players already seated who need more chips)
/// Can only be done between hands, not during active play
#[ic_cdk::update]
fn reload(amount: u64) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    let currency = get_table_currency();
    // Check escrow balance (atomic check and deduct)
    let deducted = BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let balance = balances.get(&caller).copied().unwrap_or(0);
        if amount > balance {
            return Err(format!("Insufficient escrow balance. Have: {}, need: {}",
                currency.format_amount(balance),
                currency.format_amount(amount)));
        }
        balances.insert(caller, balance.saturating_sub(amount));
        Ok(amount)
    })?;

    // Add chips to player and clear broke status
    let result = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Can't reload during a hand
        if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
            return Err("Cannot reload during a hand".to_string());
        }

        // Find the player
        let player = state.players.iter_mut()
            .flatten()
            .find(|p| p.principal == caller)
            .ok_or("Not at table")?;

        // Check max buy-in limit (with overflow protection)
        let new_total = player.chips.saturating_add(amount);
        let currency = state.config.currency;
        if new_total > state.config.max_buy_in {
            return Err(format!("Reload would exceed max buy-in of {}. Current chips: {}",
                currency.format_amount(state.config.max_buy_in),
                currency.format_amount(player.chips)));
        }

        // Add chips and clear broke status (with overflow protection)
        player.chips = new_total;
        player.broke_at = None;

        // If they were sitting out, bring them back to active
        if player.status == PlayerStatus::SittingOut {
            player.status = PlayerStatus::Active;
        }

        Ok(player.chips)
    });

    // If the table operation failed, refund the escrow (with overflow protection)
    if result.is_err() {
        BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            let current = balances.get(&caller).copied().unwrap_or(0);
            balances.insert(caller, current.saturating_add(deducted));
        });
    }

    result
}

/// Cash out and leave the table
#[ic_cdk::update]
fn cash_out() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    // Check if player is in a hand
    let in_hand = TABLE.with(|t| {
        let table = t.borrow();
        if let Some(state) = table.as_ref() {
            if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
                return state.players.iter().flatten()
                    .any(|p| p.principal == caller && !p.has_folded);
            }
        }
        false
    });

    if in_hand {
        return Err("Cannot cash out while in a hand".to_string());
    }

    // Find player and get their chips
    let chips = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        for (i, player_opt) in state.players.iter_mut().enumerate() {
            if let Some(player) = player_opt {
                if player.principal == caller {
                    let chips = player.chips;
                    state.players[i] = None;  // Remove from table
                    return Ok(chips);
                }
            }
        }

        Err("Not at table".to_string())
    })?;

    // Return chips to escrow balance (with overflow protection)
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        balances.insert(caller, current.saturating_add(chips));
    });

    Ok(chips)
}

// ============================================================================
// INITIALIZATION
// ============================================================================

#[ic_cdk::init]
fn init(config: TableConfig) {
    init_table_state(config);
}

/// Reset the table (controller only) - CAUTION: destroys all state
#[ic_cdk::update]
fn reset_table(config: TableConfig) -> Result<(), String> {
    require_controller()?;
    validate_config(&config)?;
    init_table_state(config);
    Ok(())
}

/// Set dev mode (controller only)
#[ic_cdk::update]
fn set_dev_mode(_enabled: bool) -> Result<(), String> {
    // Dev mode has been permanently disabled for production safety
    Err("Dev mode is no longer supported".to_string())
}

/// Check if dev mode is enabled (always returns false now)
#[ic_cdk::query]
fn is_dev_mode() -> bool {
    false // Dev mode permanently disabled
}

/// Add a controller (controller only)
#[ic_cdk::update]
fn add_controller(principal: Principal) -> Result<(), String> {
    require_controller()?;
    CONTROLLERS.with(|c| {
        let mut controllers = c.borrow_mut();
        if !controllers.contains(&principal) {
            controllers.push(principal);
        }
    });
    Ok(())
}

/// Remove a controller (controller only)
#[ic_cdk::update]
fn remove_controller(principal: Principal) -> Result<(), String> {
    require_controller()?;
    CONTROLLERS.with(|c| {
        c.borrow_mut().retain(|p| p != &principal);
    });
    Ok(())
}

/// Get all controllers
#[ic_cdk::query]
fn get_controllers() -> Vec<Principal> {
    CONTROLLERS.with(|c| c.borrow().clone())
}

/// Admin: Update table configuration (stakes, buy-ins, etc.)
/// Controller only - can only be done when no hand is in progress
#[ic_cdk::update]
fn admin_update_config(new_config: TableConfig) -> Result<TableConfig, String> {
    require_controller()?;

    // Validate the new config
    validate_config(&new_config)?;

    TABLE.with(|table| {
        let mut table = table.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Only allow config updates when not in the middle of a hand
        match state.phase {
            GamePhase::WaitingForPlayers | GamePhase::HandComplete => {
                // Safe to update
                state.config = new_config.clone();
                Ok(new_config)
            }
            _ => {
                Err("Cannot update config while a hand is in progress".to_string())
            }
        }
    })
}

/// Admin: Check balance for a specific player
/// Controller only
#[ic_cdk::query]
fn admin_get_balance(player: Principal) -> Result<u64, String> {
    require_controller()?;

    BALANCES.with(|b| {
        let balances = b.borrow();
        Ok(balances.get(&player).copied().unwrap_or(0))
    })
}

// REMOVED: admin_restore_balance - unnecessary attack surface.
// A compromised controller key could mint arbitrary balances and withdraw real funds.
// If balance recovery is needed, redeploy with a migration in post_upgrade.

/// Admin: Get all balances (for auditing/recovery)
/// Returns (total_assigned, list of (principal, balance))
/// Controller only
#[ic_cdk::query]
fn admin_get_all_balances() -> Result<(u64, Vec<(Principal, u64)>), String> {
    require_controller()?;

    BALANCES.with(|b| {
        let balances = b.borrow();
        let list: Vec<(Principal, u64)> = balances.iter().map(|(k, v)| (*k, *v)).collect();
        let total: u64 = balances.values().fold(0u64, |acc, &v| acc.saturating_add(v));
        Ok((total, list))
    })
}

/// Admin: Get total chips at table (for auditing)
/// Returns total chips held by seated players
/// Controller only
#[ic_cdk::query]
fn admin_get_table_chips() -> Result<u64, String> {
    require_controller()?;

    TABLE.with(|t| {
        let table = t.borrow();
        match &*table {
            Some(state) => {
                let total: u64 = state.players.iter()
                    .filter_map(|p| p.as_ref())
                    .map(|p| p.chips)
                    .fold(0u64, |acc, v| acc.saturating_add(v));
                Ok(total)
            }
            None => Ok(0)
        }
    })
}

/// Admin: Re-initialize the table (for recovery after upgrade issues)
/// Controller only
#[ic_cdk::update]
fn admin_reinit_table(config: TableConfig) -> Result<(), String> {
    require_controller()?;

    // Validate config
    validate_config(&config)?;

    // Initialize the table
    init_table_state(config);

    Ok(())
}

/// Validate table configuration parameters
fn validate_config(config: &TableConfig) -> Result<(), String> {
    // Validate player count
    if config.max_players < 2 {
        return Err("max_players must be at least 2".to_string());
    }
    if config.max_players > 10 {
        return Err("max_players cannot exceed 10".to_string());
    }

    // Validate blinds
    if config.small_blind == 0 {
        return Err("small_blind must be greater than 0".to_string());
    }
    if config.big_blind == 0 {
        return Err("big_blind must be greater than 0".to_string());
    }
    if config.small_blind > config.big_blind {
        return Err("small_blind cannot be greater than big_blind".to_string());
    }
    // Standard poker: big blind should be 2x small blind (but allow flexibility)
    if config.big_blind > config.small_blind * 10 {
        return Err("big_blind cannot be more than 10x small_blind".to_string());
    }

    // Validate buy-in
    if config.min_buy_in == 0 {
        return Err("min_buy_in must be greater than 0".to_string());
    }
    if config.max_buy_in < config.min_buy_in {
        return Err("max_buy_in must be >= min_buy_in".to_string());
    }
    // Standard poker: min buy-in should be at least 20 big blinds
    if config.min_buy_in < config.big_blind * 10 {
        return Err("min_buy_in should be at least 10 big blinds".to_string());
    }
    // Max buy-in sanity check (1000 big blinds)
    if config.max_buy_in > config.big_blind * 1000 {
        return Err("max_buy_in cannot exceed 1000 big blinds".to_string());
    }

    // Validate timeouts (reasonable bounds)
    if config.action_timeout_secs > 300 {
        return Err("action_timeout_secs cannot exceed 300 (5 minutes)".to_string());
    }
    if config.time_bank_secs > 600 {
        return Err("time_bank_secs cannot exceed 600 (10 minutes)".to_string());
    }

    // Validate ante (should be less than big blind)
    if config.ante > config.big_blind {
        return Err("ante cannot exceed big_blind".to_string());
    }

    Ok(())
}

fn init_table_state(config: TableConfig) {
    // Validate config first
    if let Err(e) = validate_config(&config) {
        ic_cdk::println!("WARNING: Invalid table config: {}. Using defaults where needed.", e);
    }

    // Apply defaults for optional config fields
    let config = TableConfig {
        small_blind: config.small_blind,
        big_blind: config.big_blind,
        min_buy_in: config.min_buy_in,
        max_buy_in: config.max_buy_in,
        max_players: config.max_players,
        action_timeout_secs: if config.action_timeout_secs == 0 {
            DEFAULT_ACTION_TIMEOUT_SECS
        } else {
            config.action_timeout_secs
        },
        ante: config.ante, // 0 means no ante
        time_bank_secs: if config.time_bank_secs == 0 {
            DEFAULT_TIME_BANK_SECS
        } else {
            config.time_bank_secs
        },
        currency: config.currency, // ICP or BTC
    };

    // Store config separately so get_max_players works before first hand
    TABLE_CONFIG.with(|c| {
        *c.borrow_mut() = Some(config.clone());
    });

    let players = (0..config.max_players).map(|_| None).collect();

    TABLE.with(|t| {
        *t.borrow_mut() = Some(TableState {
            id: 0,
            config,
            players,
            community_cards: Vec::new(),
            deck: Vec::new(),
            deck_index: 0,
            pot: 0,
            side_pots: Vec::new(),
            current_bet: 0,
            min_raise: 0,
            phase: GamePhase::WaitingForPlayers,
            dealer_seat: 0,
            small_blind_seat: 0,
            big_blind_seat: 0,
            action_on: 0,
            action_timer: None,
            shuffle_proof: None,
            hand_number: 0,
            last_aggressor: None,
            bb_has_option: false,
            first_hand: true, // Track first hand for dealer button init
            auto_deal_at: None,
            last_action: None,
        });
    });

    // Clear history
    HAND_HISTORY.with(|h| h.borrow_mut().clear());
    CURRENT_ACTIONS.with(|a| a.borrow_mut().clear());
    SHOWN_CARDS.with(|s| s.borrow_mut().clear());
}

// ============================================================================
// DECK & SHUFFLING
// ============================================================================

/// Creates a standard 52-card deck in a fixed order (Hearts, Diamonds, Clubs, Spades)
/// Each suit contains cards 2-A in ascending order
fn create_deck() -> Vec<Card> {
    let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
    let ranks = [
        Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six,
        Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten,
        Rank::Jack, Rank::Queen, Rank::King, Rank::Ace,
    ];

    let mut deck = Vec::with_capacity(52);
    for suit in suits {
        for rank in ranks {
            deck.push(Card { suit, rank });
        }
    }
    deck
}

/// Shuffles the deck using Fisher-Yates algorithm with SHA256 hash chaining.
///
/// This is a deterministic shuffle - the same seed always produces the same deck order.
/// The algorithm is provably fair because:
/// 1. The seed comes from IC's VRF (Verifiable Random Function)
/// 2. SHA256 hash chaining ensures each swap is unpredictable without the seed
/// 3. Anyone can verify by re-running this function with the revealed seed
///
/// # Algorithm
/// For each position i from 51 down to 1:
///   1. Hash(previous_hash || i) to get deterministic randomness
///   2. Select position j = random_value mod (i+1)
///   3. Swap cards at positions i and j
fn shuffle_deck(deck: &mut Vec<Card>, seed: &[u8]) {
    let mut hash_input = seed.to_vec();

    for i in (1..deck.len()).rev() {
        let mut hasher = Sha256::new();
        hasher.update(&hash_input);
        hasher.update(&[i as u8]);
        let hash_result = hasher.finalize();

        // SHA256 always produces 32 bytes, so this slice is always valid
        let random_value = u64::from_le_bytes([
            hash_result[0], hash_result[1], hash_result[2], hash_result[3],
            hash_result[4], hash_result[5], hash_result[6], hash_result[7],
        ]);
        let j = (random_value as usize) % (i + 1);

        deck.swap(i, j);
        hash_input = hash_result.to_vec();
    }
}

// ============================================================================
// HAND EVALUATION
// ============================================================================

/// Evaluates a player's best 5-card hand from their 2 hole cards and up to 5 community cards.
///
/// Generates all possible 5-card combinations from the 7 available cards and returns
/// the highest-ranking hand according to standard poker hand rankings:
/// Royal Flush > Straight Flush > Four of a Kind > Full House > Flush >
/// Straight > Three of a Kind > Two Pair > One Pair > High Card
fn evaluate_hand(hole_cards: &(Card, Card), community: &[Card]) -> HandRank {
    let mut all_cards: Vec<Card> = Vec::with_capacity(7);
    all_cards.push(hole_cards.0);
    all_cards.push(hole_cards.1);
    all_cards.extend_from_slice(community);

    // Generate all 5-card combinations and find the best
    let mut best_rank: Option<HandRank> = None;

    for combo in combinations(&all_cards, 5) {
        let rank = evaluate_five_cards(&combo);
        match &best_rank {
            None => best_rank = Some(rank),
            Some(current) if rank > *current => best_rank = Some(rank),
            _ => {}
        }
    }

    best_rank.unwrap_or(HandRank::HighCard(vec![]))
}

fn combinations(cards: &[Card], k: usize) -> Vec<Vec<Card>> {
    let mut result = Vec::new();
    let n = cards.len();
    if k > n {
        return result;
    }

    let mut indices: Vec<usize> = (0..k).collect();

    loop {
        result.push(indices.iter().map(|&i| cards[i]).collect());

        let mut i = k;
        while i > 0 {
            i -= 1;
            if indices[i] != i + n - k {
                break;
            }
        }

        if i == 0 && indices[0] == n - k {
            break;
        }

        indices[i] += 1;
        for j in (i + 1)..k {
            indices[j] = indices[j - 1] + 1;
        }
    }

    result
}

fn evaluate_five_cards(cards: &[Card]) -> HandRank {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank.value()).collect();
    ranks.sort_by(|a, b| b.cmp(a)); // Sort descending

    let mut suits: HashMap<Suit, u8> = HashMap::new();
    let mut rank_counts: HashMap<u8, u8> = HashMap::new();

    for card in cards {
        *suits.entry(card.suit).or_insert(0) += 1;
        *rank_counts.entry(card.rank.value()).or_insert(0) += 1;
    }

    let is_flush = suits.values().any(|&count| count >= 5);
    let is_straight = check_straight(&ranks);
    let straight_high = if is_straight { get_straight_high(&ranks) } else { 0 };

    // Royal Flush
    if is_flush && is_straight && straight_high == 14 {
        return HandRank::RoyalFlush;
    }

    // Straight Flush
    if is_flush && is_straight {
        return HandRank::StraightFlush(straight_high);
    }

    // Count pairs, trips, quads
    let mut pairs: Vec<u8> = Vec::new();
    let mut trips: Vec<u8> = Vec::new();
    let mut quads: Vec<u8> = Vec::new();

    for (&rank, &count) in &rank_counts {
        match count {
            4 => quads.push(rank),
            3 => trips.push(rank),
            2 => pairs.push(rank),
            _ => {}
        }
    }

    pairs.sort_by(|a, b| b.cmp(a));
    trips.sort_by(|a, b| b.cmp(a));

    // Four of a Kind
    if !quads.is_empty() {
        let kicker = ranks.iter().find(|&&r| r != quads[0]).copied().unwrap_or(0);
        return HandRank::FourOfAKind(quads[0], kicker);
    }

    // Full House
    if !trips.is_empty() && !pairs.is_empty() {
        return HandRank::FullHouse(trips[0], pairs[0]);
    }

    // Flush
    if is_flush {
        return HandRank::Flush(ranks.clone());
    }

    // Straight
    if is_straight {
        return HandRank::Straight(straight_high);
    }

    // Three of a Kind
    if !trips.is_empty() {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != trips[0])
            .take(2)
            .copied()
            .collect();
        return HandRank::ThreeOfAKind(trips[0], kickers);
    }

    // Two Pair
    if pairs.len() >= 2 {
        let kicker = ranks.iter()
            .find(|&&r| r != pairs[0] && r != pairs[1])
            .copied()
            .unwrap_or(0);
        return HandRank::TwoPair(pairs[0], pairs[1], kicker);
    }

    // One Pair
    if pairs.len() == 1 {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != pairs[0])
            .take(3)
            .copied()
            .collect();
        return HandRank::Pair(pairs[0], kickers);
    }

    // High Card
    HandRank::HighCard(ranks)
}

/// Detects if ranks form a straight and returns the high card.
/// Returns Some(high_card) if straight found, None otherwise.
/// Handles wheel (A-2-3-4-5) as a special case with high card 5.
fn detect_straight(ranks: &[u8]) -> Option<u8> {
    let mut sorted: Vec<u8> = ranks.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();

    if sorted.len() < 5 {
        return None;
    }

    // Check for wheel (A-2-3-4-5) first - it's the lowest straight
    // Must check before regular straights since A-5-4-3-2 window won't match
    if sorted.contains(&14) && sorted.contains(&5) && sorted.contains(&4)
        && sorted.contains(&3) && sorted.contains(&2) {
        return Some(5); // Wheel's high card is 5
    }

    // Check for regular straight (highest first)
    for window in sorted.windows(5) {
        if window[0] - window[4] == 4 {
            return Some(window[0]);
        }
    }

    None
}

fn check_straight(ranks: &[u8]) -> bool {
    detect_straight(ranks).is_some()
}

fn get_straight_high(ranks: &[u8]) -> u8 {
    detect_straight(ranks).unwrap_or(0)
}

// ============================================================================
// GAME FLOW
// ============================================================================

#[ic_cdk::update]
async fn start_new_hand() -> Result<ShuffleProof, String> {
    check_rate_limit()?;
    // SECURITY: Check all preconditions BEFORE calling raw_rand to prevent cycle drain
    // Any caller can call this, so we must validate everything first
    let precondition_check = TABLE.with(|t| {
        let table = t.borrow();
        let state = match table.as_ref() {
            Some(s) => s,
            None => return Err("Table not initialized".to_string()),
        };

        // Check phase - only allow starting if we're waiting or hand is complete
        if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
            return Err("Cannot start new hand: a hand is already in progress".to_string());
        }

        // Count active players BEFORE calling raw_rand to prevent cycle drain
        let active_count = state.players.iter()
            .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
            .count();

        if active_count < 2 {
            return Err("Need at least 2 active players with chips".to_string());
        }

        Ok(())
    });

    // Return early if preconditions fail - before any expensive operations
    precondition_check?;

    // Now safe to call raw_rand - we've verified the hand can actually start
    let random_bytes = raw_rand().await
        .map_err(|e| format!("Failed to get randomness: {:?}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&random_bytes);
    let seed_hash = hex::encode(hasher.finalize());
    let timestamp = ic_cdk::api::time();

    let mut deck = create_deck();
    shuffle_deck(&mut deck, &random_bytes);

    // Store seed securely - will only be revealed when hand ends
    CURRENT_SEED.with(|s| {
        *s.borrow_mut() = Some(random_bytes.clone());
    });

    let proof = ShuffleProof {
        seed_hash: seed_hash.clone(),
        revealed_seed: None, // Never revealed until hand ends
        timestamp,
    };

    let result_proof = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Double-check phase inside the lock (in case of race)
        if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
            return Err("Cannot start new hand: a hand is already in progress".to_string());
        }

        // Handle players who wanted to sit out next hand
        for player in state.players.iter_mut().flatten() {
            if player.is_sitting_out_next_hand {
                player.status = PlayerStatus::SittingOut;
                player.sitting_out_since = Some(timestamp);
                player.is_sitting_out_next_hand = false;
            }
        }

        // Auto-sit out players with no chips (busted)
        for player in state.players.iter_mut().flatten() {
            if player.status == PlayerStatus::Active && player.chips == 0 {
                player.status = PlayerStatus::SittingOut;
                player.sitting_out_since = Some(timestamp);
            }
        }

        // Count active players (not sitting out)
        let active_count = state.players.iter()
            .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
            .count();

        if active_count < 2 {
            return Err("Need at least 2 active players with chips".to_string());
        }

        // Move dealer button - on first hand, find first active player
        if state.first_hand {
            // Find first active player to be dealer
            state.dealer_seat = find_next_active_seat_with_chips(state, 0);
            state.first_hand = false;
        } else {
            state.dealer_seat = find_next_active_seat_with_chips(state, state.dealer_seat);
        }

        // Set blinds positions
        if active_count == 2 {
            // Heads up: dealer is small blind
            state.small_blind_seat = state.dealer_seat;
            state.big_blind_seat = find_next_active_seat_with_chips(state, state.dealer_seat);
        } else {
            state.small_blind_seat = find_next_active_seat_with_chips(state, state.dealer_seat);
            state.big_blind_seat = find_next_active_seat_with_chips(state, state.small_blind_seat);
        }

        // Reset state
        state.deck = deck;
        state.deck_index = 0;
        state.community_cards.clear();
        state.pot = 0;
        state.side_pots.clear();
        state.current_bet = state.config.big_blind;
        state.min_raise = state.config.big_blind;
        state.phase = GamePhase::PreFlop;
        state.shuffle_proof = Some(proof.clone());
        state.hand_number += 1;
        state.last_aggressor = None;
        state.bb_has_option = true; // BB gets option to raise if limped to
        state.auto_deal_at = None; // Clear auto-deal timer since hand is starting

        // Reset players and track starting chips for history
        STARTING_CHIPS.with(|s| s.borrow_mut().clear());
        for (i, player) in state.players.iter_mut().enumerate() {
            if let Some(ref mut p) = player {
                // Save starting chips before any deductions
                STARTING_CHIPS.with(|s| {
                    s.borrow_mut().insert(i as u8, p.chips);
                });
                p.hole_cards = None;
                p.current_bet = 0;
                p.total_bet_this_hand = 0;
                p.has_folded = false;
                p.has_acted_this_round = false;
                p.is_all_in = false;
            }
        }

        // Post antes if configured (with overflow protection)
        if state.config.ante > 0 {
            for player in state.players.iter_mut().flatten() {
                if player.status == PlayerStatus::Active && player.chips > 0 {
                    let ante_amount = state.config.ante.min(player.chips);
                    player.chips = player.chips.saturating_sub(ante_amount);
                    player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(ante_amount);
                    state.pot = state.pot.saturating_add(ante_amount);
                    if player.chips == 0 {
                        player.is_all_in = true;
                    }
                }
            }
        }

        // Post small blind (with overflow protection)
        if let Some(ref mut sb_player) = state.players[state.small_blind_seat as usize] {
            let sb_amount = state.config.small_blind.min(sb_player.chips);
            sb_player.chips = sb_player.chips.saturating_sub(sb_amount);
            sb_player.current_bet = sb_amount;
            sb_player.total_bet_this_hand = sb_player.total_bet_this_hand.saturating_add(sb_amount);
            state.pot = state.pot.saturating_add(sb_amount);
            if sb_player.chips == 0 {
                sb_player.is_all_in = true;
            }
        }

        // Post big blind (with overflow protection)
        if let Some(ref mut bb_player) = state.players[state.big_blind_seat as usize] {
            let bb_amount = state.config.big_blind.min(bb_player.chips);
            bb_player.chips = bb_player.chips.saturating_sub(bb_amount);
            bb_player.current_bet = bb_amount;
            bb_player.total_bet_this_hand = bb_player.total_bet_this_hand.saturating_add(bb_amount);
            state.pot = state.pot.saturating_add(bb_amount);
            // BB has technically "acted" by posting but still gets option
            // We track this with bb_has_option, not has_acted_this_round
            if bb_player.chips == 0 {
                bb_player.is_all_in = true;
                state.bb_has_option = false; // Can't raise if all-in
            }
        }

        // Deal hole cards to active players with chips (with bounds checking)
        for player in state.players.iter_mut().flatten() {
            if player.status == PlayerStatus::Active {
                // Check we have enough cards (need 2 cards, so index+2 must be <= len)
                if state.deck_index + 2 <= state.deck.len() {
                    let card1 = state.deck[state.deck_index];
                    let card2 = state.deck[state.deck_index + 1];
                    player.hole_cards = Some((card1, card2));
                    state.deck_index += 2;
                }
            }
        }

        // Action starts left of big blind
        state.action_on = find_next_active_seat_with_chips(state, state.big_blind_seat);

        // Start action timer using config timeout
        let now = ic_cdk::api::time();
        let timeout_ns = state.config.action_timeout_secs * 1_000_000_000;
        state.action_timer = Some(ActionTimer {
            player_seat: state.action_on,
            started_at: now,
            expires_at: now + timeout_ns,
            using_time_bank: false,
        });

        Ok(proof.clone())
    })?;

    // Clear shown cards from previous hand
    SHOWN_CARDS.with(|s| s.borrow_mut().clear());

    // Save to history (seed NOT revealed yet - will be revealed when hand ends)
    CURRENT_ACTIONS.with(|a| a.borrow_mut().clear());
    HAND_HISTORY.with(|h| {
        h.borrow_mut().push(HandHistory {
            hand_number: TABLE.with(|t| t.borrow().as_ref().map(|s| s.hand_number).unwrap_or(0)),
            shuffle_proof: ShuffleProof {
                seed_hash,
                revealed_seed: None, // Will be set when hand completes
                timestamp,
            },
            actions: Vec::new(),
            winners: Vec::new(),
            community_cards: Vec::new(),
            showdown_players: Vec::new(),
        });
    });

    Ok(result_proof)
}

/// Reveal the seed and update both table state and history
/// Called only when hand ends (showdown or single winner by fold)
fn reveal_seed_on_hand_end(state: &mut TableState) {
    // Get the stored seed and reveal it
    let revealed = CURRENT_SEED.with(|s| {
        s.borrow_mut().take().map(|seed| hex::encode(&seed))
    });

    // Get the expected seed_hash from the current state to ensure correct matching
    let expected_seed_hash = state.shuffle_proof.as_ref().map(|p| p.seed_hash.clone());

    // Update table state's shuffle proof
    if let Some(ref mut proof) = state.shuffle_proof {
        proof.revealed_seed = revealed.clone();
    }

    // Update history's shuffle proof - find entry by hand_number AND seed_hash for safety
    HAND_HISTORY.with(|h| {
        let mut history = h.borrow_mut();
        // Find the entry matching this hand's hand_number and seed_hash
        if let Some(entry) = history.iter_mut().rev().find(|e| {
            e.hand_number == state.hand_number &&
            expected_seed_hash.as_ref().map_or(true, |hash| &e.shuffle_proof.seed_hash == hash)
        }) {
            entry.shuffle_proof.revealed_seed = revealed;
        }
    });
}

/// Find next active seat that can act (not folded, not all-in)
fn find_next_active_seat(state: &TableState, from_seat: u8) -> u8 {
    let num_seats = state.players.len();
    let mut seat = (from_seat as usize + 1) % num_seats;

    for _ in 0..num_seats {
        if let Some(ref player) = state.players[seat] {
            if player.status == PlayerStatus::Active && !player.has_folded && !player.is_all_in {
                return seat as u8;
            }
        }
        seat = (seat + 1) % num_seats;
    }

    from_seat
}

/// Find next active seat with chips (for dealer/blind positions)
fn find_next_active_seat_with_chips(state: &TableState, from_seat: u8) -> u8 {
    let num_seats = state.players.len();
    let mut seat = (from_seat as usize + 1) % num_seats;

    for _ in 0..num_seats {
        if let Some(ref player) = state.players[seat] {
            if player.status == PlayerStatus::Active && player.chips > 0 {
                return seat as u8;
            }
        }
        seat = (seat + 1) % num_seats;
    }

    from_seat
}

fn count_active_players(state: &TableState) -> usize {
    state.players.iter()
        .filter(|p| p.as_ref().map(|p| !p.has_folded && p.status == PlayerStatus::Active).unwrap_or(false))
        .count()
}

fn count_players_can_act(state: &TableState) -> usize {
    state.players.iter()
        .filter(|p| p.as_ref().map(|p| {
            !p.has_folded && !p.is_all_in && p.status == PlayerStatus::Active
        }).unwrap_or(false))
        .count()
}

// ============================================================================
// PLAYER ACTIONS
// ============================================================================

/// Join table with minimum buy-in from escrow balance
/// Requires sufficient ICP deposited first via notify_deposit
#[ic_cdk::update]
fn join_table(seat: u8) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    // Check escrow balance
    let balance = BALANCES.with(|b| {
        b.borrow().get(&caller).copied().unwrap_or(0)
    });

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        if seat as usize >= state.players.len() {
            return Err("Invalid seat".to_string());
        }

        if state.players[seat as usize].is_some() {
            return Err("Seat is taken".to_string());
        }

        for p in state.players.iter().flatten() {
            if p.principal == caller {
                return Err("Already at table".to_string());
            }
        }

        // Require minimum buy-in from escrow balance
        if balance < state.config.min_buy_in {
            let currency = state.config.currency;
            return Err(format!(
                "Insufficient balance. Need {}, have {} in escrow. Deposit {} first.",
                currency.format_amount(state.config.min_buy_in),
                currency.format_amount(balance),
                currency.symbol()
            ));
        }

        // Deduct buy-in from escrow balance
        let buy_in_amount = state.config.min_buy_in;
        BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            balances.insert(caller, balance.saturating_sub(buy_in_amount));
        });

        // Determine if joining during active hand - if so, sit out until next hand
        let joining_during_hand = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;

        let initial_status = if joining_during_hand {
            PlayerStatus::SittingOut
        } else {
            PlayerStatus::Active
        };

        let time_bank = state.config.time_bank_secs;
        let sitting_out_since = if initial_status == PlayerStatus::SittingOut { Some(now) } else { None };
        state.players[seat as usize] = Some(Player {
            principal: caller,
            seat,
            chips: buy_in_amount,
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: 0,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: false,
            status: initial_status,
            last_seen: now,
            timeout_count: 0,
            time_bank_remaining: time_bank,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since,
        });

        // Auto-start if we now have enough players and waiting for players
        if state.phase == GamePhase::WaitingForPlayers {
            let active_count = state.players.iter()
                .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
                .count();

            if active_count >= 2 && state.auto_deal_at.is_none() {
                // Schedule auto-deal to start the game
                state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
            }
        }

        Ok(())
    })
}

/// Leave table and return chips to escrow balance
/// If mid-hand, this acts as a fold - pot contributions stay in the pot
#[ic_cdk::update]
fn leave_table() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    let chips = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Find the player's seat
        let seat = state.players.iter()
            .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .ok_or("Not at table")?;

        let player = state.players[seat].as_ref().ok_or("Player not found")?;
        let chips = player.chips;
        let was_in_hand = !player.has_folded &&
            state.phase != GamePhase::WaitingForPlayers &&
            state.phase != GamePhase::HandComplete;
        let was_action_on = state.action_on as usize == seat;

        // If we're in a hand, mark as folded first (pot contributions stay in pot)
        if was_in_hand {
            if let Some(ref mut p) = state.players[seat] {
                p.has_folded = true;
            }
        }

        // Remove player from table
        state.players[seat] = None;

        // If player was in the hand, advance game state
        if was_in_hand {
            // Check if only one player left - award pot
            if count_active_players(state) == 1 {
                end_hand_single_winner(state);
            } else if was_action_on {
                // If it was this player's turn, move to next player
                state.action_on = find_next_active_seat(state, state.action_on);
                let now = ic_cdk::api::time();
                let timeout_ns = state.config.action_timeout_secs * 1_000_000_000;
                state.action_timer = Some(ActionTimer {
                    player_seat: state.action_on,
                    started_at: now,
                    expires_at: now + timeout_ns,
                    using_time_bank: false,
                });
            }
        }

        Ok::<u64, String>(chips)
    })?;

    // Return remaining chips to escrow balance (with overflow protection)
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        balances.insert(caller, current.saturating_add(chips));
    });

    Ok(chips)
}

#[ic_cdk::update]
fn player_action(action: PlayerAction) -> Result<(), String> {
    check_rate_limit()?;
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Find the player
        let player_seat = state.players.iter()
            .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .ok_or("Not at table")?;

        if player_seat != state.action_on as usize {
            return Err("Not your turn".to_string());
        }

        if state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete {
            return Err("No hand in progress".to_string());
        }

        // BUGFIX: Check if the action timer has expired
        // If check_timeouts hasn't been called, we still enforce the timer here
        if let Some(ref timer) = state.action_timer {
            if now > timer.expires_at {
                return Err("Action timer has expired. Your turn was forfeited.".to_string());
            }
        }

        // First, gather all the info we need from the player without holding the mutable ref
        let (player_chips, player_current_bet) = {
            let player = state.players[player_seat].as_ref()
                .ok_or("Player not found at seat")?;
            (player.chips, player.current_bet)
        };

        // Track whether we need to reset acted flags after processing
        let mut should_reset_acted = false;
        let mut new_current_bet = state.current_bet;

        // Check if this is BB acting on their option
        let is_bb_option = state.phase == GamePhase::PreFlop
            && state.bb_has_option
            && player_seat == state.big_blind_seat as usize;

        // Validate and process action
        match action.clone() {
            PlayerAction::Fold => {
                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                player.has_folded = true;
                player.last_seen = now;
                player.has_acted_this_round = true;
                if is_bb_option {
                    state.bb_has_option = false;
                }
            }
            PlayerAction::Check => {
                // Player can check if:
                // 1. Their current bet matches the table's current bet (nothing to call)
                // 2. It's BB's option and no one has raised above BB's posted amount
                //    (handles short-stacked BB who posted less than config.big_blind)
                let can_check = state.current_bet == player_current_bet
                    || (is_bb_option && state.current_bet <= player_current_bet);

                if !can_check {
                    return Err("Cannot check, there's a bet to call".to_string());
                }
                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                player.last_seen = now;
                player.has_acted_this_round = true;
                if is_bb_option {
                    state.bb_has_option = false;
                }
            }
            PlayerAction::Call => {
                let to_call = state.current_bet.saturating_sub(player_current_bet);
                if to_call == 0 {
                    return Err("Nothing to call, use check".to_string());
                }
                let actual_call = to_call.min(player_chips);

                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                player.chips = player.chips.saturating_sub(actual_call);
                player.current_bet = player.current_bet.saturating_add(actual_call);
                player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(actual_call);
                state.pot = state.pot.saturating_add(actual_call);
                if player.chips == 0 {
                    player.is_all_in = true;
                }
                player.last_seen = now;
                player.has_acted_this_round = true;
            }
            PlayerAction::Bet(amount) => {
                if state.current_bet > 0 {
                    return Err("Cannot bet, there's already a bet. Use raise.".to_string());
                }
                if amount < state.config.big_blind {
                    return Err(format!("Minimum bet is {}", state.config.currency.format_amount(state.config.big_blind)));
                }
                if amount > player_chips {
                    return Err("Not enough chips".to_string());
                }

                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                player.chips = player.chips.saturating_sub(amount);
                player.current_bet = amount;
                player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(amount);
                state.pot = state.pot.saturating_add(amount);
                new_current_bet = amount;
                state.min_raise = amount;
                state.last_aggressor = Some(player_seat as u8);
                if player.chips == 0 {
                    player.is_all_in = true;
                }
                player.last_seen = now;
                player.has_acted_this_round = true;
                should_reset_acted = true;
                // Any bet/aggressive action removes BB's option
                state.bb_has_option = false;
            }
            PlayerAction::Raise(amount) => {
                let raise_amount = amount.saturating_sub(state.current_bet);
                if raise_amount < state.min_raise {
                    let currency = state.config.currency;
                    return Err(format!("Minimum raise is {} (to {})",
                        currency.format_amount(state.min_raise),
                        currency.format_amount(state.current_bet.saturating_add(state.min_raise))));
                }
                let total_needed = amount.saturating_sub(player_current_bet);
                if total_needed > player_chips {
                    return Err("Not enough chips".to_string());
                }

                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                player.chips = player.chips.saturating_sub(total_needed);
                player.current_bet = amount;
                player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(total_needed);
                state.pot = state.pot.saturating_add(total_needed);
                state.min_raise = raise_amount;
                new_current_bet = amount;
                state.last_aggressor = Some(player_seat as u8);
                if player.chips == 0 {
                    player.is_all_in = true;
                }
                player.last_seen = now;
                player.has_acted_this_round = true;
                should_reset_acted = true;
                // Any raise removes BB's option (not just when BB raises)
                state.bb_has_option = false;
            }
            PlayerAction::AllIn => {
                let player = state.players[player_seat].as_mut().expect("Player validated at seat");
                let all_in_amount = player.chips;
                state.pot = state.pot.saturating_add(all_in_amount);
                player.current_bet = player.current_bet.saturating_add(all_in_amount);
                player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(all_in_amount);
                let final_bet = player.current_bet;

                if final_bet > state.current_bet {
                    let raise_amount = final_bet.saturating_sub(state.current_bet);
                    if raise_amount >= state.min_raise {
                        state.min_raise = raise_amount;
                    }
                    new_current_bet = final_bet;
                    state.last_aggressor = Some(player_seat as u8);
                    should_reset_acted = true;
                    // All-in that raises removes BB's option
                    state.bb_has_option = false;
                }

                player.chips = 0;
                player.is_all_in = true;
                player.last_seen = now;
                player.has_acted_this_round = true;
            }
        }

        state.current_bet = new_current_bet;

        // Reset acted flags after we're done with the player borrow
        if should_reset_acted {
            for (i, p_opt) in state.players.iter_mut().enumerate() {
                if let Some(ref mut p) = p_opt {
                    if i != player_seat && !p.has_folded && !p.is_all_in {
                        p.has_acted_this_round = false;
                    }
                }
            }
        }

        // Track last action for UI display
        let last_action_type = match action.clone() {
            PlayerAction::Fold => LastAction::Fold,
            PlayerAction::Check => LastAction::Check,
            PlayerAction::Call => {
                let call_amount = state.current_bet.saturating_sub(player_current_bet).min(player_chips);
                LastAction::Call { amount: call_amount }
            },
            PlayerAction::Bet(amount) => LastAction::Bet { amount },
            PlayerAction::Raise(amount) => LastAction::Raise { amount },
            PlayerAction::AllIn => {
                // Get the player's final bet to show in the action
                let final_bet = state.players[player_seat].as_ref()
                    .map(|p| p.current_bet)
                    .unwrap_or(0);
                LastAction::AllIn { amount: final_bet }
            },
        };
        state.last_action = Some(LastActionInfo {
            seat: player_seat as u8,
            action: last_action_type,
            timestamp: now,
        });

        // Record action with current phase and amount
        let current_phase = phase_to_string(&state.phase);
        let action_amount = match action.clone() {
            PlayerAction::Fold | PlayerAction::Check => 0,
            PlayerAction::Call => state.current_bet.saturating_sub(player_current_bet).min(player_chips),
            PlayerAction::Bet(amt) | PlayerAction::Raise(amt) => amt,
            PlayerAction::AllIn => state.players[player_seat].as_ref()
                .map(|p| p.current_bet)
                .unwrap_or(0),
        };
        CURRENT_ACTIONS.with(|a| {
            a.borrow_mut().push(ActionRecord {
                seat: player_seat as u8,
                action: action.clone(),
                timestamp: now,
                phase: current_phase,
                amount: action_amount,
            });
        });

        // Advance game
        advance_game(state);

        Ok(())
    })
}

// Note: reset_acted_flags is now inlined in player_action to avoid borrow conflicts

fn advance_game(state: &mut TableState) {
    let now = ic_cdk::api::time();

    // Check if only one player left
    if count_active_players(state) == 1 {
        end_hand_single_winner(state);
        return;
    }

    // Check if betting round is complete
    if is_betting_round_complete(state) {
        advance_to_next_street(state);
        return;
    }

    // Move action to next player
    state.action_on = find_next_active_seat(state, state.action_on);

    // Reset timer using config timeout
    let timeout_ns = state.config.action_timeout_secs * 1_000_000_000;
    state.action_timer = Some(ActionTimer {
        player_seat: state.action_on,
        started_at: now,
        expires_at: now + timeout_ns,
        using_time_bank: false,
    });
}

fn is_betting_round_complete(state: &TableState) -> bool {
    let players_can_act = count_players_can_act(state);

    if players_can_act == 0 {
        return true;
    }

    // Special case: preflop BB option
    // If we're preflop and BB hasn't acted yet and no one raised, BB gets option
    if state.phase == GamePhase::PreFlop && state.bb_has_option {
        // Check if action is on BB
        if state.action_on == state.big_blind_seat {
            // BB still needs to act (check or raise)
            if let Some(ref bb_player) = state.players[state.big_blind_seat as usize] {
                if !bb_player.has_acted_this_round && !bb_player.is_all_in && !bb_player.has_folded {
                    return false;
                }
            }
        }
    }

    for player in state.players.iter().flatten() {
        if !player.has_folded && !player.is_all_in && player.status == PlayerStatus::Active {
            // Player hasn't acted yet this round
            if !player.has_acted_this_round {
                return false;
            }
            // Player's bet doesn't match current bet
            if player.current_bet < state.current_bet {
                return false;
            }
        }
    }

    true
}

/// Run out the remaining community cards when all active players are all-in
/// This is non-recursive to avoid stack overflow
fn run_out_board(state: &mut TableState) {
    // Calculate side pots first
    if state.side_pots.is_empty() {
        calculate_side_pots(state);
    }

    // Deal remaining cards based on current phase
    loop {
        match state.phase {
            GamePhase::PreFlop => {
                // Deal flop
                state.deck_index += 1; // Burn
                for _ in 0..3 {
                    if state.deck_index < state.deck.len() {
                        state.community_cards.push(state.deck[state.deck_index]);
                        state.deck_index += 1;
                    }
                }
                state.phase = GamePhase::Flop;
            }
            GamePhase::Flop => {
                // Deal turn
                state.deck_index += 1; // Burn
                if state.deck_index < state.deck.len() {
                    state.community_cards.push(state.deck[state.deck_index]);
                    state.deck_index += 1;
                }
                state.phase = GamePhase::Turn;
            }
            GamePhase::Turn => {
                // Deal river
                state.deck_index += 1; // Burn
                if state.deck_index < state.deck.len() {
                    state.community_cards.push(state.deck[state.deck_index]);
                    state.deck_index += 1;
                }
                state.phase = GamePhase::River;
            }
            GamePhase::River => {
                // Go to showdown
                state.phase = GamePhase::Showdown;
                determine_winners(state);
                return;
            }
            _ => {
                // Already at showdown or waiting - just determine winners
                if state.phase == GamePhase::Showdown {
                    determine_winners(state);
                }
                return;
            }
        }
    }
}

fn advance_to_next_street(state: &mut TableState) {
    let now = ic_cdk::api::time();

    // Reset for new street
    state.current_bet = 0;
    state.min_raise = state.config.big_blind;
    state.bb_has_option = false; // BB option only applies preflop

    for player in state.players.iter_mut().flatten() {
        player.current_bet = 0;
        player.has_acted_this_round = false;
    }

    match state.phase {
        GamePhase::PreFlop => {
            // Calculate side pots before dealing flop (in case of all-ins)
            calculate_side_pots(state);

            // Deal flop (burn + 3 cards - need 4 cards available)
            if state.deck_index + 3 < state.deck.len() {
                state.deck_index += 1; // Burn
                for _ in 0..3 {
                    state.community_cards.push(state.deck[state.deck_index]);
                    state.deck_index += 1;
                }
            }
            state.phase = GamePhase::Flop;
        }
        GamePhase::Flop => {
            // Deal turn (burn + 1 card - need 2 cards available)
            if state.deck_index + 1 < state.deck.len() {
                state.deck_index += 1; // Burn
                state.community_cards.push(state.deck[state.deck_index]);
                state.deck_index += 1;
            }
            state.phase = GamePhase::Turn;
        }
        GamePhase::Turn => {
            // Deal river (burn + 1 card - need 2 cards available)
            if state.deck_index + 1 < state.deck.len() {
                state.deck_index += 1; // Burn
                state.community_cards.push(state.deck[state.deck_index]);
                state.deck_index += 1;
            }
            state.phase = GamePhase::River;
        }
        GamePhase::River => {
            // Go to showdown
            state.phase = GamePhase::Showdown;
            determine_winners(state);
            return;
        }
        _ => {}
    }

    // Check if we can have more betting (need 2+ players who can act)
    if count_players_can_act(state) < 2 {
        // Run out the board without recursion
        run_out_board(state);
        return;
    }

    // Action starts with first active player after dealer
    state.action_on = find_next_active_seat(state, state.dealer_seat);

    let timeout_ns = state.config.action_timeout_secs * 1_000_000_000;
    state.action_timer = Some(ActionTimer {
        player_seat: state.action_on,
        started_at: now,
        expires_at: now + timeout_ns,
        using_time_bank: false,
    });
}

/// Calculate side pots when there are all-in players
/// This should be called before showdown or when all betting is complete
fn calculate_side_pots(state: &mut TableState) {
    // Collect ALL players who bet this hand (including folded) with their bets
    let mut all_contributions: Vec<(u8, u64, bool)> = Vec::new(); // (seat, bet, has_folded)

    for (i, player) in state.players.iter().enumerate() {
        if let Some(ref p) = player {
            if p.total_bet_this_hand > 0 {
                all_contributions.push((i as u8, p.total_bet_this_hand, p.has_folded));
            }
        }
    }

    if all_contributions.is_empty() {
        return;
    }

    // Get unique bet levels, sorted ascending
    let mut bet_levels: Vec<u64> = all_contributions.iter()
        .map(|(_, bet, _)| *bet)
        .collect();
    bet_levels.sort();
    bet_levels.dedup();

    state.side_pots.clear();
    let mut processed_amount = 0u64;

    for level in bet_levels {
        let contribution_per_player = level.saturating_sub(processed_amount);

        if contribution_per_player == 0 {
            continue;
        }

        // Calculate pot amount from all players who contributed at least up to this level
        let pot_amount: u64 = all_contributions.iter()
            .filter(|(_, bet, _)| *bet >= level)
            .map(|_| contribution_per_player)
            .fold(0u64, |acc, x| acc.saturating_add(x));

        // Add contributions from players who bet less than this level but more than processed
        let partial_contributions: u64 = all_contributions.iter()
            .filter(|(_, bet, _)| *bet > processed_amount && *bet < level)
            .map(|(_, bet, _)| bet.saturating_sub(processed_amount))
            .fold(0u64, |acc, x| acc.saturating_add(x));

        let total_pot = pot_amount.saturating_add(partial_contributions);

        // Eligible players are only those who haven't folded and bet at least this level
        let eligible_players: Vec<u8> = all_contributions.iter()
            .filter(|(_, bet, folded)| !*folded && *bet >= level)
            .map(|(seat, _, _)| *seat)
            .collect();

        if total_pot > 0 && !eligible_players.is_empty() {
            state.side_pots.push(SidePot {
                amount: total_pot,
                eligible_players,
            });
        } else if total_pot > 0 && eligible_players.is_empty() {
            // Edge case: all eligible players folded - money goes to last pot
            // If no last pot exists, we need to find any player still in the hand
            if let Some(last_pot) = state.side_pots.last_mut() {
                last_pot.amount = last_pot.amount.saturating_add(total_pot);
            } else {
                // No existing side pot - find any non-folded player to create a pot for
                let any_eligible: Vec<u8> = all_contributions.iter()
                    .filter(|(_, _, folded)| !*folded)
                    .map(|(seat, _, _)| *seat)
                    .collect();
                if !any_eligible.is_empty() {
                    state.side_pots.push(SidePot {
                        amount: total_pot,
                        eligible_players: any_eligible,
                    });
                }
                // If truly no one is eligible (everyone folded), pot is dead - this shouldn't happen
            }
        }

        processed_amount = level;
    }

    // Verify total matches state.pot - if not, adjust last pot
    let total_side_pots: u64 = state.side_pots.iter()
        .map(|sp| sp.amount)
        .fold(0u64, |acc, x| acc.saturating_add(x));

    if total_side_pots < state.pot {
        // Add remaining pot to last pot (or first eligible pot)
        let remaining = state.pot.saturating_sub(total_side_pots);
        if let Some(last_pot) = state.side_pots.last_mut() {
            last_pot.amount = last_pot.amount.saturating_add(remaining);
        }
    } else if total_side_pots > state.pot {
        // SANITY CHECK: Side pots should never exceed total pot
        // This indicates a bug - log it and cap to prevent creating chips from nothing
        ic_cdk::println!("BUG: Side pots ({}) exceed total pot ({}). Capping to pot amount.",
            total_side_pots, state.pot);
        // Proportionally reduce all side pots to match total pot
        if total_side_pots > 0 {
            let ratio = state.pot as f64 / total_side_pots as f64;
            let mut distributed: u64 = 0;
            let pot_count = state.side_pots.len();
            for (i, side_pot) in state.side_pots.iter_mut().enumerate() {
                if i == pot_count - 1 {
                    // Last pot gets remainder to avoid rounding errors
                    side_pot.amount = state.pot.saturating_sub(distributed);
                } else {
                    let adjusted = (side_pot.amount as f64 * ratio) as u64;
                    side_pot.amount = adjusted;
                    distributed = distributed.saturating_add(adjusted);
                }
            }
        }
    }
}

fn end_hand_single_winner(state: &mut TableState) {
    // Reveal the seed now that hand is ending
    reveal_seed_on_hand_end(state);

    // Find the remaining player
    let winner = state.players.iter()
        .enumerate()
        .find(|(_, p)| p.as_ref().map(|p| !p.has_folded).unwrap_or(false));

    // BUGFIX: state.pot already contains all contributions
    // side_pots are just a breakdown of the same money for eligibility tracking
    // DO NOT add them together - that would double-pay
    let total_pot = state.pot;

    let mut winners_for_history = Vec::new();

    if let Some((seat, Some(player))) = winner {
        let winner_info = Winner {
            seat: seat as u8,
            principal: player.principal,
            amount: total_pot,
            hand_rank: None,
            cards: None,
        };

        winners_for_history.push(winner_info.clone());

        // Award entire pot (with overflow protection)
        if let Some(ref mut p) = state.players[seat] {
            p.chips = p.chips.saturating_add(total_pot);
        }

        // Update local history
        HAND_HISTORY.with(|h| {
            if let Some(last) = h.borrow_mut().last_mut() {
                last.winners.push(winner_info.clone());
                last.community_cards = state.community_cards.clone();
                CURRENT_ACTIONS.with(|a| {
                    last.actions = a.borrow().clone();
                });
            }
        });

        // Store winners for display (separate from HAND_HISTORY)
        LAST_HAND_WINNERS.with(|w| {
            *w.borrow_mut() = vec![winner_info];
        });
    }

    // Record to history canister (no showdown - single winner by fold)
    record_hand_to_history(state, &winners_for_history, false);

    state.pot = 0;
    state.side_pots.clear();
    state.phase = GamePhase::HandComplete;
    state.action_timer = None;

    // Mark players with 0 chips as broke (start their reload timer)
    let now = ic_cdk::api::time();
    for player in state.players.iter_mut().flatten() {
        if player.chips == 0 && player.broke_at.is_none() {
            player.broke_at = Some(now);
        } else if player.chips > 0 {
            player.broke_at = None;
        }
    }

    // Schedule auto-deal for next hand
    state.auto_deal_at = Some(ic_cdk::api::time() + AUTO_DEAL_DELAY_NS);
}

fn determine_winners(state: &mut TableState) {
    // Reveal the seed now that hand is ending (showdown)
    reveal_seed_on_hand_end(state);

    // Calculate side pots if not already done
    if state.side_pots.is_empty() {
        calculate_side_pots(state);
    }

    // Evaluate hands for all non-folded players
    let mut player_hands: Vec<(u8, HandRank, Principal, (Card, Card))> = Vec::new();

    for (i, player) in state.players.iter().enumerate() {
        if let Some(ref p) = player {
            if !p.has_folded {
                if let Some(cards) = p.hole_cards {
                    let hand_rank = evaluate_hand(&cards, &state.community_cards);
                    player_hands.push((i as u8, hand_rank, p.principal, cards));
                }
            }
        }
    }

    if player_hands.is_empty() {
        state.phase = GamePhase::HandComplete;
        return;
    }

    let mut winner_list = Vec::new();
    let mut chips_awarded: HashMap<u8, u64> = HashMap::new();

    // If no side pots, use simple main pot logic
    if state.side_pots.is_empty() {
        // Sort by hand rank (best first)
        player_hands.sort_by(|a, b| b.1.cmp(&a.1));

        let best_rank = &player_hands[0].1;
        let winners: Vec<_> = player_hands.iter()
            .filter(|(_, rank, _, _)| rank == best_rank)
            .collect();

        // Guard against division by zero (should never happen but be safe)
        if winners.is_empty() {
            state.phase = GamePhase::HandComplete;
            return;
        }
        let pot_share = state.pot / winners.len() as u64;
        let remainder = state.pot % winners.len() as u64;

        // Find which winner gets the remainder (first clockwise from dealer)
        let winner_seats: Vec<u8> = winners.iter().map(|(seat, _, _, _)| *seat).collect();
        let remainder_seat = first_clockwise_from_dealer(
            state.dealer_seat,
            &winner_seats,
            state.players.len()
        );

        for (seat, rank, principal, cards) in winners.iter() {
            let amount = if *seat == remainder_seat { pot_share + remainder } else { pot_share };
            let entry = chips_awarded.entry(*seat).or_insert(0);
            *entry = entry.saturating_add(amount);

            winner_list.push(Winner {
                seat: *seat,
                principal: *principal,
                amount,
                hand_rank: Some(rank.clone()),
                cards: Some(*cards),
            });
        }
    } else {
        // Process each side pot separately
        for side_pot in &state.side_pots {
            // Find best hand among eligible players
            let eligible_hands: Vec<_> = player_hands.iter()
                .filter(|(seat, _, _, _)| side_pot.eligible_players.contains(seat))
                .collect();

            if eligible_hands.is_empty() {
                continue;
            }

            // Find the best hand(s) among eligible players
            let best_rank = match eligible_hands.iter().map(|(_, rank, _, _)| rank).max() {
                Some(rank) => rank,
                None => continue, // No eligible hands for this pot
            };

            let pot_winners: Vec<_> = eligible_hands.iter()
                .filter(|(_, rank, _, _)| rank == best_rank)
                .collect();

            // Guard against division by zero
            if pot_winners.is_empty() {
                continue;
            }
            let pot_share = side_pot.amount / pot_winners.len() as u64;
            let remainder = side_pot.amount % pot_winners.len() as u64;

            // Find which winner gets the remainder (first clockwise from dealer)
            let winner_seats: Vec<u8> = pot_winners.iter().map(|(seat, _, _, _)| *seat).collect();
            let remainder_seat = first_clockwise_from_dealer(
                state.dealer_seat,
                &winner_seats,
                state.players.len()
            );

            for (seat, rank, principal, cards) in pot_winners.iter() {
                let amount = if *seat == remainder_seat { pot_share + remainder } else { pot_share };
                let entry = chips_awarded.entry(*seat).or_insert(0);
                *entry = entry.saturating_add(amount);

                // Only add to winner list once per player (aggregate amounts)
                if let Some(existing) = winner_list.iter_mut().find(|w| w.seat == *seat) {
                    existing.amount = existing.amount.saturating_add(amount);
                } else {
                    winner_list.push(Winner {
                        seat: *seat,
                        principal: *principal,
                        amount,
                        hand_rank: Some(rank.clone()),
                        cards: Some(*cards),
                    });
                }
            }
        }
    }

    // Build showdown players list BEFORE awarding chips (need to access chips_awarded)
    let showdown_players: Vec<ShowdownPlayer> = player_hands.iter().map(|(seat, rank, principal, cards)| {
        let amount_won = chips_awarded.get(seat).copied().unwrap_or(0);
        ShowdownPlayer {
            seat: *seat,
            principal: *principal,
            cards: Some(*cards),
            hand_rank: Some(rank.clone()),
            amount_won,
        }
    }).collect();

    // Award chips to winners (with overflow protection)
    for (seat, amount) in chips_awarded {
        if let Some(ref mut player) = state.players[seat as usize] {
            player.chips = player.chips.saturating_add(amount);
        }
    }

    // Update local history
    HAND_HISTORY.with(|h| {
        if let Some(last) = h.borrow_mut().last_mut() {
            last.winners = winner_list.clone();
            last.community_cards = state.community_cards.clone();
            last.showdown_players = showdown_players;
            CURRENT_ACTIONS.with(|a| {
                last.actions = a.borrow().clone();
            });
        }
    });

    // Store winners for display (separate from HAND_HISTORY since that gets a new entry when a new hand starts)
    LAST_HAND_WINNERS.with(|w| {
        *w.borrow_mut() = winner_list.clone();
    });

    // Record to history canister (went to showdown)
    record_hand_to_history(state, &winner_list, true);

    state.pot = 0;
    state.side_pots.clear();
    state.phase = GamePhase::HandComplete;
    state.action_timer = None;

    // Mark players with 0 chips as broke (start their reload timer)
    let now = ic_cdk::api::time();
    for player in state.players.iter_mut().flatten() {
        if player.chips == 0 && player.broke_at.is_none() {
            player.broke_at = Some(now);
        } else if player.chips > 0 {
            // Player has chips, clear broke status
            player.broke_at = None;
        }
    }

    // Schedule auto-deal for next hand
    state.auto_deal_at = Some(ic_cdk::api::time() + AUTO_DEAL_DELAY_NS);
}

// ============================================================================
// TIMEOUT HANDLING
// ============================================================================

/// Result of check_timeouts - indicates what action was taken
#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum TimeoutCheckResult {
    NoAction,
    PlayerTimedOut(u8), // Player at this seat timed out
    AutoDealReady, // Ready to auto-deal next hand
}

/// Check for timeouts, auto-fold, and auto-deal
/// This should be called periodically or before each action
#[ic_cdk::update]
fn check_timeouts() -> TimeoutCheckResult {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        ic_cdk::trap("Anonymous callers cannot perform this action");
    }

    // Run periodic cleanup of unbounded maps
    periodic_cleanup();

    let now = ic_cdk::api::time();
    // Mark players as disconnected if no heartbeat for 30 seconds
    const DISCONNECT_TIMEOUT_NS: u64 = 30 * 1_000_000_000;

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = match table.as_mut() {
            Some(s) => s,
            None => return TimeoutCheckResult::NoAction,
        };

        // Check for disconnected players (no heartbeat)
        for player in state.players.iter_mut().flatten() {
            if player.status == PlayerStatus::Active && now > player.last_seen + DISCONNECT_TIMEOUT_NS {
                player.status = PlayerStatus::Disconnected;
                // Also set sitting_out_since for the kick timer
                if player.sitting_out_since.is_none() {
                    player.sitting_out_since = Some(now);
                }
            }
        }

        // Check for broke players who haven't reloaded in time - sit them out
        let reload_timeout_ns = RELOAD_TIMEOUT_SECS * 1_000_000_000;
        for player in state.players.iter_mut().flatten() {
            if let Some(broke_time) = player.broke_at {
                if player.chips == 0 && now > broke_time + reload_timeout_ns {
                    // Player has been broke for too long, sit them out
                    player.status = PlayerStatus::SittingOut;
                    player.sitting_out_since = Some(now);
                    player.broke_at = None; // Clear so we don't keep checking
                }
            }
        }

        // Auto-kick players who have been sitting out or disconnected for too long
        // Only when not in an active hand (WaitingForPlayers or HandComplete)
        if state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete {
            let kick_timeout_ns = SITTING_OUT_KICK_SECS * 1_000_000_000;
            for i in 0..state.players.len() {
                if let Some(ref player) = state.players[i] {
                    if player.status == PlayerStatus::SittingOut || player.status == PlayerStatus::Disconnected {
                        // Use sitting_out_since if set, otherwise fall back to last_seen
                        // (for players who were already disconnected before upgrade added this field)
                        let idle_since = player.sitting_out_since.unwrap_or(player.last_seen);
                        if now > idle_since + kick_timeout_ns {
                            // Return chips to escrow balance before removing
                            let chips = player.chips;
                            let principal = player.principal;
                            if chips > 0 {
                                BALANCES.with(|b| {
                                    let mut balances = b.borrow_mut();
                                    let current = balances.entry(principal).or_insert(0);
                                    *current = current.saturating_add(chips);
                                });
                            }
                            // Remove player from seat
                            state.players[i] = None;
                        }
                    }
                }
            }
        }

        // Check for auto-deal first
        // If auto_deal_at is not set but we have 2+ active players in WaitingForPlayers/HandComplete, set it now
        if state.auto_deal_at.is_none() && (state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete) {
            let active_count = state.players.iter()
                .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
                .count();
            if active_count >= 2 {
                state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
            }
        }

        if let Some(auto_deal_time) = state.auto_deal_at {
            if now >= auto_deal_time && (state.phase == GamePhase::HandComplete || state.phase == GamePhase::WaitingForPlayers) {
                // Only signal auto-deal if we have enough active players with chips
                let active_count = state.players.iter()
                    .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
                    .count();

                if active_count >= 2 {
                    // Ready to auto-deal - frontend should call start_new_hand
                    return TimeoutCheckResult::AutoDealReady;
                }
                // Not enough players - clear auto-deal timer
                state.auto_deal_at = None;
            }
        }

        // Then check for player timeouts
        if let Some(ref timer) = state.action_timer {
            if now > timer.expires_at {
                let seat = timer.player_seat;

                // Auto-fold the player
                if let Some(ref mut player) = state.players[seat as usize] {
                    player.has_folded = true;
                    player.timeout_count = player.timeout_count.saturating_add(1);

                    // Sit them out if too many timeouts
                    if player.timeout_count >= MAX_TIMEOUTS_BEFORE_SITOUT {
                        player.status = PlayerStatus::SittingOut;
                        player.sitting_out_since = Some(now);
                    }

                    // Record the timeout as a fold with current phase
                    let current_phase = phase_to_string(&state.phase);
                    CURRENT_ACTIONS.with(|a| {
                        a.borrow_mut().push(ActionRecord {
                            seat,
                            action: PlayerAction::Fold,
                            timestamp: now,
                            phase: current_phase,
                            amount: 0, // Fold has no amount
                        });
                    });
                }

                // Advance the game
                advance_game(state);

                return TimeoutCheckResult::PlayerTimedOut(seat);
            }
        }

        TimeoutCheckResult::NoAction
    })
}

/// Player heartbeat to show they're connected
#[ic_cdk::update]
fn heartbeat() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    // Rate limit heartbeats to prevent DoS
    let rate_limited = HEARTBEAT_RATE_LIMITS.with(|r| {
        let mut limits = r.borrow_mut();
        let (last_time, count) = limits.get(&caller).copied().unwrap_or((0, 0));

        if now - last_time > RATE_LIMIT_WINDOW_NS {
            // New window
            limits.insert(caller, (now, 1));
            false
        } else if count >= MAX_HEARTBEATS_PER_SECOND {
            true // Rate limited
        } else {
            limits.insert(caller, (last_time, count + 1));
            false
        }
    });

    if rate_limited {
        return Err("Heartbeat rate limit exceeded".to_string());
    }

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        for player in state.players.iter_mut().flatten() {
            if player.principal == caller {
                player.last_seen = now;

                // Reconnect if they were disconnected
                if player.status == PlayerStatus::Disconnected {
                    player.status = PlayerStatus::Active;
                }

                return Ok(());
            }
        }

        Err("Not at table".to_string())
    })
}

/// Sit out (voluntarily)
#[ic_cdk::update]
fn sit_out() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        for player in state.players.iter_mut().flatten() {
            if player.principal == caller {
                player.status = PlayerStatus::SittingOut;
                player.sitting_out_since = Some(now);
                return Ok(());
            }
        }

        Err("Not at table".to_string())
    })
}

/// Sit back in
#[ic_cdk::update]
fn sit_in() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        for player in state.players.iter_mut().flatten() {
            if player.principal == caller {
                player.status = PlayerStatus::Active;
                player.timeout_count = 0;
                player.is_sitting_out_next_hand = false;
                player.last_seen = now;
                player.sitting_out_since = None; // Clear sitting out timer

                // Check if we should trigger auto-deal
                if state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete {
                    let active_count = state.players.iter()
                        .filter(|p| p.as_ref().map(|p| p.status == PlayerStatus::Active && p.chips > 0).unwrap_or(false))
                        .count();

                    if active_count >= 2 && state.auto_deal_at.is_none() {
                        state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
                    }
                }

                return Ok(());
            }
        }

        Err("Not at table".to_string())
    })
}

/// Request to sit out at the end of the current hand
#[ic_cdk::update]
fn sit_out_next_hand() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        for player in state.players.iter_mut().flatten() {
            if player.principal == caller {
                player.is_sitting_out_next_hand = true;
                return Ok(());
            }
        }

        Err("Not at table".to_string())
    })
}

/// Use time bank to extend action time
/// Returns remaining time bank seconds
#[ic_cdk::update]
fn use_time_bank() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Find caller's seat
        let player_seat = state.players.iter()
            .enumerate()
            .find(|(_, p)| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .map(|(i, _)| i as u8)
            .ok_or("Not at table")?;

        // Must be the player's turn
        if state.action_on != player_seat {
            return Err("Not your turn".to_string());
        }

        // Must have time bank remaining
        let player = state.players[player_seat as usize].as_mut().ok_or("Player not found")?;
        if player.time_bank_remaining == 0 {
            return Err("No time bank remaining".to_string());
        }

        // Check if timer is already using time bank
        if let Some(ref timer) = state.action_timer {
            if timer.using_time_bank {
                return Err("Already using time bank".to_string());
            }
        }

        // Use the time bank - extend the timer
        let time_bank_ns = player.time_bank_remaining * 1_000_000_000;
        player.time_bank_remaining = 0;

        state.action_timer = Some(ActionTimer {
            player_seat,
            started_at: now,
            expires_at: now + time_bank_ns,
            using_time_bank: true,
        });

        Ok(0) // Time bank is now depleted
    })
}

/// Voluntarily show your hole cards to the table
/// Only allowed after you've folded or at the end of the hand
#[ic_cdk::update]
fn show_cards() -> Result<(Card, Card), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }

    TABLE.with(|t| {
        let table = t.borrow();
        let state = table.as_ref().ok_or("Table not initialized")?;

        // Find the player
        let player = state.players.iter().flatten()
            .find(|p| p.principal == caller)
            .ok_or("Not at table")?;

        // Must have hole cards
        let cards = player.hole_cards.ok_or("No cards to show")?;

        // Can only show if folded or hand is complete
        if !player.has_folded && state.phase != GamePhase::HandComplete && state.phase != GamePhase::Showdown {
            return Err("Can only show cards after folding or at showdown".to_string());
        }

        // Record that this player showed
        SHOWN_CARDS.with(|s| {
            let mut shown = s.borrow_mut();
            let seats = shown.entry(state.hand_number).or_insert_with(Vec::new);
            if !seats.contains(&player.seat) {
                seats.push(player.seat);
            }
        });

        Ok(cards)
    })
}

/// Check if a player voluntarily showed their cards this hand
#[ic_cdk::query]
fn did_player_show(seat: u8) -> bool {
    TABLE.with(|t| {
        let table = t.borrow();
        if let Some(state) = table.as_ref() {
            SHOWN_CARDS.with(|s| {
                s.borrow()
                    .get(&state.hand_number)
                    .map(|seats| seats.contains(&seat))
                    .unwrap_or(false)
            })
        } else {
            false
        }
    })
}

/// Get cards for a player who voluntarily showed them
#[ic_cdk::query]
fn get_shown_cards(seat: u8) -> Option<(Card, Card)> {
    TABLE.with(|t| {
        let table = t.borrow();
        let state = table.as_ref()?;

        // Check if player showed
        let did_show = SHOWN_CARDS.with(|s| {
            s.borrow()
                .get(&state.hand_number)
                .map(|seats| seats.contains(&seat))
                .unwrap_or(false)
        });

        if !did_show {
            return None;
        }

        // Get the player's cards
        state.players.get(seat as usize)?
            .as_ref()?
            .hole_cards
    })
}

// ============================================================================
// QUERIES
// ============================================================================

/// Get the raw table state (admin/debug use - exposes all data)
/// RESTRICTED: Only controllers can access this to prevent cheating
#[ic_cdk::query]
fn get_table_state() -> Result<TableState, String> {
    // SECURITY: This exposes all cards including hole cards and deck
    // Only allow controllers to access this for debugging
    if !is_controller() {
        return Err("Unauthorized: controller access required".to_string());
    }
    TABLE.with(|t| {
        t.borrow().clone().ok_or("Table not initialized".to_string())
    })
}

/// Get the table view from the caller's perspective
/// This properly hides opponent hole cards unless at showdown
#[ic_cdk::query]
fn get_table_view() -> Option<TableView> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let table = t.borrow();
        let state = table.as_ref()?;

        // Find caller's seat
        let my_seat = state.players.iter()
            .enumerate()
            .find(|(_, p)| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .map(|(i, _)| i as u8);

        // Determine if we're at showdown (cards should be revealed)
        let is_showdown = state.phase == GamePhase::Showdown || state.phase == GamePhase::HandComplete;

        // Build player views with proper card visibility
        let player_views: Vec<Option<PlayerView>> = state.players.iter()
            .enumerate()
            .map(|(i, player_opt)| {
                player_opt.as_ref().map(|player| {
                    let is_self = my_seat == Some(i as u8);

                    // Check if this player voluntarily showed
                    let voluntarily_showed = SHOWN_CARDS.with(|s| {
                        s.borrow()
                            .get(&state.hand_number)
                            .map(|seats| seats.contains(&(i as u8)))
                            .unwrap_or(false)
                    });

                    // Determine if we can see this player's hole cards:
                    // 1. It's our own cards
                    // 2. It's showdown AND they haven't folded (winners revealed)
                    // 3. They voluntarily showed their cards
                    let can_see_cards = is_self ||
                        (is_showdown && !player.has_folded) ||
                        voluntarily_showed;

                    // Get display name if set
                    let display_name = DISPLAY_NAMES.with(|names| {
                        names.borrow().get(&player.principal).cloned()
                    });

                    PlayerView {
                        principal: player.principal,
                        seat: player.seat,
                        chips: player.chips,
                        hole_cards: if can_see_cards { player.hole_cards } else { None },
                        current_bet: player.current_bet,
                        has_folded: player.has_folded,
                        is_all_in: player.is_all_in,
                        status: player.status.clone(),
                        is_self,
                        display_name,
                    }
                })
            })
            .collect();

        // Calculate time remaining
        let time_remaining = state.action_timer.as_ref().map(|timer| {
            if now >= timer.expires_at {
                0
            } else {
                (timer.expires_at - now) / 1_000_000_000
            }
        });

        // Is it my turn?
        let is_my_turn = my_seat.map(|seat| seat == state.action_on).unwrap_or(false);

        // Get winners from the most recent completed hand
        let last_hand_winners = LAST_HAND_WINNERS.with(|w| w.borrow().clone());

        // Calculate call amount, can_check, can_raise for the caller
        let (call_amount, can_check, can_raise, my_time_bank) = if let Some(seat) = my_seat {
            if let Some(Some(player)) = state.players.get(seat as usize) {
                let to_call = if state.current_bet > player.current_bet {
                    state.current_bet - player.current_bet
                } else {
                    0
                };

                // BB can check preflop if no raise
                let is_bb_with_option = state.phase == GamePhase::PreFlop
                    && state.bb_has_option
                    && seat == state.big_blind_seat
                    && state.current_bet == state.config.big_blind;

                let check_ok = to_call == 0 || is_bb_with_option;
                let raise_ok = player.chips > to_call && !player.is_all_in;

                (to_call, check_ok, raise_ok, player.time_bank_remaining)
            } else {
                (0, false, false, 0)
            }
        } else {
            (0, false, false, 0)
        };

        // Check if current action timer is using time bank
        let using_time_bank = state.action_timer.as_ref()
            .map(|t| t.using_time_bank)
            .unwrap_or(false);

        Some(TableView {
            id: state.id,
            config: state.config.clone(),
            players: player_views,
            community_cards: state.community_cards.clone(),
            pot: state.pot,
            side_pots: state.side_pots.clone(),
            current_bet: state.current_bet,
            min_raise: state.min_raise,
            phase: state.phase.clone(),
            dealer_seat: state.dealer_seat,
            small_blind_seat: state.small_blind_seat,
            big_blind_seat: state.big_blind_seat,
            action_on: state.action_on,
            time_remaining_secs: time_remaining,
            time_bank_remaining_secs: if my_seat.is_some() { Some(my_time_bank) } else { None },
            using_time_bank,
            is_my_turn,
            my_seat,
            hand_number: state.hand_number,
            shuffle_proof: state.shuffle_proof.clone(),
            last_hand_winners,
            call_amount,
            can_check,
            can_raise,
            min_bet: state.config.big_blind,
            last_action: state.last_action.clone(),
        })
    })
}

#[ic_cdk::query]
fn get_my_cards() -> Option<(Card, Card)> {
    let caller = ic_cdk::api::msg_caller();

    TABLE.with(|t| {
        let table = t.borrow();
        if let Some(ref state) = *table {
            for player in state.players.iter().flatten() {
                if player.principal == caller {
                    return player.hole_cards;
                }
            }
        }
        None
    })
}

#[ic_cdk::query]
fn get_community_cards() -> Vec<Card> {
    TABLE.with(|t| {
        t.borrow().as_ref()
            .map(|s| s.community_cards.clone())
            .unwrap_or_default()
    })
}

#[ic_cdk::query]
fn get_pot() -> u64 {
    TABLE.with(|t| {
        t.borrow().as_ref().map(|s| s.pot).unwrap_or(0)
    })
}

#[ic_cdk::query]
fn get_shuffle_proof() -> Option<ShuffleProof> {
    // Return from hand history to get the revealed_seed after hand completes
    HAND_HISTORY.with(|h| {
        h.borrow().last().map(|hh| hh.shuffle_proof.clone())
    })
}

#[ic_cdk::query]
fn get_hand_history(hand_number: u64) -> Option<HandHistory> {
    HAND_HISTORY.with(|h| {
        h.borrow().iter().find(|hh| hh.hand_number == hand_number).cloned()
    })
}

#[ic_cdk::query]
fn get_action_timer() -> Option<ActionTimer> {
    TABLE.with(|t| {
        t.borrow().as_ref().and_then(|s| s.action_timer.clone())
    })
}

#[ic_cdk::query]
fn get_time_remaining() -> Option<u64> {
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        t.borrow().as_ref().and_then(|s| {
            s.action_timer.as_ref().map(|timer| {
                if now >= timer.expires_at {
                    0
                } else {
                    (timer.expires_at - now) / 1_000_000_000 // Convert to seconds
                }
            })
        })
    })
}

#[ic_cdk::query]
fn verify_shuffle(seed_hash: String, revealed_seed: String) -> bool {
    let seed_bytes = match hex::decode(&revealed_seed) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut hasher = Sha256::new();
    hasher.update(&seed_bytes);
    let computed_hash = hex::encode(hasher.finalize());

    // Case-insensitive comparison to handle potential case differences
    // from serialization/deserialization through Candid
    computed_hash.to_lowercase() == seed_hash.to_lowercase()
}

/// Get current player count (for lobby display)
#[ic_cdk::query]
fn get_player_count() -> u8 {
    TABLE.with(|t| {
        t.borrow().as_ref()
            .map(|s| s.players.iter().filter(|p| p.is_some()).count() as u8)
            .unwrap_or(0)
    })
}

/// Get max players (for lobby display)
#[ic_cdk::query]
fn get_max_players() -> u8 {
    // First try TABLE_CONFIG (set at init, always available)
    // Fall back to TABLE state, then default to 6
    TABLE_CONFIG.with(|c| {
        c.borrow().as_ref()
            .map(|cfg| cfg.max_players)
            .unwrap_or_else(|| {
                TABLE.with(|t| {
                    t.borrow().as_ref()
                        .map(|s| s.config.max_players)
                        .unwrap_or(6)
                })
            })
    })
}

// ============================================================================
// STABLE MEMORY - Persistence across upgrades
// ============================================================================

/// Persistent state across upgrades.
/// ALL fields must have #[serde(default)] to ensure forward-compatible deserialization.
/// Adding a new field without #[serde(default)] will cause upgrade failures.
#[derive(CandidType, Deserialize, Default)]
struct PersistentState {
    #[serde(default)]
    schema_version: Option<u32>, // Increment when making breaking changes; Option for Candid compatibility
    #[serde(default)]
    balances: Vec<(Principal, u64)>,
    #[serde(default)]
    verified_deposits: Vec<(u64, Principal)>,
    #[serde(default)]
    controllers: Vec<Principal>,
    #[serde(default)]
    history_id: Option<Principal>,
    #[serde(default)]
    dev_mode: bool, // Kept for deserialization compatibility, but always ignored
    #[serde(default)]
    table_config: Option<TableConfig>,
    #[serde(default)]
    table_state: Option<TableState>, // Save active game state
    #[serde(default)]
    hand_history: Vec<HandHistory>,
    #[serde(default)]
    current_actions: Vec<ActionRecord>,
    #[serde(default)]
    starting_chips: Vec<(u8, u64)>,
    #[serde(default)]
    rate_limits: Vec<(Principal, (u64, u32))>,
    #[serde(default)]
    shown_cards: Vec<(u64, Vec<u8>)>, // hand_number -> seats that showed
    #[serde(default)]
    current_seed: Option<Vec<u8>>, // Persist seed for mid-hand upgrades
    #[serde(default)]
    display_names: Vec<(Principal, String)>, // Custom display names
    #[serde(default)]
    doge_balances: Option<Vec<(Principal, u64)>>, // Internal DOGE balance tracking; Option for Candid compatibility
    #[serde(default)]
    doge_credited_utxos: Option<Vec<String>>, // Credited DOGE UTXOs; Option for Candid compatibility
    #[serde(default)]
    min_verified_block_index: Option<u64>, // Watermark: highest pruned block index; Option for Candid compatibility
    #[serde(default)]
    failed_ckbtc_sweeps: Option<Vec<(Principal, u64)>>, // (caller, amount) for failed ckBTC sweeps awaiting retry
}

const CURRENT_SCHEMA_VERSION: u32 = 2;

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = PersistentState {
        schema_version: Some(CURRENT_SCHEMA_VERSION),
        balances: BALANCES.with(|b| b.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        verified_deposits: VERIFIED_DEPOSITS.with(|v| v.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        controllers: CONTROLLERS.with(|c| c.borrow().clone()),
        history_id: HISTORY_ID.with(|h| *h.borrow()),
        dev_mode: false, // Always false, kept for backwards compatibility
        table_config: TABLE_CONFIG.with(|c| c.borrow().clone()),
        table_state: TABLE.with(|t| t.borrow().clone()), // Save active game state
        hand_history: HAND_HISTORY.with(|h| h.borrow().clone()),
        current_actions: CURRENT_ACTIONS.with(|a| a.borrow().clone()),
        starting_chips: STARTING_CHIPS.with(|s| s.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        rate_limits: RATE_LIMITS.with(|r| r.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        shown_cards: SHOWN_CARDS.with(|s| s.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
        current_seed: CURRENT_SEED.with(|s| s.borrow().clone()),
        display_names: DISPLAY_NAMES.with(|d| d.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
        doge_balances: Some(DOGE_BALANCES.with(|b| b.borrow().iter().map(|(k, v)| (*k, *v)).collect())),
        doge_credited_utxos: Some(DOGE_CREDITED_UTXOS.with(|c| c.borrow().iter().cloned().collect())),
        min_verified_block_index: Some(MIN_VERIFIED_BLOCK_INDEX.with(|m| *m.borrow())),
        failed_ckbtc_sweeps: Some(FAILED_CKBTC_SWEEPS.with(|f| f.borrow().clone())),
    };

    if let Err(e) = ic_cdk::storage::stable_save((state,)) {
        ic_cdk::println!("CRITICAL: Failed to save state to stable memory: {:?}", e);
        // Log but don't panic - allow upgrade to proceed
        // This is safer than trapping which could brick the canister
    }
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let restore_result: Result<(PersistentState,), _> = ic_cdk::storage::stable_restore();

    let state = match restore_result {
        Ok((s,)) => {
            ic_cdk::println!("State restored successfully (schema v{})", s.schema_version.unwrap_or(1));
            s
        }
        Err(e) => {
            // Log the error prominently. We still panic to protect funds —
            // a silent empty state would lose all user balances.
            // However, if this is a fresh canister (reinstall), we allow empty init.
            let err_msg = format!("{:?}", e);
            if err_msg.contains("No more values on the wire") {
                // Fresh canister or reinstall — no prior state to restore
                ic_cdk::println!("WARNING: No prior state found (fresh install). Starting with empty state.");
                PersistentState {
                    schema_version: Some(CURRENT_SCHEMA_VERSION),
                    ..Default::default()
                }
            } else {
                panic!("CRITICAL: Failed to restore state from stable memory: {:?}. \
                        Upgrade REJECTED to protect user funds. \
                        If you used --mode reinstall, that DESTROYS ALL DATA. \
                        Always use --mode upgrade for production canisters.", e);
            }
        }
    };

    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        for (k, v) in state.balances {
            balances.insert(k, v);
        }
    });

    VERIFIED_DEPOSITS.with(|v| {
        let mut deposits = v.borrow_mut();
        for (k, val) in state.verified_deposits {
            deposits.insert(k, val);
        }
    });

    CONTROLLERS.with(|c| {
        *c.borrow_mut() = state.controllers;
    });

    HISTORY_ID.with(|h| {
        *h.borrow_mut() = state.history_id;
    });

    // dev_mode is intentionally NOT restored - it's permanently disabled
    // The field is kept in PersistentState only for backwards compatibility
    let _ = state.dev_mode; // Explicitly ignore

    // Restore table state if it exists, otherwise initialize from config
    if let Some(table_state) = state.table_state {
        TABLE.with(|t| {
            *t.borrow_mut() = Some(table_state);
        });
        // Also restore config
        if let Some(config) = state.table_config {
            TABLE_CONFIG.with(|c| {
                *c.borrow_mut() = Some(config);
            });
        }
    } else if let Some(config) = state.table_config {
        // No active game state, initialize fresh
        init_table_state(config);
    }

    // Restore hand history
    HAND_HISTORY.with(|h| {
        *h.borrow_mut() = state.hand_history;
    });

    // Restore current actions
    CURRENT_ACTIONS.with(|a| {
        *a.borrow_mut() = state.current_actions;
    });

    // Restore starting chips
    STARTING_CHIPS.with(|s| {
        let mut chips = s.borrow_mut();
        for (k, v) in state.starting_chips {
            chips.insert(k, v);
        }
    });

    // Restore rate limits
    RATE_LIMITS.with(|r| {
        let mut limits = r.borrow_mut();
        for (k, v) in state.rate_limits {
            limits.insert(k, v);
        }
    });

    // Restore shown cards
    SHOWN_CARDS.with(|s| {
        let mut shown = s.borrow_mut();
        for (k, v) in state.shown_cards {
            shown.insert(k, v);
        }
    });

    // Restore current seed (for mid-hand upgrades)
    CURRENT_SEED.with(|s| {
        *s.borrow_mut() = state.current_seed;
    });

    // Restore display names
    DISPLAY_NAMES.with(|d| {
        let mut names = d.borrow_mut();
        for (k, v) in state.display_names {
            names.insert(k, v);
        }
    });

    // Restore DOGE balances (field may be absent in old state)
    if let Some(doge_balances) = state.doge_balances {
        DOGE_BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            for (k, v) in doge_balances {
                balances.insert(k, v);
            }
        });
    }

    // Restore DOGE credited UTXOs (field may be absent in old state)
    if let Some(doge_credited_utxos) = state.doge_credited_utxos {
        DOGE_CREDITED_UTXOS.with(|c| {
            let mut credited = c.borrow_mut();
            for utxo_id in doge_credited_utxos {
                credited.insert(utxo_id);
            }
        });
    }

    // Restore watermark for VERIFIED_DEPOSITS pruning (field may be absent in old state)
    if let Some(min_block) = state.min_verified_block_index {
        MIN_VERIFIED_BLOCK_INDEX.with(|m| {
            *m.borrow_mut() = min_block;
        });
    }

    // Restore failed ckBTC sweeps (field may be absent in old state)
    if let Some(failed_sweeps) = state.failed_ckbtc_sweeps {
        FAILED_CKBTC_SWEEPS.with(|f| {
            *f.borrow_mut() = failed_sweeps;
        });
    }
}

// ============================================================================
// CKBTC MINTER INTEGRATION - For native BTC deposits
// ============================================================================

// ckBTC Minter canister ID (mainnet)
const CKBTC_MINTER_CANISTER: &str = "mqygn-kiaaa-aaaar-qaadq-cai";

/// Arguments for get_btc_address call to ckBTC minter
#[derive(CandidType, Deserialize)]
struct GetBtcAddressArgs {
    owner: Option<Principal>,
    subaccount: Option<Vec<u8>>,
}

/// Arguments for update_balance call to ckBTC minter
#[derive(CandidType, Deserialize)]
struct UpdateBalanceArgs {
    owner: Option<Principal>,
    subaccount: Option<Vec<u8>>,
}

/// Derive a per-user subaccount from their principal
/// This ensures each user gets a unique BTC deposit address within the canister
fn principal_to_subaccount(principal: &Principal) -> Vec<u8> {
    let mut subaccount = vec![0u8; 32];
    let principal_bytes = principal.as_slice();
    subaccount[0] = principal_bytes.len() as u8;
    subaccount[1..1 + principal_bytes.len()].copy_from_slice(principal_bytes);
    subaccount
}

/// UTXO info from ckBTC minter
#[derive(CandidType, Deserialize, Clone, Debug)]
struct Utxo {
    outpoint: UtxoOutpoint,
    value: u64,
    height: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct UtxoOutpoint {
    txid: Vec<u8>,
    vout: u32,
}

/// Status of a UTXO after update_balance
#[derive(CandidType, Deserialize, Clone, Debug)]
enum UtxoStatus {
    ValueTooSmall(Utxo),
    Tainted(Utxo),
    Checked(Utxo),
    Minted { block_index: u64, minted_amount: u64, utxo: Utxo },
}

/// Error from update_balance
#[derive(CandidType, Deserialize, Clone, Debug)]
enum UpdateBalanceError {
    GenericError { error_code: u64, error_message: String },
    TemporarilyUnavailable(String),
    AlreadyProcessing,
    NoNewUtxos { required_confirmations: u32, pending_utxos: Option<Vec<PendingUtxo>> },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct PendingUtxo {
    outpoint: UtxoOutpoint,
    value: u64,
    confirmations: u32,
}

/// Get a BTC deposit address for a user
/// This calls the ckBTC minter to get a unique Bitcoin address for the caller
/// The user can send real BTC to this address, and after confirmations,
/// call update_btc_balance to mint ckBTC to their wallet
#[ic_cdk::update]
async fn get_btc_deposit_address() -> Result<String, String> {
    let caller = ic_cdk::api::msg_caller();

    // Reject anonymous callers - ckBTC minter requires authenticated identity
    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity to get a BTC deposit address. Anonymous users cannot receive Bitcoin deposits.".to_string());
    }

    // Only allow for BTC tables
    let currency = get_table_currency();
    if currency != Currency::BTC {
        return Err("This function is only available for BTC tables".to_string());
    }

    let minter = Principal::from_text(CKBTC_MINTER_CANISTER)
        .map_err(|_| "Invalid minter canister ID".to_string())?;

    let subaccount = principal_to_subaccount(&caller);
    let args = GetBtcAddressArgs {
        owner: Some(ic_cdk::api::canister_self()),
        subaccount: Some(subaccount),
    };

    let result: Result<(String,), _> = ic_cdk::call(minter, "get_btc_address", (args,)).await;

    match result {
        Ok((address,)) => Ok(address),
        Err((code, msg)) => Err(format!("Failed to get BTC address: {:?} - {}", code, msg)),
    }
}

/// Update BTC balance - call this after sending BTC to the deposit address
/// This calls the ckBTC minter to check for new UTXOs and mint ckBTC
/// Returns the status of any UTXOs found
#[ic_cdk::update]
async fn update_btc_balance() -> Result<Vec<UtxoStatus>, String> {
    let caller = ic_cdk::api::msg_caller();

    // Reject anonymous callers - ckBTC minter requires authenticated identity
    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity to check for Bitcoin deposits. Anonymous users cannot receive Bitcoin deposits.".to_string());
    }

    // Only allow for BTC tables
    let currency = get_table_currency();
    if currency != Currency::BTC {
        return Err("This function is only available for BTC tables".to_string());
    }

    let minter = Principal::from_text(CKBTC_MINTER_CANISTER)
        .map_err(|_| "Invalid minter canister ID".to_string())?;

    let subaccount = principal_to_subaccount(&caller);
    let args = UpdateBalanceArgs {
        owner: Some(ic_cdk::api::canister_self()),
        subaccount: Some(subaccount.clone()),
    };

    #[derive(CandidType, Deserialize)]
    enum UpdateBalanceResult {
        Ok(Vec<UtxoStatus>),
        Err(UpdateBalanceError),
    }

    let result: Result<(UpdateBalanceResult,), _> = ic_cdk::call(minter, "update_balance", (args,)).await;

    let statuses = match result {
        Ok((UpdateBalanceResult::Ok(statuses),)) => statuses,
        Ok((UpdateBalanceResult::Err(err),)) => {
            return match err {
                UpdateBalanceError::NoNewUtxos { required_confirmations, pending_utxos } => {
                    if let Some(pending) = pending_utxos {
                        if !pending.is_empty() {
                            let first = &pending[0];
                            Err(format!(
                                "Waiting for confirmations: {} of {} required. {} pending UTXOs.",
                                first.confirmations, required_confirmations, pending.len()
                            ))
                        } else {
                            Err("No new BTC deposits found. Send BTC to your deposit address first.".to_string())
                        }
                    } else {
                        Err("No new BTC deposits found. Send BTC to your deposit address first.".to_string())
                    }
                },
                UpdateBalanceError::AlreadyProcessing => {
                    Err("Balance update already in progress. Please wait.".to_string())
                },
                UpdateBalanceError::TemporarilyUnavailable(msg) => {
                    Err(format!("ckBTC minter temporarily unavailable: {}", msg))
                },
                UpdateBalanceError::GenericError { error_message, .. } => {
                    Err(format!("Error updating balance: {}", error_message))
                },
            };
        },
        Err((code, msg)) => return Err(format!("Failed to update balance: {:?} - {}", code, msg)),
    };

    // Sweep any minted ckBTC from the user's subaccount to the canister's main account
    // and credit the user's internal BALANCES
    let mut total_minted: u64 = 0;
    for status in &statuses {
        if let UtxoStatus::Minted { minted_amount, .. } = status {
            total_minted = total_minted.saturating_add(*minted_amount);
        }
    }

    if total_minted > 0 {
        let ckbtc_fee: u64 = 10; // 10 satoshis ckBTC transfer fee
        let sweep_amount = total_minted.saturating_sub(ckbtc_fee);

        if sweep_amount > 0 {
            let ledger_id = currency.ledger_canister();
            let subaccount_arr: [u8; 32] = subaccount.clone().try_into()
                .unwrap_or([0u8; 32]);
            let transfer_args = TransferArg {
                from_subaccount: Some(subaccount_arr),
                to: Account {
                    owner: ic_cdk::api::canister_self(),
                    subaccount: None,
                },
                amount: Nat::from(sweep_amount),
                fee: Some(Nat::from(ckbtc_fee)),
                memo: None,
                created_at_time: Some(ic_cdk::api::time()),
            };

            let transfer_result: Result<(Result<Nat, TransferError>,), _> =
                ic_cdk::call(ledger_id, "icrc1_transfer", (transfer_args,)).await;

            match transfer_result {
                Ok((Ok(_),)) => {
                    // Credit the caller's escrow balance
                    BALANCES.with(|b| {
                        let mut balances = b.borrow_mut();
                        let current = balances.get(&caller).copied().unwrap_or(0);
                        balances.insert(caller, current.saturating_add(sweep_amount));
                    });
                    ic_cdk::println!("Swept {} sats ckBTC to main account for {}", sweep_amount, caller);
                }
                Ok((Err(e),)) => {
                    ic_cdk::println!("WARNING: ckBTC sweep failed for {}: {:?}. Funds in subaccount. Recording for retry.", caller, e);
                    FAILED_CKBTC_SWEEPS.with(|f| {
                        f.borrow_mut().push((caller, sweep_amount));
                    });
                }
                Err((code, msg)) => {
                    ic_cdk::println!("WARNING: ckBTC sweep call failed for {}: {:?} - {}. Recording for retry.", caller, code, msg);
                    FAILED_CKBTC_SWEEPS.with(|f| {
                        f.borrow_mut().push((caller, sweep_amount));
                    });
                }
            }
        }
    }

    Ok(statuses)
}

/// Retry a previously failed ckBTC sweep. When the sweep from a user's subaccount
/// to the canister's main account fails after ckBTC was minted, the amount is recorded.
/// The user can call this to retry sweeping those funds into their balance.
#[ic_cdk::update]
async fn retry_ckbtc_sweep() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot retry sweeps".to_string());
    }

    // Find and remove all failed sweeps for this caller
    let failed: Vec<u64> = FAILED_CKBTC_SWEEPS.with(|f| {
        let mut sweeps = f.borrow_mut();
        let mut caller_amounts = Vec::new();
        sweeps.retain(|(p, amount)| {
            if *p == caller {
                caller_amounts.push(*amount);
                false // remove from list
            } else {
                true // keep
            }
        });
        caller_amounts
    });

    if failed.is_empty() {
        return Err("No failed sweeps to retry".to_string());
    }

    let total_amount: u64 = failed.iter().copied().fold(0u64, |acc, x| acc.saturating_add(x));

    // Compute the user's deposit subaccount
    let subaccount = {
        let mut hasher = Sha256::new();
        hasher.update(b"cleardeck-deposit:");
        hasher.update(caller.as_slice());
        hasher.finalize().to_vec()
    };

    let currency = TABLE_CONFIG.with(|c| {
        c.borrow().as_ref().map(|tc| tc.currency.clone())
    }).ok_or("Table not configured")?;

    let ledger_id = currency.ledger_canister();
    let ckbtc_fee = 10u64; // ckBTC transfer fee: 10 satoshis

    if total_amount <= ckbtc_fee {
        return Err(format!("Failed sweep amount ({}) is too small to cover the fee ({})", total_amount, ckbtc_fee));
    }

    let sweep_amount = total_amount.saturating_sub(ckbtc_fee);
    let subaccount_arr: [u8; 32] = subaccount.try_into().unwrap_or([0u8; 32]);

    let transfer_args = TransferArg {
        from_subaccount: Some(subaccount_arr),
        to: Account {
            owner: ic_cdk::api::canister_self(),
            subaccount: None,
        },
        amount: Nat::from(sweep_amount),
        fee: Some(Nat::from(ckbtc_fee)),
        memo: None,
        created_at_time: Some(ic_cdk::api::time()),
    };

    let transfer_result: Result<(Result<Nat, TransferError>,), _> =
        ic_cdk::call(ledger_id, "icrc1_transfer", (transfer_args,)).await;

    match transfer_result {
        Ok((Ok(_),)) => {
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(sweep_amount));
            });
            ic_cdk::println!("Retry sweep succeeded: {} sats ckBTC for {}", sweep_amount, caller);
            Ok(sweep_amount)
        }
        Ok((Err(e),)) => {
            // Re-record the failed sweep so user can try again
            FAILED_CKBTC_SWEEPS.with(|f| {
                f.borrow_mut().push((caller, total_amount));
            });
            Err(format!("Sweep retry failed: {:?}. Your funds are safe — try again later.", e))
        }
        Err((code, msg)) => {
            FAILED_CKBTC_SWEEPS.with(|f| {
                f.borrow_mut().push((caller, total_amount));
            });
            Err(format!("Sweep retry call failed: {:?} - {}. Your funds are safe — try again later.", code, msg))
        }
    }
}

/// Query: check if the caller has any failed ckBTC sweeps pending retry.
#[ic_cdk::query]
fn get_failed_sweeps() -> Vec<u64> {
    let caller = ic_cdk::api::msg_caller();
    FAILED_CKBTC_SWEEPS.with(|f| {
        f.borrow().iter()
            .filter(|(p, _)| *p == caller)
            .map(|(_, amount)| *amount)
            .collect()
    })
}

/// Arguments for retrieve_btc_with_approval call to ckBTC minter
#[derive(CandidType, Deserialize, Debug)]
struct RetrieveBtcWithApprovalArgs {
    address: String,
    amount: u64,
    from_subaccount: Option<Vec<u8>>,
}

#[derive(CandidType, Deserialize, Debug)]
struct RetrieveBtcOk {
    block_index: u64,
}

#[derive(CandidType, Deserialize, Debug)]
enum RetrieveBtcError {
    MalformedAddress(String),
    AlreadyProcessing,
    AmountTooLow(u64),
    InsufficientFunds { balance: u64 },
    InsufficientAllowance { allowance: u64 },
    TemporarilyUnavailable(String),
    GenericError { error_code: u64, error_message: String },
}

/// Withdraw ckBTC back to a Bitcoin address
/// Flow: approve minter to spend ckBTC → minter burns ckBTC and sends BTC
#[ic_cdk::update]
async fn withdraw_btc(btc_address: String, amount: u64) -> Result<u64, String> {
    check_rate_limit()?;
    let caller = ic_cdk::api::msg_caller();

    if caller == Principal::anonymous() {
        return Err("Please log in to withdraw BTC".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::BTC {
        return Err("This function is only available for BTC tables".to_string());
    }

    // Basic BTC address validation
    if btc_address.is_empty() {
        return Err("BTC address cannot be empty".to_string());
    }

    let ckbtc_fee: u64 = 10; // 10 satoshis ckBTC transfer fee
    let min_btc_withdrawal: u64 = 10_000; // Minter minimum ~10k sats

    if amount < min_btc_withdrawal {
        return Err(format!("Minimum BTC withdrawal is {} satoshis", min_btc_withdrawal));
    }

    // Check and deduct internal balance first
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        if current < amount {
            return Err(format!("Insufficient ckBTC balance. Have {} sats, need {}", current, amount));
        }
        balances.insert(caller, current.saturating_sub(amount));
        Ok(())
    })?;

    let minter = Principal::from_text(CKBTC_MINTER_CANISTER)
        .map_err(|_| "Invalid minter canister ID".to_string())?;
    let ledger_id = currency.ledger_canister();

    let from_subaccount = principal_to_subaccount(&caller);

    // Step 1: Approve minter to spend ckBTC from user's subaccount
    use icrc_ledger_types::icrc2::approve::{ApproveArgs, ApproveError};

    let approve_args = ApproveArgs {
        from_subaccount: None, // Main account (balances are held on canister's main account)
        spender: Account {
            owner: minter,
            subaccount: None,
        },
        amount: Nat::from(amount + ckbtc_fee),
        expected_allowance: None,
        expires_at: None,
        fee: Some(Nat::from(ckbtc_fee)),
        memo: None,
        created_at_time: Some(ic_cdk::api::time()),
    };

    let approve_result: Result<(Result<Nat, ApproveError>,), _> =
        ic_cdk::call(ledger_id, "icrc2_approve", (approve_args,)).await;

    match approve_result {
        Ok((Ok(_),)) => {},
        Ok((Err(e),)) => {
            // Refund balance on failure
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(amount));
            });
            return Err(format!("Failed to approve minter: {:?}", e));
        }
        Err((code, msg)) => {
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(amount));
            });
            return Err(format!("Approve call failed: {:?} - {}", code, msg));
        }
    }

    // Step 2: Call retrieve_btc_with_approval on the minter
    let retrieve_args = RetrieveBtcWithApprovalArgs {
        address: btc_address,
        amount: amount - ckbtc_fee, // Minter expects net amount after fee
        from_subaccount: None,
    };

    #[derive(CandidType, Deserialize)]
    enum RetrieveBtcResult {
        Ok(RetrieveBtcOk),
        Err(RetrieveBtcError),
    }

    let result: Result<(RetrieveBtcResult,), _> =
        ic_cdk::call(minter, "retrieve_btc_with_approval", (retrieve_args,)).await;

    match result {
        Ok((RetrieveBtcResult::Ok(ok),)) => Ok(ok.block_index),
        Ok((RetrieveBtcResult::Err(e),)) => {
            // Refund on minter error
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(amount));
            });
            let msg = match e {
                RetrieveBtcError::MalformedAddress(a) => format!("Invalid BTC address: {}", a),
                RetrieveBtcError::AmountTooLow(min) => format!("Amount too low. Minimum: {} sats", min),
                RetrieveBtcError::InsufficientFunds { balance } => format!("Insufficient funds in minter. Balance: {} sats", balance),
                RetrieveBtcError::InsufficientAllowance { allowance } => format!("Insufficient allowance: {} sats", allowance),
                RetrieveBtcError::AlreadyProcessing => "Withdrawal already processing".to_string(),
                RetrieveBtcError::TemporarilyUnavailable(m) => format!("Minter unavailable: {}", m),
                RetrieveBtcError::GenericError { error_message, .. } => error_message,
            };
            Err(msg)
        }
        Err((code, msg)) => {
            BALANCES.with(|b| {
                let mut balances = b.borrow_mut();
                let current = balances.get(&caller).copied().unwrap_or(0);
                balances.insert(caller, current.saturating_add(amount));
            });
            Err(format!("retrieve_btc_with_approval failed: {:?} - {}", code, msg))
        }
    }
}

// ============================================================================
// NATIVE ETH DEPOSIT - Threshold ECDSA + EVM RPC for ETH → ckETH conversion
// ============================================================================
//
// Flow (mirrors BTC deposit):
// 1. get_eth_deposit_address() → derives unique Ethereum address per user via threshold ECDSA
// 2. User sends ETH to that address (simple ETH transfer, like BTC)
// 3. sweep_eth_to_cketh() → canister checks ETH balance, signs deposit(bytes32) tx
//    to ckETH helper contract, submits to Ethereum via EVM RPC
// 4. ~20 min later, ckETH minter auto-mints ckETH to user's ICP principal
// 5. User deposits ckETH to table via existing ICRC-2 approve flow

// ckETH helper contract on Ethereum mainnet (for deposit(bytes32))
const CKETH_HELPER_CONTRACT: &str = "7574eB42cA208A4f6960ECCAfDF186D627dCC175";
// EVM RPC canister on ICP mainnet
const EVM_RPC_CANISTER: &str = "7hfb6-caaaa-aaaar-qadga-cai";
// Threshold ECDSA key name (mainnet production = key_1)
const ECDSA_KEY_NAME: &str = "key_1";
// Ethereum mainnet chain ID
const ETH_CHAIN_ID: u64 = 1;
// Gas limit for depositEth contract call (~33k actual, padded for safety)
const DEPOSIT_ETH_GAS_LIMIT: u64 = 60_000;
// Cycles to attach per EVM RPC call
const EVM_RPC_CYCLES: u128 = 10_000_000_000; // 10B as recommended by EVM RPC skill
// Cycles for sign_with_ecdsa (key_1 on 34-node subnet)
const ECDSA_SIGN_CYCLES: u128 = 26_153_846_153;

// --- Threshold ECDSA types ---

#[derive(CandidType, Deserialize)]
struct EcdsaPublicKeyArgument {
    canister_id: Option<Principal>,
    derivation_path: Vec<Vec<u8>>,
    key_id: EcdsaKeyId,
}

#[derive(CandidType, Deserialize)]
struct EcdsaPublicKeyResponse {
    public_key: Vec<u8>,
    chain_code: Vec<u8>,
}

#[derive(CandidType, Deserialize)]
struct SignWithEcdsaArgument {
    message_hash: Vec<u8>,
    derivation_path: Vec<Vec<u8>>,
    key_id: EcdsaKeyId,
}

#[derive(CandidType, Deserialize)]
struct SignWithEcdsaResponse {
    signature: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone)]
struct EcdsaKeyId {
    curve: EcdsaCurve,
    name: String,
}

#[derive(CandidType, Deserialize, Clone)]
enum EcdsaCurve {
    #[serde(rename = "secp256k1")]
    Secp256k1,
}

// EVM RPC types from official crate
use evm_rpc_types::{
    RpcServices, EthMainnetService as EvmEthMainnetService,
    MultiRpcResult,
};

// --- ETH deposit result types ---

#[derive(CandidType, Deserialize, Clone, Debug)]
struct EthDepositInfo {
    eth_address: String,
    min_deposit_wei: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
enum SweepStatus {
    NoBalance,
    InsufficientForGas { balance_wei: String, estimated_gas_wei: String },
    Swept { tx_hash: String, amount_wei: String, gas_cost_wei: String },
}

// --- Helper functions ---

fn eth_keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn derivation_path_for(user: &Principal) -> Vec<Vec<u8>> {
    vec![
        vec![1u8], // schema version
        user.as_slice().to_vec(),
    ]
}

fn ecdsa_key_id() -> EcdsaKeyId {
    EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: ECDSA_KEY_NAME.to_string(),
    }
}

/// Convert compressed SEC1 public key to Ethereum address
fn pubkey_to_eth_address(pubkey_bytes: &[u8]) -> Result<String, String> {
    let pub_key = K256PublicKey::from_sec1_bytes(pubkey_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    let uncompressed = pub_key.to_encoded_point(false);
    let hash = eth_keccak256(&uncompressed.as_bytes()[1..]); // skip 0x04 prefix
    Ok(format!("0x{}", hex::encode(&hash[12..])))
}

/// Encode an ICP principal as bytes32 for the ckETH helper contract
fn principal_to_bytes32(principal: &Principal) -> [u8; 32] {
    let principal_bytes = principal.as_slice();
    let mut bytes32 = [0u8; 32];
    bytes32[0] = principal_bytes.len() as u8;
    bytes32[1..1 + principal_bytes.len()].copy_from_slice(principal_bytes);
    bytes32
}

/// Encode the deposit(bytes32) function call
fn encode_deposit_calldata(principal: &Principal) -> Vec<u8> {
    // Function selector: keccak256("deposit(bytes32)") first 4 bytes
    let selector = &eth_keccak256(b"deposit(bytes32)")[..4];
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(selector);
    calldata.extend_from_slice(&principal_to_bytes32(principal));
    calldata
}

/// Determine y-parity of ECDSA signature for Ethereum
fn y_parity(prehash: &[u8; 32], sig: &[u8; 64], pubkey: &[u8]) -> u64 {
    let orig_key = VerifyingKey::from_sec1_bytes(pubkey)
        .expect("failed to parse public key");
    let signature = K256Signature::try_from(sig.as_slice())
        .expect("failed to parse signature");
    for parity in [false, true] {
        let recid = RecoveryId::new(parity, false);
        if let Ok(recovered) = VerifyingKey::recover_from_prehash(prehash, &signature, recid) {
            if recovered == orig_key {
                return parity as u64;
            }
        }
    }
    panic!("unable to determine y parity");
}

// --- ECDSA calls to management canister ---

async fn get_ecdsa_public_key(user: &Principal) -> Result<Vec<u8>, String> {
    let args = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path_for(user),
        key_id: ecdsa_key_id(),
    };
    let result: Result<(EcdsaPublicKeyResponse,), _> = ic_cdk::call(
        Principal::management_canister(),
        "ecdsa_public_key",
        (args,),
    ).await;
    match result {
        Ok((response,)) => Ok(response.public_key),
        Err((code, msg)) => Err(format!("ecdsa_public_key failed: {:?} - {}", code, msg)),
    }
}

async fn ecdsa_sign(message_hash: Vec<u8>, user: &Principal) -> Result<Vec<u8>, String> {
    let args = SignWithEcdsaArgument {
        message_hash,
        derivation_path: derivation_path_for(user),
        key_id: ecdsa_key_id(),
    };

    // sign_with_ecdsa requires cycles
    let result: Result<(SignWithEcdsaResponse,), _> =
        ic_cdk::api::call::call_with_payment128(
            Principal::management_canister(),
            "sign_with_ecdsa",
            (args,),
            ECDSA_SIGN_CYCLES,
        ).await;

    match result {
        Ok((response,)) => Ok(response.signature),
        Err((code, msg)) => Err(format!("sign_with_ecdsa failed: {:?} - {}", code, msg)),
    }
}

// --- EVM RPC calls ---

/// Make a raw JSON-RPC call to Ethereum via the EVM RPC canister (multi_request)
async fn evm_rpc_request(json_rpc: &str) -> Result<String, String> {
    let evm_rpc = Principal::from_text(EVM_RPC_CANISTER)
        .map_err(|_| "Invalid EVM RPC canister ID".to_string())?;
    let services = RpcServices::EthMainnet(Some(vec![EvmEthMainnetService::PublicNode]));
    let config: Option<evm_rpc_types::RpcConfig> = None;
    let json = json_rpc.to_string();

    // Use ic_cdk::api::call::call_with_payment128 for proper tuple encoding
    let result: Result<(MultiRpcResult<String>,), _> =
        ic_cdk::api::call::call_with_payment128(
            evm_rpc,
            "multi_request",
            (services, config, json),
            EVM_RPC_CYCLES,
        ).await;

    match result {
        Ok((MultiRpcResult::Consistent(Ok(response)),)) => Ok(response),
        Ok((MultiRpcResult::Consistent(Err(err)),)) => Err(format!("RPC error: {:?}", err)),
        Ok((MultiRpcResult::Inconsistent(results),)) => {
            // Collect successful responses and check for consensus
            let successful: Vec<_> = results.iter()
                .filter_map(|(_, r)| r.as_ref().ok())
                .collect();
            if successful.is_empty() {
                return Err("All RPC providers returned errors".to_string());
            }
            // If multiple providers succeeded but disagree, log warning and use first
            // (trapping would be too aggressive for balance checks)
            if successful.len() > 1 && successful[0] != successful[1] {
                ic_cdk::println!("WARNING: EVM RPC providers returned inconsistent results");
            }
            Ok(successful[0].clone())
        }
        Err((code, msg)) => Err(format!("EVM RPC call failed: {:?} - {}", code, msg)),
    }
}

/// Parse a hex quantity from JSON-RPC response (e.g., "0x1a2b3c")
fn parse_json_rpc_hex_result(response: &str) -> Result<u128, String> {
    // multi_request returns raw hex like "0x1a2b" or a JSON-RPC envelope
    let hex_source = if response.starts_with('{') {
        // JSON-RPC format: {"jsonrpc":"2.0","result":"0x...","id":1}
        let parsed: serde_json::Value = serde_json::from_str(response)
            .map_err(|e| format!("JSON parse error: {} (raw: {})", e, &response[..response.len().min(100)]))?;
        parsed["result"].as_str()
            .ok_or("Missing 'result' field in JSON-RPC response")?
            .to_string()
    } else {
        // Raw hex format from multi_request: "0x1a2b"
        response.to_string()
    };
    let hex_str = hex_source.strip_prefix("0x").unwrap_or(&hex_source);
    if hex_str.is_empty() || hex_str == "0" {
        return Ok(0);
    }
    u128::from_str_radix(hex_str, 16)
        .map_err(|e| format!("Hex parse error: {}", e))
}

async fn get_eth_balance(address: &str) -> Result<u128, String> {
    let json_rpc = format!(
        r#"{{"jsonrpc":"2.0","method":"eth_getBalance","params":["{}","latest"],"id":1}}"#,
        address
    );
    let response = evm_rpc_request(&json_rpc).await?;
    parse_json_rpc_hex_result(&response)
}

async fn get_eth_nonce(address: &str) -> Result<u64, String> {
    let json_rpc = format!(
        r#"{{"jsonrpc":"2.0","method":"eth_getTransactionCount","params":["{}","latest"],"id":1}}"#,
        address
    );
    let response = evm_rpc_request(&json_rpc).await?;
    let nonce = parse_json_rpc_hex_result(&response)?;
    Ok(nonce as u64)
}

async fn get_gas_prices() -> Result<(u128, u128), String> {
    let json_rpc = r#"{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}"#;
    let response = evm_rpc_request(json_rpc).await?;
    let gas_price = parse_json_rpc_hex_result(&response)?;
    // max_fee = 2x current gas price (buffer for price fluctuation)
    // priority_fee = 1.5 Gwei (standard tip)
    let priority_fee: u128 = 1_500_000_000; // 1.5 Gwei
    let max_fee = gas_price * 2;
    Ok((max_fee.max(priority_fee + 1), priority_fee))
}

async fn send_raw_transaction(signed_tx_hex: &str) -> Result<String, String> {
    let json_rpc = format!(
        r#"{{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["{}"],"id":1}}"#,
        signed_tx_hex
    );
    let response = evm_rpc_request(&json_rpc).await?;

    // multi_request may return raw tx hash "0x..." or JSON-RPC envelope
    if response.starts_with('{') {
        let parsed: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("JSON parse error: {}", e))?;
        if let Some(error) = parsed.get("error") {
            return Err(format!("Transaction rejected: {}", error));
        }
        let tx_hash = parsed["result"].as_str()
            .ok_or("Missing tx hash in response")?;
        Ok(tx_hash.to_string())
    } else if response.starts_with("0x") {
        // Raw tx hash from multi_request
        Ok(response)
    } else {
        Err(format!("Unexpected sendRawTransaction response: {}", &response[..response.len().min(100)]))
    }
}

// --- Transaction building and signing ---

async fn build_and_sign_sweep_tx(
    user: &Principal,
    value_wei: u128,
    nonce: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    pubkey: &[u8],
) -> Result<String, String> {
    // Parse the helper contract address
    let to_address: Address = CKETH_HELPER_CONTRACT.parse()
        .map_err(|e| format!("Invalid helper contract address: {:?}", e))?;

    // Encode deposit(bytes32) calldata with user's principal
    let calldata = encode_deposit_calldata(user);

    // Build EIP-1559 transaction
    let tx = TxEip1559 {
        chain_id: ETH_CHAIN_ID,
        nonce,
        gas_limit: DEPOSIT_ETH_GAS_LIMIT,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        to: TxKind::Call(to_address),
        value: U256::from(value_wei),
        access_list: Default::default(),
        input: Bytes::from(calldata),
    };

    // Get the signing hash
    let sig_hash = tx.signature_hash();
    let sig_hash_bytes: [u8; 32] = sig_hash.into();

    // Sign with threshold ECDSA
    let signature_bytes = ecdsa_sign(sig_hash_bytes.to_vec(), user).await?;

    // Determine y-parity
    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(&signature_bytes[..64]);
    let v = y_parity(&sig_hash_bytes, &sig_array, pubkey);

    // Create the alloy Signature
    let r = U256::from_be_slice(&signature_bytes[..32]);
    let s = U256::from_be_slice(&signature_bytes[32..64]);
    let signature = alloy_primitives::PrimitiveSignature::new(r, s, v != 0);

    // Encode signed transaction using RLP
    // EIP-1559 format: 0x02 || rlp([chain_id, nonce, max_priority_fee, max_fee, gas_limit, to, value, data, access_list, y_parity, r, s])
    use alloy_rlp::Encodable as _;
    let signed_tx = tx.into_signed(signature);
    let mut rlp_buf = Vec::new();
    signed_tx.rlp_encode(&mut rlp_buf);
    // Prepend the EIP-1559 type byte
    let mut encoded = Vec::with_capacity(1 + rlp_buf.len());
    encoded.push(0x02);
    encoded.extend_from_slice(&rlp_buf);

    Ok(format!("0x{}", hex::encode(&encoded)))
}

// --- Public endpoints ---

/// Get a unique Ethereum deposit address for the caller.
/// The user can send native ETH to this address, then call sweep_eth_to_cketh()
/// to convert it to ckETH. Only available for ETH tables.
#[ic_cdk::update]
async fn get_eth_deposit_address() -> Result<EthDepositInfo, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity first.".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::ETH {
        return Err("This function is only available for ETH tables.".to_string());
    }

    // Derive Ethereum address from caller's principal via threshold ECDSA
    let pubkey = get_ecdsa_public_key(&caller).await?;
    let eth_address = pubkey_to_eth_address(&pubkey)?;

    // Minimum: enough to cover gas + have meaningful ckETH minted
    // Gas cost ≈ 60k gas * ~30 Gwei = ~0.0018 ETH
    // Minimum useful deposit: 0.005 ETH
    let min_deposit_wei = "5000000000000000".to_string(); // 0.005 ETH

    Ok(EthDepositInfo {
        eth_address,
        min_deposit_wei,
    })
}

/// Sweep deposited ETH from user's derived address to the ckETH helper contract.
/// This checks the ETH balance, estimates gas, signs a deposit(bytes32) transaction,
/// and submits it to Ethereum. After ~20 minutes, ckETH will be minted to the
/// caller's ICP principal.
#[ic_cdk::update]
async fn sweep_eth_to_cketh() -> Result<SweepStatus, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Please log in first.".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::ETH {
        return Err("This function is only available for ETH tables.".to_string());
    }

    // Get user's derived Ethereum address
    let pubkey = get_ecdsa_public_key(&caller).await?;
    let eth_address = pubkey_to_eth_address(&pubkey)?;

    // Check ETH balance
    let balance = get_eth_balance(&eth_address).await?;
    if balance == 0 {
        return Ok(SweepStatus::NoBalance);
    }

    // Get gas price and nonce
    let (max_fee_per_gas, max_priority_fee_per_gas) = get_gas_prices().await?;
    let nonce = get_eth_nonce(&eth_address).await?;

    // Calculate max gas cost
    let max_gas_cost = max_fee_per_gas * (DEPOSIT_ETH_GAS_LIMIT as u128);

    if balance <= max_gas_cost {
        return Ok(SweepStatus::InsufficientForGas {
            balance_wei: balance.to_string(),
            estimated_gas_wei: max_gas_cost.to_string(),
        });
    }

    // Value to deposit = balance - max gas cost
    let deposit_value = balance - max_gas_cost;

    // Build, sign, and submit the transaction
    let signed_tx = build_and_sign_sweep_tx(
        &caller,
        deposit_value,
        nonce,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        &pubkey,
    ).await?;

    let tx_hash = send_raw_transaction(&signed_tx).await?;

    Ok(SweepStatus::Swept {
        tx_hash,
        amount_wei: deposit_value.to_string(),
        gas_cost_wei: max_gas_cost.to_string(),
    })
}

// ============================================================================
// DOGE DEPOSIT: Threshold ECDSA + Dogecoin canister
// ============================================================================

/// DOGE internal wallet balances (separate from table escrow)
/// This tracks DOGE held in the canister's derived addresses
thread_local! {
    static DOGE_BALANCES: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
    static DOGE_CREDITED_UTXOS: RefCell<std::collections::HashSet<String>> = RefCell::new(std::collections::HashSet::new());
}

/// Candid types for the Dogecoin canister API
#[derive(CandidType, Deserialize, Clone, Debug)]
struct DogeGetUtxosRequest {
    address: String,
    filter: Option<DogeUtxoFilter>,
    network: DogeNetwork,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
enum DogeUtxoFilter {
    MinConfirmations(u32),
    Page(Vec<u8>),
}

#[derive(CandidType, Deserialize, Clone, Debug)]
enum DogeNetwork {
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "testnet")]
    Testnet,
    #[serde(rename = "regtest")]
    Regtest,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct DogeGetUtxosResponse {
    utxos: Vec<DogeUtxo>,
    tip_block_hash: Vec<u8>,
    tip_height: u32,
    next_page: Option<Vec<u8>>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct DogeUtxo {
    outpoint: DogeOutpoint,
    value: u64,
    height: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct DogeOutpoint {
    txid: Vec<u8>,
    vout: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct DogeSendTxRequest {
    transaction: Vec<u8>,
    network: DogeNetwork,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
enum DogeDepositStatus {
    Credited { utxo_id: String, amount: u64, confirmations: u32 },
    AlreadyCredited { utxo_id: String },
    InsufficientConfirmations { utxo_id: String, confirmations: u32, required: u32 },
}

/// Convert a compressed SEC1 public key to a Dogecoin P2PKH address
/// DOGE uses version byte 0x1e (produces "D..." addresses)
fn pubkey_to_doge_address(pubkey_bytes: &[u8]) -> Result<String, String> {
    use ripemd::Ripemd160;
    use sha2::{Sha256, Digest};

    // SHA256 of the compressed pubkey
    let sha256_hash = Sha256::digest(pubkey_bytes);
    // RIPEMD160 of the SHA256 hash
    let ripemd_hash = Ripemd160::digest(&sha256_hash);

    // Prepend version byte 0x1e (Dogecoin mainnet)
    let mut payload = vec![0x1e_u8];
    payload.extend_from_slice(&ripemd_hash);

    // Base58Check encode (bs58 with_check handles the double-SHA256 checksum)
    Ok(bs58::encode(&payload).with_check().into_string())
}

/// Get the user's unique Dogecoin deposit address (derived via threshold ECDSA)
#[ic_cdk::update]
async fn get_doge_deposit_address() -> Result<String, String> {
    let caller = ic_cdk::api::msg_caller();

    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity to get a DOGE deposit address.".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::DOGE {
        return Err("This function is only available for DOGE tables".to_string());
    }

    // Derive ECDSA public key for this user
    // Use derivation path prefix 2u8 to differentiate from ETH (1u8)
    let derivation_path = vec![vec![2u8], caller.as_slice().to_vec()];

    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: ECDSA_KEY_NAME.to_string(),
    };

    let args = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path,
        key_id,
    };

    let result: Result<(EcdsaPublicKeyResponse,), _> = ic_cdk::call(
        Principal::management_canister(),
        "ecdsa_public_key",
        (args,),
    ).await;

    let (response,) = result.map_err(|(code, msg)| format!("ECDSA key error: {:?} - {}", code, msg))?;

    // Convert compressed SEC1 pubkey to DOGE P2PKH address
    let address = pubkey_to_doge_address(&response.public_key)?;

    Ok(address)
}

/// Check for DOGE deposits at the user's derived address
/// Credits new confirmed UTXOs to the user's internal DOGE balance
#[ic_cdk::update]
async fn check_doge_deposit() -> Result<Vec<DogeDepositStatus>, String> {
    let caller = ic_cdk::api::msg_caller();

    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity.".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::DOGE {
        return Err("This function is only available for DOGE tables".to_string());
    }

    // First get the user's DOGE address
    let address = get_doge_deposit_address().await?;

    // Query the Dogecoin canister for UTXOs
    let dogecoin_canister = Principal::from_text(DOGECOIN_CANISTER)
        .map_err(|_| "Invalid Dogecoin canister ID".to_string())?;

    let utxo_args = DogeGetUtxosRequest {
        address: address.clone(),
        filter: Some(DogeUtxoFilter::MinConfirmations(DOGE_MIN_CONFIRMATIONS)),
        network: DogeNetwork::Mainnet,
    };

    let result: Result<(DogeGetUtxosResponse,), _> = ic_cdk::call(
        dogecoin_canister,
        "dogecoin_get_utxos",
        (utxo_args,),
    ).await;

    let (utxo_response,) = result.map_err(|(code, msg)| format!("Dogecoin canister error: {:?} - {}", code, msg))?;

    let mut statuses = Vec::new();

    for utxo in &utxo_response.utxos {
        let utxo_id = format!("{}:{}", hex::encode(&utxo.outpoint.txid), utxo.outpoint.vout);

        // Check if already credited
        let already_credited = DOGE_CREDITED_UTXOS.with(|credited| {
            credited.borrow().contains(&utxo_id)
        });

        if already_credited {
            statuses.push(DogeDepositStatus::AlreadyCredited { utxo_id });
            continue;
        }

        // Credit the user's internal DOGE balance
        DOGE_BALANCES.with(|balances| {
            let mut balances = balances.borrow_mut();
            let balance = balances.entry(caller).or_insert(0);
            *balance = balance.saturating_add(utxo.value);
        });

        // Mark UTXO as credited
        DOGE_CREDITED_UTXOS.with(|credited| {
            credited.borrow_mut().insert(utxo_id.clone());
        });

        let tip_height = utxo_response.tip_height;
        let confirmations = if tip_height >= utxo.height {
            tip_height - utxo.height + 1
        } else {
            0
        };

        statuses.push(DogeDepositStatus::Credited {
            utxo_id,
            amount: utxo.value,
            confirmations,
        });
    }

    Ok(statuses)
}

/// Get the user's internal DOGE wallet balance (not table escrow)
#[ic_cdk::query]
fn get_doge_balance() -> u64 {
    let caller = ic_cdk::api::msg_caller();
    DOGE_BALANCES.with(|balances| {
        *balances.borrow().get(&caller).unwrap_or(&0)
    })
}

/// Withdraw DOGE to an external Dogecoin address
/// Builds and signs a P2PKH transaction, submits via the Dogecoin canister
#[ic_cdk::update]
async fn withdraw_doge(destination: String, amount: u64) -> Result<String, String> {
    let caller = ic_cdk::api::msg_caller();

    if caller == Principal::anonymous() {
        return Err("Please log in with Internet Identity.".to_string());
    }

    let currency = get_table_currency();
    if currency != Currency::DOGE {
        return Err("This function is only available for DOGE tables".to_string());
    }

    // Validate destination address (basic check: starts with 'D' or '9' or 'A')
    if destination.is_empty() || (!destination.starts_with('D') && !destination.starts_with('9') && !destination.starts_with('A')) {
        return Err("Invalid Dogecoin address".to_string());
    }

    let fee = DOGE_TRANSFER_FEE; // 0.001 DOGE
    if amount <= fee {
        return Err(format!("Amount must be greater than {} shibes to cover fee", fee));
    }

    // Acquire DOGE withdrawal guard — prevents reentrancy via Drop trait
    let _guard = DogeWithdrawalGuard::new(caller)?;

    // ATOMIC: Check balance and deduct BEFORE signing (prevents double-spend)
    DOGE_BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current_balance = *balances.get(&caller).unwrap_or(&0);
        if current_balance < amount {
            return Err(format!("Insufficient DOGE balance. Have {} shibes, need {}", current_balance, amount));
        }
        // Deduct immediately while holding the lock
        balances.insert(caller, current_balance.saturating_sub(amount));
        Ok(())
    })?;

    // Get the user's derived ECDSA pubkey + address
    let derivation_path = vec![vec![2u8], caller.as_slice().to_vec()];
    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: ECDSA_KEY_NAME.to_string(),
    };

    let pk_args = EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path: derivation_path.clone(),
        key_id: key_id.clone(),
    };

    let pk_result: Result<(EcdsaPublicKeyResponse,), _> = ic_cdk::call(
        Principal::management_canister(),
        "ecdsa_public_key",
        (pk_args,),
    ).await;
    let (pk_response,) = pk_result.map_err(|(code, msg)| format!("ECDSA key error: {:?} - {}", code, msg))?;
    let pubkey = pk_response.public_key;
    let source_address = pubkey_to_doge_address(&pubkey)?;

    // Get UTXOs from the user's address
    let dogecoin_canister = Principal::from_text(DOGECOIN_CANISTER)
        .map_err(|_| "Invalid Dogecoin canister ID".to_string())?;

    let utxo_args = DogeGetUtxosRequest {
        address: source_address.clone(),
        filter: Some(DogeUtxoFilter::MinConfirmations(1)),
        network: DogeNetwork::Mainnet,
    };

    let utxo_result: Result<(DogeGetUtxosResponse,), _> = ic_cdk::call(
        dogecoin_canister,
        "dogecoin_get_utxos",
        (utxo_args,),
    ).await;
    let (utxo_response,) = utxo_result.map_err(|(code, msg)| format!("UTXO query error: {:?} - {}", code, msg))?;

    if utxo_response.utxos.is_empty() {
        return Err("No UTXOs available for withdrawal".to_string());
    }

    // Select UTXOs to cover amount + fee
    let send_amount = amount - fee;
    let mut selected_utxos: Vec<&DogeUtxo> = Vec::new();
    let mut total_input: u64 = 0;

    for utxo in &utxo_response.utxos {
        selected_utxos.push(utxo);
        total_input += utxo.value;
        if total_input >= amount {
            break;
        }
    }

    if total_input < amount {
        return Err(format!("Insufficient UTXO value. Have {} shibes in UTXOs, need {}", total_input, amount));
    }

    let change = total_input - amount;

    // Decode destination address to script pubkey
    let dest_script = doge_address_to_script_pubkey(&destination)?;
    let source_script = doge_address_to_script_pubkey(&source_address)?;

    // Build the raw transaction
    let mut tx_bytes: Vec<u8> = Vec::new();

    // Version (1, little-endian 4 bytes)
    tx_bytes.extend_from_slice(&1u32.to_le_bytes());

    // Input count (varint)
    write_varint(&mut tx_bytes, selected_utxos.len() as u64);

    // Inputs (unsigned first, we'll sign them one by one)
    for utxo in &selected_utxos {
        // txid (reversed - Dogecoin uses internal byte order)
        let mut txid = utxo.outpoint.txid.clone();
        txid.reverse();
        tx_bytes.extend_from_slice(&txid);
        // vout (little-endian 4 bytes)
        tx_bytes.extend_from_slice(&utxo.outpoint.vout.to_le_bytes());
        // scriptSig placeholder (empty for signing)
        tx_bytes.push(0);
        // sequence
        tx_bytes.extend_from_slice(&0xffffffffu32.to_le_bytes());
    }

    // Output count
    let output_count = if change > 0 { 2u64 } else { 1u64 };
    write_varint(&mut tx_bytes, output_count);

    // Output 1: destination
    tx_bytes.extend_from_slice(&send_amount.to_le_bytes());
    write_varint(&mut tx_bytes, dest_script.len() as u64);
    tx_bytes.extend_from_slice(&dest_script);

    // Output 2: change back to source (if any)
    if change > 0 {
        tx_bytes.extend_from_slice(&change.to_le_bytes());
        write_varint(&mut tx_bytes, source_script.len() as u64);
        tx_bytes.extend_from_slice(&source_script);
    }

    // Locktime
    tx_bytes.extend_from_slice(&0u32.to_le_bytes());

    // Now sign each input
    // For P2PKH, we need to sign a modified tx where the input being signed
    // has its scriptSig replaced with the scriptPubKey
    let mut signed_inputs: Vec<Vec<u8>> = Vec::new();

    for input_idx in 0..selected_utxos.len() {
        // Build the signing hash (SIGHASH_ALL)
        let sighash = compute_p2pkh_sighash(
            &selected_utxos,
            input_idx,
            &source_script,
            send_amount,
            change,
            &dest_script,
            &source_script,
        );

        // Sign with threshold ECDSA
        let sign_args = SignWithEcdsaArgument {
            message_hash: sighash.to_vec(),
            derivation_path: derivation_path.clone(),
            key_id: key_id.clone(),
        };

        let sign_result = ic_cdk::api::call::call_with_payment128(
            Principal::management_canister(),
            "sign_with_ecdsa",
            (sign_args,),
            ECDSA_SIGN_CYCLES,
        ).await;

        let (sign_response,): (SignWithEcdsaResponse,) = sign_result
            .map_err(|(code, msg)| format!("ECDSA sign error: {:?} - {}", code, msg))?;

        // Build DER-encoded signature + SIGHASH_ALL byte
        let mut sig_with_hashtype = sign_response.signature.clone();
        sig_with_hashtype.push(0x01); // SIGHASH_ALL

        // Build scriptSig: <sig_len> <sig+hashtype> <pubkey_len> <pubkey>
        let mut script_sig: Vec<u8> = Vec::new();
        script_sig.push(sig_with_hashtype.len() as u8);
        script_sig.extend_from_slice(&sig_with_hashtype);
        script_sig.push(pubkey.len() as u8);
        script_sig.extend_from_slice(&pubkey);

        signed_inputs.push(script_sig);
    }

    // Rebuild the final signed transaction
    let mut final_tx: Vec<u8> = Vec::new();
    final_tx.extend_from_slice(&1u32.to_le_bytes()); // version

    write_varint(&mut final_tx, selected_utxos.len() as u64);

    for (i, utxo) in selected_utxos.iter().enumerate() {
        let mut txid = utxo.outpoint.txid.clone();
        txid.reverse();
        final_tx.extend_from_slice(&txid);
        final_tx.extend_from_slice(&utxo.outpoint.vout.to_le_bytes());
        write_varint(&mut final_tx, signed_inputs[i].len() as u64);
        final_tx.extend_from_slice(&signed_inputs[i]);
        final_tx.extend_from_slice(&0xffffffffu32.to_le_bytes());
    }

    write_varint(&mut final_tx, output_count);
    final_tx.extend_from_slice(&send_amount.to_le_bytes());
    write_varint(&mut final_tx, dest_script.len() as u64);
    final_tx.extend_from_slice(&dest_script);
    if change > 0 {
        final_tx.extend_from_slice(&change.to_le_bytes());
        write_varint(&mut final_tx, source_script.len() as u64);
        final_tx.extend_from_slice(&source_script);
    }
    final_tx.extend_from_slice(&0u32.to_le_bytes()); // locktime

    // Submit the signed transaction
    let send_args = DogeSendTxRequest {
        transaction: final_tx,
        network: DogeNetwork::Mainnet,
    };

    let send_result: Result<((),), _> = ic_cdk::call(
        dogecoin_canister,
        "dogecoin_send_transaction",
        (send_args,),
    ).await;

    if let Err((code, msg)) = send_result {
        // Refund on failure (balance was already deducted above)
        DOGE_BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            let current = *balances.get(&caller).unwrap_or(&0);
            balances.insert(caller, current.saturating_add(amount));
        });
        return Err(format!("Send tx error: {:?} - {}", code, msg));
    }

    // Balance was already deducted before signing — no further deduction needed
    Ok(format!("Sent {} shibes ({} DOGE) to {}", send_amount, send_amount as f64 / 100_000_000.0, destination))
}

/// Write a Bitcoin-style varint
fn write_varint(buf: &mut Vec<u8>, value: u64) {
    if value < 0xfd {
        buf.push(value as u8);
    } else if value <= 0xffff {
        buf.push(0xfd);
        buf.extend_from_slice(&(value as u16).to_le_bytes());
    } else if value <= 0xffffffff {
        buf.push(0xfe);
        buf.extend_from_slice(&(value as u32).to_le_bytes());
    } else {
        buf.push(0xff);
        buf.extend_from_slice(&value.to_le_bytes());
    }
}

/// Decode a Dogecoin P2PKH address to its scriptPubKey
/// P2PKH script: OP_DUP OP_HASH160 <20-byte-hash> OP_EQUALVERIFY OP_CHECKSIG
fn doge_address_to_script_pubkey(address: &str) -> Result<Vec<u8>, String> {
    let decoded = bs58::decode(address)
        .with_check(None)
        .into_vec()
        .map_err(|e| format!("Invalid Dogecoin address: {}", e))?;

    if decoded.len() != 21 {
        return Err(format!("Invalid address length: {} (expected 21)", decoded.len()));
    }

    // decoded[0] is the version byte, decoded[1..21] is the 20-byte hash
    let pubkey_hash = &decoded[1..21];

    let mut script = Vec::with_capacity(25);
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    Ok(script)
}

/// Compute SIGHASH_ALL for a P2PKH input
/// This creates the hash that needs to be signed for input `input_idx`
fn compute_p2pkh_sighash(
    utxos: &[&DogeUtxo],
    input_idx: usize,
    source_script: &[u8],
    send_amount: u64,
    change: u64,
    dest_script: &[u8],
    change_script: &[u8],
) -> [u8; 32] {
    use sha2::{Sha256, Digest};

    let mut tx: Vec<u8> = Vec::new();

    // Version
    tx.extend_from_slice(&1u32.to_le_bytes());

    // Input count
    write_varint(&mut tx, utxos.len() as u64);

    // Inputs
    for (i, utxo) in utxos.iter().enumerate() {
        let mut txid = utxo.outpoint.txid.clone();
        txid.reverse();
        tx.extend_from_slice(&txid);
        tx.extend_from_slice(&utxo.outpoint.vout.to_le_bytes());

        if i == input_idx {
            // For the input being signed, use the source scriptPubKey
            write_varint(&mut tx, source_script.len() as u64);
            tx.extend_from_slice(source_script);
        } else {
            // Empty script for other inputs
            tx.push(0);
        }

        tx.extend_from_slice(&0xffffffffu32.to_le_bytes());
    }

    // Output count
    let output_count = if change > 0 { 2u64 } else { 1u64 };
    write_varint(&mut tx, output_count);

    // Output 1: destination
    tx.extend_from_slice(&send_amount.to_le_bytes());
    write_varint(&mut tx, dest_script.len() as u64);
    tx.extend_from_slice(dest_script);

    // Output 2: change
    if change > 0 {
        tx.extend_from_slice(&change.to_le_bytes());
        write_varint(&mut tx, change_script.len() as u64);
        tx.extend_from_slice(change_script);
    }

    // Locktime
    tx.extend_from_slice(&0u32.to_le_bytes());

    // SIGHASH_ALL (appended for signing, not part of final tx)
    tx.extend_from_slice(&1u32.to_le_bytes());

    // Double SHA256
    let hash1 = Sha256::digest(&tx);
    let hash2 = Sha256::digest(&hash1);

    let mut result = [0u8; 32];
    result.copy_from_slice(&hash2);
    result
}

// ============================================================================
// CANDID EXPORT
// ============================================================================

ic_cdk::export_candid!();
