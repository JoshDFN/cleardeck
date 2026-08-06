//! Side-pot construction.
//!
//! # Two builders live here, and only one of them may touch money
//!
//! [`build_side_pots_from_contributions`] is **the payout basis**. It splits what
//! the players actually put in, by all-in depth, and nothing else. It takes no
//! `total_pot` argument, so the defect that argument caused (docs/DEFECTS.md
//! E-03: `state.pot` overriding the contributions in both directions, one
//! direction minting chips and the other destroying them, through an `f64` ratio
//! that cannot represent an e8 pot above 2^53) is **unrepresentable** in it.
//!
//! [`build_side_pots`] / [`build_side_pots_logged`] / [`apply_side_pots`] are the
//! pre-refactor routine, ported bug-for-bug out of
//! `calculate_side_pots(state: &mut TableState)` at commit `ceacc37`, including
//! the reconciliation branches and the `f64` cap. They are **ARCHIVE ONLY**:
//!
//! * nothing on the canister's payout path calls them any more (E-01/E-03 fix);
//! * ~1,500 golden vectors in `tests/golden_vectors.rs`, 466 of which trip the
//!   proportional-capping branch, pin their exact pre-refactor behaviour. That
//!   record is what makes the fix auditable, so it is kept rather than edited.
//!
//! `the_archive_builder_and_the_payout_builder_disagree_exactly_where_e03_said`
//! at the bottom of this file states the difference between the two as a test, so
//! the archive can never quietly become the payout basis again.
//!
//! See `docs/SECURITY-FINDINGS.md`, `docs/DEFECTS.md` E-01/E-03/E-05 and
//! `docs/FINDING-01-chip-destruction.md`.

use candid::{CandidType, Deserialize};

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

/// One player's stake in the hand being settled.
///
/// `seat` is the index of the player in `TableState::players`, which is what the
/// original routine used (it enumerated the seat vector rather than reading
/// `Player::seat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contribution {
    pub seat: u8,
    pub total_bet_this_hand: u64,
    pub has_folded: bool,
}

impl Contribution {
    pub fn new(seat: u8, total_bet_this_hand: u64, has_folded: bool) -> Self {
        Self {
            seat,
            total_bet_this_hand,
            has_folded,
        }
    }
}

/// Every chip in `contributions`.
pub fn total_contributed(contributions: &[Contribution]) -> u64 {
    contributions
        .iter()
        .fold(0u64, |a, c| a.saturating_add(c.total_bet_this_hand))
}

/// THE PAYOUT BASIS: split what the players actually put in, by all-in depth.
///
/// One layer per distinct contribution level. A layer holds each contributor's
/// slice of the money between the previous level and this one, and a seat is
/// eligible for it only if it covered the level in full **and** still has a claim
/// on the pot (has not folded and has not left the table -- see
/// `Contribution::has_folded`, which the canister sets for both).
///
/// # The two properties this function is written for
///
/// 1. `sum(pots) == total_contributed(contributions)`, exactly, always. There is
///    no `total_pot` argument to reconcile against, so there is nothing that can
///    mint or destroy a chip. `every_layering_conserves_every_chip` asserts this
///    over a randomised sweep.
/// 2. A layer nobody is eligible for **cannot** be dropped. Real betting cannot
///    produce one, but a seat vacated mid-hand can (docs/DEFECTS.md E-05). Its
///    chips are carried up to the next layer that does have eligible players, and
///    if there is none, into the highest layer that had any. If NO layer has an
///    eligible player at all, a single pot with an EMPTY eligibility list is
///    returned: the caller must then decide what to do with money nobody can win,
///    and cannot silently lose it. `plan_payouts` in the canister refunds it to
///    the seats that put it there.
pub fn build_side_pots_from_contributions(contributions: &[Contribution]) -> Vec<SidePot> {
    let mut levels: Vec<u64> = contributions
        .iter()
        .map(|c| c.total_bet_this_hand)
        .filter(|v| *v > 0)
        .collect();
    levels.sort_unstable();
    levels.dedup();

    let mut pots: Vec<SidePot> = Vec::with_capacity(levels.len());
    let mut lo = 0u64;
    // Chips from layers with no eligible claimant, waiting for a layer that has one.
    let mut carry = 0u64;

    for hi in levels {
        let amount = contributions.iter().fold(0u64, |acc, c| {
            acc.saturating_add(c.total_bet_this_hand.min(hi).saturating_sub(lo))
        });
        let eligible: Vec<u8> = contributions
            .iter()
            .filter(|c| !c.has_folded && c.total_bet_this_hand >= hi)
            .map(|c| c.seat)
            .collect();
        lo = hi;
        if amount == 0 {
            continue;
        }
        if eligible.is_empty() {
            carry = carry.saturating_add(amount);
            continue;
        }
        pots.push(SidePot {
            amount: amount.saturating_add(carry),
            eligible_players: eligible,
        });
        carry = 0;
    }

    if carry > 0 {
        match pots.last_mut() {
            Some(last) => last.amount = last.amount.saturating_add(carry),
            // Nobody at all is eligible for anything. Hand it back with an empty
            // eligibility list rather than dropping it on the floor.
            None => pots.push(SidePot {
                amount: carry,
                eligible_players: Vec::new(),
            }),
        }
    }

    pots
}

