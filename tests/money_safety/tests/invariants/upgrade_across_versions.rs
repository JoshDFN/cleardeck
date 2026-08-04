//! M7 — an upgrade from a PREVIOUS RELEASE must never silently lose funds.
//!
//! # Why this file exists (docs/DEFECTS.md H-16, SECURITY-FINDINGS.md FINDING 14)
//!
//! Every other "survives an upgrade" assertion in this harness upgrades the module
//! under test **to itself**: `World::upgrade` reuses `self.table_wasm`. The same
//! Candid type is on both sides of the wire, so a record-field addition is never
//! tested as an addition — and adding a field to a persisted record is the one
//! upgrade change that can destroy funds without an error.
//!
//! Two wave-2 agents, editing different regions of `lib.rs`, each added a persisted
//! field that Candid cannot read from older state:
//!
//!   * `PersistentState::deposit_watermark: u64` — top level, so `stable_restore`
//!     fails and the upgrade is REJECTED. Loud, and therefore safe.
//!   * `TableState::departed_stakes: Vec<DepartedStake>` — nested inside
//!     `opt TableState`. Candid decodes an `opt` it cannot read as **null**, so this
//!     one makes the whole table arrive as `None`, after which `post_upgrade`
//!     re-inits an empty table and every seated player's chips are gone.
//!
//! `#[serde(default)]` does not help; only `opt` is a backward-compatible addition.
//!
//! # What this test asserts, and why it is shaped this way
//!
//! NOT "the upgrade succeeds" — it does not today, and a test that demanded it
//! would just be red. The property that actually matters is weaker and stronger at
//! the same time:
//!
//!   **either the upgrade is refused, or every e8 is still there afterwards.**
//!
//! A refusal is safe: the old code and the old state stay, and a human fixes the
//! field. Silent loss is not. So this test is green on today's tree (refusal) and
//! green on a correctly fixed tree (success with funds intact), and it goes RED on
//! exactly the dangerous middle case — an upgrade that is accepted while chips
//! vanish. That middle case is what you get by fixing `deposit_watermark` alone,
//! which is the obvious one-line remedy.

use money_safety::table_api::*;
use money_safety::wasms;
use money_safety::world::*;

const ICP: u64 = 100_000_000;

#[test]
fn m7_an_upgrade_from_the_previous_release_never_silently_loses_funds() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);

    // Install the PREVIOUS release and build real state with it, so the stable
    // memory this test upgrades from was written by the old code's own
    // `pre_upgrade`, with the old record shape. This is the whole point: state
    // written by the module under test cannot exercise the addition.
    let old = wasms::previous_release_table_canister();
    world
        .reinstall_module(old.bytes.clone())
        .expect("the previous release must install");

    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(alice, 5 * ICP).expect("alice deposits");
    world.fund_escrow(bob, 5 * ICP).expect("bob deposits");
    // `join_table` is the buy-in on this engine: it moves the minimum buy-in from
    // escrow into the seat, so after these two calls there is real money in both
    // places, which is exactly the state an upgrade has to carry.
    world.join_table(alice, 0).expect("alice sits");
    world.join_table(bob, 1).expect("bob sits");

    // Measured with SCALAR queries, not `world.snapshot()`. `snapshot()` calls
    // `get_table_state`, and this harness's own `TableState` mirror declares
    // `departed_stakes: vec ...`, so decoding the OLD canister's reply fails with
    // exactly the error this test is about: `wire_type: null, expect_type: vec
    // record { ... }`. The harness mirror is version-locked to the current wire
    // shape, which is a second face of H-16: the suite cannot even OBSERVE a
    // previous release, let alone upgrade from one.
    let escrow_before = world.escrow_all().0;
    let chips_before = world.chips_total();
    let pot_before = world.pot();
    assert!(
        chips_before > 0,
        "the fixture must actually put chips in seats, or this test proves nothing: {chips_before}"
    );
    println!(
        "M7: on {} -> escrow {escrow_before}, seated chips {chips_before}, pot {pot_before}",
        &old.sha256[..12]
    );

    let outcome = world.upgrade_to_module_under_test();

    match outcome {
        Err(rejection) => {
            // Safe outcome. The old code is still running and nothing was lost.
            // Assert that explicitly rather than trusting the rejection.
            assert_eq!(
                world.escrow_all().0,
                escrow_before,
                "a REJECTED upgrade must leave escrow untouched"
            );
            assert_eq!(
                world.chips_total(),
                chips_before,
                "a REJECTED upgrade must leave seated chips untouched"
            );
            println!(
                "M7: the cross-version upgrade was REFUSED, which is the SAFE outcome, and \
                 nothing was lost. This is the state of the tree today: docs/DEFECTS.md E-38 \
                 is still open. Rejection was:\n  {rejection}"
            );
        }
        Ok(()) => {
            let after = world.snapshot();
            assert_eq!(
                after.escrow_total, escrow_before,
                "an accepted cross-version upgrade LOST ESCROW: {escrow_before} -> {}",
                after.escrow_total
            );
            assert_eq!(
                after.chips_total, chips_before,
                "an accepted cross-version upgrade DESTROYED SEATED CHIPS: {chips_before} -> {}. \
                 This is SECURITY-FINDINGS.md FINDING 14: a non-`opt` field added to TableState \
                 makes `opt TableState` decode as null, and post_upgrade then re-inits an empty \
                 table over the top of real money.",
                after.chips_total
            );
            assert_eq!(
                after.table.pot, pot_before,
                "an accepted cross-version upgrade changed the pot: {pot_before} -> {}",
                after.table.pot
            );
            println!("M7: cross-version upgrade SUCCEEDED with every e8 intact.");
        }
    }
}
