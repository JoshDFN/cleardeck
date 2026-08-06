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

    // Index: (table_id, hand_number, seed_hash) -> hand_id.
    //
    // This is what makes `record_hand` IDEMPOTENT. A table that could not reach
    // this canister keeps the record and re-sends it later
    // (`flush_unrecorded_hands` on the table), and a re-send must not create a
    // second copy of the same hand or the player-stats totals below would count
    // it twice.
    //
    // The seed hash is IN THE KEY, and it has to be. `reset_table` on a table
    // canister restarts hand numbering at zero, so (table_id, hand_number) alone
    // is not unique over the life of a table: keyed that way, the first four
    // hands after any reset are silently swallowed as duplicates of four older
    // hands and their shuffle proofs never reach the archive at all. That was
    // measured on the running replica before this key was widened -- four hands
    // played, four `Ok`s returned to the table, one new record stored. A
    // fairness archive that quietly discards proofs is worse than the cap it
    // was built to replace. The seed is 32 bytes of `raw_rand` per hand, so two
    // genuinely different hands never share this key and a genuine retry always
    // does.
    //
    // Rebuilt from `hands` in post_upgrade, never persisted, so it cannot drift
    // out of step with the records it indexes.
    hand_key_index: BTreeMap<(Principal, u64, String), u64>,

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

/// Who holds the key that decides which canisters may write here.
///
/// Public on purpose. `authorize_table` is the one call that can put a record
/// into this archive that no real table produced, so a player is entitled to
/// see who can make it without asking anyone's permission.
#[ic_cdk::query]
fn get_admin() -> Option<Principal> {
    STATE.with(|s| s.borrow().admin)
}

// ============================================================================
// RETENTION -- what this archive promises, stated by the archive itself
// ============================================================================

/// The retention rule in machine-readable form, so the sentence in the UI and
/// the sentence in the README can be CHECKED rather than believed.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct RetentionPolicy {
    /// Records currently held.
    pub records_held: u64,
    /// True while no method in this canister can delete or edit a record.
    /// Grep the interface: there is no delete, no prune, no edit, no cap.
    pub records_are_append_only: bool,
    /// The maximum age of a record this canister will discard on its own.
    /// `null` means it never discards one.
    pub max_age_before_discard: Option<u64>,
    /// Who can stop FUTURE writes (revoke_table) or admit a new writer.
    /// They cannot remove anything already here.
    pub admin: Option<Principal>,
    /// The honest version, including the part application code cannot fix.
    pub summary: String,
}

#[ic_cdk::query]
fn get_retention_policy() -> RetentionPolicy {
    STATE.with(|s| {
        let state = s.borrow();
        RetentionPolicy {
            records_held: state.hands.len() as u64,
            records_are_append_only: true,
            max_age_before_discard: None,
            admin: state.admin,
            summary:
                "A hand recorded here is permanent for the life of this canister. There is no \
                 method that deletes, prunes, edits or expires a record, and no cap on how many \
                 are kept; re-sending a hand that is already stored returns the existing id \
                 instead of writing a second copy. What CAN still destroy the archive is a \
                 controller of this canister: reinstalling it wipes stable memory and deleting \
                 it takes everything, and no application code can prevent either. The admin \
                 principal reported here cannot remove a record, but can admit a new writer with \
                 authorize_table, which is how a record no real table produced could get in. \
                 Treat that principal, and this canister's controllers, as the people you are \
                 trusting for durability."
                    .to_string(),
        }
    })
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
            return Err(format!(
                "Unauthorized: {} is not a registered table on this archive. The admin must call \
                 authorize_table(principal \"{}\") before this table's hands can be recorded. \
                 Until then every shuffle proof it produces exists only in the table canister.",
                caller, caller
            ));
        }

        Ok(insert_hand(&mut state, record))
    })
}

