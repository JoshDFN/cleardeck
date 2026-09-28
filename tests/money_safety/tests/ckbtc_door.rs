//! THE ckBTC DEPOSIT DOOR, executed against a ledger for the first time.
//!
//! docs/SECURITY-FINDINGS.md FINDING 46; docs/DEFECTS.md E-104. FINDING 10 recorded
//! for three waves that `verify_ckbtc_deposit` "is executed by no test in this
//! repository"; T-35 that the local stack has no ckBTC ledger at all. `btc_table_1`
//! holds real ckBTC on mainnet.
//!
//! # What was wrong
//!
//! `notify_deposit(block)` on a BTC table routes to `verify_ckbtc_deposit`, which
//! accepted any transfer whose `to.owner` is the canister and whose `from.owner`
//! is the caller. It never read `to.subaccount`. The ICP door compares the full
//! 32-byte account identifier, so a transfer to a deposit SUBACCOUNT is refused
//! there and pointed at `claim_external_deposit`; the ckBTC door had no such
//! refusal, and `claim_external_deposit` sweeps the caller's subaccount on
//! whatever ledger the table uses. Send X sats to your own deposit address, call
//! `notify_deposit(block)`, be credited X; the sats are still at the address, so
//! the sweep credits X minus the fee AGAIN. Anti-replay does not help: the sweep
//! is its own new block.
//!
//! # How the ledger got here
//!
//! The ICP ledger module does not export `get_transactions`, the endpoint this
//! door reads blocks with, so a second ICP ledger cannot stand in for ckBTC. The
//! world installs the real ICRC-1 ledger module (the ckBTC ledger's code, pinned
//! by sha256 in `wasms.rs`) at `mxzaz-hqaaa-aaaar-qaada-cai`, the id the table
//! hardcodes, on a table with `btc_table_1`'s exact init args.
//!
//! Run: `cargo test --test ckbtc_door -- --test-threads=2`

use candid::{CandidType, Decode, Encode, Nat, Principal};
use money_safety::invariants::*;
use money_safety::ledger;
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::assert_holds;
use serde::Deserialize;

/// `Currency::BTC.transfer_fee()` in the canister: 10 satoshis.
const FEE: u64 = 10;

fn btc_world() -> World {
    World::new(TableConfig::heads_up_btc(), &["alice", "bob"])
}

fn ledger_at_deposit_address(world: &World, who: Principal) -> u64 {
    world.ledger_balance(world.table, Some(ledger::deposit_subaccount(&who)))
}

/// The ckBTC ledger's `log_length`: the index the NEXT block will get.
fn chain_length(world: &World) -> u64 {
    #[derive(CandidType, Deserialize)]
    struct Req {
        start: Nat,
        length: Nat,
    }
    #[derive(CandidType, Deserialize)]
    struct Resp {
        log_length: Nat,
    }
    let bytes = world
        .pic
        .query_call(
            world.ledger,
            Principal::anonymous(),
            "get_transactions",
            Encode!(&Req {
                start: Nat::from(0u64),
                length: Nat::from(0u64)
            })
            .unwrap(),
        )
        .expect("get_transactions must answer on the ckBTC ledger");
    let resp = Decode!(&bytes, Resp).expect("get_transactions reply decode");
    ledger::nat_to_u64(&resp.log_length)
}

fn refusal(out: &Outcome<u64>) -> Option<&str> {
    match out {
        Err(OpError::Err(msg)) => Some(msg.as_str()),
        _ => None,
    }
}

// ===========================================================================
// 0. THE LEDGER IS THE ONE THE TABLE NAMES
// ===========================================================================

/// The world's ledger is at the id the canister hardcodes for ckBTC, charges the
/// canister's ckBTC fee, and the table says it is a BTC table. Everything below
/// is about a different ledger if any of this is false.
#[test]
fn cb00_a_btc_world_talks_to_the_real_icrc_ledger_at_the_ckbtc_id() {
    let world = btc_world();
    assert_eq!(world.ledger, ledger::ckbtc_ledger_principal());
    assert_eq!(world.transfer_fee(), FEE);
    assert_eq!(world.config.currency, Currency::BTC);
    let alice = world.actor("alice");
    assert_eq!(world.ledger_balance(alice, None), ACTOR_START_E8S);
    assert_eq!(
        chain_length(&world),
        world.actors.len() as u64,
        "a fresh ledger holds exactly one mint block per funded actor"
    );
}

// ===========================================================================
// 1. THE REPRODUCER -- one arrival, two credits
// ===========================================================================

