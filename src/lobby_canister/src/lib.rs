// ============================================================================
// ⚠️  CRITICAL DEPLOYMENT WARNING ⚠️
// ============================================================================
// When upgrading canisters:
//
// ✅ ALWAYS use: dfx canister install <name> --mode upgrade --network ic
// ❌ NEVER use:  dfx canister install <name> --mode reinstall --network ic
//
// --mode reinstall DESTROYS ALL STATE!
// ============================================================================

use candid::{CandidType, Deserialize, Principal};
use std::cell::RefCell;
use std::collections::HashMap;

// ============================================================================
// TYPES
// ============================================================================

/// Currency type for tables - matches table_canister Currency enum
#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Currency {
    #[default]
    ICP,
    BTC,
}

impl Currency {
    /// Get display symbol for UI
    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::ICP => "ICP",
            Currency::BTC => "BTC",
        }
    }

    /// Get unit name for display
    pub fn unit_name(&self) -> &'static str {
        match self {
            Currency::ICP => "e8s",
            Currency::BTC => "sats",
        }
    }
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableConfig {
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_players: u8,
    pub ante: u64,
    pub action_timeout_secs: u64,
    pub time_bank_secs: u64,
    #[serde(default)]
    pub currency: Currency,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableInfo {
    pub id: u64,
    pub canister_id: Option<Principal>,
    pub config: TableConfig,
    pub name: String,
    pub player_count: u8,
    pub status: TableStatus,
    pub created_at: u64,
    pub created_by: Principal,
    #[serde(default)]
    pub currency: Currency,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq)]
pub enum TableStatus {
    WaitingForPlayers,
    InProgress,
    Paused,
    Closed,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct PlayerProfile {
    pub principal: Principal,
    pub username: String,
    pub total_winnings: i64,
    pub hands_played: u64,
    pub created_at: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StakeLevel {
    Micro,   // 1/2
    Low,     // 5/10
    Medium,  // 25/50
    High,    // 100/200
    VIP,     // 500/1000
}

// ============================================================================
// STATE
// ============================================================================

thread_local! {
    static TABLES: RefCell<HashMap<u64, TableInfo>> = RefCell::new(HashMap::new());
    static PLAYERS: RefCell<HashMap<Principal, PlayerProfile>> = RefCell::new(HashMap::new());
    static ADMIN: RefCell<Option<Principal>> = RefCell::new(None);
    static INITIALIZED: RefCell<bool> = RefCell::new(false);
    // Track which canister principals are authorized to update stats
    static AUTHORIZED_TABLES: RefCell<Vec<Principal>> = RefCell::new(Vec::new());
}

/// Check if caller is an authorized table canister
fn is_authorized_table() -> bool {
    let caller = ic_cdk::api::msg_caller();
    AUTHORIZED_TABLES.with(|a| a.borrow().contains(&caller))
}

/// Check if caller is admin
fn is_admin() -> bool {
    let caller = ic_cdk::api::msg_caller();
    ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false))
}

// ============================================================================
// INITIALIZATION
// ============================================================================

#[ic_cdk::init]
fn init() {
    ADMIN.with(|a| {
        *a.borrow_mut() = Some(ic_cdk::api::msg_caller());
    });
}

/// Set admin - only callable if no admin is set (recovery case) or by current admin
/// This is needed to recover admin access after reinstalls
#[ic_cdk::update]
fn set_admin(new_admin: Principal) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();

    // Allow if no admin is set (recovery case) or if caller is current admin
    let is_current_admin = ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false));
    let no_admin_set = ADMIN.with(|a| a.borrow().is_none());

    if !is_current_admin && !no_admin_set {
        return Err("Only current admin can set new admin".to_string());
    }

    ADMIN.with(|a| {
        *a.borrow_mut() = Some(new_admin);
    });

    Ok(())
}

/// Get admin principal (query)
#[ic_cdk::query]
fn get_admin() -> Option<Principal> {
    ADMIN.with(|a| *a.borrow())
}

