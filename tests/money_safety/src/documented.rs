//! The register of ALREADY-DOCUMENTED defects, by name.
//!
//! Why this file exists (docs/DEFECTS.md H-03). The harness used to decide
//! "already known" versus "new" from the SIGN of the money delta:
//!
//! ```ignore
//! matches!(self, Severity::FundDestruction | Severity::BreakdownDrift)
//! ```
//!
//! Direction is not a cause. A critic planted a 1% house rake in
//! `determine_winners` -- an operator skimming every pot -- and the fuzz run
//! returned `test result: ok`, recording the theft only as
//! `documented M1_CONSERVATION:...FundDestruction`. Dropping a side pot passed the
//! same way. "Zero blocking findings" was therefore fully compatible with a live
//! rake, i.e. the assertion could not fail for a whole DIRECTION of defect.
//!
//! So the tolerance is now an explicit allow-list. A violation is non-blocking
//! only if it matches a named entry below on all of: invariant, check, direction,
//! and magnitude bound. Everything else fails the run, including every
//! `FundDestruction` in a check nobody registered.
//!
//! # How to use this file
//!
//! * Adding an entry is admitting a defect is shipping. It requires an `id` that
//!   exists in `docs/SECURITY-FINDINGS.md` / `docs/DEFECTS.md` and a `why`.
//! * FIXING the defect means DELETING the entry.
//!   `classifier::register_entries_are_all_still_needed` in the invariants test
//!   binary fails when an entry's id is no longer in `docs/DEFECTS.md`, or when a
//!   tolerated log line is no longer in the engine source, so a fix cannot quietly
//!   leave stale tolerance behind. (That test was documented here from the start
//!   and did not exist until the wave-2 coherence pass wrote it: docs/DEFECTS.md
//!   H-19. It is a documentation-coupling check, not proof the defect is still
//!   live -- only running the engine can show that.)
//! # THE REGISTER IS EMPTY, AND SO IS THE TOLERATED-LOG-LINE LIST
//!
//! It used to carry six entries: three for E-01 (post-flop money destroyed at every
//! showdown, seen as stranded on the ledger, as value leaving the table, and as
//! `awarded < collected`), one for E-03's `side_pots` drift, one for E-03's
//! self-reported `BUG: Side pots (...)` line, and one for E-05's orphaned stake.
//! All four defects were fixed in wave 2 and all six entries were DELETED, which is
//! what fixing a defect is supposed to do to its tolerance.
//!
//! A seventh went in for E-36 in wave 6 (docs/DEFECTS.md H-28), on the check
//! `canister_reports_its_own_inconsistency`, direction `Unsigned`, bound 0 e8s. It
//! is gone too: E-36 is fixed (docs/SECURITY-FINDINGS.md FINDING 17), so the
//! `WARNING:` it excused can no longer be emitted, and the entry left with it
//! together with the [`TOLERATED_SELF_REPORTS`] line it was paired with. The engine
//! still contains that log line, deliberately, as an untolerated tripwire.
//!
//! **Nothing is excused.** If a money defect has to ship, add an entry here with an
//! id, a document, a direction and a bound -- and know that the fuzzer can then
//! explore past it.
//!
//! The classifier's own tests do not depend on the register having entries: they
//! drive [`classify_against`] with a synthetic register, so the mechanism stays
//! covered whether or not a real defect happens to be shipping. That is deliberate.
//! The bounds on the old E-01 entries were the whole world's money, because they
//! could not separate a small rake from E-01's own destruction -- E-01 *was* a rake
//! in every observable respect -- and that hole closed when E-01 did.

use crate::invariants::{Invariant, Severity, Violation};

/// Which way the money moved, from the canister's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `delta > 0`: the canister holds MORE than it owes. Money stranded.
    HoldsMore,
    /// `delta < 0`: the canister owes MORE than it holds. Chips from nothing.
    HoldsLess,
    /// The check does not carry a meaningful sign (e.g. a log line).
    Unsigned,
}

impl Direction {
    fn of(delta: i128) -> Self {
        match delta {
            d if d > 0 => Direction::HoldsMore,
            d if d < 0 => Direction::HoldsLess,
            _ => Direction::Unsigned,
        }
    }
}

/// One named, documented defect that this harness tolerates -- for now.
#[derive(Clone, Copy, Debug)]
pub struct DocumentedDefect {
    /// Register id, as used in `docs/DEFECTS.md`.
    pub id: &'static str,
    /// Where a reader can find the write-up.
    pub doc: &'static str,
    pub invariant: Invariant,
    /// Exact `Violation::check` this entry excuses. Not a prefix, not a pattern.
    pub check: &'static str,
    /// Directions this defect can produce. A violation in the OTHER direction is
    /// a different defect and blocks.
    pub directions: &'static [Direction],
    /// Largest `|delta_e8s|` this defect can produce. `None` means the mechanism
    /// genuinely has no bound the harness can state.
    pub max_abs_delta_e8s: Option<i128>,
    pub why: &'static str,
}

