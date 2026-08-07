//! THE OLDEST OPEN CLUSTER IN THE REGISTER, RE-ASKED ON THE CURRENT TREE.
//!
//! `docs/SECURITY-FINDINGS.md` FINDING 02, 05, 08, 09, 17 and 22 were all filed
//! against the payout path as it stood before the wave-2 rewrite. Five of the six
//! were closed by that rewrite and by waves 7 and 8, and their HEADERS were never
//! updated, so a reader scanning the register saw six open fund defects that are
//! not open. This file is the evidence, executed against the REAL table canister
//! wasm built from this tree, that turns each of those closures from a claim in a
//! blockquote into something that goes red when it stops being true.
//!
//! One test per finding, named for it. Every one of them is a GATE, not a
//! reproducer: it asserts the property the fix established, so re-introducing the
//! defect turns it red.
//!
//! Run it with:
//! ```text
//!   cd tests/money_safety && cargo test --test oldest_cluster -- --test-threads=1 --nocapture
//! ```

use candid::{Encode, Principal};
use money_safety::invariants::{self, *};
use money_safety::table_api::*;
use money_safety::world::*;
use std::collections::BTreeMap;
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Who the clock is on.
fn on_clock(t: &TableState) -> Principal {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
        .expect("a live hand always has somebody on the clock")
}

/// Escrow + seated chips, per principal: everything the canister owes each player
/// that is not still in the middle of the hand.
fn values(world: &World) -> BTreeMap<Principal, u64> {
    let snap = world.snapshot();
    let mut out: BTreeMap<Principal, u64> = snap.escrow.clone();
    for p in snap.table.players.iter().flatten() {
        *out.entry(p.principal).or_insert(0) += p.chips;
    }
    out
}

fn seated(names: &[&str], config: TableConfig, buy_in: u64) -> World {
    seated_with_bystanders(names, &[], config, buy_in)
}

/// Seat `names`; create and fund `bystanders` without seating them, so a test can
/// have somebody take a chair mid-hand.
fn seated_with_bystanders(
    names: &[&str],
    bystanders: &[&str],
    config: TableConfig,
    buy_in: u64,
) -> World {
    let all: Vec<&str> = names.iter().chain(bystanders.iter()).copied().collect();
    let world = World::new(config, &all);
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world.fund_escrow(who, buy_in).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    for name in bystanders {
        let who = world.actor(name);
        world.fund_escrow(who, buy_in).expect("deposit");
    }
    world
}

/// Put real money in the pot pre-flop by levelling the bets.
fn level_the_bets(world: &World) {
    for _ in 0..12 {
        let t = world.table_state();
        if t.phase != GamePhase::PreFlop {
            return;
        }
        let who = on_clock(&t);
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        return;
    }
}

// ===========================================================================
// FINDING 02 / FINDING 09 -- `state.pot` is the sole authority for side-pot
// totals, in both directions, and the capping direction is reached in play
// ===========================================================================

