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

// ---------------------------------------------------------------------------
// the build invocation, PINNED (docs/DEFECTS.md H-21)
// ---------------------------------------------------------------------------

/// Environment variables that change what `cargo build` produces, or where it
/// puts it, and which this harness therefore refuses to inherit.
///
/// # Why (docs/DEFECTS.md H-21)
///
/// The harness prints a sha256 and calls it "the wasm under test". That is a claim
/// about the CODE. It stops being one the moment the build can be steered from
/// outside:
///
/// * `RUSTUP_TOOLCHAIN` OVERRIDES `rust-toolchain.toml`. Cargo exports it to every
///   child process, so running the harness from inside another cargo invocation
///   (a wrapper script, a workspace test, an IDE) silently compiles the canister
///   with whatever compiler the parent happened to use, and identical source
///   produces a different module hash. Measured: 1.90.0 and 1.96.1 do not agree.
/// * `CARGO_TARGET_DIR` moves the OUTPUT. `cargo build` would succeed while
///   `target/wasm32-unknown-unknown/release/table_canister.wasm` -- the path this
///   file then reads and hashes -- kept whatever stale bytes were already there.
///   That is docs/DEFECTS.md H-01 all over again, reachable purely from the
///   environment.
/// * `RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` / `RUSTC` / `RUSTC_WRAPPER` /
///   `CARGO_PROFILE_*` change codegen.
///
/// Exact names, and prefixes for the families cargo reads.
pub const SCRUBBED_EXACT: &[&str] = &[
    "RUSTUP_TOOLCHAIN",
    "RUSTUP_HOME_OVERRIDE",
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

/// The toolchain channel `dir/rust-toolchain.toml` pins, e.g. `1.90.0`.
///
/// Read from the tree being compiled rather than hardcoded, so the pin cannot
/// drift away from the file the rest of the project uses. Absent or unparseable
/// is a hard error: the whole point is that the build is not left to chance.
pub fn pinned_toolchain(dir: &Path) -> String {
    let path = dir.join("rust-toolchain.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. The money-safety harness pins the compiler from that file so \
             the sha256 it prints is a claim about the CODE and not about how the harness was \
             invoked (docs/DEFECTS.md H-21).",
            path.display()
        )
    });
    parse_toolchain_channel(&text).unwrap_or_else(|| {
        panic!(
            "{} has no `channel = \"...\"` line, so there is no pin to apply",
            path.display()
        )
    })
}

