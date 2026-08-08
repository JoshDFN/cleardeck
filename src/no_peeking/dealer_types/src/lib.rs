//! The sealed dealer's wire types, and the deal arithmetic that follows from
//! `docs/SHUFFLE-SPEC.md`.
//!
//! # Why this is its own crate
//!
//! The table has to speak the dealer's Candid, and the test harness has to decode
//! it. If either linked `dealer_canister` directly it would also link that
//! crate's `canister_init` / `get_candid_pointer` exports and the module would not
//! link at all — but the deeper reason is the one this project keeps re-learning:
//! **a second copy of a type is a copy that drifts.** `src/poker_core` exists
//! because `src/table_canister/tests/unit_tests.rs` carried a private
//! re-implementation of the engine and the two had already diverged, so its green
//! tests proved nothing. One definition, linked by everybody, is the fix.
//!
//! # No `ic-cdk` here
//!
//! Same rule as `poker_core`: this crate must build for the host so the deal
//! arithmetic can be unit-tested without a replica.

use candid::{CandidType, Deserialize, Principal};
use poker_core::Card;

/// Streets, in the only order they may be revealed.
///
/// `Dealt` means hole cards exist and nothing else does. There is no variant for
/// "the deck", because the deck is not a thing the dealer will ever name.
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Street {
    Dealt,
    Flop,
    Turn,
    River,
    Complete,
}

/// One seat the deal fed, in the order the deck was consumed.
///
/// Same field names and same meaning as the live engine's `DealtInSeat`, because
/// `docs/SHUFFLE-SPEC.md` §4 says `P = length of dealt_in` and
/// `hole cards of dealt_in[k] = deck[2k], deck[2k+1]`. A verifier reproducing a
/// hand this dealer dealt reads the same field under the same name.
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct DealtInSeat {
    pub seat: u8,
    pub principal: Principal,
}

/// A seat's two cards, once they are public and only once they are public.
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct SeatCards {
    pub seat: u8,
    pub cards: (Card, Card),
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Readiness {
    /// The seat called `ack` itself, on this street.
    Acked,
    /// The seat called `stand_down` itself (folded, or all in).
    StoodDownSelf,
    /// The table called `table_stand_down` after the DEALER measured a full action
    /// clock of silence from that seat. The table cannot shorten that clock and
    /// cannot use this against a seat that is still heartbeating.
    TimedOutByTable,
    /// The table called `table_stand_down` after the street had been open for the
    /// whole GRACE period, which applies to a seat that is present and heartbeating
    /// but will not say it is ready.
    ///
    /// This is the escape hatch for the griefer, and it is deliberately a separate
    /// variant so it is legible on the public record: a hand full of these is a
    /// table stalling on purpose, and a hand with one is a player who would not
    /// move. See the dealer's `table_stand_down` for why both clocks have to exist.
    GraceExpired,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct SeatReadiness {
    pub seat: u8,
    pub how: Readiness,
}

/// One line of the append-only public reveal log.
///
/// Every card the dealer has ever made public has a row here saying WHO asked and
/// WHEN, by the dealer's own clock, and how each seat came to be ready. A player
/// who suspects the table advanced a street early does not have to take anyone's
/// word for it.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct RevealRecord {
    pub street: Street,
    pub requested_by: Principal,
    pub at_ns: u64,
    pub readiness: Vec<SeatReadiness>,
}

/// What `open_hand` gives back to the table. **No cards.**
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandOpened {
    pub hand_id: u64,
    pub seed_hash: String,
    pub dealt_in: Vec<DealtInSeat>,
    pub opened_at_ns: u64,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum FinalizeKind {
    /// The table closed the hand normally.
    Table,
    /// Somebody — anybody — opened it after `force_finalize_after_ns` because it
    /// had been left in flight.
    Forced,
}

/// Everything about a hand that is public. Readable by anyone, including
/// anonymously, and identical for every caller.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandPublic {
    pub hand_id: u64,
    pub table: Principal,
    pub seed_hash: String,
    pub dealt_in: Vec<DealtInSeat>,
    pub street: Street,
    pub community: Vec<Card>,
    pub showdown: Vec<SeatCards>,
    pub revealed_seed: Option<String>,
    pub finalized_by: Option<FinalizeKind>,
    pub opened_at_ns: u64,
    pub street_opened_at_ns: u64,
    pub reveals: Vec<RevealRecord>,
    pub ready: Vec<SeatReadiness>,
}

/// The state a caller needs to know whether the street can close yet.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct AckState {
    pub street: Street,
    pub ready: Vec<SeatReadiness>,
    pub waiting_on: Vec<u8>,
    pub can_advance: bool,
}

/// What `force_finalize` gives back: enough to settle the hand correctly without
/// anybody's co-operation.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ForcedFinalize {
    pub hand_id: u64,
    pub revealed_seed: String,
    pub community: Vec<Card>,
    pub hole_cards: Vec<SeatCards>,
    pub forced_by: Principal,
    pub at_ns: u64,
}

