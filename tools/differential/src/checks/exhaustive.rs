//! Five-card sweep: every hand, every pair, both references.
//!
//! The sweep is one pass that records, per hand, the interned class id from each
//! of the three evaluators plus the packed cards, then runs the
//! [`compare_orderings`] proof three times: ours-vs-A, ours-vs-B and A-vs-B. The
//! third one is what makes "inconclusive" a real verdict instead of an excuse:
//! if the two references disagree with each other about a hand, that hand is
//! withdrawn as evidence rather than charged to ClearDeck.
//!
//! A fourth, oracle-free check rides along: the number of hands our engine puts
//! in each category must equal the textbook combinatorial count
//! (1,302,540 high cards, 1,098,240 one pairs, ... 40 straight flushes). That
//! catches a whole-category misclassification even in the impossible case that
//! both references are wrong in the same direction.

use poker_core::HandRank;

use crate::cards::{pack5, slice_to_string, CardIdx, NUM_CARDS};
use crate::category::{category_of_cleardeck, detail_of_cleardeck, Category, Detail};
use crate::classes::{compare_orderings, RankOrderIndex, ViolationKind};
use crate::describe::{
    describe_ordering, require_unanimity, FiveWitness, PointwiseCollector, PointwiseHit,
};
use crate::engine::{ours_five, render};
use crate::oracles::poker_oracle::PokerCrateOracle;
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::{
    CategoryCount, DisagreementClass, ExhaustiveFiveSummary, OrderingRunSummary, Severity,
};
use crate::rng::SplitMix64;

/// The textbook count of distinct five-card hands per category. Independent of
/// every library in this harness.
pub const COMBINATORIAL_COUNTS: [(Category, u64); 9] = [
    (Category::HighCard, 1_302_540),
    (Category::OnePair, 1_098_240),
    (Category::TwoPair, 123_552),
    (Category::ThreeOfAKind, 54_912),
    (Category::Straight, 10_200),
    (Category::Flush, 5_108),
    (Category::FullHouse, 3_744),
    (Category::FourOfAKind, 624),
    (Category::StraightFlush, 40),
];

pub const TOTAL_FIVE_CARD_HANDS: u64 = 2_598_960;
/// Distinct five-card hand equivalence classes in Texas Hold'em.
pub const EXPECTED_CLASSES: usize = 7462;

pub struct ExhaustiveOutcome {
    pub summary: ExhaustiveFiveSummary,
    pub disagreements: Vec<DisagreementClass>,
    pub inconclusive: Vec<DisagreementClass>,
    /// The proven class correspondence, present only for a full-space sweep.
    /// [`crate::checks::sevens_exhaustive`] reuses it to check all `C(52,7)`
    /// seven-card hands in one streaming pass and constant memory.
    pub class_map: Option<ClassMap>,
}

/// `HandRank` -> the reference strength it must always coincide with.
///
/// Only meaningful once the full-space sweep has reported zero ordering
/// violations: at that point the map is a proven order-isomorphism, so "our rank
/// maps to the reference's strength" is equivalent to "the ordering is right".
#[derive(Clone, Debug, Default)]
pub struct ClassMap {
    pub rs: std::collections::BTreeMap<HandRank, rs_poker::core::Rank>,
    pub pk: std::collections::BTreeMap<HandRank, crate::oracles::poker_oracle::PokerStrength>,
    /// True when the sweep that produced this map found zero ordering violations
    /// against both references. A map built from a dirty sweep must not be used
    /// as an oracle.
    pub trustworthy: bool,
}

/// Every five-card hand, in a fixed order.
pub fn all_five_card_hands() -> impl Iterator<Item = [CardIdx; 5]> {
    (0..NUM_CARDS as u8).flat_map(move |a| {
        ((a + 1)..NUM_CARDS as u8).flat_map(move |b| {
            ((b + 1)..NUM_CARDS as u8).flat_map(move |c| {
                ((c + 1)..NUM_CARDS as u8).flat_map(move |d| {
                    ((d + 1)..NUM_CARDS as u8).map(move |e| [a, b, c, d, e])
                })
            })
        })
    })
}

/// A seeded random sample of five-card hands, for the fast `cargo test` subset.
pub fn sampled_five_card_hands(seed: u64, n: usize) -> impl Iterator<Item = [CardIdx; 5]> {
    let mut rng = SplitMix64::new(seed);
    (0..n).map(move |_| {
        let dealt = rng.deal(5);
        [dealt[0], dealt[1], dealt[2], dealt[3], dealt[4]]
    })
}

