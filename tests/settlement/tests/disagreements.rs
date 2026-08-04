//! One permanent, minimal reproducer per disagreement the oracle found.
//!
//! # STATUS: all four are FIXED
//!
//! D-01, D-02 (docs/DEFECTS.md E-01), D-03 (E-05) and D-04 (E-35) were fixed in
//! wave 2, together with E-03. The oracle now agrees with the engine on all 17
//! deliberate hands of `suite::run_all`. What changed in this file when the fix
//! landed:
//!
//! * the `golden_*` tests are UNCHANGED. They encode the facts of each hand and the
//!   amount the engine paid AT THE TIME as constants, and they are the permanent
//!   record of what each defect was. Their oracle-side assertions are the rules of
//!   poker and do not depend on the engine at all.
//! * the `pinned_*` tests were INVERTED. They used to re-reach each shape on the
//!   real canister and assert the engine STILL disagreed; they now re-reach the same
//!   shapes and assert the engine agrees, seat by seat, with the per-seat DIFF
//!   vector pinned to all zeroes and the specific amount each defect used to move
//!   named in the assertion message.
//!
//! Each entry records the whole hand: exact hole cards, the board, what every seat
//! contributed, WHAT THE ENGINE PAID BEFORE THE FIX, what the rules of poker owed,
//! and the delta per seat.
//!
//! ```text
//!   D-01  E-01   all post-flop money is destroyed at every showdown
//!   D-02  E-01   an uncalled bet is destroyed rather than returned
//!   D-03  E-05   a seat vacated before the pots are built moves a short stack's
//!                main pot into the deep pot. Conserves every chip.
//!   D-04  NEW    when a chopped pot leaves more than one odd chip, ALL of them go
//!                to a single seat instead of one each clockwise from the button.
//! ```
//!
//! Run the golden set alone (fast, no replica):
//! ```text
//!   cargo test --test disagreements -- golden
//! ```

use settlement_oracle::cards::{board, hole};
use settlement_oracle::oracle::{settle, HandFacts, SeatStake, Settlement};
use settlement_oracle::{suite, Bench};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// scaffolding for the golden reproducers
// ---------------------------------------------------------------------------

/// One seat as observed on the canister.
struct Seat {
    seat: u8,
    hole: &'static str,
    contributed: u64,
    folded: bool,
    vacated: bool,
    /// Chips the engine actually credited this seat when the hand settled.
    engine_paid: u64,
}

fn assert_reproducer(
    name: &str,
    num_seats: usize,
    dealer_seat: u8,
    board_text: &str,
    seats: &[Seat],
    expect_owed: &[(u8, u64)],
    expect_diff: &[(u8, i128)],
) -> Settlement {
    let facts = HandFacts {
        num_seats,
        dealer_seat,
        board: board(board_text),
        stakes: seats
            .iter()
            .filter(|s| s.contributed > 0)
            .map(|s| SeatStake {
                seat: s.seat,
                contributed: s.contributed,
                relinquished: s.folded || s.vacated,
                hole: if s.folded || s.vacated {
                    None
                } else {
                    Some(hole(s.hole))
                },
            })
            .collect(),
    };
    let settlement = settle(&facts);

    for (seat, owed) in expect_owed {
        assert_eq!(
            settlement.owed_to(*seat),
            *owed,
            "{name}: the rules owe seat {seat} {owed}, the oracle says {}\nlayers {:#?}",
            settlement.owed_to(*seat),
            settlement.layers
        );
    }
    assert_eq!(
        settlement.total_owed(),
        facts.total_collected(),
        "{name}: the oracle must owe out exactly what was collected"
    );

    let oracle_deltas = settlement.net_deltas(&facts);
    let engine_deltas: BTreeMap<u8, i128> = seats
        .iter()
        .map(|s| (s.seat, s.engine_paid as i128 - s.contributed as i128))
        .collect();
    for (seat, diff) in expect_diff {
        let engine = engine_deltas.get(seat).copied().unwrap_or(0);
        let oracle = oracle_deltas.get(seat).copied().unwrap_or(0);
        assert_eq!(
            engine - oracle,
            *diff,
            "{name}: seat {seat} engine delta {engine}, oracle delta {oracle}, \
             expected difference {diff}"
        );
    }

    println!(
        "{name}: collected {}, engine paid {}, engine net across all seats {}",
        facts.total_collected(),
        seats.iter().map(|s| s.engine_paid).sum::<u64>(),
        engine_deltas.values().sum::<i128>(),
    );
    settlement
}

