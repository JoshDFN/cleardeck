//! Poker hand ranking.
//!
//! MOVED VERBATIM out of `src/table_canister/src/lib.rs` (commit ceacc37).
//! `HandRank`'s variant ORDER defines who wins a pot: the derived `Ord` compares
//! by discriminant first, so `HighCard` must stay first and `RoyalFlush` last.
//! `HandRank` is also on the Candid wire (`Winner.hand_rank`, `ShowdownPlayer`).

use crate::card::{Card, Suit};
use candid::{CandidType, Deserialize};
use std::collections::HashMap;

/// The number of cards a poker hand is ranked from. Not configurable: five.
const HAND_SIZE: usize = 5;
/// A board is a flop, a turn or a river. Nothing else is a legal showdown board.
const MIN_BOARD: usize = 3;
const MAX_BOARD: usize = 5;

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

// =============================================================================
// INPUT VALIDATION AT THE ENGINE BOUNDARY  (docs/DEFECTS.md E-09)
// =============================================================================
//
// WHY THIS EXISTS. `evaluate_hand` is the function `table_canister::
// determine_winners` calls to decide who takes a pot of real ICP/ckBTC. Before
// this, it validated nothing:
//
//   evaluate_hand(Ah + Ah, [Kh Qh Jh 2c 3d]) -> Flush([14,14,13,12,11])
//       a five-card flush assembled from FOUR physical cards
//   evaluate_five_cards([2h 3h 4h 5h 9h 6c 7c]) -> StraightFlush(7)
//       a straight flush that is in no five of those cards
//   evaluate_hand(Ah + Kd, []) -> HighCard([])
//       Ord-EQUAL to every other player's HighCard([]), so everyone chops
//
// So a dealing or shuffling defect upstream was laundered into a plausible
// winning hand and the pot was paid out on it. And per docs/DEFECTS.md H-05 the
// differential harness cannot cover for this: `rs_poker` ranks `Ah Ah Ah Ah Ah`
// as a straight flush and a two-card hand as a high card, and `phevaluator`
// returns a sentinel. NO reference evaluator in the harness validates its own
// input, so the check has to be here.
//
// WHY A PANIC (= AN IC TRAP) AND NOT A `Result` ON THE MONEY PATH.
// `evaluate_hand`/`evaluate_five_cards` keep their `-> HandRank` signature and
// PANIC on impossible input. In a canister a Rust panic is a trap: the whole
// message is rejected and every state change it made is rolled back. On the
// showdown path that is the only safe failure mode available:
//
//   * The alternative is a `HandRank` for a hand that does not exist, and the
//     caller cannot tell it apart from a real one. That pays the wrong player,
//     irreversibly, from a pot of real funds.
//   * A trap moves NO money. `state.pot` and every player's
//     `total_bet_this_hand` survive the rollback intact, so the funds stay fully
//     attributable and a controller can settle or refund deliberately.
//   * The inputs are never attacker-chosen. They come from the canister's own
//     shuffled deck, so a trap here is not a griefing vector -- it can only fire
//     when the deck or the deal is already corrupt, i.e. when *nothing* the
//     engine would answer is trustworthy.
//   * Cost: a corrupt deck now wedges that hand instead of settling it wrongly.
//     Deliberate trade. A stuck hand is recoverable; a wrong payout is not.
//
// `try_evaluate_*` are the same checks without the panic, for callers that want
// to decide for themselves (harnesses, replay tools, anything off-chain).
// `evaluate_*_unchecked` is the OLD unvalidated behaviour, kept public ONLY so
// off-chain analysis tools can still probe misuse. Never call it on a money path.

/// Why `poker_core` refused to rank a hand. Every variant describes a physically
/// impossible input, i.e. an upstream bug, never a legal game state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandInputError {
    /// `evaluate_five_cards` ranks exactly five cards. It was given something else.
    NotFiveCards { got: usize },
    /// `evaluate_hand` takes two hole cards plus a flop, turn or river board.
    BadBoardLength { community: usize, total: usize },
    /// The same physical card appears at two positions in the same hand.
    DuplicateCard {
        card: Card,
        first: usize,
        second: usize,
    },
}

