//! The scenario suite.
//!
//! Each function drives ONE named settlement shape on the real canister and
//! returns the comparison. They live in the library rather than in a test file so
//! that three callers share exactly one definition of each scenario:
//!
//! * `tests/settlement.rs`, the clean run and its coverage report;
//! * `tests/disagreements.rs`, which pins each disagreement so a later fix has to
//!   update the test deliberately;
//! * the planted-bug runs in `$SCRATCH`, which must exercise the identical path or
//!   they prove nothing about this harness.

use std::cell::RefCell;

use crate::deal::DealPlan;
use crate::drive;
use crate::observe::HandComparison;
use crate::scenarios::{
    target_all_distinct, target_any_tying_group, target_short_stack_best, Bench, DEFAULT_ATTEMPTS,
};
use crate::table_api::*;

/// The ladder stacks used by the all-in scenarios.
pub const LADDER: &[(&str, u8, u64)] = &[
    ("alice", 0, 20),
    ("bob", 1, 40),
    ("carol", 2, 80),
    ("dave", 3, 160),
    ("erin", 4, 400),
];

/// Four equal deep stacks: enough behind for real betting on every street.
pub const DEEP_EVEN: &[(&str, u8, u64)] = &[
    ("alice", 0, 400),
    ("bob", 1, 400),
    ("carol", 2, 400),
    ("dave", 3, 400),
];

/// One short stack and three deep ones.
pub const ONE_SHORT_THREE_DEEP: &[(&str, u8, u64)] = &[
    ("alice", 0, 20),
    ("bob", 1, 400),
    ("carol", 2, 400),
    ("dave", 3, 400),
];

// ---------------------------------------------------------------------------
// individual scenarios
// ---------------------------------------------------------------------------

pub fn fold_out_preflop(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand_with_policy("fold_out_preflop", drive::everyone_folds())
}

/// A bet on the flop that nobody covers. The engine ends this by fold, through
/// `end_hand_single_winner`, which pays out of `state.pot` and therefore does NOT
/// lose the post-flop money -- a useful control next to the showdown cases.
pub fn uncalled_bet_on_the_flop(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand("uncalled_bet_on_the_flop", None, 1, &mut |w, r, _| {
        drive::play_out_until_phase(w, r, GamePhase::Flop, drive::passive(), 80);
        let _ = drive::act_on_turn(w, r, PlayerAction::Bet(10));
        drive::play_out(w, r, drive::everyone_folds(), 80);
    })
}

pub fn two_all_ins(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand_with_policy("two_all_ins_20_vs_400", drive::all_in_only(vec![0, 4]))
}

pub fn three_all_ins(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand_with_policy("three_all_ins_20_40_80", drive::all_in_only(vec![0, 1, 2]))
}

pub fn five_all_ins(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand_with_policy("five_all_ins_full_ladder", drive::everyone_all_in())
}

/// The shape most sensitive to a wrong eligibility set: the shortest stack holds
/// the single best hand, so it must take the main pot and nothing above it.
pub fn short_stack_holds_the_nuts(bench: &mut Bench) -> Option<HandComparison> {
    let target = target_short_stack_best(0, vec![1, 2, 3, 4]);
    bench.hand(
        "short_stack_wins_only_the_main_pot",
        Some(&*target),
        DEFAULT_ATTEMPTS,
        &mut |w, r, _| {
            drive::play_out(w, r, drive::everyone_all_in(), 200);
        },
    )
}

/// Five distinct hands, so every layer has a single unambiguous winner.
pub fn ladder_with_no_ties(bench: &mut Bench) -> Option<HandComparison> {
    let target = target_all_distinct(vec![0, 1, 2, 3, 4]);
    bench.hand(
        "ladder_with_no_ties_at_all",
        Some(&*target),
        DEFAULT_ATTEMPTS,
        &mut |w, r, _| {
            drive::play_out(w, r, drive::everyone_all_in(), 200);
        },
    )
}

