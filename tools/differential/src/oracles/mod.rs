//! The reference-evaluator abstraction.
//!
//! Every check in `crate::checks` is generic over [`Oracle`], so adding a third
//! reference costs one file and zero changes to the checks. Two oracles ship
//! in-process (see [`rs_poker_oracle`] and [`poker_oracle`]); a third, Python
//! [`phevaluator`] adjudicator lives in [`crate::adjudicator`] and is consulted
//! only when the two in-process oracles disagree with each other.
//!
//! # Why a `Strength` associated type instead of a plain `u32`
//!
//! Neither reference exposes its raw score as a public integer with a
//! documented meaning (`rs_poker::core::Rank`'s inner `u16` is private;
//! `poker::Eval`'s `hand_rank` is `pub(crate)`). Inventing a number by
//! reflecting on `Debug` output would make the harness's verdict depend on a
//! formatting detail. Instead each oracle hands back its own opaque, `Ord`
//! strength and the checks only ever *compare* strengths, which is exactly the
//! property under test.

use crate::cards::CardIdx;
use crate::category::{Category, Detail};

pub mod poker_oracle;
pub mod rs_poker_oracle;

/// One reference verdict on one hand.
#[derive(Clone, Copy, Debug)]
pub struct OracleOut<S> {
    /// Opaque strength; `Ord` such that greater is a better poker hand.
    pub strength: S,
    pub category: Category,
    /// Sub-rank detail, where the library exposes it. `None` is not a failure:
    /// it means this reference cannot be used for the detail check.
    pub detail: Option<Detail>,
}

pub trait Oracle {
    /// Opaque, totally-ordered hand strength. Greater is better.
    type Strength: Copy + Ord + std::fmt::Debug;

    /// Stable identifier used in the JSON report.
    fn name(&self) -> &'static str;

    /// Crate name and version, recorded in the report so a verdict can be
    /// re-derived years later against the same reference.
    fn provenance(&self) -> &'static str;

    fn eval5(&self, cards: &[CardIdx; 5]) -> OracleOut<Self::Strength>;

    /// Best five-card hand out of seven. Each oracle uses its own native path
    /// (a direct 7-card lookup where the library has one, otherwise best of the
    /// 21 five-card subsets), deliberately, so this is an independent check of
    /// `evaluate_hand` rather than a re-run of the same combination logic.
    fn eval7(&self, cards: &[CardIdx; 7]) -> OracleOut<Self::Strength>;
}

/// The 21 five-card subsets of a seven-card hand, in a fixed order.
pub const SUBSETS_7C5: [[usize; 5]; 21] = [
    [0, 1, 2, 3, 4],
    [0, 1, 2, 3, 5],
    [0, 1, 2, 3, 6],
    [0, 1, 2, 4, 5],
    [0, 1, 2, 4, 6],
    [0, 1, 2, 5, 6],
    [0, 1, 3, 4, 5],
    [0, 1, 3, 4, 6],
    [0, 1, 3, 5, 6],
    [0, 1, 4, 5, 6],
    [0, 2, 3, 4, 5],
    [0, 2, 3, 4, 6],
    [0, 2, 3, 5, 6],
    [0, 2, 4, 5, 6],
    [0, 3, 4, 5, 6],
    [1, 2, 3, 4, 5],
    [1, 2, 3, 4, 6],
    [1, 2, 3, 5, 6],
    [1, 2, 4, 5, 6],
    [1, 3, 4, 5, 6],
    [2, 3, 4, 5, 6],
];

#[cfg(test)]
mod tests {
    use super::SUBSETS_7C5;

    #[test]
    fn subsets_table_is_the_real_21() {
        assert_eq!(SUBSETS_7C5.len(), 21);
        let mut seen = std::collections::BTreeSet::new();
        for s in SUBSETS_7C5 {
            assert!(s.windows(2).all(|w| w[0] < w[1]), "not ascending: {s:?}");
            assert!(s.iter().all(|&i| i < 7));
            assert!(seen.insert(s), "duplicate subset {s:?}");
        }
        assert_eq!(seen.len(), 21);
    }
}
