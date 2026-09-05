//! **THE TRUST ROOT OF A DEPOSIT ADDRESS. docs/SECURITY-FINDINGS.md FINDING 42.**
//!
//! # What this target is about, and why the existing deposit gates could not see it
//!
//! A ClearDeck deposit address is
//!
//! ```text
//! account_identifier( TABLE CANISTER ID , sha256("cleardeck-deposit:" || YOUR PRINCIPAL) )
//! ```
//!
//! FINDING 40 closed the substitution of the SECOND argument: the client stopped
//! asking `get_deposit_address()` -- an uncertified query -- and derived the
//! address itself. Its decision table justified that with the sentence *"the
//! canister id [comes] from the build's own configuration"*, and
//! `depositAddress.js` repeated it.
//!
//! **It did not.** It came from `lobby.get_tables()`, which
//! `src/lobby_canister/lobby_canister.did` declares `query` -- one replica, out
//! of its own memory, with no certificate this client verifies. So the FIRST
//! argument was attacker-controllable, every derived address moved with it, and
//! the modal's cross-check still passed because it asked the canister the
//! substituted id names and a substituted canister answers consistently about
//! itself.
//!
//! `deposit_surface.rs` proves the derivation's ARITHMETIC agrees with the
//! canister, principal by principal. That gate is green with the substitution
//! live, because the arithmetic is not what is wrong: the wrong canister id is
//! hashed perfectly. This target is about WHERE THE ID CAME FROM.
//!
//! # Why these tests are file reads and a `node` run
//!
//! They need no replica and no wasm, so they are cheap enough for the fast tier,
//! and every one of them fails on the pre-fix tree (`4e08c6d`). The end-to-end
//! version -- the same substitution, in a browser, photographed -- is
//! `node tools/shots/repro-finding42.mjs`, which needs the local replica.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf()
}

fn lib_dir() -> PathBuf {
    repo_root().join("src/cleardeck_frontend/src/lib")
}

/// Everything outside `//` line comments and `/* */` blocks. Deliberately naive
/// (it does not parse strings or regex literals); it is used only on modules
/// this repository owns, and the failure direction is safe: a forbidden token
/// hidden inside a string literal still trips the scan.
fn strip_js_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == '/' && i + 1 < bytes.len() && bytes[i + 1] == '/' {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
        } else if bytes[i] == '/' && i + 1 < bytes.len() && bytes[i + 1] == '*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// The body of the `if (...) { ... }` that starts at or just after `at`, found by
/// matching braces rather than by counting bytes.
///
/// Written because the byte-offset form of these assertions could be satisfied by
/// a guard that does not guard: "the check appears before the money call" is true
/// of `if (!trusted) { error = msg; }` with no `return`, and so is "a `return`
/// appears somewhere in between", because the next statement is a validation that
/// returns. The only question worth asking is whether THIS block stops the call.
/// Returns the empty string when there is no block, which fails the caller's
/// `contains("return")` -- the safe direction.
fn guard_block(src: &str, at: usize) -> String {
    if at >= src.len() {
        return String::new();
    }
    let Some(open) = src[at..].find('{').map(|o| at + o) else {
        return String::new();
    };
    let bytes: Vec<char> = src[open..].chars().collect();
    let mut depth = 0i32;
    let mut body = String::new();
    for c in bytes {
        body.push(c);
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return body;
                }
            }
            _ => {}
        }
    }
    String::new()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// A Svelte component with its `//`, `/* */` and `<!-- -->` comments removed:
/// COMMENTS ARE NOT CODE, and the modal's prose names the money calls it no
/// longer makes (the explanation of what the fees pay for). A forbidden-token
/// scan must read the code, or the next author deletes the explanation to keep
/// the gate green, which is the wrong trade every time.
fn strip_svelte_comments(src: &str) -> String {
    let mut out = strip_js_comments(src);
    while let Some(open) = out.find("<!--") {
        let Some(close) = out[open..].find("-->").map(|c| open + c + 3) else {
            out.truncate(open);
            break;
        };
        out.replace_range(open..close, "");
    }
    out
}

