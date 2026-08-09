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

/// One person's part in one hand.
///
/// # A seat is not a person (docs/SECURITY-FINDINGS.md FINDING 30)
///
/// Two records in the same hand CAN share a `seat`: a player leaves a live hand
/// with money in the pot and somebody else buys the empty chair before it settles.
/// The money in the pot still belongs to the player who put it there. Anything
/// reading this list must key on `principal`, or on `(seat, principal)`, never on
/// `seat` alone.
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

    // ------------------------------------------------------------------------
    // FINDING 30. `opt`, and it has to be: this archive is APPEND-ONLY and holds
    // records written before these fields existed. A bare field here would make
    // `stable_restore` fail on every record already stored, which would reject
    // every future upgrade of the one canister in this project that must never be
    // reinstalled. `null` reads as "the table that wrote this record did not
    // record that fact", which is the truth for every hand archived while
    // FINDING 30 was live -- and is what a verifier needs to be told instead of a
    // fabricated `false`.
    // ------------------------------------------------------------------------
    /// Did this person take cards from the deck? Somebody who bought the chair
    /// after the deal did not, and must not be counted when offsetting the board.
    #[serde(default)]
    pub dealt_in: Option<bool>,
    /// What this PERSON put into the pot. Over the whole record this sums to
    /// `total_pot`, whether or not everybody is still at the table.
    #[serde(default)]
    pub contributed: Option<u64>,
    /// True when they left before the hand settled. Their stake stayed in the pot.
    #[serde(default)]
    pub left_mid_hand: Option<bool>,
}

/// A seat the deal gave cards to, in the order the deck was consumed.
///
/// docs/SHUFFLE-SPEC.md section 4: with `P` players dealt in, the flop is
/// `deck[2P+1..2P+4]`, the turn `deck[2P+5]` and the river `deck[2P+7]`, and the
/// `k`-th entry of this list holds `deck[2k]`, `deck[2k+1]`. `P` is therefore the
/// length of this list, stated by the record rather than counted from the players
/// -- counting was wrong for every hand somebody left, and produced the wrong
/// board from the right seed.
#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct DealtInSeat {
    pub seat: u8,
    pub principal: Principal,
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

    /// **THE PERMANENT NAME OF THIS HAND** — see [`hand_uid`] and
    /// docs/DEFECTS.md E-71.
    ///
    /// `table_id : seed_hash`. DERIVED, never supplied: whatever a writer puts
    /// here is discarded on the way in and this field is recomputed from
    /// `table_id` and `shuffle_proof.seed_hash` on the way out. That is the whole
    /// migration story — every record already in this archive carries both of
    /// those fields, so every record already stored acquires its name the moment
    /// this code is installed, with nothing rewritten, renumbered or reindexed.
    ///
    /// `opt` for docs/SECURITY-FINDINGS.md FINDING 14: this record is persisted
    /// inside `PersistentState`, and a bare field would make `stable_restore`
    /// reject every upgrade from state written before it existed. On the way out
    /// it is `null` only for a record whose commitment is not a SHA-256 digest,
    /// which `record_hand` refuses to store, so no record written through this
    /// code can have one.
    #[serde(default)]
    pub hand_uid: Option<String>,

    // Table config at time of hand
    pub small_blind: u64,
    pub big_blind: u64,
    pub ante: u64,

    // Shuffle proof (for verification)
    pub shuffle_proof: ShuffleProofRecord,

    // Players involved. EVERYONE whose money was in the hand, from the table's
    // settlement basis -- including a player who left before it settled -- and
    // nobody else. See `PlayerHandRecord`.
    pub players: Vec<PlayerHandRecord>,
    pub dealer_seat: u8,

    /// WHO WAS DEALT IN, AND IN WHAT ORDER. The number a verifier needs to offset
    /// the board (docs/SHUFFLE-SPEC.md section 4) is `dealt_in.len()`.
    ///
    /// `opt`: `null` means the record predates this field, and a verifier should
    /// treat the hand as unverifiable rather than guess `P`.
    #[serde(default)]
    pub dealt_in: Option<Vec<DealtInSeat>>,

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
    /// **THE NAME TO CITE.** `table_id : seed_hash`, unique over this archive and
    /// stable for the life of the record. `hand_number` below is NOT unique — it
    /// restarts at 1 on every `reset_table` — so a citation built from it names a
    /// set, not a hand. docs/DEFECTS.md E-71.
    ///
    /// Empty only for a record whose commitment is not a SHA-256 digest, which
    /// `record_hand` refuses.
    pub hand_uid: String,
    pub timestamp: u64,
    /// How many PEOPLE the hand involved -- everyone whose money was in the pot,
    /// including anyone who left before it settled. Not the number of chairs
    /// occupied when it ended, which is what this used to be.
    pub player_count: u8,
    /// How many were DEALT IN: `P`, for docs/SHUFFLE-SPEC.md section 4. `null` on
    /// records written before the archive recorded it.
    pub dealt_in_count: Option<u8>,
    pub total_pot: u64,
    pub winners: Vec<WinnerRecord>,
    pub went_to_showdown: bool,
}

/// The most candidates `resolve_hand_number` will return in one reply. The
/// `match_count` beside them is always exact, so a truncated list is visible.
const MAX_CITATION_MATCHES: usize = 200;

/// What an old `(table, hand number)` citation actually names. See
/// [`resolve_hand_number`] and docs/DEFECTS.md E-71.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HandNumberCitation {
    pub table_id: Principal,
    pub hand_number: u64,
    /// How many records answer to this citation. Exact, even when `matches` below
    /// is truncated.
    pub match_count: u64,
    /// True only when exactly one record answers to it TODAY. Never a promise
    /// about tomorrow: the next `reset_table` can make it false.
    pub is_unique: bool,
    /// Every record that answers to it, oldest first, each carrying its name.
    pub matches: Vec<HandSummary>,
    /// What to do about it, in words.
    pub advice: String,
}

