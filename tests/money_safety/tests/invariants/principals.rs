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
use money_safety::hand_attribution::{HandAttribution, HandAttributionWatch};
use money_safety::invariants::*;
use money_safety::scenario::on_clock;
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{assert_holds, assert_violated};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const ICP: u64 = 100_000_000;

// ---------------------------------------------------------------------------
// a watched hand driver, shared by the ordinary-hand tests below
// ---------------------------------------------------------------------------

/// Play one hand with the attribution watch fed after EVERY message.
///
/// Feeding it every message rather than only at the end is what makes a mid-hand
/// departure observable: `departed_stakes` is the only record of a departed
/// player's contribution and the engine clears it at settlement.
fn play_watched_hand(
    world: &mut World,
    watch: &mut HandAttributionWatch,
    post_flop_bet: u64,
) -> Option<HandAttribution> {
    let mut out = None;
    let feed = |world: &World, watch: &mut HandAttributionWatch, out: &mut Option<_>| {
        let snap = world.snapshot();
        let history = world.hand_history(snap.table.hand_number);
        if let Some(a) = watch.observe(&snap, world.uncredited_raw_deposits, history.as_ref()) {
            *out = Some(a);
        }
    };

    feed(world, watch, &mut out);
    world.advance(Duration::from_secs(4));
    if world.start_new_hand(world.actors[0].principal).is_err() {
        return None;
    }
    feed(world, watch, &mut out);

    for _ in 0..160 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            let _ = world.check_timeouts(world.actors[0].principal);
            feed(world, watch, &mut out);
            continue;
        };
        let post_flop = matches!(
            t.phase,
            GamePhase::Flop | GamePhase::Turn | GamePhase::River
        );
        let acted = (post_flop
            && post_flop_bet > 0
            && world
                .player_action(who, PlayerAction::Bet(post_flop_bet))
                .is_ok())
            || world.player_action(who, PlayerAction::Call).is_ok()
            || world.player_action(who, PlayerAction::Check).is_ok()
            || world.player_action(who, PlayerAction::Fold).is_ok();
        if !acted {
            world.advance(Duration::from_secs(1));
        }
        feed(world, watch, &mut out);
    }
    // Settle whatever is left and take the final observation.
    world.advance(Duration::from_secs(4));
    let _ = world.check_timeouts(world.actors[0].principal);
    feed(world, watch, &mut out);
    out
}

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

// ===========================================================================
// M8 ON ORDINARY HANDS -- the part that was missing
// ===========================================================================
//
// Everything above this line is either the FINDING 13 fixture or a synthetic
// instrument check. Between them they cover exactly the hand the invariant was
// reverse-engineered from. A pot share paid to the wrong LIVE player passes all
// three: totals conserve, the record agrees with itself, and `no free money`
// exempts anybody who staked.
//
// So these tests run the four attribution legs (see
// `invariants::attribution::check_hand_attribution`) on ordinary hands: nobody
// leaves, nobody is a stranger, the money simply has to reach the right people.

/// Seat `names`, fund them, and return the world.
fn ordinary_table(names: &[&str]) -> World {
    let world = World::new(TableConfig::six_max_icp(), names);
    for name in names {
        world
            .fund_escrow(world.actor(name), 5 * ICP)
            .expect("deposit");
    }
    for (i, name) in names.iter().enumerate() {
        world
            .join_table(world.actor(name), i as u8)
            .expect("join_table");
    }
    world.advance(Duration::from_secs(4));
    world
}

