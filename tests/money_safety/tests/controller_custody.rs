//! THE CONTROLLER SEAT — docs/SECURITY-FINDINGS.md FINDING 23.
//!
//! # What this file is for
//!
//! Every ClearDeck canister has ONE controller principal. A controller can call
//! `install_code --mode reinstall` or `uninstall_code` on a funded table. The
//! fifth blind auditor did exactly that: deposited 5 ICP as a player, ran ONE
//! command as the controller with the SAME wasm and no code change, and watched
//! the player's balance go to zero while the ledger still held the 5 ICP at the
//! canister's account. Every player-callable recovery said she had nothing.
//!
//! Nothing in this project could see it, and nothing could see it *by
//! construction*: `admin_custody.rs`'s subject is methods that call
//! `require_controller()` INSIDE the canister, and these two calls are made to the
//! MANAGEMENT canister, so no in-canister audit surface can reach them. The
//! defect's whole shape is "the instrument cannot look here".
//!
//! # The two halves of this file
//!
//! **PART 1 pins the defect.** `finding23_*` reproduce the wipe with numbers,
//! against the real table wasm and the real ICP ledger. They are written to PASS
//! while the defect is live, because a defect that nothing measures is a defect
//! that gets quietly forgotten — and this one has been open since wave 7. They
//! are the reason the register's `critical` row can be trusted.
//!
//! **PART 2 tests the fix, and first tests the PREMISE of the fix.** The proposed
//! fix is that the table's controller becomes a GUARDIAN CANISTER that exposes an
//! upgrade path and no reinstall or uninstall path. That rests on a claim about
//! the IC that must be measured before anything is built on it: that
//! controllership is **not transitive**, i.e. that whoever controls the guardian
//! cannot reach the table directly. `guardian_premise_*` measure it, in both
//! directions, including the direction where the answer is bad news.
//!
//! # Run it
//!
//! ```text
//! cd tests/money_safety
//! CLEARDECK_TABLE_WASM=../../target/wasm32-unknown-unknown/release/table_canister.wasm \
//!   cargo test --test controller_custody -- --nocapture --test-threads=2
//! ```
//!
//! or, from the repo root, `./scripts/dev.sh custody`.

use candid::{decode_one, encode_args, encode_one, CandidType, Deserialize, Encode, Principal};
use ic_management_canister_types as mgmt_types;
use money_safety::table_api::*;
use money_safety::wasms;
use money_safety::world::*;
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// `pocket-ic` instances are heavy and this file starts several. One chunk is the
/// management canister's per-chunk maximum.
const CHUNK_BYTES: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// transcript
// ---------------------------------------------------------------------------

/// Print a line that survives libtest's capture, so a reader of a PASSING run can
/// still see the numbers. Every claim this file makes is a measurement, and a
/// measurement nobody can read is a claim.
macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

