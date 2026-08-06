//! THE ADMIN CUSTODY SURFACE — docs/SECURITY-FINDINGS.md FINDING 07.
//!
//! # What this file is for
//!
//! One controller call used to destroy 100% of a funded table's chips, and every
//! instrument in this project said the table was fine. `reset_table` and
//! `admin_reinit_table` had byte-identical bodies, both replaced `TableState`
//! wholesale, and the Candid described the second one as *"for recovery after
//! upgrade issues"* — so the interface pointed an honest operator at the button
//! that deletes everybody's money.
//!
//! Measured on the running local replica on 2026-08-05, before the fix:
//!
//! ```text
//!   T2 seated              ledger_main=12_000_000_000  chips=4_000_000_000  claims=4_000_000_000
//!   reset_table            -> Ok
//!   T3 after reset_table   ledger_main=12_000_000_000  chips=0              claims=0
//!   cash_out  -> Err("Not at table")
//!   withdraw  -> Err("Insufficient balance. Have: 0.0000 ICP, requested: 20.0000 ICP")
//! ```
//!
//! 40.00000000 ICP, gone, with the canister still holding it on the ledger and no
//! restore path (`admin_restore_balance` was deliberately removed).
//!
//! # The rule these tests enforce
//!
//! > **No controller-callable method may reduce what this canister owes players
//! > without paying those players.**
//!
//! `sweep_no_admin_call_reduces_what_is_owed` is the general form and is the one
//! that catches the NEXT door somebody adds; the named tests around it are the
//! specific behaviours the fix has to have. `census_every_controller_gated_method`
//! fails when a method appears in the engine that this file has never classified,
//! so the sweep cannot silently stop covering the surface.
//!
//! # Run it
//!
//! ```text
//! cd tests/money_safety
//! CLEARDECK_TABLE_WASM=../../target/wasm32-unknown-unknown/release/table_canister.wasm \
//!   cargo test --test admin_custody -- --nocapture --test-threads=2
//! ```

use candid::{decode_one, encode_one, Encode, Principal};
use money_safety::documented;
use money_safety::invariants::{self, reachability, Severity};
use money_safety::table_api::*;
use money_safety::world::*;
use std::collections::BTreeMap;
use std::time::Duration;

const ICP: u64 = 100_000_000;

// ---------------------------------------------------------------------------
// plumbing
// ---------------------------------------------------------------------------

/// Outcome of a raw controller call, kept in a shape a transcript can print.
#[derive(Debug)]
enum AdminOutcome {
    Ok(String),
    Err(String),
    Rejected(String),
}

impl AdminOutcome {
    fn is_ok(&self) -> bool {
        matches!(self, AdminOutcome::Ok(_))
    }
    fn message(&self) -> String {
        match self {
            AdminOutcome::Ok(s) | AdminOutcome::Err(s) | AdminOutcome::Rejected(s) => s.clone(),
        }
    }
}

/// Call `method` as the controller and decode a `Result<T, String>` reply.
fn admin_call<T>(world: &World, method: &str, arg: Vec<u8>) -> AdminOutcome
where
    T: candid::CandidType + serde::de::DeserializeOwned + std::fmt::Debug,
{
    match world.pic.update_call(world.table, world.controller, method, arg) {
        Err(reject) => AdminOutcome::Rejected(format!("{reject:?}")),
        Ok(bytes) => match decode_one::<Result<T, String>>(&bytes) {
            Ok(Ok(v)) => AdminOutcome::Ok(format!("{v:?}")),
            Ok(Err(msg)) => AdminOutcome::Err(msg),
            // Not every admin method returns Result<_, String>; a decode failure
            // here is a wiring mistake in the TEST, and must read as one.
            Err(e) => AdminOutcome::Rejected(format!("reply decode failed for {method}: {e}")),
        },
    }
}

fn reset_table(world: &World, config: &TableConfig) -> AdminOutcome {
    admin_call::<()>(world, "reset_table", Encode!(config).unwrap())
}

fn admin_reinit_table(world: &World, config: &TableConfig) -> AdminOutcome {
    admin_call::<()>(world, "admin_reinit_table", Encode!(config).unwrap())
}

fn admin_update_config(world: &World, config: &TableConfig) -> AdminOutcome {
    admin_call::<TableConfig>(world, "admin_update_config", Encode!(config).unwrap())
}

