//! A WHOLE HAND, dealt by a canister nobody controls, settled by a canister that
//! never held a card.
//!
//! `docs/NO-PEEKING-FEASIBILITY.md` §8 said the sealed-dealer recommendation was
//! "proven at the primitive level but not at the game level", and named the first
//! open question: **the exact split of showdown evaluation once the table no longer
//! holds hole cards, since `evaluate_hand` and the side-pot code currently read
//! them directly.**
//!
//! This file is the answer. The split is that they keep reading hole cards
//! directly; the cards arrive as an argument from `reveal_showdown` instead of out
//! of the table's own state. Nothing in `poker_core` changes.
//!
//! It also re-checks the property five external auditors already rely on and which
//! this spike was not allowed to break: **the shuffle is still reproducible by a
//! stranger from the revealed seed**, per `docs/SHUFFLE-SPEC.md`. The harness
//! re-derives the whole deck itself and compares every card the hand showed.

use no_peeking::say;
use no_peeking::world::*;
use poker_core::{create_deck, shuffle_deck, Card};
use sha2::{Digest, Sha256};

fn banner(title: &str) {
    say!("\n================================================================");
    say!("  {title}");
    say!("================================================================");
}

fn fmt(card: &Card) -> String {
    let r = match card.rank {
        poker_core::Rank::Ten => "T".to_string(),
        poker_core::Rank::Jack => "J".to_string(),
        poker_core::Rank::Queen => "Q".to_string(),
        poker_core::Rank::King => "K".to_string(),
        poker_core::Rank::Ace => "A".to_string(),
        other => (other as u8).to_string(),
    };
    let s = match card.suit {
        poker_core::Suit::Hearts => "h",
        poker_core::Suit::Diamonds => "d",
        poker_core::Suit::Clubs => "c",
        poker_core::Suit::Spades => "s",
    };
    format!("{r}{s}")
}

fn fmt_all(cards: &[Card]) -> String {
    cards.iter().map(fmt).collect::<Vec<_>>().join(" ")
}

/// The verifier a stranger would write, following `docs/SHUFFLE-SPEC.md` and
/// nothing else. It gets the seed and the `dealt_in` list, and reproduces the hand.
struct SpecVerifier {
    deck: Vec<Card>,
    p: usize,
}

impl SpecVerifier {
    fn new(revealed_seed_hex: &str, seed_hash: &str, p: usize) -> Self {
        // Step 0 of any verification: check SHA256(revealed_seed) == seed_hash, on
        // your own machine.
        let seed = hex::decode(revealed_seed_hex).expect("seed is hex");
        let mut h = Sha256::new();
        h.update(&seed);
        assert_eq!(
            hex::encode(h.finalize()),
            seed_hash,
            "the table revealed a seed it did not commit to"
        );
        let mut deck = create_deck();
        shuffle_deck(&mut deck, &seed);
        Self { deck, p }
    }
    fn hole(&self, k: usize) -> (Card, Card) {
        (self.deck[2 * k], self.deck[2 * k + 1])
    }
    fn board(&self) -> Vec<Card> {
        let p = self.p;
        vec![
            self.deck[2 * p + 1],
            self.deck[2 * p + 2],
            self.deck[2 * p + 3],
            self.deck[2 * p + 5],
            self.deck[2 * p + 7],
        ]
    }
}

// ---------------------------------------------------------------------------