/// **REFUTED, STRUCTURALLY.** The two routines FINDING 02 is about --
/// `poker_core::side_pots::build_side_pots_logged` and `apply_side_pots`, which
/// reconcile against `state.pot` and let it win unconditionally -- have NO caller
/// left in the canister. They survive only as the archive of what the defect was,
/// pinned by ~1,500 golden vectors.
///
/// This is the leg no runtime test can cover, because a runtime test can only show
/// that the mint/destroy did not happen on the hands it played. It asserts the
/// stronger thing: there is no edge from the canister to the code that can do it.
///
/// Goes red the moment somebody puts either routine back on any path the canister
/// can reach -- which is exactly how FINDING 02 got there in the first place.
#[test]
fn finding02_the_state_pot_authority_has_no_caller_in_the_canister() {
    let src = std::fs::read_to_string("../../src/table_canister/src/lib.rs")
        .expect("src/table_canister/src/lib.rs must be readable from tests/money_safety");

    // Strip comments and doc comments: this file discusses both routines by name
    // at length, and a substring match that counted prose would be a gate that can
    // never go green.
    let code: String = src
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");

    for banned in [
        "build_side_pots_logged",
        "apply_side_pots",
        "build_side_pots(",
    ] {
        assert!(
            !code.contains(banned),
            "FINDING 02 HAS RETURNED: `{banned}` is called from \
             src/table_canister/src/lib.rs. That routine reconciles the side-pot ladder \
             against `state.pot` and lets `state.pot` win unconditionally, which MINTS chips \
             when it is too high and DESTROYS them when it is too low. The payout basis must \
             be built from the players' contributions \
             (`poker_core::build_side_pots_from_contributions`, which has no `total_pot` \
             parameter to be overridden by). See docs/SECURITY-FINDINGS.md FINDING 02."
        );
    }

    // And the positive half: the builder that CANNOT mint is the one the canister
    // uses, in both places a ladder is built (the displayed breakdown and the
    // payout plan).
    let uses = code.matches("build_side_pots_from_contributions").count();
    assert!(
        uses >= 2,
        "the contribution-basis builder is called {uses} time(s); it must build both the \
         displayed breakdown (`refresh_side_pots`) and the payout plan (`plan_payouts`)"
    );
}

/// **REFUTED, IN PLAY.** FINDING 09 is FINDING 02's destroying direction being
/// reached by ordinary randomised play, evidenced by the canister's own log line
/// `BUG: Side pots (400000000) exceed total pot (0). Capping to pot amount.`
///
/// The canister can no longer emit that line -- the routine that wrote it is off
/// the payout path -- and `state.pot` is now a redundant accumulator that is
/// CROSS-CHECKED rather than obeyed: a disagreement is reported as
/// `CRITICAL: pot accounting disagreement`, which `TOLERATED_SELF_REPORTS` (empty)
/// makes a run-stopping violation.
///
/// This drives multi-street hands with all-ins and asserts BOTH: no `BUG:` line,
/// no `CRITICAL:` line, and -- the property those lines were a proxy for --
/// `state.pot` equals the whole payout basis at every observation.
#[test]
fn finding09_the_engine_never_reports_a_pot_it_settled_against_anyway() {
    let mut world = seated(
        &["alice", "bob", "carol"],
        TableConfig::six_max_icp(),
        20 * ICP,
    );
    let _ = world.new_canister_logs();

    let mut hands = 0;
    let mut observations = 0;
    for _ in 0..4 {
        world.advance(Duration::from_secs(4));
        if world.start_new_hand(world.actor("alice")).is_err() {
            break;
        }
        hands += 1;
        for _ in 0..40 {
            let t = world.table_state();
            if !t.phase.hand_in_progress() {
                break;
            }
            // THE CROSS-CHECK, at every step: the redundant accumulator against
            // the basis money is actually settled from.
            assert_eq!(
                t.pot,
                t.payout_basis_total(),
                "hand {}: state.pot = {} but the payout basis (seated stakes + departed \
                 stakes) sums to {}. FINDING 02 is that the FIRST number used to win that \
                 argument, minting chips when it was high and destroying them when it was \
                 low.",
                t.hand_number,
                t.pot,
                t.payout_basis_total()
            );
            observations += 1;
            let who = on_clock(&t);
            // One raise per street, then everybody calls: multi-street hands with
            // real money going in after the flop, which is where the two accounts
            // of the pot had the most room to drift.
            if t.current_bet == 0 && world.player_action(who, PlayerAction::Bet(ICP)).is_ok() {
                continue;
            }
            if world.player_action(who, PlayerAction::Call).is_ok() {
                continue;
            }
            if world.player_action(who, PlayerAction::Check).is_ok() {
                continue;
            }
            break;
        }
    }
    assert!(hands > 0, "no hand was dealt, so nothing was measured");
    assert!(
        observations > 10,
        "only {observations} mid-hand observations; this test is not measuring a real hand"
    );

    let logs = world.new_canister_logs();
    let self_reports: Vec<&String> = logs
        .iter()
        .filter(|l| l.contains("BUG:") || l.contains("CRITICAL:"))
        .collect();
    assert!(
        self_reports.is_empty(),
        "FINDING 09: the canister reported its own accounting inconsistency and settled \
         anyway. Lines: {self_reports:#?}"
    );
    println!(
        "FINDING 09 refuted: {hands} hands, {observations} mid-hand cross-checks, \
         0 BUG:/CRITICAL: lines out of {} log lines",
        logs.len()
    );
}

