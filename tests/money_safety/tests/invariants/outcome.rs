//! M11 OUTCOME — the gate for docs/SECURITY-FINDINGS.md FINDING 17, and the proof
//! that it can go RED.
//!
//! Three defects in this project have had the same shape: correct totals, wrong
//! recipients, every invariant quiet. The instrument that convicts that class has
//! to be shown failing on it, or it is decoration — the exact H-03 lesson, applied
//! to the newest invariant.
//!
//! Two kinds of test here:
//!
//!   * the SYNTHETIC ones drive [`OutcomeWatch`] and [`check_hand_outcome`] with
//!     hand-built inputs and require a violation. They need no replica and they
//!     cannot be quieted by fixing the engine, which is what makes them the
//!     instrument's own gate.
//!   * [`m11_is_quiet_on_the_finding_17_sequence`] drives the REAL canister
//!     through the exact sequence FINDING 17 was measured on and requires SILENCE.
//!     Restore the `|| p.status == PlayerStatus::Active` disjunct on `is_in_hand`
//!     and it goes red with
//!     `a_live_hand_may_not_run_on_with_one_claimant`.

use candid::Principal;
use money_safety::hand_attribution::{HandAttribution, HandAttributionWatch};
use money_safety::invariants::outcome::*;
use money_safety::invariants::{Invariant, Severity};
use money_safety::table_api::*;
use money_safety::world::*;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const ICP: u64 = 100_000_000;

fn on_clock(t: &TableState) -> Option<Principal> {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
}

// ---------------------------------------------------------------------------
// THE INSTRUMENT MUST BE ABLE TO FAIL
// ---------------------------------------------------------------------------

/// M11a. A live hand with money in it and exactly ONE seat holding a claim.
///
/// This is the state FINDING 17 left the table in for as long as it took the last
/// card-holder's clock to expire: alice folded, bob had won, and the hand kept
/// running because a cardless seat was propping up `count_active_players`.
#[test]
fn m11_convicts_a_live_hand_that_has_only_one_claimant() {
    let mut watch = OutcomeWatch::new();
    let snap = synthetic::live_hand_with(vec![
        // seat 0: folded, 2,000,000 in the pot
        synthetic::seat(0, true, true, 2_000_000),
        // seat 1: HOLDS THE ONLY CLAIM
        synthetic::seat(1, false, true, 2_000_000),
        // seat 2: the mid-hand arrival, Active, NO CARDS, nothing staked
        synthetic::seat(2, false, false, 0),
    ]);

    let (vs, _) = watch.observe(&snap);
    assert_eq!(
        vs.len(),
        1,
        "a live hand with one claimant must be a violation, got {vs:?}"
    );
    assert_eq!(vs[0].invariant, Invariant::M11Outcome);
    assert_eq!(vs[0].check, "a_live_hand_may_not_run_on_with_one_claimant");
    assert_eq!(vs[0].severity, Severity::WrongOutcome);
    assert!(
        money_safety::documented::blocking_reason(&vs[0]).is_some(),
        "and it must BLOCK a run: there is no magnitude at which the wrong player wins"
    );
}

/// M11a, the worse half: a live hand nobody can win.
#[test]
fn m11_convicts_a_live_hand_with_no_claimant_at_all() {
    let mut watch = OutcomeWatch::new();
    let snap = synthetic::live_hand_with(vec![
        synthetic::seat(0, true, true, 2_000_000),
        synthetic::seat(1, true, true, 2_000_000),
        synthetic::seat(2, false, false, 0),
    ]);
    let (vs, _) = watch.observe(&snap);
    assert_eq!(vs.len(), 1, "got {vs:?}");
    assert_eq!(
        vs[0].check,
        "a_live_hand_may_not_run_on_with_no_claimant_at_all"
    );
    assert!(money_safety::documented::blocking_reason(&vs[0]).is_some());
}

/// M11a must be QUIET on an ordinary live hand. A gate that fires on everything
/// is the same as one that fires on nothing.
#[test]
fn m11_is_quiet_on_an_ordinary_live_hand() {
    let mut watch = OutcomeWatch::new();
    let snap = synthetic::live_hand_with(vec![
        synthetic::seat(0, false, true, 2_000_000),
        synthetic::seat(1, false, true, 2_000_000),
        synthetic::seat(2, true, true, 2_000_000),
    ]);
    let (vs, _) = watch.observe(&snap);
    assert!(vs.is_empty(), "an ordinary three-handed pot must be silent: {vs:?}");
}