/// Every e8 that exists in the harness world: four actors x `ACTOR_START_E8S`.
/// Nothing any defect does can strand or create more than the money that was ever
/// minted into the run, so this is a real -- if generous -- ceiling.
pub const WORLD_TOTAL_E8S: i128 = 4 * crate::world::ACTOR_START_E8S as i128;

/// Log lines the canister may emit and still be believed. THERE ARE NONE.
///
/// This used to hold `"BUG: Side pots ("`, the exact line
/// `calculate_side_pots` wrote when its reconciliation against `state.pot`
/// disagreed with the players' contributions -- E-03 / FINDING 02. The engine
/// logged it and settled anyway, out of the capped figure, which is a detected
/// accounting inconsistency on a fund-custody path being written down and ignored.
///
/// That routine is no longer on the payout path (`poker_core::side_pots`'s archive
/// section) and cannot be reached by the canister, so the line cannot be emitted.
/// EVERY `BUG:` and every `CRITICAL:` line the canister logs is a
/// `SelfReportedFailure`, which nothing can excuse -- including the one the payout
/// path can still write, `CRITICAL: pot accounting disagreement in hand N`, which
/// fires only if `state.pot` and the contributions ever disagree.
///
/// `WARNING:` lines are matched too, and that is why this list was not always
/// empty. The wave-2 payout fix wrote E-36's dual-stake condition as a `WARNING:`,
/// defensibly -- nothing about the accounting was inconsistent there -- but the
/// detector matched neither `BUG:` nor `CRITICAL:` in it, so 296 occurrences of a
/// real open defect executing against the real canister were reported as
/// `0 documented finding(s)`. Tolerance had moved into a string the classifier did
/// not read, and the answer was to name it here where
/// `register_entries_are_all_still_needed` could police it. E-36 is now fixed and
/// the name is gone with it. The engine's line stays, untolerated: emitting it
/// again would block a run rather than be counted.
pub const TOLERATED_SELF_REPORTS: &[&str] = &[
    // EMPTY, and that is the goal state. It held one entry for the whole of wave 6:
    // E-36's `"carries both a live stake and a departed stake"`, the line
    // `hand_stakes` writes when one chair carries two owners' money in a single
    // hand. E-36 is FIXED (docs/SECURITY-FINDINGS.md FINDING 17): a mid-hand
    // arrival is never dealt the action, so it can never put money into a hand,
    // so a re-occupied chair can never carry a second live stake and the line
    // cannot be emitted.
    //
    // The line itself is deliberately STILL IN `src/table_canister/src/lib.rs`. It
    // is now an untolerated tripwire: if that state ever becomes reachable again,
    // the engine says so and `check_self_reported_inconsistency` blocks the run
    // instead of counting it.
];

pub const REGISTER: &[DocumentedDefect] = &[
    // EMPTY. Nothing is excused, in any direction, at any magnitude.
    //
    // Six entries used to live here for E-01, E-03 and E-05, and a seventh for E-36
    // (docs/DEFECTS.md H-28). All are fixed and all are deleted, which is what
    // fixing a defect is supposed to do to its tolerance. Their markers are gates
    // now:
    //
    //   E-01  tests/regressions.rs reg01 (the winner is paid every e8 collected),
    //         tests/invariants/seam.rs seam_a, and the settlement oracle's
    //         pinned_e01_post_flop_money_is_no_longer_destroyed.
    //   E-03  src/table_canister/src/lib.rs payout_tests::e03_* (state.pot can
    //         neither mint nor destroy a chip) and the archive-vs-payout builder
    //         test in poker_core::side_pots.
    //   E-05  tests/regressions.rs reg05 and reg08 (a vacated seat's stake stays in
    //         the payout basis), and pinned_e05_* in the settlement oracle.
    //   E-36  tests/wave6_coherence.rs probe4 (the fold-out winner IS paid),
    //         src/table_canister/tests/coherence_regressions.rs predicate_table,
    //         and M11 OUTCOME on every fuzz step.
    //
    // Adding an entry here is admitting a money defect is shipping.
];

