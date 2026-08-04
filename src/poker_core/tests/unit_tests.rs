//! Unit tests for the ClearDeck poker engine.
//!
//! These were previously in `src/table_canister/tests/unit_tests.rs`, where they
//! carried their OWN private re-implementation of `Suit`, `Rank`, `Card`,
//! `HandRank`, `create_deck`, `shuffle_deck`, `combinations`, `check_straight`,
//! `get_straight_high`, `evaluate_five_cards`, `evaluate_hand` and a
//! `calculate_side_pots(contributions, total_pot)` that did not even share a
//! SIGNATURE with the canister's `calculate_side_pots(&mut TableState)`.
//! They were green while proving nothing about the deployed engine, and the two
//! copies had already drifted apart (the old copies of `check_straight` and
//! `get_straight_high` did not use `detect_straight`, and the old side-pot copy
//! was missing both the "no existing side pot" branch and the f64 capping branch).
//!
//! Every assertion below is preserved verbatim from that file. The only change is
//! that they now run against the REAL engine in `poker_core`, which is the code
//! `table_canister` compiles into its wasm.

use poker_core::{
    build_side_pots, check_straight, combinations, create_deck, detect_straight,
    evaluate_five_cards, evaluate_hand, get_straight_high, shuffle_deck, Card, Contribution,
    HandRank, Rank, Suit,
};

fn card(rank: Rank, suit: Suit) -> Card {
    Card { suit, rank }
}

// =============================================================================
// DECK TESTS
// =============================================================================

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

    for rank in [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ] {
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

    assert_ne!(
        deck1, deck2,
        "Different seeds should produce different shuffles"
    );
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

// =============================================================================
// HAND EVALUATION TESTS
// =============================================================================

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
    let hole = (
        card(Rank::Nine, Suit::Spades),
        card(Rank::Eight, Suit::Spades),
    );
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
    let hole = (
        card(Rank::King, Suit::Hearts),
        card(Rank::King, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Queen, Suit::Hearts),
        card(Rank::Queen, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Eight, Suit::Hearts),
        card(Rank::Seven, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Ten, Suit::Hearts),
        card(Rank::Ten, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Jack, Suit::Hearts),
        card(Rank::Jack, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Eight, Suit::Hearts),
        card(Rank::Eight, Suit::Diamonds),
    );
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
    let hole = (
        card(Rank::Ace, Suit::Hearts),
        card(Rank::King, Suit::Diamonds),
    );
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

// =============================================================================
// SIDE POT TESTS
// =============================================================================
//
// These now call the REAL pot-splitting maths. The old copies of these tests
// called a private `calculate_side_pots(contributions, total_pot)` that existed
// only in the test file; the canister's function took `&mut TableState`. The
// old test type also carried an `is_all_in` flag that the logic never read, so
// it is gone -- no assertion depended on it.

#[test]
fn test_side_pots_no_all_in() {
    // Simple case: 3 players, all bet 100, no one all-in
    let contributions = vec![
        Contribution::new(0, 100, false),
        Contribution::new(1, 100, false),
        Contribution::new(2, 100, false),
    ];

    let pots = build_side_pots(&contributions, 300);

    assert_eq!(pots.len(), 1);
    assert_eq!(pots[0].amount, 300);
    assert_eq!(pots[0].eligible_players, vec![0, 1, 2]);
}

#[test]
fn test_side_pots_one_all_in() {
    // Player 0 all-in for 50, players 1 and 2 bet 100
    let contributions = vec![
        Contribution::new(0, 50, false),
        Contribution::new(1, 100, false),
        Contribution::new(2, 100, false),
    ];

    let pots = build_side_pots(&contributions, 250);

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
        Contribution::new(0, 25, false),
        Contribution::new(1, 50, false),
        Contribution::new(2, 100, false),
        Contribution::new(3, 100, false),
    ];

    let pots = build_side_pots(&contributions, 275);

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
        Contribution::new(0, 50, true),
        Contribution::new(1, 50, false),
        Contribution::new(2, 100, false),
    ];

    let pots = build_side_pots(&contributions, 200);

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
    let contributions: Vec<Contribution> = vec![];
    let pots = build_side_pots(&contributions, 0);
    assert!(pots.is_empty());
}

#[test]
fn test_side_pots_heads_up() {
    // Simple heads-up: both players bet 50
    let contributions = vec![Contribution::new(0, 50, false), Contribution::new(1, 50, false)];

    let pots = build_side_pots(&contributions, 100);

    assert_eq!(pots.len(), 1);
    assert_eq!(pots[0].amount, 100);
    assert_eq!(pots[0].eligible_players, vec![0, 1]);
}

