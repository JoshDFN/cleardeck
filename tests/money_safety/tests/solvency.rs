//! **CAN THIS CANISTER PAY EVERYONE IT OWES, AND CAN IT SAY SO?**
//!
//! docs/SECURITY-FINDINGS.md FINDING 35, docs/DEFECTS.md E-70, driven against the
//! real mainnet ICP ledger wasm on PocketIC.
//!
//! # The state this file exists for is live on mainnet
//!
//! Measured on `kieex-haaaa-aaaaj-qor3q-cai` (table_1) on 2026-08-06, immediately
//! after the six backend canisters were upgraded:
//!
//! ```text
//!   escrow claimed   940,640,001 e8s
//!   chips at table             0
//!   pot                        0
//!   ledger main account        740,640,001 e8s
//!   -------------------------------------------
//!   SHORTFALL        200,000,000 e8s   (2.00 ICP)
//! ```
//!
//! Every published deposit subaccount was audited and held nothing. The residue is
//! the old deposit double-credit, which is closed in the deployed code and left
//! this behind. **Nothing here tries to erase it.** There is deliberately no
//! method on this canister that lets anybody change what a player is owed, and
//! `no_setter_was_added_to_fix_the_books` below is the gate that keeps it that
//! way. The defect is that the canister could not SEE it or SAY it.
//!
//! # How the shape is reproduced locally
//!
//! [`drain_main_account_behind_the_canisters_back`] moves ICP out of the table
//! canister's own main ledger account without touching its books, which is the
//! ledger-side half of exactly what the double-credit left behind: books that
//! claim more than the chain holds. The canister is given no message about it,
//! which is the whole point -- money moves and the canister is not told.
//!
//! Run: `cd tests/money_safety && cargo test --test solvency -- --nocapture`

use candid::{decode_one, Decode, Encode, Nat, Principal};
use money_safety::invariants::{self, Exemptions, Severity};
use money_safety::ledger::{self, Account, LedgerResult, TransferArg};
use money_safety::table_api::*;
use money_safety::world::*;

const ICP: u64 = 100_000_000;

// ===========================================================================
// reproducing the mainnet shape on a local replica
// ===========================================================================

/// Move `amount` e8s out of the table canister's MAIN ledger account, signed as
/// the table canister itself, with no message to the canister.
///
/// This is the local stand-in for however the mainnet residue arose: the ledger
/// balance falls and the canister's books do not, which is the same state as
/// books that rose while the ledger did not. It is done at the LEDGER, so the
/// canister has no way to know it happened except by asking -- which is the
/// property under test.
///
/// Returns the block index.
fn drain_main_account_behind_the_canisters_back(
    world: &World,
    to: Principal,
    amount: u64,
) -> Result<u64, String> {
    let args = TransferArg {
        from_subaccount: None,
        to: Account {
            owner: to,
            subaccount: None,
        },
        amount: Nat::from(amount),
        fee: Some(Nat::from(ledger::TRANSFER_FEE)),
        memo: None,
        created_at_time: None,
    };
    let bytes = world
        .pic
        .update_call(
            world.ledger,
            world.table,
            "icrc1_transfer",
            Encode!(&args).expect("transfer arg encode"),
        )
        .map_err(|r| format!("{r:?}"))?;
    Decode!(&bytes, LedgerResult)
        .map_err(|e| format!("transfer reply decode: {e}"))?
        .block()
}

/// Read EVERY account the canister owns -- the main one and every deposit
/// subaccount it can enumerate -- and return the report.
///
/// `admin_audit_deposit_custody` reads the main account too as of FINDING 35,
/// so one controller call now covers the whole census. Before it did, the
/// operator's all-clear was structurally incapable of seeing the money.
fn read_every_account(world: &mut World, also: &[Principal]) -> SolvencyReport {
    world
        .admin_audit_deposit_custody(also)
        .expect("the controller audit must run");
    solvency_of(world)
}

fn solvency_of(world: &World) -> SolvencyReport {
    world
        .solvency()
        .expect("get_solvency() must answer an ANONYMOUS caller: a player has to be able to \
                 ask whether the table can pay them without permission")
}

/// `admin_update_config` as the controller, flattened to a one-line verdict.
fn flip(world: &World, config: &TableConfig) -> String {
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

// ===========================================================================
// 1. UNKNOWN IS NOT ZERO
// ===========================================================================

/// A canister that has never asked the ledger what it holds must say **I do not
/// know**, and must not say zero and must not say solvent.
///
/// Wave 8 learned this at the deposit subaccounts and did not apply it to the
/// main account, which is FINDING 35. The failing shape it protects against is
/// concrete: a never-read account folded into a total as a zero is how
/// `total_liability()` came to read zero on a canister holding 5 ICP.
#[test]
fn a_never_observed_main_account_reads_as_unknown_and_never_as_solvent() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 3 * ICP).expect("deposit");
    let world = world;

    let r = solvency_of(&world);
    println!("NEVER OBSERVED -> {:?}", r.verdict);
    println!("  main_account={:?} held={:?} diff={:?}", r.main_account, r.held, r.difference_e8s);
    println!("  summary: {}", r.summary);

    assert_eq!(
        r.verdict,
        SolvencyVerdict::Unknown,
        "a canister that has never read its own main account must answer Unknown, not an \
         all-clear. FINDING 35."
    );
    assert_eq!(r.main_account, None, "a never-read account must read as null, NEVER as 0");
    assert_eq!(r.main_observed_at_ns, None, "and it must say it has never been read");
    assert_eq!(r.held, None, "there is no honest 'held' total built on an unread account");
    assert_eq!(r.difference_e8s, None, "and therefore no honest difference either");
    assert!(
        r.summary.contains("I DO NOT KNOW"),
        "the words have to be in the summary, not only in the variant: {}",
        r.summary
    );
    // The player-facing surface carries the same answer.
    let cs = world.custody_status(alice);
    assert_eq!(
        cs.canister_solvency,
        SolvencyVerdict::Unknown,
        "get_custody_status must carry the same verdict; a player reading only that surface \
         must not be told the table is fine"
    );
    assert_eq!(cs.canister_shortfall_e8s, None);
    assert!(
        cs.components_sum_to_total(),
        "the FULL mirror has to add up, or a field has been silently dropped again \
         (docs/SECURITY-FINDINGS.md FINDING 36): {cs:?}"
    );

    // AND THE COUNT IS PUBLIC WHILE THE ROSTER IS NOT.
    //
    // `deposit_account_census()` is every principal holding escrow at this table.
    // `get_solvency()` is callable by anybody including an anonymous caller, so
    // publishing that list would turn a solvency instrument into a player roster:
    // an unknown has to be VISIBLE without being ENUMERABLE. The count says
    // "somebody's address is unread"; the names are for the controller, and for
    // the caller about their own address.
    assert!(
        r.deposit_accounts_never_observed_count > 0,
        "alice holds escrow and her deposit address has never been read, so the count must \
         say so: {r:?}"
    );
    assert!(
        r.deposit_accounts_never_observed.is_empty(),
        "an ANONYMOUS caller was handed the principals of the players at this table: {:?}",
        r.deposit_accounts_never_observed
    );
    assert!(
        !r.summary.contains(&alice.to_text()),
        "and the prose must not leak them either: {}",
        r.summary
    );
    let as_alice = world
        .pic
        .query_call(world.table, alice, "get_solvency", Encode!().unwrap())
        .map(|b| decode_one::<SolvencyReport>(&b).expect("decode"))
        .expect("get_solvency as alice");
    assert_eq!(
        as_alice.deposit_accounts_never_observed,
        vec![alice],
        "but a player must be told about their OWN unread address"
    );

    // And the guard treats the unknown as money, exactly as it does an unread
    // deposit subaccount.
    assert!(
        invariants::check_world(&world).is_empty(),
        "an honest Unknown is not itself a violation: {:?}",
        invariants::check_world(&world)
    );
}