/// M11b. The seat that held the last claim must be the seat that is paid.
///
/// The numbers are FINDING 17's, scaled: a 4,000,000 pot with two funders, one of
/// whom folded. The winner is owed +2,000,000 and the measurement says they came
/// out level, which is exactly the refund-everyone settlement.
#[test]
fn m11_convicts_a_foldout_winner_who_was_paid_nothing() {
    let winner = Principal::from_slice(&[1]);
    let folder = Principal::from_slice(&[2]);

    let mut watch = OutcomeWatch::new();
    // Live, one claimant: seat 1 (the winner).
    let live = synthetic::live_hand_for(vec![
        (0u8, folder, true, true, 2_000_000u64),
        (1u8, winner, false, true, 2_000_000),
    ]);
    let (_, none) = watch.observe(&live);
    assert!(none.is_none(), "the hand has not ended yet");

    // Now idle: the hand settled.
    let idle = synthetic::idle_after(&live);
    let (_, finished) = watch.observe(&idle);
    let o = finished.expect("the watch must produce an outcome for the finished hand");
    let sc = o
        .sole_claimant
        .as_ref()
        .expect("the fold-out moment must have been recorded");
    assert_eq!(sc.who, winner);
    assert_eq!(sc.owed, 2_000_000, "the winner is owed the folder's stake");

    // The measurement: everybody level. That is the defect.
    let a = synthetic::attribution(
        &o,
        vec![(winner, 0i128), (folder, 0)],
        vec![(winner, 2_000_000u64), (folder, 2_000_000)],
        vec![(winner, 2_000_000u64), (folder, 2_000_000)],
        vec![folder],
    );
    let mut coverage = OutcomeCoverage::default();
    let vs = check_hand_outcome(&o, &a, &mut coverage);
    assert!(
        vs.iter().any(|v| v.check
            == "the_seat_that_held_the_last_claim_is_the_seat_that_is_paid"),
        "the fold-out winner was paid nothing and M11b must say so: {vs:?}"
    );
    assert!(
        vs.iter().any(|v| v.check
            == "a_refund_everyone_settlement_needs_nobody_to_have_held_a_claim"),
        "and M11c must say the folder got his money back while somebody held a claim: {vs:?}"
    );
    for v in &vs {
        assert_eq!(v.severity, Severity::WrongOutcome);
        assert!(
            money_safety::documented::blocking_reason(v).is_some(),
            "every outcome violation must BLOCK: {v:?}"
        );
    }
    assert_eq!(coverage.hands_outcome_checked, 1);
    assert_eq!(coverage.hands_foldout_checked, 1);
}

/// M11b and M11c must be QUIET when the fold-out winner IS paid.
#[test]
fn m11_is_quiet_when_the_foldout_winner_is_paid() {
    let winner = Principal::from_slice(&[1]);
    let folder = Principal::from_slice(&[2]);
    let mut watch = OutcomeWatch::new();
    let live = synthetic::live_hand_for(vec![
        (0u8, folder, true, true, 2_000_000u64),
        (1u8, winner, false, true, 2_000_000),
    ]);
    watch.observe(&live);
    let idle = synthetic::idle_after(&live);
    let (_, finished) = watch.observe(&idle);
    let o = finished.expect("outcome");

    let a = synthetic::attribution(
        &o,
        vec![(winner, 2_000_000i128), (folder, -2_000_000)],
        vec![(winner, 2_000_000u64), (folder, 2_000_000)],
        vec![(winner, 4_000_000u64)],
        vec![folder],
    );
    let mut coverage = OutcomeCoverage::default();
    let vs = check_hand_outcome(&o, &a, &mut coverage);
    assert!(vs.is_empty(), "the right outcome must be silent: {vs:?}");
    assert_eq!(coverage.hands_foldout_checked, 1);
}

