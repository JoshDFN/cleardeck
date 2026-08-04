//! Seven-card sweep: `evaluate_hand`, the entry point the canister actually calls.
//!
//! `table_canister` never calls `evaluate_five_cards` directly: at showdown it
//! calls `evaluate_hand(&hole_cards, &community_cards)`, which is
//! `evaluate_five_cards` maximised over the 21 five-card subsets under
//! `HandRank`'s derived `Ord`. So this check exercises the composition: the
//! per-subset evaluation, the `Ord` used to pick the winner among subsets, and
//! the class ordering, all at once.
//!
//! Reference A ranks seven cards with a single native lookup (not a 21-subset
//! loop), so an agreement here is not circular.
//!
//! The same [`compare_orderings`] proof is applied to the sample, which makes the
//! ordering coverage `O(n^2)` in the sample size rather than `O(n)`: five million
//! sampled hands means the ordering is checked over roughly 1.2e13 hand pairs.

use poker_core::HandRank;

use crate::cards::{slice_to_string, CardIdx};
use crate::category::{category_of_cleardeck, detail_of_cleardeck};
use crate::classes::{compare_orderings, RankOrderIndex, ViolationKind};
use crate::describe::{
    describe_ordering, require_unanimity, PointwiseCollector, PointwiseHit, SevenWitness,
};
use crate::engine::{ours_seven, render};
use crate::oracles::poker_oracle::{PokerCrateOracle, PokerStrength};
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::{DisagreementClass, OrderingRunSummary, Severity, SevensSummary};
use crate::rng::SplitMix64;

pub struct SevensOutcome {
    pub summary: SevensSummary,
    pub disagreements: Vec<DisagreementClass>,
    pub inconclusive: Vec<DisagreementClass>,
}

fn summarise(
    reference: &str,
    cmp: &crate::classes::OrderingComparison<SevenWitness>,
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

/// Streams `(our class id, reference class id, witness)` rows. Streamed rather
/// than materialised: at five million hands a `Vec` of rows per comparison would
/// be 80 MB of pure copy, three times over.
fn rows<'a>(
    witnesses: &'a [[CardIdx; 7]],
    ours_ins: &'a [u32],
    ours_order: &'a [u32],
    ref_ins: &'a [u32],
    ref_order: &'a [u32],
) -> impl Iterator<Item = (u32, u32, SevenWitness)> + 'a {
    (0..witnesses.len()).map(move |i| {
        (
            ours_order[ours_ins[i] as usize],
            ref_order[ref_ins[i] as usize],
            SevenWitness(witnesses[i]),
        )
    })
}