/// Initialize default tables - call this after deploying table canisters
/// Legacy 2-table setup
#[ic_cdk::update]
fn init_default_tables(headsup_canister: Principal, sixmax_canister: Principal) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let is_admin = ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false));

    if !is_admin {
        return Err("Only admin can initialize tables".to_string());
    }

    let timestamp = ic_cdk::api::time();

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();

        // Clear existing tables
        tables.clear();

        // Table 1: Heads Up - 5/10 blinds, 2 players
        tables.insert(1, TableInfo {
            id: 1,
            canister_id: Some(headsup_canister),
            config: TableConfig {
                small_blind: 5,
                big_blind: 10,
                min_buy_in: 200,
                max_buy_in: 1000,
                max_players: 2,
                ante: 0,
                action_timeout_secs: 30,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            name: "Heads Up - 5/10".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::ICP,
        });

        // Table 2: 6-Max - 10/20 blinds, 6 players
        tables.insert(2, TableInfo {
            id: 2,
            canister_id: Some(sixmax_canister),
            config: TableConfig {
                small_blind: 10,
                big_blind: 20,
                min_buy_in: 400,
                max_buy_in: 2000,
                max_players: 6,
                ante: 0,
                action_timeout_secs: 60,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            name: "6-Max - 10/20".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::ICP,
        });
    });

    INITIALIZED.with(|i| *i.borrow_mut() = true);

    Ok(())
}

/// Initialize 3 microstakes tables for mainnet beta
/// All tables use 0.01/0.02 blinds with 2 ICP min buy-in
/// Order: Heads Up, 6-Max, 9-Max
#[ic_cdk::update]
fn init_microstakes_tables(
    table1_canister: Principal,
    table2_canister: Principal,
    table3_canister: Principal
) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let is_admin = ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false));

    if !is_admin {
        return Err("Only admin can initialize tables".to_string());
    }

    let timestamp = ic_cdk::api::time();

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();

        // Clear existing tables
        tables.clear();

        // Table 1: Heads Up - 0.01/0.02 ICP blinds
        // Buy-in: 2-10 ICP (100 BB)
        // Values in e8s: 1 ICP = 100_000_000 e8s
        tables.insert(1, TableInfo {
            id: 1,
            canister_id: Some(table1_canister),
            config: TableConfig {
                small_blind: 1_000_000,      // 0.01 ICP
                big_blind: 2_000_000,        // 0.02 ICP
                min_buy_in: 200_000_000,     // 2 ICP
                max_buy_in: 1_000_000_000,   // 10 ICP
                max_players: 2,
                ante: 0,
                action_timeout_secs: 30,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            name: "Heads Up - 0.01/0.02".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::ICP,
        });

        // Table 2: 6-Max - 0.01/0.02 ICP blinds
        // Buy-in: 2-10 ICP (100 BB)
        tables.insert(2, TableInfo {
            id: 2,
            canister_id: Some(table2_canister),
            config: TableConfig {
                small_blind: 1_000_000,       // 0.01 ICP
                big_blind: 2_000_000,         // 0.02 ICP
                min_buy_in: 200_000_000,      // 2 ICP
                max_buy_in: 1_000_000_000,    // 10 ICP
                max_players: 6,
                ante: 0,
                action_timeout_secs: 45,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            name: "6-Max - 0.01/0.02".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::ICP,
        });

        // Table 3: 9-Max - 0.01/0.02 ICP blinds
        // Buy-in: 2-10 ICP (100 BB)
        tables.insert(3, TableInfo {
            id: 3,
            canister_id: Some(table3_canister),
            config: TableConfig {
                small_blind: 1_000_000,       // 0.01 ICP
                big_blind: 2_000_000,         // 0.02 ICP
                min_buy_in: 200_000_000,      // 2 ICP
                max_buy_in: 1_000_000_000,    // 10 ICP
                max_players: 9,
                ante: 0,
                action_timeout_secs: 60,
                time_bank_secs: 30,
                currency: Currency::ICP,
            },
            name: "9-Max - 0.01/0.02".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::ICP,
        });
    });

    INITIALIZED.with(|i| *i.borrow_mut() = true);

    Ok(())
}

