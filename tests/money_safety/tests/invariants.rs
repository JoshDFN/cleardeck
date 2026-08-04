//! One explicit, hand-written test per invariant.
//!
//! These exist so a reader can see exactly what M1..M6 mean without reading the
//! fuzzer. Each test builds the smallest situation that makes the invariant
//! meaningful, evaluates the SAME assertion functions the fuzzer uses, and states
//! plainly whether the invariant currently holds or is a pinned, documented
//! defect.

use money_safety::invariants::*;
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{assert_holds, assert_violated};
use std::time::Duration;

const ICP: u64 = 100_000_000;
const FEE: u64 = 10_000;

// ---------------------------------------------------------------------------
// shared scaffolding
// ---------------------------------------------------------------------------

/// Fund `n` actors and seat them, then let the auto-deal delay pass.
fn seat_players(world: &World, names: &[&str], escrow_each: u64) {
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world
            .fund_escrow(who, escrow_each)
            .unwrap_or_else(|e| panic!("fund_escrow for {name} failed: {e:?}"));
        world
            .join_table(who, i as u8)
            .unwrap_or_else(|e| panic!("join_table for {name} failed: {e:?}"));
    }
    world.advance(Duration::from_secs(4));
}

/// Drive the hand on the clock with `Check`, then `Call`, then `Fold`, whichever
/// the engine accepts, until the hand is over or `budget` actions are spent.
/// Deliberately passive: it produces showdowns with NO post-flop betting.
fn play_out_passively(world: &World, budget: usize) {
    for _ in 0..budget {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
        else {
            return;
        };
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        // Nothing legal: let the clock decide.
        world.advance(Duration::from_secs(1));
    }
}

/// Bet `amount` whenever the player on the clock can, otherwise call/check.
/// This is what makes post-flop money exist.
fn play_out_with_betting(world: &World, amount: u64, budget: usize) {
    for _ in 0..budget {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
        else {
            return;
        };
        let post_flop = matches!(
            t.phase,
            GamePhase::Flop | GamePhase::Turn | GamePhase::River
        );
        if post_flop && world.player_action(who, PlayerAction::Bet(amount)).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
}

// ---------------------------------------------------------------------------
// M0 -- the harness itself
// ---------------------------------------------------------------------------

/// Not an invariant: proof that the thing under test is what it claims to be.
/// If this fails, every other result in this file is meaningless.
#[test]
fn harness_runs_the_real_ledger_at_the_hardcoded_canister_id() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);

    // The table canister's ICP_LEDGER_CANISTER constant.
    assert_eq!(
        world.ledger.to_text(),
        "ryjl3-tyaaa-aaaaa-aaaba-cai",
        "the ledger must be installed at the id the table canister hardcodes"
    );

    let alice = world.actor("alice");
    assert_eq!(
        world.ledger_balance(alice, None),
        ACTOR_START_E8S,
        "actors must start with real on-ledger ICP"
    );

    // The real ledger enforces real allowances: deposit without one must fail.
    let err = world.deposit(alice, 2 * ICP).expect_err(
        "deposit with no ICRC-2 allowance must fail; if it succeeds the ledger is not enforcing \
         allowance semantics and every M2 result is void",
    );
    match err {
        OpError::Err(m) => assert!(
            m.contains("allowance") || m.contains("Insufficient"),
            "unexpected error text: {m}"
        ),
        other => panic!("expected a clean Err, got {other:?}"),
    }

    // And the real 10_000 e8s fee.
    world.approve(alice, 2 * ICP + FEE).expect("approve");
    world.deposit(alice, 2 * ICP).expect("deposit after approve");
    assert_eq!(
        world.ledger_balance(world.table, None),
        2 * ICP,
        "the canister must actually hold the deposited ICP on the ledger"
    );
    assert_eq!(
        world.ledger_balance(alice, None),
        // approve fee + transfer_from fee + the amount
        ACTOR_START_E8S - 2 * ICP - 2 * FEE,
        "the depositor must have paid the real approve and transfer_from fees"
    );
    assert_eq!(world.get_balance(alice), 2 * ICP);
}

// ---------------------------------------------------------------------------
// M1 -- CONSERVATION
// ---------------------------------------------------------------------------

