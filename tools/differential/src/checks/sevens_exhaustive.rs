//! Every seven-card hand: all `C(52,7) = 133,784,560` of them.
//!
//! Random sampling of seven-card hands leaves a residual "maybe the bug is in the
//! 0.001% you did not draw". This check removes it, and does so in constant memory
//! by reusing the *proven* class correspondence from the five-card sweep.
//!
//! # Why the class map is a sufficient oracle
//!
//! [`crate::checks::exhaustive`] proves, over all 2,598,960 five-card hands, that
//! our 7462 `HandRank` classes and each reference's 7462 classes are the same
//! order under a bijection. Once that holds, "the ordering of two seven-card
//! hands agrees with the reference" is *equivalent* to "each hand's `HandRank`
//! maps to the reference strength the bijection predicts", and the latter is a
//! per-hand check needing no memory of other hands. So a clean run here is a
//! complete pairwise ordering proof over all `C(133784560, 2)` ≈ 8.9e15 seven-card
//! matchups, not a sample.
//!
//! Refusing to run without a `trustworthy` map is deliberate: a map built from a
//! dirty or partial sweep would silently turn this into a tautology.

use std::collections::BTreeSet;

use poker_core::HandRank;

use crate::cards::{slice_to_string, CardIdx, NUM_CARDS};
use crate::checks::exhaustive::ClassMap;
use crate::describe::{PointwiseCollector, PointwiseHit};
use crate::engine::{ours_seven, render};
use crate::oracles::poker_oracle::PokerCrateOracle;
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::{DisagreementClass, ExhaustiveSevenSummary, Severity};

pub const TOTAL_SEVEN_CARD_HANDS: u64 = 133_784_560;

/// The number of distinct hand strengths reachable with seven cards. A
/// library-free combinatorial fact: of the 7462 five-card equivalence classes,
/// exactly 4824 are the best five of some seven-card hand (the 2638 that are not
/// are weak high-card and low one-pair holdings that seven cards always improve
/// on). Checking our engine's distinct-class count against it catches a
/// whole-class collapse even if both references were wrong the same way.
pub const EXPECTED_SEVEN_CARD_CLASSES: usize = 4824;

#[derive(Debug)]
pub struct ExhaustiveSevensOutcome {
    pub summary: ExhaustiveSevenSummary,
    pub disagreements: Vec<DisagreementClass>,
}

/// Every seven-card hand, in a fixed order.
pub fn all_seven_card_hands() -> impl Iterator<Item = [CardIdx; 7]> {
    let n = NUM_CARDS as u8;
    (0..n).flat_map(move |a| {
        ((a + 1)..n).flat_map(move |b| {
            ((b + 1)..n).flat_map(move |c| {
                ((c + 1)..n).flat_map(move |d| {
                    ((d + 1)..n).flat_map(move |e| {
                        ((e + 1)..n).flat_map(move |f| {
                            ((f + 1)..n).map(move |g| [a, b, c, d, e, f, g])
                        })
                    })
                })
            })
        })
    })
}

pub fn run(map: &ClassMap) -> Result<ExhaustiveSevensOutcome, String> {
    if !map.trustworthy {
        return Err(
            "refusing to run: the five-card class map is not trustworthy (sweep was partial or \
             reported ordering violations), so it cannot be used as an oracle"
                .to_string(),
        );
    }
    Ok(run_over(map, all_seven_card_hands(), TOTAL_SEVEN_CARD_HANDS))
}

