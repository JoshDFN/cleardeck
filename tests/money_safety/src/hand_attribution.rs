//! WHO WAS PAID, measured on every hand, not only on the scripted ones.
//!
//! # The hole this closes
//!
//! M8 (`invariants::attribution`) was written to convict
//! docs/SECURITY-FINDINGS.md FINDING 13, and it does. But it was reachable only
//! from two hand-written fixtures, and the corollary that ran on an ordinary hand
//! -- "a principal who staked nothing cannot come out richer" -- exempts every
//! player who DID stake. So a pot share paid to the wrong LIVE player passed the
//! whole harness: totals conserve, the seat is right or wrong in a way nothing
//! aggregate can see, and the one principal-level assertion that ran on ordinary
//! hands does not apply to somebody who was in the hand.
//!
//! This module makes attribution a property of EVERY completed hand. It watches a
//! hand happen -- one query per step, the snapshot the fuzzer already takes -- and
//! at settlement produces the four numbers that let a caller ask, per principal:
//!
//! ```text
//!   staked      what THIS PERSON put in, measured from total_bet_this_hand and
//!               from departed_stakes, never from the payout code
//!   claimed     what the canister's own hand history says it paid THEM
//!   measured    what actually happened to their escrow + chips
//!   owed        what the rules of poker say they should have (settlement oracle)
//! ```
//!
//! Four numbers, three independent comparisons, in
//! [`crate::invariants::attribution`]:
//!
//! * `measured == claimed - staked`  — the canister paid the person it says it
//!   paid. Convicts every defect on the APPLY side: a credit to the wrong wallet,
//!   a stack credited to the wrong chair, two winners' amounts swapped after the
//!   record was written, an aggregation in `push_winner` that reports two people's
//!   money under one name.
//! * `measured == owed - staked` — the person paid is the person the RULES name.
//!   Convicts the defects that are self-consistent: a plan that names the wrong
//!   live player, or that lets a folded seat win a layer.
//! * the two oracle-free corollaries: nobody who staked nothing may gain, and a
//!   principal whose every stake was relinquished may not come out ahead.
//!
//! # Why the measurement is `escrow + chips`
//!
//! A player who leaves mid-hand has their stack moved to escrow, so `chips` alone
//! reads as a total loss where nothing was lost. Chips in the POT belong to nobody
//! while the hand is live, which is exactly why the quantity dips by a player's
//! stake and comes back at settlement.
//!
//! # When the instrument declines to answer
//!
//! Reconstructing what each person staked is an OBSERVATION, and an observation
//! can be incomplete: `departed_stakes` is cleared at settlement, so a hand that
//! both starts and finishes inside one unobserved message can hide a departure.
//! Rather than report a false misattribution, the recorder cross-checks its own
//! reconstruction (`sum(staked) == sum(claimed)`) and marks the hand
//! [`HandAttribution::complete`]. Callers count the incomplete ones out loud --
//! `make fuzz` prints the ratio -- because an instrument that silently skips is
//! indistinguishable from one that passes.

use candid::Principal;
use std::collections::{BTreeMap, BTreeSet};

use crate::invariants::attribution::values_by_principal;
use crate::table_api::{Card, GamePhase, HandHistoryAmounts, TableState};
use crate::world::{Snapshot, World};

// ---------------------------------------------------------------------------
// what one finished hand looks like, per person
// ---------------------------------------------------------------------------

/// The settlement oracle's answer for one hand, attributed to people.
#[derive(Clone, Debug)]
pub struct OracleAnswer {
    /// Net change the rules of poker say each principal's `escrow + chips` should
    /// show: what they are owed out of the pot minus what they put in.
    pub net: BTreeMap<Principal, i128>,
    /// Inputs the rules do not define an answer for. A hand with any of these is
    /// not used for the oracle leg.
    pub anomalies: Vec<String>,
}