/// A canister id that is real, well-formed, and NOT one this build was published
/// with. It is the id the reproducer's hostile table was created at.
const UNPINNED_ID: &str = "55icz-et777-77775-aaafq-cai";
/// A local id that IS pinned by the build environment the node runs below set.
const PINNED_ID: &str = "4caro-hl777-77775-aaaba-cai";
const SOME_PLAYER: &str = "4tgka-ghyhm-4ebbe-3xqm4-alsxm-onnix-mi73y-5bcjg-ecekb-coffw-zqe";

/// Runs an ES module snippet in node with a build environment that pins exactly
/// one table id, and returns (stdout, stderr, ok).
fn node_with_pinned_table(script: &str) -> (String, String, bool) {
    let out = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(script)
        .current_dir(repo_root().join("src/cleardeck_frontend"))
        // THE BUILD'S OWN CONFIGURATION, which on a local build is exactly what
        // `tools/shots/lib/frontend-build.mjs` exports and what vite's `define`
        // substitutes into the bundle (verified: the ids appear in dist/*.js).
        .env("CANISTER_ID_TABLE_1", PINNED_ID)
        .output()
        .expect(
            "node is required to run this gate; scripts/dev.sh doctor already requires it, \
             and the frontend cannot be built without it",
        );
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

// ===========================================================================
// 1. THE DERIVATION ITSELF
// ===========================================================================

/// **THE GATE.** The real `depositAddress.js`, run in node, must REFUSE to
/// derive an address from a canister id this build was not published with, and
/// must still derive one for an id it was.
///
/// On the pre-fix tree there is no `deriveTrustedDepositAddress` at all and the
/// import fails; add one that does not consult a trust root and the refusal
/// assertion fails. Either way this is red.
#[test]
fn the_deposit_derivation_refuses_a_canister_id_this_build_never_named() {
    let module = lib_dir().join("depositAddress.js");
    assert!(module.exists(), "missing {}", module.display());

    let script = format!(
        "import {{ deriveTrustedDepositAddress }} from {module:?};\n\
         const out = {{}};\n\
         try {{ out.pinned = deriveTrustedDepositAddress({pinned:?}, {who:?}).address; }}\n\
         catch (e) {{ out.pinnedError = e.message; }}\n\
         try {{ out.unpinned = deriveTrustedDepositAddress({unpinned:?}, {who:?}).address; }}\n\
         catch (e) {{ out.refusal = e.message; }}\n\
         console.log(JSON.stringify(out));\n",
        module = module.to_string_lossy(),
        pinned = PINNED_ID,
        unpinned = UNPINNED_ID,
        who = SOME_PLAYER,
    );
    let (stdout, stderr, ok) = node_with_pinned_table(&script);
    assert!(
        ok,
        "the frontend deposit module would not run. On the pre-fix tree this is because \
         `deriveTrustedDepositAddress` does not exist: the app derived a deposit address \
         straight from the canister id `lobby.get_tables()` handed it.\n\
         stdout: {stdout}\nstderr: {stderr}"
    );
    println!("{}", stdout.trim());

    assert!(
        stdout.contains("\"pinned\":\""),
        "the derivation refused a table id the build IS published with ({PINNED_ID}). \
         A trust root that refuses the honest case is an outage, not a gate.\n{stdout}"
    );
    assert!(
        !stdout.contains("\"unpinned\":\""),
        "FINDING 42 IS BACK: depositAddress.js derived a deposit address from {UNPINNED_ID}, \
         a canister id this build was never published with. On the running app that id comes \
         from `lobby.get_tables()`, an UNCERTIFIED QUERY: one replica can answer with any \
         canister it likes and every derived address moves into it, while the modal's own \
         cross-check still passes because it asks that same substituted canister. \
         Reproduce with `node tools/shots/repro-finding42.mjs`.\n{stdout}"
    );
    assert!(
        stdout.contains("\"refusal\":\""),
        "the derivation neither produced an address for {UNPINNED_ID} nor said why. \
         A silent empty result is how a player ends up staring at a blank panel.\n{stdout}"
    );
    // The refusal has to be readable by the person it happens to.
    let refusal_mentions_id = stdout.contains(UNPINNED_ID);
    assert!(
        refusal_mentions_id,
        "the refusal does not name the canister id it refused ({UNPINNED_ID}).\n{stdout}"
    );
}

/// The trust root must be a BUILD-TIME fact. If it were empty by default, the
/// test above would pass for the wrong reason -- everything refused -- so this
/// pins the other direction: with ids compiled in, they are the set, and with
/// none compiled in, the set is empty and the app says so rather than falling
/// back to whatever arrived over the wire.
#[test]
fn the_trusted_set_is_exactly_what_the_build_was_given_and_never_widens_at_runtime() {
    let module = lib_dir().join("trustedTables.js");
    assert!(
        module.exists(),
        "missing {} -- the deposit path has no trust root at all",
        module.display()
    );

    let script = format!(
        "import {{ TRUSTED_TABLE_IDS, isTrustedTableId }} from {module:?};\n\
         console.log(JSON.stringify({{ ids: [...TRUSTED_TABLE_IDS], \
           pinned: isTrustedTableId({pinned:?}), unpinned: isTrustedTableId({unpinned:?}) }}));\n",
        module = module.to_string_lossy(),
        pinned = PINNED_ID,
        unpinned = UNPINNED_ID,
    );
    let (stdout, stderr, ok) = node_with_pinned_table(&script);
    assert!(ok, "trustedTables.js would not run:\n{stderr}");
    println!("{}", stdout.trim());
    assert!(
        stdout.contains(PINNED_ID) && stdout.contains("\"pinned\":true"),
        "the id the build was given is not in its own trusted set:\n{stdout}"
    );
    assert!(
        stdout.contains("\"unpinned\":false"),
        "an id the build was never given is trusted anyway:\n{stdout}"
    );

    // And with NOTHING compiled in, the set must be empty rather than permissive.
    let empty = Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(&script)
        .current_dir(repo_root().join("src/cleardeck_frontend"))
        .env_remove("CANISTER_ID_TABLE_1")
        .env_remove("CANISTER_ID_TABLE_2")
        .env_remove("CANISTER_ID_TABLE_3")
        .env_remove("CANISTER_ID_BTC_TABLE_1")
        .env_remove("VITE_TRUSTED_TABLE_IDS")
        .env_remove("CLEARDECK_TRUSTED_TABLE_IDS")
        .output()
        .expect("node");
    let empty_out = String::from_utf8_lossy(&empty.stdout).to_string();
    println!("no ids compiled in -> {}", empty_out.trim());
    assert!(
        empty_out.contains("\"ids\":[]") && empty_out.contains("\"unpinned\":false"),
        "a build with no table ids compiled in must trust NOTHING. Anything else means the \
         trust root falls back to something, and the only other thing available is the wire.\n\
         {empty_out}"
    );
}

/// The trust root must make no network call, for the same reason
/// `depositAddress.js` makes none: a value that can be fetched can be
/// substituted, and this is the value everything else is checked against.
#[test]
fn the_trust_root_module_cannot_ask_anybody_anything() {
    // COMMENTS ARE NOT CODE, and this file quotes the defect it exists to close
    // (`await lobby.get_tables()`) verbatim. A scan that cannot tell the two
    // apart forces the next author to delete the explanation to keep the gate
    // green, which is the wrong trade every time.
    let src = strip_js_comments(&read("src/cleardeck_frontend/src/lib/trustedTables.js"));
    for forbidden in [
        "await ",
        "fetch(",
        "HttpAgent",
        "Actor.createActor",
        "canisters.js",
        "get_tables",
        "tableActor",
    ] {
        assert!(
            !src.contains(forbidden),
            "trustedTables.js contains `{forbidden}`. The trust root is not allowed to be \
             fetched: an id that arrives over the wire is exactly what FINDING 42 is about."
        );
    }
}

// ===========================================================================
// 2. THE COMPONENT THAT SPENDS
// ===========================================================================

/// **A CORRECT MODULE THE APP DOES NOT USE IS NOT A FIX.**
///
/// This is the FINDING-41 lesson applied one level up: the two derivation gates
/// in `deposit_surface.rs` proved the arithmetic and could not see that the OISY
/// branch fetched its destination. So this one follows the id. The address door
/// lives in `lib/deposit-wallet.svelte.js` (`createWalletReader`,
/// `deriveAndVerifyDepositAddress`) since the cashier phase moved it out of the
/// modal statement for statement (docs/UI-WAVE.md section 4); the modal renders
/// what the reader publishes. So: the reader must reach the derivation only
/// through the trusted entry point and never call the arithmetic-only one; the
/// modal must press THAT reader; and the modal must hold NO derivation of its own
/// (neither entry point, no import of the derivation module), because an
/// unguarded copy in the component is invisible to a gate pointed at the module.
#[test]
fn the_deposit_modal_derives_only_through_the_trust_root() {
    let reader = read("src/cleardeck_frontend/src/lib/deposit-wallet.svelte.js");

    assert!(
        reader.contains("deriveTrustedDepositAddress("),
        "deposit-wallet.svelte.js does not call deriveTrustedDepositAddress(). The address it \
         publishes is therefore derived from whatever canister id reached it -- and that id \
         comes from `lobby.get_tables()`, an uncertified query. FINDING 42."
    );
    // The arithmetic-only entry point must not appear in the reader at all.
    let uses_raw = |src: &str| {
        src.lines()
            .enumerate()
            .filter(|(_, l)| {
                l.contains("deriveDepositAddress(") && !l.contains("deriveTrustedDepositAddress(")
            })
            .map(|(i, l)| format!("  line {}: {}", i + 1, l.trim()))
            .collect::<Vec<_>>()
    };
    let raw_in_reader = uses_raw(&reader);
    assert!(
        raw_in_reader.is_empty(),
        "deposit-wallet.svelte.js calls the ARITHMETIC-ONLY derivation, which asks no questions \
         about where the canister id came from:\n{}\n\
         Use deriveTrustedDepositAddress(). docs/SECURITY-FINDINGS.md FINDING 42.",
        raw_in_reader.join("\n")
    );
    // And the reader never prefers the canister's answer over the derived one.
    assert!(
        reader.contains("checkAgainstCanister(") && reader.contains("get_deposit_address()"),
        "deposit-wallet.svelte.js no longer cross-checks the derived address against the \
         canister's (FINDING 34 / 40): the comparison is what catches a drift, and it must \
         never replace the derived address with the reply."
    );

    // FOLLOW THE ID TO THE MODAL: the component presses THIS reader ...
    let modal = strip_svelte_comments(&read(
        "src/cleardeck_frontend/src/lib/components/DepositModal.svelte",
    ));
    assert!(
        modal.contains("from '$lib/deposit-wallet.svelte.js'") && modal.contains("createWalletReader("),
        "DepositModal.svelte does not create its wallet reader from lib/deposit-wallet.svelte.js: \
         the guarded derivation is not the one the modal publishes (the FINDING-41 lesson)."
    );
    // ... and holds NO derivation of its own: neither entry point, no import of
    // the derivation module. Any address the modal shows is the reader's.
    let derivations_in_modal = modal
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            l.contains("deriveDepositAddress(")
                || l.contains("deriveTrustedDepositAddress(")
                || l.contains("depositAddress.js")
                || l.contains("accountIdentifierHex(")
        })
        .map(|(i, l)| format!("  line {}: {}", i + 1, l.trim()))
        .collect::<Vec<_>>();
    assert!(
        derivations_in_modal.is_empty(),
        "DepositModal.svelte derives a deposit address on its own, outside the reader this \
         gate reads:\n{}\nThe modal must render only what lib/deposit-wallet.svelte.js \
         publishes. FINDING 42.",
        derivations_in_modal.join("\n")
    );
}

