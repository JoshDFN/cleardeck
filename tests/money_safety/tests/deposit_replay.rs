//! Deposit anti-replay: E-02 (fund-theft) and E-04, against the REAL ICP ledger.
//!
//! One on-ledger movement of ICP must produce at most ONE credit of escrow, for
//! the whole lifetime of the canister's state. Escrow is a claim on real ICP the
//! canister holds, so a second credit is withdrawable ICP created from nothing.
//!
//! This file owns the three deposit doors:
//!
//! ```text
//!   notify_deposit(block_index)   raw icrc1_transfer to the canister's main account
//!   deposit(amount)               ICRC-2 approve + icrc2_transfer_from pull
//!   claim_external_deposit()      sweep of the caller's per-player deposit subaccount
//! ```
//!
//! `dr00` is the THEFT reproducer and is the reason the rest exists: it builds the
//! exact intermediate source state the project was warned about (E-04's decode bugs
//! fixed, E-02's anti-replay hole still open), credits one real transfer twice, and
//! then WITHDRAWS real ICP in excess of everything that principal ever deposited.
//! Every other test here is a gate that fails if that hole reopens.
//!
//! Deliberately a separate binary from `invariants` / `regressions`: another owner
//! maintains those.

use candid::{Decode, Encode, Principal};
use money_safety::invariants::*;
use money_safety::ledger::{self, account_identifier};
use money_safety::legacy_ledger_shapes::as_the_real_ledger_returns_it as real;
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{assert_holds, wasms};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

const ICP: u64 = 100_000_000;
const FEE: u64 = ledger::TRANSFER_FEE;

/// Must equal `table_canister::MAX_VERIFIED_DEPOSITS`.
const MAX_VERIFIED_DEPOSITS: usize = 10_000;

/// `get_deposit_replay_state() -> (watermark, recorded_block_count)`.
fn deposit_replay_state(world: &World) -> (u64, u64) {
    let bytes = world
        .pic
        .query_call(
            world.table,
            Principal::anonymous(),
            "get_deposit_replay_state",
            Encode!().unwrap(),
        )
        .expect("get_deposit_replay_state must not be rejected");
    candid::decode_args::<(u64, u64)>(&bytes).expect("get_deposit_replay_state reply decode")
}

/// A submitted call's reply, decoded as the `Result<u64, String>` every money
/// method returns, so a race's outcome is readable rather than a byte dump.
fn show(r: &Result<Vec<u8>, pocket_ic::RejectResponse>) -> String {
    match r {
        Ok(bytes) => match candid::decode_one::<Result<u64, String>>(bytes) {
            Ok(Ok(v)) => format!("Ok({v})"),
            Ok(Err(e)) => format!("Err({e:?})"),
            Err(e) => format!("undecodable reply: {e}"),
        },
        Err(e) => format!("rejected: {e:?}"),
    }
}

/// Did a submitted call credit anything?
fn credited(r: &Result<Vec<u8>, pocket_ic::RejectResponse>) -> bool {
    matches!(r, Ok(bytes)
        if matches!(candid::decode_one::<Result<u64, String>>(bytes), Ok(Ok(_))))
}

// ---------------------------------------------------------------------------
// reading the ledger's own block log, independently of the canister
// ---------------------------------------------------------------------------

#[derive(candid::CandidType)]
struct GetBlocksArgs {
    start: u64,
    length: u64,
}

fn query_blocks(world: &World, start: u64, length: u64) -> real::QueryBlocksResponse {
    let bytes = world
        .pic
        .query_call(
            world.ledger,
            Principal::anonymous(),
            "query_blocks",
            Encode!(&GetBlocksArgs { start, length }).expect("query_blocks arg encode"),
        )
        .expect("query_blocks must not be rejected");
    Decode!(&bytes, real::QueryBlocksResponse).expect("query_blocks reply decode")
}

/// How many blocks the ledger has. The next block written gets this index.
fn chain_length(world: &World) -> u64 {
    query_blocks(world, 0, 0).chain_length
}

