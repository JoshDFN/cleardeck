//! M8 PRINCIPAL ATTRIBUTION, driven against the REAL canister.
//!
//! # The defect this file is the gate for (docs/SECURITY-FINDINGS.md FINDING 13)
//!
//! The wave-2 payout fix resolved a payout's owner from its SEAT INDEX. A player
//! who leaves the table mid-hand leaves their stake in the pot; if somebody else
//! takes that chair before the hand settles, the refund of the departed player's
//! stake was credited to the NEW OCCUPANT.
//!
//! Every gate the project had was blind to it, and not by accident:
//!
//!   * M1/M2 CONSERVATION — passes. The canister holds exactly what it owes.
//!   * M3 NO RAKE — passes. The plan awards exactly what it collected.
//!   * the settlement oracle — passes. It diffs per SEAT, and the seat is paid the
//!     right amount; it is the principal behind the seat that is wrong.
//!   * the canister's own logs — silent. Nothing is inconsistent.
//!
//! So this file asserts on PRINCIPALS, and on nothing else.
//!
//! # The sequence, every step an ordinary public API call
//!
//! ```text
//! alice(0) bob(1) carol(2) are seated and a hand is dealt; all three post to the flop
//! alice   leave_table()  -> departed stake (seat 0, owner alice, BB) stays in the pot;
//!                           two active players remain, so the hand continues
//! dave    join_table(0)  -> takes alice's CHAIR mid-hand: SittingOut, no hole cards
//! dave    sit_in()       -> Active (docs/DEFECTS.md E-36)
//! bob     leave_table()  -> departed stake (seat 1, owner bob); carol + dave remain
//! carol   leave_table()  -> one "active" player left (dave, who holds no cards), so the
//!                           hand settles. NO seat has a live claim on any layer, so every
//!                           stake is refunded to whoever put it in.
//! ```
//!
//! The refund of seat 0 is the whole test. It belongs to alice. Resolving it from
//! the seat names dave.
//!
//! Measured against the pre-fix build, this test reports:
//!
//! ```text
//! principal <dave> staked NOTHING in this hand and came out of it +2000000 e8s richer
//! principal <alice> came out of this hand -2000000 e8s when the rules of poker owe them +0
//! ```

use candid::Principal;
use money_safety::invariants::*;
use money_safety::scenario::on_clock;
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{assert_holds, assert_violated};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Deal a hand with the three seated players and stop once the flop is out, so
/// every one of them has the big blind in the pot and the hand is still live.
fn deal_to_the_flop(world: &World) {
    world
        .start_new_hand(world.actors[0].principal)
        .expect("start_new_hand must succeed with three seated players");
    for _ in 0..24 {
        let t = world.table_state();
        if t.phase == GamePhase::Flop || !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            continue;
        };
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
}