/// Every door that moves money is a function of the same wire-supplied canister
/// id: the ADDRESS panel, the OISY transfer's `owner`, the ICRC-2 approval's
/// `spender`, the sweep, and the BTC address FETCHED from the canister. Closing
/// one and leaving the others is precisely how FINDING 40 shipped half closed
/// and had to be reopened as FINDING 41.
///
/// The doors are in the modules that own them now (docs/UI-WAVE.md section 4):
/// the ICRC-2 and OISY doors behind `submitDeposit()` (`lib/deposit-submit.js`
/// -> `lib/deposit-flow.js`), refused in the modal's `handleDeposit()` before
/// they are pressed; the sweep in `lib/deposit-flow.svelte.js` `claim()`; the
/// BTC fetch in `lib/deposit-btc.svelte.js` `loadAddress()`. Each guard is
/// asserted in the module that owns the door, in the same strict form as before
/// (the check BEFORE the money call, and the guard's OWN block returning), the
/// trust decision the modules are handed is asserted to be the modal's
/// `tableIsTrusted`, and the modal is asserted to hold NO unguarded copy of any
/// door.
#[test]
fn every_money_door_in_the_modal_refuses_an_unpinned_table() {
    // The CODE of the modal: its comments explain fees and findings by naming
    // calls the modal no longer makes, and this gate scans for those names.
    let src = strip_svelte_comments(&read(
        "src/cleardeck_frontend/src/lib/components/DepositModal.svelte",
    ));

    // The trust decision exists and is made from the trust root, not from a reply.
    assert!(
        src.contains("isTrustedTableId(tableCanisterId)"),
        "DepositModal.svelte never asks whether `tableCanisterId` is a table this build names."
    );

    // DOOR 1 + 2: handleDeposit() drives BOTH the ICRC-2 approve+deposit path
    // and the OISY transfer through submitDeposit(), so the refusal has to be
    // before that call can start.
    let handle = src
        .split_once("async function handleDeposit()")
        .map(|(_, rest)| rest)
        .expect("DepositModal.svelte no longer has handleDeposit()");
    let guard_at = handle
        .find("!tableIsTrusted")
        .expect(
            "handleDeposit() does not refuse an untrusted table. Through submitDeposit() it \
             approves `spender: tableCanisterId` over the player's ledger balance and transfers \
             to `owner: tableCanisterId` -- both are as final as paying the address. FINDING 42.",
        );
    let submit_at = handle
        .find("submitDeposit(")
        .expect("handleDeposit() no longer presses submitDeposit(): the money call moved and this gate must follow it");
    assert!(
        guard_at < submit_at,
        "the trust check in handleDeposit() is at byte {guard_at}, AFTER submitDeposit() \
         ({submit_at}). A refusal that happens after the signature is a receipt, not a refusal."
    );

    // A GUARD THAT DOES NOT RETURN IS NOT A GUARD. The assertion above finds
    // the byte OFFSET of the check and compares it with the offset of the money
    // call, which is satisfied by an `if (!tableIsTrusted) { error = ... }` that
    // then falls through and signs anyway. Wave 14's coherence pass measured this:
    // deleting the single token `return;` from that block left all seven tests
    // green AND left `node tools/shots/repro-finding42.mjs` exiting 0 printing
    // "REFUSED", while handleDeposit() ran on to icrc2_approve against the
    // substituted canister. The window has to be the guard's OWN block. The
    // coherence pass wrote this assertion first as "a `return` appears between
    // the check and the approval", re-ran the same one-token mutation, and it
    // stayed GREEN -- because the next `return` belongs to the `if
    // (!depositAmount ...)` validation immediately below. An instrument that can
    // be satisfied by the neighbouring statement is the same defect it was
    // written to catch, one level up.
    assert!(
        guard_block(handle, guard_at).contains("return"),
        "handleDeposit()'s `!tableIsTrusted` block does not RETURN. Ordering is not refusal: a \
         guard that sets an error message and falls through still reaches submitDeposit(), \
         i.e. `icrc2_approve({{ spender: tableCanisterId }})` and `wallet.transfer`, and both \
         are as final as paying the address.\n\nThe block is:\n{}",
        guard_block(handle, guard_at)
    );
    // The two money calls behind submitDeposit() are in the modules, and the
    // modal holds NO copy of either (a copy would bypass the guard above).
    let flow = read("src/cleardeck_frontend/src/lib/deposit-flow.js");
    assert!(
        flow.contains("approveSpender(") && flow.contains("wallet.transfer("),
        "lib/deposit-flow.js no longer holds the ICRC-2 approval and the OISY transfer: the \
         money calls moved again and this gate must follow them."
    );
    for forbidden in ["icrc2_approve", "approveSpender(", "wallet.transfer", "icrc1_transfer"] {
        assert!(
            !src.contains(forbidden),
            "DepositModal.svelte contains `{forbidden}`: a money call outside the modules this \
             gate reads, reachable without handleDeposit()'s refusal."
        );
    }

    // DOOR 3: THE SWEEP (lib/deposit-flow.svelte.js claim()). Nothing at an
    // unpinned canister is the player's to sweep, and the modal's own
    // `tableIsTrusted` is the decision it is handed.
    let watch = read("src/cleardeck_frontend/src/lib/deposit-flow.svelte.js");
    let claim = watch
        .split_once("async function claim()")
        .map(|(_, rest)| rest)
        .expect("deposit-flow.svelte.js no longer has claim()");
    let claim_guard = claim.find("!isTrusted()").unwrap_or(usize::MAX);
    let claim_call = claim.find("claim_external_deposit").unwrap_or(usize::MAX);
    assert!(
        claim_guard < claim_call && guard_block(claim, claim_guard).contains("return"),
        "claim() in deposit-flow.svelte.js sweeps at an unpinned canister (guard at \
         {claim_guard}, call at {claim_call}, or the guard's block does not return)."
    );
    let watch_wiring = src
        .split_once("createAddressWatch({")
        .map(|(_, rest)| rest)
        .expect("DepositModal.svelte no longer creates its address watch");
    assert!(
        watch_wiring[..watch_wiring.find("});").unwrap_or(watch_wiring.len())]
            .contains("isTrusted: () => tableIsTrusted"),
        "the address watch is not handed the modal's own trust decision \
         (`isTrusted: () => tableIsTrusted`): its guard is deciding on something else."
    );
    assert!(
        !src.contains("claim_external_deposit("),
        "DepositModal.svelte calls claim_external_deposit itself, outside the guarded claim()."
    );

    // DOOR 4 (docs/SECURITY-FINDINGS.md FINDING 45). The BTC address is not
    // derived from the pinned id at all -- it is FETCHED from whatever canister
    // `get_tables()` named, and rendered with a Copy button. It is FINDING 40
    // unfixed for a chain whose transfers cannot be reversed, and the attacker
    // also picks the branch, because `currency` comes off the same uncertified
    // reply. The fetch lives in lib/deposit-btc.js (`fetchBtcDepositAddress`),
    // pressed only by lib/deposit-btc.svelte.js loadAddress() behind the guard.
    let btc_calls = read("src/cleardeck_frontend/src/lib/deposit-btc.js");
    assert!(
        btc_calls.contains("get_btc_deposit_address()"),
        "lib/deposit-btc.js no longer fetches get_btc_deposit_address: the door moved and \
         this gate must follow it."
    );
    let btc_state = read("src/cleardeck_frontend/src/lib/deposit-btc.svelte.js");
    let btc = btc_state
        .split_once("async function loadAddress()")
        .map(|(_, rest)| rest)
        .expect("deposit-btc.svelte.js no longer has loadAddress()");
    let btc_guard = btc.find("!isTrusted()").unwrap_or(usize::MAX);
    let btc_fetch = btc.find("fetchBtcDepositAddress(").unwrap_or(usize::MAX);
    assert!(
        btc_guard < btc_fetch && guard_block(btc, btc_guard).contains("return"),
        "loadAddress() in deposit-btc.svelte.js asks an unpinned canister for a Bitcoin address \
         (guard at {btc_guard}, fetch at {btc_fetch}, or the guard's block does not return) and \
         the modal renders the reply under \"Your Bitcoin Deposit Address\" with a Copy button. \
         Bitcoin sent to it is unrecoverable."
    );
    let btc_wiring = src
        .split_once("createBtcDeposit({")
        .map(|(_, rest)| rest)
        .expect("DepositModal.svelte no longer creates its BTC path");
    assert!(
        btc_wiring[..btc_wiring.find("});").unwrap_or(btc_wiring.len())]
            .contains("isTrusted: () => tableIsTrusted"),
        "the BTC path is not handed the modal's own trust decision \
         (`isTrusted: () => tableIsTrusted`): its guard is deciding on something else."
    );
    for forbidden in ["get_btc_deposit_address", "fetchBtcDepositAddress("] {
        assert!(
            !src.contains(forbidden),
            "DepositModal.svelte contains `{forbidden}`: a copy of the BTC door outside the \
             guarded loadAddress() this gate reads."
        );
    }
    // The render: every use of the fetched string in the modal sits beside the
    // trust decision. The fetch guard is not enough on its own: this string is
    // not derived by this build from anything it pinned, so the assertion has
    // to sit next to the pixels a player copies from.
    let unguarded_renders = src
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("btc.address") && !l.contains("tableIsTrusted"))
        .map(|(i, l)| format!("  line {}: {}", i + 1, l.trim()))
        .collect::<Vec<_>>();
    assert!(
        src.contains("tableIsTrusted ? btc.address : ''") && unguarded_renders.is_empty(),
        "the BTC address renders without asking whether the table is pinned:\n{}",
        unguarded_renders.join("\n")
    );

    // And the screen has to say so: a disabled button with no sentence beside it
    // is a bug report from the user's point of view. The sentence is the trust
    // root's own (`untrustedTableMessage`), handed to the disclosures panel the
    // sheet renders (CashierDisclosures.svelte), which paints it as
    // `.untrusted-table` whenever the decision is "not pinned".
    assert!(
        src.contains("untrustedTableMessage("),
        "the modal no longer asks the trust root for the refusal's sentence."
    );
    let disclosures_wiring = src
        .split_once("<CashierDisclosures")
        .map(|(_, rest)| rest)
        .expect("DepositModal.svelte no longer renders CashierDisclosures");
    let disclosures_open = &disclosures_wiring[..disclosures_wiring.find('>').unwrap_or(disclosures_wiring.len())];
    assert!(
        disclosures_open.contains("{tableIsTrusted}") && disclosures_open.contains("{untrustedReason}"),
        "the disclosures panel is not handed the modal's trust decision and its sentence \
         (`{{tableIsTrusted}}` and `{{untrustedReason}}` on <CashierDisclosures>)."
    );
    let disclosures = strip_svelte_comments(&read(
        "src/cleardeck_frontend/src/lib/components/CashierDisclosures.svelte",
    ));
    let untrusted_block = disclosures
        .split_once("{#if !tableIsTrusted}")
        .map(|(_, rest)| rest)
        .expect("CashierDisclosures.svelte no longer branches on !tableIsTrusted");
    let untrusted_block = &untrusted_block[..untrusted_block.find("{/if}").unwrap_or(untrusted_block.len())];
    assert!(
        untrusted_block.contains("class=\"untrusted-table\"") && untrusted_block.contains("{untrustedReason}"),
        "the modal disables the controls without telling the player why. The refusal has to be \
         rendered, in the player's words, on the screen the money would have left from \
         (CashierDisclosures.svelte, `.untrusted-table` with the trust root's sentence)."
    );

    // The disabled bindings were asserted by nothing, so removing `!tableIsTrusted`
    // from the Deposit button was a one-token edit no gate could see. The one
    // primary button (Deposit / Claim / Check, by route) is disabled through
    // `primaryDisabled`, whose every route names the trust decision.
    assert!(
        src.contains("disabled={primaryDisabled}"),
        "the primary button in DepositModal.svelte is no longer disabled through `primaryDisabled`."
    );
    let primary = src
        .split_once("const primaryDisabled = $derived.by(() => {")
        .map(|(_, rest)| rest)
        .expect("DepositModal.svelte no longer derives primaryDisabled");
    let primary_body = &primary[..primary.find("});").unwrap_or(primary.len())];
    let routes_guarded = primary_body
        .lines()
        .filter(|l| l.contains("return "))
        .filter(|l| l.contains("!tableIsTrusted"))
        .count();
    let routes_total = primary_body.lines().filter(|l| l.contains("return ")).count();
    assert!(
        routes_total >= 3 && routes_guarded == routes_total,
        "only {routes_guarded} of {routes_total} route(s) of `primaryDisabled` in DepositModal.svelte \
         are disabled on an unpinned table; the Deposit, Claim and BTC check buttons all have to be."
    );
}

