//! Reference B: `poker` 0.7.0 (MIT, deus-x-mackina/poker).
//!
//! Lineage: a Rust port of the `treys` Python package: Cactus Kev's prime-product
//! trick plus Senzee's perfect hash, a completely different table construction
//! from reference A. It only evaluates exactly five cards, so `eval7` takes the
//! best of the 21 subsets, using the library's own `Ord` to pick the maximum.
//!
//! Unlike reference A this one *does* expose sub-rank detail through
//! `Eval::classify()`, which is what makes the "right category, wrong rank
//! inside it" check possible.

use std::cmp::Ordering;

use poker::{Card, Eval, Evaluator, FiveCard};

use crate::cards::{to_poker_crate, CardIdx};
use crate::category::{category_of_poker_crate, detail_of_poker_crate};

use super::{Oracle, OracleOut, SUBSETS_7C5};

/// `poker::Eval<FiveCard>` derives `PartialEq` over its private `hand_rank`
/// **and** `rank_flags` fields, while its hand-written `Ord` compares only
/// `hand_rank`. Using the derived `PartialEq` alongside that `Ord` would give an
/// `Eq`/`Ord` pair that disagree about equality, which would silently corrupt
/// the equivalence-class analysis. This newtype forces both onto the library's
/// documented comparison (`is_equal_to` / `Ord`, both `hand_rank`-only).
#[derive(Clone, Copy, Debug)]
pub struct PokerStrength(pub Eval<FiveCard>);

impl PartialEq for PokerStrength {
    fn eq(&self, other: &Self) -> bool {
        self.0.is_equal_to(other.0)
    }
}

impl Eq for PokerStrength {}

impl Ord for PokerStrength {
    fn cmp(&self, other: &Self) -> Ordering {
        // The library's `Ord` is "greater is a better hand".
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for PokerStrength {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct PokerCrateOracle {
    eval: Evaluator,
}

impl Default for PokerCrateOracle {
    fn default() -> Self {
        Self::new()
    }
}

impl PokerCrateOracle {
    pub fn new() -> Self {
        Self {
            eval: Evaluator::new(),
        }
    }

    fn eval_five_raw(&self, cards: &[Card; 5]) -> Eval<FiveCard> {
        // `evaluate_five` only errors on a wrong card count or a duplicate card.
        // Both are structurally impossible here (fixed-size array of distinct
        // cards from the sweep / the RNG's Fisher-Yates deal), so a failure is a
        // harness bug and must be loud rather than swallowed.
        self.eval
            .evaluate_five(cards.as_slice())
            .unwrap_or_else(|e| panic!("poker crate rejected {cards:?}: {e}"))
    }
}

impl Oracle for PokerCrateOracle {
    type Strength = PokerStrength;

    fn name(&self) -> &'static str {
        "poker"
    }

    fn provenance(&self) -> &'static str {
        "poker 0.7.0 (Rust port of the treys Cactus-Kev/Senzee lookup tables)"
    }

    fn eval5(&self, cards: &[CardIdx; 5]) -> OracleOut<PokerStrength> {
        let converted: [Card; 5] = std::array::from_fn(|i| to_poker_crate(cards[i]));
        let e = self.eval_five_raw(&converted);
        let class = e.classify();
        OracleOut {
            strength: PokerStrength(e),
            category: category_of_poker_crate(class),
            detail: Some(detail_of_poker_crate(class)),
        }
    }

    fn eval7(&self, cards: &[CardIdx; 7]) -> OracleOut<PokerStrength> {
        let mut best: Option<Eval<FiveCard>> = None;
        for subset in SUBSETS_7C5 {
            let five: [Card; 5] = std::array::from_fn(|i| to_poker_crate(cards[subset[i]]));
            let e = self.eval_five_raw(&five);
            best = match best {
                None => Some(e),
                Some(cur) if e.is_better_than(cur) => Some(e),
                keep => keep,
            };
        }
        let e = best.expect("21 subsets is never empty");
        let class = e.classify();
        OracleOut {
            strength: PokerStrength(e),
            category: category_of_poker_crate(class),
            detail: Some(detail_of_poker_crate(class)),
        }
    }
}

impl PokerCrateOracle {
    /// Reference B's own royal-flush predicate, for checking ClearDeck's extra
    /// `HandRank::RoyalFlush` variant.
    pub fn is_royal_flush(s: PokerStrength) -> bool {
        s.0.is_royal_flush()
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
        let o = PokerCrateOracle::new();
        assert_eq!(o.eval5(&five("As Ks Qs Js Ts")).category, Category::StraightFlush);
        assert_eq!(o.eval5(&five("5h 4h 3h 2h Ah")).category, Category::StraightFlush);
        assert_eq!(o.eval5(&five("7c 7d 7h 7s 2c")).category, Category::FourOfAKind);
        assert_eq!(o.eval5(&five("Kd Kh 3c 3s 9d")).category, Category::TwoPair);
    }

    /// The wheel's straight high must be reported as FIVE, not as ace. If this
    /// reference reported ace-high the `Detail` comparison against ClearDeck's
    /// `Straight(5)` would produce a false disagreement.
    #[test]
    fn wheel_detail_is_five_high() {
        let o = PokerCrateOracle::new();
        let d = o.eval5(&five("5h 4d 3c 2s Ah")).detail.unwrap();
        assert_eq!(d.category, Category::Straight);
        assert_eq!(d.primary, 5, "wheel must be 5-high, got {}", d.primary);

        let sf = o.eval5(&five("5h 4h 3h 2h Ah")).detail.unwrap();
        assert_eq!(sf.category, Category::StraightFlush);
        assert_eq!(sf.primary, 5, "steel wheel must be 5-high, got {}", sf.primary);
    }

    #[test]
    fn strength_equality_follows_the_library_ordering() {
        let o = PokerCrateOracle::new();
        // Same hand in different suits: identical strength, and Eq must agree.
        let a = o.eval5(&five("Ah Kh Qd Jc 9s")).strength;
        let b = o.eval5(&five("As Ks Qh Jd 9c")).strength;
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), Ordering::Equal);
    }

    #[test]
    fn eval7_picks_the_best_of_twenty_one() {
        let o = PokerCrateOracle::new();
        let seven = parse_hand("As Ks Qs Js Ts 2c 3d").unwrap();
        let arr: [CardIdx; 7] = seven.try_into().unwrap();
        let out = o.eval7(&arr);
        assert_eq!(out.category, Category::StraightFlush);
        assert!(PokerCrateOracle::is_royal_flush(out.strength));
    }
}
