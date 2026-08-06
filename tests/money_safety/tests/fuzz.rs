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
        eprintln!(
            "money-fuzz: seed {seed:#x} M8 ATTRIBUTION: {} hand(s) finished, {} measured by \
             principal, {} of those also checked against the independent settlement oracle{}",
            report.attribution.hands_seen,
            report.attribution.hands_measured,
            report.attribution.hands_oracle_checked,
            if report.attribution.declined.is_empty() {
                String::new()
            } else {
                format!(
                    "; declined: {:?}",
                    report.attribution.declined
                )
            }
        );
        eprintln!(
            "money-fuzz: seed {seed:#x} M11 OUTCOME: {} hand(s) had their OUTCOME checked (of {} \
             the watch saw); {} of those reached a decided-by-fold-out moment and had the sharp \
             leg run; the structural leg ran on {} step(s){}",
            report.outcome.hands_outcome_checked,
            report.outcome.hands_seen,
            report.outcome.hands_foldout_checked,
            report.outcome.steps_structurally_checked,
            if report.outcome.declined.is_empty() {
                String::new()
            } else {
                format!("; declined: {:?}", report.outcome.declined)
            }
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
            .filter(|v| !money_safety::documented::is_documented(v))
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
        table_wasm_sha256: wasms::table_canister_sha256(),
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

    // THE INSTRUMENT MUST HAVE SPOKEN.
    //
    // M8 declines hands it cannot attribute -- somebody deposited mid-hand, the
    // harness did not see enough of the sequence -- and a declining gate is
    // indistinguishable from a passing one unless somebody counts. Half of the
    // hands a run completes is a low bar and it is deliberately low: the point is
    // to fail loudly if the recorder ever stops reconstructing stakes at all,
    // which is what would happen if the engine changed how `total_bet_this_hand`
    // or `departed_stakes` behave.
    let hands_seen: u64 = report.runs.iter().map(|r| r.attribution.hands_seen).sum();
    let measured: u64 = report.runs.iter().map(|r| r.attribution.hands_measured).sum();
    let oracled: u64 = report
        .runs
        .iter()
        .map(|r| r.attribution.hands_oracle_checked)
        .sum();
    eprintln!(
        "money-fuzz: M8 PRINCIPAL ATTRIBUTION ran on {measured} of the {hands_seen} hand(s) this \
         run completed; {oracled} of them were also checked against the independent settlement \
         oracle"
    );
    assert!(
        hands_seen == 0 || measured * 2 >= hands_seen,
        "M8 PRINCIPAL ATTRIBUTION could only be measured on {measured} of {hands_seen} completed \
         hands. The gate is not measuring what it claims to; see the per-run `declined` counts in \
         {} and the reconstruction in tests/money_safety/src/hand_attribution.rs.",
        path.display()
    );
    assert!(
        hands_seen < 4 || oracled > 0,
        "{hands_seen} hands completed and NOT ONE was checked against the independent settlement \
         oracle. The three oracle-free legs compare the canister against its own record, which a \
         self-consistent misdirection -- a plan that names the wrong live player -- passes."
    );

    // M11 OUTCOME MUST HAVE SPOKEN TOO.
    //
    // Its structural leg is per STEP and needs no reconstruction at all, so it
    // cannot decline: if it ran on no steps, either no hand was ever live or the
    // watch has stopped observing. Either way the instrument is not measuring and
    // saying so is the whole lesson of docs/DEFECTS.md H-03.
    let outcome_steps: u64 = report
        .runs
        .iter()
        .map(|r| r.outcome.steps_structurally_checked)
        .sum();
    let outcome_hands: u64 = report
        .runs
        .iter()
        .map(|r| r.outcome.hands_outcome_checked)
        .sum();
    let foldout_hands: u64 = report
        .runs
        .iter()
        .map(|r| r.outcome.hands_foldout_checked)
        .sum();
    eprintln!(
        "money-fuzz: M11 OUTCOME ran its structural leg on {outcome_steps} step(s) and settled \
         the OUTCOME of {outcome_hands} hand(s); {foldout_hands} of those were decided by \
         fold-out and had the sharp leg run"
    );
    assert!(
        hands_seen == 0 || outcome_steps > 0,
        "{hands_seen} hands completed and M11 OUTCOME's structural leg ran on NOT ONE step. It \
         reads the snapshot the loop already takes and declines only for a hand somebody walked \
         out of, so zero means it has stopped observing. See the per-run `outcome` block in {}.",
        path.display()
    );

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
