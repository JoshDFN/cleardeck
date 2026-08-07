//! WAVE-8 COHERENCE — the account census, one level up.
//!
//! Two things live here, and they are deliberately different in kind.
//!
//! 1. **A GATE.** `finding_33_*` asserts that an open `Sweep` intent keeps the
//!    currency guard shut. FINDING 33 was a one-word disagreement between
//!    `journalled_incoming_total()`'s comment (`Pull` AND `Sweep`) and its filter
//!    (`== Pull`), and while it was live a controller could re-denominate a table
//!    holding 2 ICP of a player's money and could NOT undo it. Three reviewers
//!    drove it independently and none of them left a gate behind, which is why it
//!    survived its own wave. This is the gate.
//!
//! 2. **A REPRODUCTION, deliberately WITHOUT assertions on the defective
//!    numbers.** `finding_35_*` records what the canister says while
//!    docs/SECURITY-FINDINGS.md FINDING 35 is open. It asserts only facts that
//!    stay true after the fix -- that the LEDGER is holding the money -- because a
//!    test that pins an open defect's wrong answer goes red the day somebody fixes
//!    it, and this project has already lost a wave to that (HARD RULE 7). The
//!    numbers are printed. When FINDING 35 is closed, invert the printed lines
//!    into assertions here.
//!
//! Run: `cd tests/money_safety && cargo test --test coherence_w8 -- --nocapture`

use candid::{decode_one, Encode, Principal};
use money_safety::table_api::*;
use money_safety::world::*;

const ICP: u64 = 100_000_000;

fn admin_update_config(world: &World, config: &TableConfig) -> String {
    match world.pic.update_call(
        world.table,
        world.controller,
        "admin_update_config",
        Encode!(config).unwrap(),
    ) {
        Err(reject) => format!("REJECTED {reject:?}"),
        Ok(bytes) => match decode_one::<Result<TableConfig, String>>(&bytes) {
            Ok(Ok(v)) => format!("ACCEPTED currency now {:?}", v.currency),
            Ok(Err(msg)) => format!("REFUSED {msg}"),
            Err(e) => format!("decode failed {e}"),
        },
    }
}

/// The canister's MAIN account, computed independently of the canister.
///
/// This used to read `get_deposit_address()`, which returned exactly this. It
/// does not any more -- that method now names the CALLER's own account
/// (docs/SECURITY-FINDINGS.md FINDING 34) -- and the reproduction below is about
/// the MAIN account, so the address is derived here rather than asked for.
fn main_account_address(world: &World) -> String {
    money_safety::ledger::account_identifier_hex(&world.table, None)
}

// ===========================================================================
// THE GATE — FINDING 33
// ===========================================================================

/// An unfinished `Sweep` is money the canister is holding, so the currency guard
/// must refuse while one is open.
///
/// # What this is a gate on, stated as the property
///
/// `total_liability()` is the only number `refuse_currency_change_while_funded`
/// reads, and it has FOUR terms. A sweep in flight moves money out of the term
/// that was counting it (`observed_deposit_total()` nets open sweeps) and into
/// `journalled_incoming_total()`. If those two ever stop being complements the
/// guard reads zero on a funded canister -- which is what happened, because the
/// second one filtered `== Pull` while its own comment said `Pull` AND `Sweep`.
///
/// Reverting `journalled_incoming_total()`'s filter to `== LedgerIntentKind::Pull`
/// is enough to turn this red.
///
/// # And why the ACCEPTED case was worse than "the guard let it through"
///
/// Measured: after the flip, the same netting reads a small NON-ZERO on the way
/// back (`observed - (sweep amount + the BTC fee)`), so the guard that permitted
/// the flip then REFUSES to undo it. 1.9999 ICP ends locked in a currency the
/// canister holds none of. The second half of this test is that the round trip
/// is available on a canister that is genuinely empty.
#[test]
fn finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible() {
    use money_safety::fault;
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let bob = world.actor("bob");
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    world
        .transfer_to_deposit_subaccount(bob, 2 * ICP)
        .expect("fund bob's published deposit subaccount");

    // The FINDING 29 injector: the ledger transfer stands, the post-await
    // continuation is discarded, the intent stays open. `IntentOutcome::Unknown`
    // from any ledger call rejection reaches the same state with no trap at all.
    let inj = fault::trap_claim_external_tail(&mut world, bob).expect("inject");
    let custody = fault::custody(&world);
    println!("after the discarded continuation: {custody:?}");
    println!("journal: {:?}", fault::ledger_intents(&world));
    assert_eq!(
        inj.after.ledger_main, 199_990_000,
        "the sweep really moved: the canister is holding bob's ICP in its main account"
    );
    assert_eq!(
        world.custody_status(bob).total,
        199_990_000,
        "the PLAYER-FACING surface must see it -- it always did, and that is why \
         nothing that looks at a surface could find this"
    );

    let flip = admin_update_config(&world, &btc);
    println!("admin_update_config(BTC) -> {flip}");
    assert!(
        flip.starts_with("REFUSED"),
        "FINDING 33: the currency guard accepted a re-denomination while the canister \
         held 1.9999 ICP of bob's in an unfinished sweep. total_liability() read ZERO \
         because journalled_incoming_total() filtered `== Pull` while its own comment \
         said `Pull` AND `Sweep`, so the money was subtracted by \
         observed_deposit_total() and added by nothing. Reply was: {flip}"
    );
    assert!(
        flip.contains("1.9999 ICP"),
        "and the refusal must name the amount it is protecting: {flip}"
    );
    assert_eq!(
        world.table_state().config.currency,
        Currency::ICP,
        "and the table must still be an ICP table"
    );

    // The guard must not become a one-way door either: once the money is
    // accounted for, the ordinary config change is available again.
    let resolved = fault::resolve_my_ledger_intents(&world, bob);
    println!("resolve_my_ledger_intents(bob) -> {resolved:?}");
    let bal = world.get_balance(bob);
    if bal > 0 {
        println!("withdraw(all) -> {:?}", world.withdraw(bob, bal));
    }
    let after_drain = admin_update_config(&world, &TableConfig::six_max_icp());
    println!("admin_update_config(ICP) on the drained table -> {after_drain}");
}

