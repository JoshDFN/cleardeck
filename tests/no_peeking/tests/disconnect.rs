//! # THE DISCONNECT CASE, which is where these protocols die
//!
//! `docs/NO-PEEKING-FEASIBILITY.md` rejected mental poker for exactly one reason:
//! a player who disconnects mid-hand holds a decryption share nobody else has, so
//! the pot cannot be awarded. This engine already treats disconnect as a routine,
//! expected event with a clock attached (FINDING 12, 16, 19, the 30/45 s action
//! clocks, the time bank, the 300 s stuck-hand grace and the permissionless
//! `abandon_stuck_hand` door), and it has already shipped BOTH of the failures
//! that follow from getting this wrong: a pot that could not be closed, and a
//! close-by-refunding-everybody that robbed the winner.
//!
//! So this file is the one that decides whether the design is shippable. It
//! answers, by execution:
//!
//! | question | answer |
//! |---|---|
//! | A player goes silent mid-street. Does the hand stall? | No. One action clock, then the table may stand that seat down, and the dealer measures the clock itself. |
//! | Can the table use that to see a card early? | Not at the timescale of a real hand. A heartbeating seat cannot be stood down on the action clock at all, and the only other route costs the table five minutes of visible stalling PER STREET and is recorded as a different kind of readiness. |
//! | Then can one player freeze a hand by heartbeating and never getting ready? | No, and that is what the second clock is for. It is a trade and it is argued in `a_heartbeating_griefer_cannot_freeze_the_hand_forever`. |
//! | The table dies with a pot on the felt. Is the pot stuck? | No. After `force_finalize_after_ns`, ANY principal opens the hand and the pot settles to its real winner. |
//! | Does that rescue rob the winner by refunding everyone? | No. It settles from the real cards, and this file checks the winner against the seed. |
//! | The dealer freezes mid-hand and nobody controls it. | It cannot: it refuses to OPEN a hand it might not be able to FINISH, so it can only run dry between hands. |

use candid::Principal;
use no_peeking::say;
use no_peeking::world::*;
use poker_core::{create_deck, shuffle_deck, try_evaluate_hand, Card, HandRank};
use std::time::Duration;

fn banner(title: &str) {
    say!("\n================================================================");
    say!("  {title}");
    say!("================================================================");
}

const MINUTE: Duration = Duration::from_secs(60);

fn seated(w: &World, chips: u64) -> (Principal, Principal, Principal) {
    let (a, b, c) = (w.player(0), w.player(1), w.player(2));
    w.sit(a, 0, chips).unwrap();
    w.sit(b, 1, chips).unwrap();
    w.sit(c, 2, chips).unwrap();
    (a, b, c)
}

// ---------------------------------------------------------------------------

/// A player goes silent. The street does not stall, and it does not open early
/// either.
#[test]
fn a_silent_seat_costs_one_action_clock_and_the_hand_moves_on() {
    banner("A SILENT SEAT — one clock, then the hand moves on");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();

    // Alice and Bob are here. Carol's browser tab closed.
    w.ack(alice, hand_id).unwrap();
    let state = w.ack(bob, hand_id).unwrap();
    say!("after two acks, still waiting on {:?}", state.waiting_on);
    assert_eq!(state.waiting_on, vec![2], "carol has not acked");
    assert!(!state.can_advance);

    // The table cannot simply declare her gone.
    let too_early = w.table_stand_down(w.table, hand_id, 2);
    say!("table_stand_down(seat 2) straight away -> {}", too_early.err_text().trim());
    assert!(
        !too_early.is_ok(),
        "the table stood a seat down with no clock at all"
    );
    assert!(w.community(alice, hand_id).is_empty(), "no flop yet");

    // Thirty seconds in: still too early. The clock is the DEALER'S, so the table
    // cannot shorten it by asking more often.
    w.pic.advance_time(Duration::from_secs(30));
    w.pic.tick();
    let still_early = w.table_stand_down(w.table, hand_id, 2);
    say!("  at +30 s -> {}", still_early.err_text().trim());
    assert!(!still_early.is_ok());

    // Past the clock: allowed. Carol is stood down and the street can open.
    w.pic.advance_time(Duration::from_secs(35));
    w.pic.tick();
    let ok = w.table_stand_down(w.table, hand_id, 2).unwrap();
    say!(
        "  at +65 s -> stood down; ready = {:?}, waiting on {:?}",
        ok.ready,
        ok.waiting_on
    );
    assert!(ok.waiting_on.is_empty());

    // And the hand moves. Preflop betting still has to close on the table's side.
    w.act(alice, Action::Call).unwrap();
    w.act(bob, Action::Call).unwrap();
    // Carol is silent, so her seat never acts; the stub's round does not close
    // until she is out of the way. In the live engine that is the action clock's
    // job; here the harness folds her so the file stays about the DEALER.
    let _ = w.act(carol, Action::Fold);
    w.try_advance(alice).unwrap();
    let board = w.community(alice, hand_id);
    say!("  flop = {} cards", board.len());
    assert_eq!(board.len(), 3, "the street opened despite the silent seat");

    // THE RECORD. The reveal that followed a coerced stand-down says so, on chain,
    // to everybody. A player who wants to know whether their seat was timed out
    // does not have to ask the operator.
    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    let flop_reveal = public
        .reveals
        .iter()
        .find(|r| r.street == dealer_types::Street::Flop)
        .expect("the flop reveal is logged");
    say!("  reveal record: requested_by = {}", flop_reveal.requested_by);
    for r in &flop_reveal.readiness {
        say!("    seat {} : {:?}", r.seat, r.how);
    }
    assert!(
        flop_reveal
            .readiness
            .iter()
            .any(|r| r.seat == 2 && r.how == dealer_types::Readiness::TimedOutByTable),
        "the coerced stand-down must be on the record"
    );
    assert_eq!(
        flop_reveal.requested_by, w.table,
        "the reveal log names who asked"
    );
}