/// `channel = "1.90.0"` out of a `rust-toolchain.toml`. Split out so it is
/// testable without a filesystem.
pub fn parse_toolchain_channel(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("channel") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let value = rest.trim().trim_matches(|c| c == '"' || c == '\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// `cargo build -p table_canister --target wasm32-unknown-unknown --release`, run
/// in `dir`, with the build environment PINNED rather than inherited.
///
/// Public so the pin itself is testable: `invariants::classifier` inspects the
/// command's environment overrides rather than trusting this comment.
pub fn pinned_table_canister_build(dir: &Path) -> Command {
    pinned_canister_build(dir, "table_canister")
}

/// The same pinned build, for any canister crate in the tree.
///
/// `history_canister` is built through here for the archive gate: the permanent
/// hand record is only worth what the ARCHIVE holds, so the gate has to run
/// against the real archive module built from this tree, for the same reason the
/// table module is (docs/DEFECTS.md H-01).
pub fn pinned_canister_build(dir: &Path, package: &str) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir).args([
        "build",
        "-p",
        package,
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
    // Then say, explicitly, which compiler this is.
    cmd.env("RUSTUP_TOOLCHAIN", pinned_toolchain(dir));
    cmd
}

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

/// Where `cargo build -p history_canister ...` puts the archive module.
pub const HISTORY_WASM_REL: &str = "target/wasm32-unknown-unknown/release/history_canister.wasm";

static HISTORY_MODULE: OnceLock<TableModule> = OnceLock::new();

/// The ARCHIVE module, built from the checked-out source.
///
/// Same rule as the table module: built, never resolved from whatever happens to
/// be on disk. A gate that reads a record out of a stale archive binary is a
/// statement about a binary that is not in this tree.
pub fn history_canister_module() -> &'static TableModule {
    HISTORY_MODULE.get_or_init(|| {
        let root = repo_root();
        let path = root.join(HISTORY_WASM_REL);
        let status = pinned_canister_build(&root, "history_canister")
            .status()
            .expect("could not run cargo to build the history canister");
        assert!(
            status.success(),
            "building history_canister for wasm32-unknown-unknown FAILED, so the archive gate \
             has nothing to run against."
        );
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "cargo reported success but {} could not be read: {e}",
                path.display()
            )
        });
        let sha256 = sha256_hex(&bytes);
        announce(&format!(
            "MONEY-SAFETY: archive wasm sha256={sha256} bytes={} path={}",
            bytes.len(),
            path.display()
        ));
        TableModule {
            bytes,
            sha256,
            path,
        }
    })
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

    let status = pinned_table_canister_build(&tree)
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
        "MONEY-SAFETY: PREVIOUS release ({PREVIOUS_RELEASE_REF}) sha256={sha256} bytes={} \
         toolchain={}",
        bytes.len(),
        pinned_toolchain(&tree)
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
    // ...with the build environment PINNED, not inherited. See
    // `pinned_table_canister_build` and docs/DEFECTS.md H-21: a parent cargo
    // exports RUSTUP_TOOLCHAIN, which overrides rust-toolchain.toml, so the same
    // source produced a different module hash depending on how the harness was
    // invoked -- and CARGO_TARGET_DIR would move the output away from the path
    // this function then reads and hashes.
    let toolchain = pinned_toolchain(&root);
    let status = pinned_table_canister_build(&root)
        .status()
        .expect("could not run cargo to build the table canister");
    assert!(
        status.success(),
        "building table_canister for wasm32-unknown-unknown with the pinned toolchain \
         {toolchain} FAILED. The money-safety harness refuses to run against any binary it did \
         not just build from this tree (docs/DEFECTS.md H-01), so there is nothing to test. \
         Fix the build, or install the toolchain: `rustup toolchain install {toolchain}`."
    );

    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cargo reported success but {} could not be read: {e}",
            path.display()
        )
    });
    let sha256 = sha256_hex(&bytes);

    // The toolchain is part of the identity of the artifact, so it is printed
    // beside the hash. Without it the sha256 is a claim about the code AND about
    // how the harness happened to be invoked, which is two claims wearing one hat.
    announce(&format!(
        "MONEY-SAFETY: wasm under test sha256={sha256} bytes={} toolchain={toolchain} path={}",
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

// ---------------------------------------------------------------------------
// the GUARDIAN module (docs/SECURITY-FINDINGS.md FINDING 23)
// ---------------------------------------------------------------------------

/// Where `scripts/build-guardian.sh` puts the guardian module, relative to the
/// repo root.
///
/// `src/guardian_canister` is a DETACHED crate (its own `[workspace]`, its own
/// Cargo.lock, its own target/), so unlike every other module here it does not
/// land in the root `target/`. That detachment is deliberate and is documented in
/// its Cargo.toml: mainnet's six backend modules currently reproduce byte for
/// byte from `cargo build --locked` at the root, and the root lockfile is an
/// input to that build, so a crate that cannot rewrite the root lockfile cannot
/// change those hashes.
pub const GUARDIAN_WASM_REL: &str =
    "src/guardian_canister/target/wasm32-unknown-unknown/release/guardian_canister.wasm";

static GUARDIAN_MODULE: OnceLock<TableModule> = OnceLock::new();

/// `scripts/build-guardian.sh`, with the build environment PINNED rather than
/// inherited (docs/DEFECTS.md H-21).
///
/// The script rather than `cargo build` directly, because the script sets the
/// reproducibility RUSTFLAGS (`-Cstrip=symbols` and two `--remap-path-prefix`es)
/// that the fund-holding canisters are built with. Running plain `cargo` here
/// produced a DIFFERENT module from the one the script produces, so the gate was
/// green about a binary no build command in this repository emits — the same
/// shape as docs/DEFECTS.md H-01. One script, one artifact.
fn pinned_guardian_build(root: &Path, crate_dir: &Path) -> Command {
    let mut cmd = Command::new(root.join("scripts").join("build-guardian.sh"));
    cmd.current_dir(root);
    for key in SCRUBBED_EXACT {
        cmd.env_remove(key);
    }
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy().to_string();
        if SCRUBBED_PREFIXES.iter().any(|p| name.starts_with(p)) {
            cmd.env_remove(&name);
        }
    }
    cmd.env("RUSTUP_TOOLCHAIN", pinned_toolchain(root));
    cmd.env("CLEARDECK_GUARDIAN_CRATE", crate_dir);
    cmd
}

/// The GUARDIAN module, built from the checked-out source.
///
/// Same rule as the table module and the archive module: BUILT, never resolved
/// from whatever happens to be on disk (docs/DEFECTS.md H-01). A gate that
/// reports "the guardian refuses to reinstall" while running a binary that is not
/// in this tree is a statement about nothing.
pub fn guardian_canister_module() -> &'static TableModule {
    GUARDIAN_MODULE.get_or_init(|| {
        let root = repo_root();
        let crate_dir = root.join("src").join("guardian_canister");
        let path = root.join(GUARDIAN_WASM_REL);
        assert!(
            crate_dir.join("Cargo.toml").exists(),
            "src/guardian_canister/Cargo.toml is absent, so there is no guardian to test. \
             docs/SECURITY-FINDINGS.md FINDING 23."
        );
        let status = pinned_guardian_build(&root, &crate_dir)
            .status()
            .expect("could not run scripts/build-guardian.sh");
        assert!(
            status.success(),
            "building guardian_canister for wasm32-unknown-unknown FAILED, so the FINDING 23 \
             gate has nothing to run against."
        );
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "cargo reported success but {} could not be read: {e}",
                path.display()
            )
        });
        let sha256 = sha256_hex(&bytes);
        announce(&format!(
            "MONEY-SAFETY: guardian wasm sha256={sha256} bytes={} path={}",
            bytes.len(),
            path.display()
        ));
        TableModule {
            bytes,
            sha256,
            path,
        }
    })
}

