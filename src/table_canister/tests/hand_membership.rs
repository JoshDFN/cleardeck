//! WHO IS IN THE HAND: the predicate table, asserted rather than described.
//!
//! # Why this file exists
//!
//! docs/SECURITY-FINDINGS.md FINDING 17 / docs/DEFECTS.md E-36. Two functions in
//! `src/table_canister/src/lib.rs` answered "is this seat in the hand":
//!
//! ```ignore
//! is_in_hand(p)  = !p.has_folded && (p.hole_cards.is_some() || p.status == Active)
//! live_claims(s) = seats where !has_folded && hole_cards.is_some()
//! ```
//!
//! A seat that was `Active` and held NO CARDS was in the first set and not the
//! second. `count_active_players` counts the first, `count_active_players(s) == 1`
//! calls `end_hand_single_winner`, and the one seat it counted could be the
//! cardless one -- at which point `live_claims` was empty, `plan_payouts` took its
//! no-claimant branch, and every stake went back to its funder including the
//! players who had FOLDED. Measured on the real canister: a 52,000,000 e8 pot,
//! three seats, all three ending on exactly their buy-in, the fold-out winner paid
//! nothing. Conservation exact, no trap, no `CRITICAL:` line, M1 through M9 silent.
//!
//! **The source comment beside the disjunct claimed the two predicates agreed.** It
//! was false, and it is the sentence that stopped anyone looking. So the claim is
//! made here instead, where it is executed:
//!
//!   * [`predicate_table`] prints and asserts the whole table -- every reachable
//!     `Player` shape against every membership predicate the engine exposes;
//!   * [`is_in_hand_and_live_claims_are_one_predicate`] asserts the equality that
//!     FINDING 17 was a counterexample to, over every combination of seat shapes at
//!     a six-handed table;
//!   * the rest drive the specific sequences.
//!
//! These are host tests over pure functions: no replica, no ledger, milliseconds.
//! The same properties are asserted against the real canister by
//! `tests/money_safety/tests/wave6_coherence.rs::probe4` and, on every hand of
//! every fuzz run, by M11 OUTCOME.

use candid::Principal;
use table_canister::{
    advance_game, can_still_act, count_active_players, is_in_hand, live_claims, plan_payouts, Card,
    Currency, GamePhase, PayoutReason, Player, PlayerStatus, Rank, Suit, TableConfig, TableState,
};

const SEC: u64 = 1_000_000_000;

fn seat_principal(seat: u8) -> Principal {
    Principal::from_slice(&[0xC0, 0xFF, 0xEE, seat])
}

fn config() -> TableConfig {
    TableConfig {
        small_blind: 10,
        big_blind: 20,
        min_buy_in: 40,
        max_buy_in: 100_000,
        max_players: 6,
        action_timeout_secs: 30,
        ante: 0,
        time_bank_secs: 30,
        currency: Currency::ICP,
    }
}

fn cards(seat: u8) -> (Card, Card) {
    // Distinct, legal, and never a duplicate across seats.
    let ranks = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
    ];
    (
        Card {
            rank: ranks[seat as usize % ranks.len()],
            suit: Suit::Clubs,
        },
        Card {
            rank: ranks[seat as usize % ranks.len()],
            suit: Suit::Diamonds,
        },
    )
}

/// A seat, described by the five fields every membership predicate reads.
#[derive(Clone, Debug)]
struct Shape {
    folded: bool,
    holds_cards: bool,
    status: PlayerStatus,
    all_in: bool,
    chips: u64,
}

impl Shape {
    fn label(&self) -> String {
        format!(
            "folded={:<5} cards={:<5} status={:<12} all_in={:<5} chips={:<4}",
            self.folded,
            self.holds_cards,
            format!("{:?}", self.status),
            self.all_in,
            self.chips
        )
    }
}

fn every_shape() -> Vec<Shape> {
    let mut out = Vec::new();
    for folded in [false, true] {
        for holds_cards in [false, true] {
            for status in [
                PlayerStatus::Active,
                PlayerStatus::SittingOut,
                PlayerStatus::Disconnected,
            ] {
                for all_in in [false, true] {
                    for chips in [0u64, 500] {
                        out.push(Shape {
                            folded,
                            holds_cards,
                            status: status.clone(),
                            all_in,
                            chips,
                        });
                    }
                }
            }
        }
    }
    out
}