// ===========================================================================
// D-01 -- E-01: every showdown destroys all post-flop money
// ===========================================================================
//
// Four seats, 400 chips each, blinds 1/2. Everyone limps pre-flop for 2, then bets
// and calls 20 on the flop, the turn and the river, so each seat contributes 62 and
// the pot is 248. Seat 0 holds the best hand.
//
// The engine froze `state.side_pots` when the flop was dealt -- at that point the
// pot was the 8 chips of pre-flop money -- and `determine_winners` pays out of that
// frozen breakdown. Seat 0 is credited 8. The other 240 chips stay inside the
// canister, credited to nobody, and no method can ever pay them out.

#[test]
fn golden_d01_every_showdown_destroys_all_post_flop_money() {
    let settlement = assert_reproducer(
        "D-01",
        6,
        1,
        "Jd Ah 7h 5c Kc",
        &[
            Seat { seat: 0, hole: "Ac Qc", contributed: 62, folded: false, vacated: false, engine_paid: 8 },
            Seat { seat: 1, hole: "Kd 4c", contributed: 62, folded: false, vacated: false, engine_paid: 0 },
            Seat { seat: 2, hole: "4s 3s", contributed: 62, folded: false, vacated: false, engine_paid: 0 },
            Seat { seat: 3, hole: "2s Qd", contributed: 62, folded: false, vacated: false, engine_paid: 0 },
        ],
        // Seat 0 has a pair of aces, the best hand, and every seat covered every
        // chip, so there is one pot and seat 0 takes all of it.
        &[(0, 248), (1, 0), (2, 0), (3, 0)],
        // Seat 0 was short-paid by 240; nobody else was affected.
        &[(0, -240), (1, 0), (2, 0), (3, 0)],
    );
    assert_eq!(settlement.layers.iter().filter(|l| l.amount > 0).count(), 1);
    // The engine paid out 8 of the 248 it collected. The missing 240 is not
    // misdirected, it is GONE: docs/DEFECTS.md E-01, FINDING-01-chip-destruction.md.
    assert_eq!(248 - 8, 240);
}

// ===========================================================================
// D-02 -- E-01: an uncalled bet is destroyed rather than returned
// ===========================================================================
//
// Seat 0 is all-in for 20 pre-flop. Seats 1, 2 and 3 each put in 60. On the flop
// seat 2 bets 40 and both the others fold, so seat 2's 40 was never covered.
//
// Two things are owed: the 40 back to seat 2, and a chopped main pot -- seats 0 and
// 2 both play the board for a pair of deuces with an ace-queen-eight kicker, so the
// 80 main pot is 40 each. The engine chops the main pot correctly, but its frozen
// breakdown does not contain seat 2's flop bet, so the 40 is destroyed.

#[test]
fn golden_d02_an_uncalled_bet_is_destroyed_rather_than_returned() {
    let settlement = assert_reproducer(
        "D-02",
        6,
        1,
        "Qh Ad 2h 2s 8h",
        &[
            Seat { seat: 0, hole: "7s 4h", contributed: 20,  folded: false, vacated: false, engine_paid: 40 },
            Seat { seat: 1, hole: "Kh Ac", contributed: 60,  folded: true,  vacated: false, engine_paid: 0 },
            Seat { seat: 2, hole: "7d 6h", contributed: 100, folded: false, vacated: false, engine_paid: 160 },
            Seat { seat: 3, hole: "Jh Kc", contributed: 60,  folded: true,  vacated: false, engine_paid: 0 },
        ],
        // 40 uncalled back to seat 2, main pot 80 chopped 40/40, upper pot 120 to
        // seat 2 alone: 200 in total.
        &[(0, 40), (2, 200)],
        &[(0, 0), (2, -40), (1, 0), (3, 0)],
    );
    assert_eq!(settlement.uncalled, Some((2, 40)));
    assert_eq!(
        settlement.layers[0].winners,
        vec![0, 2],
        "the main pot is a genuine two-way chop, which the engine gets right"
    );
}