/// The `Transfer` operation recorded at `block`, or a panic naming what was there.
fn transfer_at(world: &World, block: u64) -> real::Transfer {
    let r = query_blocks(world, block, 1);
    assert_eq!(r.blocks.len(), 1, "no block at index {block}");
    match &r.blocks[0].transaction.operation {
        Some(real::Operation::Transfer(t)) => t.clone(),
        other => panic!("block {block} is not a Transfer: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// building the E-02-open variant of the canister
// ---------------------------------------------------------------------------
//
// The theft is a property of a SOURCE STATE that this repo passed through and was
// warned never to ship: E-04's two decode bugs fixed, E-02's anti-replay record
// still absent. Reconstructing it here, from the current tree, keeps the
// reproducer executable forever instead of being a story about a lost binary.
//
// Each reversal below is asserted to apply. If the fix is refactored, this test
// fails loudly with the hunk that no longer matches -- which is the correct
// outcome, because a reader must then re-establish that the hole is still shut.

/// The two reversals that reopen E-02, i.e. the two defences the fix added:
///
/// 1. the durable anti-replay record itself. Neutralising `claim_deposit_block`
///    reproduces "this block index is not remembered", which is exactly what the
///    shipped `periodic_cleanup` did to any index it had already forgotten and
///    what the shipped `deposit()` did to the block it had just written.
/// 2. the `spender` check, which is what tells a `transfer_from` block written by
///    `deposit()` apart from a plain send. The shipped `notify_deposit` ignored
///    that field.
const E02_REVERSALS: &[(&str, &str)] = &[
    (
        "fn claim_deposit_block(block_index: u64, who: Principal) -> Result<(), String> {\n    \
         // INVARIANT ENFORCEMENT POINT",
        "#[allow(unreachable_code, unused_variables)]\n\
         fn claim_deposit_block(block_index: u64, who: Principal) -> Result<(), String> {\n    \
         // E02_REVERSAL applied by tests/deposit_replay.rs: the durable anti-replay\n    \
         // record is absent.\n    \
         return Ok(());\n    \
         // INVARIANT ENFORCEMENT POINT",
    ),
    (
        "        if transfer.spender.as_deref() == Some(&own_account[..]) {",
        "        // E02_REVERSAL: notify_deposit ignores `spender`, as shipped.\n        \
         if false && transfer.spender.as_deref() == Some(&own_account[..]) {",
    ),
];

/// The record bound, shrunk from 10,000 to 6 and the trim slack from 1,000 to 2.
///
/// The property under test in `dr07` is the BOUND: that trimming the record raises
/// the watermark instead of forgetting, so a dropped index stays refused. The
/// value of the threshold is not part of that property, and reaching the real
/// threshold costs 10,001 real ledger transfers, which is minutes of PocketIC and
/// a ledger the size of a small mainnet. So `dr07` exercises the identical code
/// with a small threshold, and `dr08` runs the same scenario at the shipped
/// constants for anyone who wants the whole thing.
const SMALL_BOUND_EDITS: &[(&str, &str)] = &[
    (
        "const MAX_VERIFIED_DEPOSITS: usize = 10_000;",
        "const MAX_VERIFIED_DEPOSITS: usize = 6;",
    ),
    (
        "const VERIFIED_DEPOSITS_SLACK: usize = 1_000;",
        "const VERIFIED_DEPOSITS_SLACK: usize = 2;",
    ),
];

/// Must equal `SMALL_BOUND_EDITS`' replacement value.
const SMALL_BOUND_MAX: usize = 6;

fn repo_root() -> PathBuf {
    wasms::repo_root()
}

/// A table_canister wasm with E-02 reopened, built once per process.
fn e02_vulnerable_wasm() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            if let Ok(p) = std::env::var("CLEARDECK_E02_VULNERABLE_WASM") {
                let p = PathBuf::from(p);
                assert!(p.exists(), "CLEARDECK_E02_VULNERABLE_WASM={p:?} does not exist");
                return p;
            }
            build_variant("e02-open", E02_REVERSALS)
        })
        .clone()
}

/// A table_canister wasm identical to the one under test except that the
/// anti-replay record is bounded at `SMALL_BOUND_MAX`. Built once per process.
fn small_bound_wasm() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| build_variant("small-bound", SMALL_BOUND_EDITS))
        .clone()
}

