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
//! # The register holds EXACTLY ONE entry, and it excuses ZERO e8s
//!
//! It used to carry six: three for E-01 (post-flop money destroyed at every
//! showdown, seen as stranded on the ledger, as value leaving the table, and as
//! `awarded < collected`), one for E-03's `side_pots` drift, one for E-03's
//! self-reported `BUG: Side pots (...)` line, and one for E-05's orphaned stake.
//! All four defects were fixed in wave 2, so all six entries were DELETED, which is
//! what fixing a defect is supposed to do to its tolerance. That left the register
//! empty, and empty is still the goal state.
//!
//! The one entry that is back names E-36 on the check
//! `canister_reports_its_own_inconsistency`, direction `Unsigned`, bound **0 e8s**
//! (docs/DEFECTS.md H-28). It is the other half of the
//! [`TOLERATED_SELF_REPORTS`] line below, which had no register entry to meet, so a
//! tolerated log line blocked exactly as hard as an untolerated one and the fuzzer's
//! own default invocation was red at HEAD. It cannot excuse a single e8 moving
//! anywhere: **nothing on the money path is excused.** Every violation that touches
//! a balance still fails the run. If a money defect has to ship, add an entry here
//! with an id, a document, a direction and a bound -- and know that the fuzzer can
//! then explore past it.
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

/// Log lines the canister may emit and still be believed. THERE IS EXACTLY ONE,
/// and it names an open defect.
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
/// `WARNING:` lines are matched too, and that is why this list is no longer empty.
/// The payout fix wrote E-36's dual-stake condition as a `WARNING:`, defensibly --
/// nothing about the accounting is inconsistent there -- but the detector matched
/// neither `BUG:` nor `CRITICAL:` in it, so 296 occurrences of a real open defect
/// executing against the real canister were reported as `0 documented finding(s)`.
/// Tolerance had moved into a string the classifier did not read. It is named here
/// instead, where `register_entries_are_all_still_needed` can police it.
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
    // THE ONLY ENTRY. It admits E-36 is shipping, which it is, and it is what
    // closes docs/DEFECTS.md H-28: the fuzzer's OWN DEFAULT INVOCATION was red.
    //
    // The two halves of the tolerance mechanism did not meet.
    // `check_self_reported_inconsistency` matched E-36's `WARNING:` against
    // `TOLERATED_SELF_REPORTS` and, because it matched, downgraded it from
    // `SelfReportedFailure` to `BreakdownDrift` on the check
    // `canister_reports_its_own_inconsistency` -- "where it can be counted". But
    // nothing in this register named that (invariant, check) pair, so `classify`
    // answered `Blocking(NotRegistered)` and the run failed. The downgrade bought
    // nothing: a line on the tolerated list blocked exactly as hard as one that was
    // not on it. `cargo test --test fuzz` with no environment at all failed at seed
    // 0xC1EA_2DEC_0003, shrunk to 12 ops, and no make target ever ran that
    // invocation because every caller passed MONEY_FUZZ_SEEDS.
    //
    // WHY THIS DIRECTION AND NOT THE OTHER. H-28 offered two fixes: name the
    // tolerance here, or stop emitting a violation for a tolerated line. The second
    // puts tolerance back into a place the register cannot see, which is the exact
    // H-03/H-20 pathology this file exists to prevent -- 296 occurrences of a real
    // open defect once reported as `0 documented finding(s)`. So it is named here,
    // counted, printed per run, and policed by
    // `register_entries_are_all_still_needed`.
    //
    // WHAT THIS DOES NOT EXCUSE. `directions` is `Unsigned` only and the bound is
    // ZERO e8s, so this entry can excuse exactly one thing: a zero-delta log-line
    // finding on that one check. It cannot excuse a single e8 moving anywhere. The
    // engine's handling of the dual-stake state is correct -- both stakes stay in
    // the payout basis, each under its own owner, because dropping either destroys a
    // chip -- so there is no accounting inconsistency to excuse, only a `WARNING:`
    // about a seat that E-36 should never have dealt the action to.
    //
    // DELETE THIS ENTRY when E-36 is fixed, together with its
    // `TOLERATED_SELF_REPORTS` line. `register_entries_are_all_still_needed` fails
    // on both halves if only one is removed.
    DocumentedDefect {
        id: "E-36",
        doc: "docs/DEFECTS.md#e-36 (the defect) and #h-28 (why this entry exists)",
        invariant: Invariant::M1bPotBreakdown,
        check: "canister_reports_its_own_inconsistency",
        // The violation is constructed with `delta_e8s: 0`, so `Direction::of(0)` is
        // `Unsigned`. A signed one on this check would be a DIFFERENT finding and
        // blocks.
        directions: &[Direction::Unsigned],
        max_abs_delta_e8s: Some(0),
        why: "E-36 is open and shipping: a player who takes an empty chair MID-HAND and calls \
              sit_in() is dealt the action and can bet into a hand it holds no cards in. The \
              payout path detects the resulting dual-stake seat and says so in a WARNING:, then \
              keeps BOTH stakes with their own owners -- which is the correct thing to do, since \
              dropping either destroys a chip. The line is therefore not an accounting \
              inconsistency and must not stop a fund-safety run; it is counted here instead so \
              the fuzzer can explore past a defect the project has already decided to ship.",
    },
    // The six entries that used to live here named E-01, E-03 and E-05, and all
    // three are fixed. Their markers are now gates:
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
            | Severity::Misattribution
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