/// Add a single BTC heads-up table
/// All values in satoshis (1 BTC = 100,000,000 sats)
#[ic_cdk::update]
fn add_btc_headsup_table(btc_table_canister: Principal) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let is_admin = ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false));

    if !is_admin {
        return Err("Only admin can add tables".to_string());
    }

    let timestamp = ic_cdk::api::time();

    // Get next table ID (after existing tables)
    let next_id = TABLES.with(|t| {
        t.borrow().keys().max().map(|m| m + 1).unwrap_or(1)
    });

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();

        // BTC Table: Heads Up - 100/200 sats (micro stakes)
        // Buy-in: 10,000-100,000 sats (0.0001-0.001 BTC)
        tables.insert(next_id, TableInfo {
            id: next_id,
            canister_id: Some(btc_table_canister),
            config: TableConfig {
                small_blind: 100,         // 100 sats
                big_blind: 200,           // 200 sats
                min_buy_in: 10_000,       // 10,000 sats (0.0001 BTC)
                max_buy_in: 100_000,      // 100,000 sats (0.001 BTC)
                max_players: 2,
                ante: 0,
                action_timeout_secs: 30,
                time_bank_secs: 30,
                currency: Currency::BTC,
            },
            name: "Heads Up - 100/200".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::BTC,
        });
    });

    Ok(())
}

/// Initialize 3 BTC (ckBTC) tables for Bitcoin players
/// All values in satoshis (1 BTC = 100,000,000 sats)
#[ic_cdk::update]
fn init_btc_tables(
    btc_table1_canister: Principal,
    btc_table2_canister: Principal,
    btc_table3_canister: Principal
) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    let is_admin = ADMIN.with(|a| a.borrow().map(|admin| admin == caller).unwrap_or(false));

    if !is_admin {
        return Err("Only admin can initialize tables".to_string());
    }

    let timestamp = ic_cdk::api::time();

    // Get next table ID (after existing tables)
    let next_id = TABLES.with(|t| {
        t.borrow().keys().max().map(|m| m + 1).unwrap_or(1)
    });

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();

        // BTC Table 1: Heads Up - 100/200 sats (micro stakes)
        // Buy-in: 10,000-100,000 sats (0.0001-0.001 BTC)
        tables.insert(next_id, TableInfo {
            id: next_id,
            canister_id: Some(btc_table1_canister),
            config: TableConfig {
                small_blind: 100,         // 100 sats
                big_blind: 200,           // 200 sats
                min_buy_in: 10_000,       // 10,000 sats (0.0001 BTC)
                max_buy_in: 100_000,      // 100,000 sats (0.001 BTC)
                max_players: 2,
                ante: 0,
                action_timeout_secs: 30,
                time_bank_secs: 30,
                currency: Currency::BTC,
            },
            name: "Heads Up - 100/200".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::BTC,
        });

        // BTC Table 2: 6-Max - 500/1000 sats
        // Buy-in: 50,000-500,000 sats (0.0005-0.005 BTC)
        tables.insert(next_id + 1, TableInfo {
            id: next_id + 1,
            canister_id: Some(btc_table2_canister),
            config: TableConfig {
                small_blind: 500,          // 500 sats
                big_blind: 1_000,          // 1,000 sats
                min_buy_in: 50_000,        // 50,000 sats (0.0005 BTC)
                max_buy_in: 500_000,       // 500,000 sats (0.005 BTC)
                max_players: 6,
                ante: 0,
                action_timeout_secs: 45,
                time_bank_secs: 30,
                currency: Currency::BTC,
            },
            name: "6-Max - 500/1000".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::BTC,
        });

        // BTC Table 3: 9-Max - 1000/2000 sats
        // Buy-in: 100,000-1,000,000 sats (0.001-0.01 BTC)
        tables.insert(next_id + 2, TableInfo {
            id: next_id + 2,
            canister_id: Some(btc_table3_canister),
            config: TableConfig {
                small_blind: 1_000,         // 1,000 sats
                big_blind: 2_000,           // 2,000 sats
                min_buy_in: 100_000,        // 100,000 sats (0.001 BTC)
                max_buy_in: 1_000_000,      // 1,000,000 sats (0.01 BTC)
                max_players: 9,
                ante: 0,
                action_timeout_secs: 60,
                time_bank_secs: 30,
                currency: Currency::BTC,
            },
            name: "9-Max - 1000/2000".to_string(),
            player_count: 0,
            status: TableStatus::WaitingForPlayers,
            created_at: timestamp,
            created_by: caller,
            currency: Currency::BTC,
        });
    });

    Ok(())
}