/// Runway. `docs/NO-PEEKING-FEASIBILITY.md` §6 Option A: the freezing threshold of
/// a zero-controller canister can never be raised, so the balance is the entire
/// safety margin and it has to be legible to everybody. Anyone may top the dealer
/// up with `deposit_cycles`; that call is open to all principals and needs no
/// controller.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct DealerHealth {
    pub cycle_balance: u128,
    pub min_open_balance: u128,
    pub can_open_hand: bool,
    pub live_hand: Option<u64>,
    pub hands_dealt: u64,
}

/// The frozen facts about a dealer. Everything here was fixed at install and none
/// of it can be changed by anyone, because there is nobody who could.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct DealerIdentity {
    pub table: Principal,
    pub action_timeout_ns: u64,
    pub street_grace_ns: u64,
    pub force_finalize_after_ns: u64,
    pub min_open_balance: u128,
    pub build: String,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct DealerInit {
    /// The ONE canister allowed to drive a hand. Frozen at install: there is no
    /// method to change it and no controller who could install one.
    pub table: Principal,
    pub action_timeout_ns: Option<u64>,
    pub street_grace_ns: Option<u64>,
    pub force_finalize_after_ns: Option<u64>,
    pub min_open_balance: Option<u128>,
}

// ---------------------------------------------------------------------------
// THE DEAL ARITHMETIC — docs/SHUFFLE-SPEC.md §4, transcribed once
// ---------------------------------------------------------------------------

/// Hard ceiling on seats. `docs/SHUFFLE-SPEC.md` §4 consumes `2P + 8` cards, so a
/// 52-card deck supports `P <= 22`; ClearDeck's tables are 6-max and 9-max.
pub const MAX_SEATS: usize = 10;

/// Indices into the SHUFFLED deck of the community cards visible at `street`.
///
/// `p` is `dealt_in.len()` and must NEVER be a count of occupied seats: a hand
/// where somebody left mid-hand produced a board that did not follow from its own
/// seed exactly because that count was re-derived instead of recorded
/// (SECURITY-FINDINGS FINDING 30, DEFECTS E-66). An auditor reported the table had
/// dealt a board that did not follow from its own seed, and was right.
pub fn community_indices(p: usize, street: Street) -> Vec<usize> {
    match street {
        Street::Dealt => Vec::new(),
        Street::Flop => vec![2 * p + 1, 2 * p + 2, 2 * p + 3],
        Street::Turn => vec![2 * p + 1, 2 * p + 2, 2 * p + 3, 2 * p + 5],
        Street::River | Street::Complete => {
            vec![2 * p + 1, 2 * p + 2, 2 * p + 3, 2 * p + 5, 2 * p + 7]
        }
    }
}

/// Indices into the shuffled deck of the `k`-th dealt-in seat's two hole cards.
pub fn hole_indices(k: usize) -> (usize, usize) {
    (2 * k, 2 * k + 1)
}

pub fn next_street(s: Street) -> Option<Street> {
    match s {
        Street::Dealt => Some(Street::Flop),
        Street::Flop => Some(Street::Turn),
        Street::Turn => Some(Street::River),
        Street::River | Street::Complete => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_indices_match_the_shuffle_spec_table() {
        // docs/SHUFFLE-SPEC.md §4 writes heads-up out longhand: hole cards
        // deck[0..4], flop deck[5..8], turn deck[9], river deck[11].
        assert_eq!(community_indices(2, Street::Flop), vec![5, 6, 7]);
        assert_eq!(community_indices(2, Street::Turn), vec![5, 6, 7, 9]);
        assert_eq!(community_indices(2, Street::River), vec![5, 6, 7, 9, 11]);
        assert_eq!(community_indices(6, Street::River), vec![13, 14, 15, 17, 19]);
        assert_eq!(community_indices(0, Street::Dealt), Vec::<usize>::new());
        assert_eq!(hole_indices(0), (0, 1));
        assert_eq!(hole_indices(3), (6, 7));
    }

    #[test]
    fn a_full_table_still_fits_in_fifty_two_cards() {
        let last = *community_indices(MAX_SEATS, Street::River).last().unwrap();
        assert!(last < 52, "2P+7 must stay inside the deck for P={MAX_SEATS}");
    }

    #[test]
    fn streets_only_go_forwards() {
        assert_eq!(next_street(Street::Dealt), Some(Street::Flop));
        assert_eq!(next_street(Street::Flop), Some(Street::Turn));
        assert_eq!(next_street(Street::Turn), Some(Street::River));
        assert_eq!(next_street(Street::River), None);
        assert_eq!(next_street(Street::Complete), None);
    }

    /// No index is used twice, so no card can be dealt twice: hole indices are
    /// `[0, 2P)` and every community index is `>= 2P + 1`.
    #[test]
    fn hole_and_community_index_ranges_never_overlap() {
        for p in 2..=MAX_SEATS {
            let mut used: Vec<usize> = (0..p).flat_map(|k| [2 * k, 2 * k + 1]).collect();
            used.extend(community_indices(p, Street::River));
            let n = used.len();
            used.sort_unstable();
            used.dedup();
            assert_eq!(used.len(), n, "an index is reused with P={p}");
        }
    }
}
