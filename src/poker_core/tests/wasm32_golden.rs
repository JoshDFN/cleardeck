//! Replays the golden vectors on **wasm32-unknown-unknown**, the target the table
//! canister is actually compiled to.
//!
//! # Why this test exists
//!
//! `usize` is 32 bits on wasm32 and 64 bits on every machine a developer tests on.
//! The shipped shuffle reduced its 64-bit draw with `(draw as usize) % (i + 1)`, so
//! the canister dealt from a different deck than the one every native test, every
//! native golden vector and every third-party reimplementation computed from the
//! same seed. The whole "anyone can verify" claim was false, and 101 green native
//! tests said nothing about it. See `docs/FINDING-02-shuffle-not-verifiable.md`.
//!
//! The general rule this test enforces: **any pure function whose result feeds the
//! deal or the money is tested on the target that runs it, not only on the host.**
//! It replays all ~15,700 vectors (shuffle, hand evaluation, straight detection and
//! side pots) inside a wasm module, using the same runner code as the native test
//! in `tests/golden_vectors.rs` so the two cannot drift.
//!
//! # How it runs
//!
//! `tests/wasm32_harness/` is a `cdylib` crate for `wasm32-unknown-unknown`
//! (deliberately outside the root workspace) that embeds `golden_vectors.txt` and
//! exposes a tiny C ABI. `tests/wasm32_harness/run.mjs` instantiates it with Node's
//! plain `WebAssembly` API -- no wasm-bindgen, no WASI, no runtime to install; the
//! repo already needs Node for the frontend. This test drives both.
//!
//! # Regenerating the shuffle vectors
//!
//! ```text
//! CLEARDECK_REGENERATE_SHUFFLE_VECTORS=1 cargo test -p poker_core --test wasm32_golden -- --nocapture
//! ```
//!
//! That rewrites ONLY the `S ` lines of `golden_vectors.txt`, computed inside the
//! wasm module, and leaves every other family byte-identical. It is a deliberate
//! act: it changes the fairness commitment. Read `docs/SHUFFLE-SPEC.md` first.

mod common;

use common::golden::{
    ALPHA, EXPECTED_FIVE_CARD, EXPECTED_SEVEN_CARD, EXPECTED_SHUFFLE, EXPECTED_SIDE_POTS,
    EXPECTED_STRAIGHT,
};
use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

fn poker_core_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    // <repo>/src/poker_core -> <repo>
    poker_core_dir()
        .parent()
        .and_then(Path::parent)
        .expect("poker_core must live at <repo>/src/poker_core")
        .to_path_buf()
}

fn harness_dir() -> PathBuf {
    poker_core_dir().join("tests/wasm32_harness")
}

fn golden_path() -> PathBuf {
    poker_core_dir().join("tests/golden_vectors.txt")
}

/// Builds the harness for wasm32 once per test binary and returns the module path.
///
/// Tests in this file run in parallel threads of one process, and cargo serialises
/// concurrent builds on a lock, so building once keeps the output readable.
fn build_harness() -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUILT.get_or_init(build_harness_uncached).clone()
}

/// Uses a target directory of its own: the outer `cargo test` owns `<repo>/target`,
/// and a nested build there would block on its lock.
fn build_harness_uncached() -> PathBuf {
    let target_dir = repo_root().join("target/wasm32-shuffle-harness");
    let status = Command::new(env!("CARGO"))
        .current_dir(harness_dir())
        .env("CARGO_TARGET_DIR", &target_dir)
        // Never inherit host RUSTFLAGS into a cross build.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET")
        .args(["build", "--release", "--target", TARGET])
        .status()
        .unwrap_or_else(|e| panic!("could not run cargo to build the wasm32 harness: {e}"));
    assert!(
        status.success(),
        "building the wasm32 harness failed. The {TARGET} target is required:\n  \
         rustup target add {TARGET}"
    );

    let wasm = target_dir
        .join(TARGET)
        .join("release/wasm32_shuffle_harness.wasm");
    assert!(wasm.is_file(), "harness wasm not found at {}", wasm.display());
    wasm
}

/// Runs `run.mjs <wasm> <mode> [arg]` and returns its stdout, asserting success.
fn run_harness(wasm: &Path, mode: &str, arg: Option<&str>) -> String {
    let mut cmd = Command::new("node");
    cmd.arg(harness_dir().join("run.mjs")).arg(wasm).arg(mode);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("could not run node (required for the wasm32 test): {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "wasm32 harness mode '{mode}' failed ({}):\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
        out.status
    );
    stdout
}

