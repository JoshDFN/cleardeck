//! Machine-readable summary printing.
//!
//! Split out of `main.rs` so the run orchestration stays readable. Format is one
//! `key=value` per line, greppable from CI without a JSON parser, followed by
//! one `DISAGREEMENT`/`INCONCLUSIVE`/`NOTE` block per finding. The full structured
//! report is the JSON file; this is the at-a-glance view.

use crate::report::Report;

pub fn print_machine_summary(report: &Report, path: &std::path::Path) {
    println!("verdict={}", report.verdict);
    println!("report_path={}", path.display());
    println!(
        "references={}",
        report
            .references
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("adjudicator_available={}", report.adjudicator.available);
    println!(
        "adjudicator_conflicts={}",
        report.adjudicator.disagreements_with_rs_poker + report.adjudicator.disagreements_with_poker
    );

    let third = &report.third_reference_exhaustive_five_card;
    println!("third_reference_available={}", third.available);
    if third.available {
        println!("third_reference_hands_checked={}", third.hands_checked);
        println!(
            "third_reference_classes={}",
            third.third_reference_class_count
        );
        for o in &third.ordering {
            println!(
                "third_reference_ordering[{}]=classes_left:{} classes_right:{} rows:{} false_ties:{} false_splits:{} inversions:{}",
                o.reference,
                o.ours_class_count,
                o.reference_class_count,
                o.rows_compared,
                o.false_ties,
                o.false_splits,
                o.inversions
            );
        }
    }

    if let Some(e) = &report.exhaustive_five_card {
        println!("five_card_hands_enumerated={}", e.hands_enumerated);
        println!(
            "five_card_category_counts_match_combinatorics={}",
            e.category_counts_match_combinatorics
        );
        println!("five_card_royal_flushes={}", e.our_royal_flush_count);
        println!("five_card_straight_flushes={}", e.our_straight_flush_count);
        println!(
            "five_card_category_mismatches={}",
            e.category_mismatches_vs_rs_poker + e.category_mismatches_vs_poker
        );
        println!("five_card_detail_mismatches={}", e.detail_mismatches_vs_poker);
        println!(
            "five_card_royal_alias_mismatches={}",
            e.royal_flush_alias_mismatches
        );
        for o in &e.ordering {
            println!(
                "five_card_ordering[{}]=classes_ours:{} classes_ref:{} rows:{} false_ties:{} false_splits:{} inversions:{}",
                o.reference,
                o.ours_class_count,
                o.reference_class_count,
                o.rows_compared,
                o.false_ties,
                o.false_splits,
                o.inversions
            );
        }
        if let Some(x) = &e.reference_cross_check {
            println!(
                "five_card_reference_cross_check=false_ties:{} false_splits:{} inversions:{} category_conflicts:{}",
                x.false_ties,
                x.false_splits,
                x.inversions,
                e.reference_cross_check_category_mismatches
            );
        }
    }

    if let Some(x) = &report.exhaustive_seven_card {
        println!(
            "exhaustive_seven_card_hands={} expected={}",
            x.hands_enumerated, x.expected_hands
        );
        println!(
            "exhaustive_seven_card_mismatches=rs_poker:{} poker:{} unmapped_handranks:{}",
            x.strength_mismatches_vs_rs_poker,
            x.strength_mismatches_vs_poker,
            x.unmapped_hand_ranks
        );
        println!(
            "exhaustive_seven_card_distinct_our_classes={} expected={:?}",
            x.distinct_our_classes, x.expected_distinct_classes
        );
    }

    if let Some(s) = &report.random_seven_card {
        println!("seven_card_hands={} seed={}", s.hands_evaluated, s.seed);
        println!("seven_card_distinct_our_classes={}", s.distinct_our_classes);
        println!(
            "seven_card_category_mismatches={}",
            s.category_mismatches_vs_rs_poker + s.category_mismatches_vs_poker
        );
        println!("seven_card_detail_mismatches={}", s.detail_mismatches_vs_poker);
        for o in &s.ordering {
            println!(
                "seven_card_ordering[{}]=classes_ours:{} classes_ref:{} rows:{} false_ties:{} false_splits:{} inversions:{}",
                o.reference,
                o.ours_class_count,
                o.reference_class_count,
                o.rows_compared,
                o.false_ties,
                o.false_splits,
                o.inversions
            );
        }
    }

    if let Some(p) = &report.random_pairs {
        println!(
            "pairs_five_vs_five={} pairs_showdown_shared_board={} seed={}",
            p.pairs_five_vs_five, p.pairs_showdown_shared_board, p.seed
        );
        println!(
            "pair_sign_mismatches=rs_poker:{} poker:{} reference_conflicts:{}",
            p.sign_mismatches_vs_rs_poker,
            p.sign_mismatches_vs_poker,
            p.reference_sign_disagreements
        );
    }

    println!(
        "degenerate_probes={} with_findings={}",
        report.degenerate_inputs.probes_run, report.degenerate_inputs.probes_with_findings
    );
    println!("disagreement_classes={}", report.disagreement_classes.len());
    println!("fund_impacting_classes={}", report.fund_impacting());
    println!("inconclusive_classes={}", report.inconclusive.len());

    for d in &report.disagreement_classes {
        println!(
            "DISAGREEMENT id={} severity={:?} check={} ref={} n={} repro=[{}] ours=[{}] ref_says=[{}]",
            d.id,
            d.severity,
            d.check,
            d.reference,
            d.occurrences,
            d.minimal_repro.join(" | "),
            d.ours.join(" | "),
            d.reference_says.join(" | ")
        );
        println!("  why: {}", d.explanation);
    }
    for d in &report.inconclusive {
        println!(
            "INCONCLUSIVE id={} n={} repro=[{}]",
            d.id,
            d.occurrences,
            d.minimal_repro.join(" | ")
        );
    }
    for n in &report.notes {
        println!("NOTE {n}");
    }

    print_headline_json(report, path);
}

/// The very last line: one compact JSON object, so a CI step can consume the
/// verdict without opening the report file or parsing `key=value` lines.
fn print_headline_json(report: &Report, path: &std::path::Path) {
    let headline = serde_json::json!({
        "harness": report.harness,
        "harness_version": report.harness_version,
        "verdict": report.verdict,
        "report_path": path.display().to_string(),
        "references": report.references.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
        "third_reference_available": report.third_reference_exhaustive_five_card.available,
        "five_card_hands": report
            .exhaustive_five_card
            .as_ref()
            .map(|e| e.hands_enumerated)
            .unwrap_or(0),
        "seven_card_hands_random": report
            .random_seven_card
            .as_ref()
            .map(|s| s.hands_evaluated)
            .unwrap_or(0),
        "seven_card_hands_exhaustive": report
            .exhaustive_seven_card
            .as_ref()
            .map(|s| s.hands_enumerated)
            .unwrap_or(0),
        "pairs_checked": report
            .random_pairs
            .as_ref()
            .map(|p| p.pairs_five_vs_five + p.pairs_showdown_shared_board)
            .unwrap_or(0),
        "disagreement_classes": report.disagreement_classes.len(),
        "fund_impacting_classes": report.fund_impacting(),
        "inconclusive_classes": report.inconclusive.len(),
    });
    // `to_string` cannot fail for a `serde_json::Value`, but never unwrap in a
    // reporting path: a panic here would destroy the run's only output.
    match serde_json::to_string(&headline) {
        Ok(line) => println!("{line}"),
        Err(e) => println!("{{\"harness_error\":\"headline serialisation failed: {e}\"}}"),
    }
}