/// Get tables filtered by currency
#[ic_cdk::query]
fn get_tables_by_currency(currency: Currency) -> Vec<TableInfo> {
    TABLES.with(|tables| {
        tables.borrow()
            .values()
            .filter(|t| t.status != TableStatus::Closed && t.currency == currency)
            .cloned()
            .collect()
    })
}

// ============================================================================
// TABLE MANAGEMENT
// ============================================================================

/// Update a table's name (admin only)
#[ic_cdk::update]
fn update_table_name(table_id: u64, new_name: String) -> Result<(), String> {
    if !is_admin() {
        return Err("Unauthorized: admin only".to_string());
    }

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();
        let table = tables.get_mut(&table_id).ok_or("Table not found")?;
        table.name = new_name;
        Ok(())
    })
}

/// Update player count for a table (called by table canister)
/// SECURITY: Only authorized table canisters or admin can update
#[ic_cdk::update]
fn update_player_count(table_id: u64, count: u8) -> Result<(), String> {
    // Verify caller is authorized
    if !is_authorized_table() && !is_admin() {
        return Err("Unauthorized: only registered table canisters can update player count".to_string());
    }

    TABLES.with(|tables| {
        let mut tables = tables.borrow_mut();
        let table = tables.get_mut(&table_id).ok_or("Table not found")?;

        table.player_count = count;

        // Update status based on player count, but NEVER reactivate a Closed table
        // Closed tables should stay closed until explicitly reopened by admin
        if table.status != TableStatus::Closed {
            if count >= 2 {
                table.status = TableStatus::InProgress;
            } else {
                table.status = TableStatus::WaitingForPlayers;
            }
        }

        Ok(())
    })
}

// ============================================================================
// PLAYER PROFILES
// ============================================================================

/// Validate username for allowed characters and reserved names
fn validate_username(username: &str) -> Result<(), String> {
    // Length check
    if username.len() < 3 || username.len() > 20 {
        return Err("Username must be 3-20 characters".to_string());
    }

    // Must be ASCII alphanumeric with underscores (no Unicode tricks)
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Username can only contain letters, numbers, and underscores".to_string());
    }

    // Must start with a letter
    if !username.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Err("Username must start with a letter".to_string());
    }

    // Reserved names (case-insensitive)
    let reserved = ["admin", "moderator", "mod", "system", "support", "help",
                    "cleardeck", "official", "staff", "bot", "dealer", "house"];
    let lower = username.to_ascii_lowercase();
    if reserved.iter().any(|r| lower.contains(r)) {
        return Err("This username is reserved".to_string());
    }

    Ok(())
}

/// Register or update player profile
#[ic_cdk::update]
fn register_player(username: String) -> Result<PlayerProfile, String> {
    let caller = ic_cdk::api::msg_caller();
    let timestamp = ic_cdk::api::time();

    // Validate username format and content
    validate_username(&username)?;

    // Atomically check username availability and register/update profile
    // This prevents TOCTOU race conditions where two concurrent requests
    // could both claim the same username
    PLAYERS.with(|players| {
        let mut players = players.borrow_mut();

        // Check if username is taken by another user
        let username_taken = players.values().any(|p| p.username == username && p.principal != caller);
        if username_taken {
            return Err("Username already taken".to_string());
        }

        // Update existing profile or create new one
        if let Some(existing) = players.get_mut(&caller) {
            existing.username = username;
            Ok(existing.clone())
        } else {
            let new_profile = PlayerProfile {
                principal: caller,
                username,
                total_winnings: 0,
                hands_played: 0,
                created_at: timestamp,
            };
            players.insert(caller, new_profile.clone());
            Ok(new_profile)
        }
    })
}

