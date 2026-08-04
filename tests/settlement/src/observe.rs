//! Watching one hand happen on the real canister, and comparing what it paid
//! against what the oracle says it owed.
//!
//! # What is measured
//!
//! For every seat: the NET change in what the canister owes that principal across
//! the hand, counted as `escrow + seated chips`. Counting it that way rather than
//! as a change in `Player::chips` is deliberate -- a player who vacates their seat
//! mid-hand has their stack moved into escrow, so `chips` alone would read as a
//! total loss where nothing was lost.
//!
//! The oracle's prediction for the same quantity is `owed - contributed`. Those
//! two numbers are directly comparable, and their difference is the finding.
//!
//! # Where the oracle's inputs come from
//!
//! All from `get_table_state`, which a controller may read in full:
//!
//! * `total_bet_this_hand` per seat -- what the seat put in. It is reset by
//!   `start_new_hand` and never reset at the end of a hand, so after settlement it
//!   still holds the finished hand's contributions. The recorder also takes the
//!   running maximum after every message, which is what preserves the figure for a
//!   seat that left the table before the hand ended.
//! * `hole_cards`, `community_cards`, `has_folded`, `dealer_seat`.
//!
//! Deliberately NOT used as an oracle input: `state.pot`, `state.side_pots`, or
//! the engine's `Winner` list. Those are recorded as EVIDENCE about the engine and
//! shown in the report, but the oracle never reads them.

use candid::Principal;
use std::collections::{BTreeMap, BTreeSet};

use crate::cards::{cards_str, hole_str};
use crate::oracle::{self, Anomaly, HandFacts, SeatStake, Settlement};
use crate::table_api::*;
use crate::world::World;
use poker_core::Card;

/// One seat's story through the hand.
#[derive(Clone, Debug)]
pub struct SeatTrace {
    pub seat: u8,
    pub principal: Principal,
    pub hole: Option<(Card, Card)>,
    pub contributed: u64,
    pub folded: bool,
    /// The seat was empty at the end of the hand although it started the hand
    /// occupied: the player left, cashed out, or was removed.
    pub vacated: bool,
    pub chips_before: u64,
    pub chips_after: u64,
}

/// Everything observed about one complete hand.
#[derive(Clone, Debug)]
pub struct HandRecord {
    pub hand_number: u64,
    pub dealer_seat: u8,
    pub num_seats: usize,
    pub board: Vec<Card>,
    /// The board the deck said would come, if the deal was read. Checked against
    /// `board` so a wrong prediction cannot silently mislead the scenario search.
    pub predicted_board: Option<Vec<Card>>,
    pub seats: Vec<SeatTrace>,

    // --- engine evidence, never an oracle input --------------------------
    /// `state.side_pots` the first time it was non-empty, i.e. the breakdown the
    /// engine froze when the flop was dealt.
    pub side_pots_first_seen: Vec<SidePot>,
    /// `state.side_pots` on the last observation before the hand completed.
    pub side_pots_at_settlement: Vec<SidePot>,
    pub pot_peak: u64,
    /// The sum of every seat's `total_bet_this_hand` at the first observation in
    /// which a flop was on the table. Anything collected above this went in on the
    /// flop, the turn or the river.
    ///
    /// Measured from the contributions rather than from `side_pots`: the engine now
    /// refreshes its side-pot breakdown after every action, so "the breakdown is
    /// smaller than the pot" no longer means "money went in after the flop" -- it
    /// means the observation caught the breakdown one action behind.
    pub wagered_at_flop: Option<u64>,
    pub engine_winners: Vec<WinnerAmount>,
    pub logs: Vec<String>,
    pub internal_total_before: u64,
    pub internal_total_after: u64,
    /// Net change per principal in `escrow + chips`.
    pub value_deltas: BTreeMap<Principal, i128>,
}

impl HandRecord {
    /// The oracle's inputs, and nothing else.
    pub fn facts(&self) -> HandFacts {
        HandFacts {
            num_seats: self.num_seats,
            dealer_seat: self.dealer_seat,
            board: self.board.clone(),
            stakes: self
                .seats
                .iter()
                .filter(|s| s.contributed > 0)
                .map(|s| SeatStake {
                    seat: s.seat,
                    contributed: s.contributed,
                    // Folding gives up the claim. So does leaving the table: a
                    // player who is not at the table cannot be dealt to, shown, or
                    // paid, and the money they already put in stays in the pot.
                    relinquished: s.folded || s.vacated,
                    hole: if s.folded || s.vacated {
                        // A folded or departed seat is not ranked, so its cards
                        // are irrelevant; passing them would only invite a
                        // duplicate-card complaint from a partial observation.
                        None
                    } else {
                        s.hole
                    },
                })
                .collect(),
        }
    }

