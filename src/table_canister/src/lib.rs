// ============================================================================
// ⚠️  CRITICAL DEPLOYMENT WARNING ⚠️
// ============================================================================
// This canister holds REAL USER FUNDS (ICP/BTC). When upgrading:
//
// ✅ ALWAYS use: dfx canister install <name> --mode upgrade --network ic
// ❌ NEVER use:  dfx canister install <name> --mode reinstall --network ic
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

// Rate limiting
const RATE_LIMIT_WINDOW_NS: u64 = 1_000_000_000; // 1 second
const MAX_ACTIONS_PER_WINDOW: u32 = 10;

// Withdrawal limits for ICP (in e8s - 1 ICP = 100_000_000 e8s)
const ICP_MAX_WITHDRAWAL_PER_TX: u64 = 10_000_000_000; // 100 ICP max per withdrawal
const ICP_MIN_WITHDRAWAL_AMOUNT: u64 = 100_000; // 0.001 ICP minimum (must cover fees)

// Withdrawal limits for BTC (in satoshis - 1 BTC = 100_000_000 satoshis)
const BTC_MAX_WITHDRAWAL_PER_TX: u64 = 10_000_000; // 0.1 BTC max per withdrawal
const BTC_MIN_WITHDRAWAL_AMOUNT: u64 = 11; // Just above 10 sat fee - receive at least 1 sat

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

// Deposit anti-replay. See "DEPOSIT ANTI-REPLAY" below and docs/DEFECTS.md E-02.
// How many credited block indices are remembered individually. Everything older
// is refused by DEPOSIT_WATERMARK instead, never forgotten.
const MAX_VERIFIED_DEPOSITS: usize = 10_000;
// Trimming sorts the whole key set, so the deposit path only trims once per
// SLACK deposits rather than on every deposit once the map is at capacity.
// periodic_cleanup trims at MAX_VERIFIED_DEPOSITS exactly.
const VERIFIED_DEPOSITS_SLACK: usize = 1_000;

// ============================================================================
// TYPES - Core poker data structures
// ============================================================================

/// Currency type for the table - determines which ledger to use
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Currency {
    #[default]
    ICP,  // Uses ICP ledger, amounts in e8s (1 ICP = 100_000_000 e8s)
    BTC,  // Uses ckBTC ledger, amounts in satoshis (1 BTC = 100_000_000 sats)
}

impl Currency {
    pub fn ledger_canister(&self) -> Principal {
        match self {
            Currency::ICP => Principal::from_text(ICP_LEDGER_CANISTER).unwrap(),
            Currency::BTC => Principal::from_text(CKBTC_LEDGER_CANISTER).unwrap(),
        }
    }

    pub fn transfer_fee(&self) -> u64 {
        match self {
            Currency::ICP => ICP_TRANSFER_FEE,
            Currency::BTC => CKBTC_TRANSFER_FEE,
        }
    }

    pub fn min_withdrawal(&self) -> u64 {
        match self {
            Currency::ICP => ICP_MIN_WITHDRAWAL_AMOUNT,
            Currency::BTC => BTC_MIN_WITHDRAWAL_AMOUNT,
        }
    }

    pub fn max_withdrawal(&self) -> u64 {
        match self {
            Currency::ICP => ICP_MAX_WITHDRAWAL_PER_TX,
            Currency::BTC => BTC_MAX_WITHDRAWAL_PER_TX,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::ICP => "ICP",
            Currency::BTC => "BTC",
        }
    }

    pub fn decimals(&self) -> u8 {
        8 // Both ICP and BTC use 8 decimals
    }

    /// Format an amount in smallest units (e8s/satoshis) as a human-readable string
    /// e.g., 200_000_000 ICP e8s -> "2.0 ICP"
    /// e.g., 50_000 BTC satoshis -> "0.0005 BTC"
    pub fn format_amount(&self, smallest_units: u64) -> String {
        let decimal = smallest_units as f64 / 100_000_000.0;
        match self {
            Currency::ICP => format!("{:.4} ICP", decimal),
            Currency::BTC => {
                if decimal >= 0.001 {
                    format!("{:.4} BTC", decimal)
                } else {
                    format!("{} sats", smallest_units)
                }
            }
        }
    }
}

// ============================================================================
// POKER PRIMITIVES -- re-exported from the `poker_core` crate
// ============================================================================
// `Suit`, `Rank`, `Card` and `HandRank` used to be declared here. They now live
// in `src/poker_core` so that unit tests, differential harnesses and fuzzers can
// link the REAL engine instead of re-implementing it (which is exactly what
// tests/unit_tests.rs used to do, and the two copies had already diverged).
//
// These are `pub use`, not private imports, so this canister's Candid surface is
// byte-for-byte unchanged. `poker_core` keeps every derive, field name, variant
// name and variant ORDER identical -- they are on the Candid wire and in this
// canister's stable-memory serialisation, and it custodies real ICP/ckBTC.
pub use poker_core::{Card, HandRank, Rank, Suit};

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
    /// Stakes of seats that were VACATED while the hand was still live.
    ///
    /// `#[serde(default)]` so state written before this field existed restores as
    /// empty, which is correct: at that point nothing had been recorded.
    ///
    /// # Why this field exists (docs/DEFECTS.md E-05)
    ///
    /// `leave_table` and `cash_out` set `players[seat] = None` mid-hand. The money
    /// that seat had already put in stays in `pot`, but the RECORD of who put it
    /// there vanished with the seat, so the payout basis -- built by enumerating
    /// the seat vector -- silently shrank, and the difference was appended to the
    /// highest bet level: the pot only the deepest stacks can win. An honest
    /// short-stacked all-in lost part of the main pot it was entitled to, with
    /// every chip conserved, which is why no conservation invariant could see it.
    ///
    /// The model: **a stake is recorded independently of seat occupancy.** Money
    /// in the pot belongs to the hand, not to the chair. Once it is in, the only
    /// thing leaving the table changes is that the player can no longer WIN it --
    /// exactly what folding does -- so a departed stake is carried with
    /// `has_folded = true` and never appears in an eligibility list. The
    /// alternative model, keeping a ghost `Player` in the seat until the hand
    /// ends, was rejected: it makes an empty chair look occupied to
    /// `join_table`, `count_active_players`, the blinds and the UI, and every one
    /// of those reads would then need to know about a state that is neither
    /// present nor absent.
    ///
    /// Entries are tagged with the hand they belong to and are ignored for any
    /// other hand, so a stale entry can never join a later hand's pot. They are
    /// cleared when the hand settles.
    #[serde(default)]
    pub departed_stakes: Vec<DepartedStake>,
}

/// The stake of a seat that was vacated while the hand was still live.
///
/// Carries the principal as well as the seat, because money that nobody at the
/// table can claim has to be refundable to the player who put it in, and by then
/// they have no seat to credit. See [`TableState::departed_stakes`].
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct DepartedStake {
    pub hand_number: u64,
    pub seat: u8,
    pub principal: Principal,
    pub contributed: u64,
}

// `SidePot` also lives in `poker_core` now (see the re-export note above); the
// pot-splitting maths that produces it has to be testable off-chain.
pub use poker_core::SidePot;

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
    // Block indices already credited, and therefore refused. Bounded to
    // MAX_VERIFIED_DEPOSITS + VERIFIED_DEPOSITS_SLACK entries by raising
    // DEPOSIT_WATERMARK, NOT by forgetting. See "DEPOSIT ANTI-REPLAY".
    static VERIFIED_DEPOSITS: RefCell<HashMap<u64, Principal>> = RefCell::new(HashMap::new());
    // Monotonically non-decreasing floor: every block index strictly below this is
    // permanently uncreditable, whether or not it is still in VERIFIED_DEPOSITS.
    static DEPOSIT_WATERMARK: RefCell<u64> = RefCell::new(0);
    // Pending deposits being verified - prevents double-crediting race condition
    static PENDING_DEPOSITS: RefCell<HashMap<u64, Principal>> = RefCell::new(HashMap::new());
    // Pending withdrawals - prevents reentrancy
    static PENDING_WITHDRAWALS: RefCell<HashMap<Principal, u64>> = RefCell::new(HashMap::new());
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

    // Bound VERIFIED_DEPOSITS. This used to drop the oldest block indices and
    // FORGET them, which made an already-credited transfer creditable again --
    // withdrawable ICP created from nothing (docs/DEFECTS.md E-02). It now raises
    // DEPOSIT_WATERMARK past whatever it drops, so dropped means refused.
    bound_verified_deposits(MAX_VERIFIED_DEPOSITS);

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
        let call_result = ic_cdk::call::Call::unbounded_wait(history_id, "record_hand")
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

// `first_clockwise_from_dealer` used to live here. It answered "which ONE of these
// seats gets the whole remainder of a chopped pot", and that question is not one the
// rules of poker ask: odd chips go out one each, walking clockwise from the button
// (Robert's Rules for flop games; the TDA rules distribute them one at a time from
// the earliest position). With two winners the two rules coincide, which is why the
// difference went unnoticed; with three the engine gave one seat two chips that
// belonged to two different seats. See `poker_core::split_pot_clockwise`, which
// replaced it, and docs/DEFECTS.md E-35.

// ============================================================================
// LEDGER INTEGRATION - Real Money Play
// ============================================================================

/// Get the canister's own principal (for receiving deposits)
fn canister_id() -> Principal {
    ic_cdk::api::canister_self()
}

/// Transfer tokens (ICP or ckBTC) from canister to a player (for withdrawals/payouts)
/// Uses the table's configured currency
async fn transfer_tokens(to: Principal, amount: u64) -> Result<u64, String> {
    use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};

    let currency = get_table_currency();
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
        fee: None, // Use default fee
        created_at_time: None,
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

// ============================================================================
// DEPOSIT ANTI-REPLAY  (docs/DEFECTS.md E-02, docs/SECURITY-FINDINGS.md FINDING 10)
// ============================================================================
//
// THE INVARIANT THIS CODE GUARANTEES, IN WORDS
//   For every ledger block index B, this canister credits escrow for B AT MOST
//   ONCE over the entire lifetime of its state. All three deposit doors --
//   `notify_deposit`, `deposit` (the ICRC-2 pull) and `verify_ckbtc_deposit` --
//   must call `claim_deposit_block` and see `Ok(())` before they touch BALANCES,
//   and `claim_deposit_block` is the ONLY writer of the record. It is also the
//   only place the rule is expressed, so there is one thing to audit.
//
// WHY A WATERMARK AND NOT SIMPLY A BIGGER SET
//   The record has to be bounded: an unbounded map eventually makes `pre_upgrade`
//   fail to serialise, and a canister holding real funds that cannot be upgraded
//   is bricked with the funds inside. The shipped code bounded it by FORGETTING
//   the oldest block indices, and a forgotten index passed the
//   already-processed check again. One on-ledger transfer, two escrow credits.
//   So the bound must never turn "spent" into "unknown". It turns it into
//   "permanently refused" instead:
//
//       block index B is REFUSED  if  B < DEPOSIT_WATERMARK
//                                 or  VERIFIED_DEPOSITS contains B
//
//   DEPOSIT_WATERMARK only ever moves UP, and `bound_verified_deposits` moves it
//   up by exactly enough to cover every index it is about to drop. "Forgotten"
//   therefore means "below the watermark", which is refused. Memory is bounded
//   and nothing is ever un-spent.
//
// WHAT THIS COSTS, STATED PLAINLY
//   A raw transfer whose block index has fallen below the watermark can never be
//   credited, even though it was never credited. Reaching that state takes
//   MAX_VERIFIED_DEPOSITS further deposits recorded after it and before the
//   sender ever calls `notify_deposit`. The trade is deliberate and it is
//   one-directional: a refused late deposit is recoverable, because the ICP is
//   still on the ledger in this canister's account and the block index is in the
//   error message, whereas a double credit is not recoverable, because the
//   invented balance leaves as somebody else's money. The error message says
//   exactly this, so a user who really did send ICP is not told to send some.
//
// ACROSS AN UPGRADE
//   Both VERIFIED_DEPOSITS and DEPOSIT_WATERMARK are in `PersistentState`, so an
//   `install_code --mode upgrade` carries the record over unchanged and a block
//   credited before the upgrade is still refused after it.
//
// A FRESH CANISTER, AND A LEDGER OLDER THAN ANYTHING WE REMEMBER
//   The ICP ledger has tens of millions of blocks that predate this canister.
//   They are not a replay risk, for a reason that does not depend on the
//   watermark: `notify_deposit` requires the block's `to` to equal THIS
//   canister's own account identifier, and no block written before this canister
//   existed can name it.
//   `--mode reinstall` is a different matter: it erases this record along with
//   every balance, after which historical blocks that really were sent to this
//   canister's account become creditable again. That is NOT defended here. It is
//   written up in docs/SECURITY-FINDINGS.md, because reinstall already destroys
//   all escrow and is already forbidden for production canisters (CLAUDE.md,
//   and `post_upgrade` panics rather than let a bad restore through).
//
// KEY SPACE
//   The record is keyed by block index alone. A table's currency is fixed at
//   init (`TableConfig.currency`), so ICP block indices and ckBTC transaction
//   indices never share one canister's key space.

/// Every block index strictly below this has been consumed and can never be
/// credited again.
fn deposit_watermark() -> u64 {
    DEPOSIT_WATERMARK.with(|w| *w.borrow())
}

/// The refusal reason for `block_index`, or `None` if it currently looks
/// claimable. Cheap, and it does NOT claim: a caller that passes this must still
/// call `claim_deposit_block` before crediting, because an `await` in between
/// gives another message the chance to claim the same block.
fn deposit_block_refusal(block_index: u64) -> Option<String> {
    let floor = deposit_watermark();
    if block_index < floor {
        return Some(format!(
            "Deposit block {} is below this table's deposit replay-protection watermark ({}) \
             and can no longer be credited automatically. Your transfer is still on the ledger \
             in this canister's account. Contact the table operator and quote block index {}. \
             (Only reachable if more than {} later deposits were recorded before you claimed \
             this one.)",
            block_index, floor, block_index, MAX_VERIFIED_DEPOSITS
        ));
    }
    if VERIFIED_DEPOSITS.with(|v| v.borrow().contains_key(&block_index)) {
        return Some("This deposit has already been credited".to_string());
    }
    None
}