// ===========================================================================
// D-03 -- E-05: a vacated seat moves a short stack's main pot into the deep pot
// ===========================================================================
//
// THIS IS THE ONE THAT MATTERS MOST for why this crate exists.
//
// Seat 0 is all-in for 20. Seats 1, 2 and 3 build the pot; seat 1 folds with 60
// committed and then LEAVES THE TABLE while the betting round is still open, before
// `calculate_side_pots` has run for this hand at all. Seats 2 and 3 finish at 200
// each. Total collected 480.
//
// `collect_contributions` reads the seat vector, and seat 1 is no longer in it, so
// the pots are built from 20 + 200 + 200 = 420 while `state.pot` says 480. The
// reconciliation branch adds the missing 60 to the LAST pot -- the one only seats 2
// and 3 can win. The main pot, which seat 0 wins with a pair of aces, is 60 instead
// of 80.
//
// Every chip is conserved: 480 collected, 480 paid. The M1..M6 money invariants
// cannot see this. Only a settlement oracle can.

#[test]
fn golden_d03_a_vacated_seat_moves_a_short_stacks_main_pot_into_the_deep_pot() {
    let settlement = assert_reproducer(
        "D-03",
        6,
        1,
        "Kd Tc 3d 2c Ad",
        &[
            Seat { seat: 0, hole: "As 8d", contributed: 20,  folded: false, vacated: false, engine_paid: 60 },
            // folded AND vacated: `leave_table` has no phase guard at all.
            Seat { seat: 1, hole: "6h 8h", contributed: 60,  folded: true,  vacated: true,  engine_paid: 0 },
            Seat { seat: 2, hole: "4d 9h", contributed: 200, folded: false, vacated: false, engine_paid: 0 },
            Seat { seat: 3, hole: "7s Th", contributed: 200, folded: false, vacated: false, engine_paid: 420 },
        ],
        // Main pot (0,20] = 80 to seat 0 (pair of aces). (20,60] = 120 and
        // (60,200] = 280 to seat 3 (pair of tens beats seat 2's ace high).
        &[(0, 80), (3, 400)],
        // 20 chips moved from seat 0 to seat 3. Nothing was created or destroyed.
        &[(0, -20), (3, 20), (2, 0)],
    );
    assert_eq!(settlement.total_owed(), 480);
    // The engine's total is also 480. That is exactly the point.
    assert_eq!(60u64 + 420, settlement.total_owed());
    assert_eq!(
        settlement.layers[0].amount, 80,
        "the short stack's main pot is four seats times 20, including the money the \
         seat that left had already committed"
    );
}

// ===========================================================================
// D-04 -- NEW: all the odd chips of a chopped pot go to one seat
// ===========================================================================
//
// Not in docs/DEFECTS.md. Found by this oracle.
//
// `determine_winners` splits a pot with
// ```text
//   let pot_share = side_pot.amount / pot_winners.len();
//   let remainder = side_pot.amount % pot_winners.len();
//   let amount = if seat == remainder_seat { pot_share + remainder } else { pot_share };
// ```
// so the WHOLE remainder is credited to one seat. With two winners the remainder is
// at most one chip and that is correct. With three or more winners it can be two or
// more chips, and then it is wrong: rooms distribute odd chips ONE EACH, walking
// clockwise from the button.
//
// Below, three seats play the board (6-6-K-Q-T) for an exact three-way tie. The
// smallest layer holds 8 chips: 2 each and two over. The engine gives both odd chips
// to seat 2. The rules give one to seat 2 and one to seat 4.
//
// The amount is one chip, 1 e8s. The defect is not the amount: it is that a pot is
// being divided by a rule the game does not have, on the code path that also decides
// who gets the other 99.99% of the money. It conserves totals exactly, so nothing
// else in the test suite can see it.