/// FINDING 46, driven exactly as the review described it: one transfer to the
/// player's OWN deposit address, `notify_deposit` on its block, then the sweep.
///
/// RED on the unfixed canister: `notify_deposit` credits 50,000 sats for a block
/// whose `to` is a deposit subaccount, the sweep credits 49,990 more, and the
/// table owes 99,990 sats against 49,990 it holds.
#[test]
fn cb01_one_ckbtc_subaccount_arrival_is_credited_exactly_once() {
    let mut world = btc_world();
    let alice = world.actor("alice");
    let sent: u64 = 50_000;

    let block = world
        .transfer_to_deposit_subaccount(alice, sent)
        .expect("send to the published ckBTC deposit address");
    assert_eq!(ledger_at_deposit_address(&world, alice), sent);

    // The block-index door must send this player to the sweep, not credit it.
    let notified = world.notify_deposit(alice, block);
    let sweep = world.claim_external_deposit(alice);
    let credited = world.get_balance(alice);
    let snap = world.snapshot();

    println!(
        "sent {sent} sats to alice's deposit subaccount (block {block})\n  \
         notify_deposit -> {notified:?}\n  claim_external_deposit -> {sweep:?}\n  \
         alice's escrow: {credited} sats; ledger holds {} sats for the table",
        snap.ledger_holdings()
    );

    assert_eq!(
        sweep.as_ref().ok().copied(),
        Some(sent - FEE),
        "the sweep is the door for a subaccount arrival and must credit it once, minus the fee"
    );
    assert!(
        notified.is_err(),
        "notify_deposit credited a transfer to a deposit SUBACCOUNT: {notified:?}. That \
         block is the one claim_external_deposit sweeps, so one arrival of {sent} sats is \
         now worth {credited} sats of escrow"
    );
    let msg = refusal(&notified).unwrap_or("");
    assert!(
        msg.contains("claim_external_deposit"),
        "the refusal must send the player to the door that works, as the ICP door's does: {msg:?}"
    );
    assert!(
        msg.contains("YOUR OWN deposit address"),
        "the refusal must say the money is at the caller's own address and not lost: {msg:?}"
    );
    assert_eq!(
        credited,
        sent - FEE,
        "ONE arrival of {sent} sats, credited {credited} sats"
    );
    assert!(
        snap.escrow_total <= snap.ledger_holdings(),
        "the table owes {} sats and holds {}: the difference is another player's ckBTC",
        snap.escrow_total,
        snap.ledger_holdings()
    );

    // The block is still a subaccount transfer after the sweep, and still refused.
    let again = world.notify_deposit(alice, block);
    assert!(again.is_err(), "credited after the sweep: {again:?}");
    assert_eq!(world.get_balance(alice), sent - FEE);
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the sweep");
}

// ===========================================================================
// 2. THE DOOR STILL WORKS FOR WHAT IT IS FOR
// ===========================================================================

/// The ckBTC door's legitimate case, executed for the first time: sats at the
/// shared MAIN account, credited by block index exactly once, and refused on
/// replay and across an upgrade. This is FINDING 10's `dr01` on the other ledger.
#[test]
fn cb02_notify_deposit_credits_a_main_account_ckbtc_transfer_exactly_once() {
    let mut world = btc_world();
    let alice = world.actor("alice");
    let sent: u64 = 50_000;

    let block = world
        .raw_transfer_to_canister(alice, sent)
        .expect("raw transfer to the main account");
    let first = world
        .notify_deposit(alice, block)
        .expect("a real transfer to the main account must be creditable on a ckBTC table");
    world.note_raw_deposit_credited(sent);
    assert_eq!(first, sent, "credited exactly what arrived");

    let second = world.notify_deposit(alice, block);
    assert!(second.is_err(), "the same ckBTC block was credited twice: {second:?}");
    assert_eq!(world.get_balance(alice), sent);

    world.upgrade().expect("upgrade");
    let third = world.notify_deposit(alice, block);
    assert!(
        third.is_err(),
        "a credited ckBTC block became creditable again across an upgrade: {third:?}"
    );
    assert_eq!(world.get_balance(alice), sent);
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after replay attempts");
}

/// A transfer whose `to.subaccount` is the explicit all-zero subaccount IS the
/// main account under ICRC-1, and some wallets always send it that way. The fix
/// must refuse deposit subaccounts, not every `Some`, or a wallet's honest deposit
/// to the main account would be stranded with no door at all.
#[test]
fn cb03_an_explicit_zero_subaccount_is_the_main_account_and_is_credited_once() {
    let mut world = btc_world();
    let alice = world.actor("alice");
    let sent: u64 = 30_000;

    let args = ledger::TransferArg {
        from_subaccount: None,
        to: ledger::Account {
            owner: world.table,
            subaccount: Some(vec![0u8; 32]),
        },
        amount: Nat::from(sent),
        fee: Some(Nat::from(FEE)),
        memo: None,
        created_at_time: None,
    };
    let bytes = world
        .pic
        .update_call(
            world.ledger,
            alice,
            "icrc1_transfer",
            Encode!(&args).unwrap(),
        )
        .expect("icrc1_transfer");
    let block = Decode!(&bytes, ledger::LedgerResult)
        .unwrap()
        .block()
        .expect("transfer to the zero subaccount");
    world.uncredited_raw_deposits = world.uncredited_raw_deposits.saturating_add(sent);
    assert_eq!(
        world.ledger_balance(world.table, None),
        sent,
        "the ledger itself files a zero-subaccount transfer under the main account"
    );

    let first = world.notify_deposit(alice, block);
    assert_eq!(
        first.as_ref().ok().copied(),
        Some(sent),
        "money at the MAIN account, addressed with an explicit zero subaccount, must be \
         creditable through the block-index door: {first:?}"
    );
    world.note_raw_deposit_credited(sent);
    assert!(world.notify_deposit(alice, block).is_err(), "credited twice");
    assert_eq!(world.get_balance(alice), sent);
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "zero subaccount");
}

