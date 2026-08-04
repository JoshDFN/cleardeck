//! The explicit cross-library category mapping.
//!
//! The three evaluators name hand classes differently and slice them
//! differently, so nothing here is inferred from an integer:
//!
//! | ClearDeck `HandRank`        | `rs_poker::CoreRank` | `poker::FiveCardHandClass` | harness [`Category`] |
//! |-----------------------------|----------------------|----------------------------|----------------------|
//! | `HighCard(Vec<u8>)`         | `HighCard`           | `HighCard`                 | `HighCard`           |
//! | `Pair(u8, Vec<u8>)`         | `OnePair`            | `Pair`                     | `OnePair`            |
//! | `TwoPair(u8, u8, u8)`       | `TwoPair`            | `TwoPair`                  | `TwoPair`            |
//! | `ThreeOfAKind(u8, Vec<u8>)` | `ThreeOfAKind`       | `ThreeOfAKind`             | `ThreeOfAKind`       |
//! | `Straight(u8)`              | `Straight`           | `Straight`                 | `Straight`           |
//! | `Flush(Vec<u8>)`            | `Flush`              | `Flush`                    | `Flush`              |
//! | `FullHouse(u8, u8)`         | `FullHouse`          | `FullHouse`                | `FullHouse`          |
//! | `FourOfAKind(u8, u8)`       | `FourOfAKind`        | `FourOfAKind`              | `FourOfAKind`        |
//! | `StraightFlush(u8)`         | `StraightFlush`      | `StraightFlush`            | `StraightFlush`      |
//! | `RoyalFlush`                | `StraightFlush`      | `StraightFlush{Ace}`       | `StraightFlush`      |
//!
//! The last row is the one real *structural* difference: ClearDeck promotes the
//! ace-high straight flush to its own `HandRank` variant, while both references
//! treat it as the top straight flush. That is a naming difference, not a bug,
//! but it is only harmless if `RoyalFlush` fires on **exactly** the ace-high
//! straight flushes, which is checked separately (see
//! `checks::exhaustive::ROYAL_FLUSH_ALIAS`).

use poker_core::HandRank;
use serde::Serialize;

use crate::cards::poker_rank_face_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Category {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::HighCard => "HighCard",
            Category::OnePair => "OnePair",
            Category::TwoPair => "TwoPair",
            Category::ThreeOfAKind => "ThreeOfAKind",
            Category::Straight => "Straight",
            Category::Flush => "Flush",
            Category::FullHouse => "FullHouse",
            Category::FourOfAKind => "FourOfAKind",
            Category::StraightFlush => "StraightFlush",
        }
    }
}

/// ClearDeck's `HandRank` -> harness category.
pub fn category_of_cleardeck(r: &HandRank) -> Category {
    match r {
        HandRank::HighCard(_) => Category::HighCard,
        HandRank::Pair(_, _) => Category::OnePair,
        HandRank::TwoPair(_, _, _) => Category::TwoPair,
        HandRank::ThreeOfAKind(_, _) => Category::ThreeOfAKind,
        HandRank::Straight(_) => Category::Straight,
        HandRank::Flush(_) => Category::Flush,
        HandRank::FullHouse(_, _) => Category::FullHouse,
        HandRank::FourOfAKind(_, _) => Category::FourOfAKind,
        HandRank::StraightFlush(_) => Category::StraightFlush,
        HandRank::RoyalFlush => Category::StraightFlush,
    }
}

/// `rs_poker`'s `CoreRank` -> harness category.
pub fn category_of_rs_poker(c: rs_poker::core::CoreRank) -> Category {
    use rs_poker::core::CoreRank;
    match c {
        CoreRank::HighCard => Category::HighCard,
        CoreRank::OnePair => Category::OnePair,
        CoreRank::TwoPair => Category::TwoPair,
        CoreRank::ThreeOfAKind => Category::ThreeOfAKind,
        CoreRank::Straight => Category::Straight,
        CoreRank::Flush => Category::Flush,
        CoreRank::FullHouse => Category::FullHouse,
        CoreRank::FourOfAKind => Category::FourOfAKind,
        CoreRank::StraightFlush => Category::StraightFlush,
    }
}

