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
use money_safety::wasms;

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

/// THE GATE ON docs/DEFECTS.md H-26, AND IT NEEDS NO REPLICA.
///
/// For eleven waves the table shape was `config_for(i)` where `i` was the seed's
/// INDEX IN `MONEY_FUZZ_SEEDS`, while every reproducer this harness printed,
/// wrote into `money-fuzz-report.json` and got quoted by in docs/DEFECTS.md named
/// the SEED and nothing else. So the documented reproducer command replayed a
/// different game from the one that found the violation, and the register's
/// standing observation -- "seed 212967420072194 finds nothing alone and two
/// fund-creation findings when 212967420072193 ran before it" -- was that defect,
/// not a nondeterministic replica.
///
/// This test asks the only question that matters: does moving a seed change what
/// it plays. It runs in milliseconds, so there is no excuse for it not being in
/// the same binary as the fuzzer it guards.
#[test]
fn a_seeds_table_shape_does_not_depend_on_where_it_appears_in_the_list() {
    // Seeds spanning the three shapes, the two the register names, the smoke
    // row's seed, and the three defaults.
    let seeds: [u64; 9] = [
        0,
        1,
        2,
        212_967_420_072_193,
        212_967_420_072_194,
        0xC1EA_2DEC_0001,
        0xC1EA_2DEC_0002,
        0xC1EA_2DEC_0003,
        u64::MAX,
    ];
    for seed in seeds {
        let first = run_shape(0, seed);
        for position in 1..6 {
            let moved = run_shape(position, seed);
            assert_eq!(
                first, moved,
                "seed {seed:#x} plays a DIFFERENT GAME at position {position} than at position 0: \
                 {} vs {}. A reproducer that names only the seed therefore does not reproduce, \
                 which is docs/DEFECTS.md H-26.",
                first.shape, moved.shape
            );
            // The generated sequence is what actually reaches the canister, so
            // assert on that too rather than trusting that equal shapes imply
            // equal ops.
            let a = generate(seed, 60, &first.config, first.actors.len());
            let b = generate(seed, 60, &moved.config, moved.actors.len());
            assert_eq!(
                a, b,
                "seed {seed:#x} generates a different op sequence at position {position}"
            );
        }
    }

    // And the shapes are still all reachable: a mapping that collapsed every seed
    // onto one table would pass everything above and test a third as much.
    let reached: std::collections::BTreeSet<&str> =
        (0u64..64).map(|s| run_shape(0, s).shape).collect();
    assert_eq!(
        reached.len(),
        SHAPES.len(),
        "the seed -> shape mapping no longer reaches all of {SHAPES:?}; it reached {reached:?}"
    );
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
        // docs/DEFECTS.md H-26: `i` is passed so the gate above has an argument to
        // vary, and `run_shape` ignores it. The shape is the seed's, not the
        // position's, which is what makes `MONEY_FUZZ_SEEDS=<seed>` a reproducer.
        let shape = run_shape(i, seed);
        let config = shape.config.clone();
        let actors: Vec<&str> = shape.actors.clone();
        let ops = generate(seed, steps, &config, actors.len());

        eprintln!(
            "money-fuzz: seed {seed:#x}, shape {}, {} ops, {}-max table, ante {} -- replay with: {}",
            shape.shape,
            ops.len(),
            config.max_players,
            config.ante,
            replay_command(seed, steps)
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
        // M12 ARCHIVE FIDELITY. Printed for the same reason M8 and M11 are: an
        // instrument that quietly declines to measure is indistinguishable from
        // one that passes, and the number that matters here is the last one --
        // a run with no mid-hand departure has not exercised FINDING 30 at all.
        eprintln!(
            "money-fuzz: seed {seed:#x} {}",
            report.archive.summary()
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
                shape: shape.shape,
                replay: replay_command(seed, steps),
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
                "  {} (seed {:#x}, shape {}, {} ops) -- {}\n    replay: {}",
                r.signature,
                r.seed,
                r.shape,
                r.ops.len(),
                r.violation.detail,
                r.replay
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