// ===========================================================================
// 3. THE SAME SHAPE AGAINST ANOTHER PLAYER'S ADDRESS
// ===========================================================================

/// Bob pays into ALICE's deposit address and then presents the block. `from` is
/// bob and `to.owner` is the canister, so the unfixed door credits bob; the sats
/// are at alice's address, so alice's sweep credits her too. Two players paid
/// for one transfer.
///
/// With the fix, bob's block is refused (it went to a deposit address, and not
/// his own), nobody is told the money is lost, and alice's sweep credits it once.
#[test]
fn cb04_a_transfer_into_another_players_ckbtc_address_is_credited_to_its_owner_once() {
    let mut world = btc_world();
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let sent: u64 = 40_000;

    let block = world
        .transfer_to_deposit_subaccount_of(bob, alice, sent)
        .expect("bob pays into alice's deposit address");
    assert_eq!(ledger_at_deposit_address(&world, alice), sent);

    let bob_notified = world.notify_deposit(bob, block);
    let alice_notified = world.notify_deposit(alice, block);
    let sweep = world.claim_external_deposit(alice);
    let snap = world.snapshot();
    println!(
        "bob -> alice's address, {sent} sats (block {block})\n  \
         notify_deposit as bob -> {bob_notified:?}\n  notify_deposit as alice -> \
         {alice_notified:?}\n  alice's sweep -> {sweep:?}\n  escrow: bob {} alice {}; ledger \
         holds {}",
        world.get_balance(bob),
        world.get_balance(alice),
        snap.ledger_holdings()
    );

    assert!(
        bob_notified.is_err(),
        "bob was credited for a transfer that went to alice's deposit address: {bob_notified:?}"
    );
    let msg = refusal(&bob_notified).unwrap_or("");
    assert!(
        msg.contains("claim_external_deposit") && msg.contains("get_deposit_subaccount"),
        "the refusal must say which door credits a deposit address and how to check one: {msg:?}"
    );
    assert!(
        alice_notified.is_err(),
        "alice was credited by block index for money the sweep will credit again: \
         {alice_notified:?}"
    );
    assert_eq!(sweep.as_ref().ok().copied(), Some(sent - FEE));
    assert_eq!(world.get_balance(bob), 0, "bob sent the money; it is not his escrow");
    assert_eq!(world.get_balance(alice), sent - FEE);
    assert!(
        snap.escrow_total <= snap.ledger_holdings(),
        "the table owes {} and holds {}",
        snap.escrow_total,
        snap.ledger_holdings()
    );
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the stranger's probe");
}

// ===========================================================================
// 4. THE PULL THIS CANISTER MADE ITSELF
// ===========================================================================

/// `deposit()` on a ckBTC table pulls with `icrc2_transfer_from` and credits at
/// once; the block it writes has `from` = the caller and `to` = the main account,
/// so only `spender` tells it from a plain send. Two lines of defence stand in
/// front of a replay: `deposit()` records the pull's block index (the record
/// refuses it first, as "already been credited"), and the door's own `spender`
/// check behind that. Both were code-read only on the ckBTC ledger (FINDING 10);
/// this executes the pair and accepts either refusal, because which one fires
/// first is not the property -- that NEITHER credits is.
#[test]
fn cb05_the_ckbtc_door_refuses_the_icrc2_pull_this_canister_made_itself() {
    let world = btc_world();
    let alice = world.actor("alice");
    let amount: u64 = 20_000;

    let approve_block = world
        .approve(alice, amount + FEE)
        .expect("icrc2_approve on the ckBTC ledger");
    let before = chain_length(&world);
    let credited = world
        .deposit(alice, amount)
        .expect("deposit() must pull on a ckBTC table");
    assert_eq!(credited, amount);
    let after = chain_length(&world);
    assert_eq!(after, before + 1, "deposit() wrote exactly one block");
    let pull_block = before;
    assert!(pull_block > approve_block);

    let replay = world.notify_deposit(alice, pull_block);
    assert!(
        replay.is_err(),
        "the ICRC-2 pull deposit() already credited was credited again by block index: \
         {replay:?}"
    );
    let msg = refusal(&replay).unwrap_or("");
    assert!(
        msg.contains("ICRC-2 pull") || msg.contains("already been credited"),
        "the refusal must come from the block record or the spender check, not from a \
         ledger error that would clear on retry: {replay:?}"
    );
    assert_eq!(world.get_balance(alice), amount);
    let _ = world.notify_deposit(alice, approve_block);
    assert_eq!(world.get_balance(alice), amount, "an approve block is not a transfer");
    assert_holds(&check_world(&world), Invariant::M2LedgerReality, "after the pull replay");
}