// ===========================================================================
// 2. ANYONE CAN TAKE THE READING
// ===========================================================================

/// `refresh_solvency()` must be open to a principal with no relationship to this
/// table at all, must move no money, and must be safe to call repeatedly.
///
/// A solvency check a player has to ask an operator for is not a solvency check.
#[test]
fn refresh_solvency_needs_no_permission_and_moves_no_money() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 3 * ICP).expect("deposit");

    let before = world.snapshot();
    let stranger = Principal::self_authenticating(b"somebody-who-has-never-played-here");

    let first = world
        .refresh_solvency(stranger)
        .expect("a stranger must be able to take the reading");
    println!("stranger refresh_solvency -> {:?} {:?}", first.verdict, first.held);
    let second = world
        .refresh_solvency(Principal::anonymous())
        .expect("and so must an anonymous caller");
    let third = world
        .refresh_solvency(alice)
        .expect("and calling it again must be safe");
    // alice has a published deposit address nobody has read, so the honest answer
    // is still Unknown until somebody looks at it. Look at it.
    assert_eq!(
        third.verdict,
        SolvencyVerdict::Unknown,
        "an unread deposit address must hold the verdict at Unknown even when the main \
         account has just been read: {}",
        third.summary
    );
    let third = read_every_account(&mut world, &[alice]);

    let after = world.snapshot();
    assert_eq!(
        (before.ledger_main, before.ledger_deposit_subaccounts, before.escrow_total),
        (after.ledger_main, after.ledger_deposit_subaccounts, after.escrow_total),
        "refresh_solvency moved money. It is a measurement and it must move none."
    );
    assert_eq!(
        first.held, second.held,
        "three readings of an unchanged ledger must agree"
    );
    assert_eq!(second.held, third.held);
    assert_eq!(
        third.verdict,
        SolvencyVerdict::CanPayEveryone,
        "a table whose books and ledger agree, with every account read, can pay everyone: {}",
        third.summary
    );
    assert_eq!(third.main_account, Some(after.ledger_main));
    assert_eq!(third.difference_e8s, Some(0));
    assert!(third.main_observed_at_ns.is_some());

    // The same reading, on its own.
    let obs = world
        .refresh_main_account_custody(Principal::anonymous())
        .expect("refresh_main_account_custody must be open too");
    assert_eq!(obs.amount, after.ledger_main);
    assert_eq!(obs.credited_since, 0);
    assert_eq!(obs.debited_since, 0);

    assert!(
        invariants::check_world(&world).is_empty(),
        "{:?}",
        invariants::check_world(&world)
    );
}

// ===========================================================================
// 3. THE MAINNET SHAPE, REPORTED HONESTLY
// ===========================================================================

