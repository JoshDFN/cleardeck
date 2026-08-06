//! Where the canister-level tests are, and a gate that keeps them there.
//!
//! WHAT USED TO BE HERE: comments only. Twenty-two lines saying that "true
//! integration tests for Internet Computer canisters require either the IC replica
//! or PocketIC framework" and suggesting some `dfx canister call` commands. No
//! assertion, no test, nothing that could fail. It read as coverage and was not.
//!
//! That mattered: docs/DEFECTS.md H-04 records that SEVEN of seven mutations to the
//! seam in `src/table_canister/src/lib.rs` survived with the whole suite green. The
//! canister could stop using the extracted engine entirely, or deal every hand from
//! the constant seed `b"CONSTANT"`, and nothing went red, because no test ever ran a
//! hand through the canister and looked at the numbers.
//!
//! # Why the PocketIC tests are not in THIS crate
//!
//! `pocket-ic` drags in tokio, reqwest and rustls. Putting that tree in the lockfile
//! of a crate that custodies real ICP and ckBTC on mainnet is not a trade this
//! project makes, and it would slow every build of the fund-holding crates. The
//! canister-level suite therefore lives in `tests/money_safety`, a DELIBERATELY
//! DETACHED cargo workspace with its own lockfile and its own target dir.
//!
//! The consequence a reader has to know: `cargo test --workspace` from the repo root
//! does NOT run it. Only these do:
//!
//! ```text
//!   ./scripts/dev.sh test        # the fast gate: includes the seam tests
//!   cd tests/money_safety && cargo test --test invariants
//! ```
//!
//! # What this file asserts
//!
//! One thing, structurally: that the canister-level suite still exists and still
//! contains the tests that kill the H-04 mutations. It fails if that file is deleted
//! or if a named test is renamed or removed -- which is exactly how a suite quietly
//! reverts to reporting green about nothing. It is not a substitute for running them.
//!
//! The seam logic that CAN be checked without a replica -- the re-export identity of
//! the Candid-facing poker types, `HandRank`'s pot-deciding `Ord`, and
//! `collect_contributions` -- is covered in `tests/unit_tests.rs` and is not
//! duplicated here.

use std::path::{Path, PathBuf};

/// `<repo>/tests/money_safety`, derived from this crate's manifest dir.
fn money_safety_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate lives at <repo>/src/table_canister")
        .join("tests")
        .join("money_safety")
}

/// The canister-level tests that close docs/DEFECTS.md H-04, and the mutation each
/// one kills. Keep this list and `docs/DEFECTS.md` H-04 in step.
const REQUIRED_SEAM_TESTS: &[(&str, &str)] = &[
    (
        "seam_a_showdown_with_post_flop_betting_accounts_for_every_e8",
        "H-04 mutations 1-4: calculate_side_pots no-op / state.pot / 2 / pots into a \
         throwaway Vec / &[] contributions",
    ),
    (
        "seam_a_control_a_passive_hand_pays_out_everything_it_collected",
        "the control case: awarded == collected when no post-flop money exists",
    ),
    (
        "seam_b_side_pots_sum_to_pot_at_the_moment_calculate_side_pots_runs",
        "H-04 mutations 1-4, at the point the breakdown is written",
    ),
    (
        "seam_c_the_deck_is_different_every_hand",
        "H-04 mutation 7: shuffle_deck shadowed so every hand comes from b\"CONSTANT\"",
    ),
    (
        "seam_d_the_recorded_showdown_ranks_are_the_ranks_evaluate_hand_returns",
        "H-04 mutation 6: evaluate_hand stubbed to return RoyalFlush for everybody",
    ),
];

#[test]
fn the_canister_level_seam_suite_still_exists() {
    let seam = money_safety_dir()
        .join("tests")
        .join("invariants")
        .join("seam.rs");
    let body = std::fs::read_to_string(&seam).unwrap_or_else(|e| {
        panic!(
            "the canister-level seam suite is GONE: {} could not be read ({e}).\n\nWithout it, \
             nothing in this repository drives the table canister's state machine and checks the \
             money, and every mutation in docs/DEFECTS.md H-04 survives again. Restore it, or move \
             it and update this test.",
            seam.display()
        )
    });

    for (name, kills) in REQUIRED_SEAM_TESTS {
        assert!(
            body.contains(name),
            "{} no longer contains `{name}`, the test that kills {kills}. If it was renamed, \
             rename it here too; if it was deleted, the mutation it caught is live again.",
            seam.display()
        );
    }
}

/// The seam suite must be part of the target `scripts/dev.sh test` actually invokes.
///
/// `cargo test --test invariants` is the exact command in `cmd_test`. A new test file
/// dropped into `tests/money_safety/tests/` becomes its own target and is NOT run by
/// it, so a suite can be written, committed, and never executed. `seam.rs` is a
/// module of the `invariants` target precisely to avoid that, and this asserts it.
#[test]
fn the_seam_suite_is_wired_into_the_target_the_fast_gate_runs() {
    let main = money_safety_dir()
        .join("tests")
        .join("invariants")
        .join("main.rs");
    let body = std::fs::read_to_string(&main).unwrap_or_else(|e| {
        panic!(
            "{} could not be read ({e}). `cargo test --test invariants` -- the command \
             scripts/dev.sh runs -- resolves to this file.",
            main.display()
        )
    });
    for module in ["mod seam;", "mod classifier;"] {
        assert!(
            body.contains(module),
            "{} does not declare `{module}`, so those tests are not part of the `invariants` \
             target and `./scripts/dev.sh test` does not run them.",
            main.display()
        );
    }
}
