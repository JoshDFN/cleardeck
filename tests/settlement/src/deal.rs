//! Reading the deal, and predicting the board that WILL come.
//!
//! `get_table_state` gives a controller the whole deck and the deck index. After
//! `start_new_hand` the hole cards are dealt and `deck_index` points at the first
//! undealt card, and from there the board is completely determined by the engine's
//! dealing pattern:
//!
//! ```text
//!   flop   burn one, then three cards   deck[i+1], deck[i+2], deck[i+3]
//!   turn   burn one, then one card      deck[i+5]
//!   river  burn one, then one card      deck[i+7]
//! ```
//!
//! (from `advance_to_next_street` and `run_out_board`; both burn one card before
//! each street and both take cards in the same order, so the prediction holds
//! whether the hand is bet out street by street or run out after an all-in.)
//!
//! This is what turns "hope randomness produces a three-way chop" into "deal,
//! look, rewind if it is not the hand we wanted". Every prediction the harness
//! makes is checked against the board the engine actually produced, so a wrong
//! prediction fails loudly instead of silently steering the search wrong.

use crate::cards::{cards_str, hole_str};
use crate::table_api::{GamePhase, TableState};
use poker_core::{evaluate_hand, Card, HandRank};
use std::collections::BTreeMap;

/// Everything about a fresh deal that is already decided.
#[derive(Clone, Debug)]
pub struct DealPlan {
    pub hand_number: u64,
    pub dealer_seat: u8,
    pub num_seats: usize,
    /// Seat -> the two cards it was dealt.
    pub hole: BTreeMap<u8, (Card, Card)>,
    /// The five community cards that will come if the hand goes to the river.
    pub board: Vec<Card>,
}

impl DealPlan {
    /// The rank each dealt seat will hold on the full five-card board.
    pub fn ranks_at_river(&self) -> BTreeMap<u8, HandRank> {
        self.hole
            .iter()
            .filter(|_| self.board.len() == 5)
            .map(|(seat, hole)| (*seat, evaluate_hand(hole, &self.board)))
            .collect()
    }

    /// Seats sharing the best hand at the river, among `among`.
    pub fn winners_among(&self, among: &[u8]) -> Vec<u8> {
        let ranks = self.ranks_at_river();
        let mut candidates: Vec<(&u8, &HandRank)> =
            ranks.iter().filter(|(s, _)| among.contains(s)).collect();
        candidates.sort_by(|a, b| b.1.cmp(a.1));
        let Some((_, best)) = candidates.first().map(|(s, r)| (*s, (*r).clone())) else {
            return Vec::new();
        };
        candidates
            .iter()
            .filter(|(_, r)| **r == best)
            .map(|(s, _)| **s)
            .collect()
    }

    /// How many seats among `among` tie for the best hand at the river.
    pub fn tie_size_among(&self, among: &[u8]) -> usize {
        self.winners_among(among).len()
    }

    pub fn describe(&self) -> String {
        let mut s = format!(
            "hand #{} button seat {} board {}",
            self.hand_number,
            self.dealer_seat,
            cards_str(&self.board)
        );
        for (seat, hole) in &self.hole {
            s.push_str(&format!("\n    seat {seat}: {}", hole_str(&Some(*hole))));
        }
        s
    }
}

/// Read the deal off a freshly started hand.
///
/// Returns `None` unless the table is at `PreFlop` with no community cards yet,
/// i.e. exactly after `start_new_hand` and before any street was dealt.
pub fn read_deal(state: &TableState) -> Option<DealPlan> {
    if state.phase != GamePhase::PreFlop || !state.community_cards.is_empty() {
        return None;
    }
    let i = state.deck_index as usize;
    // flop needs i+1..=i+3, turn i+5, river i+7
    if state.deck.len() < i + 8 {
        return None;
    }
    let board = vec![
        state.deck[i + 1],
        state.deck[i + 2],
        state.deck[i + 3],
        state.deck[i + 5],
        state.deck[i + 7],
    ];
    let hole: BTreeMap<u8, (Card, Card)> = state
        .seated()
        .filter_map(|p| p.hole_cards.map(|h| (p.seat, h)))
        .collect();
    if hole.is_empty() {
        return None;
    }
    Some(DealPlan {
        hand_number: state.hand_number,
        dealer_seat: state.dealer_seat,
        num_seats: state.players.len(),
        hole,
        board,
    })
}