fn admin_return_all_chips_to_escrow(world: &World) -> AdminOutcome {
    admin_call::<u64>(world, "admin_return_all_chips_to_escrow", Encode!().unwrap())
}

/// The canister's own escrow ledger, per principal.
fn escrow_of(world: &World, who: Principal) -> u64 {
    world.snapshot().escrow.get(&who).copied().unwrap_or(0)
}

/// A funded, seated table: `n` actors, each with `deposit` in escrow and then
/// `min_buy_in` of it converted into chips by sitting down.
fn seated_world(names: &[&str]) -> World {
    let world = World::new(TableConfig::six_max_icp(), names);
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world.fund_escrow(who, 20 * ICP).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    world
}

/// Every invariant this harness can evaluate from one observation, asserted.
fn assert_all_invariants_green(world: &World, context: &str) {
    let vs = invariants::check_world(world);
    money_safety::assert_no_new_violations(&vs, context);
}

// ---------------------------------------------------------------------------
// 1. reset_table: a CONFIG reset may not touch custody
// ---------------------------------------------------------------------------

/// **THE FIX FOR FINDING 07, DOOR ONE.**
///
/// Before: `reset_table` returned `Ok` and every seated chip ceased to exist.
/// After: it refuses, changes nothing, and names the recovery call.
#[test]
fn reset_table_refuses_while_the_table_holds_chips() {
    let world = seated_world(&["alice", "bob"]);
    let before = world.snapshot();
    assert!(
        before.chips_total > 0,
        "the reproduction needs money at the table"
    );

    let outcome = reset_table(&world, &TableConfig::six_max_icp());
    eprintln!("reset_table on a funded table -> {outcome:?}");

    assert!(
        !outcome.is_ok(),
        "FINDING 07: reset_table returned Ok on a table holding {} e8s of seated chips. \
         That call deletes every one of them with no way for anybody, including a controller, \
         to get them back.",
        before.chips_total
    );
    let msg = outcome.message();
    for expected in ["admin_return_all_chips_to_escrow", "FINDING 07"] {
        assert!(
            msg.contains(expected),
            "the refusal must tell the operator what to do instead; it does not mention \
             {expected:?}: {msg}"
        );
    }

    let after = world.snapshot();
    assert_eq!(
        after.chips_total, before.chips_total,
        "a refused reset must not move a single chip"
    );
    assert_eq!(after.escrow, before.escrow, "escrow must be untouched");
    assert_eq!(
        after.ledger_main, before.ledger_main,
        "the ledger must be untouched"
    );
    assert_all_invariants_green(&world, "after a refused reset_table");
}

/// The other half of the same rule: the fix must not break the legitimate use.
/// An operator reconfiguring an EMPTY table is doing nothing dangerous and must
/// not be blocked, or the refusal above will simply be removed by whoever needs
/// to change the blinds.
#[test]
fn reset_table_still_works_on_an_empty_table() {
    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let new_config = TableConfig {
        small_blind: 5_000_000,
        big_blind: 10_000_000,
        min_buy_in: 500_000_000,
        ..TableConfig::six_max_icp()
    };
    let outcome = reset_table(&world, &new_config);
    assert!(
        outcome.is_ok(),
        "reset_table must still reconfigure an empty table: {outcome:?}"
    );
    assert_eq!(
        world.table_state().config.big_blind,
        10_000_000,
        "the new configuration must actually be in force"
    );

    // And with money in ESCROW but nobody seated: escrow is a different ledger,
    // this path provably never writes it, so there is nothing to protect here and
    // blocking it would be pure friction.
    let alice = world.actor("alice");
    world.fund_escrow(alice, 6 * ICP).expect("deposit");
    let outcome = reset_table(&world, &TableConfig::six_max_icp());
    assert!(
        outcome.is_ok(),
        "reset_table must not be blocked by escrow alone: {outcome:?}"
    );
    assert_eq!(
        escrow_of(&world, alice),
        6 * ICP,
        "and the escrow must survive it"
    );
}