// ===========================================================================
// FINDING 05 / FINDING 08 -- the two doors out of an occupied seat mid-hand
// ===========================================================================

/// **REFUTED.** Both doors -- `leave_table` and the FINDING 08 one, a player
/// folded by their own action clock and then cashing out -- leave the leaver's
/// stake in the payout basis, so the short all-in's main pot does not shrink and
/// the deepest stack's pot does not grow.
///
/// The measurement is the one from the finding: the per-LAYER ladder before and
/// after the seat is vacated. FINDING 05/08 was a redistribution, not a leak, so
/// every total is identical either way and only the ladder can see it.
#[test]
fn finding05_and_08_neither_door_out_of_a_seat_moves_a_chip_between_pot_layers() {
    let mut doors_exercised = 0;

    for door in ["leave_table", "timeout_then_cash_out"] {
        // action_timeout SHORTER than the 30 s disconnect timeout, so one seat can
        // be folded by its own clock without the whole table being marked away.
        let config = TableConfig {
            action_timeout_secs: 10,
            ..TableConfig::six_max_icp()
        };
        let world = World::new(config, &["alice", "bob", "carol", "dave"]);
        // ONE SHORT STACK AND THREE DEEP ONES, exact. A single-layer pot cannot
        // show this defect: FINDING 05/08 moves money BETWEEN layers, so the
        // measurement needs a ladder to move it along.
        for (i, (name, stack)) in [
            ("alice", 2 * ICP),
            ("bob", 10 * ICP),
            ("carol", 10 * ICP),
            ("dave", 10 * ICP),
        ]
        .into_iter()
        .enumerate()
        {
            let who = world.actor(name);
            world.fund_escrow(who, 12 * ICP).expect("deposit");
            world.buy_in(who, i as u8, stack).expect("exact stack");
        }
        world.advance(Duration::from_secs(4));
        world.start_new_hand(world.actor("alice")).expect("deal");

        // Pre-flop: the short stack shoves, the deep seats go to 5 ICP. That is a
        // two-layer ladder -- a main pot every seat can win and a side pot only the
        // deep seats can.
        let mut raised = false;
        for _ in 0..16 {
            let t = world.table_state();
            if t.phase != GamePhase::PreFlop {
                break;
            }
            let who = on_clock(&t);
            if t.action_on == 0 {
                if world.player_action(who, PlayerAction::AllIn).is_ok() {
                    continue;
                }
            }
            if !raised && world.player_action(who, PlayerAction::Raise(5 * ICP)).is_ok() {
                raised = true;
                continue;
            }
            if world.player_action(who, PlayerAction::Call).is_ok() {
                continue;
            }
            if world.player_action(who, PlayerAction::Check).is_ok() {
                continue;
            }
            break;
        }

        let mid = world.table_state();
        assert!(mid.phase.hand_in_progress(), "{door}: must still be in a hand");
        let amounts_before: Vec<u64> = mid.side_pots.iter().map(|p| p.amount).collect();
        assert!(
            amounts_before.len() >= 2,
            "{door}: the ladder must have at least two layers or this test cannot see the \
             defect. side_pots={:?} bets={:?}",
            mid.side_pots,
            mid.seated()
                .map(|p| (p.seat, p.total_bet_this_hand))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            mid.pot,
            mid.payout_basis_total(),
            "{door}: precondition -- every e8 in the pot is attributed before anybody leaves"
        );

        let leaver = match door {
            "leave_table" => {
                // A DEEP seat that is not on the clock, so this is purely a seat
                // change and the ladder it is leaving behind has two layers.
                let who = mid
                    .seated()
                    .find(|p| p.seat != mid.action_on && p.seat != 0 && p.total_bet_this_hand > 0)
                    .map(|p| p.principal)
                    .expect("a deep seat off the clock has money in");
                if let Err(e) = world.leave_table(who) {
                    println!("{door}: leave_table refused ({e:?})");
                    continue;
                }
                who
            }
            _ => {
                // THE DOOR THAT NEEDS NO DELIBERATE CALL (FINDING 08). The seat on
                // the clock goes quiet, its own action clock folds it, and it then
                // cashes out with the hand still live.
                let who = on_clock(&mid);
                world.advance(Duration::from_secs(11));
                let timeout = world
                    .check_timeouts(world.actor("alice"))
                    .expect("check_timeouts must not be rejected");
                assert!(
                    matches!(timeout, TimeoutCheckResult::PlayerTimedOut(_)),
                    "{door}: the quiet seat must have been folded by its own clock, got \
                     {timeout:?}"
                );
                match world.cash_out(who) {
                    Ok(_) => who,
                    Err(e) => {
                        println!("{door}: cash_out refused ({e:?}); this door did not open");
                        continue;
                    }
                }
            }
        };
        doors_exercised += 1;

        let after = world.table_state();
        if !after.phase.hand_in_progress() {
            println!("{door}: the departure ended the hand outright; nothing was orphaned");
            continue;
        }

        let stake = after
            .departed_stakes
            .clone()
            .unwrap_or_default()
            .iter()
            .filter(|d| d.hand_number == after.hand_number && d.principal == leaver)
            .fold(0u64, |a, d| a + d.contributed);
        assert!(
            stake > 0,
            "{door}: PINNED FIX (E-05/FINDING 08): the vacated seat's stake must be recorded \
             against its OWNER. departed_stakes = {:?}",
            after.departed_stakes
        );
        assert_eq!(
            after.pot,
            after.payout_basis_total(),
            "{door}: FINDING 05/08 HAS RETURNED: the pot is {} but only {} of it is \
             attributed to an owner. The missing {} is what `apply_side_pots` used to append \
             to the HIGHEST bet level -- the pot only the deepest stacks can win.",
            after.pot,
            after.payout_basis_total(),
            after.pot as i128 - after.payout_basis_total() as i128,
        );
        let amounts_after: Vec<u64> = after.side_pots.iter().map(|p| p.amount).collect();
        assert_eq!(
            amounts_after, amounts_before,
            "{door}: FINDING 05/08 HAS RETURNED: the LAYER AMOUNTS changed when a seat was \
             vacated. Losing a claim is legal and changes who is ELIGIBLE; it must never \
             move an e8 from the main pot the short all-in can win into the side pot only \
             the deepest stacks can. before={:?} after={:?}",
            mid.side_pots,
            after.side_pots
        );
        let vs = check_pot_breakdown(&world.snapshot());
        assert!(vs.is_empty(), "{door}: M1b must be satisfied: {vs:?}");
        println!(
            "FINDING 05/08 refuted via {door}: stake {stake} stayed in the basis; layer \
             amounts {amounts_before:?} unchanged, eligibility {:?} -> {:?}",
            mid.side_pots
                .iter()
                .map(|p| p.eligible_players.clone())
                .collect::<Vec<_>>(),
            after
                .side_pots
                .iter()
                .map(|p| p.eligible_players.clone())
                .collect::<Vec<_>>()
        );
    }

    assert_eq!(
        doors_exercised, 2,
        "both doors out of an occupied seat mid-hand must have been exercised, or this test \
         is measuring less than the finding"
    );
}

