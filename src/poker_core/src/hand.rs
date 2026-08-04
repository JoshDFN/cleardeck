//! Poker hand ranking.
//!
//! MOVED VERBATIM out of `src/table_canister/src/lib.rs` (commit ceacc37).
//! `HandRank`'s variant ORDER defines who wins a pot: the derived `Ord` compares
//! by discriminant first, so `HighCard` must stay first and `RoyalFlush` last.
//! `HandRank` is also on the Candid wire (`Winner.hand_rank`, `ShowdownPlayer`).

use crate::card::{Card, Suit};
use candid::{CandidType, Deserialize};
use std::collections::HashMap;

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard(Vec<u8>),
    Pair(u8, Vec<u8>),
    TwoPair(u8, u8, u8),
    ThreeOfAKind(u8, Vec<u8>),
    Straight(u8),
    Flush(Vec<u8>),
    FullHouse(u8, u8),
    FourOfAKind(u8, u8),
    StraightFlush(u8),
    RoyalFlush,
}

/// Evaluates a player's best 5-card hand from their 2 hole cards and up to 5 community cards.
///
/// Generates all possible 5-card combinations from the 7 available cards and returns
/// the highest-ranking hand according to standard poker hand rankings:
/// Royal Flush > Straight Flush > Four of a Kind > Full House > Flush >
/// Straight > Three of a Kind > Two Pair > One Pair > High Card
pub fn evaluate_hand(hole_cards: &(Card, Card), community: &[Card]) -> HandRank {
    let mut all_cards: Vec<Card> = Vec::with_capacity(7);
    all_cards.push(hole_cards.0);
    all_cards.push(hole_cards.1);
    all_cards.extend_from_slice(community);

    // Generate all 5-card combinations and find the best
    let mut best_rank: Option<HandRank> = None;

    for combo in combinations(&all_cards, 5) {
        let rank = evaluate_five_cards(&combo);
        match &best_rank {
            None => best_rank = Some(rank),
            Some(current) if rank > *current => best_rank = Some(rank),
            _ => {}
        }
    }

    best_rank.unwrap_or(HandRank::HighCard(vec![]))
}

pub fn combinations(cards: &[Card], k: usize) -> Vec<Vec<Card>> {
    let mut result = Vec::new();
    let n = cards.len();
    if k > n {
        return result;
    }

    let mut indices: Vec<usize> = (0..k).collect();

    loop {
        result.push(indices.iter().map(|&i| cards[i]).collect());

        let mut i = k;
        while i > 0 {
            i -= 1;
            if indices[i] != i + n - k {
                break;
            }
        }

        if i == 0 && indices[0] == n - k {
            break;
        }

        indices[i] += 1;
        for j in (i + 1)..k {
            indices[j] = indices[j - 1] + 1;
        }
    }

    result
}

pub fn evaluate_five_cards(cards: &[Card]) -> HandRank {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank.value()).collect();
    ranks.sort_by(|a, b| b.cmp(a)); // Sort descending

    let mut suits: HashMap<Suit, u8> = HashMap::new();
    let mut rank_counts: HashMap<u8, u8> = HashMap::new();

    for card in cards {
        *suits.entry(card.suit).or_insert(0) += 1;
        *rank_counts.entry(card.rank.value()).or_insert(0) += 1;
    }

    let is_flush = suits.values().any(|&count| count >= 5);
    let is_straight = check_straight(&ranks);
    let straight_high = if is_straight { get_straight_high(&ranks) } else { 0 };

    // Royal Flush
    if is_flush && is_straight && straight_high == 14 {
        return HandRank::RoyalFlush;
    }

    // Straight Flush
    if is_flush && is_straight {
        return HandRank::StraightFlush(straight_high);
    }

    // Count pairs, trips, quads
    let mut pairs: Vec<u8> = Vec::new();
    let mut trips: Vec<u8> = Vec::new();
    let mut quads: Vec<u8> = Vec::new();

    for (&rank, &count) in &rank_counts {
        match count {
            4 => quads.push(rank),
            3 => trips.push(rank),
            2 => pairs.push(rank),
            _ => {}
        }
    }

    pairs.sort_by(|a, b| b.cmp(a));
    trips.sort_by(|a, b| b.cmp(a));

    // Four of a Kind
    if !quads.is_empty() {
        let kicker = ranks.iter().find(|&&r| r != quads[0]).copied().unwrap_or(0);
        return HandRank::FourOfAKind(quads[0], kicker);
    }

    // Full House
    if !trips.is_empty() && !pairs.is_empty() {
        return HandRank::FullHouse(trips[0], pairs[0]);
    }

    // Flush
    if is_flush {
        return HandRank::Flush(ranks.clone());
    }

    // Straight
    if is_straight {
        return HandRank::Straight(straight_high);
    }

    // Three of a Kind
    if !trips.is_empty() {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != trips[0])
            .take(2)
            .copied()
            .collect();
        return HandRank::ThreeOfAKind(trips[0], kickers);
    }

    // Two Pair
    if pairs.len() >= 2 {
        let kicker = ranks.iter()
            .find(|&&r| r != pairs[0] && r != pairs[1])
            .copied()
            .unwrap_or(0);
        return HandRank::TwoPair(pairs[0], pairs[1], kicker);
    }

    // One Pair
    if pairs.len() == 1 {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != pairs[0])
            .take(3)
            .copied()
            .collect();
        return HandRank::Pair(pairs[0], kickers);
    }

    // High Card
    HandRank::HighCard(ranks)
}

/// Detects if ranks form a straight and returns the high card.
/// Returns Some(high_card) if straight found, None otherwise.
/// Handles wheel (A-2-3-4-5) as a special case with high card 5.
pub fn detect_straight(ranks: &[u8]) -> Option<u8> {
    let mut sorted: Vec<u8> = ranks.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    sorted.dedup();

    if sorted.len() < 5 {
        return None;
    }

    // Check for wheel (A-2-3-4-5) first - it's the lowest straight
    // Must check before regular straights since A-5-4-3-2 window won't match
    if sorted.contains(&14) && sorted.contains(&5) && sorted.contains(&4)
        && sorted.contains(&3) && sorted.contains(&2) {
        return Some(5); // Wheel's high card is 5
    }

    // Check for regular straight (highest first)
    for window in sorted.windows(5) {
        if window[0] - window[4] == 4 {
            return Some(window[0]);
        }
    }

    None
}

pub fn check_straight(ranks: &[u8]) -> bool {
    detect_straight(ranks).is_some()
}

pub fn get_straight_high(ranks: &[u8]) -> u8 {
    detect_straight(ranks).unwrap_or(0)
}