// =============================================================================
// OVERFLOW PROTECTION TESTS
// =============================================================================
//
// These assert properties of the standard library, not of the engine. Kept
// verbatim so nothing is silently dropped in the move.

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

// =============================================================================
// COMBINATIONS TESTS
// =============================================================================

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

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[test]
fn test_best_hand_from_seven_cards() {
    // Test that the best 5-card hand is correctly identified from 7 cards
    let hole = (
        card(Rank::Ace, Suit::Hearts),
        card(Rank::Ace, Suit::Diamonds),
    );
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

// =============================================================================
// STRAIGHT DETECTION -- NEW COVERAGE
// =============================================================================
//
// The old test file's private `check_straight` / `get_straight_high` were written
// independently of the engine's `detect_straight`, and no test called them
// directly, so the divergence went unnoticed. These tests bind the real ones.

#[test]
fn detect_straight_finds_regular_straights() {
    assert_eq!(detect_straight(&[8, 7, 6, 5, 4]), Some(8));
    assert_eq!(detect_straight(&[14, 13, 12, 11, 10]), Some(14));
    assert_eq!(detect_straight(&[6, 5, 4, 3, 2]), Some(6));
    assert!(check_straight(&[8, 7, 6, 5, 4]));
    assert_eq!(get_straight_high(&[8, 7, 6, 5, 4]), 8);
}

#[test]
fn detect_straight_rejects_non_straights() {
    assert_eq!(detect_straight(&[14, 13, 12, 11, 9]), None);
    assert_eq!(detect_straight(&[2, 2, 3, 4, 5]), None); // only 4 distinct ranks
    assert!(!check_straight(&[14, 13, 12, 11, 9]));
    assert_eq!(get_straight_high(&[14, 13, 12, 11, 9]), 0);
}

#[test]
fn detect_straight_handles_the_wheel() {
    assert_eq!(detect_straight(&[14, 5, 4, 3, 2]), Some(5));
    assert_eq!(get_straight_high(&[14, 5, 4, 3, 2]), 5);
}

#[test]
fn detect_straight_is_order_insensitive() {
    assert_eq!(detect_straight(&[4, 8, 6, 5, 7]), Some(8));
    assert_eq!(detect_straight(&[2, 14, 4, 3, 5]), Some(5));
}

#[test]
fn detect_straight_prefers_the_wheel_over_a_higher_straight_when_both_are_present() {
    // LATENT BUG, documented not fixed: the wheel is checked BEFORE regular
    // straights, so with six-or-more ranks containing both A-2-3-4-5 and a higher
    // straight, the wheel wins and the higher straight is discarded.
    // 6-5-4-3-2-A contains a 6-high straight, yet this returns 5.
    //
    // NOT reachable from the engine today: `evaluate_five_cards` is only ever
    // called on exactly five cards (from `combinations(all_cards, 5)`), and five
    // distinct ranks cannot contain both a wheel and a higher straight. This test
    // pins the behaviour so a future caller that passes 6 or 7 ranks -- an obvious
    // "optimisation" -- fails loudly instead of silently misranking hands.
    assert_eq!(
        detect_straight(&[14, 6, 5, 4, 3, 2]),
        Some(5),
        "wheel-first ordering: see the comment above before 'fixing' this"
    );
    assert_eq!(
        detect_straight(&[14, 7, 6, 5, 4, 3, 2]),
        Some(5),
        "wheel-first ordering: see the comment above before 'fixing' this"
    );
    // Sanity: without the ace the higher straight is found correctly.
    assert_eq!(detect_straight(&[7, 6, 5, 4, 3, 2]), Some(7));
}

#[test]
fn evaluate_five_cards_matches_evaluate_hand_on_the_same_five_cards() {
    // `evaluate_hand` is `evaluate_five_cards` over all 5-subsets; on exactly
    // five cards the two must agree.
    let cards = vec![
        card(Rank::Nine, Suit::Spades),
        card(Rank::Eight, Suit::Spades),
        card(Rank::Seven, Suit::Spades),
        card(Rank::Six, Suit::Spades),
        card(Rank::Five, Suit::Spades),
    ];
    let hole = (cards[0], cards[1]);
    assert_eq!(
        evaluate_five_cards(&cards),
        evaluate_hand(&hole, &cards[2..])
    );
}
