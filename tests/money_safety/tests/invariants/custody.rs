//! M10 CUSTODY VISIBILITY — the auditor's sequence, driven end to end.
//!
//! > `cash_out(bob) -> Ok = 0` while 298,000,000 e8s of bob's was in the pot;
//! > `get_balance() -> 0`; `withdraw` refused for insufficient balance; bob left.
//!
//! docs/SECURITY-FINDINGS.md FINDING 18. Every test in this file reads the
//! canister **as the player**, through caller-scoped surfaces only. That is the
//! whole point: the money was never missing, it was in a field only a controller
//! could read, so M1 balanced, M9's drain was green, and the harness agreed with
//! the canister that bob had nothing.
//!
//! These are gates, not probes. They assert the FIXED behaviour and they go red
//! on the module that has the defect — verified by running them against a build
//! with `src/table_canister/src/lib.rs` restored to `4af6dc6`.

use money_safety::invariants::custody::{committed_by_principal, read_surfaces, CustodyStatus};
use money_safety::invariants::*;
use money_safety::assert_no_new_violations;
use money_safety::table_api::*;
use money_safety::world::*;
use candid::{decode_one, Encode, Principal};
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Past the action clock AND past `STUCK_HAND_GRACE_NS` (300 s), without anybody
/// calling `check_timeouts` — which is what a table looks like when every client
/// has closed its tab (FINDING 19: nothing on chain moves the game).
const PAST_THE_GRACE: Duration = Duration::from_secs(30 + 301);

fn seats(t: &TableState) -> String {
    let s: Vec<String> = t
        .players
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
        .map(|(i, p)| {
            format!(
                "seat{i}={} chips={} bet={} folded={}",
                p.principal.to_text().split('-').next().unwrap_or(""),
                p.chips,
                p.total_bet_this_hand,
                p.has_folded
            )
        })
        .collect();
    if s.is_empty() {
        "(none)".to_string()
    } else {
        s.join("  ")
    }
}

fn custody_status(world: &World, who: Principal) -> Option<CustodyStatus> {
    world
        .query_as(who, "get_custody_status", Encode!().unwrap())
        .ok()
        .and_then(|b| decode_one::<CustodyStatus>(&b).ok())
}

/// Seat `who` with EXACTLY `chips` in front of them and EXACTLY zero escrow, so
/// `get_balance()` reporting 0 means "the canister admits to owing me nothing"
/// and not "I have some change left over". `join_table` always buys in
/// `min_buy_in`; `reload` tops the stack up from escrow.
fn seat_with_exact_stack(world: &World, who: Principal, seat: u8, chips: u64) {
    let min_buy_in = world.config.min_buy_in;
    assert!(chips >= min_buy_in, "stack must cover the minimum buy-in");
    world.fund_escrow(who, chips).expect("deposit");
    world.join_table(who, seat).expect("seat");
    let top_up = chips - min_buy_in;
    if top_up > 0 {
        world.reload(who, top_up).expect("reload");
    }
    assert_eq!(
        world.get_balance(who),
        0,
        "the fixture must leave zero escrow, or `get_balance() -> 0` proves nothing"
    );
}

// ---------------------------------------------------------------------------
// THE AUDITOR'S SEQUENCE
// ---------------------------------------------------------------------------

