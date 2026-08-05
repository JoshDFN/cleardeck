//! Tests OF the gate, not of the canister.
//!
//! docs/DEFECTS.md H-02 and H-03 are both the same class of failure: an assertion no
//! input could make red. The fix is only real if the fix itself is tested, so these
//! feed synthetic violations through the classifier and the log matcher and assert
//! which ones stop a run. Every case below is a shape wave 1 proved could pass.
//!
//! No PocketIC instance is needed, so these are microseconds.

use money_safety::documented::{
    self, BlockingReason, Direction, Disposition, DocumentedDefect, REGISTER, WORLD_TOTAL_E8S,
};
use money_safety::invariants::*;
use money_safety::table_api::GamePhase;

fn violation(
    invariant: Invariant,
    check: &'static str,
    severity: Severity,
    delta: i128,
) -> Violation {
    Violation {
        invariant,
        check,
        severity,
        delta_e8s: delta,
        detail: "synthetic".to_string(),
        phase: "HandComplete".to_string(),
    }
}

fn reason(v: &Violation) -> BlockingReason {
    match documented::classify(v) {
        Disposition::Blocking(r) => r,
        Disposition::Documented(d) => panic!("expected blocking, got documented as {}", d.id),
    }
}

// ---------------------------------------------------------------------------
// H-03 -- the sign of the delta must not decide anything
// ---------------------------------------------------------------------------

/// The exact shape the critic used to smuggle a 1% house rake past the fuzzer: a
/// negative-delta `FundDestruction` in a check nobody registered. It used to be
/// classified "documented" purely because `FundDestruction` was on a two-item
/// allow-list of SEVERITIES, and the run reported `ok`.
#[test]
fn a_fund_destruction_in_an_unregistered_check_blocks() {
    let v = violation(
        Invariant::M3NoRake,
        "some_new_check_nobody_registered",
        Severity::FundDestruction,
        -40_000,
    );
    assert_eq!(reason(&v), BlockingReason::NotRegistered);
}

#[test]
fn a_breakdown_drift_in_an_unregistered_check_blocks() {
    let v = violation(
        Invariant::M1bPotBreakdown,
        "a_check_that_does_not_exist_yet",
        Severity::BreakdownDrift,
        1_000,
    );
    assert_eq!(reason(&v), BlockingReason::NotRegistered);
}

/// A SYNTHETIC register, for the tests of the classifier mechanism itself.
///
/// These tests used to point at E-01's real entry. E-01 was fixed in wave 2 and its
/// entries were deleted -- which is what fixing a defect must do to its tolerance --
/// so the mechanism is now exercised against a register written here. That is
/// strictly better: whether the direction, bound and severity rules work is not a
/// question about which defects happen to be shipping this month.
const SYNTHETIC: &[DocumentedDefect] = &[DocumentedDefect {
    id: "E-00",
    doc: "this test file",
    invariant: Invariant::M1Conservation,
    check: "ledger_equals_owed",
    directions: &[Direction::HoldsMore],
    max_abs_delta_e8s: Some(WORLD_TOTAL_E8S),
    why: "a synthetic entry, so the classifier's direction/bound/severity rules can be \
          tested without a real defect having to be live",
}];

fn synthetic_reason(v: &Violation) -> BlockingReason {
    match documented::classify_against(v, SYNTHETIC) {
        Disposition::Blocking(r) => r,
        Disposition::Documented(d) => panic!("expected blocking, got documented as {}", d.id),
    }
}

/// A registered defect excuses only the direction its mechanism can produce. The
/// synthetic entry strands money INSIDE the canister (`delta > 0` on M1); the
/// canister owing more than it holds is a different defect entirely and must never
/// be excused by it -- even though the old classifier keyed on nothing but severity.
#[test]
fn a_registered_check_in_the_wrong_direction_blocks() {
    let wrong_way = violation(
        Invariant::M1Conservation,
        "ledger_equals_owed",
        Severity::FundDestruction,
        -5_000,
    );
    assert_eq!(synthetic_reason(&wrong_way), BlockingReason::WrongDirection);

    let right_way = violation(
        Invariant::M1Conservation,
        "ledger_equals_owed",
        Severity::FundDestruction,
        5_000,
    );
    assert!(
        documented::classify_against(&right_way, SYNTHETIC).is_documented(),
        "the entry's own direction must be tolerated, or a documented defect could not \
         be explored past at all"
    );
    // And against the REAL register -- which is empty, because nothing on the money
    // path is tolerated any more -- both directions block.
    assert_eq!(reason(&right_way), BlockingReason::NotRegistered);
}

