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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

/// Where `cargo build -p table_canister --target wasm32-unknown-unknown --release`
/// puts the module, relative to the repo root. There is exactly one such path and
/// the harness both builds into it and reads from it, so "the artifact" and "the
/// artifact under test" cannot be two different files.
pub const TABLE_WASM_REL: &str = "target/wasm32-unknown-unknown/release/table_canister.wasm";

/// The table canister module under test, together with its identity.
#[derive(Debug)]
pub struct TableModule {
    pub bytes: Vec<u8>,
    /// Lowercase hex sha256 of `bytes`. This is also the IC module hash, so it can
    /// be compared against `canister_status().module_hash` after installation.
    pub sha256: String,
    pub path: PathBuf,
}

static TABLE_MODULE: OnceLock<TableModule> = OnceLock::new();

/// The module under test. Built from the checked-out source on first use, once
/// per test binary, and announced on fd 2 before anything else can run.
///
/// There is NO stale-artifact resolution path. `docs/DEFECTS.md` H-01: the old
/// resolution order preferred an existing `target/.../table_canister.wasm` with no
/// freshness check whatsoever, so with a 10x-credit fund-theft bug in `lib.rs` and
/// a pristine wasm at that path all ten invariant tests reported `ok`; deleting
/// that one file made the identical source go 7/10 red. A harness that can report
/// green about a binary it never built is worse than no harness.
pub fn table_canister_module() -> &'static TableModule {
    TABLE_MODULE.get_or_init(build_table_canister)
}

pub fn table_canister_wasm() -> Vec<u8> {
    table_canister_module().bytes.clone()
}

/// Lowercase hex sha256 of the module under test.
pub fn table_canister_sha256() -> &'static str {
    &table_canister_module().sha256
}

/// The git ref whose table canister counts as "the previous release" for
/// cross-version upgrade testing. `801aa79` is the wave-1 baseline: the last commit
/// before any wave-2 field was added to `PersistentState` or `TableState`.
pub const PREVIOUS_RELEASE_REF: &str = "801aa79";

static PREVIOUS_RELEASE: OnceLock<TableModule> = OnceLock::new();

/// The table canister as of [`PREVIOUS_RELEASE_REF`], built from source.
///
/// # Why this exists (docs/DEFECTS.md H-16, SECURITY-FINDINGS.md FINDING 14)
///
/// `World::upgrade` reuses `self.table_wasm`, so every "survives an upgrade"
/// assertion in this harness upgrades the new module TO ITSELF. Same Candid type on
/// both sides of the wire, so a record-field addition is never tested as an
/// addition -- and adding a field to a persisted record is the one upgrade change
/// that can silently destroy funds. Two wave-2 agents each added a field that is
/// not a Candid-compatible addition, and nothing in the suite could see it.
///
/// Built by `git archive <ref> | tar -x` into `target/money-safety/`, then compiled
/// there, rather than by checking a binary into the repo: a fixture nobody can
/// rebuild rots into a story about a lost file.
pub fn previous_release_table_canister() -> &'static TableModule {
    PREVIOUS_RELEASE.get_or_init(build_previous_release)
}

