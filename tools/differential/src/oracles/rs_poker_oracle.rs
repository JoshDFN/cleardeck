//! Reference A: `rs_poker` 5.0.0 (Apache-2.0, elliottneilclark/rs-poker).
//!
//! Lineage: its own perfect-hash evaluator whose tables are generated from
//! scratch in the crate's `build.rs`. It ranks 5-, 6- and 7-card hands with a
//! single table lookup, so `eval7` here is a genuinely independent
//! implementation of "best five of seven" rather than a re-run of ClearDeck's
//! own 21-subset loop.
//!
//! Strength is the evaluator's packed `u16` score `(category << 12) | subrank`,
//! wrapped in `rs_poker::core::Rank`, which derives `Ord` over that `u16`.

use rs_poker::core::{Card, CoreRank, Rank, Rankable};

use crate::cards::{to_rs_poker, CardIdx};
use crate::category::category_of_rs_poker;

use super::{Oracle, OracleOut};

pub struct RsPokerOracle;

impl Oracle for RsPokerOracle {
    type Strength = Rank;

    fn name(&self) -> &'static str {
        "rs_poker"
    }

    fn provenance(&self) -> &'static str {
        "rs_poker 5.0.0 (own perfect-hash tables generated in build.rs)"
    }

    fn eval5(&self, cards: &[CardIdx; 5]) -> OracleOut<Rank> {
        let hand: Vec<Card> = cards.iter().map(|&c| to_rs_poker(c)).collect();
        let rank = hand.rank();
        OracleOut {
            strength: rank,
            category: category_of_rs_poker(rank.category()),
            // rs_poker deliberately collapses kicker detail into the subrank and
            // exposes no accessor for it, so this oracle abstains from the
            // sub-rank detail check.
            detail: None,
        }
    }

    fn eval7(&self, cards: &[CardIdx; 7]) -> OracleOut<Rank> {
        let hand: Vec<Card> = cards.iter().map(|&c| to_rs_poker(c)).collect();
        let rank = hand.rank();
        OracleOut {
            strength: rank,
            category: category_of_rs_poker(rank.category()),
            detail: None,
        }
    }
}

impl RsPokerOracle {
    /// Whether this hand is the *ace-high* straight flush, used to check
    /// ClearDeck's extra `HandRank::RoyalFlush` variant is exactly that set.
    /// `Rank::STRAIGHT_FLUSH_MIN` is the 5-high (wheel) straight flush and the
    /// ace-high one is the single largest score in the whole space.
    pub fn is_royal_flush(rank: Rank) -> bool {
        rank.category() == CoreRank::StraightFlush && rank == Self::best_rank()
    }

    /// The single highest score the reference can produce: an ace-high straight
    /// flush. Computed, not hard-coded, so a table change in the reference
    /// cannot silently invalidate the check.
    pub fn best_rank() -> Rank {
        let royal: Vec<Card> = ["As", "Ks", "Qs", "Js", "Ts"]
            .iter()
            .map(|s| to_rs_poker(crate::cards::from_str(s).expect("literal card")))
            .collect();
        royal.rank()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::parse_hand;
    use crate::category::Category;

    fn five(s: &str) -> [CardIdx; 5] {
        let v = parse_hand(s).unwrap();
        [v[0], v[1], v[2], v[3], v[4]]
    }

    #[test]
    fn known_hands_land_in_the_right_category() {
        let o = RsPokerOracle;
        assert_eq!(o.eval5(&five("As Ks Qs Js Ts")).category, Category::StraightFlush);
        assert_eq!(o.eval5(&five("5h 4h 3h 2h Ah")).category, Category::StraightFlush);
        assert_eq!(o.eval5(&five("7c 7d 7h 7s 2c")).category, Category::FourOfAKind);
        assert_eq!(o.eval5(&five("5h 4d 3c 2s Ah")).category, Category::Straight);
        assert_eq!(o.eval5(&five("Ac Kd Qh Js 9c")).category, Category::HighCard);
    }

    #[test]
    fn royal_flush_is_the_unique_maximum() {
        let o = RsPokerOracle;
        let royal = o.eval5(&five("As Ks Qs Js Ts")).strength;
        let king_high_sf = o.eval5(&five("Ks Qs Js Ts 9s")).strength;
        assert!(RsPokerOracle::is_royal_flush(royal));
        assert!(!RsPokerOracle::is_royal_flush(king_high_sf));
        assert!(royal > king_high_sf);
    }

    #[test]
    fn the_wheel_is_the_weakest_straight() {
        let o = RsPokerOracle;
        assert!(o.eval5(&five("6h 5d 4c 3s 2h")).strength > o.eval5(&five("5h 4d 3c 2s Ah")).strength);
    }
}