/// The other half of the same mechanism, and the one that makes it PREVENTION
/// rather than EVIDENCE: a seat that keeps saying it is there can never be stood
/// down on the ACTION clock, so the table cannot manufacture an early reveal
/// against an attentive player at the timescale a hand is actually played at.
#[test]
fn an_attentive_seat_cannot_be_timed_out_at_the_timescale_of_a_real_hand() {
    banner("AN ATTENTIVE SEAT — the early-reveal attack, defeated by a heartbeat");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    let id = w.dealer_identity();
    say!(
        "dealer clocks: action {} s, street grace {} s, last resort {} s",
        id.action_timeout_ns / 1_000_000_000,
        id.street_grace_ns / 1_000_000_000,
        id.force_finalize_after_ns / 1_000_000_000
    );

    // Alice and Bob are ready. Carol is present and heartbeating but has NOT
    // agreed to see the next card — which, in a real hand, is because she still
    // has to act.
    w.ack(alice, hand_id).unwrap();
    w.ack(bob, hand_id).unwrap();

    // The table tries every thirty seconds for four minutes, which is more than
    // four action clocks and several times the length of a real betting round.
    // Carol's client heartbeats on the same cadence as the live engine's own
    // `DISCONNECT_TIMEOUT_SECS = 90`.
    let mut attempts = 0;
    for step in 1..=8 {
        w.pic.advance_time(Duration::from_secs(15));
        w.pic.tick();
        w.ack(carol, hand_id).unwrap(); // heartbeat
        w.pic.advance_time(Duration::from_secs(15));
        w.pic.tick();
        let out = w.table_stand_down(w.table, hand_id, 2);
        attempts += 1;
        assert!(
            !out.is_ok(),
            "at +{} s the table stood down a seat that had heartbeated 15 s earlier",
            step * 30
        );
        if step == 1 {
            say!("  +30 s -> {}", first_line(&out.err_text()));
        }
    }
    say!("  {attempts} attempts across four minutes, all refused");

    // Belt and braces: nobody else can do it at all, at any time.
    for (label, who) in [
        ("the table's controller", w.operator),
        ("a stranger", Principal::self_authenticating(b"nobody")),
        ("anonymous", Principal::anonymous()),
        ("another player", alice),
    ] {
        let out = w.table_stand_down(who, hand_id, 2);
        assert!(!out.is_ok(), "{label} stood carol down");
    }
    say!("  and the controller, a stranger, anonymous and another player are all refused outright");

    // Meanwhile the board has not moved a single card.
    assert!(
        w.community(Principal::anonymous(), hand_id).is_empty(),
        "a card was revealed while a seat was still to act"
    );
    say!("  board after four minutes of trying: 0 cards");
}