// ===========================================================================
// FINDING 17 -- a hand settled with NO live claim, every stake handed back
// ===========================================================================

/// **REFUTED.** The exact sequence: a stranger takes a chair mid-hand, the player
/// who is going to lose folds, and the winner's client drops so their own clock
/// runs out. On the wave-6 module this settled as "hand every seat back exactly
/// what it put in" -- the fold-out winner was paid NOTHING and every player ended
/// on precisely their buy-in, with exact conservation and no log line.
///
/// It cannot happen now because `count_active_players` and `live_claims` are ONE
/// predicate: whoever the engine counts is exactly whoever it can pay.
#[test]
fn finding17_a_fold_out_pays_the_seat_that_holds_the_claim() {
    let config = TableConfig {
        action_timeout_secs: 10,
        ..TableConfig::six_max_icp()
    };
    let world = seated_with_bystanders(&["alice", "bob"], &["carol"], config, 20 * ICP);
    let carol = world.actor("carol");

    // MEASURED BEFORE THE DEAL. Taken mid-hand it would exclude the money already
    // in the pot, and "every player ends on precisely their buy-in" -- the exact
    // signature of FINDING 17 -- would read as a zero delta for everybody.
    let before = values(&world);

    world.advance(Duration::from_secs(4));
    world.start_new_hand(world.actor("alice")).expect("deal");
    level_the_bets(&world);

    // A real pot, and a stranger in a chair they were never dealt into.
    let mid = world.table_state();
    assert!(mid.pot > 0, "the reproduction needs a real pot");
    let _ = world.join_table(carol, 2);
    let _ = world.sit_in(carol);

    // The player on the clock folds. That decides the hand.
    let folder = on_clock(&world.table_state());
    let winner = world
        .table_state()
        .seated()
        .find(|p| p.principal != folder && p.principal != carol && p.hole_cards.is_some())
        .map(|p| p.principal)
        .expect("the other card-holder");
    world
        .player_action(folder, PlayerAction::Fold)
        .expect("folding on your own turn");

    // And the winner's client drops: their own clock expires unanswered.
    world.advance(Duration::from_secs(11));
    let _ = world.check_timeouts(carol);

    let after = values(&world);
    let t = world.table_state();
    assert!(
        !t.phase.hand_in_progress(),
        "the hand must be over once one of the two card-holders folds; phase = {:?}",
        t.phase
    );

    let d = |who: Principal| after[&who] as i128 - before[&who] as i128;
    assert!(
        d(winner) > 0,
        "FINDING 17 HAS RETURNED: the seat that still held a live claim came out of the hand \
         {} e8s up. It won a {} e8 pot by fold-out and was paid nothing -- the settlement \
         took the 'nobody can win ANY of this money' branch and handed every stake back.",
        d(winner),
        mid.pot
    );
    assert!(
        d(folder) < 0,
        "FINDING 17 HAS RETURNED: the player who FOLDED came out of the hand {} e8s, so they \
         got their stake back. Folding has to cost what was put in.",
        d(folder)
    );
    assert_eq!(
        d(carol),
        0,
        "the stranger staked nothing and must win nothing"
    );
    assert_eq!(
        d(winner) + d(folder) + d(carol),
        0,
        "NO RAKE: chips awarded must equal chips wagered, to the e8"
    );
    println!(
        "FINDING 17 refuted: pot {} -> winner {:+}, folder {:+}, mid-hand arrival {:+}",
        mid.pot,
        d(winner),
        d(folder),
        d(carol)
    );
}