static GUARDIAN_VARIANT: OnceLock<TableModule> = OnceLock::new();

/// A guardian module that is a DIFFERENT BINARY from the one under test, with one
/// constant changed so the difference is visible on the wire.
///
/// # Why the self-upgrade test needs this
///
/// The obvious self-upgrade test proposes the guardian's own module, executes, and
/// asserts the installed module hash equals the module it proposed. That assertion
/// passes when NOTHING HAPPENED, because the module it proposed is the module that
/// was already installed. Written that way it reported `ok` on a build whose
/// self-upgrade trapped in `ic-cdk`'s callback and never landed — the CORRECT
/// TOTALS, WRONG RECIPIENTS signature, in the gate rather than in the canister.
///
/// So the self-upgrade is tested against a module that is not the one installed:
/// the hash must CHANGE, and `max_pending` — visible through `get_config()` — must
/// change with it. Neither can happen unless the upgrade really executed.
///
/// Built by copying the crate into `target/money-safety/`, patching one line and
/// compiling, rather than by checking a binary in: a fixture nobody can rebuild
/// rots into a story about a lost file (same rule as `previous_release`).
pub fn guardian_variant_module() -> &'static TableModule {
    GUARDIAN_VARIANT.get_or_init(|| {
        let root = repo_root();
        let src = root.join("src").join("guardian_canister");
        let tree = cache_dir().join("guardian-variant");
        let path = tree.join("target/wasm32-unknown-unknown/release/guardian_canister.wasm");

        // Fresh copy every time: a stale variant tree that no longer matches the
        // guardian under test would make this a comparison against a ghost.
        let _ = std::fs::remove_dir_all(tree.join("src"));
        std::fs::create_dir_all(tree.join("src")).expect("cannot create the variant tree");
        std::fs::copy(src.join("Cargo.toml"), tree.join("Cargo.toml"))
            .expect("cannot copy the guardian manifest");
        // The lockfile too: a variant that resolves its own dependency versions is
        // a variant that differs from the module under test in ways this test does
        // not control, and it needs the network to do it.
        let _ = std::fs::copy(src.join("Cargo.lock"), tree.join("Cargo.lock"));
        let lib = std::fs::read_to_string(src.join("src").join("lib.rs"))
            .expect("cannot read the guardian source");
        const FROM: &str = "pub const MAX_PENDING: usize = 32;";
        const TO: &str = "pub const MAX_PENDING: usize = 31;";
        assert!(
            lib.contains(FROM),
            "the guardian variant patches the line `{FROM}`, which is no longer in \
             src/guardian_canister/src/lib.rs. Update the patch, or the self-upgrade gate is \
             comparing a module against itself and cannot fail."
        );
        std::fs::write(tree.join("src").join("lib.rs"), lib.replacen(FROM, TO, 1))
            .expect("cannot write the variant source");

        let status = pinned_guardian_build(&root, &tree)
            .status()
            .expect("could not run scripts/build-guardian.sh for the variant");
        assert!(status.success(), "building the guardian variant FAILED");

        let bytes = std::fs::read(&path).expect("variant guardian wasm unreadable");
        let sha256 = sha256_hex(&bytes);
        assert_ne!(
            sha256,
            guardian_canister_module().sha256,
            "the guardian variant compiled to the SAME module as the guardian under test, so \
             the self-upgrade gate would pass without anything happening"
        );
        announce(&format!(
            "MONEY-SAFETY: guardian VARIANT wasm sha256={sha256} bytes={}",
            bytes.len()
        ));
        TableModule {
            bytes,
            sha256,
            path,
        }
    })
}