/// Copy the workspace's Rust crates, apply `edits` to `table_canister`'s lib.rs,
/// and build a wasm from the result. Every edit must match exactly once; a hunk
/// that no longer matches fails the test loudly, which is the point -- a reader
/// then has to re-establish the property by hand.
fn build_variant(name: &str, edits: &[(&str, &str)]) -> PathBuf {
    let root = repo_root();
    let work = wasms::cache_dir().join(format!("variant-{name}"));
    let target = wasms::cache_dir().join(format!("variant-{name}-target"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(work.join("src")).expect("cannot create the variant work dir");

    for f in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        let from = root.join(f);
        if from.exists() {
            std::fs::copy(&from, work.join(f)).unwrap_or_else(|e| panic!("copy {f}: {e}"));
        }
    }
    for crate_dir in [
        "poker_core",
        "lobby_canister",
        "table_canister",
        "history_canister",
    ] {
        copy_tree(&root.join("src").join(crate_dir), &work.join("src").join(crate_dir));
    }

    let lib_path = work.join("src/table_canister/src/lib.rs");
    let mut src = std::fs::read_to_string(&lib_path).expect("cannot read the copied lib.rs");
    for (find, replace) in edits {
        let n = src.matches(*find).count();
        assert_eq!(
            n, 1,
            "variant '{name}': hunk matched {n} times, expected exactly 1. The deposit \
             anti-replay code has moved; re-read src/table_canister/src/lib.rs and \
             re-establish that a block index still cannot be credited twice, then \
             update this hunk. Hunk:\n{find}"
        );
        src = src.replace(*find, replace);
    }
    std::fs::write(&lib_path, src).expect("cannot write the patched lib.rs");

    let status = std::process::Command::new("cargo")
        .current_dir(&work)
        .env("CARGO_TARGET_DIR", &target)
        .args([
            "build",
            "-p",
            "table_canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .unwrap_or_else(|e| panic!("could not run cargo for variant '{name}': {e}"));
    assert!(status.success(), "building variant '{name}' failed");

    let out = target.join("wasm32-unknown-unknown/release/table_canister.wasm");
    assert!(out.exists(), "variant '{name}' produced no wasm at {out:?}");
    out
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("mkdir {to:?}: {e}"));
    for entry in std::fs::read_dir(from).unwrap_or_else(|e| panic!("read_dir {from:?}: {e}")) {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        // `target` would be gigabytes and is rebuilt anyway.
        if name == "target" {
            continue;
        }
        let src = entry.path();
        let dst = to.join(&name);
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {src:?}: {e}"));
        }
    }
}

/// A `World` whose table canister runs a variant build.
///
/// `World::new` always installs the module the harness just built from this tree
/// (deliberately: `docs/DEFECTS.md` H-01), so the variant is put in place with a
/// real `install_code --mode reinstall` as the very first thing that happens,
/// before any money exists. Everything after that is the ordinary `World` API
/// talking to a different binary.
fn world_with_variant(wasm_path: PathBuf, config: TableConfig, actors: &[&str]) -> World {
    let wasm = std::fs::read(&wasm_path)
        .unwrap_or_else(|e| panic!("cannot read the variant wasm {wasm_path:?}: {e}"));
    let world = World::new(config.clone(), actors);
    world
        .pic
        .reinstall_canister(
            world.table,
            wasm,
            candid::encode_one(&config).expect("table init arg encode"),
            Some(world.controller),
        )
        .expect("could not reinstall the table canister with the variant build");
    eprintln!(
        "    deposit_replay: variant build installed from {}",
        wasm_path.display()
    );
    world
}

// ---------------------------------------------------------------------------
// DR-00 -- THE THEFT
// ---------------------------------------------------------------------------

/// E-02, executed: one ICRC-2 deposit is credited TWICE and the excess is then
/// WITHDRAWN as real ICP. Runs against a build with E-04's decode bugs fixed and
/// E-02's anti-replay record absent -- the exact state DEFECTS.md's fix ordering
/// exists to prevent shipping.
///
/// The mechanism needs no privilege, no timing and no extreme values:
///
/// 1. `deposit(n)` moves `n` on the ledger and credits `n`. The block it writes
///    has `from` = the caller's account and `to` = the canister's account, which
///    is exactly the pair `notify_deposit` verifies. `notify_deposit` never looked
///    at `spender`, the one field that says a `transfer_from` did it.
/// 2. So `notify_deposit(that block)` credits the SAME `n` again.
/// 3. `withdraw` pays strictly against escrow, so the second credit leaves as
///    real ICP -- backed by the other players' deposits.
#[test]
fn dr00_theft_one_icrc2_deposit_credited_twice_then_withdrawn() {
    let world = world_with_variant(
        e02_vulnerable_wasm(),
        TableConfig::six_max_icp(),
        &["alice", "bob"],
    );
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    // Another player's honest money. This is what actually gets stolen: it is the
    // real ICP that backs the balance the attacker invents.
    world.fund_escrow(bob, 20 * ICP).expect("bob deposits");

    let alice_wallet_start = world.ledger_balance(alice, None);
    let stake = 3 * ICP;

    // --- 1. one honest ICRC-2 deposit ------------------------------------
    let before = chain_length(&world);
    world.fund_escrow(alice, stake).expect("alice deposits");
    let after = chain_length(&world);
    // approve wrote one block, transfer_from wrote the next.
    assert_eq!(after, before + 2, "approve + transfer_from wrote two blocks");
    let deposit_block = after - 1;

    let t = transfer_at(&world, deposit_block);
    assert_eq!(t.amount.e8s, stake, "block {deposit_block} is alice's deposit");
    assert_eq!(
        t.from,
        account_identifier(&alice, None).to_vec(),
        "its `from` is alice's own account -- what notify_deposit checks"
    );
    assert_eq!(
        t.to,
        account_identifier(&world.table, None).to_vec(),
        "its `to` is the canister's main account -- what notify_deposit checks"
    );
    assert_eq!(
        t.spender,
        Some(account_identifier(&world.table, None).to_vec()),
        "and `spender` is the canister: this was a transfer_from, not a plain send"
    );

    let escrow_once = world.get_balance(alice);
    let held_once = world.ledger_balance(world.table, None);
    assert_eq!(escrow_once, stake);

    // --- 2. the same block, credited a second time -----------------------
    let credited_twice = world
        .notify_deposit(alice, deposit_block)
        .expect("E-02: notify_deposit credits the ICRC-2 deposit's own block again");
    let held_twice = world.ledger_balance(world.table, None);

    assert_eq!(
        credited_twice,
        2 * stake,
        "escrow doubled from ONE on-ledger movement"
    );
    assert_eq!(
        held_twice, held_once,
        "and not one e8 more real ICP arrived: {held_once} -> {held_twice}"
    );
    println!(
        "DR-00 double credit: block {deposit_block} credited twice. escrow {escrow_once} -> \
         {credited_twice} while the canister's ledger balance stayed at {held_twice}"
    );

    // --- 3. withdraw the invented money ---------------------------------
    let owed = world.escrow_all().0;
    assert!(
        owed > held_twice,
        "the canister now owes {owed} but holds {held_twice}"
    );

    let take = 2 * stake;
    world
        .withdraw(alice, take)
        .expect("E-02 is THEFT: the invented balance withdraws as real ICP");

    let alice_wallet_end = world.ledger_balance(alice, None);
    let profit = alice_wallet_end as i128 - alice_wallet_start as i128;
    println!(
        "DR-00 THEFT: alice's own ledger wallet {alice_wallet_start} -> {alice_wallet_end} \
         (profit {profit} e8s) having deposited {stake} once"
    );
    assert_eq!(
        profit,
        stake as i128 - 3 * FEE as i128,
        "alice is up one whole deposit, minus the three ledger fees the round trip \
         costs her: icrc2_approve, icrc2_transfer_from, and the withdrawal transfer"
    );
    assert!(profit > 0, "alice ends with MORE real ICP than she started");

    // --- 4. the hole in the ledger it leaves -----------------------------
    let bobs_escrow = world.get_balance(bob);
    let held_end = world.ledger_balance(world.table, None);
    println!(
        "DR-00 SHORTFALL: bob's escrow is {bobs_escrow} but the canister holds only {held_end}"
    );
    assert!(
        bobs_escrow > held_end,
        "bob can no longer be paid what the canister says he owns: escrow {bobs_escrow} \
         vs holdings {held_end}"
    );
}

// ---------------------------------------------------------------------------
// DR-01..DR-0n -- the gates on the CURRENT build
// ---------------------------------------------------------------------------

/// E-04 fixed: a real transfer to the canister's main account is creditable
/// exactly once. The FIRST notify_deposit must SUCCEED (that is E-04) and the
/// second must not credit (that is E-02).
#[test]
fn dr01_notify_deposit_credits_a_real_transfer_exactly_once() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let sent = 5 * ICP;

    let block = world
        .raw_transfer_to_canister(alice, sent)
        .expect("raw transfer");
    let first = world
        .notify_deposit(alice, block)
        .expect("E-04: a legitimate transfer must be creditable at all");
    world.note_raw_deposit_credited(sent);
    assert_eq!(first, sent, "credited exactly what arrived");

    let before = world.snapshot();
    let second = world.notify_deposit(alice, block);
    let after = world.snapshot();
    assert!(
        second.is_err(),
        "the same block was credited twice: {second:?}"
    );
    let vs = check_no_credit(alice, "notify_deposit replayed", &before, &after);
    assert_holds(&vs, Invariant::M6NoDoublePay, "notify_deposit replayed");

    // and it is still refused after a real upgrade
    world.upgrade().expect("upgrade");
    let before = world.snapshot();
    assert!(
        world.notify_deposit(alice, block).is_err(),
        "a credited block became creditable again across an upgrade"
    );
    let after = world.snapshot();
    let vs = check_no_credit(alice, "notify_deposit replayed after upgrade", &before, &after);
    assert_holds(&vs, Invariant::M6NoDoublePay, "replay after upgrade");
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after replay attempts");
}

