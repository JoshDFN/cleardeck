//! Reaching the interesting settlements ON PURPOSE.
//!
//! The task this crate answers is explicit that the hard cases must be reached
//! deliberately and not hoped for. Two mechanisms make that possible against the
//! real canister, with no engine modification and no faucet:
//!
//! # 1. Exact stacks, via a snapshot baseline
//!
//! `buy_in(seat, amount)` takes an exact amount, so the ladder of all-in depths is
//! chosen, not observed. To keep those depths across many hands the harness takes a
//! canister snapshot of the seated, between-hands table and restores it before
//! every scenario. Stacks, escrow, hand number and the button all come back
//! identical, so each scenario runs against the same known starting position.
//!
//! # 2. The deal we want, via rewind-and-re-deal
//!
//! The shuffle comes from `raw_rand`, which no test can choose. But after
//! `start_new_hand` the DECK IS FULLY DETERMINED and a controller can read it
//! (`src/deal.rs`), so the harness knows every seat's hole cards and the exact
//! five-card board that will come before any chip has moved. If that deal is not
//! the one a scenario needs -- say a three-way exact tie -- the snapshot is
//! restored and the hand is dealt again. The subnet's randomness is not part of
//! canister state, so each re-deal is a fresh shuffle.
//!
//! That turns "an exact three-way chop" from a 1-in-a-few-hundred accident into a
//! search of a few hundred single-message deals, and it is why the coverage report
//! can show a non-zero count for classes a purely random driver would miss.

use std::time::Duration;

use crate::coverage::Coverage;
use crate::deal::DealPlan;
use crate::drive::{self, Policy};
use crate::observe::{HandComparison, HandRecorder};
use crate::table_api::*;
use crate::world::World;

/// Default ceiling on rewind-and-re-deal attempts per targeted scenario.
pub const DEFAULT_ATTEMPTS: usize = 1_500;

/// A seated table plus the snapshot that restores it.
pub struct Bench {
    pub world: World,
    baseline: Vec<u8>,
    pub coverage: Coverage,
    pub comparisons: Vec<HandComparison>,
    pub disagreements: Vec<HandComparison>,
    /// Total rewind-and-re-deal attempts spent, so the cost of the search is
    /// visible in the report.
    pub deal_attempts: usize,
    /// Targets the search never hit, with the attempts spent.
    pub unhit_targets: Vec<(String, usize)>,
    /// Scenario labels that are ALLOWED to settle differently from the rules of
    /// poker. Every other hand this bench runs must agree, per hand, or the run
    /// fails where the hand ran -- see [`Bench::gate`].
    ///
    /// It is empty, and it must stay empty. E-01, E-03, E-05 and the odd-chip rule
    /// were fixed in wave 2 and the oracle went quiet on all 17 deliberate hands.
    /// Adding a label here is admitting a payout defect is shipping, and it needs an
    /// entry in `docs/DEFECTS.md` in the same change.
    pub allow_disagreement: Vec<String>,
    /// Record disagreements instead of failing on them. Set ONLY by the planted-bug
    /// proof (`SETTLEMENT_EXPECT_WRONG_SEAT=1`), which deliberately runs an engine
    /// with a payout bug in it and has to be able to observe the oracle firing.
    pub record_disagreements_without_failing: bool,
    seats: Vec<(String, u8, u64)>,
}

impl Bench {
    /// Fund every actor, seat them with exact stacks, and snapshot the position.
    pub fn new(config: TableConfig, seats: &[(&str, u8, u64)], escrow_each: u64) -> Self {
        Self::with_bystanders(config, seats, &[], escrow_each)
    }

    /// As [`Bench::new`], plus principals who exist and have escrow but are NOT
    /// seated.
    ///
    /// Needed for the docs/SECURITY-FINDINGS.md FINDING 13 shape: somebody has to
    /// be available to take a chair that a player vacates MID-HAND, and they must
    /// not have been at the table when the hand was dealt.
    pub fn with_bystanders(
        config: TableConfig,
        seats: &[(&str, u8, u64)],
        bystanders: &[&str],
        escrow_each: u64,
    ) -> Self {
        let mut names: Vec<&str> = seats.iter().map(|(n, _, _)| *n).collect();
        names.extend_from_slice(bystanders);
        let world = World::new(config, &names);
        drive::fund(&world, &names, escrow_each);
        drive::seat_exact(&world, seats);
        let baseline = world.take_snapshot(None);
        Self {
            world,
            baseline,
            coverage: Coverage::default(),
            comparisons: Vec::new(),
            disagreements: Vec::new(),
            deal_attempts: 0,
            unhit_targets: Vec::new(),
            allow_disagreement: Vec::new(),
            // `SETTLEMENT_RECORD_ONLY=1` turns the per-hand gate into a recorder. It
            // exists so a "before" can be measured: point the harness at an engine
            // that HAS a payout defect (a reverted copy of the tree, or a planted
            // bug) and get the full count per class instead of a failure at the first
            // bad hand. It must never be set in CI.
            record_disagreements_without_failing: std::env::var("SETTLEMENT_RECORD_ONLY")
                .as_deref()
                == Ok("1"),
            seats: seats
                .iter()
                .map(|(n, s, a)| (n.to_string(), *s, *a))
                .collect(),
        }
    }