/// The 2.00 ICP shortfall, reproduced locally, and every new surface saying so.
///
/// This is the whole wave in one test. Before FINDING 35 was closed, every
/// assertion below was unreachable: there was no query that could express the
/// shortfall, no field on the player's own custody record that could carry it,
/// and the refusal a player got when the payout failed was
/// `InsufficientFunds { balance: Nat(740640001) }`.
#[test]
fn finding_35_a_canister_that_cannot_pay_everyone_says_so_on_every_surface() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    // Books and ledger agree: 9.40640001 ICP of escrow, all of it really there.
    world.fund_escrow(alice, 5 * ICP).expect("deposit");
    world.fund_escrow(bob, 4 * ICP + 40_640_001).expect("deposit");
    let healthy = read_every_account(&mut world, &[alice, bob]);
    println!("HEALTHY -> {:?} owed={} held={:?}", healthy.verdict, healthy.owed, healthy.held);
    assert_eq!(healthy.verdict, SolvencyVerdict::CanPayEveryone);

    // -- and now the mainnet shape: 2.00 ICP leaves the ledger and the books do
    //    not move. The canister is not told.
    let outside = Principal::self_authenticating(b"wherever-the-2-ICP-went");
    let block = drain_main_account_behind_the_canisters_back(&world, outside, 2 * ICP)
        .expect("the ledger must let the canister's own account be debited");
    println!("2.00 ICP left the canister's main account at block {block}");

    let snap = world.snapshot();
    println!(
        "LEDGER main={} escrow_total={} (short by {})",
        snap.ledger_main,
        snap.escrow_total,
        snap.escrow_total as i128 - snap.ledger_main as i128
    );
    assert!(
        snap.escrow_total > snap.ledger_main,
        "the fixture must actually produce a shortfall"
    );

    // ---- (a) THE HARNESS INVARIANT GOES RED, at a never-excusable severity ----
    let vs = invariants::check_insolvency_is_reported(&world, &snap);
    for v in &vs {
        println!("VIOLATION {} [{:?}] {}", v.check, v.severity, v.detail);
    }
    let short_leg = vs
        .iter()
        .find(|v| v.check == "owes_more_than_it_holds_across_every_account")
        .expect("OWES <= HOLDS must fire on a canister that cannot pay everyone");
    assert_eq!(
        short_leg.severity,
        Severity::FundCreation,
        "a shortfall is never excusable"
    );
    assert!(
        matches!(
            money_safety::documented::classify(short_leg),
            money_safety::documented::Disposition::Blocking(_)
        ),
        "and the register must not be able to excuse it"
    );
    assert!(
        !vs.iter().any(|v| v.check == "insolvency_is_not_reported_after_a_refresh"),
        "the canister DID have to say it, and it did not: {vs:#?}"
    );

    // ---- (b) get_solvency() says it, in numbers and in words ----------------
    let r = world.refresh_solvency(bob).expect("any player may refresh");
    println!("SOLVENCY -> {:?}", r.verdict);
    println!("  owed={} held={:?} diff={:?}", r.owed, r.held, r.difference_e8s);
    println!("  summary: {}", r.summary);
    assert_eq!(r.verdict, SolvencyVerdict::CannotPayEveryone);
    // A payout debits main by exactly the amount plus this ledger's fee for the
    // transfer that took it, so the shortfall is 2 ICP + one fee.
    let expected_short = 2 * ICP + ledger::TRANSFER_FEE;
    assert_eq!(
        r.shortfall_e8s,
        Some(expected_short),
        "the shortfall must be stated exactly: {}",
        r.summary
    );
    assert_eq!(r.difference_e8s, Some(-(expected_short as i128)));
    assert_eq!(r.main_account, Some(snap.ledger_main));
    assert!(
        r.summary.contains("CANNOT PAY EVERYONE"),
        "the words matter as much as the number: {}",
        r.summary
    );
    assert_eq!(
        r.owed,
        snap.escrow_total,
        "and the owed side must be the escrow it is short against"
    );
    // FINDING 37: the guard's own number is now readable without triggering the
    // guard, and it agrees with the published terms.
    assert_eq!(
        r.guard_liability,
        r.owed.saturating_sub(r.payouts_in_flight) + r.unattributed_at_main.unwrap_or(0),
        "guard_liability must be reconstructible from the published terms"
    );
    assert_eq!(
        r.unattributed_at_main,
        Some(0),
        "a canister that is SHORT has no unattributed surplus at its main account"
    );

    // ---- (c) the player's own custody record says it ------------------------
    for who in [alice, bob] {
        let cs = world.custody_status(who);
        assert_eq!(
            cs.canister_solvency,
            SolvencyVerdict::CannotPayEveryone,
            "get_custody_status is the surface a player already reads, and it has to carry \
             this: {cs:?}"
        );
        assert_eq!(cs.canister_shortfall_e8s, Some(expected_short));
        assert!(
            cs.advice.contains("owes more than it holds"),
            "and the advice has to say it in words, first: {}",
            cs.advice
        );
        assert!(
            cs.components_sum_to_total(),
            "the FULL mirror still has to add up: {cs:?}"
        );
    }

    // ---- (d) THE REFUSAL. A player whose withdrawal fails for want of canister
    //          funds must be told the canister is short, not handed a debug
    //          string. ------------------------------------------------------
    //
    // "The last people to ask will be the ones who are not paid" is not a figure
    // of speech, and this is it happening: alice asks first and is paid in full
    // out of money that was never all there, and bob -- who did nothing wrong and
    // whose balance is exactly right -- is the one the ledger turns away.
    let alice_balance = world.get_balance(alice);
    world
        .withdraw(alice, alice_balance)
        .expect("alice asks first and there is still enough for her");
    let bob_balance = world.get_balance(bob);
    println!(
        "alice withdrew {alice_balance}; bob is owed {bob_balance} and the canister now holds {}",
        world.snapshot().ledger_main
    );
    let err = world
        .withdraw(bob, bob_balance)
        .expect_err("bob's withdrawal cannot be paid: the canister does not have the money");
    let msg = format!("{err:?}");
    println!("WITHDRAW REFUSAL -> {msg}");
    assert!(
        msg.contains("THIS TABLE COULD NOT PAY YOU BECAUSE THIS CANISTER IS SHORT"),
        "the refusal must name the cause: {msg}"
    );
    assert!(
        msg.contains("Your escrow has been put back in full"),
        "and say the money is not gone: {msg}"
    );
    assert!(
        msg.contains("get_solvency()"),
        "and name the method that lets them check it: {msg}"
    );
    assert!(
        !msg.contains("InsufficientFunds { balance"),
        "and it must not be the ledger's debug string: {msg}"
    );
    assert_eq!(
        world.get_balance(bob),
        bob_balance,
        "the refund must be complete: a failed payout may not cost the player anything"
    );

    // ---- (e) the failure ITSELF was an observation --------------------------
    //
    // The ledger's refusal carried the real balance of the account it refused to
    // pay out of. That is the one fact this canister could not otherwise get, and
    // it arrived free at the moment it mattered.
    let after_refusal = solvency_of(&world);
    assert_eq!(
        after_refusal.main_account,
        Some(world.snapshot().ledger_main),
        "the InsufficientFunds refusal is a reading of the main account and must be written \
         down: {}",
        after_refusal.summary
    );
    assert_eq!(after_refusal.verdict, SolvencyVerdict::CannotPayEveryone);
}

// ===========================================================================
// 4. THE READING SURVIVES A REAL UPGRADE
// ===========================================================================

/// The observation is a custody record, so an upgrade may not lose it -- and if
/// it ever does, the honest state to land in is `Unknown`, never zero.
///
/// docs/SECURITY-FINDINGS.md FINDING 14: the field is `opt` at the TOP LEVEL of
/// `PersistentState` and a flat tuple inside, so state written before it existed
/// still restores. This drives a real `install_code --mode upgrade`.
#[test]
fn the_main_account_reading_survives_a_real_upgrade() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 4 * ICP).expect("deposit");
    drain_main_account_behind_the_canisters_back(
        &world,
        Principal::self_authenticating(b"elsewhere"),
        ICP,
    )
    .expect("drain");

    let before = world.refresh_solvency(alice).expect("refresh");
    assert_eq!(before.verdict, SolvencyVerdict::CannotPayEveryone);
    println!("BEFORE UPGRADE: {:?} short={:?}", before.verdict, before.shortfall_e8s);

    world.upgrade().expect("a same-version upgrade must be accepted");

    let after = solvency_of(&world);
    println!("AFTER UPGRADE:  {:?} short={:?}", after.verdict, after.shortfall_e8s);
    assert_eq!(
        after.verdict,
        SolvencyVerdict::CannotPayEveryone,
        "an upgrade that forgets the canister is insolvent re-opens FINDING 35 once per \
         deploy -- and this canister was upgraded on mainnet WHILE it was 2.00 ICP short"
    );
    assert_eq!(after.shortfall_e8s, before.shortfall_e8s);
    assert_eq!(after.main_account, before.main_account);
    assert_eq!(
        after.main_observed_at_ns, before.main_observed_at_ns,
        "the AGE of the reading has to survive too: a record that looks freshly taken after \
         every upgrade is claiming a freshness it does not have"
    );
    assert!(
        world.custody_status(alice).canister_shortfall_e8s.is_some(),
        "and the player-facing surface still carries it"
    );
}