/// THE WIDENED GATE. Every settled hand says WHO was paid, by principal.
///
/// Four hands, three of them with real post-flop betting, on a four-handed table.
/// Each is checked on all four legs, and the test additionally requires that the
/// instrument SPOKE: a run in which every hand was declined would otherwise be
/// indistinguishable from a clean one.
#[test]
fn m8_every_ordinary_hand_pays_the_people_the_rules_of_poker_name() {
    let mut world = ordinary_table(&["alice", "bob", "carol", "dave"]);
    let mut watch = HandAttributionWatch::new();

    let mut measured = 0usize;
    let mut oracled = 0usize;
    for hand in 0..4u64 {
        let bet = if hand == 0 { 0 } else { 20_000_000 };
        let Some(a) = play_watched_hand(&mut world, &mut watch, bet) else {
            continue;
        };
        println!("M8 ordinary: {}", a.summary());
        if !a.is_measurable() {
            println!(
                "  DECLINED: {}",
                a.incompleteness.as_deref().unwrap_or("closed_world=false")
            );
            continue;
        }
        measured += 1;
        if a.oracle.as_ref().is_some_and(|o| o.anomalies.is_empty()) {
            oracled += 1;
        }
        for (who, delta) in &a.measured {
            println!(
                "  {who}  staked {:>10}  claimed {:>10}  moved {:>+11}  oracle {:>+11}",
                a.staked.get(who).copied().unwrap_or(0),
                a.claimed.get(who).copied().unwrap_or(0),
                delta,
                a.oracle
                    .as_ref()
                    .and_then(|o| o.net.get(who).copied())
                    .unwrap_or(0)
            );
        }
        let vs = check_hand_attribution(&a);
        assert_holds(
            &vs,
            Invariant::M8PrincipalAttribution,
            &format!("ordinary hand #{} must pay the right PEOPLE", a.hand_number),
        );
    }

    assert!(
        measured >= 3,
        "only {measured} of 4 ordinary hands could be attributed by principal. An \
         attribution gate that declines to measure is not a gate; fix the recorder in \
         tests/money_safety/src/hand_attribution.rs before believing a green run."
    );
    assert!(
        oracled >= 3,
        "only {oracled} of 4 ordinary hands were checked against the INDEPENDENT settlement \
         oracle. The other legs compare the canister against its own record, which a defect \
         that is self-consistent (a plan naming the wrong live player) passes."
    );
}

/// The same, heads-up, where every hand is a single showdown between two people
/// and any misdirection is a straight swap.
#[test]
fn m8_heads_up_hands_pay_the_right_person() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    for name in ["alice", "bob"] {
        world
            .fund_escrow(world.actor(name), 5 * ICP)
            .expect("deposit");
    }
    world.join_table(world.actor("alice"), 0).expect("seat");
    world.join_table(world.actor("bob"), 1).expect("seat");
    world.advance(Duration::from_secs(4));
    let mut world = world;

    let mut watch = HandAttributionWatch::new();
    let mut measured = 0usize;
    for _ in 0..3 {
        let Some(a) = play_watched_hand(&mut world, &mut watch, 10_000_000) else {
            continue;
        };
        println!("M8 heads-up: {}", a.summary());
        if !a.is_measurable() {
            continue;
        }
        measured += 1;
        let vs = check_hand_attribution(&a);
        assert_holds(
            &vs,
            Invariant::M8PrincipalAttribution,
            &format!("heads-up hand #{}", a.hand_number),
        );
    }
    assert!(measured >= 2, "only {measured} of 3 heads-up hands were measurable");
}

