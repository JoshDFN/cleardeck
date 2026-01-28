// Unit tests for lobby canister core logic
// These tests verify pure functions without IC infrastructure

// =============================================================================
// TYPE DEFINITIONS (mirror the canister types for testing)
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct TableConfig {
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_players: u8,
    pub ante: u64,
    pub action_timeout_secs: u64,
    pub time_bank_secs: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableInfo {
    pub id: u64,
    pub config: TableConfig,
    pub name: String,
    pub player_count: u8,
    pub status: TableStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TableStatus {
    WaitingForPlayers,
    InProgress,
    Paused,
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerProfile {
    pub username: String,
    pub total_winnings: i64,
    pub hands_played: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StakeLevel {
    Micro,   // 1-9 BB
    Low,     // 10-49 BB
    Medium,  // 50-199 BB
    High,    // 200-999 BB
    VIP,     // 1000+ BB
}

// =============================================================================
// LOGIC FUNCTIONS
// =============================================================================

fn get_stake_level(big_blind: u64) -> StakeLevel {
    match big_blind {
        1..=9 => StakeLevel::Micro,
        10..=49 => StakeLevel::Low,
        50..=199 => StakeLevel::Medium,
        200..=999 => StakeLevel::High,
        _ => StakeLevel::VIP,
    }
}

fn validate_username(username: &str) -> Result<(), String> {
    if username.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }
    if username.len() > 20 {
        return Err("Username must be at most 20 characters".to_string());
    }
    Ok(())
}

fn filter_tables_by_stake(tables: &[TableInfo], stake: StakeLevel) -> Vec<TableInfo> {
    let (min_bb, max_bb) = match stake {
        StakeLevel::Micro => (1, 10),
        StakeLevel::Low => (10, 50),
        StakeLevel::Medium => (50, 200),
        StakeLevel::High => (200, 1000),
        StakeLevel::VIP => (1000, u64::MAX),
    };

    tables.iter()
        .filter(|t| {
            t.status != TableStatus::Closed &&
                t.config.big_blind >= min_bb &&
                t.config.big_blind < max_bb
        })
        .cloned()
        .collect()
}

fn filter_available_tables(tables: &[TableInfo]) -> Vec<TableInfo> {
    tables.iter()
        .filter(|t| {
            t.status != TableStatus::Closed &&
                t.player_count < t.config.max_players
        })
        .cloned()
        .collect()
}

fn get_leaderboard(players: &[PlayerProfile], limit: usize) -> Vec<PlayerProfile> {
    let mut sorted = players.to_vec();
    sorted.sort_by(|a, b| b.total_winnings.cmp(&a.total_winnings));
    sorted.into_iter().take(limit).collect()
}

fn update_player_stats(profile: &mut PlayerProfile, winnings: i64, hands: u64) {
    profile.total_winnings += winnings;
    profile.hands_played += hands;
}

fn should_update_table_status(player_count: u8) -> TableStatus {
    if player_count >= 2 {
        TableStatus::InProgress
    } else {
        TableStatus::WaitingForPlayers
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // STAKE LEVEL TESTS
    // =========================================================================

    #[test]
    fn test_stake_level_micro() {
        assert_eq!(get_stake_level(1), StakeLevel::Micro);
        assert_eq!(get_stake_level(5), StakeLevel::Micro);
        assert_eq!(get_stake_level(9), StakeLevel::Micro);
    }

    #[test]
    fn test_stake_level_low() {
        assert_eq!(get_stake_level(10), StakeLevel::Low);
        assert_eq!(get_stake_level(20), StakeLevel::Low);
        assert_eq!(get_stake_level(49), StakeLevel::Low);
    }

    #[test]
    fn test_stake_level_medium() {
        assert_eq!(get_stake_level(50), StakeLevel::Medium);
        assert_eq!(get_stake_level(100), StakeLevel::Medium);
        assert_eq!(get_stake_level(199), StakeLevel::Medium);
    }

    #[test]
    fn test_stake_level_high() {
        assert_eq!(get_stake_level(200), StakeLevel::High);
        assert_eq!(get_stake_level(500), StakeLevel::High);
        assert_eq!(get_stake_level(999), StakeLevel::High);
    }

    #[test]
    fn test_stake_level_vip() {
        assert_eq!(get_stake_level(1000), StakeLevel::VIP);
        assert_eq!(get_stake_level(5000), StakeLevel::VIP);
        assert_eq!(get_stake_level(10000), StakeLevel::VIP);
    }

    // =========================================================================
    // USERNAME VALIDATION TESTS
    // =========================================================================

    #[test]
    fn test_valid_username() {
        assert!(validate_username("abc").is_ok());
        assert!(validate_username("player123").is_ok());
        assert!(validate_username("a".repeat(20).as_str()).is_ok());
    }

    #[test]
    fn test_username_too_short() {
        assert!(validate_username("").is_err());
        assert!(validate_username("a").is_err());
        assert!(validate_username("ab").is_err());
    }

    #[test]
    fn test_username_too_long() {
        assert!(validate_username("a".repeat(21).as_str()).is_err());
        assert!(validate_username("a".repeat(100).as_str()).is_err());
    }

    // =========================================================================
    // TABLE FILTER TESTS
    // =========================================================================

    fn create_test_tables() -> Vec<TableInfo> {
        vec![
            TableInfo {
                id: 1,
                config: TableConfig {
                    small_blind: 5,
                    big_blind: 10,
                    min_buy_in: 200,
                    max_buy_in: 1000,
                    max_players: 6,
                    ante: 0,
                    action_timeout_secs: 30,
                    time_bank_secs: 30,
                },
                name: "Low Stakes".to_string(),
                player_count: 3,
                status: TableStatus::InProgress,
            },
            TableInfo {
                id: 2,
                config: TableConfig {
                    small_blind: 50,
                    big_blind: 100,
                    min_buy_in: 2000,
                    max_buy_in: 10000,
                    max_players: 6,
                    ante: 0,
                    action_timeout_secs: 30,
                    time_bank_secs: 30,
                },
                name: "Medium Stakes".to_string(),
                player_count: 2,
                status: TableStatus::InProgress,
            },
            TableInfo {
                id: 3,
                config: TableConfig {
                    small_blind: 500,
                    big_blind: 1000,
                    min_buy_in: 20000,
                    max_buy_in: 100000,
                    max_players: 6,
                    ante: 0,
                    action_timeout_secs: 30,
                    time_bank_secs: 30,
                },
                name: "VIP Stakes".to_string(),
                player_count: 0,
                status: TableStatus::WaitingForPlayers,
            },
            TableInfo {
                id: 4,
                config: TableConfig {
                    small_blind: 5,
                    big_blind: 10,
                    min_buy_in: 200,
                    max_buy_in: 1000,
                    max_players: 2,
                    ante: 0,
                    action_timeout_secs: 30,
                    time_bank_secs: 30,
                },
                name: "Closed Table".to_string(),
                player_count: 0,
                status: TableStatus::Closed,
            },
        ]
    }

    #[test]
    fn test_filter_tables_by_stake_low() {
        let tables = create_test_tables();
        let filtered = filter_tables_by_stake(&tables, StakeLevel::Low);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 1);
    }

    #[test]
    fn test_filter_tables_by_stake_medium() {
        let tables = create_test_tables();
        let filtered = filter_tables_by_stake(&tables, StakeLevel::Medium);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 2);
    }

    #[test]
    fn test_filter_tables_by_stake_vip() {
        let tables = create_test_tables();
        let filtered = filter_tables_by_stake(&tables, StakeLevel::VIP);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 3);
    }

    #[test]
    fn test_filter_tables_excludes_closed() {
        let tables = create_test_tables();
        let filtered = filter_tables_by_stake(&tables, StakeLevel::Low);

        // Table 4 is Low stakes but closed
        assert!(!filtered.iter().any(|t| t.id == 4));
    }

    #[test]
    fn test_filter_available_tables() {
        let tables = create_test_tables();
        let filtered = filter_available_tables(&tables);

        // Should include tables with available seats (not closed)
        assert!(filtered.iter().any(|t| t.id == 1)); // Has 3/6 players
        assert!(filtered.iter().any(|t| t.id == 2)); // Has 2/6 players
        assert!(filtered.iter().any(|t| t.id == 3)); // Has 0/6 players
        assert!(!filtered.iter().any(|t| t.id == 4)); // Closed
    }

    #[test]
    fn test_filter_available_excludes_full_tables() {
        let mut tables = create_test_tables();
        tables[0].player_count = 6; // Make table full
        tables[0].status = TableStatus::InProgress;

        let filtered = filter_available_tables(&tables);

        // Table 1 should be excluded (full)
        assert!(!filtered.iter().any(|t| t.id == 1));
    }

    // =========================================================================
    // LEADERBOARD TESTS
    // =========================================================================

    fn create_test_players() -> Vec<PlayerProfile> {
        vec![
            PlayerProfile {
                username: "Alice".to_string(),
                total_winnings: 5000,
                hands_played: 100,
            },
            PlayerProfile {
                username: "Bob".to_string(),
                total_winnings: -1000,
                hands_played: 50,
            },
            PlayerProfile {
                username: "Charlie".to_string(),
                total_winnings: 10000,
                hands_played: 200,
            },
            PlayerProfile {
                username: "Diana".to_string(),
                total_winnings: 2500,
                hands_played: 75,
            },
        ]
    }

    #[test]
    fn test_leaderboard_sorted_by_winnings() {
        let players = create_test_players();
        let leaderboard = get_leaderboard(&players, 10);

        assert_eq!(leaderboard[0].username, "Charlie"); // 10000
        assert_eq!(leaderboard[1].username, "Alice");   // 5000
        assert_eq!(leaderboard[2].username, "Diana");   // 2500
        assert_eq!(leaderboard[3].username, "Bob");     // -1000
    }

    #[test]
    fn test_leaderboard_respects_limit() {
        let players = create_test_players();
        let leaderboard = get_leaderboard(&players, 2);

        assert_eq!(leaderboard.len(), 2);
        assert_eq!(leaderboard[0].username, "Charlie");
        assert_eq!(leaderboard[1].username, "Alice");
    }

    #[test]
    fn test_leaderboard_empty_players() {
        let players: Vec<PlayerProfile> = vec![];
        let leaderboard = get_leaderboard(&players, 10);

        assert!(leaderboard.is_empty());
    }

    // =========================================================================
    // PLAYER STATS TESTS
    // =========================================================================

    #[test]
    fn test_update_player_stats_positive_winnings() {
        let mut profile = PlayerProfile {
            username: "Test".to_string(),
            total_winnings: 1000,
            hands_played: 50,
        };

        update_player_stats(&mut profile, 500, 10);

        assert_eq!(profile.total_winnings, 1500);
        assert_eq!(profile.hands_played, 60);
    }

    #[test]
    fn test_update_player_stats_negative_winnings() {
        let mut profile = PlayerProfile {
            username: "Test".to_string(),
            total_winnings: 1000,
            hands_played: 50,
        };

        update_player_stats(&mut profile, -750, 5);

        assert_eq!(profile.total_winnings, 250);
        assert_eq!(profile.hands_played, 55);
    }

    #[test]
    fn test_update_player_stats_can_go_negative() {
        let mut profile = PlayerProfile {
            username: "Test".to_string(),
            total_winnings: 500,
            hands_played: 50,
        };

        update_player_stats(&mut profile, -1000, 10);

        assert_eq!(profile.total_winnings, -500);
        assert_eq!(profile.hands_played, 60);
    }

    // =========================================================================
    // TABLE STATUS TESTS
    // =========================================================================

    #[test]
    fn test_table_status_waiting_with_no_players() {
        assert_eq!(should_update_table_status(0), TableStatus::WaitingForPlayers);
    }

    #[test]
    fn test_table_status_waiting_with_one_player() {
        assert_eq!(should_update_table_status(1), TableStatus::WaitingForPlayers);
    }

    #[test]
    fn test_table_status_in_progress_with_two_players() {
        assert_eq!(should_update_table_status(2), TableStatus::InProgress);
    }

    #[test]
    fn test_table_status_in_progress_with_many_players() {
        assert_eq!(should_update_table_status(6), TableStatus::InProgress);
    }

    // =========================================================================
    // AUTHORIZATION TESTS (Logic Only)
    // =========================================================================

    #[test]
    fn test_authorization_list_contains() {
        let authorized: Vec<u64> = vec![1, 2, 3, 4, 5];
        assert!(authorized.contains(&3));
        assert!(!authorized.contains(&10));
    }

    #[test]
    fn test_authorization_list_add() {
        let mut authorized: Vec<u64> = vec![1, 2, 3];
        if !authorized.contains(&4) {
            authorized.push(4);
        }
        assert!(authorized.contains(&4));
        assert_eq!(authorized.len(), 4);
    }

    #[test]
    fn test_authorization_list_remove() {
        let mut authorized: Vec<u64> = vec![1, 2, 3, 4, 5];
        authorized.retain(|&x| x != 3);
        assert!(!authorized.contains(&3));
        assert_eq!(authorized.len(), 4);
    }

    #[test]
    fn test_authorization_no_duplicates() {
        let mut authorized: Vec<u64> = vec![1, 2, 3];
        if !authorized.contains(&2) {
            authorized.push(2);
        }
        assert_eq!(authorized.len(), 3); // Should not add duplicate
    }
}