/// Update player stats (called by table canister after hand completes)
/// SECURITY: Only authorized table canisters or admin can update
#[ic_cdk::update]
fn update_player_stats(player: Principal, winnings: i64, hands: u64) -> Result<(), String> {
    // Verify caller is authorized
    if !is_authorized_table() && !is_admin() {
        return Err("Unauthorized: only registered table canisters can update player stats".to_string());
    }

    PLAYERS.with(|players| {
        let mut players = players.borrow_mut();
        if let Some(profile) = players.get_mut(&player) {
            // Use saturating operations to prevent overflow
            profile.total_winnings = profile.total_winnings.saturating_add(winnings);
            profile.hands_played = profile.hands_played.saturating_add(hands);
            Ok(())
        } else {
            Err("Player not found".to_string())
        }
    })
}

// ============================================================================
// QUERIES
// ============================================================================

/// Get all active tables
#[ic_cdk::query]
fn get_tables() -> Vec<TableInfo> {
    TABLES.with(|tables| {
        tables.borrow()
            .values()
            .filter(|t| t.status != TableStatus::Closed)
            .cloned()
            .collect()
    })
}

/// Get tables by stake level
#[ic_cdk::query]
fn get_tables_by_stake(stake: StakeLevel) -> Vec<TableInfo> {
    let (min_bb, max_bb) = match stake {
        StakeLevel::Micro => (1, 10),
        StakeLevel::Low => (10, 50),
        StakeLevel::Medium => (50, 200),
        StakeLevel::High => (200, 1000),
        StakeLevel::VIP => (1000, u64::MAX),
    };

    TABLES.with(|tables| {
        tables.borrow()
            .values()
            .filter(|t| {
                t.status != TableStatus::Closed &&
                    t.config.big_blind >= min_bb &&
                    t.config.big_blind < max_bb
            })
            .cloned()
            .collect()
    })
}

/// Get tables with available seats
#[ic_cdk::query]
fn get_available_tables() -> Vec<TableInfo> {
    TABLES.with(|tables| {
        tables.borrow()
            .values()
            .filter(|t| {
                t.status != TableStatus::Closed &&
                    t.player_count < t.config.max_players
            })
            .cloned()
            .collect()
    })
}

/// Get a specific table
#[ic_cdk::query]
fn get_table(table_id: u64) -> Option<TableInfo> {
    TABLES.with(|tables| {
        tables.borrow().get(&table_id).cloned()
    })
}

/// Get player profile
#[ic_cdk::query]
fn get_player(principal: Principal) -> Option<PlayerProfile> {
    PLAYERS.with(|players| {
        players.borrow().get(&principal).cloned()
    })
}

/// Get my profile
#[ic_cdk::query]
fn get_my_profile() -> Option<PlayerProfile> {
    let caller = ic_cdk::api::msg_caller();
    PLAYERS.with(|players| {
        players.borrow().get(&caller).cloned()
    })
}

/// Get leaderboard (top players by winnings)
/// Limit is capped at 100 to prevent excessive memory usage
#[ic_cdk::query]
fn get_leaderboard(limit: usize) -> Vec<PlayerProfile> {
    // Cap limit to prevent memory exhaustion attacks
    let capped_limit = limit.min(100);

    PLAYERS.with(|players| {
        let mut profiles: Vec<_> = players.borrow().values().cloned().collect();
        profiles.sort_by(|a, b| b.total_winnings.cmp(&a.total_winnings));
        profiles.into_iter().take(capped_limit).collect()
    })
}

/// Get total stats
#[ic_cdk::query]
fn get_stats() -> (u64, u64, u64) {
    let table_count = TABLES.with(|t| t.borrow().len() as u64);
    let player_count = PLAYERS.with(|p| p.borrow().len() as u64);
    let active_tables = TABLES.with(|t| {
        t.borrow().values().filter(|table| table.status == TableStatus::InProgress).count() as u64
    });

    (table_count, player_count, active_tables)
}