// ---------------------------------------------------------------------------
// the guardian's Candid surface, mirrored
// ---------------------------------------------------------------------------
//
// Mirrored rather than linked: src/guardian_canister is a canister crate and the
// thing under test is its WIRE, not its Rust types. `guardian_census_*` below
// parses the committed `guardian_canister.did` and fails if the wire grows a
// method this file has not classified, which is the same protection
// `admin_custody::census_every_controller_gated_method_is_classified_here` gives
// the table.

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
enum GuardianAction {
    UpgradeGuarded {
        target: Principal,
        wasm_module_hash: Vec<u8>,
        chunk_hashes: Vec<Vec<u8>>,
        arg: Vec<u8>,
    },
    UpgradeSelf {
        wasm_module_hash: Vec<u8>,
        chunk_hashes: Vec<Vec<u8>>,
        arg: Vec<u8>,
    },
    TransferOperator {
        new_operator: Principal,
    },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct GuardianInit {
    operator: Principal,
    guarded: Vec<Principal>,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
enum ProposalStatus {
    Pending,
    Executing,
    Executed { at_secs: u64 },
    Cancelled { at_secs: u64 },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct Proposal {
    id: u64,
    kind: String,
    action: GuardianAction,
    proposed_by: Principal,
    proposed_at_secs: u64,
    ready_at_secs: u64,
    status: ProposalStatus,
    last_error: Option<String>,
    attempts: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
struct GuardianConfig {
    operator: Principal,
    guarded: Vec<Principal>,
    guarded_delay_secs: u64,
    self_delay_secs: u64,
    operator_delay_secs: u64,
    max_pending: u64,
    refused_operations: Vec<String>,
}

// ---------------------------------------------------------------------------
// plumbing
// ---------------------------------------------------------------------------

/// A raw controller-level outcome, in a shape a transcript can print.
#[derive(Debug)]
enum Mgmt {
    Ok,
    Rejected(String),
}

impl Mgmt {
    fn of<T>(r: Result<T, pocket_ic::RejectResponse>) -> Self {
        match r {
            Ok(_) => Mgmt::Ok,
            Err(e) => Mgmt::Rejected(format!("{:?}: {}", e.error_code, e.reject_message)),
        }
    }
    fn is_ok(&self) -> bool {
        matches!(self, Mgmt::Ok)
    }
    fn text(&self) -> String {
        match self {
            Mgmt::Ok => "Ok".to_string(),
            Mgmt::Rejected(s) => format!("REJECTED {}", s.lines().next().unwrap_or("")),
        }
    }
}

/// A raw ingress message to `aaaaa-aa`, exactly as `icp canister <verb>` submits
/// one, routed by the effective canister id.
fn mgmt(
    world: &World,
    effective: Principal,
    sender: Principal,
    method: &str,
    arg: Vec<u8>,
) -> Mgmt {
    Mgmt::of(world.pic.update_call_with_effective_principal(
        Principal::management_canister(),
        pocket_ic::common::rest::RawEffectivePrincipal::CanisterId(effective.as_slice().to_vec()),
        sender,
        method,
        arg,
    ))
}

/// A valid, empty wasm module: the 8-byte header and nothing else.
///
/// The premise tests are about WHO may call `install_code`, not about what gets
/// installed, and a real 2.8 MB module would be refused for exceeding the ingress
/// limit — the wrong reason, which would make the test read green while proving
/// nothing.
const MINIMAL_WASM: &[u8] = b"\x00asm\x01\x00\x00\x00";

/// Seat two players and put real money on the table, then report the books.
///
/// Returns `(ledger_at_canister, escrow_total, chips_total)`.
fn fund_a_table(world: &mut World) -> (u64, u64, u64) {
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    // `join_table` auto-buys-in at `min_buy_in`, so the seat itself moves escrow
    // into chips; `reload` tops the stack up from the same escrow.
    world.fund_escrow(alice, 20 * ICP).expect("alice deposit");
    world.fund_escrow(bob, 20 * ICP).expect("bob deposit");
    world.join_table(alice, 0).expect("alice seat");
    world.join_table(bob, 1).expect("bob seat");
    world.reload(alice, 5 * ICP).expect("alice reload");
    world.reload(bob, 5 * ICP).expect("bob reload");
    let snap = world.snapshot();
    (snap.ledger_main, snap.escrow_total, snap.chips_total)
}

/// Chips on the table, read through each PLAYER's own caller-scoped view.
///
/// Every total in `World` is read through a `require_controller()` query
/// (`admin_get_table_chips`, `admin_get_all_balances`, `get_table_state`), and
/// after the handover the operator is no longer a controller of the table, so all
/// of them stop answering — which is itself a measured consequence of the design
/// and has its own test below. `get_custody_status` is caller-scoped, so it keeps
/// working either side of the handover and the before/after comparison stays
/// apples to apples.
///
/// It is also the more honest reading: it asks the money's OWNER what they can
/// see, not the operator.
fn chips_via_players(world: &World, who: &[Principal]) -> u64 {
    who.iter().fold(0u64, |acc, p| {
        acc.saturating_add(world.custody_status(*p).chips_at_table)
    })
}

fn player_view(world: &World, who: Principal) -> (u64, String) {
    let balance = world.get_balance(who);
    let withdraw = match world.withdraw(who, 5 * ICP) {
        Ok(n) => format!("Ok({n})"),
        Err(OpError::Err(m)) => format!("Err(\"{m}\")"),
        Err(OpError::Trap(m)) => format!("TRAP(\"{m}\")"),
    };
    (balance, withdraw)
}

// ===========================================================================
// PART 1 — THE DEFECT, REPRODUCED
// ===========================================================================

/// The auditor's wipe, on the local replica, with numbers.
///
/// One `install_code --mode reinstall` with the SAME module and no code change.
/// Not an upgrade, not a downgrade, not a bad build: the identical bytes.
#[test]
fn finding23_a_reinstall_of_a_funded_table_destroys_every_balance() {
    let mut world = World::default_world();
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let (ledger_before, escrow_before, chips_before) = fund_a_table(&mut world);
    let alice_before = world.get_balance(alice);
    let bob_before = world.get_balance(bob);

    say!("\n--- FINDING 23: install_code --mode reinstall, same wasm, no code change ---");
    say!("wasm under test  sha256={}", wasms::table_canister_sha256());
    say!(
        "BEFORE   ledger_at_canister={ledger_before}  escrow={escrow_before}  chips={chips_before}"
    );
    say!("         alice escrow={alice_before}  bob escrow={bob_before}");

    let same_wasm = wasms::table_canister_wasm();
    let outcome = Mgmt::of(world.pic.reinstall_canister(
        world.table,
        same_wasm,
        encode_one(&world.config).expect("config"),
        Some(world.controller),
    ));
    say!("reinstall_canister(controller) -> {}", outcome.text());
    assert!(
        outcome.is_ok(),
        "THE DEFECT IS THAT THIS SUCCEEDS. If it now fails, FINDING 23 has been fixed by some \
         other route and this test must be rewritten to assert the new behaviour, not deleted."
    );

    let ledger_after = world.ledger_balance(world.table, None);
    let (alice_after, alice_withdraw) = player_view(&world, alice);
    let (bob_after, bob_withdraw) = player_view(&world, bob);

    say!("AFTER    ledger_at_canister={ledger_after}  (unchanged: the ICP is still there)");
    say!("         alice escrow={alice_after}   withdraw(5 ICP) -> {alice_withdraw}");
    say!("         bob   escrow={bob_after}   withdraw(5 ICP) -> {bob_withdraw}");
    say!(
        "DESTROYED {} e8s = {:.8} ICP of player claims, with the ledger untouched",
        escrow_before + chips_before,
        (escrow_before + chips_before) as f64 / ICP as f64
    );

    // The ledger did not move. The books did.
    assert_eq!(
        ledger_after, ledger_before,
        "the reinstall must not move money on the LEDGER -- that is the whole point: the ICP is \
         still at the canister's account and nobody can reach it"
    );
    assert_eq!(alice_after, 0, "alice's balance survived a reinstall?");
    assert_eq!(bob_after, 0, "bob's balance survived a reinstall?");
    assert!(
        alice_withdraw.contains("Err") || alice_withdraw.contains("TRAP"),
        "a player with a wiped balance must not be able to withdraw; got {alice_withdraw}"
    );
    assert!(
        ledger_before > 0,
        "the fixture must actually have money on the ledger or this proves nothing"
    );
}

/// The other half, which the wave-7 handover never acknowledged at all.
///
/// `uninstall_code` leaves a canister with no code, no state and no balances,
/// still holding every e8 on the ledger. There is not even a canister left to ask.
#[test]
fn finding23_an_uninstall_of_a_funded_table_leaves_money_with_no_code_to_claim_it() {
    let mut world = World::default_world();
    let (ledger_before, escrow_before, chips_before) = fund_a_table(&mut world);

    say!("\n--- FINDING 23: uninstall_code on a funded table ---");
    say!(
        "BEFORE   ledger_at_canister={ledger_before}  escrow={escrow_before}  chips={chips_before}"
    );

    let outcome = Mgmt::of(world.pic.uninstall_canister(world.table, Some(world.controller)));
    say!("uninstall_canister(controller) -> {}", outcome.text());
    assert!(
        outcome.is_ok(),
        "THE DEFECT IS THAT THIS SUCCEEDS. See the note on the reinstall test."
    );

    let ledger_after = world.ledger_balance(world.table, None);
    let status = world
        .pic
        .canister_status(world.table, Some(world.controller))
        .expect("canister_status");
    let alice = world.actor("alice");
    let probe = world
        .pic
        .query_call(world.table, alice, "get_balance", Encode!().unwrap());
    say!("AFTER    ledger_at_canister={ledger_after}  module_hash={:?}", status.module_hash);
    say!("         get_balance() as a player -> {}", match &probe {
        Ok(_) => "Ok".to_string(),
        Err(e) => format!("REJECTED {}", e.reject_message),
    });
    say!(
        "STRANDED {ledger_after} e8s = {:.8} ICP, at an address whose canister has no code",
        ledger_after as f64 / ICP as f64
    );

    assert_eq!(ledger_after, ledger_before, "the ledger must be untouched");
    assert!(
        status.module_hash.is_none(),
        "uninstall_code must leave the canister empty"
    );
    assert!(
        probe.is_err(),
        "a canister with no code cannot answer a player, so there is no recovery surface at all"
    );
}

/// FINDING 23c — the operator does not have to destroy it. They can TAKE it.
///
/// # Why this test exists, and why its absence was a defect in its own right
///
/// The two tests above prove DESTRUCTION and stop there, and for one wave the
/// project drew the wrong conclusion from that stopping point. The deposit screen
/// and the README's decision table both told a depositing player that their money
/// would sit at the canister's ledger address "unreachable by anybody, including
/// the operator", and named the worst case as destruction rather than theft. The
/// stated reason was that no ClearDeck method pays a controller.
///
/// That is a true statement about this code and an irrelevant one about a
/// controller, because **a controller replaces the code.** `install_code` installs
/// whatever module it is handed, and the canister's ledger account is spendable by
/// whatever is then running in it. The money never had to pass through a ClearDeck
/// method at all.
///
/// So this test does what the disclosure claimed was impossible, with the SAME
/// privilege and the SAME verb as the wipe: one `install_code --mode reinstall`,
/// with a 500-byte module (`tests/thief_canister`) that is not ClearDeck, and then
/// one `icrc1_transfer` into a wallet the operator owns.
///
/// Like the two above it is written to PASS while the defect is live. If it ever
/// fails, something has closed the controller seat and the test must be rewritten
/// to assert the new behaviour, not deleted.
///
/// In a real attack the module would be installed with `--mode upgrade` instead,
/// which preserves state, so every player's balance would keep reading normally
/// until the transfer cleared. `reinstall` is used here only because it needs no
/// state migration and makes the ledger arithmetic unambiguous.
#[test]
fn finding23c_the_operator_can_pay_the_ledger_balance_to_themselves() {
    #[derive(CandidType, Deserialize)]
    struct ThiefInit {
        ledger: Principal,
        thief: Principal,
    }

    let mut world = World::default_world();
    let operator = world.controller;
    let ledger = world.ledger;
    // The operator's OWN wallet: an ordinary principal they hold the key for, not
    // the canister and not a player's account. `actor` mints a fresh keypair.
    let operator_wallet = world.actor("carol");

    let (ledger_at_canister, escrow_before, chips_before) = fund_a_table(&mut world);
    let operator_before = world.ledger_balance(operator_wallet, None);

    say!("\n--- FINDING 23c: the operator does not have to destroy it. They can TAKE it ---");
    say!(
        "BEFORE   ledger_at_canister={ledger_at_canister}  escrow={escrow_before}  \
         chips={chips_before}"
    );
    say!("         operator_wallet={operator_before}");
    assert!(
        ledger_at_canister > 0,
        "the fixture must have real money on the ledger, or this proves nothing"
    );

    let thief = wasms::thief_canister_module();
    say!("thief module sha256={} ({} bytes, NOT ClearDeck)", thief.sha256, thief.bytes.len());
    let init = encode_one(&ThiefInit {
        ledger,
        thief: operator_wallet,
    })
    .expect("encode thief init");
    world
        .pic
        .reinstall_canister(world.table, thief.bytes.clone(), init, Some(operator))
        .expect("the controller installs a module that is not ClearDeck");
    say!("reinstall_canister(controller, thief_module) -> Ok");

    // Leave one ledger fee behind so the transfer is valid.
    let take = ledger_at_canister.saturating_sub(10_000);
    let raw = world
        .pic
        .update_call(world.table, operator, "steal", encode_one(take).unwrap())
        .expect("steal() reached the installed module");
    let msg: String = decode_one(&raw).unwrap_or_else(|_| "(non-string reply)".to_string());
    say!("steal({take}) -> {msg}");

    let operator_after = world.ledger_balance(operator_wallet, None);
    let canister_after = world.ledger_balance(world.table, None);
    let moved = operator_after.saturating_sub(operator_before);
    say!("AFTER    ledger_at_canister={canister_after}  operator_wallet={operator_after}");
    say!(
        "MOVED    {moved} e8s = {:.8} ICP of player deposits into a wallet the operator owns",
        moved as f64 / ICP as f64
    );

    assert!(
        operator_after > operator_before,
        "the operator's wallet did not grow, so the transfer did not land. If icrc1_transfer was \
         rejected, print the reject: the ledger fee or the account shape is the usual cause."
    );
    assert_eq!(
        moved, take,
        "the operator must receive exactly what the installed module transferred"
    );
    // The ledger fee comes out of the CANISTER's account as well as the transfer
    // amount, so the account does not land on `ledger_at_canister - take`: it lands
    // on zero. Writing the obvious subtraction here failed on the first run with
    // `left: 0, right: 10000` while 39.99990000 ICP had provably moved — the
    // instrument disagreeing with a correct result, which is the failure this
    // repository keeps re-learning in the other direction. Stated as the
    // conservation identity instead, so it cannot be off by a fee again.
    assert_eq!(
        canister_after + moved + 10_000,
        ledger_at_canister,
        "every e8 must be accounted for: what is left at the canister, plus what the operator \
         received, plus the one ledger fee, is what the canister held before"
    );
    assert_eq!(
        canister_after, 0,
        "the whole ledger balance was taken, so nothing is left at the canister's account"
    );
    say!(
        "RETRACTED  \"the operator cannot pay it to themselves\" is FALSE. The worst case for a \
         depositor is THEFT, and it profits the operator."
    );
}

// ===========================================================================
// PART 2 — THE PREMISE, MEASURED BEFORE ANYTHING IS BUILT ON IT
// ===========================================================================

/// Install the guardian on a fresh canister and return its principal.
fn install_guardian(world: &World, operator: Principal, guarded: &[Principal]) -> Principal {
    let module = wasms::guardian_canister_module();
    let g = world.pic.create_canister_with_settings(Some(operator), None);
    world.pic.add_cycles(g, 100_000_000_000_000);
    world.pic.install_canister(
        g,
        module.bytes.clone(),
        encode_one(&GuardianInit {
            operator,
            guarded: guarded.to_vec(),
        })
        .expect("guardian init arg"),
        Some(operator),
    );
    g
}

fn guardian_call<T>(
    world: &World,
    guardian: Principal,
    sender: Principal,
    method: &str,
    arg: Vec<u8>,
) -> Result<T, String>
where
    T: CandidType + serde::de::DeserializeOwned,
{
    match world.pic.update_call(guardian, sender, method, arg) {
        Err(e) => Err(format!("REJECTED {}", e.reject_message)),
        Ok(bytes) => match decode_one::<Result<T, String>>(&bytes) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(m)) => Err(m),
            Err(e) => Err(format!("reply decode failed for {method}: {e}")),
        },
    }
}

fn guardian_query<T>(world: &World, guardian: Principal, method: &str, arg: Vec<u8>) -> T
where
    T: CandidType + serde::de::DeserializeOwned,
{
    let bytes = world
        .pic
        // Anonymous ON PURPOSE. A timelock queue only protects a player if a
        // player -- with no key, no identity and no relationship to the operator
        // -- can read it.
        .query_call(guardian, Principal::anonymous(), method, arg)
        .unwrap_or_else(|e| panic!("{method} must not be rejected: {}", e.reject_message));
    decode_one::<T>(&bytes).unwrap_or_else(|e| panic!("{method} reply did not decode: {e}"))
}

/// Push a module into the TARGET canister's chunk store, through the guardian, and
/// return `(chunk_hashes, module_sha256_bytes)`.
fn upload_module_through_guardian(
    world: &World,
    guardian: Principal,
    operator: Principal,
    target: Principal,
    wasm: &[u8],
) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut hashes = Vec::new();
    for chunk in wasm.chunks(CHUNK_BYTES) {
        let h: Vec<u8> = guardian_call(
            world,
            guardian,
            operator,
            "upload_chunk",
            encode_args((target, chunk.to_vec())).expect("upload_chunk arg"),
        )
        .expect("upload_chunk through the guardian");
        hashes.push(h);
    }
    let module_hash = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(wasm);
        h.finalize().to_vec()
    };
    (hashes, module_hash)
}