/// E-02 mechanism 2: the block written by `deposit()`'s own `icrc2_transfer_from`
/// must not be creditable through `notify_deposit`. This is DR-00's primitive,
/// asserted to be shut.
#[test]
fn dr02_the_icrc2_deposit_block_cannot_be_replayed_through_notify_deposit() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(bob, 20 * ICP).expect("bob deposits");

    let wallet_start = world.ledger_balance(alice, None);
    let stake = 3 * ICP;
    let before_len = chain_length(&world);
    world.fund_escrow(alice, stake).expect("alice deposits");
    let deposit_block = chain_length(&world) - 1;
    assert_eq!(deposit_block, before_len + 1);
    assert_eq!(world.get_balance(alice), stake);

    // Sweep every block index the ledger has, not just the deposit's own: any of
    // them crediting alice again is the same defect.
    let horizon = chain_length(&world) + 4;
    let mut credited: Vec<(u64, u64)> = Vec::new();
    for b in 0..horizon {
        // RETRY THE SAME INDEX after a rate-limit refusal. Without this the sweep
        // is vacuous on the only index that matters:
        // MAX_DEPOSIT_VERIFICATIONS_PER_MINUTE is 5, so `b == 5` is the 6th call
        // and is always rate-limited -- and in this fixture the ICRC-2 deposit
        // block is deterministically 5. The loop used to advance the clock and
        // then move on to b+1, so the named regression on the project's only
        // fund-theft primitive never presented the deposit block at all, and
        // stayed green with BOTH defences removed. docs/DEFECTS.md H-18.
        let mut attempts = 0;
        loop {
            let bal_before = world.get_balance(alice);
            let r = world.notify_deposit(alice, b);
            let bal_after = world.get_balance(alice);
            if bal_after > bal_before {
                credited.push((b, bal_after - bal_before));
            }
            let rate_limited = matches!(&r, Err(OpError::Err(m)) if m.contains("Too many"));
            if !rate_limited {
                break;
            }
            world.advance(Duration::from_secs(61));
            attempts += 1;
            assert!(
                attempts < 4,
                "block {b} stayed rate-limited across {attempts} clock advances; the sweep \
                 would silently skip it"
            );
        }
    }
    assert!(
        credited.is_empty(),
        "DOUBLE CREDIT (E-02): notify_deposit re-credited already-credited money at \
         {credited:?}. Deposit block was {deposit_block}."
    );
    assert_eq!(world.get_balance(alice), stake, "escrow unchanged by the sweep");

    // The withdrawal half of DR-00 must now be impossible.
    assert!(
        world.withdraw(alice, 2 * stake).is_err(),
        "alice withdrew more than she ever deposited"
    );
    world.advance(Duration::from_secs(61));
    world.withdraw(alice, stake).expect("her own deposit is still withdrawable");
    let wallet_end = world.ledger_balance(alice, None);
    let profit = wallet_end as i128 - wallet_start as i128;
    println!("DR-02: alice net {profit} e8s after deposit {stake} + full withdraw (fees only)");
    assert!(
        profit < 0,
        "a round trip must cost the ledger fees, never turn a profit: {profit}"
    );
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the sweep");
}

