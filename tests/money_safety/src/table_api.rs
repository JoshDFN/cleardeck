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

impl Card {
    /// The same card as the engine's own type, so a test can re-evaluate a hand
    /// through `poker_core::evaluate_hand` -- the exact function the canister's
    /// showdown path calls.
    pub fn to_engine(self) -> poker_core::Card {
        poker_core::Card {
            suit: match self.suit {
                Suit::Hearts => poker_core::Suit::Hearts,
                Suit::Diamonds => poker_core::Suit::Diamonds,
                Suit::Clubs => poker_core::Suit::Clubs,
                Suit::Spades => poker_core::Suit::Spades,
            },
            rank: match self.rank {
                Rank::Two => poker_core::Rank::Two,
                Rank::Three => poker_core::Rank::Three,
                Rank::Four => poker_core::Rank::Four,
                Rank::Five => poker_core::Rank::Five,
                Rank::Six => poker_core::Rank::Six,
                Rank::Seven => poker_core::Rank::Seven,
                Rank::Eight => poker_core::Rank::Eight,
                Rank::Nine => poker_core::Rank::Nine,
                Rank::Ten => poker_core::Rank::Ten,
                Rank::Jack => poker_core::Rank::Jack,
                Rank::Queen => poker_core::Rank::Queen,
                Rank::King => poker_core::Rank::King,
                Rank::Ace => poker_core::Rank::Ace,
            },
        }
    }
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

/// Reply of `get_stuck_hand_status`: whether this table is holding a hand that no
/// message can move, and whether anybody can end it.
/// docs/SECURITY-FINDINGS.md FINDING 15.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct StuckHandStatus {
    pub is_stuck: bool,
    pub hand_in_progress: bool,
    pub abandonable_in_ns: Option<u64>,
    pub refundable_pot: u64,
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
    /// Stakes of seats vacated mid-hand. Part of the payout basis since the E-05
    /// fix: money in the pot belongs to the hand, not to the chair, so leaving the
    /// table no longer removes the record of what a player put in. Mirrored here
    /// because M1b's attribution leg has to measure `pot` against the WHOLE basis;
    /// against the seated players alone it would report every departure as an
    /// orphaned stake, which is exactly what the fix stops being true.
    ///
    /// `opt vec`, matching the canister. It is `opt` because `TableState` is
    /// persisted nested inside `opt TableState` and a non-`opt` addition there makes
    /// an upgrade from older state silently restore a null table
    /// (docs/SECURITY-FINDINGS.md FINDING 14). The mirror must track that, or this
    /// harness cannot decode `get_table_state` at all.
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

    /// What the SEATED players are credited with putting in this hand.
    pub fn wagered_total(&self) -> u64 {
        self.seated()
            .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand))
    }

    /// Every departed stake recorded on the table, whatever hand it belongs to.
    pub fn departed(&self) -> &[DepartedStake] {
        self.departed_stakes.as_deref().unwrap_or(&[])
    }

    /// Stakes recorded for seats that left mid-hand, for THIS hand only.
    pub fn departed_total(&self) -> u64 {
        self.departed()
            .iter()
            .filter(|d| d.hand_number == self.hand_number)
            .fold(0u64, |a, d| a.saturating_add(d.contributed))
    }

    /// THE PAYOUT BASIS: every chip this hand collected, whether or not the seat
    /// that put it in is still occupied. This is what `pot` must equal.
    pub fn payout_basis_total(&self) -> u64 {
        self.wagered_total().saturating_add(self.departed_total())
    }

    pub fn seat_of(&self, who: Principal) -> Option<u8> {
        self.seated().find(|p| p.principal == who).map(|p| p.seat)
    }

    /// Whoever is sitting in `seat`, if anybody.
    pub fn player_at(&self, seat: u8) -> Option<&Player> {
        self.players.get(seat as usize).and_then(|p| p.as_ref())
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

/// Narrow view of `Winner`: who was paid how much, and -- critically -- with WHICH
/// hand the canister thought they won.
///
/// `hand_rank` is `poker_core::HandRank`, the engine's own type and the type on the
/// canister's Candid wire, so a test can compare it against a fresh
/// `poker_core::evaluate_hand` of the revealed cards with no mirror in between.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct WinnerAmount {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
    pub hand_rank: Option<poker_core::HandRank>,
}