/// M11c must not fire on an exact CHOP, where every funder is also level.
///
/// This is the false positive the leg is written to avoid: two players contribute
/// the same, tie at showdown, and each gets their own money back. Nobody
/// relinquished anything, so nothing was taken from anybody.
#[test]
fn m11_does_not_mistake_an_exact_chop_for_a_refund() {
    let a1 = Principal::from_slice(&[1]);
    let a2 = Principal::from_slice(&[2]);
    let mut watch = OutcomeWatch::new();
    let live = synthetic::live_hand_for(vec![
        (0u8, a1, false, true, 2_000_000u64),
        (1u8, a2, false, true, 2_000_000),
    ]);
    watch.observe(&live);
    let idle = synthetic::idle_after(&live);
    let (_, finished) = watch.observe(&idle);
    let o = finished.expect("outcome");
    assert!(
        o.sole_claimant.is_none(),
        "two claimants: the hand was never decided by fold-out"
    );

    let a = synthetic::attribution(
        &o,
        vec![(a1, 0i128), (a2, 0)],
        vec![(a1, 2_000_000u64), (a2, 2_000_000)],
        vec![(a1, 2_000_000u64), (a2, 2_000_000)],
        vec![], // NOBODY relinquished
    );
    let mut coverage = OutcomeCoverage::default();
    let vs = check_hand_outcome(&o, &a, &mut coverage);
    assert!(
        vs.is_empty(),
        "a chop leaves every funder level and is not a refund: {vs:?}"
    );
}

// ---------------------------------------------------------------------------
// AGAINST THE REAL CANISTER
// ---------------------------------------------------------------------------