/// E-02, third door: `claim_external_deposit` sweeps the caller's own deposit
/// subaccount. One arrival there must produce one credit, the sweep block must
/// not be creditable through `notify_deposit`, and a second claim with nothing
/// left must credit nothing.
#[test]
fn dr03_a_subaccount_arrival_is_swept_and_credited_exactly_once() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let sent = 4 * ICP;

    world
        .transfer_to_deposit_subaccount(alice, sent)
        .expect("send to the deposit address");
    let first = world.claim_external_deposit(alice).expect("claim");
    assert_eq!(first, sent - FEE, "credited the arrival minus the sweep fee");

    let before = world.snapshot();
    let second = world.claim_external_deposit(alice);
    let after = world.snapshot();
    assert!(second.is_err(), "a second claim credited again: {second:?}");
    let vs = check_no_credit(alice, "claim_external_deposit replayed", &before, &after);
    assert_holds(&vs, Invariant::M6NoDoublePay, "claim_external_deposit replayed");

    // The sweep itself wrote a ledger block whose `to` IS the canister's main
    // account. It must not be creditable: its `from` is the canister's own
    // subaccount, not the caller's account.
    let sweep_block = chain_length(&world) - 1;
    let t = transfer_at(&world, sweep_block);
    assert_eq!(t.to, account_identifier(&world.table, None).to_vec());
    assert_eq!(
        t.from,
        account_identifier(&world.table, Some(ledger::deposit_subaccount(&alice))).to_vec(),
        "the sweep's `from` is the canister's own deposit subaccount"
    );
    let before = world.snapshot();
    assert!(
        world.notify_deposit(alice, sweep_block).is_err(),
        "the sweep block was credited to a player"
    );
    let after = world.snapshot();
    let vs = check_no_credit(alice, "notify_deposit on the sweep block", &before, &after);
    assert_holds(&vs, Invariant::M6NoDoublePay, "sweep block replay");
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the sweep replay");
}

/// Two `notify_deposit` calls for the same block, both in flight before either
/// executes its ledger query. Only one may credit.
#[test]
fn dr04_concurrent_notify_deposit_for_one_block_credits_once() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let sent = 6 * ICP;
    let block = world
        .raw_transfer_to_canister(alice, sent)
        .expect("raw transfer");

    let arg = candid::encode_one(block).unwrap();
    let a = world
        .pic
        .submit_call(world.table, alice, "notify_deposit", arg.clone())
        .expect("submit first");
    let b = world
        .pic
        .submit_call(world.table, alice, "notify_deposit", arg)
        .expect("submit second");
    let ra = world.pic.await_call(a);
    let rb = world.pic.await_call(b);

    println!("DR-04 concurrent notify_deposit: a={} b={}", show(&ra), show(&rb));
    let n = credited(&ra) as u32 + credited(&rb) as u32;
    assert_eq!(n, 1, "exactly one of two concurrent calls may credit, {n} did");
    assert_eq!(
        world.get_balance(alice),
        sent,
        "escrow credited once for one transfer"
    );
    world.note_raw_deposit_credited(sent);
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after concurrent notify");
}

