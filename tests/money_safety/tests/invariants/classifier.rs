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
    assert!(
        documented::TOLERATED_SELF_REPORTS.is_empty(),
        "no log line may be tolerated while no defect is documented as producing one"
    );
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