/// The part of the largest stake that no other player covered.
///
/// If exactly one seat put in strictly more than every other seat, the difference
/// between it and the next-largest stake was never matched by anybody and must be
/// handed back before the pots are formed. If two or more seats tie for the
/// largest stake, everything was covered and nothing comes back.
///
/// Folded seats count as "other players" here, deliberately: their money is in the
/// pot and the bettor can win it, so it covers part of the bet. And the seat that
/// gets the money back is the largest contributor *whether or not it folded* --
/// legal betting cannot make the unique largest contributor fold, but
/// `leave_table` can (docs/DEFECTS.md E-05), and money nobody covered still was
/// not covered.
pub fn uncalled_excess(contributions: &[Contribution]) -> Option<(u8, u64)> {
    let mut top: Option<(u8, u64)> = None;
    let mut second = 0u64;
    for c in contributions.iter().filter(|c| c.total_bet_this_hand > 0) {
        match top {
            Some((_, t)) if c.total_bet_this_hand > t => {
                second = t;
                top = Some((c.seat, c.total_bet_this_hand));
            }
            Some((_, t)) if c.total_bet_this_hand == t => {
                // A tie for the top means the top was fully covered.
                second = t;
            }
            Some((_, _)) => second = second.max(c.total_bet_this_hand),
            None => top = Some((c.seat, c.total_bet_this_hand)),
        }
    }
    match top {
        Some((seat, amount)) if amount > second => Some((seat, amount - second)),
        _ => None,
    }
}

/// Split `amount` between `winners`, giving the chips that do not divide out ONE
/// EACH, walking clockwise from the button.
///
/// This is the rule real rooms use: Robert's Rules of Poker gives the odd chip to
/// the first player clockwise from the button in flop games, and the TDA rules put
/// odd chips with the player(s) in the earliest position, one at a time when there
/// is more than one. It matters here because ClearDeck's chips are e8s, so a
/// three-way chop of a pot that is not a multiple of three leaves real,
/// withdrawable chips to place.
///
/// The routine this replaces credited the WHOLE remainder to a single seat
/// (`pot_share + remainder`). For two winners the two rules coincide; for three or
/// more they differ by a chip per extra winner. docs/DEFECTS.md E-35.
///
/// Returns one `(seat, amount)` entry per winner, in `winners` order, and always
/// pays out exactly `amount`.
pub fn split_pot_clockwise(
    amount: u64,
    winners: &[u8],
    dealer_seat: u8,
    num_seats: usize,
) -> Vec<(u8, u64)> {
    if winners.is_empty() {
        return Vec::new();
    }
    let n = winners.len() as u64;
    let share = amount / n;
    let remainder = (amount % n) as usize;

    let modulus = num_seats.max(1) as u64;
    let dealer = dealer_seat as u64 % modulus;
    // The seats that take an odd chip: the first `remainder` of them walking
    // clockwise from the seat immediately after the button.
    let mut order: Vec<u8> = winners.to_vec();
    order.sort_by_key(|s| ((*s as u64 % modulus) + modulus - dealer - 1) % modulus);
    let odd: Vec<u8> = order.into_iter().take(remainder).collect();

    winners
        .iter()
        .map(|seat| {
            let extra = if odd.contains(seat) { 1 } else { 0 };
            (*seat, share.saturating_add(extra))
        })
        .collect()
}

