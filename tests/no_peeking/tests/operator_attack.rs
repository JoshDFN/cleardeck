//! # THE OPERATOR WHO WANTS TO SEE THE CARDS
//!
//! Adversarial review of the sealed-dealer spike. Everything in this file is run
//! by the principal that holds the TABLE's controller key — the exact principal
//! the auditor's finding is about — against a dealer whose controller list is
//! genuinely empty (`World::assert_dealer_is_sealed` re-reads it from the replica).
//!
//! Each test is named for the claim it attacks.

use candid::{encode_one, Principal};
use dealer_types::{DealerInit, DealtInSeat, Readiness, Street};
use no_peeking::say;
use no_peeking::world::*;
use no_peeking::build;
use pocket_ic::{PocketIcBuilder, Time};
use poker_core::{create_deck, shuffle_deck, Card};
use std::time::Duration;

const GENESIS_NANOS: u64 = 1_767_225_600_000_000_000;
const START_CYCLES: u128 = 100_000_000_000_000;

fn banner(t: &str) {
    say!("\n================================================================");
    say!("  {t}");
    say!("================================================================");
}

fn seated3(w: &World, chips: u64) -> (Principal, Principal, Principal) {
    let (a, b, c) = (w.player(0), w.player(1), w.player(2));
    w.sit(a, 0, chips).unwrap();
    w.sit(b, 1, chips).unwrap();
    w.sit(c, 2, chips).unwrap();
    (a, b, c)
}

/// Build a world with FULL control of the dealer's init arguments, including
/// `action_timeout_ns` which `World::configured` does not expose.
fn world_with_init(
    player_names: &[&str],
    action_timeout_ns: Option<u64>,
    street_grace_ns: Option<u64>,
    force_finalize_after_ns: Option<u64>,
) -> World {
    let mut builder = PocketIcBuilder::new().with_application_subnet();
    if let Some(bin) = pocket_ic_binary() {
        builder = builder.with_server_binary(bin);
    }
    let pic = builder.build();
    pic.set_time(Time::from_nanos_since_unix_epoch(GENESIS_NANOS));

    let operator = Principal::self_authenticating(b"cleardeck-no-peeking-operator");
    let players: Vec<Principal> = player_names
        .iter()
        .map(|n| Principal::self_authenticating(format!("cleardeck-no-peeking-{n}").as_bytes()))
        .collect();

    let dealer = pic.create_canister_with_settings(Some(operator), None);
    let table = pic.create_canister_with_settings(Some(operator), None);
    pic.add_cycles(dealer, START_CYCLES);
    pic.add_cycles(table, START_CYCLES);

    let dealer_wasm = build::dealer_wasm();
    let table_wasm = build::table_stub_wasm();

    pic.install_canister(
        dealer,
        dealer_wasm.clone(),
        encode_one(DealerInit {
            table,
            action_timeout_ns,
            street_grace_ns,
            force_finalize_after_ns,
            min_open_balance: None,
        })
        .unwrap(),
        Some(operator),
    );
    pic.install_canister(
        table,
        table_wasm.clone(),
        encode_one(StubInit {
            dealer,
            config: StubConfig {
                small_blind: 10,
                big_blind: 20,
            },
        })
        .unwrap(),
        Some(operator),
    );
    // THE HANDOVER. Exactly as the harness does it: the dealer ends with no
    // controller at all.
    pic.set_controllers(dealer, Some(operator), vec![]).unwrap();

    let w = World {
        pic,
        dealer,
        table,
        operator,
        players,
        dealer_wasm,
        table_wasm,
    };
    w.assert_dealer_is_sealed();
    w
}