/// Narrow view of `ShowdownPlayer`: every player who reached the showdown, with the
/// hole cards the canister revealed and the rank it assigned them.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct ShowdownRecord {
    pub seat: u8,
    pub principal: Principal,
    pub cards: Option<(poker_core::Card, poker_core::Card)>,
    pub hand_rank: Option<poker_core::HandRank>,
    pub amount_won: u64,
}

/// One person's part in one hand, as the table's own record states it.
///
/// The SAME VALUES the table sends to the archive canister -- `record_hand_to_history`
/// builds the list once and clones it into both -- so a gate that reads this is
/// reading the permanent record without needing the archive installed.
/// docs/SECURITY-FINDINGS.md FINDING 30.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct ArchivedPlayer {
    pub seat: u8,
    pub principal: Principal,
    pub starting_chips: u64,
    pub ending_chips: u64,
    pub amount_won: u64,
    /// `null` on a record written before the table recorded it.
    #[serde(default)]
    pub dealt_in: Option<bool>,
    #[serde(default)]
    pub contributed: Option<u64>,
    #[serde(default)]
    pub left_mid_hand: Option<bool>,
}

/// A seat the deal gave cards to, in deck order. `P` for SHUFFLE-SPEC section 4.
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct DealtInSeat {
    pub seat: u8,
    pub principal: Principal,
}

/// Narrow view of `HandHistory`.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct HandHistoryAmounts {
    pub hand_number: u64,
    pub winners: Vec<WinnerAmount>,
    pub community_cards: Vec<Card>,
    pub showdown_players: Vec<ShowdownRecord>,
    /// Everyone whose money was in the hand. `None` means the build under test
    /// does not record it -- which is the FINDING 30 defect itself, so M12 treats
    /// `None` as a violation rather than as "nothing to check".
    #[serde(default)]
    pub participants: Option<Vec<ArchivedPlayer>>,
    #[serde(default)]
    pub dealt_in: Option<Vec<DealtInSeat>>,
}

impl HandHistoryAmounts {
    pub fn awarded_total(&self) -> u64 {
        self.winners
            .iter()
            .fold(0u64, |a, w| a.saturating_add(w.amount))
    }
}

// ---------------------------------------------------------------------------
// CUSTODY SURFACES -- THE ACCOUNT CENSUS
// (docs/SECURITY-FINDINGS.md FINDING 21, FINDING 28)
// ---------------------------------------------------------------------------

/// Reply of `get_custody_status`: everything the canister is holding for ONE
/// caller, across every ledger account it owns.
///
/// `unswept_deposit` is the field FINDING 28 is about. Before it existed this
/// record answered `total = 0` for a player whose money was sitting at the
/// deposit address the canister itself had published to them.
///
/// **Mirrored in FULL on purpose.** A partial mirror here would let the field be
/// deleted from the canister without a single test noticing, which is precisely
/// how the deposit subaccounts stayed outside every instrument for seven waves.
///
/// # And that is exactly what happened to it (docs/DEFECTS.md E-72, FINDING 36)
///
/// The comment above was written by one agent; a second agent added
/// `unfinished_ledger_ops` to the canister's record in the same wave; **Candid
/// record subtyping drops unknown fields silently**, so nothing failed, nothing
/// warned, and every harness assertion against this surface read nine of ten
/// fields. The mechanism the comment exists to prevent is the mechanism that got
/// it. The field is here now, and [`CustodyStatus::components_sum_to_total`] is
/// what makes the completeness load-bearing rather than decorative: it can only
/// hold while every component field is declared.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct CustodyStatus {
    pub escrow: u64,
    pub chips_at_table: u64,
    pub committed_in_pot: u64,
    pub committed_is_stuck: bool,
    pub abandonable_in_ns: Option<u64>,
    /// Money at the deposit subaccount the canister published to this caller, as
    /// of `unswept_deposit_observed_at_ns`.
    pub unswept_deposit: u64,
    /// `None` means the canister has NEVER asked the ledger about that address.
    /// It does not mean the address is empty.
    pub unswept_deposit_observed_at_ns: Option<u64>,
    /// Money of this caller's that the canister has moved on the ledger and not
    /// finished booking. docs/SECURITY-FINDINGS.md FINDING 29.
    pub unfinished_ledger_ops: u64,
    pub total: u64,
    /// Whether the canister can pay EVERYONE, not just this caller.
    /// docs/SECURITY-FINDINGS.md FINDING 35.
    pub canister_solvency: SolvencyVerdict,
    /// Magnitude of the shortfall when `canister_solvency` is
    /// `CannotPayEveryone`. `None` in the other two states -- including
    /// `Unknown`, so this is never the field to branch on.
    pub canister_shortfall_e8s: Option<u64>,
    pub advice: String,
}