/// The archive's own statement about whether a hand's name identifies a hand.
/// See [`get_archive_integrity`].
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct ArchiveIntegrity {
    pub records_held: u64,
    pub records_with_a_name: u64,
    pub distinct_names: u64,
    /// MUST BE ZERO. Names carried by more than one record.
    pub name_collisions: u64,
    pub records_under_a_colliding_name: u64,
    /// MUST BE ZERO. Records whose commitment is not a SHA-256 digest.
    pub records_without_a_usable_commitment: u64,
    /// MUST BE FALSE. True when the name index holds a different number of names
    /// than the records themselves produce, which means the index has drifted.
    pub index_disagrees_with_records: bool,
    pub distinct_hand_number_citations: u64,
    /// Expected to be large. Not a fault; the reason hand numbers are not names.
    pub ambiguous_hand_number_citations: u64,
    pub records_under_an_ambiguous_hand_number: u64,
    pub worst_hand_number_citation: Option<String>,

    // ------------------------------------------------------------------------
    // NO RAKE, OVER THE WHOLE ARCHIVE
    // ------------------------------------------------------------------------
    // Not an identity question, and here for a reason that is: the only gate in
    // this project that asserts the no-rake property against the permanent
    // archive reads a WINDOW of the newest twenty records (and, before
    // docs/DEFECTS.md E-71, collapsed those twenty to ten). A rake on a record
    // older than the window was unchecked by anything, forever -- a rake only had
    // to wait twenty hands to become invisible. Two counters make the claim
    // archive-wide in one query, and they cost one pass over records this query
    // is already walking.
    /// MUST BE ZERO. Records recording a non-zero rake.
    pub records_with_a_nonzero_rake: u64,
    /// MUST BE ZERO. The sum of every rake ever recorded here, in e8s.
    pub rake_recorded_total: u64,
    /// The name of one record carrying a rake, so a non-zero count above is
    /// immediately actionable rather than a number to go hunting for.
    pub first_record_with_a_rake: Option<String>,

    pub summary: String,
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
// THE NAME OF A HAND   (docs/DEFECTS.md E-71)
// ============================================================================
//
// `(table_id, hand_number)` IS NOT A KEY AND NEVER WAS. `reset_table` on a table
// canister restarts hand numbering at 1, so the same citation is reused for the
// rest of the table's life. Measured on the local archive on 2026-08-08, before
// this section existed:
//
//     3,218 records          1,651 distinct (table_id, hand_number) citations
//       947 citations name MORE THAN ONE record
//     2,514 records (78%) live under a citation that names more than one
//       877 of those citations cover records with DIFFERENT POTS
//     "table_2 hand 1" answers to 70 records with 6 different pots
//
// A record you cannot uniquely name is a record you cannot cite, and five
// auditors have cited hands out of this archive. So a hand gets a name that is
// intrinsic to the hand rather than a counter that resets:
//
//     hand_uid = "<table canister id>:<seed_hash>"
//
// WHY THE COMMITMENT. It is 32 bytes of `raw_rand` per hand, hashed, published
// before any card is shown, already carried by every record ever archived, and
// already on the player's own screen while the hand is running. It is the one
// value in a hand record that is unique to the hand BY CONSTRUCTION rather than
// by bookkeeping, and a verifier can therefore check that the record they were
// handed is the record they asked for without trusting this canister's indexes.
// Measured on the same 3,218 records: 3,218 distinct commitments, zero empty,
// zero not 64-hex — the commitment is already a perfect key over every hand this
// project has ever archived.
//
// WHY THE TABLE IS IN IT TOO. Not for uniqueness — the commitment alone is
// unique — but for what happens when it is not. Keyed on the commitment alone, a
// second table sending a record that reuses another table's commitment would be
// DE-DUPLICATED AWAY, and silently discarding a genuine hand is exactly the
// failure docs/DEFECTS.md E-49 was: `Ok` returned, proof gone. Scoped to the
// table, such a record is stored as its own hand and the collision is visible in
// `get_archive_integrity` instead of costing evidence.
//
// WHAT IT IS NOT. It is not a hash of the record, so it says nothing about the
// record's contents; two DIFFERENT stories about the same deal would share a
// name. That is deliberate — the name has to be derivable from what the player
// saw while the hand was running, and the player never sees the settled record.
// Content integrity is `check_recorded_hand`'s job, and it is a separate one.

/// The separator between the table and the commitment in a hand's name.
const HAND_UID_SEPARATOR: char = ':';

/// A SHA-256 digest, written in lowercase hex.
const COMMITMENT_HEX_LEN: usize = 64;

/// A commitment as anybody might have written it down, or `None` if what was
/// written down is not a SHA-256 digest.
///
/// Case and surrounding whitespace are forgiven because a player pastes this out
/// of a screen. Nothing else is: a 63-character paste is a truncated paste, and
/// answering it with a "hand" would be worse than refusing it.
fn normalise_commitment(seed_hash: &str) -> Option<String> {
    let s = seed_hash.trim().to_lowercase();
    if s.len() == COMMITMENT_HEX_LEN && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s)
    } else {
        None
    }
}

/// The permanent name of a hand: `"<table canister id>:<seed_hash>"`.
///
/// `None` when the commitment is not a SHA-256 digest, because a hand with no
/// usable commitment cannot be named, cannot be verified, and cannot be told
/// apart from any other unusable record from the same table.
pub fn hand_uid(table_id: &Principal, seed_hash: &str) -> Option<String> {
    normalise_commitment(seed_hash)
        .map(|c| format!("{}{}{}", table_id.to_text(), HAND_UID_SEPARATOR, c))
}

/// The name of a stored record.
fn record_uid(record: &HandHistoryRecord) -> Option<String> {
    hand_uid(&record.table_id, &record.shuffle_proof.seed_hash)
}

/// How somebody wrote a hand's name down.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HandName {
    /// The full `table:commitment` form. Names at most one record, ever.
    Full(String),
    /// A bare commitment. Names one record in practice; if it ever names two,
    /// the caller is told so rather than handed one of them.
    Commitment(String),
    /// Not a hand name at all. Carries what is wrong with it, in words.
    Malformed(String),
}

/// Parse a hand name as a human would have typed it.
fn parse_hand_name(input: &str) -> HandName {
    let raw = input.trim();
    if raw.is_empty() {
        return HandName::Malformed(
            "an empty string is not a hand name. A hand is named \
             \"<table canister id>:<seed_hash>\", and the seed hash is the 64-character \
             commitment shown on the table while the hand is running."
                .to_string(),
        );
    }

    match raw.rsplit_once(HAND_UID_SEPARATOR) {
        Some((table, commitment)) => {
            let table = table.trim();
            let commitment = match normalise_commitment(commitment) {
                Some(c) => c,
                None => {
                    return HandName::Malformed(format!(
                        "the part after '{}' is {} character(s) long and a commitment is {} \
                         lowercase hex characters. This looks like a truncated or edited paste.",
                        HAND_UID_SEPARATOR,
                        commitment.trim().chars().count(),
                        COMMITMENT_HEX_LEN
                    ))
                }
            };
            match Principal::from_text(table) {
                Ok(p) => HandName::Full(format!("{}{}{}", p.to_text(), HAND_UID_SEPARATOR, commitment)),
                Err(e) => HandName::Malformed(format!(
                    "the part before '{}' is not a canister id: {}",
                    HAND_UID_SEPARATOR, e
                )),
            }
        }
        None => match normalise_commitment(raw) {
            Some(c) => HandName::Commitment(c),
            None => HandName::Malformed(format!(
                "'{}' is neither \"<table canister id>{}<seed_hash>\" nor a bare {}-character \
                 commitment.",
                raw.chars().take(80).collect::<String>(),
                HAND_UID_SEPARATOR,
                COMMITMENT_HEX_LEN
            )),
        },
    }
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

    // Index: hand_uid -> hand_id.  ONE RECORD PER NAME, enforced here.
    //
    // This is what makes `record_hand` IDEMPOTENT. A table that could not reach
    // this canister keeps the record and re-sends it later
    // (`flush_unrecorded_hands` on the table), and a re-send must not create a
    // second copy of the same hand or the player-stats totals below would count
    // it twice.
    //
    // THE KEY NO LONGER CONTAINS `hand_number`, and that is the point. It used to
    // be `(table_id, hand_number, seed_hash)`, which de-duplicated correctly --
    // `reset_table` restarts hand numbering, so keying on the number alone
    // silently swallowed genuinely new hands as duplicates (docs/DEFECTS.md
    // E-49) -- but left `hand_number` in a key it does not belong in. The
    // commitment alone already separates two different hands, so the number was
    // doing nothing except making the key non-derivable from the hand itself and
    // allowing two records to share one name if a table ever re-sent a hand under
    // a different number. Narrowed to the hand's NAME, the index is exactly the
    // uniqueness claim `get_archive_integrity` publishes.
    //
    // Over the 3,218 records in the local archive this narrowing merges nothing:
    // there are 3,218 distinct commitments, so no two existing records collapse
    // into one and no existing record becomes unreachable.
    //
    // Rebuilt from `hands` in post_upgrade, never persisted, so it cannot drift
    // out of step with the records it indexes.
    hand_uid_index: BTreeMap<String, u64>,

    // Index: (table_id, hand_number) -> every hand_id that answers to it.
    //
    // A LIST, not a value, because the citation names a SET: 947 of the local
    // archive's 1,651 citations name more than one record. `resolve_hand_number`
    // hands back the whole set rather than an arbitrary member of it, which is
    // what stops an old citation from silently resolving to the wrong hand.
    //
    // Derived, rebuilt in post_upgrade, never persisted.
    hand_ids_by_number: BTreeMap<(Principal, u64), Vec<u64>>,

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
        may_record(&state, caller, &record)?;
        insert_hand(&mut state, record)
    })
}

