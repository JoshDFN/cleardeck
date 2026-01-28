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
}
