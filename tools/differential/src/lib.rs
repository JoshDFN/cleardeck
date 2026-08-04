//! Differential hand-evaluator harness for ClearDeck.
//!
//! # The rule this harness enforces
//!
//! A disagreement between `poker_core` and a mature reference is a BUG IN
//! `poker_core` until proven otherwise. Two independent references are consulted;
//! ClearDeck is only ever convicted when **both** agree against it, and a hand the
//! two references disagree about is reported as *inconclusive* rather than charged
//! to ClearDeck.
//!
//! # References
//!
//! | role | crate | lineage |
//! |------|-------|---------|
//! | A (always) | `rs_poker` 5.0.0 | its own perfect-hash tables generated in `build.rs`; native 5/6/7-card lookup |
//! | B (always) | `poker` 0.7.0 | Rust port of `treys`: Cactus Kev prime products + Senzee perfect hash |
//! | C (opt-in) | `phevaluator` (Python, C extension) | Henry Lee's tables; run over the WHOLE five-card space, and audits that A and B are independent |
//!
//! Reference C is opt-in (`CLEARDECK_PHE_PYTHON`) only because the harness must
//! also work on a machine with no Python. `treys` itself is deliberately not used:
//! reference B is already a port of it, so the two would not be independent.
//!
//! # What is checked
//!
//! 1. All `C(52,5) = 2,598,960` five-card hands through
//!    `poker_core::evaluate_five_cards`: hand CATEGORY (via an explicit
//!    cross-library name mapping, never an opaque int), sub-rank DETAIL, and the
//!    complete pairwise ORDERING.
//! 2. The ordering check is exhaustive, not sampled: see
//!    [`classes::compare_orderings`]. All `C(2598960,2)` ≈ 3.4e12 hand pairs are
//!    covered in `O(n)`.
//! 3. Millions of seeded random seven-card hands through
//!    `poker_core::evaluate_hand`, the function `table_canister` calls at showdown.
//! 4. The literal `sign(our_cmp(a,b)) == sign(ref_cmp(a,b))` check over random
//!    pairs, including two players sharing one board.
//! 5. Degenerate inputs no reference will accept: short boards, six or seven cards
//!    into the five-card entry point, duplicate cards.
//!
//! # Deliberately outside the repo workspace
//!
//! `tools/differential/Cargo.toml` carries its own `[workspace]` stanza so the
//! reference evaluators never enter the root `Cargo.lock` that the reproducible
//! wasm32 canister build resolves against.

pub mod adjudicator;
pub mod cards;
pub mod category;
pub mod checks;
pub mod classes;
pub mod describe;
pub mod engine;
pub mod oracles;
pub mod report;
pub mod rng;
pub mod summary;

pub const HARNESS_NAME: &str = "cleardeck-differential";
pub const HARNESS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default seeds. Fixed constants, never time-derived: a report is worthless if
/// its repro cases cannot be replayed.
pub const SEED_PAIRS: u64 = 0x0000_C1EA_2DEC_0001;
pub const SEED_SEVENS: u64 = 0x0000_C1EA_2DEC_0002;
pub const SEED_ADJUDICATOR: u64 = 0x0000_C1EA_2DEC_0003;
pub const SEED_FAST_FIVE: u64 = 0x0000_C1EA_2DEC_0004;

/// Where the report lands, in precedence order:
/// `CLEARDECK_DIFF_REPORT` (explicit path) > `$SCRATCH/differential-report.json`
/// > `./differential-report.json`.
pub fn default_report_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("CLEARDECK_DIFF_REPORT") {
        return std::path::PathBuf::from(p);
    }
    if let Ok(scratch) = std::env::var("SCRATCH") {
        if !scratch.trim().is_empty() {
            return std::path::Path::new(&scratch).join("differential-report.json");
        }
    }
    std::path::PathBuf::from("differential-report.json")
}