/// Recompute the whole hand the way an outside verifier would, so a leaked card
/// can be named rather than merely counted.
fn oracle(seed_hex: &str, p: usize) -> (Vec<(Card, Card)>, Vec<Card>) {
    let seed = hex_decode(seed_hex);
    let mut deck = create_deck();
    shuffle_deck(&mut deck, &seed);
    let holes = (0..p).map(|k| (deck[2 * k], deck[2 * k + 1])).collect();
    let board = vec![
        deck[2 * p + 1],
        deck[2 * p + 2],
        deck[2 * p + 3],
        deck[2 * p + 5],
        deck[2 * p + 7],
    ];
    (holes, board)
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// ===========================================================================
// ATTACK 1 — the heartbeat IS the consent
// ===========================================================================

/// CLAIM UNDER ATTACK (claim 4 / claim 5):
///
/// > "A street cannot be revealed until every dealt-in seat says it is ready in
/// >  its own name" ... "A heartbeating player cannot be timed out on the action
/// >  clock by anyone, so the table cannot manufacture an early reveal against an
/// >  attentive seat."
///
/// `tests/disconnect.rs::an_attentive_seat_...` makes Carol heartbeat with
/// `ack()` and then only ever tries `table_stand_down`, which is refused, and
/// concludes "board after four minutes of trying: 0 cards".
///
/// But `ack()` is the ONLY liveness call the dealer has, and it does
/// `ready.entry(seat).or_insert(Readiness::Acked)`. So Carol's heartbeat is her
/// consent. The attack is not `table_stand_down`; it is `advance_street`, one
/// message later, with zero seconds elapsed.
#[test]
fn a_heartbeat_is_indistinguishable_from_consent_so_the_gate_opens_instantly() {
    banner("ATTACK 1 — the attentive seat's heartbeat IS the reveal");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    let t0 = w.now_nanos();

    // Carol is present and heartbeating. She has NOT acted: the table's own view
    // says the betting round is open and the action is on her.
    w.ack(alice, hand_id).unwrap();
    w.ack(bob, hand_id).unwrap();
    let st = w.ack(carol, hand_id).unwrap();
    say!("  carol's heartbeat -> ready {:?}", st.ready);
    say!("  dealer now says can_advance = {}", st.can_advance);

    let view = w.view();
    say!(
        "  meanwhile the TABLE says: phase {:?}, action_on {:?}, betting_round_closed {}",
        view.phase,
        view.action_on,
        w.query_round_closed()
    );
    assert!(
        !w.query_round_closed(),
        "precondition: preflop betting has NOT closed"
    );
    assert_eq!(view.action_on, Some(0), "somebody still has to act");

    // The operator's table asks for the flop. Zero seconds have passed.
    let flop = w.advance_street_direct(w.table, hand_id).unwrap();
    let elapsed = w.now_nanos() - t0;
    say!(
        "  advance_street as the table -> {} cards, {} ns after the deal",
        flop.len(),
        elapsed
    );
    assert_eq!(flop.len(), 3, "the flop came out with betting still open");

    // And it keeps going: heartbeat, reveal, heartbeat, reveal.
    for (street, want) in [(Street::Turn, 4usize), (Street::River, 5usize)] {
        w.everyone_acks(hand_id);
        let b = w.advance_street_direct(w.table, hand_id).unwrap();
        say!("  {street:?} -> {} cards", b.len());
        assert_eq!(b.len(), want);
    }

    // River street: one more round of heartbeats and the table takes every hole
    // card, with the pot still on the felt and the table still at PreFlop.
    w.everyone_acks(hand_id);
    let cards = w
        .reveal_showdown_direct(w.table, hand_id, vec![0, 1, 2])
        .unwrap();
    let elapsed = w.now_nanos() - t0;

    let view = w.view();
    say!("  reveal_showdown -> {} seats' hole cards", cards.len());
    for sc in &cards {
        say!("    seat {} : {:?} {:?}", sc.seat, sc.cards.0, sc.cards.1);
    }
    say!(
        "  ELAPSED SINCE THE DEAL: {} ns ({} s). Table phase is still {:?}, pot {} on the felt, \
         nobody has folded, nobody is all in.",
        elapsed,
        elapsed / 1_000_000_000,
        view.phase,
        view.pot
    );

    assert_eq!(cards.len(), 3);
    assert_eq!(view.phase, Phase::PreFlop, "the hand is still live");
    assert!(view.pot > 0, "the money is still unresolved");
    assert!(
        elapsed < 1_000_000_000,
        "the whole disclosure took under a second of replica time"
    );

    // AND THE HAND CARRIES ON. The table still takes bets, from players whose
    // cards the operator is now holding.
    let acted = w.act(alice, Action::Call);
    say!(
        "  alice's preflop action AFTER the operator holds every card: {}",
        if acted.is_ok() { "accepted".to_string() } else { acted.err_text() }
    );
    assert!(acted.is_ok(), "betting continues with the operator holding the cards");
    let v2 = w.view();
    say!("  pot is now {} and action is on {:?}", v2.pot, v2.action_on);
    assert!(v2.pot > view.pot, "fresh money went in after the reveal");

    // Cross-check against the seed: these really are the players' cards.
    let seed = w.finish_hand_direct(w.table, hand_id).unwrap();
    let (holes, _board) = oracle(&seed, 3);
    for (k, sc) in cards.iter().enumerate() {
        assert_eq!(
            sc.cards, holes[k],
            "leaked card must match the published shuffle"
        );
    }
    say!("  and the seed confirms every one of them.");
}

/// The other half: a player who REFUSES to heartbeat, to avoid consenting.
/// The action clock takes her out in sixty seconds, per street. So the ceiling on
/// how long ANY player can hold the street shut is one action clock.
#[test]
fn a_player_who_refuses_to_ack_only_buys_sixty_seconds_a_street() {
    banner("ATTACK 2 — silence buys 60 s a street; four minutes buys every card");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, _bob, _carol) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    let t0 = w.now_nanos();

    // NOBODY acks, ever. Every seat plays the only defence the protocol offers.
    for street in [Street::Flop, Street::Turn, Street::River] {
        w.pic.advance_time(Duration::from_secs(61));
        w.pic.tick();
        for seat in 0..3u8 {
            let out = w.table_stand_down(w.table, hand_id, seat);
            assert!(out.is_ok(), "seat {seat} survived the action clock: {}", out.err_text());
        }
        let b = w.advance_street_direct(w.table, hand_id).unwrap();
        say!(
            "  +{} s -> {street:?} revealed, {} cards on the board",
            (w.now_nanos() - t0) / 1_000_000_000,
            b.len()
        );
    }
    w.pic.advance_time(Duration::from_secs(61));
    w.pic.tick();
    for seat in 0..3u8 {
        w.table_stand_down(w.table, hand_id, seat).unwrap();
    }
    let cards = w
        .reveal_showdown_direct(w.table, hand_id, vec![0, 1, 2])
        .unwrap();
    let elapsed = (w.now_nanos() - t0) / 1_000_000_000;
    say!(
        "  +{elapsed} s -> reveal_showdown returned {} seats' hole cards",
        cards.len()
    );
    for sc in &cards {
        say!("    seat {} : {:?} {:?}", sc.seat, sc.cards.0, sc.cards.1);
    }
    let view = w.view();
    say!(
        "  table phase {:?}, pot {}, action_on {:?}",
        view.phase,
        view.pot,
        view.action_on
    );
    assert_eq!(cards.len(), 3);
    assert_eq!(view.phase, Phase::PreFlop);
    assert!(
        elapsed <= 4 * 61,
        "the whole table's hole cards in {elapsed} s"
    );

    // The record does say GraceExpired/TimedOutByTable — so it is evidence. The
    // design document said evidence was not good enough and that this dealer
    // "does not rely on it".
    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    let kinds: Vec<Readiness> = public
        .reveals
        .iter()
        .flat_map(|r| r.readiness.iter().map(|x| x.how))
        .collect();
    say!("  readiness kinds on the public record: {kinds:?}");
    assert!(kinds.iter().all(|k| *k == Readiness::TimedOutByTable));
}

