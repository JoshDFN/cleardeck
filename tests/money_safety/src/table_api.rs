//! Candid mirror of the table canister's surface.
//!
//! Mirrored from `src/table_canister/src/lib.rs`, NOT from
//! `src/table_canister/table_canister.did` -- that file is known to be stale
//! (docs/SECURITY-FINDINGS.md FINDING 03: it is missing `Player.sitting_out_since`
//! and renumbers the `Result_N` aliases), so a client generated from it
//! mis-decodes several method results. The harness must speak to the code.
//!
//! Where the harness does not need a field it is simply left out: Candid record
//! subtyping lets a decoder drop fields the target type does not declare, which
//! also keeps this file from breaking every time an unrelated display field is
//! added. Types that participate in M5 (UPGRADE DURABILITY) are mirrored in FULL,
//! on purpose -- a partial mirror there would hide exactly the loss M5 looks for.

use candid::{CandidType, Principal};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// cards
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

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
    /// Six-max ICP table, 0.01/0.02, buy-in 2..10 ICP. Deep enough for real
    /// multi-street betting and wide enough for genuine side pots.
    pub fn six_max_icp() -> Self {
        Self {
            small_blind: 1_000_000,
            big_blind: 2_000_000,
            min_buy_in: 200_000_000,
            max_buy_in: 1_000_000_000,
            max_players: 6,
            action_timeout_secs: 30,
            ante: 0,
            time_bank_secs: 30,
            currency: Currency::ICP,
        }
    }

    /// Heads-up, matching the shape of the deployed `table_1`.
    pub fn heads_up_icp() -> Self {
        Self {
            max_players: 2,
            ..Self::six_max_icp()
        }
    }

    /// With an ante, so the ante branch of `start_new_hand` is exercised.
    pub fn six_max_with_ante() -> Self {
        Self {
            ante: 500_000,
            ..Self::six_max_icp()
        }
    }
}

// ---------------------------------------------------------------------------
// game state
// ---------------------------------------------------------------------------

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

impl GamePhase {
    /// True in the phases where the canister itself refuses withdraw/cash_out
    /// for players still in the hand.
    pub fn hand_in_progress(&self) -> bool {
        !matches!(self, GamePhase::WaitingForPlayers | GamePhase::HandComplete)
    }
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
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

/// Full mirror of `TableState`, minus the display-only `last_action` field.
///
/// Everything that carries money or hand identity is here, because M5 compares
/// two of these across an upgrade.
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
}

impl TableState {
    pub fn seated(&self) -> impl Iterator<Item = &Player> {
        self.players.iter().flatten()
    }

    pub fn chips_total(&self) -> u64 {
        self.seated().fold(0u64, |a, p| a.saturating_add(p.chips))
    }

    pub fn side_pots_total(&self) -> u64 {
        self.side_pots
            .iter()
            .fold(0u64, |a, sp| a.saturating_add(sp.amount))
    }

    pub fn wagered_total(&self) -> u64 {
        self.seated()
            .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand))
    }

    pub fn seat_of(&self, who: Principal) -> Option<u8> {
        self.seated().find(|p| p.principal == who).map(|p| p.seat)
    }
}

// ---------------------------------------------------------------------------
// calls
// ---------------------------------------------------------------------------

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
pub enum TimeoutCheckResult {
    NoAction,
    PlayerTimedOut(u8),
    AutoDealReady,
}

/// Narrow view of `Winner`: only what "who was paid how much" needs.
/// `hand_rank` and `cards` are dropped by Candid record subtyping.
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