/// THE INSTRUMENT CHECK for the widened gate: each of the four legs must be able
/// to go RED, and each must convict the misdirection it exists for.
///
/// Driven on synthetic `HandAttribution` values so the claim does not depend on a
/// live engine defect; the engine-side proof is the planted-bug matrix in
/// docs/DEFECTS.md.
#[test]
fn m8_each_attribution_leg_convicts_the_misdirection_it_exists_for() {
    use money_safety::hand_attribution::{HandAttribution, OracleAnswer};

    let alice = Principal::from_slice(&[1]);
    let bob = Principal::from_slice(&[2]);
    let carol = Principal::from_slice(&[3]);

    let base = |measured: Vec<(Principal, i128)>,
                staked: Vec<(Principal, u64)>,
                claimed: Vec<(Principal, u64)>,
                relinquished: Vec<Principal>,
                oracle: Option<Vec<(Principal, i128)>>| HandAttribution {
        hand_number: 7,
        phase: GamePhase::HandComplete,
        measured: measured.into_iter().collect(),
        staked: staked.into_iter().collect(),
        claimed: claimed.into_iter().collect(),
        relinquished_only: relinquished.into_iter().collect(),
        oracle: oracle.map(|net| OracleAnswer {
            net: net.into_iter().collect(),
            anomalies: Vec::new(),
        }),
        tainted: Default::default(),
        closed_world: true,
        complete: true,
        incompleteness: None,
    };

    // A clean hand: alice and bob each staked 100, alice won the 200.
    let clean = base(
        vec![(alice, 100), (bob, -100)],
        vec![(alice, 100), (bob, 100)],
        vec![(alice, 200)],
        vec![bob],
        Some(vec![(alice, 100), (bob, -100)]),
    );
    assert!(
        check_hand_attribution(&clean).is_empty(),
        "the control hand must be silent: {:?}",
        check_hand_attribution(&clean)
    );

    // LEG 1: the record says alice won, the money went to bob. Conserves exactly.
    let wrong_wallet = base(
        vec![(alice, -100), (bob, 100)],
        vec![(alice, 100), (bob, 100)],
        vec![(alice, 200)],
        vec![bob],
        Some(vec![(alice, 100), (bob, -100)]),
    );
    let vs = check_award_reaches_the_person_it_names(&wrong_wallet);
    assert_violated(
        &vs,
        Invariant::M8PrincipalAttribution,
        "a payout credited to a wallet the record does not name",
    );
    assert_eq!(vs.len(), 2, "both sides of the misdirection: {vs:?}");

    // LEG 2: the record and the money agree with each other, and both are wrong.
    // Nothing that compares the canister against itself can see this.
    let self_consistent = base(
        vec![(alice, -100), (bob, 100)],
        vec![(alice, 100), (bob, 100)],
        vec![(bob, 200)],
        vec![],
        Some(vec![(alice, 100), (bob, -100)]),
    );
    assert!(
        check_award_reaches_the_person_it_names(&self_consistent).is_empty(),
        "by construction the canister agrees with itself here"
    );
    assert_violated(
        &check_against_the_oracle(&self_consistent),
        Invariant::M8PrincipalAttribution,
        "a self-consistent payout to the wrong live player",
    );

    // LEG 3: carol never staked and gained.
    let bystander = base(
        vec![(alice, 50), (bob, -100), (carol, 50)],
        vec![(alice, 100), (bob, 100)],
        vec![(alice, 150), (carol, 50)],
        vec![bob],
        None,
    );
    assert_violated(
        &check_no_free_money_in_hand(&bystander),
        Invariant::M8PrincipalAttribution,
        "a principal who staked nothing was paid",
    );

    // LEG 4: bob folded and came out ahead.
    let folded_profit = base(
        vec![(alice, -100), (bob, 100)],
        vec![(alice, 100), (bob, 100)],
        vec![(bob, 200)],
        vec![bob],
        None,
    );
    let vs = check_relinquished_cannot_profit(&folded_profit);
    assert_violated(
        &vs,
        Invariant::M8PrincipalAttribution,
        "a folded player cannot come out ahead",
    );
    assert_eq!(vs[0].delta_e8s, 100);

    // Every one of them must BLOCK, whatever the register says.
    for a in [&wrong_wallet, &self_consistent, &bystander, &folded_profit] {
        let vs = check_hand_attribution(a);
        assert!(!vs.is_empty(), "the composite check must fire too");
        assert!(
            vs.iter()
                .all(|v| money_safety::documented::blocking_reason(v).is_some()),
            "a misattribution must be BLOCKING: {vs:?}"
        );
    }

    // And an UNMEASURABLE hand must produce nothing at all, rather than a
    // fabricated finding from an incomplete reconstruction.
    let mut incomplete = wrong_wallet.clone();
    incomplete.complete = false;
    assert!(
        check_hand_attribution(&incomplete).is_empty(),
        "an incomplete observation must never be reported as a misattribution"
    );
}

