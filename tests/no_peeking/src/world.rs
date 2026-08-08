//! The world: a PocketIC replica running a **sealed dealer** and a table that
//! holds no cards, plus the helpers a test needs to play a hand and then attack it.
//!
//! # The handover is part of the harness, not part of the story
//!
//! [`World::new`] installs the dealer and then drops its controller list to `[]`
//! with a real `update_settings`, before any test runs. Everything after that
//! point is measured against a canister that nobody controls, which is the only
//! configuration in which the claim means anything.
//! [`World::assert_dealer_is_sealed`] re-reads the controller list from the replica
//! rather than trusting that the call was made.

use candid::{decode_one, encode_args, encode_one, CandidType, Principal};
use dealer_types::{
    AckState, DealerHealth, DealerIdentity, DealerInit, DealtInSeat, ForcedFinalize, HandOpened,
    HandPublic, SeatCards,
};
use poker_core::Card;
use pocket_ic::{PocketIc, PocketIcBuilder, RejectResponse, Time};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::build;

/// 2026-01-01T00:00:00Z, so timestamps in a transcript are readable.
const GENESIS_NANOS: u64 = 1_767_225_600_000_000_000;

/// Cycles handed to each canister at creation. Comfortably above the dealer's own
/// `min_open_balance` floor, so the runway gate is exercised deliberately by
/// `tests/disconnect.rs` rather than tripping by accident everywhere else.
const START_CYCLES: u128 = 100_000_000_000_000;

pub fn pocket_ic_binary() -> Option<PathBuf> {
    if std::env::var("POCKET_IC_BIN").is_ok() {
        return None; // the crate reads the env var itself
    }
    let dfx_cache = Command::new("dfx")
        .arg("cache")
        .arg("show")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let candidate = PathBuf::from(dfx_cache).join("pocket-ic");
    candidate.exists().then_some(candidate)
}