/// Two `claim_external_deposit` calls in flight over ONE subaccount arrival. The
/// second must not credit a sweep that moved nothing.
#[test]
fn dr05_concurrent_claim_external_deposit_credits_one_arrival_once() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let sent = 4 * ICP;
    world
        .transfer_to_deposit_subaccount(alice, sent)
        .expect("send to the deposit address");

    let a = world
        .pic
        .submit_call(world.table, alice, "claim_external_deposit", Encode!().unwrap())
        .expect("submit first");
    let b = world
        .pic
        .submit_call(world.table, alice, "claim_external_deposit", Encode!().unwrap())
        .expect("submit second");
    let ra = world.pic.await_call(a);
    let rb = world.pic.await_call(b);
    println!("DR-05 concurrent claim: a={} b={}", show(&ra), show(&rb));

    let escrow = world.get_balance(alice);
    let moved = sent - FEE;
    assert!(
        escrow <= moved,
        "escrow {escrow} exceeds the {moved} that actually moved on the ledger"
    );
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after concurrent claims");
    assert_holds(&check_world(&world), Invariant::M1Conservation, "after concurrent claims");
}

/// Two `deposit()` calls in flight against ONE allowance. The ledger's allowance
/// accounting is what must stop the second, and escrow must never exceed what the
/// canister actually pulled.
#[test]
fn dr06_concurrent_icrc2_deposits_cannot_reuse_one_allowance() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let amount = 5 * ICP;
    world
        .approve(alice, amount + FEE)
        .expect("approve exactly one deposit");

    let arg = candid::encode_one(amount).unwrap();
    let a = world
        .pic
        .submit_call(world.table, alice, "deposit", arg.clone())
        .expect("submit first");
    let b = world
        .pic
        .submit_call(world.table, alice, "deposit", arg)
        .expect("submit second");
    let ra = world.pic.await_call(a);
    let rb = world.pic.await_call(b);
    println!("DR-06 concurrent deposit: a={} b={}", show(&ra), show(&rb));

    let escrow = world.get_balance(alice);
    let held = world.ledger_balance(world.table, None);
    assert!(
        escrow <= held,
        "escrow {escrow} exceeds the {held} the canister actually holds"
    );
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after concurrent deposits");
}

/// E-02 mechanism 1 -- the bound that FORGOT.
///
/// `periodic_cleanup` bounded `VERIFIED_DEPOSITS` by dropping the oldest block
/// indices and forgetting them, and a forgotten index passed the already-credited
/// check again. This test fills the record past the bound with real ICRC-2
/// deposits, makes the bound actually run, and then proves the indices it dropped
/// are REFUSED rather than creditable -- before and after a real upgrade.
///
/// It also pins the cost of that design, in the safe direction: a raw transfer
/// that was never claimed and has fallen below the watermark is refused too, with
/// an error saying the money is still on the ledger. Stranded is recoverable; a
/// double credit is not.
///
/// Runs against a build whose only difference from the module under test is the
/// value of `MAX_VERIFIED_DEPOSITS` (see `SMALL_BOUND_EDITS`), because reaching
/// the shipped threshold costs 10,001 real ledger transfers. `dr08` is the same
/// scenario at the shipped constants.
#[test]
fn dr07_bounding_the_record_refuses_dropped_blocks_instead_of_forgetting_them() {
    bound_scenario(small_bound_wasm(), SMALL_BOUND_MAX, "DR-07");
}

/// `dr07` at the shipped `MAX_VERIFIED_DEPOSITS`, against the module actually
/// under test: 10,001 real ICRC-2 deposits through the real ICP ledger.
///
///   cargo test --test deposit_replay -- --ignored dr08 --nocapture
///
/// Heavy enough to be fragile for reasons that have nothing to do with the
/// canister: it drives ~10,000 real ledger blocks and thousands of PocketIC
/// rounds, and the PocketIC sandbox processes have been observed dying mid-run on
/// a loaded machine. A failure here is only evidence about the canister if the
/// assertions failed; a transport or sandbox error is evidence about the host.
#[test]
#[ignore = "10,001 real ICRC-2 deposits through the real ledger: minutes, and a large PocketIC state"]
fn dr08_the_bound_at_shipped_scale() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    bound_scenario_on(world, MAX_VERIFIED_DEPOSITS, "DR-08");
}

fn bound_scenario(wasm: PathBuf, max_verified: usize, tag: &str) {
    let world = world_with_variant(wasm, TableConfig::six_max_icp(), &["alice", "bob"]);
    bound_scenario_on(world, max_verified, tag);
}