fn player_of(seat: u8, s: Shape) -> Player {
    Player {
        principal: seat_principal(seat),
        seat,
        chips: s.chips,
        hole_cards: if s.holds_cards { Some(cards(seat)) } else { None },
        current_bet: 0,
        total_bet_this_hand: 0,
        has_folded: s.folded,
        has_acted_this_round: true,
        is_all_in: s.all_in,
        status: s.status,
        last_seen: 0,
        timeout_count: 0,
        time_bank_remaining: 30,
        is_sitting_out_next_hand: false,
        broke_at: None,
        sitting_out_since: None,
    }
}

fn table(players: Vec<Option<Player>>) -> TableState {
    TableState {
        id: 1,
        config: config(),
        players,
        community_cards: Vec::new(),
        deck: Vec::new(),
        deck_index: 0,
        pot: 0,
        side_pots: Vec::new(),
        current_bet: 0,
        min_raise: 20,
        phase: GamePhase::PreFlop,
        dealer_seat: 0,
        small_blind_seat: 0,
        big_blind_seat: 1,
        action_on: 0,
        action_timer: None,
        last_action: None,
        shuffle_proof: None,
        hand_number: 1,
        last_aggressor: None,
        bb_has_option: false,
        first_hand: false,
        auto_deal_at: None,
        departed_stakes: None,
    }
}

// ===========================================================================
// THE TABLE
// ===========================================================================

/// **THE PREDICATE TABLE.** Every reachable `Player` shape against every
/// membership predicate, with the two that must agree asserted to agree.
///
/// Run it with `--nocapture` to read the table itself; the assertions below are
/// what makes it a gate rather than a printout.
///
/// The columns:
///
/// | predicate            | question                                        |
/// |----------------------|-------------------------------------------------|
/// | `is_in_hand`         | in THIS hand: may win it, counted for the       |
/// |                      | fold-out, owed a turn                            |
/// | `can_still_act`      | still owed an ACTION this street                 |
/// | `live_claims`        | who may be PAID                                  |
///
/// `deals_in_this_hand` (`status == Active`) and `will_be_dealt_in`
/// (`status == Active && chips > 0`) are private to the engine because nothing
/// outside `start_new_hand` and the between-hands bookkeeping may ask them. They
/// are covered by [`the_between_hands_predicates_say_nothing_about_a_live_hand`].
#[test]
fn predicate_table() {
    println!(
        "\n{:<64} | {:<10} | {:<13} | {}",
        "SEAT SHAPE", "is_in_hand", "can_still_act", "in live_claims"
    );
    println!("{}", "-".repeat(64 + 3 + 10 + 3 + 13 + 3 + 14));

    let mut disagreements: Vec<String> = Vec::new();
    for shape in every_shape() {
        let p = player_of(0, shape.clone());
        let state = table(vec![Some(p.clone())]);

        let in_hand = is_in_hand(&p);
        let can_act = can_still_act(&p);
        let claims = live_claims(&state);
        let payable = claims.iter().any(|(s, _, _)| *s == 0);

        println!(
            "{:<64} | {:<10} | {:<13} | {}",
            shape.label(),
            in_hand,
            can_act,
            payable
        );

        // THE EQUALITY FINDING 17 WAS A COUNTEREXAMPLE TO.
        if in_hand != payable {
            disagreements.push(format!(
                "{}  is_in_hand={in_hand} but live_claims says payable={payable}",
                shape.label()
            ));
        }

        // The rule, restated independently of the engine.
        assert_eq!(
            in_hand,
            !shape.folded && shape.holds_cards,
            "is_in_hand must be exactly `dealt into this hand and has not given the claim up`: {}",
            shape.label()
        );
        assert_eq!(
            can_act,
            in_hand && !shape.all_in,
            "can_still_act must be exactly `is_in_hand and not all-in`: {}",
            shape.label()
        );

        // `count_active_players` must be `live_claims(...).len()`.
        assert_eq!(
            count_active_players(&state),
            claims.len(),
            "count_active_players is what triggers the fold-out settlement and live_claims is \
             what can be PAID by it: if they ever disagree the engine settles a hand nobody can \
             be paid out of. {}",
            shape.label()
        );
    }

    assert!(
        disagreements.is_empty(),
        "PARTICIPATION AND ELIGIBILITY DISAGREE at {} seat shape(s). That is \
         docs/SECURITY-FINDINGS.md FINDING 17 exactly: the seat below is counted as being in the \
         hand and cannot be paid out of it, so a hand can settle on it with no live claim and \
         every stake is refunded, including the folders'.\n  {}",
        disagreements.len(),
        disagreements.join("\n  ")
    );
}