/// The whole point: every committed vector, replayed on wasm32.
#[test]
fn golden_vectors_reproduce_on_wasm32() {
    let wasm = build_harness();
    let stdout = run_harness(&wasm, "check", None);
    println!("{stdout}");

    // A green run that checked nothing is the failure mode to fear here, so pin
    // the population as reported from INSIDE the module.
    let counts = stdout
        .lines()
        .find(|l| l.starts_with("counts:"))
        .expect("harness must report its vector counts");
    let expected = format!(
        "counts: S={} E={} F={} D={} P={}",
        EXPECTED_SHUFFLE,
        EXPECTED_SEVEN_CARD,
        EXPECTED_FIVE_CARD,
        EXPECTED_STRAIGHT,
        EXPECTED_SIDE_POTS
    );
    assert_eq!(
        counts, expected,
        "the wasm32 run did not replay the expected number of vectors"
    );
}

/// The pre-shuffle deck order is an input to the fairness commitment, so pin it on
/// this target too, not only on the host.
#[test]
fn create_deck_order_is_pinned_on_wasm32() {
    let wasm = build_harness();
    let stdout = run_harness(&wasm, "deck-order", None);
    assert_eq!(
        stdout.trim(),
        std::str::from_utf8(ALPHA).unwrap(),
        "create_deck() order differs inside the wasm module"
    );
}

/// Regenerates the `S` family from wasm32 execution.
///
/// Runs as a normal test so it cannot rot: without the env var it *verifies* that
/// the committed `S` lines are exactly what wasm32 produces (a stricter statement
/// than `run_golden`, because it compares the whole line including the seed and its
/// order in the file); with the env var it rewrites them.
#[test]
fn shuffle_vectors_are_what_wasm32_produces() {
    let wasm = build_harness();
    let regenerated = run_harness(&wasm, "regenerate", None);
    let fresh: Vec<&str> = regenerated.lines().collect();
    assert_eq!(
        fresh.len(),
        EXPECTED_SHUFFLE,
        "harness emitted {} shuffle lines",
        fresh.len()
    );

    let golden = std::fs::read_to_string(golden_path()).expect("read golden_vectors.txt");
    let committed: Vec<&str> = golden.lines().filter(|l| l.starts_with("S ")).collect();

    if std::env::var_os("CLEARDECK_REGENERATE_SHUFFLE_VECTORS").is_some() {
        let mut out = String::with_capacity(golden.len());
        let mut fresh_iter = fresh.iter();
        let mut replaced = 0usize;
        for line in golden.lines() {
            if line.starts_with("S ") {
                out.push_str(fresh_iter.next().expect("one fresh line per committed line"));
                replaced += 1;
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        assert_eq!(replaced, EXPECTED_SHUFFLE);
        assert!(fresh_iter.next().is_none(), "leftover regenerated lines");
        std::fs::write(golden_path(), out).expect("rewrite golden_vectors.txt");
        println!(
            "REGENERATED {replaced} shuffle vectors from wasm32 execution. \
             Review the diff: this changes the fairness commitment."
        );
        return;
    }

    assert_eq!(
        committed.len(),
        EXPECTED_SHUFFLE,
        "golden_vectors.txt has {} S lines",
        committed.len()
    );
    let mismatches: Vec<String> = committed
        .iter()
        .zip(fresh.iter())
        .enumerate()
        .filter(|(_, (c, f))| c != f)
        .map(|(idx, (c, f))| format!("  vector {idx}\n    committed {c}\n    wasm32    {f}"))
        .collect();
    assert!(
        mismatches.is_empty(),
        "{} of {} committed shuffle vectors are not what wasm32 computes.\n{}\n\n\
         If the change to the shuffle is INTENDED, regenerate deliberately:\n  \
         CLEARDECK_REGENERATE_SHUFFLE_VECTORS=1 cargo test -p poker_core --test wasm32_golden",
        mismatches.len(),
        committed.len(),
        mismatches
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Cross-language check: two verifiers written from `docs/SHUFFLE-SPEC.md`, sharing
/// no code with `poker_core`, must reproduce the same 2,000 decks. This is what a
/// player does, so it is a test, not a demo. If the spec drifts from the code, this
/// is what catches it.
#[test]
fn outsider_reimplementations_reproduce_the_vectors() {
    let verify = poker_core_dir().join("tests/verify");
    let vectors = golden_path();

    for (label, mut cmd) in [
        ("python", {
            let mut c = Command::new("python3");
            c.arg(verify.join("verify_shuffle.py"));
            c
        }),
        ("node", {
            let mut c = Command::new("node");
            c.arg(verify.join("verify_shuffle.mjs"));
            c
        }),
    ] {
        let out = cmd
            .arg("--vectors")
            .arg(&vectors)
            .output()
            .unwrap_or_else(|e| panic!("could not run the {label} verifier: {e}"));
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        println!("[{label}] {}", stdout.trim());
        assert!(
            out.status.success(),
            "the {label} verifier disagreed with the committed vectors:\n{stdout}"
        );
        assert!(
            stdout.contains(&format!("checked {EXPECTED_SHUFFLE} shuffle vectors, 0 mismatched")),
            "the {label} verifier did not check all {EXPECTED_SHUFFLE} vectors:\n{stdout}"
        );
    }
}