/// **THE PREMISE.** Controllership is not transitive.
///
/// Hand the table's controller seat to the guardian canister, then try every
/// destructive management call as the principal who controls the guardian. If any
/// of them lands, the entire idea is dead and no amount of guardian code can save
/// it.
///
/// This test deliberately measures the operations the guardian does not expose,
/// straight from the operator's own key, because "the guardian has no such method"
/// is only worth something if there is no way round the guardian.
#[test]
fn guardian_premise_a_controller_of_the_guardian_is_not_a_controller_of_the_table() {
    let mut world = World::default_world();
    let operator = world.controller; // the key holder: controls the guardian
    let (ledger_before, escrow_before, chips_before) = fund_a_table(&mut world);
    let alice = world.actor("alice");
    let alice_before = world.get_balance(alice);

    let guardian = install_guardian(&world, operator, &[world.table]);
    say!("\n--- THE PREMISE: controllership is not transitive ---");
    say!("guardian wasm sha256={}", wasms::guardian_canister_module().sha256);
    say!("table    {}", world.table);
    say!("guardian {}   (controlled by the operator key {})", guardian, operator);

    // --- the handover ------------------------------------------------------
    world
        .pic
        .set_controllers(world.table, Some(operator), vec![guardian])
        .expect("handover: the current controller sets the guardian as the only controller");
    let status = world
        .pic
        .canister_status(world.table, Some(guardian))
        .expect("the guardian can read the table's status");
    say!(
        "AFTER HANDOVER  table controllers = {:?}",
        status.settings.controllers
    );
    assert_eq!(
        status.settings.controllers,
        vec![guardian],
        "the handover must leave the guardian as the ONLY controller"
    );
    say!(
        "BOOKS    ledger_at_canister={ledger_before}  escrow={escrow_before}  chips={chips_before}  alice={alice_before}"
    );

    // --- every destructive verb, straight from the operator's key -----------
    //
    // These go to `aaaaa-aa` as RAW ingress messages, not through pocket-ic's
    // convenience wrappers. Two reasons. First, that is literally what
    // `icp canister install --mode reinstall` submits, so the thing measured here
    // is the protocol's own controller check and not a helper's behaviour.
    // Second, `PocketIc::reinstall_canister` silently switches to a chunked
    // install for a module this size and `unwrap()`s the first management call,
    // so a REJECTION -- the outcome this test is about -- would arrive as a
    // harness panic rather than a result.
    let cfg = encode_one(&world.config).expect("config");
    let table = world.table;

    let attempts: Vec<(&str, Mgmt)> = vec![
        (
            "install_code --mode reinstall",
            mgmt(
                &world,
                table,
                operator,
                "install_code",
                Encode!(&mgmt_types::InstallCodeArgs {
                    mode: mgmt_types::CanisterInstallMode::Reinstall,
                    canister_id: table,
                    wasm_module: MINIMAL_WASM.to_vec(),
                    arg: cfg.clone(),
                    sender_canister_version: None,
                })
                .unwrap(),
            ),
        ),
        (
            "install_code --mode upgrade",
            mgmt(
                &world,
                table,
                operator,
                "install_code",
                Encode!(&mgmt_types::InstallCodeArgs {
                    mode: mgmt_types::CanisterInstallMode::Upgrade(None),
                    canister_id: table,
                    wasm_module: MINIMAL_WASM.to_vec(),
                    arg: cfg.clone(),
                    sender_canister_version: None,
                })
                .unwrap(),
            ),
        ),
        (
            "upload_chunk (step 1 of a chunked install)",
            mgmt(
                &world,
                table,
                operator,
                "upload_chunk",
                Encode!(&mgmt_types::UploadChunkArgs {
                    canister_id: table,
                    chunk: vec![0u8; 16],
                })
                .unwrap(),
            ),
        ),
        (
            "uninstall_code",
            mgmt(
                &world,
                table,
                operator,
                "uninstall_code",
                Encode!(&mgmt_types::UninstallCodeArgs {
                    canister_id: table,
                    sender_canister_version: None,
                })
                .unwrap(),
            ),
        ),
        (
            "update_settings(controllers=[operator])",
            mgmt(
                &world,
                table,
                operator,
                "update_settings",
                Encode!(&mgmt_types::UpdateSettingsArgs {
                    canister_id: table,
                    settings: mgmt_types::CanisterSettings {
                        controllers: Some(vec![operator]),
                        ..Default::default()
                    },
                    sender_canister_version: None,
                })
                .unwrap(),
            ),
        ),
        (
            "stop_canister",
            mgmt(
                &world,
                table,
                operator,
                "stop_canister",
                Encode!(&mgmt_types::CanisterIdRecord {
                    canister_id: table
                })
                .unwrap(),
            ),
        ),
        (
            "delete_canister",
            mgmt(
                &world,
                table,
                operator,
                "delete_canister",
                Encode!(&mgmt_types::CanisterIdRecord {
                    canister_id: table
                })
                .unwrap(),
            ),
        ),
        (
            "take_canister_snapshot",
            mgmt(
                &world,
                table,
                operator,
                "take_canister_snapshot",
                Encode!(&mgmt_types::TakeCanisterSnapshotArgs {
                    canister_id: table,
                    replace_snapshot: None,
                })
                .unwrap(),
            ),
        ),
        (
            "canister_status",
            mgmt(
                &world,
                table,
                operator,
                "canister_status",
                Encode!(&mgmt_types::CanisterIdRecord {
                    canister_id: table
                })
                .unwrap(),
            ),
        ),
    ];

    say!("\n  as the OPERATOR (controller of the guardian, NOT of the table):");
    let mut landed: Vec<&str> = Vec::new();
    for (name, outcome) in &attempts {
        say!("    {:<42} -> {}", name, outcome.text());
        if outcome.is_ok() {
            landed.push(name);
        }
    }

    // The books must be exactly where they were.
    let ledger_after = world.ledger_balance(world.table, None);
    let alice_after = world.get_balance(alice);
    say!(
        "\n  BOOKS AFTER  ledger_at_canister={ledger_after}  alice={alice_after}   (unchanged: {})",
        ledger_after == ledger_before && alice_after == alice_before
    );

    assert!(
        landed.is_empty(),
        "PREMISE FAILED. Controllership leaked: the operator reached the table directly with \
         {landed:?}. Every guardian design in this project rests on this not happening; if this \
         assertion ever fires, docs/SECURITY-FINDINGS.md FINDING 23 must be reopened with the \
         guardian marked as NOT a mitigation."
    );
    assert_eq!(alice_after, alice_before, "no balance may move");
    assert_eq!(ledger_after, ledger_before, "no ledger money may move");
}