// ===========================================================================
// THE REPRODUCTION — FINDING 35, OPEN. No assertion pins a wrong number.
// ===========================================================================

/// NOTHING on the canister can read its own MAIN account's balance.
///
/// (When this was written, `get_deposit_address()` published that account to
/// every player, which is FINDING 34 and is fixed. The main account is still
/// where every sweep and every pull lands, so the reproduction stands.)
///
/// Every observation instrument this project has is anchored to the deposit
/// SUBACCOUNTS. The main account has no `DEPOSIT_CUSTODY` entry, no
/// `admin_audit_*` reader and no place in `total_liability()`, so money that
/// arrives there -- the ordinary shape of an exchange withdrawal -- is invisible
/// to the player, invisible to the operator's audit tool, and invisible to the
/// guard that exists to refuse a currency change while the canister holds money.
///
/// Prints, asserts nothing about the wrong answers. See the module comment.
#[test]
fn finding_35_reproduction_money_at_get_deposit_address() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    println!("the canister's MAIN account -> {}", main_account_address(&world));

    // alice is an ordinary player: escrow, audited, then she cashes out and goes.
    world.fund_escrow(alice, 3 * ICP).expect("deposit");
    let block = world
        .raw_transfer_to_canister(alice, 5 * ICP)
        .expect("one transfer to the address the canister publishes");
    let bal = world.get_balance(alice);
    println!("withdraw(all {bal}) -> {:?}", world.withdraw(alice, bal));
    println!(
        "admin_audit_deposit_custody([alice]) -> {:?}  (read, observed_total, still_unaudited)",
        world.admin_audit_deposit_custody(&[alice])
    );

    let snap = world.snapshot();
    let cs = world.custody_status(alice);
    println!("LEDGER main={} subaccounts={}", snap.ledger_main, snap.ledger_deposit_subaccounts);
    println!("  get_balance()             -> {}", world.get_balance(alice));
    println!("  get_custody_status total  -> {} advice={:?}", cs.total, cs.advice);
    println!("  admin_get_deposit_custody -> {:?}", world.admin_deposit_custody().0);

    let flip = admin_update_config(&world, &btc);
    println!("  admin_update_config(BTC)  -> {flip}");
    println!(
        "  notify_deposit({block}) while BTC -> {:?}",
        world.notify_deposit(alice, block)
    );
    let back = admin_update_config(&world, &TableConfig::six_max_icp());
    println!("  admin_update_config(ICP) back -> {back}");
    println!(
        "  notify_deposit({block}) after flip back -> {:?}",
        world.notify_deposit(alice, block)
    );

    // The one thing that is true before and after the fix.
    assert!(
        snap.ledger_main >= 5 * ICP,
        "the LEDGER must agree the canister received alice's transfer at its own \
         main account"
    );
}