// ===========================================================================
// 5. THE THING THAT MUST NOT BE BUILT
// ===========================================================================

/// **No method on this canister may change what a player is owed.**
///
/// `admin_restore_balance` was deleted once already because a compromised
/// controller key could mint arbitrary balances and withdraw real funds, and four
/// independent auditors flagged unilateral custody. The temptation this wave
/// creates is specific and strong: there is a real 2.00 ICP discrepancy on
/// mainnet and a one-line setter would make it disappear from every surface
/// written above. It would not return the money to anybody. It would only stop
/// the canister saying it was missing.
///
/// This test is the gate. It is a source-level check because a method that does
/// not exist cannot be called, and "we did not add it" is exactly the kind of
/// claim that quietly stops being true.
#[test]
fn no_setter_was_added_to_fix_the_books() {
    let src = std::fs::read_to_string(
        money_safety::wasms::repo_root().join("src/table_canister/src/lib.rs"),
    )
    .expect("lib.rs must be readable");

    for banned in [
        "fn admin_restore_balance",
        "fn admin_set_balance",
        "fn admin_adjust_balance",
        "fn admin_write_off",
        "fn admin_reconcile_balances",
    ] {
        assert!(
            !src.contains(banned),
            "`{banned}` is in src/table_canister/src/lib.rs. A controller who can edit a \
             balance can mint one and withdraw it. The 2.00 ICP shortfall on mainnet table_1 \
             is real and the canister's job is to REPORT it, not to make it disappear from \
             the report. docs/SECURITY-FINDINGS.md FINDING 35."
        );
    }

    // And the surfaces this wave added must be READ-ONLY: the two refresh calls
    // write an observation and nothing else.
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 3 * ICP).expect("deposit");
    drain_main_account_behind_the_canisters_back(
        &world,
        Principal::self_authenticating(b"elsewhere"),
        ICP,
    )
    .expect("drain");

    let escrow_before = world.escrow_all();
    for _ in 0..3 {
        let _ = world.refresh_solvency(alice);
        let _ = world.refresh_main_account_custody(alice);
    }
    assert_eq!(
        world.escrow_all(),
        escrow_before,
        "a solvency refresh CHANGED a balance. It is a measurement. If it can move money it \
         is the setter this test exists to refuse."
    );
    let r = solvency_of(&world);
    assert_eq!(
        r.verdict,
        SolvencyVerdict::CannotPayEveryone,
        "and calling it repeatedly must not talk the canister out of the shortfall: {}",
        r.summary
    );
}

// ===========================================================================
// 6. THE INSTRUMENT'S OWN BLIND SPOT
// ===========================================================================

/// The written-down main balance may be too LOW and may never be too HIGH.
///
/// Everything the report says rests on that asymmetry -- it is what makes
/// "I cannot pay everyone" trustworthy and a false all-clear impossible. It is
/// argued in the canister's comments; this drives it through the movements that
/// could break it, because the standing lesson here is that an argument in a
/// comment is not an instrument.
#[test]
fn the_written_down_main_balance_is_never_higher_than_the_ledger() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let mut checkpoints: Vec<(&str, u64, Option<u64>)> = Vec::new();
    let mut check = |world: &World, label: &'static str, out: &mut Vec<(&str, u64, Option<u64>)>| {
        let snap = world.snapshot();
        let claimed = snap.canister_solvency.as_ref().and_then(|r| r.main_account);
        out.push((label, snap.ledger_main, claimed));
        let vs = invariants::check_written_down_holdings_are_not_overstated(&snap);
        assert!(
            vs.is_empty(),
            "at '{label}': the canister claims more than the ledger holds. {vs:#?}"
        );
        assert!(
            invariants::check_solvency_report_is_coherent(&snap).is_empty(),
            "at '{label}': the report does not add up. {:#?}",
            invariants::check_solvency_report_is_coherent(&snap)
        );
    };

    world.fund_escrow(alice, 5 * ICP).expect("deposit");
    world.refresh_solvency(alice).expect("reading");
    check(&world, "after the first reading", &mut checkpoints);

    // A pull AFTER the reading: main goes up, and the record must follow without
    // ever getting ahead of the chain.
    world.fund_escrow(bob, 3 * ICP).expect("deposit");
    check(&world, "after a pull, no new reading", &mut checkpoints);

    // A sweep from a deposit subaccount, likewise.
    world
        .transfer_to_deposit_subaccount(alice, 2 * ICP)
        .expect("external wallet transfer");
    world
        .claim_external_deposit(alice)
        .expect("sweep");
    check(&world, "after a sweep, no new reading", &mut checkpoints);

    // A payout: main goes down, and the record must come down with it.
    let bal = world.get_balance(bob);
    world.withdraw(bob, bal).expect("withdraw");
    check(&world, "after a payout, no new reading", &mut checkpoints);

    // A raw transfer in with no message at all -- the one movement the canister
    // cannot learn about. The record must be too LOW here, never too high.
    world
        .raw_transfer_to_canister(alice, ICP)
        .expect("an exchange withdrawal lands with no message");
    check(&world, "after an unannounced arrival", &mut checkpoints);

    for (label, ledger_main, claimed) in &checkpoints {
        println!("{label:38} ledger_main={ledger_main:>12} claimed={claimed:?}");
        if let Some(c) = claimed {
            assert!(
                c <= ledger_main,
                "at '{label}' the canister claimed {c} in an account the ledger says holds \
                 {ledger_main}"
            );
        }
    }
    // And the last checkpoint must be strictly low, or the fixture never exercised
    // the one-directional property at all.
    let (_, last_ledger, last_claim) = checkpoints.last().copied().unwrap();
    assert!(
        last_claim.unwrap() < last_ledger,
        "the unannounced arrival should leave the written-down balance BELOW the ledger; the \
         fixture is not testing what it claims to test"
    );
}

// ===========================================================================
// 7. THE OPERATOR'S AUDIT TOOL CAN SEE IT NOW
// ===========================================================================