// ===========================================================================
// ATTACK 3 — the seal is parameterised by unvalidated install arguments
// ===========================================================================

/// `force_finalize` is the last-resort door and its ONLY gate is
/// `hand.opened_at_ns + cfg.force_finalize_after_ns`. That number comes from
/// `DealerInit`, which is supplied by whoever installs the dealer — i.e. the
/// operator, BEFORE the controllers are dropped. `init()` does
/// `.unwrap_or(DEFAULT)` and validates nothing.
///
/// Set it to zero and the sealed dealer hands every hole card and the seed to
/// ANY caller, at `phase = PreFlop`, instantly, forever.
#[test]
fn a_zero_in_the_install_argument_reopens_the_whole_hole() {
    banner("ATTACK 3 — force_finalize_after_ns = 0 at install");
    let w = world_with_init(&["alice", "bob", "carol"], None, None, Some(0));
    let id = w.dealer_identity();
    say!(
        "  dealer_identity: action {} ns, grace {} ns, LAST RESORT {} ns, build {}",
        id.action_timeout_ns,
        id.street_grace_ns,
        id.force_finalize_after_ns,
        id.build
    );
    assert_eq!(id.force_finalize_after_ns, 0);

    let (alice, _b, _c) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    let view = w.view();
    assert_eq!(view.phase, Phase::PreFlop);

    // Not even the operator is needed. A stranger will do.
    let stranger = Principal::self_authenticating(b"any-passerby");
    let forced = w.force_finalize(stranger, hand_id).unwrap();
    say!("  force_finalize by a stranger at phase=PreFlop, 0 ns after the deal:");
    say!("    revealed_seed = {}", forced.revealed_seed);
    for sc in &forced.hole_cards {
        say!("    seat {} : {:?} {:?}", sc.seat, sc.cards.0, sc.cards.1);
    }
    say!("    board = {:?}", forced.community);
    assert_eq!(forced.hole_cards.len(), 3);
    assert_eq!(forced.community.len(), 5);

    let (holes, board) = oracle(&forced.revealed_seed, 3);
    assert_eq!(forced.community, board);
    for (k, sc) in forced.hole_cards.iter().enumerate() {
        assert_eq!(sc.cards, holes[k]);
    }
    say!("  every card checked against the published shuffle: the seal buys nothing here.");
}