/// Three seats, an all-in for less, a side pot, and a showdown. The full loop.
#[test]
fn a_three_handed_hand_with_a_side_pot_settles_from_cards_the_table_never_held() {
    banner("A WHOLE HAND — the table holds no cards and still pays the right seat");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));

    // Deliberately uneven stacks so the all-in creates a REAL side pot: carol can
    // only win what she covered.
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    w.sit(carol, 2, 120).unwrap();

    let hand_id = w.start_hand(alice).unwrap();
    say!("hand {hand_id} opened; dealer = {}", w.dealer);

    let opened = w.hand_public(alice, hand_id).expect("hand is public");
    say!("  commitment  seed_hash = {}", opened.seed_hash);
    say!(
        "  dealt_in    {:?}",
        opened
            .dealt_in
            .iter()
            .map(|d| d.seat)
            .collect::<Vec<_>>()
    );
    assert!(
        opened.revealed_seed.is_none(),
        "the seed must not be public while the hand is live"
    );
    assert!(opened.community.is_empty(), "no board before the flop");

    // Each player reads their own two cards, and only their own.
    let mut hole = Vec::new();
    for (i, p) in [alice, bob, carol].into_iter().enumerate() {
        let cards = w.my_hole_cards(p, hand_id).unwrap();
        say!("  seat {i} reads its own cards: {} {}", fmt(&cards.0), fmt(&cards.1));
        hole.push(cards);
    }
    assert!(
        w.my_hole_cards(alice, hand_id).unwrap() == hole[0],
        "a second read must give the same cards"
    );

    // ---- PREFLOP -------------------------------------------------------
    // Blinds are posted by start_hand. Action is on the seat left of the big
    // blind; three-handed with the button at seat 0 that is seat 0 again.
    let view = w.view();
    say!(
        "\npreflop  pot={} current_bet={} action_on={:?}",
        view.pot,
        view.current_bet,
        view.action_on
    );
    assert_eq!(view.phase, Phase::PreFlop);

    // Carol shoves for her whole 120 (she has already posted the big blind).
    // Alice and Bob both call, so the main pot is capped at Carol's stake and
    // everything above it forms a side pot only Alice and Bob can win.
    drive_preflop(&w, hand_id, alice, bob, carol);

    // ---- STREETS -------------------------------------------------------
    // Every street: the players say they are ready, and only then does the board
    // appear. This is the mechanism the feasibility doc left as a sketch.
    for street in ["FLOP", "TURN", "RIVER"] {
        let before = w.community(alice, hand_id);
        // The table cannot advance before the players are ready.
        let premature = w.try_advance(alice);
        say!(
            "\n{street}: table asks to advance before the acks -> {}",
            first_line(&premature.err_text())
        );
        assert!(
            !premature.is_ok(),
            "the dealer let a street be revealed with seats still to act"
        );
        assert_eq!(
            w.community(alice, hand_id),
            before,
            "a refused advance must not have moved the board"
        );

        w.everyone_acks(hand_id);
        w.try_advance(alice).unwrap();
        let board = w.community(alice, hand_id);
        say!("{street}: board = {}", fmt_all(&board));

        // PUBLIC AND SIMULTANEOUS: everybody sees the same board in the same
        // moment, including a principal that has nothing to do with the table.
        let stranger = candid::Principal::self_authenticating(b"a-total-stranger");
        assert_eq!(w.community(stranger, hand_id), board);
        assert_eq!(w.community(candid::Principal::anonymous(), hand_id), board);
        assert_eq!(w.view_as(w.operator).board, board);

        // Everyone checks it down; carol is already all in and has stood down.
        // The RIVER needs this too: its betting round is what closes before the
        // showdown, and skipping it is how a harness accidentally proves nothing.
        check_around(&w, alice, bob);
    }

    // ---- SHOWDOWN ------------------------------------------------------
    w.everyone_acks(hand_id);
    let phase = w.try_advance(alice).unwrap();
    say!("\nshowdown reached, phase now {phase:?}");

    let view = w.view();
    assert!(
        !view.showdown.is_empty(),
        "the showdown must have published cards"
    );
    for sc in &view.showdown {
        say!(
            "  seat {} shows {} {}",
            sc.seat,
            fmt(&sc.cards.0),
            fmt(&sc.cards.1)
        );
    }
    say!("  side pots  {:?}", view.side_pots);
    for win in &view.winners {
        say!("  WINNER seat {} takes {} ({:?})", win.seat, win.amount, win.rank);
    }
    assert!(!view.winners.is_empty(), "somebody must win the pot");

    // The side pot exists and Carol is not eligible for it.
    assert!(
        view.side_pots.len() >= 2,
        "an all-in for less must create a side pot, got {:?}",
        view.side_pots
    );
    let top = view.side_pots.last().unwrap();
    assert!(
        !top.eligible_players.contains(&2),
        "carol was all in for 120 and cannot be eligible for the top pot {top:?}"
    );

    // ---- CONSERVATION --------------------------------------------------
    let total_after = w.chips_of(alice) + w.chips_of(bob) + w.chips_of(carol);
    say!(
        "\nchips: alice {} bob {} carol {} -> total {}",
        w.chips_of(alice),
        w.chips_of(bob),
        w.chips_of(carol),
        total_after
    );
    assert_eq!(
        total_after, 2_120,
        "chips are conserved: 1000 + 1000 + 120 in, the same out"
    );

    // ---- THE SHUFFLE IS STILL VERIFIABLE BY A STRANGER -----------------
    banner("docs/SHUFFLE-SPEC.md still applies, word for word");
    let public = w.hand_public(candid::Principal::anonymous(), hand_id).unwrap();
    let seed = public
        .revealed_seed
        .clone()
        .expect("the seed is published when the hand ends");
    say!("revealed_seed = {seed}");
    let verifier = SpecVerifier::new(&seed, &public.seed_hash, public.dealt_in.len());

    for (k, d) in public.dealt_in.iter().enumerate() {
        let expected = verifier.hole(k);
        let actual = hole[k];
        say!(
            "  dealt_in[{k}] seat {} : dealer gave {} {}, the SPEC gives {} {}",
            d.seat,
            fmt(&actual.0),
            fmt(&actual.1),
            fmt(&expected.0),
            fmt(&expected.1)
        );
        assert_eq!(
            actual, expected,
            "the cards a player was dealt must follow from the published seed"
        );
    }
    let board = verifier.board();
    say!("  board       : dealer showed {}", fmt_all(&view.board));
    say!("  board       : the SPEC gives {}", fmt_all(&board));
    assert_eq!(
        view.board, board,
        "the board must follow from the published seed"
    );

    // And every showdown card the table settled from is the card the seed says.
    for sc in &view.showdown {
        let k = public
            .dealt_in
            .iter()
            .position(|d| d.seat == sc.seat)
            .expect("a showdown seat was dealt in");
        assert_eq!(
            sc.cards,
            verifier.hole(k),
            "seat {} showed a card that does not follow from the seed",
            sc.seat
        );
    }
    say!("\n  OK: every card in this hand follows from the seed, and the seed matches\n      the commitment published before any card was shown.");
}

