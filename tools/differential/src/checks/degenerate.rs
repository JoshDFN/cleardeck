//! Inputs the reference libraries cannot even represent.
//!
//! `rs_poker` and `poker` both refuse (or debug-assert on) fewer than five cards,
//! more than seven cards, and duplicate cards. `poker_core` accepts all three
//! silently and returns a `HandRank` that looks perfectly ordinary. These probes
//! record exactly what it returns, so the fix wave knows whether the answer is
//! merely useless or actively wrong.
//!
//! Where a reference *can* be consulted (six-card boards; five-card boards) it is,
//! and the probe becomes a real differential check.

use poker_core::{detect_straight, HandRank};
use rs_poker::core::Rankable;

use crate::cards::{parse_hand, slice_to_string, to_rs_poker, CardIdx};
use crate::category::{category_of_cleardeck, category_of_rs_poker, Category};
use crate::engine::{ours_five_cards_raw, ours_hand, render};
use crate::report::{DegenerateSummary, DisagreementClass, Severity};

fn finding(
    id: &str,
    severity: Severity,
    cards: &[CardIdx],
    ours: String,
    reference_says: String,
    explanation: String,
) -> DisagreementClass {
    DisagreementClass {
        id: id.to_string(),
        severity,
        check: "degenerate_inputs".to_string(),
        reference: "poker rulebook / rs_poker where representable".to_string(),
        occurrences: 1,
        minimal_repro: vec![slice_to_string(cards)],
        ours: vec![ours],
        reference_says: vec![reference_says],
        explanation,
    }
}

fn cards(s: &str) -> Vec<CardIdx> {
    parse_hand(s).expect("literal cards in a probe must parse")
}

/// rs_poker's native rank for any 5..=7 card hand, as a category.
fn rs_category(hand: &[CardIdx]) -> Category {
    let converted: Vec<rs_poker::core::Card> = hand.iter().map(|&c| to_rs_poker(c)).collect();
    category_of_rs_poker(converted.rank().category())
}

pub struct DegenerateOutcome {
    pub summary: DegenerateSummary,
    pub findings: Vec<DisagreementClass>,
}