impl core::fmt::Display for HandInputError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HandInputError::NotFiveCards { got } => write!(
                f,
                "IMPOSSIBLE HAND: evaluate_five_cards ranks exactly {} cards, got {}",
                HAND_SIZE, got
            ),
            HandInputError::BadBoardLength { community, total } => write!(
                f,
                "IMPOSSIBLE HAND: evaluate_hand needs a {}-, {}- or {}-card board \
                 (flop/turn/river), got {} community cards ({} cards in total)",
                MIN_BOARD,
                MIN_BOARD + 1,
                MAX_BOARD,
                community,
                total
            ),
            HandInputError::DuplicateCard {
                card,
                first,
                second,
            } => write!(
                f,
                "IMPOSSIBLE HAND: {:?} of {:?} appears at BOTH position {} and position {}; \
                 the deck or the deal is corrupt",
                card.rank, card.suit, first, second
            ),
        }
    }
}

/// Every card in `cards` must be a distinct physical card.
///
/// `Card` deliberately does not derive `Hash` (its derives are on the Candid wire
/// and in stable memory; see `card.rs`), so this is a pairwise scan. At most 7
/// cards ever reach it, so that is 21 comparisons.
pub fn validate_distinct(cards: &[Card]) -> Result<(), HandInputError> {
    for (i, a) in cards.iter().enumerate() {
        for (j, b) in cards.iter().enumerate().skip(i + 1) {
            if a == b {
                return Err(HandInputError::DuplicateCard {
                    card: *a,
                    first: i,
                    second: j,
                });
            }
        }
    }
    Ok(())
}

/// `evaluate_five_cards`'s contract: exactly five distinct cards.
pub fn validate_five_cards(cards: &[Card]) -> Result<(), HandInputError> {
    if cards.len() != HAND_SIZE {
        return Err(HandInputError::NotFiveCards { got: cards.len() });
    }
    validate_distinct(cards)
}

/// `evaluate_hand`'s contract: two hole cards, a 3/4/5-card board, all distinct.
pub fn validate_hand_input(
    hole_cards: &(Card, Card),
    community: &[Card],
) -> Result<(), HandInputError> {
    let total = 2 + community.len();
    if community.len() < MIN_BOARD || community.len() > MAX_BOARD {
        return Err(HandInputError::BadBoardLength {
            community: community.len(),
            total,
        });
    }
    let mut all_cards: Vec<Card> = Vec::with_capacity(total);
    all_cards.push(hole_cards.0);
    all_cards.push(hole_cards.1);
    all_cards.extend_from_slice(community);
    validate_distinct(&all_cards)
}

/// Evaluates a player's best 5-card hand from their 2 hole cards and up to 5 community cards.
///
/// Generates all possible 5-card combinations from the 7 available cards and returns
/// the highest-ranking hand according to standard poker hand rankings:
/// Royal Flush > Straight Flush > Four of a Kind > Full House > Flush >
/// Straight > Three of a Kind > Two Pair > One Pair > High Card
///
/// # Panics
///
/// Traps (panics, which is an IC trap that rolls the message back) if the input is
/// not two hole cards plus a flop/turn/river board of DISTINCT cards. Read the
/// block comment above for why a trap and not a `Result` here. Use
/// [`try_evaluate_hand`] if you want to handle the rejection yourself.
pub fn evaluate_hand(hole_cards: &(Card, Card), community: &[Card]) -> HandRank {
    try_evaluate_hand(hole_cards, community).unwrap_or_else(|e| panic!("{}", e))
}

/// [`evaluate_hand`] with the rejection returned instead of trapping.
pub fn try_evaluate_hand(
    hole_cards: &(Card, Card),
    community: &[Card],
) -> Result<HandRank, HandInputError> {
    validate_hand_input(hole_cards, community)?;
    Ok(evaluate_hand_unchecked(hole_cards, community))
}

