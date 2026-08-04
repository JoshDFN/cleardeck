//! Inputs a well-behaved evaluator should refuse.
//!
//! `poker_core` accepts fewer than five cards, more than five cards into its
//! five-card entry point, and duplicate cards, and returns a `HandRank` that looks
//! perfectly ordinary. These probes record exactly what it returns, so the fix wave
//! knows whether the answer is merely useless or actively wrong.
//!
//! # The corrected conclusion (docs/DEFECTS.md H-05)
//!
//! This file used to attach hard-coded strings claiming "both references reject a
//! hand containing the same card twice" and "no reference will evaluate fewer than
//! five cards". Neither reference was called, and both claims are false of
//! reference A: `rs_poker` silently ranks `Ah Ah Ah Ah Ah` as `StraightFlush(0)`,
//! `Kh Kh Kd Kd Qs` as `FourOfAKind(155)` and the two-card hand `Ah Kd` as
//! `HighCard(2140)`. `phevaluator` hands back the out-of-range sentinel `0` for
//! duplicates instead of raising. Only `poker` 0.7.0 errors.
//!
//! So: **no evaluator in this harness validates its own input.** Every probe below
//! now ASKS both references through [`super::reference_probe`] and reports the
//! measured answer, and E-09 has to be fixed at the `poker_core` boundary rather
//! than delegated to a reference that would supposedly have caught it.
//!
//! Every probe also asks `poker_core` inside a panic guard, so a probe emits a
//! finding only while the engine still ACCEPTS the input. Once `evaluate_hand`
//! starts refusing, the probe goes quiet by itself instead of reporting a defect
//! that has been fixed.

use poker_core::{detect_straight, HandRank};
use rs_poker::core::Rankable;

use super::reference_probe::{ask_both, describe_with_third, ReferenceVerdicts};
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

/// What `poker_core` did with a degenerate input, measured the same way the
/// references are measured: a rank, or a refusal.
///
/// A `HandRank` return is not the only possible outcome any more -- fixing E-09
/// means `evaluate_hand` and `evaluate_five_cards` start rejecting these inputs, and
/// on this side of the boundary a rejection can only arrive as a panic. Catching it
/// is what lets a probe report NOTHING once the defect is gone.
enum OurAnswer {
    Ranked(HandRank),
    Refused(String),
}

fn ask_engine<F: FnOnce() -> HandRank + std::panic::UnwindSafe>(f: F) -> OurAnswer {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let out = std::panic::catch_unwind(f);
    std::panic::set_hook(previous);
    match out {
        Ok(rank) => OurAnswer::Ranked(rank),
        Err(e) => OurAnswer::Refused(
            e.downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panicked".to_string()),
        ),
    }
}