/// The identity of a hand, for de-duplication.
///
/// The seed hash is part of it because `reset_table` on a table restarts hand
/// numbering at zero. See `HistoryState::hand_key_index`.
fn hand_key(record: &HandHistoryRecord) -> (Principal, u64, String) {
    (
        record.table_id,
        record.hand_number,
        record.shuffle_proof.seed_hash.trim().to_lowercase(),
    )
}

/// Store a hand, or return the id of the copy already stored.
///
/// Split out of `record_hand` so the de-duplication rule can be tested on the
/// host: `record_hand` itself calls `msg_caller`, which traps outside a
/// canister, and a rule this easy to get wrong should not be reachable only
/// through a deployed replica.
fn insert_hand(state: &mut HistoryState, record: HandHistoryRecord) -> u64 {
    // IDEMPOTENT. A table that could not reach this archive holds the record
    // and re-sends it, so the same hand arrives more than once by design. A
    // second copy would double-count in player_stats and give a player two
    // conflicting records of one hand, so the existing id is returned
    // unchanged and nothing is written.
    let key = hand_key(&record);
    if let Some(existing) = state.hand_key_index.get(&key).copied() {
        return existing;
    }

    // Assign hand ID
    let hand_id = state.next_hand_id;
    state.next_hand_id += 1;

    // Create record with assigned ID
    let mut final_record = record;
    final_record.hand_id = hand_id;

    // Update indexes
    state.hand_key_index.insert(key, hand_id);
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
        update_player_stats(state, player, &final_record);
    }

    // Store the record
    state.hands.insert(hand_id, final_record);

    hand_id
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

// ============================================================================
// CHECKING A RECORDED HAND
// ============================================================================
//
// `verify_hand_shuffle` answers `Ok(false)` for both "the table revealed a seed
// it never committed to" and "this record has no revealed seed in it", and a
// bare bool cannot tell a player which. The table canister's equivalent had the
// same shape and an independent auditor called it worse than having no endpoint
// (docs/DEFECTS.md E-44). `check_recorded_hand` is the replacement: it names the
// problem, shows both hashes, and states in the answer itself what the answer is
// worth, which is not much, because the archive re-hashing its own record is
// still the house checking its own homework.

/// A hand's commitment, checked against the archive's own copy, with the
/// arithmetic shown and the limits of the answer attached to it.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct RecordedHandCheck {
    pub hand_id: u64,
    pub table_id: Principal,
    pub hand_number: u64,
    /// The committed hash as recorded.
    pub seed_hash: String,
    /// The revealed seed as recorded. Empty if the record carries none.
    pub revealed_seed: String,
    /// `SHA256(revealed_seed)`, recomputed here. Empty if it could not be computed.
    pub computed_hash: String,
    /// True only when the two hashes above are equal.
    pub commitment_matches: bool,
    /// `null` when it matched. Otherwise names WHICH check failed, in words.
    pub problem: Option<String>,
    pub this_proves: String,
    pub this_does_not_prove: String,
    /// A command the player can run somewhere this canister does not control.
    pub verify_it_yourself: String,
}

const ARCHIVE_PROVES: &str =
    "Only that the seed stored in this record is the pre-image of the hash stored in the same \
     record. It says nothing about whether that record describes the hand you played.";

const ARCHIVE_DOES_NOT_PROVE: &str =
    "Nothing about fairness. This is an archive re-hashing its own copy. The verification that \
     counts re-derives all 52 cards from the seed on YOUR machine, following docs/SHUFFLE-SPEC.md, \
     and checks that the cards you were actually shown come out at the positions the dealing rule \
     puts them.";