/// Claim `block_index` for `who`, atomically.
///
/// `Ok(())` means the caller now holds the exclusive right to credit that block
/// and MUST do it in this same message: there must be no `await` between this
/// returning `Ok` and the `BALANCES` update, or the claim and the credit can come
/// apart across an upgrade or a trap.
fn claim_deposit_block(block_index: u64, who: Principal) -> Result<(), String> {
    // INVARIANT ENFORCEMENT POINT (see the module comment above): a block index
    // is claimable at most once, ever.
    if let Some(reason) = deposit_block_refusal(block_index) {
        return Err(reason);
    }
    let claimed = VERIFIED_DEPOSITS.with(|v| {
        use std::collections::hash_map::Entry;
        match v.borrow_mut().entry(block_index) {
            Entry::Occupied(_) => false,
            Entry::Vacant(slot) => {
                slot.insert(who);
                true
            }
        }
    });
    if !claimed {
        return Err("This deposit has already been credited".to_string());
    }
    // Enforce the memory bound at the WRITE point, not only from the timeout
    // path: nothing obliges a depositor to ever call `check_timeouts`, and an
    // unbounded record is a route to an unupgradeable canister.
    bound_verified_deposits(MAX_VERIFIED_DEPOSITS + VERIFIED_DEPOSITS_SLACK);
    Ok(())
}

/// Bound `VERIFIED_DEPOSITS` to `MAX_VERIFIED_DEPOSITS` entries once it exceeds
/// `trigger`, WITHOUT ever forgetting that a block was spent: raise
/// `DEPOSIT_WATERMARK` past every index about to be dropped, then drop them.
fn bound_verified_deposits(trigger: usize) {
    let len = VERIFIED_DEPOSITS.with(|v| v.borrow().len());
    if len <= trigger || len <= MAX_VERIFIED_DEPOSITS {
        return;
    }
    let drop_count = len - MAX_VERIFIED_DEPOSITS;
    let mut keys: Vec<u64> = VERIFIED_DEPOSITS.with(|v| v.borrow().keys().copied().collect());
    keys.sort_unstable();
    // The highest index being dropped. Everything at or below it becomes refused
    // by the watermark instead of by its own entry, so dropping it is not
    // forgetting it.
    let highest_dropped = keys[drop_count - 1];
    let new_floor = highest_dropped.saturating_add(1);
    DEPOSIT_WATERMARK.with(|w| {
        let mut w = w.borrow_mut();
        if new_floor > *w {
            *w = new_floor;
        }
    });
    let floor = deposit_watermark();
    VERIFIED_DEPOSITS.with(|v| v.borrow_mut().retain(|index, _| *index >= floor));
    ic_cdk::println!(
        "deposit replay protection: watermark raised to {} (dropped {} of {} recorded block \
         indices; they remain permanently uncreditable)",
        floor, drop_count, len
    );
}