impl CustodyStatus {
    /// The identity that makes a FULL mirror mean something.
    ///
    /// FINDING 36's fix is not "add the missing field": it is one assertion that
    /// cannot hold unless every component field is present, so the next silently
    /// dropped field fails a test instead of quietly narrowing the record.
    pub fn components_sum_to_total(&self) -> bool {
        self.escrow
            .saturating_add(self.chips_at_table)
            .saturating_add(self.committed_in_pot)
            .saturating_add(self.unswept_deposit)
            .saturating_add(self.unfinished_ledger_ops)
            == self.total
    }
}

/// Can the canister pay everyone it owes? Mirrored from
/// `src/table_canister/src/lib.rs`. docs/SECURITY-FINDINGS.md FINDING 35.
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum SolvencyVerdict {
    CanPayEveryone,
    CannotPayEveryone,
    Unknown,
}

/// One reading of the canister's MAIN ledger account.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct MainAccountObservation {
    pub amount: u64,
    pub observed_at_ns: u64,
    pub ledger: Principal,
    pub credited_since: u64,
    pub debited_since: u64,
}

/// Reply of `get_solvency()` / `refresh_solvency()`.
///
/// **Mirrored in FULL on purpose**, with [`CustodyStatus`]'s cautionary tale one
/// screen above it. Every field here is asserted somewhere in
/// `invariants::solvency`, which is the only thing that keeps a full mirror
/// honest.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct SolvencyReport {
    pub currency: String,
    pub ledger: Principal,
    pub as_of_ns: u64,

    pub escrow: u64,
    pub chips_at_table: u64,
    pub pot: u64,
    pub committed_stake: u64,
    pub unswept_deposits: u64,
    pub unfinished_incoming: u64,
    pub pulls_in_flight: u64,
    pub sweep_fees_in_flight: u64,
    pub payouts_in_flight: u64,
    pub owed: u64,

    /// `None` means the canister has NEVER read its own main account.
    pub main_account: Option<u64>,
    pub main_observed_at_ns: Option<u64>,
    pub main_ledger: Option<Principal>,
    pub main_credited_since_reading: u64,
    pub main_debited_since_reading: u64,
    pub deposit_subaccounts: u64,
    pub deposit_accounts_observed: u64,
    pub deposit_oldest_observed_at_ns: Option<u64>,
    /// How many enumerable deposit accounts have never been read. The COUNT is
    /// public; the roster is not.
    pub deposit_accounts_never_observed_count: u64,
    /// Which ones, scoped to the caller: all for a controller, your own for
    /// anybody else. The harness reads `get_solvency()` as ANONYMOUS, so this is
    /// normally empty and the count above is the field to branch on.
    pub deposit_accounts_never_observed: Vec<Principal>,
    /// `None` whenever `main_account` is `None`.
    pub held: Option<u64>,

    pub difference_e8s: Option<i128>,
    pub shortfall_e8s: Option<u64>,
    pub unattributed_at_main: Option<u64>,
    pub guard_liability: u64,
    pub verdict: SolvencyVerdict,
    pub summary: String,
}

/// Reply of `get_deposit_custody` / `refresh_deposit_custody`.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct DepositAddressCustody {
    pub subaccount: Vec<u8>,
    /// MIRRORED IN FULL ON PURPOSE, and this is the field FINDING 36 is about:
    /// a mirror that drops a field turns a reply the canister sends into a reply
    /// the harness cannot see, and the harness then reports the canister is
    /// silent about money it is in fact naming.
    pub canister: Principal,
    /// The 64-hex spelling of `(canister, subaccount)` -- see FINDING 34.
    pub address: String,
    pub ledger: Principal,
    pub observed_amount: u64,
    pub observed_at_ns: Option<u64>,
    pub transfer_fee: u64,
    pub sweepable: bool,
    pub note: String,
}