/// UNVALIDATED. The pre-E-09 behaviour: ranks whatever it is given, including
/// physically impossible inputs, and returns a plausible-looking answer for them.
///
/// Public ONLY so off-chain analysis tools (`tools/differential`) can keep probing
/// misuse without catching a panic. **Never call this on a path that moves funds** --
/// call [`evaluate_hand`] or [`try_evaluate_hand`].
pub fn evaluate_hand_unchecked(hole_cards: &(Card, Card), community: &[Card]) -> HandRank {
    let mut all_cards: Vec<Card> = Vec::with_capacity(7);
    all_cards.push(hole_cards.0);
    all_cards.push(hole_cards.1);
    all_cards.extend_from_slice(community);

    // Generate all 5-card combinations and find the best
    let mut best_rank: Option<HandRank> = None;

    for combo in combinations(&all_cards, HAND_SIZE) {
        let rank = evaluate_five_cards_unchecked(&combo);
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

/// Ranks exactly five distinct cards.
///
/// # Panics
///
/// Traps (panics, which is an IC trap that rolls the message back) unless given
/// exactly five DISTINCT cards. Read the block comment above [`HandInputError`]
/// for why a trap and not a `Result` here. Use [`try_evaluate_five_cards`] if you
/// want to handle the rejection yourself.
pub fn evaluate_five_cards(cards: &[Card]) -> HandRank {
    try_evaluate_five_cards(cards).unwrap_or_else(|e| panic!("{}", e))
}

/// [`evaluate_five_cards`] with the rejection returned instead of trapping.
pub fn try_evaluate_five_cards(cards: &[Card]) -> Result<HandRank, HandInputError> {
    validate_five_cards(cards)?;
    Ok(evaluate_five_cards_unchecked(cards))
}

/// UNVALIDATED. The pre-E-09 behaviour: with more than five cards it tests the
/// flush on one suit and the straight over ALL the ranks independently, so it
/// invents straight flushes that are in none of the cards; with duplicates it
/// ranks impossible hands.
///
/// Public ONLY so off-chain analysis tools (`tools/differential`) can keep probing
/// misuse without catching a panic. **Never call this on a path that moves funds** --
/// call [`evaluate_five_cards`] or [`try_evaluate_five_cards`].
pub fn evaluate_five_cards_unchecked(cards: &[Card]) -> HandRank {
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

// =============================================================================
// TESTS -- the E-09 boundary. Four of these are the exact inputs docs/DEFECTS.md
// E-09 tabulates; each one used to return a confident, plausible, WRONG answer.
// =============================================================================

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::card::{Rank, Suit};

    /// `"Ah Kd Qc"` -> three cards. Same notation the defect register uses.
    fn cards(spec: &str) -> Vec<Card> {
        spec.split_whitespace()
            .map(|s| {
                let b = s.as_bytes();
                assert_eq!(b.len(), 2, "card spec must be rank+suit, got {:?}", s);
                let rank = match b[0] {
                    b'2' => Rank::Two,
                    b'3' => Rank::Three,
                    b'4' => Rank::Four,
                    b'5' => Rank::Five,
                    b'6' => Rank::Six,
                    b'7' => Rank::Seven,
                    b'8' => Rank::Eight,
                    b'9' => Rank::Nine,
                    b'T' => Rank::Ten,
                    b'J' => Rank::Jack,
                    b'Q' => Rank::Queen,
                    b'K' => Rank::King,
                    b'A' => Rank::Ace,
                    other => panic!("bad rank {:?}", other as char),
                };
                let suit = match b[1] {
                    b'h' => Suit::Hearts,
                    b'd' => Suit::Diamonds,
                    b'c' => Suit::Clubs,
                    b's' => Suit::Spades,
                    other => panic!("bad suit {:?}", other as char),
                };
                Card { suit, rank }
            })
            .collect()
    }

    fn hole_and_board(spec: &str) -> ((Card, Card), Vec<Card>) {
        let c = cards(spec);
        assert!(c.len() >= 2);
        ((c[0], c[1]), c[2..].to_vec())
    }

    // ---- the four E-09 manifestations -------------------------------------

    /// E-09 row 1. Hole cards Ah + Ah with a Kh Qh Jh board is FOUR physical
    /// hearts, and the engine used to report a five-card flush counting the ace
    /// of hearts twice.
    #[test]
    fn a_duplicated_hole_card_is_refused_instead_of_becoming_a_flush() {
        let (hole, board) = hole_and_board("Ah Ah Kh Qh Jh 2c 3d");
        assert_eq!(
            try_evaluate_hand(&hole, &board),
            Err(HandInputError::DuplicateCard {
                card: Card { suit: Suit::Hearts, rank: Rank::Ace },
                first: 0,
                second: 1,
            })
        );
        // What it used to answer, and would answer again if the check were removed.
        assert_eq!(
            evaluate_hand_unchecked(&hole, &board),
            HandRank::Flush(vec![14, 14, 13, 12, 11]),
            "the unchecked path must still reproduce the defect for the harness"
        );
    }

    #[test]
    #[should_panic(expected = "IMPOSSIBLE HAND")]
    fn a_duplicated_hole_card_traps_on_the_money_path() {
        let (hole, board) = hole_and_board("Ah Ah Kh Qh Jh 2c 3d");
        let _ = evaluate_hand(&hole, &board);
    }

    /// E-09 row 1 variant: a hole card that also lies on the board.
    #[test]
    fn a_hole_card_repeated_on_the_board_is_refused() {
        let (hole, board) = hole_and_board("Ah Kh Ah 2c 3d 4s 5h");
        assert!(matches!(
            try_evaluate_hand(&hole, &board),
            Err(HandInputError::DuplicateCard { first: 0, second: 2, .. })
        ));
        assert_eq!(
            evaluate_hand_unchecked(&hole, &board),
            HandRank::Straight(5),
            "unchecked must still reproduce the fabricated wheel"
        );
    }

    /// E-09 row 2. Five copies of the ace of hearts is not a flush.
    #[test]
    fn five_identical_aces_are_refused() {
        let c = cards("Ah Ah Ah Ah Ah");
        assert!(matches!(
            try_evaluate_five_cards(&c),
            Err(HandInputError::DuplicateCard { .. })
        ));
        assert_eq!(
            evaluate_five_cards_unchecked(&c),
            HandRank::Flush(vec![14, 14, 14, 14, 14])
        );
    }

    #[test]
    #[should_panic(expected = "IMPOSSIBLE HAND")]
    fn five_identical_aces_trap_on_the_money_path() {
        let _ = evaluate_five_cards(&cards("Ah Ah Ah Ah Ah"));
    }

    /// E-09 row 3. `2h 3h 4h 5h 9h 6c 7c` holds a 9-high heart flush and a
    /// 7-high offsuit straight. There is NO straight flush in it, yet
    /// `evaluate_five_cards` used to report `StraightFlush(7)` because it tested
    /// the flush on one suit and the straight over all seven ranks separately.
    #[test]
    fn more_than_five_cards_is_refused_instead_of_fabricating_a_straight_flush() {
        for spec in ["2h 3h 4h 5h 9h 6c", "2h 3h 4h 5h 9h 6c 7c"] {
            let c = cards(spec);
            assert_eq!(
                try_evaluate_five_cards(&c),
                Err(HandInputError::NotFiveCards { got: c.len() }),
                "{spec}"
            );
        }
        assert_eq!(
            evaluate_five_cards_unchecked(&cards("2h 3h 4h 5h 9h 6c 7c")),
            HandRank::StraightFlush(7),
            "the unchecked path must still fabricate it, or the finding is stale"
        );
    }

    #[test]
    #[should_panic(expected = "IMPOSSIBLE HAND")]
    fn seven_cards_into_evaluate_five_cards_traps() {
        let _ = evaluate_five_cards(&cards("2h 3h 4h 5h 9h 6c 7c"));
    }

    #[test]
    fn fewer_than_five_cards_is_refused() {
        for spec in ["Ah Kd", "Ah Kd Qc", "Ah Kd Qc Js"] {
            let c = cards(spec);
            assert_eq!(
                try_evaluate_five_cards(&c),
                Err(HandInputError::NotFiveCards { got: c.len() }),
                "{spec}"
            );
        }
    }

    /// E-09 row 4, and the worst of the four: `HighCard([])` compares EQUAL
    /// between any two players, so a short board used to make the whole table
    /// chop instead of trapping.
    #[test]
    fn a_short_board_is_refused_instead_of_making_every_player_chop() {
        for spec in ["Ah Kd", "Ah Kd Qc", "Ah Kd Qc Js"] {
            let (hole, board) = hole_and_board(spec);
            assert_eq!(
                try_evaluate_hand(&hole, &board),
                Err(HandInputError::BadBoardLength {
                    community: board.len(),
                    total: 2 + board.len(),
                }),
                "{spec}"
            );
        }

        // The consequence being prevented, spelled out: two different hands used
        // to compare EQUAL, so both players were paid as winners.
        let (h1, b1) = hole_and_board("Ah Kd Qc");
        let (h2, b2) = hole_and_board("2h 3d Qc");
        assert_eq!(
            evaluate_hand_unchecked(&h1, &b1),
            evaluate_hand_unchecked(&h2, &b2),
            "unchecked: AK and 32 still chop on a short board"
        );
    }

    #[test]
    #[should_panic(expected = "IMPOSSIBLE HAND")]
    fn a_short_board_traps_on_the_money_path() {
        let (hole, board) = hole_and_board("Ah Kd");
        let _ = evaluate_hand(&hole, &board);
    }

    #[test]
    fn a_board_longer_than_the_river_is_refused() {
        let (hole, board) = hole_and_board("Ah Kd Qc Js Ts 9s 8s 7s");
        assert_eq!(
            try_evaluate_hand(&hole, &board),
            Err(HandInputError::BadBoardLength { community: 6, total: 8 })
        );
    }

    // ---- the legal inputs must be untouched -------------------------------

    #[test]
    fn every_legal_board_length_still_evaluates() {
        // flop, turn, river: 5, 6 and 7 cards in total.
        let (hole, flop) = hole_and_board("As Ks Qs Js Ts");
        assert_eq!(try_evaluate_hand(&hole, &flop), Ok(HandRank::RoyalFlush));
        let (hole, turn) = hole_and_board("As Ks Qs Js Ts 2c");
        assert_eq!(try_evaluate_hand(&hole, &turn), Ok(HandRank::RoyalFlush));
        let (hole, river) = hole_and_board("As Ks Qs Js Ts 2c 3d");
        assert_eq!(try_evaluate_hand(&hole, &river), Ok(HandRank::RoyalFlush));
        assert_eq!(evaluate_hand(&hole, &river), HandRank::RoyalFlush);
    }

    #[test]
    fn a_legal_five_card_hand_still_evaluates() {
        assert_eq!(
            try_evaluate_five_cards(&cards("As Ks Qs Js Ts")),
            Ok(HandRank::RoyalFlush)
        );
        assert_eq!(
            evaluate_five_cards(&cards("2h 7d 9c Js Ah")),
            HandRank::HighCard(vec![14, 11, 9, 7, 2])
        );
    }

    /// The whole 52-card deck is distinct, and every 5-subset of a real deal is
    /// accepted. Cheap proof that the check cannot reject a legal hand.
    #[test]
    fn a_real_deck_and_every_real_deal_out_of_it_validates() {
        let deck = crate::card::create_deck();
        assert_eq!(deck.len(), 52);
        assert_eq!(validate_distinct(&deck), Ok(()));
        for window in deck.windows(7) {
            let hole = (window[0], window[1]);
            assert_eq!(validate_hand_input(&hole, &window[2..7]), Ok(()));
            assert!(try_evaluate_hand(&hole, &window[2..7]).is_ok());
        }
    }

    #[test]
    fn validate_distinct_reports_the_first_duplicate_pair() {
        let c = cards("2h 3h 4h 3h 5h");
        assert_eq!(
            validate_distinct(&c),
            Err(HandInputError::DuplicateCard {
                card: Card { suit: Suit::Hearts, rank: Rank::Three },
                first: 1,
                second: 3,
            })
        );
    }

    /// The rejection has to be readable in a canister reject message: that string
    /// is the ONLY diagnostic anyone gets from a trapped update call.
    #[test]
    fn the_rejection_message_names_the_impossible_card() {
        let e = try_evaluate_five_cards(&cards("Ah Ah Ah Ah Ah")).unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("IMPOSSIBLE HAND"), "{msg}");
        assert!(msg.contains("Ace"), "{msg}");
        assert!(msg.contains("Hearts"), "{msg}");
    }
}
