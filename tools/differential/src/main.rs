//! `cargo run --release`: the full differential run.
//!
//! Prints a machine-readable summary (one `key=value` per line, then one compact
//! JSON object on the final line) and writes the full report to
//! `$SCRATCH/differential-report.json` (see
//! [`cleardeck_differential::default_report_path`]).
//!
//! Exit status: `0` when the engine agreed with both references everywhere the run
//! looked, `1` when any fund-impacting disagreement was found, `2` on a harness
//! error. The exhaustive sweep is a deliberate `--release`-only cost; the fast,
//! seeded subset that CI runs lives in `tests/fast_subset.rs`.

use std::process::ExitCode;

use cleardeck_differential::checks::{
    degenerate, exhaustive, pairs, sevens, sevens_exhaustive, third_exhaustive,
};
use cleardeck_differential::oracles::poker_oracle::PokerCrateOracle;
use cleardeck_differential::oracles::rs_poker_oracle::RsPokerOracle;
use cleardeck_differential::oracles::Oracle;
use cleardeck_differential::report::{ReferenceInfo, Report, Severity};
use cleardeck_differential::summary::print_machine_summary;
use cleardeck_differential::{
    adjudicator, default_report_path, HARNESS_NAME, HARNESS_VERSION, SEED_ADJUDICATOR, SEED_PAIRS,
    SEED_SEVENS,
};