/// The claim `reset_table`'s doc comment in `src/table_canister/src/lib.rs` makes
/// by name. It is why the refusal is keyed on TABLE custody and not on escrow.
#[test]
fn reset_table_never_touches_escrow() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(alice, 7 * ICP).expect("deposit");
    world.fund_escrow(bob, 3 * ICP).expect("deposit");
    let before = world.snapshot().escrow.clone();

    assert!(reset_table(&world, &TableConfig::six_max_icp()).is_ok());
    assert!(admin_reinit_table(&world, &TableConfig::six_max_icp()).is_ok());

    assert_eq!(
        world.snapshot().escrow,
        before,
        "neither reset door may write BALANCES"
    );
    // And the money is still really there: escrow is a claim, the ledger is a fact.
    assert!(world.withdraw(alice, 5 * ICP).is_ok(), "alice must be able to withdraw after a reset");
}

// ---------------------------------------------------------------------------
// 2. admin_reinit_table: RECOVERY must recover, not destroy
// ---------------------------------------------------------------------------

/// **THE FIX FOR FINDING 07, DOOR TWO.**
///
/// The door the Candid pointed at. It now pays every chip back to the player who
/// owns it before it rebuilds anything, so the operator who reaches for "recovery
/// after upgrade issues" gets recovery.
#[test]
fn admin_reinit_table_returns_every_chip_to_the_player_who_owns_it() {
    let mut world = seated_world(&["alice", "bob", "carol"]);
    let before = world.snapshot();
    let chips_by_owner: BTreeMap<Principal, u64> = before
        .table
        .seated()
        .map(|p| (p.principal, p.chips))
        .collect();
    assert_eq!(chips_by_owner.len(), 3);

    let outcome = admin_reinit_table(&world, &TableConfig::six_max_icp());
    assert!(outcome.is_ok(), "admin_reinit_table must succeed: {outcome:?}");

    let after = world.snapshot();
    assert_eq!(after.chips_total, 0, "the table was rebuilt, so the seats are empty");
    assert_eq!(
        after.internal_total(),
        before.internal_total(),
        "TOTAL LIABILITY MUST BE UNCHANGED TO THE E8. Before {} after {}: the money moved from \
         chips into escrow, it did not go anywhere else.",
        before.internal_total(),
        after.internal_total()
    );
    for (owner, chips) in &chips_by_owner {
        let gained = escrow_of(&world, *owner)
            .saturating_sub(before.escrow.get(owner).copied().unwrap_or(0));
        assert_eq!(
            gained, *chips,
            "each player's OWN chips must land in their OWN escrow: {owner} was holding {chips} \
             and gained {gained}"
        );
    }
    assert_all_invariants_green(&world, "after admin_reinit_table");

    // And it is real money, not a bookkeeping entry: take it all out to the ledger.
    let report = reachability::drain(&mut world);
    assert!(
        report.table_is_really_empty(),
        "after the reinit every player must still be able to reach their money. owed_after={} \
         orphaned={}\n  {}",
        report.owed_after,
        report.orphaned_e8s(),
        report.log.join("\n  ")
    );
    for owner in chips_by_owner.keys() {
        assert!(
            report.returned_to_wallets.get(owner).copied().unwrap_or(0) > 0,
            "{owner} got no ledger money back"
        );
    }
}

/// Mid-hand, the money is not in the stacks — it is in the pot, and some of it may
/// belong to somebody who has already left the chair. The recovery path uses the
/// SAME payout basis settlement uses (`hand_stakes`), so a stake reaches its owner
/// and not the occupant of its seat (FINDING 13).
#[test]
fn admin_reinit_table_mid_hand_returns_the_pot_to_the_players_who_put_it_in() {
    let mut world = seated_world(&["alice", "bob", "carol"]);
    let before = world.snapshot();
    world.advance(Duration::from_secs(4));
    world.start_new_hand(world.actor("alice")).expect("deal");

    // Build a real multi-street pot.
    for _ in 0..6 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
        else {
            break;
        };
        if world.player_action(who, PlayerAction::Raise(ICP / 2)).is_err() {
            let _ = world.player_action(who, PlayerAction::Call);
        }
        if world.pot() >= ICP {
            break;
        }
    }
    let mid = world.snapshot();
    assert!(mid.table.pot > 0, "the reproduction needs a real pot");
    eprintln!(
        "mid-hand: pot={} chips={} escrow={}",
        mid.table.pot, mid.chips_total, mid.escrow_total
    );

    let outcome = admin_reinit_table(&world, &TableConfig::six_max_icp());
    assert!(outcome.is_ok(), "admin_reinit_table mid-hand must succeed: {outcome:?}");

    let after = world.snapshot();
    assert_eq!(after.table.pot, 0);
    assert_eq!(after.chips_total, 0);
    assert_eq!(
        after.internal_total(),
        mid.internal_total(),
        "the pot and the stacks must both come back, to the e8"
    );
    // Everybody ends holding exactly what they walked in with: the hand is
    // abandoned, not settled, so nobody wins and nobody loses.
    for actor in &world.actors {
        assert_eq!(
            escrow_of(&world, actor.principal),
            20 * ICP,
            "{} must get their whole 20 ICP back, stack and stake alike",
            actor.name
        );
    }
    assert_eq!(before.internal_total(), after.internal_total());
    assert_all_invariants_green(&world, "after a mid-hand admin_reinit_table");

    let report = reachability::drain(&mut world);
    assert!(
        report.table_is_really_empty(),
        "owed_after={} orphaned={}",
        report.owed_after,
        report.orphaned_e8s()
    );
}