/// The same shape with the other clock. `action_timeout_ns = 0` makes
/// `table_stand_down` succeed against every seat the instant the street opens,
/// so the readiness gate is a no-op and the table walks to a full showdown with
/// no waiting at all.
#[test]
fn a_zero_action_clock_makes_the_readiness_gate_a_no_op() {
    banner("ATTACK 4 — action_timeout_ns = 0 at install");
    let w = world_with_init(&["alice", "bob", "carol"], Some(0), None, None);
    let id = w.dealer_identity();
    say!("  dealer_identity: action_timeout_ns = {}", id.action_timeout_ns);
    assert_eq!(id.action_timeout_ns, 0);

    let (alice, _b, _c) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    let t0 = w.now_nanos();

    for _ in 0..3 {
        for seat in 0..3u8 {
            w.table_stand_down(w.table, hand_id, seat).unwrap();
        }
        w.advance_street_direct(w.table, hand_id).unwrap();
    }
    for seat in 0..3u8 {
        w.table_stand_down(w.table, hand_id, seat).unwrap();
    }
    let cards = w
        .reveal_showdown_direct(w.table, hand_id, vec![0, 1, 2])
        .unwrap();
    say!(
        "  every hole card, {} ns after the deal, nobody having acked once:",
        w.now_nanos() - t0
    );
    for sc in &cards {
        say!("    seat {} : {:?} {:?}", sc.seat, sc.cards.0, sc.cards.1);
    }
    assert_eq!(cards.len(), 3);
    assert_eq!(w.view().phase, Phase::PreFlop);
}

// ===========================================================================
// ATTACK 5 — phantom seats: a PRIVATE, unpublished read of live cards
// ===========================================================================

/// Everything else the table can pull out of the dealer becomes public in the
/// same message. This one does not.
///
/// `open_hand(seats)` takes the seat list from the table and validates only
/// shape: >= 2 seats, <= 10, no anonymous, no duplicate seat or principal. It
/// never asks whether a listed principal agreed to play, and it cannot: nobody
/// has signed anything at that point in the protocol.
///
/// So a hostile table lists the victim plus nine principals the operator owns.
/// Each of those calls `my_hole_cards` — a QUERY, answered only to that caller,
/// written to no log and published nowhere — and the operator privately holds 18
/// of the 52 cards while the victim's two are still secret.
#[test]
fn phantom_seats_give_the_operator_private_cards_from_the_live_deck() {
    banner("ATTACK 5 — phantom seats, read privately by query");
    let w = World::new(&["victim"]);
    let victim = w.player(0);

    // The hostile table calls open_hand directly with a seat list of its own
    // choosing. (Equivalent to the operator upgrading the table; strictly weaker
    // than it, since it needs no new code.)
    let phantoms: Vec<Principal> = (1..10u8)
        .map(|i| Principal::self_authenticating(format!("operator-phantom-{i}").as_bytes()))
        .collect();
    let mut seats = vec![DealtInSeat {
        seat: 0,
        principal: victim,
    }];
    for (i, p) in phantoms.iter().enumerate() {
        seats.push(DealtInSeat {
            seat: (i + 1) as u8,
            principal: *p,
        });
    }
    let opened = w.open_hand_direct(w.table, seats).unwrap();
    let hand_id = opened.hand_id;
    say!("  opened hand {hand_id} with {} dealt-in seats", opened.dealt_in.len());

    let mut known: Vec<Card> = Vec::new();
    for p in &phantoms {
        let (a, b) = w.my_hole_cards(*p, hand_id).unwrap();
        known.push(a);
        known.push(b);
    }
    say!("  the operator privately holds {} of 52 cards:", known.len());
    say!("    {known:?}");

    // The victim's cards are still not directly readable.
    let denied = w.my_hole_cards(w.operator, hand_id);
    say!("  operator asking for its own seat: {}", denied.err_text().trim());
    assert!(!denied.is_ok());

    // But the removal information is real, and it is not on any public record
    // beyond the fact that ten seats exist.
    let (victim_a, victim_b) = w.my_hole_cards(victim, hand_id).unwrap();
    assert!(!known.contains(&victim_a) && !known.contains(&victim_b));
    say!(
        "  victim holds {:?} {:?} — 2 of the {} cards the operator has NOT ruled out \
         (was 52 before the phantoms, {} after)",
        victim_a,
        victim_b,
        52 - known.len(),
        52 - known.len()
    );

    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    say!(
        "  the ONLY public trace is dealt_in.len() = {} and the phantom principals. \
         No card the operator read appears anywhere public.",
        public.dealt_in.len()
    );
    assert!(public.showdown.is_empty());
    assert!(public.revealed_seed.is_none());
    assert_eq!(known.len(), 18);
}