/// Search for a deal in which exactly `size` of the dealt hands tie, then fold
/// every other seat so the tie IS the showdown.
pub fn exact_tie(bench: &mut Bench, size: usize, label: &str) -> Option<HandComparison> {
    let finder = target_any_tying_group(size);
    let group: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    let target = |plan: &DealPlan| match finder(plan) {
        Some(found) => {
            *group.borrow_mut() = found;
            true
        }
        None => false,
    };
    bench.hand(label, Some(&target), DEFAULT_ATTEMPTS, &mut |w, r, _| {
        let keep = group.borrow().clone();
        drive::play_out(w, r, drive::all_in_only(keep), 200);
    })
}

/// Two seats deeper than the short stack tie, so the chop happens in a LAYER above
/// a main pot the short stack is eligible for.
pub fn chopped_side_pot(bench: &mut Bench) -> Option<HandComparison> {
    let finder = target_any_tying_group(2);
    let group: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    let target = |plan: &DealPlan| match finder(plan) {
        Some(found) if found.iter().all(|s| *s != 0) => {
            *group.borrow_mut() = found;
            true
        }
        _ => false,
    };
    bench.hand(
        "chopped_side_pot_above_a_short_all_in",
        Some(&target),
        DEFAULT_ATTEMPTS,
        &mut |w, r, _| {
            let mut keep = group.borrow().clone();
            keep.push(0);
            drive::play_out(w, r, drive::all_in_only(keep), 200);
        },
    )
}

/// Real money wagered on the flop, turn and river, then a showdown. Requires the
/// deep-even stacks.
pub fn post_flop_betting_to_showdown(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand_with_policy("post_flop_betting_to_showdown", drive::bet_every_street(20))
}

/// The same, with every showdown hand distinct so the winner is unambiguous.
pub fn post_flop_betting_no_ties(bench: &mut Bench) -> Option<HandComparison> {
    let target = target_all_distinct(vec![0, 1, 2, 3]);
    bench.hand(
        "post_flop_betting_four_way_showdown",
        Some(&*target),
        DEFAULT_ATTEMPTS,
        &mut |w, r, _| {
            drive::play_out(w, r, drive::bet_every_street(8), 300);
        },
    )
}

/// Two seats fold having each put in three times what the short all-in could
/// cover: dead money sitting above a short stack's reach. Requires
/// [`ONE_SHORT_THREE_DEEP`].
pub fn folded_money_above_a_short_all_in(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand(
        "folded_money_above_a_short_all_in",
        None,
        1,
        &mut |w, r, _| {
            drive::play_out_until_phase(
                w,
                r,
                GamePhase::Flop,
                drive::shove_one_else_raise_to(0, 60),
                120,
            );
            let _ = drive::act_on_turn(w, r, PlayerAction::Bet(40));
            drive::play_out(w, r, drive::everyone_folds(), 120);
        },
    )
}

/// A seat vacates AFTER the side pots were frozen at the flop. The frozen
/// breakdown still names it, so the money is unaffected: the control case for the
/// one below.
pub fn vacate_after_the_flop(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand("seat_vacated_after_the_flop", None, 1, &mut |w, r, _| {
        drive::play_out_until_phase(
            w,
            r,
            GamePhase::Flop,
            drive::shove_one_else_raise_to(0, 60),
            120,
        );
        if let Some(who) = w.table_state().player_at(1).map(|p| p.principal) {
            let _ = w.player_action(who, PlayerAction::Fold);
            r.observe(w);
            if w.leave_table(who).is_err() {
                let _ = w.cash_out(who);
            }
            r.observe(w);
        }
        drive::play_out(w, r, drive::passive(), 120);
    })
}