/// **THE BLIND SPOT IN THE FIRST VERSION OF THIS FILE.**
///
/// Every test above starts from a table that has never dealt a hand, and the first
/// implementation of `table_custody` was green against all of them while being
/// wrong: `Player::total_bet_this_hand` is cleared only by `start_new_hand`, so
/// between hands every seat still reports the bets of the hand that is already
/// OVER while `state.pot` is back to zero. Reading the stakes there counts the same
/// money twice, and the conservation guard then refused every ordinary
/// post-hand table with `state.pot says 0 but the stakes ... sum to 2000000000`.
///
/// It was found by deploying to the local replica and calling the method on a real
/// table, not by this suite — which is the standing lesson of this project in one
/// line. This test is the suite catching up: it plays a hand to completion FIRST.
#[test]
fn the_admin_doors_work_on_a_table_that_has_already_played_a_hand() {
    let mut world = seated_world(&["alice", "bob"]);
    let liability = world.snapshot().internal_total();
    world.advance(Duration::from_secs(4));
    world.start_new_hand(world.actor("alice")).expect("deal");

    // Play it out however it goes; all that matters is that it FINISHES.
    for _ in 0..12 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
        else {
            break;
        };
        if world.player_action(who, PlayerAction::Call).is_err() {
            let _ = world.player_action(who, PlayerAction::Fold);
        }
    }
    for _ in 0..4 {
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
        world.advance(Duration::from_secs(120));
        let _ = world.check_timeouts(world.actor("alice"));
    }
    let t = world.table_state();
    assert!(
        !t.phase.hand_in_progress(),
        "the fixture needs a FINISHED hand; phase is {:?}",
        t.phase
    );
    let stale: u64 = t.seated().map(|p| p.total_bet_this_hand).sum();
    assert_eq!(t.pot, 0, "a finished hand leaves no pot");
    assert!(
        stale > 0,
        "THE FIXTURE HAS STOPPED REPRODUCING THE HAZARD. It depends on \
         Player::total_bet_this_hand still carrying the finished hand's bets while pot is 0. If \
         the engine now clears it at settlement, that is a good change -- but this test is no \
         longer measuring anything and must be rewritten, not deleted."
    );
    eprintln!("post-hand: pot=0 but the seats still report {stale} e8s of total_bet_this_hand");

    // Door one: still refuses, because there are still chips in the seats -- and it
    // must refuse for THAT reason, not because it miscounted the stale bets.
    let refusal = reset_table(&world, &TableConfig::six_max_icp());
    assert!(!refusal.is_ok());
    assert!(
        refusal.message().contains(&format!("holding {} in seated chips", world.snapshot().chips_total)),
        "the refusal must count the STACKS, not the stale bets: {}",
        refusal.message()
    );

    // Door two: must work, and conserve.
    let outcome = admin_reinit_table(&world, &TableConfig::six_max_icp());
    assert!(
        outcome.is_ok(),
        "admin_reinit_table must work on an ordinary post-hand table. This is the exact call that \
         failed on the live replica: {}",
        outcome.message()
    );
    let after = world.snapshot();
    assert_eq!(
        after.internal_total(),
        liability,
        "and it must conserve to the e8 across a played hand"
    );
    assert_eq!(after.chips_total, 0);
    assert_all_invariants_green(&world, "after a post-hand admin_reinit_table");

    let report = reachability::drain(&mut world);
    assert!(
        report.table_is_really_empty(),
        "owed_after={} orphaned={}",
        report.owed_after,
        report.orphaned_e8s()
    );
}