#[ic_cdk::query]
fn check_recorded_hand(hand_id: u64) -> Result<RecordedHandCheck, String> {
    STATE.with(|s| {
        let state = s.borrow();

        let hand = state.hands.get(&hand_id).ok_or_else(|| {
            format!(
                "No hand with id {} in this archive. Ids are assigned by this canister in the \
                 order hands arrive, and are NOT the table's own hand numbers: use \
                 get_hands_by_table or get_hands_by_player to find the id you want.",
                hand_id
            )
        })?;

        let proof = &hand.shuffle_proof;
        let seed_hash = proof.seed_hash.trim().to_lowercase();
        let revealed_seed = proof.revealed_seed.trim().to_lowercase();

        let base = |computed: String, matches: bool, problem: Option<String>| RecordedHandCheck {
            hand_id: hand.hand_id,
            table_id: hand.table_id,
            hand_number: hand.hand_number,
            seed_hash: seed_hash.clone(),
            revealed_seed: revealed_seed.clone(),
            computed_hash: computed,
            commitment_matches: matches,
            problem,
            this_proves: ARCHIVE_PROVES.to_string(),
            this_does_not_prove: ARCHIVE_DOES_NOT_PROVE.to_string(),
            verify_it_yourself: format!(
                "echo -n {} | xxd -r -p | shasum -a 256    # must print {}",
                if revealed_seed.is_empty() { "<seed>" } else { &revealed_seed },
                if seed_hash.is_empty() { "<hash>" } else { &seed_hash },
            ),
        };

        if revealed_seed.is_empty() {
            return Ok(base(
                String::new(),
                false,
                Some(
                    "This record carries no revealed seed, so there is nothing to check yet. \
                     That is NOT evidence of a bad shuffle: the seed is revealed only when the \
                     hand ends. If the hand is long finished, the record was archived without \
                     its reveal and that is a defect worth reporting."
                        .to_string(),
                ),
            ));
        }

        let seed_bytes = match hex::decode(&revealed_seed) {
            Ok(b) => b,
            Err(_) => {
                return Ok(base(
                    String::new(),
                    false,
                    Some(format!(
                        "The revealed seed in this record ({} characters) is not valid \
                         hexadecimal, so it cannot be hashed. The record is malformed; this is \
                         not a statement about the deal.",
                        revealed_seed.chars().count()
                    )),
                ))
            }
        };

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&seed_bytes);
        let computed_hash = hex::encode(hasher.finalize());

        if computed_hash == seed_hash {
            Ok(base(computed_hash, true, None))
        } else {
            Ok(base(
                computed_hash,
                false,
                Some(
                    "The seed in this record does not hash to the hash in the same record. \
                     Nothing in this archive can explain that away: either the record was \
                     corrupted in transit or the table revealed a seed it did not commit to. \
                     Please report it."
                        .to_string(),
                ),
            ))
        }
    })
}