/// `admin_audit_deposit_custody` answered `(N audited, 0 held, 0 unaudited)` -- an
/// all-clear, in the affirmative -- on a canister that could not pay its players,
/// because it iterated the deposit subaccounts and the money was not in one.
///
/// Its tuple shape is load-bearing for existing readers and cannot grow, so the
/// fix is that it now READS the main account and the reading lands where every
/// surface can see it.
#[test]
fn the_operator_audit_now_reads_the_main_account_too() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 6 * ICP).expect("deposit");
    drain_main_account_behind_the_canisters_back(
        &world,
        Principal::self_authenticating(b"elsewhere"),
        3 * ICP,
    )
    .expect("drain");

    // Before the audit the canister has never looked.
    assert_eq!(solvency_of(&world).verdict, SolvencyVerdict::Unknown);

    let audit = world
        .admin_audit_deposit_custody(&[alice])
        .expect("the controller audit must run");
    println!("admin_audit_deposit_custody -> {audit:?} (read, observed_total, still_unaudited)");

    let r = solvency_of(&world);
    println!("after the audit: {:?} -- {}", r.verdict, r.summary);
    assert_eq!(
        r.verdict,
        SolvencyVerdict::CannotPayEveryone,
        "the operator's own audit tool must no longer be able to return an all-clear on a \
         canister that cannot pay. docs/SECURITY-FINDINGS.md FINDING 35."
    );
    assert_eq!(r.main_account, Some(world.snapshot().ledger_main));
}

// ===========================================================================
// 8. THE CURRENCY GUARD, WHICH WAS BLIND AT THIS ACCOUNT TOO
// ===========================================================================

/// FINDING 35's second half: `refuse_currency_change_while_funded` reads
/// `total_liability()`, which had no term for the main account and no
/// "unknown is not zero" leg covering it, so an audited, drained table sitting on
/// 5 ICP at its own published address read a liability of exactly zero and the
/// flip to a ledger it holds nothing on was ACCEPTED.
#[test]
fn the_currency_guard_refuses_while_the_main_account_has_never_been_read() {
    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    // (a) A table that has never read its own main account must refuse, even
    //     though every book says zero.
    let never_read = flip(&world, &btc);
    println!("flip with the main account NEVER READ -> {never_read}");
    assert!(
        never_read.starts_with("REFUSED"),
        "UNKNOWN IS NOT ZERO at the main account either: {never_read}"
    );
    assert!(
        never_read.contains("NEVER asked"),
        "and the refusal has to say why, and name the fix: {never_read}"
    );

    // (b) Read it, find it genuinely empty, and the flip is available. The guard
    //     must not become a one-way door.
    world
        .refresh_solvency(Principal::anonymous())
        .expect("anyone may take the reading");
    let empty = flip(&world, &btc);
    println!("flip after reading an empty main account -> {empty}");
    assert!(
        empty.starts_with("ACCEPTED"),
        "a guard that can never be satisfied is a lock, not a guard: {empty}"
    );

    // (c) AND THE READING DOES NOT SURVIVE THE FLIP AS A FIGURE.
    //
    // The table is denominated in ckBTC now and the only reading on record was
    // taken on the ICP ledger. A balance on one ledger is not evidence about an
    // account on another, and counting it would let the canister answer "I can
    // pay everyone" in satoshis it has never looked for. `DepositObservation`
    // records which ledger a reading came from precisely because a table that has
    // changed currency holds two accounts; here that record is ENFORCED.
    let on_btc = solvency_of(&world);
    println!("solvency on the flipped table -> {:?} -- {}", on_btc.verdict, on_btc.summary);
    assert_eq!(
        on_btc.verdict,
        SolvencyVerdict::Unknown,
        "a reading taken on the ICP ledger must not be counted as a reading of the ckBTC \
         account this table now owes out of"
    );
    assert_eq!(on_btc.main_account, None);
    assert_eq!(on_btc.held, None);
    assert!(
        on_btc.main_ledger.is_some(),
        "and the report must still SAY which ledger the reading it is declining to use came \
         from, or a reader cannot tell 'never read' from 'read on the wrong chain'"
    );
    assert!(
        on_btc.summary.contains("not evidence about an account on another"),
        "and say so in words: {}",
        on_btc.summary
    );

    // (d) The flip BACK needs a reading of the ckBTC main account: the same rule,
    //     applied to the ledger now in force. PocketIC has no ckBTC ledger
    //     installed, so it cannot be taken here -- on mainnet both ledgers exist
    //     and refresh_solvency() is one public call. Asserted rather than left as
    //     a surprise for whoever changes a currency next.
    let back = flip(&world, &TableConfig::six_max_icp());
    println!("flip BACK to ICP without a ckBTC reading -> {back}");
    assert!(
        back.starts_with("REFUSED") && back.contains("NEVER asked"),
        "the rule has to be the same in both directions, and say so: {back}"
    );

    // (e) An ordinary config change -- one that does NOT touch the currency --
    //     must be completely unaffected. A solvency guard that blocks raising the
    //     blinds is a guard that gets switched off.
    let ordinary = flip(
        &world,
        &TableConfig {
            currency: Currency::BTC,
            action_timeout_secs: 45,
            ..TableConfig::six_max_icp()
        },
    );
    println!("ordinary (same-currency) config change -> {ordinary}");
    assert!(
        ordinary.starts_with("ACCEPTED"),
        "this guard is about the CURRENCY and must not touch anything else: {ordinary}"
    );
}