/// The other direction of the premise, and the bad news half.
///
/// The guardian's own controller CAN replace the guardian's code. So a guardian
/// that its operator controls is worth nothing: replace it with a version that
/// forwards `install_code --mode reinstall` and the wipe is back, in two commands
/// instead of one.
///
/// This is why the deployed guardian's controller list is `[guardian]` and not
/// `[operator]`, and why that arrangement is the thing being tested, not an
/// afterthought.
#[test]
fn guardian_premise_a_guardian_whose_operator_controls_it_is_worth_nothing() {
    let world = World::default_world();
    let operator = world.controller;
    let guardian = install_guardian(&world, operator, &[world.table]);

    say!("\n--- THE PREMISE, BAD-NEWS HALF: who controls the guardian? ---");
    let before = world
        .pic
        .canister_status(guardian, Some(operator))
        .expect("status");
    say!("guardian controllers (as created) = {:?}", before.settings.controllers);

    // As created, the operator controls the guardian, so the operator can put
    // ANY code in it -- including code that reinstalls the table.
    let hostile = Mgmt::of(world.pic.reinstall_canister(
        guardian,
        wasms::guardian_canister_module().bytes.clone(),
        encode_one(&GuardianInit {
            operator,
            guarded: vec![],
        })
        .unwrap(),
        Some(operator),
    ));
    say!("  operator reinstalls the GUARDIAN            -> {}", hostile.text());
    assert!(
        hostile.is_ok(),
        "if this ever fails, the reasoning below has changed and must be re-derived"
    );
    say!("  => a guardian the operator controls is a two-command wipe, not a fix.");

    // The arrangement that closes it: the guardian controls itself.
    world
        .pic
        .set_controllers(guardian, Some(operator), vec![guardian])
        .expect("self-control handover");
    let after = world
        .pic
        .canister_status(guardian, Some(guardian))
        .expect("status as self");
    say!("\nguardian controllers (after handover) = {:?}", after.settings.controllers);
    assert_eq!(after.settings.controllers, vec![guardian]);

    let attempts: Vec<(&str, Mgmt)> = vec![
        (
            "install_code --mode reinstall (guardian)",
            Mgmt::of(world.pic.reinstall_canister(
                guardian,
                wasms::guardian_canister_module().bytes.clone(),
                encode_one(&GuardianInit {
                    operator,
                    guarded: vec![],
                })
                .unwrap(),
                Some(operator),
            )),
        ),
        (
            "install_code --mode upgrade (guardian)",
            Mgmt::of(world.pic.upgrade_canister(
                guardian,
                wasms::guardian_canister_module().bytes.clone(),
                encode_one(&GuardianInit {
                    operator,
                    guarded: vec![],
                })
                .unwrap(),
                Some(operator),
            )),
        ),
        (
            "uninstall_code (guardian)",
            Mgmt::of(world.pic.uninstall_canister(guardian, Some(operator))),
        ),
        (
            "update_settings(controllers=[operator])",
            Mgmt::of(
                world
                    .pic
                    .set_controllers(guardian, Some(operator), vec![operator]),
            ),
        ),
    ];
    say!("  as the OPERATOR, against a SELF-CONTROLLED guardian:");
    let mut landed = Vec::new();
    for (name, outcome) in &attempts {
        say!("    {:<42} -> {}", name, outcome.text());
        if outcome.is_ok() {
            landed.push(*name);
        }
    }
    assert!(
        landed.is_empty(),
        "a self-controlled guardian must be unreachable from the operator's key; {landed:?} landed"
    );
    say!("  => the ONLY route into the guardian's code is the guardian's own timelocked queue.");
}

// ===========================================================================
// PART 3 — WHAT THE GUARDIAN ACTUALLY DOES
// ===========================================================================