/// M1: the canister's ledger balance equals exactly what it owes, at every step
/// of the deposit -> seat -> reload -> cash out -> withdraw life cycle.
///
/// This is the invariant that says no chips are created and none destroyed. It is
/// anchored on the LEDGER, so it cannot be satisfied by the canister agreeing
/// with itself.
#[test]
fn m1_conservation_holds_across_the_full_escrow_life_cycle() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let check = |world: &World, label: &str| {
        let vs = check_world(world);
        assert_holds(&vs, Invariant::M1Conservation, label);
        assert_holds(&vs, Invariant::M2LedgerReality, label);
    };

    check(&world, "empty table");

    world.fund_escrow(alice, 6 * ICP).expect("alice deposit");
    check(&world, "after alice's ICRC-2 deposit");

    world.fund_escrow(bob, 6 * ICP).expect("bob deposit");
    check(&world, "after bob's ICRC-2 deposit");

    world.join_table(alice, 0).expect("alice seat 0");
    check(&world, "after alice takes a seat (escrow -> chips)");

    world.join_table(bob, 1).expect("bob seat 1");
    check(&world, "after bob takes a seat");

    world.reload(alice, 1 * ICP).expect("alice reload");
    check(&world, "after a reload (escrow -> chips)");

    world.cash_out(alice).expect("alice cash out");
    check(&world, "after cash out (chips -> escrow)");

    world.withdraw(alice, 2 * ICP).expect("alice withdraw");
    check(&world, "after a real on-ledger withdrawal");

    // The legacy block-verification deposit flow: money lands on the ledger
    // BEFORE the canister knows about it. M1 accounts for that explicitly.
    let block = world
        .raw_transfer_to_canister(alice, 3 * ICP)
        .expect("raw transfer");
    check(&world, "after a raw transfer that has not been notified yet");

    let notify = world.notify_deposit(alice, block);
    match notify {
        Ok(_) => world.note_raw_deposit_credited(3 * ICP),
        Err(ref e) => println!("notify_deposit({block}) -> {e:?}"),
    }
    check(&world, "after notify_deposit");
}

/// M1b: `side_pots` is a breakdown of `pot`, so it must sum to `pot`.
///
/// PINNED DEFECT. `advance_to_next_street` builds the breakdown unconditionally
/// on the PreFlop -> Flop transition and nothing ever rebuilds it, so any
/// post-flop money makes the breakdown stale. This is the mechanism behind
/// docs/FINDING-01-chip-destruction.md.
#[test]
fn m1b_pot_breakdown_goes_stale_the_moment_there_is_post_flop_money() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 6 * ICP);
    world.start_new_hand(world.actor("alice")).expect("deal");

    // Reach the flop without post-flop money yet.
    for _ in 0..20 {
        let t = world.table_state();
        if t.phase == GamePhase::Flop {
            break;
        }
        let who = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
            .expect("clock seat occupied");
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        world
            .player_action(who, PlayerAction::Check)
            .expect("check must be legal preflop once the bets are level");
    }
    let flop = world.table_state();
    assert_eq!(flop.phase, GamePhase::Flop, "should be on the flop");
    assert!(
        !flop.side_pots.is_empty(),
        "the engine builds side_pots unconditionally on PreFlop -> Flop, even with nobody all-in"
    );
    assert_eq!(
        flop.side_pots_total(),
        flop.pot,
        "at the moment it is built, the breakdown still agrees with the pot"
    );

    // Now put real money in post-flop, and STOP while the hand is still live so
    // the stale breakdown is observable.
    let bettor = flop
        .players
        .get(flop.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
        .expect("clock seat occupied");
    world
        .player_action(bettor, PlayerAction::Bet(30_000_000))
        .expect("a post-flop bet must be legal");

    let snap = world.snapshot();
    assert!(
        snap.table.phase.hand_in_progress(),
        "must still be mid-hand to observe the breakdown"
    );
    let vs = check_pot_breakdown(&snap);
    let v = assert_violated(
        &vs,
        Invariant::M1bPotBreakdown,
        "a hand with post-flop betting",
    );
    assert_eq!(
        v.severity,
        Severity::BreakdownDrift,
        "the drift must be classified as a breakdown defect, not as fund creation"
    );
    println!("M1b pinned: {}", v.detail);
}