struct Corpus {
    packed: Vec<u32>,
    ours: Vec<u32>,
    rs: Vec<u32>,
    pk: Vec<u32>,
    ours_class_count: usize,
    rs_class_count: usize,
    pk_class_count: usize,
}

fn summarise(
    reference: &str,
    cmp: &crate::classes::OrderingComparison<FiveWitness>,
) -> OrderingRunSummary {
    let count = |k: ViolationKind| cmp.violations.iter().filter(|v| v.kind == k).count();
    OrderingRunSummary {
        reference: reference.to_string(),
        ours_class_count: cmp.ours_class_count,
        reference_class_count: cmp.reference_class_count,
        rows_compared: cmp.rows_compared,
        false_ties: count(ViolationKind::FalseTie),
        false_splits: count(ViolationKind::FalseSplit),
        inversions: count(ViolationKind::Inversion),
    }
}

/// Runs the sweep over whatever set of five-card hands it is handed.
///
/// `full_space` tells the check whether to hold the run to the combinatorial
/// counts and the 7462-class expectation (true only for [`all_five_card_hands`]).
pub fn sweep_five(
    hands: impl Iterator<Item = [CardIdx; 5]>,
    full_space: bool,
) -> ExhaustiveOutcome {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();

    let mut ours_idx: RankOrderIndex<HandRank> = RankOrderIndex::new();
    let mut rs_idx: RankOrderIndex<rs_poker::core::Rank> = RankOrderIndex::new();
    let mut pk_idx: RankOrderIndex<crate::oracles::poker_oracle::PokerStrength> =
        RankOrderIndex::new();

    let mut corpus = Corpus {
        packed: Vec::new(),
        ours: Vec::new(),
        rs: Vec::new(),
        pk: Vec::new(),
        ours_class_count: 0,
        rs_class_count: 0,
        pk_class_count: 0,
    };

    let mut cat_counts = [0u64; 9];
    let mut royal_count = 0u64;
    let mut straight_flush_count = 0u64;

    let mut cat_vs_rs = PointwiseCollector::new(
        "exhaustive_five_card",
        "rs_poker",
        Severity::Cosmetic,
        "category",
    );
    let mut cat_vs_pk = PointwiseCollector::new(
        "exhaustive_five_card",
        "poker",
        Severity::Cosmetic,
        "category",
    );
    let mut detail_vs_pk = PointwiseCollector::new(
        "exhaustive_five_card",
        "poker",
        Severity::Cosmetic,
        "subrank-detail",
    );
    let mut royal_alias = PointwiseCollector::new(
        "exhaustive_five_card",
        "both",
        Severity::Cosmetic,
        "royal-flush-alias",
    );
    let mut ref_cat_cross = PointwiseCollector::new(
        "exhaustive_five_card",
        "rs_poker-vs-poker",
        Severity::Inconclusive,
        "reference-category-conflict",
    );

    let mut hands_enumerated = 0u64;
    let mut class_map = ClassMap::default();

    for hand in hands {
        hands_enumerated += 1;
        let our_rank = ours_five(&hand);
        let rs_out = rs.eval5(&hand);
        let pk_out = pk.eval5(&hand);

        let our_cat = category_of_cleardeck(&our_rank);
        cat_counts[our_cat as usize] += 1;
        match &our_rank {
            HandRank::RoyalFlush => royal_count += 1,
            HandRank::StraightFlush(_) => straight_flush_count += 1,
            _ => {}
        }

        // The references must agree with each other before either can convict us.
        if rs_out.category != pk_out.category {
            ref_cat_cross.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: format!(
                    "rs_poker={} poker={}",
                    rs_out.category.name(),
                    pk_out.category.name()
                ),
                shape: format!(
                    "{}-vs-{}",
                    rs_out.category.name(),
                    pk_out.category.name()
                ),
            });
        } else {
            if our_cat != rs_out.category {
                cat_vs_rs.record(PointwiseHit {
                    cards: hand.to_vec(),
                    ours: render(&our_rank),
                    reference_says: rs_out.category.name().to_string(),
                    shape: format!("{}-not-{}", our_cat.name(), rs_out.category.name()),
                });
            }
            if our_cat != pk_out.category {
                cat_vs_pk.record(PointwiseHit {
                    cards: hand.to_vec(),
                    ours: render(&our_rank),
                    reference_says: pk_out.category.name().to_string(),
                    shape: format!("{}-not-{}", our_cat.name(), pk_out.category.name()),
                });
            }
        }

        // Sub-rank detail: only reference B exposes it.
        if let Some(ref_detail) = pk_out.detail {
            let our_detail = detail_of_cleardeck(&our_rank);
            if our_detail != ref_detail {
                detail_vs_pk.record(PointwiseHit {
                    cards: hand.to_vec(),
                    ours: format!("{} {:?}", render(&our_rank), our_detail),
                    reference_says: format!("{ref_detail:?}"),
                    shape: detail_shape(&our_detail, &ref_detail),
                });
            }
        }

        // ClearDeck's extra `RoyalFlush` variant must be exactly the ace-high
        // straight flushes, per BOTH references.
        let ours_royal = matches!(our_rank, HandRank::RoyalFlush);
        let rs_royal = RsPokerOracle::is_royal_flush(rs_out.strength);
        let pk_royal = PokerCrateOracle::is_royal_flush(pk_out.strength);
        if ours_royal != rs_royal || ours_royal != pk_royal {
            royal_alias.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: format!("rs_poker_royal={rs_royal} poker_royal={pk_royal}"),
                shape: format!("ours={ours_royal}-rs={rs_royal}-pk={pk_royal}"),
            });
        }

        class_map
            .rs
            .entry(our_rank.clone())
            .or_insert(rs_out.strength);
        class_map
            .pk
            .entry(our_rank.clone())
            .or_insert(pk_out.strength);

        corpus.packed.push(pack5(&hand));
        corpus.ours.push(ours_idx.intern(&our_rank));
        corpus.rs.push(rs_idx.intern(&rs_out.strength));
        corpus.pk.push(pk_idx.intern(&pk_out.strength));
    }

    corpus.ours_class_count = ours_idx.len();
    corpus.rs_class_count = rs_idx.len();
    corpus.pk_class_count = pk_idx.len();
    let (ours_order, _ours_values) = ours_idx.finish();
    let (rs_order, _) = rs_idx.finish();
    let (pk_order, _) = pk_idx.finish();

    let rows_ours_rs = corpus.packed.iter().enumerate().map(|(i, &p)| {
        (
            ours_order[corpus.ours[i] as usize],
            rs_order[corpus.rs[i] as usize],
            FiveWitness(p),
        )
    });
    let cmp_rs = compare_orderings(
        rows_ours_rs,
        corpus.ours_class_count,
        corpus.rs_class_count,
    );

    let rows_ours_pk = corpus.packed.iter().enumerate().map(|(i, &p)| {
        (
            ours_order[corpus.ours[i] as usize],
            pk_order[corpus.pk[i] as usize],
            FiveWitness(p),
        )
    });
    let cmp_pk = compare_orderings(
        rows_ours_pk,
        corpus.ours_class_count,
        corpus.pk_class_count,
    );

    // Reference vs reference. Any violation here means the affected hands cannot
    // be used as evidence about ClearDeck.
    let rows_rs_pk = corpus.packed.iter().enumerate().map(|(i, &p)| {
        (
            rs_order[corpus.rs[i] as usize],
            pk_order[corpus.pk[i] as usize],
            FiveWitness(p),
        )
    });
    let cmp_refs = compare_orderings(rows_rs_pk, corpus.rs_class_count, corpus.pk_class_count);

    // Ordering: convict only where BOTH references agree against us.
    let mut ordering_entries = describe_ordering(
        "exhaustive_five_card",
        "rs_poker",
        &cmp_rs.violations,
    );
    ordering_entries.extend(describe_ordering(
        "exhaustive_five_card",
        "poker",
        &cmp_pk.violations,
    ));
    let (ordering_convictions, ordering_unproven) =
        require_unanimity(ordering_entries, "rs_poker", "poker");

    let mut disagreements = ordering_convictions;

    let detail_total = detail_vs_pk.total();
    let cat_rs_total = cat_vs_rs.total();
    let cat_pk_total = cat_vs_pk.total();
    let royal_total = royal_alias.total();
    let ref_cross_total = ref_cat_cross.total();

    // Category: same unanimity rule.
    let mut category_entries = cat_vs_rs.finish(|h| {
        format!(
            "ClearDeck labels {} as {} but rs_poker calls it {}; the pot may still be correct, \
             the reported hand name is not.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    });
    category_entries.extend(cat_vs_pk.finish(|h| {
        format!(
            "ClearDeck labels {} as {} but poker calls it {}.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    }));
    let (category_convictions, category_unproven) =
        require_unanimity(category_entries, "rs_poker", "poker");
    disagreements.extend(category_convictions);

    // Sub-rank detail is exempt from unanimity: `rs_poker` exposes no kicker
    // detail, so `poker` is the only reference that can speak here.
    disagreements.extend(detail_vs_pk.finish(|h| {
        format!(
            "ClearDeck picks the right category for {} but the wrong rank inside it: {} vs {}.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    }));
    disagreements.extend(royal_alias.finish(|h| {
        format!(
            "HandRank::RoyalFlush does not coincide with the references' ace-high straight flush on {} ({}).",
            slice_to_string(&h.cards),
            h.reference_says
        )
    }));

    let mut inconclusive: Vec<DisagreementClass> = ref_cat_cross.finish(|h| {
        format!(
            "rs_poker and poker disagree about {} ({}); this hand is withdrawn as evidence.",
            slice_to_string(&h.cards),
            h.reference_says
        )
    });
    inconclusive.extend(describe_ordering(
        "exhaustive_five_card",
        "rs_poker-vs-poker",
        &cmp_refs.violations,
    ));
    inconclusive.extend(ordering_unproven);
    inconclusive.extend(category_unproven);
    for entry in inconclusive.iter_mut() {
        entry.severity = Severity::Inconclusive;
    }

    let category_counts: Vec<CategoryCount> = COMBINATORIAL_COUNTS
        .iter()
        .map(|&(cat, expected)| CategoryCount {
            category: cat,
            ours: cat_counts[cat as usize],
            expected: if full_space { Some(expected) } else { None },
        })
        .collect();
    let counts_ok = !full_space
        || category_counts
            .iter()
            .all(|c| c.expected == Some(c.ours));

    let summary = ExhaustiveFiveSummary {
        hands_enumerated,
        expected_hands: if full_space { TOTAL_FIVE_CARD_HANDS } else { 0 },
        category_mismatches_vs_rs_poker: cat_rs_total,
        category_mismatches_vs_poker: cat_pk_total,
        detail_mismatches_vs_poker: detail_total,
        royal_flush_alias_mismatches: royal_total,
        our_royal_flush_count: royal_count,
        our_straight_flush_count: straight_flush_count,
        category_counts,
        category_counts_match_combinatorics: counts_ok,
        ordering: vec![summarise("rs_poker", &cmp_rs), summarise("poker", &cmp_pk)],
        reference_cross_check: Some(summarise("rs_poker-vs-poker", &cmp_refs)),
        reference_cross_check_category_mismatches: ref_cross_total,
    };

    // The map is only an oracle if the sweep it came from was clean AND covered
    // the whole space. A sampled sweep leaves classes unseen; a dirty sweep means
    // the correspondence is not a function.
    class_map.trustworthy = full_space
        && cmp_rs.violations.is_empty()
        && cmp_pk.violations.is_empty()
        && cmp_refs.violations.is_empty()
        && class_map.rs.len() == EXPECTED_CLASSES
        && class_map.pk.len() == EXPECTED_CLASSES;

    ExhaustiveOutcome {
        summary,
        disagreements,
        inconclusive,
        class_map: if full_space { Some(class_map) } else { None },
    }
}

fn detail_shape(ours: &Detail, theirs: &Detail) -> String {
    if ours.category != theirs.category {
        return format!("{}-not-{}", ours.category.name(), theirs.category.name());
    }
    let which = match (
        ours.primary == theirs.primary,
        ours.secondary == theirs.secondary,
    ) {
        (false, false) => "primary+secondary",
        (false, true) => "primary",
        (true, false) => "secondary",
        (true, true) => "equal",
    };
    format!("{}-wrong-{}", ours.category.name(), which)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumeration_visits_every_hand_exactly_once() {
        // Cheap structural check on the iterator itself: count and strict
        // ascension. The 2.6M evaluation sweep stays behind `--release`.
        let mut n = 0u64;
        for h in all_five_card_hands() {
            assert!(h.windows(2).all(|w| w[0] < w[1]), "not ascending: {h:?}");
            n += 1;
        }
        assert_eq!(n, TOTAL_FIVE_CARD_HANDS);
    }

    #[test]
    fn combinatorial_counts_sum_to_the_whole_space() {
        let total: u64 = COMBINATORIAL_COUNTS.iter().map(|&(_, n)| n).sum();
        assert_eq!(total, TOTAL_FIVE_CARD_HANDS);
    }
}
