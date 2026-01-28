// Unit tests for history canister core logic
// These tests verify pure functions without IC infrastructure

use sha2::{Sha256, Digest};
use std::collections::BTreeMap;

// =============================================================================
// TYPE DEFINITIONS (mirror the canister types for testing)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
pub struct ShuffleProofRecord {
    pub seed_hash: String,
    pub revealed_seed: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub struct PlayerStats {
    pub hands_played: u64,
    pub hands_won: u64,
    pub total_winnings: i64,
    pub biggest_pot_won: u64,
    pub showdowns_won: u64,
    pub showdowns_total: u64,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            hands_played: 0,
            hands_won: 0,
            total_winnings: 0,
            biggest_pot_won: 0,
            showdowns_won: 0,
            showdowns_total: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerHandRecord {
    pub seat: u8,
    pub starting_chips: u64,
    pub ending_chips: u64,
    pub hole_cards: Option<(Card, Card)>,
    pub final_hand_rank: Option<HandRank>,
    pub amount_won: u64,
}

#[derive(Clone, Debug)]
pub struct HandSummary {
    pub hand_id: u64,
    pub hand_number: u64,
    pub timestamp: u64,
    pub player_count: u8,
    pub total_pot: u64,
    pub went_to_showdown: bool,
}

// =============================================================================
// LOGIC FUNCTIONS
// =============================================================================

mod hex {
    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if s.len() % 2 != 0 {
            return Err(());
        }

        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }
}

fn verify_shuffle_proof(proof: &ShuffleProofRecord) -> Result<bool, String> {
    let seed_bytes = hex::decode(&proof.revealed_seed)
        .map_err(|_| "Invalid revealed seed hex".to_string())?;

    let mut hasher = Sha256::new();
    hasher.update(&seed_bytes);
    let computed_hash = hex::encode(hasher.finalize());

    Ok(computed_hash == proof.seed_hash)
}

fn update_player_stats(stats: &mut PlayerStats, player: &PlayerHandRecord, went_to_showdown: bool) {
    stats.hands_played += 1;

    let profit = player.ending_chips as i64 - player.starting_chips as i64;
    stats.total_winnings += profit;

    if player.amount_won > 0 {
        stats.hands_won += 1;
        if player.amount_won > stats.biggest_pot_won {
            stats.biggest_pot_won = player.amount_won;
        }
    }

    // Check if went to showdown
    if went_to_showdown && player.hole_cards.is_some() {
        stats.showdowns_total += 1;
        if player.amount_won > 0 {
            stats.showdowns_won += 1;
        }
    }
}

fn create_hand_summary(
    hand_id: u64,
    hand_number: u64,
    timestamp: u64,
    players: &[PlayerHandRecord],
    total_pot: u64,
    went_to_showdown: bool,
) -> HandSummary {
    HandSummary {
        hand_id,
        hand_number,
        timestamp,
        player_count: players.len() as u8,
        total_pot,
        went_to_showdown,
    }
}

fn paginate<T: Clone>(items: &[T], offset: usize, limit: usize) -> Vec<T> {
    items.iter()
        .rev()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // HEX ENCODING TESTS
    // =========================================================================

    #[test]
    fn test_hex_encode() {
        let bytes = vec![0x00, 0x01, 0x0f, 0xff];
        assert_eq!(hex::encode(&bytes), "00010fff");
    }

    #[test]
    fn test_hex_decode() {
        let hex_str = "00010fff";
        let result = hex::decode(hex_str).unwrap();
        assert_eq!(result, vec![0x00, 0x01, 0x0f, 0xff]);
    }

    #[test]
    fn test_hex_decode_odd_length() {
        let hex_str = "00010ff"; // 7 chars - invalid
        assert!(hex::decode(hex_str).is_err());
    }

    #[test]
    fn test_hex_decode_invalid_chars() {
        let hex_str = "gggg";
        assert!(hex::decode(hex_str).is_err());
    }

    #[test]
    fn test_hex_roundtrip() {
        let original = vec![1, 2, 3, 4, 5, 255, 128, 0];
        let encoded = hex::encode(&original);
        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    // =========================================================================
    // SHUFFLE PROOF VERIFICATION TESTS
    // =========================================================================

    #[test]
    fn test_verify_valid_shuffle_proof() {
        // Create a valid seed and hash
        let seed = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let revealed_seed = hex::encode(&seed);

        let mut hasher = Sha256::new();
        hasher.update(&seed);
        let seed_hash = hex::encode(hasher.finalize());

        let proof = ShuffleProofRecord {
            seed_hash,
            revealed_seed,
            timestamp: 1234567890,
        };

        let result = verify_shuffle_proof(&proof);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_invalid_shuffle_proof() {
        let proof = ShuffleProofRecord {
            seed_hash: "aaaa".repeat(16), // Fake hash
            revealed_seed: hex::encode(vec![1, 2, 3, 4]),
            timestamp: 1234567890,
        };

        let result = verify_shuffle_proof(&proof);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Hash doesn't match
    }

    #[test]
    fn test_verify_shuffle_proof_invalid_hex() {
        let proof = ShuffleProofRecord {
            seed_hash: "abcdef".to_string(),
            revealed_seed: "gggg".to_string(), // Invalid hex
            timestamp: 1234567890,
        };

        let result = verify_shuffle_proof(&proof);
        assert!(result.is_err());
    }

    // =========================================================================
    // PLAYER STATS UPDATE TESTS
    // =========================================================================

    fn card(rank: Rank, suit: Suit) -> Card {
        Card { suit, rank }
    }

    #[test]
    fn test_update_stats_winning_hand() {
        let mut stats = PlayerStats::default();
        let player = PlayerHandRecord {
            seat: 0,
            starting_chips: 1000,
            ending_chips: 1500,
            hole_cards: Some((card(Rank::Ace, Suit::Hearts), card(Rank::King, Suit::Hearts))),
            final_hand_rank: Some(HandRank::Pair(14, vec![13, 12, 11])),
            amount_won: 500,
        };

        update_player_stats(&mut stats, &player, true);

        assert_eq!(stats.hands_played, 1);
        assert_eq!(stats.hands_won, 1);
        assert_eq!(stats.total_winnings, 500);
        assert_eq!(stats.biggest_pot_won, 500);
        assert_eq!(stats.showdowns_total, 1);
        assert_eq!(stats.showdowns_won, 1);
    }

    #[test]
    fn test_update_stats_losing_hand() {
        let mut stats = PlayerStats::default();
        let player = PlayerHandRecord {
            seat: 0,
            starting_chips: 1000,
            ending_chips: 700,
            hole_cards: Some((card(Rank::Two, Suit::Hearts), card(Rank::Three, Suit::Hearts))),
            final_hand_rank: Some(HandRank::HighCard(vec![14, 13, 12, 11, 10])),
            amount_won: 0,
        };

        update_player_stats(&mut stats, &player, true);

        assert_eq!(stats.hands_played, 1);
        assert_eq!(stats.hands_won, 0);
        assert_eq!(stats.total_winnings, -300);
        assert_eq!(stats.biggest_pot_won, 0);
        assert_eq!(stats.showdowns_total, 1);
        assert_eq!(stats.showdowns_won, 0);
    }

    #[test]
    fn test_update_stats_folded_before_showdown() {
        let mut stats = PlayerStats::default();
        let player = PlayerHandRecord {
            seat: 0,
            starting_chips: 1000,
            ending_chips: 950, // Lost blinds
            hole_cards: None, // Didn't show cards (folded)
            final_hand_rank: None,
            amount_won: 0,
        };

        update_player_stats(&mut stats, &player, true);

        assert_eq!(stats.hands_played, 1);
        assert_eq!(stats.hands_won, 0);
        assert_eq!(stats.total_winnings, -50);
        assert_eq!(stats.showdowns_total, 0); // Didn't go to showdown
    }

    #[test]
    fn test_update_stats_biggest_pot_tracked() {
        let mut stats = PlayerStats::default();

        // First win
        let player1 = PlayerHandRecord {
            seat: 0,
            starting_chips: 1000,
            ending_chips: 1300,
            hole_cards: None,
            final_hand_rank: None,
            amount_won: 300,
        };
        update_player_stats(&mut stats, &player1, false);

        assert_eq!(stats.biggest_pot_won, 300);

        // Smaller win
        let player2 = PlayerHandRecord {
            seat: 0,
            starting_chips: 1300,
            ending_chips: 1400,
            hole_cards: None,
            final_hand_rank: None,
            amount_won: 100,
        };
        update_player_stats(&mut stats, &player2, false);

        assert_eq!(stats.biggest_pot_won, 300); // Still 300 (bigger)

        // Bigger win
        let player3 = PlayerHandRecord {
            seat: 0,
            starting_chips: 1400,
            ending_chips: 2000,
            hole_cards: None,
            final_hand_rank: None,
            amount_won: 600,
        };
        update_player_stats(&mut stats, &player3, false);

        assert_eq!(stats.biggest_pot_won, 600); // Now 600
    }

    #[test]
    fn test_update_stats_cumulative() {
        let mut stats = PlayerStats::default();

        for i in 0..10 {
            let won = i % 2 == 0;
            let player = PlayerHandRecord {
                seat: 0,
                starting_chips: 1000,
                ending_chips: if won { 1100 } else { 900 },
                hole_cards: None,
                final_hand_rank: None,
                amount_won: if won { 100 } else { 0 },
            };
            update_player_stats(&mut stats, &player, false);
        }

        assert_eq!(stats.hands_played, 10);
        assert_eq!(stats.hands_won, 5); // Won every other hand
        // Net: 5 wins * 100 - 5 losses * 100 = 0
        assert_eq!(stats.total_winnings, 0);
    }

    // =========================================================================
    // HAND SUMMARY TESTS
    // =========================================================================

    #[test]
    fn test_create_hand_summary() {
        let players = vec![
            PlayerHandRecord {
                seat: 0,
                starting_chips: 1000,
                ending_chips: 1500,
                hole_cards: None,
                final_hand_rank: None,
                amount_won: 500,
            },
            PlayerHandRecord {
                seat: 1,
                starting_chips: 1000,
                ending_chips: 500,
                hole_cards: None,
                final_hand_rank: None,
                amount_won: 0,
            },
        ];

        let summary = create_hand_summary(1, 42, 1234567890, &players, 500, true);

        assert_eq!(summary.hand_id, 1);
        assert_eq!(summary.hand_number, 42);
        assert_eq!(summary.timestamp, 1234567890);
        assert_eq!(summary.player_count, 2);
        assert_eq!(summary.total_pot, 500);
        assert!(summary.went_to_showdown);
    }

    // =========================================================================
    // PAGINATION TESTS
    // =========================================================================

    #[test]
    fn test_paginate_basic() {
        let items: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result = paginate(&items, 0, 3);

        // Returns newest first (reversed), takes 3
        assert_eq!(result, vec![10, 9, 8]);
    }

    #[test]
    fn test_paginate_with_offset() {
        let items: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let result = paginate(&items, 2, 3);

        // Skip 2 from end, take 3
        assert_eq!(result, vec![8, 7, 6]);
    }

    #[test]
    fn test_paginate_limit_exceeds_size() {
        let items: Vec<i32> = vec![1, 2, 3];
        let result = paginate(&items, 0, 10);

        assert_eq!(result, vec![3, 2, 1]);
    }

    #[test]
    fn test_paginate_offset_exceeds_size() {
        let items: Vec<i32> = vec![1, 2, 3];
        let result = paginate(&items, 10, 5);

        assert!(result.is_empty());
    }

    #[test]
    fn test_paginate_empty() {
        let items: Vec<i32> = vec![];
        let result = paginate(&items, 0, 10);

        assert!(result.is_empty());
    }

    // =========================================================================
    // INDEX TESTS (logic for hands_by_table and hands_by_player)
    // =========================================================================

    #[test]
    fn test_index_insert_and_lookup() {
        let mut hands_by_player: BTreeMap<u64, Vec<u64>> = BTreeMap::new();

        // Player 1 plays hand 1, 3, 5
        hands_by_player.entry(1).or_default().push(1);
        hands_by_player.entry(1).or_default().push(3);
        hands_by_player.entry(1).or_default().push(5);

        // Player 2 plays hand 2, 3, 4
        hands_by_player.entry(2).or_default().push(2);
        hands_by_player.entry(2).or_default().push(3);
        hands_by_player.entry(2).or_default().push(4);

        assert_eq!(hands_by_player.get(&1).unwrap(), &vec![1, 3, 5]);
        assert_eq!(hands_by_player.get(&2).unwrap(), &vec![2, 3, 4]);
        assert!(hands_by_player.get(&3).is_none());
    }

    #[test]
    fn test_index_preserves_order() {
        let mut hands_by_table: BTreeMap<u64, Vec<u64>> = BTreeMap::new();

        for hand_id in 1..=100 {
            hands_by_table.entry(1).or_default().push(hand_id);
        }

        let hand_ids = hands_by_table.get(&1).unwrap();
        assert_eq!(hand_ids.len(), 100);
        assert_eq!(hand_ids[0], 1); // First hand
        assert_eq!(hand_ids[99], 100); // Last hand
    }

    // =========================================================================
    // AUTHORIZATION TESTS (Logic Only)
    // =========================================================================

    #[test]
    fn test_authorized_tables_logic() {
        let mut authorized_tables: Vec<u64> = vec![];

        // Check empty list
        assert!(!authorized_tables.contains(&1));

        // Add table
        authorized_tables.push(1);
        assert!(authorized_tables.contains(&1));
        assert!(!authorized_tables.contains(&2));

        // Add another
        authorized_tables.push(2);
        assert!(authorized_tables.contains(&1));
        assert!(authorized_tables.contains(&2));

        // Remove table
        authorized_tables.retain(|&t| t != 1);
        assert!(!authorized_tables.contains(&1));
        assert!(authorized_tables.contains(&2));
    }

    #[test]
    fn test_authorization_check() {
        let authorized_tables: Vec<u64> = vec![1, 2, 3];
        let admin: Option<u64> = Some(99);

        // Helper function to check auth
        fn is_authorized(caller: u64, authorized: &[u64], admin: Option<u64>) -> bool {
            authorized.contains(&caller) || admin == Some(caller)
        }

        // Authorized table
        assert!(is_authorized(1, &authorized_tables, admin));

        // Admin
        assert!(is_authorized(99, &authorized_tables, admin));

        // Unauthorized
        assert!(!is_authorized(50, &authorized_tables, admin));
    }
}
