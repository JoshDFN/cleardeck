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

/// How long a seat may go without a heartbeat before it is SHOWN as disconnected.
///
/// This number no longer decides anything about money. Before the E-32 fix it did:
/// a seat that crossed it was dropped out of the betting round while staying
/// eligible for the pot, so 30 seconds of a flaky connection either handed a
/// cheater a free showdown or took an honest player's hand off them. Participation
/// is now decided by [`is_in_hand`], and the only clock that can fold anybody is
/// the ACTION clock (`TableConfig::action_timeout_secs`, plus the seat's time
/// bank), which is the clock a player can actually see running.
///
/// What is left for it to do is cosmetic and slow: the "disconnected" badge in the
/// UI, exclusion from the NEXT deal (a seat that is not there does not get cards),
/// and the start of the [`SITTING_OUT_KICK_SECS`] clock that eventually frees the
/// chair and returns the stack to escrow. 30 seconds was mean for all three -- a
/// tab reload can cost that -- so it is 90, giving a real disconnection 90 + 120 =
/// 3 and a half minutes before its seat is given away.
const DISCONNECT_TIMEOUT_SECS: u64 = 90;

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
    /// # Why this is `opt` and must stay `opt` (docs/SECURITY-FINDINGS.md FINDING 14)
    ///
    /// `TableState` is persisted NESTED inside `PersistentState::table_state`,
    /// which is `opt TableState`. Candid's rule for `opt t` is that a value which
    /// cannot be decoded as `t` arrives as **null**, not as an error, so the
    /// moment this field is anything other than `opt` an upgrade from state
    /// written before it existed restores `table_state = None`, `post_upgrade`
    /// re-initialises an empty table, and every seated player's chips and the live
    /// pot are destroyed with nothing in the log. This shipped once as a bare
    /// `vec` with a `#[serde(default)]` and a comment claiming that was
    /// backward-compatible: **Candid does not honour serde defaults.** Only `opt`,
    /// `reserved` and `null` may be added to a record and still read older state.
    ///
    /// `None` and `Some(vec![])` mean the same thing here -- no seat has left this
    /// hand -- and everything reads it through
    /// [`TableState::departed_stakes`](TableState::departed_stakes) /
    /// [`TableState::departed_stakes_mut`](TableState::departed_stakes_mut) so no
    /// call site has to care which one it is.
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
    pub departed_stakes: Option<Vec<DepartedStake>>,
}

impl TableState {
    /// Every stake recorded for a seat that left mid-hand. Empty when the field
    /// is `None`, which is the same thing as an empty list.
    pub fn departed_stakes(&self) -> &[DepartedStake] {
        self.departed_stakes.as_deref().unwrap_or(&[])
    }

    /// The departed-stake list, created empty if it does not exist yet.
    pub fn departed_stakes_mut(&mut self) -> &mut Vec<DepartedStake> {
        self.departed_stakes.get_or_insert_with(Vec::new)
    }

    /// Forget every departed stake. Only correct at a hand boundary, when the
    /// money they represent has already been paid out.
    pub fn clear_departed_stakes(&mut self) {
        self.departed_stakes = None;
    }
}

/// The stake of a seat that was vacated while the hand was still live.
///
/// # The `principal` is the OWNER, and the owner is what gets paid
///
/// A seat is a chair; a stake belongs to a person. Resolving a payout's owner
/// from the seat instead of from the stake is
/// [FINDING 13](../../../docs/SECURITY-FINDINGS.md): a departed player's refund
/// was credited to whoever had since taken their chair, which conserves every
/// chip and so was invisible to every conservation invariant. Nothing on the
/// payout path may re-derive an owner from a seat index; it reads this field, via
/// [`Stake`].
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

    // ------------------------------------------------------------------
    // CUSTODY, from the caller's point of view (FINDING 18)
    // ------------------------------------------------------------------
    /// The CALLER's own money in this pot: their seat's stake plus any stake
    /// recorded for a seat of theirs already vacated in this hand.
    ///
    /// Carried on the view, and not only on [`get_custody_status`], because the
    /// view is the one call every client already makes on a loop. A figure that
    /// has to be fetched separately is a figure a client can forget, and
    /// "the client forgot" is indistinguishable from FINDING 18 to the player.
    ///
    /// **Non-zero with `my_seat = null` is the whole finding**: money of yours in
    /// a hand you are no longer sitting in.
    pub my_committed_in_pot: u64,
    /// True when this hand can no longer be moved by any message, so nobody can
    /// win the pot and `abandon_stuck_hand()` refunds every stake on request.
    pub hand_is_unmovable: bool,
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
/// Called from [`advance_table_clock`] -- so from the `check_timeouts` update AND
/// from the on-chain clock -- to run at most once per CLEANUP_INTERVAL_NS.
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

// ---------------------------------------------------------------------------
// Whether the archive is actually receiving anything
// ---------------------------------------------------------------------------
//
// `record_hand_to_history` was fire-and-forget with an `ic_cdk::println!` on
// every failure path, and canister stdout is not a thing a player can read. An
// independent auditor found the consequence: the history canister was deployed,
// authorised for NO tables and holding ZERO records, every `record_hand` call
// had been rejected as unauthorised, and nothing anywhere said so
// (docs/DEFECTS.md T-34). The table looked healthy the whole time.
//
// So: count both outcomes, keep the last error, and keep the records that did
// not land so they can be sent again once the wiring is fixed. All of it is
// readable with one query, `get_history_status`, by anybody.

/// How many un-archived hands the table will hold onto for a later retry.
/// Bounded because this lives in the heap of a canister that holds funds.
const MAX_UNRECORDED_HANDS: usize = 64;

/// How many backlog records one `flush_unrecorded_hands` call will send.
const MAX_FLUSH_BATCH: usize = 16;

/// Minimum gap between flushes, table-wide.
///
/// `flush_unrecorded_hands` is open to any player on purpose, and each call can
/// fan out to MAX_FLUSH_BATCH inter-canister calls. With the archive down, a
/// failed batch goes straight back into the backlog, so without a cooldown a
/// caller could loop flush -> fail -> re-buffer -> flush and burn the cycles of
/// a canister that custodies funds. The per-caller rate limiter does not bound
/// this, because the cost is per CALL and the limiter is per caller. Ten
/// seconds is nothing to somebody who wants their proof archived and everything
/// to somebody trying to spend the table's cycles.
const FLUSH_COOLDOWN_NS: u64 = 10_000_000_000;

thread_local! {
    // Deliberately NOT in PersistentState: these describe the current running
    // instance, and `get_history_status` says so in its own field names.
    static HISTORY_RECORDED_OK: RefCell<u64> = RefCell::new(0);
    static HISTORY_FAILED: RefCell<u64> = RefCell::new(0);
    static HISTORY_LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
    static HISTORY_LAST_RECORDED_HAND: RefCell<Option<u64>> = RefCell::new(None);
    static HISTORY_IN_FLIGHT: RefCell<u64> = RefCell::new(0);
    /// Hands the archive has not acknowledged. Retryable via `flush_unrecorded_hands`.
    static UNRECORDED_HANDS: RefCell<Vec<HandHistoryRecord>> = RefCell::new(Vec::new());
    /// Hands dropped from the backlog because it was full. Never silent.
    static UNRECORDED_DROPPED: RefCell<u64> = RefCell::new(0);
    /// When the backlog was last flushed, for FLUSH_COOLDOWN_NS.
    static LAST_FLUSH_AT: RefCell<u64> = RefCell::new(0);
}

/// Everything a player or a gate needs to know about whether the fairness
/// record for this table is actually being archived anywhere durable.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HistoryStatus {
    /// The archive this table sends to. `null` means NOTHING IS BEING ARCHIVED
    /// and every proof on this table is subject to the local cap below.
    pub history_canister: Option<Principal>,
    /// Hands the archive acknowledged since this instance last started.
    pub recorded_ok_since_start: u64,
    /// Hands the archive refused, or that failed to reach it, since start.
    pub failed_since_start: u64,
    /// Calls sent and not yet answered. A zero backlog with a non-zero value
    /// here does not yet mean the records landed.
    pub in_flight: u64,
    /// Hands held for retry. Non-zero means the archive is behind.
    pub unrecorded_backlog: u64,
    /// Hands dropped because the backlog hit its cap. These are gone from here.
    pub unrecorded_dropped: u64,
    /// The hand number of the last acknowledged record.
    pub last_recorded_hand: Option<u64>,
    /// Verbatim last failure, so a wiring mistake reads as a wiring mistake.
    pub last_error: Option<String>,
    /// How many hands this table keeps locally before pruning the oldest.
    pub local_history_cap: u64,
    /// How many hands are in the local ring right now.
    pub local_history_len: u64,
}

/// The retention rule, in machine-readable form, so the sentence in the UI and
/// the sentence in the README can be checked against the canister rather than
/// believed. A player is entitled to know how long the evidence lasts and who
/// can destroy it BEFORE they sit down.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct FairnessRetention {
    /// Hands kept in this table canister. The oldest is pruned past this.
    pub table_keeps_last_n_hands: u64,
    /// True: the table's own copy is erased by the controller-only
    /// `reset_table` / `admin_reinit_table`, and pruned by `periodic_cleanup`.
    pub table_copy_is_destructible_by_controller: bool,
    /// Where the durable copy goes. `null` means there is no durable copy.
    pub archive_canister: Option<Principal>,
    /// Plain English, deliberately unflattering where the truth is unflattering.
    pub summary: String,
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

/// Is the fairness record for this table reaching a durable archive?
///
/// Deliberately a public query with no auth: the person who needs this answer is
/// the player, and the answer they need is sometimes "no".
#[ic_cdk::query]
fn get_history_status() -> HistoryStatus {
    HistoryStatus {
        history_canister: HISTORY_ID.with(|h| *h.borrow()),
        recorded_ok_since_start: HISTORY_RECORDED_OK.with(|c| *c.borrow()),
        failed_since_start: HISTORY_FAILED.with(|c| *c.borrow()),
        in_flight: HISTORY_IN_FLIGHT.with(|c| *c.borrow()),
        unrecorded_backlog: UNRECORDED_HANDS.with(|b| b.borrow().len() as u64),
        unrecorded_dropped: UNRECORDED_DROPPED.with(|c| *c.borrow()),
        last_recorded_hand: HISTORY_LAST_RECORDED_HAND.with(|h| *h.borrow()),
        last_error: HISTORY_LAST_ERROR.with(|e| e.borrow().clone()),
        local_history_cap: MAX_HAND_HISTORY_ENTRIES as u64,
        local_history_len: HAND_HISTORY.with(|h| h.borrow().len() as u64),
    }
}

/// How long a proof survives, and who can destroy it.
#[ic_cdk::query]
fn get_fairness_retention() -> FairnessRetention {
    let archive = HISTORY_ID.with(|h| *h.borrow());
    let summary = match archive {
        Some(id) => format!(
            "This table keeps the last {n} hands. Older ones are pruned by periodic_cleanup, and \
             a controller of THIS canister can erase all of them at once with reset_table. Every \
             settled hand is also written to the archive canister {id}, which has no method that \
             deletes or edits a record: once a hand is acknowledged there it is permanent for the \
             life of that canister. What can still destroy it is a controller of the ARCHIVE \
             canister reinstalling or deleting the canister itself, which no application code can \
             prevent. So: a proof survives locally for {n} hands, and in the archive until \
             somebody with the archive's controller key takes it away. Check get_history_status \
             on this table to see whether archiving is actually working right now.",
            n = MAX_HAND_HISTORY_ENTRIES,
            id = id,
        ),
        None => format!(
            "NO ARCHIVE IS CONFIGURED ON THIS TABLE. The only copy of every shuffle proof is the \
             last {n} hands held in this canister. Older hands are already gone, and a controller \
             of this canister can erase the rest at once with reset_table. Do not rely on being \
             able to re-check a hand later: copy the seed hash and the revealed seed yourself \
             while the hand is on your screen.",
            n = MAX_HAND_HISTORY_ENTRIES,
        ),
    };
    FairnessRetention {
        table_keeps_last_n_hands: MAX_HAND_HISTORY_ENTRIES as u64,
        table_copy_is_destructible_by_controller: true,
        archive_canister: archive,
        summary,
    }
}

/// Re-send every hand the archive has not acknowledged.
///
/// Callable by anyone who is not anonymous, on purpose. The backlog exists
/// because the archive was unreachable or misconfigured, and the party with the
/// strongest interest in the proof surviving is the player, not the operator.
/// Rate-limited on the shared player limiter, and bounded per call.
#[ic_cdk::update]
fn flush_unrecorded_hands() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot flush the history backlog".to_string());
    }
    check_rate_limit()?;

    let history_id = HISTORY_ID.with(|h| *h.borrow()).ok_or_else(|| {
        "No history canister is configured on this table, so there is nowhere to flush to. \
         A controller must call set_history_canister first."
            .to_string()
    })?;

    // Nothing to send costs nothing to send, so an empty backlog does not claim
    // the cooldown. Only a call that actually fans out is throttled.
    if UNRECORDED_HANDS.with(|b| b.borrow().is_empty()) {
        return Ok(0);
    }

    // Table-wide cooldown, checked and claimed before anything is drained.
    let now = ic_cdk::api::time();
    LAST_FLUSH_AT.with(|l| {
        let last = *l.borrow();
        if last != 0 && now < last + FLUSH_COOLDOWN_NS {
            return Err(format!(
                "A flush ran {}s ago. This table accepts one every {}s so a failing archive \
                 cannot be turned into a way of burning its cycles. The backlog is not lost: \
                 read get_history_status and try again.",
                (now - last) / 1_000_000_000,
                FLUSH_COOLDOWN_NS / 1_000_000_000,
            ));
        }
        *l.borrow_mut() = now;
        Ok(())
    })?;

    let batch: Vec<HandHistoryRecord> = UNRECORDED_HANDS.with(|b| {
        let mut backlog = b.borrow_mut();
        let take = backlog.len().min(MAX_FLUSH_BATCH);
        backlog.drain(0..take).collect()
    });

    let sent = batch.len() as u64;
    for record in batch {
        dispatch_hand_to_history(history_id, record);
    }
    Ok(sent)
}

/// This canister's own principal, or a placeholder off-chain.
///
/// `ic_cdk::api::canister_self()` TRAPS outside a canister ("canister_self_size
/// should only be called inside canisters"), and `payout_tests` drives real
/// settlement in-process on the host. Before this pass `record_hand_to_history`
/// returned early when no archive was configured, which is why the host never
/// reached it; now every settled hand builds a record so an unwired table can
/// still buffer it, so the host reaches it every time. Off-wasm there IS no
/// canister identity, and the only place the placeholder can appear is a host
/// test's in-memory backlog, which is never sent anywhere.
fn self_principal() -> Principal {
    #[cfg(target_arch = "wasm32")]
    {
        ic_cdk::api::canister_self()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Principal::anonymous()
    }
}

/// Remember an acknowledged record.
fn note_history_success(hand_number: u64) {
    HISTORY_RECORDED_OK.with(|c| *c.borrow_mut() += 1);
    HISTORY_LAST_RECORDED_HAND.with(|h| *h.borrow_mut() = Some(hand_number));
}

/// Remember a failure, keep the record, and never lose the reason.
fn note_history_failure(message: String, record: HandHistoryRecord) {
    ic_cdk::println!("HISTORY: {}", message);
    HISTORY_FAILED.with(|c| *c.borrow_mut() += 1);
    HISTORY_LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
    UNRECORDED_HANDS.with(|b| {
        let mut backlog = b.borrow_mut();
        // Drop the OLDEST when full: a full backlog means the archive has been
        // broken for a while, and the recent hands are the ones somebody is
        // still able to check against their own screen.
        while backlog.len() >= MAX_UNRECORDED_HANDS {
            backlog.remove(0);
            UNRECORDED_DROPPED.with(|d| *d.borrow_mut() += 1);
        }
        backlog.push(record);
    });
}

/// Send one record and account for the outcome. Never traps, never blocks the
/// hand: a hand that has settled has settled whether or not the archive is up.
fn dispatch_hand_to_history(history_id: Principal, record: HandHistoryRecord) {
    HISTORY_IN_FLIGHT.with(|c| *c.borrow_mut() += 1);
    ic_cdk::futures::spawn(async move {
        let hand_number = record.hand_number;
        let retry_copy = record.clone();

        let call_result = ic_cdk::call::Call::unbounded_wait(history_id, "record_hand")
            .with_arg(record)
            .await;

        HISTORY_IN_FLIGHT.with(|c| {
            let mut n = c.borrow_mut();
            *n = n.saturating_sub(1);
        });

        match call_result {
            // `Response::candid::<R>()` is `decode_one`, NOT a tuple decode. The
            // original wiring asked it for `(Result<u64, String>,)`, i.e. a
            // one-field RECORD wrapping the variant, which the archive never
            // sends. So every write, including the ones the archive accepted and
            // stored, came back "Failed to decode history response" -- the table
            // could not observe its own successes. It was invisible because the
            // only report was an `ic_cdk::println!`. Measured on the running
            // replica: 3 hands played, `get_table_hand_count` on the archive said
            // 3, and the table said 0 recorded / 3 failed.
            Ok(response) => match response.candid::<Result<u64, String>>() {
                Ok(Ok(_archive_id)) => note_history_success(hand_number),
                Ok(Err(e)) => note_history_failure(
                    format!(
                        "hand {hand_number}: archive {history_id} REFUSED the record: {e}. \
                         Held for retry."
                    ),
                    retry_copy,
                ),
                Err(e) => note_history_failure(
                    format!(
                        "hand {hand_number}: archive {history_id} answered in a shape this table \
                         cannot decode ({e:?}). The two canisters disagree about record_hand's \
                         interface. Held for retry."
                    ),
                    retry_copy,
                ),
            },
            Err(e) => note_history_failure(
                format!(
                    "hand {hand_number}: the call to archive {history_id} failed ({e:?}). \
                     Held for retry."
                ),
                retry_copy,
            ),
        }
    });
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
    // The record is built whether or not an archive is configured. An unwired
    // table used to return here and leave no trace at all, which is how a table
    // that had archived nothing for its whole life still looked healthy
    // (docs/DEFECTS.md T-34). Now the hand goes into the retry backlog and
    // `get_history_status` says why, so wiring the archive later recovers it.
    let history_id = HISTORY_ID.with(|h| *h.borrow());
    let table_id = self_principal();

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
        None => {
            // A settled hand always has a proof, because `start_new_hand` sets
            // one before it deals. If that ever stops being true the hand is
            // unarchivable and the player must be told, not left with a silent
            // gap in the record. Counted, with the reason kept.
            HISTORY_FAILED.with(|c| *c.borrow_mut() += 1);
            HISTORY_LAST_ERROR.with(|e| {
                *e.borrow_mut() = Some(format!(
                    "hand {}: settled with NO shuffle proof, so there is nothing to archive and \
                     this hand cannot be checked by anyone. This should be impossible; please \
                     report it.",
                    state.hand_number
                ));
            });
            ic_cdk::println!(
                "HISTORY: hand {} settled with no shuffle proof",
                state.hand_number
            );
            return;
        }
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
                    // `try_`, not `evaluate_hand`. This is a HISTORY FIELD. It ran
                    // last on the settlement path and it could trap the whole
                    // settlement -- rolling back a completed payout, leaving the
                    // pot unpaid and the table unmovable -- to avoid writing one
                    // `null` into an archive record. A hand the evaluator cannot
                    // describe is recorded without a description.
                    // docs/SECURITY-FINDINGS.md FINDING 15.
                    final_hand_rank: if show_cards {
                        p.hole_cards.as_ref().and_then(|cards| {
                            poker_core::try_evaluate_hand(cards, &state.community_cards).ok()
                        })
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

    // Async, because a settled hand must not wait on an archive. NOT
    // fire-and-forget: every outcome is counted, and a record the archive did
    // not acknowledge is kept for `flush_unrecorded_hands` rather than lost to
    // a `println!` nobody can read. See `get_history_status`.
    match history_id {
        Some(id) => dispatch_hand_to_history(id, record),
        None => note_history_failure(
            format!(
                "hand {}: no archive canister is configured on this table, so this shuffle proof \
                 exists ONLY in the local ring of {} hands. Held for retry; a controller must \
                 call set_history_canister, then anyone may call flush_unrecorded_hands.",
                record.hand_number, MAX_HAND_HISTORY_ENTRIES
            ),
            record,
        ),
    }
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
    //
    // ONLY WHILE THE HAND CAN ACTUALLY PROGRESS. docs/SECURITY-FINDINGS.md
    // FINDING 15: this refusal is the second half of the lock. It is a fair rule
    // for a hand in play -- your chips are committed and your escrow is the
    // collateral behind them -- and it is not a rule at all once the hand has
    // stopped moving, at which point it is just the last door closing. A stuck
    // hand does not gate a withdrawal, because a withdrawal cannot touch the pot:
    // it pays out of escrow, and the stake in the pot stays in the pot.
    let in_hand = TABLE.with(|t| {
        let table = t.borrow();
        if let Some(state) = table.as_ref() {
            if hand_is_stuck(state, now) {
                return false;
            }
            if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
                // IN THE CURRENT HAND, asked of the one predicate. `!has_folded`
                // alone also refused a seat that holds NO CARDS -- a mid-hand
                // arrival -- which is not in the hand and has nothing committed to
                // it. See the "WHO IS IN THE HAND" section.
                return state.players.iter().flatten()
                    .any(|p| p.principal == caller && is_in_hand(p));
            }
        }
        false
    });

    // WHAT THIS CALLER STILL HAS IN THE MIDDLE, and whether anybody can win it.
    //
    // docs/SECURITY-FINDINGS.md FINDING 18. Every refusal below used to describe
    // the escrow balance and nothing else, so the one player in the world who
    // needed to hear about a stake in a stuck pot -- the one asking for money that
    // is not in their escrow because it is in that pot -- was told "Insufficient
    // balance. Have: 0.0000 ICP" and left. Read once, here, so both refusals speak
    // with one voice.
    let (committed, committed_is_stuck, committed_hand) = TABLE.with(|t| {
        let table = t.borrow();
        match table.as_ref() {
            Some(state) => (
                committed_stake_of(state, caller),
                hand_is_stuck(state, now),
                state.hand_number,
            ),
            None => (0, false, 0),
        }
    });
    let committed_note = committed_stake_sentence(committed, committed_is_stuck, committed_hand);

    if in_hand {
        return Err(if committed_note.is_empty() {
            "Cannot withdraw while in a hand".to_string()
        } else {
            format!("Cannot withdraw while in a hand. {committed_note}")
        });
    }

    // ATOMIC: Check balance, deduct, AND mark pending in single critical section
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current_balance = balances.get(&caller).copied().unwrap_or(0);

        if amount > current_balance {
            let currency = get_table_currency();
            let base = format!(
                "Insufficient balance. Have: {}, requested: {}",
                currency.format_amount(current_balance),
                currency.format_amount(amount)
            );
            // THE REFUSAL THAT HAS TO NAME THE RECOVERY. This is the exact reply
            // the auditor got while 2.98 ICP of theirs sat in a pot nobody could
            // win, and it is the last thing the canister ever said to them.
            return Err(if committed_note.is_empty() {
                base
            } else {
                format!("{base}. {committed_note}")
            });
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

/// Your WITHDRAWABLE escrow balance, and nothing else.
///
/// # This number is not everything the canister is holding for you
///
/// It excludes the chips in front of you at the table, and it excludes any stake
/// of yours still sitting in a live pot -- including a pot you have already left,
/// which is the case that cost an auditor 2.98 ICP of confidence
/// (docs/SECURITY-FINDINGS.md FINDING 18: `cash_out -> Ok = 0`,
/// `get_balance() -> 0`, and 298,000,000 e8s of theirs in a hand nobody could
/// move). The number was right. It was not the answer to the question being
/// asked.
///
/// Widening this reply is not possible without breaking every client that
/// decodes a `nat64`, so the whole answer lives at [`get_custody_status`], which
/// returns this figure plus the two it omits, and `withdraw`'s refusals now carry
/// the same sentence. **A client showing a balance should call
/// `get_custody_status`.**
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
                .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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

/// Cash out and leave the table.
///
/// # The number this returns is the WHOLE of what you are leaving with
///
/// docs/SECURITY-FINDINGS.md FINDING 18. It was not, once: cashing out of a hand
/// that no message could move returned `Ok = 0` while 2.98 ICP of the caller's was
/// in the pot, and no surface anywhere said so. Rather than annotate the reply --
/// which a `nat64` cannot carry, and which would still leave the money in a pot
/// nobody could win -- this now SETTLES the unmovable hand before it vacates the
/// seat, so the stake comes back into the stack and leaves with its owner. See
/// [`settle_unmovable_hand`] for why that is not a new power and cannot shut this
/// door.
#[ic_cdk::update]
fn cash_out() -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();

    // A hand nobody can move is a hand nobody can win. Settle it FIRST, so what
    // follows is an ordinary cash-out from a finished hand and the stake this
    // caller has in the middle goes home with them instead of staying behind
    // as an invisible claim on an empty table.
    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        if let Some(state) = table.as_mut() {
            if hand_is_stuck(state, now) {
                settle_unmovable_hand(state, now, UnmovableReason::AnExitDoorFoundItUnmovable);
            }
        }
    });

    // Check if player is in a hand
    //
    // ONLY WHILE THE HAND CAN ACTUALLY PROGRESS -- see the note on the same guard
    // in `withdraw`, and docs/SECURITY-FINDINGS.md FINDING 15.
    //
    // This comment used to end: "Cashing out of a stuck hand does not take the
    // stake with it: `record_departed_stake` below keeps every chip this player
    // has already put in inside the payout basis, so the only thing that leaves is
    // the stack behind, which was never contested." Every clause of that was true
    // and the conclusion a reader drew from it -- that the player was therefore
    // fine -- was not: the stake stayed in a pot NOBODY COULD WIN, on a table they
    // had just left, and no surface said so. That is FINDING 18, and the settle
    // above is the answer. **The stake now leaves with its owner.** The
    // `hand_is_stuck` arm below is reached only when the settle declined a plan
    // that does not conserve, and then the departed-stake record is what keeps the
    // money in the basis.
    let in_hand = TABLE.with(|t| {
        let table = t.borrow();
        if let Some(state) = table.as_ref() {
            if hand_is_stuck(state, now) {
                return false;
            }
            if state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete {
                // The same predicate `withdraw` uses, and the same one the rest
                // of the engine uses: see the "WHO IS IN THE HAND" section.
                return state.players.iter().flatten()
                    .any(|p| p.principal == caller && is_in_hand(p));
            }
        }
        false
    });

    if in_hand {
        return Err("Cannot cash out while in a hand".to_string());
    }

    // Find player and get their chips
    let result = TABLE.with(|t| {
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

        // THE LAST PLAYER OUT TURNS THE LIGHTS OFF (docs/SECURITY-FINDINGS.md
        // FINDING 18, the second shape). Wave 6 measured the alternative: both
        // players cashed out of a stuck hand and the canister was left holding a
        // live pre-flop hand with a 3,000,000 e8 pot and ZERO seated players,
        // telling nobody. Nobody holds cards, so nobody can win it -- there is
        // exactly one lawful ending and it costs one call to reach.
        if table_is_empty(state) {
            settle_unmovable_hand(state, now, UnmovableReason::NobodyLeftToWinIt);
        }

        Ok::<u64, String>(chips)
    });

    // RE-AIM THE ON-CHAIN CLOCK, in the convention `player_action` and
    // `use_time_bank` set: an entry point that changes the next deadline says so,
    // or the one-shot wake stays pointed at a clock that no longer exists. Both
    // things this function can now do change it -- settling an unmovable hand
    // retires its action clock and starts the auto-deal clock, and vacating the
    // last seat ends the hand outright.
    //
    // UNCONDITIONALLY, not `if result.is_ok()`: the settle at the top of this
    // function runs before the seat is looked up, so the deadline can move even on
    // the call that goes on to return "Not at table". Costs one comparison when
    // nothing moved (`schedule_next_wake` re-arms only on a changed deadline).
    schedule_next_wake();
    let chips = result?;

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
    // Nothing else in this canister evaluates a deadline on its own. See
    // "THE ON-CHAIN CLOCK" and docs/SECURITY-FINDINGS.md FINDING 19.
    start_clock();
}