/// The guardian holds the seat, and the wipe is unreachable from every direction:
/// through the guardian's interface (no such method), round the guardian (not a
/// controller), and by asking the guardian to do it under a different name.
///
/// Then: the one thing it CAN do — a state-preserving upgrade — is executed for
/// real, through the chunk store, and the money is still there afterwards.
#[test]
fn guardian_holds_the_seat_the_wipe_is_unreachable_and_the_upgrade_still_works() {
    let mut world = World::default_world();
    let operator = world.controller;
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let (ledger_before, escrow_before, _) = fund_a_table(&mut world);
    let alice_before = world.get_balance(alice);
    let chips_before = chips_via_players(&world, &[alice, bob]);

    let guardian = install_guardian(&world, operator, &[world.table]);
    world
        .pic
        .set_controllers(world.table, Some(operator), vec![guardian])
        .expect("table handover");
    world
        .pic
        .set_controllers(guardian, Some(operator), vec![guardian])
        .expect("guardian self-control");

    say!("\n--- THE GUARDIAN HOLDS THE SEAT ---");
    say!("table    {} controllers=[guardian]", world.table);
    say!("guardian {} controllers=[guardian]", guardian);
    say!(
        "BOOKS    ledger_at_canister={ledger_before}  escrow={escrow_before}  chips={chips_before}  alice={alice_before}"
    );

    let cfg = guardian_query::<GuardianConfig>(&world, guardian, "get_config", Encode!().unwrap());
    say!("\nguardian config, read ANONYMOUSLY:");
    say!("  operator            {}", cfg.operator);
    say!("  guarded             {:?}", cfg.guarded);
    say!(
        "  notice periods      guarded={}s  self={}s  operator={}s",
        cfg.guarded_delay_secs, cfg.self_delay_secs, cfg.operator_delay_secs
    );
    say!("  refused operations  {:?}", cfg.refused_operations);
    assert!(
        cfg.refused_operations
            .iter()
            .any(|s| s == "install_code:reinstall"),
        "the guardian must publish its refusal list as data"
    );

    // --- 1. ask the guardian to do the wipe, by every name it might answer to
    say!("\n  every destructive verb, ASKED OF THE GUARDIAN as the operator:");
    let mut answered: Vec<String> = Vec::new();
    for method in [
        "reinstall",
        "reinstall_canister",
        "install_code",
        "uninstall",
        "uninstall_code",
        "delete_canister",
        "stop_canister",
        "start_canister",
        "update_settings",
        "set_controllers",
        "load_canister_snapshot",
        "take_canister_snapshot",
        "reset_table",
        "admin_reinit_table",
    ] {
        let r = world
            .pic
            .update_call(guardian, operator, method, Encode!(&world.table).unwrap());
        let line = match &r {
            Ok(_) => {
                answered.push(method.to_string());
                "ANSWERED".to_string()
            }
            Err(e) => format!("REJECTED {}", e.reject_message.lines().next().unwrap_or("")),
        };
        say!("    {:<26} -> {}", method, line);
    }
    assert!(
        answered.is_empty(),
        "the guardian answered a destructive method: {answered:?}. Nothing in this design \
         survives that."
    );

    // --- 2. round the guardian ---------------------------------------------
    let direct = mgmt(
        &world,
        world.table,
        operator,
        "install_code",
        Encode!(&mgmt_types::InstallCodeArgs {
            mode: mgmt_types::CanisterInstallMode::Reinstall,
            canister_id: world.table,
            wasm_module: MINIMAL_WASM.to_vec(),
            arg: encode_one(&world.config).unwrap(),
            sender_canister_version: None,
        })
        .unwrap(),
    );
    say!(
        "\n  operator -> table  install_code --mode reinstall  -> {}",
        direct.text()
    );
    assert!(!direct.is_ok(), "controllership leaked");

    // --- 3. the one thing it CAN do, executed for real -----------------------
    let wasm = wasms::table_canister_wasm();
    let (chunk_hashes, module_hash) =
        upload_module_through_guardian(&world, guardian, operator, world.table, &wasm);
    say!(
        "\n  uploaded {} bytes as {} chunk(s) into the TABLE's own chunk store",
        wasm.len(),
        chunk_hashes.len()
    );

    let id: u64 = guardian_call(
        &world,
        guardian,
        operator,
        "propose",
        encode_one(&GuardianAction::UpgradeGuarded {
            target: world.table,
            wasm_module_hash: module_hash.clone(),
            chunk_hashes: chunk_hashes.clone(),
            arg: encode_one(&world.config).unwrap(),
        })
        .unwrap(),
    )
    .expect("propose UpgradeGuarded");
    let wait =
        guardian_query::<Option<u64>>(&world, guardian, "seconds_until_ready", encode_one(id).unwrap());
    say!("  propose(UpgradeGuarded) -> proposal {id}, ready in {:?}s", wait);

    // Early execution must fail.
    let early: Result<(), String> =
        guardian_call(&world, guardian, operator, "execute", encode_one(id).unwrap());
    say!("  execute() BEFORE the notice period -> {:?}", early);
    assert!(early.is_err(), "the timelock must hold");

    // A player who is not the operator must not be able to drive the queue.
    let not_operator: Result<u64, String> = guardian_call(
        &world,
        guardian,
        alice,
        "propose",
        encode_one(&GuardianAction::TransferOperator { new_operator: alice }).unwrap(),
    );
    say!("  propose() as a PLAYER -> {:?}", not_operator);
    assert!(not_operator.is_err(), "only the operator may propose");

    world.advance(Duration::from_secs(cfg.guarded_delay_secs + 60));
    let late: Result<(), String> =
        guardian_call(&world, guardian, operator, "execute", encode_one(id).unwrap());
    say!("  execute() AFTER the notice period  -> {:?}", late);
    assert!(late.is_ok(), "the guardian must be able to upgrade: {late:?}");

    let status = world
        .pic
        .canister_status(world.table, Some(guardian))
        .expect("status via the guardian");
    let installed = status
        .module_hash
        .as_ref()
        .map(hex::encode)
        .unwrap_or_default();
    let ledger_after = world.ledger_balance(world.table, None);
    let alice_after = world.get_balance(alice);
    let chips_after = chips_via_players(&world, &[alice, bob]);
    say!("\n  AFTER THE UPGRADE");
    say!("    module_hash         {installed}");
    say!("    ledger_at_canister  {ledger_after}  (was {ledger_before})");
    say!("    alice escrow        {alice_after}  (was {alice_before})");
    say!("    chips on the table  {chips_after}  (was {chips_before})");

    assert_eq!(installed, wasms::table_canister_sha256());
    assert_eq!(alice_after, alice_before, "an upgrade must preserve balances");
    assert_eq!(ledger_after, ledger_before);
    assert_eq!(chips_after, chips_before);

    let history = guardian_query::<Vec<Proposal>>(&world, guardian, "history", Encode!().unwrap());
    say!(
        "\n  public record: {} finished proposal(s); newest = {:?} {:?}",
        history.len(),
        history.last().map(|p| p.kind.clone()),
        history.last().map(|p| p.status.clone())
    );
    assert_eq!(history.len(), 1);
    assert!(matches!(
        history[0].status,
        ProposalStatus::Executed { .. }
    ));
}

/// WHAT THE HANDOVER COSTS, measured rather than asserted.
///
/// The handover does more than close `install_code`. `require_controller()` inside
/// the table asks `ic_cdk::api::is_controller(caller)`, so the moment the guardian
/// is the only controller, EVERY controller-gated method on the table stops
/// answering the operator's key. That cuts both ways and both directions matter:
///
/// * the destructive ones go away — `reset_table` and `admin_reinit_table` are
///   docs/SECURITY-FINDINGS.md FINDING 07, and after the handover the key holder
///   cannot call them at all;
/// * so do the AUDIT ones. `admin_get_all_balances`, `admin_get_deposit_custody`
///   and `admin_audit_deposit_custody` — the surface FINDING 21 and FINDING 28
///   exist to have — become unreachable from the operator's key, and the guardian
///   exposes no passthrough, deliberately, because a passthrough that could carry
///   `admin_get_all_balances` could carry `admin_reinit_table`.
///
/// The second bullet is a real regression and it is written down here, in a test
/// that prints it, rather than left for the next auditor to find. What makes it
/// survivable is that not one of those methods is on a player's path to their
/// money: `get_balance`, `get_custody_status`, `refresh_deposit_custody`,
/// `claim_external_deposit`, `cash_out` and `withdraw` are all caller-scoped and
/// all still work. That is asserted here, not assumed.
#[test]
fn guardian_handover_closes_the_controller_gated_surface_including_the_audit_half() {
    let mut world = World::default_world();
    let operator = world.controller;
    let alice = world.actor("alice");
    fund_a_table(&mut world);

    let guardian = install_guardian(&world, operator, &[world.table]);
    world
        .pic
        .set_controllers(world.table, Some(operator), vec![guardian])
        .expect("handover");

    say!("\n--- WHAT THE HANDOVER COSTS ---");
    say!("  controller-gated methods, called with the OPERATOR key after the handover:");
    let mut still_answering = Vec::new();
    for (method, arg) in [
        ("reset_table", Encode!(&world.config).unwrap()),
        ("admin_reinit_table", Encode!(&world.config).unwrap()),
        ("admin_get_all_balances", Encode!().unwrap()),
        ("admin_get_deposit_custody", Encode!().unwrap()),
        ("admin_get_table_chips", Encode!().unwrap()),
    ] {
        let out = world
            .pic
            .query_call(world.table, operator, method, arg.clone())
            .or_else(|_| world.pic.update_call(world.table, operator, method, arg));
        let line = match &out {
            Err(e) => format!("REJECTED {}", e.reject_message.lines().next().unwrap_or("")),
            Ok(bytes) => match decode_one::<Result<candid::Reserved, String>>(bytes) {
                Ok(Err(m)) => format!("Err(\"{m}\")"),
                _ => {
                    still_answering.push(method);
                    "ANSWERED".to_string()
                }
            },
        };
        say!("    {:<28} -> {}", method, line);
    }
    assert!(
        still_answering.is_empty(),
        "after the handover the operator key must reach NO controller-gated method on the \
         table; {still_answering:?} still answered"
    );

    say!("\n  the player's own path to their money, same moment:");
    let balance = world.get_balance(alice);
    let custody = world.custody_status(alice);
    let refreshed = world.refresh_deposit_custody(alice);
    let withdrawn = world.withdraw(alice, 5 * ICP);
    say!("    get_balance(alice)             -> {balance}");
    say!(
        "    get_custody_status(alice)      -> escrow={} chips={} total={}",
        custody.escrow, custody.chips_at_table, custody.total
    );
    say!("    refresh_deposit_custody(alice) -> {}", refreshed.is_ok());
    say!("    withdraw(alice, 5 ICP)         -> {:?}", withdrawn);
    assert!(balance > 0, "the player must still be able to read their balance");
    assert!(
        withdrawn.is_ok(),
        "the player must still be able to take their money out after the handover: {withdrawn:?}"
    );
    say!("  => the handover removes the OPERATOR's reach, not the PLAYER's.");
}