/// The standalone recovery primitive, which is what the refusal in door one points
/// at. It reports what it moved, and moving is all it can do.
#[test]
fn admin_return_all_chips_to_escrow_conserves_and_reports_the_total() {
    let world = seated_world(&["alice", "bob"]);
    let before = world.snapshot();

    let outcome = admin_return_all_chips_to_escrow(&world);
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(
        outcome.message(),
        before.chips_total.to_string(),
        "it must report exactly what it moved"
    );

    let after = world.snapshot();
    assert_eq!(after.chips_total, 0);
    assert_eq!(after.escrow_total, before.escrow_total + before.chips_total);
    assert_eq!(after.internal_total(), before.internal_total());
    assert_eq!(
        after.ledger_main, before.ledger_main,
        "it must not touch the ledger"
    );
    assert_all_invariants_green(&world, "after admin_return_all_chips_to_escrow");

    // Idempotent, and honest about it.
    let again = admin_return_all_chips_to_escrow(&world);
    assert!(again.is_ok());
    assert_eq!(again.message(), "0");
}

// ---------------------------------------------------------------------------
// 3. THE GENERAL RULE — the test that catches the NEXT door
// ---------------------------------------------------------------------------

/// **No controller-callable method may reduce what this canister owes players
/// without paying those players.**
///
/// FINDING 07 survived four waves because the destructive step was a shared helper
/// two differently-named public methods called, and nothing in the project ever
/// asked the general question. This asks it: drive the whole admin surface against
/// a funded, seated table and assert, after every single call, that
///
/// ```text
///   owed_before - owed_after  <=  ledger_before - ledger_after
/// ```
///
/// i.e. liability may only fall by money that actually LEFT the canister. Admin
/// calls make no transfers, so in practice the right-hand side is zero and the rule
/// reduces to "liability may not fall at all".
#[test]
fn sweep_no_admin_call_reduces_what_is_owed() {
    let world = seated_world(&["alice", "bob"]);
    let stranger = world.actor("bob");
    let cfg = TableConfig::six_max_icp();

    // Every controller-callable UPDATE in the engine, with a plausible argument.
    // Queries cannot move money and are covered by the census test below.
    let calls: Vec<(&str, Vec<u8>)> = vec![
        ("set_history_canister", Encode!(&Some(stranger)).unwrap()),
        ("set_dev_mode", encode_one(true).unwrap()),
        ("add_controller", Encode!(&stranger).unwrap()),
        ("remove_controller", Encode!(&stranger).unwrap()),
        ("admin_update_config", Encode!(&cfg).unwrap()),
        ("reset_table", Encode!(&cfg).unwrap()),
        ("admin_reinit_table", Encode!(&cfg).unwrap()),
        ("admin_return_all_chips_to_escrow", Encode!().unwrap()),
        // Not controller-gated, but it is the other update on the admin surface a
        // controller reaches for, and it must not move money either.
        ("flush_unrecorded_hands", Encode!().unwrap()),
    ];

    for (method, arg) in calls {
        let before = world.snapshot();
        let owed_before = before.internal_total();
        let outcome = match world
            .pic
            .update_call(world.table, world.controller, method, arg)
        {
            Ok(_) => "replied".to_string(),
            Err(r) => format!("REJECTED {r:?}"),
        };
        let after = world.snapshot();
        let owed_after = after.internal_total();
        let paid_out = before.ledger_main.saturating_sub(after.ledger_main);
        let liability_drop = owed_before.saturating_sub(owed_after);

        eprintln!(
            "SWEEP {method:<34} {outcome:<12} owed {owed_before} -> {owed_after}  \
             ledger {} -> {}",
            before.ledger_main, after.ledger_main
        );

        assert!(
            liability_drop <= paid_out,
            "FINDING 07 SHAPE: `{method}` reduced what the canister owes players by {liability_drop} \
             e8s while only {paid_out} e8s left the canister on the ledger. {} e8s stopped being \
             owed to anybody without being paid to anybody. That is the exact shape of FINDING 07 \
             and it is never acceptable on a controller-callable method.",
            liability_drop - paid_out
        );
        // And the standing invariant, read off the ledger, after every one of them.
        money_safety::assert_no_new_violations(
            &invariants::check_world(&world),
            &format!("after controller call `{method}`"),
        );
    }
}

