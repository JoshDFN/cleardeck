//! Locating, building and PINNING the two wasm modules the harness executes.
//!
//! Two hard requirements:
//!
//! * The table canister wasm must be built from the checked-out source, so the
//!   harness tests the code in the tree and not a stale artifact.
//! * The ledger must be the REAL ICP ledger, at the exact canister id the table
//!   canister hardcodes (`ICP_LEDGER_CANISTER`), so no test-only shim can hide a
//!   defect in the deposit/withdraw path. The module is pinned by sha256.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned build of the mainnet ICP ledger (`ledger-canister.wasm.gz`).
///
/// Provenance, recorded so a reader can re-derive it:
///   URL    https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz
///   sha256 a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec
///
/// `download.dfinity.systems/ic/<commit>/...` is immutable per commit. The commit
/// is the one the OISY wallet pins for its ICP ledger, i.e. a build that is in
/// real use, not a random nightly. Exports verified to include `icrc1_transfer`,
/// `icrc2_approve`, `icrc2_transfer_from`, `icrc1_balance_of` and `query_blocks`
/// -- every ledger method the table canister calls.
pub const ICP_LEDGER_SHA256: &str =
    "a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec";

pub const ICP_LEDGER_URL: &str = "https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz";

/// Repo root, derived from this crate's manifest dir (`<repo>/tests/money_safety`).
pub fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .expect("crate must live at <repo>/tests/money_safety")
        .to_path_buf()
}

/// Where downloaded/derived artifacts are cached. Inside `target/`, which is
/// already gitignored, so the harness never adds binaries to the repo.
pub fn cache_dir() -> PathBuf {
    let dir = repo_root().join("target").join("money-safety");
    std::fs::create_dir_all(&dir).expect("cannot create harness cache dir");
    dir
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// The REAL ICP ledger module, gzipped exactly as published.
///
/// Resolution order:
///   1. `$ICP_LEDGER_WASM` (an explicit path; still hash-checked unless
///      `$ICP_LEDGER_WASM_SKIP_HASH=1`, which exists only so a reader can try a
///      different ledger build on purpose).
///   2. the cache under `target/money-safety/`.
///   3. download from the pinned immutable URL.
pub fn icp_ledger_wasm() -> Vec<u8> {
    if let Ok(path) = std::env::var("ICP_LEDGER_WASM") {
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("ICP_LEDGER_WASM={path} could not be read: {e}"));
        if std::env::var("ICP_LEDGER_WASM_SKIP_HASH").as_deref() != Ok("1") {
            assert_pinned(&bytes, &path);
        }
        return bytes;
    }

    let cached = cache_dir().join("ledger-canister.wasm.gz");
    if cached.exists() {
        let bytes = std::fs::read(&cached).expect("cached ledger wasm unreadable");
        if sha256_hex(&bytes) == ICP_LEDGER_SHA256 {
            return bytes;
        }
        // A corrupt or superseded cache entry is not a reason to fail; re-fetch.
        let _ = std::fs::remove_file(&cached);
    }

    let bytes = download(ICP_LEDGER_URL);
    assert_pinned(&bytes, ICP_LEDGER_URL);
    std::fs::write(&cached, &bytes).expect("cannot write ledger wasm cache");
    bytes
}

fn assert_pinned(bytes: &[u8], origin: &str) {
    let got = sha256_hex(bytes);
    assert_eq!(
        got, ICP_LEDGER_SHA256,
        "ICP ledger wasm from {origin} does not match the pinned sha256. \
         The harness refuses to run against an unidentified ledger, because a \
         ledger that does not enforce real allowance/fee/balance semantics would \
         silently invalidate every M2 (LEDGER REALITY) result."
    );
}

fn download(url: &str) -> Vec<u8> {
    let out = cache_dir().join("download.tmp");
    let status = Command::new("curl")
        .args(["-sSL", "--fail", "--max-time", "180", "-o"])
        .arg(&out)
        .arg(url)
        .status()
        .unwrap_or_else(|e| panic!("could not run curl to fetch {url}: {e}"));
    assert!(
        status.success(),
        "curl failed to fetch {url}. Fetch it by hand into \
         target/money-safety/ledger-canister.wasm.gz, or point $ICP_LEDGER_WASM at it."
    );
    let bytes = std::fs::read(&out).expect("download produced no file");
    let _ = std::fs::remove_file(&out);
    bytes
}

/// The table canister module under test.
///
/// Resolution order:
///   1. `$CLEARDECK_TABLE_WASM`.
///   2. an existing release build in the shared `target/`.
///   3. `cargo build -p table_canister --target wasm32-unknown-unknown --release`,
///      into a harness-private target dir so it cannot collide with a build another
///      process is running in the shared one.
pub fn table_canister_wasm() -> Vec<u8> {
    if let Ok(path) = std::env::var("CLEARDECK_TABLE_WASM") {
        return std::fs::read(&path)
            .unwrap_or_else(|e| panic!("CLEARDECK_TABLE_WASM={path} could not be read: {e}"));
    }

    let shared = repo_root()
        .join("target/wasm32-unknown-unknown/release")
        .join("table_canister.wasm");
    if shared.exists() {
        return std::fs::read(&shared).expect("table_canister.wasm unreadable");
    }

    let private_target = cache_dir().join("cargo-target");
    let status = Command::new("cargo")
        .current_dir(repo_root())
        .env("CARGO_TARGET_DIR", &private_target)
        .args([
            "build",
            "-p",
            "table_canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .expect("could not run cargo to build the table canister");
    assert!(status.success(), "building table_canister for wasm32 failed");

    let built = private_target
        .join("wasm32-unknown-unknown/release")
        .join("table_canister.wasm");
    std::fs::read(&built).expect("cargo reported success but produced no table_canister.wasm")
}

/// Path to a PocketIC **server** binary compatible with the `pocket-ic` crate.
///
/// `POCKET_IC_BIN` wins; otherwise the dfx cache copy is used, which is the
/// version this harness was written against.
pub fn pocket_ic_binary() -> Option<PathBuf> {
    if std::env::var("POCKET_IC_BIN").is_ok() {
        return None; // the crate reads the env var itself
    }
    let dfx_cache = Command::new("dfx")
        .arg("cache")
        .arg("show")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let candidate = PathBuf::from(dfx_cache).join("pocket-ic");
    candidate.exists().then_some(candidate)
}