/// ARCHIVE. Build the side pots for a hand the way the pre-refactor engine did.
///
/// **Not the payout basis.** See the module docs: this reconciles against
/// `total_pot` and `total_pot` wins unconditionally, which is docs/DEFECTS.md
/// E-03. It is kept because ~1,500 golden vectors pin it as the record of what the
/// defect was. Use [`build_side_pots_from_contributions`] for anything that moves
/// money.
///
/// `contributions` must already exclude players who bet nothing this hand: the
/// original routine filtered on `total_bet_this_hand > 0` while collecting, and
/// that filter is part of the observable behaviour.
///
/// `total_pot` is the canister's `TableState::pot`.
pub fn build_side_pots(contributions: &[Contribution], total_pot: u64) -> Vec<SidePot> {
    build_side_pots_logged(contributions, total_pot).0
}

/// Same as [`build_side_pots`], but also returns the diagnostics the original
/// routine wrote with `ic_cdk::println!`, so the canister can still log them
/// without `poker_core` depending on `ic-cdk`.
pub fn build_side_pots_logged(
    contributions: &[Contribution],
    total_pot: u64,
) -> (Vec<SidePot>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let mut side_pots: Vec<SidePot> = Vec::new();

    // Get unique bet levels, sorted ascending
    let mut bet_levels: Vec<u64> = contributions
        .iter()
        .map(|c| c.total_bet_this_hand)
        .collect();
    bet_levels.sort();
    bet_levels.dedup();

    let mut processed_amount = 0u64;

    for level in bet_levels {
        let contribution_per_player = level.saturating_sub(processed_amount);

        if contribution_per_player == 0 {
            continue;
        }

        // Calculate pot amount from all players who contributed at least up to this level
        let pot_amount: u64 = contributions
            .iter()
            .filter(|c| c.total_bet_this_hand >= level)
            .map(|_| contribution_per_player)
            .fold(0u64, |acc, x| acc.saturating_add(x));

        // Add contributions from players who bet less than this level but more than processed
        let partial_contributions: u64 = contributions
            .iter()
            .filter(|c| c.total_bet_this_hand > processed_amount && c.total_bet_this_hand < level)
            .map(|c| c.total_bet_this_hand.saturating_sub(processed_amount))
            .fold(0u64, |acc, x| acc.saturating_add(x));

        let level_pot = pot_amount.saturating_add(partial_contributions);

        // Eligible players are only those who haven't folded and bet at least this level
        let eligible_players: Vec<u8> = contributions
            .iter()
            .filter(|c| !c.has_folded && c.total_bet_this_hand >= level)
            .map(|c| c.seat)
            .collect();

        if level_pot > 0 && !eligible_players.is_empty() {
            side_pots.push(SidePot {
                amount: level_pot,
                eligible_players,
            });
        } else if level_pot > 0 && eligible_players.is_empty() {
            // Edge case: all eligible players folded - money goes to last pot
            // If no last pot exists, we need to find any player still in the hand
            if let Some(last_pot) = side_pots.last_mut() {
                last_pot.amount = last_pot.amount.saturating_add(level_pot);
            } else {
                // No existing side pot - find any non-folded player to create a pot for
                let any_eligible: Vec<u8> = contributions
                    .iter()
                    .filter(|c| !c.has_folded)
                    .map(|c| c.seat)
                    .collect();
                if !any_eligible.is_empty() {
                    side_pots.push(SidePot {
                        amount: level_pot,
                        eligible_players: any_eligible,
                    });
                }
                // If truly no one is eligible (everyone folded), pot is dead - this shouldn't happen
            }
        }

        processed_amount = level;
    }

    // Verify total matches total_pot - if not, adjust last pot
    let total_side_pots: u64 = side_pots
        .iter()
        .map(|sp| sp.amount)
        .fold(0u64, |acc, x| acc.saturating_add(x));

    if total_side_pots < total_pot {
        // Add remaining pot to last pot (or first eligible pot)
        let remaining = total_pot.saturating_sub(total_side_pots);
        if let Some(last_pot) = side_pots.last_mut() {
            last_pot.amount = last_pot.amount.saturating_add(remaining);
        }
    } else if total_side_pots > total_pot {
        // SANITY CHECK: Side pots should never exceed total pot
        // This indicates a bug - log it and cap to prevent creating chips from nothing
        warnings.push(format!(
            "BUG: Side pots ({}) exceed total pot ({}). Capping to pot amount.",
            total_side_pots, total_pot
        ));
        // Proportionally reduce all side pots to match total pot
        if total_side_pots > 0 {
            let ratio = total_pot as f64 / total_side_pots as f64;
            let mut distributed: u64 = 0;
            let pot_count = side_pots.len();
            for (i, side_pot) in side_pots.iter_mut().enumerate() {
                if i == pot_count - 1 {
                    // Last pot gets remainder to avoid rounding errors
                    side_pot.amount = total_pot.saturating_sub(distributed);
                } else {
                    let adjusted = (side_pot.amount as f64 * ratio) as u64;
                    side_pot.amount = adjusted;
                    distributed = distributed.saturating_add(adjusted);
                }
            }
        }
    }

    (side_pots, warnings)
}

