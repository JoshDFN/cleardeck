//! Card notation, so a disagreement reads like a hand history instead of a
//! Candid dump.
//!
//! `Ah Kd 7c` -- rank letter then suit letter, the notation every poker tool
//! uses. A reproducer that says `seat 2 held Ah Ac on Ks Kd 7h 3c 2s` can be
//! checked by a human against the rules of poker; `Card { suit: Hearts, rank: Ace }`
//! repeated fourteen times cannot.

use poker_core::{Card, Rank, Suit};

pub fn suit_char(s: Suit) -> char {
    match s {
        Suit::Hearts => 'h',
        Suit::Diamonds => 'd',
        Suit::Clubs => 'c',
        Suit::Spades => 's',
    }
}

pub fn rank_char(r: Rank) -> char {
    match r {
        Rank::Two => '2',
        Rank::Three => '3',
        Rank::Four => '4',
        Rank::Five => '5',
        Rank::Six => '6',
        Rank::Seven => '7',
        Rank::Eight => '8',
        Rank::Nine => '9',
        Rank::Ten => 'T',
        Rank::Jack => 'J',
        Rank::Queen => 'Q',
        Rank::King => 'K',
        Rank::Ace => 'A',
    }
}

pub fn card_str(c: &Card) -> String {
    format!("{}{}", rank_char(c.rank), suit_char(c.suit))
}

pub fn cards_str(cards: &[Card]) -> String {
    cards.iter().map(card_str).collect::<Vec<_>>().join(" ")
}

pub fn hole_str(hole: &Option<(Card, Card)>) -> String {
    match hole {
        Some((a, b)) => format!("{} {}", card_str(a), card_str(b)),
        None => "-- --".to_string(),
    }
}

/// Parse `"Ah"` into a card. Used by the hand-written oracle rule tests, where
/// stating the cards in notation is the whole point.
pub fn card(text: &str) -> Card {
    let mut chars = text.chars();
    let r = chars.next().unwrap_or_else(|| panic!("empty card {text:?}"));
    let s = chars
        .next()
        .unwrap_or_else(|| panic!("card {text:?} has no suit"));
    assert!(
        chars.next().is_none(),
        "card {text:?} has trailing characters"
    );
    let rank = match r {
        '2' => Rank::Two,
        '3' => Rank::Three,
        '4' => Rank::Four,
        '5' => Rank::Five,
        '6' => Rank::Six,
        '7' => Rank::Seven,
        '8' => Rank::Eight,
        '9' => Rank::Nine,
        'T' | 't' => Rank::Ten,
        'J' | 'j' => Rank::Jack,
        'Q' | 'q' => Rank::Queen,
        'K' | 'k' => Rank::King,
        'A' | 'a' => Rank::Ace,
        other => panic!("bad rank {other:?} in {text:?}"),
    };
    let suit = match s {
        'h' | 'H' => Suit::Hearts,
        'd' | 'D' => Suit::Diamonds,
        'c' | 'C' => Suit::Clubs,
        's' | 'S' => Suit::Spades,
        other => panic!("bad suit {other:?} in {text:?}"),
    };
    Card { suit, rank }
}

/// Parse `"Ks Kd 7h 3c 2s"` into a board.
pub fn board(text: &str) -> Vec<Card> {
    text.split_whitespace().map(card).collect()
}

/// Parse `"Ah Ac"` into hole cards.
pub fn hole(text: &str) -> (Card, Card) {
    let cards = board(text);
    assert_eq!(cards.len(), 2, "hole cards must be exactly two: {text:?}");
    (cards[0], cards[1])
}