/// The same equality over a whole table rather than one seat, across every
/// combination of the shapes that differ for these predicates.
///
/// One seat at a time cannot see the defect that matters: FINDING 17 needed a
/// cardless seat sitting BESIDE a card-holder, so that the count stayed above one
/// while only one claim was live.
#[test]
fn is_in_hand_and_live_claims_are_one_predicate() {
    // The four shapes that are distinguishable to these predicates.
    let interesting = [
        Shape { folded: false, holds_cards: true, status: PlayerStatus::Active, all_in: false, chips: 500 },
        Shape { folded: true, holds_cards: true, status: PlayerStatus::Active, all_in: false, chips: 500 },
        // THE FINDING 17 SEAT: Active, no cards.
        Shape { folded: false, holds_cards: false, status: PlayerStatus::Active, all_in: false, chips: 500 },
        Shape { folded: false, holds_cards: true, status: PlayerStatus::Disconnected, all_in: false, chips: 500 },
    ];

    let n = interesting.len();
    let mut checked = 0usize;
    // Every assignment of those four shapes to six seats would be 4096 tables;
    // three seats is 64 and is enough to put any pair beside any other.
    for a in 0..n {
        for b in 0..n {
            for c in 0..n {
                let state = table(vec![
                    Some(player_of(0, interesting[a].clone())),
                    Some(player_of(1, interesting[b].clone())),
                    Some(player_of(2, interesting[c].clone())),
                    None,
                    None,
                    None,
                ]);
                let counted: Vec<u8> = state
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.as_ref().map(is_in_hand).unwrap_or(false))
                    .map(|(i, _)| i as u8)
                    .collect();
                let payable: Vec<u8> = live_claims(&state).iter().map(|(s, _, _)| *s).collect();
                assert_eq!(
                    counted, payable,
                    "the seats counted as being in the hand and the seats that can be paid must \
                     be the SAME SEATS. shapes = [{:?}, {:?}, {:?}]",
                    interesting[a], interesting[b], interesting[c]
                );
                assert_eq!(count_active_players(&state), payable.len());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, n * n * n, "the loop must have run");
}

// ===========================================================================
// THE SEQUENCE
// ===========================================================================

/// **THE FINDING 17 SHAPE, at the predicate level.**
///
/// Two seats hold cards; a third has been made `Active` mid-hand and holds none
/// (`join_table` + `sit_in`, docs/DEFECTS.md E-36). One card-holder folds.
///
/// With the old predicate `count_active_players` returned 2 here, so
/// `end_hand_single_winner` did NOT fire, the hand ran on, and the remaining
/// card-holder was eventually folded by his own clock -- at which point the count
/// reached 1 on the CARDLESS seat and the hand settled with nothing to pay.
#[test]
fn a_cardless_seat_does_not_keep_a_decided_hand_alive() {
    let mut folded = player_of(0, Shape {
        folded: true,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    folded.total_bet_this_hand = 20;
    let mut winner = player_of(1, Shape {
        folded: false,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    winner.total_bet_this_hand = 20;
    // Seated mid-hand and sat in: Active, no cards, nothing staked.
    let arrival = player_of(2, Shape {
        folded: false,
        holds_cards: false,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 500,
    });

    let state = table(vec![Some(folded), Some(winner), Some(arrival), None, None, None]);

    assert_eq!(
        count_active_players(&state),
        1,
        "the hand is DECIDED: one seat holds cards and has not folded. A cardless arrival must \
         not keep the count above one -- that is what let FINDING 17 run the hand on until the \
         winner's own clock folded him."
    );
    let claims = live_claims(&state);
    assert_eq!(
        claims.iter().map(|(s, _, _)| *s).collect::<Vec<_>>(),
        vec![1u8],
        "and the one seat counted must be the one that can be paid"
    );
}

/// The settlement of that same state pays the winner, and does NOT take the
/// refund-everyone branch.
///
/// This is the OUTCOME assertion at the pure level: `plan_payouts` is the function
/// that chose to refund everybody, and the reason it did is that `live_claims` was
/// empty. Here it is not.
#[test]
fn the_decided_hand_pays_the_seat_that_holds_the_claim() {
    let mut folded = player_of(0, Shape {
        folded: true,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    folded.total_bet_this_hand = 20;
    let mut winner = player_of(1, Shape {
        folded: false,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    winner.total_bet_this_hand = 20;
    let arrival = player_of(2, Shape {
        folded: false,
        holds_cards: false,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 500,
    });
    let mut state = table(vec![Some(folded), Some(winner), Some(arrival), None, None, None]);
    state.pot = 40;

    let plan = plan_payouts(&state);
    assert!(plan.conserves(), "the plan must pay out what it collected");
    assert_eq!(plan.collected, 40);
    assert_eq!(
        plan.amount_for(1),
        40,
        "seat 1 held the only live claim on a 40 pot and must be paid all of it; got {:?}",
        plan.payouts
    );
    assert_eq!(plan.amount_for(0), 0, "the seat that FOLDED must be paid nothing");
    assert_eq!(plan.amount_for(2), 0, "the seat that staked nothing must be paid nothing");
    assert!(
        plan.payouts
            .iter()
            .all(|p| matches!(p.reason, PayoutReason::PotShare { .. })),
        "a decided hand is WON, not refunded. A `Refund` here is the FINDING 17 settlement: \
         every stake handed back and the hand un-played. payouts={:?}",
        plan.payouts
    );
}

/// A cardless seat is never given the action.
///
/// `find_next_active_seat` asks `can_still_act`, which now requires cards, so
/// `advance_game` can never point the clock at a seat that was not dealt in. That
/// is E-36's first half: before this, such a seat was offered the action and
/// `apply_player_action` never checked for cards either, so it could bet into a
/// hand it was never dealt into.
#[test]
fn a_cardless_seat_is_never_offered_the_action() {
    let mut a = player_of(0, Shape {
        folded: false,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    a.has_acted_this_round = false;
    let mut b = player_of(1, Shape {
        folded: false,
        holds_cards: true,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 480,
    });
    b.has_acted_this_round = false;
    let arrival = player_of(2, Shape {
        folded: false,
        holds_cards: false,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 500,
    });
    let mut state = table(vec![Some(a), Some(b), Some(arrival), None, None, None]);
    state.pot = 40;
    state.action_on = 1;

    advance_game(&mut state, 10 * SEC);

    assert_ne!(
        state.action_on, 2,
        "seat 2 holds no cards and must never be on the clock. If it is, it can be folded by that \
         clock and it can bet into a hand it was never dealt into (docs/DEFECTS.md E-36)."
    );
    assert!(
        state.phase.eq(&GamePhase::PreFlop) || !matches!(state.phase, GamePhase::WaitingForPlayers),
        "sanity: the hand did not vanish"
    );
}

/// The two between-hands predicates say NOTHING about a hand in progress.
///
/// `deals_in_this_hand` and `will_be_dealt_in` are private, so this asserts the
/// property that matters from outside: a seat can be `Active` with chips and still
/// not be in the hand, and a seat can be `Disconnected` with no chips and still be
/// in it. Anything that decides hand membership from `status` or `chips` gets both
/// of these backwards, which is docs/DEFECTS.md E-32 in one direction and
/// FINDING 17 in the other.
#[test]
fn the_between_hands_predicates_say_nothing_about_a_live_hand() {
    let active_with_chips_no_cards = player_of(0, Shape {
        folded: false,
        holds_cards: false,
        status: PlayerStatus::Active,
        all_in: false,
        chips: 500,
    });
    assert!(
        !is_in_hand(&active_with_chips_no_cards),
        "Active with chips is what makes a seat eligible for the NEXT hand. It says nothing about \
         this one, and treating it as membership is FINDING 17."
    );

    let disconnected_all_in_broke = player_of(1, Shape {
        folded: false,
        holds_cards: true,
        status: PlayerStatus::Disconnected,
        all_in: true,
        chips: 0,
    });
    assert!(
        is_in_hand(&disconnected_all_in_broke),
        "a seat that was dealt in and has not folded is in the hand however it is displayed and \
         whatever is left behind it. Dropping it is docs/DEFECTS.md E-32: it was never asked to \
         act, so the street closed without it."
    );
    assert!(
        !can_still_act(&disconnected_all_in_broke),
        "but it is all-in, so it is owed no further action"
    );
}