/// The `.did` promise and the register entry are only worth what the code does,
/// but a header comment that states a FALSE property is worse than none: FINDING
/// 40's "the canister id [comes] from the build's own configuration" is what let
/// FINDING 42 sit unnoticed for three waves. If the correction is deleted, this
/// goes red.
#[test]
fn the_derivation_module_does_not_repeat_finding_40s_false_claim() {
    let src = read("src/cleardeck_frontend/src/lib/depositAddress.js");
    let false_claim = "the canister id from the build's own\n// configuration";
    assert!(
        !src.contains(false_claim),
        "depositAddress.js still claims the canister id comes from the build's own \
         configuration. It comes from `lobby.get_tables()`. docs/SECURITY-FINDINGS.md FINDING 42."
    );
    assert!(
        src.contains("FINDING 42"),
        "depositAddress.js does not mention FINDING 42, so the next reader has no way to know \
         which of the two arguments of the hash is the trusted one."
    );
}

// ===========================================================================
// 3. THE TRANSPORT UNDER ALL OF IT
// ===========================================================================

/// **docs/DEFECTS.md T-43.** The agent must not switch off query-signature
/// verification.
///
/// This is not the answer to FINDING 42 -- a verified query is still one
/// replica's opinion, signed -- but with it off, FINDING 42's substitution
/// needs no replica at all: any box on the path (boundary node, gateway, proxy,
/// CDN edge) can rewrite any query reply, including the one that names the
/// canister a deposit address is derived from.
///
/// `false` is not `@dfinity/agent`'s default; it has to be written. If it ever
/// has to be written again, this gate is the conversation: say in the comment
/// what broke, on which network, with what error.
#[test]
fn the_agent_verifies_query_signatures() {
    let src = strip_js_comments(&read("src/cleardeck_frontend/src/lib/canisters.js"));
    let compact: String = src.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        !compact.contains("verifyQuerySignatures:false"),
        "canisters.js builds its agent with `verifyQuerySignatures: false`. That is not the \
         library default: it switches off the only check binding a query reply to a node of the \
         subnet that hosts the canister, so ANY box on the path can rewrite ANY query reply -- \
         including `lobby.get_tables()`, which is where a deposit address's canister id comes \
         from (docs/SECURITY-FINDINGS.md FINDING 42). Measured on the local replica with it \
         `true`: the lobby, the table and the deposit modal all render with zero console errors \
         (artifacts/finding42/evidence.json). docs/DEFECTS.md T-43."
    );
    assert!(
        compact.contains("verifyQuerySignatures:true"),
        "canisters.js no longer states `verifyQuerySignatures` at all. The library default is \
         `true` today, but this app spent a wave with it explicitly `false` and nothing noticed; \
         state it, so the next reader can see which way it is set without knowing agent-js's \
         release history. docs/DEFECTS.md T-43."
    );
}