pub fn run(seed: u64, hands: u64) -> SevensOutcome {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let mut rng = SplitMix64::new(seed);

    let mut ours_idx: RankOrderIndex<HandRank> = RankOrderIndex::new();
    let mut rs_idx: RankOrderIndex<rs_poker::core::Rank> = RankOrderIndex::new();
    let mut pk_idx: RankOrderIndex<PokerStrength> = RankOrderIndex::new();

    let mut witnesses: Vec<[CardIdx; 7]> = Vec::with_capacity(hands as usize);
    let mut ours_ins: Vec<u32> = Vec::with_capacity(hands as usize);
    let mut rs_ins: Vec<u32> = Vec::with_capacity(hands as usize);
    let mut pk_ins: Vec<u32> = Vec::with_capacity(hands as usize);

    let mut cat_vs_rs = PointwiseCollector::new(
        "random_seven_card",
        "rs_poker",
        Severity::Cosmetic,
        "category7",
    );
    let mut cat_vs_pk =
        PointwiseCollector::new("random_seven_card", "poker", Severity::Cosmetic, "category7");
    let mut detail_vs_pk = PointwiseCollector::new(
        "random_seven_card",
        "poker",
        Severity::Cosmetic,
        "subrank-detail7",
    );
    let mut ref_cross = PointwiseCollector::new(
        "random_seven_card",
        "rs_poker-vs-poker",
        Severity::Inconclusive,
        "reference-category-conflict7",
    );

    for _ in 0..hands {
        let dealt = rng.deal(7);
        let hand: [CardIdx; 7] = [
            dealt[0], dealt[1], dealt[2], dealt[3], dealt[4], dealt[5], dealt[6],
        ];

        let our_rank = ours_seven(&hand);
        let rs_out = rs.eval7(&hand);
        let pk_out = pk.eval7(&hand);
        let our_cat = category_of_cleardeck(&our_rank);

        if rs_out.category != pk_out.category {
            ref_cross.record(PointwiseHit {
                cards: hand.to_vec(),
                ours: render(&our_rank),
                reference_says: format!(
                    "rs_poker={} poker={}",
                    rs_out.category.name(),
                    pk_out.category.name()
                ),
                shape: format!("{}-vs-{}", rs_out.category.name(), pk_out.category.name()),
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

        if let Some(ref_detail) = pk_out.detail {
            let our_detail = detail_of_cleardeck(&our_rank);
            if our_detail != ref_detail {
                detail_vs_pk.record(PointwiseHit {
                    cards: hand.to_vec(),
                    ours: format!("{} {:?}", render(&our_rank), our_detail),
                    reference_says: format!("{ref_detail:?}"),
                    shape: format!(
                        "{}-detail-mismatch",
                        our_detail.category.name()
                    ),
                });
            }
        }

        witnesses.push(hand);
        ours_ins.push(ours_idx.intern(&our_rank));
        rs_ins.push(rs_idx.intern(&rs_out.strength));
        pk_ins.push(pk_idx.intern(&pk_out.strength));
    }

    let ours_class_count = ours_idx.len();
    let rs_class_count = rs_idx.len();
    let pk_class_count = pk_idx.len();
    let (ours_order, _) = ours_idx.finish();
    let (rs_order, _) = rs_idx.finish();
    let (pk_order, _) = pk_idx.finish();

    let cmp_rs = compare_orderings(
        rows(&witnesses, &ours_ins, &ours_order, &rs_ins, &rs_order),
        ours_class_count,
        rs_class_count,
    );
    let cmp_pk = compare_orderings(
        rows(&witnesses, &ours_ins, &ours_order, &pk_ins, &pk_order),
        ours_class_count,
        pk_class_count,
    );
    let cmp_refs = compare_orderings(
        rows(&witnesses, &rs_ins, &rs_order, &pk_ins, &pk_order),
        rs_class_count,
        pk_class_count,
    );

    // Convict only where BOTH references agree against us.
    let mut ordering_entries =
        describe_ordering("random_seven_card", "rs_poker", &cmp_rs.violations);
    ordering_entries.extend(describe_ordering(
        "random_seven_card",
        "poker",
        &cmp_pk.violations,
    ));
    let (ordering_convictions, ordering_unproven) =
        require_unanimity(ordering_entries, "rs_poker", "poker");
    let mut disagreements = ordering_convictions;

    let cat_rs_total = cat_vs_rs.total();
    let cat_pk_total = cat_vs_pk.total();
    let detail_total = detail_vs_pk.total();

    let mut category_entries = cat_vs_rs.finish(|h| {
        format!(
            "evaluate_hand labels the 7 cards {} as {} but rs_poker's native 7-card lookup says {}.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    });
    category_entries.extend(cat_vs_pk.finish(|h| {
        format!(
            "evaluate_hand labels the 7 cards {} as {} but poker's best-of-21 says {}.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    }));
    let (category_convictions, category_unproven) =
        require_unanimity(category_entries, "rs_poker", "poker");
    disagreements.extend(category_convictions);

    // Sub-rank detail is exempt: only `poker` exposes it.
    disagreements.extend(detail_vs_pk.finish(|h| {
        format!(
            "evaluate_hand picks the right category for {} but the wrong rank inside it: {} vs {}.",
            slice_to_string(&h.cards),
            h.ours,
            h.reference_says
        )
    }));

    let mut inconclusive: Vec<DisagreementClass> = ref_cross.finish(|h| {
        format!(
            "rs_poker and poker disagree about the 7 cards {} ({}); withdrawn as evidence.",
            slice_to_string(&h.cards),
            h.reference_says
        )
    });
    inconclusive.extend(describe_ordering(
        "random_seven_card",
        "rs_poker-vs-poker",
        &cmp_refs.violations,
    ));
    inconclusive.extend(ordering_unproven);
    inconclusive.extend(category_unproven);
    for entry in inconclusive.iter_mut() {
        entry.severity = Severity::Inconclusive;
    }

    SevensOutcome {
        summary: SevensSummary {
            seed,
            hands_evaluated: hands,
            distinct_our_classes: ours_class_count,
            category_mismatches_vs_rs_poker: cat_rs_total,
            category_mismatches_vs_poker: cat_pk_total,
            detail_mismatches_vs_poker: detail_total,
            ordering: vec![summarise("rs_poker", &cmp_rs), summarise("poker", &cmp_pk)],
            reference_cross_check: Some(summarise("rs_poker-vs-poker", &cmp_refs)),
        },
        disagreements,
        inconclusive,
    }
}