/// The guardian's own escape hatch, executed for real, against a DIFFERENT MODULE.
///
/// A guardian with a bug on canisters holding money is a permanent loss of
/// upgradeability, so `UpgradeSelf` has to actually work — and it has to be
/// measured, because a canister upgrading ITSELF from inside an open call context
/// is exactly the kind of thing that is assumed to work and does not. It did not:
/// the first version of this canister awaited `install_chunked_code` on itself and
/// trapped inside `ic-cdk`'s callback, leaving the module unchanged. See
/// `self_upgrade` in src/guardian_canister/src/lib.rs for the transcript.
///
/// **The first version of THIS TEST could not see that.** It proposed the
/// guardian's own module and then asserted the installed hash equalled the module
/// it had proposed — which is true when nothing happens at all. It printed the
/// trap and reported `ok`. It now upgrades to `guardian_variant_module()`, a
/// different binary with `max_pending` changed, so the assertion is that the
/// module hash and an observable behaviour both CHANGED. Neither can happen
/// without the upgrade landing.
#[test]
fn guardian_can_replace_its_own_code_but_only_through_the_longer_notice_period() {
    let world = World::default_world();
    let operator = world.controller;
    let guardian = install_guardian(&world, operator, &[world.table]);
    world
        .pic
        .set_controllers(guardian, Some(operator), vec![guardian])
        .expect("self-control");

    say!("\n--- THE ESCAPE: can a self-controlled guardian replace its own code? ---");
    let cfg = guardian_query::<GuardianConfig>(&world, guardian, "get_config", Encode!().unwrap());
    let running = wasms::guardian_canister_module();
    let module = wasms::guardian_variant_module();
    say!("  running  {}  max_pending={}", running.sha256, cfg.max_pending);
    say!("  target   {}  (a different binary)", module.sha256);
    let (chunk_hashes, module_hash) =
        upload_module_through_guardian(&world, guardian, operator, guardian, &module.bytes);
    say!(
        "  uploaded {} bytes as {} chunk(s) into the guardian's own chunk store",
        module.bytes.len(),
        chunk_hashes.len()
    );

    // Something in the queue that must survive the self-upgrade, so this also
    // measures that UpgradeSelf really is an upgrade and not a reinstall.
    let survivor: u64 = guardian_call(
        &world,
        guardian,
        operator,
        "propose",
        encode_one(&GuardianAction::TransferOperator {
            new_operator: world.actor("carol"),
        })
        .unwrap(),
    )
    .expect("propose a survivor");

    let id: u64 = guardian_call(
        &world,
        guardian,
        operator,
        "propose",
        encode_one(&GuardianAction::UpgradeSelf {
            wasm_module_hash: module_hash.clone(),
            chunk_hashes,
            arg: encode_one(&GuardianInit {
                operator,
                guarded: vec![],
            })
            .unwrap(),
        })
        .unwrap(),
    )
    .expect("propose UpgradeSelf");
    say!(
        "  propose(UpgradeSelf) -> proposal {id}; self notice = {}s vs guarded {}s",
        cfg.self_delay_secs,
        cfg.guarded_delay_secs
    );
    assert!(
        cfg.self_delay_secs > cfg.guarded_delay_secs,
        "replacing the guardian must never be the QUICKEST route to a table"
    );

    // The guarded notice period is not enough for a self-upgrade.
    world.advance(Duration::from_secs(cfg.guarded_delay_secs + 60));
    let too_early: Result<(), String> =
        guardian_call(&world, guardian, operator, "execute", encode_one(id).unwrap());
    say!("  execute() at the GUARDED notice period -> {:?}", too_early);
    assert!(too_early.is_err());

    world.advance(Duration::from_secs(cfg.self_delay_secs));
    let out: Result<(), String> =
        guardian_call(&world, guardian, operator, "execute", encode_one(id).unwrap());
    say!("  execute() at the SELF notice period    -> {:?}", out);
    assert!(
        out.is_ok(),
        "the self-upgrade must be accepted; without it a guardian bug is permanent: {out:?}"
    );
    // The one-way install lands on the NEXT round, not inside `execute`.
    world.advance(Duration::from_secs(1));

    let status = world
        .pic
        .canister_status(guardian, Some(guardian))
        .expect("status");
    let installed = status.module_hash.as_ref().map(hex::encode).unwrap_or_default();
    say!(
        "  guardian module_hash after = {installed}  (was {})",
        running.sha256
    );
    assert_ne!(
        installed, running.sha256,
        "THE MODULE DID NOT CHANGE. The self-upgrade did not land, so the guardian's only \
         escape from its own bugs does not exist and the whole design is a one-way door."
    );
    assert_eq!(
        installed, module.sha256,
        "the self-upgrade must leave the guardian running the module it named"
    );

    // The state must have survived, which is what makes this an UPGRADE.
    let after = guardian_query::<GuardianConfig>(&world, guardian, "get_config", Encode!().unwrap());
    let still_open =
        guardian_query::<Option<Proposal>>(&world, guardian, "get_proposal", encode_one(survivor).unwrap());
    say!("  operator after  = {} (was {})", after.operator, cfg.operator);
    say!("  surviving proposal {survivor} = {:?}", still_open.as_ref().map(|p| (&p.kind, &p.status)));
    assert_eq!(
        after.operator, cfg.operator,
        "a self-UPGRADE must preserve the operator; if this is the anonymous principal the \
         guardian was RE-INITIALISED, which means the queue and the operator were destroyed"
    );
    assert!(
        still_open.is_some(),
        "the queue must survive the guardian's own upgrade, or the timelock is a fiction: an \
         operator could clear a pending proposal by upgrading around it"
    );
    assert_eq!(
        still_open.as_ref().unwrap().status,
        ProposalStatus::Pending,
        "the surviving proposal must still be pending with its original notice period"
    );
    assert_eq!(
        after.max_pending, 31,
        "the NEW module's behaviour must be the one on the wire (the variant sets \
         max_pending = 31); {} means the old code is still running",
        after.max_pending
    );

    // The self-upgrade goes out ONE-WAY, so `execute` cannot know it worked. The
    // only thing that can mark it executed is the new module's `post_upgrade`.
    let record = guardian_query::<Option<Proposal>>(
        &world,
        guardian,
        "get_proposal",
        encode_one(id).unwrap(),
    )
    .expect("the self-upgrade proposal must still be in the public record");
    say!("  self-upgrade proposal {id} = {:?}", record.status);
    assert!(
        matches!(record.status, ProposalStatus::Executed { .. }),
        "an UpgradeSelf is only marked Executed by post_upgrade, i.e. only when the new module \
         really booted; got {:?}",
        record.status
    );
}

// ===========================================================================
// PART 4 — THE SWEEP: EVERY METHOD ON THE WIRE, DRIVEN
// ===========================================================================