/// Reset the table CONFIGURATION (controller only).
///
/// # This used to destroy every seated player's money (FINDING 07)
///
/// It was byte-identical to [`admin_reinit_table`] -- `require_controller`,
/// `validate_config`, `init_table_state` -- and `init_table_state` builds a brand
/// new [`TableState`] with empty seats. One call therefore erased every chip stack
/// and the whole pot: not returned to escrow, not withdrawable, not recoverable by
/// anybody including a controller, while the canister went on holding the ICP on
/// the ledger. Measured on the running local replica on 2026-08-05: two seats
/// holding 20 ICP each, `reset_table` -> `Ok`, chips 4_000_000_000 -> 0, ledger
/// balance unchanged, both players reading `get_balance = 0`, no restore path.
///
/// **There is no legitimate reason a CONFIG reset touches custody**, so this
/// function now refuses while the table holds any. The recovery door is
/// [`admin_return_all_chips_to_escrow`], which pays the money back to the people
/// who own it and is what the error message points at.
///
/// It does NOT require escrow to be empty: `BALANCES` is a separate map that this
/// path provably never writes (`money_safety::admin_custody::
/// reset_table_never_touches_escrow`), so an escrow balance is not at risk from a
/// config reset -- except through `currency`, which selects the LEDGER a
/// withdrawal is paid from. That one is guarded separately by
/// [`refuse_currency_change_while_funded`].
#[ic_cdk::update]
fn reset_table(config: TableConfig) -> Result<(), String> {
    require_controller()?;
    validate_config(&config)?;
    refuse_currency_change_while_funded(&config)?;
    refuse_while_table_holds_custody("reset_table")?;
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
/// Controller only - can only be done when no hand is in progress.
///
/// This one never replaced [`TableState`], so it never had FINDING 07's shape --
/// but `TableConfig::currency` is not a display setting, it is the LEDGER every
/// withdrawal is paid from, so this door could strand every balance without ever
/// writing to `BALANCES`. Guarded by [`refuse_currency_change_while_funded`],
/// exactly as the two reset doors are. See docs/SECURITY-FINDINGS.md FINDING 20.
///
/// `TABLE_CONFIG` is written alongside `state.config` because
/// [`get_table_currency`] reads the former and everything the player sees reads
/// the latter; leaving them to drift would mean the table charged blinds in one
/// currency and paid withdrawals out of another.
#[ic_cdk::update]
fn admin_update_config(new_config: TableConfig) -> Result<TableConfig, String> {
    require_controller()?;

    // Validate the new config
    validate_config(&new_config)?;
    refuse_currency_change_while_funded(&new_config)?;

    TABLE.with(|table| {
        let mut table = table.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // Only allow config updates when not in the middle of a hand
        match state.phase {
            GamePhase::WaitingForPlayers | GamePhase::HandComplete => {
                // Safe to update
                state.config = new_config.clone();
                TABLE_CONFIG.with(|c| {
                    *c.borrow_mut() = Some(new_config.clone());
                });
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

/// Admin: return every chip at the table to the escrow of the player who owns it,
/// and abandon the hand in progress. Controller only. **Conserving: it can move
/// money, it can never destroy or create it.**
///
/// This is the recovery primitive [`reset_table`] and [`admin_reinit_table`] point
/// at, and the thing FINDING 07 needed and did not have. Every seated stack goes
/// to that seat's own player, and every stake in the live hand goes to the player
/// who put it in -- `hand_stakes`, the same payout basis settlement uses, so a
/// stake left behind by a player who already walked away reaches THEM and not
/// whoever took their chair (FINDING 13). Escrow only ever goes UP.
///
/// It refuses -- changing nothing -- unless `state.pot` and the attributed stakes
/// agree to the e8. If they disagree there is money in the pot that no owner is
/// credited with, and paying out only the attributed part would destroy the
/// remainder, which is the defect this function exists to prevent. The escape from
/// that state is `abandon_stuck_hand`, which settles the hand from the
/// contributions themselves and is callable by anybody.
///
/// # Why this is not the `admin_restore_balance` mistake
///
/// `admin_restore_balance` was removed because it could MINT escrow from nothing:
/// a stolen controller key could invent a balance and withdraw real ICP. This
/// method cannot invent an e8. It reads what the table already holds, credits
/// exactly that to exactly those owners, and then zeroes what it credited; the
/// canister's total liability is unchanged to the e8 and its ledger balance is not
/// touched at all. The worst a stolen key does with it is end a hand early, which
/// that key can already do by upgrading the canister.
#[ic_cdk::update]
fn admin_return_all_chips_to_escrow() -> Result<u64, String> {
    require_controller()?;
    return_all_table_custody_to_escrow()
}

/// Admin: return every chip to its owner's escrow and THEN rebuild the table with
/// a fresh configuration. Controller only.
///
/// # This is the door the interface pointed at, and it was the destructive one
///
/// The Candid comment used to read *"for recovery after upgrade issues"*, which is
/// exactly the situation an honest operator reaches for it in, and the body was
/// byte-identical to [`reset_table`]: it replaced [`TableState`] wholesale and
/// every seated chip ceased to exist (FINDING 07). It now does what its name and
/// its documentation say: [`admin_return_all_chips_to_escrow`] FIRST, so every
/// player keeps a withdrawable claim on their own money, and only then a new table.
///
/// The difference from [`reset_table`] is deliberate and is the whole point of
/// there being two functions:
///
/// * `reset_table` is a CONFIG operation. It refuses while money is at the table.
/// * `admin_reinit_table` is a RECOVERY operation. It moves the money somewhere the
///   players can still reach it, and then resets.
///
/// Neither can destroy a chip, and `init_table_state` traps if a third door is ever
/// added that tries.
#[ic_cdk::update]
fn admin_reinit_table(config: TableConfig) -> Result<(), String> {
    require_controller()?;

    // Validate config BEFORE anything moves: a rejected config must leave the
    // table exactly as it was, hand included.
    validate_config(&config)?;
    refuse_currency_change_while_funded(&config)?;

    // Custody first, structure second. If this returns Err nothing has moved and
    // the table is untouched.
    let returned = return_all_table_custody_to_escrow()?;
    if returned > 0 {
        ic_cdk::println!(
            "ADMIN REINIT: {} e8s returned to the escrow of the players who owned it before the \
             table was re-initialised. No chip was destroyed. See docs/SECURITY-FINDINGS.md \
             FINDING 07.",
            returned
        );
    }

    // Initialize the table. `init_table_state` traps if any custody is still here.
    init_table_state(config);

    Ok(())
}

// ============================================================================
// ADMIN CUSTODY SAFETY  (docs/SECURITY-FINDINGS.md FINDING 07)
// ============================================================================
//
// THE RULE THIS SECTION ENFORCES, IN WORDS
//   No controller-callable method may reduce what this canister owes players
//   without paying those players. Config is config; custody is custody; an admin
//   method may touch both only by moving money to an account its owner can still
//   withdraw from.
//
// The reason it is a section rather than two `if`s is that FINDING 07 survived
// four waves BECAUSE the destructive step was a shared helper (`init_table_state`)
// that two differently-named public methods called without either of them looking
// at what the table was holding. The guard therefore lives at the helper as well
// as at each door, so a third door added later cannot re-open it silently.

/// What the TABLE STRUCTURE holds in custody, and whose it is.
///
/// Deliberately NOT escrow. `BALANCES` is a separate map that survives everything
/// in this section; what is measured here is the money that lives inside
/// [`TableState`] -- seated stacks and the stakes in the live hand -- and would
/// cease to exist if that struct were replaced.
#[derive(Clone, Debug, Default)]
pub struct TableCustody {
    /// Sum of every seated `Player::chips`.
    pub chips: u64,
    /// Sum of every stake in the live hand, as `hand_stakes` attributes them.
    pub staked: u64,
    /// The redundant `state.pot` accumulator, for cross-checking `staked`.
    pub pot: u64,
    /// `(owner, amount)`, one entry per stack and one per stake. Sums to
    /// `chips + staked`. An owner can appear more than once.
    pub owed: Vec<(Principal, u64)>,
}

impl TableCustody {
    /// Everything a player would lose if `TableState` were replaced right now.
    pub fn total(&self) -> u64 {
        self.chips.saturating_add(self.staked)
    }

    /// Nothing at the table belongs to anybody: safe to rebuild.
    pub fn is_empty(&self) -> bool {
        self.total() == 0 && self.pot == 0
    }

    /// Every e8 the pot accumulator claims is attributed to a named owner. When
    /// this is false, returning the attributed part would destroy the difference.
    pub fn pot_is_fully_attributed(&self) -> bool {
        self.staked == self.pot
    }
}

/// Read [`TableCustody`] off a table.
///
/// # `total_bet_this_hand` IS STALE BETWEEN HANDS, and that nearly cost this fix
///
/// `start_new_hand` is the only place that clears `Player::total_bet_this_hand`,
/// so from the moment a hand settles until the next one is dealt every seat still
/// reports the bets it made in the hand that is already OVER, while `state.pot` is
/// back to zero because `finish_hand` emptied it. Reading `hand_stakes` there
/// counts the same money twice: once in the winner's restored stack, once as a
/// "stake" that no longer exists.
///
/// The first version of this function did exactly that. It was green against every
/// PocketIC test written for it -- because none of those tests had played a hand
/// before calling the admin path -- and it refused on the very first live table it
/// met, with `state.pot says 0 but the stakes ... sum to 2000000000`. The
/// conservation guard caught it, which is the guard doing its job; the counting
/// was still wrong.
///
/// So the stakes are read ONLY while a hand is actually live. When it is not,
/// custody is the stacks and nothing else, and a non-zero `pot` outside a hand is
/// an anomaly the callers refuse on rather than try to attribute.
pub fn table_custody(state: &TableState) -> TableCustody {
    let mut owed: Vec<(Principal, u64)> = Vec::new();
    let mut chips: u64 = 0;
    for p in state.players.iter().flatten() {
        if p.chips > 0 {
            chips = chips.saturating_add(p.chips);
            owed.push((p.principal, p.chips));
        }
    }

    let hand_is_live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;
    let mut staked: u64 = 0;
    if hand_is_live {
        // THE PAYOUT BASIS, not the seat vector: a stake belongs to the player who
        // put it in, even if that player has left and somebody else is in the chair.
        for stake in hand_stakes(state) {
            if stake.amount > 0 {
                staked = staked.saturating_add(stake.amount);
                owed.push((stake.owner, stake.amount));
            }
        }
    }

    TableCustody {
        chips,
        staked,
        pot: state.pot,
        owed,
    }
}

/// Total escrow across every player. Used only to decide whether a config change
/// is safe, never to move money.
fn escrow_total() -> u64 {
    BALANCES.with(|b| {
        b.borrow()
            .values()
            .fold(0u64, |acc, v| acc.saturating_add(*v))
    })
}

/// Everything this canister owes anybody: escrow + seated chips + the pot.
fn total_liability() -> u64 {
    let table = TABLE.with(|t| {
        t.borrow()
            .as_ref()
            .map(|s| {
                let c = table_custody(s);
                c.chips.saturating_add(c.pot.max(c.staked))
            })
            .unwrap_or(0)
    });
    escrow_total().saturating_add(table)
}

/// Refuse a config whose `currency` differs from the live one while the canister
/// still owes anybody anything.
///
/// `currency` is not cosmetic: `Currency::ledger_canister()` is what `withdraw`
/// and `transfer_tokens` pay out of, and `min_withdrawal` / `transfer_fee` /
/// `format_amount` all follow it. Flipping an ICP table to BTC while balances
/// exist points every withdrawal at the ckBTC ledger, where this canister holds
/// nothing, so every player's money becomes unreachable until somebody flips it
/// back -- and flipping a BTC table to ICP would let sat-denominated balances be
/// withdrawn as ICP e8s. Neither is a config change; both are custody changes
/// wearing a config change's clothes.
fn refuse_currency_change_while_funded(new_config: &TableConfig) -> Result<(), String> {
    let current = TABLE_CONFIG.with(|c| c.borrow().as_ref().map(|c| c.currency));
    let Some(current) = current else {
        return Ok(()); // First initialisation: there is nothing to strand.
    };
    if current == new_config.currency {
        return Ok(());
    }
    let owed = total_liability();
    if owed == 0 {
        return Ok(());
    }
    Err(format!(
        "Refusing to change this table's currency from {} to {} while it still owes players {}. \
         The currency selects the LEDGER every withdrawal is paid from, so changing it now would \
         point every player's withdrawal at a ledger this canister holds nothing on -- every \
         balance would become unpayable without a single one of them changing. Drain the table to \
         zero first (every player withdraws), then change the currency. \
         See docs/SECURITY-FINDINGS.md FINDING 20.",
        current.symbol(),
        new_config.currency.symbol(),
        current.format_amount(owed),
    ))
}

/// Refuse an operation that would replace [`TableState`] while it holds custody.
fn refuse_while_table_holds_custody(method: &str) -> Result<(), String> {
    let custody = TABLE.with(|t| t.borrow().as_ref().map(table_custody));
    let Some(custody) = custody else {
        return Ok(());
    };
    if custody.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Refusing: {method} rebuilds the table, and this table is holding {} in seated chips and \
         {} in the pot for real players. Rebuilding would delete that money -- it is not returned \
         to escrow, not withdrawable and not recoverable by anybody, including you. Call \
         admin_return_all_chips_to_escrow first (it pays every chip back to the player who owns \
         it, and it cannot create or destroy one), or use admin_reinit_table, which does that for \
         you and then resets. See docs/SECURITY-FINDINGS.md FINDING 07.",
        custody.chips, custody.pot
    ))
}

/// Move every chip at the table into the escrow of the player who owns it, then
/// empty the table of money. Returns the total moved.
///
/// Conservation is the post-condition, checked before anything is written: the
/// credits must sum to exactly what the table held. Nothing here can change the
/// canister's total liability, and nothing here touches the ledger.
fn return_all_table_custody_to_escrow() -> Result<u64, String> {
    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let Some(state) = table.as_mut() else {
            return Ok(0); // No table, no custody.
        };
        let custody = table_custody(state);
        if custody.is_empty() {
            return Ok(0);
        }
        if !custody.pot_is_fully_attributed() {
            return Err(format!(
                "Refusing to touch this table: at phase {} state.pot says {} but the stakes that \
                 can be attributed to an owner sum to {} (delta {}). Returning only the \
                 attributed part would DESTROY the difference, and crediting more than was \
                 collected would invent chips. Nothing has been moved. Call abandon_stuck_hand -- \
                 it is public, it needs no privilege, and it settles the hand from the \
                 contributions themselves -- then try again. \
                 See docs/SECURITY-FINDINGS.md FINDING 07.",
                phase_to_string(&state.phase),
                custody.pot,
                custody.staked,
                custody.pot as i128 - custody.staked as i128
            ));
        }

        // Sum with CHECKED arithmetic. `saturating_add` here would clamp a
        // wrapped stack and silently under-pay, which is the failure mode this
        // whole section exists to make impossible.
        let mut credited: u64 = 0;
        for (_, amount) in &custody.owed {
            credited = credited.checked_add(*amount).ok_or_else(|| {
                "Refusing: the amounts owed at this table cannot be added without overflowing \
                 u64, so at least one stack or stake has wrapped. Nothing has been moved."
                    .to_string()
            })?;
        }
        if credited != custody.total() {
            return Err(format!(
                "Refusing: the per-owner amounts sum to {credited} but the table holds {}. \
                 Nothing has been moved.",
                custody.total()
            ));
        }

        // Past this line nothing can fail, so the write is all-or-nothing.
        for (owner, amount) in &custody.owed {
            credit_escrow(*owner, *amount);
        }
        for player in state.players.iter_mut().flatten() {
            player.chips = 0;
            player.total_bet_this_hand = 0;
        }
        state.pot = 0;
        state.side_pots.clear();
        state.clear_departed_stakes();
        Ok(credited)
    })
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

/// Build a brand-new [`TableState`] over whatever is there.
///
/// # THE LAST LINE OF DEFENCE (docs/SECURITY-FINDINGS.md FINDING 07)
///
/// This function is where the money went. It is called by `init`, by
/// [`reset_table`] and by [`admin_reinit_table`], and for four waves it replaced
/// the table without any of its callers looking at what the table was holding, so
/// two separate public methods each destroyed 100% of a funded table's chips.
///
/// Guarding only the callers would leave the same trap set for the next door
/// somebody adds, which is precisely how this survived: `reset_table` and
/// `admin_reinit_table` were byte-identical because writing a second door was one
/// copy-paste. So the guard is HERE, where the destruction happens, and it TRAPS
/// rather than returns: a caller that reaches this point with money still at the
/// table has a bug, and a trap rolls the whole message back so not one e8 moves.
/// `init` cannot trip it (there is no table yet), and both reset doors clear or
/// refuse custody before they call it.
fn init_table_state(config: TableConfig) {
    // MUST be first: nothing below may run while the table owes anybody anything.
    let held = TABLE.with(|t| t.borrow().as_ref().map(table_custody));
    if let Some(custody) = held {
        if !custody.is_empty() {
            ic_cdk::trap(&format!(
                "refusing to re-initialise a table that is holding {} in seated chips and {} in \
                 the pot for real players: rebuilding TableState would delete that money with no \
                 way for anybody, including a controller, to get it back. Nothing has been \
                 changed. Return the chips first with admin_return_all_chips_to_escrow. \
                 See docs/SECURITY-FINDINGS.md FINDING 07.",
                custody.chips, custody.pot
            ));
        }
    }

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
            departed_stakes: None,
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
//
// ---------------------------------------------------------------------------
// `evaluate_hand` IS DELIBERATELY NOT IMPORTED (docs/SECURITY-FINDINGS.md FINDING 15)
// ---------------------------------------------------------------------------
//
// `poker_core::evaluate_hand` TRAPS on an impossible input. That is right for a
// pure library and wrong for every caller in this file, because a trap here is
// not a failed call, it is a locked table: the message rolls back, the state that
// caused the trap is still there, and the next call takes the identical path. On
// the module deployed to the local replica, one such trap closed `player_action`,
// `leave_table` and `check_timeouts` at once while `withdraw` and `cash_out` were
// refusing "while in a hand" -- about 420 ICP with no door open at all.
//
// So this canister calls `poker_core::try_evaluate_hand` at every site, always by
// its full path, and NEVER imports the trapping name. That makes the rule
// greppable rather than remembered:
//
//     grep -n '[^_]evaluate_hand(' src/table_canister/src/lib.rs   # must be empty
//
// which is exactly what
// `no_trapping_evaluator_is_reachable_from_an_update_entry_point` in
// `tests/money_safety/tests/fund_reachability.rs` asserts on every run. Adding
// `evaluate_hand` back to this import list is enough to fail it.
use poker_core::{create_deck, shuffle_deck, Contribution};

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
            .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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
            .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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
        state.clear_departed_stakes();
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
                if will_be_dealt_in(player) {
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

        // DEAL. This loop is the only writer of the fact `is_in_hand` reads, so it
        // is the only thing that decides who is in this hand. See
        // [`deals_in_this_hand`] for why it does NOT ask for chips: the blinds have
        // already been posted above and can have left a seat all-in at zero.
        for player in state.players.iter_mut().flatten() {
            if deals_in_this_hand(player) {
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

    // A new hand means a brand-new action clock. Aim the on-chain wake at it, so a
    // table that goes silent the instant the cards are dealt still resolves on its
    // own clock and not on the watchdog's grid.
    schedule_next_wake();

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

// ============================================================================
// WHO IS IN THE HAND
// ============================================================================
//
// docs/DEFECTS.md E-32 and E-06 were one mistake asked in four places.
//
// PARTICIPATION -- whose turn is it (`find_next_active_seat`), is anybody left to
// play for the pot (`count_active_players`), can anybody still put money in
// (`count_players_can_act`), does the street close (`is_betting_round_complete`)
// -- used to be decided by `status == PlayerStatus::Active`.
//
// ELIGIBILITY -- who may WIN the pot (`live_claims`) -- is decided by "seated,
// holds cards, has not folded", and says nothing about `status`.
//
// Those are not the same set. A seat that stops heartbeating for 30 seconds is
// marked `Disconnected` by `check_timeouts`; it then left the PARTICIPATION set
// and stayed in the ELIGIBILITY set. It was never asked to call, so the street
// closed without it, and it arrived at the showdown holding its chips AND its
// claim on every pot level it had already paid into. That is a cheat, it is
// entirely client-side -- stop sending heartbeats -- and it is worth about a big
// blind a hand in a game whose whole edge is a fraction of one.
//
// It cut the other way too, and that half hurt the honest player: with a
// `Disconnected` seat still holding cards, `count_active_players` could reach 1
// while two seats still had a live claim, so `end_hand_single_winner` gave the
// pot away without a showdown the other seat was entitled to.
//
// THE RULE. Poker offers a player facing a bet three options -- call, raise,
// fold -- and no fourth. A player who does not act on their hand does not get to
// keep a claim on the money: Robert's Rules of Poker (Ciaffone), the rule the
// wave-4 audit already recorded against E-32, is that a player called upon to act
// who fails to act has a folded hand. Every online room implements exactly this:
// your clock runs whether or not your client is still connected, and the hand is
// folded when it expires. The all-in-style "disconnect protection" that a few
// rooms offered in the early 2000s -- capping a dropped player at what they had
// already contributed while they kept the rest of their stack -- was withdrawn
// precisely because it could be triggered on purpose. That is the exact shape of
// what this engine was doing by accident.
//
// So participation is now asked of THE SAME PREDICATE as eligibility. Not "the
// same rule written twice" -- literally the same function: [`live_claims`] on the
// payout path calls [`is_in_hand`], so the two cannot drift apart by an edit to
// one of them. That closes docs/SECURITY-FINDINGS.md FINDING 17 / docs/DEFECTS.md
// E-36, which was the same mistake at a fifth site: `is_in_hand` used to accept a
// seat that was `Active` and held NO CARDS, `live_claims` did not, and the gap paid
// a fold-out winner nothing.
//
// # THE PREDICATE TABLE
//
// Five questions get asked about a seat in this file. THREE of them are the same
// question and are now one function; the other two are genuinely different and are
// named so a reader cannot mistake them for the first three. Nothing else in this
// file may hand-roll any of these five -- see `predicate_table` in
// `src/table_canister/tests/hand_membership.rs`, which fails if a call site
// reintroduces one inline.
//
//   name                  rule                                asks
//   --------------------  ----------------------------------  ------------------
//   is_in_hand            !folded && hole_cards.is_some()      in THIS hand: may
//                                                              win it, is counted
//                                                              for the fold-out,
//                                                              is owed a turn
//   can_still_act         is_in_hand && !is_all_in             still owed an
//                                                              ACTION this street
//   live_claims           is_in_hand, per seat, with the       who may be PAID
//                         cards attached
//   deals_in_this_hand    status == Active                     who is DEALT when
//                                                              a hand starts. The
//                                                              only writer of the
//                                                              fact is_in_hand
//                                                              reads. NO chips
//                                                              test: the blinds
//                                                              are posted first
//                                                              and can leave a
//                                                              seat all-in at 0,
//                                                              and that seat must
//                                                              still be dealt in
//   will_be_dealt_in      status == Active && chips > 0        BETWEEN hands: can
//                                                              a hand start, who
//                                                              gets the button and
//                                                              the blinds, is
//                                                              auto-deal due
//
// The last two say NOTHING about the hand in progress, and no call site may use
// them to decide who is in one. `status` is a between-hands intention and a display
// state; the hand in progress is decided by the cards that were dealt into it.
//
// Everything else here holds:
//
//   * a seat in the hand is never skipped. It is offered the action and its clock
//     runs, so a real disconnection gets the full `action_timeout_secs` (plus its
//     time bank) to come back -- and, if nobody has bet, costs it nothing at all.
//   * a seat that does not act is folded by `resolve_expired_action_timer`, which
//     is the ONE path out of a hand for a player who will not act. It forfeits
//     what it has in, exactly like any other fold.
//   * `PlayerStatus::Disconnected` no longer moves money. It is a display state
//     and the start of the auto-kick clock, nothing more.
//
// Tests: `tests/betting_rules.rs` sections 4 and 5.

/// Is this seat IN the hand -- so still able to win money from it?
///
/// **THE** predicate. Participation and eligibility are one question and this is
/// it: [`live_claims`], the payout path's eligibility list, is this function
/// applied per seat with the cards attached. There is no second definition to
/// drift from.
///
/// `hole_cards.is_some()` is what "was dealt into THIS hand" means, and it is the
/// whole rule. `status` says nothing here: a seat that stops heartbeating is marked
/// `Disconnected` and is still in the hand (E-32), and a seat made `Active` during a
/// live hand without being dealt in is NOT in it, whatever its status says.
///
/// # WHY THERE IS NO `|| status == Active` DISJUNCT
///
/// There used to be, and it cost a player the pot. docs/SECURITY-FINDINGS.md
/// FINDING 17 / docs/DEFECTS.md E-36: a cardless `Active` seat was counted by
/// [`count_active_players`], `count_active_players(state) == 1` is what calls
/// `end_hand_single_winner`, and the one seat it counted could be the cardless one.
/// `live_claims` was then EMPTY, [`plan_payouts`] took its no-claimant branch, and
/// every stake went back to its funder -- including the players who had FOLDED.
/// Measured: a 52,000,000 e8 pot, three seats, all three ending on exactly their
/// buy-in, the fold-out winner paid nothing. No trap, no `CRITICAL:` line,
/// conservation exact, M1 through M9 silent.
///
/// The comment that used to sit here said of the disjunct that *"including it
/// changes nothing; it holds no cards, so `live_claims` still refuses to pay it."*
/// That was true of the PAYOUT and false of everything upstream of it, and it is
/// the sentence that stopped anyone looking. **Do not write another one. The
/// statement that these two predicates agree is a TEST, not a comment:**
///
/// * `predicate_table` and `is_in_hand_and_live_claims_are_one_predicate` in
///   `src/table_canister/tests/hand_membership.rs` -- every reachable
///   `Player` shape, both predicates, no exceptions;
/// * `probe4_finding_17_the_foldout_winner_is_paid_the_pot` in
///   `tests/money_safety/tests/wave6_coherence.rs` -- the measured sequence, now
///   asserting that the winner IS paid;
/// * M10 OUTCOME in `tests/money_safety/src/invariants/outcome.rs`, evaluated on
///   every fuzz step: a live hand may never run on with one claimant or none, and a
///   refund-everyone settlement is a violation unless the hand genuinely never had
///   a claimant.
pub fn is_in_hand(p: &Player) -> bool {
    !p.has_folded && p.hole_cards.is_some()
}

/// Who gets cards when [`start_new_hand`] deals. The ONLY writer of the fact
/// [`is_in_hand`] reads.
///
/// Deliberately WITHOUT the `chips > 0` test that [`will_be_dealt_in`] carries:
/// `start_new_hand` sits out every broke seat, then posts antes and blinds, and
/// posting a blind can take a seat to exactly zero and mark it all-in. That seat is
/// in the hand and must be dealt into it. Asking `chips > 0` here would deal it out
/// of a pot it had just been forced to pay into, which is the FINDING 17 shape
/// again from the other end: money in, no claim.
fn deals_in_this_hand(p: &Player) -> bool {
    p.status == PlayerStatus::Active
}

/// Will this seat take part in the NEXT hand -- can a hand start, who gets the
/// button and the blinds, is auto-deal due?
///
/// A question about the seat BETWEEN hands, and it must never be used to decide
/// anything about a hand in progress; that is [`is_in_hand`]. It was written out
/// inline at ten call sites, which is how an eleventh could have quietly grown a
/// different rule.
fn will_be_dealt_in(p: &Player) -> bool {
    p.status == PlayerStatus::Active && p.chips > 0
}

/// Is this seat still owed an action -- in the hand, and with chips behind?
///
/// An all-in seat is IN the hand and can win, but can never be asked for more
/// money; that is the one legitimate way to be eligible for a pot without matching
/// every later bet, and the side-pot layering is what caps it.
pub fn can_still_act(p: &Player) -> bool {
    is_in_hand(p) && !p.is_all_in
}

/// Find the next seat that is owed an action (in the hand, not all-in).
///
/// Deliberately does NOT consult `status`: see the note above. A seat that has
/// stopped responding is still offered the action, and its clock -- not its
/// connection -- is what takes it out of the hand.
fn find_next_active_seat(state: &TableState, from_seat: u8) -> u8 {
    let num_seats = state.players.len();
    let mut seat = (from_seat as usize + 1) % num_seats;

    for _ in 0..num_seats {
        if let Some(ref player) = state.players[seat] {
            if can_still_act(player) {
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
            if will_be_dealt_in(player) {
                return seat as u8;
            }
        }
        seat = (seat + 1) % num_seats;
    }

    from_seat
}

/// How many seats still have a claim on this pot: the count [`live_claims`] will
/// be able to pay, and nothing else.
///
/// It is `live_claims(state).len()` computed without building the vector -- the
/// same predicate over the same seats -- and `predicate_table` in
/// `src/table_canister/tests/hand_membership.rs` asserts that equality on
/// every state it builds. `count_active_players(state) == 1` is what calls
/// `end_hand_single_winner`, so if this could ever count a seat `live_claims`
/// cannot pay, the engine would settle a fold-out with no claimant and refund every
/// stake, including the folders'. That is docs/SECURITY-FINDINGS.md FINDING 17 and
/// it is the reason the two are one predicate.
pub fn count_active_players(state: &TableState) -> usize {
    state.players.iter()
        .filter(|p| p.as_ref().map(is_in_hand).unwrap_or(false))
        .count()
}

/// How many seats can still put money in.
fn count_players_can_act(state: &TableState) -> usize {
    state.players.iter()
        .filter(|p| p.as_ref().map(can_still_act).unwrap_or(false))
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
                .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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
    let now = ic_cdk::api::time();

    let result = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        // The same first move as `cash_out`, for the same reason and with the same
        // effect: a hand no message can move is settled before the seat is
        // vacated, so this caller's stake leaves with them instead of becoming an
        // invisible claim on a table they are no longer at.
        // docs/SECURITY-FINDINGS.md FINDING 18.
        if hand_is_stuck(state, now) {
            settle_unmovable_hand(state, now, UnmovableReason::AnExitDoorFoundItUnmovable);
        }

        // Find the player's seat
        let seat = state.players.iter()
            .position(|p| p.as_ref().map(|p| p.principal == caller).unwrap_or(false))
            .ok_or("Not at table")?;

        let player = state.players[seat].as_ref().ok_or("Player not found")?;
        let hand_is_live = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;
        // ASKED OF THE ONE PREDICATE. This used to be `!player.has_folded &&
        // hand_is_live`, a third hand-rolled statement of "in the hand" that
        // accepted a seat holding no cards -- so vacating such a seat marked it
        // folded and re-ran the fold-out check on its behalf. Both are meaningless
        // for a seat that was never dealt in, and a predicate nobody can enumerate
        // is how FINDING 17 stayed invisible. See "WHO IS IN THE HAND".
        let was_in_hand = hand_is_live && is_in_hand(player);
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

        // Rebuild the displayed breakdown from the basis that now exists.
        //
        // PAIRED WITH `return_uncalled_bet` ABOVE, and it has to be: that call
        // REDUCES `state.pot`, and `state.side_pots` was built against the larger
        // figure. Every other `return_uncalled_bet` call site in this file is
        // immediately followed by `refresh_side_pots`; this one was not, and the
        // 9-seed money-safety fuzz found the drift on seed 5
        // (`M1b_POT_BREAKDOWN:side_pots_sum_to_pot`, 2,000,000 e8s). No money moved
        // wrongly -- `plan_payouts` rebuilds the layering from `hand_contributions`
        // and never reads `state.side_pots` -- but the side pots a player is SHOWN
        // no longer added up to the pot they were shown. See docs/DEFECTS.md E-39.
        //
        // Runs AFTER `record_departed_stake` so the departing seat's stake is in the
        // basis the layering is built from; before it, the rebuild would itself
        // orphan the stake, which is E-05 all over again.
        if hand_is_live {
            refresh_side_pots(state);
        }

        // Advance the game if this departure changed anything about it: the leaver
        // held a live claim, or the clock was pointing at their chair.
        //
        // The second disjunct is new and it is a safety net rather than a live path.
        // Narrowing `was_in_hand` to the one predicate means a cardless seat no
        // longer takes this branch, and a cardless seat should never be `action_on`
        // -- `find_next_active_seat` asks `can_still_act`, which now requires cards.
        // If one ever is, leaving without advancing would point the clock at an
        // empty chair, so the action is moved on anyway. Costs nothing when the
        // condition never holds.
        if was_in_hand || (hand_is_live && was_action_on) {
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

        // THE LAST PLAYER OUT TURNS THE LIGHTS OFF. Same rule as `cash_out`, same
        // reason: a hand with nobody in it has one lawful ending and the canister
        // must not sit on it. Runs after the advance above, so a departure that
        // leaves exactly one player still settles as a fold-out win and only a
        // departure that leaves NOBODY reaches this.
        // docs/SECURITY-FINDINGS.md FINDING 18.
        if table_is_empty(state) {
            settle_unmovable_hand(state, now, UnmovableReason::NobodyLeftToWinIt);
        }

        Ok::<u64, String>(chips)
    });

    // RE-AIM THE ON-CHAIN CLOCK. See the same call in `cash_out`: this function
    // can retire a clock (settling an unmovable hand, ending the hand by fold-out,
    // vacating the last seat) and can also set a new one when the action moves on
    // from the chair it just emptied.
    schedule_next_wake();
    let chips = result?;

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

    let out = TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;
        apply_player_action(state, caller, now, action)
    });
    // RE-AIM THE ON-CHAIN CLOCK. This is not only about precision: without it the
    // one-shot wake stays pointed at the clock this action just REPLACED, so it
    // fires early on every single betting action, finds nothing to do, and
    // re-arms -- an extra timer message per action, at ~15M cycles each. See
    // "THE ON-CHAIN CLOCK".
    schedule_next_wake();
    out
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

    // Reset acted flags after we're done with the player borrow.
    //
    // Exactly the seats `is_betting_round_complete` will interrogate, which is
    // `can_still_act` -- so this is asked of that predicate rather than of a
    // hand-rolled `!has_folded && !is_all_in`. Same set for every seat that was
    // dealt in; the difference is a cardless seat, which is not in the betting
    // round and whose flag means nothing. See "WHO IS IN THE HAND".
    if should_reset_acted {
        for (i, p_opt) in state.players.iter_mut().enumerate() {
            if let Some(ref mut p) = p_opt {
                if i != player_seat && can_still_act(p) {
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
            // ASKED OF THE ONE PREDICATE. `!is_all_in && !has_folded` is
            // `can_still_act` with the cards test missing, and the missing test
            // matters here: `big_blind_seat` can be a chair whose original occupant
            // LEFT and which a mid-hand arrival has taken. That newcomer holds no
            // cards and has not acted, so the hand-rolled version held the street
            // open for a seat that is not in the hand at all. See "WHO IS IN THE
            // HAND" and docs/SECURITY-FINDINGS.md FINDING 17.
            if let Some(ref bb_player) = state.players[state.big_blind_seat as usize] {
                if !bb_player.has_acted_this_round && can_still_act(bb_player) {
                    return false;
                }
            }
        }
    }

    // THE INVARIANT THIS FUNCTION IS. A street may not close while a seat that can
    // still win the pot has not acted, or has not matched the bet. Asking that of
    // `can_still_act` rather than of `status == Active` is what closes E-32: a seat
    // that has stopped responding is exactly a seat that has not acted, so the
    // round stays open, the action is offered to it, and its clock decides.
    for player in state.players.iter().flatten() {
        if can_still_act(player) {
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
                .departed_stakes()
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

/// One stake in the hand being settled, WITH THE PRINCIPAL IT BELONGS TO.
///
/// # Why the owner travels with the money (docs/SECURITY-FINDINGS.md FINDING 13)
///
/// `poker_core::Contribution` is keyed by seat, and correctly so: pot LAYERING is
/// a seat question -- who covered which bet level, who is eligible for which
/// layer. **Ownership is not a seat question.** A seat is a chair, and a chair can
/// be vacated mid-hand and taken by somebody else before the hand settles; the
/// money already in the pot still belongs to the player who put it there.
///
/// The first version of the payout fix re-derived a payout's owner from its seat
/// index (`principal_of(state, seat)`), so a departed player's refunded stake was
/// credited to whoever had since taken their chair. It conserved every chip, the
/// plan paid out exactly what it collected, and no conservation invariant, no
/// settlement-oracle per-seat diff and no `CRITICAL:` line could see it, because
/// the only thing wrong was WHO HAD THE MONEY.
///
/// So the owner is carried, never looked up. Every [`Payout`] this file builds
/// takes its principal from the `Stake` or the live claim that generated it, and
/// [`Payout::principal`] is not an `Option`: "a payout with no known owner" is
/// unrepresentable rather than trapped-on.
#[derive(Clone, Copy, Debug)]
pub struct Stake {
    /// Index into `TableState::players`. Two stakes CAN share a seat.
    pub seat: u8,
    /// The player whose chips these are. The only thing that may be paid.
    pub owner: Principal,
    pub amount: u64,
    /// True when this stake has given up its claim: it folded, or its player left
    /// the table. It is in the pot and counts towards the bet levels; it can never
    /// win a layer.
    pub relinquished: bool,
}

impl Stake {
    fn contribution(&self) -> Contribution {
        Contribution::new(self.seat, self.amount, self.relinquished)
    }
}

/// Every stake in the current hand, with its owner, whether or not its seat is
/// still occupied.
///
/// THE payout basis. `collect_contributions` alone reads the seat vector, so a seat
/// vacated mid-hand disappeared from it while its money stayed in the pot
/// (docs/DEFECTS.md E-05); the departed stakes recorded by `leave_table` /
/// `cash_out` are what closes that.
///
/// A departed stake is carried with `relinquished = true`: it is in the pot, it
/// counts towards the bet levels, and it can never win a layer.
pub fn hand_stakes(state: &TableState) -> Vec<Stake> {
    let mut out: Vec<Stake> = state
        .players
        .iter()
        .enumerate()
        .filter_map(|(seat, player)| {
            let p = player.as_ref()?;
            (p.total_bet_this_hand > 0).then_some(Stake {
                seat: seat as u8,
                owner: p.principal,
                amount: p.total_bet_this_hand,
                relinquished: p.has_folded,
            })
        })
        .collect();

    for stake in state
        .departed_stakes()
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
        // What this path must do about it is keep BOTH stakes, WITH THEIR OWNERS.
        // Dropping either is a destroyed chip; merging them is FINDING 13, one
        // player's money paid to another. The new occupant is not in `live_claims`
        // (no hole cards), so they cannot win the departed player's money or their
        // own; it is carried to a layer that can be settled. Reported as a WARNING
        // and not as `CRITICAL:` deliberately: nothing about the engine's accounting
        // is inconsistent here, and `CRITICAL:` is reserved in this file for accounts
        // that disagree, which the money-safety classifier treats as a failure that
        // stops the run.
        if out.iter().any(|c| c.seat == stake.seat) {
            ic_cdk::println!(
                "WARNING: seat {} carries both a live stake and a departed stake in hand {} \
                 (the chair was re-occupied mid-hand, docs/DEFECTS.md E-36). Both are in the \
                 payout basis, each with its own owner; the new occupant holds no cards and \
                 can win neither.",
                stake.seat,
                state.hand_number
            );
        }
        out.push(Stake {
            seat: stake.seat,
            owner: stake.principal,
            amount: stake.contributed,
            relinquished: true,
        });
    }
    // Stable, so two stakes at one seat keep live-then-departed order.
    out.sort_by_key(|c| c.seat);
    out
}

/// The payout basis as `poker_core` sees it: seat, amount, claim given up or not.
///
/// This is [`hand_stakes`] with the owners dropped, and it is ONLY correct to use
/// where ownership is irrelevant -- pot layering, the uncalled-bet rule, the
/// redundant `state.pot` cross-check. Anything that moves money must read
/// [`hand_stakes`] (docs/SECURITY-FINDINGS.md FINDING 13).
pub fn hand_contributions(state: &TableState) -> Vec<Contribution> {
    hand_stakes(state).iter().map(Stake::contribution).collect()
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
    let stakes = state.departed_stakes_mut();
    stakes.retain(|d| d.hand_number == hand_number);
    stakes.push(DepartedStake {
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
    /// Where the money came from. Used for the record and for deciding whether the
    /// credit can go into a stack rather than into escrow. **Never** used to work
    /// out who to pay.
    pub seat: u8,
    /// WHO IS PAID. Carried from the [`Stake`] or the live claim that generated
    /// this payout, never re-derived from `seat`.
    ///
    /// This used to be an `Option<Principal>` filled in by looking `seat` up in
    /// `state.players`, which paid a departed player's stake to whoever had taken
    /// their chair (docs/SECURITY-FINDINGS.md FINDING 13). Making it a plain
    /// `Principal` sourced from the stake is what makes that unrepresentable
    /// rather than merely unlikely.
    pub principal: Principal,
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

    /// What this plan pays a given PRINCIPAL, across every seat and every layer.
    ///
    /// The question [`amount_for`](Self::amount_for) cannot answer, and the one
    /// that matters: a seat is a chair, and two people can have money riding on
    /// one chair in a single hand.
    pub fn amount_for_principal(&self, who: Principal) -> u64 {
        self.payouts
            .iter()
            .filter(|p| p.principal == who)
            .fold(0u64, |a, p| a.saturating_add(p.amount))
    }
}

/// Every seat that still has a claim on the pot.
///
/// **This is [`is_in_hand`], per seat, with the cards attached** -- it calls it
/// rather than restating it, and that is load-bearing. The two used to state the
/// rule separately, the statements disagreed about one seat shape, and the
/// disagreement paid a fold-out winner nothing (docs/SECURITY-FINDINGS.md
/// FINDING 17). One function, one rule, no drift: whoever `count_active_players`
/// counts is exactly whoever this can pay, so the engine can never hand a pot to
/// one player while another still holds cards in it, and can never settle a
/// fold-out on a seat that cannot be paid.
/// `pub` only so `src/table_canister/tests/hand_membership.rs` can assert the
/// equality this whole section rests on. Not an update/query method, so it is not
/// part of the Candid surface.
pub fn live_claims(state: &TableState) -> Vec<(u8, Principal, (Card, Card))> {
    state
        .players
        .iter()
        .enumerate()
        .filter_map(|(seat, player)| {
            let p = player.as_ref()?;
            if !is_in_hand(p) {
                return None;
            }
            // `is_in_hand` has already established this is `Some`; the `?` is how
            // the cards are carried out, not a second, weaker test.
            Some((seat as u8, p.principal, p.hole_cards?))
        })
        .collect()
}

/// Rank the claims, IF there is a board to rank them against.
///
/// A hand that ends before the flop has no board and needs no ranking: everybody
/// else folded, so the last player standing takes the pot without showing. So a
/// short board is not an error here, it is the ordinary pre-flop fold-out, and it
/// returns an empty ranking.
///
/// # This call site is the one that locked a funded table
///
/// docs/SECURITY-FINDINGS.md FINDING 15. The module deployed on the local replica
/// (`0x5298915c…`) ranks unconditionally, with `poker_core::evaluate_hand`, which
/// TRAPS on a 0-card board. Reached from `end_hand_single_winner`, that trap took
/// out `player_action`, `leave_table` and `check_timeouts` in the same state,
/// while `withdraw` and `cash_out` were refusing with "Cannot withdraw while in a
/// hand" -- every door a player has, shut at once, over about 420 ICP. Verbatim
/// backtrace from that canister's own log:
///
/// ```text
/// poker_core::hand::evaluate_hand
/// table_canister::plan_payouts
/// table_canister::settle_hand
/// table_canister::end_hand_single_winner
/// canister_update leave_table
/// ```
///
/// Two things had to be true for that, and both are now false:
///
/// 1. the hand reached a fold-out settlement while TWO seats still held live
///    claims and there was no board to rank them by -- fixed at the cause, in the
///    "WHO IS IN THE HAND" section above: participation is now the same predicate
///    as eligibility, so a seat that has not folded is never counted out of the
///    hand by a missed heartbeat;
/// 2. the evaluator call on the settlement path could TRAP. It cannot now. A
///    rejected claim is DROPPED from the ranking, loudly, and the money it was
///    contesting is refunded to the seats that put it there by
///    [`plan_payouts`]'s carry rule. Refusing beats trapping here for one reason
///    only: a trap rolls the message back, so the state that caused it is still
///    there, and the next call takes the identical path. There is no such thing
///    as a recoverable trap on the settlement path.
fn rank_claims(
    state: &TableState,
    claim: &[(u8, Principal, (Card, Card))],
) -> Vec<(u8, HandRank, Principal, (Card, Card))> {
    if !(3..=5).contains(&state.community_cards.len()) {
        return Vec::new();
    }
    claim
        .iter()
        .filter_map(|(seat, principal, cards)| {
            match poker_core::try_evaluate_hand(cards, &state.community_cards) {
                Ok(rank) => Some((*seat, rank, *principal, *cards)),
                Err(e) => {
                    // A legal board length and still an impossible hand means a
                    // duplicate card: the deck and the deal disagree. Say so, in
                    // the CRITICAL: dialect the money-safety harness watches for,
                    // and settle the rest of the hand rather than freezing the
                    // table around one corrupt seat.
                    ic_cdk::println!(
                        "CRITICAL: hand {}: seat {} cannot be ranked and is dropped from the \
                         showdown ({}). The money it was contesting is refunded to the seats \
                         that put it in. See docs/SECURITY-FINDINGS.md FINDING 15.",
                        state.hand_number,
                        seat,
                        e
                    );
                    None
                }
            }
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
///
/// # Who each payout names (docs/SECURITY-FINDINGS.md FINDING 13)
///
/// Two sources, and no third:
///
/// * a **refund** names the owner of the [`Stake`] the money came from, so a
///   departed player's stake reaches that player even if the chair has since been
///   taken by somebody else, and two stakes at one seat owned by two different
///   players each reach their own owner;
/// * a **pot share** names the owner of the LIVE CLAIM that won the layer, which
///   by construction is the player sitting in that seat holding those cards.
///
/// Nothing here reads a principal out of `state.players` by seat index.
/// Give every stake back to the player who put it in, and award nothing.
///
/// The plan of last resort, and the reason a stuck hand can never hold anybody's
/// money hostage (docs/SECURITY-FINDINGS.md FINDING 15). It is:
///
/// * **always available.** It needs no board, no ranking, no evaluator and no
///   opinion about who was winning. There is no input it cannot produce a plan
///   for, which is exactly the property the settlement path was missing.
/// * **exactly conserving.** Every e8 the hand collected goes back to the seat
///   that funded it, so it satisfies [`PayoutPlan::conserves`] by construction and
///   `apply_payouts` will accept it.
/// * **ungameable.** You get your own money back, no more and no less, so there is
///   nothing to win by forcing it -- unlike "pay the deepest stack", which is what
///   this engine used to do with money it could not attribute (FINDING 02).
///
/// It is NOT a settlement. Nobody wins the hand; the hand is abandoned. It is only
/// ever reached from [`abandon_stuck_hand`], which requires the hand to have been
/// provably immovable first, and it says so in the log.
pub fn refund_every_stake(state: &TableState) -> PayoutPlan {
    let stakes = hand_stakes(state);
    let contributions: Vec<Contribution> = stakes.iter().map(Stake::contribution).collect();
    let collected = poker_core::total_contributed(&contributions);
    let side_pots = poker_core::build_side_pots_from_contributions(&contributions);
    let payouts: Vec<Payout> = stakes
        .iter()
        .filter(|s| s.amount > 0)
        .map(|s| Payout {
            seat: s.seat,
            // THE OWNER OF THE STAKE, not the occupant of the chair (FINDING 13).
            principal: s.owner,
            amount: s.amount,
            reason: PayoutReason::Refund { layer: 0 },
        })
        .collect();
    let awarded = payouts
        .iter()
        .fold(0u64, |a, p| a.saturating_add(p.amount));
    PayoutPlan {
        side_pots,
        payouts,
        ranked: Vec::new(),
        collected,
        awarded,
    }
}

pub fn plan_payouts(state: &TableState) -> PayoutPlan {
    let stakes = hand_stakes(state);
    let contributions: Vec<Contribution> = stakes.iter().map(Stake::contribution).collect();
    let collected = poker_core::total_contributed(&contributions);
    let side_pots = poker_core::build_side_pots_from_contributions(&contributions);

    let claim = live_claims(state);
    let ranked = rank_claims(state, &claim);
    let num_seats = state.players.len();
    let mut payouts: Vec<Payout> = Vec::new();

    // The owner of a winning layer. `winners` only ever contains seats that came
    // out of `claim`, so this always resolves; if it ever does not, the hand must
    // NOT settle -- a payout with a guessed owner is FINDING 13. Panicking here
    // traps the message, which rolls the whole settlement back and moves no chips.
    let winner_principal = |seat: u8| -> Principal {
        claim
            .iter()
            .find(|(s, _, _)| *s == seat)
            .map(|(_, p, _)| *p)
            .unwrap_or_else(|| {
                panic!(
                    "CRITICAL: refusing to settle hand {}: seat {} was awarded a pot layer but \
                     holds no live claim, so there is no owner to pay. Nothing has been \
                     credited. See docs/SECURITY-FINDINGS.md FINDING 13.",
                    state.hand_number, seat
                )
            })
    };

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
        for s in stakes.iter().filter(|s| s.amount > 0) {
            payouts.push(Payout {
                seat: s.seat,
                // THE OWNER OF THE STAKE, not the occupant of the chair.
                principal: s.owner,
                amount: s.amount,
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
                // The owner of the live claim that won the layer.
                principal: winner_principal(seat),
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
        let funders: Vec<&Stake> = stakes.iter().filter(|s| s.amount > 0).collect();
        for (i, s) in funders.iter().enumerate() {
            let share = if i + 1 == funders.len() {
                carry.saturating_sub(handed)
            } else {
                ((carry as u128 * s.amount as u128) / total.max(1) as u128) as u64
            };
            handed = handed.saturating_add(share);
            payouts.push(Payout {
                seat: s.seat,
                // THE OWNER OF THE STAKE, not the occupant of the chair.
                principal: s.owner,
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

// `principal_of(state, seat)` used to live here: it looked a payout's owner up in
// `state.players` by seat index and only fell back to `departed_stakes` when the
// chair was EMPTY. That is docs/SECURITY-FINDINGS.md FINDING 13 -- a departed
// player's refunded stake credited to whoever had taken their chair -- and it is
// deleted rather than fixed on purpose. There is now no function in this file that
// can turn a seat index into a payee, so the mistake cannot be made again by
// calling the wrong helper. Owners come from `Stake::owner` and from `live_claims`.

/// Collect ALL players who bet this hand (including folded) with their bets.
///
/// `pub` only so `tests/unit_tests.rs` can pin it; it is not an update/query
/// method, so it is not part of the Candid surface.
///
/// **NOT ON THE PAYOUT PATH ANY MORE.** [`hand_stakes`] builds the same seated
/// stakes in one pass so that each stake's OWNER comes from the same `Player` the
/// amount came from, instead of being looked up by seat afterwards
/// (docs/SECURITY-FINDINGS.md FINDING 13). The selection rule is identical -- a
/// seated player with `total_bet_this_hand > 0`, carrying `has_folded` -- and
/// `collect_contributions_and_hand_stakes_select_the_same_seated_stakes` in
/// `payout_tests` pins that the two never drift apart. This function is retained
/// as the small, directly unit-tested statement of that rule.
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
/// [`plan_payouts`]. Paying out a plan that does not add up is worse than any
/// alternative, so this stays.
///
/// # The old justification for the trap was FALSE, and it mattered
///
/// It used to read: *"trapping leaves the hand unsettled and the state untouched,
/// which is recoverable (players can still leave the table with their stacks and
/// the code can be fixed and the message retried)"*.
///
/// **Players cannot still leave the table.** `leave_table` reaches this same code,
/// so it traps too; so does `player_action`; so does `check_timeouts`; and
/// `withdraw` and `cash_out` refuse "while in a hand". A trap anywhere on the
/// settlement path closes every door a player has, simultaneously and permanently,
/// because a trap rolls the message back and the next call takes the identical
/// path. That is not a theory: it is what happened, over about 420 ICP, and it is
/// docs/SECURITY-FINDINGS.md FINDING 15.
///
/// The post-condition is kept and the argument for it is replaced. It is safe to
/// trap here **only because it is now unreachable from a plan that exists**:
/// [`plan_payouts`] refunds anything it cannot award rather than dropping it, and
/// [`refund_every_stake`] is conserving by construction. And it is safe to be
/// wrong about that, because [`abandon_stuck_hand`] does not run through any of
/// this and gives every player their stake back regardless.
///
/// Returns one aggregated [`Winner`] per credited (seat, PRINCIPAL) pair. A refund
/// of money nobody could win is included in that list: `Winner::amount` means
/// "chips credited to this player when the hand settled", which is what keeps
/// `sum(winners) == collected` -- the identity the money-safety suite checks as
/// M3 NO RAKE.
///
/// # Where the money goes (docs/SECURITY-FINDINGS.md FINDING 13)
///
/// `payout.principal` decides, and nothing else. If that player is sitting in the
/// payout's seat, the credit goes into their stack; otherwise it goes into their
/// ESCROW -- which is where a departing player's stack went when they left, so it
/// is the account they can withdraw from. The chair is never consulted for
/// identity, only for "can this be a stack credit".
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

        let owed = payout.principal;
        match seated {
            // The usual case: the player being paid is sitting in the payout's seat,
            // so the credit goes into their stack.
            Some(occupant) if occupant == owed => {
                if let Some(ref mut p) = state.players[payout.seat as usize] {
                    p.chips = p.chips.saturating_add(payout.amount);
                }
                push_winner(state, &mut winners, payout, owed);
            }
            // The payout's seat is occupied by somebody ELSE. Reached when a player
            // leaves mid-hand and another takes the chair before the hand settles
            // (docs/DEFECTS.md E-36), and the sole reason FINDING 13 was a defect
            // rather than an impossibility. The money belongs to `owed`, so it goes
            // to `owed`'s escrow -- exactly where their stack went when they left.
            //
            // Logged WITHOUT a `CRITICAL:`/`WARNING:` prefix on purpose: nothing
            // about the engine's accounting is inconsistent here, the engine is
            // paying the right person, and the money-safety classifier treats every
            // self-reported `CRITICAL:`/`WARNING:` line as a finding that stops the
            // run. The condition that produced the empty chair is already reported
            // by `hand_stakes`.
            Some(occupant) => {
                credit_escrow(owed, payout.amount);
                ic_cdk::println!(
                    "paid {} to the escrow of {} for seat {} in hand {}: that chair is now \
                     occupied by {}, and this stake belongs to {}",
                    payout.amount,
                    owed,
                    payout.seat,
                    state.hand_number,
                    occupant,
                    owed
                );
                push_winner(state, &mut winners, payout, owed);
            }
            // The seat is empty: a refund to a player who left. Their stack already
            // went back to escrow when they left, so this goes to the same place.
            None => {
                credit_escrow(owed, payout.amount);
                ic_cdk::println!(
                    "refunded {} to the escrow of {} (seat {} left hand {})",
                    payout.amount,
                    owed,
                    payout.seat,
                    state.hand_number
                );
                push_winner(state, &mut winners, payout, owed);
            }
        }
    }
    winners
}

/// Add `amount` to `who`'s escrow balance. The only way settlement pays a player
/// who is not sitting in the seat the money came from.
fn credit_escrow(who: Principal, amount: u64) {
    BALANCES.with(|b| {
        let mut balances = b.borrow_mut();
        let current = balances.get(&who).copied().unwrap_or(0);
        balances.insert(who, current.saturating_add(amount));
    });
}

/// Fold one payout into the aggregated winner list.
///
/// Aggregated by `(seat, principal)`, NOT by seat. One chair can carry two stakes
/// belonging to two different players in a single hand (docs/DEFECTS.md E-36), and
/// merging them into one `Winner` would report one player's money under the other
/// player's name -- the reporting face of docs/SECURITY-FINDINGS.md FINDING 13.
fn push_winner(
    state: &TableState,
    winners: &mut Vec<Winner>,
    payout: &Payout,
    principal: Principal,
) {
    if let Some(existing) = winners
        .iter_mut()
        .find(|w| w.seat == payout.seat && w.principal == principal)
    {
        existing.amount = existing.amount.saturating_add(payout.amount);
        return;
    }
    // The cards belong to the person in the chair, and this record belongs to
    // `principal`. If those are not the same player -- a departed stake refunded at
    // a chair somebody else has taken -- then there is no hand to attach, and
    // attaching the occupant's would publish one player's hole cards under another
    // player's name.
    let shown = state
        .players
        .get(payout.seat as usize)
        .and_then(|p| p.as_ref())
        .filter(|p| p.principal == principal)
        .and_then(|p| p.hole_cards);
    let rank = match payout.reason {
        // A refund is not a win, so it carries no hand.
        PayoutReason::Refund { .. } => None,
        // `try_`, not `evaluate_hand`: a hand this cannot rank costs the WINNER
        // RECORD its hand description and nothing else. Trapping here would undo
        // a settlement that has already decided who is owed what, and leave the
        // table wedged on a display field (FINDING 15).
        PayoutReason::PotShare { .. } => shown
            .filter(|_| state.community_cards.len() >= 3)
            .and_then(|cards| poker_core::try_evaluate_hand(&cards, &state.community_cards).ok()),
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
            // By PRINCIPAL, not by seat. One chair can carry money belonging to two
            // people in a single hand (docs/DEFECTS.md E-36), and `amount_for(seat)`
            // would report the departed player's refund as this player's winnings.
            // See docs/SECURITY-FINDINGS.md FINDING 13.
            amount_won: plan.amount_for_principal(*principal),
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
    state.clear_departed_stakes();
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
// FUND REACHABILITY -- THE HAND THAT CANNOT MOVE
// ============================================================================
//
// docs/SECURITY-FINDINGS.md FINDING 15, and the rule it exists to enforce:
//
//     NO STATE MAY MAKE A PLAYER'S FUNDS UNREACHABLE.
//
// Every other invariant in this project asks whether the arithmetic is right.
// None of them asks whether the player can still get the money out, and a table
// can satisfy all of them while being frozen with funded seats. That is the state
// an independent auditor reached in ordinary play, over about 420 ICP, and the
// reason it was reachable is that a player's four doors are not independent:
//
//   `check_timeouts`, `player_action`, `leave_table`   all run the settlement path
//   `withdraw`, `cash_out`                             both refuse "while in a hand"
//
// So ONE failure on the settlement path shuts all five at once, and because a
// trap rolls the message back, the failure is permanent rather than transient.
// Three separate things now stand between a player and that state:
//
//  1. the settlement path cannot trap on an unrankable hand any more (`rank_claims`,
//     `push_winner`, `record_hand_to_history` all use `try_evaluate_hand`), and it
//     cannot be handed an unrankable hand in the first place (the "WHO IS IN THE
//     HAND" section);
//  2. `plan_payouts` always produces a conserving plan, refunding any layer it
//     cannot settle to the seats that funded it;
//  3. and if both of those are wrong anyway -- which is the assumption to make,
//     because both were wrong before -- [`abandon_stuck_hand`] below is a door
//     that does not depend on either. It needs no evaluator, no board and no
//     opinion about who was winning.
//
// "Cannot withdraw while in a hand" is a reasonable answer only while the hand can
// actually progress, so `withdraw` and `cash_out` stop giving it once the hand is
// provably stuck, and say what to call instead.

/// How long past its own expiry an action clock may sit before the hand it
/// belongs to is treated as stuck.
///
/// Well clear of any legitimate delay: the longest configured `action_timeout_secs`
/// on any table is 60 s and the time bank adds 30 s, so a hand that has not moved
/// for five minutes past a clock that has already run out is not slow, it is
/// broken. Short enough that a player is not left waiting on a support ticket.
const STUCK_HAND_GRACE_NS: u64 = 300 * 1_000_000_000;

/// Is this table holding a hand that no message can move?
///
/// Two ways for that to be true, and they are facts about the state rather than
/// guesses about the cause:
///
/// * **the clock ran out and nothing resolved it.** `check_timeouts` and
///   `player_action` both resolve an expired timer, so a timer that is still
///   expired [`STUCK_HAND_GRACE_NS`] later means every attempt to resolve it has
///   failed -- which, on this path, means every attempt has TRAPPED;
/// * **there is no clock at all** while a hand is live. `resolve_expired_action_timer`
///   is the only thing that moves a hand nobody is acting on, and it does nothing
///   without a timer, so this state is immovable by construction, immediately and
///   not after any grace period.
///
/// Deliberately says nothing about WHY. A predicate that had to recognise the
/// specific defect would have to be updated for the next one.
pub fn hand_is_stuck(state: &TableState, now: u64) -> bool {
    let live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;
    if !live {
        return false;
    }
    match state.action_timer {
        Some(ref t) => now > t.expires_at.saturating_add(STUCK_HAND_GRACE_NS),
        None => true,
    }
}

/// The one branch of [`hand_is_stuck`] the CLOCK is allowed to act on by itself.
///
/// # Why this is narrower, and why the difference is deliberate
///
/// `hand_is_stuck` has two arms. This has one.
///
/// * **expired clock + [`STUCK_HAND_GRACE_NS`]** -- kept. Five minutes after a
///   clock that has already run out, with `advance_table_clock` having had every
///   opportunity in between, there is no reading of this state in which the hand
///   is merely slow. Refunding it is right whether a human asks or nobody does.
/// * **a live hand with NO clock at all** -- NOT acted on automatically. It is a
///   real stuck state and `abandon_stuck_hand` still refunds it on request, but it
///   is a state the engine also *believes* impossible, and a belief this project
///   has disproved repeatedly. A clock that fires every 30 s on a predicate whose
///   truth conditions are not fully understood would convert one unknown
///   legitimate no-clock moment into an automatic refund of a hand that should
///   have been played, on every table, forever, with nobody in the loop.
///   A human calling `abandon_stuck_hand` is a decision; a timer doing it is a
///   policy, and this branch has not earned one.
///
/// So the automatic door is the arm whose meaning is unambiguous, and the manual
/// door stays wider. If the no-clock arm is ever proven unreachable in legitimate
/// play, this is the one predicate to widen.
fn clock_should_abandon(state: &TableState, now: u64) -> bool {
    let live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;
    live && match state.action_timer {
        Some(ref t) => now > t.expires_at.saturating_add(STUCK_HAND_GRACE_NS),
        None => false,
    }
}

/// What a client needs to tell a player why the table is not moving, and what
/// they can do about it. A query, so it costs nothing and works when the update
/// path does not.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StuckHandStatus {
    /// True when [`abandon_stuck_hand`] would succeed right now.
    pub is_stuck: bool,
    /// True while a hand is live at all.
    pub hand_in_progress: bool,
    /// Nanoseconds until this hand becomes abandonable. `null` when it already is,
    /// or when no hand is live.
    pub abandonable_in_ns: Option<u64>,
    /// Chips that would be handed back if it were abandoned now.
    pub refundable_pot: u64,
}

#[ic_cdk::query]
fn get_stuck_hand_status() -> StuckHandStatus {
    let now = ic_cdk::api::time();
    TABLE.with(|t| {
        let table = t.borrow();
        let state = match table.as_ref() {
            Some(s) => s,
            None => {
                return StuckHandStatus {
                    is_stuck: false,
                    hand_in_progress: false,
                    abandonable_in_ns: None,
                    refundable_pot: 0,
                }
            }
        };
        let live = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;
        let is_stuck = hand_is_stuck(state, now);
        let abandonable_in_ns = match (live, is_stuck, state.action_timer.as_ref()) {
            (true, false, Some(t)) => {
                Some(t.expires_at.saturating_add(STUCK_HAND_GRACE_NS).saturating_sub(now))
            }
            _ => None,
        };
        StuckHandStatus {
            is_stuck,
            hand_in_progress: live,
            abandonable_in_ns,
            refundable_pot: if live { state.pot } else { 0 },
        }
    })
}

// ============================================================================
// WHAT THE CANISTER IS HOLDING FOR *YOU* (docs/SECURITY-FINDINGS.md FINDING 18)
// ============================================================================

/// Everything this canister is holding for ONE caller, including the part that is
/// not in their escrow balance.
///
/// # Why `get_balance` was not enough
///
/// `get_balance()` returns a `nat64` and answers exactly one question: how much
/// can I withdraw right now. An auditor cashed out of a stuck hand, read
/// `get_balance() -> 0`, and left -- while 298,000,000 e8s of theirs was still in
/// the pot. Nothing was wrong with the number. The number was never the whole
/// answer, and there was no surface that gave the whole answer.
///
/// This is that surface. It is a QUERY, so it is free and it still answers when
/// the update path is refusing everything, and it is CALLER-SCOPED, so a client
/// cannot show it for the wrong person.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CustodyStatus {
    /// Withdrawable right now: the same figure `get_balance()` returns.
    pub escrow: u64,
    /// Chips in front of you at the table. Yours, but not withdrawable until you
    /// leave the seat.
    pub chips_at_table: u64,
    /// **Your own money in the current hand's pot.** Counts the stake of a seat
    /// you have already left, which is the case `get_balance` cannot see.
    pub committed_in_pot: u64,
    /// True when the hand holding `committed_in_pot` can no longer be moved by
    /// any message, so nobody can win it and `abandon_stuck_hand()` would hand
    /// every stake back right now.
    pub committed_is_stuck: bool,
    /// Nanoseconds until the hand becomes abandonable. `null` when it already is,
    /// or when nothing of yours is committed.
    pub abandonable_in_ns: Option<u64>,
    /// `escrow + chips_at_table + committed_in_pot`: everything the canister is
    /// holding that belongs to you.
    pub total: u64,
    /// What to do next, in words, NAMING the method when a method is needed. A
    /// recovery path a player has to read the interface definition to find is not
    /// a recovery path.
    pub advice: String,
}

/// The one place the "you still have money in a pot" sentence is written.
///
/// Shared by [`get_custody_status`] and by `withdraw`'s refusals, so the two can
/// never say different things about the same state. States the amount twice --
/// formatted for a human, and in raw e8s -- because the formatted figure is
/// rounded to four decimals and a player reconciling a balance needs the exact
/// number.
fn committed_stake_sentence(committed: u64, stuck: bool, hand_number: u64) -> String {
    if committed == 0 {
        return String::new();
    }
    let currency = get_table_currency();
    let amount = format!("{} ({} e8s)", currency.format_amount(committed), committed);
    if stuck {
        format!(
            "{amount} of yours is still committed to hand {hand_number}, and that hand can no \
             longer be moved by any message, so nobody can win it. Call abandon_stuck_hand() -- \
             any principal may -- and every stake goes back to whoever put it in, including \
             yours, into your withdrawable balance."
        )
    } else {
        format!(
            "{amount} of yours is committed to hand {hand_number}, which is still in play: it is \
             contested and will be paid out when the hand settles. If the table stops moving, \
             abandon_stuck_hand() refunds every stake once the action clock has been expired for \
             5 minutes."
        )
    }
}

/// Everything the canister holds for the caller, and what to do about the part
/// that is not withdrawable.
#[ic_cdk::query]
fn get_custody_status() -> CustodyStatus {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();
    let escrow = BALANCES.with(|b| b.borrow().get(&caller).copied().unwrap_or(0));

    TABLE.with(|t| {
        let table = t.borrow();
        let Some(state) = table.as_ref() else {
            return CustodyStatus {
                escrow,
                chips_at_table: 0,
                committed_in_pot: 0,
                committed_is_stuck: false,
                abandonable_in_ns: None,
                total: escrow,
                advice: String::new(),
            };
        };

        let chips_at_table = state
            .players
            .iter()
            .flatten()
            .filter(|p| p.principal == caller)
            .map(|p| p.chips)
            .fold(0, u64::saturating_add);
        let committed_in_pot = committed_stake_of(state, caller);
        let committed_is_stuck = committed_in_pot > 0 && hand_is_stuck(state, now);
        let abandonable_in_ns = match (committed_in_pot, committed_is_stuck, &state.action_timer) {
            (c, false, Some(timer)) if c > 0 => Some(
                timer
                    .expires_at
                    .saturating_add(STUCK_HAND_GRACE_NS)
                    .saturating_sub(now),
            ),
            _ => None,
        };

        CustodyStatus {
            escrow,
            chips_at_table,
            committed_in_pot,
            committed_is_stuck,
            abandonable_in_ns,
            total: escrow
                .saturating_add(chips_at_table)
                .saturating_add(committed_in_pot),
            advice: committed_stake_sentence(
                committed_in_pot,
                committed_is_stuck,
                state.hand_number,
            ),
        }
    })
}

/// Abandon a hand that no message can move, and give every stake back.
///
/// **Anybody may call this. There is no privilege check, and that is deliberate:**
/// a recovery path only a controller can take is not a recovery path, it is a
/// support queue, and the players whose money is stuck are the ones with the
/// incentive to use it. It cannot be used as a weapon, because
///
/// * it refuses unless [`hand_is_stuck`] -- five minutes past a clock that has
///   already expired, or a live hand with no clock at all. A hand that is merely
///   slow is not abandonable, and any ordinary call (`check_timeouts`,
///   `player_action`) resets the clock and takes it further out of reach;
/// * the only outcome it can produce is [`refund_every_stake`]: each player gets
///   back exactly what they put into this hand. There is nothing to win by calling
///   it, whatever cards you were holding.
///
/// Returns the number of e8s handed back.
#[ic_cdk::update]
fn abandon_stuck_hand() -> Result<u64, String> {
    let out = try_abandon_stuck_hand(ic_cdk::api::time());
    // A successful abandonment ends the hand, which retires its action clock and
    // starts the auto-deal and idle clocks. Re-point the precise wake at whichever
    // of those is next.
    if out.is_ok() {
        schedule_next_wake();
    }
    out
}

/// The body of [`abandon_stuck_hand`], with `now` supplied.
///
/// Split out so the ON-CHAIN CLOCK can perform the identical recovery without a
/// caller. It is the same code, not a copy: an automatic refund that diverged from
/// the manual one would be the worst possible outcome, because the manual one is
/// what every document describes.
///
/// The clock reaches this through [`clock_should_abandon`], which is STRICTLY
/// NARROWER than [`hand_is_stuck`] -- see that function for why.
fn try_abandon_stuck_hand(now: u64) -> Result<u64, String> {
    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        if !hand_is_stuck(state, now) {
            let live = state.phase != GamePhase::WaitingForPlayers
                && state.phase != GamePhase::HandComplete;
            return Err(if live {
                "This hand can still progress. Call check_timeouts, or take your action. A hand \
                 is only abandonable once its action clock has been expired for 5 minutes."
                    .to_string()
            } else {
                "No hand in progress".to_string()
            });
        }

        settle_unmovable_hand(state, now, UnmovableReason::NoMessageCanMoveIt).ok_or_else(|| {
            // Only reachable if the refund plan itself does not conserve, which
            // `settle_unmovable_hand` refuses to act on rather than trapping.
            format!(
                "This hand cannot be refunded automatically: its own payout basis does not add \
                 up, so nothing has been moved. hand {} at phase {}. Please report it; the \
                 canister has logged the detail.",
                state.hand_number,
                phase_to_string(&state.phase)
            )
        })
    })
}

/// Why a hand is being closed with no winner and every stake handed back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnmovableReason {
    /// [`hand_is_stuck`]: the action clock has been expired for
    /// [`STUCK_HAND_GRACE_NS`], or a live hand has no clock at all. Nothing any
    /// player sends can advance it.
    NoMessageCanMoveIt,
    /// The same condition, found by an EXIT DOOR: somebody called `cash_out` or
    /// `leave_table` on a hand that could not be moved. Distinguished from
    /// [`UnmovableReason::NoMessageCanMoveIt`] only in the log line, and only so
    /// that a reader -- or a test -- can tell which of the three doors did the
    /// work. The money moves identically.
    AnExitDoorFoundItUnmovable,
    /// The last seated player has just left. Nobody holds cards, so nobody can
    /// win the pot -- not now and not ever, because the seats are empty and a
    /// new occupant is not in this hand.
    NobodyLeftToWinIt,
}

/// Hand every stake back and close a hand that can no longer produce a winner.
///
/// Returns the e8s refunded, or `None` when there was nothing to do.
///
/// # ONE routine, three callers, and why it must never trap
///
/// `abandon_stuck_hand` (a player asking), the on-chain clock (nobody asking) and
/// now the two EXIT DOORS, `cash_out` and `leave_table`. The exit doors are the
/// reason for the no-trap rule: a trap rolls the whole message back, so a
/// settlement that traps inside `cash_out` would take the cash-out with it and
/// shut the door the player was walking through -- which is
/// docs/SECURITY-FINDINGS.md FINDING 15 exactly, re-created by the fix for
/// FINDING 18. [`apply_payouts`] traps when a plan does not conserve, so the plan
/// is checked FIRST and a non-conserving one is reported and declined. The
/// player still leaves; the stake stays where it is; [`check_custody_is_visible`
/// in the money-safety harness](../../../tests/money_safety/src/invariants/custody.rs)
/// is what convicts the resulting state.
///
/// # Why the exit doors settle at all (docs/SECURITY-FINDINGS.md FINDING 18)
///
/// FINDING 15's fix lifted the "cannot cash out while in a hand" refusal once the
/// hand is stuck, which was right -- that refusal was the second half of a fund
/// lock. But it was also the ONLY thing that had ever told a player their money
/// was committed, so lifting it produced `Ok = 0` while 2.98 ICP of the caller's
/// was in the pot, `get_balance() -> 0`, and a canister left holding a live
/// pre-flop hand with no players in it.
///
/// Annotating the reply was the auditor's minimum. This is the stronger answer:
/// **remove the state instead of describing it.** A stuck hand cannot be won by
/// anybody, and neither can a hand with nobody in it, so there is exactly one
/// correct outcome in both cases and the player who is on their way out is
/// entitled to have it happen before they go. It grants no new power -- a stuck
/// hand is already refundable by ANY principal through `abandon_stuck_hand`, so
/// the caller could produce this identical state in one extra call -- and it
/// makes the number `cash_out` returns true again.
fn settle_unmovable_hand(
    state: &mut TableState,
    now: u64,
    reason: UnmovableReason,
) -> Option<u64> {
    let live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;
    if !live {
        return None;
    }

    let plan = refund_every_stake(state);
    if !plan.conserves() {
        // NOT a trap, and not a silent skip either. See the no-trap rule above.
        ic_cdk::println!(
            "CRITICAL: refusing to refund hand {} at phase {}: the refund plan hands back {} out \
             of {} collected (delta {}). Nothing has been moved and the stakes are still in the \
             pot. See docs/SECURITY-FINDINGS.md FINDING 18.",
            state.hand_number,
            phase_to_string(&state.phase),
            plan.awarded,
            plan.collected,
            plan.awarded as i128 - plan.collected as i128
        );
        return None;
    }

    match reason {
        // Loud, and in the dialect the money-safety harness treats as the
        // canister's own testimony against itself. Reaching this line means the
        // settlement path failed to do its job, and that must never be quiet.
        UnmovableReason::NoMessageCanMoveIt => ic_cdk::println!(
            "CRITICAL: hand {} was ABANDONED as unmovable at phase {}: no message could advance \
             it. {} e8s across {} stakes returned to the players who put them in; nobody won the \
             hand. See docs/SECURITY-FINDINGS.md FINDING 15.",
            state.hand_number,
            phase_to_string(&state.phase),
            plan.collected,
            plan.payouts.len()
        ),
        // Same event, same money, different discoverer: the player walking out.
        UnmovableReason::AnExitDoorFoundItUnmovable => ic_cdk::println!(
            "CRITICAL: hand {} was ABANDONED as unmovable at phase {} BY AN EXIT DOOR: a player \
             called cash_out or leave_table and no message could advance the hand they were \
             leaving. {} e8s across {} stakes returned to the players who put them in, including \
             the leaver's, so nobody walks away from a stake they were never told about. See \
             docs/SECURITY-FINDINGS.md FINDING 18.",
            state.hand_number,
            phase_to_string(&state.phase),
            plan.collected,
            plan.payouts.len()
        ),
        // Deliberately NOT `CRITICAL:`. Nothing has gone wrong with the
        // accounting and nothing failed: the last player exercised an ordinary
        // right to leave, and a hand with no players in it has exactly one lawful
        // ending, which the canister has just performed. `CRITICAL:` is reserved
        // in this file for accounts that disagree, and the money-safety
        // classifier stops a run on every one of them.
        UnmovableReason::NobodyLeftToWinIt => ic_cdk::println!(
            "hand {} ended at phase {} with NO players left at the table: {} e8s across {} \
             stakes returned to the escrow balances of the principals who put them in. Nobody \
             could have won it.",
            state.hand_number,
            phase_to_string(&state.phase),
            plan.collected,
            plan.payouts.len()
        ),
    }

    reveal_seed_on_hand_end(state);
    let refunded = plan.collected;
    // Through `apply_payouts`, so this is held to the SAME conservation
    // post-condition as a real settlement -- and it cannot trap there, because
    // the identical predicate was evaluated above.
    let winners = apply_payouts(state, &plan);
    record_hand_to_history(state, &winners, false);
    finish_hand(state, now);
    Some(refunded)
}

/// Is anybody still sitting at this table?
fn table_is_empty(state: &TableState) -> bool {
    state.players.iter().all(|p| p.is_none())
}

/// Everything of `who`'s that is in the CURRENT hand's pot.
///
/// Their live seat's `total_bet_this_hand`, plus every stake recorded for a seat
/// of theirs that has already been vacated in this hand. Summed BY PRINCIPAL and
/// never by seat: one chair can carry two people's money in a single hand
/// (docs/DEFECTS.md E-36) and resolving an owner from a seat index is
/// docs/SECURITY-FINDINGS.md FINDING 13.
///
/// Zero once the hand is over, which is correct rather than convenient: at that
/// point the money has been paid out, and what is left is in a stack or an escrow
/// balance that the ordinary surfaces already report.
pub fn committed_stake_of(state: &TableState, who: Principal) -> u64 {
    let live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;
    if !live {
        return 0;
    }
    let seated: u64 = state
        .players
        .iter()
        .flatten()
        .filter(|p| p.principal == who)
        .map(|p| p.total_bet_this_hand)
        .fold(0, u64::saturating_add);
    state
        .departed_stakes()
        .iter()
        .filter(|d| d.hand_number == state.hand_number && d.principal == who)
        .map(|d| d.contributed)
        .fold(seated, u64::saturating_add)
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
///
/// MARKING A SEAT `Disconnected` HERE DOES NOT TAKE IT OUT OF THE HAND. It used
/// to, by omission -- every count that drove the betting round filtered on
/// `status == Active` -- and that was docs/DEFECTS.md E-32 and E-06. See the note
/// above [`is_in_hand`]. A seat in a live hand leaves it by folding, by being
/// folded when its ACTION clock expires ([`resolve_expired_action_timer`]), or by
/// being all-in. Nothing in this function folds anybody except that one call.
/// # THIS IS A BACKSTOP NOW, NOT THE ENGINE'S ONLY CLOCK
///
/// Until [FINDING 19](../../../docs/SECURITY-FINDINGS.md#finding-19) was closed
/// this update call was the ONLY thing in the canister that evaluated any
/// deadline, so a table whose clients all closed their tabs froze forever.
/// [`advance_table_clock`] is now also driven by an on-chain timer (see
/// "THE ON-CHAIN CLOCK" below). This entry point is kept, deliberately, for the
/// case where a timer is ever lost -- and it runs THE SAME FUNCTION, so the two
/// paths cannot drift.
#[ic_cdk::update]
fn check_timeouts() -> TimeoutCheckResult {
    let result = advance_table_clock(ic_cdk::api::time());
    // Whatever this call just changed may have moved the next deadline. Re-point
    // the precise wake at it; without this the watchdog interval is the only
    // thing that would notice, up to CLOCK_WATCHDOG_SECS late.
    schedule_next_wake();
    result
}

/// Everything the table's clock does, in one place, driven by a `now` the caller
/// supplies.
///
/// **Both clock paths call exactly this**: the `check_timeouts` update above, and
/// [`on_clock_tick`]. There is no second copy of the timeout rules to fall out of
/// step with this one, which is the specific failure the timer work was told to
/// avoid.
///
/// Order matters and is not arbitrary. The unmovable-hand refund runs FIRST and
/// returns immediately when it fires, because the states it exists for are exactly
/// the states in which the rest of this function TRAPS (docs/SECURITY-FINDINGS.md
/// FINDING 15). Run last, a trap below would roll the refund back with it; run
/// first and short-circuited, the refund either commits alone or does not happen
/// at all.
pub fn advance_table_clock(now: u64) -> TimeoutCheckResult {
    // Run periodic cleanup of unbounded maps
    periodic_cleanup();

    // THE REFUND IS NOT HERE, AND THAT IS A CORRECTION, NOT AN OMISSION.
    //
    // The first version of this function opened by refunding any hand whose clock
    // had been expired for STUCK_HAND_GRACE_NS, so that a timeout path which
    // TRAPPED could not take the refund down with it. The money-safety fuzzer
    // convicted it on seed 0xc1ea2dec0002 within one run:
    //
    //   CRITICAL: hand 7 was ABANDONED as unmovable at phase flop ...
    //   4000000 e8s across 2 stakes returned ... nobody won the hand.
    //
    // Nothing was unmovable. The fuzzer had jumped an hour of simulated time, so
    // the clock was five minutes past its expiry the FIRST time anything looked at
    // it -- and the hand was voided without the ordinary timeout path ever being
    // tried on it. On mainnet the same shape arrives after any stall in which the
    // canister does not execute: a subnet halt, or -- pointedly -- a canister that
    // was frozen for want of cycles and then topped up. The right answer there is
    // to fold the seat that did not act and play the hand out, not to void it.
    //
    // "Past its grace" is a statement about WALL CLOCK. "Nothing can move this
    // hand" is a statement about ATTEMPTS. They are only the same while the clock
    // is actually running, and the escalation that tells them apart lives in
    // `on_clock_tick`, which can count attempts across separate messages. This
    // function does the timeouts and nothing else, so `check_timeouts` and the
    // clock still run exactly the same code.
    let disconnect_timeout_ns: u64 = DISCONNECT_TIMEOUT_SECS * 1_000_000_000;

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = match table.as_mut() {
            Some(s) => s,
            None => return TimeoutCheckResult::NoAction,
        };

        // Check for disconnected players (no heartbeat)
        for player in state.players.iter_mut().flatten() {
            if player.status == PlayerStatus::Active && now > player.last_seen + disconnect_timeout_ns {
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
                .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
                .count();
            if active_count >= 2 {
                state.auto_deal_at = Some(now + AUTO_DEAL_DELAY_NS);
            }
        }

        if let Some(auto_deal_time) = state.auto_deal_at {
            if now >= auto_deal_time && (state.phase == GamePhase::HandComplete || state.phase == GamePhase::WaitingForPlayers) {
                // Only signal auto-deal if we have enough active players with chips
                let active_count = state.players.iter()
                    .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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
/// resolved the timeout. So if the frontend stopped polling, `state.action_on`
/// stayed pointed at a seat that could no longer act and the hand could not
/// progress at all. That was hit immediately in manual play: both seats ended up
/// `Disconnected` and `start_new_hand` refused with "Need at least 2 active
/// players with chips" until they were sat back in by hand. See
/// docs/DEFECTS.md E-31.
///
/// This paragraph used to end *"Nothing in the canister calls `check_timeouts` on
/// its own -- there is no heartbeat timer driving it, the frontend does."* That
/// was TRUE and it was the whole of FINDING 19: a fund-holding canister whose
/// liveness was outsourced to a browser tab. It is no longer true --
/// `on_clock_tick` drives `advance_table_clock` on chain, see "THE ON-CHAIN
/// CLOCK" -- and the sentence is rewritten here rather than deleted because a
/// stale comment asserting no clock exists is exactly how the next reader stops
/// looking. docs/DEFECTS.md E-54.
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

// ============================================================================
// THE ON-CHAIN CLOCK  (docs/SECURITY-FINDINGS.md FINDING 19)
// ============================================================================
//
// Before this section existed, `ic-cdk-timers` was a declared dependency that
// nothing imported: `set_timer` appeared nowhere in `src/`, and every deadline in
// this engine -- the action clock, the disconnect threshold, the sitting-out kick,
// the reload timer, the stuck-hand grace -- was evaluated only inside a message
// somebody else sent. A table whose clients all closed their tabs froze. Measured
// on this harness before the change: a heads-up hand with a 3,000,000 e8 pot sat
// in `PreFlop` for TWENTY SIMULATED MINUTES with both seats still marked `Active`,
// and would have sat there forever.
//
// # The shape, and why it is this shape
//
// Two mechanisms, doing two different jobs.
//
// ## 1. A repeating WATCHDOG interval -- the part that cannot die
//
// `ic-cdk-timers` 1.0 does not run a timer callback inside `canister_global_timer`.
// It makes a bounded-wait SELF-CALL and runs the callback in that message, for the
// explicit purpose of catching traps at the call boundary (see the crate's own
// comment: *"the closest thing to a catch_unwind that's available here"*). The
// consequence that decides this design is in `do_timer` step 7:
//
//   > If a repeated timer is successfully DISPATCHED (irrespective of the timer's
//   > own success), reschedule it.
//
// So a REPEATING timer is rescheduled before its callback runs, and survives a
// callback that traps. A self-rescheduling ONE-SHOT does not: the re-arm lives in
// the same message as the work, so one trap and the clock is gone for good, with
// nothing on chain to notice. On a canister whose settlement path has trapped
// before -- FINDING 15 is exactly that -- a clock that dies on the first trap is
// not a clock.
//
// The watchdog is therefore the floor. A table whose timeout path traps on every
// tick still gets its money back, because the escalation in `on_clock_tick` writes
// its sighting in a message that cannot trap and runs the work in a message of its
// own: the trap rolls back the work and leaves the sighting, so the stuck-hand
// grace keeps running and the refund fires in the end. (This sentence used to say
// the refund was handled FIRST inside `advance_table_clock`. It was, and that
// version voided playable hands after any stall -- docs/DEFECTS.md E-56.)
//
// ## 2. A one-shot WAKE aimed at the exact next deadline -- the part that is precise
//
// Arming and cancelling a timer are free: they are heap operations plus
// `ic0.global_timer_set` inside a message the canister is already executing. Only
// a FIRE costs anything, because only a fire is a message. So the wake is pointed
// at [`next_wake_deadline`] -- the earliest moment at which crossing a deadline
// would actually CHANGE state -- and re-pointed every time the state moves. On a
// table where players act inside their clocks it is re-armed further out on every
// action and never fires at all.
//
// ## What a tick costs, MEASURED on this build
//
// `tests/money_safety/tests/timers.rs::idle_table_cycle_burn_and_runway`, empty
// table, one simulated hour, PocketIC 13-node application subnet:
//
// | watchdog | ticks/hour | cycles burned | cycles per tick |
// |----------|-----------|----------------|-----------------|
// | 30 s     | 120       | 1,843,363,200  | **15,361,360**  |
// | 60 s     | 60        |   923,160,120  | **15,386,002**  |
//
// Exactly linear in the number of ticks, so the per-tick figure prices every
// alternative. A tick is NOT one message: the crate's trap-catching self-call
// makes it a `canister_global_timer` execution, an inter-canister call, the
// `timer_executor` update, and a reply callback. That is where 15.4M goes, and it
// is the price of the trap resistance above.
//
// | shape                          | idle-table burn | idle-table burn/year | worst-case lateness |
// |--------------------------------|-----------------|----------------------|---------------------|
// | `set_timer_interval(1s)`       | 1.328 T/day     | **485 T**            | 1 s                 |
// | `set_timer_interval(10s)`      | 0.133 T/day     | 48.5 T               | 10 s                |
// | **watchdog 30 s + one-shot**   | **0.0442 T/day**| **16.2 T**           | ~0 while a hand runs |
// | watchdog 60 s + one-shot       | 0.0222 T/day    | 8.1 T                | ~0 while a hand runs |
//
// The bare 1 s interval the finding suggests is 485 T/year for a table nobody is
// sitting at, and it is LESS precise than the hybrid, not more: the one-shot is
// aimed at the actual expiry rather than at a fixed 1 s grid.
//
// For comparison, this canister burned 0.00007 T/day before the clock existed
// (storage only). The clock multiplies idle burn by ~630x. That is the honest cost
// of this change and it is why `get_cycle_status` exists below.
//
// `CLOCK_WATCHDOG_SECS` is the single knob: burn is `86400 / period * 15.4M`
// cycles per day. Doubling it halves the bill and doubles only the worst case for
// deadlines that arrive with no arm site, of which there are currently none.
//
// ## The one thing a reader should be suspicious of
//
// The precise wake is only as good as [`next_wake_deadline`]. A deadline that
// function forgets is a deadline the one-shot never aims at. That is a PRECISION
// bug, never a LIVENESS bug, and only because the watchdog exists: a forgotten
// deadline is still crossed within `CLOCK_WATCHDOG_SECS`. That is the whole reason
// the watchdog is not tuned out once the one-shot works.

/// How often the trap-proof watchdog runs, in seconds.
///
/// This is the answer to "how late can this table possibly be" for any deadline
/// the one-shot wake misses, including every deadline created between ticks by a
/// client that then disappeared. 30 s against a 30 s action clock and a 90 s
/// disconnect threshold, at a measured cost given above.
const CLOCK_WATCHDOG_SECS: u64 = 30;

/// The one-shot wake is never armed closer than this.
///
/// A deadline whose crossing does not change the state would otherwise re-arm at
/// zero delay forever, turning the clock into a cycle-burning spin loop. Every
/// entry in [`next_wake_deadline`] is chosen so that crossing it DOES change the
/// state, so this floor should be unreachable -- it is here because "should be" is
/// how this project got most of its findings.
const CLOCK_MIN_WAKE_NS: u64 = 1_000_000_000;

thread_local! {
    /// The repeating watchdog. Deliberately never cleared once started.
    static CLOCK_WATCHDOG: RefCell<Option<ic_cdk_timers::TimerId>> = const { RefCell::new(None) };
    /// The one-shot aimed at the next real deadline, and the deadline it is aimed
    /// at. Replaced, not accumulated: a second wake left armed is a second message.
    static CLOCK_WAKE: RefCell<Option<(ic_cdk_timers::TimerId, u64)>> = const { RefCell::new(None) };
    /// Ticks the on-chain clock has run since this instance started. Observable
    /// via `get_cycle_status`, because "the timer is armed" is a claim and a
    /// counter that moves is evidence.
    static CLOCK_TICKS: RefCell<u64> = const { RefCell::new(0) };
    static CLOCK_LAST_TICK_AT: RefCell<u64> = const { RefCell::new(0) };
    /// `(time, cycle balance)` at the FIRST tick of this instance, or at the last
    /// tick that observed a top-up. The burn rate `get_cycle_status` reports is
    /// MEASURED between this and [`CYCLES_LATEST`], not computed from a price list
    /// that could be wrong or out of date.
    static CYCLES_ORIGIN: RefCell<(u64, u128)> = const { RefCell::new((0, 0)) };
    /// `(hand_number, when the CLOCK first saw this hand sitting on an expired
    /// action timer)`.
    ///
    /// The stuck-hand grace is measured from HERE, not from the timer's own
    /// `expires_at`, and the difference is the whole point. See [`on_clock_tick`].
    static CLOCK_STUCK_SINCE: RefCell<Option<(u64, u64)>> = const { RefCell::new(None) };
    /// `(time, cycle balance)` at the most recent tick.
    ///
    /// # Why both samples are taken in the SAME context, and why that is not fussy
    ///
    /// The first version of this sampled the origin in `start_clock` and compared
    /// it against the balance read inside `get_cycle_status`. It reported a burn of
    /// ZERO for an hour in which 1.84 BILLION cycles were demonstrably burned.
    ///
    /// The reason is that `ic-cdk-timers` runs every callback behind a self-call,
    /// and a canister executing inside that callback has an outstanding PREPAYMENT
    /// for the call and its reserved response. The balance visible from inside a
    /// tick is therefore depressed, by more than an hour of burn, relative to the
    /// balance visible from a query. Subtracting one from the other measured the
    /// prepayment, not the burn, and produced a comfortable-looking answer.
    ///
    /// Both samples are now taken at the same point of the same kind of message, so
    /// the offset is identical in both and cancels exactly. A cycles gauge that
    /// reads high is the one failure mode that matters here: it is the gauge saying
    /// "plenty of fuel" to a canister that is about to stop honouring withdrawals.
    static CYCLES_LATEST: RefCell<(u64, u128)> = const { RefCell::new((0, 0)) };
}

/// Start (or restart) the on-chain clock.
///
/// Called from `init` and from `post_upgrade`. **`post_upgrade` is not optional:**
/// timers live in the heap and in the system's global-timer field, and an upgrade
/// wipes both. A canister upgraded without this call keeps its funds, its chips
/// and its hand, and silently loses the only thing that moves them.
fn start_clock() {
    // Idempotent: a second call must not leave two watchdogs running.
    CLOCK_WATCHDOG.with(|w| {
        if let Some(old) = w.borrow_mut().take() {
            ic_cdk_timers::clear_timer(old);
        }
    });
    let id = ic_cdk_timers::set_timer_interval(
        std::time::Duration::from_secs(CLOCK_WATCHDOG_SECS),
        || async { on_clock_tick() },
    );
    CLOCK_WATCHDOG.with(|w| *w.borrow_mut() = Some(id));

    // Deliberately NOT sampled here: see CYCLES_LATEST. The origin is taken by the
    // first tick, in the same message context every later sample is taken in.
    CYCLES_ORIGIN.with(|c| *c.borrow_mut() = (0, 0));
    CYCLES_LATEST.with(|c| *c.borrow_mut() = (0, 0));

    schedule_next_wake();
}

/// One tick of the on-chain clock.
///
/// Normally: run [`advance_table_clock`] -- the SAME function `check_timeouts`
/// runs, so the two paths cannot drift -- and re-aim the one-shot wake. If it
/// traps, this whole message rolls back and the re-aim is lost with it. That is
/// survivable only because of the watchdog, which was rescheduled before this
/// callback started. See the section header.
///
/// # The recovery escalation, and the defect it exists because of
///
/// A hand whose action clock expired more than [`STUCK_HAND_GRACE_NS`] ago is
/// *abandonable* by the wall clock. It is not necessarily *unmovable*: it is only
/// unmovable if the ordinary timeout path has actually been tried on it and
/// failed. Those two are the same thing only while the clock has been running, and
/// they come apart after any stall in which the canister does not execute -- a
/// subnet halt, or a canister frozen for want of cycles and later topped up
/// ([E-55](../../../docs/DEFECTS.md#e-55)), or a test harness advancing time in
/// hour-long jumps.
///
/// Conflating them refunded a perfectly playable hand. The money-safety fuzzer
/// convicted the first version of this on its second seed; see the comment in
/// [`advance_table_clock`] and docs/DEFECTS.md E-56.
///
/// So the grace is measured from **when this canister first saw the clock overdue**
/// ([`CLOCK_STUCK_SINCE`]), not from the timer's own `expires_at`. An hour-long
/// stall therefore buys no credit toward abandonment: the first tick after it
/// starts the grace and hands the hand to the ordinary timeout path, which folds
/// the seat that did not act and plays on.
///
/// The sighting is written in THIS message, which does nothing that can trap, and
/// the work runs in a message of its own. That is what makes the escalation
/// survive a trapping timeout path: the rollback takes the attempt and leaves the
/// sighting, so the grace keeps running and the refund eventually fires. It costs
/// a second message only while a clock is actually overdue, so the idle-table burn
/// measured in the section header is unaffected.
fn on_clock_tick() {
    let now = ic_cdk::api::time();
    // `*c.borrow_mut() = c.borrow() + 1` PANICS: the assigned value is evaluated
    // first and its `Ref` guard lives to the end of the statement, so the
    // `borrow_mut` on the left hits an outstanding shared borrow. It was written
    // that way here, it trapped on every tick, and the only reason it did not take
    // the clock with it is the watchdog rescheduling before the callback runs.
    // Left as a comment rather than deleted: it is the cheapest possible reminder
    // of what the watchdog is actually for.
    CLOCK_TICKS.with(|c| {
        let mut ticks = c.borrow_mut();
        *ticks = ticks.saturating_add(1);
    });
    CLOCK_LAST_TICK_AT.with(|c| *c.borrow_mut() = now);

    // Both cycle samples are taken HERE, at the same point of the same kind of
    // message, so the outstanding self-call prepayment is identical in both and
    // cancels out of the difference. See CYCLES_LATEST.
    let balance = ic_cdk::api::canister_cycle_balance();
    CYCLES_ORIGIN.with(|c| {
        let mut origin = c.borrow_mut();
        // First tick of this instance, or a top-up: re-baseline. Keeping an origin
        // from before a top-up would report a burn rate too low forever after.
        if origin.0 == 0 || balance > origin.1 {
            *origin = (now, balance);
        }
    });
    CYCLES_LATEST.with(|c| *c.borrow_mut() = (now, balance));

    // Read-only, and it must not trap: everything below depends on THIS message
    // committing even when the work it schedules does not.
    let overdue = TABLE
        .try_with(|t| {
            t.try_borrow().ok().and_then(|table| {
                table.as_ref().map(|s| {
                    let live = s.phase != GamePhase::WaitingForPlayers
                        && s.phase != GamePhase::HandComplete;
                    let expired = s
                        .action_timer
                        .as_ref()
                        .map(|t| now > t.expires_at)
                        .unwrap_or(false);
                    (live && expired, s.hand_number)
                })
            })
        })
        .ok()
        .flatten();

    let Some((overdue, hand_number)) = overdue else {
        CLOCK_STUCK_SINCE.with(|s| *s.borrow_mut() = None);
        let _ = advance_table_clock(now);
        schedule_next_wake();
        return;
    };

    if !overdue {
        // The hand is moving, or there is no hand. Forget any earlier observation
        // so a later stall on the same hand starts its grace from scratch.
        CLOCK_STUCK_SINCE.with(|s| *s.borrow_mut() = None);
        let _ = advance_table_clock(now);
        schedule_next_wake();
        return;
    }

    // A clock has run out. Record WHEN THIS CANISTER FIRST SAW IT, if it has not
    // already, and read back the first-sighting time.
    let since = CLOCK_STUCK_SINCE.with(|s| {
        let mut slot = s.borrow_mut();
        match *slot {
            Some((h, t)) if h == hand_number => t,
            _ => {
                *slot = Some((hand_number, now));
                now
            }
        }
    });

    // This message does nothing that can trap, so the sighting above is COMMITTED
    // whatever happens next. The work goes in a message of its own, which is the
    // entire mechanism: a timeout path that traps rolls back alone and leaves the
    // sighting standing, so the grace period keeps running and the refund below
    // eventually fires. Two messages instead of one, and only while a clock is
    // actually overdue -- an idle table never reaches this branch, so the burn
    // measured in the section header is unaffected.
    if now > since.saturating_add(STUCK_HAND_GRACE_NS) {
        // The ordinary timeout path has had every tick of a full grace period, in
        // messages of its own, and this hand has still not moved. It is not slow.
        // Refund every stake, through the same body `abandon_stuck_hand` runs --
        // which grants nobody any authority they did not already have, because that
        // method is deliberately permissionless.
        CLOCK_STUCK_SINCE.with(|s| *s.borrow_mut() = None);
        ic_cdk_timers::set_timer(std::time::Duration::ZERO, async {
            let now = ic_cdk::api::time();
            // Re-checked at the MOMENT OF ACTION, in the message that acts. The
            // decision above was made in an earlier message and the table may have
            // moved since. This is also the guard that keeps the automatic door
            // strictly narrower than the manual one: `clock_should_abandon` refuses
            // the live-hand-with-no-clock branch that `hand_is_stuck` accepts.
            let still_unmovable = TABLE.with(|t| {
                t.borrow()
                    .as_ref()
                    .map(|s| clock_should_abandon(s, now))
                    .unwrap_or(false)
            });
            if still_unmovable {
                let _ = try_abandon_stuck_hand(now);
            }
            schedule_next_wake();
        });
    } else {
        ic_cdk_timers::set_timer(std::time::Duration::ZERO, async {
            let _ = advance_table_clock(ic_cdk::api::time());
            schedule_next_wake();
        });
    }
    schedule_next_wake();
}

/// Point the one-shot wake at [`next_wake_deadline`], replacing whatever it was
/// aimed at before.
///
/// Cheap enough to call from anywhere that changes the state: no message is sent,
/// nothing is charged, and if the deadline has not moved the existing wake is left
/// exactly where it is.
///
/// # The arm sites, and why each one is there
///
/// | site | why |
/// |------|-----|
/// | `init`, `post_upgrade` (via [`start_clock`]) | nothing is armed at all without these |
/// | [`on_clock_tick`] | every tick re-aims from the state it just left behind |
/// | `check_timeouts` | the manual path must leave the clock in the same place the timer path would |
/// | `start_new_hand` | creates the first action clock of a hand |
/// | `player_action` | REPLACES the action clock the wake is aimed at |
/// | `use_time_bank` | EXTENDS the action clock the wake is aimed at |
///
/// The last three are about cycles as much as precision. A wake left aimed at a
/// clock that has already been replaced fires early, finds nothing, and re-arms --
/// one wasted timer message per betting action, at ~15.4M cycles each. Firing
/// early is never a correctness problem; being armed late is, and that is what the
/// watchdog covers.
fn schedule_next_wake() {
    let now = ic_cdk::api::time();
    // A borrow failure here must not trap -- this runs at the end of other
    // people's update calls. Falling through to "no deadline" costs precision for
    // one tick; the watchdog covers it.
    let deadline = TABLE
        .try_with(|t| {
            t.try_borrow()
                .ok()
                .and_then(|table| table.as_ref().and_then(|s| next_wake_deadline(s, now)))
        })
        .ok()
        .flatten();

    let Some(deadline) = deadline else {
        CLOCK_WAKE.with(|w| {
            if let Some((old, _)) = w.borrow_mut().take() {
                ic_cdk_timers::clear_timer(old);
            }
        });
        return;
    };

    // Already aimed there. Re-arming would be harmless but pointless.
    if CLOCK_WAKE.with(|w| w.borrow().map(|(_, at)| at == deadline).unwrap_or(false)) {
        return;
    }

    let delay = deadline.saturating_sub(now).max(CLOCK_MIN_WAKE_NS);
    // Nothing beyond the watchdog's own period is worth a second timer for.
    if delay > CLOCK_WATCHDOG_SECS * 1_000_000_000 {
        CLOCK_WAKE.with(|w| {
            if let Some((old, _)) = w.borrow_mut().take() {
                ic_cdk_timers::clear_timer(old);
            }
        });
        return;
    }

    CLOCK_WAKE.with(|w| {
        let mut slot = w.borrow_mut();
        if let Some((old, _)) = slot.take() {
            ic_cdk_timers::clear_timer(old);
        }
        let id = ic_cdk_timers::set_timer(std::time::Duration::from_nanos(delay), async {
            on_clock_tick()
        });
        *slot = Some((id, deadline));
    });
}

/// The earliest future moment at which crossing a deadline would CHANGE this
/// table's state, or `None` if there is nothing pending.
///
/// Every entry must satisfy that condition. An entry whose crossing changes
/// nothing is a spin loop -- `auto_deal_at` is the live example and is commented
/// where it is handled.
///
/// # Deadlines already in the PAST are deliberately not returned
///
/// `consider` only accepts `at > now`. A deadline that is already past and has not
/// been acted on means the last attempt to act on it did not commit -- on this
/// canister, that it TRAPPED. Aiming the one-shot at it would retry once a second
/// forever, at ~15.4M cycles a go, for as long as the trap persists. The watchdog
/// retries it every `CLOCK_WATCHDOG_SECS` instead, which is the right pacing for a
/// retry, and [`clock_should_abandon`] refunds the hand outright once the grace
/// period is up. Pinned by
/// `clock_schedule_tests::an_already_missed_deadline_is_left_to_the_watchdog_rather_than_spun_on`.
///
/// `now` is a parameter and this function is `pub` so the whole schedule is
/// host-testable without a replica: see `tests/unit_tests.rs`.
pub fn next_wake_deadline(state: &TableState, now: u64) -> Option<u64> {
    let mut next: Option<u64> = None;
    let mut consider = |at: u64| {
        if at > now {
            next = Some(next.map_or(at, |cur: u64| cur.min(at)));
        }
    };

    let live =
        state.phase != GamePhase::WaitingForPlayers && state.phase != GamePhase::HandComplete;

    // The action clock. `resolve_expired_action_timer` fires on `now > expires_at`,
    // so the first instant that does anything is one nanosecond after it.
    if let Some(ref timer) = state.action_timer {
        consider(timer.expires_at.saturating_add(1));
        // And the moment the hand becomes unmovable, for the case where resolving
        // that clock keeps trapping. Dominated by the line above while the timer
        // exists, so it costs nothing; it is here so the schedule states the whole
        // rule rather than the part that usually matters.
        if live {
            consider(
                timer
                    .expires_at
                    .saturating_add(STUCK_HAND_GRACE_NS)
                    .saturating_add(1),
            );
        }
    }

    let disconnect_ns = DISCONNECT_TIMEOUT_SECS * 1_000_000_000;
    let reload_ns = RELOAD_TIMEOUT_SECS * 1_000_000_000;
    let kick_ns = SITTING_OUT_KICK_SECS * 1_000_000_000;

    for player in state.players.iter().flatten() {
        // Crossing this marks the seat Disconnected, once.
        if player.status == PlayerStatus::Active {
            consider(player.last_seen.saturating_add(disconnect_ns).saturating_add(1));
        }
        // Crossing this sits a broke player out and clears `broke_at`, once.
        if let Some(broke_at) = player.broke_at {
            if player.chips == 0 {
                consider(broke_at.saturating_add(reload_ns).saturating_add(1));
            }
        }
        // Crossing this frees the seat and returns its chips to escrow, once.
        // Only between hands, which is the same guard the kick itself uses.
        if !live
            && (player.status == PlayerStatus::SittingOut
                || player.status == PlayerStatus::Disconnected)
        {
            let idle_since = player.sitting_out_since.unwrap_or(player.last_seen);
            consider(idle_since.saturating_add(kick_ns).saturating_add(1));
        }
    }

    // `auto_deal_at` is NOT scheduled when the table could actually deal.
    //
    // Crossing it then changes NOTHING: `advance_table_clock` returns
    // `AutoDealReady` and leaves `auto_deal_at` set, so the same deadline would be
    // due again immediately, and the wake would re-arm at the floor and spin
    // forever burning cycles on a table that is merely waiting for a client to
    // call `start_new_hand`. It IS scheduled in the other case, where crossing it
    // clears the field -- a real, one-time state change.
    //
    // The clock deliberately does not deal hands by itself. Dealing to seats whose
    // clients are gone would post their blinds, hand after hand, and that is a way
    // to lose money to a timer rather than to a player.
    if let Some(at) = state.auto_deal_at {
        // The between-hands question, asked of the named predicate rather than
        // written out again: see `will_be_dealt_in` in "WHO IS IN THE HAND".
        let active_with_chips = state
            .players
            .iter()
            .flatten()
            .filter(|p| will_be_dealt_in(p))
            .count();
        if active_with_chips < 2 {
            consider(at.max(now.saturating_add(1)));
        }
    }

    next
}

// ----------------------------------------------------------------------------
// CYCLES: the runway, visible to anybody
// ----------------------------------------------------------------------------
//
// A canister below its freezing threshold rejects every update call. On this
// canister that is `deposit`, `withdraw`, `cash_out`, `player_action` and
// `abandon_stuck_hand` all failing at once -- every player unable to reach their
// own money simultaneously, with no attacker and no in-application remedy. The
// clock added here BURNS CYCLES CONTINUOUSLY, so it makes that failure arrive
// sooner, and it would be indefensible to add it without making the runway
// visible.
//
// This is observability only. There is deliberately no top-up mechanism here: see
// docs/SECURITY-FINDINGS.md FINDING 19 for what one would need.

/// What is left in the tank, measured rather than assumed.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CycleStatus {
    /// Total cycle balance.
    pub balance: u128,
    /// What can actually be SPENT: the balance minus the reserve the system holds
    /// back for the freezing threshold. This, not `balance`, is the number that
    /// reaches zero when update calls start being rejected.
    pub liquid_balance: u128,
    /// `balance - liquid_balance`: the freezing reserve, roughly this canister's
    /// idle burn over its configured freezing threshold (30 days by default).
    pub reserved_for_freezing: u128,
    /// Cycles burned per day, MEASURED over `sample_window_secs` on this instance.
    /// Zero until the window is long enough to say anything.
    pub observed_burn_per_day: u128,
    /// `liquid_balance / observed_burn_per_day`. `null` when the window is too
    /// short to have measured a burn rate, which is not the same as "plenty".
    pub runway_days: Option<u64>,
    /// How long the measurement above has been running. An upgrade resets it.
    pub sample_window_secs: u64,
    /// True once the sample window is long enough for `runway_days` to mean
    /// something. Reported separately so a reader is never left guessing whether
    /// a small number is a measurement or an artefact.
    pub measurement_is_meaningful: bool,
    /// Ticks the on-chain clock has run since this instance started. **Zero on a
    /// canister that has been up for minutes means the clock is not running** --
    /// which is what a lost `post_upgrade` re-arm looks like from outside.
    pub clock_ticks: u64,
    /// When the clock last ran, in IC nanoseconds. 0 means never.
    pub clock_last_tick_at: u64,
    /// Whether the repeating watchdog is armed in this instance.
    pub clock_watchdog_armed: bool,
    /// The next moment the clock has a reason to run, if it has one.
    pub next_wake_at: Option<u64>,
}

/// A window shorter than this cannot say anything useful about a burn rate: the
/// install and the first few messages dominate it.
const CYCLE_SAMPLE_MIN_SECS: u64 = 300;

/// The runway, for anybody who asks. A query, so it costs the caller nothing and
/// keeps working when the update path does not.
#[ic_cdk::query]
fn get_cycle_status() -> CycleStatus {
    let now = ic_cdk::api::time();
    let balance = ic_cdk::api::canister_cycle_balance();
    let liquid = ic_cdk::api::canister_liquid_cycle_balance();

    let (origin_time, origin_balance) = CYCLES_ORIGIN.with(|c| *c.borrow());
    let (latest_time, latest_balance) = CYCLES_LATEST.with(|c| *c.borrow());
    // Tick-to-tick, never tick-to-query: see CYCLES_LATEST for what the mixed
    // comparison reported and why it was wrong in the dangerous direction.
    let window_secs = latest_time.saturating_sub(origin_time) / 1_000_000_000;
    let burned = origin_balance.saturating_sub(latest_balance);
    let meaningful = origin_time > 0 && window_secs >= CYCLE_SAMPLE_MIN_SECS && burned > 0;

    let per_day = if meaningful {
        burned.saturating_mul(86_400) / (window_secs as u128).max(1)
    } else {
        0
    };
    let runway_days = if per_day > 0 {
        Some((liquid / per_day) as u64)
    } else {
        None
    };

    let next_wake_at = TABLE.with(|t| {
        t.borrow()
            .as_ref()
            .and_then(|s| next_wake_deadline(s, now))
    });

    CycleStatus {
        balance,
        liquid_balance: liquid,
        reserved_for_freezing: balance.saturating_sub(liquid),
        observed_burn_per_day: per_day,
        runway_days,
        sample_window_secs: window_secs,
        measurement_is_meaningful: meaningful,
        clock_ticks: CLOCK_TICKS.with(|c| *c.borrow()),
        clock_last_tick_at: CLOCK_LAST_TICK_AT.with(|c| *c.borrow()),
        clock_watchdog_armed: CLOCK_WATCHDOG.with(|w| w.borrow().is_some()),
        next_wake_at,
    }
}

#[cfg(test)]
mod clock_schedule_tests {
    use super::*;

    const SEC: u64 = 1_000_000_000;
    const NOW: u64 = 1_000 * SEC;

    fn bare_table(phase: GamePhase) -> TableState {
        TableState {
            id: 0,
            config: TableConfig {
                small_blind: 1,
                big_blind: 2,
                min_buy_in: 10,
                max_buy_in: 1000,
                max_players: 6,
                action_timeout_secs: 30,
                ante: 0,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            players: (0..6).map(|_| None).collect(),
            community_cards: Vec::new(),
            deck: Vec::new(),
            deck_index: 0,
            pot: 0,
            side_pots: Vec::new(),
            current_bet: 0,
            min_raise: 2,
            phase,
            dealer_seat: 0,
            small_blind_seat: 0,
            big_blind_seat: 1,
            action_on: 0,
            action_timer: None,
            shuffle_proof: None,
            hand_number: 1,
            last_aggressor: None,
            bb_has_option: false,
            first_hand: false,
            auto_deal_at: None,
            last_action: None,
            departed_stakes: None,
        }
    }

    fn seat(chips: u64, status: PlayerStatus, last_seen: u64) -> Player {
        Player {
            principal: Principal::anonymous(),
            seat: 0,
            chips,
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: 0,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: false,
            status,
            last_seen,
            timeout_count: 0,
            time_bank_remaining: 0,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since: None,
        }
    }

    #[test]
    fn an_empty_table_has_nothing_to_wake_for() {
        let st = bare_table(GamePhase::WaitingForPlayers);
        assert_eq!(next_wake_deadline(&st, NOW), None);
    }

    #[test]
    fn the_action_clock_is_scheduled_one_nanosecond_past_expiry() {
        // `resolve_expired_action_timer` fires on `now > expires_at`, so waking AT
        // the expiry would do nothing and re-arm, which is the spin loop.
        let mut st = bare_table(GamePhase::PreFlop);
        st.action_timer = Some(ActionTimer {
            player_seat: 0,
            started_at: NOW,
            expires_at: NOW + 30 * SEC,
            using_time_bank: false,
        });
        assert_eq!(next_wake_deadline(&st, NOW), Some(NOW + 30 * SEC + 1));
    }

    #[test]
    fn the_earliest_deadline_wins() {
        let mut st = bare_table(GamePhase::PreFlop);
        st.action_timer = Some(ActionTimer {
            player_seat: 0,
            started_at: NOW,
            expires_at: NOW + 30 * SEC,
            using_time_bank: false,
        });
        // An Active seat last seen 80 s ago is 10 s from the disconnect threshold,
        // which is sooner than the action clock.
        st.players[1] = Some(seat(
            100,
            PlayerStatus::Active,
            NOW - (DISCONNECT_TIMEOUT_SECS - 10) * SEC,
        ));
        assert_eq!(
            next_wake_deadline(&st, NOW),
            Some(NOW + 10 * SEC + 1),
            "the schedule must be the MINIMUM of every pending deadline, not the first one found"
        );
    }

    /// **THE ANTI-SPIN PROPERTY.** Crossing `auto_deal_at` on a table that CAN deal
    /// changes nothing: `advance_table_clock` returns `AutoDealReady` and leaves the
    /// field set, so the same deadline is due again immediately. Scheduling it would
    /// re-arm at the floor forever, burning ~15.4M cycles a second on a table that is
    /// merely waiting for a client to call `start_new_hand`.
    #[test]
    fn auto_deal_is_not_scheduled_when_the_table_could_actually_deal() {
        let mut st = bare_table(GamePhase::HandComplete);
        st.players[0] = Some(seat(100, PlayerStatus::Active, NOW));
        st.players[1] = Some(seat(100, PlayerStatus::Active, NOW));
        st.auto_deal_at = Some(NOW + 3 * SEC);
        assert_eq!(
            next_wake_deadline(&st, NOW),
            Some(NOW + DISCONNECT_TIMEOUT_SECS * SEC + 1),
            "the only thing due here is the disconnect threshold; auto-deal must not be scheduled"
        );
    }

    /// The other half of the same rule: when crossing it CLEARS the field, it is a
    /// real one-time state change and must be scheduled.
    #[test]
    fn auto_deal_is_scheduled_when_crossing_it_would_clear_it() {
        let mut st = bare_table(GamePhase::HandComplete);
        st.players[0] = Some(seat(100, PlayerStatus::Active, NOW));
        st.auto_deal_at = Some(NOW + 3 * SEC);
        assert_eq!(next_wake_deadline(&st, NOW), Some(NOW + 3 * SEC));
    }

    /// A deadline ALREADY in the past is not scheduled, and that is deliberate.
    ///
    /// An action clock that is past its expiry and still there means the last
    /// attempt to resolve it did not commit -- on this canister, that means it
    /// TRAPPED. Re-arming at the one-second floor would retry it once a second
    /// forever at ~15.4M cycles a go. The watchdog retries it every 30 s instead,
    /// which is the right pacing for a retry, and `clock_should_abandon` refunds
    /// the hand once the grace period is up.
    #[test]
    fn an_already_missed_deadline_is_left_to_the_watchdog_rather_than_spun_on() {
        let mut st = bare_table(GamePhase::PreFlop);
        st.action_timer = Some(ActionTimer {
            player_seat: 0,
            started_at: NOW - 100 * SEC,
            expires_at: NOW - 70 * SEC,
            using_time_bank: false,
        });
        // The expiry is past; the stuck-hand threshold is still ahead, and that is
        // the only thing left to aim at.
        assert_eq!(
            next_wake_deadline(&st, NOW),
            Some(NOW - 70 * SEC + STUCK_HAND_GRACE_NS + 1)
        );
    }

    /// The clock's automatic refund is strictly narrower than the manual one.
    #[test]
    fn the_clock_refuses_the_no_clock_branch_that_abandon_stuck_hand_accepts() {
        let st = bare_table(GamePhase::Flop); // live hand, no action timer at all
        assert!(
            hand_is_stuck(&st, NOW),
            "the manual door stays open for a live hand with no clock"
        );
        assert!(
            !clock_should_abandon(&st, NOW),
            "the automatic door must NOT act on the branch whose truth conditions are not \
             fully understood; see clock_should_abandon"
        );
    }

    #[test]
    fn the_clock_takes_the_expired_clock_branch_after_the_grace_period() {
        let mut st = bare_table(GamePhase::Turn);
        st.action_timer = Some(ActionTimer {
            player_seat: 0,
            started_at: 0,
            expires_at: NOW,
            using_time_bank: false,
        });
        assert!(!clock_should_abandon(&st, NOW + STUCK_HAND_GRACE_NS));
        assert!(clock_should_abandon(&st, NOW + STUCK_HAND_GRACE_NS + 1));
    }

    #[test]
    fn a_finished_hand_is_never_abandonable_by_the_clock() {
        for phase in [GamePhase::WaitingForPlayers, GamePhase::HandComplete] {
            let mut st = bare_table(phase);
            st.action_timer = Some(ActionTimer {
                player_seat: 0,
                started_at: 0,
                expires_at: 0,
                using_time_bank: false,
            });
            assert!(!clock_should_abandon(&st, u64::MAX));
        }
    }
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

/// Sit out (voluntarily).
///
/// **You cannot sit out of a hand you are already in.** This used to set
/// `PlayerStatus::SittingOut` unconditionally, with no phase check, which was
/// docs/DEFECTS.md E-32 with no waiting at all: one call, from a player facing a
/// bet they did not like, took them out of the betting round while leaving them
/// holding cards and eligible for every pot layer they had already paid into.
/// Reproduced on the local `table_2`, hand 5: the seat that called this was never
/// asked to match a 2 ICP bet, kept the chips, and was still `has_folded = false`
/// with cards at the showdown.
///
/// Real rooms defer it for the same reason: "sit out" takes effect between hands,
/// and the hand you are in you must finish -- by acting, or by letting your clock
/// run out, which folds you. So a request made mid-hand becomes
/// [`sit_out_next_hand`], and the caller stays in the betting round they are
/// already in.
///
/// A player who is not in the current hand (folded, never dealt in, or there is no
/// hand) sits out immediately, as before.
#[ic_cdk::update]
fn sit_out() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();

    TABLE.with(|t| {
        let mut table = t.borrow_mut();
        let state = table.as_mut().ok_or("Table not initialized")?;

        let hand_is_live = state.phase != GamePhase::WaitingForPlayers
            && state.phase != GamePhase::HandComplete;

        for player in state.players.iter_mut().flatten() {
            if player.principal == caller {
                if hand_is_live && is_in_hand(player) {
                    player.is_sitting_out_next_hand = true;
                    return Ok(());
                }
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
                        .filter(|p| p.as_ref().map(will_be_dealt_in).unwrap_or(false))
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

    let out = TABLE.with(|t| {
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
    });
    // The clock this just extended is the one the on-chain wake is aimed at.
    if out.is_ok() {
        schedule_next_wake();
    }
    out
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
            // FINDING 18. Computed for the CALLER, not for `my_seat`: a player
            // who has left still has a stake in the hand and no seat at all, and
            // that is precisely the case every other field here goes blank for.
            my_committed_in_pot: committed_stake_of(state, caller),
            hand_is_unmovable: hand_is_stuck(state, now),
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

// ============================================================================
// THE COMMITMENT CHECK
// ============================================================================
//
// This used to be `verify_shuffle : (text, text) -> (bool) query`, and an
// independent auditor called it "worse than having no endpoint"
// (docs/DEFECTS.md E-44). Three separate faults, and the third is the one that
// matters:
//
//   1. The Candid carried no parameter names, so nothing on the wire said which
//      of the two hex strings was the hash.
//   2. The return was a bare `bool`, so "you passed them backwards" and "the
//      table revealed a seed it never committed to" were the SAME ANSWER.
//   3. Both strings are 64 lowercase hex characters for a real hand, so a
//      reader cannot tell them apart by looking, and the natural reading order
//      -- seed first, then hash -- was the order that returned `false`.
//
// The person most likely to call this is someone who already suspects they were
// cheated. Handing that person a `false` they cannot interpret manufactures an
// accusation out of a typo.
//
// What is here now:
//
//   * `check_shuffle_commitment` takes a RECORD with named fields. Candid keys
//     record fields by name, not position, so the arguments cannot be
//     transposed on the wire at all.
//   * It answers with a VARIANT that names which check failed, and carries both
//     hashes so the caller can see the arithmetic rather than trust the verdict.
//   * If the values are put in each other's fields it says so, in an arm that
//     is explicitly NOT a failed proof, and still shows the match.
//   * Every arm carries `this_proves` and `this_does_not_prove` in plain text,
//     because the honest verification is the one that runs on the player's
//     machine and this one cannot be allowed to impersonate it.
//
// `verify_shuffle` is kept, order-insensitive, purely so nobody who already
// wrote the old call gets a false accusation out of it.

/// Arguments for `check_shuffle_commitment`.
///
/// A record, not two positional strings, precisely so the order cannot be got
/// wrong: Candid matches record fields by hashed name.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CommitmentCheckArgs {
    /// The `seed_hash` from the hand's `ShuffleProof`: 64 lowercase hex chars.
    pub seed_hash: String,
    /// The `revealed_seed` from the same `ShuffleProof`: hex, 32 bytes in a live hand.
    pub revealed_seed: String,
}

/// A commitment that checks out, whichever field the caller put it in.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CommitmentMatch {
    /// The value that behaved like the committed hash.
    pub committed_hash: String,
    /// The value that behaved like the revealed seed.
    pub revealed_seed: String,
    /// `SHA256(revealed_seed)`, recomputed here. Equal to `committed_hash`.
    pub computed_hash: String,
    pub this_proves: String,
    pub this_does_not_prove: String,
}

/// The two values are hex, and neither reading of them is a commit-reveal pair.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CommitmentNoMatch {
    /// What was passed in the `seed_hash` field.
    pub seed_hash_field: String,
    /// What was passed in the `revealed_seed` field.
    pub revealed_seed_field: String,
    /// `SHA256(revealed_seed_field)` -- the documented reading.
    pub computed_from_revealed_seed: String,
    /// `SHA256(seed_hash_field)` -- the swapped reading, shown so the caller can
    /// see that BOTH readings were tried before this answer was given.
    pub computed_from_seed_hash: String,
    pub meaning: String,
}

/// One of the two fields is not a hex string this canister can read.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct CommitmentMalformed {
    /// `"seed_hash"` or `"revealed_seed"` -- the field at fault, by name.
    pub field: String,
    pub reason: String,
    pub character_length: u64,
}

/// The answer to "is this seed the one that hash commits to?", with the reason.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum CommitmentCheck {
    /// `SHA256(revealed_seed) == seed_hash`, values in the fields named for them.
    Match(CommitmentMatch),
    /// The pair IS a valid commit-reveal pair, but the caller put each value in
    /// the other's field. This is a note about the call, NOT a failed proof.
    FieldsSwapped(CommitmentMatch),
    /// Both values parsed, neither reading matches. This is the only answer that
    /// is evidence of anything wrong with the hand.
    NoMatch(CommitmentNoMatch),
    /// The call could not be evaluated. Says which field and why.
    Malformed(CommitmentMalformed),
}

const COMMITMENT_PROVES: &str =
    "Only that the revealed seed is the pre-image of the committed hash. This canister \
     re-hashed 32 bytes it was handed; it did not look at a card, a hand or a pot.";

const COMMITMENT_DOES_NOT_PROVE: &str =
    "Nothing about fairness. This is the table checking its own homework on the table's own \
     machine. The verification that counts re-derives all 52 cards from the seed on YOUR \
     machine -- docs/SHUFFLE-SPEC.md, or src/poker_core/tests/verify/verify_shuffle.{py,mjs}. \
     A rigged table can answer Match all day; what it cannot do is make the deck you \
     re-derive contain the cards you were shown.";

/// Normalise one field: trim, lowercase, and refuse anything that is not hex.
fn read_hex_field(field: &str, value: &str) -> Result<(String, Vec<u8>), CommitmentMalformed> {
    let text = value.trim().to_lowercase();
    let malformed = |reason: &str| CommitmentMalformed {
        field: field.to_string(),
        reason: reason.to_string(),
        character_length: text.chars().count() as u64,
    };
    if text.is_empty() {
        return Err(malformed(
            "empty. Copy this value out of the hand's ShuffleProof; both fields are required.",
        ));
    }
    // Character set BEFORE parity, or a principal (27 chars, hyphens) is
    // reported as "an odd number of hex characters", which sends the caller
    // looking for a truncated paste instead of a pasted-the-wrong-thing.
    if !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(malformed(
            "not hexadecimal. Expected only the characters 0-9 and a-f. A principal, a hand \
             number, a quoted string or a value copied with surrounding punctuation lands here.",
        ));
    }
    if text.len() % 2 != 0 {
        return Err(malformed(
            "an odd number of hex characters, so it cannot be a whole number of bytes. \
             A truncated copy-paste does this.",
        ));
    }
    match hex::decode(&text) {
        Ok(bytes) => Ok((text, bytes)),
        Err(_) => Err(malformed(
            "not hexadecimal. Expected only the characters 0-9 and a-f.",
        )),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Check a commit-reveal pair and say WHICH check failed.
///
/// See the block comment above for why this replaced a `(text, text) -> bool`.
/// It is a query, it reads no table state, and its answer is worth exactly what
/// the `this_does_not_prove` field says it is worth.
#[ic_cdk::query]
fn check_shuffle_commitment(args: CommitmentCheckArgs) -> CommitmentCheck {
    let (hash_field, hash_field_bytes) = match read_hex_field("seed_hash", &args.seed_hash) {
        Ok(v) => v,
        Err(e) => return CommitmentCheck::Malformed(e),
    };
    let (seed_field, seed_field_bytes) = match read_hex_field("revealed_seed", &args.revealed_seed)
    {
        Ok(v) => v,
        Err(e) => return CommitmentCheck::Malformed(e),
    };

    // The documented reading: the `revealed_seed` field holds the seed.
    let hash_of_seed_field = sha256_hex(&seed_field_bytes);
    if hash_of_seed_field == hash_field {
        return CommitmentCheck::Match(CommitmentMatch {
            committed_hash: hash_field,
            revealed_seed: seed_field,
            computed_hash: hash_of_seed_field,
            this_proves: COMMITMENT_PROVES.to_string(),
            this_does_not_prove: COMMITMENT_DOES_NOT_PROVE.to_string(),
        });
    }

    // The transposed reading. A SHA-256 two-cycle would make both readings hold
    // at once; none is known, and the documented reading is answered first, so
    // this arm can only be reached when the documented one did not hold.
    let hash_of_hash_field = sha256_hex(&hash_field_bytes);
    if hash_of_hash_field == seed_field {
        return CommitmentCheck::FieldsSwapped(CommitmentMatch {
            committed_hash: seed_field,
            revealed_seed: hash_field,
            computed_hash: hash_of_hash_field,
            this_proves: COMMITMENT_PROVES.to_string(),
            this_does_not_prove: COMMITMENT_DOES_NOT_PROVE.to_string(),
        });
    }

    CommitmentCheck::NoMatch(CommitmentNoMatch {
        seed_hash_field: hash_field,
        revealed_seed_field: seed_field,
        computed_from_revealed_seed: hash_of_seed_field,
        computed_from_seed_hash: hash_of_hash_field,
        meaning:
            "Neither value hashes to the other, so these two strings are not a commit-reveal \
             pair. Before concluding anything about the table, check that both were copied from \
             the SAME hand's ShuffleProof: a hash from one hand and a seed from another lands \
             here, and so does a truncated paste. If they are from one hand, the table revealed \
             a seed it did not commit to, and that IS a broken proof -- please report it."
                .to_string(),
    })
}

/// Deprecated. Kept only so a caller who already wrote the old two-string call
/// cannot be handed a false accusation by it.
///
/// It now answers the same question in EITHER argument order, so a `false` can
/// no longer mean "you called it backwards". A `false` still cannot distinguish
/// a malformed argument from a broken proof, which is exactly why this is
/// deprecated: use `check_shuffle_commitment`, which names both.
#[ic_cdk::query]
fn verify_shuffle(seed_hash: String, revealed_seed: String) -> bool {
    matches!(
        check_shuffle_commitment(CommitmentCheckArgs {
            seed_hash,
            revealed_seed,
        }),
        CommitmentCheck::Match(_) | CommitmentCheck::FieldsSwapped(_)
    )
}

#[cfg(test)]
mod commitment_check_tests {
    //! The one property that matters here is NEGATIVE: no honest caller can get
    //! an accusation out of this endpoint by making a mistake. Each test below
    //! is a mistake a real person makes with two 64-character hex strings.

    use super::*;

    /// A real commit-reveal pair from a hand `table_1` dealt on the local
    /// replica. Kept verbatim so this test is pinned to something the engine
    /// actually produced, not to something the test computed for itself.
    const SEED: &str = "2b8a1a4ab2910e4940ee5d3800ef1289fcd0b3436b926adc387d37d0c184235a";
    const HASH: &str = "41779ced2453b2e910c1ac0cc994cc9486f7f00dead6dae79fcf2eac40b7c9a8";

    fn check(seed_hash: &str, revealed_seed: &str) -> CommitmentCheck {
        check_shuffle_commitment(CommitmentCheckArgs {
            seed_hash: seed_hash.to_string(),
            revealed_seed: revealed_seed.to_string(),
        })
    }

    #[test]
    fn the_fixture_really_is_a_commit_reveal_pair() {
        assert_eq!(sha256_hex(&hex::decode(SEED).unwrap()), HASH);
    }

    #[test]
    fn the_documented_field_placement_matches() {
        match check(HASH, SEED) {
            CommitmentCheck::Match(m) => {
                assert_eq!(m.computed_hash, HASH);
                assert_eq!(m.committed_hash, HASH);
                assert_eq!(m.revealed_seed, SEED);
                assert!(!m.this_does_not_prove.is_empty());
            }
            other => panic!("expected Match, got {other:?}"),
        }
    }

    #[test]
    fn transposing_the_two_values_is_reported_as_a_transposition_not_a_failure() {
        // The whole finding. `verify_shuffle(seed, hash)` used to answer `false`
        // here, and `false` reads as "you were cheated". docs/DEFECTS.md E-44.
        match check(SEED, HASH) {
            CommitmentCheck::FieldsSwapped(m) => {
                assert_eq!(m.committed_hash, HASH, "the hash must still be identified as the hash");
                assert_eq!(m.revealed_seed, SEED);
            }
            other => panic!("expected FieldsSwapped, got {other:?}"),
        }
    }

    #[test]
    fn the_deprecated_bool_cannot_manufacture_an_accusation() {
        assert!(verify_shuffle(HASH.to_string(), SEED.to_string()));
        assert!(verify_shuffle(SEED.to_string(), HASH.to_string()));
    }

    #[test]
    fn case_and_surrounding_whitespace_are_a_copy_paste_artefact_not_a_verdict() {
        assert!(matches!(
            check(&format!("  {}  ", HASH.to_uppercase()), &format!("\n{SEED}\t")),
            CommitmentCheck::Match(_)
        ));
    }

    #[test]
    fn a_genuine_mismatch_is_still_reported_and_shows_both_readings() {
        let other_seed = "951ca24e5324ddefaddf82ecb62f5aa773ef5222e09fff7143ef34dcf4511395";
        match check(HASH, other_seed) {
            CommitmentCheck::NoMatch(n) => {
                assert_eq!(n.computed_from_revealed_seed, sha256_hex(&hex::decode(other_seed).unwrap()));
                assert_eq!(n.computed_from_seed_hash, sha256_hex(&hex::decode(HASH).unwrap()));
                assert_ne!(n.computed_from_revealed_seed, n.seed_hash_field);
            }
            other => panic!("expected NoMatch, got {other:?}"),
        }
    }

    #[test]
    fn a_non_hex_field_is_named_as_non_hex_not_as_truncated() {
        // A principal is 27 characters, an odd number. Reporting "an odd number
        // of hex characters" would send the caller hunting for a truncated
        // paste, so the character-set check runs first.
        match check("4fbx2-kt777-77775-aaabq-cai", SEED) {
            CommitmentCheck::Malformed(m) => {
                assert_eq!(m.field, "seed_hash");
                assert!(m.reason.contains("not hexadecimal"), "reason was: {}", m.reason);
            }
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn a_truncated_paste_is_named_as_a_truncated_paste() {
        match check(HASH, &SEED[..17]) {
            CommitmentCheck::Malformed(m) => {
                assert_eq!(m.field, "revealed_seed");
                assert_eq!(m.character_length, 17);
                assert!(m.reason.contains("odd number"), "reason was: {}", m.reason);
            }
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_field_says_which_one_is_empty() {
        match check("", SEED) {
            CommitmentCheck::Malformed(m) => assert_eq!(m.field, "seed_hash"),
            other => panic!("expected Malformed, got {other:?}"),
        }
        match check(HASH, "") {
            CommitmentCheck::Malformed(m) => assert_eq!(m.field, "revealed_seed"),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn every_answer_that_could_be_read_as_a_verdict_carries_its_own_disclaimer() {
        for c in [check(HASH, SEED), check(SEED, HASH)] {
            let (proves, not) = match c {
                CommitmentCheck::Match(m) | CommitmentCheck::FieldsSwapped(m) => {
                    (m.this_proves, m.this_does_not_prove)
                }
                other => panic!("expected a matching arm, got {other:?}"),
            };
            assert!(proves.contains("pre-image"), "this_proves lost its meaning: {proves}");
            assert!(
                not.contains("Nothing about fairness"),
                "an answer that reads as a fairness verdict must say it is not one: {not}"
            );
        }
    }
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
    /// Deposit replay-protection floor.
    ///
    /// `opt`, and it must stay `opt` (docs/SECURITY-FINDINGS.md FINDING 14). This
    /// shipped as a bare `nat64` with a `#[serde(default)]` and a comment claiming
    /// that made it backward-compatible. **Candid does not honour serde defaults.**
    /// Only `opt`, `reserved` and `null` may be added to a record and still read
    /// state written before the field existed, so as a bare `nat64` at the TOP level
    /// of the persisted record it made the whole `stable_restore` fail with
    /// `Subtyping error: field deposit_watermark is not optional field`, and the
    /// deposit fix -- the fix for the only proven fund theft in this project --
    /// could not be deployed by upgrade at all.
    ///
    /// `None` means "state written before this field existed", which restores as a
    /// floor of 0. That is the correct reading: at that point nothing had been
    /// dropped from `verified_deposits`, so no block index was below the floor.
    #[serde(default)]
    deposit_watermark: Option<u64>,
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
    // That has already happened once in this file: `TableState::departed_stakes`
    // shipped as a bare `vec`, masked only because `deposit_watermark` was a bare
    // `nat64` at the TOP level, which failed the whole restore and got the upgrade
    // rejected instead. Both are `opt` as of this change, and the cross-version
    // upgrade test (`tests/money_safety/tests/invariants/upgrade_across_versions.rs`)
    // goes RED if either one regresses.
    //
    // An `opt` wrapping a flat scalar is the one shape that cannot itself be
    // silently dropped, so these stay readable when `table_state` does not, and
    // `post_upgrade` refuses the upgrade when they disagree with what came back.
    //
    // THIS IS A GUARD RAIL, NOT THE FIX, and it is kept because it protects fields
    // that do not exist yet: it turns the NEXT non-`opt` addition to `TableState`
    // from silent chip destruction into a rejected upgrade, for every upgrade from
    // this commit onwards. It cannot fire for state written before the digest
    // existed, which is why the cross-version test, not the guard, is the gate.
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

/// Write everything the canister owes into stable memory.
///
/// # This TRAPS if the save fails, and that is the whole point
///
/// It used to log `CRITICAL: Failed to save state to stable memory` and let the
/// upgrade proceed, with a comment arguing that trapping "could brick the
/// canister". That reasoning is backwards on a canister that custodies funds:
///
/// * A trap in `pre_upgrade` ABORTS the upgrade. The old code keeps running with
///   its heap intact and nothing is lost -- the canister is not bricked, the
///   *upgrade* is refused, which is a state a human can act on.
/// * Proceeding after a failed save has exactly two outcomes, and both are worse.
///   If stable memory is empty, `post_upgrade`'s `stable_restore` fails and it
///   panics anyway -- the same refusal, minus the accurate reason. If stable memory
///   still holds an OLDER snapshot from a previous upgrade, `stable_restore`
///   SUCCEEDS and the canister silently rolls back to it: every escrow balance,
///   every chip and every hand since that snapshot is gone, and worse,
///   `verified_deposits` and `deposit_watermark` roll back with it, which re-opens
///   the E-02 replay window on ledger blocks that were already credited. A silent
///   rollback of the anti-replay record is a fund-theft primitive.
///
/// An upgrade that proceeds after failing to save is how state is lost. So it does
/// not proceed.
#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let digest = table_digest();
    let state = PersistentState {
        balances: BALANCES.with(|b| b.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        verified_deposits: VERIFIED_DEPOSITS.with(|v| v.borrow().iter().map(|(k, v)| (*k, *v)).collect()),
        deposit_watermark: Some(deposit_watermark()),
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

    let escrow_at_save = state
        .balances
        .iter()
        .fold(0u64, |a, (_, v)| a.saturating_add(*v));

    if let Err(e) = ic_cdk::storage::stable_save((state,)) {
        // See the doc comment above: refuse the UPGRADE rather than proceed with an
        // unsaved or stale snapshot. The trap rolls the message back; the running
        // canister and its heap are untouched.
        ic_cdk::trap(&format!(
            "CRITICAL: failed to save state to stable memory: {:?}. Upgrade REFUSED rather \
             than proceeding with an unsaved snapshot -- proceeding would either be rejected \
             by post_upgrade anyway or silently restore an OLDER snapshot, rolling back \
             {} e8s of escrow, every chip at the table, and the deposit anti-replay record. \
             The old code is still running and nothing has been lost. \
             See docs/SECURITY-FINDINGS.md FINDING 14.",
            e, escrow_at_save
        ));
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
    //
    // `None` means the saved state predates the field, so the floor is 0: nothing
    // had been dropped from `verified_deposits` at that point, so no block index
    // was below the floor. The `>` keeps this monotonic -- a restore can only ever
    // RAISE the floor, never lower it -- so no decode outcome can re-open a
    // window that was already closed (docs/DEFECTS.md E-02).
    let saved_watermark = state.deposit_watermark.unwrap_or(0);
    DEPOSIT_WATERMARK.with(|w| {
        let mut w = w.borrow_mut();
        if saved_watermark > *w {
            *w = saved_watermark;
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

    // ------------------------------------------------------------------------
    // RE-ARM THE CLOCK. LAST, AND NOT OPTIONAL.
    //
    // Timers are not persisted across canister upgrades: the CDK's task queue is
    // heap, and the system's global-timer field is cleared. An upgrade that skips
    // this leaves a canister that still holds every player's money, still holds a
    // live hand, and has silently lost the only thing on chain that can move it --
    // and nothing about it looks wrong from outside. `get_cycle_status().clock_ticks`
    // staying at 0 is how you would find out; `tests/money_safety/tests/timers.rs`
    // is how this build proves it does not happen.
    //
    // Last, so it schedules against the state that was actually restored above.
    // ------------------------------------------------------------------------
    start_clock();
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
            departed_stakes: None,
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
        let stale_hand = state.hand_number - 1;
        state.departed_stakes_mut().push(DepartedStake {
            hand_number: stale_hand,
            seat: 4,
            principal: principal(4),
            contributed: 1_000_000,
        });
        let plan = plan_payouts(&state);
        assert_eq!(plan.collected, 40, "the stale stake is not in the basis");
        assert!(plan.conserves());
    }

    // -----------------------------------------------------------------------
    // FINDING 13: the payee is the OWNER OF THE STAKE, never the chair
    //
    // Every assertion in this section is on a PRINCIPAL. That is the point: the
    // defect these pin conserved every chip, awarded exactly what it collected,
    // paid the right AMOUNT to the right SEAT, and gave one player's money to
    // another. Nothing that compares totals or seats can see it.
    // -----------------------------------------------------------------------

    /// Build the FINDING 13 state: seat 1's player left mid-hand with `stake` in the
    /// pot, and a DIFFERENT principal has since taken that chair.
    ///
    /// `alice` is `principal(1)` -- the seat's original occupant, whose money this
    /// is. The stranger is `principal(9)`, who was never dealt in: `join_table`
    /// seats a mid-hand arrival `SittingOut` with no hole cards.
    fn departed_seat_retaken(stake: u64) -> (TableState, Principal, Principal) {
        let alice = principal(1);
        let stranger = principal(9);
        // Only seat 1 has money in, and it is a departed stake, so no seat at the
        // table has a live claim on any layer: the refund branch.
        let mut state = table(vec![], "Kd Tc 3d 2c Ad", 0);
        state.pot = stake;
        let hand = state.hand_number;
        state.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 1,
            principal: alice,
            contributed: stake,
        });
        state.players[1] = Some(Player {
            principal: stranger,
            seat: 1,
            chips: 7,
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: 0,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: false,
            status: PlayerStatus::SittingOut,
            last_seen: 0,
            timeout_count: 0,
            time_bank_remaining: 30,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since: Some(0),
        });
        (state, alice, stranger)
    }

    /// THE REPRODUCER. A departed player's stake is owed to that player, and the
    /// plan must NAME that player, not whoever is now in the chair.
    ///
    /// This is the assertion the whole class of defect turns on, and it is stated on
    /// principals because every seat-level and total-level assertion passes either
    /// way: note `plan.conserves()` below.
    #[test]
    fn finding13_a_departed_stake_is_planned_for_its_owner_not_for_the_new_occupant() {
        let (state, alice, stranger) = departed_seat_retaken(50);
        let plan = plan_payouts(&state);

        assert!(
            plan.conserves(),
            "the plan must still pay out exactly what it collected: {} of {}",
            plan.awarded,
            plan.collected
        );
        assert_eq!(plan.amount_for(1), 50, "seat 1 is owed the whole stake");

        let named: Vec<Principal> = plan.payouts.iter().map(|p| p.principal).collect();
        assert_eq!(
            named,
            vec![alice],
            "the stake belongs to {alice}; the plan named {named:?}. The chair is occupied by \
             {stranger}, who was never dealt in. See docs/SECURITY-FINDINGS.md FINDING 13."
        );
        assert_eq!(plan.amount_for_principal(alice), 50);
        assert_eq!(
            plan.amount_for_principal(stranger),
            0,
            "{stranger} put nothing into this hand and must be paid nothing"
        );
    }

    /// And applying that plan must move the money to alice, not into the stranger's
    /// stack. Measured on BALANCES (escrow) and on the stranger's chips.
    #[test]
    fn finding13_applying_the_plan_credits_the_owners_escrow_and_not_the_strangers_stack() {
        let (mut state, alice, stranger) = departed_seat_retaken(50);
        BALANCES.with(|b| b.borrow_mut().clear());
        let stranger_chips_before = chips_at(&state, 1);

        let plan = plan_payouts(&state);
        let winners = apply_payouts(&mut state, &plan);

        let escrow = |who: Principal| BALANCES.with(|b| b.borrow().get(&who).copied().unwrap_or(0));
        assert_eq!(
            escrow(alice),
            50,
            "alice ({alice}) left the table, so her stake must reach her ESCROW"
        );
        assert_eq!(
            escrow(stranger),
            0,
            "the stranger ({stranger}) must not be credited anything"
        );
        assert_eq!(
            chips_at(&state, 1),
            stranger_chips_before,
            "the stranger's stack must not change: it was {stranger_chips_before} and the \
             defect made it {stranger_chips_before} + 50"
        );

        // The record has to name the right person too, or the hand history and the
        // frontend both attribute alice's money to the stranger.
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].principal, alice);
        assert_eq!(winners[0].seat, 1);
        assert_eq!(winners[0].amount, 50);
        assert!(
            winners[0].cards.is_none() && winners[0].hand_rank.is_none(),
            "a refund is not a win and must never carry the occupant's cards"
        );
        BALANCES.with(|b| b.borrow_mut().clear());
    }

    /// The second variant from the finding: TWO departed stakes at ONE seat, owed to
    /// TWO DIFFERENT principals. Resolving the owner from the seat returned the
    /// first match for both, so the second player's money went to the first player.
    #[test]
    fn finding13_two_departed_stakes_at_one_seat_each_reach_their_own_owner() {
        let alice = principal(1);
        let bob = principal(2);
        let mut state = table(vec![], "Kd Tc 3d 2c Ad", 0);
        state.pot = 30;
        let hand = state.hand_number;
        state.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 1,
            principal: alice,
            contributed: 10,
        });
        state.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 1,
            principal: bob,
            contributed: 20,
        });

        let plan = plan_payouts(&state);
        assert!(plan.conserves(), "{} of {}", plan.awarded, plan.collected);
        assert_eq!(
            plan.amount_for_principal(alice),
            10,
            "alice put in 10 and must get 10 back, not 0 and not 30"
        );
        assert_eq!(
            plan.amount_for_principal(bob),
            20,
            "bob put in 20 and must get 20 back"
        );

        BALANCES.with(|b| b.borrow_mut().clear());
        apply_payouts(&mut state, &plan);
        let escrow = |who: Principal| BALANCES.with(|b| b.borrow().get(&who).copied().unwrap_or(0));
        assert_eq!((escrow(alice), escrow(bob)), (10, 20));
        BALANCES.with(|b| b.borrow_mut().clear());
    }

    /// `hand_stakes` builds the seated stakes itself so that each owner comes from
    /// the same `Player` the amount came from. That duplicates the SELECTION rule
    /// `collect_contributions` states, and duplicated rules drift, so this pins
    /// that they agree on every seated stake.
    #[test]
    fn collect_contributions_and_hand_stakes_select_the_same_seated_stakes() {
        let mut state = table(
            vec![
                seat(0, 100, 20, "As 8d"),
                folded_seat(1, 0, 35),
                seat(3, 100, 200, "7s Th"),
            ],
            "Kd Tc 3d 2c Ad",
            1,
        );
        // A departed stake as well, which `collect_contributions` cannot see: it is
        // the difference between the two, and the only difference.
        let hand = state.hand_number;
        state.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 4,
            principal: principal(4),
            contributed: 55,
        });

        let from_seats = collect_contributions(&state.players);
        let seated_stakes: Vec<Contribution> = hand_stakes(&state)
            .iter()
            .filter(|s| state.players[s.seat as usize].is_some())
            .map(|s| Contribution::new(s.seat, s.amount, s.relinquished))
            .collect();
        assert_eq!(
            seated_stakes, from_seats,
            "hand_stakes and collect_contributions must select the same seated stakes"
        );
        assert_eq!(
            hand_stakes(&state).len(),
            from_seats.len() + 1,
            "the departed stake is the only thing hand_stakes adds"
        );
        // And every seated stake names the player actually in that chair.
        for s in hand_stakes(&state) {
            if let Some(p) = state.players[s.seat as usize].as_ref() {
                assert_eq!(s.owner, p.principal);
            }
        }
    }

    /// A pot SHARE names the player holding the winning cards, and that is read
    /// from the live claim rather than from the seat vector, so the two can never
    /// diverge.
    #[test]
    fn finding13_a_pot_share_names_the_player_holding_the_cards() {
        let state = table(
            vec![seat(0, 100, 20, "As Ad"), seat(1, 100, 20, "2c 3d")],
            "Kd Tc 3h 7c 9d",
            0,
        );
        let plan = plan_payouts(&state);
        assert_eq!(plan.amount_for_principal(principal(0)), 40);
        assert_eq!(plan.amount_for_principal(principal(1)), 0);
        for p in &plan.payouts {
            assert_eq!(
                p.principal,
                state.players[p.seat as usize]
                    .as_ref()
                    .expect("a pot share is only ever awarded to an occupied seat")
                    .principal,
                "a pot share must name the seat's own occupant"
            );
        }
    }
}

// ============================================================================
// FUND REACHABILITY -- host tests for the two pure pieces
// ============================================================================
//
// docs/SECURITY-FINDINGS.md FINDING 15. The PocketIC gate lives in
// `tests/money_safety/tests/fund_reachability.rs` and drives the real canister
// end to end; these are the fast checks on the parts that are pure functions, so
// a mistake in either shows up in `cargo test` rather than in a 20-second
// replica run.
#[cfg(test)]
mod stuck_hand_tests {
    use super::*;

    const SEC: u64 = 1_000_000_000;

    fn table_at(phase: GamePhase, timer: Option<ActionTimer>) -> TableState {
        TableState {
            id: 0,
            config: TableConfig {
                small_blind: 1,
                big_blind: 2,
                min_buy_in: 10,
                max_buy_in: 1000,
                max_players: 6,
                action_timeout_secs: 30,
                ante: 0,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            players: (0..6).map(|_| None).collect(),
            community_cards: Vec::new(),
            deck: Vec::new(),
            deck_index: 0,
            pot: 0,
            side_pots: Vec::new(),
            current_bet: 0,
            min_raise: 2,
            phase,
            dealer_seat: 0,
            small_blind_seat: 0,
            big_blind_seat: 1,
            action_on: 0,
            action_timer: timer,
            shuffle_proof: None,
            hand_number: 1,
            last_aggressor: None,
            bb_has_option: false,
            first_hand: false,
            auto_deal_at: None,
            last_action: None,
            departed_stakes: None,
        }
    }

    fn timer_expiring_at(t: u64) -> Option<ActionTimer> {
        Some(ActionTimer {
            player_seat: 0,
            started_at: 0,
            expires_at: t,
            using_time_bank: false,
        })
    }

    /// A hand that is merely slow is NOT abandonable. This is the property that
    /// keeps `abandon_stuck_hand` from being a weapon, so it is pinned first.
    #[test]
    fn a_running_clock_is_not_a_stuck_hand() {
        let st = table_at(GamePhase::PreFlop, timer_expiring_at(30 * SEC));
        assert!(!hand_is_stuck(&st, 1 * SEC), "clock still running");
        assert!(!hand_is_stuck(&st, 31 * SEC), "just expired: check_timeouts's job");
        assert!(
            !hand_is_stuck(&st, 30 * SEC + STUCK_HAND_GRACE_NS),
            "the grace period is exclusive at its own boundary"
        );
    }

    #[test]
    fn a_clock_expired_past_the_grace_period_is_a_stuck_hand() {
        let st = table_at(GamePhase::PreFlop, timer_expiring_at(30 * SEC));
        assert!(hand_is_stuck(&st, 30 * SEC + STUCK_HAND_GRACE_NS + 1));
    }

    /// A live hand with no clock at all cannot be advanced by anything:
    /// `resolve_expired_action_timer` is the only thing that moves a hand nobody
    /// is acting on, and it does nothing without a timer. So this is stuck
    /// immediately, with no grace period, and that is a fact about the code rather
    /// than a tolerance.
    #[test]
    fn a_live_hand_with_no_clock_is_stuck_immediately() {
        assert!(hand_is_stuck(&table_at(GamePhase::Flop, None), 0));
        assert!(hand_is_stuck(&table_at(GamePhase::River, None), 0));
    }

    /// An idle table is never stuck, whatever the clock says. Otherwise every
    /// finished hand would look abandonable and `withdraw`'s guard would be
    /// permanently off.
    #[test]
    fn an_idle_table_is_never_stuck() {
        for phase in [GamePhase::WaitingForPlayers, GamePhase::HandComplete] {
            assert!(!hand_is_stuck(&table_at(phase.clone(), None), u64::MAX));
            assert!(!hand_is_stuck(
                &table_at(phase, timer_expiring_at(0)),
                u64::MAX
            ));
        }
    }

    /// The plan of last resort must give every player exactly their own money and
    /// nothing else, and it must satisfy the same post-condition a real settlement
    /// does -- otherwise `apply_payouts` would trap on it and the escape hatch
    /// would be another closed door.
    #[test]
    fn refunding_every_stake_conserves_exactly_and_pays_each_owner_their_own() {
        let mut st = table_at(GamePhase::PreFlop, None);
        let wagers = [(0u8, 25u64), (1, 100), (2, 100), (3, 7)];
        for (seat, wagered) in wagers {
            st.players[seat as usize] = Some(Player {
                principal: Principal::from_slice(&[seat + 1]),
                seat,
                chips: 500,
                hole_cards: None,
                current_bet: 0,
                total_bet_this_hand: wagered,
                // Seat 3 folded. A refund is not a payout: an abandoned hand was
                // never played, so a folded seat gets its money back too.
                has_folded: seat == 3,
                has_acted_this_round: true,
                is_all_in: false,
                status: PlayerStatus::Active,
                last_seen: 0,
                timeout_count: 0,
                time_bank_remaining: 30,
                is_sitting_out_next_hand: false,
                broke_at: None,
                sitting_out_since: None,
            });
        }
        st.pot = wagers.iter().map(|(_, w)| w).sum();

        let plan = refund_every_stake(&st);
        assert!(plan.conserves(), "apply_payouts would trap on a plan that does not");
        assert_eq!(plan.collected, 232);
        assert_eq!(plan.awarded, 232);
        assert!(plan.ranked.is_empty(), "an abandoned hand ranks nobody");
        for (seat, wagered) in wagers {
            assert_eq!(
                plan.amount_for_principal(Principal::from_slice(&[seat + 1])),
                wagered,
                "seat {seat} must get back exactly what it put in, and nothing else"
            );
        }
        assert!(
            plan.payouts
                .iter()
                .all(|p| matches!(p.reason, PayoutReason::Refund { .. })),
            "nobody wins an abandoned hand"
        );
    }

    /// It needs no board, no cards and no evaluator. That is the whole point: it is
    /// the one plan that exists for every input, including the inputs that produced
    /// FINDING 15.
    #[test]
    fn refunding_every_stake_needs_no_board_and_no_cards() {
        let mut st = table_at(GamePhase::PreFlop, None);
        st.players[0] = Some(Player {
            principal: Principal::from_slice(&[1]),
            seat: 0,
            chips: 0,
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: 5,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: true,
            status: PlayerStatus::Disconnected,
            last_seen: 0,
            timeout_count: 0,
            time_bank_remaining: 0,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since: None,
        });
        st.pot = 5;
        assert!(st.community_cards.is_empty());
        let plan = refund_every_stake(&st);
        assert!(plan.conserves());
        assert_eq!(plan.awarded, 5);
    }

    // ---------------------------------------------------------------------
    // CUSTODY VISIBILITY (docs/SECURITY-FINDINGS.md FINDING 18)
    // ---------------------------------------------------------------------
    //
    // These pin the MECHANISM the two exit doors use, at host speed, with no
    // replica and no clock. They exist because on a tree that also carries the
    // on-chain clock (FINDING 19) the replica fixture can be satisfied by the
    // clock's 30-second watchdog instead of by the exit door, and a gate that
    // another fix can satisfy is a gate that stops convicting when the code it
    // names is deleted.

    fn funded_seat(principal: u8, seat: u8, chips: u64, staked: u64) -> Player {
        Player {
            principal: Principal::from_slice(&[principal]),
            seat,
            chips,
            hole_cards: None,
            current_bet: 0,
            total_bet_this_hand: staked,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: chips == 0,
            status: PlayerStatus::Active,
            last_seen: 0,
            timeout_count: 0,
            time_bank_remaining: 0,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since: None,
        }
    }

    /// The whole of FINDING 18 in one assertion: a stake is counted for its OWNER
    /// whether or not that owner still has a chair.
    #[test]
    fn committed_stake_counts_a_stake_whose_seat_is_already_gone() {
        let alice = Principal::from_slice(&[1]);
        let bob = Principal::from_slice(&[2]);
        let mut st = table_at(GamePhase::PreFlop, timer_expiring_at(30 * SEC));
        st.players[0] = Some(funded_seat(1, 0, 100, 40));
        st.pot = 100;
        let hand = st.hand_number;
        st.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 1,
            principal: bob,
            contributed: 60,
        });

        assert_eq!(committed_stake_of(&st, alice), 40, "the seated stake");
        assert_eq!(
            committed_stake_of(&st, bob),
            60,
            "the stake of a seat that has already been vacated -- the figure every \
             caller-scoped surface used to report as zero"
        );

        // A stale entry from an earlier hand can never be counted into this one.
        st.departed_stakes_mut().push(DepartedStake {
            hand_number: hand - 1,
            seat: 2,
            principal: bob,
            contributed: 999,
        });
        assert_eq!(committed_stake_of(&st, bob), 60, "an earlier hand is not this hand");

        // And once the hand is over there is nothing committed: the money is in a
        // stack or an escrow balance, both of which the ordinary surfaces report.
        st.phase = GamePhase::HandComplete;
        assert_eq!(committed_stake_of(&st, alice), 0);
        assert_eq!(committed_stake_of(&st, bob), 0);
    }

    /// The exit doors' mechanism: every stake goes back to the principal that put
    /// it in -- INCLUDING a departed one, which lands in escrow -- and the hand is
    /// closed.
    #[test]
    fn settling_an_unmovable_hand_returns_every_stake_to_its_owner() {
        let alice = Principal::from_slice(&[1]);
        let bob = Principal::from_slice(&[2]);
        BALANCES.with(|b| b.borrow_mut().clear());

        let mut st = table_at(GamePhase::PreFlop, None);
        st.players[0] = Some(funded_seat(1, 0, 100, 40));
        let hand = st.hand_number;
        st.departed_stakes_mut().push(DepartedStake {
            hand_number: hand,
            seat: 1,
            principal: bob,
            contributed: 60,
        });
        st.pot = 100;

        let refunded = settle_unmovable_hand(&mut st, 42, UnmovableReason::AnExitDoorFoundItUnmovable)
            .expect("a conserving refund plan must be acted on");

        assert_eq!(refunded, 100, "every e8 collected is handed back");
        assert_eq!(
            st.players[0].as_ref().unwrap().chips,
            140,
            "the seated owner's stake goes back into their stack"
        );
        assert_eq!(
            BALANCES.with(|b| b.borrow().get(&bob).copied().unwrap_or(0)),
            60,
            "the departed owner's stake goes to THEIR escrow, not to whoever holds the seat"
        );
        assert_eq!(
            BALANCES.with(|b| b.borrow().get(&alice).copied().unwrap_or(0)),
            0,
            "and not to anybody else"
        );
        assert_eq!(st.pot, 0);
        assert_eq!(st.phase, GamePhase::HandComplete);
        assert!(st.departed_stakes().is_empty(), "the hand is closed out");
        BALANCES.with(|b| b.borrow_mut().clear());
    }

    /// It must never trap, because it runs INSIDE `cash_out` and `leave_table`
    /// and a trap there rolls back the exit the player was taking -- FINDING 15
    /// re-created by the fix for FINDING 18.
    ///
    /// # This test does NOT claim to have reached the guard
    ///
    /// It could not, and that is the finding it records. `refund_every_stake`
    /// derives `awarded` and `collected` from the SAME list of stakes, so
    /// `conserves()` is true by construction, and no state this test could build
    /// -- saturating stakes, two stakes at one seat, a stake with no seat, zero
    /// stakes -- made it false. Writing a test that pretends otherwise (build a
    /// hostile state, watch it not trap, call it proof) is exactly the kind of
    /// instrument this project keeps finding to be blind.
    ///
    /// So what is asserted is the reason the guard is unreachable: the plan
    /// conserves for every one of those states. The guard itself is deliberate
    /// belt-and-braces -- it makes "the exit door cannot trap" a property of
    /// `settle_unmovable_hand`, rather than a property inherited from a
    /// construction somewhere else that a later change could break silently.
    #[test]
    fn the_refund_plan_conserves_for_every_state_the_exit_doors_can_meet() {
        let bob = Principal::from_slice(&[2]);
        let cases: Vec<(&str, TableState)> = vec![
            ("empty table, live hand", table_at(GamePhase::PreFlop, None)),
            (
                "one seat, saturating stake",
                {
                    let mut st = table_at(GamePhase::Turn, None);
                    st.players[0] = Some(funded_seat(1, 0, 0, u64::MAX));
                    st
                },
            ),
            (
                "two saturating stakes",
                {
                    let mut st = table_at(GamePhase::River, None);
                    st.players[0] = Some(funded_seat(1, 0, 0, u64::MAX));
                    st.players[1] = Some(funded_seat(2, 1, 0, u64::MAX));
                    st
                },
            ),
            (
                "a departed stake at a re-occupied seat (E-36)",
                {
                    let mut st = table_at(GamePhase::Flop, None);
                    st.players[1] = Some(funded_seat(3, 1, 10, 7));
                    let hand = st.hand_number;
                    st.departed_stakes_mut().push(DepartedStake {
                        hand_number: hand,
                        seat: 1,
                        principal: bob,
                        contributed: 11,
                    });
                    st
                },
            ),
            (
                "everybody in for zero",
                {
                    let mut st = table_at(GamePhase::PreFlop, None);
                    st.players[0] = Some(funded_seat(1, 0, 10, 0));
                    st.players[1] = Some(funded_seat(2, 1, 10, 0));
                    st
                },
            ),
        ];

        for (name, st) in cases {
            let plan = refund_every_stake(&st);
            assert!(
                plan.conserves(),
                "{name}: refund plan awards {} of {} collected, so settle_unmovable_hand would \
                 DECLINE to settle and the stakes would stay in the pot",
                plan.awarded,
                plan.collected
            );
        }
    }

    /// A hand that is not live has nothing to settle, whichever door asks.
    #[test]
    fn settling_a_finished_hand_is_a_no_op() {
        let mut st = table_at(GamePhase::HandComplete, None);
        assert_eq!(
            settle_unmovable_hand(&mut st, 42, UnmovableReason::NobodyLeftToWinIt),
            None
        );
        let mut st = table_at(GamePhase::WaitingForPlayers, None);
        assert_eq!(
            settle_unmovable_hand(&mut st, 42, UnmovableReason::NoMessageCanMoveIt),
            None
        );
    }
}