/// Set up the FINDING 13 state and run it to settlement.
///
/// Returns `(world, before, staked, expected, stake_of_the_departed)` where
/// `before` is the per-principal value snapshot taken with the table idle.
fn play_the_finding13_sequence() -> (
    World,
    BTreeMap<Principal, u64>,
    BTreeSet<Principal>,
    BTreeMap<Principal, i128>,
    u64,
) {
    let world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "dave"],
    );
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    let dave = world.actor("dave");

    for who in [alice, bob, carol, dave] {
        world.fund_escrow(who, 5 * ICP).expect("deposit");
    }
    world.join_table(alice, 0).expect("alice sits");
    world.join_table(bob, 1).expect("bob sits");
    world.join_table(carol, 2).expect("carol sits");
    world.advance(Duration::from_secs(4));

    // Baseline with the table IDLE: no pot, so every e8 is attributable.
    let idle = world.table_state();
    assert!(
        !idle.phase.hand_in_progress() && idle.pot == 0,
        "the baseline must be taken between hands, got {:?} pot {}",
        idle.phase,
        idle.pot
    );
    let before = values_now(&world);

    deal_to_the_flop(&world);
    let live = world.table_state();
    assert_eq!(
        live.phase,
        GamePhase::Flop,
        "the fixture needs a live hand on the flop"
    );
    let stake = live
        .player_at(0)
        .expect("alice is still seated at 0")
        .total_bet_this_hand;
    assert!(stake > 0, "alice must have money in the pot");
    for seat in 0..3u8 {
        assert_eq!(
            live.player_at(seat).map(|p| p.total_bet_this_hand),
            Some(stake),
            "all three seats must have staked the same, or leaving triggers an \
             uncalled-bet return and the fixture stops being the one described"
        );
    }

    // 1. alice leaves, and her stake stays in the pot with her name on it.
    world.leave_table(alice).expect("alice leaves mid-hand");
    let after_alice = world.table_state();
    assert_eq!(
        after_alice
            .departed()
            .iter()
            .map(|d| (d.seat, d.principal, d.contributed))
            .collect::<Vec<_>>(),
        vec![(0u8, alice, stake)],
        "alice's departure must record HER as the owner of the stake at seat 0"
    );
    assert!(
        after_alice.phase.hand_in_progress(),
        "two active players remain, so the hand must still be live, not {:?}",
        after_alice.phase
    );

    // 2. dave takes alice's CHAIR, mid-hand, and sits in (docs/DEFECTS.md E-36).
    world.join_table(dave, 0).expect("dave takes seat 0 mid-hand");
    world.sit_in(dave).expect("dave sits in");
    let retaken = world.table_state();
    assert_eq!(
        retaken.player_at(0).map(|p| p.principal),
        Some(dave),
        "seat 0 must now be dave's chair"
    );
    assert_eq!(
        retaken.player_at(0).and_then(|p| p.hole_cards),
        None,
        "a mid-hand arrival holds no cards, so he can win nothing"
    );
    assert_eq!(
        retaken.player_at(0).map(|p| p.total_bet_this_hand),
        Some(0),
        "dave has staked nothing in this hand"
    );

    // 3. bob leaves. carol and dave are still 'active', so the hand continues.
    world.leave_table(bob).expect("bob leaves mid-hand");
    // 4. carol leaves. Only dave is left, and he holds no cards, so the hand
    //    settles with NO live claim on any layer: every stake is refunded.
    world.leave_table(carol).expect("carol leaves mid-hand");

    let end = world.table_state();
    assert!(
        !end.phase.hand_in_progress(),
        "the hand must have settled once the last card-holder left, phase is {:?}",
        end.phase
    );
    assert_eq!(end.pot, 0, "a settled hand leaves no pot");

    // Every stake was refunded to whoever put it in, so nobody's value changed.
    let staked: BTreeSet<Principal> = [alice, bob, carol].into_iter().collect();
    let expected: BTreeMap<Principal, i128> = [alice, bob, carol, dave]
        .iter()
        .map(|p| (*p, 0i128))
        .collect();
    (world, before, staked, expected, stake)
}

/// THE GATE. On principals, and only on principals.
#[test]
fn m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair() {
    let (world, before, staked, expected, stake) = play_the_finding13_sequence();
    let after = values_now(&world);
    let alice = world.actor("alice");
    let dave = world.actor("dave");

    println!(
        "M8: stake {stake} e8s left in the pot by {alice}; the chair was then taken by {dave}"
    );
    for (who, delta) in value_deltas(&before, &after) {
        println!("M8:   {who} {delta:+}");
    }

    // The oracle-free half first: it is the one that needs no derivation at all.
    let free = check_no_free_money(&before, &after, &staked, &GamePhase::HandComplete);
    assert_holds(
        &free,
        Invariant::M8PrincipalAttribution,
        "a principal who staked nothing must not be paid",
    );

    // And the full statement: every stake back to its own owner, to the e8.
    let vs = check_principal_attribution(&before, &after, &expected, &GamePhase::HandComplete);
    assert_holds(
        &vs,
        Invariant::M8PrincipalAttribution,
        "every refunded stake reaches the principal who put it in",
    );

    // Stated again as bare numbers, so a reader of the failure sees the two sides
    // of the same e8 rather than a violation list.
    let d = value_deltas(&before, &after);
    assert_eq!(
        d.get(&alice).copied().unwrap_or(0),
        0,
        "alice put {stake} into the pot and must have got exactly {stake} back; she is the \
         only person that money ever belonged to"
    );
    assert_eq!(
        d.get(&dave).copied().unwrap_or(0),
        0,
        "dave took alice's chair and staked nothing. Any gain here is alice's money in his \
         hands: docs/SECURITY-FINDINGS.md FINDING 13."
    );

    // Where the money physically went: alice's escrow, not dave's stack.
    let end = world.table_state();
    assert_eq!(
        end.player_at(0).map(|p| p.chips),
        Some(world.config.min_buy_in),
        "dave's stack must still be exactly his buy-in"
    );
    assert!(
        end.player_at(0).map(|p| p.principal) == Some(dave),
        "seat 0 is still dave's chair"
    );

    // And the RECORD names alice, so the hand history and the client cannot report
    // her money as his.
    let history = world
        .hand_history(end.hand_number)
        .expect("the settled hand must be in the history");
    let named: Vec<(u8, Principal, u64)> = history
        .winners
        .iter()
        .map(|w| (w.seat, w.principal, w.amount))
        .collect();
    assert!(
        named.iter().any(|(s, p, a)| *s == 0 && *p == alice && *a == stake),
        "the recorded winner list must credit seat 0's {stake} to {alice}; it says {named:?}"
    );
    assert!(
        !named.iter().any(|(_, p, _)| *p == dave),
        "the recorded winner list must not name {dave} at all; it says {named:?}"
    );
    assert_eq!(
        history.awarded_total(),
        3 * stake,
        "all three stakes must be accounted for in the record"
    );
}