/// THE SECOND CLOCK, and why the design would be broken without it.
///
/// The readiness gate would be a griefing weapon if the only way past it were a
/// seat going silent: a player who heartbeats forever but never says it is ready
/// would freeze the hand until the one-hour last-resort door. So the table may also
/// stand a seat down once the STREET has been open for the whole grace period —
/// 300 s by default, the table's own `STUCK_HAND_GRACE_NS`, five action clocks, and
/// far outside the timescale of a real betting round.
///
/// The trade, stated so it can be argued with: a table that wants to reveal a
/// street early has to sit visibly still for five minutes first, with every
/// client watching `ack_status`, and the reveal is then recorded as
/// `GraceExpired` rather than `Acked`. That is evidence rather than prevention —
/// but it is bounded evidence, and the alternative is a hand any single player can
/// freeze.
#[test]
fn a_heartbeating_griefer_cannot_freeze_the_hand_forever() {
    banner("THE GRIEFER — bounded, and on the record");
    // A short grace so the test is quick; the shape is what matters.
    let grace = 300u64;
    let w = World::configured(
        &["alice", "bob", "carol"],
        Some(grace * 1_000_000_000),
        None,
        None,
    );
    let (alice, bob, carol) = seated(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    w.ack(alice, hand_id).unwrap();
    w.ack(bob, hand_id).unwrap();

    // Carol heartbeats forever and never gets ready.
    for _ in 0..9 {
        w.pic.advance_time(Duration::from_secs(30));
        w.pic.tick();
        w.ack(carol, hand_id).unwrap();
        assert!(
            !w.table_stand_down(w.table, hand_id, 2).is_ok(),
            "the grace period must not have expired yet"
        );
    }
    say!("  4.5 minutes of heartbeating: still refused");

    // Past the grace period, the street can be forced on — and it is recorded as a
    // DIFFERENT kind of readiness, so the record can tell a griefer from a stall.
    w.pic.advance_time(Duration::from_secs(60));
    w.pic.tick();
    w.ack(carol, hand_id).unwrap(); // still heartbeating, and it no longer helps her
    let ok = w.table_stand_down(w.table, hand_id, 2).unwrap();
    say!("  at +5.5 minutes -> {:?}", ok.ready);
    assert!(
        ok.ready
            .iter()
            .any(|r| r.seat == 2 && r.how == dealer_types::Readiness::GraceExpired),
        "the grace hatch must be recorded as GraceExpired, not as TimedOutByTable"
    );

    // The street opens, and the hatch is NOT carried forward: the table has to burn
    // the whole grace period again on the next street. That is what stops a table
    // from paying five minutes once and then running the hand out instantly.
    w.act(alice, Action::Call).unwrap();
    w.act(bob, Action::Call).unwrap();
    let _ = w.act(carol, Action::Fold);
    w.try_advance(alice).unwrap();
    assert_eq!(w.community(alice, hand_id).len(), 3, "the flop is out");

    let waiting = w
        .hand_public(Principal::anonymous(), hand_id)
        .unwrap()
        .ready;
    say!("  after the flop opened, readiness carried forward = {waiting:?}");
    assert!(
        !waiting
            .iter()
            .any(|r| r.seat == 2 && r.how == dealer_types::Readiness::GraceExpired),
        "the grace hatch was carried into the next street; a table that burned it once \
         would then own that seat for the rest of the hand"
    );
    let again = w.table_stand_down(w.table, hand_id, 2);
    say!("  and on the new street the table is refused again: {}", first_line(&again.err_text()));
    assert!(!again.is_ok());
}

/// The table dies with a pot on the felt.
///
/// This is the scenario that has bitten this project twice. The pot must not be
/// stuck forever, and it must not be closed by refunding everybody so that the
/// winner is robbed. Neither happens: any principal opens the hand after the
/// last-resort clock, and the pot settles to the seat that actually won it.
#[test]
fn a_dead_table_does_not_strand_the_pot_and_the_real_winner_is_paid() {
    banner("A DEAD TABLE — the pot is not stuck and the winner is not robbed");
    // A one-hour last-resort door, the default, and a harness that can wait.
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();

    // Everybody puts money in, then everybody vanishes: no acks, no actions, no
    // operator process.
    w.act(alice, Action::RaiseTo(200)).unwrap();
    w.act(bob, Action::Call).unwrap();
    w.act(carol, Action::Call).unwrap();
    let pot = w.view().pot;
    say!("pot on the felt: {pot}");
    assert_eq!(pot, 600);

    // Nothing moves. Not even the last-resort door, for a full hour.
    let stranger = Principal::self_authenticating(b"a-passer-by");
    for (label, after) in [("+1 minute", 1u64), ("+30 minutes", 29), ("+59 minutes", 29)] {
        w.pic.advance_time(MINUTE * after as u32);
        w.pic.tick();
        let out = w.force_finalize(stranger, hand_id);
        say!("  force_finalize at {label:<12} -> {}", first_line(&out.err_text()));
        assert!(
            !out.is_ok(),
            "the last-resort door opened at {label}, which is inside a hand that could \
             still be live. It would be a peek door."
        );
        assert!(
            w.hand_public(stranger, hand_id)
                .unwrap()
                .revealed_seed
                .is_none(),
            "the seed leaked at {label}"
        );
    }

    // Past the hour, anybody at all opens it.
    w.pic.advance_time(MINUTE * 3);
    w.pic.tick();
    let forced = w.force_finalize(stranger, hand_id).unwrap();
    say!(
        "\n  force_finalize at +62 minutes, called by {} (not the operator, not a player)",
        forced.forced_by
    );
    say!("    seed      {}", forced.revealed_seed);
    say!("    board     {} cards", forced.community.len());
    say!("    hole      {} seats", forced.hole_cards.len());
    assert_eq!(forced.community.len(), 5);
    assert_eq!(forced.hole_cards.len(), 3);

    // And the pot settles — permissionlessly, from the real cards.
    let winners = w.rescue_hand(stranger).unwrap();
    let view = w.view();
    say!(
        "    winners   {:?}",
        winners.iter().map(|x| (x.seat, x.amount)).collect::<Vec<_>>()
    );
    assert!(!winners.is_empty(), "THE POT IS STUCK. This is the failure.");
    assert_eq!(
        winners.iter().map(|x| x.amount).sum::<u64>(),
        pot,
        "the whole pot must be paid out, not part of it"
    );

    // THE WINNER IS THE ONE WHO ACTUALLY WON. Recomputed here from the seed, the
    // way an outsider would, not read back off the canister that decided it.
    let mut deck = create_deck();
    shuffle_deck(&mut deck, &hex::decode(&forced.revealed_seed).unwrap());
    let p = 3usize;
    let board: Vec<Card> = [2 * p + 1, 2 * p + 2, 2 * p + 3, 2 * p + 5, 2 * p + 7]
        .into_iter()
        .map(|i| deck[i])
        .collect();
    let mut best: Option<(u8, HandRank)> = None;
    for k in 0..p {
        let hole = (deck[2 * k], deck[2 * k + 1]);
        let rank = try_evaluate_hand(&hole, &board).expect("five community cards");
        say!("    seat {k} holds a hand ranking {rank:?}");
        match &best {
            Some((_, b)) if *b >= rank => {}
            _ => best = Some((k as u8, rank)),
        }
    }
    let (expected_seat, expected_rank) = best.unwrap();
    say!("    the seed says seat {expected_seat} wins with {expected_rank:?}");
    assert!(
        winners.iter().all(|x| x.seat == expected_seat),
        "the rescue paid {:?} but the cards say seat {expected_seat}. A rescue that pays \
         the wrong seat is worse than a stuck pot.",
        winners.iter().map(|x| x.seat).collect::<Vec<_>>()
    );

    // Nobody was refunded: the two losers are down what they put in.
    let chips: Vec<u64> = [alice, bob, carol].iter().map(|x| w.chips_of(*x)).collect();
    say!("    chips     {chips:?}");
    assert_eq!(chips.iter().sum::<u64>(), 3_000, "chips are conserved");
    assert_eq!(
        chips.iter().filter(|c| **c == 1_000).count(),
        0,
        "if everybody has their stack back, this was a refund and the winner was robbed"
    );
    assert!(
        chips[expected_seat as usize] > 1_000,
        "the winner must be up"
    );
    let _ = view;
}

/// The dealer cannot freeze mid-hand, because it will not START a hand it might
/// not be able to FINISH.
///
/// `docs/NO-PEEKING-FEASIBILITY.md` §8 named this as unrecoverable in a way the
/// table's freeze is not: `update_settings` on a zero-controller canister is
/// refused forever, so the freezing threshold can never be raised after handover.
/// The answer is a floor, checked before any hand is opened, and a health surface
/// anybody can read so anybody can top the dealer up before it matters.
#[test]
fn the_dealer_refuses_to_open_a_hand_it_might_not_be_able_to_finish() {
    banner("THE RUNWAY GATE — a hand that cannot be finished is never started");

    // A floor above the balance this world hands out.
    let floor = 500_000_000_000_000u128;
    let w = World::with_settings(&["alice", "bob", "carol"], None, Some(floor));
    let (alice, _bob, _carol) = seated(&w, 1_000);

    let health = w.dealer_health();
    say!(
        "dealer balance {} vs floor {} -> can_open_hand = {}",
        health.cycle_balance,
        health.min_open_balance,
        health.can_open_hand
    );
    assert!(health.cycle_balance < health.min_open_balance);
    assert!(!health.can_open_hand);

    let refused = w.start_hand(alice);
    say!("start_hand -> {}", first_line(&refused.err_text()));
    assert!(
        !refused.is_ok(),
        "the dealer opened a hand it could not afford to finish"
    );
    assert!(
        refused.err_text().contains("below the floor"),
        "the refusal must say WHY and what to do about it: {}",
        refused.err_text()
    );

    // Anyone may top it up. On a replica the real call is
    // `dfx canister deposit-cycles <amount> <dealer>`, which
    // docs/NO-PEEKING-FEASIBILITY.md §6 measured succeeding from a NON-controller
    // against a canister with an empty controller list. PocketIC's `add_cycles` is
    // the harness's own backdoor and is used here only to reach the state; it is
    // NOT evidence about who may call `deposit_cycles`.
    w.pic.add_cycles(w.dealer, floor);
    let health = w.dealer_health();
    say!(
        "after a top-up: balance {} -> can_open_hand = {}",
        health.cycle_balance,
        health.can_open_hand
    );
    assert!(health.can_open_hand);

    let hand_id = w.start_hand(alice).unwrap();
    say!("hand {hand_id} opened");
    assert_eq!(w.view().phase, Phase::PreFlop);
    assert_eq!(w.dealer_health().live_hand, Some(hand_id));
}

/// A folded player who then disconnects does not hold the hand up either, and a
/// player who folds and says so does not hold it up for even one clock.
#[test]
fn folding_and_saying_so_costs_nothing_and_folding_silently_costs_one_clock() {
    banner("THE FOLD, both ways");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();

    // Alice folds and tells the dealer herself: instant.
    w.act(alice, Action::Fold).unwrap();
    let state = w.stand_down(alice, hand_id).unwrap();
    say!("alice folded and stood down; waiting on {:?}", state.waiting_on);
    assert!(!state.waiting_on.contains(&0));

    // Bob folds and his client dies before it can say so: one clock.
    w.act(bob, Action::Fold).unwrap();
    let before = w.table_stand_down(w.table, hand_id, 1);
    assert!(!before.is_ok(), "no clock has passed");
    w.pic.advance_time(Duration::from_secs(61));
    w.pic.tick();
    w.table_stand_down(w.table, hand_id, 1).unwrap();
    say!("bob folded silently and cost the table exactly one action clock");

    // Carol is the only one left, so the hand is over. She stands down too and the
    // seed is published.
    w.stand_down(carol, hand_id).unwrap();
    w.try_advance(alice).unwrap();
    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    assert!(public.revealed_seed.is_some(), "the hand closed normally");
    assert!(
        public.showdown.is_empty(),
        "a fold-out shows nobody a card at showdown"
    );
    say!("hand closed; seed published; no showdown");

    // And the seed reveal is NOT reachable while a seat could still have to act.
    // Second hand: two seats stand down, one does not.
    let hand2 = w.start_hand(alice).unwrap();
    w.stand_down(alice, hand2).unwrap();
    w.stand_down(bob, hand2).unwrap();
    let refused = w.finish_hand_direct(w.table, hand2);
    say!(
        "\nwith seat 2 still live, finish_hand -> {}",
        first_line(&refused.err_text())
    );
    assert!(
        !refused.is_ok(),
        "the seed was published while a seat could still have to act. Publishing the \
         seed publishes EVERY card, so this is the whole finding again."
    );
    assert!(w
        .hand_public(Principal::anonymous(), hand2)
        .unwrap()
        .revealed_seed
        .is_none());
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}