/// The sweep above can only cover the methods it names. This one reads the engine
/// and fails when a controller-gated method exists that this file has never
/// classified, so the surface cannot grow behind the sweep's back.
///
/// Every name below is one this wave audited for the FINDING 07 shape: *can this
/// admin action reduce what the canister owes players without paying them?*
#[test]
fn census_every_controller_gated_method_is_classified_here() {
    /// (method, verdict). The verdict is the audit result, recorded so a reader can
    /// see the whole surface in one place.
    const AUDITED: &[(&str, &str)] = &[
        // --- CANNOT touch custody at all -----------------------------------
        ("set_history_canister", "no custody: writes HISTORY_ID"),
        ("add_controller", "no custody: privilege only"),
        ("remove_controller", "no custody: privilege only"),
        ("admin_get_balance", "query, read-only"),
        ("admin_get_all_balances", "query, read-only"),
        ("admin_get_table_chips", "query, read-only"),
        // --- CAN touch custody, and is therefore guarded -------------------
        (
            "reset_table",
            "FINDING 07 door one: refuses while the table holds chips or a pot",
        ),
        (
            "admin_reinit_table",
            "FINDING 07 door two: returns every chip to its owner's escrow first",
        ),
        (
            "admin_return_all_chips_to_escrow",
            "the recovery primitive: conserving by construction, escrow only goes up",
        ),
        (
            "admin_update_config",
            "FINDING 20: refuses a currency change while the canister owes anybody anything",
        ),
    ];

    /// On the admin surface, driven by the sweep, and deliberately NOT
    /// controller-gated. Recorded so "it has no require_controller" is a decision
    /// somebody wrote down rather than an omission nobody noticed, and asserted
    /// below so a gate appearing on one of them is a change, not a surprise.
    const AUDITED_UNGATED: &[(&str, &str)] = &[
        (
            "set_dev_mode",
            "no custody, no gate needed: the body is `Err(\"Dev mode is no longer supported\")` \
             for every caller, so there is no privileged behaviour to protect",
        ),
        (
            "flush_unrecorded_hands",
            "no custody: pushes already-settled hand records at the archive. Open to any \
             non-anonymous caller on purpose (a player must be able to make their own evidence \
             durable) and rate-limited by a table-wide cooldown",
        ),
    ];

    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/table_canister/src/lib.rs"
    ))
    .expect("src/table_canister/src/lib.rs must be readable");

    // Every `fn NAME` whose body calls `require_controller()`.
    let lines: Vec<&str> = src.lines().collect();
    let mut found: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("require_controller()") {
            continue;
        }
        let owner = lines[..i].iter().rev().find_map(|l| {
            let t = l.trim_start();
            let rest = t
                .strip_prefix("fn ")
                .or_else(|| t.strip_prefix("async fn "))
                .or_else(|| t.strip_prefix("pub fn "))?;
            rest.split('(').next().map(|s| s.to_string())
        });
        if let Some(name) = owner {
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found.sort();
    assert!(
        found.len() > 5,
        "the census found only {found:?}; it has stopped parsing the engine and is now vacuous"
    );

    let classified: Vec<&str> = AUDITED.iter().map(|(m, _)| *m).collect();
    let unclassified: Vec<&String> = found
        .iter()
        // `require_controller` itself, and any helper that merely re-exports it.
        .filter(|n| n.as_str() != "require_controller")
        .filter(|n| !classified.contains(&n.as_str()))
        .collect();
    assert!(
        unclassified.is_empty(),
        "NEW CONTROLLER-CALLABLE METHOD(S) WITH NO RECORDED AUDIT: {unclassified:?}\n\
         Every controller-gated method must be audited for the FINDING 07 shape -- can it reduce \
         what the canister owes players without paying them? -- and then named in AUDITED in \
         tests/money_safety/tests/admin_custody.rs, and added to the sweep above if it is an \
         update. See docs/SECURITY-FINDINGS.md FINDING 07."
    );

    let missing: Vec<&str> = classified
        .iter()
        .copied()
        .filter(|m| !found.contains(&m.to_string()))
        .collect();
    assert!(
        missing.is_empty(),
        "these methods are classified here but are no longer controller-gated in the engine: \
         {missing:?}. Either the gate was removed (a privilege-escalation defect) or the method \
         was, in which case delete its row."
    );

    let newly_gated: Vec<&str> = AUDITED_UNGATED
        .iter()
        .map(|(m, _)| *m)
        .filter(|m| found.contains(&m.to_string()))
        .collect();
    assert!(
        newly_gated.is_empty(),
        "{newly_gated:?} are recorded here as deliberately open and now call \
         require_controller(). That may be right, but it is a change to who can call them: move \
         the row into AUDITED with an audit verdict."
    );
}