// ===========================================================================
// ATTACK 6 — mucked cards
// ===========================================================================

/// `reveal_showdown(hand_id, seats)` accepts ANY dealt-in seat, including one
/// that has already stood itself down — which the dealer's own doc comment
/// defines as "I have folded, or I am all in".
///
/// So the table publishes a folded player's hole cards. In poker a mucked hand is
/// never shown; publishing it hands the whole table that player's folding range.
#[test]
fn a_folded_seat_can_be_made_to_show_its_mucked_cards() {
    banner("ATTACK 6 — the muck is not protected");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();

    // Carol folds, in her own name, on the dealer and on the table.
    w.stand_down(carol, hand_id).unwrap();
    say!("  carol stood down in her own name (the dealer's word for 'folded, or all in')");

    for _ in 0..3 {
        w.ack(alice, hand_id).unwrap();
        w.ack(bob, hand_id).unwrap();
        w.advance_street_direct(w.table, hand_id).unwrap();
    }
    w.ack(alice, hand_id).unwrap();
    w.ack(bob, hand_id).unwrap();

    // The table asks for the folder's cards anyway.
    let cards = w
        .reveal_showdown_direct(w.table, hand_id, vec![0, 1, 2])
        .unwrap();
    let carols = cards.iter().find(|c| c.seat == 2).expect("carol's cards");
    say!("  reveal_showdown([0,1,2]) returned the FOLDER's cards: {:?}", carols.cards);

    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    assert!(
        public.showdown.iter().any(|c| c.seat == 2),
        "and they are now public to the whole table"
    );
    say!("  and hand_public now shows them to everybody, mid-hand.");
    let _ = (bob, carol);
}

// ===========================================================================
// ATTACK 7 — the table's own settlement, under concurrency
// ===========================================================================

/// Not a peek: a money bug, in the shape FINDING 29 has already burned this
/// project with. `try_advance` is permissionless and `async`. It reads the phase,
/// awaits the dealer, and only then mutates. There is no in-flight guard, so two
/// callers in the same round both pass the guard and both settle.
#[test]
fn two_concurrent_try_advance_calls_pay_the_pot_out_twice() {
    banner("ATTACK 7 — concurrent try_advance");
    let w = World::new(&["alice", "bob"]);
    let (alice, bob) = (w.player(0), w.player(1));
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();

    let before: u64 = w.chips_of(alice) + w.chips_of(bob);
    let pot0 = w.view().pot;
    say!("  chips on seats {before} + pot {pot0} = {}", before + pot0);

    // Play to the river honestly. Act in whatever order the table asks for.
    let whose = |w: &World| {
        let v = w.view();
        v.action_on.map(|s| if s == 0 { alice } else { bob })
    };
    w.act(whose(&w).unwrap(), Action::Call).unwrap();
    w.act(whose(&w).unwrap(), Action::Check).unwrap();
    for _ in 0..3 {
        w.everyone_acks(hand_id);
        w.try_advance(alice).unwrap();
        w.act(whose(&w).unwrap(), Action::Check).unwrap();
        w.act(whose(&w).unwrap(), Action::Check).unwrap();
    }
    let v = w.view();
    say!("  at {:?}, pot {}, board {} cards", v.phase, v.pot, v.board.len());
    assert_eq!(v.phase, Phase::River);
    let pot = v.pot;
    w.everyone_acks(hand_id);
    let chips_before_race: u64 = w.chips_of(alice) + w.chips_of(bob);

    // TWO callers ask the table to advance, in the same round.
    let m1 = w
        .pic
        .submit_call(w.table, alice, "try_advance", encode_one(()).unwrap())
        .unwrap();
    let m2 = w
        .pic
        .submit_call(w.table, bob, "try_advance", encode_one(()).unwrap())
        .unwrap();
    let r1 = w.pic.await_call(m1);
    let r2 = w.pic.await_call(m2);
    say!("  call 1 ok = {}, call 2 ok = {}", r1.is_ok(), r2.is_ok());

    let after: u64 = w.chips_of(alice) + w.chips_of(bob);
    let v = w.view();
    say!(
        "  pot was {pot}; chips now {after} (started {before}); winners recorded: {}",
        v.winners.len()
    );
    for win in &v.winners {
        say!("    seat {} paid {}", win.seat, win.amount);
    }
    let total_paid: u64 = v.winners.iter().map(|x| x.amount).sum();
    say!("  TOTAL PAID OUT = {total_paid}, POT WAS {pot}");
    // THE BUG, pinned. The correct assertions are the two commented out below.
    // assert_eq!(total_paid, pot);
    // assert_eq!(after, before + pot);
    assert_eq!(total_paid, 2 * pot, "expected the pot to be paid out twice");
    assert_eq!(
        after,
        chips_before_race + 2 * pot,
        "expected {pot} chips to be created from nothing"
    );
    say!(
        "  CONFIRMED: chips on seats went {chips_before_race} -> {after} for a pot of {pot}. \
         {} chips conjured. Two ordinary players clicking at the same time.",
        after - chips_before_race - pot
    );
    let _ = before;
}