/// Fill the anti-replay record past its bound with real deposits, run the bound,
/// and assert that dropping an index refuses it rather than resurrecting it.
fn bound_scenario_on(mut world: World, max_verified: usize, tag: &str) {
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    // 1. a raw transfer that IS credited. Its block index must stay refused for
    //    ever, including once the bound drops its individual record.
    let credited_amount = 5 * ICP;
    let credited_block = world
        .raw_transfer_to_canister(alice, credited_amount)
        .expect("raw transfer");
    world
        .notify_deposit(alice, credited_block)
        .expect("E-04: a real transfer must be creditable once");
    world.note_raw_deposit_credited(credited_amount);

    // 2. a raw transfer that is NEVER claimed, so the stranding cost is visible.
    let stranded_amount = 2 * ICP;
    let stranded_block = world
        .raw_transfer_to_canister(alice, stranded_amount)
        .expect("raw transfer");

    let (watermark0, recorded0) = deposit_replay_state(&world);
    assert_eq!(watermark0, 0, "a fresh canister starts with no floor");
    assert_eq!(recorded0, 1, "one credited block recorded so far");

    // 3. fill the record past the bound with real ICRC-2 deposits. Each one is a
    //    real icrc2_transfer_from through the real ledger and records one block.
    let n = max_verified + 1;
    let each = 20_000u64; // the canister's minimum ICP deposit
    world
        .approve(bob, (each + FEE) * n as u64)
        .expect("one allowance covering every deposit");
    for i in 0..n {
        world
            .deposit(bob, each)
            .unwrap_or_else(|e| panic!("deposit {i} of {n} failed: {e:?}"));
        // At shipped scale this loop is thousands of real ledger round trips, so
        // say where it is rather than looking hung.
        if n > 100 && i % 500 == 0 {
            println!("{tag} filling the record: {i}/{n}");
        }
    }
    let (watermark_pre, recorded_pre) = deposit_replay_state(&world);
    println!("{tag} after {n} ICRC-2 deposits: watermark={watermark_pre} recorded={recorded_pre}");

    // 4. run the bound. periodic_cleanup trims at exactly MAX_VERIFIED_DEPOSITS and
    //    is reached through check_timeouts, at most once per CLEANUP_INTERVAL_NS.
    world.advance(Duration::from_secs(31));
    world.check_timeouts(alice).expect("check_timeouts");
    let (watermark, recorded) = deposit_replay_state(&world);
    println!("{tag} after the bound ran: watermark={watermark} recorded={recorded}");
    assert!(
        (recorded as usize) <= max_verified,
        "{tag}: the record is not bounded: {recorded} entries, bound {max_verified}"
    );
    assert!(
        watermark > credited_block && watermark > stranded_block,
        "{tag}: the bound must have swept past both early blocks: watermark={watermark} \
         credited_block={credited_block} stranded_block={stranded_block}"
    );

    // 5. a dropped block is REFUSED, by the watermark, and credits nothing.
    for (label, block) in [
        ("already credited", credited_block),
        ("never claimed", stranded_block),
    ] {
        let before = world.snapshot();
        let r = world.notify_deposit(alice, block);
        let after = world.snapshot();
        let msg = match &r {
            Err(OpError::Err(m)) => m.clone(),
            other => panic!("{tag}: E-02 REOPENED -- notify_deposit({block}) returned {other:?}"),
        };
        assert!(
            msg.contains("watermark"),
            "{tag}: {label} block {block} refused for the wrong reason: {msg}"
        );
        println!("{tag} {label} block {block} refused: {msg}");
        let vs = check_no_credit(alice, label, &before, &after);
        assert_holds(&vs, Invariant::M6NoDoublePay, label);
    }

    // 6. and across a real upgrade: the floor is persistent state, not a cache.
    world.upgrade().expect("upgrade");
    let (watermark_after, _) = deposit_replay_state(&world);
    assert_eq!(
        watermark_after, watermark,
        "{tag}: the watermark must survive an upgrade"
    );
    for block in [credited_block, stranded_block] {
        let before = world.snapshot();
        assert!(
            world.notify_deposit(alice, block).is_err(),
            "{tag}: E-02 REOPENED across an upgrade at block {block}"
        );
        let after = world.snapshot();
        let vs = check_no_credit(alice, "after upgrade", &before, &after);
        assert_holds(&vs, Invariant::M6NoDoublePay, "after upgrade");
    }

    // 7. the canister is never short: the stranding is in the safe direction.
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the bound ran");

    // 8. the watermark only ever moves up.
    world.advance(Duration::from_secs(31));
    world.check_timeouts(alice).expect("check_timeouts");
    let (watermark_final, _) = deposit_replay_state(&world);
    assert!(
        watermark_final >= watermark,
        "{tag}: the watermark went DOWN: {watermark} -> {watermark_final}"
    );
}