/// ARCHIVE. Replace `existing` with freshly-built side pots, the pre-refactor way.
///
/// **Not the payout basis** -- see the module docs and [`build_side_pots`].
///
/// This is the exact contract the canister used to need, including one subtlety
/// that is easy to lose in a refactor: when nobody has bet anything this hand the
/// original routine returned EARLY, **without** clearing the previous hand's side
/// pots. That is reproduced here.
///
/// Returns the diagnostics the caller should log.
pub fn apply_side_pots(
    existing: &mut Vec<SidePot>,
    contributions: &[Contribution],
    total_pot: u64,
) -> Vec<String> {
    if contributions.is_empty() {
        return Vec::new();
    }

    let (pots, warnings) = build_side_pots_logged(contributions, total_pot);
    *existing = pots;
    warnings
}

// ===========================================================================
// tests for the payout-basis primitives
// ===========================================================================
//
// These run on the host under a plain `cargo test --workspace`, with no replica
// and no wasm, and they are the only place the layering maths is stated
// independently of the canister. The end-to-end proof that the RIGHT SEAT gets
// paid is `tests/settlement/` (an independent oracle driving the real canister on
// PocketIC); this file pins the arithmetic that oracle measures.

#[cfg(test)]
mod tests {
    use super::*;

    fn c(seat: u8, bet: u64) -> Contribution {
        Contribution::new(seat, bet, false)
    }
    fn folded(seat: u8, bet: u64) -> Contribution {
        Contribution::new(seat, bet, true)
    }
    fn sum(pots: &[SidePot]) -> u64 {
        pots.iter().fold(0u64, |a, p| a.saturating_add(p.amount))
    }

    // --- the layering ------------------------------------------------------

    #[test]
    fn one_bet_level_is_one_pot_everybody_is_eligible_for() {
        let pots = build_side_pots_from_contributions(&[c(0, 62), c(1, 62), c(2, 62), c(3, 62)]);
        assert_eq!(pots.len(), 1);
        assert_eq!(pots[0].amount, 248);
        assert_eq!(pots[0].eligible_players, vec![0, 1, 2, 3]);
    }