/// Everything measured about who was paid in one completed hand.
#[derive(Clone, Debug)]
pub struct HandAttribution {
    pub hand_number: u64,
    pub phase: GamePhase,
    /// Measured change in `escrow + chips`, per principal, across the whole hand.
    pub measured: BTreeMap<Principal, i128>,
    /// What each principal put into the pot, from `total_bet_this_hand` and
    /// `departed_stakes`. Never from the payout code.
    pub staked: BTreeMap<Principal, u64>,
    /// What the canister's own hand history says it paid each principal.
    pub claimed: BTreeMap<Principal, u64>,
    /// Principals every one of whose stakes in this hand was given up -- folded, or
    /// the seat was vacated -- and who therefore hold no claim on any layer.
    pub relinquished_only: BTreeSet<Principal>,
    /// The oracle's answer, when the hand is one the rules define cleanly.
    pub oracle: Option<OracleAnswer>,
    /// People whose money crossed the canister boundary during this hand -- a
    /// deposit, a withdrawal, a sweep -- so their `escrow + chips` moved for a
    /// reason that has nothing to do with the pot. Excluded from the per-person
    /// comparisons, and from those alone: everybody else at the table is still
    /// measured.
    pub tainted: BTreeSet<Principal>,
    /// True when nothing crossed the canister boundary during the hand in a way
    /// the harness cannot attribute to one person (a raw transfer being credited,
    /// a deposit subaccount being swept).
    pub closed_world: bool,
    /// True when the reconstruction of who staked what balances against what the
    /// canister says it paid out. False means the harness did not see enough of
    /// the hand to attribute it, NOT that the hand is wrong.
    pub complete: bool,
    /// Why `complete` is false, for the report.
    pub incompleteness: Option<String>,
}

impl HandAttribution {
    pub fn staked_total(&self) -> u64 {
        self.staked.values().fold(0u64, |a, v| a.saturating_add(*v))
    }

    pub fn claimed_total(&self) -> u64 {
        self.claimed.values().fold(0u64, |a, v| a.saturating_add(*v))
    }

    /// Every principal the hand touched in a way this instrument can speak about.
    ///
    /// [`HandAttribution::tainted`] people are left out: their escrow moved for a
    /// reason outside the hand, so no statement of the form
    /// `value_delta == award - stake` holds for them and asserting one would be a
    /// fabricated finding. Everybody else at the same table is still measured, which
    /// is the whole reason the exclusion is per person rather than per hand.
    pub fn everyone(&self) -> BTreeSet<Principal> {
        let mut out: BTreeSet<Principal> = self.measured.keys().copied().collect();
        out.extend(self.staked.keys().copied());
        out.extend(self.claimed.keys().copied());
        for who in &self.tainted {
            out.remove(who);
        }
        out
    }

    /// True when this hand is worth asserting on at all.
    pub fn is_measurable(&self) -> bool {
        self.complete && self.closed_world
    }

    /// A one-line summary for a report.
    pub fn summary(&self) -> String {
        format!(
            "hand #{} staked={} claimed={} people={} oracle={} complete={} closed_world={}",
            self.hand_number,
            self.staked_total(),
            self.claimed_total(),
            self.everyone().len(),
            match &self.oracle {
                Some(o) if o.anomalies.is_empty() => "yes",
                Some(_) => "anomalous",
                None => "no",
            },
            self.complete,
            self.closed_world
        )
    }
}

// ---------------------------------------------------------------------------
// the recorder
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Baseline {
    values: BTreeMap<Principal, u64>,
    deposit_subaccounts: u64,
    uncredited_raw: u64,
    hand_number: u64,
}

/// One hand's worth of accumulated observation.
#[derive(Clone, Debug)]
struct Accumulator {
    hand_number: u64,
    dealer_seat: u8,
    num_seats: usize,
    /// Running maximum of `total_bet_this_hand` per (seat, owner). Two owners CAN
    /// share a chair in one hand (docs/DEFECTS.md E-36), which is why the key is
    /// not the seat alone.
    ///
    /// Used only for a (seat, owner) whose chair is no longer theirs at the end:
    /// their last live figure is the most that was ever in the pot for them.
    stakes: BTreeMap<(u8, Principal), u64>,
    /// The MOST RECENT `total_bet_this_hand` per (seat, owner).
    ///
    /// This, and not the maximum, is what a still-seated player actually staked.
    /// An uncalled bet is handed back DURING the hand -- the engine reduces
    /// `total_bet_this_hand` and the pot together and pays no `Winner` record for
    /// it -- so the peak counts money that was never contested. Measured against
    /// the peak, an ordinary heads-up hand where the big blind is not called reads
    /// as "staked 3000000, awarded 2000000" and the recorder declines a hand that
    /// is perfectly fine.
    last_stake: BTreeMap<(u8, Principal), u64>,
    /// Who is sitting where, as of the latest observation.
    occupancy: BTreeMap<u8, Principal>,
    /// The cards seen at a (seat, owner). Kept as a running record because the
    /// engine clears hole cards at settlement.
    hole: BTreeMap<(u8, Principal), (Card, Card)>,
    folded: BTreeSet<(u8, Principal)>,
    /// (seat, owner) pairs whose owner is no longer in that chair.
    departed: BTreeSet<(u8, Principal)>,
    board: Vec<Card>,
}