/// A discrepancy bigger than any documented mechanism can produce is not that
/// mechanism.
#[test]
fn a_registered_defect_cannot_excuse_an_impossible_magnitude() {
    let absurd = violation(
        Invariant::M1Conservation,
        "ledger_equals_owed",
        Severity::FundDestruction,
        WORLD_TOTAL_E8S + 1,
    );
    assert_eq!(synthetic_reason(&absurd), BlockingReason::OverBound);
}

/// The severities that are never excusable, whatever the register says. The test
/// deliberately uses a tuple the SYNTHETIC register does name, so the only thing
/// that can block it is the severity rule.
#[test]
fn never_excusable_severities_block_even_on_a_registered_check() {
    for severity in [
        Severity::FundCreation,
        Severity::RakeTaken,
        Severity::DoublePay,
        Severity::DurabilityLoss,
        Severity::SelfReportedFailure,
        Severity::Misattribution,
    ] {
        let v = violation(
            Invariant::M1Conservation,
            "ledger_equals_owed",
            severity,
            5_000,
        );
        assert_eq!(
            synthetic_reason(&v),
            BlockingReason::NeverExcusable,
            "{severity:?} must always block"
        );
    }
}

/// A rake shows up as awarded < the engine's own payout basis, and that is
/// `RakeTaken`, which nothing can excuse. This is the assertion H-03 says the
/// harness was missing.
#[test]
fn a_shortfall_against_the_engines_own_payout_basis_blocks() {
    let vs = check_awarded_equals_payout_basis(7, 1_000_000, 990_000, &GamePhase::HandComplete);
    assert_eq!(vs.len(), 1, "a 1% shortfall must produce exactly one finding");
    assert_eq!(vs[0].severity, Severity::RakeTaken);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
    assert!(
        check_awarded_equals_payout_basis(7, 1_000_000, 1_000_000, &GamePhase::HandComplete)
            .is_empty(),
        "paying out the whole basis must produce nothing"
    );
}

// ---------------------------------------------------------------------------
// H-02 -- the canister reporting its own inconsistency must be able to fail a run
// ---------------------------------------------------------------------------

/// `pre_upgrade`'s `CRITICAL: Failed to save state to stable memory` is a
/// total-fund-loss-on-upgrade event. It used to be folded into the same
/// non-blocking bucket as the known side-pot warning, and a run in which the line
/// appeared reported `test result: ok`.
#[test]
fn a_critical_log_line_blocks() {
    let logs = vec![
        "CRITICAL: Failed to save state to stable memory: Err(\"out of memory\")".to_string(),
    ];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::HandComplete);
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].severity, Severity::SelfReportedFailure);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
}

/// A `BUG:` line the register does not enumerate is a new defect, not a known one.
#[test]
fn an_unenumerated_bug_log_line_blocks() {
    let logs = vec!["BUG: pot went negative, clamping to zero".to_string()];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::HandComplete);
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].severity, Severity::SelfReportedFailure);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
}

/// The line that USED to be tolerated now blocks like any other.
///
/// `BUG: Side pots (...) exceed total pot (...). Capping to pot amount.` was the
/// engine writing down that its own accounting was inconsistent and settling anyway,
/// out of the capped figure. It was tolerated so the fuzzer could explore past E-03.
/// E-03 is fixed, the routine that wrote it is off the payout path, and the
/// tolerated list is empty -- so if this line ever appears again it stops the run.
#[test]
fn the_formerly_enumerated_bug_line_now_blocks_too() {
    let logs = vec![
        "BUG: Side pots (300) exceed total pot (200). Capping to pot amount.".to_string(),
    ];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::HandComplete);
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].severity, Severity::SelfReportedFailure);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
    // The tolerated list must contain exactly the lines a DOCUMENTED open defect
    // produces, and nothing else. It was empty until the coherence pass, which is
    // the right default; the one entry names E-36. When E-36 is fixed the entry
    // goes with it and this drops back to zero.
    assert_eq!(
        documented::TOLERATED_SELF_REPORTS,
        ["carries both a live stake and a departed stake"],
        "the tolerated-self-report list changed. Every entry must name an OPEN defect in \
         docs/DEFECTS.md and must be deleted when that defect is fixed; adding one is \
         admitting a money-path defect is shipping."
    );
}