// ---------------------------------------------------------------------------
// 4. FINDING 20 — currency is not a display setting, it selects the LEDGER
// ---------------------------------------------------------------------------

/// Found while auditing the surface for FINDING 07's shape, and the same family:
/// an admin call that makes what the canister owes unpayable without paying it.
/// `Currency` chooses which ledger `withdraw` transfers from, so flipping an ICP
/// table to BTC while balances exist points every withdrawal at a ledger this
/// canister holds nothing on.
#[test]
fn currency_cannot_be_changed_while_the_canister_owes_anybody_anything() {
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    // (a) with money in escrow only.
    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 5 * ICP).expect("deposit");
    for (name, outcome) in [
        ("reset_table", reset_table(&world, &btc)),
        ("admin_reinit_table", admin_reinit_table(&world, &btc)),
        ("admin_update_config", admin_update_config(&world, &btc)),
    ] {
        assert!(
            !outcome.is_ok(),
            "FINDING 20: `{name}` accepted a currency flip while 5 ICP sat in escrow. Every \
             withdrawal would then be attempted against the ckBTC ledger."
        );
        assert!(
            outcome.message().contains("currency"),
            "the refusal must say why: {}",
            outcome.message()
        );
    }
    assert_eq!(
        world.table_state().config.currency,
        Currency::ICP,
        "and the table must still be an ICP table"
    );
    assert!(
        world.withdraw(alice, 4 * ICP).is_ok(),
        "alice's ICP must still be withdrawable"
    );

    // (b) with money at the table.
    let seated = seated_world(&["alice", "bob"]);
    assert!(!reset_table(&seated, &btc).is_ok());
    assert!(!admin_reinit_table(&seated, &btc).is_ok());

    // (c) on a genuinely empty canister it is an ordinary config change.
    let empty = World::new(TableConfig::six_max_icp(), &["alice"]);
    assert!(
        reset_table(&empty, &btc).is_ok(),
        "an empty table may be re-denominated"
    );
    assert_eq!(empty.table_state().config.currency, Currency::BTC);
}

// ---------------------------------------------------------------------------
// 5. THE INSTRUMENT ITSELF — it must be red on the numbers the defect produced
// ---------------------------------------------------------------------------

/// The gate on the gate.
///
/// M9's drain reported `fully_drained` while 4 ICP sat unclaimable, because
/// `internal_total` counts what the canister SAYS it owes and the attack changes
/// exactly that. `check_no_orphaned_custody` is anchored to `icrc1_balance_of`
/// instead. This drives it with the numbers the wave-6 coherence pass measured and
/// requires it to be RED, so the instrument cannot rot into a no-op the way the
/// ones before it did.
#[test]
fn the_orphan_invariant_is_red_on_the_numbers_finding_07_produced() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let mut snap = world.snapshot();

    // The measured post-reset state: the canister holds 4 ICP on the ledger and
    // says it owes nothing at all.
    snap.ledger_main = 4_000_000_000;
    snap.escrow_total = 0;
    snap.escrow.clear();
    snap.chips_total = 0;
    snap.table.pot = 0;

    let vs = reachability::check_no_orphaned_custody(&snap, 0);
    assert_eq!(
        vs.len(),
        1,
        "the orphan check must fire on 4 ICP held against zero claims; it produced {vs:?}"
    );
    assert_eq!(vs[0].severity, Severity::OrphanedCustody);
    assert_eq!(vs[0].delta_e8s, 4_000_000_000);
    assert_eq!(
        documented::blocking_reason(&vs[0]),
        Some(
            "this severity is never excusable: chips from nothing, a double payout, state lost \
             across an upgrade, a rake, or the canister reporting an unregistered failure"
        ),
        "orphaned custody must be unexcusable by the register, whatever anybody adds to it"
    );

    // The exact same numbers with the money still OWED are not a finding: the check
    // must convict unowned money, not merely a large balance.
    snap.escrow_total = 4_000_000_000;
    assert!(
        reachability::check_no_orphaned_custody(&snap, 0).is_empty(),
        "money the canister owes somebody is not orphaned"
    );

    // And a raw deposit the canister has not been asked to credit yet is the one
    // legitimate reason to hold more than you owe.
    snap.escrow_total = 0;
    assert!(
        reachability::check_no_orphaned_custody(&snap, 4_000_000_000).is_empty(),
        "an uncredited raw deposit is claimable via notify_deposit and is not orphaned"
    );
}

