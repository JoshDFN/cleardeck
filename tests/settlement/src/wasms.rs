//! Building, IDENTIFYING and FRESHNESS-CHECKING the two wasm modules the harness
//! executes.
//!
//! # Trap 1, in full
//!
//! The money-safety harness that came before this one resolved the table canister
//! wasm as "use `target/wasm32-unknown-unknown/release/table_canister.wasm` if it
//! exists, otherwise build". With a stale-but-present artifact it had therefore
//! NEVER compiled the canister it claimed to be testing. That was proven the hard
//! way: with a ten-times-credit fund-theft bug planted in `lib.rs` and a pristine
//! wasm on disk, all ten money invariants reported ok; deleting that one file made
//! the identical source go seven-of-ten red.
//!
//! An oracle whose whole purpose is to be believed when it disagrees with the
//! engine cannot be exposed to that. So this module:
//!
//! 1. **always builds**, unless `$CLEARDECK_TABLE_WASM` explicitly overrides it;
//! 2. **freshness-checks** whatever it ends up with against the mtime of every
//!    Rust source and manifest that goes into it, and refuses to run if any input
//!    is newer than the artifact;
//! 3. **prints the sha256** of the module once per process, so the number is in
//!    the test output next to the findings;
//! 4. is paired with an assertion in [`crate::world`] that the module hash the
//!    replica reports for the installed canister equals the sha256 of the bytes
//!    this module handed it. Freshness of the file plus identity of the installed
//!    module closes the loop.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::SystemTime;

/// Pinned build of the mainnet ICP ledger (`ledger-canister.wasm.gz`).
///
/// Provenance, so a reader can re-derive it:
///   URL    https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz
///   sha256 a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec
///
/// Same pin as `tests/money_safety`, deliberately: two harnesses disagreeing about
/// which ledger is "the" ledger would be a needless variable.
pub const ICP_LEDGER_SHA256: &str =
    "a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec";

pub const ICP_LEDGER_URL: &str = "https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz";

/// Repo root, derived from this crate's manifest dir (`<repo>/tests/settlement`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live at <repo>/tests/settlement")
        .to_path_buf()
}

/// Cache for downloaded artifacts. Inside `target/`, which is gitignored, so the
/// harness never adds a binary to the repo.
pub fn cache_dir() -> PathBuf {
    let dir = repo_root().join("target").join("settlement-oracle");
    std::fs::create_dir_all(&dir).expect("cannot create harness cache dir");
    dir
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// the table canister
// ---------------------------------------------------------------------------

/// A wasm module together with the identity of the exact bytes.
#[derive(Clone)]
pub struct Module {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub path: PathBuf,
}

static TABLE_MODULE: OnceLock<Module> = OnceLock::new();

/// The table canister module under test, built from the checked-out source.
///
/// Built once per test process and memoised, so N `World`s in one binary all run
/// provably the same bytes.
pub fn table_canister_module() -> &'static Module {
    TABLE_MODULE.get_or_init(|| {
        let path = match std::env::var("CLEARDECK_TABLE_WASM") {
            Ok(p) => {
                // An explicit override still gets the freshness check. The only
                // legitimate use is `scripts/dev.sh`, which has just built it.
                PathBuf::from(p)
            }
            Err(_) => build_table_canister(),
        };
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("table canister wasm {} unreadable: {e}", path.display()));
        assert_fresh(&path);
        let sha256 = sha256_hex(&bytes);
        eprintln!(
            "\n[settlement-oracle] table_canister wasm under test\n\
             [settlement-oracle]   path   {}\n\
             [settlement-oracle]   bytes  {}\n\
             [settlement-oracle]   sha256 {}\n",
            path.display(),
            bytes.len(),
            sha256
        );
        Module {
            bytes,
            sha256,
            path,
        }
    })
}

/// Environment variables that change what `cargo build` produces, or where it
/// puts it. The harness scrubs every one of them rather than inheriting it.
///
/// docs/DEFECTS.md H-21: `RUSTUP_TOOLCHAIN` OVERRIDES `rust-toolchain.toml` and
/// cargo exports it to every child process, so running this harness from inside
/// another cargo invocation compiled the canister with a different compiler and
/// produced a different module hash from identical source. `CARGO_TARGET_DIR`
/// is worse: the build would succeed somewhere else while the path read and
/// hashed below kept whatever stale bytes were already there.
pub const SCRUBBED_EXACT: &[&str] = &[
    "RUSTUP_TOOLCHAIN",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO",
    "CARGO_ENCODED_RUSTFLAGS",
];

pub const SCRUBBED_PREFIXES: &[&str] = &[
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
    "CARGO_UNSTABLE_",
];

/// `channel = "1.90.0"` out of a `rust-toolchain.toml`.
pub fn parse_toolchain_channel(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        // `continue`, not `?`: a manifest is mostly lines that are not the pin, and
        // returning None from the first of them would report every rust-toolchain
        // file in the repo as having no channel.
        let Some(rest) = line.strip_prefix("channel") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = rest.trim().trim_matches(|c| c == '"' || c == '\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// The toolchain the repo pins, read from `rust-toolchain.toml`.
pub fn pinned_toolchain(dir: &Path) -> String {
    let path = dir.join("rust-toolchain.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parse_toolchain_channel(&text)
        .unwrap_or_else(|| panic!("{} has no channel to pin", path.display()))
}

/// The table-canister build, with the environment PINNED rather than inherited.
pub fn pinned_table_canister_build(dir: &Path) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir).args([
        "build",
        "-p",
        "table_canister",
        "--target",
        "wasm32-unknown-unknown",
        "--release",
    ]);
    for key in SCRUBBED_EXACT {
        cmd.env_remove(key);
    }
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy().to_string();
        if SCRUBBED_PREFIXES.iter().any(|p| name.starts_with(p)) {
            cmd.env_remove(&name);
        }
    }
    cmd.env("RUSTUP_TOOLCHAIN", pinned_toolchain(dir));
    cmd
}

