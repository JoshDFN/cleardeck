//! M9 FUND REACHABILITY -- the auditor's lock, and the property that outranks it.
//!
//! > **M9.** For any state a sequence of legal calls can reach, there exists a
//! > sequence of legal calls by each funded player that returns that player's
//! > balance to the ledger.
//!
//! docs/SECURITY-FINDINGS.md FINDING 15. An independent auditor, playing an
//! ordinary hand on the running local stack, locked about 420 ICP:
//!
//! ```text
//! check_timeouts   IC0503 trap: IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or
//!                  5-card board (flop/turn/river), got 0 community cards
//! player_action    the same trap
//! leave_table      the same trap
//! withdraw         Err("Cannot withdraw while in a hand")
//! cash_out         Err("Cannot cash out while in a hand")
//! ```
//!
//! Every invariant this harness had was silent, and correctly so: not one chip had
//! gone missing. **That is the gap.** Conservation cannot see a table nobody can
//! empty.
//!
//! The exact sequence, reproduced against the deployed module `0x5298915c…` on the
//! running local replica, is in the wave report; it is replayed here against the
//! module built from THIS source, and it must end with every player's money back
//! on the ledger.

use money_safety::actions::{Act, Op, StepResult};
use money_safety::assert_no_new_violations;
use money_safety::invariants::reachability::{self, MONEY_DOORS};
use money_safety::invariants::*;
use money_safety::table_api::*;
use money_safety::world::*;
use std::time::Duration;

const ICP: u64 = 100_000_000;

fn on_clock(t: &TableState) -> Option<candid::Principal> {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
}

/// Seat `n` players with a real ICRC-2 deposit and deal a hand.
fn seated_hand(config: TableConfig, names: &[&str]) -> World {
    let world = World::new(config, names);
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world.fund_escrow(who, 20 * ICP).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world
        .start_new_hand(world.actor(names[0]))
        .expect("deal a hand");
    world
}

// ---------------------------------------------------------------------------
// 1. The auditor's exact sequence
// ---------------------------------------------------------------------------

/// **THE REPRODUCER.** Pre-flop, three funded seats, one player's client stops
/// sending heartbeats, and then ordinary play continues.
///
/// On the module the auditor was given, this state closed every door at once. The
/// mechanism, from that canister's own log:
///
/// ```text
/// poker_core::hand::evaluate_hand
/// table_canister::plan_payouts
/// table_canister::settle_hand
/// table_canister::end_hand_single_winner
/// canister_update leave_table
/// ```
///
/// A seat that stops heartbeating was dropped from the count that decides
/// "everybody else folded" while staying in the set that decides "who may win the
/// pot". So the engine ran the fold-out settlement with TWO seats still holding
/// live claims and no board to rank them by, and the evaluator trapped on the
/// impossible input rather than refusing it.
///
/// What this test asserts now, in order:
///
/// * no call traps, at any point;
/// * the hand reaches a real conclusion;
/// * and every player gets every e8 back out to the ledger.
#[test]
fn m9_the_auditors_disconnect_sequence_leaves_every_door_open() {
    let mut world = seated_hand(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");

    let staked = world.snapshot().internal_total();
    assert!(staked > 0, "the table must actually hold money");

    // Carol's client stops sending heartbeats. `check_timeouts` marks a silent
    // seat `Disconnected` after 30 s and does NOT fold it; `sit_out` reaches the
    // identical state in one call, which is how the auditor's table got there.
    world.sit_out(carol).expect("carol goes quiet");

    // Ordinary play continues: whoever is on the clock folds, repeatedly. On the
    // deployed module the FIRST of these traps.
    let mut outcomes: Vec<String> = Vec::new();
    for _ in 0..6 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let who = match on_clock(&t) {
            Some(w) => w,
            None => break,
        };
        let r = world.player_action(who, PlayerAction::Fold);
        if let Err(OpError::Trap(m)) = &r {
            panic!(
                "M9 VIOLATED: player_action TRAPPED with {staked} e8s at the table. A trap rolls \
                 the message back, so this door is shut permanently for this state, and \
                 leave_table and check_timeouts run the same path while withdraw and cash_out \
                 refuse during a hand. This is docs/SECURITY-FINDINGS.md FINDING 15: {m}"
            );
        }
        outcomes.push(format!("{r:?}"));
    }

    // Every door, tried explicitly, exactly as the auditor tried them.
    for who in [alice, bob, carol] {
        if let Err(OpError::Trap(m)) = world.check_timeouts(who) {
            panic!("M9 VIOLATED: check_timeouts TRAPPED: {m}");
        }
    }
    for who in [alice, bob] {
        match world.leave_table(who) {
            Err(OpError::Trap(m)) => panic!("M9 VIOLATED: leave_table TRAPPED: {m}"),
            _ => {}
        }
    }

    // And now the part that matters: the money comes out.
    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "M9 VIOLATED: {} of {} e8s could not be got out by any player call.\n  {}",
        report.owed_after,
        report.owed_before,
        report.log.join("\n  ")
    );
    for who in [alice, bob, carol] {
        assert!(
            report.returned_to_wallets.get(&who).copied().unwrap_or(0) > 0,
            "every funded player must end with real ledger money back in their own wallet; \
             {who} got none.\n  {}",
            report.log.join("\n  ")
        );
    }
    assert_no_new_violations(
        &check_point_in_time(&world.snapshot(), world.uncredited_raw_deposits),
        "after the auditor's sequence and a full drain",
    );
}

