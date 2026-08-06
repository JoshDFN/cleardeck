//! THE PERMANENT RECORD, as an outsider reads it.
//!
//! Everything else in this harness asks where the MONEY went. This module asks
//! what the RECORD says, because "provably fair" is a claim about the record and
//! nothing in this project measured it.
//!
//! docs/SECURITY-FINDINGS.md FINDING 30: `record_hand_to_history` built its player
//! list from `state.players` as they stood AT SETTLEMENT, so the archive omitted
//! anyone who left mid-hand and invented anyone who sat down mid-hand. Money was
//! never involved: every conservation invariant in this harness was silent, and
//! the only artifact the product's central claim rests on was false.
//!
//! Two things live here:
//!
//!   * a Candid mirror of the `history` canister's record, NARROW on purpose --
//!     Candid record subtyping lets a decoder drop fields it does not declare, and
//!     every field added since the fix is `opt`, so this one mirror decodes a
//!     record written by the archive BEFORE the fix (fields absent -> `None`) and
//!     one written after (fields present). That is what let the defect be measured
//!     and the fix measured with the same code;
//!   * [`install_archive`], which puts the REAL archive module -- built from this
//!     tree -- next to the table under test and wires the two together, so a gate
//!     reads what was actually archived rather than what the table meant to send.

use candid::{decode_one, encode_one, CandidType, Encode, Principal};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::table_api::Card;
use crate::wasms;
use crate::world::World;

// ---------------------------------------------------------------------------
// the record, as the archive stores it
// ---------------------------------------------------------------------------

/// One seat that took cards from the deck, in the order the deal consumed them.
///
/// This is the field a verifier needs and could not have: `P`, the number of
/// players dealt in, is what offsets the board in docs/SHUFFLE-SPEC.md section 4.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct DealtInSeat {
    pub seat: u8,
    pub principal: Principal,
}

/// One person's part in one hand.
///
/// `dealt_in`, `contributed` and `left_mid_hand` are `opt` because the archive is
/// append-only and holds records written before they existed. `None` means "this
/// record predates the fix", which is exactly the answer a verifier should get for
/// the hands FINDING 30 was found on -- not a fabricated `false`.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct PlayerHandRecord {
    pub seat: u8,
    pub principal: Principal,
    pub starting_chips: u64,
    pub ending_chips: u64,
    pub amount_won: u64,
    pub position: String,
    #[serde(default)]
    pub dealt_in: Option<bool>,
    #[serde(default)]
    pub contributed: Option<u64>,
    #[serde(default)]
    pub left_mid_hand: Option<bool>,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct WinnerRecord {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct ShuffleProofRecord {
    pub seed_hash: String,
    pub revealed_seed: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct HandHistoryRecord {
    pub hand_id: u64,
    pub table_id: Principal,
    pub hand_number: u64,
    pub shuffle_proof: ShuffleProofRecord,
    pub players: Vec<PlayerHandRecord>,
    pub dealer_seat: u8,
    pub flop: Option<(Card, Card, Card)>,
    pub turn: Option<Card>,
    pub river: Option<Card>,
    pub total_pot: u64,
    pub rake: u64,
    pub winners: Vec<WinnerRecord>,
    pub went_to_showdown: bool,
    /// Absent on every record written before FINDING 30 was fixed.
    #[serde(default)]
    pub dealt_in: Option<Vec<DealtInSeat>>,
}

impl HandHistoryRecord {
    /// What each principal is recorded as having put into this pot.
    pub fn contributed_by_principal(&self) -> BTreeMap<Principal, u64> {
        let mut out: BTreeMap<Principal, u64> = BTreeMap::new();
        for p in &self.players {
            *out.entry(p.principal).or_insert(0) =
                out.get(&p.principal).copied().unwrap_or(0) + p.contributed.unwrap_or(0);
        }
        out
    }

    /// Everybody the record names as having taken part.
    pub fn named_principals(&self) -> BTreeSet<Principal> {
        self.players.iter().map(|p| p.principal).collect()
    }

    /// `P` for docs/SHUFFLE-SPEC.md section 4, from the record alone.
    pub fn players_dealt_in(&self) -> Option<usize> {
        self.dealt_in.as_ref().map(|d| d.len())
    }

    pub fn awarded_total(&self) -> u64 {
        self.winners
            .iter()
            .fold(0u64, |a, w| a.saturating_add(w.amount))
    }
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct HandSummary {
    pub hand_id: u64,
    pub table_id: Principal,
    pub hand_number: u64,
    pub player_count: u8,
    pub total_pot: u64,
}

// ---------------------------------------------------------------------------
// the archive canister, installed next to the table
// ---------------------------------------------------------------------------

/// The archive, wired to the table under test.
pub struct Archive {
    pub canister: Principal,
}

/// Install the real `history` module, authorise the table to write to it, and
/// point the table at it. Panics rather than returning: a gate that silently ran
/// without an archive would report green about nothing.
pub fn install_archive(world: &World) -> Archive {
    let module = wasms::history_canister_module();
    let canister = world.pic.create_canister_with_settings(Some(world.controller), None);
    world.pic.add_cycles(canister, 100_000_000_000_000);
    world.pic.install_canister(
        canister,
        module.bytes.clone(),
        Encode!().expect("history init arg"),
        Some(world.controller),
    );

    // The archive's admin is whoever installed it, and only the admin may admit a
    // writer. Without this the table's every `record_hand` is refused and the
    // archive holds nothing at all -- which is how the archive spent wave 5
    // (docs/DEFECTS.md T-34).
    let bytes = world
        .pic
        .update_call(
            canister,
            world.controller,
            "authorize_table",
            encode_one(world.table).expect("authorize_table arg"),
        )
        .expect("authorize_table must not be rejected");
    decode_one::<Result<(), String>>(&bytes)
        .expect("authorize_table reply decode")
        .expect("authorize_table must succeed for the archive's admin");

    let bytes = world
        .pic
        .update_call(
            world.table,
            world.controller,
            "set_history_canister",
            encode_one(Some(canister)).expect("set_history_canister arg"),
        )
        .expect("set_history_canister must not be rejected");
    decode_one::<Result<(), String>>(&bytes)
        .expect("set_history_canister reply decode")
        .expect("set_history_canister must succeed for a controller");

    Archive { canister }
}

impl Archive {
    pub fn total_hands(&self, world: &World) -> u64 {
        let bytes = world
            .pic
            .query_call(
                self.canister,
                Principal::anonymous(),
                "get_total_hands",
                Encode!().expect("arg"),
            )
            .expect("get_total_hands must not be rejected");
        decode_one::<u64>(&bytes).expect("get_total_hands decode")
    }

    pub fn hand(&self, world: &World, hand_id: u64) -> Option<HandHistoryRecord> {
        let bytes = world
            .pic
            .query_call(
                self.canister,
                Principal::anonymous(),
                "get_hand",
                encode_one(hand_id).expect("arg"),
            )
            .expect("get_hand must not be rejected");
        decode_one::<Option<HandHistoryRecord>>(&bytes).expect("get_hand decode")
    }

    /// Every record the archive holds for the table under test, oldest first.
    pub fn hands_of_table(&self, world: &World) -> Vec<HandHistoryRecord> {
        (1..=self.total_hands(world))
            .filter_map(|id| self.hand(world, id))
            .filter(|h| h.table_id == world.table)
            .collect()
    }

    /// The archive's own record for one of the table's hand numbers.
    pub fn hand_numbered(&self, world: &World, hand_number: u64) -> Option<HandHistoryRecord> {
        self.hands_of_table(world)
            .into_iter()
            .find(|h| h.hand_number == hand_number)
    }
}
