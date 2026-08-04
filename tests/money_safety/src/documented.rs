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
//! * FIXING the defect means DELETING the entry. `register_entries_are_all_still_
//!   needed` in the invariants test binary fails once an entry stops being hit, so a
//!   fix cannot quietly leave stale tolerance behind.
//! # The register is EMPTY, and that is the goal state
//!
//! It used to carry six entries: three for E-01 (post-flop money destroyed at every
//! showdown, seen as stranded on the ledger, as value leaving the table, and as
//! `awarded < collected`), one for E-03's `side_pots` drift, one for E-03's
//! self-reported `BUG: Side pots (...)` line, and one for E-05's orphaned stake.
//! All four defects were fixed in wave 2, so all six entries were DELETED, which is
//! what fixing a defect is supposed to do to its tolerance.
//!
//! An empty register means: **nothing on the money path is excused.** Every
//! violation any invariant reports now fails the run. If a defect has to ship, add
//! an entry here with an id, a document, a direction and a bound -- and know that
//! the fuzzer can then explore past it.
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
/// The whitelist is therefore empty and EVERY `BUG:` or `CRITICAL:` line the
/// canister logs is a `SelfReportedFailure`, which nothing can excuse.
///
/// That includes the one the payout path can still write:
/// `CRITICAL: pot accounting disagreement in hand N`, which fires only if
/// `state.pot` and the contributions ever disagree. Honest play cannot produce it.
pub const TOLERATED_SELF_REPORTS: &[&str] = &[
    // docs/DEFECTS.md E-36. A player who takes an empty chair MID-HAND and calls
    // `sit_in()` is given the action and can bet into a hand they hold no cards in.
    // `hand_contributions` reports it and keeps BOTH stakes, which is the correct
    // thing for the PAYOUT path to do -- dropping either is a destroyed chip -- so
    // this line is not an accounting inconsistency and must not stop the run.
    //
    // It is listed here, rather than left as a `WARNING:` the detector did not
    // match, because an untolerated-but-invisible defect is the worst of both:
    // a 1,200-step fuzz run emitted 296 of these and reported
    // `0 documented finding(s)`.
    //
    // DELETE THIS ENTRY when E-36 is fixed (a seat taken mid-hand must not be
    // dealt the action). The line will stop being emitted, and leaving a stale
    // tolerance behind is exactly what H-19 is about.
    "carries both a live stake and a departed stake",
];

pub const REGISTER: &[DocumentedDefect] = &[
    // EMPTY ON PURPOSE. See the module docs: the six entries that used to live here
    // named E-01, E-03 and E-05, and all three are fixed. Their markers are now
    // gates:
    //
    //   E-01  tests/regressions.rs reg01 (the winner is paid every e8 collected),
    //         tests/invariants/seam.rs seam_a, and the settlement oracle's
    //         pinned_e01_post_flop_money_is_no_longer_destroyed.
    //   E-03  src/table_canister/src/lib.rs payout_tests::e03_* (state.pot can
    //         neither mint nor destroy a chip) and the archive-vs-payout builder
    //         test in poker_core::side_pots.
    //   E-05  tests/regressions.rs reg05 and reg08 (a vacated seat's stake stays in
    //         the payout basis), and pinned_e05_* in the settlement oracle.
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