impl Accumulator {
    fn new(t: &TableState) -> Self {
        Self {
            hand_number: t.hand_number,
            dealer_seat: t.dealer_seat,
            num_seats: t.players.len(),
            stakes: BTreeMap::new(),
            last_stake: BTreeMap::new(),
            occupancy: BTreeMap::new(),
            hole: BTreeMap::new(),
            folded: BTreeSet::new(),
            departed: BTreeSet::new(),
            board: Vec::new(),
        }
    }

    /// What each (seat, owner) ACTUALLY had in the pot when the hand settled.
    ///
    /// The last figure seen for a chair still held by that owner; the running
    /// maximum otherwise, because a stake whose owner has left the chair has no
    /// later figure and `departed_stakes` records exactly what was in the pot at
    /// the moment they went.
    fn effective_stakes(&self) -> BTreeMap<(u8, Principal), u64> {
        self.stakes
            .iter()
            .map(|((seat, who), max)| {
                let still_theirs = self.occupancy.get(seat) == Some(who);
                let amount = if still_theirs {
                    self.last_stake.get(&(*seat, *who)).copied().unwrap_or(*max)
                } else {
                    *max
                };
                ((*seat, *who), amount)
            })
            .filter(|(_, amount)| *amount > 0)
            .collect()
    }

    fn observe(&mut self, t: &TableState) {
        self.dealer_seat = t.dealer_seat;
        self.num_seats = self.num_seats.max(t.players.len());
        if t.community_cards.len() > self.board.len() {
            self.board = t.community_cards.clone();
        }
        let mut occupied: BTreeMap<u8, Principal> = BTreeMap::new();
        for p in t.seated() {
            occupied.insert(p.seat, p.principal);
            if let Some(h) = p.hole_cards {
                self.hole.insert((p.seat, p.principal), h);
            }
            if p.total_bet_this_hand > 0 {
                let e = self.stakes.entry((p.seat, p.principal)).or_insert(0);
                *e = (*e).max(p.total_bet_this_hand);
                self.last_stake
                    .insert((p.seat, p.principal), p.total_bet_this_hand);
            } else if self.stakes.contains_key(&(p.seat, p.principal)) {
                // Everything they put in came back as an uncalled bet.
                self.last_stake.insert((p.seat, p.principal), 0);
            }
            if p.has_folded {
                self.folded.insert((p.seat, p.principal));
            }
        }
        // `departed_stakes` is the ONLY record of whose money is in the pot once a
        // chair is empty, and it is cleared at settlement, so it has to be read
        // while the hand is live.
        for d in t.departed() {
            if d.hand_number != self.hand_number || d.contributed == 0 {
                continue;
            }
            let e = self.stakes.entry((d.seat, d.principal)).or_insert(0);
            *e = (*e).max(d.contributed);
            self.departed.insert((d.seat, d.principal));
        }
        self.occupancy = occupied.clone();
        // A stake whose owner is not in that chair any more has been given up, even
        // when the chair itself is occupied by somebody else.
        let keys: Vec<(u8, Principal)> = self.stakes.keys().copied().collect();
        for (seat, who) in keys {
            if occupied.get(&seat) != Some(&who) {
                self.departed.insert((seat, who));
            }
        }
    }
}