/// A hand that ends by everyone folding shows NO cards at all, and the seed is
/// still published so the hand stays verifiable. This is the fold-out path, and it
/// is the one that would let a hostile table publish the seed early if the gate on
/// `finish_hand` were wrong.
#[test]
fn a_foldout_hand_pays_the_survivor_and_shows_nobody_a_card() {
    banner("FOLD-OUT — no showdown, no cards shown, seed still published");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    w.sit(carol, 2, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();

    let before: Vec<u64> = [alice, bob, carol].iter().map(|p| w.chips_of(*p)).collect();
    say!("chips before: {before:?}  pot={}", w.view().pot);

    // Everyone folds to one player. Each folder tells the DEALER itself, in its own
    // name — the table cannot do it for them.
    let order = [(alice, 0u8), (bob, 1u8)];
    for (who, seat) in order {
        w.act(who, Action::Fold).unwrap();
        w.stand_down(who, hand_id).unwrap();
        say!("  seat {seat} folded and stood down");
    }
    // The survivor also stands down: the hand is over for them too.
    w.stand_down(carol, hand_id).unwrap();

    let phase = w.try_advance(alice).unwrap();
    say!("phase now {phase:?}");

    let view = w.view();
    assert!(
        view.showdown.is_empty(),
        "a fold-out must show nobody a card, got {:?}",
        view.showdown
    );
    assert!(
        view.board.is_empty(),
        "a fold-out preflop must not put a board out, got {:?}",
        view.board
    );
    say!("winners: {:?}", view.winners.iter().map(|w| (w.seat, w.amount)).collect::<Vec<_>>());
    assert!(!view.winners.is_empty(), "the survivor must be paid");
    assert!(
        view.winners.iter().all(|w| w.seat == 2),
        "carol was the only seat left, so every pot is hers: {:?}",
        view.winners
    );
    assert_eq!(
        view.winners.iter().map(|w| w.amount).sum::<u64>(),
        30,
        "the survivor takes the whole 30-chip pot, in however many layers it formed"
    );

    let total: u64 = [alice, bob, carol].iter().map(|p| w.chips_of(*p)).sum();
    assert_eq!(total, 3_000, "chips are conserved through a fold-out");

    // Verifiability survives a hand nobody showed down.
    let public = w.hand_public(candid::Principal::anonymous(), hand_id).unwrap();
    let seed = public.revealed_seed.clone().expect("seed published");
    let verifier = SpecVerifier::new(&seed, &public.seed_hash, public.dealt_in.len());
    say!(
        "after the hand a stranger can reconstruct seat 0's folded cards: {} {}",
        fmt(&verifier.hole(0).0),
        fmt(&verifier.hole(0).1)
    );
    say!(
        "  (that is UNCHANGED behaviour and is stated in docs/SHUFFLE-SPEC.md: the\n   seed opens every card once the hand is dead. This spike protects a LIVE hand.\n   Post-hand mucking privacy is Option C, vetKD for the archive, and is a\n   different canister's problem.)"
    );
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

/// Build a REAL side pot: the two deep stacks put in 300 each, the short stack can
/// only cover 120. The pot therefore layers into a main pot all three can win and a
/// top pot only the two deep seats are eligible for — which is the case that used
/// to destroy money in this engine (DEFECTS E-01: stale `side_pots` used as the
/// payout basis) and is the case a settlement path must be shown to survive.
fn drive_preflop(
    w: &World,
    hand_id: u64,
    alice: candid::Principal,
    bob: candid::Principal,
    carol: candid::Principal,
) {
    // Seats: 0 alice (button, posts nothing), 1 bob (small blind 10),
    // 2 carol (big blind 20, and only 100 chips behind).
    w.act(alice, Action::RaiseTo(300)).unwrap();
    say!("  seat 0 raises to 300");
    w.act(bob, Action::Call).unwrap();
    say!("  seat 1 calls 300");
    w.act(carol, Action::AllIn).unwrap();
    say!("  seat 2 is ALL IN for 120, which is less than the 300 in front of her");
    assert!(
        w.query_round_closed(),
        "the preflop round should be closed: two seats matched at 300 and the third is all in"
    );

    // Carol is all in; she stands down so the table stops waiting for her on every
    // later street. She does it HERSELF — the dealer accepts nothing else without a
    // full action clock of measured silence.
    w.stand_down(carol, hand_id).unwrap();
    say!("  seat 2 stood down (all in, nothing left to decide)");
}

fn check_around(w: &World, alice: candid::Principal, bob: candid::Principal) {
    for _ in 0..6 {
        if w.query_round_closed() {
            break;
        }
        let Some(seat) = w.view().action_on else { break };
        let who = if seat == 0 { alice } else { bob };
        if !w.act(who, Action::Check).is_ok() {
            break;
        }
    }
}