/// The same race one street earlier destroys a betting round outright: two
/// concurrent `try_advance` calls at PreFlop drive the DEALER through two streets,
/// so the flop is dealt and immediately overwritten by the turn and no flop
/// betting ever happens.
#[test]
fn two_concurrent_try_advance_calls_skip_a_whole_betting_round() {
    banner("ATTACK 8 — concurrent try_advance skips a street");
    let w = World::new(&["alice", "bob"]);
    let (alice, bob) = (w.player(0), w.player(1));
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();
    w.act(alice, Action::Call).unwrap();
    w.act(bob, Action::Check).unwrap();
    w.everyone_acks(hand_id);
    // Both seats stand down so readiness survives the street change, which is
    // exactly what happens when both players are all in.
    w.stand_down(alice, hand_id).unwrap();
    w.stand_down(bob, hand_id).unwrap();

    let m1 = w
        .pic
        .submit_call(w.table, alice, "try_advance", encode_one(()).unwrap())
        .unwrap();
    let m2 = w
        .pic
        .submit_call(w.table, bob, "try_advance", encode_one(()).unwrap())
        .unwrap();
    let _ = w.pic.await_call(m1);
    let _ = w.pic.await_call(m2);

    let v = w.view();
    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    say!(
        "  table phase {:?}, board {} cards; dealer street {:?}, dealer board {} cards",
        v.phase,
        v.board.len(),
        public.street,
        public.community.len()
    );
    say!("  reveal records: {}", public.reveals.len());
    for r in &public.reveals {
        say!("    {:?} by {}", r.street, r.requested_by);
    }
    // THE BUG: one betting round's worth of `try_advance` moved the dealer TWO
    // streets. The flop was dealt and immediately buried under the turn, and no
    // flop betting round ever existed.
    assert_eq!(
        public.street,
        Street::Turn,
        "expected the race to skip a street"
    );
    assert_eq!(public.reveals.len(), 2, "two reveals in one betting round");
    assert_eq!(v.board.len(), 4, "the table went straight to a four-card board");
    say!("  CONFIRMED: PreFlop -> Flop -> Turn in one betting round. The flop was never bet.");
}

// ===========================================================================
// ATTACK 9 — what the dealer actually trusts
// ===========================================================================