/// Check if caller is admin (safe to expose - doesn't reveal admin identity)
#[ic_cdk::query]
fn is_caller_admin() -> bool {
    is_admin()
}

/// Check if tables are initialized
#[ic_cdk::query]
fn is_initialized() -> bool {
    INITIALIZED.with(|i| *i.borrow())
}

// ============================================================================
// ADMIN FUNCTIONS - Table Authorization
// ============================================================================

/// Add an authorized table canister (admin only)
#[ic_cdk::update]
fn add_authorized_table(table_principal: Principal) -> Result<(), String> {
    if !is_admin() {
        return Err("Unauthorized: admin only".to_string());
    }

    AUTHORIZED_TABLES.with(|a| {
        let mut tables = a.borrow_mut();
        if !tables.contains(&table_principal) {
            tables.push(table_principal);
        }
    });

    Ok(())
}

/// Remove an authorized table canister (admin only)
#[ic_cdk::update]
fn remove_authorized_table(table_principal: Principal) -> Result<(), String> {
    if !is_admin() {
        return Err("Unauthorized: admin only".to_string());
    }

    AUTHORIZED_TABLES.with(|a| {
        a.borrow_mut().retain(|p| p != &table_principal);
    });

    Ok(())
}

/// Get all authorized tables
#[ic_cdk::query]
fn get_authorized_tables() -> Vec<Principal> {
    AUTHORIZED_TABLES.with(|a| a.borrow().clone())
}

// ============================================================================
// UPGRADE HOOKS - Persist state across upgrades
// ============================================================================

#[derive(CandidType, Deserialize)]
struct PersistentState {
    tables: Vec<(u64, TableInfo)>,
    players: Vec<(Principal, PlayerProfile)>,
    admin: Option<Principal>,
    initialized: bool,
    #[serde(default)]
    authorized_tables: Vec<Principal>,
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = PersistentState {
        tables: TABLES.with(|t| t.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
        players: PLAYERS.with(|p| p.borrow().iter().map(|(k, v)| (*k, v.clone())).collect()),
        admin: ADMIN.with(|a| *a.borrow()),
        initialized: INITIALIZED.with(|i| *i.borrow()),
        authorized_tables: AUTHORIZED_TABLES.with(|a| a.borrow().clone()),
    };

    if let Err(e) = ic_cdk::storage::stable_save((state,)) {
        ic_cdk::println!("CRITICAL: Failed to save state to stable memory: {:?}", e);
        // Log but don't panic - allow upgrade to proceed with potential data loss
        // This is safer than trapping which could brick the canister
    }
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let restore_result: Result<(PersistentState,), _> = ic_cdk::storage::stable_restore();

    let state = match restore_result {
        Ok((s,)) => s,
        Err(e) => {
            // FAIL LOUDLY - do NOT silently lose state!
            // If this panics, the upgrade will be rejected and the old code will remain.
            panic!("CRITICAL: Failed to restore state from stable memory: {:?}. \
                    Upgrade REJECTED. \
                    If you used --mode reinstall, that DESTROYS ALL DATA. \
                    Always use --mode upgrade for production canisters.", e);
        }
    };

    TABLES.with(|t| {
        let mut tables = t.borrow_mut();
        for (k, v) in state.tables {
            tables.insert(k, v);
        }
    });

    PLAYERS.with(|p| {
        let mut players = p.borrow_mut();
        for (k, v) in state.players {
            players.insert(k, v);
        }
    });

    ADMIN.with(|a| {
        *a.borrow_mut() = state.admin;
    });

    INITIALIZED.with(|i| {
        *i.borrow_mut() = state.initialized;
    });

    AUTHORIZED_TABLES.with(|a| {
        *a.borrow_mut() = state.authorized_tables;
    });
}

// ============================================================================
// CANDID EXPORT
// ============================================================================

ic_cdk::export_candid!();