    pub fn seat_of(&self, who: Principal) -> Option<u8> {
        self.seats
            .iter()
            .find(|s| s.principal == who)
            .map(|s| s.seat)
    }

    /// Net change per SEAT in what the canister owes the player who occupied it.
    pub fn engine_deltas(&self) -> BTreeMap<u8, i128> {
        self.seats
            .iter()
            .map(|s| {
                (
                    s.seat,
                    self.value_deltas
                        .get(&s.principal)
                        .copied()
                        .unwrap_or(s.chips_after as i128 - s.chips_before as i128),
                )
            })
            .collect()
    }

    /// Chips the hand made vanish: money that was collected and paid to nobody.
    pub fn chips_destroyed(&self) -> i128 {
        self.internal_total_before as i128 - self.internal_total_after as i128
    }
}

// ---------------------------------------------------------------------------
// the recorder
// ---------------------------------------------------------------------------

/// Observes a hand from before it is dealt until after it settles.
///
/// Usage: [`HandRecorder::open`] BEFORE `start_new_hand`, [`HandRecorder::observe`]
/// after every message, [`HandRecorder::close`] once the phase is `HandComplete`.
pub struct HandRecorder {
    values_before: BTreeMap<Principal, u64>,
    internal_before: u64,
    principal_at: BTreeMap<u8, Principal>,
    chips_before: BTreeMap<u8, u64>,
    seats_at_open: BTreeSet<u8>,

    hand_number: u64,
    dealer_seat: u8,
    num_seats: usize,
    hole: BTreeMap<u8, (Card, Card)>,
    contributed: BTreeMap<u8, u64>,
    folded: BTreeSet<u8>,
    present: BTreeSet<u8>,
    board: Vec<Card>,
    side_pots_first_seen: Vec<SidePot>,
    side_pots_latest: Vec<SidePot>,
    pot_peak: u64,
    wagered_at_flop: Option<u64>,
    predicted_board: Option<Vec<Card>>,
    logs: Vec<String>,
}

impl HandRecorder {
    pub fn open(world: &mut World) -> Self {
        let state = world.table_state();
        assert!(
            !state.phase.hand_in_progress(),
            "HandRecorder::open must be called between hands, not in phase {:?}",
            state.phase
        );
        let principal_at: BTreeMap<u8, Principal> =
            state.seated().map(|p| (p.seat, p.principal)).collect();
        let chips_before: BTreeMap<u8, u64> = state.seated().map(|p| (p.seat, p.chips)).collect();
        let seats_at_open: BTreeSet<u8> = principal_at.keys().copied().collect();
        // Drain any log lines from setup so the record only carries this hand's.
        let _ = world.new_canister_logs();
        Self {
            values_before: world.values(),
            internal_before: world.internal_total(),
            principal_at,
            chips_before,
            seats_at_open,
            hand_number: 0,
            dealer_seat: state.dealer_seat,
            num_seats: state.players.len(),
            hole: BTreeMap::new(),
            contributed: BTreeMap::new(),
            folded: BTreeSet::new(),
            present: BTreeSet::new(),
            board: Vec::new(),
            side_pots_first_seen: Vec::new(),
            side_pots_latest: Vec::new(),
            pot_peak: 0,
            wagered_at_flop: None,
            predicted_board: None,
            logs: Vec::new(),
        }
    }

    /// Record the board the deck says will come, so it can be checked later.
    pub fn note_predicted_board(&mut self, board: Vec<Card>) {
        self.predicted_board = Some(board);
    }