/// The exact state FINDING 35 measured: an audited, DRAINED table with ICP
/// sitting in the main account, reading a liability of zero.
///
/// Money at the main account that nobody is credited with is a liability -- it
/// belongs to whoever sent it, and `notify_deposit` exists to attach a name to it
/// later -- and it is the fifth term FINDING 35 prescribed for
/// `total_liability()`.
#[test]
fn uncredited_money_at_the_main_account_is_a_liability_the_guard_can_see() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    world
        .raw_transfer_to_canister(alice, 5 * ICP)
        .expect("an exchange withdrawal, with no message to the canister");
    world.refresh_solvency(alice).expect("reading");

    let funded = flip(&world, &btc);
    println!("flip with 5 ICP uncredited at the main account -> {funded}");
    assert!(
        funded.starts_with("REFUSED"),
        "the guard read a liability of ZERO on a canister holding 5 ICP in its own main \
         account, and accepted the flip. {funded}"
    );

    let r = solvency_of(&world);
    println!("{}", r.summary);
    assert_eq!(
        r.unattributed_at_main,
        Some(5 * ICP),
        "and the report must name it as money held for somebody it cannot yet name: {}",
        r.summary
    );
    assert_eq!(
        r.guard_liability,
        5 * ICP,
        "which is exactly the fifth term FINDING 35 prescribed for total_liability()"
    );

    // AND THE PUBLIC INSTRUMENT MUST BE ON THE SAME NUMBER.
    //
    // This assertion used to read `difference_e8s == Some(5 * ICP)` under the
    // comment "it is a surplus against the escrow books, not a shortfall, and the
    // report must not confuse the two". It was asserting
    // docs/SECURITY-FINDINGS.md FINDING 43: the guard counted the 5 ICP and the
    // public `owed` did not, so the reply called a player's money a SURPLUS in the
    // same breath as naming it unattributed. There is one definition now.
    assert_eq!(
        r.owed, r.guard_liability,
        "the public `owed` and the guard's total_liability() are one number: {}",
        r.summary
    );
    assert_eq!(r.verdict, SolvencyVerdict::CanPayEveryone);
    assert_eq!(
        r.difference_e8s,
        Some(0),
        "the canister holds exactly what it owes -- 5 ICP against 5 ICP -- so there is no \
         surplus and no shortfall. 5 ICP of PROFIT on a table that takes no rake is FINDING \
         43. summary = {}",
        r.summary
    );
    assert!(
        !r.summary.contains("a surplus of"),
        "and the sentence a player reads must not offer it as profit: {}",
        r.summary
    );
}

// ===========================================================================
// 9. THE GATE ON THE GATE
// ===========================================================================

/// **Every leg of `invariants::solvency` must actually be able to go red.**
///
/// The standing lesson of this repository is that a green instrument is not
/// evidence: five times a defect has been invisible to a gate written by somebody
/// who knew the answer, and once (docs/DEFECTS.md, wave 9) a bundle verifier
/// passed 12 of 12 while measuring nothing. So this drives the checks against
/// reports that LIE, in each of the ways a report can lie, and fails if any of
/// them is waved through.
///
/// It mutates a real snapshot rather than building a synthetic one, so the parts
/// it does not falsify are the parts a live canister actually produces -- and it
/// runs the falsified snapshot through [`invariants::check_point_in_time`], the
/// composite the fuzzer calls, so "the check exists" and "the check is wired in"
/// are both established here.
#[test]
fn every_solvency_leg_can_go_red() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 4 * ICP).expect("deposit");
    read_every_account(&mut world, &[alice]);
    let clean = world.snapshot();
    assert!(
        invariants::check_point_in_time(&clean, Exemptions::of(&world)).is_empty(),
        "the honest snapshot must be silent, or nothing below means anything: {:#?}",
        invariants::check_point_in_time(&clean, Exemptions::of(&world))
    );

    // A MUTATION THAT MUTATES NOTHING IS THE FAILURE MODE THIS FILE EXISTS TO
    // AVOID. Wave 9's bundle verifier passed 12 of 12 while measuring nothing,
    // because one wrong regex made its scan return an empty set. So every lie
    // below must actually change the report, and the closure asserts it.
    let lie = |label: &str, mutate: &dyn Fn(&mut SolvencyReport)| -> Vec<String> {
        let mut snap = clean.clone();
        let original = snap.canister_solvency.clone().expect("report");
        let mut r = original.clone();
        mutate(&mut r);
        assert_ne!(
            r, original,
            "the mutation for '{label}' changed nothing, so whatever it proves about the gate \
             is worthless"
        );
        snap.canister_solvency = Some(r);
        invariants::check_point_in_time(&snap, Exemptions::of(&world))
            .into_iter()
            .map(|v| v.check.to_string())
            .collect()
    };

    // Each entry: what the report is made to claim, and the check that must fire.
    let cases: Vec<(&str, Box<dyn Fn(&mut SolvencyReport)>, &str)> = vec![
        (
            "no solvency surface at all",
            Box::new(|_: &mut SolvencyReport| {}), // handled separately below
            "",
        ),
        (
            "escrow understated",
            Box::new(|r: &mut SolvencyReport| r.escrow = r.escrow.saturating_sub(1)),
            "solvency_escrow_disagrees_with_admin_balances",
        ),
        (
            "chips overstated",
            Box::new(|r: &mut SolvencyReport| r.chips_at_table += 7),
            "solvency_chips_disagree_with_table",
        ),
        (
            "pot invented",
            Box::new(|r: &mut SolvencyReport| r.pot += 1),
            "solvency_pot_disagrees_with_table",
        ),
        (
            "deposit custody invented",
            Box::new(|r: &mut SolvencyReport| r.unswept_deposits += 1),
            "solvency_deposits_disagree_with_deposit_census",
        ),
        (
            "owed is not the sum of its terms",
            Box::new(|r: &mut SolvencyReport| r.owed = r.owed.saturating_sub(1)),
            "solvency_owed_is_not_the_sum_of_its_terms",
        ),
        (
            "the guard's number drifts from the report",
            Box::new(|r: &mut SolvencyReport| r.guard_liability += 1),
            "guard_liability_disagrees_with_the_published_terms",
        ),
        (
            "held is not built from its parts",
            Box::new(|r: &mut SolvencyReport| r.held = r.held.map(|h| h + 1)),
            "solvency_held_is_not_built_from_its_published_parts",
        ),
        (
            "the difference does not follow from held and owed",
            Box::new(|r: &mut SolvencyReport| {
                r.difference_e8s = Some(r.difference_e8s.unwrap_or(0) + 1)
            }),
            "solvency_difference_is_not_held_minus_owed",
        ),
        (
            "pulls larger than the whole journal",
            Box::new(|r: &mut SolvencyReport| r.pulls_in_flight = r.unfinished_incoming + 1),
            "solvency_pulls_exceed_unfinished_incoming",
        ),
        // docs/SECURITY-FINDINGS.md FINDING 43. A canister that takes no rake
        // cannot have a surplus, so a positive difference is always somebody
        // else's money booked as this canister's own.
        (
            "a SURPLUS, which a no-rake custodian cannot have",
            Box::new(|r: &mut SolvencyReport| {
                r.difference_e8s = Some(r.difference_e8s.unwrap_or(0).abs() + 1)
            }),
            "solvency_reports_a_surplus",
        ),
        (
            "the SENTENCE offering a surplus, whatever the number says",
            Box::new(|r: &mut SolvencyReport| {
                r.summary.push_str(" ... a surplus of 100000000 e8s.")
            }),
            "solvency_summary_claims_a_surplus",
        ),
        // FINDING 38. `held` is built from ledger readings and nothing else; an
        // in-flight term in it is what let one anonymous refresh_solvency()
        // publish the same e8s twice.
        (
            "an in-flight pull added to the HELD side",
            Box::new(|r: &mut SolvencyReport| {
                r.pulls_in_flight += 5;
                r.unfinished_incoming += 5;
                r.held = r.held.map(|h| h + 5);
            }),
            "solvency_held_is_not_built_from_its_published_parts",
        ),
        (
            "an all-clear built on an account nobody read",
            Box::new(|r: &mut SolvencyReport| {
                r.main_observed_at_ns = None;
                r.verdict = SolvencyVerdict::CanPayEveryone;
            }),
            "never_observed_main_account_is_not_reported_as_unknown",
        ),
        (
            "an all-clear with unread deposit addresses",
            Box::new(|r: &mut SolvencyReport| {
                r.deposit_accounts_never_observed_count = 1;
                r.deposit_accounts_never_observed = vec![Principal::anonymous()];
                r.verdict = SolvencyVerdict::CanPayEveryone;
            }),
            "never_observed_deposit_account_reported_as_solvent",
        ),
        (
            "THE ONE THAT MATTERS: claiming more in the main account than the ledger holds",
            Box::new(|r: &mut SolvencyReport| {
                r.main_account = r.main_account.map(|m| m + 1);
                r.held = r.held.map(|h| h + 1);
                r.difference_e8s = r.difference_e8s.map(|d| d + 1);
            }),
            "canister_claims_more_in_its_main_account_than_the_ledger_holds",
        ),
        (
            "claiming more in the deposit subaccounts than the ledger holds",
            Box::new(|r: &mut SolvencyReport| {
                r.deposit_subaccounts += 1;
                r.held = r.held.map(|h| h + 1);
                r.difference_e8s = r.difference_e8s.map(|d| d + 1);
            }),
            "canister_claims_more_in_its_deposit_subaccounts_than_the_ledger_holds",
        ),
    ];

    for (label, mutate, expected) in cases {
        if expected.is_empty() {
            continue;
        }
        let fired = lie(label, &*mutate);
        println!("{label:64} -> {fired:?}");
        assert!(
            fired.iter().any(|c| c == expected),
            "a report that lies about '{label}' was waved through. Expected `{expected}`, got \
             {fired:?}. A gate that cannot go red is not a gate."
        );
    }

    // And the absence of the surface entirely.
    let mut gone = clean.clone();
    gone.canister_solvency = None;
    let fired: Vec<String> = invariants::check_point_in_time(&gone, Exemptions::of(&world))
        .into_iter()
        .map(|v| v.check.to_string())
        .collect();
    println!("{:64} -> {fired:?}", "no get_solvency() on the build at all");
    assert!(
        fired.iter().any(|c| c == "no_solvency_surface_at_all"),
        "a build with no solvency surface must be a finding, not a skipped check: {fired:?}"
    );

    // Every one of those is never-excusable: the register must not be able to
    // reach any of them.
    let mut snap = clean.clone();
    let mut r = snap.canister_solvency.clone().unwrap();
    r.main_account = r.main_account.map(|m| m + 1);
    snap.canister_solvency = Some(r);
    for v in invariants::check_point_in_time(&snap, Exemptions::of(&world)) {
        assert!(
            matches!(
                money_safety::documented::classify(&v),
                money_safety::documented::Disposition::Blocking(_)
            ),
            "the register was able to excuse {v:?}"
        );
    }
}