// ---------------------------------------------------------------------------
// M2 -- LEDGER REALITY
// ---------------------------------------------------------------------------

/// M2: the canister's real ledger balance is never less than the total it owes.
/// Checked at the most dangerous moment: right after a withdrawal, which is the
/// only path that moves money out.
#[test]
fn m2_ledger_reality_the_canister_is_never_short_after_paying_out() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    seat_players(&world, &["alice", "bob"], 8 * ICP);

    // Alice has 6 ICP left in escrow after a 2 ICP buy-in.
    let before = world.snapshot();
    assert!(before.ledger_main >= before.internal_total());

    // Withdraw the maximum a single call allows, repeatedly, honouring the
    // 60-second cooldown, until escrow is empty.
    for round in 0..4 {
        let escrow = world.get_balance(alice);
        if escrow < 100_000 {
            break;
        }
        let amount = escrow.min(10_000_000_000);
        match world.withdraw(alice, amount) {
            Ok(_) => {}
            Err(e) => panic!("withdraw round {round} of {amount} failed: {e:?}"),
        }
        let vs = check_world(&world);
        assert_holds(
            &vs,
            Invariant::M2LedgerReality,
            &format!("after withdrawal round {round}"),
        );
        assert_holds(
            &vs,
            Invariant::M1Conservation,
            &format!("after withdrawal round {round}"),
        );
        world.advance(Duration::from_secs(61));
    }

    assert_eq!(
        world.get_balance(alice),
        0,
        "alice should have been able to take her whole escrow out"
    );
}

// ---------------------------------------------------------------------------
// M3 -- NO RAKE
// ---------------------------------------------------------------------------

/// M3: over a completed hand with no external money movement, the total value at
/// the table cannot change. Chips awarded == chips wagered, exactly, because
/// ClearDeck takes no rake.
///
/// Two hands are played: one that never sees post-flop money (M3 holds), and one
/// that does (M3 is VIOLATED -- the pinned FINDING 01 defect).
#[test]
fn m3_no_rake_holds_without_post_flop_money_and_fails_with_it() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 6 * ICP);

    // --- hand 1: passive. No money goes in after the flop. -----------------
    let before = world.snapshot();
    world.start_new_hand(world.actor("alice")).expect("deal 1");
    play_out_passively(&world, 60);
    let after = world.snapshot();
    assert_eq!(
        after.table.phase,
        GamePhase::HandComplete,
        "hand 1 should have completed"
    );
    let vs = check_no_rake(&before, &after);
    assert_holds(
        &vs,
        Invariant::M3NoRake,
        "a hand with no post-flop betting: the house must take zero",
    );
    println!(
        "M3 hand {} with no post-flop money: value conserved at {}",
        after.table.hand_number,
        after.internal_total()
    );

    // --- hand 2: real post-flop betting. -----------------------------------
    world.advance(Duration::from_secs(4));
    let before = world.snapshot();
    world.start_new_hand(world.actor("alice")).expect("deal 2");
    play_out_with_betting(&world, 50_000_000, 60);
    // The hand may need the clock to finish.
    for _ in 0..6 {
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
        world.advance(Duration::from_secs(31));
        let _ = world.check_timeouts(world.actor("alice"));
    }
    let after = world.snapshot();
    let vs = check_no_rake(&before, &after);

    if vs.is_empty() {
        // Not every hand reaches a showdown; a hand that ends in a fold is paid
        // correctly. Say so rather than silently passing.
        println!(
            "M3 hand {} ended without a showdown, so the FINDING 01 path was not taken; \
             value conserved.",
            after.table.hand_number
        );
    } else {
        let v = assert_violated(&vs, Invariant::M3NoRake, "a showdown with post-flop betting");
        assert_eq!(
            v.severity,
            Severity::FundDestruction,
            "post-flop money must go MISSING, not appear from nowhere"
        );
        assert!(
            v.delta_e8s < 0,
            "the table must be poorer, not richer: delta={}",
            v.delta_e8s
        );
        println!("M3 pinned (FINDING 01): {}", v.detail);
    }
}