/// What the harness does with a violation.
#[derive(Clone, Copy, Debug)]
pub enum Disposition {
    /// Matches a named register entry: counted, reported, does not fail the run.
    Documented(&'static DocumentedDefect),
    /// Not registered: fails the run.
    Blocking(BlockingReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockingReason {
    /// No register entry names this (invariant, check) pair at all.
    NotRegistered,
    /// A register entry names the pair but not this direction.
    WrongDirection,
    /// A register entry names the pair and direction, but the discrepancy is
    /// larger than the documented mechanism can produce.
    OverBound,
    /// A severity that is never excusable, whatever the register says.
    NeverExcusable,
}

impl BlockingReason {
    pub fn explain(self) -> &'static str {
        match self {
            BlockingReason::NotRegistered => {
                "no entry in tests/money_safety/src/documented.rs names this invariant+check, so it \
                 is a NEW defect"
            }
            BlockingReason::WrongDirection => {
                "the registered defect for this check cannot move money in this direction"
            }
            BlockingReason::OverBound => {
                "larger than the registered defect's mechanism can produce"
            }
            BlockingReason::NeverExcusable => {
                "this severity is never excusable: chips from nothing, a double payout, state lost \
                 across an upgrade, a rake, or the canister reporting an unregistered failure"
            }
        }
    }
}

impl Disposition {
    pub fn is_documented(&self) -> bool {
        matches!(self, Disposition::Documented(_))
    }
}

/// Severities the register may never excuse, regardless of any entry.
fn never_excusable(s: Severity) -> bool {
    matches!(
        s,
        Severity::FundCreation
            | Severity::DoublePay
            | Severity::DurabilityLoss
            | Severity::RakeTaken
            | Severity::SelfReportedFailure
            | Severity::Misattribution
            // Money nobody can withdraw. There is no tolerance for this and no
            // defect id that excuses it: a table that cannot be emptied is not a
            // documented defect, it is a custody failure.
            // docs/SECURITY-FINDINGS.md FINDING 15.
            | Severity::FundsUnreachable
            // Money inside the canister that belongs to NOBODY. Deliberately its
            // own severity and not a `FundDestruction`, which this register is
            // allowed to excuse: FINDING 07 destroyed 40 ICP through a labelled
            // recovery button while every instrument anchored to "what does the
            // canister say it owes" reported a clean table. There is no id that
            // excuses unowned money and no magnitude at which it is acceptable.
            // docs/SECURITY-FINDINGS.md FINDING 07.
            | Severity::OrphanedCustody
            // The player's own money, invisible to the player. No id excuses
            // telling somebody they have nothing while holding their stake: the
            // recovery path might as well not exist if the only way to learn it is
            // needed is to read the interface definition.
            // docs/SECURITY-FINDINGS.md FINDING 18.
            | Severity::CustodyInvisible
            // The right totals and the wrong result: a hand that did not end when
            // the rules say it ended, or a pot paid to somebody other than the seat
            // that held the last live claim. There is no magnitude at which the
            // wrong player wins the hand, and no direction -- the winner's side of
            // it looks like a shortfall and everybody else's like a refund, and
            // they are the same defect. Three separate defects in this project have
            // had exactly this shape and all three were invisible to every gate
            // that asked about totals.
            // docs/SECURITY-FINDINGS.md FINDING 17.
            | Severity::WrongOutcome
            // A permanent record that names the wrong people. No id excuses it and
            // no magnitude applies: it carries zero e8s by construction, which is
            // why every money instrument in this harness was green while the
            // archive was inventing a small blind who never played the hand. The
            // record is what a player would be shown to settle a dispute.
            // docs/SECURITY-FINDINGS.md FINDING 30.
            | Severity::FalseRecord
    )
}

/// Classify one violation. This is the ONLY place that decides whether a finding
/// stops a run.
pub fn classify(v: &Violation) -> Disposition {
    classify_against(v, REGISTER)
}

/// [`classify`] against an explicit register.
///
/// Exists so the classifier's own tests -- the H-03 fix, which is the reason this
/// file exists at all -- can be driven with a synthetic register instead of
/// depending on which real defects happen to be shipping. When the last real entry
/// was deleted (E-01, E-03 and E-05 fixed in wave 2) those tests would otherwise
/// have had nothing to point at, and the honest options were to leave a stale
/// tolerance in place or to stop testing the mechanism. Neither is acceptable.
pub fn classify_against(
    v: &Violation,
    register: &'static [DocumentedDefect],
) -> Disposition {
    if never_excusable(v.severity) {
        return Disposition::Blocking(BlockingReason::NeverExcusable);
    }
    let named: Vec<&'static DocumentedDefect> = register
        .iter()
        .filter(|d| d.invariant == v.invariant && d.check == v.check)
        .collect();
    if named.is_empty() {
        return Disposition::Blocking(BlockingReason::NotRegistered);
    }
    let dir = Direction::of(v.delta_e8s);
    let right_direction: Vec<&'static DocumentedDefect> = named
        .iter()
        .copied()
        .filter(|d| d.directions.contains(&dir))
        .collect();
    if right_direction.is_empty() {
        return Disposition::Blocking(BlockingReason::WrongDirection);
    }
    match right_direction
        .into_iter()
        .find(|d| d.max_abs_delta_e8s.is_none_or(|m| v.delta_e8s.abs() <= m))
    {
        Some(d) => Disposition::Documented(d),
        None => Disposition::Blocking(BlockingReason::OverBound),
    }
}

/// Convenience for the fuzzer and the tests.
pub fn is_documented(v: &Violation) -> bool {
    classify(v).is_documented()
}

/// One line naming why a violation blocks, for an assertion message.
pub fn blocking_reason(v: &Violation) -> Option<&'static str> {
    match classify(v) {
        Disposition::Documented(_) => None,
        Disposition::Blocking(r) => Some(r.explain()),
    }
}