    /// The micro-stakes ladder used by most scenarios: five seats at five
    /// deliberately different depths, so an all-in from everybody produces four
    /// distinct side pots.
    pub fn ladder() -> Self {
        Self::new(
            TableConfig::micro_six_max(),
            &[
                ("alice", 0, 20),
                ("bob", 1, 40),
                ("carol", 2, 80),
                ("dave", 3, 160),
                ("erin", 4, 400),
            ],
            1_000_000,
        )
    }

    /// Restore the seated, between-hands position.
    pub fn rewind(&self) {
        self.world.load_snapshot(&self.baseline);
    }

    /// Re-seat everyone with new exact stacks and make that the new baseline.
    pub fn set_stacks(&mut self, seats: &[(&str, u8, u64)]) {
        self.rewind();
        let state = self.world.table_state();
        for p in state.seated() {
            let _ = self.world.leave_table(p.principal);
        }
        drive::seat_exact(&self.world, seats);
        self.baseline = self.world.take_snapshot(Some(self.baseline.clone()));
        self.seats = seats
            .iter()
            .map(|(n, s, a)| (n.to_string(), *s, *a))
            .collect();
    }

    /// Move the button one seat on and re-snapshot, by playing one throwaway hand.
    /// Used so the odd-chip rule is not only ever tested from one button position.
    pub fn advance_button(&mut self) {
        self.rewind();
        let mut rec = HandRecorder::open(&mut self.world);
        if drive::deal(&mut self.world, &mut rec).is_ok() {
            drive::play_out(&mut self.world, &mut rec, drive::everyone_folds(), 60);
        }
        self.world.advance(Duration::from_secs(4));
        let seats: Vec<(&str, u8, u64)> = self
            .seats
            .iter()
            .map(|(n, s, a)| (n.as_str(), *s, *a))
            .collect();
        // Re-seat from escrow so the new baseline has clean, equal-to-intent stacks
        // even though the throwaway hand moved chips around.
        let state = self.world.table_state();
        for p in state.seated() {
            let _ = self.world.leave_table(p.principal);
        }
        drive::seat_exact(&self.world, &seats);
        self.baseline = self.world.take_snapshot(Some(self.baseline.clone()));
    }

