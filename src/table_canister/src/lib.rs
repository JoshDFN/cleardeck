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
            // Only allow alphanumeric and some safe symbols
            if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ') {
                return Err("Name can only contain letters, numbers, spaces, underscores and hyphens".to_string());
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
            // Convert action to history format
            let hist_action = match &action.action {
                PlayerAction::Fold => HistoryPlayerAction::Fold,
                PlayerAction::Check => HistoryPlayerAction::Check,
                PlayerAction::Call => HistoryPlayerAction::Call(0), // Amount not tracked in simple format
                PlayerAction::Bet(amt) => HistoryPlayerAction::Bet(*amt),
                PlayerAction::Raise(amt) => HistoryPlayerAction::Raise(*amt),
                PlayerAction::AllIn => HistoryPlayerAction::AllIn(0),
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

/// Legacy function for backwards compatibility - calls transfer_tokens
async fn transfer_icp(to: Principal, amount: u64) -> Result<u64, String> {
    transfer_tokens(to, amount).await
}

/// Verify and credit a deposit by checking the ledger transaction
/// Players should first transfer ICP to the canister's account, then call this with the block index
#[ic_cdk::update]
async fn notify_deposit(block_index: u64) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
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

    let call_result = ic_cdk::call::Call::unbounded_wait(ledger_id, "query_blocks")
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

        // Note: We can't verify the sender's principal from the account identifier
        // because account identifiers are one-way hashes. We trust that if the
        // transfer was sent to us and the caller is claiming it, that's valid.
        // The deposit can only be claimed once due to VERIFIED_DEPOSITS check.

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
        if let Some(Operation::Transfer(ref transfer)) = block.transaction.operation {
            match verify_and_credit(transfer) {
                Ok(new_balance) => {
                    clear_pending();
                    return Ok(new_balance);
                }
                Err(e) => {
                    clear_pending();
                    return Err(e);
                }
            }
        } else {
            clear_pending();
            return Err("Transaction is not a transfer".to_string());
        }
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

    let amount: u64 = transfer.amount.0.clone().try_into().unwrap_or(0);
    if amount == 0 {
        return Err("Invalid transaction amount".to_string());
    }

    // Mark deposit as verified (prevent double-crediting)
    let already_verified = VERIFIED_DEPOSITS.with(|v| {
        let mut deposits = v.borrow_mut();
        if deposits.contains_key(&block_index) {
            true
        } else {
            deposits.insert(block_index, caller);
            false
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

/// Withdraw your balance from the table
#[ic_cdk::update]
async fn withdraw(amount: u64) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
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
    let result = transfer_icp(caller, amount).await;

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

    // If the table operation failed, refund the escrow
    if result.is_err() {
        BALANCES.with(|b| {
            let mut balances = b.borrow_mut();
            let current = balances.get(&caller).copied().unwrap_or(0);
            balances.insert(caller, current + deducted);
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

    // Return chips to escrow balance
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        balances.insert(caller, current + chips);
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

/// Admin: Restore balance for a player (TEMPORARY - for recovery only)
/// Controller only - ADDS to existing balance
#[ic_cdk::update]
fn admin_restore_balance(player: Principal, amount: u64) -> Result<u64, String> {
    require_controller()?;

    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&player).copied().unwrap_or(0);
        let new_balance = current.saturating_add(amount);
        balances.insert(player, new_balance);
        Ok(new_balance)
    })
}

/// Admin: Get all balances (for auditing/recovery)
/// Returns (total_assigned, list of (principal, balance))
/// Controller only
#[ic_cdk::query]
fn admin_get_all_balances() -> Result<(u64, Vec<(Principal, u64)>), String> {
    require_controller()?;

    BALANCES.with(|b| {
        let balances = b.borrow();
        let list: Vec<(Principal, u64)> = balances.iter().map(|(k, v)| (*k, *v)).collect();
        let total: u64 = balances.values().sum();
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
                    .sum();
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

fn check_straight(ranks: &[u8]) -> bool {
    let mut sorted: Vec<u8> = ranks.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();

    if sorted.len() < 5 {
        return false;
    }

    // Check for regular straight
    for window in sorted.windows(5) {
        if window[0] - window[4] == 4 {
            return true;
        }
    }

    // Check for wheel (A-2-3-4-5)
    if sorted.contains(&14) && sorted.contains(&5) && sorted.contains(&4)
        && sorted.contains(&3) && sorted.contains(&2) {
        return true;
    }

    false
}

fn get_straight_high(ranks: &[u8]) -> u8 {
    let mut sorted: Vec<u8> = ranks.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();

    // Check for wheel first
    if sorted.contains(&14) && sorted.contains(&5) && sorted.contains(&4)
        && sorted.contains(&3) && sorted.contains(&2) {
        return 5; // Wheel's high card is 5
    }

    // Regular straight
    for window in sorted.windows(5) {
        if window[0] - window[4] == 4 {
            return window[0];
        }
    }

    0
}

// ============================================================================
// GAME FLOW
// ============================================================================

#[ic_cdk::update]
async fn start_new_hand() -> Result<ShuffleProof, String> {
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
/// If mid-hand, this acts as a fold - pot contributions stay in the pot
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

    // Return remaining chips to escrow balance
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&caller).copied().unwrap_or(0);
        balances.insert(caller, current + chips);
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
                // BB can check even if current_bet equals their posted blind
                let can_check = state.current_bet == player_current_bet
                    || (is_bb_option && state.current_bet == state.config.big_blind);

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

        // Record action with current phase
        let current_phase = phase_to_string(&state.phase);
        CURRENT_ACTIONS.with(|a| {
            a.borrow_mut().push(ActionRecord {
                seat: player_seat as u8,
                action: action.clone(),
                timestamp: now,
                phase: current_phase,
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
            *chips_awarded.entry(*seat).or_insert(0) += amount;

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
                *chips_awarded.entry(*seat).or_insert(0) += amount;

                // Only add to winner list once per player (aggregate amounts)
                if let Some(existing) = winner_list.iter_mut().find(|w| w.seat == *seat) {
                    existing.amount += amount;
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
                                    *balances.entry(principal).or_insert(0) += chips;
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
                    player.timeout_count += 1;

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
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = PersistentState {
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
        current_seed: CURRENT_SEED.with(|s| s.borrow().clone()), // Save seed for mid-hand upgrades
        display_names: DISPLAY_NAMES.with(|d| d.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
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