    #[test]
    fn an_all_in_ladder_layers_by_depth_and_conserves_every_chip() {
        // 20 / 40 / 80 / 160 / 400 -- the settlement harness's ladder.
        let contributions = [c(0, 20), c(1, 40), c(2, 80), c(3, 160), c(4, 400)];
        let pots = build_side_pots_from_contributions(&contributions);
        let amounts: Vec<u64> = pots.iter().map(|p| p.amount).collect();
        //   (0,20]   5 x 20 = 100        eligible 0,1,2,3,4
        //   (20,40]  4 x 20 =  80        eligible 1,2,3,4
        //   (40,80]  3 x 40 = 120        eligible 2,3,4
        //   (80,160] 2 x 80 = 160        eligible 3,4
        //   (160,400]  1 x 240 = 240     eligible 4
        assert_eq!(amounts, vec![100, 80, 120, 160, 240]);
        assert_eq!(
            pots.iter()
                .map(|p| p.eligible_players.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![0, 1, 2, 3, 4],
                vec![1, 2, 3, 4],
                vec![2, 3, 4],
                vec![3, 4],
                vec![4]
            ]
        );
        assert_eq!(sum(&pots), total_contributed(&contributions));
    }

    #[test]
    fn folded_money_stays_in_the_pot_but_buys_no_eligibility() {
        // docs/DEFECTS.md E-05's D-03 shape: seat 1 folded with 60 in.
        let contributions = [c(0, 20), folded(1, 60), c(2, 200), c(3, 200)];
        let pots = build_side_pots_from_contributions(&contributions);
        assert_eq!(
            pots.iter()
                .map(|p| (p.amount, p.eligible_players.clone()))
                .collect::<Vec<_>>(),
            vec![
                (80, vec![0, 2, 3]),   // 4 seats x 20, INCLUDING the folded seat's
                (120, vec![2, 3]),     // (20,60]: 3 seats x 40
                (280, vec![2, 3]),     // (60,200]: 2 seats x 140
            ]
        );
        assert_eq!(sum(&pots), 480);
    }

    /// The E-05 regression, stated at the level of this function: whether the seat
    /// is still in the seat vector or not must not change one chip, as long as its
    /// contribution is still in the list. That is exactly what
    /// `TableState::departed_stakes` guarantees on the canister side.
    #[test]
    fn a_departed_seats_stake_layers_identically_to_a_folded_one() {
        let folded_in_seat = build_side_pots_from_contributions(&[
            c(0, 50),
            folded(1, 200),
            c(2, 200),
        ]);
        let departed = build_side_pots_from_contributions(&[
            c(0, 50),
            folded(1, 200), // recorded as a departed stake, has_folded = true
            c(2, 200),
        ]);
        let amounts: Vec<u64> = folded_in_seat.iter().map(|p| p.amount).collect();
        assert_eq!(amounts, vec![150, 300]);
        assert_eq!(
            amounts,
            departed.iter().map(|p| p.amount).collect::<Vec<u64>>()
        );
        // And the pre-refactor route, which dropped the seat from the list
        // entirely, moved 50 out of the main pot into the deep pot.
        let dropped = build_side_pots(&[c(0, 50), c(2, 200)], 450);
        assert_eq!(dropped[0].amount, 100, "the main pot shrank from 150");
        assert_eq!(dropped[1].amount, 350, "and the 50 moved into the deep pot");
    }

    #[test]
    fn a_layer_nobody_is_eligible_for_is_carried_never_dropped() {
        // Seats 1 and 2 both put in 200 and both gave up their claim; seat 0 is
        // all-in for 50 and is the only live seat. Real betting cannot reach this;
        // two `leave_table` calls can.
        let contributions = [c(0, 50), folded(1, 200), folded(2, 200)];
        let pots = build_side_pots_from_contributions(&contributions);
        assert_eq!(sum(&pots), 450, "not one chip may be dropped");
        assert_eq!(pots.len(), 1);
        assert_eq!(pots[0].eligible_players, vec![0]);
    }