/// The drain's own verdict must be ledger-anchored too, because that is the
/// number the auditor and the fuzzer both read. `fully_drained()` alone is the
/// claim; `table_is_really_empty()` is the claim plus the fact.
#[test]
fn a_drain_report_that_says_fully_drained_is_not_enough() {
    let report = reachability::DrainReport {
        owed_before: 4_000_000_000,
        owed_after: 0,
        ledger_main_after: 4_000_000_000,
        uncredited_raw: 0,
        returned_to_wallets: BTreeMap::new(),
        log: vec!["reset_table -> Ok".to_string()],
    };
    assert!(
        report.fully_drained(),
        "this is the state M9 called `fully_drained`, and it was right about what it measured"
    );
    assert!(
        !report.table_is_really_empty(),
        "and it must not be enough: 4 ICP is inside a canister that owes it to nobody"
    );
    assert_eq!(report.orphaned_e8s(), 4_000_000_000);

    let vs = reachability::check_drain(&report, &GamePhase::WaitingForPlayers);
    assert_eq!(
        vs.len(),
        1,
        "check_drain must convict this report exactly once: {vs:?}"
    );
    assert_eq!(vs[0].check, "drain_left_money_that_belongs_to_nobody");
    assert_eq!(vs[0].severity, Severity::OrphanedCustody);
    assert!(documented::blocking_reason(&vs[0]).is_some());
}

/// End to end: the sequence the auditor ran, on the current module, checked with
/// the new instrument. Before the fix this left 4 ICP orphaned and every
/// invariant green; it must now leave nothing orphaned by any route.
#[test]
fn the_auditors_sequence_leaves_nothing_that_belongs_to_nobody() {
    let mut world = seated_world(&["alice", "bob"]);
    let cfg = TableConfig::six_max_icp();
    let held = world.snapshot().ledger_main;

    eprintln!("reset_table        -> {:?}", reset_table(&world, &cfg));
    eprintln!("admin_reinit_table -> {:?}", admin_reinit_table(&world, &cfg));

    let snap = world.snapshot();
    eprintln!(
        "after both doors: ledger_main={} claims={} (escrow {} + chips {} + pot {})",
        snap.ledger_main,
        snap.internal_total(),
        snap.escrow_total,
        snap.chips_total,
        snap.table.pot
    );
    // THE NEW INSTRUMENT FIRST, deliberately. On a module with the guards removed
    // this is the line that fails, and it fails with the orphan violation naming
    // the exact number of e8s that stopped belonging to anybody -- which is what
    // the whole harness was unable to say before this wave.
    assert_all_invariants_green(&world, "after the auditor's two calls");
    assert_eq!(snap.ledger_main, held, "no money left the canister");
    assert_eq!(
        snap.internal_total(),
        held,
        "and every e8 of it is still owed to somebody"
    );

    let report = reachability::drain(&mut world);
    assert!(
        report.table_is_really_empty(),
        "owed_after={} orphaned={} -- FINDING 07 is NOT fixed.\n  {}",
        report.owed_after,
        report.orphaned_e8s(),
        report.log.join("\n  ")
    );
    for actor in &world.actors {
        assert!(
            report
                .returned_to_wallets
                .get(&actor.principal)
                .copied()
                .unwrap_or(0)
                > 0,
            "{} got no ledger money back after the two admin calls",
            actor.name
        );
    }
    eprintln!(
        "FINDING 07 CLOSED: both doors ran, {} e8s stayed owed to their owners, and every e8 \
         reached a real wallet.",
        held
    );
}
