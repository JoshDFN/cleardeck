//! Settlement, unchanged, with the cards arriving as an argument.
//!
//! `docs/NO-PEEKING-FEASIBILITY.md` §8 flagged the showdown split as the first open
//! question of the sealed-dealer design, because `evaluate_hand` and the side-pot
//! code read hole cards directly. **This file is the answer, and the answer is that
//! there is no split.** `build_side_pots_from_contributions`, `split_pot_clockwise`
//! and `try_evaluate_hand` are called here exactly as
//! `src/table_canister/src/lib.rs` calls them; the only difference is where the
//! cards come from. `poker_core` is not touched, so the golden vectors, the
//! settlement oracle and the money-safety invariants all still bind to the same
//! code.

use crate::{seat_mut, with_table, Phase, Table, Winner, MAX_SEATS};
use dealer_types::SeatCards;
use poker_core::{
    build_side_pots_from_contributions, split_pot_clockwise, try_evaluate_hand, Contribution,
    HandRank,
};

// ---------------------------------------------------------------------------
// SETTLEMENT — unchanged poker_core, cards arriving as an argument
// ---------------------------------------------------------------------------

pub fn contributions(t: &Table) -> Vec<Contribution> {
    t.seats
        .iter()
        .flatten()
        .filter(|s| s.in_hand && s.total_bet_this_hand > 0)
        .map(|s| Contribution::new(s.seat, s.total_bet_this_hand, s.has_folded))
        .collect()
}

fn pay(t: &mut Table, seat: u8, amount: u64, rank: Option<HandRank>) {
    if let Some(p) = seat_mut(t, seat) {
        p.chips = p.chips.saturating_add(amount);
        let principal = p.principal;
        t.winners.push(Winner {
            seat,
            principal,
            amount,
            rank,
        });
    }
}

pub fn settle_foldout(winner: Option<u8>) -> Result<(), String> {
    with_table(|t| {
        let pots = build_side_pots_from_contributions(&contributions(t));
        let Some(seat) = winner else {
            return;
        };
        for pot in pots {
            if pot.eligible_players.contains(&seat) {
                pay(t, seat, pot.amount, None);
            } else {
                // Chips nobody left is eligible for go back to the deepest
                // eligible contributor. Cannot happen in a fold-out with one
                // survivor, and is here so the money is never silently dropped.
                if let Some(s) = pot.eligible_players.first().copied() {
                    pay(t, s, pot.amount, None);
                }
            }
        }
        t.phase = Phase::Showdown;
    })
}

/// The showdown, with the cards handed in.
///
/// `try_evaluate_hand`, not `evaluate_hand`: `poker_core::evaluate_hand` traps on
/// an impossible input and a trap here would cost the WINNER the pot
/// (SECURITY-FINDINGS FINDING 15). The spike inherits that rule rather than
/// re-learning it.
pub fn settle_showdown(cards: Vec<SeatCards>) -> Result<(), String> {
    with_table(|t| {
        t.showdown = cards.clone();
        let board = t.board.clone();
        let ranks: Vec<(u8, HandRank)> = cards
            .iter()
            .filter_map(|sc| {
                try_evaluate_hand(&sc.cards, &board)
                    .ok()
                    .map(|r| (sc.seat, r))
            })
            .collect();

        let pots = build_side_pots_from_contributions(&contributions(t));
        let button = t.dealer_seat;
        for pot in pots {
            let mut best: Option<HandRank> = None;
            let mut winners: Vec<u8> = Vec::new();
            for (seat, rank) in &ranks {
                if !pot.eligible_players.contains(seat) {
                    continue;
                }
                match &best {
                    None => {
                        best = Some(rank.clone());
                        winners = vec![*seat];
                    }
                    Some(b) if rank > b => {
                        best = Some(rank.clone());
                        winners = vec![*seat];
                    }
                    Some(b) if rank == b => winners.push(*seat),
                    _ => {}
                }
            }
            if winners.is_empty() {
                continue;
            }
            for (seat, amount) in split_pot_clockwise(pot.amount, &winners, button, MAX_SEATS) {
                pay(t, seat, amount, best.clone());
            }
        }
        t.phase = Phase::Showdown;
    })
}