/// **THE GATE FOR FINDING 18.** Bob is all-in for 2.98 ICP, the table stops
/// moving, and bob cashes out. On the fixed build he must not be able to walk
/// away believing he has nothing: the withdrawal path alone has to get his money
/// back to his ledger wallet.
///
/// The assertion is deliberately about the WITHDRAWAL PATH and not about a
/// string. `cash_out -> get_balance -> withdraw` is the sequence a leaving player
/// actually performs, and it either ends with 2.98 ICP on their ledger wallet or
/// it does not.
///
/// # WHAT CHANGED IN WAVE 7, and why this fixture no longer settles the hand
///
/// This test used to advance time WITHOUT a tick and then assert that `cash_out`
/// itself had voided the hand -- `logs.contains("BY AN EXIT DOOR")`. That is
/// docs/DEFECTS.md E-59: the hand in that window is not unmovable, it is merely
/// unwatched, and the on-chain clock plays it out two rounds later. An exit door
/// that voids it is a door that pays a different set of recipients than the clock
/// would, for whoever sends a message first. So the fixture now asserts the
/// opposite of what it used to on that one point:
///
/// * during the stall, `cash_out` **must refuse**, and the refusal must state what
///   bob still has in the middle -- that is FINDING 18's actual guarantee, and it
///   is now carried by the refusal rather than by voiding the hand;
/// * once the clock has played the hand out, the withdrawal path must get bob's
///   money to his ledger wallet. That was and is the whole point.
#[test]
fn m10_the_auditors_sequence_bob_leaves_with_his_stake() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    seat_with_exact_stack(&world, alice, 0, 298 * ICP / 100);
    seat_with_exact_stack(&world, bob, 1, 298 * ICP / 100);
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    // Bob puts his whole stack in. Whoever is on the clock first is decided by the
    // blinds, so drive it generically: everybody who is asked before bob just
    // calls, and bob shoves.
    for _ in 0..4 {
        let t = world.table_state();
        let Some(seat) = t.players.get(t.action_on as usize).and_then(|p| p.as_ref()) else {
            break;
        };
        if seat.principal == bob {
            world
                .player_action(bob, PlayerAction::AllIn)
                .expect("bob shoves");
            break;
        }
        world
            .player_action(seat.principal, PlayerAction::Call)
            .expect("call");
    }

    // ...and now every client closes its tab. Nobody calls check_timeouts, and
    // nothing executes on this canister at all -- see the note above.
    let _ = world.new_canister_logs();
    world.advance_time_only(PAST_THE_GRACE);

    let t = world.table_state();
    let bob_stake = committed_by_principal(&t)
        .into_iter()
        .find(|(p, _)| *p == bob)
        .map(|(_, a)| a)
        .unwrap_or(0);
    // `get_stuck_hand_status` is a QUERY, and a PocketIC query executes no round,
    // so it still reports the pre-advance time here and says `is_stuck: false`.
    // That is a fact about the harness, not about the canister: the round the next
    // UPDATE executes runs at the advanced time, which is what the exit door sees
    // and what the log line below proves it saw.
    eprintln!(
        "AUDITOR/before: phase={:?} pot={} bob_stake={}\n    {}",
        t.phase,
        t.pot,
        bob_stake,
        seats(&t)
    );
    assert_eq!(
        bob_stake,
        298 * ICP / 100,
        "bob must be all-in for 2.98 ICP, the auditor's own number"
    );

    // ---- MID-STALL: the exit door must REFUSE, and must say what he has -----
    //
    // docs/DEFECTS.md E-59. Nothing is unmovable here. The seat on the clock has
    // simply not been folded yet because the canister has not executed since the
    // clock ran out, and voiding the hand from this state hands every stake back
    // instead of paying the winner the clock is about to produce.
    let refused = world.cash_out(bob).expect_err("cash_out must refuse: the hand is playable");
    let refused = match refused {
        OpError::Err(m) => m,
        other => panic!("expected a clean Err, got {other:?}"),
    };
    eprintln!("AUDITOR/cash_out(bob) mid-stall -> Err({refused})");
    let mid_stall_logs = world.new_canister_logs();
    assert!(
        !mid_stall_logs.iter().any(|l| l.contains("ABANDONED as unmovable")),
        "an exit door VOIDED a hand the clock goes on to play out. logs:\n  {}",
        mid_stall_logs.join("\n  ")
    );
    assert!(
        refused.contains(&bob_stake.to_string()),
        "FINDING 18: the refusal must state the exact amount of his that is in the middle, or \
         he is told nothing while the canister holds 2.98 ICP of his; got {refused:?}"
    );
    assert!(
        world.table_state().phase.hand_in_progress(),
        "and the refusal must have left the hand exactly where it was"
    );

    // ---- AND NOW THE CLOCK GETS ITS TURN -----------------------------------
    //
    // With no ingress at all. The seat that did not act is folded and the hand is
    // decided, which is the outcome bob's cash_out was racing.
    for _ in 0..12 {
        world.pic.advance_time(Duration::from_secs(10));
        for _ in 0..4 {
            world.pic.tick();
        }
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
    }
    assert!(
        !world.table_state().phase.hand_in_progress(),
        "the on-chain clock must resolve the hand with no external caller (FINDING 19)"
    );

    let cashed = world.cash_out(bob).expect("cash_out must not refuse once the hand is over");
    let balance = world.get_balance(bob);
    let surfaces = read_surfaces(&world, bob);
    eprintln!(
        "AUDITOR/cash_out(bob) -> Ok({cashed})   get_balance -> {balance}\n    {}",
        surfaces.log.join("\n    ")
    );

    assert!(
        cashed >= bob_stake,
        "FINDING 18: cash_out returned Ok({cashed}) while {bob_stake} e8s of bob's was in the \
         pot. A leaving player is told the whole truth about their money or the number is a \
         lie.\n  surfaces:\n    {}",
        surfaces.log.join("\n    ")
    );
    assert!(
        balance >= bob_stake,
        "FINDING 18: get_balance() reports {balance} while {bob_stake} e8s of bob's is still \
         inside the canister"
    );

    // The point of the whole exercise: the money reaches his wallet, using only
    // the methods the withdrawal path itself names.
    let wallet_before = world.ledger_balance(bob, None);
    world.withdraw(bob, balance).expect("withdraw");
    let wallet_after = world.ledger_balance(bob, None);
    eprintln!("AUDITOR/withdraw({balance}) -> wallet {wallet_before} -> {wallet_after}");
    assert!(
        wallet_after > wallet_before + bob_stake - 100_000,
        "bob's 2.98 ICP did not reach his ledger wallet: {wallet_before} -> {wallet_after}"
    );

    // And the table is not left holding a hand nobody is in.
    let after = world.table_state();
    eprintln!(
        "AUDITOR/after: phase={:?} pot={}  {}",
        after.phase,
        after.pot,
        seats(&after)
    );
    assert_no_new_violations(
        &check_custody_is_visible(&world, &world.snapshot()),
        "after the auditor's sequence",
    );
}