/// Watches hands go by and produces one [`HandAttribution`] per completed hand.
///
/// Feed it [`HandAttributionWatch::observe`] after every message. It is a pure
/// function of the snapshots it is shown, so the fuzzer, the hand-written
/// scenarios and a regression reproducer all measure the same way.
#[derive(Clone, Debug, Default)]
pub struct HandAttributionWatch {
    baseline: Option<Baseline>,
    acc: Option<Accumulator>,
    /// Every actor's own ledger wallet at the previous observation. A change in it
    /// is a deposit, a withdrawal or a raw transfer: money crossing the canister
    /// boundary, which moves that ONE person's escrow for a reason the pot cannot
    /// explain.
    last_wallets: BTreeMap<Principal, u64>,
    /// Who that has happened to since the current baseline.
    tainted: BTreeSet<Principal>,
    /// Hands the watch produced an attribution for.
    pub hands_seen: u64,
    /// Hands it could measure (`complete && closed_world`).
    pub hands_measured: u64,
    /// Hands the oracle leg ran on.
    pub hands_oracled: u64,
    /// Why hands were not measured, counted by reason.
    pub declined: BTreeMap<String, u64>,
}

impl HandAttributionWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one snapshot. Returns an attribution when a hand has just finished.
    ///
    /// `history` is only read at the instant a hand settles, so callers that would
    /// have to pay for a query to obtain it should use [`Self::observe_lazily`].
    pub fn observe(
        &mut self,
        snap: &Snapshot,
        uncredited_raw: u64,
        history: Option<&HandHistoryAmounts>,
    ) -> Option<HandAttribution> {
        let owned = history.cloned();
        self.observe_lazily(snap, uncredited_raw, |_| owned)
    }

    /// As [`Self::observe`], but the hand history is fetched only when a hand has
    /// actually just settled.
    ///
    /// # Why the laziness is deliberate
    ///
    /// The history is only needed at the instant a hand settles. Fetching it on
    /// every step is 220 extra replica messages in a default fuzz run, for one
    /// answer per hand.
    ///
    /// It was also, on first suspicion, the reason a fuzz seed stopped
    /// reproducing. It was not -- MEASURED, and the truth is worse. Seed
    /// `0xc1b1576c1102` at 400 steps plays 4 hands and finds nothing when it is the
    /// only seed in the process, and 5 hands with two blocking findings when seed
    /// `0xc1b1576c1101` ran before it in the same process. That is true with the
    /// per-step query and equally true without it, and true on a pristine
    /// `git archive HEAD` tree that has never seen this file: consecutive
    /// `World`s in one test binary do not get independent subnet randomness, so a
    /// run is NOT the pure function of `(seed, config, actors, steps)` that
    /// `crate::fuzz` documents. See docs/DEFECTS.md H-24. Keeping the message
    /// stream minimal does not fix that, but it stops this file from adding to it.
    pub fn observe_lazily(
        &mut self,
        snap: &Snapshot,
        uncredited_raw: u64,
        fetch_history: impl FnOnce(u64) -> Option<HandHistoryAmounts>,
    ) -> Option<HandAttribution> {
        let t = &snap.table;

        // Who has had money cross the canister boundary since the last observation?
        for (who, balance) in &snap.actor_wallets {
            if self.last_wallets.get(who).is_some_and(|b| b != balance) {
                self.tainted.insert(*who);
            }
        }
        self.last_wallets = snap.actor_wallets.clone();

        // Start (or restart) the accumulator when the hand number moves.
        match &self.acc {
            Some(a) if a.hand_number == t.hand_number => {}
            _ if t.hand_number > 0 => self.acc = Some(Accumulator::new(t)),
            _ => {}
        }
        if let Some(acc) = self.acc.as_mut() {
            if acc.hand_number == t.hand_number {
                acc.observe(t);
            }
        }

        let idle = !t.phase.hand_in_progress() && t.pot == 0;
        let mut produced = None;

        if idle {
            if let (Some(base), Some(acc)) = (self.baseline.clone(), self.acc.clone()) {
                if t.hand_number == base.hand_number + 1 && acc.hand_number == t.hand_number {
                    let history = fetch_history(t.hand_number);
                    produced =
                        Some(self.build(&base, &acc, snap, uncredited_raw, history.as_ref()));
                }
            }
            // Whatever happened, this is the new baseline.
            self.baseline = Some(Baseline {
                values: values_by_principal(snap),
                deposit_subaccounts: snap.ledger_deposit_subaccounts,
                uncredited_raw,
                hand_number: t.hand_number,
            });
            self.tainted.clear();
        }
        produced
    }

    fn build(
        &mut self,
        base: &Baseline,
        acc: &Accumulator,
        snap: &Snapshot,
        uncredited_raw: u64,
        history: Option<&HandHistoryAmounts>,
    ) -> HandAttribution {
        let after = values_by_principal(snap);
        let measured = signed_deltas(&base.values, &after);

        let effective = acc.effective_stakes();
        let mut staked: BTreeMap<Principal, u64> = BTreeMap::new();
        for ((_, who), amount) in &effective {
            let e = staked.entry(*who).or_insert(0);
            *e = e.saturating_add(*amount);
        }

        let mut claimed: BTreeMap<Principal, u64> = BTreeMap::new();
        if let Some(h) = history {
            for w in &h.winners {
                let e = claimed.entry(w.principal).or_insert(0);
                *e = e.saturating_add(w.amount);
            }
        }

        // Somebody holds a claim at a chair only while they are still in it, still
        // hold cards and have not folded.
        let mut relinquished_only: BTreeSet<Principal> = BTreeSet::new();
        let mut has_live_claim: BTreeSet<Principal> = BTreeSet::new();
        for (seat, who) in effective.keys() {
            let key = (*seat, *who);
            if !acc.folded.contains(&key) && !acc.departed.contains(&key) {
                has_live_claim.insert(*who);
            }
        }
        for who in staked.keys() {
            if !has_live_claim.contains(who) {
                relinquished_only.insert(*who);
            }
        }

        // A raw transfer being CREDITED, or a deposit subaccount being swept, adds
        // to somebody's escrow without moving their wallet in the same window, so
        // it cannot be attributed to one person. Those two are the only remaining
        // whole-hand bail-outs; a plain deposit or withdrawal taints one player.
        let closed_world = base.uncredited_raw == uncredited_raw
            && base.deposit_subaccounts == snap.ledger_deposit_subaccounts;
        let tainted = self.tainted.clone();

        let staked_total = staked.values().fold(0u64, |a, v| a.saturating_add(*v));
        let claimed_total = claimed.values().fold(0u64, |a, v| a.saturating_add(*v));
        let (complete, incompleteness) = if history.is_none() {
            (
                false,
                Some(format!("no hand history recorded for hand {}", acc.hand_number)),
            )
        } else if staked_total != claimed_total {
            (
                false,
                Some(format!(
                    "the observed stakes sum to {staked_total} but the canister's own record \
                     awards {claimed_total}: the harness did not see the whole hand, so it \
                     cannot say who was owed what. (A rake or a destroyed chip is M3's \
                     question, not this one.)"
                )),
            )
        } else if staked_total == 0 {
            (false, Some("the hand collected nothing".to_string()))
        } else {
            (true, None)
        };

        let oracle = if complete && closed_world {
            oracle_answer(acc, &effective, &staked)
        } else {
            None
        };

        self.hands_seen += 1;
        if complete && closed_world {
            self.hands_measured += 1;
        } else {
            let reason = if !closed_world {
                "money crossed the canister boundary during the hand".to_string()
            } else {
                incompleteness
                    .clone()
                    .unwrap_or_else(|| "unstated".to_string())
            };
            *self.declined.entry(short_reason(&reason)).or_insert(0) += 1;
        }
        if oracle.as_ref().is_some_and(|o| o.anomalies.is_empty()) {
            self.hands_oracled += 1;
        }

        for who in &tainted {
            relinquished_only.remove(who);
        }

        let out = HandAttribution {
            hand_number: acc.hand_number,
            phase: snap.table.phase.clone(),
            measured,
            staked,
            claimed,
            relinquished_only,
            oracle,
            tainted,
            closed_world,
            complete,
            incompleteness,
        };
        if !out.is_measurable() && std::env::var("MONEY_ATTRIBUTION_DEBUG").as_deref() == Ok("1") {
            eprintln!(
                "M8 declined hand #{}: {}\n  staked={:?}\n  claimed={:?}\n  measured={:?}\n  \
                 tainted={:?}",
                out.hand_number,
                out.incompleteness.as_deref().unwrap_or("closed_world=false"),
                out.staked,
                out.claimed,
                out.measured,
                out.tainted
            );
        }
        out
    }
}

