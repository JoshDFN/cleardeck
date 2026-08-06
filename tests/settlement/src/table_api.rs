//! Candid mirror of the table canister's surface.
//!
//! Mirrored from `src/table_canister/src/lib.rs`, NOT from
//! `src/table_canister/table_canister.did` -- that file is known not to describe
//! the deployed code (docs/DEFECTS.md E-08, 241 diff lines), so a client
//! generated from it mis-decodes several method results.
//!
//! This is an independent copy rather than a reuse of `tests/money_safety`'s
//! mirror, on purpose: the settlement oracle is the instrument that has to be
//! trusted when it disagrees with the engine, and it should not share a decode
//! path with another harness that a different owner is editing in parallel.
//!
//! Cards are re-exported from `poker_core` so that what the harness decodes off
//! the wire is literally the type the evaluator ranks. Their Candid encoding is
//! frozen (see the `poker_core` crate docs), so there is no drift risk.

use candid::{CandidType, Principal};
use serde::Deserialize;

pub use poker_core::{Card, Rank, Suit};

// ---------------------------------------------------------------------------
// configuration
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Default)]
pub enum Currency {
    #[default]
    ICP,
    BTC,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct TableConfig {
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_players: u8,
    pub action_timeout_secs: u64,
    pub ante: u64,
    pub time_bank_secs: u64,
    pub currency: Currency,
}

impl TableConfig {
    /// A six-max table whose CHIP UNIT IS ONE e8s.
    ///
    /// This is the workhorse config for settlement testing, and the small blind
    /// really is 1. Two reasons:
    ///
    /// * exact stack depths. `buy_in(seat, amount)` takes an exact amount, and
    ///   with `min_buy_in = 20` the harness can seat five players at five
    ///   deliberately chosen depths and get exactly the side-pot ladder it wants.
    /// * odd chips. A pot of 21 chopped three ways leaves a real remainder. At
    ///   0.01/0.02 ICP every pot is a multiple of 1_000_000 e8s and a two-way
    ///   chop can never leave a remainder, so the odd-chip rule would never be
    ///   exercised at all.
    ///
    /// It satisfies `validate_config`: `big_blind <= 10 * small_blind`,
    /// `min_buy_in >= 10 * big_blind`, `max_buy_in <= 1000 * big_blind`.
    pub fn micro_six_max() -> Self {
        Self {
            small_blind: 1,
            big_blind: 2,
            min_buy_in: 20,
            max_buy_in: 2_000,
            max_players: 6,
            action_timeout_secs: 300,
            ante: 0,
            time_bank_secs: 30,
            currency: Currency::ICP,
        }
    }

    /// Real ICP magnitudes: 0.01/0.02 blinds, 2..10 ICP buy-in. Used for a
    /// cross-check that nothing about the oracle depends on chip scale.
    pub fn realistic_six_max() -> Self {
        Self {
            small_blind: 1_000_000,
            big_blind: 2_000_000,
            min_buy_in: 200_000_000,
            max_buy_in: 1_000_000_000,
            max_players: 6,
            action_timeout_secs: 300,
            ante: 0,
            time_bank_secs: 30,
            currency: Currency::ICP,
        }
    }

    pub fn with_ante(self, ante: u64) -> Self {
        Self { ante, ..self }
    }
}

// ---------------------------------------------------------------------------
// game state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    WaitingForPlayers,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    HandComplete,
}

impl GamePhase {
    pub fn hand_in_progress(&self) -> bool {
        !matches!(self, GamePhase::WaitingForPlayers | GamePhase::HandComplete)
    }
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum PlayerStatus {
    Active,
    SittingOut,
    Disconnected,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
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
    pub time_bank_remaining: u64,
    pub is_sitting_out_next_hand: bool,
    pub broke_at: Option<u64>,
    pub sitting_out_since: Option<u64>,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct ActionTimer {
    pub player_seat: u8,
    pub started_at: u64,
    pub expires_at: u64,
    pub using_time_bank: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct ShuffleProof {
    pub seed_hash: String,
    pub revealed_seed: Option<String>,
    pub timestamp: u64,
}

/// Mirror of `TableState`, minus the display-only `last_action` field, which
/// Candid record subtyping drops on decode.
///
/// `deck` and `deck_index` are here because they are the whole reason the harness
/// can reach a named showdown on purpose: after the deal the deck is fully
/// determined, so the board that WILL come is knowable before a single chip moves.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct TableState {
    pub id: u64,
    pub config: TableConfig,
    pub players: Vec<Option<Player>>,
    pub community_cards: Vec<Card>,
    pub deck: Vec<Card>,
    pub deck_index: u64,
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
    pub bb_has_option: bool,
    pub first_hand: bool,
    pub auto_deal_at: Option<u64>,
    /// Stakes of seats vacated mid-hand, each carrying the PRINCIPAL it belongs to.
    ///
    /// The oracle needs this to answer the question its per-seat diff cannot:
    /// WHO the money in a seat belongs to when the chair changed hands mid-hand
    /// (docs/SECURITY-FINDINGS.md FINDING 13).
    ///
    /// `opt vec`, matching the canister: `TableState` is persisted nested inside
    /// `opt TableState`, so a non-`opt` addition here makes an upgrade from older
    /// state silently restore a null table (FINDING 14).
    #[serde(default)]
    pub departed_stakes: Option<Vec<DepartedStake>>,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct DepartedStake {
    pub hand_number: u64,
    pub seat: u8,
    pub principal: Principal,
    pub contributed: u64,
}

impl TableState {
    /// Every departed stake recorded on the table right now.
    pub fn departed(&self) -> &[DepartedStake] {
        self.departed_stakes.as_deref().unwrap_or(&[])
    }

    pub fn seated(&self) -> impl Iterator<Item = &Player> {
        self.players.iter().flatten()
    }

    pub fn player_at(&self, seat: u8) -> Option<&Player> {
        self.players.get(seat as usize).and_then(|p| p.as_ref())
    }

    pub fn chips_total(&self) -> u64 {
        self.seated().fold(0u64, |a, p| a.saturating_add(p.chips))
    }

    pub fn wagered_total(&self) -> u64 {
        self.seated()
            .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand))
    }

    pub fn side_pots_total(&self) -> u64 {
        self.side_pots
            .iter()
            .fold(0u64, |a, sp| a.saturating_add(sp.amount))
    }

    pub fn seat_of(&self, who: Principal) -> Option<u8> {
        self.seated().find(|p| p.principal == who).map(|p| p.seat)
    }

    /// Seats that are not folded and are still seated: the showdown contenders.
    pub fn contenders(&self) -> Vec<u8> {
        self.seated()
            .filter(|p| !p.has_folded)
            .map(|p| p.seat)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// calls
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum PlayerAction {
    Fold,
    Check,
    Call,
    Bet(u64),
    Raise(u64),
    AllIn,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum TimeoutCheckResult {
    NoAction,
    PlayerTimedOut(u8),
    AutoDealReady,
}

/// Narrow view of `Winner`: who was paid how much. `hand_rank` and `cards` are
/// dropped by Candid record subtyping.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct WinnerAmount {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
}

/// Narrow view of `HandHistory`.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct HandHistoryAmounts {
    pub hand_number: u64,
    pub winners: Vec<WinnerAmount>,
}