/// One sentence stating what the references did, for the `explanation` field.
///
/// This is the H-05 correction in the report itself: if neither reference refused,
/// the finding says so out loud rather than implying corroboration it does not have.
fn corroboration(v: &ReferenceVerdicts) -> &'static str {
    if v.none_refused() {
        "NEITHER reference refused this input either, so no reference would have caught it: the \
         validation has to live in poker_core"
    } else if v.rs_poker.is_refusal() && v.poker_crate.is_refusal() {
        "both references refused the input"
    } else {
        "one reference refused the input and the other ranked it anyway"
    }
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
    // available, and `evaluate_hand` then falls back to `HandRank::HighCard(vec![])`,
    // which is `Ord`-EQUAL for every player and `Ord`-LESS than every real hand.
    //
    // Reported only while the engine actually accepts the input. Both references are
    // asked, and their measured answers go in the report: `poker` 0.7.0 errors on the
    // card count, `rs_poker` ranks two cards as `HighCard(2140)` without complaint.
    for spec in ["Ah Kd", "Ah Kd Qc", "Ah Kd Qc Js"] {
        probes += 1;
        let hand = cards(spec);
        let hand_for_engine = hand.clone();
        let OurAnswer::Ranked(ours) = ask_engine(move || ours_hand(&hand_for_engine)) else {
            continue; // the engine now refuses a short board: nothing to report
        };
        let verdicts = ask_both(&hand);
        let degenerate_ordering = ours == HandRank::HighCard(Vec::new());
        findings.push(finding(
            "degenerate/fewer-than-five-cards-returns-empty-highcard",
            Severity::Latent,
            &hand,
            render(&ours),
            describe_with_third(&hand, &verdicts),
            format!(
                "evaluate_hand accepted {} card(s) and returned {}{}. {}.",
                hand.len(),
                render(&ours),
                if degenerate_ordering {
                    ", which compares EQUAL between players and LESS than every real hand instead \
                     of refusing, so any showdown reached before the flop would chop rather than \
                     error"
                } else {
                    ", a rank for a board that cannot make a five-card hand"
                },
                corroboration(&verdicts)
            ),
        ));
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
        let hand_for_engine = hand.clone();
        let OurAnswer::Ranked(ours) = ask_engine(move || ours_five_cards_raw(&hand_for_engine))
        else {
            continue; // evaluate_five_cards now rejects >5 cards: nothing to report
        };
        // These inputs are 6 or 7 DISTINCT cards, so rs_poker's native 6/7-card path
        // is a legitimate oracle here: its answer is the best real hand present.
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
    //
    // This probe used to be pushed UNCONDITIONALLY with the hard-coded claim "both
    // references reject a hand containing the same card twice". Measured: `poker`
    // 0.7.0 does; `rs_poker` ranks `Ah Ah Ah Ah Ah` as `StraightFlush(0)` and
    // `Kh Kh Kd Kd Qs` as `FourOfAKind(155)`.
    for spec in ["Ah Ah Ah Ah Ah", "Ah Ah Ah Ah Kh", "Kh Kh Kd Kd Qs"] {
        probes += 1;
        let hand = cards(spec);
        let hand_for_engine = hand.clone();
        let OurAnswer::Ranked(ours) = ask_engine(move || ours_five_cards_raw(&hand_for_engine))
        else {
            continue; // evaluate_five_cards now rejects duplicates: nothing to report
        };
        let verdicts = ask_both(&hand);
        let distinct: std::collections::BTreeSet<CardIdx> = hand.iter().copied().collect();
        findings.push(finding(
            "degenerate/duplicate-cards-are-silently-ranked",
            Severity::Latent,
            &hand,
            render(&ours),
            describe_with_third(&hand, &verdicts),
            format!(
                "evaluate_five_cards accepts the physically impossible hand {} ({} distinct \
                 physical cards) and returns {}, so a dealing or shuffling defect would surface as \
                 a wrong winner rather than a trap. {}.",
                slice_to_string(&hand),
                distinct.len(),
                render(&ours),
                corroboration(&verdicts)
            ),
        ));
    }

    // ---- P7: a duplicated card reaching `evaluate_hand` --------------------
    // The showdown path is `evaluate_hand(&hole, &community)`. If a dealing defect
    // ever hands the same physical card to a player twice (or puts a hole card on
    // the board), the 21-subset loop happily uses BOTH copies, so a five-card hand
    // can be built out of four physical cards.
    // Also previously pushed unconditionally, with the hard-coded claim that "both
    // references reject the input outright". Measured: `rs_poker` ranks
    // `Ah Ah Kh Qh Jh 2c 3d` as `StraightFlush(0)`.
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
        let hand_for_engine = hand.clone();
        let OurAnswer::Ranked(ours) = ask_engine(move || ours_hand(&hand_for_engine)) else {
            continue; // evaluate_hand now rejects duplicates: nothing to report
        };
        let verdicts = ask_both(&hand);
        let distinct: std::collections::BTreeSet<CardIdx> = hand.iter().copied().collect();
        findings.push(finding(
            "degenerate/duplicate-card-reaches-evaluate_hand",
            Severity::Latent,
            &hand,
            render(&ours),
            describe_with_third(&hand, &verdicts),
            format!(
                "evaluate_hand accepts {} ({} distinct physical cards) and returns {}: {}. \
                 It validates neither the card count nor distinctness, so a dealing or \
                 shuffling defect is laundered into a plausible-looking winning hand \
                 instead of trapping. {}.",
                slice_to_string(&hand),
                distinct.len(),
                render(&ours),
                note,
                corroboration(&verdicts)
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