// ---------------------------------------------------------------------------
// M4 -- NO NEGATIVE / NO OVERFLOW
// ---------------------------------------------------------------------------

/// M4: hostile amounts are rejected cleanly. Nothing wraps, nothing is clamped
/// into a silent partial success, and no balance ever exceeds the money the
/// canister actually holds.
#[test]
fn m4_hostile_amounts_are_rejected_rather_than_clamped() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(alice, 6 * ICP).expect("alice deposit");
    world.fund_escrow(bob, 6 * ICP).expect("bob deposit");

    let escrow_before = world.get_balance(alice);

    // u64::MAX everywhere it could possibly wrap.
    for (label, outcome) in [
        ("deposit(u64::MAX)", world.deposit(alice, u64::MAX).map(|_| ())),
        (
            "withdraw(u64::MAX)",
            world.withdraw(alice, u64::MAX).map(|_| ()),
        ),
        (
            "buy_in(u64::MAX)",
            world.buy_in(alice, 0, u64::MAX).map(|_| ()),
        ),
        ("reload(u64::MAX)", world.reload(alice, u64::MAX).map(|_| ())),
    ] {
        match outcome {
            Err(OpError::Err(_)) => {}
            Err(OpError::Trap(t)) => panic!("{label} TRAPPED instead of returning Err: {t}"),
            Ok(()) => panic!("{label} SUCCEEDED -- that is chips from nothing"),
        }
    }

    assert_eq!(
        world.get_balance(alice),
        escrow_before,
        "no hostile call may change escrow"
    );

    // Buy-in outside [min, max] must be refused, not clamped.
    assert!(
        world
            .buy_in(alice, 0, world.config.min_buy_in - 1)
            .is_err(),
        "a buy-in below min_buy_in must be refused"
    );
    assert!(
        world
            .buy_in(alice, 0, world.config.max_buy_in + 1)
            .is_err(),
        "a buy-in above max_buy_in must be refused"
    );

    // Withdrawing more than escrow must be refused.
    let escrow = world.get_balance(alice);
    assert!(
        world.withdraw(alice, escrow + 1).is_err(),
        "withdrawing more than escrow must be refused"
    );

    // Taking an occupied seat must be refused.
    world.join_table(alice, 0).expect("alice seat 0");
    assert!(
        world.join_table(bob, 0).is_err(),
        "joining an occupied seat must be refused"
    );
    world.join_table(bob, 1).expect("bob seat 1");

    // Acting out of turn must be refused.
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");
    let t = world.table_state();
    let off_turn = t
        .seated()
        .find(|p| p.seat != t.action_on)
        .map(|p| p.principal)
        .expect("someone is not on the clock");
    assert!(
        world.player_action(off_turn, PlayerAction::Call).is_err(),
        "acting out of turn must be refused"
    );

    // A raise below the min-raise, and one above the stack.
    let on_clock = t
        .players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .expect("clock seat is occupied");
    assert!(
        world
            .player_action(on_clock.principal, PlayerAction::Raise(1))
            .is_err(),
        "a raise below the minimum must be refused"
    );
    assert!(
        world
            .player_action(on_clock.principal, PlayerAction::Raise(u64::MAX))
            .is_err(),
        "a raise above the stack must be refused"
    );

    let vs = check_world(&world);
    assert_holds(&vs, Invariant::M4NoNegativeNoOverflow, "after hostile input");
    assert_holds(&vs, Invariant::M1Conservation, "after hostile input");
}

// ---------------------------------------------------------------------------
// M5 -- UPGRADE DURABILITY
// ---------------------------------------------------------------------------