    /// Fold the current table state into the record. Cheap: one query.
    pub fn observe(&mut self, world: &mut World) {
        let state = world.table_state();
        if state.hand_number > self.hand_number {
            self.hand_number = state.hand_number;
            self.dealer_seat = state.dealer_seat;
        }
        self.num_seats = state.players.len();
        if state.community_cards.len() > self.board.len() {
            self.board = state.community_cards.clone();
        }
        self.pot_peak = self.pot_peak.max(state.pot);
        if self.wagered_at_flop.is_none() && state.community_cards.len() >= 3 {
            self.wagered_at_flop = Some(
                state
                    .seated()
                    .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand)),
            );
        }
        if !state.side_pots.is_empty() {
            if self.side_pots_first_seen.is_empty() {
                self.side_pots_first_seen = state.side_pots.clone();
            }
            self.side_pots_latest = state.side_pots.clone();
        }
        for p in state.seated() {
            self.present.insert(p.seat);
            if let Some(h) = p.hole_cards {
                self.hole.insert(p.seat, h);
            }
            let entry = self.contributed.entry(p.seat).or_insert(0);
            *entry = (*entry).max(p.total_bet_this_hand);
            if p.has_folded {
                self.folded.insert(p.seat);
            }
            self.principal_at.entry(p.seat).or_insert(p.principal);
            self.chips_before.entry(p.seat).or_insert(p.chips);
        }
        self.logs.extend(world.new_canister_logs());
    }

    pub fn close(mut self, world: &mut World) -> HandRecord {
        self.observe(world);
        let state = world.table_state();
        let values_after = world.values();
        let internal_after = world.internal_total();

        let chips_after: BTreeMap<u8, u64> = state.seated().map(|p| (p.seat, p.chips)).collect();
        let occupied_now: BTreeSet<u8> = chips_after.keys().copied().collect();

        let mut seats: Vec<SeatTrace> = Vec::new();
        let mut all_seats: BTreeSet<u8> = self.seats_at_open.clone();
        all_seats.extend(self.contributed.keys().copied());
        all_seats.extend(occupied_now.iter().copied());
        for seat in all_seats {
            let Some(principal) = self.principal_at.get(&seat).copied() else {
                continue;
            };
            seats.push(SeatTrace {
                seat,
                principal,
                hole: self.hole.get(&seat).copied(),
                contributed: self.contributed.get(&seat).copied().unwrap_or(0),
                folded: self.folded.contains(&seat),
                vacated: self.seats_at_open.contains(&seat) && !occupied_now.contains(&seat),
                chips_before: self.chips_before.get(&seat).copied().unwrap_or(0),
                chips_after: chips_after.get(&seat).copied().unwrap_or(0),
            });
        }

        let mut value_deltas: BTreeMap<Principal, i128> = BTreeMap::new();
        for (who, after) in &values_after {
            let before = self.values_before.get(who).copied().unwrap_or(0);
            value_deltas.insert(*who, *after as i128 - before as i128);
        }
        for (who, before) in &self.values_before {
            value_deltas
                .entry(*who)
                .or_insert(0 - (*before as i128));
        }

        let engine_winners = world
            .hand_history(self.hand_number)
            .map(|h| h.winners)
            .unwrap_or_default();

        HandRecord {
            hand_number: self.hand_number,
            dealer_seat: self.dealer_seat,
            num_seats: self.num_seats,
            board: self.board,
            predicted_board: self.predicted_board,
            seats,
            side_pots_first_seen: self.side_pots_first_seen,
            side_pots_at_settlement: self.side_pots_latest,
            pot_peak: self.pot_peak,
            wagered_at_flop: self.wagered_at_flop,
            engine_winners,
            logs: self.logs,
            internal_total_before: self.internal_before,
            internal_total_after: internal_after,
            value_deltas,
        }
    }
}

// ---------------------------------------------------------------------------
// the comparison
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeatCompare {
    pub seat: u8,
    pub principal: Principal,
    /// What the canister actually did to this seat's money.
    pub engine: i128,
    /// What the rules of poker say should have happened to it.
    pub oracle: i128,
    /// `engine - oracle`. Positive: the seat was overpaid. Negative: underpaid.
    pub diff: i128,
}

#[derive(Clone, Debug)]
pub struct HandComparison {
    pub label: String,
    pub record: HandRecord,
    pub facts: HandFacts,
    pub settlement: Settlement,
    pub seats: Vec<SeatCompare>,
    /// True when every seat's delta matches the oracle exactly.
    pub agrees: bool,
    /// Chips the canister collected and paid to nobody.
    pub destroyed: i128,
    /// The oracle could not apply the rules cleanly to this hand. Reported in its
    /// own bucket rather than counted as a payout defect.
    pub anomalies: Vec<Anomaly>,
}

impl HandComparison {
    pub fn build(label: &str, record: HandRecord) -> Self {
        if let (Some(pred), true) = (record.predicted_board.clone(), record.board.len() == 5) {
            assert_eq!(
                cards_str(&pred),
                cards_str(&record.board),
                "the harness predicted the board {} from the deck but the engine dealt {}. \
                 The deal-prediction in src/deal.rs is wrong, which would silently steer the \
                 scenario search; fix it before trusting any coverage number.",
                cards_str(&pred),
                cards_str(&record.board)
            );
        }
        let facts = record.facts();
        let settlement = oracle::settle(&facts);
        let oracle_deltas = settlement.net_deltas(&facts);
        let engine_deltas = record.engine_deltas();

        let mut seats: Vec<SeatCompare> = Vec::new();
        for trace in &record.seats {
            let engine = engine_deltas.get(&trace.seat).copied().unwrap_or(0);
            let oracle_delta = oracle_deltas.get(&trace.seat).copied().unwrap_or(0);
            seats.push(SeatCompare {
                seat: trace.seat,
                principal: trace.principal,
                engine,
                oracle: oracle_delta,
                diff: engine - oracle_delta,
            });
        }
        let agrees = seats.iter().all(|s| s.diff == 0);
        let anomalies = settlement.anomalies.clone();
        Self {
            label: label.to_string(),
            destroyed: record.chips_destroyed(),
            record,
            facts,
            settlement,
            seats,
            agrees,
            anomalies,
        }
    }