/// The hazard the E-02 fix itself introduces, closed.
///
/// `deposit()` now records the block index its own `icrc2_transfer_from` writes.
/// That creates a new interleaving to get wrong: a `notify_deposit` naming that
/// same block index can pass its cheap pre-flight check, suspend on
/// `query_blocks`, and resume AFTER `deposit()` has recorded and credited the
/// block. If `notify_deposit` credited on the strength of the pre-flight check it
/// would credit one movement twice, so the enforcement point is a second, ATOMIC
/// claim taken after the await and immediately before the credit.
///
/// Both calls are put in flight, `delay` rounds apart, for every delay from 0 to
/// 4 -- which walks the notify_deposit query across the round that writes the
/// block and the round that credits it. Whatever order the two continuations end
/// up in, one pull must produce exactly one credit. The reached interleaving is
/// printed for each delay so a reader can see which ones were actually covered.
#[test]
fn dr09_deposit_and_notify_deposit_racing_for_the_same_new_block() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let amount = 1 * ICP;
    let mut expected_escrow = 0u64;

    let mut refused_by_spender_check = 0;
    let mut refused_by_the_record = 0;
    for delay in 0..5u32 {
        // Past the 5-per-minute notify_deposit rate limit, so every delay gets a
        // real attempt rather than a rate-limit rejection.
        world.advance(Duration::from_secs(61));
        // The approve writes its own block, so the transfer_from lands on the next.
        world.approve(alice, amount + FEE).expect("approve");
        let pull_block = chain_length(&world);

        let a = world
            .pic
            .submit_call(
                world.table,
                alice,
                "deposit",
                candid::encode_one(amount).unwrap(),
            )
            .expect("submit deposit");
        for _ in 0..delay {
            world.pic.tick();
        }
        let b = world
            .pic
            .submit_call(
                world.table,
                alice,
                "notify_deposit",
                candid::encode_one(pull_block).unwrap(),
            )
            .expect("submit notify_deposit");
        let ra = world.pic.await_call(a);
        let rb = world.pic.await_call(b);
        println!(
            "DR-09 delay={delay} block={pull_block}: deposit={} notify_deposit={}",
            show(&ra),
            show(&rb)
        );

        // The pull really happened and really landed where we predicted.
        let t = transfer_at(&world, pull_block);
        assert_eq!(t.amount.e8s, amount, "block {pull_block} is the ICRC-2 pull");
        assert_eq!(
            t.spender,
            Some(account_identifier(&world.table, None).to_vec()),
            "and it is a transfer_from, so `spender` is this canister"
        );

        expected_escrow += amount;
        let escrow = world.get_balance(alice);
        let held = world.ledger_balance(world.table, None);
        assert_eq!(
            held, expected_escrow,
            "delay={delay}: {expected_escrow} has moved into the canister in total"
        );
        assert_eq!(
            escrow, expected_escrow,
            "delay={delay}: DOUBLE CREDIT -- escrow {escrow} for {expected_escrow} of movement"
        );
        assert_eq!(
            credited(&ra) as u32 + credited(&rb) as u32,
            1,
            "delay={delay}: exactly one of the two calls may report a credit"
        );
        let why = show(&rb);
        if why.contains("ICRC-2 pull performed by this canister") {
            // notify_deposit's ledger query DID see the pull, got into
            // verify_and_credit, and the `spender` field refused it.
            refused_by_spender_check += 1;
        } else if why.contains("already been credited") {
            // deposit() had already recorded the block.
            refused_by_the_record += 1;
        } else {
            panic!("delay={delay}: refused for an unexpected reason: {why}");
        }

        // And nothing can claim that block afterwards either.
        assert!(
            world.notify_deposit(alice, pull_block).is_err(),
            "delay={delay}: the pull block is still claimable"
        );
        assert_eq!(
            world.get_balance(alice),
            expected_escrow,
            "delay={delay}: the follow-up claim credited something"
        );
        assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the race");
        assert_holds(&check_world(&world), Invariant::M1Conservation, "after the race");
    }

    // Two independent defences cover this interleaving, and the delays reach both:
    //
    //   * the `spender` check, reached when notify_deposit's ledger query really
    //     did see the pull. This is the one that does not depend on timing at all:
    //     a transfer_from block whose spender is this canister is never creditable
    //     through notify_deposit, whenever it is presented.
    //   * the record, reached when deposit() had already claimed the block.
    //
    // The post-await claim inside verify_and_credit is a third, and by design it
    // should be UNREACHABLE here: the spender check refuses first. It is kept
    // because it is the only thing that would still hold if the spender check were
    // ever weakened, and because it is what makes "claim then credit with no await
    // in between" true of every door uniformly.
    println!(
        "DR-09: refused by the spender check {refused_by_spender_check} time(s), by the \
         recorded block index {refused_by_the_record} time(s), across 5 interleavings"
    );
    assert!(
        refused_by_spender_check > 0,
        "no interleaving got as far as verify_and_credit, so the `spender` check -- the \
         defence that does not depend on timing -- was never exercised. Widen the delay \
         range."
    );
    assert!(
        refused_by_the_record > 0,
        "no interleaving was refused by the recorded block index, so deposit()'s recording \
         was never exercised. Widen the delay range."
    );
}