/// The committed guardian interface, as text. The gate parses this rather than
/// the Rust source, because the `.did` is what a client is generated from and
/// what `icp` installs as `candid:service` metadata: if a destructive verb ever
/// appears there, it is reachable by every client in the world.
pub fn guardian_candid_text() -> String {
    let path = repo_root()
        .join("src")
        .join("guardian_canister")
        .join("guardian_canister.did");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. Regenerate it with `./scripts/build-guardian.sh --did`.",
            path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// the THIEF module (docs/SECURITY-FINDINGS.md FINDING 23c)
// ---------------------------------------------------------------------------

static THIEF_MODULE: OnceLock<TableModule> = OnceLock::new();

/// A module that is NOT ClearDeck, for the one question the wipe tests cannot
/// answer: can the operator KEEP the money, or only destroy it?
///
/// For one wave the deposit screen and the README both answered "only destroy",
/// on the argument that no ClearDeck method pays a controller. That argument is
/// true of this code and irrelevant to a controller, because a controller
/// replaces the code. `tests/thief_canister` is the smallest module that settles
/// it, and this builds it rather than resolving whatever is on disk, for the same
/// reason as every other module here (docs/DEFECTS.md H-01).
///
/// It is a detached crate under `tests/`, so it touches neither the root
/// Cargo.lock nor `icp.yaml`, and no deploy path can reach it.
///
/// Unlike the guardian there is no build SCRIPT: the guardian needs one because
/// its reproducibility flags have to match the fund-holding canisters' exactly,
/// and this module ships nowhere and is compared to nothing. The build
/// environment is still scrubbed and the toolchain still pinned, so the gate
/// cannot pass or fail because of an inherited `RUSTFLAGS`.
pub fn thief_canister_module() -> &'static TableModule {
    THIEF_MODULE.get_or_init(|| {
        let root = repo_root();
        let crate_dir = root.join("tests").join("thief_canister");
        let path = crate_dir.join("target/wasm32-unknown-unknown/release/thief_canister.wasm");
        assert!(
            crate_dir.join("Cargo.toml").exists(),
            "tests/thief_canister/Cargo.toml is absent, so the theft half of FINDING 23 has \
             nothing to run against and the custody disclosure would rest on an argument again."
        );

        let mut cmd = Command::new("cargo");
        cmd.current_dir(&crate_dir);
        for key in SCRUBBED_EXACT {
            cmd.env_remove(key);
        }
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy().to_string();
            if SCRUBBED_PREFIXES.iter().any(|p| name.starts_with(p)) {
                cmd.env_remove(&name);
            }
        }
        cmd.env("RUSTUP_TOOLCHAIN", pinned_toolchain(&root));
        cmd.env("RUSTFLAGS", "-Cstrip=symbols");
        cmd.args([
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--locked",
        ]);
        let status = cmd.status().expect("could not run cargo for tests/thief_canister");
        assert!(
            status.success(),
            "building tests/thief_canister for wasm32-unknown-unknown FAILED, so the FINDING 23c \
             gate has nothing to run against."
        );

        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "cargo reported success but {} could not be read: {e}",
                path.display()
            )
        });
        let sha256 = sha256_hex(&bytes);
        announce(&format!(
            "MONEY-SAFETY: thief wasm sha256={sha256} bytes={} path={}",
            bytes.len(),
            path.display()
        ));
        TableModule {
            bytes,
            sha256,
            path,
        }
    })
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