/// The same state, reached the way the auditor actually reached it: by a silent
/// client and the 30-second disconnect timer, with nobody calling `sit_out`.
#[test]
fn m9_a_silent_client_mid_hand_does_not_lock_the_table() {
    // A long action clock, so the seat really is taken out of the hand by SILENCE
    // and not by its own timer running out. That is the auditor's condition
    // exactly: a live pre-flop hand, one client not sending heartbeats.
    let config = TableConfig {
        action_timeout_secs: 600,
        ..TableConfig::six_max_icp()
    };
    let mut world = seated_hand(config, &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let staked = world.snapshot().internal_total();

    // Alice and Bob keep heartbeating; Carol does not. Past the 90 s disconnect
    // sweep, well inside the action clock.
    for _ in 0..10 {
        world.advance(Duration::from_secs(11));
        world.heartbeat(alice).expect("alice is connected");
        world.heartbeat(bob).expect("bob is connected");
    }
    // The disconnect sweep runs.
    if let Err(OpError::Trap(m)) = world.check_timeouts(alice) {
        panic!("M9 VIOLATED: check_timeouts TRAPPED after a silent seat: {m}");
    }
    assert_eq!(
        world
            .table_state()
            .players
            .iter()
            .flatten()
            .filter(|p| p.status == PlayerStatus::Disconnected)
            .count(),
        1,
        "exactly one seat should have been marked Disconnected"
    );

    // Play on to a conclusion, folding every time, and never trap.
    for _ in 0..24 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let who = match on_clock(&t) {
            Some(w) => w,
            None => break,
        };
        for act in [Act::Fold, Act::Check, Act::Call] {
            match world.player_action(who, act.to_candid()) {
                Err(OpError::Trap(m)) => panic!(
                    "M9 VIOLATED: player_action TRAPPED with {staked} e8s at the table: {m}"
                ),
                Ok(()) => break,
                Err(OpError::Err(_)) => continue,
            }
        }
        if let Err(OpError::Trap(m)) = world.check_timeouts(alice) {
            panic!("M9 VIOLATED: check_timeouts TRAPPED: {m}");
        }
    }

    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "M9 VIOLATED: {} e8s stranded.\n  {}",
        report.owed_after,
        report.log.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 2. The escape hatch, tested as a hatch rather than as a decoration
// ---------------------------------------------------------------------------

/// A hand nobody is acting on, left alone forever, must still be endable by
/// somebody, and the money must come back to the people who staked it.
///
/// This is the door that does not depend on the settlement path being correct. It
/// exists because assuming the settlement path is correct is what produced
/// FINDING 15 in the first place.
#[test]
fn m9_a_hand_nobody_can_move_is_abandonable_by_anybody() {
    let mut world = seated_hand(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let carol = world.actor("carol");
    let staked_in_pot = world.table_state().pot;
    assert!(staked_in_pot > 0, "blinds must be in the pot");

    // Not stuck yet: the clock is running, and refusing here is the whole reason
    // this cannot be used as a weapon against a slow player.
    let status = world.stuck_hand_status();
    assert!(!status.is_stuck, "a fresh hand is not stuck");
    assert!(
        matches!(world.abandon_stuck_hand(alice), Err(OpError::Err(_))),
        "a hand that can still progress must NOT be abandonable"
    );

    // Nobody acts, for a long time.
    world.advance(Duration::from_secs(30 + 300 + 5));
    let status = world.stuck_hand_status();
    assert!(status.is_stuck, "an hour-dead clock is a stuck hand");
    assert_eq!(status.refundable_pot, staked_in_pot);

    // Carol did not deal the hand, is not on the clock and holds no privilege.
    let refunded = world
        .abandon_stuck_hand(carol)
        .expect("anybody may end a hand that nobody can move");
    assert_eq!(
        refunded, staked_in_pot,
        "abandoning returns exactly what the hand collected"
    );
    assert_eq!(world.table_state().pot, 0);
    assert!(!world.table_state().phase.hand_in_progress());

    // Everybody has exactly what they started the hand with: nobody won, nobody
    // lost, and in particular nobody's blind was handed to anybody else.
    assert_no_new_violations(
        &check_point_in_time(&world.snapshot(), world.uncredited_raw_deposits),
        "after abandoning a stuck hand",
    );

    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "M9 VIOLATED: {} e8s stranded after abandoning.\n  {}",
        report.owed_after,
        report.log.join("\n  ")
    );
}

/// The two guards that were the second half of the lock. "Cannot withdraw while in
/// a hand" is a fair rule for a hand in play and no rule at all for a hand that has
/// stopped moving.
///
/// # WHAT THIS TEST STOPPED COVERING WHEN THE ON-CHAIN CLOCK LANDED, STATED PLAINLY
///
/// The second half used to construct a stuck hand by advancing 30 + 300 + 5
/// seconds and then requiring `withdraw` and `cash_out` to succeed *while the hand
/// was still live*. That construction no longer produces a stuck hand:
/// docs/DEFECTS.md E-54 put a clock on chain, and a hand that is merely stale is
/// exactly what that clock is for. It folds the seat whose timer expired and plays
/// the hand out, so by the time `cash_out` is called the hand is over and the
/// refusal is gone because there is no hand -- not because the guard lifted.
///
/// It failed here as `Err("Cannot cash out while in a hand")`, in the window
/// during which `withdraw`'s own ledger await let the clock advance the hand.
///
/// **So this test no longer exercises the `hand_is_stuck` branch inside `withdraw`
/// and `cash_out`.** That is a real coverage loss and it is recorded rather than
/// hidden. What still covers that branch:
///
/// * `src/table_canister/src/lib.rs::stuck_hand_tests` pins `hand_is_stuck` itself,
///   including the live-hand-with-no-clock arm that the clock deliberately will
///   NOT resolve (`clock_schedule_tests` pins that asymmetry);
/// * `m9_a_stuck_hand_can_be_abandoned_by_anybody` above still drives the real
///   `abandon_stuck_hand` door end to end.
///
/// What this test asserts now is the property a player actually cares about, which
/// is strictly what M9 is for: **after the table goes quiet, their money comes
/// back** -- whether the clock played the hand out or the guard lifted.
#[test]
fn m9_the_in_a_hand_refusal_lifts_once_the_hand_cannot_progress() {
    let mut world = seated_hand(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");

    // While the hand is live and moving, the refusal stands. That is deliberate,
    // and this test pins it so the fix cannot be "delete the guard".
    assert!(
        matches!(world.withdraw(alice, 1 * ICP), Err(OpError::Err(ref e)) if e.contains("in a hand")),
        "a player in a live hand may not withdraw"
    );
    assert!(
        matches!(world.cash_out(alice), Err(OpError::Err(ref e)) if e.contains("in a hand")),
        "a player in a live hand may not cash out"
    );

    world.advance(Duration::from_secs(30 + 300 + 5));

    // Escrow is not in the hand at all -- a withdrawal cannot touch the pot -- so
    // this must work whichever way the hand went.
    let before = world.snapshot().internal_total();
    world
        .withdraw(alice, 1 * ICP)
        .expect("escrow must be withdrawable once the hand cannot progress");
    assert!(
        world.snapshot().internal_total() < before,
        "the withdrawal must actually have left the canister"
    );

    // And every player's whole stack must come back. This is the assertion that
    // matters: no route left open, no e8 stranded, nobody depending on a support
    // ticket. It holds whether the clock played the hand out or the guard lifted.
    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "M9 VIOLATED: {} e8s were still unreachable after the table went quiet for \
         30 + 300 + 5 seconds. Neither the on-chain clock nor the stuck-hand guard \
         got the money back.\n  {}",
        report.owed_after,
        report.log.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// 3. The class, not the instance
// ---------------------------------------------------------------------------

/// No trapping evaluator may be reachable from an update entry point.
///
/// A grep, and deliberately a grep: it is the only form of this assertion that
/// covers call sites nobody has written yet. `poker_core::evaluate_hand` traps on
/// an impossible input, which is correct for a pure library and catastrophic for
/// this canister, where a trap is a permanently closed door rather than a failed
/// call. `try_evaluate_hand` returns the rejection instead, and every site in
/// `table_canister` must use it.
///
/// Two of the three sites were guarded by hand in wave 3 and the third was not.
/// "Guarded by hand at each site" is exactly the discipline that failed; this test
/// replaces it with a rule.
#[test]
fn no_trapping_evaluator_is_reachable_from_an_update_entry_point() {
    let src = std::fs::read_to_string(
        money_safety::wasms::repo_root().join("src/table_canister/src/lib.rs"),
    )
    .expect("read the table canister source");

    let offenders: Vec<(usize, &str)> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let code = l.trim_start();
            // Comments and doc comments may name it; code may not.
            !code.starts_with("//") && !code.starts_with("///")
        })
        .filter(|(_, l)| {
            // `evaluate_hand(` not preceded by `try_` or `_unchecked`.
            l.match_indices("evaluate_hand(").any(|(i, _)| {
                let before = &l[..i];
                !before.ends_with("try_") && !before.ends_with("five_cards_")
            })
        })
        .map(|(i, l)| (i + 1, l.trim()))
        .collect();

    assert!(
        offenders.is_empty(),
        "src/table_canister/src/lib.rs calls the TRAPPING evaluator. On the settlement path a \
         trap is not a failed call, it is every door a player has closing at once and staying \
         closed -- docs/SECURITY-FINDINGS.md FINDING 15. Use \
         `poker_core::try_evaluate_hand(..)` and handle the rejection.\n{}",
        offenders
            .iter()
            .map(|(n, l)| format!("  lib.rs:{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // And the import list, so the trapping name cannot come back through the door
    // it left by.
    assert!(
        !src.contains("use poker_core::{create_deck, evaluate_hand"),
        "`evaluate_hand` must not be imported into table_canister at all"
    );
}

/// The per-step M9 check has to be able to fail, or it is decoration.
///
/// Feeds it a synthetic transcript entry and asserts it fires, and asserts it
/// stays silent on the two cases it must not fire on: a trap on an empty table
/// (a robustness bug, not a custody one) and an ordinary `Err` return.
#[test]
fn m9s_per_step_check_can_actually_fail() {
    let trapped = StepResult {
        op: Op::LeaveTable { actor: 0 },
        outcome: "TRAP(IMPOSSIBLE HAND: evaluate_hand needs a 3-card board)".to_string(),
    };
    let vs = check_no_settlement_trap(&trapped, 42_000_000_000, &GamePhase::PreFlop);
    assert_eq!(vs.len(), 1, "a settlement trap over real money must fire M9");
    assert_eq!(vs[0].severity, Severity::FundsUnreachable);
    assert!(
        !money_safety::documented::is_documented(&vs[0]),
        "M9 must never be excusable by the documented-defect register"
    );

    assert!(
        check_no_settlement_trap(&trapped, 0, &GamePhase::PreFlop).is_empty(),
        "a trap on an empty table is not a custody failure"
    );
    let refused = StepResult {
        op: Op::LeaveTable { actor: 0 },
        outcome: "Err(Not at table)".to_string(),
    };
    assert!(
        check_no_settlement_trap(&refused, 42_000_000_000, &GamePhase::PreFlop).is_empty(),
        "a clean refusal is not a closed door"
    );

    // The doors this invariant is about, named once so a reader can check the list
    // against the canister's Candid rather than against this comment.
    assert_eq!(MONEY_DOORS.len(), 6);
}