/// **THE TEST THAT CATCHES THE NEXT DOOR.**
///
/// # Why a fixed list of method names is not enough, measured
///
/// `guardian_holds_the_seat_...` above asks the guardian for `reinstall`,
/// `uninstall_code`, `delete_canister` and eleven other names it might answer to.
/// That was written first, and it is blind in the way this project's defects are
/// always blind: it can only refuse the doors somebody thought to knock on.
///
/// Measured, by adding one plausible method to the guardian — an
/// `emergency_reinstall(target, wasm_module_hash, chunk_hashes, arg)` that forwards
/// `install_chunked_code` with `mode = Reinstall`, i.e. the auditor's wipe with a
/// friendly name:
///
/// ```text
///   guardian_holds_the_seat_the_wipe_is_unreachable_and_the_upgrade_still_works ... ok   <-- BLIND
///   guardian_census_every_method_on_the_wire_is_classified_here                 ... FAILED
/// ```
///
/// The census caught the NAME. Nothing caught the BEHAVIOUR — and the census can
/// be satisfied by a future author writing the new name into `ALLOWED`, which is a
/// one-line change made by somebody who believes their method is safe.
///
/// A wasm string scan does not help either, and that was measured too: the clean
/// guardian's binary already contains `uninstall_code`, `delete_canister` and
/// `update_settings`, because `REFUSED_OPERATIONS` publishes exactly those words.
///
/// # What this does instead
///
/// It parses the committed `guardian_canister.did`, and for EVERY update method on
/// it — including ones this file has never heard of — synthesises arguments from
/// the declared types, filled with the values that make the call hostile rather
/// than harmless: the real table principal, the real module hash, the real chunk
/// hashes already sitting in the table's chunk store. Then it calls each one AS
/// THE OPERATOR against a funded table the guardian controls, and requires, after
/// every call:
///
///   * every player's escrow and chips are exactly what they were, and
///   * the table is running exactly the module it was running.
///
/// A method that can destroy a balance fails this whether or not anybody
/// remembered to name it.
#[test]
fn guardian_sweep_no_method_on_the_wire_can_touch_a_funded_table() {
    use candid::types::value::{IDLArgs, IDLField, IDLValue, VariantValue};
    use candid::types::{Type, TypeEnv, TypeInner};
    use candid_parser::{check_prog, IDLProg};

    let mut world = World::default_world();
    let operator = world.controller;
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let (ledger_before, _, _) = fund_a_table(&mut world);
    let escrow_before = [world.get_balance(alice), world.get_balance(bob)];
    let chips_before = chips_via_players(&world, &[alice, bob]);

    let guardian = install_guardian(&world, operator, &[world.table]);
    world
        .pic
        .set_controllers(world.table, Some(operator), vec![guardian])
        .expect("table handover");
    world
        .pic
        .set_controllers(guardian, Some(operator), vec![guardian])
        .expect("guardian self-control");

    let module_before = installed_module(&world, world.table, guardian);

    // The materials a hostile call would need, really uploaded, so a method that
    // wants to install something has everything it needs.
    let wasm = wasms::table_canister_wasm();
    let (chunk_hashes, module_hash) =
        upload_module_through_guardian(&world, guardian, operator, world.table, &wasm);

    say!("\n--- THE SWEEP: every update method on the guardian's wire, driven ---");
    say!(
        "  materials in the table's chunk store: {} chunk(s), module hash {}",
        chunk_hashes.len(),
        hex::encode(&module_hash)
    );
    say!(
        "  BEFORE  ledger={ledger_before}  escrow={escrow_before:?}  chips={chips_before}  module={module_before}"
    );

    // --- the committed interface, parsed --------------------------------
    let text = wasms::guardian_candid_text();
    let prog: IDLProg = text
        .parse()
        .unwrap_or_else(|e| panic!("guardian_canister.did does not PARSE: {e}"));
    let mut env = TypeEnv::new();
    let service = check_prog(&mut env, &prog)
        .unwrap_or_else(|e| panic!("guardian_canister.did does not TYPE-CHECK: {e}"))
        .expect("guardian_canister.did declares no service");
    let methods = env
        .as_service(&service)
        .expect("service type")
        .to_vec();

    // --- argument synthesis ---------------------------------------------
    /// The values a hostile call would want, and a CURSOR.
    ///
    /// `assign` names, per BLOB SLOT in argument order, which of `fills` goes
    /// there. Filling every blob slot with the same bytes is not enough and that
    /// was measured: the mutation's
    /// `emergency_reinstall(target, wasm_module_hash, chunk_hashes, arg)` has two
    /// blob slots that must differ — the real module hash in one and a real
    /// `TableConfig` in the other — and with one filling for both, every attempt
    /// was rejected for the WRONG reason (bad hash, or an install argument the
    /// table's `canister_init` trapped on) and the sweep called it safe twice.
    struct Ctx<'a> {
        principal: Principal,
        fills: &'a [(&'a str, Vec<u8>)],
        assign: &'a [usize],
        cursor: std::cell::Cell<usize>,
        blobs: Vec<Vec<u8>>,
        number: u64,
    }

    impl Ctx<'_> {
        fn next_blob(&self) -> Vec<u8> {
            let i = self.cursor.get();
            self.cursor.set(i + 1);
            let which = self.assign.get(i).copied().unwrap_or(0);
            self.fills[which.min(self.fills.len() - 1)].1.clone()
        }
    }

    fn synth(env: &TypeEnv, t: &Type, ctx: &Ctx<'_>, depth: u32) -> IDLValue {
        if depth > 6 {
            return IDLValue::Null;
        }
        let t = env.trace_type(t).unwrap_or_else(|_| t.clone());
        match t.as_ref() {
            TypeInner::Null => IDLValue::Null,
            TypeInner::Bool => IDLValue::Bool(true),
            TypeInner::Text => IDLValue::Text(String::new()),
            TypeInner::Nat => IDLValue::Nat(ctx.number.into()),
            TypeInner::Int => IDLValue::Int((ctx.number as i64).into()),
            TypeInner::Nat8 => IDLValue::Nat8(ctx.number as u8),
            TypeInner::Nat16 => IDLValue::Nat16(ctx.number as u16),
            TypeInner::Nat32 => IDLValue::Nat32(ctx.number as u32),
            TypeInner::Nat64 => IDLValue::Nat64(ctx.number),
            TypeInner::Int8 => IDLValue::Int8(ctx.number as i8),
            TypeInner::Int16 => IDLValue::Int16(ctx.number as i16),
            TypeInner::Int32 => IDLValue::Int32(ctx.number as i32),
            TypeInner::Int64 => IDLValue::Int64(ctx.number as i64),
            TypeInner::Float32 => IDLValue::Float32(0.0),
            TypeInner::Float64 => IDLValue::Float64(0.0),
            TypeInner::Principal => IDLValue::Principal(ctx.principal),
            TypeInner::Reserved => IDLValue::Reserved,
            TypeInner::Empty | TypeInner::Unknown => IDLValue::Null,
            TypeInner::Opt(inner) => IDLValue::Opt(Box::new(synth(env, inner, ctx, depth + 1))),
            TypeInner::Vec(inner) => {
                let inner_t = env.trace_type(inner).unwrap_or_else(|_| inner.clone());
                match inner_t.as_ref() {
                    // blob
                    TypeInner::Nat8 => IDLValue::Blob(ctx.next_blob()),
                    // vec blob -> the real chunk-hash list
                    TypeInner::Vec(b)
                        if matches!(
                            env.trace_type(b).unwrap_or_else(|_| b.clone()).as_ref(),
                            TypeInner::Nat8
                        ) =>
                    {
                        IDLValue::Vec(
                            ctx.blobs
                                .iter()
                                .map(|h| IDLValue::Blob(h.clone()))
                                .collect(),
                        )
                    }
                    _ => IDLValue::Vec(vec![synth(env, inner, ctx, depth + 1)]),
                }
            }
            TypeInner::Record(fields) => IDLValue::Record(
                fields
                    .iter()
                    .map(|f| IDLField {
                        id: f.id.as_ref().clone(),
                        val: synth(env, &f.ty, ctx, depth + 1),
                    })
                    .collect(),
            ),
            TypeInner::Variant(fields) => {
                let f = &fields[0];
                IDLValue::Variant(VariantValue(
                    Box::new(IDLField {
                        id: f.id.as_ref().clone(),
                        val: synth(env, &f.ty, ctx, depth + 1),
                    }),
                    0,
                ))
            }
            _ => IDLValue::Null,
        }
    }

    // Every variant arm, not just the first: `propose(Action)` is a variant and the
    // dangerous arm is never arm 0 by luck.
    fn arms(env: &TypeEnv, t: &Type) -> usize {
        let t = env.trace_type(t).unwrap_or_else(|_| t.clone());
        match t.as_ref() {
            TypeInner::Variant(f) => f.len().max(1),
            _ => 1,
        }
    }

    fn synth_arm(env: &TypeEnv, t: &Type, ctx: &Ctx<'_>, arm: usize) -> IDLValue {
        let traced = env.trace_type(t).unwrap_or_else(|_| t.clone());
        if let TypeInner::Variant(fields) = traced.as_ref() {
            if let Some(f) = fields.get(arm) {
                return IDLValue::Variant(VariantValue(
                    Box::new(IDLField {
                        id: f.id.as_ref().clone(),
                        val: synth(env, &f.ty, ctx, 1),
                    }),
                    arm as u64,
                ));
            }
        }
        synth(env, t, ctx, 0)
    }

    // MORE THAN ONE FILLING PER BLOB, AND HERE IS WHY.
    //
    // With `bytes` = the module hash only, the mutation test's hostile
    // `emergency_reinstall` DID reach `install_chunked_code(mode = Reinstall)` —
    // and the table's `canister_init` then trapped on the nonsense install
    // argument ("binary parser error: Unexpected bytes at byte offset 0"), the
    // message rolled back, and the sweep reported `ok`. The wipe was one valid
    // argument away and the gate called it safe. That is this project's standing
    // failure mode reproduced inside the instrument, so the sweep drives every
    // method once per plausible filling, and a REAL `TableConfig` encoding is one
    // of them.
    let fills: Vec<(&str, Vec<u8>)> = vec![
        ("module hash", module_hash.clone()),
        ("a valid TableConfig install arg", encode_one(&world.config).unwrap()),
        ("empty", Vec::new()),
    ];

    let mut driven = 0usize;
    let mut undrivable: Vec<String> = Vec::new();
    for (name, ty) in &methods {
        let func = env.as_func(ty).expect("method must be a function type");
        // Queries cannot change anything; the sweep is about updates.
        if func
            .modes
            .iter()
            .any(|m| matches!(m, candid::types::internal::FuncMode::Query))
        {
            continue;
        }
        let arm_count = func.args.iter().map(|a| arms(&env, a)).max().unwrap_or(1);
        let mut any = false;
        for arm in 0..arm_count {
        // How many blob slots does this method have? Count them once, then drive
        // the cross-product of fillings across those slots.
        let counting = Ctx {
            principal: world.table,
            fills: &fills,
            assign: &[],
            cursor: std::cell::Cell::new(0),
            blobs: chunk_hashes.clone(),
            number: 1,
        };
        for a in &func.args {
            let _ = synth_arm(&env, a, &counting, arm);
        }
        let slots = counting.cursor.get().min(3);
        let combos: Vec<Vec<usize>> = if slots == 0 {
            vec![Vec::new()]
        } else {
            let mut out = vec![Vec::new()];
            for _ in 0..slots {
                let mut next = Vec::new();
                for prefix in &out {
                    for f in 0..fills.len() {
                        let mut c = prefix.clone();
                        c.push(f);
                        next.push(c);
                    }
                }
                out = next;
            }
            out
        };
        for assign in &combos {
            let fill_name = if assign.is_empty() {
                "-".to_string()
            } else {
                assign
                    .iter()
                    .map(|i| fills[*i].0)
                    .collect::<Vec<_>>()
                    .join(" + ")
            };
            let ctx = Ctx {
                principal: world.table,
                fills: &fills,
                assign,
                cursor: std::cell::Cell::new(0),
                blobs: chunk_hashes.clone(),
                number: 1,
            };
            let values: Vec<IDLValue> = func
                .args
                .iter()
                .map(|a| synth_arm(&env, a, &ctx, arm))
                .collect();
            let Ok(bytes) = IDLArgs::new(&values).to_bytes_with_types(&env, &func.args) else {
                continue;
            };
            any = true;
            driven += 1;
            // RE-ARM THE MATERIALS BEFORE EVERY CALL.
            //
            // Without this the sweep is order-dependent and silently toothless:
            // `clear_chunk_store` sorts before `emergency_reinstall` in the
            // committed .did, so the mutation test's hostile method was driven
            // against an EMPTY chunk store and could not have installed anything
            // whatever it wanted to. It reported `ok`. A sweep whose verdict
            // depends on alphabetical order is not a sweep.
            let stored: Vec<Vec<u8>> = guardian_call(
                &world,
                guardian,
                operator,
                "stored_chunks",
                encode_one(world.table).unwrap(),
            )
            .unwrap_or_default();
            if stored.len() != chunk_hashes.len() {
                let (again, _) = upload_module_through_guardian(
                    &world, guardian, operator, world.table, &wasm,
                );
                assert_eq!(again, chunk_hashes, "re-upload must produce the same chunk hashes");
            }
            let out = world.pic.update_call(guardian, operator, name, bytes);
            let line = match &out {
                Ok(_) => "answered".to_string(),
                Err(e) => format!("REJECTED {}", e.reject_message.lines().next().unwrap_or("")),
            };
            say!("    {:<22} arm {arm}  blobs=[{:<48}] -> {line}", name, fill_name);

            // AFTER EVERY SINGLE CALL, not once at the end: a sweep that only
            // checks at the end cannot say WHICH door did it.
            let escrow_now = [world.get_balance(alice), world.get_balance(bob)];
            let chips_now = chips_via_players(&world, &[alice, bob]);
            let module_now = installed_module(&world, world.table, guardian);
            assert_eq!(
                escrow_now, escrow_before,
                "guardian method `{name}` (arm {arm}, blob = {fill_name}) CHANGED player escrow: \
                 {escrow_before:?} -> {escrow_now:?}. No method on this canister may reduce what \
                 the table owes."
            );
            assert_eq!(
                chips_now, chips_before,
                "guardian method `{name}` (arm {arm}, blob = {fill_name}) CHANGED chips"
            );
            assert_eq!(
                module_now, module_before,
                "guardian method `{name}` (arm {arm}, blob = {fill_name}) CHANGED the table's \
                 installed module with NO notice period. Only `execute` may ever do that, and \
                 only after its timelock."
            );
            assert_eq!(
                world.ledger_balance(world.table, None),
                ledger_before,
                "guardian method `{name}` (arm {arm}, blob = {fill_name}) moved money on the ledger"
            );
        }
        }
        if !any {
            undrivable.push(name.clone());
        }
    }

    say!(
        "\n  {driven} update call(s) driven across {} method(s); books and module unchanged",
        methods.len()
    );
    assert!(
        driven > 0,
        "the sweep drove nothing, so it proves nothing. Check the .did parse."
    );
    assert!(
        undrivable.is_empty(),
        "the sweep could not synthesise arguments for {undrivable:?}, so those methods were NOT \
         exercised. A method the sweep cannot drive is a method the sweep does not cover: extend \
         the synthesiser rather than leaving the hole."
    );
}

