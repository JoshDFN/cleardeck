//! Cards, ranks, suits and deck construction.
//!
//! MOVED VERBATIM out of `src/table_canister/src/lib.rs` (commit ceacc37).
//! These types are on the Candid wire AND in the stable-memory serialisation of
//! a canister that custodies real ICP/ckBTC. Every derive, field name, variant
//! name and variant ORDER here is load-bearing: changing any of them silently
//! changes the on-wire encoding. Do not reorder. Do not add or remove derives.

use candid::{CandidType, Deserialize};

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    pub fn value(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

/// Creates a standard 52-card deck in a fixed order (Hearts, Diamonds, Clubs, Spades)
/// Each suit contains cards 2-A in ascending order
pub fn create_deck() -> Vec<Card> {
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