/// Deposit replay-protection state, so an operator or a user can see whether a
/// given block index is still claimable: `(watermark, recorded_block_count)`.
/// Any block index below the watermark, or already recorded, will be refused.
#[ic_cdk::query]
fn get_deposit_replay_state() -> (u64, u64) {
    (
        deposit_watermark(),
        VERIFIED_DEPOSITS.with(|v| v.borrow().len() as u64),
    )
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

    // Cheap pre-flight: refuse a block that is already consumed before paying for
    // a ledger query. This is NOT the enforcement point -- `claim_deposit_block`
    // below is, because the state can change across the await.
    if let Some(reason) = deposit_block_refusal(block_index) {
        return Err(reason);
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
    let ledger_id = currency.ledger_canister();

    // For BTC (ckBTC), use different verification method
    if currency == Currency::BTC {
        clear_pending();
        return verify_ckbtc_deposit(block_index, caller, canister).await;
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

    // The ICP ledger's did says `type AccountIdentifier = blob`, i.e. a BARE
    // 32-byte blob, NOT `record { hash : blob }`. Declaring it as a record made
    // the `Operation` variant arms mismatch, and under Candid's `opt` rule a
    // mismatched `opt Operation` decodes to `null` with NO error -- so every
    // legitimate transfer was reported as "Transaction is not a transfer".
    // See docs/DEFECTS.md E-04 / SECURITY-FINDINGS.md FINDING 06 BUG B.
    type AccountIdentifier = Vec<u8>;

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
        spender: Option<AccountIdentifier>,
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
        allowance_e8s: candid::Int,
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

    let call_result = ic_cdk::call::Call::unbounded_wait(ledger_id, "query_blocks")
        .with_arg(request)
        .await;

    let response = match call_result {
        // `Response::candid::<R>()` is `decode_one::<R>()`. Asking for
        // `(QueryBlocksResponse,)` asked the decoder for ONE value of type
        // `record { 0 : QueryBlocksResponse }`, which the reply can never be.
        // See docs/DEFECTS.md E-04 / SECURITY-FINDINGS.md FINDING 06 BUG A.
        Ok(response) => match response.candid::<QueryBlocksResponse>() {
            Ok(r) => r,
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
        if transfer.to.len() != 32 {
            return Err("Invalid destination account".to_string());
        }
        let to_bytes: [u8; 32] = transfer.to.clone().try_into()
            .map_err(|_| "Invalid destination account length")?;

        if to_bytes != expected_to {
            return Err("Transfer was not to this canister".to_string());
        }

        // Verify the sender is the caller by computing their expected account identifier.
        // Account identifiers are deterministic: SHA224(domain || principal || subaccount).
        let expected_from = compute_account_identifier(&caller, None);
        let from_bytes: [u8; 32] = transfer.from.clone().try_into()
            .map_err(|_| "Invalid source account length".to_string())?;
        if from_bytes != expected_from {
            return Err("This transfer was not sent from your account. Only the sender can claim their deposit.".to_string());
        }

        // An ICRC-2 pull whose spender is THIS canister was made by `deposit()`,
        // which credited it when the pull happened. `spender` is the only field
        // that distinguishes such a block from a plain send: `from` is the
        // caller's account and `to` is this canister's account either way, and
        // ignoring `spender` is what made one deposit creditable twice
        // (docs/DEFECTS.md E-02). `deposit()` also records its block index now,
        // so this is a second line of defence -- and the only one that covers
        // pulls made before that recording existed.
        let own_account = compute_account_identifier(&canister, None);
        if transfer.spender.as_deref() == Some(&own_account[..]) {
            return Err("This block is an ICRC-2 pull performed by this canister on your \
                        behalf (the deposit() flow). It was credited to your balance when the \
                        pull happened and cannot be credited again.".to_string());
        }

        let amount = transfer.amount.e8s;
        if amount == 0 {
            return Err("Transfer amount is zero; nothing to credit".to_string());
        }

        // THE ENFORCEMENT POINT. Claim the block, atomically, with no await
        // between this and the credit below. The pre-flight check further up ran
        // BEFORE the ledger query, so anything could have claimed this block in
        // between -- notably `deposit()`, whose own pull writes a block of exactly
        // this shape. The suffix marks that this is the post-verification claim
        // and not the pre-flight, because the two are diagnosed differently.
        claim_deposit_block(block_index, caller).map_err(|reason| {
            format!(
                "{} [refused at the post-verification claim: the block was claimed \
                 while your request was being verified]",
                reason
            )
        })?;

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
    let min_deposit = if currency == Currency::BTC { 1_000 } else { 20_000 }; // 1000 sats or 0.0002 ICP
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
        created_at_time: None,
    };

    // Use ic_cdk::call which properly handles Candid encoding/decoding
    let transfer_result: Result<(Result<Nat, TransferFromError>,), _> =
        ic_cdk::call(ledger_id, "icrc2_transfer_from", (transfer_from_args,)).await;

    let transfer_result = match transfer_result {
        Ok((result,)) => result,
        Err((code, msg)) => return Err(format!("Failed to call ledger: {:?} - {}", code, msg)),
    };

    match transfer_result {
        Ok(block_index) => {
            // Record the block this pull just wrote, BEFORE crediting. Without
            // this, the block satisfied every check `notify_deposit` performs
            // (its `from` is the caller, its `to` is this canister) and the same
            // movement could be credited a second time -- withdrawable ICP
            // created from nothing (docs/DEFECTS.md E-02, the FUND-THEFT entry).
            // Both ledgers type block indices as 64-bit, so the conversion below
            // cannot lose information in practice.
            let block: u64 = block_index.0.clone().try_into().unwrap_or(u64::MAX);
            if let Err(reason) = claim_deposit_block(block, caller) {
                // The pull SUCCEEDED, so real money has already moved. Which of
                // the two refusals this is decides whether crediting here would
                // double-credit or whether NOT crediting here would strand the
                // money, so the two are handled separately rather than lumped.
                let already_recorded =
                    VERIFIED_DEPOSITS.with(|v| v.borrow().contains_key(&block));
                if already_recorded {
                    // Something already claimed this exact block. The only
                    // principal that can claim it is the block's `from`, which is
                    // this caller, so the caller already holds the credit. Report
                    // the balance they really have and do NOT credit twice.
                    let current =
                        BALANCES.with(|b| b.borrow().get(&caller).copied().unwrap_or(0));
                    ic_cdk::println!(
                        "deposit(): block {} was already claimed ({}); no second credit \
                         applied for {}. Balance remains {}.",
                        block, reason, caller, current
                    );
                    return Ok(current);
                }
                // Refused but NOT recorded, which can only be the watermark. That
                // is unreachable by construction -- `bound_verified_deposits`
                // never raises the watermark above the newest recorded index, and
                // this block is newer than every recorded index -- but if it ever
                // happens, money has been pulled that nothing will ever credit.
                // Credit it here and shout. This cannot double-credit: a
                // below-watermark block is refused to every other claimant.
                ic_cdk::println!(
                    "CRITICAL: deposit(): the block {} this pull just wrote was refused by \
                     the deposit watermark ({}): {}. Crediting anyway rather than stranding \
                     the caller's transfer. This means bound_verified_deposits has a bug.",
                    block, deposit_watermark(), reason
                );
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
        created_at_time: None,
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

/// Verify a ckBTC deposit using ICRC-3 get_transactions API
async fn verify_ckbtc_deposit(block_index: u64, caller: Principal, canister: Principal) -> Result<u64, String> {
    let ledger_id = Principal::from_text(CKBTC_LEDGER_CANISTER).unwrap();

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

    let call_result = ic_cdk::call::Call::unbounded_wait(ledger_id, "get_transactions")
        .with_arg(request)
        .await;

    let response = match call_result {
        Ok(response) => match response.candid::<(GetTransactionsResponse,)>() {
            Ok((r,)) => r,
            Err(e) => return Err(format!("Failed to decode ckBTC ledger response: {:?}", e)),
        },
        Err(e) => return Err(format!("Failed to query ckBTC ledger: {:?}", e)),
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

    // An ICRC-2 pull whose spender is THIS canister was made by `deposit()`,
    // which credited it when the pull happened. Same rule and same reason as the
    // ICP door (docs/DEFECTS.md E-02): `from` is the caller and `to` is this
    // canister either way, so `spender` is the only field that tells the two
    // apart. `deposit()` records its block index now, so this is the second line
    // of defence -- and the only one covering pulls made before that recording
    // existed, which on a ckBTC table is the whole of any pre-fix history.
    if transfer
        .spender
        .as_ref()
        .is_some_and(|s| s.owner == canister)
    {
        return Err("This transaction is an ICRC-2 pull performed by this canister on your \
                    behalf (the deposit() flow). It was credited to your balance when the \
                    pull happened and cannot be credited again.".to_string());
    }

    let amount: u64 = transfer.amount.0.clone().try_into().unwrap_or(0);
    if amount == 0 {
        return Err("Invalid transaction amount".to_string());
    }

    // THE ENFORCEMENT POINT for the ckBTC door: one atomic claim, no await
    // between it and the credit below. Same rule and same record as the ICP door,
    // including the watermark (see "DEPOSIT ANTI-REPLAY").
    claim_deposit_block(block_index, caller)?;

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

/// Withdraw your balance from the table
#[ic_cdk::update]
async fn withdraw(amount: u64) -> Result<u64, String> {
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

    // Check if player already has a pending withdrawal (prevent reentrancy)
    let has_pending = PENDING_WITHDRAWALS.with(|p| {
        p.borrow().contains_key(&caller)
    });
    if has_pending {
        return Err("A withdrawal is already in progress".to_string());
    }

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

    // ATOMIC: Check balance, deduct, AND mark pending in single critical section
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
        balances.insert(caller, current_balance - amount);

        // Mark this withdrawal as pending to prevent reentrancy
        PENDING_WITHDRAWALS.with(|p| {
            p.borrow_mut().insert(caller, amount);
        });

        Ok(())
    })?;

    // Transfer to player's wallet
    let result = transfer_tokens(caller, amount).await;

    // Clear pending state regardless of outcome
    PENDING_WITHDRAWALS.with(|p| {
        p.borrow_mut().remove(&caller);
    });

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
            balances.insert(caller, balance - amount);
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
        balances.insert(caller, balance - amount);
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

        let seat = state
            .players
            .iter()
            .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .ok_or("Not at table")?;

        // The guard above only refuses players who have NOT folded, so a folded
        // player -- including one folded by the action timer without ever calling
        // anything -- can vacate a seat while the hand is live. Their stake stays in
        // the payout basis, exactly as with `leave_table`. docs/DEFECTS.md E-05,
        // FINDING 08: this is the door that needs no deliberate call at all.
        let hand_is_live = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;
        if hand_is_live {
            record_departed_stake(state, seat);
        }

        let chips = state.players[seat].as_ref().map(|p| p.chips).unwrap_or(0);
        state.players[seat] = None; // Remove from table
        Ok::<u64, String>(chips)
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
            departed_stakes: Vec::new(),
        });
    });

    // Clear history
    HAND_HISTORY.with(|h| h.borrow_mut().clear());
    CURRENT_ACTIONS.with(|a| a.borrow_mut().clear());
    SHOWN_CARDS.with(|s| s.borrow_mut().clear());
}

// ============================================================================
// DECK, SHUFFLING & HAND EVALUATION -- moved to the `poker_core` crate
// ============================================================================
// `create_deck`, `shuffle_deck`, `combinations`, `evaluate_hand`,
// `evaluate_five_cards`, `detect_straight`, `check_straight` and
// `get_straight_high` now live in `src/poker_core`. Their behaviour is pinned by
// `src/poker_core/tests/golden_vectors.rs`, which replays vectors captured from
// this file before the move -- including the provably-fair shuffle, whose output
// for a revealed seed is a public commitment to every hand already played.
//
// Only the entry points this canister actually calls are imported. `combinations`,
// `evaluate_five_cards`, `detect_straight`, `check_straight` and
// `get_straight_high` were private helpers here before the move (never part of
// the Candid surface) and are reached through `evaluate_hand` inside
// `poker_core`; they stay `pub` there for the test and fuzz harnesses.
use poker_core::{create_deck, evaluate_hand, shuffle_deck, Contribution};

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
        // Nothing has left this hand yet. Entries are tagged with the hand number
        // as well, so a leftover could not join this pot even if one survived.
        state.departed_stakes.clear();
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
            balances.insert(caller, balance - buy_in_amount);
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
///
/// If mid-hand this acts as a fold: the money already in the pot stays there, but
/// the RECORD of who put it there is kept in `state.departed_stakes` so it stays
/// part of the payout basis. See docs/DEFECTS.md E-05 for what happened before.
#[ic_cdk::update]
fn leave_table() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();

    let chips = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Find the player's seat
        let seat = state.players.iter()
            .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .ok_or("Not at table")?;

        let player = state.players[seat].as_ref().ok_or("Player not found")?;
        let hand_is_live = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;
        let was_in_hand = !player.has_folded && hand_is_live;
        let was_action_on = state.action_on as usize == seat;

        // If we're in a hand, mark as folded first (pot contributions stay in pot)
        if was_in_hand {
            if let Some(ref mut p) = state.players[seat] {
                p.has_folded = true;
            }
        }

        // Anything this player bet that nobody covered was never in play, and once
        // they are gone nobody can ever cover it. Hand it back before they leave,
        // rather than leaving it in a pot they are no longer eligible for.
        if hand_is_live {
            let contributions = hand_contributions(state);
            if let Some((top, _excess)) = poker_core::uncalled_excess(&contributions) {
                if top as usize == seat {
                    return_uncalled_bet(state);
                }
            }
        }

        // Keep this seat's stake in the payout basis. Money in the pot belongs to
        // the hand, not to the chair.
        if hand_is_live {
            record_departed_stake(state, seat);
        }

        let chips = state.players[seat]
            .as_ref()
            .map(|p| p.chips)
            .unwrap_or(0);

        // Remove player from table
        state.players[seat] = None;

        // If player was in the hand, advance game state
        if was_in_hand {
            let now = ic_cdk::api::time();
            // Check if only one player left - award pot
            if count_active_players(state) == 1 {
                end_hand_single_winner(state, now);
            } else if was_action_on {
                // If it was this player's turn, move to next player
                state.action_on = find_next_active_seat(state, state.action_on);
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
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;
        apply_player_action(state, caller, now, action)
    })
}

/// The betting rules of [`player_action`], with the platform pulled out.
///
/// `player_action` is `check_rate_limit()` + `msg_caller()` + `time()` + the
/// `TABLE` borrow + THIS. Everything that decides what is legal, what it does to
/// the table and where the action goes next lives here, so
/// `tests/betting_rules.rs` drives the same code the canister runs rather than a
/// re-implementation of it. docs/DEFECTS.md H-04 is what happens otherwise: seven
/// of seven mutations to this file survived with all 101 tests green.
pub fn apply_player_action(
    state: &mut TableState,
    caller: Principal,
    now: u64,
    action: PlayerAction,
) -> Result<(), String> {
    // Find the player
    let player_seat = state.players.iter()
        .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
        .ok_or("Not at table")?;

    if state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete {
        return Err("No hand in progress".to_string());
    }

    // An action timer that has already expired must be RESOLVED, not merely
    // refused. Refusing left `action_on` pointing at a seat that could no longer
    // act, so unless something else called `check_timeouts` the hand could not
    // progress at all and the table wedged. Resolving it here, BEFORE the
    // whose-turn check, means any player touching the table unwedges it.
    // See docs/DEFECTS.md E-31.
    let phase_before_timeout = state.phase.clone();
    let timed_out_seat = resolve_expired_action_timer(state, now);

    if state.phase == GamePhase::WaitingForPlayers || state.phase == GamePhase::HandComplete {
        // Resolving the stale timeout ended the hand (everyone else had folded,
        // or the board ran out). Nothing left for this action to do.
        return Err(timed_out_message(timed_out_seat, player_seat));
    }

    // Resolving the stale timer moved the hand to a NEW STREET, so a card this
    // player has not seen is now on the board. Their message was composed against
    // the old board; applying it here would commit chips to a street they were
    // never shown -- measured before this guard as an out-of-turn `Bet(500)` sent
    // on the flop landing on the turn. The table is still unwedged (the timeout
    // above has already been applied and persists), so the fix is to refuse THIS
    // message and let them re-send against the board they can actually see.
    // See docs/DEFECTS.md "E-31 -- correction".
    if state.phase != phase_before_timeout {
        return Err(format!(
            "A player's clock ran out and the hand moved on to {}. Your action was sent \
             against the {} board, so it was not applied. Re-send it now that you can see \
             the new card.",
            phase_to_string(&state.phase),
            phase_to_string(&phase_before_timeout)
        ));
    }

    if player_seat != state.action_on as usize {
        // Either it was never their turn, or their own clock had run out and the
        // action has moved on. Say which.
        if timed_out_seat == Some(player_seat as u8) {
            return Err(timed_out_message(timed_out_seat, player_seat));
        }
        return Err("Not your turn".to_string());
    }

    // First, gather all the info we need from the player without holding the mutable ref
    let (player_chips, player_current_bet, player_has_acted) = {
        let player = state.players[player_seat].as_ref()
            .ok_or("Player not found at seat")?;
        (player.chips, player.current_bet, player.has_acted_this_round)
    };

    // Track whether we need to reset acted flags after processing
    let mut should_reset_acted = false;
    let mut new_current_bet = state.current_bet;

    // Check if this is BB acting on their option
    let is_bb_option = state.phase == GamePhase::PreFlop
        && state.bb_has_option
        && player_seat == state.big_blind_seat as usize;

    // ------------------------------------------------------------------
    // STANDARD RULE: an incomplete all-in raise does not reopen the betting.
    // docs/DEFECTS.md E-30. Tests: tests/betting_rules.rs section 1.
    //
    // TDA 2022 Rule 47-A, verbatim: "An all-in wager (or CUMULATIVE MULTIPLE SHORT
    // ALL-INS) totaling less than a full bet or raise will not reopen betting for
    // players who have already acted and are not facing at least a full bet or
    // raise when the action returns to them." Robert's Rules of Poker (Ciaffone),
    // no-limit section, states the other half of the same rule: "Multiple all-in
    // wagers, each of an amount too small to qualify as a raise, still act as a
    // raise and reopen the betting if the resulting wager size to a player
    // qualifies as a raise."
    //
    // So the test is NOT "is this player facing anything at all" -- that was the
    // first implementation of this fix and it is wrong, because it closes a player
    // whose two short all-ins have added up to a full raise. The test is "is this
    // player facing at least a full raise", i.e. `amount_owed >= min_raise`.
    //
    // `state.min_raise` is exactly the right number to compare against, and it is
    // load-bearing that it is: the `AllIn` arm below updates `min_raise` ONLY when
    // the shove is a full raise, so after any number of incomplete all-ins
    // `min_raise` still holds the last FULL bet-or-raise increment. Cumulative
    // shorts therefore accumulate in `current_bet` while the yardstick stays put,
    // which is precisely what Rule 47-A asks to be measured.
    //
    // A player who has not yet acted this round (including the big blind, whose
    // posted blind is not an action) keeps a full option regardless.
    //
    // docs/DEFECTS.md E-30 and "E-30 -- correction".
    // ------------------------------------------------------------------
    // Named `amount_owed` rather than `to_call`: the `Call` arm below already has a
    // local `to_call` and shadowing it here would read as two different numbers.
    let amount_owed = state.current_bet.saturating_sub(player_current_bet);
    let action_is_closed_to_raising = player_has_acted && amount_owed < state.min_raise;

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
            if action_is_closed_to_raising {
                return Err(closed_action_message(&state.config.currency, amount_owed));
            }
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
            // A shove that puts MORE than the current bet in front of this
            // player is a raise. If the action is closed to them they are not
            // allowed to raise, so refuse rather than silently reinterpret an
            // over-shove as a call: quietly changing the size of somebody's
            // wager is not a safe default in a canister that holds funds.
            // Shoving for the call amount or less is a call for less, which is
            // always legal.
            if action_is_closed_to_raising
                && player_current_bet.saturating_add(player_chips) > state.current_bet
            {
                return Err(closed_action_message(&state.config.currency, amount_owed));
            }

            let min_raise_in_force = state.min_raise;
            let player = state.players[player_seat].as_mut().expect("Player validated at seat");
            let all_in_amount = player.chips;
            state.pot = state.pot.saturating_add(all_in_amount);
            player.current_bet = player.current_bet.saturating_add(all_in_amount);
            player.total_bet_this_hand = player.total_bet_this_hand.saturating_add(all_in_amount);
            let final_bet = player.current_bet;

            if final_bet > state.current_bet {
                let raise_amount = final_bet.saturating_sub(state.current_bet);
                // A FULL raise (at least the min-raise increment in force)
                // reopens the betting to everyone. An INCOMPLETE all-in raise
                // does NOT: see the rule note above `action_is_closed_to_raising`.
                // It still raises the amount owed, and it still costs the big
                // blind their free check, but players who have already acted
                // get no new raise -- only call or fold.
                let is_full_raise = raise_amount >= min_raise_in_force;
                if is_full_raise {
                    state.min_raise = raise_amount;
                    should_reset_acted = true;
                }
                new_current_bet = final_bet;
                state.last_aggressor = Some(player_seat as u8);
                // Any all-in above the current bet removes BB's free option:
                // the big blind now faces a bet and can no longer check.
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
    advance_game(state, now);

    Ok(())
}

/// The rejection a player sees when the action is not reopened to them.
///
/// It has to say what they MAY do, not just what they may not: an "invalid
/// action" with no explanation is indistinguishable from a bug to the player.
fn closed_action_message(currency: &Currency, to_call: u64) -> String {
    format!(
        "The all-in raise was less than a full raise, so the betting is not reopened \
         to you. You may only call {} or fold.",
        currency.format_amount(to_call)
    )
}

/// The rejection a player sees when their own clock ran out before their message
/// arrived. The timeout has already been applied by then.
fn timed_out_message(timed_out_seat: Option<u8>, player_seat: usize) -> String {
    if timed_out_seat == Some(player_seat as u8) {
        "Your action timer expired before this action arrived; the hand has moved on."
            .to_string()
    } else {
        "No hand in progress".to_string()
    }
}

// Note: reset_acted_flags is now inlined in player_action to avoid borrow conflicts

/// Move the hand on after an action has been applied.
///
/// `now` is passed in rather than read from `ic_cdk::api::time()` so the betting
/// state machine has no platform dependency and `tests/betting_rules.rs` can
/// drive THIS function instead of a copy of it. See docs/DEFECTS.md H-04: seven
/// of seven mutations to this file used to survive with the whole suite green.
pub fn advance_game(state: &mut TableState, now: u64) {
    // Keep the DISPLAYED side-pot breakdown in step with the money after every
    // action, not once per street. `side_pots` is a breakdown of what has been
    // collected, so a reader of `get_table_state` -- the UI, or a money-safety
    // invariant -- must never see it disagree with `pot`. It is display-only: the
    // payout is always recomputed from the contributions when the hand settles.
    if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
        refresh_side_pots(state);
    }

    // Check if only one player left
    if count_active_players(state) == 1 {
        end_hand_single_winner(state, now);
        return;
    }

    // Check if betting round is complete
    if is_betting_round_complete(state) {
        advance_to_next_street(state, now);
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

/// True when nobody left in the hand still owes an action this street.
///
/// `pub` so `tests/betting_rules.rs` can ask the engine, in poker terms, whether
/// a round is closed -- which is the whole question behind "did an incomplete
/// all-in wrongly reopen the betting".
pub fn is_betting_round_complete(state: &TableState) -> bool {
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
///
/// `now` is passed in rather than read from `ic_cdk::api::time()` for the same
/// reason as [`advance_game`]: the whole settlement path is then host-testable, and
/// docs/DEFECTS.md H-04 is what happens when it is not.
fn run_out_board(state: &mut TableState, now: u64) {
    // This is THE all-in moment: nobody left can call, so anything the last
    // aggressor bet over the biggest stack that could cover it comes straight back
    // before the board runs out. Real clients do it here; ours used to leave the
    // excess sitting in a pot only the bettor was eligible for, which paid the
    // right player but showed everybody an inflated pot.
    return_uncalled_bet(state);
    // Keep the DISPLAYED breakdown current. It is display only: the payout is
    // recomputed from the contributions when the hand settles and never reads this.
    refresh_side_pots(state);

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
                determine_winners(state, now);
                return;
            }
            _ => {
                // Already at showdown or waiting - just determine winners
                if state.phase == GamePhase::Showdown {
                    determine_winners(state, now);
                }
                return;
            }
        }
    }
}

/// Close the current betting round and open the next street.
///
/// `now` is passed in for the same reason as [`advance_game`]: no platform
/// dependency in the betting state machine.
pub fn advance_to_next_street(state: &mut TableState, now: u64) {
    // The betting round is closed, so every seat that could still have called the
    // biggest bet has now either matched it, folded, or is all-in for less. Whatever
    // the largest contributor put in above the second-largest was therefore never
    // covered and can never be covered: hand it back before the pots are formed.
    // Idempotent -- after the return the top is level with the second, so calling it
    // again does nothing.
    return_uncalled_bet(state);
    // Refresh the DISPLAYED side-pot breakdown on EVERY street, not just the flop.
    // Building it once at the PreFlop -> Flop transition and never again is exactly
    // what made it stale, and the stale value was then used as the payout basis
    // (docs/DEFECTS.md E-01). It is now display-only state and the payout is always
    // recomputed from the contributions.
    refresh_side_pots(state);

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
            determine_winners(state, now);
            return;
        }
        _ => {}
    }

    // Check if we can have more betting (need 2+ players who can act)
    if count_players_can_act(state) < 2 {
        // Run out the board without recursion
        run_out_board(state, now);
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

// ============================================================================
// THE PAYOUT PATH
// ============================================================================
//
// docs/DEFECTS.md E-01, E-03, E-05 and docs/FINDING-01-chip-destruction.md were
// three doors into one mistake: the money was paid out of a SEPARATE ACCOUNT of
// the pot that could drift away from what the players actually put in.
//
//   E-01  `state.side_pots` was built once, at the PreFlop -> Flop transition, and
//         `determine_winners` only rebuilt it "if empty" -- which it never was.
//         Every chip wagered on the flop, turn and river was paid to nobody and
//         then discarded by `state.pot = 0`. Unrecoverable: `withdraw` pays only
//         against the caller's own escrow and there is no admin withdrawal.
//   E-03  the split was reconciled against `state.pot` and `state.pot` won
//         unconditionally: too high and the excess was minted into the highest bet
//         level, too low and every pot was scaled down through an `f64` ratio.
//   E-05  a seat vacated mid-hand took the RECORD of its stake with it while the
//         money stayed in the pot, which is what drove E-03's minting direction.
//
// The rules this code now lives by:
//
//   1. THE CONTRIBUTIONS ARE THE ONLY PAYOUT BASIS. Every chip in the pot got
//      there by being subtracted from a stack and added to that seat's
//      `total_bet_this_hand` in the same statement, so paying out the
//      contributions restores exactly what was collected, by construction.
//   2. THE BASIS IS BUILT AT PAYOUT TIME, EVERY TIME. `state.side_pots` is
//      display-only state and is never read to decide who is paid what.
//   3. `state.pot` IS A REDUNDANT ACCUMULATOR, NOT A SOURCE OF TRUTH. It is
//      cross-checked against the contributions and any disagreement is reported
//      as a `CRITICAL:` line -- an undocumented one of those fails the
//      money-safety suite -- but it can never move a chip.
//   4. NOTHING VANISHES. A stake whose seat left is still in the basis; a layer
//      nobody can win is carried, and if truly nobody can claim it, refunded to
//      the seats that put it there.
//   5. THE ARITHMETIC IS CHECKED BEFORE IT IS APPLIED. `plan_payouts` is pure and
//      host-testable, and the canister refuses to settle a plan that does not pay
//      out exactly what it collected.

/// Refresh the DISPLAYED side-pot breakdown from the current contributions.
///
/// Display only. It exists because the UI shows a main pot and side pots while the
/// hand is live, and because a reader of `get_table_state` should see the truth. No
/// payout ever reads it: [`plan_payouts`] rebuilds the breakdown from the
/// contributions at the moment money moves.
fn refresh_side_pots(state: &mut TableState) {
    let contributions = hand_contributions(state);
    state.side_pots = poker_core::build_side_pots_from_contributions(&contributions);
    report_pot_disagreement(state, &contributions);
}

/// Cross-check the redundant `state.pot` accumulator against the contributions.
///
/// These cannot disagree: every write to `state.pot` in this file is paired with a
/// write to a seat's `total_bet_this_hand`, and a seat that leaves mid-hand now
/// leaves its stake behind in `departed_stakes`. If they ever do disagree the
/// engine says so loudly, and the money-safety suite blocks on an undocumented
/// `CRITICAL:` line.
///
/// It deliberately does NOT trap and does not adjust anything. The contributions
/// are the account that corresponds to chips actually taken from stacks, so
/// settling from them is conservation-exact whatever `state.pot` says; trapping
/// here would instead leave the pot unsettled, and an unsettled pot is the one
/// state from which money genuinely cannot be recovered (FINDING 01).
fn report_pot_disagreement(state: &TableState, contributions: &[Contribution]) {
    let collected = poker_core::total_contributed(contributions);
    if collected != state.pot {
        ic_cdk::println!(
            "CRITICAL: pot accounting disagreement in hand {}: state.pot = {} but the \
             contributions (including {} departed stake(s)) sum to {}. Settling from the \
             contributions, which is what was taken from the stacks.",
            state.hand_number,
            state.pot,
            state
                .departed_stakes
                .iter()
                .filter(|d| d.hand_number == state.hand_number)
                .count(),
            collected
        );
    }
}

/// Hand back the part of the largest stake that nobody covered.
///
/// Only ever called when the betting round is closed (a street transition, the
/// all-in run-out, or settlement), because only then is "nobody covered it"
/// final. Returns what was handed back, if anything.
///
/// The chips go back to the seat's stack, and `total_bet_this_hand` and
/// `state.pot` are both reduced, so the two accounts of the pot stay in step and
/// the displayed pot stops including money that was never in play.
///
/// If the largest contributor has already left the table its stake is left alone
/// here; `leave_table` returns its own uncalled excess before it vacates, and
/// anything still unclaimable at settlement is refunded by [`apply_payouts`].
fn return_uncalled_bet(state: &mut TableState) -> Option<(u8, u64)> {
    let contributions = hand_contributions(state);
    let (seat, excess) = poker_core::uncalled_excess(&contributions)?;
    if excess == 0 {
        return None;
    }
    let player = state.players.get_mut(seat as usize)?.as_mut()?;
    if player.total_bet_this_hand < excess {
        return None;
    }
    player.total_bet_this_hand -= excess;
    player.chips = player.chips.saturating_add(excess);
    // An all-in player who gets money back is not all-in for that money any more,
    // but the betting round is closed, so this only affects whether the engine
    // thinks they can act on later streets -- which they can, if they have chips.
    if player.chips > 0 {
        player.is_all_in = false;
    }
    state.pot = state.pot.saturating_sub(excess);
    ic_cdk::println!(
        "returned uncalled bet of {} to seat {} in hand {}",
        excess,
        seat,
        state.hand_number
    );
    Some((seat, excess))
}

/// Every stake in the current hand, whether or not its seat is still occupied.
///
/// THE payout basis. `collect_contributions` alone reads the seat vector, so a seat
/// vacated mid-hand disappeared from it while its money stayed in the pot
/// (docs/DEFECTS.md E-05); the departed stakes recorded by `leave_table` /
/// `cash_out` are what closes that.
///
/// A departed stake is carried with `has_folded = true`: it is in the pot, it
/// counts towards the bet levels, and it can never win a layer.
pub fn hand_contributions(state: &TableState) -> Vec<Contribution> {
    let mut out = collect_contributions(&state.players);
    for stake in state
        .departed_stakes
        .iter()
        .filter(|d| d.hand_number == state.hand_number && d.contributed > 0)
    {
        // A seat CAN carry two stakes in one hand, and the money-safety fuzzer found
        // the sequence: a player leaves mid-hand, somebody else takes the empty chair
        // (`join_table` seats them `SittingOut`), they call `sit_in()`, and from then
        // on `find_next_active_seat` will give them the action even though they hold
        // no cards -- so they can put money into a hand they were never dealt into.
        // That is docs/DEFECTS.md E-36, and it belongs to the seating and betting
        // path, not to this one.
        //
        // What this path must do about it is keep BOTH stakes. Dropping either is a
        // destroyed chip. The new occupant is not in `live_claims` (no hole cards), so
        // they cannot win the departed player's money or their own; it is carried to a
        // layer that can be settled. Reported as a WARNING and not as `CRITICAL:`
        // deliberately: nothing about the engine's accounting is inconsistent here,
        // and `CRITICAL:` is reserved in this file for accounts that disagree, which
        // the money-safety classifier treats as a failure that stops the run.
        if out.iter().any(|c| c.seat == stake.seat) {
            ic_cdk::println!(
                "WARNING: seat {} carries both a live stake and a departed stake in hand {} \
                 (the chair was re-occupied mid-hand, docs/DEFECTS.md E-36). Both are in the \
                 payout basis; the new occupant holds no cards and can win neither.",
                stake.seat,
                state.hand_number
            );
        }
        out.push(Contribution::new(stake.seat, stake.contributed, true));
    }
    out.sort_by_key(|c| c.seat);
    out
}

/// Record the stake of a seat that is about to be vacated mid-hand.
///
/// Called by every door out of an occupied seat while a hand is live
/// (`leave_table`, `cash_out`). Returns the stake recorded, if any.
fn record_departed_stake(state: &mut TableState, seat: usize) -> u64 {
    let hand_number = state.hand_number;
    let Some(player) = state.players.get(seat).and_then(|p| p.as_ref()) else {
        return 0;
    };
    let contributed = player.total_bet_this_hand;
    if contributed == 0 {
        return 0;
    }
    let principal = player.principal;
    // Drop anything from an earlier hand: bounded by the number of seats per hand.
    state
        .departed_stakes
        .retain(|d| d.hand_number == hand_number);
    state.departed_stakes.push(DepartedStake {
        hand_number,
        seat: seat as u8,
        principal,
        contributed,
    });
    ic_cdk::println!(
        "seat {} left hand {} with {} in the pot; the stake stays in the payout basis",
        seat,
        hand_number,
        contributed
    );
    contributed
}

/// Why a seat is being credited. Carried so the record says which pot, and so a
/// refund can never be mistaken for a win.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayoutReason {
    /// A share of pot layer `layer`, won at showdown or by everybody else folding.
    PotShare { layer: usize },
    /// A layer no remaining player could win, returned to the seats that put the
    /// money there. Real betting cannot produce this.
    Refund { layer: usize },
}

/// One credit the settlement must make.
#[derive(Clone, Debug)]
pub struct Payout {
    pub seat: u8,
    /// `None` for a departed stake being refunded: it has no seat any more, so the
    /// money goes to that principal's escrow.
    pub principal: Option<Principal>,
    pub amount: u64,
    pub reason: PayoutReason,
}

/// What a hand owes, derived from the contributions and the cards. PURE.
#[derive(Clone, Debug)]
pub struct PayoutPlan {
    /// The pot layers this plan pays out of, newly built from the contributions.
    pub side_pots: Vec<SidePot>,
    /// Every credit to make, layer by layer.
    pub payouts: Vec<Payout>,
    /// Seats that reached a showdown, with the rank they held.
    pub ranked: Vec<(u8, HandRank, Principal, (Card, Card))>,
    /// Chips the hand collected: the contributions after any uncalled bet has been
    /// returned. This is what the plan must pay out, exactly.
    pub collected: u64,
    /// Sum of `payouts`.
    pub awarded: u64,
}

impl PayoutPlan {
    /// The post-condition. ClearDeck takes no rake, so this is exact to the e8.
    pub fn conserves(&self) -> bool {
        self.awarded == self.collected
    }

    pub fn amount_for(&self, seat: u8) -> u64 {
        self.payouts
            .iter()
            .filter(|p| p.seat == seat)
            .fold(0u64, |a, p| a.saturating_add(p.amount))
    }
}

/// Every seat that still has a claim on the pot: seated, not folded, holding cards.
///
/// A seat with no cards cannot win a pot, and it cannot have bet anything either,
/// because `join_table` seats a mid-hand arrival as `SittingOut`, which never gets
/// the action.
fn live_claims(state: &TableState) -> Vec<(u8, Principal, (Card, Card))> {
    state
        .players
        .iter()
        .enumerate()
        .filter_map(|(seat, player)| {
            let p = player.as_ref()?;
            if p.has_folded {
                return None;
            }
            Some((seat as u8, p.principal, p.hole_cards?))
        })
        .collect()
}

/// Rank the claims, IF there is a board to rank them against.
///
/// A hand that ends before the flop has no board and needs no ranking: everybody
/// else folded, so the last player standing takes the pot without showing.
/// `poker_core::evaluate_hand` rejects a 0-card board by trapping -- that is the
/// E-09 fix doing its job -- so ranking unconditionally here would trap on every
/// pre-flop fold-out.
fn rank_claims(
    state: &TableState,
    claim: &[(u8, Principal, (Card, Card))],
) -> Vec<(u8, HandRank, Principal, (Card, Card))> {
    if !(3..=5).contains(&state.community_cards.len()) {
        return Vec::new();
    }
    claim
        .iter()
        .map(|(seat, principal, cards)| {
            (
                *seat,
                evaluate_hand(cards, &state.community_cards),
                *principal,
                *cards,
            )
        })
        .collect()
}

/// The claimants holding the best hand, or all of them if they tie.
///
/// A SINGLE claimant wins without showing a hand: that is the rule that pays the
/// last player standing when everybody else folds, and it is why a pre-flop
/// fold-out needs no board. `None` means two or more claimants and nothing to rank
/// them by, which real betting cannot produce.
fn best_hands_among(
    claimants: &[u8],
    ranked: &[(u8, HandRank, Principal, (Card, Card))],
) -> Option<Vec<u8>> {
    if claimants.len() == 1 {
        return Some(claimants.to_vec());
    }
    let ranks: Vec<(u8, &HandRank)> = ranked
        .iter()
        .filter(|(s, _, _, _)| claimants.contains(s))
        .map(|(s, rank, _, _)| (*s, rank))
        .collect();
    let best = ranks.iter().map(|(_, rank)| *rank).max().cloned()?;
    Some(
        ranks
            .iter()
            .filter(|(_, rank)| **rank == best)
            .map(|(s, _)| *s)
            .collect(),
    )
}

/// Decide who gets what. PURE: no platform calls, no mutation, host-testable.
///
/// The rules, in order:
///
/// 1. the pot is split into layers by all-in depth, from the contributions alone
///    (`poker_core::build_side_pots_from_contributions`);
/// 2. a seat is eligible for a layer only if it covered that layer in full and
///    still has a claim -- it has not folded and has not left the table;
/// 3. among the eligible seats, the best five-card hand takes the layer and equal
///    hands chop it. One remaining claimant takes the layer without showing a hand,
///    which is what pays the last player standing when everybody else folds;
/// 4. chips that do not divide go out ONE EACH, clockwise from the button;
/// 5. a layer nobody can win is refunded to the seats that put the money there,
///    rather than being handed to whoever happens to be deepest.
///
/// Callers must return the uncalled bet BEFORE calling this ([`return_uncalled_bet`]);
/// the plan then covers only money that was actually contested.
pub fn plan_payouts(state: &TableState) -> PayoutPlan {
    let contributions = hand_contributions(state);
    let collected = poker_core::total_contributed(&contributions);
    let side_pots = poker_core::build_side_pots_from_contributions(&contributions);

    let claim = live_claims(state);
    let ranked = rank_claims(state, &claim);
    let num_seats = state.players.len();
    let mut payouts: Vec<Payout> = Vec::new();

    let claimants_of = |pot: &SidePot| -> Vec<u8> {
        let mut c: Vec<u8> = pot
            .eligible_players
            .iter()
            .copied()
            .filter(|seat| claim.iter().any(|(s, _, _)| s == seat))
            .collect();
        c.sort_unstable();
        c.dedup();
        c
    };

    // Nobody at the table can win ANY of this money. That is a corrupt state -- a
    // hand cannot reach settlement with no rankable claim on it -- so rather than
    // guess, hand every seat back exactly what it put in. It is conserving, it is
    // exact, and it cannot be gamed: you get your own stake, no more and no less.
    // Before this, the engine returned early here and left the money in `pot`,
    // where the next `start_new_hand` zeroed it: destroyed.
    if side_pots.iter().all(|p| claimants_of(p).is_empty()) {
        for c in contributions.iter().filter(|c| c.total_bet_this_hand > 0) {
            payouts.push(Payout {
                seat: c.seat,
                principal: principal_of(state, c.seat),
                amount: c.total_bet_this_hand,
                reason: PayoutReason::Refund { layer: 0 },
            });
        }
        return PayoutPlan {
            side_pots,
            awarded: payouts
                .iter()
                .fold(0u64, |a, p| a.saturating_add(p.amount)),
            payouts,
            ranked,
            collected,
        };
    }

    // Chips from a layer whose eligible seats cannot be ranked, carried into the
    // next layer that can be settled. `build_side_pots_from_contributions` already
    // carries layers with no ELIGIBLE seat; this is the same rule for the one case
    // it cannot see, an eligible seat holding no cards. Unreachable in play: a seat
    // is only eligible for a layer it paid into, and a seat that paid was dealt in.
    let mut carry = 0u64;

    for (layer, pot) in side_pots.iter().enumerate() {
        if pot.amount == 0 {
            continue;
        }
        let claimants = claimants_of(pot);

        if claimants.is_empty() {
            carry = carry.saturating_add(pot.amount);
            continue;
        }
        let pot_amount = pot.amount.saturating_add(carry);
        carry = 0;

        let winners = match best_hands_among(&claimants, &ranked) {
            Some(w) => w,
            // Two or more claimants and no board to rank them against. Real betting
            // cannot reach a settlement in that state; carry the money to a layer
            // that can be settled rather than guessing a winner.
            None => {
                carry = carry.saturating_add(pot_amount);
                continue;
            }
        };

        for (seat, amount) in
            poker_core::split_pot_clockwise(pot_amount, &winners, state.dealer_seat, num_seats)
        {
            payouts.push(Payout {
                seat,
                principal: principal_of(state, seat),
                amount,
                reason: PayoutReason::PotShare { layer },
            });
        }
    }

    // Chips carried past the last settleable layer. Unreachable: the top layer
    // always has a claimant in any hand that got as far as settling. If it ever
    // happens, the money goes back to the seats that funded the hand rather than
    // being dropped -- a dropped chip is a destroyed chip, and it would make the
    // plan fail its own post-condition and refuse to settle at all.
    if carry > 0 {
        let total = poker_core::total_contributed(&contributions);
        let mut handed = 0u64;
        let funders: Vec<&Contribution> = contributions
            .iter()
            .filter(|c| c.total_bet_this_hand > 0)
            .collect();
        for (i, c) in funders.iter().enumerate() {
            let share = if i + 1 == funders.len() {
                carry.saturating_sub(handed)
            } else {
                ((carry as u128 * c.total_bet_this_hand as u128) / total.max(1) as u128) as u64
            };
            handed = handed.saturating_add(share);
            payouts.push(Payout {
                seat: c.seat,
                principal: principal_of(state, c.seat),
                amount: share,
                reason: PayoutReason::Refund { layer: 0 },
            });
        }
    }

    let awarded = payouts
        .iter()
        .fold(0u64, |a, p| a.saturating_add(p.amount));

    PayoutPlan {
        side_pots,
        payouts,
        ranked,
        collected,
        awarded,
    }
}

/// Who occupies `seat`, if anybody. Falls back to a departed stake's principal, so
/// a refund can reach a player who has already left.
fn principal_of(state: &TableState, seat: u8) -> Option<Principal> {
    if let Some(p) = state.players.get(seat as usize).and_then(|p| p.as_ref()) {
        return Some(p.principal);
    }
    state
        .departed_stakes
        .iter()
        .find(|d| d.hand_number == state.hand_number && d.seat == seat)
        .map(|d| d.principal)
}

/// Collect ALL players who bet this hand (including folded) with their bets.
///
/// `pub` only so `tests/unit_tests.rs` can pin it; it is not an update/query
/// method, so it is not part of the Candid surface.
///
/// NOTE: the seat recorded is the INDEX into `players`, matching the original
/// implementation, which enumerated the seat vector rather than reading
/// `Player::seat`. Empty seats and players who bet nothing are dropped -- the
/// "bet nothing" filter is load-bearing, because an empty contribution list makes
/// `apply_side_pots` leave the existing side pots untouched.
pub fn collect_contributions(players: &[Option<Player>]) -> Vec<Contribution> {
    players
        .iter()
        .enumerate()
        .filter_map(|(i, player)| match player {
            Some(p) if p.total_bet_this_hand > 0 => Some(Contribution::new(
                i as u8,
                p.total_bet_this_hand,
                p.has_folded,
            )),
            _ => None,
        })
        .collect()
}

/// Apply a settlement plan to the table. THE ONLY PLACE CHIPS ARE AWARDED.
///
/// Refuses -- by trapping, so the whole message is rolled back -- to apply a plan
/// that does not pay out exactly what the hand collected. ClearDeck takes no rake,
/// so that is exact to the e8, and a plan that fails it can only mean a bug in
/// [`plan_payouts`]. Trapping leaves the hand unsettled and the state untouched,
/// which is recoverable (players can still leave the table with their stacks and
/// the code can be fixed and the message retried); paying out a plan that does not
/// add up is not.
///
/// Returns one aggregated [`Winner`] per credited seat. A refund of money nobody
/// could win is included in that list: `Winner::amount` means "chips credited to
/// this seat when the hand settled", which is what keeps
/// `sum(winners) == collected` -- the identity the money-safety suite checks as
/// M3 NO RAKE.
fn apply_payouts(state: &mut TableState, plan: &PayoutPlan) -> Vec<Winner> {
    if !plan.conserves() {
        ic_cdk::trap(&format!(
            "CRITICAL: refusing to settle hand {}: the payout plan awards {} out of {} \
             collected (delta {}). Nothing has been credited. side_pots={:?} payouts={:?}",
            state.hand_number,
            plan.awarded,
            plan.collected,
            plan.awarded as i128 - plan.collected as i128,
            plan.side_pots,
            plan.payouts
        ));
    }

    let mut winners: Vec<Winner> = Vec::new();
    for payout in &plan.payouts {
        if payout.amount == 0 {
            continue;
        }
        let seated = state
            .players
            .get(payout.seat as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal);

        match (seated, payout.principal) {
            // The seat is occupied, but by somebody OTHER than the player this money
            // is owed to: the original occupant left and a new player took the chair.
            // Pay the person who is owed it, into their escrow, not the chair.
            // `join_table` seats a mid-hand arrival as SittingOut with no cards, so
            // this cannot arise from ordinary play; it is here because crediting a
            // stranger would be a real loss and the check costs one comparison.
            (Some(occupant), Some(owed)) if occupant != owed => {
                BALANCES.with(|b| {
                    let mut balances = b.borrow_mut();
                    let current = balances.get(&owed).copied().unwrap_or(0);
                    balances.insert(owed, current.saturating_add(payout.amount));
                });
                ic_cdk::println!(
                    "CRITICAL: seat {} is occupied by {} but {} is owed {}; paid to their \
                     escrow instead of the seat",
                    payout.seat,
                    occupant,
                    owed,
                    payout.amount
                );
                push_winner(state, &mut winners, payout, owed);
            }
            // The usual case: the seat is still occupied by the player being paid.
            (Some(occupant), _) => {
                if let Some(ref mut p) = state.players[payout.seat as usize] {
                    p.chips = p.chips.saturating_add(payout.amount);
                }
                push_winner(state, &mut winners, payout, occupant);
            }
            // The seat is empty and this is a refund to a player who left. Their
            // stack already went back to escrow when they left, so this goes to the
            // same place.
            (None, Some(who)) => {
                BALANCES.with(|b| {
                    let mut balances = b.borrow_mut();
                    let current = balances.get(&who).copied().unwrap_or(0);
                    balances.insert(who, current.saturating_add(payout.amount));
                });
                ic_cdk::println!(
                    "refunded {} to the escrow of {} (seat {} left hand {})",
                    payout.amount,
                    who,
                    payout.seat,
                    state.hand_number
                );
                push_winner(state, &mut winners, payout, who);
            }
            // No seat and no recorded principal: there is nobody to credit, so
            // paying this plan out would destroy the chips. Refuse.
            (None, None) => ic_cdk::trap(&format!(
                "CRITICAL: refusing to settle hand {}: {} chips are owed to seat {} but that \
                 seat is empty and no departed stake records who was in it. Nothing has been \
                 credited.",
                state.hand_number, payout.amount, payout.seat
            )),
        }
    }
    winners
}

/// Fold one payout into the aggregated winner list.
fn push_winner(
    state: &TableState,
    winners: &mut Vec<Winner>,
    payout: &Payout,
    principal: Principal,
) {
    if let Some(existing) = winners.iter_mut().find(|w| w.seat == payout.seat) {
        existing.amount = existing.amount.saturating_add(payout.amount);
        return;
    }
    let shown = state
        .players
        .get(payout.seat as usize)
        .and_then(|p| p.as_ref())
        .and_then(|p| p.hole_cards);
    let rank = match payout.reason {
        // A refund is not a win, so it carries no hand.
        PayoutReason::Refund { .. } => None,
        PayoutReason::PotShare { .. } => shown
            .filter(|_| state.community_cards.len() >= 3)
            .map(|cards| evaluate_hand(&cards, &state.community_cards)),
    };
    winners.push(Winner {
        seat: payout.seat,
        principal,
        amount: payout.amount,
        // Only a showdown reveals a hand. A pot won because everybody else folded
        // is recorded without one, as it was before.
        hand_rank: if state.phase == GamePhase::Showdown {
            rank
        } else {
            None
        },
        cards: if state.phase == GamePhase::Showdown {
            shown
        } else {
            None
        },
    });
}

/// Settle the hand: return the uncalled bet, build the payout basis from the
/// contributions, pay it out, and record what happened.
///
/// ONE routine for both endings. Before this, a hand that ended by fold was paid
/// out of `state.pot` and a hand that ended at a showdown was paid out of the
/// frozen `state.side_pots`, and only one of those two was ever right
/// (docs/DEFECTS.md E-01). There is now a single payout basis and a single place it
/// is applied.
fn settle_hand(state: &mut TableState) -> Vec<Winner> {
    // Whatever nobody covered goes back first, so it is not treated as contested
    // money and cannot end up in somebody else's pot.
    return_uncalled_bet(state);

    let plan = plan_payouts(state);
    report_pot_disagreement(state, &hand_contributions(state));
    // Publish the breakdown the payout is actually being made from, so
    // `get_table_state` shows the truth for the rest of this message.
    state.side_pots = plan.side_pots.clone();

    let winners = apply_payouts(state, &plan);

    let showdown_players: Vec<ShowdownPlayer> = plan
        .ranked
        .iter()
        .map(|(seat, rank, principal, cards)| ShowdownPlayer {
            seat: *seat,
            principal: *principal,
            cards: Some(*cards),
            hand_rank: Some(rank.clone()),
            amount_won: plan.amount_for(*seat),
        })
        .collect();

    // Update local history
    HAND_HISTORY.with(|h| {
        if let Some(last) = h.borrow_mut().last_mut() {
            last.winners = winners.clone();
            last.community_cards = state.community_cards.clone();
            if state.phase == GamePhase::Showdown {
                last.showdown_players = showdown_players;
            }
            CURRENT_ACTIONS.with(|a| {
                last.actions = a.borrow().clone();
            });
        }
    });

    // Store winners for display (separate from HAND_HISTORY, which gets a new entry
    // when a new hand starts)
    LAST_HAND_WINNERS.with(|w| {
        *w.borrow_mut() = winners.clone();
    });

    winners
}

/// Close the hand out once the money has moved.
fn finish_hand(state: &mut TableState, now: u64) {
    // Every chip collected has been credited to a seat or an escrow balance -- that
    // is what `apply_payouts` refuses to proceed without -- so the pot is empty.
    state.pot = 0;
    state.side_pots.clear();
    state.departed_stakes.clear();
    state.phase = GamePhase::HandComplete;
    state.action_timer = None;

    // Mark players with 0 chips as broke (start their reload timer)
    for player in state.players.iter_mut().flatten() {
        if player.chips == 0 && player.broke_at.is_none() {
            player.broke_at = Some(now);
        } else if player.chips > 0 {
            // Player has chips, clear broke status
            player.broke_at = None;
        }
    }

    // Schedule auto-deal for next hand
    state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
}

/// Everybody folded except one player: they take the pot without showing a hand.
pub fn end_hand_single_winner(state: &mut TableState, now: u64) {
    // Reveal the seed now that hand is ending
    reveal_seed_on_hand_end(state);

    let winners = settle_hand(state);

    // Record to history canister (no showdown - single winner by fold)
    record_hand_to_history(state, &winners, false);

    finish_hand(state, now);
}

/// The showdown.
pub fn determine_winners(state: &mut TableState, now: u64) {
    // Reveal the seed now that hand is ending (showdown)
    reveal_seed_on_hand_end(state);

    let winners = settle_hand(state);

    // Record to history canister (went to showdown)
    record_hand_to_history(state, &winners, true);

    finish_hand(state, now);
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

        // Then check for player timeouts. ONE shared code path with the
        // resolution inside `player_action`, so the two can never disagree about
        // what a timeout does.
        if let Some(seat) = resolve_expired_action_timer(state, now) {
            return TimeoutCheckResult::PlayerTimedOut(seat);
        }

        TimeoutCheckResult::NoAction
    })
}