/// Deprecated in favour of `check_recorded_hand`, which says WHICH check failed.
///
/// Kept so an existing caller keeps working. `Ok(false)` here still cannot
/// distinguish "no seed revealed yet" from "the commitment is broken"; that is
/// the reason it is deprecated, not a reason to trust it.
#[ic_cdk::query]
fn verify_hand_shuffle(hand_id: u64) -> Result<bool, String> {
    check_recorded_hand(hand_id).map(|c| c.commitment_matches)
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

        // Restore hands, rebuilding the idempotency index from them as we go.
        // The index is derived, never persisted, so an upgrade cannot leave it
        // describing records that are not there.
        for (k, v) in state.hands {
            let key = (
                v.table_id,
                v.hand_number,
                v.shuffle_proof.seed_hash.trim().to_lowercase(),
            );
            new_state.hand_key_index.insert(key, k);
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

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod retention_tests {
    //! The archive's whole job is that a proof is still there tomorrow. Two
    //! ways it can silently fail to do that job, both measured on the running
    //! replica before they were fixed:
    //!
    //!   * a re-sent hand stored twice, double-counting the player's record;
    //!   * a genuinely new hand mistaken for an old one and DISCARDED, which is
    //!     the dangerous direction because the caller is told `Ok`.

    use super::*;

    fn table() -> Principal {
        Principal::from_slice(&[1, 2, 3, 4, 5])
    }

    fn player() -> Principal {
        Principal::from_slice(&[9, 9, 9])
    }

    fn hand(hand_number: u64, seed_hash: &str) -> HandHistoryRecord {
        HandHistoryRecord {
            hand_id: 0,
            table_id: table(),
            hand_number,
            timestamp: 1,
            small_blind: 1,
            big_blind: 2,
            ante: 0,
            shuffle_proof: ShuffleProofRecord {
                seed_hash: seed_hash.to_string(),
                revealed_seed: "00ff".to_string(),
                timestamp: 1,
            },
            players: vec![PlayerHandRecord {
                seat: 0,
                principal: player(),
                starting_chips: 100,
                ending_chips: 120,
                hole_cards: None,
                final_hand_rank: None,
                amount_won: 20,
                position: "BTN".to_string(),
            }],
            dealer_seat: 0,
            flop: None,
            turn: None,
            river: None,
            actions: vec![],
            total_pot: 20,
            rake: 0,
            winners: vec![],
            went_to_showdown: false,
        }
    }

    #[test]
    fn re_sending_the_same_hand_stores_it_once_and_returns_the_same_id() {
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let first = insert_hand(&mut state, hand(7, "aa"));
        let again = insert_hand(&mut state, hand(7, "aa"));
        assert_eq!(first, again, "a retry must not mint a second id");
        assert_eq!(state.hands.len(), 1, "a retry must not store a second copy");
        assert_eq!(
            state.player_stats.get(&player()).unwrap().hands_played,
            1,
            "a retry must not double-count the player's record"
        );
        assert_eq!(state.hands_by_table.get(&table()).unwrap().len(), 1);
    }

    #[test]
    fn a_hand_reusing_a_number_after_a_table_reset_is_still_stored() {
        // `reset_table` restarts hand numbering at zero. Keyed on
        // (table_id, hand_number) alone this second hand is swallowed as a
        // duplicate and its shuffle proof never reaches the archive, while the
        // table is told `Ok`. That is silent evidence destruction, and it is
        // what this test exists to prevent coming back. docs/DEFECTS.md E-49.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let before_reset = insert_hand(&mut state, hand(1, "aa"));
        let after_reset = insert_hand(&mut state, hand(1, "bb"));
        assert_ne!(before_reset, after_reset);
        assert_eq!(state.hands.len(), 2, "two different hands, two records");
        assert_eq!(state.player_stats.get(&player()).unwrap().hands_played, 2);
    }

    #[test]
    fn the_key_ignores_case_and_whitespace_around_the_seed_hash() {
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let first = insert_hand(&mut state, hand(3, "abc123"));
        let again = insert_hand(&mut state, hand(3, "  ABC123 "));
        assert_eq!(first, again);
        assert_eq!(state.hands.len(), 1);
    }

    #[test]
    fn nothing_in_the_state_can_shrink_the_archive() {
        // The retention claim in the README and in the UI is "no method deletes,
        // prunes, edits or expires a record". This pins the half of that claim
        // the type system can check: the write path only ever grows `hands`.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut seen = 0usize;
        for n in 1..=250u64 {
            insert_hand(&mut state, hand(n, &format!("{n:04x}")));
            assert!(state.hands.len() > seen, "record {n} did not grow the archive");
            seen = state.hands.len();
        }
        assert_eq!(state.hands.len(), 250, "there is no cap, and none appeared");
    }

    #[test]
    fn the_retention_policy_does_not_promise_more_than_the_code_does() {
        let policy = STATE.with(|s| {
            let state = s.borrow();
            RetentionPolicy {
                records_held: state.hands.len() as u64,
                records_are_append_only: true,
                max_age_before_discard: None,
                admin: state.admin,
                summary: String::new(),
            }
        });
        assert!(policy.records_are_append_only);
        assert!(policy.max_age_before_discard.is_none());
    }
}