/// **THE SECOND SHAPE (docs/WAVE-06.md §2).** Both players cash out of a stuck
/// hand and the canister is left holding a live pre-flop hand with a pot and ZERO
/// seated players, telling nobody.
///
/// A hand with no players in it cannot be won by anybody, so there is exactly one
/// correct outcome and the canister must reach it by itself.
///
/// The wave-6 measurement, verbatim, on the module this repo shipped:
///
/// ```text
/// PROBE3 AFTER cash_out: phase=PreFlop pot=3000000 seats: (none)
/// PROBE3 internal_total still owed = 3000000
/// ```
#[test]
fn m10_a_live_hand_cannot_outlive_its_last_player() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    for (i, who) in [alice, bob].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");
    world.advance_time_only(PAST_THE_GRACE);

    let before = world.table_state();
    eprintln!("ORPHAN/before: phase={:?} pot={}", before.phase, before.pot);
    assert!(before.pot > 0, "the fixture must put money in the middle");

    // MID-STALL, BOTH DOORS MUST REFUSE (docs/DEFECTS.md E-59). This fixture used
    // to have both players walk out of the stall, which voided a playable hand
    // twice over. Neither may.
    let _ = world.new_canister_logs();
    for who in [alice, bob] {
        let r = world.cash_out(who);
        eprintln!("ORPHAN/mid-stall cash_out -> {r:?}");
        assert!(
            r.is_err(),
            "cash_out must not vacate a seat in a hand the clock is about to play out"
        );
    }
    assert!(
        !world
            .new_canister_logs()
            .iter()
            .any(|l| l.contains("ABANDONED as unmovable")),
        "an exit door voided a hand the clock goes on to play out"
    );
    assert!(
        world.table_state().phase.hand_in_progress(),
        "and nothing moved: the refusals must have left the hand exactly where it was"
    );

    // The clock resolves it, with no ingress. THEN both players leave, which is
    // the sequence that used to strand a live pre-flop hand at an empty table.
    for _ in 0..12 {
        world.pic.advance_time(Duration::from_secs(10));
        for _ in 0..4 {
            world.pic.tick();
        }
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
    }

    let a = world.cash_out(alice).expect("alice cash_out");
    let b = world.cash_out(bob).expect("bob cash_out");
    let after = world.table_state();
    eprintln!(
        "ORPHAN/after: cash_out alice -> {a}, bob -> {b}; phase={:?} pot={} seats: {}",
        after.phase,
        after.pot,
        seats(&after)
    );

    assert_eq!(
        after.seated().count(),
        0,
        "the fixture is supposed to empty the table"
    );
    assert!(
        !after.phase.hand_in_progress(),
        "a hand with NO players in it must not still be live: phase={:?} pot={}",
        after.phase,
        after.pot
    );
    assert_eq!(
        after.pot, 0,
        "a hand with no players must not still be holding a pot"
    );

    // Everything is where the players can reach it with one ordinary call.
    let owed = world.snapshot();
    assert_eq!(
        owed.escrow_total,
        owed.internal_total(),
        "after the last player leaves, every e8 the canister holds must be in an escrow \
         balance somebody can withdraw"
    );
    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "{} of {} e8s could not be got out:\n  {}",
        report.owed_after,
        report.owed_before,
        report.log.join("\n  ")
    );
}

