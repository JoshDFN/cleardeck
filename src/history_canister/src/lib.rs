use candid::{CandidType, Deserialize, Principal};
use std::cell::RefCell;
use std::collections::BTreeMap;

// ============================================================================
// TYPES - Hand history data structures
// ============================================================================

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash)]
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

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum PlayerAction {
    Fold,
    Check,
    Call(u64),      // Amount called
    Bet(u64),       // Amount bet
    Raise(u64),     // Total raise amount
    AllIn(u64),     // All-in amount
    PostBlind(u64), // Blind posted
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ActionRecord {
    pub seat: u8,
    pub principal: Principal,
    pub action: PlayerAction,
    pub timestamp: u64,
    pub phase: String, // "preflop", "flop", "turn", "river"
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct PlayerHandRecord {
    pub seat: u8,
    pub principal: Principal,
    pub starting_chips: u64,
    pub ending_chips: u64,
    pub hole_cards: Option<(Card, Card)>, // Revealed at showdown or if player won
    pub final_hand_rank: Option<HandRank>,
    pub amount_won: u64,
    pub position: String, // "dealer", "sb", "bb", "utg", etc.
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ShuffleProofRecord {
    pub seed_hash: String,      // SHA-256 hash of seed (committed before dealing)
    pub revealed_seed: String,  // The actual seed (revealed after hand)
    pub timestamp: u64,         // When the commitment was made
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandHistoryRecord {
    // Identifiers
    pub hand_id: u64,           // Global unique hand ID
    pub table_id: Principal,    // Table canister ID
    pub hand_number: u64,       // Hand number at that table
    pub timestamp: u64,         // When hand started

    // Table config at time of hand
    pub small_blind: u64,
    pub big_blind: u64,
    pub ante: u64,

    // Shuffle proof (for verification)
    pub shuffle_proof: ShuffleProofRecord,

    // Players involved
    pub players: Vec<PlayerHandRecord>,
    pub dealer_seat: u8,

    // Community cards
    pub flop: Option<(Card, Card, Card)>,
    pub turn: Option<Card>,
    pub river: Option<Card>,

    // All actions in order
    pub actions: Vec<ActionRecord>,

    // Pot info
    pub total_pot: u64,
    pub rake: u64, // If any rake is taken

    // Winners
    pub winners: Vec<WinnerRecord>,

    // Summary
    pub went_to_showdown: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct WinnerRecord {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
    pub hand_rank: Option<HandRank>,
    pub pot_type: String, // "main" or "side_1", "side_2", etc.
}

// Query result types
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandSummary {
    pub hand_id: u64,
    pub table_id: Principal,
    pub hand_number: u64,
    pub timestamp: u64,
    pub player_count: u8,
    pub total_pot: u64,
    pub winners: Vec<WinnerRecord>,
    pub went_to_showdown: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct PlayerStats {
    pub principal: Principal,
    pub hands_played: u64,
    pub hands_won: u64,
    pub total_winnings: i64, // Can be negative
    pub biggest_pot_won: u64,
    pub showdowns_won: u64,
    pub showdowns_total: u64,
}

// ============================================================================
// STATE
// ============================================================================

thread_local! {
    static STATE: RefCell<HistoryState> = RefCell::new(HistoryState::default());
}

#[derive(Default)]
struct HistoryState {
    // All hand records, keyed by hand_id
    hands: BTreeMap<u64, HandHistoryRecord>,

    // Index: table_id -> list of hand_ids
    hands_by_table: BTreeMap<Principal, Vec<u64>>,

    // Index: player principal -> list of hand_ids
    hands_by_player: BTreeMap<Principal, Vec<u64>>,

    // Player stats cache
    player_stats: BTreeMap<Principal, PlayerStats>,

    // Next hand ID
    next_hand_id: u64,

    // Authorized table canisters that can write history
    authorized_tables: Vec<Principal>,

    // Admin principal
    admin: Option<Principal>,
}

// ============================================================================
// ADMIN FUNCTIONS
// ============================================================================

#[ic_cdk::init]
fn init() {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.admin = Some(ic_cdk::api::msg_caller());
        state.next_hand_id = 1;
    });
}

#[ic_cdk::update]
fn authorize_table(table_canister: Principal) -> Result<(), String> {
    STATE.with(|s| {
        let mut state = s.borrow_mut();

        // Only admin can authorize
        if state.admin != Some(ic_cdk::api::msg_caller()) {
            return Err("Unauthorized".to_string());
        }

        if !state.authorized_tables.contains(&table_canister) {
            state.authorized_tables.push(table_canister);
        }

        Ok(())
    })
}

#[ic_cdk::update]
fn revoke_table(table_canister: Principal) -> Result<(), String> {
    STATE.with(|s| {
        let mut state = s.borrow_mut();

        if state.admin != Some(ic_cdk::api::msg_caller()) {
            return Err("Unauthorized".to_string());
        }

        state.authorized_tables.retain(|t| t != &table_canister);
        Ok(())
    })
}

#[ic_cdk::query]
fn get_authorized_tables() -> Vec<Principal> {
    STATE.with(|s| s.borrow().authorized_tables.clone())
}

// ============================================================================
// WRITE FUNCTIONS (called by table canister)
// ============================================================================

#[ic_cdk::update]
fn record_hand(record: HandHistoryRecord) -> Result<u64, String> {
    let caller = ic_cdk::api::msg_caller();

    STATE.with(|s| {
        let mut state = s.borrow_mut();

        // SECURITY FIX: Verify caller is authorized (either a registered table or admin)
        // Removed the empty authorized_tables bypass - that would allow anyone to inject fake history
        let is_authorized = state.authorized_tables.contains(&caller)
            || state.admin == Some(caller);

        if !is_authorized {
            return Err("Unauthorized: table not registered. Admin must add table via authorize_table.".to_string());
        }

        // Assign hand ID
        let hand_id = state.next_hand_id;
        state.next_hand_id += 1;

        // Create record with assigned ID
        let mut final_record = record;
        final_record.hand_id = hand_id;

        // Update indexes
        state.hands_by_table
            .entry(final_record.table_id)
            .or_default()
            .push(hand_id);

        for player in &final_record.players {
            state.hands_by_player
                .entry(player.principal)
                .or_default()
                .push(hand_id);

            // Update player stats
            update_player_stats(&mut state, player, &final_record);
        }

        // Store the record
        state.hands.insert(hand_id, final_record);

        Ok(hand_id)
    })
}

fn update_player_stats(state: &mut HistoryState, player: &PlayerHandRecord, hand: &HandHistoryRecord) {
    let stats = state.player_stats.entry(player.principal).or_insert(PlayerStats {
        principal: player.principal,
        hands_played: 0,
        hands_won: 0,
        total_winnings: 0,
        biggest_pot_won: 0,
        showdowns_won: 0,
        showdowns_total: 0,
    });

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
    if hand.went_to_showdown {
        // Player went to showdown if they have revealed cards and didn't fold
        if player.hole_cards.is_some() {
            stats.showdowns_total += 1;
            if player.amount_won > 0 {
                stats.showdowns_won += 1;
            }
        }
    }
}

// ============================================================================
// QUERY FUNCTIONS
// ============================================================================

#[ic_cdk::query]
fn get_hand(hand_id: u64) -> Option<HandHistoryRecord> {
    STATE.with(|s| s.borrow().hands.get(&hand_id).cloned())
}

#[ic_cdk::query]
fn get_hands_by_table(table_id: Principal, offset: u64, limit: u64) -> Vec<HandSummary> {
    STATE.with(|s| {
        let state = s.borrow();

        let hand_ids = state.hands_by_table.get(&table_id);
        if hand_ids.is_none() {
            return vec![];
        }

        let hand_ids = hand_ids.unwrap();

        // Get hands in reverse order (newest first)
        hand_ids.iter()
            .rev()
            .skip(offset as usize)
            .take(limit as usize)
            .filter_map(|id| state.hands.get(id))
            .map(|h| to_summary(h))
            .collect()
    })
}

#[ic_cdk::query]
fn get_hands_by_player(player: Principal, offset: u64, limit: u64) -> Vec<HandSummary> {
    STATE.with(|s| {
        let state = s.borrow();

        let hand_ids = state.hands_by_player.get(&player);
        if hand_ids.is_none() {
            return vec![];
        }

        let hand_ids = hand_ids.unwrap();

        hand_ids.iter()
            .rev()
            .skip(offset as usize)
            .take(limit as usize)
            .filter_map(|id| state.hands.get(id))
            .map(|h| to_summary(h))
            .collect()
    })
}

#[ic_cdk::query]
fn get_recent_hands(limit: u64) -> Vec<HandSummary> {
    STATE.with(|s| {
        let state = s.borrow();

        state.hands.iter()
            .rev()
            .take(limit as usize)
            .map(|(_, h)| to_summary(h))
            .collect()
    })
}

#[ic_cdk::query]
fn get_player_stats(player: Principal) -> Option<PlayerStats> {
    STATE.with(|s| s.borrow().player_stats.get(&player).cloned())
}

#[ic_cdk::query]
fn get_total_hands() -> u64 {
    STATE.with(|s| s.borrow().hands.len() as u64)
}

#[ic_cdk::query]
fn get_table_hand_count(table_id: Principal) -> u64 {
    STATE.with(|s| {
        s.borrow().hands_by_table
            .get(&table_id)
            .map(|v| v.len() as u64)
            .unwrap_or(0)
    })
}

#[ic_cdk::query]
fn verify_hand_shuffle(hand_id: u64) -> Result<bool, String> {
    STATE.with(|s| {
        let state = s.borrow();

        let hand = state.hands.get(&hand_id)
            .ok_or("Hand not found")?;

        let proof = &hand.shuffle_proof;

        // Verify SHA-256(revealed_seed) == seed_hash
        use sha2::{Sha256, Digest};

        let seed_bytes = hex::decode(&proof.revealed_seed)
            .map_err(|_| "Invalid revealed seed hex")?;

        let mut hasher = Sha256::new();
        hasher.update(&seed_bytes);
        let computed_hash = hex::encode(hasher.finalize());

        Ok(computed_hash == proof.seed_hash)
    })
}

// ============================================================================
// HELPERS
// ============================================================================

fn to_summary(hand: &HandHistoryRecord) -> HandSummary {
    HandSummary {
        hand_id: hand.hand_id,
        table_id: hand.table_id,
        hand_number: hand.hand_number,
        timestamp: hand.timestamp,
        player_count: hand.players.len() as u8,
        total_pot: hand.total_pot,
        winners: hand.winners.clone(),
        went_to_showdown: hand.went_to_showdown,
    }
}

// Add hex encoding support
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

// ============================================================================
// UPGRADE HOOKS - Persist state across upgrades
// ============================================================================

#[derive(CandidType, Deserialize)]
struct PersistentState {
    hands: Vec<(u64, HandHistoryRecord)>,
    hands_by_table: Vec<(Principal, Vec<u64>)>,
    hands_by_player: Vec<(Principal, Vec<u64>)>,
    player_stats: Vec<(Principal, PlayerStats)>,
    next_hand_id: u64,
    authorized_tables: Vec<Principal>,
    admin: Option<Principal>,
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = STATE.with(|s| {
        let s = s.borrow();
        PersistentState {
            hands: s.hands.iter().map(|(k, v)| (*k, v.clone())).collect(),
            hands_by_table: s.hands_by_table.iter().map(|(k, v)| (*k, v.clone())).collect(),
            hands_by_player: s.hands_by_player.iter().map(|(k, v)| (*k, v.clone())).collect(),
            player_stats: s.player_stats.iter().map(|(k, v)| (*k, v.clone())).collect(),
            next_hand_id: s.next_hand_id,
            authorized_tables: s.authorized_tables.clone(),
            admin: s.admin,
        }
    });

    if let Err(e) = ic_cdk::storage::stable_save((state,)) {
        ic_cdk::println!("CRITICAL: Failed to save state to stable memory: {:?}", e);
        // Log but don't panic - allow upgrade to proceed
    }
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    let restore_result: Result<(PersistentState,), _> = ic_cdk::storage::stable_restore();

    let state = match restore_result {
        Ok((s,)) => s,
        Err(e) => {
            // FAIL LOUDLY - do NOT silently lose hand history!
            // If this panics, the upgrade will be rejected and the old code will remain.
            // This protects shuffle proofs and hand records from being lost.
            panic!("CRITICAL: Failed to restore state from stable memory: {:?}. \
                    Upgrade REJECTED to protect hand history. \
                    If you used --mode reinstall, that DESTROYS ALL DATA. \
                    Always use --mode upgrade for production canisters.", e);
        }
    };

    STATE.with(|s| {
        let mut new_state = HistoryState::default();
        
        // Restore hands
        for (k, v) in state.hands {
            new_state.hands.insert(k, v);
        }
        
        // Restore indexes
        for (k, v) in state.hands_by_table {
            new_state.hands_by_table.insert(k, v);
        }
        
        for (k, v) in state.hands_by_player {
            new_state.hands_by_player.insert(k, v);
        }
        
        // Restore player stats
        for (k, v) in state.player_stats {
            new_state.player_stats.insert(k, v);
        }
        
        new_state.next_hand_id = state.next_hand_id;
        new_state.authorized_tables = state.authorized_tables;
        new_state.admin = state.admin;
        
        *s.borrow_mut() = new_state;
    });
}

// Candid export
ic_cdk::export_candid!();