// ===========================================================================
// 10. THE PUBLISHED INTERFACE HAS TO DESCRIBE THE CODE
// ===========================================================================

/// The committed `table_canister.did` is what `icp.yaml` installs and what every
/// client is generated from, and this project already has a finding for it being
/// out of step with the code it claims to describe
/// (docs/SECURITY-FINDINGS.md FINDING 03 / docs/DEFECTS.md E-08, 241 diff lines).
///
/// This does not try to close that. It asserts the narrower thing that is this
/// wave's responsibility, and it asserts it the strongest way available: **the
/// committed file is parsed and used as the DECODER for a real reply.** A client
/// generated from that file can read what the canister actually sends, or this
/// test fails.
///
/// `ic_cdk 0.19`'s `export_candid!()` emits the interface at build time through
/// `candid-extractor` rather than as a queryable method, so the canister cannot
/// simply be asked for its own interface and compared. Decoding a live reply is
/// better anyway: a textual diff proves two files agree, and this proves the
/// bytes on the wire agree with the file the clients are built from.
#[test]
fn the_committed_candid_decodes_a_real_solvency_reply() {
    use candid_parser::{check_prog, IDLProg, TypeEnv};

    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.fund_escrow(alice, 3 * ICP).expect("deposit");
    drain_main_account_behind_the_canisters_back(
        &world,
        Principal::self_authenticating(b"elsewhere"),
        ICP,
    )
    .expect("drain");
    // Take a reading and reach an INSOLVENT state first, so the reply exercises
    // the arms that only appear when something is wrong: a populated
    // `shortfall_e8s`, a negative `difference_e8s` (candid `int`), and the
    // `CannotPayEveryone` verdict.
    world.refresh_solvency(alice).expect("reading");

    let path = money_safety::wasms::repo_root().join("src/table_canister/table_canister.did");
    let text = std::fs::read_to_string(&path).expect("the committed .did must be readable");
    let prog: IDLProg = text
        .parse()
        .unwrap_or_else(|e| panic!("the committed .did does not PARSE: {e}"));
    let mut env = TypeEnv::new();
    let service = check_prog(&mut env, &prog)
        .unwrap_or_else(|e| panic!("the committed .did does not TYPE-CHECK: {e}"))
        .expect("the committed .did declares no service");

    // Decode one real reply per surface, against the committed types.
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "get_solvency",
            world
                .pic
                .query_call(
                    world.table,
                    Principal::anonymous(),
                    "get_solvency",
                    Encode!().unwrap(),
                )
                .expect("get_solvency must answer anyone"),
        ),
        (
            "get_custody_status",
            world
                .pic
                .query_call(world.table, alice, "get_custody_status", Encode!().unwrap())
                .expect("get_custody_status"),
        ),
        (
            "refresh_solvency",
            world
                .pic
                .update_call(world.table, alice, "refresh_solvency", Encode!().unwrap())
                .expect("refresh_solvency"),
        ),
        (
            "refresh_main_account_custody",
            world
                .pic
                .update_call(
                    world.table,
                    alice,
                    "refresh_main_account_custody",
                    Encode!().unwrap(),
                )
                .expect("refresh_main_account_custody"),
        ),
    ];

    for (method, bytes) in cases {
        let func = env
            .get_method(&service, method)
            .unwrap_or_else(|e| {
                panic!(
                    "`{method}` is exported by the canister and is NOT in the committed .did,                      which is the interface icp.yaml installs and every client is generated                      from: {e}"
                )
            })
            .clone();
        let decoded = candid::IDLArgs::from_bytes_with_types(&bytes, &env, &func.rets)
            .unwrap_or_else(|e| {
                panic!(
                    "the committed .did cannot decode what `{method}` actually returns: {e}.                      A client generated from that file would fail on this reply.                      docs/SECURITY-FINDINGS.md FINDING 03."
                )
            });
        let rendered = decoded.to_string();
        println!("{method} decoded against the committed .did: {} bytes", bytes.len());
        // The decode is the assertion; these are the two values a reader of the
        // report would act on, and they must survive the round trip as the
        // committed types describe them.
        if method == "get_solvency" {
            assert!(
                rendered.contains("CannotPayEveryone"),
                "the committed types must carry the verdict arm that matters: {rendered}"
            );
            assert!(
                rendered.contains("shortfall_e8s"),
                "and the magnitude field: {rendered}"
            );
        }
        if method == "get_custody_status" {
            assert!(
                rendered.contains("canister_solvency"),
                "the surface a player already reads must carry the verdict: {rendered}"
            );
        }
    }
}

