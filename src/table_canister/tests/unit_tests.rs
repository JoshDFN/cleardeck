// Unit tests for table canister core logic
// These tests verify pure functions without IC infrastructure

use sha2::{Sha256, Digest};
use std::collections::HashMap;

// =============================================================================
// TYPE DEFINITIONS (mirror the canister types for testing)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    fn value(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Debug)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct PlayerContribution {
    pub seat: u8,
    pub total_bet: u64,
    pub has_folded: bool,
    pub is_all_in: bool,
}

// =============================================================================
// DECK CREATION AND SHUFFLE
// =============================================================================

fn create_deck() -> Vec<Card> {
    let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
    let ranks = [
        Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six,
        Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten,
        Rank::Jack, Rank::Queen, Rank::King, Rank::Ace,
    ];

    let mut deck = Vec::with_capacity(52);
    for suit in suits {
        for rank in ranks {
            deck.push(Card { suit, rank });
        }
    }
    deck
}

fn shuffle_deck(deck: &mut Vec<Card>, seed: &[u8]) {
    let mut hash_input = seed.to_vec();

    for i in (1..deck.len()).rev() {
        let mut hasher = Sha256::new();
        hasher.update(&hash_input);
        hasher.update(&[i as u8]);
        let hash_result = hasher.finalize();

        let random_value = u64::from_le_bytes([
            hash_result[0], hash_result[1], hash_result[2], hash_result[3],
            hash_result[4], hash_result[5], hash_result[6], hash_result[7],
        ]);
        let j = (random_value as usize) % (i + 1);

        deck.swap(i, j);
        hash_input = hash_result.to_vec();
    }
}

// =============================================================================
// HAND EVALUATION
// =============================================================================

fn combinations(cards: &[Card], k: usize) -> Vec<Vec<Card>> {
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

fn check_straight(ranks: &[u8]) -> bool {
    let mut sorted_ranks = ranks.to_vec();
    sorted_ranks.sort();
    sorted_ranks.dedup();

    if sorted_ranks.len() < 5 {
        return false;
    }

    // Check for regular straight
    for window in sorted_ranks.windows(5) {
        if window[4] - window[0] == 4 {
            return true;
        }
    }

    // Check for wheel (A-2-3-4-5)
    if sorted_ranks.contains(&14) && sorted_ranks.contains(&2) &&
       sorted_ranks.contains(&3) && sorted_ranks.contains(&4) &&
       sorted_ranks.contains(&5) {
        return true;
    }

    false
}

fn get_straight_high(ranks: &[u8]) -> u8 {
    let mut sorted_ranks = ranks.to_vec();
    sorted_ranks.sort();
    sorted_ranks.dedup();

    // Check for wheel first (high card is 5, not Ace)
    if sorted_ranks.contains(&14) && sorted_ranks.contains(&2) &&
       sorted_ranks.contains(&3) && sorted_ranks.contains(&4) &&
       sorted_ranks.contains(&5) {
        // Check if there's a higher straight
        for window in sorted_ranks.windows(5) {
            if window[4] - window[0] == 4 && window[4] > 5 {
                return window[4];
            }
        }
        return 5; // Wheel
    }

    // Find highest straight
    for i in (0..=sorted_ranks.len().saturating_sub(5)).rev() {
        let window = &sorted_ranks[i..i+5];
        if window[4] - window[0] == 4 {
            return window[4];
        }
    }

    0
}

fn evaluate_five_cards(cards: &[Card]) -> HandRank {
    let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank.value()).collect();
    ranks.sort_by(|a, b| b.cmp(a));

    let mut suits: HashMap<Suit, u8> = HashMap::new();
    let mut rank_counts: HashMap<u8, u8> = HashMap::new();

    for card in cards {
        *suits.entry(card.suit).or_insert(0) += 1;
        *rank_counts.entry(card.rank.value()).or_insert(0) += 1;
    }

    let is_flush = suits.values().any(|&count| count >= 5);
    let is_straight = check_straight(&ranks);
    let straight_high = if is_straight { get_straight_high(&ranks) } else { 0 };

    if is_flush && is_straight && straight_high == 14 {
        return HandRank::RoyalFlush;
    }

    if is_flush && is_straight {
        return HandRank::StraightFlush(straight_high);
    }

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

    if !quads.is_empty() {
        let kicker = ranks.iter().find(|&&r| r != quads[0]).copied().unwrap_or(0);
        return HandRank::FourOfAKind(quads[0], kicker);
    }

    if !trips.is_empty() && !pairs.is_empty() {
        return HandRank::FullHouse(trips[0], pairs[0]);
    }

    if is_flush {
        return HandRank::Flush(ranks.clone());
    }

    if is_straight {
        return HandRank::Straight(straight_high);
    }

    if !trips.is_empty() {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != trips[0])
            .take(2)
            .copied()
            .collect();
        return HandRank::ThreeOfAKind(trips[0], kickers);
    }

    if pairs.len() >= 2 {
        let kicker = ranks.iter()
            .find(|&&r| r != pairs[0] && r != pairs[1])
            .copied()
            .unwrap_or(0);
        return HandRank::TwoPair(pairs[0], pairs[1], kicker);
    }

    if pairs.len() == 1 {
        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != pairs[0])
            .take(3)
            .copied()
            .collect();
        return HandRank::Pair(pairs[0], kickers);
    }

    HandRank::HighCard(ranks.into_iter().take(5).collect())
}

