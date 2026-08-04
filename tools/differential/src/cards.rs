//! One canonical card encoding, and lossless conversions into every evaluator.
//!
//! Everything in the harness speaks `CardIdx`: a `u8` in `0..52` equal to
//! `rank_index * 4 + suit_index`, where `rank_index` is `0` for a deuce through
//! `12` for an ace and `suit_index` is `0..4` for clubs/diamonds/hearts/spades.
//!
//! The three evaluators disagree about *everything* representational: suit order,
//! whether a deuce is `2` or `0`, whether an enum carries explicit discriminants.
//! Because of that, the conversions here are written out longhand rather than cast,
//! and `tests/fast_subset.rs` proves each one is a bijection.

use poker_core::{Card as CdCard, Rank as CdRank, Suit as CdSuit};

/// A card as `rank_index * 4 + suit_index`, `0..52`.
pub type CardIdx = u8;

pub const NUM_CARDS: usize = 52;
pub const RANK_CHARS: [char; 13] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A',
];
pub const SUIT_CHARS: [char; 4] = ['c', 'd', 'h', 's'];

/// `0` for a deuce .. `12` for an ace.
#[inline]
pub fn rank_index(c: CardIdx) -> u8 {
    c / 4
}

/// `0..4` for clubs/diamonds/hearts/spades.
#[inline]
pub fn suit_index(c: CardIdx) -> u8 {
    c % 4
}

/// The rank as poker's usual 2..14 face value, which is what `poker_core`'s
/// `HandRank` payloads carry.
#[inline]
pub fn face_value(c: CardIdx) -> u8 {
    rank_index(c) + 2
}

/// `"Ah"`, `"Td"`, `"2c"`, the notation every reference library also accepts,
/// so a repro case can be pasted straight into any of them.
pub fn to_str(c: CardIdx) -> String {
    let mut s = String::with_capacity(2);
    s.push(RANK_CHARS[rank_index(c) as usize]);
    s.push(SUIT_CHARS[suit_index(c) as usize]);
    s
}

pub fn slice_to_string(cards: &[CardIdx]) -> String {
    cards
        .iter()
        .map(|&c| to_str(c))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses `"Ah"` / `"ah"` back to a `CardIdx`. Used by the regression cases so
/// they read as poker rather than as integers.
pub fn from_str(s: &str) -> Result<CardIdx, String> {
    let chars: Vec<char> = s.trim().chars().collect();
    if chars.len() != 2 {
        return Err(format!("card {s:?} must be exactly 2 characters"));
    }
    let r = chars[0].to_ascii_uppercase();
    let su = chars[1].to_ascii_lowercase();
    let ri = RANK_CHARS
        .iter()
        .position(|&c| c == r)
        .ok_or_else(|| format!("bad rank {r:?} in card {s:?}"))?;
    let si = SUIT_CHARS
        .iter()
        .position(|&c| c == su)
        .ok_or_else(|| format!("bad suit {su:?} in card {s:?}"))?;
    Ok((ri * 4 + si) as CardIdx)
}

pub fn parse_hand(s: &str) -> Result<Vec<CardIdx>, String> {
    s.split_whitespace().map(from_str).collect()
}

// ---------------------------------------------------------------------------
// ClearDeck `poker_core`
// ---------------------------------------------------------------------------

#[inline]
pub fn to_cleardeck(c: CardIdx) -> CdCard {
    CdCard {
        suit: match suit_index(c) {
            0 => CdSuit::Clubs,
            1 => CdSuit::Diamonds,
            2 => CdSuit::Hearts,
            _ => CdSuit::Spades,
        },
        rank: match rank_index(c) {
            0 => CdRank::Two,
            1 => CdRank::Three,
            2 => CdRank::Four,
            3 => CdRank::Five,
            4 => CdRank::Six,
            5 => CdRank::Seven,
            6 => CdRank::Eight,
            7 => CdRank::Nine,
            8 => CdRank::Ten,
            9 => CdRank::Jack,
            10 => CdRank::Queen,
            11 => CdRank::King,
            _ => CdRank::Ace,
        },
    }
}

/// Inverse of [`to_cleardeck`]; used only to prove the conversion is a bijection.
pub fn from_cleardeck(c: &CdCard) -> CardIdx {
    let si = match c.suit {
        CdSuit::Clubs => 0u8,
        CdSuit::Diamonds => 1,
        CdSuit::Hearts => 2,
        CdSuit::Spades => 3,
    };
    let ri = c.rank.value() - 2;
    ri * 4 + si
}

// ---------------------------------------------------------------------------
// Reference A: rs_poker
// ---------------------------------------------------------------------------

#[inline]
pub fn to_rs_poker(c: CardIdx) -> rs_poker::core::Card {
    use rs_poker::core::{Suit, Value};
    let suit = match suit_index(c) {
        0 => Suit::Club,
        1 => Suit::Diamond,
        2 => Suit::Heart,
        _ => Suit::Spade,
    };
    let value = match rank_index(c) {
        0 => Value::Two,
        1 => Value::Three,
        2 => Value::Four,
        3 => Value::Five,
        4 => Value::Six,
        5 => Value::Seven,
        6 => Value::Eight,
        7 => Value::Nine,
        8 => Value::Ten,
        9 => Value::Jack,
        10 => Value::Queen,
        11 => Value::King,
        _ => Value::Ace,
    };
    rs_poker::core::Card::new(value, suit)
}

// ---------------------------------------------------------------------------
// Reference B: poker
// ---------------------------------------------------------------------------

#[inline]
pub fn to_poker_crate(c: CardIdx) -> poker::Card {
    use poker::{Rank, Suit};
    let suit = match suit_index(c) {
        0 => Suit::Clubs,
        1 => Suit::Diamonds,
        2 => Suit::Hearts,
        _ => Suit::Spades,
    };
    let rank = match rank_index(c) {
        0 => Rank::Two,
        1 => Rank::Three,
        2 => Rank::Four,
        3 => Rank::Five,
        4 => Rank::Six,
        5 => Rank::Seven,
        6 => Rank::Eight,
        7 => Rank::Nine,
        8 => Rank::Ten,
        9 => Rank::Jack,
        10 => Rank::Queen,
        11 => Rank::King,
        _ => Rank::Ace,
    };
    poker::Card::new(rank, suit)
}

/// `poker`'s `Rank` back to a 2..14 face value, for comparing our `HandRank`
/// payload numbers against `Eval::classify()`'s detail.
pub fn poker_rank_face_value(r: poker::Rank) -> u8 {
    use poker::Rank;
    match r {
        Rank::Two => 2,
        Rank::Three => 3,
        Rank::Four => 4,
        Rank::Five => 5,
        Rank::Six => 6,
        Rank::Seven => 7,
        Rank::Eight => 8,
        Rank::Nine => 9,
        Rank::Ten => 10,
        Rank::Jack => 11,
        Rank::Queen => 12,
        Rank::King => 13,
        Rank::Ace => 14,
    }
}

/// Packs up to 5 cards into a `u32` (6 bits each) so the exhaustive sweep can
/// hold one row per hand in ~12 bytes instead of a heap `Vec`.
#[inline]
pub fn pack5(cards: &[CardIdx; 5]) -> u32 {
    let mut v = 0u32;
    for (i, &c) in cards.iter().enumerate() {
        v |= (c as u32) << (6 * i);
    }
    v
}

#[inline]
pub fn unpack5(v: u32) -> [CardIdx; 5] {
    let mut out = [0u8; 5];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = ((v >> (6 * i)) & 0x3f) as u8;
    }
    out
}