fn short_reason(reason: &str) -> String {
    reason.chars().take(64).collect()
}

fn signed_deltas(
    before: &BTreeMap<Principal, u64>,
    after: &BTreeMap<Principal, u64>,
) -> BTreeMap<Principal, i128> {
    let mut who: BTreeSet<Principal> = before.keys().copied().collect();
    who.extend(after.keys().copied());
    who.into_iter()
        .map(|p| {
            let b = before.get(&p).copied().unwrap_or(0) as i128;
            let a = after.get(&p).copied().unwrap_or(0) as i128;
            (p, a - b)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// the oracle leg
// ---------------------------------------------------------------------------

/// Derive what the RULES OF POKER owe each principal in this hand.
///
/// The rules themselves come from `settlement_oracle::oracle`, the independent
/// derivation written for `tests/settlement` -- deliberately the same code, so
/// there is exactly one statement of the rules in this repo and the fuzzer cannot
/// drift into agreeing with a bug the deliberate suite would catch.
///
/// Returns `None` when this hand cannot be attributed to people at all: a chair
/// that carried two owners' money has a per-seat answer the rules of poker do not
/// split between them (an orphan layer is returned one chip at a time, clockwise,
/// which is a statement about chairs). Those hands are covered by the other three
/// legs and counted, never silently dropped.
fn oracle_answer(
    acc: &Accumulator,
    effective: &BTreeMap<(u8, Principal), u64>,
    staked: &BTreeMap<Principal, u64>,
) -> Option<OracleAnswer> {
    use settlement_oracle::oracle::{settle, HandFacts, SeatStake};

    // One entry per chair, and the owner of that chair's money.
    let mut per_seat: BTreeMap<u8, Vec<(Principal, u64)>> = BTreeMap::new();
    for ((seat, who), amount) in effective {
        per_seat.entry(*seat).or_default().push((*who, *amount));
    }
    let mut owner_of: BTreeMap<u8, Principal> = BTreeMap::new();
    let mut stakes: Vec<SeatStake> = Vec::new();
    for (seat, owners) in &per_seat {
        if owners.len() != 1 {
            return None; // one chair, two people's money: see the doc comment.
        }
        let (who, amount) = owners[0];
        let key = (*seat, who);
        let relinquished = acc.folded.contains(&key) || acc.departed.contains(&key);
        owner_of.insert(*seat, who);
        stakes.push(SeatStake {
            seat: *seat,
            contributed: amount,
            relinquished,
            hole: if relinquished {
                None
            } else {
                acc.hole.get(&key).map(|(a, b)| (a.to_engine(), b.to_engine()))
            },
        });
    }
    if stakes.is_empty() {
        return None;
    }

    let facts = HandFacts {
        num_seats: acc.num_seats.max(1),
        dealer_seat: acc.dealer_seat,
        board: acc.board.iter().map(|c| c.to_engine()).collect(),
        stakes,
    };
    let settlement = settle(&facts);
    let per_seat_net = settlement.net_deltas(&facts);

    let mut net: BTreeMap<Principal, i128> = BTreeMap::new();
    for (seat, delta) in &per_seat_net {
        let Some(who) = owner_of.get(seat) else {
            return None;
        };
        *net.entry(*who).or_insert(0) += *delta;
    }
    // Anybody who was in the world but not in this hand is owed exactly nothing.
    for who in staked.keys() {
        net.entry(*who).or_insert(0);
    }

    Some(OracleAnswer {
        net,
        anomalies: settlement
            .anomalies
            .iter()
            .map(|a| format!("{a:?}"))
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// convenience for hand-written tests
// ---------------------------------------------------------------------------

/// Observe `world` once through a fresh watch. Useful for a test that drives a
/// hand itself and wants the attribution at the end.
pub fn observe_now(
    watch: &mut HandAttributionWatch,
    world: &World,
) -> Option<HandAttribution> {
    let snap = world.snapshot();
    let history = world.hand_history(snap.table.hand_number);
    watch.observe(&snap, world.uncredited_raw_deposits, history.as_ref())
}
