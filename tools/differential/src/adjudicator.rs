//! Third reference, consulted as an adjudicator: `phevaluator` over a pipe.
//!
//! The harness convicts ClearDeck only when BOTH in-process references agree
//! against it. That rule is only as good as the claim that the two references are
//! genuinely independent, so a third one, from a third lineage, audits them:
//! it re-ranks a seeded sample and the ordering of `rs_poker` and `poker` must
//! match it exactly.
//!
//! It is optional by design. `cargo run --release` must work on a machine with no
//! Python at all, so an absent interpreter downgrades the report to
//! `adjudicator.available = false` instead of failing.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::cards::{slice_to_string, CardIdx};
use crate::classes::{compare_orderings, RankOrderIndex};
use crate::describe::FiveWitness;
use crate::oracles::poker_oracle::{PokerCrateOracle, PokerStrength};
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::AdjudicatorSummary;
use crate::rng::SplitMix64;

/// Env var holding the path to a Python interpreter with `phevaluator` installed.
pub const PYTHON_ENV_VAR: &str = "CLEARDECK_PHE_PYTHON";

/// phevaluator ranks 1 (best) .. 7462 (worst); negate so greater is better,
/// matching every other strength in the harness.
fn to_strength(rank: i64) -> i64 {
    -rank
}

fn script_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("oracle3")
        .join("phe_oracle.py")
}

/// Hands per child invocation. Bounds memory on both sides of the pipe; the
/// child buffers its entire answer until stdin EOF, so an unbounded write would
/// risk filling its 64 KiB stdout pipe and deadlocking.
pub const CHUNK: usize = 250_000;

/// Ranks any number of hands through the third reference, chunked. Returns one
/// score per input hand, in order, where **1 is the best possible hand**.
pub fn rank_hands(python: &str, hands: &[String]) -> Result<Vec<i64>, String> {
    let mut out = Vec::with_capacity(hands.len());
    for chunk in hands.chunks(CHUNK) {
        out.extend(ask_python(python, chunk)?);
    }
    Ok(out)
}

fn ask_python(python: &str, hands: &[String]) -> Result<Vec<i64>, String> {
    let script = script_path();
    let mut child = Command::new(python)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn {python} {}: {e}", script.display()))?;

    {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "child stdin unavailable".to_string())?;
        // Buffered: one syscall per hand over millions of hands dominated the cost.
        let mut writer = std::io::BufWriter::with_capacity(1 << 16, stdin);
        for hand in hands {
            writeln!(writer, "{hand}").map_err(|e| format!("write to child failed: {e}"))?;
        }
        writer
            .flush()
            .map_err(|e| format!("flush to child failed: {e}"))?;
        // Dropping the writer closes stdin, which is the child's EOF signal.
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("child wait failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || stdout.starts_with("ERROR ") {
        return Err(format!(
            "adjudicator exited {:?}: {}{}",
            out.status.code(),
            stdout.lines().next().unwrap_or("").trim(),
            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")
        ));
    }

    let scores: Result<Vec<i64>, String> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.trim()
                .parse::<i64>()
                .map_err(|e| format!("unparseable adjudicator output {l:?}: {e}"))
        })
        .collect();
    let scores = scores?;
    if scores.len() != hands.len() {
        return Err(format!(
            "adjudicator returned {} scores for {} hands",
            scores.len(),
            hands.len()
        ));
    }
    Ok(scores)
}

/// Cross-checks both in-process references against `phevaluator` over a seeded
/// sample of five-card hands.
pub fn audit_references(seed: u64, sample: usize) -> AdjudicatorSummary {
    let Ok(python) = std::env::var(PYTHON_ENV_VAR) else {
        return AdjudicatorSummary {
            available: false,
            detail: format!(
                "{PYTHON_ENV_VAR} not set; the two in-process references were not independently audited. \
                 See tools/differential/oracle3/phe_oracle.py for the one-line setup."
            ),
            ..Default::default()
        };
    };

    let mut rng = SplitMix64::new(seed);
    let hands: Vec<[CardIdx; 5]> = (0..sample)
        .map(|_| {
            let d = rng.deal(5);
            [d[0], d[1], d[2], d[3], d[4]]
        })
        .collect();
    let rendered: Vec<String> = hands.iter().map(|h| slice_to_string(h)).collect();

    let scores = match rank_hands(&python, &rendered) {
        Ok(s) => s,
        Err(e) => {
            return AdjudicatorSummary {
                available: false,
                detail: format!("adjudicator unusable: {e}"),
                ..Default::default()
            }
        }
    };

    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();

    let mut phe_idx: RankOrderIndex<i64> = RankOrderIndex::new();
    let mut rs_idx: RankOrderIndex<rs_poker::core::Rank> = RankOrderIndex::new();
    let mut pk_idx: RankOrderIndex<PokerStrength> = RankOrderIndex::new();

    let mut phe_ins = Vec::with_capacity(hands.len());
    let mut rs_ins = Vec::with_capacity(hands.len());
    let mut pk_ins = Vec::with_capacity(hands.len());

    for (i, hand) in hands.iter().enumerate() {
        phe_ins.push(phe_idx.intern(&to_strength(scores[i])));
        rs_ins.push(rs_idx.intern(&rs.eval5(hand).strength));
        pk_ins.push(pk_idx.intern(&pk.eval5(hand).strength));
    }

    let phe_classes = phe_idx.len();
    let rs_classes = rs_idx.len();
    let pk_classes = pk_idx.len();
    let (phe_order, _) = phe_idx.finish();
    let (rs_order, _) = rs_idx.finish();
    let (pk_order, _) = pk_idx.finish();

    let rs_vs_phe = compare_orderings(
        (0..hands.len()).map(|i| {
            (
                rs_order[rs_ins[i] as usize],
                phe_order[phe_ins[i] as usize],
                FiveWitness(crate::cards::pack5(&hands[i])),
            )
        }),
        rs_classes,
        phe_classes,
    );
    let pk_vs_phe = compare_orderings(
        (0..hands.len()).map(|i| {
            (
                pk_order[pk_ins[i] as usize],
                phe_order[phe_ins[i] as usize],
                FiveWitness(crate::cards::pack5(&hands[i])),
            )
        }),
        pk_classes,
        phe_classes,
    );

    AdjudicatorSummary {
        available: true,
        detail: format!(
            "phevaluator via {python}; {} distinct classes observed in the sample",
            phe_classes
        ),
        hands_checked: hands.len() as u64,
        disagreements_with_rs_poker: rs_vs_phe.violations.len() as u64,
        disagreements_with_poker: pk_vs_phe.violations.len() as u64,
    }
}