// ---------------------------------------------------------------------------
// the table stub's wire types, mirrored ONLY where they are the stub's own
// ---------------------------------------------------------------------------
//
// `DealtInSeat`, `SeatCards` and the rest come from `dealer_types`, linked. These
// three are the stub's own and are small enough that a mirror is honest; they are
// pinned by `table_view_decodes_against_the_stubs_own_types` in tests/full_hand.rs,
// which decodes a REAL reply into them.

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Idle,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    Complete,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Action {
    Fold,
    Check,
    Call,
    RaiseTo(u64),
    AllIn,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct Seat {
    pub seat: u8,
    pub principal: Principal,
    pub chips: u64,
    pub current_bet: u64,
    pub total_bet_this_hand: u64,
    pub has_folded: bool,
    pub has_acted_this_round: bool,
    pub is_all_in: bool,
    pub in_hand: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Winner {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
    pub rank: Option<poker_core::HandRank>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableView {
    pub dealer_canister: Principal,
    pub phase: Phase,
    pub hand_id: Option<u64>,
    pub seed_hash: Option<String>,
    pub revealed_seed: Option<String>,
    pub dealt_in: Vec<DealtInSeat>,
    pub seats: Vec<Seat>,
    pub board: Vec<Card>,
    pub showdown: Vec<SeatCards>,
    pub pot: u64,
    pub side_pots: Vec<SidePot>,
    pub current_bet: u64,
    pub min_raise: u64,
    pub action_on: Option<u8>,
    pub dealer_seat: u8,
    pub winners: Vec<Winner>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct StubConfig {
    pub small_blind: u64,
    pub big_blind: u64,
}

#[derive(Clone, Copy, Debug, CandidType)]
pub struct StubInit {
    pub dealer: Principal,
    pub config: StubConfig,
}

// ---------------------------------------------------------------------------
// outcomes
// ---------------------------------------------------------------------------

/// The result of a call, kept in a shape a test can assert on without unwrapping
/// three layers: the replica may reject it, the canister may return `Err`, and
/// only the third case is a real answer.
#[derive(Debug)]
pub enum Outcome<T> {
    Ok(T),
    /// The canister replied `Err(String)`.
    Refused(String),
    /// The replica rejected the message (trap, no such method, not a controller).
    Rejected(String),
}

impl<T> Outcome<T> {
    pub fn unwrap(self) -> T {
        match self {
            Outcome::Ok(v) => v,
            Outcome::Refused(e) => panic!("canister refused: {e}"),
            Outcome::Rejected(e) => panic!("replica rejected: {e}"),
        }
    }
    pub fn is_ok(&self) -> bool {
        matches!(self, Outcome::Ok(_))
    }
    pub fn err_text(&self) -> String {
        match self {
            Outcome::Ok(_) => String::new(),
            Outcome::Refused(e) | Outcome::Rejected(e) => e.clone(),
        }
    }
}

fn reject_text(e: RejectResponse) -> String {
    format!("{:?}: {}", e.reject_code, e.reject_message)
}

// ---------------------------------------------------------------------------
// the world
// ---------------------------------------------------------------------------

pub struct World {
    pub pic: PocketIc,
    pub dealer: Principal,
    pub table: Principal,
    /// Controller of the TABLE. Deliberately kept, because the auditor's finding is
    /// about exactly this principal, and `tests/no_peek.rs` attacks with it.
    pub operator: Principal,
    pub players: Vec<Principal>,
    pub dealer_wasm: Vec<u8>,
    pub table_wasm: Vec<u8>,
}

impl World {
    pub fn new(player_names: &[&str]) -> Self {
        Self::with_settings(player_names, None, None)
    }

    pub fn with_settings(
        player_names: &[&str],
        force_finalize_after_ns: Option<u64>,
        min_open_balance: Option<u128>,
    ) -> Self {
        Self::configured(player_names, None, force_finalize_after_ns, min_open_balance)
    }

    pub fn configured(
        player_names: &[&str],
        street_grace_ns: Option<u64>,
        force_finalize_after_ns: Option<u64>,
        min_open_balance: Option<u128>,
    ) -> Self {
        let mut builder = PocketIcBuilder::new().with_application_subnet();
        if let Some(bin) = pocket_ic_binary() {
            builder = builder.with_server_binary(bin);
        }
        let pic = builder.build();
        pic.set_time(Time::from_nanos_since_unix_epoch(GENESIS_NANOS));

        let operator = Principal::self_authenticating(b"cleardeck-no-peeking-operator");
        let players: Vec<Principal> = player_names
            .iter()
            .map(|n| Principal::self_authenticating(format!("cleardeck-no-peeking-{n}").as_bytes()))
            .collect();

        // Both canisters must exist before either is installed: each needs the
        // other's principal in its init argument, and neither may be changeable
        // afterwards.
        let dealer = pic.create_canister_with_settings(Some(operator), None);
        let table = pic.create_canister_with_settings(Some(operator), None);
        pic.add_cycles(dealer, START_CYCLES);
        pic.add_cycles(table, START_CYCLES);

        let dealer_wasm = build::dealer_wasm();
        let table_wasm = build::table_stub_wasm();

        pic.install_canister(
            dealer,
            dealer_wasm.clone(),
            encode_one(DealerInit {
                table,
                action_timeout_ns: None,
                street_grace_ns,
                force_finalize_after_ns,
                min_open_balance,
            })
            .expect("dealer init encode"),
            Some(operator),
        );
        pic.install_canister(
            table,
            table_wasm.clone(),
            encode_one(StubInit {
                dealer,
                config: StubConfig {
                    small_blind: 10,
                    big_blind: 20,
                },
            })
            .expect("table init encode"),
            Some(operator),
        );

        // ---- THE HANDOVER ------------------------------------------------
        // From here the dealer has no controller. `docs/NO-PEEKING-FEASIBILITY.md`
        // §6 Option A measured every consequence of this against a replica; the
        // harness re-measures the ones that matter in tests/no_peek.rs rather than
        // citing them.
        pic.set_controllers(dealer, Some(operator), vec![])
            .expect("could not drop the dealer's controllers");

        let world = Self {
            pic,
            dealer,
            table,
            operator,
            players,
            dealer_wasm,
            table_wasm,
        };
        world.assert_dealer_is_sealed();
        world
    }

    /// Re-read the controller list FROM THE REPLICA. The handover is the premise of
    /// every other claim in this harness, so it is measured, not assumed.
    pub fn assert_dealer_is_sealed(&self) {
        let controllers = self.pic.get_controllers(self.dealer);
        assert!(
            controllers.is_empty(),
            "the dealer still has controllers {controllers:?}; nothing else in this \
             harness means anything until that list is empty"
        );
        let table_controllers = self.pic.get_controllers(self.table);
        assert_eq!(
            table_controllers,
            vec![self.operator],
            "the TABLE must keep a controller: the whole point is that the auditor's \
             attack is run with a real controller key and still finds nothing"
        );
    }

    pub fn player(&self, i: usize) -> Principal {
        self.players[i]
    }

    pub fn now_nanos(&self) -> u64 {
        self.pic.get_time().as_nanos_since_unix_epoch()
    }

    pub fn advance(&self, d: Duration) {
        self.pic.advance_time(d);
        self.pic.tick();
    }

    // --- raw call plumbing ------------------------------------------------

    pub fn update_raw(
        &self,
        target: Principal,
        sender: Principal,
        method: &str,
        arg: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.pic
            .update_call(target, sender, method, arg)
            .map_err(reject_text)
    }

    pub fn query_raw(
        &self,
        target: Principal,
        sender: Principal,
        method: &str,
        arg: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.pic
            .query_call(target, sender, method, arg)
            .map_err(reject_text)
    }

    fn call<T: for<'a> candid::Deserialize<'a> + candid::CandidType>(
        &self,
        target: Principal,
        sender: Principal,
        method: &str,
        arg: Vec<u8>,
        is_query: bool,
    ) -> Outcome<T> {
        let raw = if is_query {
            self.query_raw(target, sender, method, arg)
        } else {
            self.update_raw(target, sender, method, arg)
        };
        match raw {
            Err(e) => Outcome::Rejected(e),
            Ok(bytes) => match decode_one::<Result<T, String>>(&bytes) {
                Ok(Ok(v)) => Outcome::Ok(v),
                Ok(Err(e)) => Outcome::Refused(e),
                Err(_) => match decode_one::<T>(&bytes) {
                    Ok(v) => Outcome::Ok(v),
                    Err(e) => Outcome::Rejected(format!("undecodable reply: {e}")),
                },
            },
        }
    }

    // --- dealer -----------------------------------------------------------

    pub fn my_hole_cards(&self, who: Principal, hand_id: u64) -> Outcome<(Card, Card)> {
        self.call(
            self.dealer,
            who,
            "my_hole_cards",
            encode_one(hand_id).unwrap(),
            true,
        )
    }

    pub fn ack(&self, who: Principal, hand_id: u64) -> Outcome<AckState> {
        self.call(self.dealer, who, "ack", encode_one(hand_id).unwrap(), false)
    }

    pub fn stand_down(&self, who: Principal, hand_id: u64) -> Outcome<AckState> {
        self.call(
            self.dealer,
            who,
            "stand_down",
            encode_one(hand_id).unwrap(),
            false,
        )
    }

    pub fn table_stand_down(&self, who: Principal, hand_id: u64, seat: u8) -> Outcome<AckState> {
        self.call(
            self.dealer,
            who,
            "table_stand_down",
            encode_args((hand_id, seat)).unwrap(),
            false,
        )
    }

    pub fn advance_street_direct(&self, who: Principal, hand_id: u64) -> Outcome<Vec<Card>> {
        self.call(
            self.dealer,
            who,
            "advance_street",
            encode_one(hand_id).unwrap(),
            false,
        )
    }

    pub fn reveal_showdown_direct(
        &self,
        who: Principal,
        hand_id: u64,
        seats: Vec<u8>,
    ) -> Outcome<Vec<SeatCards>> {
        self.call(
            self.dealer,
            who,
            "reveal_showdown",
            encode_args((hand_id, seats)).unwrap(),
            false,
        )
    }

    pub fn finish_hand_direct(&self, who: Principal, hand_id: u64) -> Outcome<String> {
        self.call(
            self.dealer,
            who,
            "finish_hand",
            encode_one(hand_id).unwrap(),
            false,
        )
    }

    pub fn force_finalize(&self, who: Principal, hand_id: u64) -> Outcome<ForcedFinalize> {
        self.call(
            self.dealer,
            who,
            "force_finalize",
            encode_one(hand_id).unwrap(),
            false,
        )
    }

    pub fn open_hand_direct(&self, who: Principal, seats: Vec<DealtInSeat>) -> Outcome<HandOpened> {
        self.call(
            self.dealer,
            who,
            "open_hand",
            encode_one(seats).unwrap(),
            false,
        )
    }

    pub fn hand_public(&self, who: Principal, hand_id: u64) -> Option<HandPublic> {
        let bytes = self
            .query_raw(self.dealer, who, "hand_public", encode_one(hand_id).unwrap())
            .expect("hand_public is public and must never reject");
        decode_one::<Option<HandPublic>>(&bytes).expect("hand_public decode")
    }

    pub fn community(&self, who: Principal, hand_id: u64) -> Vec<Card> {
        let bytes = self
            .query_raw(self.dealer, who, "community", encode_one(hand_id).unwrap())
            .expect("community is public and must never reject");
        decode_one::<Vec<Card>>(&bytes).expect("community decode")
    }

    pub fn dealer_health(&self) -> DealerHealth {
        let bytes = self
            .query_raw(
                self.dealer,
                Principal::anonymous(),
                "dealer_health",
                encode_one(()).unwrap(),
            )
            .expect("dealer_health is public");
        decode_one::<DealerHealth>(&bytes).expect("dealer_health decode")
    }

    pub fn dealer_identity(&self) -> DealerIdentity {
        let bytes = self
            .query_raw(
                self.dealer,
                Principal::anonymous(),
                "dealer_identity",
                encode_one(()).unwrap(),
            )
            .expect("dealer_identity is public");
        decode_one::<Result<DealerIdentity, String>>(&bytes)
            .expect("dealer_identity decode")
            .expect("dealer initialised")
    }

    // --- table ------------------------------------------------------------

    pub fn sit(&self, who: Principal, seat: u8, chips: u64) -> Outcome<()> {
        self.call(
            self.table,
            who,
            "sit",
            encode_args((seat, chips)).unwrap(),
            false,
        )
    }

    pub fn start_hand(&self, who: Principal) -> Outcome<u64> {
        self.call(self.table, who, "start_hand", encode_one(()).unwrap(), false)
    }

    pub fn act(&self, who: Principal, action: Action) -> Outcome<()> {
        self.call(self.table, who, "act", encode_one(action).unwrap(), false)
    }

    pub fn try_advance(&self, who: Principal) -> Outcome<Phase> {
        self.call(
            self.table,
            who,
            "try_advance",
            encode_one(()).unwrap(),
            false,
        )
    }

    pub fn nudge_timeout(&self, who: Principal, seat: u8) -> Outcome<String> {
        self.call(
            self.table,
            who,
            "nudge_timeout",
            encode_one(seat).unwrap(),
            false,
        )
    }

    pub fn rescue_hand(&self, who: Principal) -> Outcome<Vec<Winner>> {
        self.call(
            self.table,
            who,
            "rescue_hand",
            encode_one(()).unwrap(),
            false,
        )
    }

    pub fn view(&self) -> TableView {
        self.view_as(self.operator)
    }

    /// The auditor's `get_table_state`, run as whoever you like.
    pub fn view_as(&self, who: Principal) -> TableView {
        let bytes = self
            .query_raw(
                self.table,
                who,
                "get_table_state",
                encode_one(()).unwrap(),
            )
            .expect("get_table_state must not reject");
        decode_one::<Option<TableView>>(&bytes)
            .expect("get_table_state decode")
            .expect("table initialised")
    }

    pub fn chips_of(&self, who: Principal) -> u64 {
        let bytes = self
            .query_raw(self.table, who, "chips_of", encode_one(who).unwrap())
            .expect("chips_of must not reject");
        decode_one::<u64>(&bytes).expect("chips_of decode")
    }

    /// The table's own view of whether the betting round has closed.
    pub fn query_round_closed(&self) -> bool {
        let bytes = self
            .query_raw(
                self.table,
                Principal::anonymous(),
                "betting_round_closed",
                encode_one(()).unwrap(),
            )
            .expect("betting_round_closed must not reject");
        decode_one::<bool>(&bytes).expect("betting_round_closed decode")
    }

    /// Everybody acks the current street, which is the ordinary case: attentive
    /// clients. Seats that already stood down are skipped by the dealer anyway.
    pub fn everyone_acks(&self, hand_id: u64) {
        for p in &self.players {
            let _ = self.ack(*p, hand_id);
        }
    }
}