fn evaluate_hand(hole_cards: &(Card, Card), community: &[Card]) -> HandRank {
    let mut all_cards: Vec<Card> = Vec::with_capacity(7);
    all_cards.push(hole_cards.0);
    all_cards.push(hole_cards.1);
    all_cards.extend_from_slice(community);

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

// =============================================================================
// SIDE POT CALCULATION
// =============================================================================

fn calculate_side_pots(contributions: &[PlayerContribution], total_pot: u64) -> Vec<SidePot> {
    if contributions.is_empty() {
        return vec![];
    }

    // Get unique bet levels, sorted ascending
    let mut bet_levels: Vec<u64> = contributions.iter()
        .filter(|c| c.total_bet > 0)
        .map(|c| c.total_bet)
        .collect();
    bet_levels.sort();
    bet_levels.dedup();

    let mut side_pots = Vec::new();
    let mut processed_amount = 0u64;

    for level in bet_levels {
        let contribution_per_player = level.saturating_sub(processed_amount);

        if contribution_per_player == 0 {
            continue;
        }

        // Calculate pot amount from all players who contributed at least up to this level
        let pot_amount: u64 = contributions.iter()
            .filter(|c| c.total_bet >= level)
            .map(|_| contribution_per_player)
            .fold(0u64, |acc, x| acc.saturating_add(x));

        // Add contributions from players who bet less than this level but more than processed
        let partial_contributions: u64 = contributions.iter()
            .filter(|c| c.total_bet > processed_amount && c.total_bet < level)
            .map(|c| c.total_bet.saturating_sub(processed_amount))
            .fold(0u64, |acc, x| acc.saturating_add(x));

        let total_pot_amount = pot_amount.saturating_add(partial_contributions);

        // Eligible players are only those who haven't folded and bet at least this level
        let eligible_players: Vec<u8> = contributions.iter()
            .filter(|c| !c.has_folded && c.total_bet >= level)
            .map(|c| c.seat)
            .collect();

        if total_pot_amount > 0 && !eligible_players.is_empty() {
            side_pots.push(SidePot {
                amount: total_pot_amount,
                eligible_players,
            });
        } else if total_pot_amount > 0 && eligible_players.is_empty() {
            // Edge case: all eligible players folded - money goes to last pot
            if let Some(last_pot) = side_pots.last_mut() {
                last_pot.amount = last_pot.amount.saturating_add(total_pot_amount);
            }
        }

        processed_amount = level;
    }

    // Verify total matches expected - if not, adjust last pot
    let total_side_pots: u64 = side_pots.iter()
        .map(|sp| sp.amount)
        .fold(0u64, |acc, x| acc.saturating_add(x));

    if total_side_pots < total_pot {
        let remaining = total_pot.saturating_sub(total_side_pots);
        if let Some(last_pot) = side_pots.last_mut() {
            last_pot.amount = last_pot.amount.saturating_add(remaining);
        }
    }

    side_pots
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // DECK TESTS
    // =========================================================================

    #[test]
    fn test_create_deck_has_52_cards() {
        let deck = create_deck();
        assert_eq!(deck.len(), 52);
    }

    #[test]
    fn test_create_deck_has_all_suits() {
        let deck = create_deck();
        let suits: Vec<Suit> = deck.iter().map(|c| c.suit).collect();

        assert_eq!(suits.iter().filter(|&&s| s == Suit::Hearts).count(), 13);
        assert_eq!(suits.iter().filter(|&&s| s == Suit::Diamonds).count(), 13);
        assert_eq!(suits.iter().filter(|&&s| s == Suit::Clubs).count(), 13);
        assert_eq!(suits.iter().filter(|&&s| s == Suit::Spades).count(), 13);
    }

    #[test]
    fn test_create_deck_has_all_ranks() {
        let deck = create_deck();
        let ranks: Vec<Rank> = deck.iter().map(|c| c.rank).collect();

        for rank in [Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six,
                     Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten,
                     Rank::Jack, Rank::Queen, Rank::King, Rank::Ace] {
            assert_eq!(ranks.iter().filter(|&&r| r == rank).count(), 4);
        }
    }

    #[test]
    fn test_create_deck_no_duplicates() {
        let deck = create_deck();
        let mut seen = std::collections::HashSet::new();
        for card in &deck {
            let key = (card.suit, card.rank);
            assert!(!seen.contains(&key), "Duplicate card found: {:?}", card);
            seen.insert(key);
        }
    }

    #[test]
    fn test_shuffle_deterministic() {
        let seed = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let mut deck1 = create_deck();
        let mut deck2 = create_deck();

        shuffle_deck(&mut deck1, &seed);
        shuffle_deck(&mut deck2, &seed);

        assert_eq!(deck1, deck2, "Same seed should produce same shuffle");
    }

    #[test]
    fn test_shuffle_different_seeds_different_results() {
        let seed1 = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let seed2 = vec![16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1];

        let mut deck1 = create_deck();
        let mut deck2 = create_deck();

        shuffle_deck(&mut deck1, &seed1);
        shuffle_deck(&mut deck2, &seed2);

        assert_ne!(deck1, deck2, "Different seeds should produce different shuffles");
    }

    #[test]
    fn test_shuffle_preserves_all_cards() {
        let seed = vec![42u8; 32];
        let original_deck = create_deck();
        let mut shuffled_deck = original_deck.clone();

        shuffle_deck(&mut shuffled_deck, &seed);

        assert_eq!(shuffled_deck.len(), 52);

        // Check all original cards are still present
        for card in &original_deck {
            assert!(shuffled_deck.contains(card), "Missing card: {:?}", card);
        }
    }

    // =========================================================================
    // HAND EVALUATION TESTS
    // =========================================================================

    fn card(rank: Rank, suit: Suit) -> Card {
        Card { suit, rank }
    }

    #[test]
    fn test_evaluate_royal_flush() {
        let hole = (card(Rank::Ace, Suit::Hearts), card(Rank::King, Suit::Hearts));
        let community = vec![
            card(Rank::Queen, Suit::Hearts),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Ten, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::RoyalFlush);
    }

    #[test]
    fn test_evaluate_straight_flush() {
        let hole = (card(Rank::Nine, Suit::Spades), card(Rank::Eight, Suit::Spades));
        let community = vec![
            card(Rank::Seven, Suit::Spades),
            card(Rank::Six, Suit::Spades),
            card(Rank::Five, Suit::Spades),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::StraightFlush(9));
    }

    #[test]
    fn test_evaluate_four_of_a_kind() {
        let hole = (card(Rank::King, Suit::Hearts), card(Rank::King, Suit::Diamonds));
        let community = vec![
            card(Rank::King, Suit::Clubs),
            card(Rank::King, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::FourOfAKind(13, 14)); // Kings with Ace kicker
    }

    #[test]
    fn test_evaluate_full_house() {
        let hole = (card(Rank::Queen, Suit::Hearts), card(Rank::Queen, Suit::Diamonds));
        let community = vec![
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Jack, Suit::Spades),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::FullHouse(12, 11)); // Queens full of Jacks
    }

    #[test]
    fn test_evaluate_flush() {
        let hole = (card(Rank::Ace, Suit::Clubs), card(Rank::Ten, Suit::Clubs));
        let community = vec![
            card(Rank::Seven, Suit::Clubs),
            card(Rank::Four, Suit::Clubs),
            card(Rank::Two, Suit::Clubs),
            card(Rank::King, Suit::Hearts),
            card(Rank::Queen, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        match result {
            HandRank::Flush(cards) => {
                assert_eq!(cards[0], 14); // Ace high flush
            }
            _ => panic!("Expected flush, got {:?}", result),
        }
    }

    #[test]
    fn test_evaluate_straight() {
        let hole = (card(Rank::Eight, Suit::Hearts), card(Rank::Seven, Suit::Diamonds));
        let community = vec![
            card(Rank::Six, Suit::Clubs),
            card(Rank::Five, Suit::Spades),
            card(Rank::Four, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::King, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::Straight(8)); // 8-high straight
    }

    #[test]
    fn test_evaluate_wheel_straight() {
        let hole = (card(Rank::Ace, Suit::Hearts), card(Rank::Two, Suit::Diamonds));
        let community = vec![
            card(Rank::Three, Suit::Clubs),
            card(Rank::Four, Suit::Spades),
            card(Rank::Five, Suit::Hearts),
            card(Rank::King, Suit::Clubs),
            card(Rank::Queen, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::Straight(5)); // Wheel (A-2-3-4-5)
    }

    #[test]
    fn test_evaluate_three_of_a_kind() {
        let hole = (card(Rank::Ten, Suit::Hearts), card(Rank::Ten, Suit::Diamonds));
        let community = vec![
            card(Rank::Ten, Suit::Clubs),
            card(Rank::King, Suit::Spades),
            card(Rank::Queen, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        match result {
            HandRank::ThreeOfAKind(rank, kickers) => {
                assert_eq!(rank, 10);
                assert_eq!(kickers[0], 13); // King kicker
                assert_eq!(kickers[1], 12); // Queen kicker
            }
            _ => panic!("Expected three of a kind, got {:?}", result),
        }
    }

    #[test]
    fn test_evaluate_two_pair() {
        let hole = (card(Rank::Jack, Suit::Hearts), card(Rank::Jack, Suit::Diamonds));
        let community = vec![
            card(Rank::Nine, Suit::Clubs),
            card(Rank::Nine, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::TwoPair(11, 9, 14)); // Jacks and Nines with Ace kicker
    }

    #[test]
    fn test_evaluate_one_pair() {
        let hole = (card(Rank::Eight, Suit::Hearts), card(Rank::Eight, Suit::Diamonds));
        let community = vec![
            card(Rank::King, Suit::Clubs),
            card(Rank::Queen, Suit::Spades),
            card(Rank::Ten, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        match result {
            HandRank::Pair(rank, kickers) => {
                assert_eq!(rank, 8);
                assert_eq!(kickers[0], 13); // King
                assert_eq!(kickers[1], 12); // Queen
                assert_eq!(kickers[2], 10); // Ten
            }
            _ => panic!("Expected pair, got {:?}", result),
        }
    }

    #[test]
    fn test_evaluate_high_card() {
        let hole = (card(Rank::Ace, Suit::Hearts), card(Rank::King, Suit::Diamonds));
        let community = vec![
            card(Rank::Ten, Suit::Clubs),
            card(Rank::Seven, Suit::Spades),
            card(Rank::Four, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        match result {
            HandRank::HighCard(cards) => {
                assert_eq!(cards[0], 14); // Ace high
                assert_eq!(cards[1], 13); // King
            }
            _ => panic!("Expected high card, got {:?}", result),
        }
    }

    #[test]
    fn test_hand_ranking_comparison() {
        // Verify hand rankings are ordered correctly
        let royal_flush = HandRank::RoyalFlush;
        let straight_flush = HandRank::StraightFlush(9);
        let four_kind = HandRank::FourOfAKind(10, 5);
        let full_house = HandRank::FullHouse(10, 5);
        let flush = HandRank::Flush(vec![14, 12, 10, 8, 6]);
        let straight = HandRank::Straight(10);
        let three_kind = HandRank::ThreeOfAKind(10, vec![8, 6]);
        let two_pair = HandRank::TwoPair(10, 8, 6);
        let pair = HandRank::Pair(10, vec![8, 6, 4]);
        let high_card = HandRank::HighCard(vec![14, 12, 10, 8, 6]);

        assert!(royal_flush > straight_flush);
        assert!(straight_flush > four_kind);
        assert!(four_kind > full_house);
        assert!(full_house > flush);
        assert!(flush > straight);
        assert!(straight > three_kind);
        assert!(three_kind > two_pair);
        assert!(two_pair > pair);
        assert!(pair > high_card);
    }

    // =========================================================================
    // SIDE POT TESTS
    // =========================================================================

    #[test]
    fn test_side_pots_no_all_in() {
        // Simple case: 3 players, all bet 100, no one all-in
        let contributions = vec![
            PlayerContribution { seat: 0, total_bet: 100, has_folded: false, is_all_in: false },
            PlayerContribution { seat: 1, total_bet: 100, has_folded: false, is_all_in: false },
            PlayerContribution { seat: 2, total_bet: 100, has_folded: false, is_all_in: false },
        ];

        let pots = calculate_side_pots(&contributions, 300);

        assert_eq!(pots.len(), 1);
        assert_eq!(pots[0].amount, 300);
        assert_eq!(pots[0].eligible_players, vec![0, 1, 2]);
    }

    #[test]
    fn test_side_pots_one_all_in() {
        // Player 0 all-in for 50, players 1 and 2 bet 100
        let contributions = vec![
            PlayerContribution { seat: 0, total_bet: 50, has_folded: false, is_all_in: true },
            PlayerContribution { seat: 1, total_bet: 100, has_folded: false, is_all_in: false },
            PlayerContribution { seat: 2, total_bet: 100, has_folded: false, is_all_in: false },
        ];

        let pots = calculate_side_pots(&contributions, 250);

        assert_eq!(pots.len(), 2);
        // Main pot: 50 * 3 = 150 (all 3 eligible)
        assert_eq!(pots[0].amount, 150);
        assert_eq!(pots[0].eligible_players, vec![0, 1, 2]);
        // Side pot: 50 * 2 = 100 (only players 1 and 2)
        assert_eq!(pots[1].amount, 100);
        assert_eq!(pots[1].eligible_players, vec![1, 2]);
    }

    #[test]
    fn test_side_pots_multiple_all_ins() {
        // Player 0 all-in 25, Player 1 all-in 50, Player 2 bets 100, Player 3 bets 100
        let contributions = vec![
            PlayerContribution { seat: 0, total_bet: 25, has_folded: false, is_all_in: true },
            PlayerContribution { seat: 1, total_bet: 50, has_folded: false, is_all_in: true },
            PlayerContribution { seat: 2, total_bet: 100, has_folded: false, is_all_in: false },
            PlayerContribution { seat: 3, total_bet: 100, has_folded: false, is_all_in: false },
        ];

        let pots = calculate_side_pots(&contributions, 275);

        assert_eq!(pots.len(), 3);
        // Main pot: 25 * 4 = 100 (all 4 eligible)
        assert_eq!(pots[0].amount, 100);
        assert_eq!(pots[0].eligible_players, vec![0, 1, 2, 3]);
        // Side pot 1: 25 * 3 = 75 (players 1, 2, 3)
        assert_eq!(pots[1].amount, 75);
        assert_eq!(pots[1].eligible_players, vec![1, 2, 3]);
        // Side pot 2: 50 * 2 = 100 (players 2, 3)
        assert_eq!(pots[2].amount, 100);
        assert_eq!(pots[2].eligible_players, vec![2, 3]);
    }

    #[test]
    fn test_side_pots_with_fold() {
        // Player 0 bets 50 and folds, Player 1 all-in 50, Player 2 bets 100
        let contributions = vec![
            PlayerContribution { seat: 0, total_bet: 50, has_folded: true, is_all_in: false },
            PlayerContribution { seat: 1, total_bet: 50, has_folded: false, is_all_in: true },
            PlayerContribution { seat: 2, total_bet: 100, has_folded: false, is_all_in: false },
        ];

        let pots = calculate_side_pots(&contributions, 200);

        // Folded player's money goes into pot but they're not eligible to win
        assert_eq!(pots.len(), 2);
        // Main pot: 50 * 3 = 150 (only 1, 2 eligible)
        assert_eq!(pots[0].amount, 150);
        assert_eq!(pots[0].eligible_players, vec![1, 2]);
        // Side pot: 50 (only player 2 eligible)
        assert_eq!(pots[1].amount, 50);
        assert_eq!(pots[1].eligible_players, vec![2]);
    }

    #[test]
    fn test_side_pots_empty_contributions() {
        let contributions: Vec<PlayerContribution> = vec![];
        let pots = calculate_side_pots(&contributions, 0);
        assert!(pots.is_empty());
    }

    #[test]
    fn test_side_pots_heads_up() {
        // Simple heads-up: both players bet 50
        let contributions = vec![
            PlayerContribution { seat: 0, total_bet: 50, has_folded: false, is_all_in: false },
            PlayerContribution { seat: 1, total_bet: 50, has_folded: false, is_all_in: false },
        ];

        let pots = calculate_side_pots(&contributions, 100);

        assert_eq!(pots.len(), 1);
        assert_eq!(pots[0].amount, 100);
        assert_eq!(pots[0].eligible_players, vec![0, 1]);
    }

    // =========================================================================
    // OVERFLOW PROTECTION TESTS
    // =========================================================================

    #[test]
    fn test_saturating_add_overflow() {
        let large: u64 = u64::MAX - 10;
        let result = large.saturating_add(100);
        assert_eq!(result, u64::MAX);
    }

    #[test]
    fn test_saturating_sub_underflow() {
        let small: u64 = 10;
        let result = small.saturating_sub(100);
        assert_eq!(result, 0);
    }

    // =========================================================================
    // COMBINATIONS TESTS
    // =========================================================================

    #[test]
    fn test_combinations_7_choose_5() {
        let cards: Vec<Card> = vec![
            card(Rank::Ace, Suit::Hearts),
            card(Rank::King, Suit::Hearts),
            card(Rank::Queen, Suit::Hearts),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Ten, Suit::Hearts),
            card(Rank::Nine, Suit::Hearts),
            card(Rank::Eight, Suit::Hearts),
        ];

        let combos = combinations(&cards, 5);

        // 7 choose 5 = 21
        assert_eq!(combos.len(), 21);

        // Each combination should have 5 cards
        for combo in &combos {
            assert_eq!(combo.len(), 5);
        }
    }

    #[test]
    fn test_combinations_all_unique() {
        let cards: Vec<Card> = vec![
            card(Rank::Ace, Suit::Hearts),
            card(Rank::King, Suit::Hearts),
            card(Rank::Queen, Suit::Hearts),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Ten, Suit::Hearts),
        ];

        let combos = combinations(&cards, 3);

        // Check no duplicate combinations
        let mut seen = std::collections::HashSet::new();
        for combo in &combos {
            let mut sorted_combo = combo.clone();
            sorted_combo.sort_by_key(|c| (c.suit, c.rank));
            let key = format!("{:?}", sorted_combo);
            assert!(!seen.contains(&key), "Duplicate combination found");
            seen.insert(key);
        }
    }

    // =========================================================================
    // EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_best_hand_from_seven_cards() {
        // Test that the best 5-card hand is correctly identified from 7 cards
        let hole = (card(Rank::Ace, Suit::Hearts), card(Rank::Ace, Suit::Diamonds));
        let community = vec![
            card(Rank::Ace, Suit::Clubs),
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
        ];

        let result = evaluate_hand(&hole, &community);
        assert_eq!(result, HandRank::FourOfAKind(14, 13)); // Four Aces with King kicker
    }

    #[test]
    fn test_flush_vs_straight() {
        // When both flush and straight are possible, flush should win
        let hole = (card(Rank::Ace, Suit::Hearts), card(Rank::Two, Suit::Hearts));
        let community = vec![
            card(Rank::Three, Suit::Hearts),
            card(Rank::Four, Suit::Hearts),
            card(Rank::Five, Suit::Hearts), // This makes both a wheel straight AND a flush
            card(Rank::King, Suit::Spades),
            card(Rank::Queen, Suit::Clubs),
        ];

        let result = evaluate_hand(&hole, &community);
        // Should be straight flush (A-2-3-4-5 of hearts), not just a flush
        assert_eq!(result, HandRank::StraightFlush(5));
    }

    #[test]
    fn test_two_pair_vs_trips() {
        // Three of a kind should beat two pair
        let three_kind = HandRank::ThreeOfAKind(7, vec![13, 12]);
        let two_pair = HandRank::TwoPair(14, 13, 12);

        assert!(three_kind > two_pair);
    }

    #[test]
    fn test_higher_pair_wins() {
        let pair_aces = HandRank::Pair(14, vec![13, 12, 11]);
        let pair_kings = HandRank::Pair(13, vec![14, 12, 11]);

        assert!(pair_aces > pair_kings);
    }

    #[test]
    fn test_same_pair_kicker_matters() {
        let pair_with_ace = HandRank::Pair(10, vec![14, 8, 6]);
        let pair_with_king = HandRank::Pair(10, vec![13, 8, 6]);

        assert!(pair_with_ace > pair_with_king);
    }

    // =========================================================================
    // ICP ACCOUNT IDENTIFIER (compute_account_identifier)
    // SHA224("\x0Aaccount-id" || principal || subaccount) with CRC32 prefix
    // =========================================================================

    fn compute_account_identifier(principal_bytes: &[u8], subaccount: Option<[u8; 32]>) -> [u8; 32] {
        use sha2::{Sha224, Digest};
        let mut hasher = Sha224::new();
        hasher.update(b"\x0Aaccount-id");
        hasher.update(principal_bytes);
        hasher.update(subaccount.unwrap_or([0u8; 32]));
        let hash = hasher.finalize(); // 28 bytes
        let crc = crc32fast::hash(&hash);
        let mut result = [0u8; 32];
        result[0..4].copy_from_slice(&crc.to_be_bytes());
        result[4..32].copy_from_slice(&hash);
        result
    }

    #[test]
    fn test_account_id_is_32_bytes() {
        let principal = b"\x01\x02\x03\x04\x05";
        let id = compute_account_identifier(principal, None);
        assert_eq!(id.len(), 32);
    }

    #[test]
    fn test_account_id_is_deterministic() {
        let principal = b"\xde\xad\xbe\xef\x01\x02\x03\x04";
        let a = compute_account_identifier(principal, None);
        let b = compute_account_identifier(principal, None);
        assert_eq!(a, b);
    }

    #[test]
    fn test_account_id_unique_per_principal() {
        let p1 = b"\x01\x02\x03\x04\x05";
        let p2 = b"\x01\x02\x03\x04\x06";
        let id1 = compute_account_identifier(p1, None);
        let id2 = compute_account_identifier(p2, None);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_account_id_none_subaccount_equals_zero_subaccount() {
        let principal = b"\xaa\xbb\xcc\xdd";
        let with_none = compute_account_identifier(principal, None);
        let with_zeros = compute_account_identifier(principal, Some([0u8; 32]));
        assert_eq!(with_none, with_zeros);
    }

    #[test]
    fn test_account_id_subaccount_changes_result() {
        let principal = b"\xaa\xbb\xcc\xdd";
        let default_sub = compute_account_identifier(principal, None);
        let mut custom = [0u8; 32];
        custom[31] = 1;
        let custom_sub = compute_account_identifier(principal, Some(custom));
        assert_ne!(default_sub, custom_sub);
    }

    #[test]
    fn test_account_id_crc32_prefix_validates() {
        // The first 4 bytes must be CRC32 of bytes [4..32].
        // This is the format verified by the NNS wallet.
        use sha2::{Sha224, Digest};
        let principal = b"\x01\x02\x03\x04\x05\x06\x07\x08";
        let id = compute_account_identifier(principal, None);
        let crc_stored = u32::from_be_bytes([id[0], id[1], id[2], id[3]]);
        let crc_computed = crc32fast::hash(&id[4..]);
        assert_eq!(crc_stored, crc_computed, "CRC32 prefix must validate the hash body");
    }

    #[test]
    fn test_account_id_hex_is_64_chars() {
        let principal = b"\x01\x02\x03\x04\x05";
        let id = compute_account_identifier(principal, None);
        let hex = hex::encode(id);
        assert_eq!(hex.len(), 64);
    }

    // =========================================================================
    // CURRENCY FORMAT_AMOUNT
    // =========================================================================

    fn format_icp(e8s: u64) -> String {
        let decimal = e8s as f64 / 100_000_000.0;
        format!("{:.4} ICP", decimal)
    }

    fn format_btc(satoshis: u64) -> String {
        let decimal = satoshis as f64 / 100_000_000.0;
        if decimal >= 0.001 {
            format!("{:.4} BTC", decimal)
        } else {
            format!("{} sats", satoshis)
        }
    }

    fn format_eth(wei: u64) -> String {
        let decimal = wei as f64 / 1_000_000_000_000_000_000.0;
        if decimal >= 0.001 {
            format!("{:.6} ETH", decimal)
        } else {
            let gwei = wei as f64 / 1_000_000_000.0;
            format!("{:.2} Gwei", gwei)
        }
    }

    fn format_doge(shibes: u64) -> String {
        let decimal = shibes as f64 / 100_000_000.0;
        if decimal >= 1.0 {
            format!("{:.2} DOGE", decimal)
        } else {
            format!("{} shibes", shibes)
        }
    }

    // ICP formatting
    #[test]
    fn test_format_icp_one_icp() {
        assert_eq!(format_icp(100_000_000), "1.0000 ICP");
    }

    #[test]
    fn test_format_icp_zero() {
        assert_eq!(format_icp(0), "0.0000 ICP");
    }

    #[test]
    fn test_format_icp_fractional() {
        assert_eq!(format_icp(50_000_000), "0.5000 ICP");
    }

    #[test]
    fn test_format_icp_always_shows_icp_label() {
        // ICP always uses the ICP label — no unit switching
        assert!(format_icp(1).ends_with(" ICP"));
        assert!(format_icp(100_000_000_000).ends_with(" ICP"));
    }

    // BTC formatting
    #[test]
    fn test_format_btc_above_threshold_shows_btc() {
        // 0.001 BTC = 100_000 sats — threshold is >=
        assert_eq!(format_btc(100_000), "0.0010 BTC");
    }

    #[test]
    fn test_format_btc_below_threshold_shows_sats() {
        // 99_999 sats < 0.001 BTC
        assert_eq!(format_btc(99_999), "99999 sats");
    }

    #[test]
    fn test_format_btc_one_btc() {
        assert_eq!(format_btc(100_000_000), "1.0000 BTC");
    }

    #[test]
    fn test_format_btc_zero_shows_sats() {
        assert_eq!(format_btc(0), "0 sats");
    }

    #[test]
    fn test_format_btc_one_sat_shows_sats() {
        assert_eq!(format_btc(1), "1 sats");
    }

    // ETH formatting
    #[test]
    fn test_format_eth_one_eth() {
        // 1 ETH = 1_000_000_000_000_000_000 wei
        assert_eq!(format_eth(1_000_000_000_000_000_000), "1.000000 ETH");
    }

    #[test]
    fn test_format_eth_above_threshold_shows_eth() {
        // 0.001 ETH = 1_000_000_000_000_000 wei
        assert_eq!(format_eth(1_000_000_000_000_000), "0.001000 ETH");
    }

    #[test]
    fn test_format_eth_below_threshold_shows_gwei() {
        // 500_000_000 wei = 0.5 Gwei
        assert_eq!(format_eth(500_000_000), "0.50 Gwei");
    }

    #[test]
    fn test_format_eth_zero_shows_gwei() {
        assert_eq!(format_eth(0), "0.00 Gwei");
    }

    // DOGE formatting
    #[test]
    fn test_format_doge_one_doge() {
        assert_eq!(format_doge(100_000_000), "1.00 DOGE");
    }

    #[test]
    fn test_format_doge_above_threshold_shows_doge() {
        // exactly 1.0 DOGE
        assert_eq!(format_doge(100_000_000), "1.00 DOGE");
    }

    #[test]
    fn test_format_doge_below_threshold_shows_shibes() {
        assert_eq!(format_doge(99_999_999), "99999999 shibes");
    }

    #[test]
    fn test_format_doge_zero_shows_shibes() {
        assert_eq!(format_doge(0), "0 shibes");
    }

    #[test]
    fn test_format_doge_one_shibe_shows_shibes() {
        assert_eq!(format_doge(1), "1 shibes");
    }

    // =========================================================================
    // TABLE CONFIG VALIDATION (validate_config)
    // =========================================================================

    #[derive(Clone)]
    struct TestTableConfig {
        small_blind: u64,
        big_blind: u64,
        min_buy_in: u64,
        max_buy_in: u64,
        max_players: u8,
        action_timeout_secs: u64,
        ante: u64,
        time_bank_secs: u64,
    }

    impl TestTableConfig {
        fn valid() -> Self {
            // A minimal valid 2-player heads-up config
            Self {
                small_blind: 50,
                big_blind: 100,
                min_buy_in: 1_000,  // 10x big blind
                max_buy_in: 10_000, // 100x big blind
                max_players: 2,
                action_timeout_secs: 30,
                ante: 0,
                time_bank_secs: 60,
            }
        }
    }

    fn validate_config(cfg: &TestTableConfig) -> Result<(), String> {
        if cfg.max_players < 2 {
            return Err("max_players must be at least 2".to_string());
        }
        if cfg.max_players > 10 {
            return Err("max_players cannot exceed 10".to_string());
        }
        if cfg.small_blind == 0 {
            return Err("small_blind must be greater than 0".to_string());
        }
        if cfg.big_blind == 0 {
            return Err("big_blind must be greater than 0".to_string());
        }
        if cfg.small_blind > cfg.big_blind {
            return Err("small_blind cannot be greater than big_blind".to_string());
        }
        if cfg.big_blind > cfg.small_blind * 10 {
            return Err("big_blind cannot be more than 10x small_blind".to_string());
        }
        if cfg.min_buy_in == 0 {
            return Err("min_buy_in must be greater than 0".to_string());
        }
        if cfg.max_buy_in < cfg.min_buy_in {
            return Err("max_buy_in must be >= min_buy_in".to_string());
        }
        if cfg.min_buy_in < cfg.big_blind * 10 {
            return Err("min_buy_in should be at least 10 big blinds".to_string());
        }
        if cfg.max_buy_in > cfg.big_blind * 1000 {
            return Err("max_buy_in cannot exceed 1000 big blinds".to_string());
        }
        if cfg.action_timeout_secs > 300 {
            return Err("action_timeout_secs cannot exceed 300 (5 minutes)".to_string());
        }
        if cfg.time_bank_secs > 600 {
            return Err("time_bank_secs cannot exceed 600 (10 minutes)".to_string());
        }
        if cfg.ante > cfg.big_blind {
            return Err("ante cannot exceed big_blind".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_config_valid_base_case() {
        assert!(validate_config(&TestTableConfig::valid()).is_ok());
    }

    #[test]
    fn test_config_max_players_too_low() {
        let mut cfg = TestTableConfig::valid();
        cfg.max_players = 1;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_max_players_minimum_is_2() {
        let mut cfg = TestTableConfig::valid();
        cfg.max_players = 2;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_max_players_too_high() {
        let mut cfg = TestTableConfig::valid();
        cfg.max_players = 11;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_max_players_maximum_is_10() {
        let mut cfg = TestTableConfig::valid();
        cfg.max_players = 10;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_small_blind_zero() {
        let mut cfg = TestTableConfig::valid();
        cfg.small_blind = 0;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_big_blind_zero() {
        let mut cfg = TestTableConfig::valid();
        cfg.big_blind = 0;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_small_blind_exceeds_big_blind() {
        let mut cfg = TestTableConfig::valid();
        cfg.small_blind = 200;
        cfg.big_blind = 100;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_equal_blinds_accepted() {
        // small == big is a valid (if unusual) straddle setup
        let mut cfg = TestTableConfig::valid();
        cfg.small_blind = 100;
        cfg.big_blind = 100;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_big_blind_more_than_10x_small_blind() {
        let mut cfg = TestTableConfig::valid();
        cfg.small_blind = 10;
        cfg.big_blind = 101; // > 10x
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_big_blind_exactly_10x_small_blind_accepted() {
        let mut cfg = TestTableConfig::valid();
        cfg.small_blind = 10;
        cfg.big_blind = 100;
        // min_buy_in must be at least 10 * 100 = 1000
        cfg.min_buy_in = 1_000;
        cfg.max_buy_in = 10_000;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_min_buy_in_zero() {
        let mut cfg = TestTableConfig::valid();
        cfg.min_buy_in = 0;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_max_buy_in_less_than_min_buy_in() {
        let mut cfg = TestTableConfig::valid();
        cfg.max_buy_in = cfg.min_buy_in - 1;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_min_buy_in_below_10_big_blinds() {
        let mut cfg = TestTableConfig::valid();
        cfg.big_blind = 100;
        cfg.min_buy_in = 999; // 10 * 100 = 1000 required
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_min_buy_in_exactly_10_big_blinds() {
        let mut cfg = TestTableConfig::valid();
        cfg.big_blind = 100;
        cfg.min_buy_in = 1_000;
        cfg.max_buy_in = 10_000;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_max_buy_in_exceeds_1000_big_blinds() {
        let mut cfg = TestTableConfig::valid();
        cfg.big_blind = 100;
        cfg.max_buy_in = 100_001; // > 1000 * 100
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_action_timeout_too_long() {
        let mut cfg = TestTableConfig::valid();
        cfg.action_timeout_secs = 301;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_action_timeout_maximum_accepted() {
        let mut cfg = TestTableConfig::valid();
        cfg.action_timeout_secs = 300;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_time_bank_too_long() {
        let mut cfg = TestTableConfig::valid();
        cfg.time_bank_secs = 601;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_time_bank_maximum_accepted() {
        let mut cfg = TestTableConfig::valid();
        cfg.time_bank_secs = 600;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_ante_exceeds_big_blind() {
        let mut cfg = TestTableConfig::valid();
        cfg.ante = cfg.big_blind + 1;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn test_config_ante_equals_big_blind_accepted() {
        let mut cfg = TestTableConfig::valid();
        cfg.ante = cfg.big_blind;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn test_config_zero_ante_accepted() {
        let mut cfg = TestTableConfig::valid();
        cfg.ante = 0;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn timing_validate_config_batch_1000() {
        let cfg = TestTableConfig::valid();
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            let _ = validate_config(&cfg);
        }
        let us = start.elapsed().as_micros();
        println!("[timing] validate_config 1000 calls: {}µs", us);
        assert!(us < 10_000, "validate_config 1000x took {}µs", us);
    }

    #[test]
    fn timing_account_id_batch_1000() {
        let principals: Vec<Vec<u8>> = (0u16..1000)
            .map(|i| vec![(i >> 8) as u8, (i & 0xff) as u8])
            .collect();
        let start = std::time::Instant::now();
        for p in &principals {
            let _ = compute_account_identifier(p, None);
        }
        let ms = start.elapsed().as_millis();
        println!("[timing] compute_account_identifier 1000 calls: {}ms", ms);
        assert!(ms < 500, "account ID 1000x took {}ms", ms);
    }

    // =========================================================================
    // BTC DEPOSIT — SUBACCOUNT DERIVATION
    // =========================================================================

    // Mirror of the canister's principal_to_subaccount() — pure function under test.
    // Uses a length-prefixed encoding: byte[0] = length, byte[1..] = principal bytes.
    fn btc_subaccount_from_bytes(principal_bytes: &[u8]) -> [u8; 32] {
        let mut subaccount = [0u8; 32];
        subaccount[0] = principal_bytes.len() as u8;
        let copy_len = principal_bytes.len().min(31);
        subaccount[1..1 + copy_len].copy_from_slice(&principal_bytes[..copy_len]);
        subaccount
    }

    // Mirror of the canister's compute_deposit_subaccount() — SHA256-based.
    fn deposit_subaccount_from_bytes(principal_bytes: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"cleardeck-deposit:");
        hasher.update(principal_bytes);
        hasher.finalize().into()
    }

    #[test]
    fn test_btc_subaccount_is_deterministic() {
        let principal = b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e";
        let a = btc_subaccount_from_bytes(principal);
        let b = btc_subaccount_from_bytes(principal);
        assert_eq!(a, b);
    }

    #[test]
    fn test_btc_subaccount_is_32_bytes() {
        let principal = b"\xab\xcd\xef\x01\x23";
        let sub = btc_subaccount_from_bytes(principal);
        assert_eq!(sub.len(), 32);
    }

    #[test]
    fn test_btc_subaccount_length_prefix() {
        // First byte must equal the length of the principal bytes
        let principal = b"\x01\x02\x03\x04\x05";
        let sub = btc_subaccount_from_bytes(principal);
        assert_eq!(sub[0], principal.len() as u8);
    }

    #[test]
    fn test_btc_subaccount_principal_bytes_embedded() {
        let principal = b"\xaa\xbb\xcc\xdd";
        let sub = btc_subaccount_from_bytes(principal);
        assert_eq!(&sub[1..5], &[0xaa, 0xbb, 0xcc, 0xdd]);
    }

    #[test]
    fn test_btc_subaccount_unique_per_principal() {
        let p1 = b"\x01\x02\x03\x04\x05";
        let p2 = b"\x01\x02\x03\x04\x06"; // differs only in last byte
        let sub1 = btc_subaccount_from_bytes(p1);
        let sub2 = btc_subaccount_from_bytes(p2);
        assert_ne!(sub1, sub2);
    }

    #[test]
    fn test_btc_subaccount_single_byte_principal() {
        let principal = b"\xff";
        let sub = btc_subaccount_from_bytes(principal);
        assert_eq!(sub[0], 1); // length = 1
        assert_eq!(sub[1], 0xff);
        assert_eq!(&sub[2..], &[0u8; 30]);
    }

    #[test]
    fn test_deposit_subaccount_is_deterministic() {
        let principal = b"\xde\xad\xbe\xef\x01\x02\x03\x04";
        let a = deposit_subaccount_from_bytes(principal);
        let b = deposit_subaccount_from_bytes(principal);
        assert_eq!(a, b);
    }

    #[test]
    fn test_deposit_subaccount_is_32_bytes() {
        let principal = b"\x01\x02";
        let sub = deposit_subaccount_from_bytes(principal);
        assert_eq!(sub.len(), 32);
    }

    #[test]
    fn test_deposit_subaccount_unique_per_principal() {
        let p1 = b"\x01\x02\x03\x04\x05";
        let p2 = b"\x05\x04\x03\x02\x01";
        let sub1 = deposit_subaccount_from_bytes(p1);
        let sub2 = deposit_subaccount_from_bytes(p2);
        assert_ne!(sub1, sub2);
    }

    #[test]
    fn test_btc_and_deposit_subaccounts_differ() {
        // The two derivation schemes must not collide — they serve different
        // purposes (ckBTC minter path vs external ICRC wallet path).
        let principal = b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a";
        let btc_sub = btc_subaccount_from_bytes(principal);
        let dep_sub = deposit_subaccount_from_bytes(principal);
        assert_ne!(btc_sub, dep_sub, "BTC and deposit subaccounts must be distinct");
    }

    // =========================================================================
    // BTC DEPOSIT — SWEEP AMOUNT CALCULATION
    // =========================================================================

    const CKBTC_FEE: u64 = 10; // satoshis

    fn sweep_amount(minted: u64) -> Option<u64> {
        if minted == 0 {
            return None;
        }
        let net = minted.saturating_sub(CKBTC_FEE);
        if net > 0 { Some(net) } else { None }
    }

    #[test]
    fn test_sweep_deducts_ckbtc_fee() {
        assert_eq!(sweep_amount(10_000), Some(9_990));
    }

    #[test]
    fn test_sweep_exact_fee_yields_none() {
        // Minted == fee → nothing to sweep
        assert_eq!(sweep_amount(CKBTC_FEE), None);
    }

    #[test]
    fn test_sweep_below_fee_yields_none() {
        assert_eq!(sweep_amount(5), None);
    }

    #[test]
    fn test_sweep_zero_minted_yields_none() {
        assert_eq!(sweep_amount(0), None);
    }

    #[test]
    fn test_sweep_one_sat_above_fee() {
        assert_eq!(sweep_amount(CKBTC_FEE + 1), Some(1));
    }

    #[test]
    fn test_sweep_large_deposit() {
        // 1 BTC = 100_000_000 sats
        assert_eq!(sweep_amount(100_000_000), Some(99_999_990));
    }

    #[test]
    fn test_sweep_saturating_sub_no_underflow() {
        // u64::MAX minted should not panic
        let result = sweep_amount(u64::MAX);
        assert_eq!(result, Some(u64::MAX - CKBTC_FEE));
    }

    // =========================================================================
    // BTC DEPOSIT — BALANCE ARITHMETIC
    // =========================================================================

    #[test]
    fn test_balance_credit_saturating_add() {
        let current: u64 = u64::MAX - 5;
        let credit: u64 = 10; // would overflow without saturating
        let result = current.saturating_add(credit);
        assert_eq!(result, u64::MAX, "saturating_add must clamp at u64::MAX");
    }

    #[test]
    fn test_balance_deduct_exact_amount() {
        let balance: u64 = 50_000;
        let withdraw: u64 = 50_000;
        assert_eq!(balance.checked_sub(withdraw), Some(0));
    }

    #[test]
    fn test_balance_deduct_insufficient_funds() {
        let balance: u64 = 9_000;
        let withdraw: u64 = 10_000;
        assert!(balance < withdraw, "withdrawal must be rejected when balance insufficient");
    }

    #[test]
    fn test_balance_refund_saturating_add() {
        // On withdrawal failure, refund must restore balance exactly
        let balance_before: u64 = 50_000;
        let deducted: u64 = 10_000;
        let after_deduct = balance_before - deducted;
        let after_refund = after_deduct.saturating_add(deducted);
        assert_eq!(after_refund, balance_before);
    }

    // =========================================================================
    // BTC DEPOSIT — WITHDRAWAL VALIDATION
    // =========================================================================

    const MIN_BTC_WITHDRAWAL: u64 = 10_000; // satoshis

    fn validate_withdraw_btc(address: &str, amount: u64, balance: u64) -> Result<(), String> {
        if address.is_empty() {
            return Err("BTC address cannot be empty".to_string());
        }
        if amount < MIN_BTC_WITHDRAWAL {
            return Err(format!("Minimum BTC withdrawal is {} satoshis", MIN_BTC_WITHDRAWAL));
        }
        if balance < amount {
            return Err(format!("Insufficient ckBTC balance. Have {} sats, need {}", balance, amount));
        }
        Ok(())
    }

    #[test]
    fn test_withdraw_btc_valid() {
        assert!(validate_withdraw_btc("bc1qvalidaddress", 10_000, 50_000).is_ok());
    }

    #[test]
    fn test_withdraw_btc_empty_address_rejected() {
        assert!(validate_withdraw_btc("", 10_000, 50_000).is_err());
    }

    #[test]
    fn test_withdraw_btc_below_minimum_rejected() {
        assert!(validate_withdraw_btc("bc1qvalidaddress", 9_999, 50_000).is_err());
    }

    #[test]
    fn test_withdraw_btc_exactly_minimum_accepted() {
        assert!(validate_withdraw_btc("bc1qvalidaddress", 10_000, 10_000).is_ok());
    }

    #[test]
    fn test_withdraw_btc_insufficient_balance_rejected() {
        assert!(validate_withdraw_btc("bc1qvalidaddress", 10_000, 5_000).is_err());
    }

    #[test]
    fn test_withdraw_btc_zero_amount_rejected() {
        assert!(validate_withdraw_btc("bc1qvalidaddress", 0, 50_000).is_err());
    }

    // =========================================================================
    // BTC DEPOSIT — TIMING TESTS
    // Measures wall-clock duration of pure, synchronous operations that run
    // on every deposit/withdrawal.  These are not hard assertions (they would
    // be flaky on slow CI runners) but they log the duration so regressions
    // are visible in `cargo test -- --nocapture`.
    // =========================================================================

    fn elapsed_micros(start: std::time::Instant) -> u128 {
        start.elapsed().as_micros()
    }

    #[test]
    fn timing_btc_subaccount_single_derivation() {
        let principal = b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e";
        let start = std::time::Instant::now();
        let _ = btc_subaccount_from_bytes(principal);
        let us = elapsed_micros(start);
        println!("[timing] btc_subaccount single derivation: {}µs", us);
        // Sanity: must complete well under 1ms on any reasonable hardware
        assert!(us < 1_000, "subaccount derivation took {}µs — unexpectedly slow", us);
    }

    #[test]
    fn timing_deposit_subaccount_single_derivation() {
        let principal = b"\xde\xad\xbe\xef\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a";
        let start = std::time::Instant::now();
        let _ = deposit_subaccount_from_bytes(principal);
        let us = elapsed_micros(start);
        println!("[timing] deposit_subaccount (SHA256) single derivation: {}µs", us);
        assert!(us < 1_000, "SHA256 subaccount derivation took {}µs — unexpectedly slow", us);
    }

    #[test]
    fn timing_btc_subaccount_batch_1000() {
        let principals: Vec<Vec<u8>> = (0u16..1000)
            .map(|i| vec![(i >> 8) as u8, (i & 0xff) as u8, 0xaa, 0xbb])
            .collect();

        let start = std::time::Instant::now();
        for p in &principals {
            let _ = btc_subaccount_from_bytes(p);
        }
        let ms = start.elapsed().as_millis();
        println!("[timing] btc_subaccount 1000 derivations: {}ms", ms);
        assert!(ms < 100, "1000 subaccount derivations took {}ms — unexpectedly slow", ms);
    }

    #[test]
    fn timing_deposit_subaccount_batch_1000() {
        let principals: Vec<Vec<u8>> = (0u16..1000)
            .map(|i| vec![(i >> 8) as u8, (i & 0xff) as u8, 0xcc, 0xdd])
            .collect();

        let start = std::time::Instant::now();
        for p in &principals {
            let _ = deposit_subaccount_from_bytes(p);
        }
        let ms = start.elapsed().as_millis();
        println!("[timing] deposit_subaccount (SHA256) 1000 derivations: {}ms", ms);
        assert!(ms < 500, "1000 SHA256 derivations took {}ms — unexpectedly slow", ms);
    }

    #[test]
    fn timing_sweep_calculation_batch_1000() {
        // Simulates processing 1000 UTXOs in a single update_btc_balance call
        let utxos: Vec<u64> = (1u64..=1000).map(|i| i * 1_000).collect();

        let start = std::time::Instant::now();
        let total: u64 = utxos.iter()
            .filter_map(|&minted| sweep_amount(minted))
            .fold(0u64, |acc, x| acc.saturating_add(x));
        let us = elapsed_micros(start);
        println!("[timing] sweep calculation 1000 UTXOs: {}µs (total={})", us, total);
        assert!(us < 10_000, "sweep calculation over 1000 UTXOs took {}µs", us);
    }

    #[test]
    fn timing_balance_arithmetic_batch_10000() {
        // Simulates 10,000 sequential balance credits (e.g. bulk deposit sweep)
        let mut balance: u64 = 0;
        let start = std::time::Instant::now();
        for i in 0u64..10_000 {
            balance = balance.saturating_add(i.saturating_mul(100));
        }
        let us = elapsed_micros(start);
        println!("[timing] balance arithmetic 10k operations: {}µs (final={})", us, balance);
        assert!(us < 10_000, "10k balance operations took {}µs", us);
    }
}
