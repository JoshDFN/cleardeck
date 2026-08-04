//! The fuzz driver.
//!
//! Runs seeded hostile sequences against the real canister, asserts M1..M6 after
//! every step, shrinks any NEW violation to a minimal reproducer, and writes
//! `money-fuzz-report.json`.
//!
//! Tuning, all via the environment:
//!   MONEY_FUZZ_SEEDS=1,2,3      which seeds to run (default: a fixed set)
//!   MONEY_FUZZ_STEPS=220        steps per run
//!   MONEY_FUZZ_SHRINK=60        candidate runs the shrinker may spend
//!   MONEY_FUZZ_REPORT=<path>    where to write the report
//!
//! Each step is a real IC message against a real replica, so this is minutes, not
//! seconds. It is a plain `#[test]`, not `#[ignore]`d, because a money invariant
//! that is not run is not an invariant.

use money_safety::fuzz::*;
use money_safety::invariants::Violation;
use money_safety::table_api::TableConfig;
use money_safety::wasms;

const ACTORS: [&str; 4] = ["alice", "bob", "carol", "attacker"];

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn seeds() -> Vec<u64> {
    match std::env::var("MONEY_FUZZ_SEEDS") {
        Ok(v) => v
            .split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect(),
        // A fixed set, so the suite is reproducible run to run. Three different
        // table shapes: heads-up, 6-max, and 6-max with an ante.
        Err(_) => vec![0xC1EA_2DEC_0001, 0xC1EA_2DEC_0002, 0xC1EA_2DEC_0003],
    }
}

fn config_for(index: usize) -> TableConfig {
    match index % 3 {
        0 => TableConfig::heads_up_icp(),
        1 => TableConfig::six_max_icp(),
        _ => TableConfig::six_max_with_ante(),
    }
}

#[test]
fn hostile_sequences_never_create_chips_double_pay_or_lose_state_across_an_upgrade() {
    let steps = env_usize("MONEY_FUZZ_STEPS", DEFAULT_STEPS);
    let shrink_budget = env_usize("MONEY_FUZZ_SHRINK", 60);
    let seed_list = seeds();

    let mut runs = Vec::new();
    let mut reproducers: Vec<Reproducer> = Vec::new();
    let mut total_steps = 0usize;
    let mut blocking_sigs: Vec<String> = Vec::new();
    let mut documented_sigs: Vec<String> = Vec::new();

    for (i, seed) in seed_list.iter().copied().enumerate() {
        let config = config_for(i);
        let actors: Vec<&str> = ACTORS
            .iter()
            .copied()
            .take((config.max_players as usize + 2).min(ACTORS.len()))
            .collect();
        let ops = generate(seed, steps, &config, actors.len());

        eprintln!(
            "money-fuzz: seed {seed:#x}, {} ops, {}-max table, ante {}",
            ops.len(),
            config.max_players,
            config.ante
        );
        let (report, violations) = run_sequence(seed, &ops, config.clone(), &actors);
        total_steps += report.steps_executed;

        eprintln!(
            "money-fuzz: seed {seed:#x} finished: {} hands, {} upgrades, {} blocking finding(s), \
             {} documented finding(s), worst stranded {} e8s",
            report.hands_completed,
            report.upgrades,
            report.blocking_findings.len(),
            report.documented_findings.len(),
            report.max_stranded_e8s
        );
        for f in &report.documented_findings {
            documented_sigs.push(f.signature.clone());
            eprintln!(
                "  documented  {} x{} worst_delta={} -- {}",
                f.signature, f.occurrences, f.worst_delta_e8s, f.violation.detail
            );
        }

        // Shrink every NEW violation to a minimal reproducer.
        let blocking: Vec<Violation> = violations
            .iter()
            .filter(|v| !v.severity.is_documented_defect())
            .cloned()
            .collect();
        let mut seen: Vec<String> = Vec::new();
        for v in blocking {
            let sig = v.signature();
            if seen.contains(&sig) {
                continue;
            }
            seen.push(sig.clone());
            blocking_sigs.push(sig.clone());
            eprintln!("money-fuzz: shrinking NEW violation {sig} ...");
            let minimal = shrink(seed, &ops, &config, &actors, &sig, shrink_budget);
            eprintln!(
                "money-fuzz: shrunk {} ops -> {} ops",
                ops.len(),
                minimal.len()
            );
            reproducers.push(Reproducer {
                signature: sig,
                seed,
                ops: minimal,
                violation: v,
            });
        }

        runs.push(report);
    }

    documented_sigs.sort();
    documented_sigs.dedup();

    let report = FuzzReport {
        harness: "tests/money_safety -- ClearDeck money-safety harness (M1..M6)",
        ledger: "REAL mainnet ICP ledger installed at ryjl3-tyaaa-aaaaa-aaaba-cai",
        ledger_sha256: wasms::ICP_LEDGER_SHA256,
        runs,
        minimal_reproducers: reproducers.clone(),
        totals: Totals {
            runs: seed_list.len(),
            steps: total_steps,
            blocking_signatures: blocking_sigs,
            documented_signatures: documented_sigs,
        },
    };
    let path = write_report(&report);
    eprintln!("money-fuzz: report written to {}", path.display());

    assert!(
        reproducers.is_empty(),
        "the fuzzer found {} invariant violation(s) that are NOT documented defects. Minimal \
         reproducers are in {}. Signatures:\n{}",
        reproducers.len(),
        path.display(),
        reproducers
            .iter()
            .map(|r| format!(
                "  {} (seed {:#x}, {} ops) -- {}",
                r.signature,
                r.seed,
                r.ops.len(),
                r.violation.detail
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