/// **THE GATE.** The FINDING 17 sequence, driven against the real module, with
/// M11 watching every step. It must be silent.
///
/// Restore the `|| p.status == PlayerStatus::Active` disjunct on `is_in_hand` and
/// this goes red at `a_live_hand_may_not_run_on_with_one_claimant`, on the step
/// after alice folds — which is the moment the hand was decided and did not end.
#[test]
fn m11_is_quiet_on_the_finding_17_sequence() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for who in [alice, bob, carol] {
        world.fund_escrow(who, 20 * ICP).expect("deposit");
    }
    world.join_table(alice, 0).expect("seat alice");
    world.join_table(bob, 1).expect("seat bob");
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    let mut watch = OutcomeWatch::new();
    let mut attribution = HandAttributionWatch::new();
    let mut violations = Vec::new();
    let mut steps = 0usize;
    let mut observe = |world: &World,
                       watch: &mut OutcomeWatch,
                       attribution: &mut HandAttributionWatch,
                       violations: &mut Vec<money_safety::invariants::Violation>,
                       steps: &mut usize| {
        let snap = world.snapshot();
        let history = world.hand_history(snap.table.hand_number);
        let attributed = attribution.observe(&snap, world.uncredited_raw_deposits, history.as_ref());
        let (structural, finished) = watch.observe(&snap);
        violations.extend(structural);
        if let (Some(o), Some(a)) = (finished.as_ref(), attributed.as_ref()) {
            let mut cov = watch.coverage.clone();
            violations.extend(check_hand_outcome(o, a, &mut cov));
            watch.coverage = cov;
        }
        *steps += 1;
    };

    observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);

    // Build a real pot.
    for _ in 0..4 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(w) = on_clock(&t) else { break };
        if world.player_action(w, PlayerAction::Raise(ICP / 2)).is_err() {
            let _ = world.player_action(w, PlayerAction::Call);
        }
        observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);
        if world.pot() >= ICP / 2 {
            break;
        }
    }

    // The mid-hand arrival.
    world.join_table(carol, 2).expect("carol takes an empty chair");
    world.sit_in(carol).expect("carol sits in");
    observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);

    // One fold. The other card-holder has won.
    let t = world.table_state();
    let folder = on_clock(&t).expect("somebody on the clock");
    world.player_action(folder, PlayerAction::Fold).expect("fold");
    observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);

    // Nobody answers again; every clock expires.
    for _ in 0..8 {
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
        world.advance(Duration::from_secs(120));
        let _ = world.check_timeouts(alice);
        observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);
    }
    observe(&world, &mut watch, &mut attribution, &mut violations, &mut steps);

    println!(
        "M11 on the FINDING 17 sequence: {steps} observation(s), structural leg ran on {} of \
         them, {} hand(s) had their outcome checked, declined {:?}",
        watch.coverage.steps_structurally_checked,
        watch.coverage.hands_outcome_checked,
        watch.coverage.declined
    );

    assert!(
        watch.coverage.steps_structurally_checked > 0,
        "the instrument must have looked at something"
    );
    assert!(
        violations.is_empty(),
        "M11 OUTCOME fired on the FINDING 17 sequence, which means the defect is back:\n  {}",
        violations
            .iter()
            .map(|v| format!("{}: {}", v.check, v.detail))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// synthetic snapshots
// ---------------------------------------------------------------------------

mod synthetic {
    use super::*;

    pub fn seat(n: u8, folded: bool, cards: bool, staked: u64) -> Player {
        seat_for(n, Principal::from_slice(&[0xC0, 0xFF, 0xEE, n]), folded, cards, staked)
    }

    pub fn seat_for(
        n: u8,
        who: Principal,
        folded: bool,
        cards: bool,
        staked: u64,
    ) -> Player {
        Player {
            principal: who,
            seat: n,
            chips: 500_000_000,
            hole_cards: if cards {
                Some((
                    Card { rank: Rank::Two, suit: Suit::Clubs },
                    Card { rank: Rank::Three, suit: Suit::Diamonds },
                ))
            } else {
                None
            },
            current_bet: 0,
            total_bet_this_hand: staked,
            has_folded: folded,
            has_acted_this_round: true,
            is_all_in: false,
            status: PlayerStatus::Active,
            last_seen: 0,
            timeout_count: 0,
            time_bank_remaining: 30,
            is_sitting_out_next_hand: false,
            broke_at: None,
            sitting_out_since: None,
        }
    }

    pub fn live_hand_with(players: Vec<Player>) -> Snapshot {
        let pot = players
            .iter()
            .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand));
        let mut seats: Vec<Option<Player>> = players.into_iter().map(Some).collect();
        while seats.len() < 6 {
            seats.push(None);
        }
        Snapshot {
            ledger_main: 0,
            ledger_deposit_subaccounts: 0,
            actor_wallets: BTreeMap::new(),
            escrow: BTreeMap::new(),
            escrow_total: 0,
            chips_total: 0,
            canister_unswept_deposits: 0,
            canister_unaudited_deposit_accounts: 0,
            // A synthetic snapshot for the OUTCOME legs, which are about who won a
            // hand and never about custody. `None` here means "this fixture makes
            // no claim about solvency"; the solvency legs are driven from real
            // worlds in `invariants/solvency.rs` and `tests/solvency.rs`.
            canister_solvency: None,
            ledger_deposit_by_principal: BTreeMap::new(),
            canister_deposit_by_principal: BTreeMap::new(),
            table: TableState {
                id: 1,
                config: TableConfig::six_max_icp(),
                players: seats,
                community_cards: Vec::new(),
                deck: Vec::new(),
                deck_index: 0,
                pot,
                side_pots: Vec::new(),
                current_bet: 0,
                min_raise: 2_000_000,
                phase: GamePhase::Flop,
                dealer_seat: 0,
                small_blind_seat: 0,
                big_blind_seat: 1,
                action_on: 0,
                action_timer: None,
                shuffle_proof: None,
                hand_number: 1,
                last_aggressor: None,
                bb_has_option: false,
                first_hand: false,
                auto_deal_at: None,
                departed_stakes: None,
            },
        }
    }

    pub fn live_hand_for(rows: Vec<(u8, Principal, bool, bool, u64)>) -> Snapshot {
        live_hand_with(
            rows.into_iter()
                .map(|(n, who, folded, cards, staked)| seat_for(n, who, folded, cards, staked))
                .collect(),
        )
    }

    /// The same table after settlement: idle, empty pot, same hand number.
    pub fn idle_after(live: &Snapshot) -> Snapshot {
        let mut out = live.clone();
        out.table.phase = GamePhase::HandComplete;
        out.table.pot = 0;
        out
    }

    /// A `HandAttribution` with the numbers a caller wants to state.
    pub fn attribution(
        o: &HandOutcome,
        measured: Vec<(Principal, i128)>,
        staked: Vec<(Principal, u64)>,
        claimed: Vec<(Principal, u64)>,
        relinquished: Vec<Principal>,
    ) -> HandAttribution {
        HandAttribution {
            hand_number: o.hand_number,
            phase: GamePhase::HandComplete,
            measured: measured.into_iter().collect(),
            staked: staked.into_iter().collect(),
            claimed: claimed.into_iter().collect(),
            relinquished_only: relinquished.into_iter().collect::<BTreeSet<_>>(),
            oracle: None,
            tainted: BTreeSet::new(),
            closed_world: true,
            complete: true,
            incompleteness: None,
        }
    }
}