// ===========================================================================
// FINDING 22 -- the recovery door could void a live hand, invisibly
// ===========================================================================
//
// THE ONE THAT WAS STILL REAL, and the only one in this cluster that is a design
// question rather than a bug. Reproduced on module `792a9487...` before the fix:
//
//   controller sees: phase=Flop pot=6000000 cards=[(0, Jd/Ac), (1, Th/5d), (2, 6d/3h)]
//   admin_return_all_chips_to_escrow -> Ok
//   after: phase=Flop pot=0 action_on=2 cards_still_dealt=3
//     ...dae 1998000000 -> 2000000000 (+2000000)
//     ...oae 1998000000 -> 2000000000 (+2000000)
//     ...7ae 1998000000 -> 2000000000 (+2000000)
//   invariant violations: 0
//   hand 1 in the permanent record: winners: [], community_cards: [], participants: None
//
// A live hand on the flop, every hole card readable by the caller, ended by one
// controller call. Every player back on exactly their buy-in, every invariant
// silent, the table left mid-street with cards on the board and every stack at
// zero, and NOTHING anywhere saying it happened.
//
// THE DECISION, in full in docs/SECURITY-FINDINGS.md FINDING 22: keep the power,
// narrow the window it can act in, and make every use of it permanent.
//
//   * The door refuses while a hand can still be moved. `hand_cannot_move_right_now`
//     is pure state -- `now > action_timer.expires_at`, or no timer at all -- so no
//     trap, no lost timer and no missing stall witness can stop it becoming true.
//     That is what makes it safe to gate on, where `hand_is_stuck` would not be:
//     gating recovery on a predicate a trapping canister cannot satisfy is how
//     FINDING 15 shut every door at once.
//   * A live hand it DOES reach is closed through `settle_unmovable_hand`, the same
//     routine the permissionless `abandon_stuck_hand` uses, so the money moves
//     identically and the hand is finished rather than left open.
//   * Every credit of that hand is written to the archive with
//     `pot_type = "refund:ended-by-controller"`, and the canister logs a
//     `CRITICAL:` line, which the money-safety classifier treats as run-stopping.

