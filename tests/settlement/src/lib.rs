//! ClearDeck settlement oracle.
//!
//! ```text
//!   src/oracle.rs      THE ORACLE: what the rules of poker say each seat is owed
//!   src/observe.rs     watch one hand on the real canister, compare against it
//!   src/deal.rs        read the deal, predict the board the deck will produce
//!   src/drive.rs       exact stacks, stated betting policies
//!   src/scenarios.rs   reach the hard settlements on purpose (rewind-and-re-deal)
//!   src/suite.rs       the named scenarios themselves, shared by every caller
//!   src/coverage.rs    what was ACTUALLY reached, derived from the records
//!   src/wasms.rs       build + freshness-check + identify the module under test
//!   src/world.rs       PocketIC, real ICP ledger, real table canister wasm
//!
//!   tests/oracle_rules.rs   the oracle's own rules, hand-written, no replica
//!   tests/settlement.rs     the comparison harness against the real canister
//!   tests/disagreements.rs  a permanent minimal reproducer per disagreement found
//! ```
//!
//! # The one thing to understand before reading anything else
//!
//! This crate is a MEASURING INSTRUMENT, not a fix. Several of the disagreements
//! it reports are already documented defects (docs/DEFECTS.md E-01, E-03, E-05)
//! and they are deliberately left in place. The value of the instrument is that a
//! later change to the payout code can be judged by whether the oracle goes quiet.
//!
//! # Why it can be trusted against the engine
//!
//! * The oracle derives payouts from the rules of poker. It does not call
//!   `build_side_pots`, `apply_side_pots`, or anything lifted from
//!   `determine_winners`. The only engine code it uses is the hand evaluator,
//!   which wave 1 proved exhaustively.
//! * It self-checks conservation: every chip collected must be owed to somebody,
//!   and `settle` panics if its own arithmetic loses one.
//! * The wasm is built by the harness, freshness-checked against every source file
//!   that feeds it, printed by sha256, and the replica's reported module hash is
//!   asserted to equal that sha256 after install.
//! * It is proven able to convict a wrong-seat payout: `tests/settlement.rs`
//!   documents planted-bug runs in which the engine still conserved every chip and
//!   the oracle still fired. See `README.md`.

pub mod cards;
pub mod coverage;
pub mod deal;
pub mod drive;
pub mod ledger;
pub mod observe;
pub mod oracle;
pub mod scenarios;
pub mod suite;
pub mod table_api;
pub mod wasms;
pub mod world;

pub use coverage::{Coverage, Features};
pub use observe::{HandComparison, HandRecord, HandRecorder, SeatCompare};
pub use oracle::{settle, Anomaly, Award, AwardReason, HandFacts, SeatStake, Settlement};
pub use scenarios::Bench;
pub use world::{OpError, World};

/// Assert every hand in a batch settled the way the rules of poker say it should,
/// listing the full reproducer for each hand that did not.
///
/// This is the assertion a FIXED engine has to satisfy, and since wave 2 it passes:
/// E-01, E-03, E-05 and the odd-chip rule are fixed and the oracle is quiet on all
/// 17 deliberate hands. Note that `Bench::gate` now applies the same rule PER HAND,
/// where the hand ran, so a disagreement fails at its own scenario as well as here.
pub fn assert_all_agree(batch: &[HandComparison], context: &str) {
    let bad: Vec<&HandComparison> = batch.iter().filter(|c| !c.agrees).collect();
    assert!(
        bad.is_empty(),
        "{context}: {} of {} hands were settled differently from the rules of poker.\n\n{}",
        bad.len(),
        batch.len(),
        bad.iter().map(|c| c.report()).collect::<Vec<_>>().join("\n")
    );
}

/// Assert a specific hand DISAGREES with the rules of poker.
///
/// Kept, unused, deliberately. It is the shape a marker takes while a payout defect
/// is live, and if one is ever found again the marker that pins it should be written
/// with this rather than invented afresh. Every marker in `tests/disagreements.rs`
/// used it until wave 2 fixed E-01, E-03, E-05 and the odd-chip rule; they now
/// assert the opposite, per seat.
#[allow(dead_code)]
pub fn assert_disagrees(c: &HandComparison, context: &str) {
    assert!(
        !c.agrees,
        "{context}: this hand is pinned as a KNOWN payout defect but the engine and the \
         oracle now AGREE on it. If the defect has been fixed, update this test and its \
         entry in docs/DEFECTS.md together.\n\n{}",
        c.report()
    );
}