/// Split out so a test can run the same logic over a small slice of the space.
pub fn run_over(
    map: &ClassMap,
    hands: impl Iterator<Item = [CardIdx; 7]>,
    expected: u64,
) -> ExhaustiveSevensOutcome {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();

    let mut collector = PointwiseCollector::new(
        "exhaustive_seven_card",
        "both (via the proven five-card class map)",
        Severity::FundImpacting,
        "class-map",
    );

    let mut hands_enumerated = 0u64;
    let mut unmapped = 0u64;
    let mut mismatched_rs = 0u64;
    let mut mismatched_pk = 0u64;
    let mut distinct_classes: BTreeSet<HandRank> = BTreeSet::new();

    for hand in hands {
        hands_enumerated += 1;
        let our_rank = ours_seven(&hand);
        // `contains` first, deliberately: `insert` would need an owned `HandRank`,
        // and cloning its `Vec<u8>` payload 133.8 million times is pure waste when
        // only 4824 of those clones are ever new.
        if !distinct_classes.contains(&our_rank) {
            distinct_classes.insert(our_rank.clone());
        }

        let (Some(&expected_rs), Some(&expected_pk)) =
            (map.rs.get(&our_rank), map.pk.get(&our_rank))
        else {
            unmapped += 1;
            collector.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: "no five-card hand produces this HandRank".to_string(),
                shape: "unreachable-handrank".to_string(),
            });
            continue;
        };

        let actual_rs = rs.eval7(&hand).strength;
        if actual_rs != expected_rs {
            mismatched_rs += 1;
            collector.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: format!(
                    "rs_poker says {actual_rs:?}, the class map predicts {expected_rs:?}"
                ),
                shape: "rs_poker-strength-mismatch".to_string(),
            });
        }

        let actual_pk = pk.eval7(&hand).strength;
        if actual_pk != expected_pk {
            mismatched_pk += 1;
            collector.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: "poker's best-of-21 strength differs from the class-map prediction"
                    .to_string(),
                shape: "poker-strength-mismatch".to_string(),
            });
        }
    }

    let disagreements = collector.finish(|h: &PointwiseHit| {
        format!(
            "evaluate_hand on {} returns {}, which does not correspond to the reference strength \
             the exhaustively-proven five-card class map requires ({}); at showdown this pays the \
             wrong player.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    });

    ExhaustiveSevensOutcome {
        summary: ExhaustiveSevenSummary {
            hands_enumerated,
            expected_hands: expected,
            unmapped_hand_ranks: unmapped,
            strength_mismatches_vs_rs_poker: mismatched_rs,
            strength_mismatches_vs_poker: mismatched_pk,
            distinct_our_classes: distinct_classes.len(),
            // Only meaningful for a full-space run.
            expected_distinct_classes: if expected == TOTAL_SEVEN_CARD_HANDS {
                Some(EXPECTED_SEVEN_CARD_CLASSES)
            } else {
                None
            },
        },
        disagreements,
    }
}

#[cfg(test)]
fn binomial(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut acc = 1u64;
    for i in 0..k {
        acc = acc * (n - i) / (i + 1);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_total_is_the_binomial_coefficient() {
        assert_eq!(binomial(52, 7), TOTAL_SEVEN_CARD_HANDS);
    }

    /// Counting all 133.8M tuples takes ~29s in a debug build, which is too slow
    /// for CI. Instead: verify a *slice* of the enumeration exactly (every hand
    /// whose two lowest cards are 0 and 1, i.e. `C(50,5)` of them) and verify the
    /// declared total against the closed form above. Together those pin both the
    /// shape and the size of the iterator.
    #[test]
    fn seven_card_enumeration_slice_is_ascending_and_the_right_size() {
        let mut n = 0u64;
        for h in all_seven_card_hands().take_while(|h| h[0] == 0 && h[1] == 1) {
            assert!(h.windows(2).all(|w| w[0] < w[1]), "not ascending: {h:?}");
            n += 1;
        }
        assert_eq!(n, binomial(50, 5), "slice size");
    }

    #[test]
    fn refuses_to_run_against_an_untrustworthy_class_map() {
        let map = ClassMap::default();
        assert!(!map.trustworthy);
        let err = run(&map).expect_err("an untrustworthy map must be refused");
        assert!(err.contains("trustworthy"), "unhelpful error: {err}");
    }
}