/// A hand that is being played cannot be ended by a controller.
#[test]
fn finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played() {
    let world = seated(
        &["alice", "bob", "carol"],
        TableConfig::six_max_icp(),
        20 * ICP,
    );
    world.advance(Duration::from_secs(4));
    world.start_new_hand(world.actor("alice")).expect("deal");
    level_the_bets(&world);

    let mid = world.table_state();
    assert!(mid.pot > 0, "the reproduction needs a real pot");
    assert!(
        mid.action_timer.is_some(),
        "somebody must be on the clock, or this is not a hand that can still be played"
    );
    assert!(
        mid.seated()
            .filter(|p| !p.has_folded && p.hole_cards.is_some())
            .count()
            >= 2,
        "the hand must still be contested"
    );
    // The controller really can read every hand first. That is the power the
    // refusal below has to be measured against.
    assert!(
        mid.seated().all(|p| p.hole_cards.is_some()),
        "get_table_state shows the controller every hole card: {:?}",
        mid.seated().map(|p| (p.seat, p.hole_cards)).collect::<Vec<_>>()
    );

    let before = world.snapshot();
    for method in ["admin_return_all_chips_to_escrow", "admin_reinit_table"] {
        let arg = if method == "admin_reinit_table" {
            Encode!(&TableConfig::six_max_icp()).unwrap()
        } else {
            Encode!().unwrap()
        };
        let reply = world
            .pic
            .update_call(world.table, world.controller, method, arg)
            .expect("the call itself must not be rejected");
        let text = String::from_utf8_lossy(&reply).to_string();
        assert!(
            text.contains("Refusing") && text.contains("being played right now"),
            "FINDING 22 HAS RETURNED: `{method}` ended a live, contested hand after its \
             caller had read every hole card. Reply bytes rendered: {text:?}"
        );
        println!("{method} on a live hand -> refused");
    }

    // AND NOTHING MOVED.
    let after = world.snapshot();
    assert_eq!(after.table.pot, before.table.pot, "the pot must be untouched");
    assert_eq!(after.table.phase, before.table.phase, "the phase must be untouched");
    assert_eq!(after.chips_total, before.chips_total, "no stack may have moved");
    assert_eq!(after.escrow_total, before.escrow_total, "no escrow may have moved");
    assert_eq!(
        after
            .table
            .seated()
            .map(|p| (p.seat, p.hole_cards))
            .collect::<Vec<_>>(),
        before
            .table
            .seated()
            .map(|p| (p.seat, p.hole_cards))
            .collect::<Vec<_>>(),
        "the deal must be untouched"
    );
    money_safety::assert_no_new_violations(
        &invariants::check_world(&world),
        "after a refused mid-hand recovery",
    );
}