/// **THE RESIDUAL CASE, and the one the strong fix does NOT remove.** A player
/// folds out of a hand that is still moving — an ordinary, contested departure,
/// which correctly leaves their stake in the pot — and the table then stops
/// moving with them already gone. Their money is now a refund waiting to be
/// asked for, and they are not at the table to be told.
///
/// This is what the reporting surfaces are for, and it is the case that proves
/// they are not decoration.
#[test]
fn m10_a_departed_stake_in_a_pot_that_stopped_moving_is_visible_to_its_owner() {
    let world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "attacker"],
    );
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for (i, who) in [alice, bob, carol].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    // Everybody puts real money in, then alice leaves the hand while it can still
    // progress. Her stake stays in the pot, correctly: it is contested.
    for _ in 0..3 {
        let t = world.table_state();
        let Some(p) = t.players.get(t.action_on as usize).and_then(|p| p.as_ref()) else {
            break;
        };
        if p.principal == alice {
            break;
        }
        world
            .player_action(p.principal, PlayerAction::Call)
            .expect("call");
    }
    let left = world.leave_table(alice).expect("alice leaves");
    let t = world.table_state();
    let alice_stake = committed_by_principal(&t)
        .into_iter()
        .find(|(p, _)| *p == alice)
        .map(|(_, a)| a)
        .unwrap_or(0);
    eprintln!(
        "DEPARTED: leave_table(alice) -> Ok({left}); her stake still in hand {} = {alice_stake}; \
         phase={:?} pot={}",
        t.hand_number, t.phase, t.pot
    );
    assert!(
        alice_stake > 0,
        "the fixture must leave a real stake behind; got {alice_stake}"
    );
    assert!(
        t.phase.hand_in_progress(),
        "the hand must still be live and movable at this point"
    );

    // ---- WHILE THE HAND IS STILL MOVING ------------------------------------
    //
    // Nothing is wrong yet. Her stake is contested and will be paid to whoever
    // wins the hand -- that is what folding means. But she is no longer at the
    // table, so every surface that is scoped to a chair now goes blank for her,
    // and this is the moment the auditor's shape begins.
    let surfaces = read_surfaces(&world, alice);
    eprintln!(
        "DEPARTED/alice's surfaces, hand still moving:\n    {}",
        surfaces.log.join("\n    ")
    );
    assert!(
        !surfaces.reports_nothing(),
        "FINDING 18: {alice_stake} e8s of alice's is in the pot, she is not at the table, and \
         every surface she can read says zero:\n    {}",
        surfaces.log.join("\n    ")
    );
    assert_eq!(
        surfaces.best_report(),
        alice_stake,
        "the figure a surface reports must be the stake itself, not an approximation"
    );
    assert_eq!(
        surfaces.custody_status,
        Some(alice_stake),
        "get_custody_status must report the stake of a seat she has already left"
    );
    assert_eq!(
        surfaces.table_view,
        Some(alice_stake),
        "the table view must carry it too: it is the call every client already makes on a loop"
    );

    let status = custody_status(&world, alice).expect("get_custody_status must answer");
    eprintln!("DEPARTED/custody_status = {status:?}");
    assert!(
        !status.committed_is_stuck,
        "the hand can still be moved at this point, and the status must not overstate it"
    );
    assert!(
        status.abandonable_in_ns.is_some(),
        "a client must be able to show WHEN the money becomes recoverable, not only whether"
    );
    assert!(
        status.advice.contains("abandon_stuck_hand"),
        "the advice must NAME the method that recovers the money; got {:?}",
        status.advice
    );
    assert_eq!(
        status.total,
        status.escrow + status.chips_at_table + status.committed_in_pot,
        "`total` must be the sum it claims to be"
    );

    // ---- AND NOW BOB AND CAROL CLOSE THEIR TABS TOO ------------------------
    //
    // Advanced without a tick, so the round the `withdraw` below executes is the
    // first thing to run at the new time and the refusal is written from the state
    // the auditor was actually in.
    world.advance_time_only(PAST_THE_GRACE);

    // The withdrawal path must state the amount and name the recovery. This is the
    // exact reply the auditor got, and the last thing the canister ever said to
    // them: "Insufficient balance. Have: 0.0000 ICP".
    let refusal = world
        .withdraw(alice, status.escrow + alice_stake)
        .expect_err("withdrawing more than escrow must refuse");
    let refusal = match refusal {
        OpError::Err(m) => m,
        other => panic!("expected a clean Err, got {other:?}"),
    };
    eprintln!("DEPARTED/withdraw refusal: {refusal}");
    assert!(
        refusal.contains("abandon_stuck_hand"),
        "withdraw's refusal must name the method that gets the money back; got {refusal:?}"
    );
    assert!(
        refusal.contains(&alice_stake.to_string()),
        "withdraw's refusal must state the exact amount that is committed; got {refusal:?}"
    );

    // **AND IT MUST NOT SAY THE HAND IS DEAD.** docs/DEFECTS.md E-59: this
    // assertion used to be `refusal.contains("no longer be moved")`, and the
    // sentence was false -- the hand is playable and the clock plays it out. Alice
    // FOLDED by leaving, so the outcome she was being invited to buy with one
    // permissionless call is the return of a stake she had already surrendered.
    assert!(
        !refusal.contains("no longer be moved"),
        "the canister told a player who folded that the hand is dead, at the moment its own \
         clock was about to award her stake to somebody else; got {refusal:?}"
    );
    let denied = world.abandon_stuck_hand(alice);
    eprintln!("DEPARTED/abandon_stuck_hand(alice) -> {denied:?}");
    assert!(
        denied.is_err(),
        "a hand nobody has watched fail is not abandonable, whoever asks"
    );

    // The clock decides it, and her stake goes where folding sends it.
    let owed_before = world.snapshot().internal_total();
    for _ in 0..12 {
        world.pic.advance_time(Duration::from_secs(10));
        for _ in 0..4 {
            world.pic.tick();
        }
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
    }
    let after = world.table_state();
    eprintln!(
        "DEPARTED/after the clock: phase={:?} pot={}  {}",
        after.phase,
        after.pot,
        seats(&after)
    );
    assert!(
        !after.phase.hand_in_progress(),
        "the on-chain clock must resolve it with no external caller"
    );
    assert_eq!(
        world.snapshot().internal_total(),
        owed_before,
        "resolving the hand must not create or destroy an e8"
    );
    let residual = custody_status(&world, alice).expect("get_custody_status must answer");
    assert_eq!(
        residual.committed_in_pot, 0,
        "once the hand is settled nothing of hers is in the middle any more: {residual:?}"
    );
    assert!(
        residual.advice.is_empty() || !residual.advice.contains("no longer be moved"),
        "and no surface may still be describing a hand that is over: {residual:?}"
    );
}

/// The invariant itself, on an ordinary healthy table: it must be SILENT. A gate
/// that fires on normal play is a gate everybody turns off.
#[test]
fn m10_is_silent_on_an_ordinary_hand() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for (i, who) in [alice, bob, carol].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    for _ in 0..12 {
        let violations = check_custody_is_visible(&world, &world.snapshot());
        assert!(
            violations.is_empty(),
            "M10 fired during ordinary play, where a seated player's stake is on the screen in \
             front of them:\n{violations:#?}"
        );
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(p) = t.players.get(t.action_on as usize).and_then(|p| p.as_ref()) else {
            break;
        };
        if world
            .player_action(p.principal, PlayerAction::Check)
            .is_err()
        {
            let _ = world.player_action(p.principal, PlayerAction::Call);
        }
    }
}