pub fn run() -> DegenerateOutcome {
    let mut findings = Vec::new();
    let mut probes = 0u64;

    // ---- P1: five total cards (2 hole + 3 community, i.e. the flop) ----------
    probes += 1;
    {
        let hand = cards("Ah Kh Qh Jh Th");
        let ours = ours_hand(&hand);
        let expected = rs_category(&hand);
        if category_of_cleardeck(&ours) != expected {
            findings.push(finding(
                "degenerate/flop-only-five-cards",
                Severity::FundImpacting,
                &hand,
                render(&ours),
                format!("rs_poker: {}", expected.name()),
                "evaluate_hand disagrees with the reference on a 5-card (flop) board.".into(),
            ));
        }
    }

    // ---- P2: six total cards (2 hole + 4 community, i.e. the turn) -----------
    // rs_poker ranks six cards natively, so this is a real differential check of
    // `evaluate_hand`'s 15-subset path.
    for spec in [
        "Ah Kh Qh Jh Th 2c",
        "2h 3h 4h 5h 9h 6c",
        "7c 7d 7h 7s 2c 3d",
        "Ac 2d 3h 4s 5c Kd",
    ] {
        probes += 1;
        let hand = cards(spec);
        let ours = ours_hand(&hand);
        let expected = rs_category(&hand);
        if category_of_cleardeck(&ours) != expected {
            findings.push(finding(
                "degenerate/turn-only-six-cards",
                Severity::FundImpacting,
                &hand,
                render(&ours),
                format!("rs_poker: {}", expected.name()),
                "evaluate_hand disagrees with the reference on a 6-card (turn) board.".into(),
            ));
        }
    }

    // ---- P3: fewer than five cards reach `evaluate_hand` ---------------------
    // `combinations(cards, 5)` returns EMPTY when fewer than five cards are
    // available, and `evaluate_hand` then falls back to `HandRank::HighCard(vec![])`
    //, which is `Ord`-EQUAL for every player and `Ord`-LESS than every real hand.
    for spec in ["Ah Kd", "Ah Kd Qc", "Ah Kd Qc Js"] {
        probes += 1;
        let hand = cards(spec);
        let ours = ours_hand(&hand);
        if ours == HandRank::HighCard(Vec::new()) {
            findings.push(finding(
                "degenerate/fewer-than-five-cards-returns-empty-highcard",
                Severity::Latent,
                &hand,
                render(&ours),
                "no reference will evaluate fewer than five cards; the rulebook has no ranking for them".into(),
                format!(
                    "evaluate_hand with only {} cards returns HandRank::HighCard([]), which compares EQUAL \
                     between players and LESS than every real hand instead of refusing; any showdown \
                     reached before the flop would chop rather than error.",
                    hand.len()
                ),
            ));
        }
    }

    // ---- P4: `evaluate_five_cards` handed six or seven cards ----------------
    // The name promises five; the signature takes `&[Card]`. With more than five
    // cards the flush test (`any suit count >= 5`) and the straight test (over ALL
    // ranks, suits ignored) can both pass on disjoint card sets, so the function
    // reports a straight flush that is not in the hand.
    for spec in [
        // 5 hearts (flush, 9-high) + 6c 7c makes a 3-4-5-6-7 straight in mixed suits.
        "2h 3h 4h 5h 9h 6c 7c",
        // Same trap with six cards.
        "2h 3h 4h 5h 9h 6c",
    ] {
        probes += 1;
        let hand = cards(spec);
        let ours = ours_five_cards_raw(&hand);
        let truth = rs_category(&hand);
        let our_cat = category_of_cleardeck(&ours);
        if our_cat != truth {
            findings.push(finding(
                "degenerate/evaluate_five_cards-fabricates-a-hand-from-more-than-five-cards",
                Severity::Latent,
                &hand,
                render(&ours),
                format!("best real hand in those cards is a {}", truth.name()),
                format!(
                    "evaluate_five_cards is `pub` and takes `&[Card]`, so a caller can hand it {} cards; \
                     it then tests `is_flush` on one suit and `is_straight` on ALL ranks independently and \
                     reports {} even though no such hand exists in the cards.",
                    hand.len(),
                    our_cat.name()
                ),
            ));
        }
    }

    // ---- P5: duplicate cards -----------------------------------------------
    // A dealing bug that hands the same card out twice must not be laundered into
    // a plausible-looking rank.
    for spec in ["Ah Ah Ah Ah Ah", "Ah Ah Ah Ah Kh", "Kh Kh Kd Kd Qs"] {
        probes += 1;
        let hand = cards(spec);
        let ours = ours_five_cards_raw(&hand);
        findings.push(finding(
            "degenerate/duplicate-cards-are-silently-ranked",
            Severity::Latent,
            &hand,
            render(&ours),
            "both references reject a hand containing the same card twice".into(),
            format!(
                "evaluate_five_cards accepts the physically impossible hand {} and returns {}; \
                 a dealing or shuffling defect would surface as a wrong winner rather than a trap.",
                slice_to_string(&hand),
                render(&ours)
            ),
        ));
    }

    // ---- P7: a duplicated card reaching `evaluate_hand` --------------------
    // The showdown path is `evaluate_hand(&hole, &community)`. If a dealing defect
    // ever hands the same physical card to a player twice (or puts a hole card on
    // the board), the 21-subset loop happily uses BOTH copies, so a five-card hand
    // can be built out of four physical cards.
    for (spec, note) in [
        (
            // Hole = Ah Ah. The "flush" Ah Ah Kh Qh Jh uses four physical cards.
            "Ah Ah Kh Qh Jh 2c 3d",
            "a five-card flush built from only four physical cards",
        ),
        (
            // A hole card that is also on the board.
            "Ah Kh Ah 2c 3d 4s 5h",
            "a hole card duplicated on the board",
        ),
    ] {
        probes += 1;
        let hand = cards(spec);
        let ours = ours_hand(&hand);
        let distinct: std::collections::BTreeSet<CardIdx> = hand.iter().copied().collect();
        findings.push(finding(
            "degenerate/duplicate-card-reaches-evaluate_hand",
            Severity::Latent,
            &hand,
            render(&ours),
            format!(
                "{} distinct physical cards were supplied; both references reject the input outright",
                distinct.len()
            ),
            format!(
                "evaluate_hand accepts {} ({} distinct physical cards) and returns {}: {}. \
                 It validates neither the card count nor distinctness, so a dealing or \
                 shuffling defect is laundered into a plausible-looking winning hand \
                 instead of trapping.",
                slice_to_string(&hand),
                distinct.len(),
                render(&ours),
                note
            ),
        ));
    }

    // ---- P6: `detect_straight` prefers the wheel over a better straight -----
    // Only reachable when more than five ranks are passed in, which today only
    // happens through the P4 misuse. Documented here so the fix wave keeps it.
    for ranks in [
        vec![14u8, 6, 5, 4, 3, 2],
        vec![14u8, 7, 6, 5, 4, 3, 2],
    ] {
        probes += 1;
        let got = detect_straight(&ranks);
        let best = ranks
            .iter()
            .copied()
            .filter(|&high| {
                high >= 6 && (0..5).all(|d| ranks.contains(&(high - d)))
            })
            .max();
        if let Some(best_high) = best {
            if got != Some(best_high) {
                // This probe's input is a RANK slice, not cards, so it cannot use
                // `finding()`'s card rendering: the reproducer is the literal call.
                findings.push(DisagreementClass {
                    id: "degenerate/detect_straight-prefers-the-wheel".to_string(),
                    severity: Severity::Latent,
                    check: "degenerate_inputs".to_string(),
                    reference: "poker rulebook".to_string(),
                    occurrences: 1,
                    minimal_repro: vec![format!("poker_core::detect_straight(&{ranks:?})")],
                    ours: vec![format!("{got:?}")],
                    reference_says: vec![format!("Some({best_high}), the best straight present")],
                    explanation: format!(
                        "detect_straight checks the wheel BEFORE the descending window scan, so with more \
                         than five ranks it returns the WEAKEST straight ({got:?}) instead of the best \
                         ({best_high}-high). Unreachable from evaluate_hand today because it only ever \
                         passes five ranks."
                    ),
                });
            }
        }
    }

    let with_findings = findings.len() as u64;
    let findings = merge_by_root_cause(findings);
    DegenerateOutcome {
        summary: DegenerateSummary {
            probes_run: probes,
            probes_with_findings: with_findings,
        },
        findings,
    }
}

/// Collapses probe hits that share an `id` into one entry per root cause, keeping
/// the first (smallest) reproducer plus up to [`REPRO_CAP`] illustrative extras.
const REPRO_CAP: usize = 3;

fn merge_by_root_cause(findings: Vec<DisagreementClass>) -> Vec<DisagreementClass> {
    let mut order: Vec<String> = Vec::new();
    let mut merged: std::collections::BTreeMap<String, DisagreementClass> =
        std::collections::BTreeMap::new();

    for f in findings {
        match merged.get_mut(&f.id) {
            None => {
                order.push(f.id.clone());
                merged.insert(f.id.clone(), f);
            }
            Some(existing) => {
                existing.occurrences += 1;
                if existing.minimal_repro.len() < REPRO_CAP {
                    existing.minimal_repro.extend(f.minimal_repro);
                    existing.ours.extend(f.ours);
                    existing.reference_says.extend(f.reference_says);
                }
            }
        }
    }

    order
        .into_iter()
        .filter_map(|id| merged.remove(&id))
        .collect()
}
