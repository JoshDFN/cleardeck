//! Locating and BUILDING the two spike modules, so the harness tests the code in
//! the tree and not a stale artifact.
//!
//! The rule is the one `tests/money_safety/src/wasms.rs` states at length and this
//! project has already been burned by (DEFECTS H-01, H-21): the harness both
//! builds into a path and reads from that path, so "the artifact" and "the artifact
//! under test" cannot be two different files, and the environment cannot be used to
//! steer the compiler out from under it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Environment variables that change what `cargo build` produces or where it puts
/// it. Scrubbed for the reason `tests/money_safety` scrubs them: `RUSTUP_TOOLCHAIN`
/// overrides `rust-toolchain.toml`, and cargo exports it to every child process, so
/// running this harness from inside another cargo invocation would silently compile
/// the canisters with a different compiler. `CARGO_TARGET_DIR` moves the OUTPUT,
/// which would leave this file reading whatever stale bytes were already there.
const SCRUBBED_EXACT: &[&str] = &[
    "RUSTUP_TOOLCHAIN",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "CARGO",
    "CARGO_ENCODED_RUSTFLAGS",
];

const SCRUBBED_PREFIXES: &[&str] = &["CARGO_BUILD_", "CARGO_PROFILE_", "CARGO_TARGET_"];

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live at <repo>/tests/no_peeking")
        .to_path_buf()
}

pub fn spike_root() -> PathBuf {
    repo_root().join("src").join("no_peeking")
}

fn build_once() -> &'static PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let root = spike_root();
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&root)
            .args(["build", "--release", "--target", "wasm32-unknown-unknown"]);
        for k in SCRUBBED_EXACT {
            cmd.env_remove(k);
        }
        let to_remove: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| SCRUBBED_PREFIXES.iter().any(|p| k.starts_with(p)))
            .collect();
        for k in to_remove {
            cmd.env_remove(k);
        }
        let status = cmd.status().expect("cargo build could not be started");
        assert!(
            status.success(),
            "cargo build of the no-peeking spike failed; run it by hand in {}",
            root.display()
        );
        root.join("target").join("wasm32-unknown-unknown").join("release")
    })
}

fn module(name: &str) -> Vec<u8> {
    let path = build_once().join(format!("{name}.wasm"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cargo reported success but {} could not be read: {e}",
            path.display()
        )
    })
}

pub fn dealer_wasm() -> Vec<u8> {
    module("dealer_canister")
}

pub fn table_stub_wasm() -> Vec<u8> {
    module("table_stub")
}

/// The LIVE table canister's module, if it has already been built.
///
/// Used only by the differential half of `tests/no_peek.rs`, which reads the
/// deployed engine's own surface to show what the finding is about. Absent is not
/// a failure: that half reports itself skipped rather than pretending the
/// comparison happened.
pub fn live_table_wasm() -> Option<Vec<u8>> {
    let path = repo_root()
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("table_canister.wasm");
    std::fs::read(path).ok()
}