/// The module hash the table is really running, read through the guardian (the
/// only principal that can ask after the handover).
fn installed_module(world: &World, target: Principal, asker: Principal) -> String {
    world
        .pic
        .canister_status(target, Some(asker))
        .expect("canister_status via the controller")
        .module_hash
        .as_ref()
        .map(hex::encode)
        .unwrap_or_else(|| "NONE".to_string())
}

// ===========================================================================
// PART 5 — THE CENSUS
// ===========================================================================

/// Every method on the guardian's committed interface must be classified HERE.
///
/// Same protection as
/// `admin_custody::census_every_controller_gated_method_is_classified_here`, and
/// for the same reason: the way a custody surface grows a hole is that somebody
/// adds a method and no test notices. A guardian is only worth what its SMALLEST
/// interface is worth.
#[test]
fn guardian_census_every_method_on_the_wire_is_classified_here() {
    // Verbs that reach the management canister and are STATE-PRESERVING.
    const ALLOWED: &[&str] = &[
        "propose",             // opens a timelocked proposal; touches nothing
        "cancel",              // closes one
        "execute",             // the ONLY door to install_chunked_code(upgrade)
        "upload_chunk",        // writes to a chunk store, never to canister state
        "clear_chunk_store",   // empties one
        "stored_chunks",       // reads one
        "note_guarded",        // a label; grants nothing
    ];
    // Read-only, unauthenticated on purpose: a timelock nobody can read is not a
    // timelock.
    const PUBLIC_READS: &[&str] = &[
        "get_config",
        "list_proposals",
        "get_proposal",
        "history",
        "seconds_until_ready",
        "refused_operations",
    ];
    // Must NEVER appear. Each of these is a way to destroy or freeze a player's
    // money that the guardian exists to make unreachable.
    const FORBIDDEN: &[&str] = &[
        "reinstall",
        "install",
        "install_code",
        "uninstall",
        "uninstall_code",
        "delete_canister",
        "stop_canister",
        "start_canister",
        "update_settings",
        "set_controllers",
        "load_canister_snapshot",
        "take_canister_snapshot",
        "upload_canister_snapshot_data",
        "upload_canister_snapshot_metadata",
        "reset_table",
        "admin_reinit_table",
        "set_timelock",
        "set_delay",
        "emergency",
        "pause",
    ];

    let did = wasms::guardian_candid_text();
    let service = did
        .split_once("service :")
        .map(|(_, s)| s.to_string())
        .expect("guardian_canister.did has no service block");
    let methods = parse_service_methods(&service);
    say!("\n--- GUARDIAN CENSUS: {} methods on the wire ---", methods.len());

    let mut unclassified = Vec::new();
    for m in &methods {
        let class = if ALLOWED.contains(&m.as_str()) {
            "state-preserving"
        } else if PUBLIC_READS.contains(&m.as_str()) {
            "public read"
        } else {
            unclassified.push(m.clone());
            "UNCLASSIFIED"
        };
        say!("  {:<24} {}", m, class);
    }
    assert!(
        unclassified.is_empty(),
        "the guardian grew {} method(s) this census has never classified: {unclassified:?}. \
         Classify them here — as `state-preserving`, as a `public read`, or by deciding the \
         guardian must not have them — before the interface ships.",
        unclassified.len()
    );

    let present: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|f| methods.iter().any(|m| m == f))
        .collect();
    assert!(
        present.is_empty(),
        "the guardian's committed interface contains {present:?}. The entire value of this \
         canister is that those verbs do not exist on it."
    );

    // And the same list, straight from the running canister, as data.
    say!("  refused_operations (from the source) = {:?}", REFUSED_SNAPSHOT);
    for verb in REFUSED_SNAPSHOT {
        assert!(
            !methods.iter().any(|m| m == verb.split(':').next().unwrap()),
            "the guardian both refuses and exposes {verb}"
        );
    }
}

/// The list the guardian publishes, mirrored so the census can check the wire
/// against it without a replica.
const REFUSED_SNAPSHOT: &[&str] = &[
    "install_code:reinstall",
    "install_code:install",
    "uninstall_code",
    "delete_canister",
    "update_settings",
    "stop_canister",
    "start_canister",
    "load_canister_snapshot",
];

/// Method names out of a Candid `service : { ... }` block.
fn parse_service_methods(service: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in service.lines() {
        let line = line.trim();
        if line.starts_with("//") || line.is_empty() {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        if !rest.trim_start().starts_with('(') {
            continue;
        }
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        out.push(name.to_string());
    }
    out.sort();
    out.dedup();
    out
}