/// A hand nothing can move IS reachable by the recovery door -- that is the whole
/// point of there being one -- and when it is used the hand is CLOSED and the
/// permanent record says a controller closed it.
#[test]
fn finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked() {
    let config = TableConfig {
        action_timeout_secs: 10,
        ..TableConfig::six_max_icp()
    };
    let mut world = seated(&["alice", "bob", "carol"], config, 20 * ICP);
    let before_deal = world.snapshot().internal_total();
    let escrow_before_deal = values(&world);
    world.advance(Duration::from_secs(4));
    world.start_new_hand(world.actor("alice")).expect("deal");
    level_the_bets(&world);
    let _ = world.new_canister_logs();

    let mid = world.table_state();
    let hand = mid.hand_number;
    let pot = mid.pot;
    assert!(pot > 0, "the recovery needs a real pot to hand back");

    // Let the clock run out WITHOUT anybody resolving it. `hand_cannot_move_right_now`
    // is now true, so the recovery door opens.
    world.advance_time_only(Duration::from_secs(11));
    let reply = world
        .pic
        .update_call(
            world.table,
            world.controller,
            "admin_return_all_chips_to_escrow",
            Encode!().unwrap(),
        )
        .expect("must not be rejected");
    let text = String::from_utf8_lossy(&reply).to_string();
    assert!(
        !text.contains("Refusing"),
        "a hand nothing can move must still be reachable, or the recovery primitive \
         `reset_table` points at does not exist: {text:?}"
    );

    // 1. THE HAND IS CLOSED, not left open with cards on the board and every stack
    //    at zero. That state made `reload` refuse "Cannot reload during a hand".
    let after = world.table_state();
    assert!(
        !after.phase.hand_in_progress(),
        "FINDING 22's second half HAS RETURNED: the recovery door emptied the pot and left \
         the table at phase {:?} with {} seats still holding cards",
        after.phase,
        after.seated().filter(|p| p.hole_cards.is_some()).count()
    );
    assert_eq!(after.pot, 0, "the pot must be empty");

    // 2. NO MONEY WAS CREATED OR DESTROYED, and every player has exactly what they
    //    walked in with: nobody won.
    let now_values = values(&world);
    assert_eq!(
        world.snapshot().internal_total(),
        before_deal,
        "NO RAKE: total liability must be unchanged to the e8"
    );
    for (who, v) in &escrow_before_deal {
        assert_eq!(
            now_values.get(who).copied().unwrap_or(0),
            *v,
            "{} must hold exactly what they had before the deal", who.to_text()
        );
    }

    // 3. THE PERMANENT RECORD SAYS A CONTROLLER DID IT. Every credit of this hand
    //    is archived with pot_type = "refund:ended-by-controller", which no hand
    //    that was played out can carry.
    let logs = world.new_canister_logs();
    let marked: Vec<&String> = logs
        .iter()
        .filter(|l| l.contains("ENDED BY A CONTROLLER"))
        .collect();
    assert!(
        !marked.is_empty(),
        "FINDING 22 HAS RETURNED: a controller ended hand {hand} ({pot} e8s in the pot) and \
         the canister said nothing about it. Conservation is exact, so this line is the only \
         thing in the system that can tell this hand apart from one that was played out. \
         Logs: {logs:#?}"
    );
    assert!(
        marked[0].contains("CRITICAL:"),
        "the line must be CRITICAL:, so the money-safety classifier stops a run on it: {:?}",
        marked[0]
    );
    assert!(
        marked[0].contains("refund:ended-by-controller"),
        "the line must name the label the archive carries: {:?}",
        marked[0]
    );
    println!("{}", marked[0]);

    // And the hand really was recorded as ended, with the refunds as its credits.
    let record = world
        .hand_history(hand)
        .unwrap_or_else(|| panic!("hand {hand} must be in the local record"));
    assert_eq!(
        record.awarded_total(),
        pot,
        "every e8 the hand collected must appear in the record as a credit"
    );
    println!(
        "FINDING 22: hand {hand} ended by a controller, {pot} e8s handed back across {} \
         credits, phase now {:?}",
        record.winners.len(),
        after.phase
    );

    money_safety::assert_no_new_violations(
        &invariants::check_world(&world),
        "after a controller ended an unmovable hand",
    );
}