/// E-36's dual-stake line is TOLERATED, not invisible.
///
/// The payout fix wrote it as `WARNING:` rather than `CRITICAL:`, which is a
/// defensible severity -- nothing about the engine's accounting is inconsistent
/// there -- but `check_self_reported_inconsistency` matched neither, so 296
/// occurrences of a real open defect executing against the real canister were
/// reported as `0 documented finding(s)` in a 1,200-step fuzz run. Tolerance living
/// in a string the classifier does not read is exactly the H-03 pathology this
/// module exists to prevent. docs/DEFECTS.md H-20.
#[test]
fn a_warning_line_is_a_finding_and_e36s_is_a_tolerated_one() {
    let logs = vec![
        "WARNING: seat 2 carries both a live stake and a departed stake in hand 6 \
         (the chair was re-occupied mid-hand, docs/DEFECTS.md E-36)."
            .to_string(),
    ];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::HandComplete);
    assert_eq!(vs.len(), 1, "a WARNING: line must not be invisible");
    assert_eq!(
        vs[0].severity,
        Severity::BreakdownDrift,
        "E-36 is named in the register, so it is counted rather than blocking"
    );
}

/// ...and a WARNING the register does NOT name blocks, so the next person who
/// downgrades a self-report to `WARNING:` to keep a run green cannot.
#[test]
fn an_unregistered_warning_line_blocks() {
    let logs = vec!["WARNING: something nobody documented just happened".to_string()];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::River);
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].severity, Severity::SelfReportedFailure);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
}

/// The payout path's own self-report -- the one line it can still write -- blocks.
/// It fires only if `state.pot` and the players' contributions ever disagree, which
/// honest play cannot produce since E-05 was fixed.
#[test]
fn the_payout_paths_pot_disagreement_line_blocks() {
    let logs = vec![
        "CRITICAL: pot accounting disagreement in hand 7: state.pot = 480 but the \
         contributions (including 1 departed stake(s)) sum to 420."
            .to_string(),
    ];
    let vs = check_self_reported_inconsistency(&logs, &GamePhase::River);
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].severity, Severity::SelfReportedFailure);
    assert_eq!(reason(&vs[0]), BlockingReason::NeverExcusable);
}

/// A clean log produces nothing at all: the matcher must not be so eager that the
/// blocking rule above is unusable.
#[test]
fn ordinary_log_lines_produce_nothing() {
    let logs = vec![
        "Player 0 joined seat 0".to_string(),
        "Debugging note: side pots rebuilt".to_string(),
        "hand 4 complete".to_string(),
    ];
    assert!(
        check_self_reported_inconsistency(&logs, &GamePhase::HandComplete).is_empty(),
        "no ordinary line may be read as a self-reported failure"
    );
}

// ---------------------------------------------------------------------------
// the register itself
// ---------------------------------------------------------------------------

/// Every entry must name a defect id and say why, and no entry may be a blanket
/// pass: an entry with no direction, or an unbounded magnitude on a money check, is
/// the sign-based tolerance coming back through the register.
#[test]
fn every_register_entry_is_specific() {
    // An EMPTY register is the goal state, not a failure: it means no defect on the
    // money path is tolerated. It became empty in wave 2 when E-01, E-03 and E-05
    // were fixed and their six entries were deleted. What must not happen is an
    // entry that is vague.
    for d in REGISTER {
        assert!(d.id.starts_with('E'), "entry {:?} has no defect id", d.check);
        assert!(!d.doc.is_empty(), "{} names no document", d.id);
        assert!(!d.why.is_empty(), "{} gives no reason", d.id);
        assert!(
            !d.directions.is_empty(),
            "{}/{} excuses every direction, which is exactly the H-03 defect",
            d.id,
            d.check
        );
        assert!(
            d.max_abs_delta_e8s.is_some(),
            "{}/{} excuses an unbounded magnitude",
            d.id,
            d.check
        );
    }
}