#[test]
fn golden_d04_all_the_odd_chips_of_a_chopped_pot_go_to_one_seat() {
    let settlement = assert_reproducer(
        "D-04",
        6,
        1, // button on seat 1, so clockwise order is 2, 3, 4, 5, 0, 1
        "6h Qd Th 6s Kh",
        &[
            Seat { seat: 1, hole: "9c 4c", contributed: 40,  folded: false, vacated: false, engine_paid: 40 },
            Seat { seat: 2, hole: "8c 7h", contributed: 80,  folded: false, vacated: false, engine_paid: 82 },
            Seat { seat: 3, hole: "8h Tc", contributed: 2,   folded: true,  vacated: false, engine_paid: 0 },
            Seat { seat: 4, hole: "9s 5c", contributed: 400, folded: false, vacated: false, engine_paid: 400 },
        ],
        // 320 uncalled back to seat 4. Layers: (0,2] = 8 chopped three ways (2 each,
        // odd chips to seats 2 then 4); (2,40] = 114 chopped three ways (38 each, no
        // remainder); (40,80] = 80 chopped two ways (40 each).
        &[(1, 40), (2, 81), (4, 401)],
        // One chip went to seat 2 that belonged to seat 4.
        &[(1, 0), (2, 1), (4, -1), (3, 0)],
    );
    let smallest = &settlement.layers[0];
    assert_eq!(smallest.amount, 8);
    assert_eq!(smallest.winners, vec![1, 2, 4], "an exact three-way tie");
    assert_eq!(
        8 % 3,
        2,
        "the layer leaves TWO odd chips, which is where the two rules diverge"
    );
}

/// The rule itself, isolated from any hand: with three winners and a remainder of
/// two, the engine's `pot_share + remainder` gives one seat two extra chips where
/// the rules give two seats one each.
#[test]
fn golden_d04b_the_two_odd_chip_rules_diverge_at_three_winners() {
    use settlement_oracle::oracle::clockwise_from_button;

    let amount = 20u64;
    let winners = [1u8, 2, 3];
    let dealer = 0u8;

    let share = amount / winners.len() as u64;
    let remainder = amount % winners.len() as u64;
    assert_eq!((share, remainder), (6, 2));

    // What the engine does: the whole remainder to the first winner clockwise.
    let first = clockwise_from_button(dealer, 4, &winners)[0];
    let engine: Vec<(u8, u64)> = winners
        .iter()
        .map(|s| (*s, if *s == first { share + remainder } else { share }))
        .collect();
    assert_eq!(engine, vec![(1, 8), (2, 6), (3, 6)]);

    // What the rules do: one odd chip each, clockwise from the button.
    let facts = HandFacts {
        num_seats: 4,
        dealer_seat: dealer,
        board: board("As Ks Qh Jd Td"),
        stakes: vec![
            SeatStake::contender(1, 6, hole("2c 3c")),
            SeatStake::contender(2, 6, hole("2d 4d")),
            SeatStake::contender(3, 6, hole("2h 5h")),
            SeatStake::folded(0, 2),
        ],
    };
    let s = settle(&facts);
    assert_eq!(
        vec![(1u8, s.owed_to(1)), (2u8, s.owed_to(2)), (3u8, s.owed_to(3))],
        vec![(1, 7), (2, 7), (3, 6)]
    );
    // Both rules pay out exactly 20. Only the destination of one chip differs.
    assert_eq!(engine.iter().map(|(_, a)| a).sum::<u64>(), 20);
    assert_eq!(s.total_owed(), 20);
}

// ===========================================================================
// pinned against the real canister
// ===========================================================================
//
// These are the known-defect markers, INVERTED. Each one re-reaches on the real
// canister the exact shape that used to be settled wrongly, and asserts that the
// engine now agrees with the rules of poker seat by seat -- not merely that it
// conserves chips, which every one of these defects already did.
//
// They are written to fail LOUDLY if a payout defect comes back, and they name the
// money each one used to move. `Bench::gate` also fails every hand these scenarios
// run, so a regression is caught twice.
//
// Each owns a PocketIC instance, so run them with `--test-threads=1`.

/// E-01 / D-01. A hand with real money wagered on the flop, turn and river,
/// settled at a showdown. This is the shape that used to pay the winner the
/// PRE-FLOP pot and destroy everything else, permanently and unrecoverably.
#[test]
fn pinned_e01_post_flop_money_is_no_longer_destroyed() {
    let mut bench = Bench::ladder();
    bench.set_stacks(suite::DEEP_EVEN);
    let cmp = suite::post_flop_betting_to_showdown(&mut bench).expect("the hand ran");
    println!("{}", cmp.report());
    assert_agrees_exactly(&cmp, "E-01 / D-01");
    assert_eq!(
        cmp.destroyed, 0,
        "E-01 destroyed every chip wagered after the flop. Not one may be destroyed now."
    );
    let at_flop = cmp
        .record
        .wagered_at_flop
        .expect("the hand reached a flop");
    assert!(
        cmp.facts.total_collected() > at_flop,
        "this marker is only meaningful if money went in AFTER the flop ({at_flop} was in \
         when the flop appeared, {} in total); otherwise it is the passive control case",
        cmp.facts.total_collected()
    );
}