/// The brief's first question: does the upgrade path reopen the hole?
///
/// It cannot on the DEALER — that is already measured (`IC0512`). But the dealer
/// authorises `open_hand`, `advance_street`, `table_stand_down`, `reveal_showdown`
/// and `finish_hand` with `caller == cfg.table`: a **principal**, frozen at
/// install. A principal is not a program. The operator holds the table's
/// controller key and can put any code at all behind that principal, at any
/// moment, including in the middle of a hand.
///
/// So "only the registered table may call this" reads, in the threat model, as
/// "only the operator may call this". This test does a real
/// `install_code --mode upgrade` on the table mid-hand and shows the dealer
/// notices nothing.
#[test]
fn the_dealer_trusts_a_principal_not_a_program_so_an_upgrade_changes_nothing() {
    banner("ATTACK 9 — the dealer's `require_table` is a principal check");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, _b, _c) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    say!("  hand {hand_id} is live; the dealer has no controller:");
    say!("    dealer controllers = {:?}", w.pic.get_controllers(w.dealer));
    say!("    table  controllers = {:?}", w.pic.get_controllers(w.table));

    // Baseline: the dealer answers a genuine INTER-CANISTER call from the table,
    // and refuses it on the clock rather than on identity. So the ingress-as-table
    // impersonation the rest of this file uses is a faithful stand-in for code
    // running inside the table.
    let before_upgrade = w.nudge_timeout(alice, 0);
    say!(
        "  real inter-canister table_stand_down before the upgrade -> {}",
        first_words(&before_upgrade.err_text())
    );
    assert!(
        before_upgrade.err_text().contains("action clock"),
        "the dealer refused on the CLOCK, i.e. it accepted the caller"
    );

    // The operator replaces the table's code WHILE THE HAND IS LIVE.
    w.pic
        .upgrade_canister(w.table, w.table_wasm.clone(), vec![], Some(w.operator))
        .expect("the operator can always upgrade the TABLE");
    say!("  install_code --mode upgrade on the table, mid-hand: ACCEPTED");

    // And the sealed dealer still takes its word for everything.
    w.everyone_acks(hand_id);
    let flop = w.advance_street_direct(w.table, hand_id).unwrap();
    say!(
        "  the dealer still answers advance_street from that principal: {} cards",
        flop.len()
    );
    assert_eq!(flop.len(), 3);
    say!(
        "  CONFIRMED: the dealer's entire authorisation model is one principal that the \
         operator can point at arbitrary code, mid-hand, with no notice to anyone."
    );

    // SIDE EFFECT worth its own line: the upgrade wiped the table's hand state
    // while the DEALER's hand stayed live. Nobody can finish it, and the dealer
    // refuses to open another until it is finished.
    let health = w.dealer_health();
    say!(
        "  after the upgrade: dealer live_hand = {:?}, can_open_hand = {}",
        health.live_hand,
        health.can_open_hand
    );
    assert_eq!(health.live_hand, Some(hand_id));
    assert!(!health.can_open_hand);
    let blocked = w.open_hand_direct(
        w.table,
        vec![
            DealtInSeat { seat: 0, principal: w.player(0) },
            DealtInSeat { seat: 1, principal: w.player(1) },
        ],
    );
    say!("  a fresh hand -> {}", blocked.err_text().trim());
    assert!(!blocked.is_ok());
    say!(
        "  so a routine mid-hand table upgrade halts the whole table until the \
         one-hour last-resort door opens."
    );
}

fn first_words(s: &str) -> String {
    s.split_whitespace().take(14).collect::<Vec<_>>().join(" ")
}

// ===========================================================================
// ATTACK 10 — the commonest hand in poker bricks the dealer for an hour
// ===========================================================================

/// Most poker hands end in a fold-out, not a showdown.
///
/// `act(Fold)` is a TABLE call. It tells the dealer nothing. `finish_hand` then
/// refuses to publish the seed because the folder never called `stand_down` on
/// the dealer in its own name — which is the gate that makes attack 2 cost the
/// operator four minutes, so it cannot simply be removed.
///
/// The table swallows that refusal into `last_error` and marks itself Complete.
/// The dealer's hand stays live forever, and `open_hand` refuses while any hand
/// is live. The table is dead until the one-hour last-resort door.
#[test]
fn an_ordinary_fold_out_leaves_the_dealer_stuck_and_the_table_cannot_deal_again() {
    banner("ATTACK 10 — a fold-out bricks the table for an hour");
    let w = World::new(&["alice", "bob"]);
    let (alice, bob) = (w.player(0), w.player(1));
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();

    // Alice folds preflop. Nothing exotic: the single most common way a hand ends.
    w.act(alice, Action::Fold).unwrap();
    w.try_advance(bob).unwrap();

    let v = w.view();
    say!("  table phase {:?}, winners {:?}", v.phase, v.winners.len());
    say!("  table last_error = {:?}", v.last_error);
    say!("  table revealed_seed = {:?}", v.revealed_seed);
    assert_eq!(v.phase, Phase::Complete, "the table thinks it is done");
    assert!(
        v.last_error.is_some(),
        "the seed was not published and the table only recorded it"
    );
    assert!(v.revealed_seed.is_none());

    // THE DEALER DOES NOT AGREE.
    let health = w.dealer_health();
    say!(
        "  dealer: live_hand = {:?}, can_open_hand = {}",
        health.live_hand,
        health.can_open_hand
    );
    assert_eq!(health.live_hand, Some(hand_id));
    assert!(!health.can_open_hand);

    // So the next hand never starts.
    let next = w.start_hand(bob);
    say!("  start_hand again -> {}", next.err_text().trim());
    assert!(!next.is_ok(), "a second hand must be impossible here");

    // The shuffle of the hand just played is also unverifiable: no seed was ever
    // published, so the one property five auditors checked does not hold for it.
    let public = w.hand_public(Principal::anonymous(), hand_id).unwrap();
    assert!(public.revealed_seed.is_none());
    say!("  and hand {hand_id}'s seed is still unpublished, so that hand is not verifiable.");

    // It clears only on the last-resort clock, an hour later, and clearing it
    // publishes every hole card of a hand that had NO showdown — including the
    // folder's muck.
    w.pic.advance_time(Duration::from_secs(3_601));
    w.pic.tick();
    let forced = w.force_finalize(Principal::anonymous(), hand_id).unwrap();
    say!(
        "  +1 h: force_finalize unsticks it, and publishes {} seats' hole cards for a hand \
         that never reached a showdown:",
        forced.hole_cards.len()
    );
    for sc in &forced.hole_cards {
        say!("    seat {} : {:?} {:?}", sc.seat, sc.cards.0, sc.cards.1);
    }
    assert_eq!(forced.hole_cards.len(), 2);
    assert!(w.dealer_health().can_open_hand);
    say!("  DOWNTIME PER FOLD-OUT: {} s.", 3_601);
}