/// The instrument check: M8 must actually be able to FAIL.
///
/// A gate that cannot go red is decoration. This drives the same assertion
/// functions with a deliberately wrong expectation and requires a violation, which
/// is how the wave-1 `is_documented_defect` hole (docs/DEFECTS.md H-03) is kept
/// shut for the new invariant too.
#[test]
fn m8_convicts_a_misattributed_payout() {
    let alice = Principal::from_slice(&[1]);
    let stranger = Principal::from_slice(&[9]);
    let before: BTreeMap<Principal, u64> = [(alice, 1_000u64), (stranger, 7u64)].into_iter().collect();
    // The shape of FINDING 13 exactly: alice is 50 short, the stranger is 50 up,
    // and the total is unchanged -- which is why conservation cannot see it.
    let after: BTreeMap<Principal, u64> = [(alice, 950u64), (stranger, 57u64)].into_iter().collect();
    assert_eq!(
        before.values().sum::<u64>(),
        after.values().sum::<u64>(),
        "the fixture must conserve, or it is not testing the blind spot"
    );

    let expected: BTreeMap<Principal, i128> = [(alice, 0i128), (stranger, 0i128)].into_iter().collect();
    let vs = check_principal_attribution(&before, &after, &expected, &GamePhase::HandComplete);
    assert_violated(
        &vs,
        Invariant::M8PrincipalAttribution,
        "a conserving misattribution must still be a violation",
    );
    assert_eq!(vs.len(), 2, "both sides of the misattribution: {vs:?}");
    assert!(
        vs.iter().all(|v| money_safety::documented::blocking_reason(v).is_some()),
        "a misattribution must be BLOCKING, whatever the register says: {vs:?}"
    );

    let staked: BTreeSet<Principal> = [alice].into_iter().collect();
    let free = check_no_free_money(&before, &after, &staked, &GamePhase::HandComplete);
    assert_violated(
        &free,
        Invariant::M8PrincipalAttribution,
        "a principal who staked nothing and gained must be convicted",
    );
    assert_eq!(free.len(), 1);
    assert_eq!(free[0].delta_e8s, 50);
}

/// An ordinary hand must satisfy M8b as well: nobody who is not at the table gains.
///
/// Cheap, and it puts the invariant on the normal path rather than only on the
/// pathological one, so a future change that starts paying a bystander is caught
/// even if nobody re-runs the FINDING 13 sequence.
#[test]
fn m8b_an_ordinary_hand_pays_nobody_who_did_not_stake() {
    let mut world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "dave"],
    );
    let dave = world.actor("dave");
    // dave is funded but never sits down: a bystander with escrow at the canister.
    for name in ["alice", "bob", "carol", "dave"] {
        world
            .fund_escrow(world.actor(name), 5 * ICP)
            .expect("deposit");
    }
    for (i, name) in ["alice", "bob", "carol"].iter().enumerate() {
        world
            .join_table(world.actor(name), i as u8)
            .expect("join_table");
    }
    world.advance(Duration::from_secs(4));

    let before = values_now(&world);
    let outcome = money_safety::scenario::play_one_hand(&mut world, 20_000_000);
    let after = values_now(&world);

    let staked: BTreeSet<Principal> = world
        .table_state()
        .seated()
        .filter(|p| p.total_bet_this_hand > 0)
        .map(|p| p.principal)
        .collect();
    assert!(
        !staked.is_empty(),
        "the hand must have collected something: {outcome:?}"
    );
    let vs = check_no_free_money(&before, &after, &staked, &GamePhase::HandComplete);
    assert_holds(
        &vs,
        Invariant::M8PrincipalAttribution,
        "an ordinary hand must not pay a bystander",
    );
    assert_eq!(
        value_deltas(&before, &after).get(&dave).copied().unwrap_or(0),
        0,
        "dave never sat down and must be exactly where he started"
    );
}
