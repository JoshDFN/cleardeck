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

    // 3. bob leaves. Leaving mid-hand relinquishes the claim, so CAROL is now the
    //    only seat still holding cards -- dave holds none -- and the hand is over
    //    the instant bob's departure is applied. Carol wins it.
    //
    //    THIS IS WHAT THE FINDING 17 FIX CHANGED, and this fixture is where it is
    //    visible in money. Before it, the cardless seat 0 kept
    //    `count_active_players` at 2, the hand ran on, carol could leave too, and
    //    the hand settled with NO live claim on it: every stake handed back to its
    //    funder, including alice's and bob's, and carol -- who had won -- paid
    //    nothing. That un-played settlement was the state this test used to assert.
    //    See docs/SECURITY-FINDINGS.md FINDING 17.
    world.leave_table(bob).expect("bob leaves mid-hand");

    let end = world.table_state();
    assert!(
        !end.phase.hand_in_progress(),
        "the hand must settle the instant only one seat still holds cards, phase is {:?}.          A cardless mid-hand arrival must not keep it alive: docs/SECURITY-FINDINGS.md          FINDING 17.",
        end.phase
    );
    assert_eq!(end.pot, 0, "a settled hand leaves no pot");

    // alice and bob both gave up their claims by leaving; carol held the only live
    // one and takes both their stakes. dave took a chair and staked nothing, so he
    // is owed nothing -- and THAT is the FINDING 13 assertion, now made while real
    // money is moving instead of while everything is flat.
    let staked: BTreeSet<Principal> = [alice, bob, carol].into_iter().collect();
    let expected: BTreeMap<Principal, i128> = [
        (alice, -(stake as i128)),
        (bob, -(stake as i128)),
        (carol, 2 * stake as i128),
        (dave, 0i128),
    ]
    .into_iter()
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

    // And the full statement: every e8 to the person the rules of poker name.
    let vs = check_principal_attribution(&before, &after, &expected, &GamePhase::HandComplete);
    assert_holds(
        &vs,
        Invariant::M8PrincipalAttribution,
        "every stake reaches the principal the rules of poker say it belongs to",
    );

    // Stated again as bare numbers, so a reader of the failure sees the two sides
    // of the same e8 rather than a violation list.
    let d = value_deltas(&before, &after);
    let carol = world.actor("carol");
    assert_eq!(
        d.get(&alice).copied().unwrap_or(0),
        -(stake as i128),
        "alice left the hand with {stake} in the pot: she gave up her claim on it, so she \
         forfeits it to the seat that still held one"
    );
    assert_eq!(
        d.get(&carol).copied().unwrap_or(0),
        2 * stake as i128,
        "carol held the ONLY live claim when bob left, so she wins both forfeited stakes. \
         If she is level here, the hand was un-played and every stake went back to its \
         funder: docs/SECURITY-FINDINGS.md FINDING 17."
    );
    assert_eq!(
        d.get(&dave).copied().unwrap_or(0),
        0,
        "dave took alice's chair and staked nothing. Any gain here is somebody else's money \
         in his hands: docs/SECURITY-FINDINGS.md FINDING 13."
    );

    // Where the money physically did NOT go: dave's stack, which is untouched.
    let end = world.table_state();
    assert_eq!(
        end.player_at(0).map(|p| p.chips),
        Some(world.config.min_buy_in),
        "dave's stack must still be exactly his buy-in: he staked nothing and won nothing"
    );
    assert!(
        end.player_at(0).map(|p| p.principal) == Some(dave),
        "seat 0 is still dave's chair"
    );

    // And the RECORD names carol, at HER seat, for the whole pot -- and does not
    // name dave at all. A record that credited seat 0 would be crediting the chair
    // rather than the person: docs/SECURITY-FINDINGS.md FINDING 13.
    let history = world
        .hand_history(end.hand_number)
        .expect("the settled hand must be in the history");
    let named: Vec<(u8, Principal, u64)> = history
        .winners
        .iter()
        .map(|w| (w.seat, w.principal, w.amount))
        .collect();
    assert!(
        named
            .iter()
            .any(|(s, p, a)| *s == 2 && *p == carol && *a == 3 * stake),
        "the recorded winner list must credit the whole {} pot to {carol} at seat 2; it says \
         {named:?}",
        3 * stake
    );
    assert!(
        !named.iter().any(|(_, p, _)| *p == dave),
        "the recorded winner list must not name {dave} at all; it says {named:?}"
    );
    assert!(
        !named.iter().any(|(_, p, _)| *p == alice),
        "alice gave up her claim by leaving; the record must not credit her anything. It says \
         {named:?}"
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

/// A CHAIR THAT CARRIES TWO PEOPLE'S MONEY IS NOW UNREACHABLE, AND THIS IS THE
/// TEST THAT SAYS SO.
///
/// # What this test used to be, and why it changed
///
/// It used to BUILD the two-owner chair on purpose. alice leaves a live hand with
/// money in the pot, dave takes her chair and calls `sit_in()`, and -- because
/// `find_next_active_seat` asked only for `status == Active` -- dave was then
/// given the action and could bet into a hand he had never been dealt into. That
/// put a second owner's money at seat 0, which is the first half of
/// docs/DEFECTS.md E-36, and the test asserted that `push_winner` reported both
/// stakes under their own names rather than merging them.
///
/// **E-36 is fixed** (docs/SECURITY-FINDINGS.md FINDING 17): `can_still_act`
/// requires cards, so a mid-hand arrival is never offered the action and can never
/// put a chip into a hand it holds none in. The premise is gone. Rather than delete
/// the hand -- which would leave nothing asserting the fix from this direction --
/// it now drives the identical sequence and asserts that **dave never gets the
/// action, is refused every action he sends, and never ends up with a stake.**
///
/// `push_winner`'s aggregation by `(seat, principal)` stays as it is. It is now
/// defence in depth rather than a live requirement, and the reason to keep it is
/// that the merge it prevents publishes one player's money under another player's
/// name -- silently, because `credit_escrow` has already run by the time the record
/// is written, so conservation and the oracle are both blind to it.
#[test]
fn a_mid_hand_arrival_is_never_dealt_the_action_and_can_never_stake_the_hand() {
    let mut world = World::new(
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

    deal_to_the_flop(&world);
    let live = world.table_state();
    assert_eq!(live.phase, GamePhase::Flop, "the fixture needs a live flop");
    let alice_stake = live.player_at(0).expect("alice seated").total_bet_this_hand;
    assert!(alice_stake > 0);

    // alice leaves with money in the pot; dave takes her CHAIR and sits in. Both
    // calls still return Ok -- taking an empty chair is legal, and this test is
    // not about refusing it.
    world.leave_table(alice).expect("alice leaves mid-hand");
    world.join_table(dave, 0).expect("dave takes seat 0");
    world.sit_in(dave).expect("dave sits in");

    let retaken = world.table_state();
    assert_eq!(retaken.player_at(0).map(|p| p.principal), Some(dave));
    assert_eq!(
        retaken.player_at(0).and_then(|p| p.hole_cards),
        None,
        "a mid-hand arrival holds no cards"
    );

    // Drive the hand and watch the clock. dave must never be on it, and every
    // action he sends must be refused.
    let mut dave_was_on_the_clock = false;
    let mut dave_refusals = 0usize;
    for _ in 0..16 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        if t.player_at(t.action_on).map(|p| p.principal) == Some(dave) {
            dave_was_on_the_clock = true;
        }
        match world.player_action(dave, PlayerAction::Bet(4_000_000)) {
            Err(_) => dave_refusals += 1,
            Ok(()) => panic!(
                "docs/DEFECTS.md E-36 HAS RETURNED: dave holds no cards in this hand and the \
                 engine accepted a bet from him. He can lose money he cannot win, and his \
                 stake makes seat 0 carry two owners' money in one hand."
            ),
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            continue;
        };
        if world.player_action(who, PlayerAction::Check).is_err() {
            let _ = world.player_action(who, PlayerAction::Call);
        }
    }

    assert!(
        !dave_was_on_the_clock,
        "docs/DEFECTS.md E-36 HAS RETURNED: seat 0 holds no cards and was given the action. \
         A seat on the clock can be FOLDED by that clock, which is how FINDING 17 reached a \
         settlement with no live claim on it."
    );
    assert!(
        dave_refusals > 0,
        "the fixture must actually have tried to make dave act; it never got the chance"
    );

    // No second stake at seat 0, ever.
    let mid = world.table_state();
    assert_eq!(
        mid.player_at(0).map(|p| p.total_bet_this_hand),
        Some(0),
        "dave must have nothing in this hand"
    );

    // And the engine never reported the two-owner state. That `WARNING:` is no
    // longer on `TOLERATED_SELF_REPORTS`, so if it were emitted the money-safety
    // classifier would BLOCK on it rather than count it.
    let logs = world.new_canister_logs().join("\n");
    assert!(
        !logs.contains("carries both a live stake and a departed stake"),
        "the engine reported a chair carrying two owners' stakes, which E-36's fix is \
         supposed to have made unreachable:\n{logs}"
    );

    // Settle the hand however it ends, and check dave got nothing out of it.
    let before_settle = values_now(&world);
    let _ = world.leave_table(bob);
    world.advance(Duration::from_secs(4));
    let _ = world.check_timeouts(carol);
    let after = values_now(&world);
    let end = world.table_state();
    assert!(
        !end.phase.hand_in_progress(),
        "the hand must have settled, phase {:?}",
        end.phase
    );
    assert_eq!(
        value_deltas(&before_settle, &after)
            .get(&dave)
            .copied()
            .unwrap_or(0),
        0,
        "dave staked nothing and must be paid nothing: docs/SECURITY-FINDINGS.md FINDING 13"
    );

    let history = world
        .hand_history(end.hand_number)
        .expect("the settled hand must be in the history");
    let named: Vec<(u8, Principal, u64)> = history
        .winners
        .iter()
        .map(|w| (w.seat, w.principal, w.amount))
        .collect();
    println!("winner list with a re-occupied chair: {named:?}");
    assert!(
        !named.iter().any(|(_, p, _)| *p == dave),
        "the record must not name {dave}; it says {named:?}"
    );
}