struct Config {
    seven_card_hands: u64,
    pairs_each_mode: u64,
    adjudicator_sample: usize,
    skip_exhaustive: bool,
    /// All `C(52,7)` seven-card hands. ~20 minutes; off by default.
    exhaustive_sevens: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seven_card_hands: 5_000_000,
            pairs_each_mode: 2_000_000,
            adjudicator_sample: 50_000,
            skip_exhaustive: false,
            exhaustive_sevens: false,
        }
    }
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let need = |i: usize| -> Result<u64, String> {
            args.get(i + 1)
                .ok_or_else(|| format!("{} needs a value", args[i]))?
                .replace('_', "")
                .parse::<u64>()
                .map_err(|e| format!("{}: {e}", args[i]))
        };
        match args[i].as_str() {
            "--sevens" => {
                cfg.seven_card_hands = need(i)?;
                i += 2;
            }
            "--pairs" => {
                cfg.pairs_each_mode = need(i)?;
                i += 2;
            }
            "--adjudicator-sample" => {
                cfg.adjudicator_sample = need(i)? as usize;
                i += 2;
            }
            "--skip-exhaustive" => {
                cfg.skip_exhaustive = true;
                i += 1;
            }
            "--exhaustive-sevens" => {
                cfg.exhaustive_sevens = true;
                i += 1;
            }
            "--help" | "-h" => {
                println!(
                    "usage: cargo run --release -- [--sevens N] [--pairs N] \
                     [--adjudicator-sample N] [--skip-exhaustive] [--exhaustive-sevens]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(cfg)
}

fn main() -> ExitCode {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cleardeck-differential: {e}");
            return ExitCode::from(2);
        }
    };

    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let references = vec![
        ReferenceInfo {
            name: rs.name().to_string(),
            provenance: rs.provenance().to_string(),
        },
        ReferenceInfo {
            name: pk.name().to_string(),
            provenance: pk.provenance().to_string(),
        },
    ];

    let mut disagreements = Vec::new();
    let mut inconclusive = Vec::new();
    let mut notes = Vec::new();

    eprintln!("# auditing the two references against phevaluator ...");
    let adj = adjudicator::audit_references(SEED_ADJUDICATOR, cfg.adjudicator_sample);
    if !adj.available {
        notes.push(format!("adjudicator skipped: {}", adj.detail));
    } else if adj.disagreements_with_rs_poker > 0 || adj.disagreements_with_poker > 0 {
        notes.push(format!(
            "ADJUDICATOR CONFLICT: phevaluator disagrees with rs_poker {} time(s) and with poker {} time(s); \
             treat every verdict below as unproven until this is resolved",
            adj.disagreements_with_rs_poker, adj.disagreements_with_poker
        ));
    }

    let third = if cfg.skip_exhaustive {
        notes.push(
            "third-reference exhaustive five-card sweep SKIPPED by --skip-exhaustive".to_string(),
        );
        cleardeck_differential::report::ThirdReferenceSummary::default()
    } else {
        eprintln!("# third reference (phevaluator) over all 2,598,960 five-card hands ...");
        let out = third_exhaustive::run();
        if !out.summary.available {
            notes.push(format!("third reference skipped: {}", out.summary.detail));
        }
        disagreements.extend(out.disagreements);
        inconclusive.extend(out.inconclusive);
        out.summary
    };

    let mut class_map = None;
    let exhaustive_summary = if cfg.skip_exhaustive {
        notes.push("exhaustive five-card sweep SKIPPED by --skip-exhaustive".to_string());
        None
    } else {
        eprintln!("# exhaustive five-card sweep: 2,598,960 hands x 3 evaluators ...");
        let out = exhaustive::sweep_five(exhaustive::all_five_card_hands(), true);
        disagreements.extend(out.disagreements);
        inconclusive.extend(out.inconclusive);
        class_map = out.class_map;
        Some(out.summary)
    };

    let exhaustive_sevens_summary = if !cfg.exhaustive_sevens {
        None
    } else {
        match class_map.as_ref() {
            None => {
                notes.push(
                    "--exhaustive-sevens ignored: it needs the five-card class map, which \
                     --skip-exhaustive suppressed"
                        .to_string(),
                );
                None
            }
            Some(map) => {
                eprintln!(
                    "# exhaustive seven-card sweep: {} hands (this takes ~20 minutes) ...",
                    sevens_exhaustive::TOTAL_SEVEN_CARD_HANDS
                );
                match sevens_exhaustive::run(map) {
                    Err(e) => {
                        notes.push(format!("--exhaustive-sevens refused: {e}"));
                        None
                    }
                    Ok(out) => {
                        disagreements.extend(out.disagreements);
                        Some(out.summary)
                    }
                }
            }
        }
    };

    eprintln!("# seven-card sweep: {} hands ...", cfg.seven_card_hands);
    let sevens_out = sevens::run(SEED_SEVENS, cfg.seven_card_hands);
    disagreements.extend(sevens_out.disagreements);
    inconclusive.extend(sevens_out.inconclusive);

    eprintln!(
        "# pairwise sign check: {} pairs per mode ...",
        cfg.pairs_each_mode
    );
    let pairs_out = pairs::run(SEED_PAIRS, cfg.pairs_each_mode);
    disagreements.extend(pairs_out.disagreements);
    inconclusive.extend(pairs_out.inconclusive);

    eprintln!("# degenerate-input probes ...");
    let degen = degenerate::run();
    disagreements.extend(degen.findings);

    let fund_impacting = disagreements
        .iter()
        .filter(|d| d.severity == Severity::FundImpacting)
        .count();
    let verdict = if fund_impacting > 0 {
        "DISAGREE_FUND_IMPACTING"
    } else if disagreements
        .iter()
        .any(|d| d.severity == Severity::Cosmetic)
    {
        "DISAGREE_COSMETIC"
    } else if disagreements.iter().any(|d| d.severity == Severity::Latent) {
        "AGREE_WITH_LATENT_MISUSE_FINDINGS"
    } else {
        "AGREE"
    };

    let report = Report {
        harness: HARNESS_NAME,
        harness_version: HARNESS_VERSION,
        engine_under_test: "poker_core::{evaluate_five_cards, evaluate_hand} (path ../../src/poker_core)",
        references,
        adjudicator: adj,
        third_reference_exhaustive_five_card: third,
        exhaustive_five_card: exhaustive_summary,
        exhaustive_seven_card: exhaustive_sevens_summary,
        random_pairs: Some(pairs_out.summary),
        random_seven_card: Some(sevens_out.summary),
        degenerate_inputs: degen.summary,
        disagreement_classes: disagreements,
        inconclusive,
        verdict,
        notes,
    };

    let path = default_report_path();
    let json = match serde_json::to_string_pretty(&report) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("cleardeck-differential: could not serialise report: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, format!("{json}\n")) {
        eprintln!(
            "cleardeck-differential: could not write {}: {e}",
            path.display()
        );
        return ExitCode::from(2);
    }

    print_machine_summary(&report, &path);

    if fund_impacting > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

