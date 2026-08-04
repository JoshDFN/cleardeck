//! ClearDeck money-safety harness.
//!
//! Six named invariants (M1..M6, see [`invariants`]) evaluated against the REAL
//! table canister wasm talking to the REAL mainnet ICP ledger wasm, installed at
//! the exact canister id the table canister hardcodes, on PocketIC.
//!
//! ```text
//!   tests/invariants.rs   one hand-written test per invariant
//!   tests/regressions.rs  a minimal reproducer per violation actually found
//!   tests/fuzz.rs         the seeded hostile-sequence fuzzer
//! ```
//!
//! Nothing here patches the engine. The instruction for this work was to
//! characterise and prove, so the harness is built to distinguish, precisely, the
//! defects already documented in `docs/SECURITY-FINDINGS.md` from new ones.

pub mod actions;
pub mod documented;
pub mod fuzz;
pub mod invariants;
pub mod legacy_ledger_shapes;
pub mod ledger;
pub mod rng;
pub mod scenario;
pub mod table_api;
pub mod wasms;
pub mod world;

pub use documented::{classify, is_documented, Disposition};
pub use invariants::{Invariant, Severity, Violation};
pub use table_api::{Card, PlayerAction, PlayerStatus, TableConfig, TableState};
pub use world::{OpError, Snapshot, World};

/// Convenience for tests: assert that a set of violations contains none that the
/// project has not already characterised.
///
/// "Characterised" means matched by a NAMED entry in [`documented::REGISTER`], not
/// "has a negative delta" -- see `documented.rs` and docs/DEFECTS.md H-03.
pub fn assert_no_new_violations(vs: &[Violation], context: &str) {
    let blocking: Vec<&Violation> = vs.iter().filter(|v| !documented::is_documented(v)).collect();
    assert!(
        blocking.is_empty(),
        "{context}: {} invariant violation(s) that are NOT documented defects:\n{}",
        blocking.len(),
        blocking
            .iter()
            .map(|v| format!(
                "  [{}] {} {:?} delta={} phase={} -- {}\n      BLOCKS BECAUSE: {}",
                v.invariant.name(),
                v.check,
                v.severity,
                v.delta_e8s,
                v.phase,
                v.detail,
                documented::blocking_reason(v).unwrap_or("")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Assert a specific invariant is currently intact.
pub fn assert_holds(vs: &[Violation], invariant: Invariant, context: &str) {
    let hits: Vec<&Violation> = vs.iter().filter(|v| v.invariant == invariant).collect();
    assert!(
        hits.is_empty(),
        "{context}: {} violated:\n{}",
        invariant.name(),
        hits.iter()
            .map(|v| format!("  delta={} phase={} -- {}", v.delta_e8s, v.phase, v.detail))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Assert a specific invariant IS violated -- used to pin a documented defect so
/// a future "fix" is a deliberate act that has to update this test.
pub fn assert_violated(vs: &[Violation], invariant: Invariant, context: &str) -> Violation {
    vs.iter()
        .find(|v| v.invariant == invariant)
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "{context}: expected {} to be VIOLATED (it is a documented defect pinned by this \
                 test) but it held. If the defect has been fixed, update this test and \
                 docs/SECURITY-FINDINGS.md together.",
                invariant.name()
            )
        })
}