/// A seat vacates BEFORE the side pots are built, with a real stake committed.
///
/// The short stack is required to hold the best hand: if a deep seat wins every
/// layer, moving chips between layers is invisible in the payout. This is the
/// E-05 shape, and it is a pure redistribution -- every chip is conserved.
/// Requires [`ONE_SHORT_THREE_DEEP`].
pub fn vacate_before_the_side_pots_are_built(bench: &mut Bench) -> Option<HandComparison> {
    let target = target_short_stack_best(0, vec![1, 2, 3]);
    bench.hand(
        "seat_vacated_before_the_side_pots_were_built",
        Some(&*target),
        DEFAULT_ATTEMPTS,
        &mut |w, r, _| {
            drive::vacate_a_committed_seat_preflop(w, r, 0, 60, 200);
            // Check it down from the flop, so the only thing being measured is the
            // eligibility of the layers and not post-flop money as well.
            drive::play_out(w, r, drive::passive(), 200);
        },
    )
}

/// THE CHAIR CHANGES HANDS MID-HAND: the shape the per-seat oracle is blind to.
///
/// # What this scenario is for (docs/SECURITY-FINDINGS.md FINDING 13)
///
/// A player leaves the table with money already in the pot -- the stake stays in
/// the payout basis, correctly -- and a DIFFERENT principal then takes that empty
/// chair before the hand settles. Every per-seat number is right afterwards; the
/// question this scenario asks is whose escrow the money landed in.
///
/// Requires a bench built with a bystander, because the newcomer must not have
/// been at the table when the hand was dealt. See [`chair_swap_bench`].
pub fn a_vacated_chair_is_taken_mid_hand(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand(
        "a_vacated_chair_is_taken_mid_hand",
        None,
        1,
        &mut |w, r, _| {
            // Get everybody's blinds and calls in, then stop on the flop.
            drive::play_out_until_phase(w, r, GamePhase::Flop, drive::passive(), 120);
            let state = w.table_state();
            // Whoever has money in and is NOT on the clock can leave cleanly.
            let leaver = state
                .seated()
                .find(|p| p.total_bet_this_hand > 0 && p.seat != state.action_on)
                .map(|p| (p.seat, p.principal));
            let Some((seat, who)) = leaver else { return };
            if w.leave_table(who).is_err() {
                return;
            }
            r.observe(w);
            // The bystander takes the chair mid-hand. `join_table` seats a mid-hand
            // arrival SittingOut with no cards, and `sit_in` makes them Active
            // (docs/DEFECTS.md E-36) -- which is the state that made FINDING 13
            // reachable rather than theoretical.
            let stranger = w.actor("mallory");
            if w.join_table(stranger, seat).is_ok() {
                let _ = w.sit_in(stranger);
            }
            r.observe(w);
            drive::play_out(w, r, drive::passive(), 200);
        },
    )
}

/// THE FINDING 13 SHAPE, end to end: a chair changes hands mid-hand and then every
/// player still holding cards leaves, so NOBODY can win and every stake has to go
/// back to whoever put it in.
///
/// This is the hand where "pay the chair" and "pay the person" give different
/// answers, and it is reachable entirely through the public API. Under the defect
/// the departed player's refund lands in the NEW OCCUPANT's stack: totals balance,
/// the seat is paid the right amount, and the per-seat diff is zero. Only the
/// principal column moves.
pub fn every_card_holder_leaves_after_a_chair_swap(bench: &mut Bench) -> Option<HandComparison> {
    bench.hand(
        "every_card_holder_leaves_after_a_chair_swap",
        None,
        1,
        &mut |w, r, _| {
            drive::play_out_until_phase(w, r, GamePhase::Flop, drive::passive(), 120);
            let state = w.table_state();
            let Some((seat, first_owner)) = state
                .seated()
                .find(|p| p.total_bet_this_hand > 0 && p.seat != state.action_on)
                .map(|p| (p.seat, p.principal))
            else {
                return;
            };
            if w.leave_table(first_owner).is_err() {
                return;
            }
            r.observe(w);

            let stranger = w.actor("mallory");
            if w.join_table(stranger, seat).is_err() {
                return;
            }
            let _ = w.sit_in(stranger);
            r.observe(w);

            // Everybody else who is holding cards leaves too. The hand ends the
            // moment only the newcomer -- who holds none -- is left, and no layer
            // has a claimant, so every stake is refunded to its owner.
            loop {
                let state = w.table_state();
                if !state.phase.hand_in_progress() {
                    break;
                }
                let Some(who) = state
                    .seated()
                    .find(|p| p.principal != stranger && p.hole_cards.is_some() && !p.has_folded)
                    .map(|p| p.principal)
                else {
                    break;
                };
                if w.leave_table(who).is_err() {
                    break;
                }
                r.observe(w);
            }
        },
    )
}