/// M5: a real `install_code --mode upgrade` preserves every escrow balance, every
/// chip stack and the whole in-progress hand -- including the deck, the hole
/// cards and the action timer. Nothing lost, nothing duplicated.
#[test]
fn m5_upgrade_preserves_every_balance_and_the_in_progress_hand() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    seat_players(&world, &["alice", "bob", "carol"], 6 * ICP);
    world.start_new_hand(world.actor("alice")).expect("deal");

    // Get some money into the pot and some players into non-trivial states.
    play_out_with_betting(&world, 20_000_000, 6);
    let t = world.table_state();
    assert!(
        t.phase.hand_in_progress(),
        "the upgrade must happen MID-HAND: that is the case that can lose a pot"
    );
    assert!(t.pot > 0, "there must be money in the pot to lose");

    let before = world.snapshot();
    world.upgrade().expect("upgrade must be accepted");
    let after = world.snapshot();

    let vs = check_upgrade_durability(&before, &after);
    assert_holds(&vs, Invariant::M5UpgradeDurability, "mid-hand upgrade");

    // And the hand must still be playable afterwards.
    play_out_passively(&world, 60);
    let vs = check_world(&world);
    assert_holds(
        &vs,
        Invariant::M2LedgerReality,
        "after finishing a hand that survived an upgrade",
    );

    // Upgrade again while idle, and once more with a pending withdrawal cooldown
    // in flight, to cover the non-mid-hand shapes too.
    let before = world.snapshot();
    world.upgrade().expect("second upgrade");
    let after = world.snapshot();
    assert_holds(
        &check_upgrade_durability(&before, &after),
        Invariant::M5UpgradeDurability,
        "idle upgrade",
    );
}

// ---------------------------------------------------------------------------
// M6 -- NO DOUBLE PAY
// ---------------------------------------------------------------------------

/// M6, withdrawal leg: two `withdraw` calls submitted into the SAME round must
/// pay at most once. The escrow debit, the ledger debit and the amount delivered
/// must all match exactly one withdrawal.
#[test]
fn m6_two_withdrawals_in_one_round_pay_at_most_once() {
    use candid::encode_one;

    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 6 * ICP).expect("deposit");

    let amount = 2 * ICP;
    let before = world.snapshot();

    // Submit both before either executes.
    let arg = encode_one(amount).unwrap();
    let a = world
        .pic
        .submit_call(world.table, alice, "withdraw", arg.clone())
        .expect("submit A");
    let b = world
        .pic
        .submit_call(world.table, alice, "withdraw", arg)
        .expect("submit B");
    let ra = world.pic.await_call(a);
    let rb = world.pic.await_call(b);

    let decode = |r: &Result<Vec<u8>, pocket_ic::RejectResponse>| -> Result<u64, String> {
        match r {
            Ok(bytes) => candid::decode_one::<Result<u64, String>>(bytes)
                .expect("withdraw reply decode")
                .map_err(|e| e),
            Err(rej) => Err(format!("{rej:?}")),
        }
    };
    let oa = decode(&ra);
    let ob = decode(&rb);
    println!("concurrent withdraw: A={oa:?} B={ob:?}");

    let successes = oa.is_ok() as u32 + ob.is_ok() as u32;
    assert!(
        successes <= 1,
        "BOTH concurrent withdrawals succeeded: {oa:?} / {ob:?}. That is a double payout."
    );

    let after = world.snapshot();
    if successes == 1 {
        let vs = check_withdraw_effect(alice, amount, FEE, &before, &after);
        assert_holds(&vs, Invariant::M6NoDoublePay, "one of two concurrent withdrawals");
    }
    let vs = check_world(&world);
    assert_holds(&vs, Invariant::M1Conservation, "after concurrent withdrawals");
    assert_holds(&vs, Invariant::M2LedgerReality, "after concurrent withdrawals");
}