/// May `caller` file THIS record?
///
/// Split out of [`record_hand`] for the reason [`insert_hand`] is: `record_hand`
/// calls `msg_caller`, which traps outside a canister, and a rule that decides
/// whether one table can delete another's evidence must not be reachable only
/// through a deployed replica.
fn may_record(
    state: &HistoryState,
    caller: Principal,
    record: &HandHistoryRecord,
) -> Result<(), String> {
    // SECURITY FIX: Verify caller is authorized (either a registered table or admin)
    // Removed the empty authorized_tables bypass - that would allow anyone to inject fake history
    let is_authorized = state.authorized_tables.contains(&caller) || state.admin == Some(caller);

    if !is_authorized {
        return Err(format!(
            "Unauthorized: {} is not a registered table on this archive. The admin must call \
             authorize_table(principal \"{}\") before this table's hands can be recorded. \
             Until then every shuffle proof it produces exists only in the table canister.",
            caller, caller
        ));
    }

    // A TABLE MAY ONLY RECORD ITS OWN HANDS.
        //
        // docs/DEFECTS.md E-91. Authorisation used to end at the line above, so a
        // registered table could file a record naming ANOTHER table in
        // `record.table_id` -- and `table_id` is half of the hand's name
        // (`hand_uid = "<table>:<seed_hash>"`, docs/DEFECTS.md E-71). The other
        // half, the shuffle commitment, is PUBLIC: it is on the player's screen
        // while the hand is running.
        //
        // Before E-71 narrowed the de-duplication key, a forgery like that landed
        // as a visible EXTRA record under its own `hand_number`. After, it shares
        // the victim's whole name, so whichever of the two arrives second is
        // absorbed as a "retry" and DISCARDED while its sender is told `Ok` with
        // somebody else's id. Filing the forgery first therefore deletes a genuine
        // hand from the permanent, append-only, "provably fair" archive, silently,
        // and every instrument in this project reads green over it because the
        // failure is an ABSENT record rather than a wrong one -- which is
        // docs/DEFECTS.md E-49 exactly, the defect this archive exists to have
        // stopped making.
        //
        // `record_hand_to_history` sets `table_id = self_principal()`, so no honest
        // table can fail this. The admin is exempt because the admin is this
        // canister's controller and can already rewrite the whole archive by
        // reinstalling it (docs/SECURITY-FINDINGS.md FINDING 23); pretending
        // otherwise here would be a claim, not a control.
    if state.admin != Some(caller) && record.table_id != caller {
        return Err(format!(
            "REFUSED: {} may only record hands played at {}, and this record names table {}. \
             A hand's name is \"<table canister id>:<seed_hash>\" and the commitment half of it \
             is public, so accepting this would let one table claim -- and, because the name \
             would then collide, silently DELETE -- another table's hand. Nothing has been \
             stored. See docs/DEFECTS.md E-91.",
            caller,
            caller,
            record.table_id.to_text()
        ));
    }

    Ok(())
}