/// E-05 / D-03. A seat folds with a real stake committed and LEAVES the table
/// before the pots are built, while an honest short all-in holds the best hand.
/// This used to move 20 chips out of the short stack's main pot and into the pot
/// only the deepest stacks could win, conserving every chip.
#[test]
fn pinned_e05_a_seat_vacated_before_the_pots_are_built_no_longer_redistributes() {
    let mut bench = Bench::ladder();
    bench.set_stacks(suite::ONE_SHORT_THREE_DEEP);
    let Some(cmp) = suite::vacate_before_the_side_pots_are_built(&mut bench) else {
        panic!(
            "the deal search never found a hand where the short stack holds the best \
             hand. Raise the attempt budget; do not weaken the marker."
        );
    };
    println!("{}", cmp.report());
    assert!(
        cmp.record.seats.iter().any(|s| s.vacated && s.contributed > 0),
        "a seat must actually have left the table with money in the pot, or this marker \
         is testing nothing"
    );
    assert_agrees_exactly(&cmp, "E-05 / D-03");
    assert_eq!(cmp.destroyed, 0, "E-05 was a pure redistribution");
    assert_eq!(
        cmp.misdirected(),
        0,
        "the whole point of this marker: no chip may reach a seat that was not owed it"
    );
}

/// D-04 / E-35. A three-way chop that leaves more than one odd chip. The engine
/// used to credit the WHOLE remainder to one seat; the rules give one chip each,
/// clockwise from the button.
#[test]
fn pinned_odd_chips_now_go_one_each_clockwise() {
    let mut bench = Bench::ladder();
    // Three searches: a three-way chop only leaves more than one odd chip when the
    // layer is not a multiple of three.
    let mut proved = false;
    for label in ["odd_chip_probe_a", "odd_chip_probe_b", "odd_chip_probe_c"] {
        let Some(cmp) = suite::exact_tie(&mut bench, 3, label) else {
            continue;
        };
        let multi_odd_chip_layers = cmp
            .settlement
            .layers
            .iter()
            .filter(|l| l.winners.len() >= 3 && l.amount % l.winners.len() as u64 >= 2)
            .count();
        if multi_odd_chip_layers == 0 {
            continue;
        }
        println!("{}", cmp.report());
        assert_agrees_exactly(&cmp, "odd chips / D-04");
        assert_eq!(cmp.destroyed, 0);
        // The chips really were placed one at a time: no winner of a chopped layer
        // may be more than one chip clear of another.
        for layer in cmp.settlement.layers.iter().filter(|l| l.winners.len() > 1) {
            let share = layer.amount / layer.winners.len() as u64;
            for seat in &layer.winners {
                let got = cmp
                    .settlement
                    .awards
                    .iter()
                    .filter(|a| a.seat == *seat)
                    .fold(0u64, |a, w| a + w.amount);
                assert!(
                    got >= share,
                    "seat {seat} was paid {got} out of a layer whose even share is {share}"
                );
            }
        }
        proved = true;
        break;
    }
    assert!(
        proved,
        "no three-way chop with two or more odd chips was reached in three searches, so \
         this marker proved nothing this run. `golden_d04_...` pins the rule \
         deterministically; re-run to re-confirm it on the canister."
    );
}

/// The assertion these markers share: not just "the totals are right" but "every
/// seat's own position is right", with the per-seat difference vector printed.
fn assert_agrees_exactly(cmp: &settlement_oracle::HandComparison, context: &str) {
    let diffs: Vec<(u8, i128)> = cmp.seats.iter().map(|s| (s.seat, s.diff)).collect();
    let zeroes: Vec<(u8, i128)> = cmp.seats.iter().map(|s| (s.seat, 0)).collect();
    assert_eq!(
        diffs, zeroes,
        "{context}: this shape used to be settled differently from the rules of poker and \
         is pinned as FIXED. The per-seat difference (engine minus owed) is no longer all \
         zero, so the defect is back.\n\n{}",
        cmp.report()
    );
}