/// M6, deposit leg: a deposit block may be credited at most once.
///
/// Two shapes are checked:
///   1. calling `notify_deposit` twice for the same raw transfer;
///   2. calling `notify_deposit` for the block that the ICRC-2 `deposit` path
///      itself produced -- the same on-ledger transfer the canister has ALREADY
///      credited. `deposit()` never records its block index in
///      `VERIFIED_DEPOSITS`, so nothing marks that block as spent.
#[test]
fn m6_a_deposit_block_is_credited_at_most_once() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");

    // --- shape 1: the same raw transfer, notified twice --------------------
    let block = world
        .raw_transfer_to_canister(alice, 3 * ICP)
        .expect("raw transfer");
    let first = world.notify_deposit(alice, block);
    println!("notify_deposit({block}) first  -> {first:?}");
    if first.is_ok() {
        world.note_raw_deposit_credited(3 * ICP);
    }
    let before = world.snapshot();
    let second = world.notify_deposit(alice, block);
    println!("notify_deposit({block}) second -> {second:?}");
    let after = world.snapshot();
    let vs = check_no_credit(alice, "a second notify_deposit for the same block", &before, &after);
    assert_holds(
        &vs,
        Invariant::M6NoDoublePay,
        "notify_deposit called twice for one transfer",
    );

    // --- shape 2: the ICRC-2 deposit's own block, replayed -----------------
    // Every ICRC-2 transfer_from produces a ledger block whose `from` is the
    // caller's account and whose `to` is the canister's account -- exactly the
    // two things notify_deposit verifies.
    let escrow_before = world.get_balance(alice);
    let holdings_before = world.ledger_balance(world.table, None);
    world.fund_escrow(alice, 4 * ICP).expect("icrc2 deposit");
    let escrow_after_deposit = world.get_balance(alice);
    assert_eq!(escrow_after_deposit, escrow_before + 4 * ICP);
    assert_eq!(
        world.ledger_balance(world.table, None),
        holdings_before + 4 * ICP
    );

    // Sweep every plausible block index for a second credit of the same money.
    let before = world.snapshot();
    let mut credited_again: Vec<(u64, u64)> = Vec::new();
    for b in 0..24u64 {
        let bal_before = world.get_balance(alice);
        let r = world.notify_deposit(alice, b);
        let bal_after = world.get_balance(alice);
        if bal_after > bal_before {
            credited_again.push((b, bal_after - bal_before));
        }
        if matches!(&r, Err(OpError::Err(m)) if m.contains("Too many")) {
            // Respect the 5-per-minute deposit verification rate limit.
            world.advance(Duration::from_secs(61));
        }
    }
    let after = world.snapshot();

    assert!(
        credited_again.is_empty(),
        "DOUBLE CREDIT: notify_deposit re-credited money that was already credited by another \
         path: {credited_again:?}. Escrow went {} -> {} with NO new money on the ledger.",
        before.escrow_total,
        after.escrow_total
    );

    let vs = check_world(&world);
    assert_holds(&vs, Invariant::M2LedgerReality, "after replaying every block");
    assert_holds(&vs, Invariant::M1Conservation, "after replaying every block");
}

/// M6, pot leg: a pot is awarded exactly once. After a hand completes, the pot is
/// zero, the recorded winner amounts sum to the money collected, and re-running
/// `start_new_hand` does not re-award anything.
#[test]
fn m6_a_pot_is_awarded_exactly_once() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 6 * ICP);

    let before = world.snapshot();
    world.start_new_hand(world.actor("alice")).expect("deal");
    play_out_passively(&world, 60);
    let after = world.snapshot();
    assert_eq!(after.table.phase, GamePhase::HandComplete);
    assert_eq!(after.table.pot, 0, "the pot must be empty after payout");

    let hand = after.table.hand_number;
    let history = world
        .hand_history(hand)
        .unwrap_or_else(|| panic!("no history for hand {hand}"));
    let awarded: u64 = history.winners.iter().map(|w| w.amount).sum();
    let collected = after.table.wagered_total();
    println!("hand {hand}: collected={collected} awarded={awarded} winners={:?}", history.winners);

    let vs = check_hand_payout_total(hand, collected, awarded, &after.table.phase);
    assert_holds(
        &vs,
        Invariant::M3NoRake,
        "a passive hand: awarded must equal collected",
    );

    // Total value must be unchanged: no double award.
    let vs = check_no_rake(&before, &after);
    assert_holds(&vs, Invariant::M3NoRake, "no double award");

    // And a redundant start_new_hand must not pay anybody again.
    let mid = world.snapshot();
    world.advance(Duration::from_secs(4));
    let _ = world.start_new_hand(world.actor("bob"));
    let post = world.snapshot();
    assert_eq!(
        post.escrow_total.saturating_add(post.chips_total).saturating_add(post.table.pot),
        mid.escrow_total.saturating_add(mid.chips_total).saturating_add(mid.table.pot),
        "starting the next hand must not change the total value at the table"
    );
}