/// `poker`'s `FiveCardHandClass` -> harness category.
pub fn category_of_poker_crate(c: poker::evaluate::FiveCardHandClass) -> Category {
    use poker::evaluate::FiveCardHandClass as F;
    match c {
        F::HighCard { .. } => Category::HighCard,
        F::Pair { .. } => Category::OnePair,
        F::TwoPair { .. } => Category::TwoPair,
        F::ThreeOfAKind { .. } => Category::ThreeOfAKind,
        F::Straight { .. } => Category::Straight,
        F::Flush { .. } => Category::Flush,
        F::FullHouse { .. } => Category::FullHouse,
        F::FourOfAKind { .. } => Category::FourOfAKind,
        F::StraightFlush { .. } => Category::StraightFlush,
    }
}

/// The *sub-rank detail* both ClearDeck and `poker` can express, normalised to
/// 2..14 face values. `None` fields mean "this library does not report it".
///
/// Comparing this catches a class of bug that a category-only check misses
/// entirely: naming the right category but the wrong rank inside it (e.g.
/// crediting a full house to the pair instead of the trips).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Detail {
    pub category: Category,
    /// Primary rank: high card, pair rank, trips rank, quad rank, straight high,
    /// flush high card.
    pub primary: u8,
    /// Secondary rank: the low pair of a two pair, the pair of a full house.
    pub secondary: Option<u8>,
}

pub fn detail_of_cleardeck(r: &HandRank) -> Detail {
    match r {
        HandRank::HighCard(v) => Detail {
            category: Category::HighCard,
            primary: v.first().copied().unwrap_or(0),
            secondary: None,
        },
        HandRank::Pair(p, _) => Detail {
            category: Category::OnePair,
            primary: *p,
            secondary: None,
        },
        HandRank::TwoPair(hi, lo, _) => Detail {
            category: Category::TwoPair,
            primary: *hi,
            secondary: Some(*lo),
        },
        HandRank::ThreeOfAKind(t, _) => Detail {
            category: Category::ThreeOfAKind,
            primary: *t,
            secondary: None,
        },
        HandRank::Straight(h) => Detail {
            category: Category::Straight,
            primary: *h,
            secondary: None,
        },
        HandRank::Flush(v) => Detail {
            category: Category::Flush,
            primary: v.first().copied().unwrap_or(0),
            secondary: None,
        },
        HandRank::FullHouse(t, p) => Detail {
            category: Category::FullHouse,
            primary: *t,
            secondary: Some(*p),
        },
        HandRank::FourOfAKind(q, _) => Detail {
            category: Category::FourOfAKind,
            primary: *q,
            secondary: None,
        },
        HandRank::StraightFlush(h) => Detail {
            category: Category::StraightFlush,
            primary: *h,
            secondary: None,
        },
        HandRank::RoyalFlush => Detail {
            category: Category::StraightFlush,
            primary: 14,
            secondary: None,
        },
    }
}

pub fn detail_of_poker_crate(c: poker::evaluate::FiveCardHandClass) -> Detail {
    use poker::evaluate::FiveCardHandClass as F;
    let fv = poker_rank_face_value;
    match c {
        F::HighCard { rank } => Detail {
            category: Category::HighCard,
            primary: fv(rank),
            secondary: None,
        },
        F::Pair { rank } => Detail {
            category: Category::OnePair,
            primary: fv(rank),
            secondary: None,
        },
        F::TwoPair {
            high_rank,
            low_rank,
        } => Detail {
            category: Category::TwoPair,
            primary: fv(high_rank),
            secondary: Some(fv(low_rank)),
        },
        F::ThreeOfAKind { rank } => Detail {
            category: Category::ThreeOfAKind,
            primary: fv(rank),
            secondary: None,
        },
        F::Straight { rank } => Detail {
            category: Category::Straight,
            primary: fv(rank),
            secondary: None,
        },
        F::Flush { rank } => Detail {
            category: Category::Flush,
            primary: fv(rank),
            secondary: None,
        },
        F::FullHouse { trips, pair } => Detail {
            category: Category::FullHouse,
            primary: fv(trips),
            secondary: Some(fv(pair)),
        },
        F::FourOfAKind { rank } => Detail {
            category: Category::FourOfAKind,
            primary: fv(rank),
            secondary: None,
        },
        F::StraightFlush { rank } => Detail {
            category: Category::StraightFlush,
            primary: fv(rank),
            secondary: None,
        },
    }
}