/// Apply an action timer that has already expired, and move the hand on.
///
/// Returns the seat that timed out, or `None` if there was no expired timer.
///
/// WHY THIS IS ONE FUNCTION. `player_action` used to *refuse* an action whose
/// timer had expired and change nothing else, while only `check_timeouts` ever
/// resolved the timeout. Nothing in the canister calls `check_timeouts` on its
/// own -- there is no heartbeat timer driving it, the frontend does -- so if the
/// frontend stopped polling, `state.action_on` stayed pointed at a seat that
/// could no longer act and the hand could not progress at all. That was hit
/// immediately in manual play: both seats ended up `Disconnected` and
/// `start_new_hand` refused with "Need at least 2 active players with chips"
/// until they were sat back in by hand. See docs/DEFECTS.md E-31.
///
/// A timed-out action must RESOLVE the hand state, because the alternative is a
/// table that no message can move. The seat's action is forfeited, the game
/// advances, and the next player gets a fresh clock starting now (they must not
/// be charged for the idle period).
///
/// WHAT A FORFEITED ACTION IS. It is a fold, including when checking would have
/// been free. That is NOT what online poker rooms do -- they check when there is
/// nothing to call -- and it is recorded as an audit finding rather than changed
/// here, because it changes which hands reach showdown and
/// `tests/money_safety/tests/regressions.rs` REG-08 pins the current behaviour.
/// This function deliberately preserves it.
///
/// The one addition to what `check_timeouts` did before is
/// `has_acted_this_round = true` on the folded seat, so that flag keeps meaning
/// exactly "has taken an action this round" -- which is what the incomplete-all-in
/// rule above derives from. It is unobservable: every read of it is already
/// guarded by `!has_folded`.
///
/// `now` is passed in, not read from `ic_cdk::api::time()`, so the timer path is
/// host-testable. See `tests/betting_rules.rs`.
pub fn resolve_expired_action_timer(state: &mut TableState, now: u64) -> Option<u8> {
    let seat = match state.action_timer {
        Some(ref timer) if now > timer.expires_at => timer.player_seat,
        _ => return None,
    };

    if seat as usize >= state.players.len() {
        // A timer pointing at a seat that does not exist cannot be resolved as a
        // fold. Drop it so it cannot wedge the table forever.
        state.action_timer = None;
        return None;
    }

    // Auto-fold the player
    if let Some(ref mut player) = state.players[seat as usize] {
        player.has_folded = true;
        player.has_acted_this_round = true;
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

    // Advance the game. This replaces the timer, so the same expiry can never be
    // resolved twice.
    advance_game(state, now);

    Some(seat)
}

/// Player heartbeat to show they're connected
#[ic_cdk::update]
fn heartbeat() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
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

#[derive(CandidType, Deserialize)]
struct PersistentState {
    balances: Vec<(Principal, u64)>,
    verified_deposits: Vec<(u64, Principal)>,
    /// Deposit replay-protection floor. `#[serde(default)]` so state written
    /// before this field existed restores as 0, which is correct: at that point
    /// nothing had been dropped, so no index was below the floor.
    #[serde(default)]
    deposit_watermark: u64,
    controllers: Vec<Principal>,
    history_id: Option<Principal>,
    #[serde(default)] // For backwards compatibility with old state
    dev_mode: bool, // Kept for deserialization compatibility, but always ignored
    table_config: Option<TableConfig>,
    table_state: Option<TableState>, // Save active game state
    hand_history: Vec<HandHistory>,
    current_actions: Vec<ActionRecord>,
    starting_chips: Vec<(u8, u64)>,
    rate_limits: Vec<(Principal, (u64, u32))>,
    shown_cards: Vec<(u64, Vec<u8>)>, // hand_number -> seats that showed
    #[serde(default)]
    current_seed: Option<Vec<u8>>, // Persist seed for mid-hand upgrades
    #[serde(default)]
    display_names: Vec<(Principal, String)>, // Custom display names

    // ------------------------------------------------------------------------
    // TABLE-STATE INTEGRITY DIGEST  (docs/DEFECTS.md E-38,
    //                                docs/SECURITY-FINDINGS.md FINDING 14)
    // ------------------------------------------------------------------------
    //
    // These three fields are redundant. They exist to make ONE specific silent
    // failure loud, and they are FLAT SCALARS wrapped in `opt` on purpose.
    //
    // `table_state` above is `opt TableState`. Candid's rule for `opt t` is that a
    // value which cannot be decoded as `t` arrives as **null**, not as an error. So
    // the moment anybody adds a field to `TableState` that is not itself `opt`, an
    // upgrade from state written before that field silently restores
    // `table_state = None`; `post_upgrade` then takes its "no active game state"
    // branch, calls `init_table_state`, and every seated player's chips and the live
    // pot are destroyed with nothing in the log. `#[serde(default)]` does NOT
    // prevent this: Candid does not honour serde defaults, only `opt`.
    //
    // That has already happened once in this file. `TableState::departed_stakes` is
    // a bare `vec`, so it is live right now -- currently masked only because
    // `deposit_watermark` is a bare `nat64` at the TOP level, which fails the whole
    // restore and gets the upgrade rejected instead. Fixing that one field alone
    // converts a rejected upgrade into silent chip destruction.
    //
    // An `opt` wrapping a flat scalar is the one shape that cannot itself be
    // silently dropped, so these stay readable when `table_state` does not, and
    // `post_upgrade` refuses the upgrade when they disagree with what came back.
    //
    // THIS IS A GUARD RAIL, NOT THE FIX. The fix is to make both fields `opt`, in
    // one change, plus a harness that upgrades from a PREVIOUS RELEASE's wasm
    // (docs/DEFECTS.md H-16). All this does is turn silent loss into a refusal.
    #[serde(default)]
    table_was_present: Option<bool>,
    #[serde(default)]
    table_pot_at_save: Option<u64>,
    #[serde(default)]
    table_seated_chips_at_save: Option<u64>,
}

/// `(pot, sum of seated players' chips)` for the live table, if there is one.
///
/// Used only to build and check [`PersistentState`]'s integrity digest.
fn table_digest() -> Option<(u64, u64)> {
    TABLE.with(|t| {
        t.borrow().as_ref().map(|s| {
            let seated = s
                .players
                .iter()
                .flatten()
                .fold(0u64, |acc, p| acc.saturating_add(p.chips));
            (s.pot, seated)
        })
    })
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let digest = table_digest();
    let state = PersistentState {
        balances: BALANCES.with(|b| b.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        verified_deposits: VERIFIED_DEPOSITS.with(|v| v.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        deposit_watermark: deposit_watermark(),
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
        current_seed: CURRENT_SEED.with(|s| s.borrow().clone()), // Save seed for mid-hand upgrades
        display_names: DISPLAY_NAMES.with(|d| d.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
        // See the field comments: redundant on purpose, and read back in post_upgrade.
        table_was_present: Some(digest.is_some()),
        table_pot_at_save: Some(digest.map(|(pot, _)| pot).unwrap_or(0)),
        table_seated_chips_at_save: Some(digest.map(|(_, chips)| chips).unwrap_or(0)),
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
        Ok((s,)) => s,
        Err(e) => {
            // FAIL LOUDLY - do NOT silently lose user funds!
            // If this panics, the upgrade will be rejected and the old code will remain.
            // This is much safer than silently losing all user balances.
            panic!("CRITICAL: Failed to restore state from stable memory: {:?}. \
                    Upgrade REJECTED to protect user funds. \
                    If you used --mode reinstall, that DESTROYS ALL DATA. \
                    Always use --mode upgrade for production canisters.", e);
        }
    };

    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        for (k, v) in state.balances {
            balances.insert(k, v);
        }
    });

    // Restore the anti-replay record. The watermark FIRST, so that if anything
    // below it somehow survived in the saved set it is still refused.
    DEPOSIT_WATERMARK.with(|w| {
        let mut w = w.borrow_mut();
        if state.deposit_watermark > *w {
            *w = state.deposit_watermark;
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

    // ------------------------------------------------------------------------
    // TABLE-STATE INTEGRITY CHECK (docs/SECURITY-FINDINGS.md FINDING 14)
    //
    // Runs BEFORE anything touches TABLE. If the redundant digest says a table was
    // saved and no table came back, the only way that happens is a Candid decode of
    // `opt TableState` silently yielding null -- which is what Candid does when a
    // non-`opt` field was added to `TableState`. Carrying on from here would
    // re-init an EMPTY table over the top of real chips, so refuse the upgrade
    // instead. `post_upgrade` panicking rejects the install and leaves the old code
    // and the old state in place, which is recoverable; destroyed chips are not.
    // ------------------------------------------------------------------------
    if state.table_was_present == Some(true) && state.table_state.is_none() {
        panic!(
            "CRITICAL: the saved state records a live table (pot {}, {} chips in seats) but \
             table_state decoded as null. That is what Candid does to an `opt` record whose \
             inner type gained a field that is not itself `opt`. Restoring would re-initialise \
             an EMPTY table and destroy every seated player's chips. Upgrade REJECTED. \
             See docs/SECURITY-FINDINGS.md FINDING 14 and docs/DEFECTS.md E-38.",
            state.table_pot_at_save.unwrap_or(0),
            state.table_seated_chips_at_save.unwrap_or(0)
        );
    }
    if let (Some(restored), Some(true)) = (state.table_state.as_ref(), state.table_was_present) {
        let seated = restored
            .players
            .iter()
            .flatten()
            .fold(0u64, |acc, p| acc.saturating_add(p.chips));
        let pot_ok = state.table_pot_at_save.is_none_or(|p| p == restored.pot);
        let chips_ok = state.table_seated_chips_at_save.is_none_or(|c| c == seated);
        if !pot_ok || !chips_ok {
            panic!(
                "CRITICAL: the restored table does not match the digest written beside it: \
                 pot {} (saved {:?}), seated chips {} (saved {:?}). Some part of TableState \
                 did not survive the decode. Upgrade REJECTED rather than settle from a \
                 half-restored table. See docs/SECURITY-FINDINGS.md FINDING 14.",
                restored.pot, state.table_pot_at_save, seated, state.table_seated_chips_at_save
            );
        }
    }

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
    subaccount: Option<[u8; 32]>,
}

/// Arguments for update_balance call to ckBTC minter
#[derive(CandidType, Deserialize)]
struct UpdateBalanceArgs {
    owner: Option<Principal>,
    subaccount: Option<[u8; 32]>,
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

    let args = GetBtcAddressArgs {
        owner: Some(caller),
        subaccount: None,
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

    let args = UpdateBalanceArgs {
        owner: Some(caller),
        subaccount: None,
    };

    #[derive(CandidType, Deserialize)]
    enum UpdateBalanceResult {
        Ok(Vec<UtxoStatus>),
        Err(UpdateBalanceError),
    }

    let result: Result<(UpdateBalanceResult,), _> = ic_cdk::call(minter, "update_balance", (args,)).await;

    match result {
        Ok((UpdateBalanceResult::Ok(statuses),)) => Ok(statuses),
        Ok((UpdateBalanceResult::Err(err),)) => {
            match err {
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
            }
        },
        Err((code, msg)) => Err(format!("Failed to update balance: {:?} - {}", code, msg)),
    }
}

// ============================================================================
// CANDID EXPORT
// ============================================================================

ic_cdk::export_candid!();

// ============================================================================
// PAYOUT PATH TESTS -- host-speed, no replica, no wasm
// ============================================================================
//
// docs/DEFECTS.md H-04: seven of seven mutations to THIS FILE survived with all
// 101 tests green, because nothing drove a hand through the canister's own
// settlement code and looked at the money. `tests/integration_test.rs` was
// comments only, and `tests/unit_tests.rs` tested private copies of the engine.
//
// These tests drive the REAL `determine_winners` / `end_hand_single_winner` /
// `plan_payouts` over a real `TableState`, on the host, in milliseconds. They are
// the fast leg of the proof; the slow leg is `tests/settlement/`, an independent
// oracle that derives what each seat is owed from the rules of poker and drives
// the real canister wasm on PocketIC against the real ICP ledger.
//
// Every case here states the money, the cards, and the answer the rules of poker
// give, so a reader can check the expected numbers by hand.

#[cfg(test)]
mod payout_tests {
    use super::*;
    use poker_core::{Rank, Suit};

    const SEC: u64 = 1_000_000_000;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card { suit, rank }
    }

    /// Two cards from a short notation, e.g. `hole("As", "8d")`.
    fn c(text: &str) -> Card {
        let bytes: Vec<char> = text.chars().collect();
        let rank = match bytes[0] {
            '2' => Rank::Two,
            '3' => Rank::Three,
            '4' => Rank::Four,
            '5' => Rank::Five,
            '6' => Rank::Six,
            '7' => Rank::Seven,
            '8' => Rank::Eight,
            '9' => Rank::Nine,
            'T' => Rank::Ten,
            'J' => Rank::Jack,
            'Q' => Rank::Queen,
            'K' => Rank::King,
            'A' => Rank::Ace,
            other => panic!("bad rank {other}"),
        };
        let suit = match bytes[1] {
            'h' => Suit::Hearts,
            'd' => Suit::Diamonds,
            'c' => Suit::Clubs,
            's' => Suit::Spades,
            other => panic!("bad suit {other}"),
        };
        card(rank, suit)
    }

    fn board(text: &str) -> Vec<Card> {
        text.split_whitespace().map(c).collect()
    }

    fn config(small_blind: u64, big_blind: u64) -> TableConfig {
        TableConfig {
            small_blind,
            big_blind,
            min_buy_in: 1,
            max_buy_in: u64::MAX / 4,
            max_players: 6,
            action_timeout_secs: 30,
            ante: 0,
            time_bank_secs: 30,
            currency: Currency::ICP,
        }
    }

    fn principal(seat: u8) -> Principal {
        Principal::from_slice(&[seat + 1])
    }

    /// A seat: chips behind, chips already in the pot this hand, folded, hole cards.
    struct Seat {
        seat: u8,
        chips: u64,
        wagered: u64,
        folded: bool,
        hole: Option<&'static str>,
    }

    fn seat(seat: u8, chips: u64, wagered: u64, hole: &'static str) -> Seat {
        Seat {
            seat,
            chips,
            wagered,
            folded: false,
            hole: Some(hole),
        }
    }

    fn folded_seat(seat: u8, chips: u64, wagered: u64) -> Seat {
        Seat {
            seat,
            chips,
            wagered,
            folded: true,
            hole: None,
        }
    }

    /// A table at the river with the given seats, ready to settle. `pot` is the sum
    /// of the stakes, exactly as `player_action` maintains it.
    fn table(seats: Vec<Seat>, board_text: &str, dealer_seat: u8) -> TableState {
        let cfg = config(1, 2);
        let mut players: Vec<Option<Player>> = (0..cfg.max_players).map(|_| None).collect();
        let mut pot = 0u64;
        for s in &seats {
            pot += s.wagered;
            players[s.seat as usize] = Some(Player {
                principal: principal(s.seat),
                seat: s.seat,
                chips: s.chips,
                hole_cards: s.hole.map(|h| {
                    let cards = board(h);
                    (cards[0], cards[1])
                }),
                current_bet: 0,
                total_bet_this_hand: s.wagered,
                has_folded: s.folded,
                has_acted_this_round: true,
                is_all_in: s.chips == 0,
                status: PlayerStatus::Active,
                last_seen: 0,
                timeout_count: 0,
                time_bank_remaining: 30,
                is_sitting_out_next_hand: false,
                broke_at: None,
                sitting_out_since: None,
            });
        }
        TableState {
            id: 0,
            config: cfg,
            players,
            community_cards: board(board_text),
            deck: poker_core::create_deck(),
            deck_index: 52,
            pot,
            side_pots: Vec::new(),
            current_bet: 0,
            min_raise: 2,
            phase: GamePhase::River,
            dealer_seat,
            small_blind_seat: 0,
            big_blind_seat: 1,
            action_on: 0,
            action_timer: None,
            shuffle_proof: None,
            hand_number: 7,
            last_aggressor: None,
            bb_has_option: false,
            first_hand: false,
            auto_deal_at: None,
            last_action: None,
            departed_stakes: Vec::new(),
        }
    }

    /// Everything the canister owes: chips in front of players plus what is still
    /// in the pot. Must not change across a settlement.
    fn table_value(state: &TableState) -> u64 {
        state
            .players
            .iter()
            .flatten()
            .fold(state.pot, |a, p| a.saturating_add(p.chips))
    }

    fn chips_at(state: &TableState, seat: u8) -> u64 {
        state.players[seat as usize]
            .as_ref()
            .map(|p| p.chips)
            .unwrap_or(0)
    }

    fn settle(state: &mut TableState) -> Vec<Winner> {
        state.phase = GamePhase::Showdown;
        let before = table_value(state);
        determine_winners(state, 10 * SEC);
        assert_eq!(
            table_value(state),
            before,
            "CONSERVATION: a settlement may not change what the table owes in total"
        );
        assert_eq!(state.pot, 0, "the pot must be empty after settling");
        LAST_HAND_WINNERS.with(|w| w.borrow().clone())
    }

    // -----------------------------------------------------------------------
    // E-01: the post-flop pot
    // -----------------------------------------------------------------------

    /// THE LEAD'S SCENARIO, exactly. Heads-up, blinds 1,000,000 / 2,000,000.
    /// Pre-flop call and check (2,000,000 each). A 30,000,000 bet and call on the
    /// flop. Checks to showdown. The pot is 64,000,000 and the winner must get all
    /// of it.
    ///
    /// Before the fix this hand paid the winner 4,000,000 -- the frozen pre-flop
    /// breakdown -- and destroyed 60,000,000 (docs/FINDING-01-chip-destruction.md).
    #[test]
    fn e01_the_whole_post_flop_pot_goes_to_the_winner() {
        let mut state = table(
            vec![
                // 200,000,000 buy-in each, 32,000,000 of it already in the pot.
                seat(0, 168_000_000, 32_000_000, "Ac Qc"),
                seat(1, 168_000_000, 32_000_000, "Kd 4c"),
            ],
            "Jd Ah 7h 5c Kc",
            1,
        );
        assert_eq!(state.pot, 64_000_000);

        let winners = settle(&mut state);

        // Seat 0 holds a pair of aces, seat 1 a pair of kings.
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].seat, 0);
        assert_eq!(
            winners[0].amount, 64_000_000,
            "the winner is paid the WHOLE pot, not the pre-flop part of it"
        );
        assert_eq!(chips_at(&state, 0), 168_000_000 + 64_000_000);
        assert_eq!(chips_at(&state, 1), 168_000_000);
    }

    /// The same shape with the side-pot breakdown deliberately stale, which is the
    /// state the engine was ALWAYS in from the flop onwards: `state.side_pots` was
    /// built once at the PreFlop -> Flop transition and never rebuilt.
    ///
    /// A stale breakdown must not be able to move one chip.
    #[test]
    fn e01_a_stale_side_pot_breakdown_cannot_change_the_payout() {
        let fresh = {
            let mut state = table(
                vec![
                    seat(0, 100, 62, "Ac Qc"),
                    seat(1, 100, 62, "Kd 4c"),
                    seat(2, 100, 62, "4s 3s"),
                    seat(3, 100, 62, "2s Qd"),
                ],
                "Jd Ah 7h 5c Kc",
                1,
            );
            settle(&mut state);
            (chips_at(&state, 0), chips_at(&state, 1))
        };

        let mut state = table(
            vec![
                seat(0, 100, 62, "Ac Qc"),
                seat(1, 100, 62, "Kd 4c"),
                seat(2, 100, 62, "4s 3s"),
                seat(3, 100, 62, "2s Qd"),
            ],
            "Jd Ah 7h 5c Kc",
            1,
        );
        // Exactly what E-01 froze: the pre-flop money only, 2 chips per seat.
        state.side_pots = vec![SidePot {
            amount: 8,
            eligible_players: vec![0, 1, 2, 3],
        }];
        let winners = settle(&mut state);

        assert_eq!(winners[0].seat, 0);
        assert_eq!(winners[0].amount, 248, "all four streets, not the pre-flop 8");
        assert_eq!((chips_at(&state, 0), chips_at(&state, 1)), fresh);
    }

    /// A hand that ends because everybody folded is paid from the same basis as a
    /// showdown. Before the fix these were two different code paths reading two
    /// different accounts of the pot, and only one of them was right.
    #[test]
    fn a_fold_out_and_a_showdown_are_paid_from_the_same_basis() {
        let mut state = table(
            vec![
                seat(0, 100, 62, "Ac Qc"),
                folded_seat(1, 100, 62),
                folded_seat(2, 100, 62),
            ],
            "Jd Ah 7h 5c Kc",
            1,
        );
        let before = table_value(&state);
        end_hand_single_winner(&mut state, 10 * SEC);
        assert_eq!(table_value(&state), before, "CONSERVATION");
        assert_eq!(state.pot, 0);
        assert_eq!(
            chips_at(&state, 0),
            100 + 186,
            "the last player standing takes every chip collected"
        );
    }

    /// A hand that ends BEFORE the flop has no board, and the last player standing
    /// takes the pot without showing a hand. Ranking them would be both unnecessary
    /// and impossible: `poker_core::evaluate_hand` traps on a 0-card board, by
    /// design, so a settlement that ranks unconditionally traps on every pre-flop
    /// fold-out. This test is that trap, pinned.
    #[test]
    fn a_pre_flop_fold_out_settles_with_no_board_at_all() {
        let mut state = table(
            vec![
                seat(0, 100, 1, "Ac Qc"),  // small blind, folds
                seat(1, 100, 2, "Kd 4c"),  // big blind, wins
            ],
            "",
            0,
        );
        state.phase = GamePhase::PreFlop;
        if let Some(p) = state.players[0].as_mut() {
            p.has_folded = true;
        }
        assert!(state.community_cards.is_empty());

        let before = table_value(&state);
        end_hand_single_winner(&mut state, 10 * SEC);
        assert_eq!(table_value(&state), before, "CONSERVATION");
        assert_eq!(state.pot, 0);
        // The big blind's own uncalled 1 comes back, and it wins the 2 that was
        // contested: 100 + 1 + 2 = 103 all told, and the small blind keeps 100.
        assert_eq!(chips_at(&state, 1), 103);
        assert_eq!(chips_at(&state, 0), 100);
        let winners = LAST_HAND_WINNERS.with(|w| w.borrow().clone());
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].seat, 1);
        assert!(
            winners[0].hand_rank.is_none(),
            "a pot won by everybody folding shows no hand"
        );
    }

    // -----------------------------------------------------------------------
    // E-03: state.pot can never be the payout basis
    // -----------------------------------------------------------------------

    /// E-03 direction A: `state.pot` OVERSTATES the contributions. The old code
    /// appended the difference to the highest bet level -- minting chips into the
    /// pot only the deepest stacks can win. It must now be impossible for
    /// `state.pot` to move a chip.
    #[test]
    fn e03_an_overstated_pot_cannot_mint_a_chip() {
        let mut state = table(
            vec![
                seat(0, 0, 50, "As 8d"),   // short all-in, best hand
                seat(1, 100, 200, "4d 9h"),
                seat(2, 100, 200, "7s Th"),
            ],
            "Kd Tc 3d 2c Ad",
            1,
        );
        let honest = state.pot;
        assert_eq!(honest, 450);
        // Corrupt the redundant accumulator by 100.
        state.pot += 100;

        let plan = plan_payouts(&state);
        assert_eq!(
            plan.collected, honest,
            "the payout basis is the contributions, not state.pot"
        );
        assert_eq!(
            plan.side_pots.iter().map(|p| p.amount).sum::<u64>(),
            honest,
            "and no side pot was inflated to meet state.pot"
        );
        assert!(plan.conserves());

        // Seat 0 has a pair of aces and wins the main pot; seat 2 has a pair of
        // tens and beats seat 2's ace-high for the rest.
        assert_eq!(plan.amount_for(0), 150, "3 x 50");
        assert_eq!(plan.amount_for(2), 300, "2 x 150");
        assert_eq!(plan.amount_for(1), 0);
    }

    /// E-03 direction B: `state.pot` UNDERSTATES the contributions. The old code
    /// scaled every side pot down through an `f64` ratio and destroyed the
    /// difference, and logged `BUG: Side pots (...) exceed total pot (...)` while
    /// settling anyway.
    #[test]
    fn e03_an_understated_pot_cannot_destroy_a_chip() {
        let mut state = table(
            vec![
                seat(0, 0, 50, "As 8d"),
                seat(1, 100, 200, "4d 9h"),
                seat(2, 100, 200, "7s Th"),
            ],
            "Kd Tc 3d 2c Ad",
            1,
        );
        state.pot = 0; // the shape the fuzzer actually produced

        let plan = plan_payouts(&state);
        assert_eq!(plan.collected, 450);
        assert_eq!(plan.awarded, 450, "every chip wagered is still paid out");
        assert!(plan.conserves());
    }

    /// At e8 magnitudes the `f64` ratio the old reconciliation used cannot even
    /// represent the pot. Nothing on the payout path may touch a float.
    #[test]
    fn e03_the_payout_is_exact_at_magnitudes_f64_cannot_represent() {
        let big = 9_007_199_254_740_993u64; // 2^53 + 1
        let mut state = table(
            vec![seat(0, 0, big, "Ac Qc"), seat(1, 0, big, "Kd 4c")],
            "Jd Ah 7h 5c Kc",
            1,
        );
        let winners = settle(&mut state);
        assert_eq!(winners[0].seat, 0);
        assert_eq!(winners[0].amount, 2 * big, "not one e8 lost to rounding");
        assert_eq!(chips_at(&state, 0), 2 * big);
    }

    // -----------------------------------------------------------------------
    // E-05: a vacated seat
    // -----------------------------------------------------------------------

    /// THE E-05 REGRESSION. Seat 1 folds with 60 committed and leaves the table
    /// before the pots are built. The main pot the honest short all-in can win must
    /// still contain seat 1's 20 of it.
    ///
    /// Before the fix `collect_contributions` read the seat vector, seat 1 was no
    /// longer in it, and the missing 60 was appended to the pot only the deep stacks
    /// could win: seat 0's main pot fell from 80 to 60 and 20 chips moved to seat 3,
    /// with every chip conserved so no conservation invariant could see it.
    #[test]
    fn e05_a_departed_seats_stake_stays_in_the_main_pot() {
        let mut state = table(
            vec![
                seat(0, 0, 20, "As 8d"),    // all-in, pair of aces: best hand
                folded_seat(1, 0, 60),      // folds, then leaves
                seat(2, 100, 200, "4d 9h"), // ace high
                seat(3, 100, 200, "7s Th"), // pair of tens
            ],
            "Kd Tc 3d 2c Ad",
            1,
        );
        // Exactly what `leave_table` does now: record the stake, then vacate.
        let recorded = record_departed_stake(&mut state, 1);
        assert_eq!(recorded, 60);
        state.players[1] = None;

        let contributions = hand_contributions(&state);
        assert_eq!(
            poker_core::total_contributed(&contributions),
            480,
            "the departed stake is still in the payout basis"
        );

        let plan = plan_payouts(&state);
        assert_eq!(
            plan.side_pots
                .iter()
                .map(|p| (p.amount, p.eligible_players.clone()))
                .collect::<Vec<_>>(),
            vec![
                (80, vec![0, 2, 3]),
                (120, vec![2, 3]),
                (280, vec![2, 3]),
            ],
            "the main pot is four seats times 20, including the seat that left"
        );
        assert_eq!(plan.amount_for(0), 80, "the short all-in wins the whole main pot");
        assert_eq!(plan.amount_for(3), 400);
        assert_eq!(plan.amount_for(2), 0);
        assert!(plan.conserves());
    }

    /// The same hand, settled two ways: seat 1 folds and STAYS, versus seat 1 folds
    /// and LEAVES. Nothing about the money changed between them, so not one chip may
    /// move. That equality is the whole of E-05.
    #[test]
    fn e05_folding_and_leaving_pay_out_identically() {
        let make = || {
            table(
                vec![
                    seat(0, 0, 20, "As 8d"),
                    folded_seat(1, 0, 60),
                    seat(2, 100, 200, "4d 9h"),
                    seat(3, 100, 200, "7s Th"),
                ],
                "Kd Tc 3d 2c Ad",
                1,
            )
        };

        let mut stayed = make();
        let stayed_winners = settle(&mut stayed);

        let mut left = make();
        record_departed_stake(&mut left, 1);
        left.players[1] = None;
        let left_winners = settle(&mut left);

        let render = |ws: &[Winner]| {
            let mut v: Vec<(u8, u64)> = ws.iter().map(|w| (w.seat, w.amount)).collect();
            v.sort_unstable();
            v
        };
        assert_eq!(render(&stayed_winners), render(&left_winners));
        assert_eq!(render(&stayed_winners), vec![(0, 80), (3, 400)]);
    }

    // -----------------------------------------------------------------------
    // uncalled bets
    // -----------------------------------------------------------------------

    /// A bet nobody covered comes back to the bettor before the pots are formed,
    /// so the displayed pot stops including money that was never in play.
    #[test]
    fn an_uncalled_bet_is_returned_before_the_pots_are_formed() {
        let mut state = table(
            vec![
                seat(0, 0, 20, "7s 4h"),
                folded_seat(1, 40, 60),
                seat(2, 300, 100, "7d 6h"), // bet 40 more than anyone covered
                folded_seat(3, 40, 60),
            ],
            "Qh Ad 2h 2s 8h",
            1,
        );
        assert_eq!(state.pot, 240);

        let returned = return_uncalled_bet(&mut state);
        assert_eq!(returned, Some((2, 40)));
        assert_eq!(chips_at(&state, 2), 340, "the 40 is back in front of seat 2");
        assert_eq!(state.pot, 200, "and out of the displayed pot");
        assert_eq!(
            state.players[2].as_ref().unwrap().total_bet_this_hand,
            60,
            "the two accounts of the pot stay in step"
        );
        assert_eq!(return_uncalled_bet(&mut state), None, "idempotent");

        // Seats 0 and 2 both play the board for a pair of deuces, so the 80 main pot
        // is chopped and seat 2 takes the 120 above seat 0's reach.
        let plan = plan_payouts(&state);
        assert_eq!(plan.amount_for(0), 40);
        assert_eq!(plan.amount_for(2), 160);
        assert!(plan.conserves());
    }

    /// The net result of a hand must not depend on WHETHER the uncalled bet was
    /// returned separately: an uncontested overbet used to sit in a solo side pot
    /// and reach the same player. This pins that the fix did not change who ends up
    /// with the money, only when and how it is described.
    #[test]
    fn returning_an_uncalled_bet_changes_no_seats_net_position() {
        let seats = || {
            vec![
                seat(0, 0, 100, "As 8d"),
                seat(1, 0, 300, "7s Th"), // over-bet: 200 uncovered
            ]
        };
        let mut with_return = table(seats(), "Kd Tc 3d 2c Ad", 1);
        return_uncalled_bet(&mut with_return);
        let winners_a = settle(&mut with_return);

        let mut without = table(seats(), "Kd Tc 3d 2c Ad", 1);
        let winners_b = settle(&mut without);

        // Seat 0 holds a pair of aces and wins the 200 that was actually contested;
        // seat 1 gets its uncontested 200 back either way.
        assert_eq!(chips_at(&with_return, 1), chips_at(&without, 1));
        assert_eq!(chips_at(&with_return, 0), chips_at(&without, 0));
        assert_eq!(chips_at(&with_return, 0), 200);
        assert_eq!(chips_at(&with_return, 1), 200);
        // What DID change is the description. Only the 200 that was actually
        // contested is recorded as won; before the fix the engine left seat 1's
        // uncovered 200 in a side pot only seat 1 was eligible for and recorded it
        // as a 200 win, which is why the pot the players were shown was inflated.
        assert_eq!(winners_a.iter().map(|w| w.amount).sum::<u64>(), 200);
        assert_eq!(
            winners_b.iter().map(|w| w.amount).sum::<u64>(),
            200,
            "settlement returns the uncalled bet itself, so both routes agree"
        );
        assert!(
            !winners_a.iter().any(|w| w.seat == 1),
            "seat 1 won nothing: it got its own uncontested bet back"
        );
    }

    // -----------------------------------------------------------------------
    // odd chips
    // -----------------------------------------------------------------------

    /// A three-way chop that leaves two odd chips must place them one each,
    /// clockwise from the button. The engine used to give both to a single seat.
    /// docs/DEFECTS.md E-35, settlement oracle D-04.
    #[test]
    fn e35_odd_chips_go_one_each_clockwise_from_the_button() {
        // Three seats play the board for an exact tie, over a folded seat's 2. That
        // is what makes a layer indivisible by three: three EQUAL live stakes always
        // divide by three, so the remainder has to come from dead money underneath
        // them. The settlement oracle found the same shape on the real canister
        // (D-04) with the same 8-chip layer.
        //
        // Layers: (0,2] = 4 x 2 = 8 chopped three ways -> 2 each and TWO over;
        //         (2,7] = 3 x 5 = 15 chopped three ways -> 5 each, nothing over.
        let make = |dealer: u8| {
            table(
                vec![
                    folded_seat(0, 0, 2),
                    seat(1, 0, 7, "2c 3c"),
                    seat(2, 0, 7, "2d 4d"),
                    seat(3, 0, 7, "2h 5h"),
                ],
                "As Ks Qh Jd Td",
                dealer,
            )
        };
        // Button on seat 0, so clockwise order is 1, 2, 3: the two odd chips go to
        // seats 1 and 2, one each. The engine used to give BOTH to seat 1.
        let mut state = make(0);
        assert_eq!(state.pot, 23);
        let winners = settle(&mut state);
        let mut paid: Vec<(u8, u64)> = winners.iter().map(|w| (w.seat, w.amount)).collect();
        paid.sort_unstable();
        assert_eq!(paid, vec![(1, 8), (2, 8), (3, 7)]);
        assert_eq!(paid.iter().map(|(_, a)| a).sum::<u64>(), 23);
        assert!(
            paid.iter().all(|(_, a)| *a >= 7 && *a <= 8),
            "no winner may be more than one chip clear of another: {paid:?}"
        );

        // And the placement follows the button: from seat 2 the order is 3, 1, 2.
        let mut state = make(2);
        let winners = settle(&mut state);
        let mut paid: Vec<(u8, u64)> = winners.iter().map(|w| (w.seat, w.amount)).collect();
        paid.sort_unstable();
        assert_eq!(paid, vec![(1, 8), (2, 7), (3, 8)]);
        assert_eq!(paid.iter().map(|(_, a)| a).sum::<u64>(), 23);
    }

    /// A two-way chop of an odd pot: the single odd chip goes to the first winner
    /// clockwise from the button, which is where the old rule and this one agree.
    ///
    /// The odd chip comes from a folded seat's 1, so that no part of the pot is an
    /// uncalled bet -- two seats cannot contribute unequally without one of them
    /// being owed the difference back.
    #[test]
    fn a_two_way_chop_of_an_odd_pot_places_its_one_chip_clockwise() {
        for (dealer, expect) in [
            (0u8, vec![(1u8, 4u64), (2, 3)]),
            (1, vec![(1, 3), (2, 4)]),
        ] {
            let mut state = table(
                vec![
                    folded_seat(0, 0, 1),
                    seat(1, 0, 3, "2c 3c"),
                    seat(2, 0, 3, "2d 4d"),
                ],
                "As Ks Qh Jd Td",
                dealer,
            );
            assert_eq!(state.pot, 7);
            let winners = settle(&mut state);
            let mut paid: Vec<(u8, u64)> = winners.iter().map(|w| (w.seat, w.amount)).collect();
            paid.sort_unstable();
            // Layers: (0,1] = 3 chips chopped two ways, one odd chip to place;
            //         (1,3] = 4 chips chopped two ways, 2 each.
            assert_eq!(paid, expect, "button on seat {dealer}");
            assert_eq!(paid.iter().map(|(_, a)| a).sum::<u64>(), 7);
        }
    }

    // -----------------------------------------------------------------------
    // the post-condition, and the shapes real betting cannot produce
    // -----------------------------------------------------------------------

    /// Money nobody left at the table can win is refunded to the seats that put it
    /// there, and to the ESCROW of a player who has already left. Real betting
    /// cannot reach this; two `leave_table` calls can.
    #[test]
    fn money_nobody_can_win_is_refunded_not_handed_to_the_deepest_stack() {
        let mut state = table(
            vec![folded_seat(0, 0, 30), folded_seat(1, 0, 30)],
            "As Ks Qh Jd Td",
            1,
        );
        let plan = plan_payouts(&state);
        assert!(plan.conserves());
        assert_eq!(plan.amount_for(0), 30);
        assert_eq!(plan.amount_for(1), 30);
        assert!(plan
            .payouts
            .iter()
            .all(|p| matches!(p.reason, PayoutReason::Refund { .. })));

        // And applying it really moves the chips.
        let before = table_value(&state);
        state.phase = GamePhase::Showdown;
        determine_winners(&mut state, 10 * SEC);
        assert_eq!(table_value(&state), before);
        assert_eq!(chips_at(&state, 0), 30);
    }

    /// The refusal. A plan that does not pay out exactly what it collected must
    /// never be applied.
    ///
    /// `should_panic` carries no expected string because off-canister
    /// `ic_cdk::trap` panics with its own "trap should only be called inside
    /// canisters" message rather than the argument; on the replica the argument IS
    /// the message, and it names the hand, the shortfall and the whole plan.
    #[test]
    #[should_panic]
    fn a_plan_that_does_not_add_up_is_refused() {
        let mut state = table(
            vec![seat(0, 0, 100, "Ac Qc"), seat(1, 0, 100, "Kd 4c")],
            "Jd Ah 7h 5c Kc",
            1,
        );
        let mut plan = plan_payouts(&state);
        assert!(plan.conserves());
        // A rake: keep one chip of a 200 pot.
        plan.awarded -= 1;
        plan.payouts[0].amount -= 1;
        assert!(!plan.conserves(), "the plan now keeps a chip");
        apply_payouts(&mut state, &plan);
    }

    /// Property: for any shape of stakes and any board, the plan pays out exactly
    /// what the hand collected. This is the post-condition E-01 and E-03 broke, and
    /// the no-rake property makes it exact to the e8.
    #[test]
    fn every_settlement_pays_out_exactly_what_it_collected() {
        let deck = poker_core::create_deck();
        let mut seed = 0xc1ea_dec4_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 33
        };
        let mut settled = 0usize;
        let mut with_side_pots = 0usize;
        let mut with_chops = 0usize;

        for case in 0..2_000 {
            let n = 2 + (next() % 4) as usize;
            // Deal without replacement out of a shuffled deck.
            let mut cards = deck.clone();
            for i in (1..cards.len()).rev() {
                let j = (next() % (i as u64 + 1)) as usize;
                cards.swap(i, j);
            }
            let mut seats: Vec<Seat> = Vec::new();
            for i in 0..n {
                let wagered = 1 + next() % 500;
                let folded = next() % 4 == 0;
                let hole: Vec<Card> = cards[i * 2..i * 2 + 2].to_vec();
                seats.push(Seat {
                    seat: i as u8,
                    chips: next() % 100,
                    wagered,
                    folded,
                    hole: None,
                });
                // Hole cards have to be set by hand: the notation helper only takes
                // static text.
                let _ = hole;
            }
            // Up to five seats take cards[0..10], so the board starts at 10.
            let board_cards: Vec<Card> = cards[10..15].to_vec();
            let mut state = table(seats, "As Ks Qh Jd Td", (next() % 6) as u8);
            state.community_cards = board_cards;
            for (i, p) in state.players.iter_mut().enumerate() {
                if let Some(p) = p {
                    p.hole_cards = Some((cards[i * 2], cards[i * 2 + 1]));
                }
            }

            let plan = plan_payouts(&state);
            assert!(
                plan.conserves(),
                "case {case}: awarded {} of {} collected. contributions={:?} pots={:?}",
                plan.awarded,
                plan.collected,
                hand_contributions(&state),
                plan.side_pots
            );
            if plan.side_pots.len() > 1 {
                with_side_pots += 1;
            }
            let payees: std::collections::BTreeSet<u8> =
                plan.payouts.iter().map(|p| p.seat).collect();
            if payees.len() > 1 {
                with_chops += 1;
            }

            // And applying it conserves the table's total value.
            state.phase = GamePhase::Showdown;
            let before = table_value(&state);
            determine_winners(&mut state, 10 * SEC);
            assert_eq!(table_value(&state), before, "case {case}: CONSERVATION");
            assert_eq!(state.pot, 0, "case {case}: the pot must be settled");
            settled += 1;
        }
        assert_eq!(settled, 2_000);
        assert!(
            with_side_pots > 100,
            "only {with_side_pots} of the sweep had a real side-pot ladder"
        );
        assert!(
            with_chops > 50,
            "only {with_chops} of the sweep paid more than one seat"
        );
    }

    /// The two accounts of the pot must agree in every ordinary hand, including one
    /// where a seat left mid-hand. This is the check that would catch a future
    /// `state.pot` update with no matching `total_bet_this_hand` update.
    #[test]
    fn the_pot_and_the_contributions_agree_even_when_a_seat_leaves() {
        let mut state = table(
            vec![
                seat(0, 0, 20, "As 8d"),
                folded_seat(1, 0, 60),
                seat(2, 100, 200, "4d 9h"),
                seat(3, 100, 200, "7s Th"),
            ],
            "Kd Tc 3d 2c Ad",
            1,
        );
        assert_eq!(
            poker_core::total_contributed(&hand_contributions(&state)),
            state.pot
        );
        record_departed_stake(&mut state, 1);
        state.players[1] = None;
        assert_eq!(
            poker_core::total_contributed(&hand_contributions(&state)),
            state.pot,
            "vacating a seat must not change the payout basis"
        );
    }

    /// A stake from an earlier hand can never join this hand's pot.
    #[test]
    fn a_stale_departed_stake_is_ignored() {
        let mut state = table(vec![seat(0, 0, 20, "As 8d"), seat(1, 0, 20, "4d 9h")], "Kd Tc 3d 2c Ad", 1);
        state.departed_stakes.push(DepartedStake {
            hand_number: state.hand_number - 1,
            seat: 4,
            principal: principal(4),
            contributed: 1_000_000,
        });
        let plan = plan_payouts(&state);
        assert_eq!(plan.collected, 40, "the stale stake is not in the basis");
        assert!(plan.conserves());
    }
}