/// A bench for [`a_vacated_chair_is_taken_mid_hand`]: four seated players and one
/// funded, UNSEATED principal ready to take a chair.
pub fn chair_swap_bench() -> Bench {
    Bench::with_bystanders(
        TableConfig::micro_six_max(),
        DEEP_EVEN,
        &["mallory"],
        1_000_000,
    )
}

// ---------------------------------------------------------------------------
// the whole suite
// ---------------------------------------------------------------------------

/// Every scenario, in order, on a ladder bench. Re-stacks the bench as it goes.
pub fn run_all(bench: &mut Bench) {
    fold_out_preflop(bench);
    uncalled_bet_on_the_flop(bench);
    two_all_ins(bench);
    three_all_ins(bench);
    five_all_ins(bench);
    short_stack_holds_the_nuts(bench);
    ladder_with_no_ties(bench);
    exact_tie(bench, 2, "exact_tie_two_way_chop");
    // A three-way chop only leaves odd chips when the pot is not a multiple of
    // three, so more than one sample is needed before the odd-chip rule can be
    // said to have been exercised at all.
    exact_tie(bench, 3, "exact_tie_three_way_chop");
    exact_tie(bench, 3, "exact_tie_three_way_chop_b");
    exact_tie(bench, 3, "exact_tie_three_way_chop_c");
    chopped_side_pot(bench);

    bench.set_stacks(DEEP_EVEN);
    post_flop_betting_to_showdown(bench);
    post_flop_betting_no_ties(bench);

    bench.set_stacks(ONE_SHORT_THREE_DEEP);
    folded_money_above_a_short_all_in(bench);
    vacate_after_the_flop(bench);
    vacate_before_the_side_pots_are_built(bench);
}

/// The scenarios that need a bystander, so they cannot run on the ladder bench.
///
/// Kept separate rather than folded into [`run_all`] because the bench itself is
/// different: `Bench::with_bystanders` puts a funded, unseated principal in the
/// world, which is the whole precondition.
pub fn run_chair_swap(bench: &mut Bench) {
    a_vacated_chair_is_taken_mid_hand(bench);
    every_card_holder_leaves_after_a_chair_swap(bench);
}

/// A short subset for the planted-bug runs: no card searches, so it is fast, and
/// every hand is a multi-way showdown with side pots, which is where a wrong-seat
/// payout has room to hide.
pub fn run_wrong_seat_probe(bench: &mut Bench) {
    two_all_ins(bench);
    three_all_ins(bench);
    five_all_ins(bench);
    bench.hand_with_policy("probe_all_in_1_2_3", drive::all_in_only(vec![1, 2, 3]));
    bench.hand_with_policy("probe_all_in_0_2_4", drive::all_in_only(vec![0, 2, 4]));
    bench.hand_with_policy("probe_all_in_2_3_4", drive::all_in_only(vec![2, 3, 4]));
    bench.hand_with_policy("probe_all_in_0_1_2_3", drive::all_in_only(vec![0, 1, 2, 3]));
    bench.hand_with_policy("probe_passive_showdown", drive::passive());
}