    pub fn seat_of(&self, name: &str) -> u8 {
        self.seats
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, s, _)| *s)
            .unwrap_or_else(|| panic!("no seat for {name}"))
    }

    /// Run one hand.
    ///
    /// `target`, if given, is searched for by rewind-and-re-deal. `play` drives the
    /// betting; anything it leaves unfinished is walked to the end passively.
    pub fn hand(
        &mut self,
        label: &str,
        target: Option<&dyn Fn(&DealPlan) -> bool>,
        max_attempts: usize,
        play: &mut dyn FnMut(&mut World, &mut HandRecorder, &DealPlan),
    ) -> Option<HandComparison> {
        self.rewind();
        let mut attempts = 0usize;
        let (mut recorder, plan) = loop {
            let mut recorder = HandRecorder::open(&mut self.world);
            let plan = match drive::deal(&mut self.world, &mut recorder) {
                Ok(p) => p,
                Err(e) => panic!("[{label}] could not deal: {e}"),
            };
            let ok = match target {
                None => true,
                Some(pred) => pred(&plan),
            };
            if ok {
                break (recorder, plan);
            }
            attempts += 1;
            self.deal_attempts += 1;
            if attempts >= max_attempts {
                self.unhit_targets.push((label.to_string(), attempts));
                self.rewind();
                return None;
            }
            self.rewind();
        };

        play(&mut self.world, &mut recorder, &plan);
        // Whatever the script left unfinished, finish passively.
        drive::play_out(&mut self.world, &mut recorder, drive::passive(), 200);
        let record = recorder.close(&mut self.world);
        let cmp = HandComparison::build(label, record);
        self.coverage.record(&cmp);
        if !cmp.agrees {
            self.disagreements.push(cmp.clone());
        }
        self.comparisons.push(cmp.clone());
        self.gate(&cmp);
        Some(cmp)
    }

    /// EVERY HAND IS GATED, WHERE IT RAN.
    ///
    /// The first version of this crate recorded disagreements and left the deciding
    /// to the caller, and its critic showed what that costs: a conserving wrong-seat
    /// payout was planted on a path the suite hits four times per run and all 37
    /// tests stayed green, because the only failing assertion was an 8-hand probe
    /// that never placed an odd chip and ignored errors of one chip, while the class
    /// and randomised tests asserted nothing per hand.
    ///
    /// So the assertion lives here now: any hand whose per-seat deltas differ from
    /// the oracle fails the test that ran it, with the whole reproducer printed, and
    /// the only way to pass is an explicit label in [`Bench::allow_disagreement`].
    /// A payout that lands the right totals at the wrong seats cannot pass.
    fn gate(&self, cmp: &HandComparison) {
        if cmp.agrees || self.record_disagreements_without_failing {
            return;
        }
        if self.allow_disagreement.iter().any(|l| *l == cmp.label) {
            return;
        }
        panic!(
            "`{}` was settled differently from the rules of poker.\n\
             Per-SEAT DIFF   (engine minus what the rules owe): {:?}\n\
             Per-PRINCIPAL DIFF (WHO was paid, by principal):   {:?}\n\
             Every chip may still be conserved -- {} destroyed -- and the totals may \
             still be right; what is wrong is WHO HAS THE MONEY. A run where the SEAT \
             column is all zeroes and the PRINCIPAL column is not is \
             docs/SECURITY-FINDINGS.md FINDING 13 exactly: the right amount, the right \
             chair, the wrong person.\n\n{}\n\nIf this is a deliberate, documented \
             defect, add `{}` to Bench::allow_disagreement AND an entry to \
             docs/DEFECTS.md in the same change. Do not delete the scenario.",
            cmp.label,
            cmp.seats
                .iter()
                .map(|s| (s.seat, s.diff))
                .collect::<Vec<_>>(),
            cmp.misattributed()
                .iter()
                .map(|p| (p.principal.to_text(), p.diff))
                .collect::<Vec<_>>(),
            cmp.destroyed,
            cmp.report(),
            cmp.label,
        );
    }

    /// Run one hand under a single betting policy, with no card target.
    pub fn hand_with_policy(
        &mut self,
        label: &str,
        policy: Policy,
    ) -> Option<HandComparison> {
        let mut policy = Some(policy);
        self.hand(label, None, 1, &mut |w, r, _| {
            if let Some(p) = policy.take() {
                drive::play_out(w, r, p, 300);
            }
        })
    }
}

// ---------------------------------------------------------------------------
// deal targets
// ---------------------------------------------------------------------------

/// Exactly `size` of the seats in `among` tie for the best hand at the river.
pub fn target_tie_among(among: Vec<u8>, size: usize) -> Box<dyn Fn(&DealPlan) -> bool> {
    Box::new(move |plan: &DealPlan| plan.tie_size_among(&among) == size)
}

/// Some `size`-subset of the dealt seats ties for the best hand among themselves.
///
/// Much cheaper to hit than "these three exact seats tie", and just as good: the
/// scenario then folds every seat outside the tying set, so the tie IS the
/// showdown.
pub fn target_any_tying_group(size: usize) -> Box<dyn Fn(&DealPlan) -> Option<Vec<u8>>> {
    Box::new(move |plan: &DealPlan| {
        let ranks = plan.ranks_at_river();
        let mut groups: std::collections::BTreeMap<String, Vec<u8>> =
            std::collections::BTreeMap::new();
        for (seat, rank) in &ranks {
            groups
                .entry(format!("{rank:?}"))
                .or_default()
                .push(*seat);
        }
        groups
            .into_values()
            .find(|g| g.len() == size)
            .map(|mut g| {
                g.sort_unstable();
                g
            })
    })
}

/// All the seats in `among` hold DIFFERENT hands: no chop, so every layer has a
/// single winner and any misassignment is unambiguous.
pub fn target_all_distinct(among: Vec<u8>) -> Box<dyn Fn(&DealPlan) -> bool> {
    Box::new(move |plan: &DealPlan| {
        let ranks = plan.ranks_at_river();
        let mut seen: Vec<String> = among
            .iter()
            .filter_map(|s| ranks.get(s).map(|r| format!("{r:?}")))
            .collect();
        let total = seen.len();
        seen.sort();
        seen.dedup();
        total == among.len() && seen.len() == among.len()
    })
}

/// The seat with the SHORTEST stack in `among` holds the single best hand, and the
/// deepest holds the worst. That makes "the short stack wins the main pot and only
/// the main pot" a checkable claim, and it is the shape most sensitive to a wrong
/// eligibility set.
pub fn target_short_stack_best(short: u8, others: Vec<u8>) -> Box<dyn Fn(&DealPlan) -> bool> {
    Box::new(move |plan: &DealPlan| {
        let mut among = others.clone();
        among.push(short);
        plan.winners_among(&among) == vec![short]
    })
}