    pub fn worst_diff(&self) -> i128 {
        self.seats.iter().map(|s| s.diff.abs()).max().unwrap_or(0)
    }

    /// True when the engine's `state.pot` was observed HIGHER than every
    /// contribution the harness could see.
    ///
    /// This is the one place the instrument audits its own input. The oracle takes
    /// each seat's stake from `Player::total_bet_this_hand`, which is the engine's
    /// bookkeeping; if that were understated the oracle would be measuring against a
    /// lie. `state.pot` is a second, independent accumulator -- every bet increments
    /// both -- so `pot > sum(total_bet_this_hand)` means the two disagree and the
    /// oracle's input is incomplete for that hand.
    ///
    /// One-sided on purpose. The peak of `state.pot` is only observable when the
    /// hand takes more than one message to settle: an all-in that runs the board out
    /// and pays in a single message has zeroed the pot again before the harness can
    /// look, so a LOWER observed peak means nothing. It is also exactly the E-03
    /// Direction A precondition, which is why it is worth counting.
    pub fn engine_pot_exceeded_observed_contributions(&self) -> bool {
        self.record.pot_peak > self.facts.total_collected()
    }

    pub fn misdirected(&self) -> u64 {
        // Half the sum of absolute differences, when they cancel: that is the
        // amount that went to the wrong seat rather than vanishing.
        let positive: i128 = self.seats.iter().filter(|s| s.diff > 0).map(|s| s.diff).sum();
        positive.max(0) as u64
    }

    /// A full, self-contained reproducer: the cards, the money, what the engine
    /// paid, what was owed, and the per-seat delta.
    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "--- {} | hand #{} | button seat {} | {} seats\n",
            self.label, self.record.hand_number, self.record.dealer_seat, self.record.num_seats
        ));
        out.push_str(&format!("board: {}\n", cards_str(&self.record.board)));
        out.push_str("seat  hole    contributed  folded  left   engine_delta  oracle_delta  DIFF\n");
        for s in &self.seats {
            let trace = self
                .record
                .seats
                .iter()
                .find(|t| t.seat == s.seat)
                .expect("trace for compared seat");
            out.push_str(&format!(
                "  {:>2}  {:<6}  {:>11}  {:>6}  {:>4}  {:>12}  {:>12}  {:>+6}\n",
                s.seat,
                hole_str(&trace.hole),
                trace.contributed,
                if trace.folded { "yes" } else { "no" },
                if trace.vacated { "yes" } else { "no" },
                s.engine,
                s.oracle,
                s.diff
            ));
        }
        out.push_str(&format!(
            "collected {}   oracle owed {}   engine paid {}   destroyed {}   \
             engine state.pot peak seen {}\n",
            self.facts.total_collected(),
            self.settlement.total_owed(),
            self.record
                .engine_winners
                .iter()
                .fold(0u64, |a, w| a.saturating_add(w.amount)),
            self.destroyed,
            self.record.pot_peak
        ));
        if self.engine_pot_exceeded_observed_contributions() {
            out.push_str(
                "WARNING: state.pot was observed HIGHER than the sum of every seat's \
                 total_bet_this_hand. The engine's two accounts of the pot disagree, so \
                 the oracle's input for this hand is incomplete.\n",
            );
        }
        if let Some((seat, amount)) = self.settlement.uncalled {
            out.push_str(&format!(
                "oracle: uncalled bet of {amount} returned to seat {seat}\n"
            ));
        }
        out.push_str("oracle pot layers:\n");
        for l in &self.settlement.layers {
            if l.amount == 0 {
                continue;
            }
            out.push_str(&format!(
                "  layer {} ({}..{}]  amount {:>8}  eligible {:?}  winners {:?}\n",
                l.index, l.lo, l.hi, l.amount, l.eligible, l.winners
            ));
        }
        out.push_str(&format!(
            "engine side_pots frozen at flop: {:?}\n",
            self.record.side_pots_first_seen
        ));
        out.push_str(&format!(
            "engine winner list: {:?}\n",
            self.record
                .engine_winners
                .iter()
                .map(|w| (w.seat, w.amount))
                .collect::<Vec<_>>()
        ));
        if !self.anomalies.is_empty() {
            out.push_str(&format!("oracle anomalies: {:?}\n", self.anomalies));
        }
        for line in &self.record.logs {
            if line.contains("BUG") || line.contains("CRITICAL") || line.contains("WARNING") {
                out.push_str(&format!("canister log: {}\n", line.trim_end()));
            }
        }
        out
    }
}