    #[test]
    fn money_no_seat_can_claim_comes_back_with_an_empty_eligibility_list() {
        let pots = build_side_pots_from_contributions(&[folded(0, 30), folded(1, 30)]);
        assert_eq!(sum(&pots), 60);
        assert_eq!(pots.len(), 1);
        assert!(
            pots[0].eligible_players.is_empty(),
            "the caller must be forced to decide what happens to money nobody can win"
        );
    }

    #[test]
    fn no_contributions_is_no_pots() {
        assert!(build_side_pots_from_contributions(&[]).is_empty());
        assert!(build_side_pots_from_contributions(&[c(0, 0), c(1, 0)]).is_empty());
    }

    /// Property: the layering conserves every chip, for any shape of stakes. This
    /// is the property `state.pot` reconciliation used to break in both directions
    /// (docs/DEFECTS.md E-03).
    #[test]
    fn every_layering_conserves_every_chip() {
        // A small deterministic LCG, so the sweep is reproducible.
        let mut seed = 0x5eed_1234_u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            seed >> 33
        };
        for case in 0..5_000 {
            let n = 2 + (next() % 5) as usize;
            let contributions: Vec<Contribution> = (0..n)
                .map(|seat| {
                    let bet = next() % 1_000;
                    let has_folded = next() % 3 == 0;
                    Contribution::new(seat as u8, bet, has_folded)
                })
                .collect();
            let pots = build_side_pots_from_contributions(&contributions);
            assert_eq!(
                sum(&pots),
                total_contributed(&contributions),
                "case {case}: {contributions:?} -> {pots:?}"
            );
        }
    }

    /// The same property at real ICP magnitudes, where the archive builder's `f64`
    /// ratio loses precision (an e8 pot can exceed 2^53).
    #[test]
    fn conservation_holds_at_e8_magnitudes_where_f64_would_not() {
        let big = 9_007_199_254_740_993u64; // 2^53 + 1: not representable in f64
        let contributions = [c(0, big), c(1, big), c(2, big / 2)];
        let pots = build_side_pots_from_contributions(&contributions);
        assert_eq!(sum(&pots), total_contributed(&contributions));
        assert_eq!(big as f64 as u64, big - 1, "2^53+1 does not survive f64");
    }

    // --- the uncalled bet --------------------------------------------------

    #[test]
    fn the_unique_largest_stake_gets_back_what_nobody_covered() {
        assert_eq!(uncalled_excess(&[c(0, 100), c(1, 300)]), Some((1, 200)));
        assert_eq!(uncalled_excess(&[c(1, 300), c(0, 100)]), Some((1, 200)));
        assert_eq!(uncalled_excess(&[c(0, 100), c(1, 100)]), None);
        assert_eq!(uncalled_excess(&[]), None);
        assert_eq!(uncalled_excess(&[c(0, 100)]), Some((0, 100)));
    }

    #[test]
    fn a_tie_for_the_largest_stake_means_nothing_is_uncalled() {
        assert_eq!(uncalled_excess(&[c(0, 50), c(1, 200), c(2, 200)]), None);
    }

    #[test]
    fn folded_money_covers_a_bet_and_a_folded_seat_can_still_be_owed_the_excess() {
        // Seat 1 bets 300 total and folds; seat 2 has 200 in. 100 was uncovered.
        assert_eq!(
            uncalled_excess(&[c(0, 50), folded(1, 300), c(2, 200)]),
            Some((1, 100))
        );
    }

    #[test]
    fn returning_the_uncalled_bet_then_layering_never_changes_the_total() {
        let mut contributions = vec![c(0, 20), folded(1, 60), c(2, 100), c(3, 400)];
        let gross = total_contributed(&contributions);
        let (seat, excess) = uncalled_excess(&contributions).expect("seat 3 over-bet");
        assert_eq!((seat, excess), (3, 300));
        for x in contributions.iter_mut().filter(|x| x.seat == seat) {
            x.total_bet_this_hand -= excess;
        }
        let pots = build_side_pots_from_contributions(&contributions);
        assert_eq!(sum(&pots) + excess, gross);
    }

    // --- the odd chips -----------------------------------------------------

    #[test]
    fn a_pot_that_divides_evenly_needs_no_odd_chip_rule() {
        assert_eq!(
            split_pot_clockwise(114, &[1, 2, 4], 1, 6),
            vec![(1, 38), (2, 38), (4, 38)]
        );
    }

    #[test]
    fn odd_chips_go_one_each_clockwise_from_the_button() {
        // docs/DEFECTS.md E-35 / settlement D-04: 8 chips, three winners, button on
        // seat 1, so clockwise order is 2, 3, 4, 5, 0, 1 and the two odd chips go to
        // seats 2 and 4 -- not both to seat 2.
        let split = split_pot_clockwise(8, &[1, 2, 4], 1, 6);
        assert_eq!(split, vec![(1, 2), (2, 3), (4, 3)]);
        assert_eq!(split.iter().map(|(_, a)| a).sum::<u64>(), 8);
    }

    #[test]
    fn the_two_way_chop_is_where_the_old_rule_and_this_one_coincide() {
        let split = split_pot_clockwise(81, &[0, 3], 1, 6);
        assert_eq!(split, vec![(0, 40), (3, 41)]);
        assert_eq!(split.iter().map(|(_, a)| a).sum::<u64>(), 81);
    }

    #[test]
    fn splitting_always_pays_out_the_whole_pot() {
        for amount in 0..200u64 {
            for winners in [
                vec![0u8],
                vec![0, 1],
                vec![2, 4],
                vec![0, 1, 2],
                vec![1, 2, 3, 4, 5],
            ] {
                for dealer in 0..6u8 {
                    let split = split_pot_clockwise(amount, &winners, dealer, 6);
                    assert_eq!(
                        split.iter().map(|(_, a)| *a).sum::<u64>(),
                        amount,
                        "amount {amount} winners {winners:?} dealer {dealer}"
                    );
                    let max = split.iter().map(|(_, a)| *a).max().unwrap();
                    let min = split.iter().map(|(_, a)| *a).min().unwrap();
                    assert!(max - min <= 1, "shares must differ by at most one chip");
                }
            }
        }
    }

    #[test]
    fn splitting_between_nobody_pays_nobody() {
        assert!(split_pot_clockwise(100, &[], 0, 6).is_empty());
    }

    // --- the archive, and why it is not the payout basis --------------------

    /// The two builders must be shown to disagree, in both of E-03's directions,
    /// so nobody can quietly put the archive back on the payout path and believe
    /// the golden vectors still cover it.
    #[test]
    fn the_archive_builder_and_the_payout_builder_disagree_exactly_where_e03_said() {
        let contributions = [c(0, 50), c(1, 100), c(2, 100)];
        let honest = total_contributed(&contributions);
        assert_eq!(honest, 250);

        let payout = build_side_pots_from_contributions(&contributions);
        assert_eq!(sum(&payout), honest, "the payout basis is the contributions");

        // Direction A: `pot` overstates the contributions (E-05's orphaned stake).
        // The archive MINTS the difference into the highest bet level.
        let inflated = build_side_pots(&contributions, honest + 100);
        assert_eq!(sum(&inflated), 350);
        assert_eq!(
            inflated.last().unwrap().amount,
            payout.last().unwrap().amount + 100,
            "the excess lands in the pot only the deepest stacks can win"
        );

        // Direction B: `pot` understates them. The archive DESTROYS the difference.
        let capped = build_side_pots(&contributions, 60);
        assert_eq!(sum(&capped), 60, "190 chips destroyed by the f64 cap");

        // And the warning the engine used to log and then settle anyway.
        let (_, warnings) = build_side_pots_logged(&contributions, 0);
        assert!(
            warnings.iter().any(|w| w.starts_with("BUG: Side pots")),
            "the archive still reports its own inconsistency: {warnings:?}"
        );
    }
}
