//! The oracle's own rules, stated as tests a poker player can check by eye.
//!
//! No replica, no canister, no wasm: these run in milliseconds and they are what
//! makes the oracle trustworthy enough to convict the engine. Every case states the
//! cards in ordinary notation, the money, and the answer the rules of poker give.
//!
//! If you are reviewing this crate, read this file first. If any of these is wrong,
//! every finding downstream is worthless.

use settlement_oracle::cards::{board, hole};
use settlement_oracle::oracle::{clockwise_from_button, settle, Anomaly, HandFacts, SeatStake};

fn facts(num_seats: usize, dealer: u8, b: &str, stakes: Vec<SeatStake>) -> HandFacts {
    HandFacts {
        num_seats,
        dealer_seat: dealer,
        board: board(b),
        stakes,
    }
}

fn contender(seat: u8, contributed: u64, cards: &str) -> SeatStake {
    SeatStake::contender(seat, contributed, hole(cards))
}

fn folded(seat: u8, contributed: u64) -> SeatStake {
    SeatStake::folded(seat, contributed)
}

// ===========================================================================
// the basics
// ===========================================================================

#[test]
fn the_best_hand_wins_the_whole_pot() {
    // Seat 1 has trip kings, seat 0 has two pair. 100 each in, seat 1 wins 200.
    let f = facts(
        3,
        2,
        "Ks Kd 7h 3c 2s",
        vec![
            contender(0, 100, "Ah 7c"),
            contender(1, 100, "Kh 4d"),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(1), 200);
    assert_eq!(s.owed_to(0), 0);
    assert!(s.is_clean(), "{:?}", s.anomalies);
}

#[test]
fn a_folded_seat_wins_nothing_and_its_money_stays_in_the_pot() {
    // Seat 2 folded 50 in. Two live seats at 100 each. Pot 250 to the winner.
    let f = facts(
        3,
        0,
        "Ks Kd 7h 3c 2s",
        vec![
            contender(0, 100, "Ah 7c"),
            contender(1, 100, "Kh 4d"),
            folded(2, 50),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(1), 250, "the folder's 50 is dead money in the pot");
    assert_eq!(s.owed_to(2), 0);
    assert_eq!(s.total_owed(), 250);
}

#[test]
fn an_exact_tie_is_chopped() {
    // Both seats play the board: identical hands, 200 chopped two ways.
    let f = facts(
        3,
        2,
        "As Ks Qh Jd Td",
        vec![
            contender(0, 100, "2c 3c"),
            contender(1, 100, "2d 3d"),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 100);
    assert_eq!(s.owed_to(1), 100);
}

#[test]
fn a_three_way_tie_is_chopped_three_ways() {
    // All three play the same straight on the board.
    let f = facts(
        4,
        3,
        "As Ks Qh Jd Td",
        vec![
            contender(0, 100, "2c 3c"),
            contender(1, 100, "2d 4d"),
            contender(2, 100, "2h 5h"),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 100);
    assert_eq!(s.owed_to(1), 100);
    assert_eq!(s.owed_to(2), 100);
}

// ===========================================================================
// uncalled bets
// ===========================================================================

#[test]
fn an_uncalled_bet_comes_back() {
    // Seat 0 bet 300 total, seat 1 folded after putting in 100. 200 of seat 0's
    // bet was never covered, so it is returned; seat 0 then wins the 200 pot.
    let f = facts(
        2,
        0,
        "Ks Kd 7h 3c 2s",
        vec![contender(0, 300, "Ah 7c"), folded(1, 100)],
    );
    let s = settle(&f);
    assert_eq!(s.uncalled, Some((0, 200)));
    assert_eq!(s.owed_to(0), 400, "200 returned plus the 200 pot");
    assert_eq!(s.total_owed(), 400);
}

#[test]
fn the_big_blind_gets_its_own_blind_back_when_everybody_folds() {
    // Everyone folds to the big blind. SB 1, BB 2. BB is owed 3: its own 2 back
    // (1 of which was never covered) plus the small blind.
    let f = facts(
        6,
        4,
        "",
        vec![folded(5, 1), SeatStake::contender(0, 2, hole("Ah Kh"))],
    );
    let s = settle(&f);
    assert_eq!(s.uncalled, Some((0, 1)));
    assert_eq!(s.owed_to(0), 3);
    assert_eq!(s.total_owed(), 3);
    assert!(
        s.is_clean(),
        "one contender on an empty board must not be an anomaly: {:?}",
        s.anomalies
    );
}

#[test]
fn a_big_all_in_over_a_short_stack_gets_the_overbet_back() {
    // Seat 0 all-in for 50. Seat 1 shoves 500. Nobody covered 450 of it.
    let f = facts(
        2,
        1,
        "Ks Kd 7h 3c 2s",
        vec![contender(0, 50, "Kh 4d"), contender(1, 500, "Ah 7c")],
    );
    let s = settle(&f);
    assert_eq!(s.uncalled, Some((1, 450)));
    // Seat 0 has trip kings and wins the 100 main pot; seat 1 gets 450 back.
    assert_eq!(s.owed_to(0), 100);
    assert_eq!(s.owed_to(1), 450);
    assert_eq!(s.total_owed(), 550);
}

// ===========================================================================
// side pots
// ===========================================================================

#[test]
fn a_short_all_in_can_only_win_the_main_pot() {
    // The canonical case. Seat 0 all-in 50 with the best hand at the table; seats
    // 1 and 2 in for 200 each. Seat 0 can only win the 150 main pot no matter how
    // good its hand is. The 300 side pot goes to the better of the two seats that
    // covered it.
    let f = facts(
        3,
        2,
        "Ks Kd 7h 3c 2s",
        vec![
            contender(0, 50, "Kh Kc"),  // four of a kind, kings
            contender(1, 200, "7d 7c"), // sevens full of kings
            contender(2, 200, "As 3s"), // two pair
        ],
    );
    let s = settle(&f);
    assert_eq!(s.layers.len(), 2);
    assert_eq!(s.layers[0].amount, 150);
    assert_eq!(s.layers[0].eligible, vec![0, 1, 2]);
    assert_eq!(s.layers[1].amount, 300);
    assert_eq!(s.layers[1].eligible, vec![1, 2]);
    assert_eq!(s.owed_to(0), 150);
    assert_eq!(s.owed_to(1), 300);
    assert_eq!(s.owed_to(2), 0);
    assert_eq!(s.total_owed(), 450);
}

#[test]
fn three_all_in_depths_make_three_pots() {
    // 20 / 40 / 80 / 80. Layers: (0,20] = 80, (20,40] = 60, (40,80] = 80.
    let f = facts(
        4,
        3,
        "Ks 7s 2s 9d 4c",
        vec![
            contender(0, 20, "2h 2d"),  // trip deuces
            contender(1, 40, "9h 9c"),  // trip nines
            contender(2, 80, "As 3s"),  // flush
            contender(3, 80, "Kh Kc"),  // trip kings
        ],
    );
    let s = settle(&f);
    let live: Vec<_> = s.layers.iter().filter(|l| l.amount > 0).collect();
    assert_eq!(live.len(), 3);
    assert_eq!((live[0].amount, live[0].eligible.clone()), (80, vec![0, 1, 2, 3]));
    assert_eq!((live[1].amount, live[1].eligible.clone()), (60, vec![1, 2, 3]));
    assert_eq!((live[2].amount, live[2].eligible.clone()), (80, vec![2, 3]));
    // Seat 2's flush is the best hand and it covered everything, so it takes all
    // three pots.
    assert_eq!(s.owed_to(2), 220);
    assert_eq!(s.total_owed(), 220);
}

#[test]
fn dead_money_above_a_short_all_in_belongs_to_the_deeper_pot() {
    // Seat 1 put in 200 and FOLDED; seat 0 is all-in for 50 and seat 2 covers.
    // Main pot (0,50] = 150, eligible {0,2}. Upper pot (50,200] = 300, eligible
    // {2} only. This is the shape docs/DEFECTS.md E-05 corrupts.
    let f = facts(
        3,
        2,
        "Ks 7s 2s 9d 4c",
        vec![
            contender(0, 50, "Kh Kd"),
            folded(1, 200),
            contender(2, 200, "9h 9c"),
        ],
    );
    let s = settle(&f);
    let live: Vec<_> = s.layers.iter().filter(|l| l.amount > 0).collect();
    assert_eq!(live.len(), 2);
    assert_eq!((live[0].amount, live[0].eligible.clone()), (150, vec![0, 2]));
    assert_eq!((live[1].amount, live[1].eligible.clone()), (300, vec![2]));
    assert_eq!(s.owed_to(0), 150, "trip kings beats trip nines for the main pot");
    assert_eq!(s.owed_to(2), 300);
    assert_eq!(s.total_owed(), 450);
}

#[test]
fn a_chopped_side_pot_leaves_the_main_pot_alone() {
    // Seat 0 all-in 50 wins the main pot; seats 1 and 2 chop the side pot.
    let f = facts(
        3,
        2,
        "Ks Kd Qh Jd Td",
        vec![
            contender(0, 50, "Kh Kc"),  // quad kings
            contender(1, 200, "Ah 2c"), // ace-high straight
            contender(2, 200, "Ad 3c"), // the same straight
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 150);
    assert_eq!(s.owed_to(1), 150);
    assert_eq!(s.owed_to(2), 150);
    assert_eq!(s.total_owed(), 450);
}

// ===========================================================================
// odd chips
// ===========================================================================

#[test]
fn a_single_odd_chip_goes_to_the_first_seat_clockwise_from_the_button() {
    // 21 chips, two winners. 10 each and one over. Button on seat 3, so walking
    // clockwise the order is 4, 5, 0, 1, 2, 3 -- seat 4 gets the odd chip.
    let f = facts(
        6,
        3,
        "As Ks Qh Jd Td",
        vec![
            SeatStake::contender(4, 11, hole("2c 3c")),
            SeatStake::contender(1, 10, hole("2d 3d")),
        ],
    );
    let s = settle(&f);
    // Seat 4 put in one more than seat 1, so 1 chip comes back uncalled first and
    // the pot is 20, which divides. Re-do it with equal stakes to isolate the rule.
    assert_eq!(s.uncalled, Some((4, 1)));

    let f = facts(
        6,
        3,
        "As Ks Qh Jd Td",
        vec![
            SeatStake::contender(4, 10, hole("2c 3c")),
            SeatStake::contender(1, 10, hole("2d 3d")),
            SeatStake::folded(0, 1),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.total_collected, 21);
    assert_eq!(
        s.owed_to(4),
        11,
        "seat 4 is the first winner clockwise from the button on seat 3"
    );
    assert_eq!(s.owed_to(1), 10);
}

#[test]
fn two_odd_chips_go_one_each_clockwise_not_both_to_one_seat() {
    // 20 chips, three winners: 6 each and 2 over. Button on seat 0, so the
    // clockwise order among {1,2,3} is 1, 2, 3: seats 1 and 2 take a chip each.
    //
    // This is the case where the rule real rooms use and the engine's
    // `pot_share + remainder` differ. The oracle states the room rule.
    let f = facts(
        4,
        0,
        "As Ks Qh Jd Td",
        vec![
            SeatStake::contender(1, 6, hole("2c 3c")),
            SeatStake::contender(2, 6, hole("2d 4d")),
            SeatStake::contender(3, 6, hole("2h 5h")),
            SeatStake::folded(0, 2),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.total_collected, 20);
    assert_eq!(s.owed_to(1), 7);
    assert_eq!(s.owed_to(2), 7);
    assert_eq!(s.owed_to(3), 6);
    assert_eq!(s.total_owed(), 20);
}

#[test]
fn the_clockwise_walk_wraps_around_the_table() {
    assert_eq!(clockwise_from_button(3, 6, &[1, 2, 4, 5]), vec![4, 5, 1, 2]);
    assert_eq!(clockwise_from_button(5, 6, &[0, 3]), vec![0, 3]);
    assert_eq!(clockwise_from_button(0, 6, &[0, 1, 5]), vec![1, 5, 0]);
    // A single winner takes the whole thing regardless of position.
    assert_eq!(clockwise_from_button(2, 6, &[4]), vec![4]);
}

// ===========================================================================
// conservation, and the shapes the rules do not define
// ===========================================================================

#[test]
fn every_chip_collected_is_always_owed_to_somebody() {
    // A deliberately awkward mix: two all-in depths, a folder above the short
    // stack, an uncalled overbet and a two-way chop, all in one hand.
    let f = facts(
        6,
        5,
        "Ks Kd Qh Jd Td",
        vec![
            contender(0, 7, "Kh Kc"),
            folded(1, 33),
            contender(2, 90, "Ah 2c"),
            contender(3, 90, "Ad 3c"),
            folded(4, 5),
            contender(5, 130, "9h 8h"),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.total_owed(), s.total_collected);
    assert_eq!(s.total_collected, 7 + 33 + 90 + 90 + 5 + 130);
    assert_eq!(s.uncalled, Some((5, 40)));
}

#[test]
fn a_pot_layer_nobody_can_win_is_returned_and_flagged() {
    // Impossible under legal betting, reachable through engine defects: seat 0 is
    // all-in for 50 and the only live seat, while seats 1 and 2 both folded having
    // matched each other at 200. The 300 above seat 0's reach belongs to nobody.
    let f = facts(
        3,
        2,
        "Ks 7s 2s 9d 4c",
        vec![
            contender(0, 50, "Kh Kd"),
            folded(1, 200),
            folded(2, 200),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 150);
    assert_eq!(s.owed_to(1), 150);
    assert_eq!(s.owed_to(2), 150);
    assert_eq!(s.total_owed(), 450, "nothing is destroyed");
    assert!(
        s.anomalies
            .iter()
            .any(|a| matches!(a, Anomaly::OrphanLayer { .. })),
        "the orphan layer must be flagged, not silently absorbed: {:?}",
        s.anomalies
    );
}

#[test]
fn a_live_seat_with_no_cards_is_flagged_and_wins_nothing() {
    let f = facts(
        3,
        2,
        "Ks 7s 2s 9d 4c",
        vec![
            SeatStake {
                seat: 0,
                contributed: 100,
                relinquished: false,
                hole: None,
            },
            contender(1, 100, "Kh Kd"),
        ],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 0);
    assert_eq!(s.owed_to(1), 200);
    assert!(s
        .anomalies
        .iter()
        .any(|a| matches!(a, Anomaly::ContenderWithoutCards { seat: 0 })));
}

#[test]
fn two_contenders_on_a_short_board_is_flagged() {
    let f = facts(
        3,
        2,
        "Ks 7s 2s",
        vec![contender(0, 100, "Kh Kd"), contender(1, 100, "As 3s")],
    );
    let s = settle(&f);
    // It still settles -- three cards is enough to rank -- but a reader is told.
    assert_eq!(s.total_owed(), 200);
    assert!(s
        .anomalies
        .iter()
        .any(|a| matches!(a, Anomaly::ShortBoardAtShowdown { .. })));
}

#[test]
fn a_seat_that_left_the_table_relinquishes_its_claim() {
    // Same money as `dead_money_above_a_short_all_in`, but seat 1 vacated rather
    // than folded. The rules answer identically: no claim, money stays in.
    let vacated = SeatStake {
        seat: 1,
        contributed: 200,
        relinquished: true,
        hole: None,
    };
    let f = facts(
        3,
        2,
        "Ks 7s 2s 9d 4c",
        vec![contender(0, 50, "Kh Kd"), vacated, contender(2, 200, "9h 9c")],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(0), 150);
    assert_eq!(s.owed_to(2), 300);
    assert_eq!(s.total_owed(), 450);
    assert!(s.is_clean(), "{:?}", s.anomalies);
}

#[test]
fn nothing_in_the_pot_owes_nothing() {
    let f = facts(3, 0, "", vec![]);
    let s = settle(&f);
    assert_eq!(s.total_owed(), 0);
    assert_eq!(s.total_collected, 0);
}

// ===========================================================================
// the oracle must not agree with a wrong-seat payout
// ===========================================================================

#[test]
fn the_oracle_would_not_agree_with_the_worst_hand_winning() {
    // A sanity check ON THE ORACLE: if it awarded the pot to the worst hand, this
    // is the case that would catch it. Trip kings must beat two pair, and the
    // oracle must say so with money.
    let f = facts(
        2,
        1,
        "Ks Kd 7h 3c 2s",
        vec![contender(0, 100, "Ah 7c"), contender(1, 100, "Kh 4d")],
    );
    let s = settle(&f);
    assert_eq!(s.owed_to(1), 200);
    assert_eq!(s.owed_to(0), 0);
    let deltas = s.net_deltas(&f);
    assert_eq!(deltas[&0], -100);
    assert_eq!(deltas[&1], 100);
    // And note the two deltas sum to zero: a wrong-seat payout conserves totals.
    // That is exactly why this oracle has to exist.
    assert_eq!(deltas[&0] + deltas[&1], 0);
}

// ===========================================================================
// THE BUILD IS PINNED, NOT INHERITED (docs/DEFECTS.md H-21)
// ===========================================================================

/// The sha256 this crate prints is a claim about the CODE only if the build
/// cannot be steered from outside. Cargo exports `RUSTUP_TOOLCHAIN`, which
/// OVERRIDES `rust-toolchain.toml`, and `CARGO_TARGET_DIR`, which would send the
/// output somewhere else while the harness went on hashing whatever stale bytes
/// were left at the path it reads.
#[test]
fn the_canister_build_pins_its_toolchain_and_its_output_directory() {
    use settlement_oracle::wasms::{
        parse_toolchain_channel, pinned_table_canister_build, pinned_toolchain, repo_root,
    };
    use std::collections::BTreeMap;

    // The parser must survive an ordinary manifest. Written with `?` instead of
    // `continue` it returns None on the first line that is not the pin -- the
    // section header -- and every build refuses to start. That is not a
    // hypothetical: it is what the first version of this function did, and the
    // baseline run of the planted-bug matrix is what found it.
    assert_eq!(
        parse_toolchain_channel("# comment\n[toolchain]\nchannel = \"1.90.0\"\n").as_deref(),
        Some("1.90.0")
    );
    assert_eq!(parse_toolchain_channel("[toolchain]\nprofile=\"minimal\"\n"), None);

    let root = repo_root();
    let poison = [
        ("RUSTUP_TOOLCHAIN", "1.96.1"),
        ("CARGO_TARGET_DIR", "/tmp/somewhere-else"),
        ("RUSTFLAGS", "-C opt-level=0"),
    ];
    for (k, v) in poison {
        std::env::set_var(k, v);
    }
    let cmd = pinned_table_canister_build(&root);
    let overrides: BTreeMap<String, Option<String>> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|v| v.to_string_lossy().to_string()),
            )
        })
        .collect();
    for (k, _) in poison {
        std::env::remove_var(k);
    }

    assert_eq!(
        overrides.get("RUSTUP_TOOLCHAIN"),
        Some(&Some(pinned_toolchain(&root))),
        "the pinned toolchain must WIN over an inherited one: {overrides:?}"
    );
    for (key, planted) in poison.iter().skip(1) {
        assert_eq!(
            overrides.get(*key).cloned().flatten(),
            None,
            "{key} (planted {planted:?}) reaches the canister build: {overrides:?}"
        );
    }
}