// ===========================================================================
// ATTACK 11 — the other read surfaces the brief names
// ===========================================================================

/// Canister logs, and the management-canister doors, against the SEALED dealer.
#[test]
fn logs_and_management_doors_on_the_sealed_dealer() {
    banner("ATTACK 11 — logs, snapshots, upgrades against a zero-controller canister");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, _b, _c) = seated3(&w, 1_000);
    let hand_id = w.start_hand(alice).unwrap();
    w.everyone_acks(hand_id);
    w.advance_street_direct(w.table, hand_id).unwrap();

    // Logs.
    let logs = w.pic.fetch_canister_logs(w.dealer, Principal::anonymous());
    match &logs {
        Ok(records) => {
            say!("  fetch_canister_logs(dealer) as anonymous -> {} records", records.len());
            for r in records {
                say!("    {}", String::from_utf8_lossy(&r.content));
            }
            assert!(
                records.is_empty(),
                "the sealed dealer wrote a log line; it must write none"
            );
        }
        Err(e) => say!("  fetch_canister_logs(dealer) as anonymous -> refused: {}", e.reject_message),
    }
    let logs_op = w.pic.fetch_canister_logs(w.dealer, w.operator);
    match &logs_op {
        Ok(r) => {
            say!("  fetch_canister_logs(dealer) as the OPERATOR -> {} records", r.len());
            assert!(r.is_empty());
        }
        Err(e) => say!("  as the OPERATOR -> refused: {}", e.reject_message),
    }

    // The management doors, re-measured here rather than cited.
    let snap = w.pic.take_canister_snapshot(w.dealer, Some(w.operator), None);
    say!(
        "  take_canister_snapshot(dealer) as the operator -> {}",
        match &snap {
            Ok(_) => "ALLOWED (the seal is broken)".to_string(),
            Err(e) => format!("refused: {}", first_words(&e.reject_message)),
        }
    );
    assert!(snap.is_err(), "a snapshot of the dealer must be impossible");

    let up = w
        .pic
        .upgrade_canister(w.dealer, w.dealer_wasm.clone(), vec![], Some(w.operator));
    say!(
        "  upgrade_canister(dealer) as the operator -> {}",
        match &up {
            Ok(_) => "ALLOWED (the seal is broken)".to_string(),
            Err(e) => format!("refused: {}", first_words(&e.reject_message)),
        }
    );
    assert!(up.is_err(), "the dealer must not be upgradable");

    let re = w.pic.set_controllers(w.dealer, Some(w.operator), vec![w.operator]);
    say!(
        "  set_controllers(dealer, [operator]) -> {}",
        match &re {
            Ok(_) => "ALLOWED (the seal is broken)".to_string(),
            Err(e) => format!("refused: {}", first_words(&e.reject_message)),
        }
    );
    assert!(re.is_err(), "the seal must be one-way");
    say!("  CONFIRMED: the seal itself holds. Everything found in this file goes around it.");
}