/// The register must not name a check that does not exist. A typo'd `check` string
/// silently turns into a permanently-unreachable entry, and worse, the real check it
/// was meant to excuse starts blocking -- or, if the typo goes the other way, the
/// entry excuses nothing while looking like it does.
#[test]
fn every_register_entry_names_a_check_the_invariants_actually_emit() {
    // The complete set of `check` names the invariant functions in
    // src/invariants.rs can produce. Kept here on purpose: if someone renames a
    // check, this list and the register have to be revisited together.
    const EMITTED: &[&str] = &[
        "canister_is_short",
        "ledger_equals_owed",
        "side_pots_sum_to_pot",
        "pot_is_fully_attributed",
        "seated_chips_are_addable",
        "chips_total_is_exact",
        "internal_total_is_representable",
        "escrow_within_holdings",
        "chips_within_holdings",
        "value_conserved_across_hand",
        "awarded_equals_collected",
        "awarded_equals_payout_basis",
        "escrow_survives_upgrade",
        "table_state_survives_upgrade",
        "chips_survive_upgrade",
        "withdraw_debits_escrow_once",
        "withdraw_delivers_once",
        "withdraw_moves_ledger_once",
        "canister_reports_its_own_inconsistency",
        "canister_reports_an_unregistered_failure",
        "no_second_credit",
    ];
    for d in REGISTER {
        assert!(
            EMITTED.contains(&d.check),
            "register entry {}/{} names a check no invariant emits. Either it is a typo, or a \
             check was renamed and the register was not updated -- in which case the real check \
             is now BLOCKING and the entry is dead weight.",
            d.id,
            d.check
        );
    }
}

// ---------------------------------------------------------------------------
// Register hygiene (docs/DEFECTS.md H-19)
// ---------------------------------------------------------------------------
//
// `documented.rs` has documented `register_entries_are_all_still_needed` since it
// was written, as the mechanism that stops a stale tolerance surviving a fix:
// "FIXING the defect means DELETING the entry ... so a fix cannot quietly leave
// stale tolerance behind." `grep` found no such test. The register is empty, so
// nothing was actually being excused -- and that is exactly when to write it,
// before the first entry goes back in.

/// Every REGISTER entry must name a defect that is still open in docs/DEFECTS.md,
/// and every TOLERATED_SELF_REPORTS entry likewise.
///
/// This is a documentation-coupling test on purpose. It cannot tell whether a
/// defect is really still present -- only running the engine can do that -- but it
/// CAN tell that somebody deleted the defect entry and left the excuse behind,
/// which is the failure mode that matters.
#[test]
fn register_entries_are_all_still_needed() {
    let defects = std::fs::read_to_string(
        money_safety::wasms::repo_root().join("docs/DEFECTS.md"),
    )
    .expect("docs/DEFECTS.md must be readable: the register is only meaningful with it");

    for entry in documented::REGISTER {
        assert!(
            defects.contains(entry.id),
            "register entry {} excuses a money-path finding, but {} does not appear in \
             docs/DEFECTS.md. Either the defect was fixed and this entry should have been \
             DELETED with it, or the id is wrong.",
            entry.id,
            entry.id
        );
        assert!(
            !entry.why.trim().is_empty(),
            "register entry {} has no `why`. An excuse with no reason is how tolerance \
             becomes permanent.",
            entry.id
        );
    }

    // The tolerated self-reports are keyed on log substrings rather than ids, so
    // check the other direction: the substring must still be produced by the
    // engine source. If the line is gone, the tolerance is stale.
    let engine = std::fs::read_to_string(
        money_safety::wasms::repo_root().join("src/table_canister/src/lib.rs"),
    )
    .expect("the engine source must be readable");
    for line in documented::TOLERATED_SELF_REPORTS {
        assert!(
            engine.contains(line),
            "TOLERATED_SELF_REPORTS still excuses {line:?}, but src/table_canister/src/lib.rs \
             no longer contains that text. The defect was fixed and the tolerance was left \
             behind -- delete it."
        );
    }
}