fn build_table_canister() -> PathBuf {
    let root = repo_root();
    let toolchain = pinned_toolchain(&root);
    let status = pinned_table_canister_build(&root)
        .status()
        .expect("could not run cargo to build the table canister");
    assert!(
        status.success(),
        "building table_canister for wasm32-unknown-unknown with the pinned toolchain \
         {toolchain} failed. The settlement oracle refuses to fall back to an existing \
         artifact: see the trap-1 note at the top of src/wasms.rs."
    );
    let built = root
        .join("target/wasm32-unknown-unknown/release")
        .join("table_canister.wasm");
    assert!(
        built.exists(),
        "cargo reported success but produced no {}",
        built.display()
    );
    built
}

/// Every path whose content ends up inside `table_canister.wasm`.
fn build_inputs() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = vec![
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
        root.join("src/table_canister/Cargo.toml"),
        root.join("src/poker_core/Cargo.toml"),
    ];
    for dir in [
        root.join("src/table_canister/src"),
        root.join("src/poker_core/src"),
    ] {
        collect_rs(&dir, &mut out);
    }
    out.retain(|p| p.exists());
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

/// Refuse to run against a wasm older than any of its sources.
///
/// `$SETTLEMENT_SKIP_FRESHNESS=1` exists only so a reader can deliberately test a
/// module that is NOT the current tree (for instance a saved baseline build), and
/// it says so loudly when used.
pub fn assert_fresh(wasm: &Path) {
    if std::env::var("SETTLEMENT_SKIP_FRESHNESS").as_deref() == Ok("1") {
        eprintln!(
            "[settlement-oracle] WARNING: SETTLEMENT_SKIP_FRESHNESS=1. The module at {} \
             is NOT being checked against the source tree. Any result attributed to \
             'the source tree' from this run is unsupported.",
            wasm.display()
        );
        return;
    }
    let Some(wasm_time) = mtime(wasm) else {
        panic!("cannot stat {} to check freshness", wasm.display());
    };
    let mut newer: Vec<(PathBuf, SystemTime)> = Vec::new();
    for input in build_inputs() {
        if let Some(t) = mtime(&input) {
            if t > wasm_time {
                newer.push((input, t));
            }
        }
    }
    assert!(
        newer.is_empty(),
        "STALE WASM. {} is older than {} of its build inputs, so this run would test \
         code that is not in the tree -- exactly the failure that made an earlier \
         harness report 10/10 green while a fund-theft bug sat in lib.rs.\n\
         Newer than the wasm:\n{}\n\
         Fix it with `./scripts/dev.sh wasm` (or just unset $CLEARDECK_TABLE_WASM and \
         let this harness build).",
        wasm.display(),
        newer.len(),
        newer
            .iter()
            .map(|(p, _)| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---------------------------------------------------------------------------
// the ICP ledger
// ---------------------------------------------------------------------------

pub fn icp_ledger_wasm() -> Vec<u8> {
    if let Ok(path) = std::env::var("ICP_LEDGER_WASM") {
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("ICP_LEDGER_WASM={path} could not be read: {e}"));
        assert_pinned(&bytes, &path);
        return bytes;
    }

    // Share the money-safety cache when it is already populated: the bytes are
    // hash-checked either way, so there is nothing to gain from downloading twice.
    for cached in [
        cache_dir().join("ledger-canister.wasm.gz"),
        repo_root().join("target/money-safety/ledger-canister.wasm.gz"),
    ] {
        if cached.exists() {
            if let Ok(bytes) = std::fs::read(&cached) {
                if sha256_hex(&bytes) == ICP_LEDGER_SHA256 {
                    return bytes;
                }
            }
        }
    }

    let bytes = download(ICP_LEDGER_URL);
    assert_pinned(&bytes, ICP_LEDGER_URL);
    std::fs::write(cache_dir().join("ledger-canister.wasm.gz"), &bytes)
        .expect("cannot write ledger wasm cache");
    bytes
}

fn assert_pinned(bytes: &[u8], origin: &str) {
    let got = sha256_hex(bytes);
    assert_eq!(
        got, ICP_LEDGER_SHA256,
        "ICP ledger wasm from {origin} does not match the pinned sha256. The harness \
         refuses to run against an unidentified ledger."
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
         target/settlement-oracle/ledger-canister.wasm.gz, or point $ICP_LEDGER_WASM at it."
    );
    let bytes = std::fs::read(&out).expect("download produced no file");
    let _ = std::fs::remove_file(&out);
    bytes
}

// ---------------------------------------------------------------------------
// the PocketIC server
// ---------------------------------------------------------------------------

/// Path to a PocketIC **server** binary compatible with the `pocket-ic` crate.
///
/// `$POCKET_IC_BIN` wins (the crate reads it itself); otherwise the dfx cache
/// copy, which is pocket-ic-server 11.0.0 on this machine.
pub fn pocket_ic_binary() -> Option<PathBuf> {
    if std::env::var("POCKET_IC_BIN").is_ok() {
        return None;
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