fn build_previous_release() -> TableModule {
    let root = repo_root();
    let tree = cache_dir().join(format!("old-release-{PREVIOUS_RELEASE_REF}"));
    let path = tree.join(TABLE_WASM_REL);

    if !tree.join("Cargo.toml").exists() {
        std::fs::create_dir_all(&tree).expect("cannot create the old-release tree");
        let archive = Command::new("git")
            .current_dir(&root)
            .args(["archive", PREVIOUS_RELEASE_REF])
            .output()
            .expect("could not run git archive");
        assert!(
            archive.status.success(),
            "git archive {PREVIOUS_RELEASE_REF} failed: {}",
            String::from_utf8_lossy(&archive.stderr)
        );
        let mut tar = Command::new("tar")
            .args(["-x", "-C"])
            .arg(&tree)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("could not run tar");
        tar.stdin
            .as_mut()
            .expect("tar stdin")
            .write_all(&archive.stdout)
            .expect("could not pipe the archive into tar");
        assert!(tar.wait().expect("tar wait").success(), "tar -x failed");
    }

    let status = Command::new("cargo")
        .current_dir(&tree)
        .args([
            "build",
            "-p",
            "table_canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .expect("could not run cargo to build the previous release");
    assert!(
        status.success(),
        "building the {PREVIOUS_RELEASE_REF} table canister FAILED, so no cross-version upgrade \
         can be tested. Tree: {}",
        tree.display()
    );

    let bytes = std::fs::read(&path).expect("previous-release wasm could not be read");
    let sha256 = sha256_hex(&bytes);
    announce(&format!(
        "MONEY-SAFETY: PREVIOUS release ({PREVIOUS_RELEASE_REF}) sha256={sha256} bytes={}",
        bytes.len()
    ));
    TableModule {
        bytes,
        sha256,
        path,
    }
}

fn build_table_canister() -> TableModule {
    let root = repo_root();
    let path = root.join(TABLE_WASM_REL);

    // ALWAYS build. Not "build if missing": build.
    //
    // The shared target dir is used deliberately rather than a harness-private
    // one, because it is the path `scripts/dev.sh` exports as
    // $CLEARDECK_TABLE_WASM and the path `icp deploy` installs from. Having one
    // artifact is what makes "the sha256 printed by this harness" and "the sha256
    // of the deployed module" the same claim. Cargo takes a file lock on the
    // target dir, so a concurrent build in another process serialises rather
    // than corrupting.
    let status = Command::new("cargo")
        .current_dir(&root)
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
    assert!(
        status.success(),
        "building table_canister for wasm32-unknown-unknown FAILED. The money-safety harness \
         refuses to run against any binary it did not just build from this tree (docs/DEFECTS.md \
         H-01), so there is nothing to test. Fix the build."
    );

    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cargo reported success but {} could not be read: {e}",
            path.display()
        )
    });
    let sha256 = sha256_hex(&bytes);

    announce(&format!(
        "MONEY-SAFETY: wasm under test sha256={sha256} bytes={} path={}",
        bytes.len(),
        path.display()
    ));

    // $CLEARDECK_TABLE_WASM does not select the artifact any more -- it is a
    // cross-check. If a caller points it somewhere else, the two must be the same
    // module or the run is meaningless.
    if let Ok(declared) = std::env::var("CLEARDECK_TABLE_WASM") {
        let declared = PathBuf::from(declared);
        if declared.canonicalize().ok() != path.canonicalize().ok() {
            let other = std::fs::read(&declared).unwrap_or_else(|e| {
                panic!(
                    "CLEARDECK_TABLE_WASM={} could not be read: {e}",
                    declared.display()
                )
            });
            let other_sha = sha256_hex(&other);
            assert_eq!(
                other_sha,
                sha256,
                "CLEARDECK_TABLE_WASM points at a DIFFERENT module than the one this harness just \
                 built from the checked-out source.\n  declared {} sha256={other_sha}\n  built    \
                 {} sha256={sha256}\nRefusing to run: the result could not be attributed to either \
                 binary. Rebuild with `./scripts/dev.sh wasm` and re-run.",
                declared.display(),
                path.display()
            );
        }
    }

    TableModule {
        bytes,
        sha256,
        path,
    }
}

/// Emit a line that survives `cargo test`'s per-test output capture.
///
/// libtest replaces the capture target used by `print!`/`eprint!`, so a banner
/// written with those macros is only visible when the test that happened to
/// trigger it FAILS -- exactly backwards for an identity banner. Writing to fd 2
/// through `/dev/stderr` bypasses the capture, so the sha256 appears on every run,
/// passing or failing, before any test result.
fn announce(line: &str) {
    let direct = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/stderr")
        .and_then(|mut f| {
            writeln!(f, "{line}")?;
            f.flush()
        });
    if direct.is_err() {
        eprintln!("{line}");
    }
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