// ===========================================================================
// 11. THE HOLE IN THE FIRST VERSION OF THIS INSTRUMENT
// ===========================================================================

/// **A payout the ledger executed and whose continuation was discarded.**
///
/// The first version of `check_written_down_holdings_are_not_overstated`
/// asserted `main_account <= ledger_main` unconditionally, on the strength of an
/// argument written in a comment. The argument was wrong in exactly one reachable
/// state and this is it: the record is adjusted at `settle_intent`, which is the
/// moment the ledger's answer comes back, so a payout that LANDED and whose
/// callback was dropped (docs/SECURITY-FINDINGS.md FINDING 29) has left the chain
/// and not yet left the record.
///
/// The thing that makes it harmless is not the size of the gap, it is that the
/// same amount is on the OWED side as `payouts_in_flight` for exactly as long, so
/// the published DIFFERENCE is identical before and after the movement settles.
/// That is what is asserted here, because "it cancels" is precisely the sort of
/// claim this project has repeatedly been wrong about in a comment.
#[test]
fn a_payout_in_flight_moves_the_parts_and_not_the_answer() {
    use money_safety::fault;

    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(alice, 5 * ICP).expect("deposit");
    world.fund_escrow(bob, 3 * ICP).expect("deposit");
    read_every_account(&mut world, &[alice, bob]);

    let before = solvency_of(&world);
    println!(
        "BEFORE: verdict={:?} main={:?} held={:?} diff={:?} payouts_in_flight={}",
        before.verdict, before.main_account, before.held, before.difference_e8s,
        before.payouts_in_flight
    );
    assert_eq!(before.payouts_in_flight, 0);

    // The FINDING 29 injector: the ledger transfer stands, the post-await
    // continuation is discarded, the payout intent stays open.
    let inj = fault::trap_withdraw_tail(&mut world, alice, 2 * ICP)
        .expect("inject a discarded withdrawal continuation");
    println!("after the discarded continuation: ledger_main={}", inj.after.ledger_main);

    let during = solvency_of(&world);
    let snap = world.snapshot();
    println!(
        "DURING: verdict={:?} main={:?} (LEDGER says {}) held={:?} diff={:?} payouts_in_flight={}",
        during.verdict, during.main_account, snap.ledger_main, during.held,
        during.difference_e8s, during.payouts_in_flight
    );

    assert!(
        during.payouts_in_flight > 0,
        "the fixture must actually leave a payout open, or it is testing nothing"
    );
    assert!(
        during.main_account.unwrap() > snap.ledger_main,
        "and the record must actually be stale-high here, or the bound below is untested"
    );

    // (a) the bound holds and the check is SILENT -- no false alarm.
    let vs = invariants::check_written_down_holdings_are_not_overstated(&snap);
    assert!(
        vs.is_empty(),
        "a payout in flight is not a canister claiming money it does not have; the allowance \
         is exactly what the journal names. {vs:#?}"
    );
    assert!(
        invariants::check_point_in_time(&snap, Exemptions::of(&world)).is_empty(),
        "and nothing else may go red on it either: {:#?}",
        invariants::check_point_in_time(&snap, Exemptions::of(&world))
    );

    // (b) THE ANSWER IS UNCHANGED. This is the property the allowance rests on.
    assert_eq!(
        during.difference_e8s, before.difference_e8s,
        "the published difference moved while a payout was in flight. The whole justification \
         for letting main_account run ahead of the chain is that `payouts_in_flight` cancels \
         it on the owed side, to the e8."
    );
    assert_eq!(during.verdict, before.verdict);

    // (c) and once it settles, the parts agree again with nothing in flight.
    //
    // The lease the injected call took has to expire first: an entry somebody
    // else is driving is deliberately not resolvable, which is what stops two
    // callers issuing the same movement twice.
    world.advance(std::time::Duration::from_secs(45));
    let resolved = fault::resolve_my_ledger_intents(&world, alice);
    println!("resolve_my_ledger_intents(alice) -> {resolved:?}");
    let after = solvency_of(&world);
    let snap = world.snapshot();
    println!(
        "AFTER:  verdict={:?} main={:?} (LEDGER says {}) held={:?} diff={:?} payouts_in_flight={}",
        after.verdict, after.main_account, snap.ledger_main, after.held,
        after.difference_e8s, after.payouts_in_flight
    );
    assert_eq!(after.payouts_in_flight, 0, "the intent must be closed");
    assert_eq!(
        after.main_account,
        Some(snap.ledger_main),
        "and the record must be back in step with the chain, to the e8"
    );
    assert_eq!(
        after.difference_e8s, before.difference_e8s,
        "and the answer must STILL be the same: a withdrawal that completes takes the money \
         out of both sides at once"
    );
    assert!(invariants::check_point_in_time(&snap, Exemptions::of(&world)).is_empty());
}