/// A CHAIR THAT CARRIES TWO PEOPLE'S MONEY MUST REPORT BOTH OF THEM.
///
/// # Why this hand exists
///
/// `push_winner` aggregates by `(seat, principal)` rather than by seat, and the
/// comment on it says why: one chair can carry two stakes belonging to two
/// different players in a single hand (docs/DEFECTS.md E-36), and merging them
/// reports one player's money under the other player's name.
///
/// Nothing tested that. Aggregating by seat alone leaves every chip in the right
/// escrow -- `credit_escrow` has already run by the time the record is written --
/// so conservation, the settlement oracle's per-seat and per-principal diffs, and
/// the canister's own logs are all silent. The only thing wrong is the RECORD: the
/// winner list the UI shows, the history canister archives, and a player would
/// quote in a dispute.
///
/// So this hand builds the two-owner chair on purpose and asserts on the record,
/// by principal. It is also the hand where `check_award_reaches_the_person_it_names`
/// has two names to tell apart at one seat.
#[test]
fn m8_a_chair_with_two_owners_credits_each_of_them_under_their_own_name() {
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

    let mut watch = HandAttributionWatch::new();
    let feed = |world: &World, watch: &mut HandAttributionWatch| -> Option<HandAttribution> {
        let snap = world.snapshot();
        let history = world.hand_history(snap.table.hand_number);
        watch.observe(&snap, world.uncredited_raw_deposits, history.as_ref())
    };
    feed(&world, &mut watch);

    deal_to_the_flop(&world);
    feed(&world, &mut watch);
    let live = world.table_state();
    assert_eq!(live.phase, GamePhase::Flop, "the fixture needs a live flop");
    let alice_stake = live.player_at(0).expect("alice seated").total_bet_this_hand;
    assert!(alice_stake > 0);

    // alice leaves with money in the pot; dave takes her CHAIR and sits in.
    world.leave_table(alice).expect("alice leaves mid-hand");
    feed(&world, &mut watch);
    world.join_table(dave, 0).expect("dave takes seat 0");
    world.sit_in(dave).expect("dave sits in");
    feed(&world, &mut watch);

    // Drive until dave is given the action -- E-36 -- and make him bet into a hand
    // he holds no cards in. That is the second stake at seat 0.
    let mut dave_stake = 0u64;
    for _ in 0..16 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            feed(&world, &mut watch);
            continue;
        };
        if who == dave {
            let _ = world.player_action(dave, PlayerAction::Bet(4_000_000));
            feed(&world, &mut watch);
            dave_stake = world
                .table_state()
                .player_at(0)
                .map(|p| p.total_bet_this_hand)
                .unwrap_or(0);
            break;
        }
        if world.player_action(who, PlayerAction::Check).is_err() {
            let _ = world.player_action(who, PlayerAction::Call);
        }
        feed(&world, &mut watch);
    }
    assert!(
        dave_stake > 0,
        "the fixture must get a second owner's money into seat 0 (docs/DEFECTS.md E-36); \
         without it this test is not measuring the two-owner chair at all"
    );
    let mid = world.table_state();
    assert_eq!(
        mid.departed()
            .iter()
            .filter(|d| d.hand_number == mid.hand_number)
            .map(|d| (d.seat, d.principal, d.contributed))
            .collect::<Vec<_>>(),
        vec![(0u8, alice, alice_stake)],
        "alice's stake must still be recorded at seat 0 under HER name"
    );

    // Everybody still holding cards leaves, so no layer has a claimant and every
    // stake goes back to whoever put it in -- including both stakes at seat 0.
    let _ = world.leave_table(bob);
    feed(&world, &mut watch);
    let _ = world.leave_table(carol);
    let settled = feed(&world, &mut watch);
    let end = world.table_state();
    assert!(
        !end.phase.hand_in_progress(),
        "the hand must have settled once the last card-holder left, phase {:?}",
        end.phase
    );

    // --- THE ASSERTION: the record names both of them, separately -----------
    //
    // Measured against what each of them ACTUALLY had in the pot when the hand
    // settled, not against what they bet: dave's bet was uncalled, so the engine
    // handed part of it straight back mid-hand and only the rest was ever at
    // stake. `a.staked` is that figure, reconstructed from `total_bet_this_hand`
    // and `departed_stakes` and never from the payout code.
    let a = settled.expect("the watch must have produced an attribution for this hand");
    println!("two-owner chair: {}", a.summary());
    assert!(
        a.is_measurable(),
        "the two-owner hand must be measurable: {:?}",
        a.incompleteness
    );
    let alice_effective = a.staked.get(&alice).copied().unwrap_or(0);
    let dave_effective = a.staked.get(&dave).copied().unwrap_or(0);
    assert_eq!(
        alice_effective, alice_stake,
        "alice's stake is the one she left in the pot"
    );
    assert!(
        dave_effective > 0,
        "dave bet {dave_stake} into a hand he holds no cards in and must still have \
         something at stake after the uncalled part came back; staked={:?}",
        a.staked
    );

    let history = world
        .hand_history(end.hand_number)
        .expect("the settled hand must be in the history");
    let named: Vec<(u8, Principal, u64)> = history
        .winners
        .iter()
        .map(|w| (w.seat, w.principal, w.amount))
        .collect();
    println!("winner list for the two-owner chair: {named:?}");
    assert!(
        named
            .iter()
            .any(|(s, p, x)| *s == 0 && *p == alice && *x == alice_effective),
        "seat 0's {alice_effective} e8s belong to {alice} and the record must say so; it \
         says {named:?}"
    );
    assert!(
        named
            .iter()
            .any(|(s, p, x)| *s == 0 && *p == dave && *x == dave_effective),
        "the other {dave_effective} e8s at seat 0 belong to {dave} and the record must say \
         so SEPARATELY; it says {named:?}. Merging the two is one player's money published \
         under another player's name (docs/DEFECTS.md E-36, SECURITY-FINDINGS FINDING 13)."
    );
    let for_alice: u64 = named
        .iter()
        .filter(|(_, p, _)| *p == alice)
        .map(|(_, _, x)| *x)
        .sum();
    assert_eq!(
        for_alice, alice_effective,
        "the record must credit {alice} with exactly her own {alice_effective} e8s, not \
         with anybody else's: {named:?}"
    );

    // --- and the money agrees with the record, per person -------------------
    let vs = check_hand_attribution(&a);
    assert_holds(
        &vs,
        Invariant::M8PrincipalAttribution,
        "a chair with two owners must pay, and report, each of them",
    );
}