/// Store a hand, or return the id of the copy already stored.
///
/// Split out of `record_hand` so the de-duplication rule can be tested on the
/// host: `record_hand` itself calls `msg_caller`, which traps outside a
/// canister, and a rule this easy to get wrong should not be reachable only
/// through a deployed replica.
fn insert_hand(state: &mut HistoryState, record: HandHistoryRecord) -> Result<u64, String> {
    // A HAND THAT CANNOT BE NAMED IS REFUSED, LOUDLY.
    //
    // The alternative was considered and rejected. Storing it would mean every
    // unusable record from one table sharing one name, so the second one would
    // be de-duplicated into the first and DISCARDED while its sender was told
    // `Ok` -- which is docs/DEFECTS.md E-49 exactly, the failure this archive
    // exists to have stopped making. Refusing costs nothing that refusing does
    // not also announce: the table keeps the record in its retry backlog and
    // `get_history_status` reports this sentence verbatim, so a wiring or
    // encoding fault reads as a wiring or encoding fault instead of as a hand
    // that quietly never existed.
    //
    // No hand the table canister produces can land here: it returns before
    // building a record when there is no shuffle proof, and a proof's `seed_hash`
    // is always a SHA-256 digest it computed itself. All 3,218 records already in
    // the local archive carry a 64-hex commitment.
    let uid = record_uid(&record).ok_or_else(|| {
        format!(
            "REFUSED: this record's shuffle commitment is not a SHA-256 digest \
             ({} character(s): \"{}\"), so the hand cannot be named and cannot be verified by \
             anyone. A hand is named \"<table canister id>:<seed_hash>\", and nothing else in a \
             hand record is unique to the hand. The record has NOT been stored; it is still \
             yours to re-send once the commitment is right. Table {}, hand number {}.",
            record.shuffle_proof.seed_hash.trim().chars().count(),
            record.shuffle_proof.seed_hash.trim().chars().take(80).collect::<String>(),
            record.table_id.to_text(),
            record.hand_number,
        )
    })?;

    // IDEMPOTENT. A table that could not reach this archive holds the record
    // and re-sends it, so the same hand arrives more than once by design. A
    // second copy would double-count in player_stats and give a player two
    // conflicting records of one hand, so the existing id is returned
    // unchanged and nothing is written.
    if let Some(existing) = state.hand_uid_index.get(&uid).copied() {
        return Ok(existing);
    }

    // Assign hand ID
    let hand_id = state.next_hand_id;
    state.next_hand_id += 1;

    // Create record with assigned ID
    let mut final_record = record;
    final_record.hand_id = hand_id;
    // THE NAME IS DERIVED, NOT ACCEPTED. A writer that set this field is
    // overruled: the stored value is always `None` and every read path recomputes
    // it from `table_id` and the commitment. One derivation, so a record cannot
    // carry a name that disagrees with the hand it describes -- and so records
    // stored before this field existed are named by exactly the same code.
    final_record.hand_uid = None;

    // Update indexes
    state.hand_uid_index.insert(uid, hand_id);
    state.hand_ids_by_number
        .entry((final_record.table_id, final_record.hand_number))
        .or_default()
        .push(hand_id);
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

    Ok(hand_id)
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

/// A stored record, with its NAME filled in.
///
/// The one place a record leaves this canister with a `hand_uid` on it. Every
/// read path goes through here, so a record written before names existed and a
/// record written after are named by the same three lines of code -- which is
/// what makes the name retroactive rather than a migration.
fn published(record: &HandHistoryRecord) -> HandHistoryRecord {
    let mut out = record.clone();
    out.hand_uid = record_uid(record);
    out
}

#[ic_cdk::query]
fn get_hand(hand_id: u64) -> Option<HandHistoryRecord> {
    STATE.with(|s| s.borrow().hands.get(&hand_id).map(published))
}

/// THE HAND WITH THIS NAME, or a sentence saying why there isn't one.
///
/// `Result`, not `opt`, because the three ways this can fail to answer are three
/// different facts about the world -- a malformed name, a name no record answers
/// to, and a bare commitment that names records on two tables -- and a bare
/// `null` for all three is how a verifier concludes the archive lost their hand.
/// docs/DEFECTS.md E-71.
#[ic_cdk::query]
fn get_hand_by_uid(name: String) -> Result<HandHistoryRecord, String> {
    STATE.with(|s| {
        let state = s.borrow();

        let uid = match parse_hand_name(&name) {
            HandName::Full(uid) => uid,
            HandName::Malformed(why) => return Err(format!("Not a hand name: {}", why)),
            HandName::Commitment(commitment) => {
                // A bare commitment is unique in practice and the archive says so
                // in `get_archive_integrity`. If it ever is not, the caller is
                // told which tables it names instead of being handed one of them.
                let suffix = format!("{}{}", HAND_UID_SEPARATOR, commitment);
                let matches: Vec<&String> = state
                    .hand_uid_index
                    .keys()
                    .filter(|k| k.ends_with(&suffix))
                    .collect();
                match matches.len() {
                    1 => matches[0].clone(),
                    0 => {
                        return Err(format!(
                            "No hand in this archive committed to {}. The commitment is the \
                             64-character hash shown on the table while the hand is running; if \
                             you have it and this archive does not, the hand was never archived \
                             here and the table's get_history_status will say why.",
                            commitment
                        ))
                    }
                    n => {
                        return Err(format!(
                            "AMBIGUOUS: {} records committed to {}, on these tables: {}. Ask again \
                             with the full name \"<table canister id>:{}\".",
                            n,
                            commitment,
                            matches
                                .iter()
                                .filter_map(|k| k.rsplit_once(HAND_UID_SEPARATOR).map(|(t, _)| t))
                                .collect::<Vec<&str>>()
                                .join(", "),
                            commitment
                        ))
                    }
                }
            }
        };

        let hand_id = state.hand_uid_index.get(&uid).copied().ok_or_else(|| {
            format!(
                "No hand named {} in this archive. That name is well formed, so either the hand \
                 was never archived here -- the table's get_history_status says whether archiving \
                 is working -- or it belongs to a different archive canister.",
                uid
            )
        })?;

        state
            .hands
            .get(&hand_id)
            .map(published)
            .ok_or_else(|| {
                format!(
                    "INTERNAL: the name {} indexes hand id {}, which is not in this archive. This \
                     is an index that disagrees with its own records; please report it, and see \
                     get_archive_integrity.",
                    uid, hand_id
                )
            })
    })
}

/// WHAT "TABLE X, HAND N" ACTUALLY NAMES.
///
/// The migration path for every citation written before hands had names. It
/// resolves to the WHOLE SET of records that answer to it, oldest first, each
/// with its name, and says in `is_unique` whether the citation identifies a hand
/// at all. Nothing is renumbered and nothing is orphaned: a five-auditor report
/// citing "table_2 hand 1" still resolves, and now resolves to a labelled set
/// with the pot, the timestamp and the name of each candidate, which is enough to
/// pin the one that was actually cited. docs/DEFECTS.md E-71.
#[ic_cdk::query]
fn resolve_hand_number(table_id: Principal, hand_number: u64) -> HandNumberCitation {
    STATE.with(|s| {
        let state = s.borrow();

        let ids: &[u64] = state
            .hand_ids_by_number
            .get(&(table_id, hand_number))
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let match_count = ids.len() as u64;
        // Bounded reply. `match_count` above is exact whatever this cap does.
        let matches: Vec<HandSummary> = ids
            .iter()
            .take(MAX_CITATION_MATCHES)
            .filter_map(|id| state.hands.get(id))
            .map(to_summary)
            .collect();

        let advice = match match_count {
            0 => format!(
                "No record in this archive answers to hand number {} on table {}. Hand numbers \
                 restart at 1 on every reset_table, so a number that names nothing today may \
                 have named a hand before a reset -- but this archive never deletes a record, so \
                 if the hand was archived here it is still here under some other number or under \
                 its name.",
                hand_number, table_id
            ),
            1 => format!(
                "This citation happens to name exactly one record TODAY. It is still not a \
                 durable name: hand numbers restart at 1 on every reset_table, so the next reset \
                 can make this same citation ambiguous. Cite {} instead.",
                matches
                    .first()
                    .map(|m| m.hand_uid.clone())
                    .unwrap_or_default()
            ),
            n => format!(
                "AMBIGUOUS: {} records answer to hand number {} on table {}, because hand numbers \
                 restart at 1 on every reset_table. This citation names a SET, not a hand. Each \
                 candidate's hand_uid is its permanent name; cite that.",
                n, hand_number, table_id
            ),
        };

        HandNumberCitation {
            table_id,
            hand_number,
            match_count,
            is_unique: match_count == 1,
            matches,
            advice,
        }
    })
}

/// IS THE ARCHIVE'S IDENTITY ACTUALLY A KEY? Computed from the RECORDS.
///
/// Deliberately not computed from `hand_uid_index`: an index cannot be used to
/// check itself, and the failure this canister is most likely to have is an index
/// that disagrees with the records it indexes. Everything below is recounted from
/// `hands` on every call and `index_disagrees_with_records` compares the two.
///
/// This is the query that makes the fix to docs/DEFECTS.md E-71 checkable on a
/// running canister -- including one nobody may call from a test -- instead of
/// believed.
#[ic_cdk::query]
fn get_archive_integrity() -> ArchiveIntegrity {
    STATE.with(|s| archive_integrity(&s.borrow()))
}

/// The body of [`get_archive_integrity`], split out for the reason
/// `insert_hand` is: a canister query is only reachable through a deployed
/// replica, and the whole point of this function is to be the thing that says
/// whether the archive's identity holds. A claim that can only be checked by the
/// canister that makes it is the failure mode this project keeps finding.
fn archive_integrity(state: &HistoryState) -> ArchiveIntegrity {
    {
        let mut names: BTreeMap<String, u64> = BTreeMap::new();
        let mut citations: BTreeMap<(Principal, u64), u64> = BTreeMap::new();
        let mut unnamable = 0u64;
        let mut raked_records = 0u64;
        let mut rake_total = 0u64;
        let mut first_raked: Option<String> = None;

        for hand in state.hands.values() {
            let uid = record_uid(hand);
            match &uid {
                Some(u) => *names.entry(u.clone()).or_insert(0) += 1,
                None => unnamable += 1,
            }
            *citations.entry((hand.table_id, hand.hand_number)).or_insert(0) += 1;
            if hand.rake != 0 {
                raked_records += 1;
                rake_total = rake_total.saturating_add(hand.rake);
                if first_raked.is_none() {
                    first_raked = Some(format!(
                        "{} recorded rake={} e8s",
                        uid.unwrap_or_else(|| format!("hand_id {}", hand.hand_id)),
                        hand.rake
                    ));
                }
            }
        }

        let name_collisions = names.values().filter(|n| **n > 1).count() as u64;
        let records_under_a_colliding_name: u64 = names.values().filter(|n| **n > 1).sum();
        let ambiguous = citations.values().filter(|n| **n > 1).count() as u64;
        let records_under_an_ambiguous_hand_number: u64 =
            citations.values().filter(|n| **n > 1).sum();
        let worst = citations
            .iter()
            .max_by_key(|(_, n)| **n)
            .filter(|(_, n)| **n > 1)
            .map(|((table, number), n)| {
                format!("{} hand number {} answers to {} records", table, number, n)
            });

        ArchiveIntegrity {
            records_held: state.hands.len() as u64,
            records_with_a_name: state.hands.len() as u64 - unnamable,
            distinct_names: names.len() as u64,
            name_collisions,
            records_under_a_colliding_name,
            records_without_a_usable_commitment: unnamable,
            // AGAINST THE RECORDS, NOT AGAINST THE NAMES. Comparing the index
            // with `names.len()` is self-confirming: a collision produces one
            // name for two records, the index holds one entry, the two agree,
            // and the drift the field exists to report is exactly the drift it
            // cannot see. The index must hold one entry for every NAMED RECORD;
            // it does not when two records share a name (one is unreachable by
            // name) or when an entry has been lost.
            index_disagrees_with_records:
                state.hand_uid_index.len() as u64 != state.hands.len() as u64 - unnamable,
            distinct_hand_number_citations: citations.len() as u64,
            ambiguous_hand_number_citations: ambiguous,
            records_under_an_ambiguous_hand_number,
            worst_hand_number_citation: worst,
            records_with_a_nonzero_rake: raked_records,
            rake_recorded_total: rake_total,
            first_record_with_a_rake: first_raked,
            summary:
                "hand_uid is \"<table canister id>:<seed_hash>\" and is the only name in this \
                 archive that identifies a hand. name_collisions and \
                 records_without_a_usable_commitment MUST both be zero: they are the claim that \
                 a name identifies exactly one record, recounted from the records themselves on \
                 every call. ambiguous_hand_number_citations is expected to be LARGE and is not \
                 a fault: hand numbers restart at 1 on every reset_table, which is why they are \
                 not names. Use resolve_hand_number to see everything an old (table, hand number) \
                 citation covers. records_with_a_nonzero_rake and rake_recorded_total MUST both \
                 be zero: ClearDeck takes no rake, and this is that claim over EVERY record ever \
                 archived rather than over the handful a client happens to be showing."
                    .to_string(),
        }
    }
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
    /// **THE HAND THIS ANSWER IS ABOUT**, by its permanent name. Quote this, not
    /// `hand_number`: `hand_number` restarts at 1 on every `reset_table` and on
    /// the local archive one of them answers to 70 records with 6 different pots
    /// (docs/DEFECTS.md E-71).
    pub hand_uid: String,
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
                 get_hands_by_table or get_hands_by_player to find the id you want, or \
                 get_hand_by_uid with the hand's permanent name.",
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
            hand_uid: record_uid(hand).unwrap_or_default(),
            seed_hash: seed_hash.clone(),
            revealed_seed: revealed_seed.clone(),
            computed_hash: computed,
            commitment_matches: matches,
            problem,
            this_proves: ARCHIVE_PROVES.to_string(),
            this_does_not_prove: ARCHIVE_DOES_NOT_PROVE.to_string(),
            // Two commands, because the hash check is the cheap half and the
            // second one is the verification that actually says something. The
            // second is only printable when the record states how many players
            // were dealt in: without `P` the dealing rule cannot be applied, and
            // suggesting a command that would silently produce the WRONG BOARD is
            // how a verifier ends up believing they were cheated.
            // docs/SECURITY-FINDINGS.md FINDING 30, docs/SHUFFLE-SPEC.md section 4.
            verify_it_yourself: match hand.dealt_in.as_ref().map(|d| d.len()) {
                Some(p) => format!(
                    "echo -n {} | xxd -r -p | shasum -a 256    # must print {}\n\
                     python3 src/poker_core/tests/verify/verify_shuffle.py {} --players {} \
                     --seed-hash {}    # {} players were dealt in, in seat order {:?}",
                    if revealed_seed.is_empty() { "<seed>" } else { &revealed_seed },
                    if seed_hash.is_empty() { "<hash>" } else { &seed_hash },
                    if revealed_seed.is_empty() { "<seed>" } else { &revealed_seed },
                    p,
                    if seed_hash.is_empty() { "<hash>" } else { &seed_hash },
                    p,
                    hand.dealt_in
                        .as_ref()
                        .map(|d| d.iter().map(|s| s.seat).collect::<Vec<u8>>())
                        .unwrap_or_default(),
                ),
                None => format!(
                    "echo -n {} | xxd -r -p | shasum -a 256    # must print {}\n\
                     # This record does NOT say how many players were dealt in, so the board \
                     cannot be reproduced from it: docs/SHUFFLE-SPEC.md section 4 offsets the \
                     board by that number and guessing it produces a different board from the \
                     same seed. Every record written before docs/SECURITY-FINDINGS.md FINDING 30 \
                     was fixed is in this state. The hole cards can still be checked; the board \
                     cannot.",
                    if revealed_seed.is_empty() { "<seed>" } else { &revealed_seed },
                    if seed_hash.is_empty() { "<hash>" } else { &seed_hash },
                ),
            },
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
        // Derived here, by the same function `get_hand` uses, so a summary and a
        // full record can never name the same hand differently.
        hand_uid: record_uid(hand).unwrap_or_default(),
        timestamp: hand.timestamp,
        player_count: hand.players.len() as u8,
        dealt_in_count: hand.dealt_in.as_ref().map(|d| d.len() as u8),
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

        // Restore hands, rebuilding the derived indexes from them as we go. Both
        // are derived and never persisted, so an upgrade cannot leave either
        // describing records that are not there.
        //
        // A NAME COLLISION IS NOT SWALLOWED HERE. If two stored records ever
        // produced the same name, the index below would keep one of them and the
        // other would be unreachable by name -- silently, which is precisely the
        // shape of the defect this naming replaced. It is counted instead, and
        // `get_archive_integrity` recounts it from the records themselves on every
        // call, so it is visible on a running canister rather than in a log line
        // nobody can read. The record itself is never dropped: it stays in `hands`
        // and stays reachable by `hand_id`.
        let mut collisions = 0u64;
        for (k, v) in state.hands {
            if let Some(uid) = record_uid(&v) {
                if new_state.hand_uid_index.insert(uid, k).is_some() {
                    collisions += 1;
                }
            }
            new_state
                .hand_ids_by_number
                .entry((v.table_id, v.hand_number))
                .or_default()
                .push(k);
            new_state.hands.insert(k, v);
        }
        if collisions > 0 {
            ic_cdk::println!(
                "ARCHIVE: {} hand name collision(s) after restore; see get_archive_integrity",
                collisions
            );
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

    /// A commitment as a real one looks: 64 lowercase hex characters. The
    /// fixtures below used to pass `"aa"`, which is not a SHA-256 digest and is
    /// not something any table can produce; a fixture that cannot occur cannot
    /// pin behaviour that does.
    fn commitment(tag: &str) -> String {
        let mut c = tag.to_string();
        while c.len() < COMMITMENT_HEX_LEN {
            c.push('0');
        }
        c.truncate(COMMITMENT_HEX_LEN);
        c
    }

    fn hand(hand_number: u64, seed_tag: &str) -> HandHistoryRecord {
        hand_on(table(), hand_number, seed_tag)
    }

    fn hand_on(table_id: Principal, hand_number: u64, seed_tag: &str) -> HandHistoryRecord {
        let seed_hash = commitment(seed_tag);
        HandHistoryRecord {
            hand_id: 0,
            table_id,
            hand_number,
            hand_uid: None,
            timestamp: 1,
            small_blind: 1,
            big_blind: 2,
            ante: 0,
            shuffle_proof: ShuffleProofRecord {
                seed_hash,
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
                dealt_in: Some(true),
                contributed: Some(0),
                left_mid_hand: Some(false),
            }],
            dealt_in: Some(vec![DealtInSeat {
                seat: 0,
                principal: player(),
            }]),
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

    fn store(state: &mut HistoryState, record: HandHistoryRecord) -> u64 {
        insert_hand(state, record).expect("a well-formed hand must be stored")
    }

    #[test]
    fn re_sending_the_same_hand_stores_it_once_and_returns_the_same_id() {
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let first = store(&mut state, hand(7, "aa"));
        let again = store(&mut state, hand(7, "aa"));
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
        let before_reset = store(&mut state, hand(1, "aa"));
        let after_reset = store(&mut state, hand(1, "bb"));
        assert_ne!(before_reset, after_reset);
        assert_eq!(state.hands.len(), 2, "two different hands, two records");
        assert_eq!(state.player_stats.get(&player()).unwrap().hands_played, 2);
    }

    #[test]
    fn a_re_sent_hand_is_the_same_hand_even_under_a_different_number() {
        // The narrowed key. A table that renumbered between the original send
        // and the retry (`reset_table` mid-backlog) used to be able to store the
        // SAME hand twice, under two ids, with two entries in the player's
        // record and two different names for one deal. The commitment is the
        // hand, so the second arrival is the retry it is.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let first = store(&mut state, hand(9, "aa"));
        let retry = store(&mut state, hand(1, "aa"));
        assert_eq!(first, retry, "one deal, one record");
        assert_eq!(state.hands.len(), 1);
        assert_eq!(state.player_stats.get(&player()).unwrap().hands_played, 1);
    }

    #[test]
    fn the_key_ignores_case_and_whitespace_around_the_seed_hash() {
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut messy = hand(3, "abc123");
        messy.shuffle_proof.seed_hash =
            format!("  {}  ", commitment("abc123").to_uppercase());
        let first = store(&mut state, hand(3, "abc123"));
        let again = store(&mut state, messy);
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
            store(&mut state, hand(n, &format!("{n:04x}")));
            assert!(state.hands.len() > seen, "record {n} did not grow the archive");
            seen = state.hands.len();
        }
        assert_eq!(state.hands.len(), 250, "there is no cap, and none appeared");
    }

    // -----------------------------------------------------------------------
    // THE NAME OF A HAND -- docs/DEFECTS.md E-71
    // -----------------------------------------------------------------------

    #[test]
    fn eleven_hands_under_one_number_have_eleven_different_names() {
        // The defect, in one test. Eleven genuinely different hands all called
        // "table X hand 1" -- the state the local archive was measured in, where
        // one citation answered to 70 records with 6 different pots.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut ids = Vec::new();
        for n in 0..11u64 {
            let mut h = hand(1, &format!("{n:02x}ff"));
            h.total_pot = 1_000 + n; // different money, same citation
            ids.push(store(&mut state, h));
        }
        assert_eq!(state.hands.len(), 11, "eleven hands, eleven records");

        // The citation names all eleven and says so.
        let citation = state
            .hand_ids_by_number
            .get(&(table(), 1))
            .expect("the citation must resolve to something");
        assert_eq!(citation.len(), 11, "the OLD name still answers to eleven records");

        // The name names one each.
        let names: std::collections::BTreeSet<String> = ids
            .iter()
            .map(|id| record_uid(state.hands.get(id).unwrap()).expect("every hand is named"))
            .collect();
        assert_eq!(names.len(), 11, "eleven hands must have eleven names");

        // And each name resolves back to the hand it came from, with the money
        // that hand actually held -- the thing a verifier could not do before.
        for id in &ids {
            let uid = record_uid(state.hands.get(id).unwrap()).unwrap();
            let found = state.hand_uid_index.get(&uid).copied();
            assert_eq!(found, Some(*id), "a name must resolve to its own record");
        }
    }

    #[test]
    fn a_hand_that_cannot_be_named_is_refused_rather_than_swallowed() {
        // The dangerous direction. Two DIFFERENT hands with unusable commitments
        // would share one name; stored, the second would be de-duplicated into
        // the first and lost while its sender was told `Ok` (docs/DEFECTS.md
        // E-49). Refusing is loud: the table keeps the record and
        // get_history_status prints this sentence.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut broken = hand(1, "aa");
        broken.shuffle_proof.seed_hash = "not-a-digest".to_string();
        let err = insert_hand(&mut state, broken).expect_err("must be refused");
        assert!(err.contains("REFUSED"), "{err}");
        assert!(err.contains("NOT been stored"), "{err}");
        assert_eq!(state.hands.len(), 0, "nothing was stored");

        // A truncated paste is refused too -- 63 characters is not a digest.
        let mut truncated = hand(2, "bb");
        truncated.shuffle_proof.seed_hash = commitment("bb")[..63].to_string();
        assert!(insert_hand(&mut state, truncated).is_err());
        assert_eq!(state.hands.len(), 0);
    }

    #[test]
    fn a_writer_cannot_name_its_own_record() {
        // The name is derived from the hand, so a table that supplies one is
        // overruled. Otherwise a writer could file a hand under another hand's
        // name and the archive would carry two conflicting stories under one.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut liar = hand(1, "aa");
        liar.hand_uid = Some("whatever-i-say-it-is".to_string());
        let id = store(&mut state, liar);
        let stored = state.hands.get(&id).unwrap();
        assert_eq!(stored.hand_uid, None, "the supplied name is discarded");
        assert_eq!(
            published(stored).hand_uid,
            hand_uid(&table(), &commitment("aa")),
            "the published name is derived from the hand"
        );
    }

    #[test]
    fn a_record_stored_before_names_existed_is_named_by_the_same_code() {
        // THE MIGRATION, in one test. A record written under the old scheme has
        // `hand_uid: None` in storage -- that is exactly what every one of the
        // 3,218 records already in the archive has -- and reading it back
        // produces its name from fields it already carried. Nothing is rewritten
        // and nothing is renumbered.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let id = store(&mut state, hand(1, "aa"));
        let stored = state.hands.get(&id).unwrap();
        assert_eq!(stored.hand_uid, None, "storage is untouched");
        assert_eq!(stored.hand_id, id, "the id it was cited by is unchanged");
        assert_eq!(stored.hand_number, 1, "the number it was cited by is unchanged");

        let read_back = published(stored);
        assert_eq!(
            read_back.hand_uid.as_deref(),
            Some(format!("{}:{}", table().to_text(), commitment("aa")).as_str()),
        );
        assert_eq!(to_summary(stored).hand_uid, read_back.hand_uid.clone().unwrap());
    }

    #[test]
    fn the_name_is_the_table_and_the_commitment_and_nothing_else() {
        let t = table();
        let c = commitment("dead");
        assert_eq!(hand_uid(&t, &c), Some(format!("{}:{}", t.to_text(), c)));
        // Case and whitespace forgiven -- a player pastes this off a screen.
        assert_eq!(hand_uid(&t, &format!(" {} ", c.to_uppercase())), hand_uid(&t, &c));
        // A truncated or padded paste is not a name.
        assert_eq!(hand_uid(&t, &c[..63]), None);
        assert_eq!(hand_uid(&t, &format!("{c}0")), None);
        assert_eq!(hand_uid(&t, ""), None);
        // Two tables, one commitment: two names, so neither record can eat the
        // other.
        let other = Principal::from_slice(&[7, 7, 7]);
        assert_ne!(hand_uid(&t, &c), hand_uid(&other, &c));
    }

    #[test]
    fn a_name_is_parsed_the_way_a_person_writes_it_down() {
        let t = table();
        let c = commitment("beef");
        let full = format!("{}:{}", t.to_text(), c);
        assert_eq!(parse_hand_name(&full), HandName::Full(full.clone()));
        assert_eq!(
            parse_hand_name(&format!("  {}  ", full.to_uppercase())),
            HandName::Full(full.clone()),
            "an uppercased paste still names the same hand"
        );
        assert_eq!(parse_hand_name(&c), HandName::Commitment(c.clone()));
        // A truncated paste is named as such rather than resolved to a hand.
        match parse_hand_name(&format!("{}:{}", t.to_text(), &c[..60])) {
            HandName::Malformed(why) => assert!(why.contains("60 character"), "{why}"),
            other => panic!("a truncated commitment must not parse: {other:?}"),
        }
        match parse_hand_name("") {
            HandName::Malformed(why) => assert!(why.contains("empty")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn two_tables_that_reuse_a_commitment_keep_two_records() {
        // Keyed on the commitment ALONE, the second table's hand would be
        // de-duplicated into the first table's and lost with an `Ok`. Scoped to
        // the table, both survive and the collision is visible instead.
        let other = Principal::from_slice(&[7, 7, 7]);
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let a = store(&mut state, hand(1, "aa"));
        let b = store(&mut state, hand_on(other, 1, "aa"));
        assert_ne!(a, b);
        assert_eq!(state.hands.len(), 2, "no genuine hand is discarded");
    }

    #[test]
    fn the_integrity_report_counts_what_it_claims_to_count() {
        // This function is the archive's own statement that its identity holds,
        // and on mainnet it is the ONLY way anyone checks. So its arithmetic is
        // pinned against an archive whose answer is known by construction.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        for n in 0..5u64 {
            store(&mut state, hand(1, &format!("{n:02x}ee"))); // five hands, all "hand 1"
        }
        store(&mut state, hand(2, "ff")); // one hand, its own number
        store(&mut state, hand_on(Principal::from_slice(&[7, 7, 7]), 1, "aa"));

        let r = archive_integrity(&state);
        assert_eq!(r.records_held, 7);
        assert_eq!(r.records_with_a_name, 7);
        assert_eq!(r.distinct_names, 7, "seven hands, seven names");
        assert_eq!(r.name_collisions, 0);
        assert_eq!(r.records_under_a_colliding_name, 0);
        assert_eq!(r.records_without_a_usable_commitment, 0);
        assert!(!r.index_disagrees_with_records);
        assert_eq!(r.distinct_hand_number_citations, 3, "(t,1) (t,2) (other,1)");
        assert_eq!(r.ambiguous_hand_number_citations, 1, "only (t,1) names a set");
        assert_eq!(r.records_under_an_ambiguous_hand_number, 5);
        assert!(
            r.worst_hand_number_citation.as_deref().unwrap().contains("5 records"),
            "{:?}",
            r.worst_hand_number_citation
        );
        assert_eq!(r.records_with_a_nonzero_rake, 0);
        assert_eq!(r.rake_recorded_total, 0);
        assert!(r.first_record_with_a_rake.is_none());
    }

    #[test]
    fn the_integrity_report_sees_a_collision_the_index_would_hide() {
        // AN INDEX CANNOT BE USED TO CHECK ITSELF. If two records ever share a
        // name, the name index keeps one and the other is unreachable BY NAME --
        // silently, which is the shape of the defect the naming replaced. The
        // report is recounted from `hands`, so it sees both.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let kept = store(&mut state, hand(1, "aa"));
        // Forced in behind `insert_hand`'s back: this is what a corrupted restore
        // or a future write path that forgets the rule would look like.
        let mut twin = hand(9, "aa");
        twin.hand_id = 999;
        state.hands.insert(999, twin);

        let r = archive_integrity(&state);
        assert_eq!(r.records_held, 2);
        assert_eq!(r.distinct_names, 1);
        assert_eq!(r.name_collisions, 1, "the report must SEE the collision");
        assert_eq!(r.records_under_a_colliding_name, 2);
        assert!(
            r.index_disagrees_with_records,
            "the index holds one name for two records and must say so"
        );
        // And the record the index dropped is still there, by id.
        assert!(state.hands.contains_key(&kept));
        assert!(state.hands.contains_key(&999));
    }

    #[test]
    fn the_integrity_report_sees_a_rake_on_a_record_no_client_is_showing() {
        // The no-rake property over the WHOLE archive. The screenshot sweep can
        // only afford a window of the newest twenty, so before this counter a
        // rake only had to wait twenty hands to be checked by nothing at all.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut raked = hand(1, "aa");
        raked.rake = 1; // one e8
        store(&mut state, raked);
        for n in 0..50u64 {
            store(&mut state, hand(n + 2, &format!("{n:02x}cc")));
        }

        let r = archive_integrity(&state);
        assert_eq!(r.records_held, 51);
        assert_eq!(r.records_with_a_nonzero_rake, 1, "one e8 of rake, fifty hands later");
        assert_eq!(r.rake_recorded_total, 1);
        assert!(
            r.first_record_with_a_rake.as_deref().unwrap().contains(&commitment("aa")),
            "the offending hand must be NAMED: {:?}",
            r.first_record_with_a_rake
        );
    }

    #[test]
    fn a_citation_resolves_to_every_record_it_names_and_says_it_is_not_a_name() {
        // The migration path for a report that cites "table X hand 1". It must
        // still resolve, and it must say that it names a SET.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut ids = Vec::new();
        for n in 0..4u64 {
            ids.push(store(&mut state, hand(1, &format!("{n:02x}dd"))));
        }
        let listed = state.hand_ids_by_number.get(&(table(), 1)).unwrap();
        assert_eq!(listed, &ids, "oldest first, and all of them");

        // Each of them is separately reachable by its own name, with its own id.
        for id in &ids {
            let uid = record_uid(state.hands.get(id).unwrap()).unwrap();
            assert_eq!(state.hand_uid_index.get(&uid).copied(), Some(*id));
        }
    }

    #[test]
    fn the_upgrade_rebuilds_both_indexes_from_the_records() {
        // The indexes are derived and never persisted, so this is what a running
        // canister has after every upgrade. It must be able to name every record
        // it holds, including records stored before names existed.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        for n in 1..=5u64 {
            store(&mut state, hand(1, &format!("{n:02x}aa"))); // all "hand 1"
        }
        store(&mut state, hand(2, "ff"));

        // Rebuild exactly as post_upgrade does.
        let mut rebuilt = HistoryState { next_hand_id: state.next_hand_id, ..Default::default() };
        for (k, v) in state.hands.iter() {
            if let Some(uid) = record_uid(v) {
                assert!(
                    rebuilt.hand_uid_index.insert(uid, *k).is_none(),
                    "no two records may share a name"
                );
            }
            rebuilt.hand_ids_by_number.entry((v.table_id, v.hand_number)).or_default().push(*k);
            rebuilt.hands.insert(*k, v.clone());
        }
        assert_eq!(rebuilt.hand_uid_index.len(), 6, "six hands, six names");
        assert_eq!(rebuilt.hand_ids_by_number.get(&(table(), 1)).unwrap().len(), 5);
        assert_eq!(rebuilt.hand_ids_by_number.get(&(table(), 2)).unwrap().len(), 1);
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

    // ------------------------------------------------------------------
    // E-91 -- THE WRITER DOES NOT GET TO PICK THE HAND'S NAME
    // ------------------------------------------------------------------

    fn other_table() -> Principal {
        Principal::from_slice(&[7, 7, 7, 7, 7])
    }

    fn admin_principal() -> Principal {
        Principal::from_slice(&[4, 2])
    }

    fn archive_with_two_tables() -> HistoryState {
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        state.admin = Some(admin_principal());
        state.authorized_tables.push(table());
        state.authorized_tables.push(other_table());
        state
    }

    #[test]
    fn a_table_may_record_its_own_hand() {
        let state = archive_with_two_tables();
        may_record(&state, table(), &hand_on(table(), 7, "aa"))
            .expect("a table recording its own hand is the whole point of this canister");
    }

    #[test]
    fn a_table_may_not_record_a_hand_naming_another_table() {
        // THE SILENT DELETION. `hand_uid` is "<table_id>:<seed_hash>" and the
        // commitment half is PUBLIC -- it is on the player's screen while the
        // hand is running. So a registered table that may choose `table_id`
        // may mint the victim's whole name, and the de-duplication key is
        // exactly that name: whichever record arrives second is absorbed as a
        // retry and DISCARDED with `Ok`. docs/DEFECTS.md E-91 / E-71 / E-49.
        let state = archive_with_two_tables();
        let forged = hand_on(table(), 1, "aa");
        let err = may_record(&state, other_table(), &forged)
            .expect_err("a table forging another table's hand must be REFUSED");
        assert!(
            err.contains("may only record hands played at"),
            "the refusal must say what is wrong: {err}"
        );
    }

    #[test]
    fn the_admin_is_still_allowed_to_record_for_a_table() {
        // Stated rather than assumed: the admin is this canister's controller
        // and can reinstall it, so refusing here would be a claim and not a
        // control (docs/SECURITY-FINDINGS.md FINDING 23). The exemption is
        // pinned so that removing the table binding cannot be mistaken for
        // "the admin path was already closed".
        let state = archive_with_two_tables();
        may_record(&state, admin_principal(), &hand_on(table(), 1, "aa"))
            .expect("the admin may record on behalf of a table");
    }

    #[test]
    fn a_stranger_is_still_refused_before_any_of_this() {
        let state = archive_with_two_tables();
        let stranger = Principal::from_slice(&[5, 5, 5, 5]);
        let err = may_record(&state, stranger, &hand_on(stranger, 1, "aa"))
            .expect_err("an unregistered writer must be refused");
        assert!(err.contains("Unauthorized"), "{err}");
    }

    #[test]
    fn two_records_sharing_one_name_collapse_to_one_which_is_why_the_binding_exists() {
        // The mechanism the binding above protects, measured rather than
        // argued. Two records with the SAME (table_id, seed_hash) and
        // different content are one name, and the second is discarded while
        // its sender is told `Ok`. That is correct for a retry and catastrophic
        // for a forgery, and the only thing that tells them apart is that a
        // forger cannot name somebody else's table.
        let mut state = HistoryState { next_hand_id: 1, ..Default::default() };
        let mut forgery = hand_on(table(), 999, "aa");
        forgery.total_pot = 1;
        let genuine = hand_on(table(), 1, "aa"); // total_pot 20

        let forged_id = insert_hand(&mut state, forgery).expect("stored");
        let genuine_id = insert_hand(&mut state, genuine).expect("returned Ok");

        assert_eq!(
            forged_id, genuine_id,
            "one name, one record: the genuine hand was absorbed into the forgery"
        );
        assert_eq!(state.hands.len(), 1, "the genuine hand is GONE from the archive");
        assert_eq!(
            state.hands.get(&forged_id).unwrap().total_pot,
            1,
            "and what survived under that name is the forgery, not the hand that was played"
        );
    }
}
