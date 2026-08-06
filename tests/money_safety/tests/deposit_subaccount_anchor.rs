//! THE ACCOUNT CENSUS -- every money instrument anchored to every account the
//! canister owns.
//!
//! docs/SECURITY-FINDINGS.md FINDING 21, FINDING 28, FINDING 11; docs/DEFECTS.md
//! E-12.
//!
//! # What was wrong
//!
//! The canister owns two kinds of ledger account -- its main account, and one
//! deposit subaccount per principal, `sha256("cleardeck-deposit:" || principal)`,
//! which is the address `get_deposit_subaccount()` publishes to external wallets.
//! **Every money instrument in the project was anchored to the first one.**
//!
//! | instrument | anchor | saw the subaccounts? |
//! |---|---|---|
//! | M1 `ledger_equals_owed` | `icrc1_balance_of(table, None)` | no |
//! | M9 `check_no_orphaned_custody` | `ledger_main - claims - uncredited_raw` | no |
//! | `total_liability()` (the FINDING 20 currency guard) | `escrow + chips + pot` | no |
//! | `DrainReport::table_is_really_empty` | `ledger_main` | no |
//! | all four balance surfaces | `BALANCES` | no |
//!
//! Two independent reviewers reached the hole from opposite directions in the same
//! wave: the currency-guard critic found the guard reading a liability of zero on
//! a table holding 5 ICP, and the third auditor found a player told they had
//! nothing while the canister held their money at the address it had given them,
//! with the refusal instructing them to send MORE money to it.
//!
//! # What each test here is for
//!
//! * `reproduce_*` -- the defect, driven end to end. These are the tests that must
//!   have been RED on the previous module, and the file records what they printed.
//! * everything else -- the gates. One per instrument, each phrased as the
//!   property rather than as the example, so re-anchoring cannot be undone
//!   quietly.
//!
//! Run: `cargo test --test deposit_subaccount_anchor -- --test-threads=2`

use candid::Principal;
use money_safety::assert_no_new_violations;
use money_safety::invariants::reachability;
use money_safety::invariants::{self, Exemptions, Severity};
use money_safety::ledger;
use money_safety::table_api::*;
use money_safety::world::*;

const ICP: u64 = 100_000_000;
/// `Currency::ICP.transfer_fee()` in the canister.
const FEE: u64 = 10_000;

fn ledger_at_deposit_address(world: &World, who: Principal) -> u64 {
    world.ledger_balance(world.table, Some(ledger::deposit_subaccount(&who)))
}

// ===========================================================================
// 1. REPRODUCTION -- the state, and every surface reporting zero in it
// ===========================================================================

/// FINDING 28, driven exactly as the auditor drove it.
///
/// The point of this test is not that the numbers are now right. It is that the
/// canister's own surfaces are the ONLY witness a player has, and in this state
/// they were unanimous and wrong. It asserts the whole set at once, because any
/// one of them being fixed alone still leaves a player who reads a different one
/// being told they have nothing.
#[test]
fn money_at_a_published_deposit_address_is_visible_on_every_surface() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let bob = world.actor("bob");
    let sent = 5 * ICP;

    // The documented external-wallet flow: ask the canister where to send, send
    // there. NO message reaches the canister -- this is a ledger transfer.
    let subaccount = world.deposit_custody(bob).subaccount;
    assert_eq!(
        subaccount,
        ledger::deposit_subaccount(&bob).to_vec(),
        "the address the canister publishes must be the one this harness funds, or the whole \
         test is about a different account"
    );
    world
        .transfer_to_deposit_subaccount(bob, sent)
        .expect("send to the published deposit address");

    assert_eq!(
        ledger_at_deposit_address(&world, bob),
        sent,
        "the LEDGER must agree the canister is holding it"
    );

    // The canister has not been told, so it does not know -- and it says so
    // rather than reporting a confident zero. `observed_at_ns == None` is the
    // machine-readable form of "nobody has looked".
    let before = world.custody_status(bob);
    assert_eq!(
        before.unswept_deposit_observed_at_ns, None,
        "an unread account must report `never observed`, not a balance of zero"
    );
    assert!(
        world.deposit_custody(bob).note.contains("has not asked"),
        "and the dedicated surface must say so in words: {:?}",
        world.deposit_custody(bob).note
    );

    // One player-callable, privilege-free update, and every surface is true.
    let refreshed = world
        .refresh_deposit_custody(bob)
        .expect("refresh_deposit_custody must answer any principal for their own address");
    assert_eq!(refreshed.observed_amount, sent);
    assert!(refreshed.sweepable, "5 ICP is far above the {FEE} e8 fee");

    let status = world.custody_status(bob);
    assert_eq!(
        status.unswept_deposit, sent,
        "get_custody_status must report money at the address the canister published"
    );
    assert_eq!(
        status.total, sent,
        "and `total` -- the field whose entire job is \"what is this canister holding for me\" -- \
         read 0 here while the canister held {sent} e8s of bob's (FINDING 28)"
    );
    assert!(
        !status.advice.is_empty() && status.advice.contains("claim_external_deposit"),
        "the advice must name the call that recovers it: {:?}",
        status.advice
    );

    let (total, list, unaudited) = world.admin_deposit_custody();
    assert_eq!(total, sent, "the controller surface must see it too");
    assert!(
        list.iter().any(|(p, amount, _)| *p == bob && *amount == sent),
        "and must attribute it to bob, not merely total correctly: {list:?}"
    );
    assert!(
        !unaudited.contains(&bob),
        "bob's address has now been read, so he is not in the unaudited list"
    );

    // And the claim path moves it.
    let credited = world
        .claim_external_deposit(bob)
        .expect("claim must sweep 5 ICP");
    assert_eq!(credited, sent - FEE, "swept, minus the ledger's sweep fee");
    assert_eq!(
        world.custody_status(bob).unswept_deposit,
        0,
        "and the observation must be cleared, or the same e8s are counted twice"
    );
    assert_no_new_violations(&invariants::check_world(&world), "after the sweep");
}

/// The sentence itself. FINDING 28's headline was not a number, it was a string:
/// *"No claimable balance. Send ICP to your deposit address first"*, returned to a
/// player whose 10,000 e8s were at that exact address, at or below the transfer
/// fee, where nothing can move them.
#[test]
fn the_claim_refusal_may_not_say_there_is_nothing_when_there_is() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let bob = world.actor("bob");
    let dust = FEE; // exactly at the fee: unsweepable, and real money.

    world
        .transfer_to_deposit_subaccount(bob, dust)
        .expect("send dust to the published deposit address");
    assert_eq!(ledger_at_deposit_address(&world, bob), dust);

    let err = match world.claim_external_deposit(bob) {
        Err(OpError::Err(m)) => m,
        other => panic!("a claim of unsweepable dust must be refused with a reason, got {other:?}"),
    };
    eprintln!("claim_external_deposit refusal:\n  {err}");

    assert!(
        !err.contains("No claimable balance"),
        "the refusal still tells a player holding {dust} e8s at this canister's own address that \
         they have no balance: {err}"
    );
    assert!(
        err.contains(&dust.to_string()),
        "a refusal about money must state the amount it is refusing to move: {err}"
    );
    assert!(
        err.contains("transfer fee") || err.contains("fee"),
        "and why it cannot move it: {err}"
    );
    assert!(
        err.contains("NOT LOST") || err.contains("not lost"),
        "and that the money is still there: {err}"
    );

    // Every surface, on the same state.
    let status = world.custody_status(bob);
    assert_eq!(status.unswept_deposit, dust);
    assert_eq!(status.total, dust, "dust is custody, not rounding");
    assert!(
        status.advice.contains("dust") || status.advice.contains("SAME address"),
        "the advice must name the only recovery there is -- topping the address up past the \
         fee -- or say plainly that it is unrecoverable: {:?}",
        status.advice
    );
    assert_eq!(world.admin_deposit_custody().0, dust);
}

/// SWEEPABILITY (task item 4). Dust at or below the fee cannot be moved by any
/// transfer -- that is arithmetic, not a defect that can be coded away. What is
/// not allowed is for it to be silent. So: it is VISIBLE on every surface, and it
/// is RECOVERABLE by topping the same address up past the fee, and this test
/// drives the recovery to the ledger.
#[test]
fn dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let bob = world.actor("bob");

    world
        .transfer_to_deposit_subaccount(bob, FEE - 1)
        .expect("send sub-fee dust");
    world.claim_external_deposit(bob).expect_err("unsweepable");

    let status = world.custody_status(bob);
    assert_eq!(
        status.unswept_deposit,
        FEE - 1,
        "unrecoverable-for-now is still custody and must be reported, not rounded away"
    );

    // The remedy the refusal names, driven for real.
    let top_up: u64 = 2 * ICP;
    world
        .transfer_to_deposit_subaccount(bob, top_up)
        .expect("top the same address up");
    let credited = world
        .claim_external_deposit(bob)
        .expect("now above the fee, the whole balance sweeps");
    assert_eq!(
        credited,
        (FEE - 1) + top_up - FEE,
        "the dust came out WITH the top-up: it was never lost, only immovable on its own"
    );
    assert_eq!(world.custody_status(bob).unswept_deposit, 0);
    assert_no_new_violations(&invariants::check_world(&world), "after the dust recovery");
}

// ===========================================================================
// 2. THE INSTRUMENTS -- one gate per anchor
// ===========================================================================

/// FINDING 21, first half. `total_liability()` fed the currency guard, and it was
/// `escrow + chips + pot`. A table whose ONLY money is in a deposit subaccount
/// read a liability of zero, and the guard that exists to refuse a re-denomination
/// while players are owed money permitted it -- after which
/// `claim_external_deposit` looked for the player's ICP on the ckBTC ledger.
#[test]
fn the_currency_guard_refuses_on_a_table_funded_only_through_a_deposit_subaccount() {
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };
    let mut world = World::new(TableConfig::six_max_icp(), &["victim"]);
    let victim = world.actor("victim");

    world
        .transfer_to_deposit_subaccount(victim, 5 * ICP)
        .expect("the documented external-wallet flow");
    world
        .refresh_deposit_custody(victim)
        .expect("the canister looks at its own address");

    let snap = world.snapshot();
    assert_eq!(snap.escrow_total, 0, "no escrow");
    assert_eq!(snap.chips_total, 0, "no chips");
    assert_eq!(snap.table.pot, 0, "no pot");
    assert_eq!(
        snap.ledger_deposit_subaccounts,
        5 * ICP,
        "and 5 ICP of the victim's in the canister's own subaccount"
    );

    for (name, outcome) in [
        ("reset_table", set_currency(&world, "reset_table", &btc)),
        (
            "admin_reinit_table",
            set_currency(&world, "admin_reinit_table", &btc),
        ),
        (
            "admin_update_config",
            set_currency(&world, "admin_update_config", &btc),
        ),
    ] {
        let msg = match outcome {
            Err(OpError::Err(m)) => m,
            other => panic!(
                "FINDING 21: `{name}` accepted a currency flip while 5 ICP of a player's money \
                 sat in this canister's own deposit subaccount. After the flip, \
                 claim_external_deposit looks for that ICP on the ckBTC ledger and the money is \
                 reachable only by flipping back. Got {other:?}"
            ),
        };
        assert!(
            msg.contains("currency"),
            "the refusal must say why: {msg}"
        );
    }
    assert_eq!(
        world.table_state().config.currency,
        Currency::ICP,
        "and the table must still be an ICP table"
    );
}

/// FINDING 21, the sharper half: **unknown is not zero.**
///
/// The guard above only refuses because somebody looked. A controller who flips
/// the currency BEFORE anybody looks would strand the same 5 ICP with the same
/// mechanism, and `total_liability()` would still read zero -- correctly, because
/// the balance is genuinely unknown to the canister. So the guard also refuses
/// while any deposit address it has published has never been read.
#[test]
fn the_currency_guard_refuses_while_a_published_address_has_never_been_read() {
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };
    let mut world = World::new(TableConfig::six_max_icp(), &["victim"]);
    let victim = world.actor("victim");

    // Escrow, then take it all out again: the canister now knows this principal,
    // has published nothing to it, and owes it nothing.
    world.fund_escrow(victim, 3 * ICP).expect("deposit");
    world.withdraw(victim, 3 * ICP).expect("withdraw it all");
    assert_eq!(world.snapshot().escrow_total, 0, "owes nothing");

    // The money arrives at the deposit address AFTER the books read zero, which is
    // the ordering that makes the flip look safe.
    world
        .transfer_to_deposit_subaccount(victim, 5 * ICP)
        .expect("external wallet, no message to the canister");

    let msg = match set_currency(&world, "admin_update_config", &btc) {
        Err(OpError::Err(m)) => m,
        other => panic!(
            "the guard read a liability of zero and allowed the flip while a deposit address it \
             published had never been read. Got {other:?}"
        ),
    };
    assert!(
        msg.contains("never been read") || msg.contains("unaudited") || msg.contains("audit"),
        "the refusal must name what it does not know, and how to find out: {msg}"
    );

    // The named remedy, driven: audit, discover the 5 ICP, and the refusal changes
    // from "I do not know" to "you owe this".
    let (read, observed, still_unaudited) = world
        .admin_audit_deposit_custody(&[])
        .expect("the controller may audit its own deposit addresses");
    assert!(read >= 1, "the audit must have read at least victim's address");
    assert_eq!(observed, 5 * ICP, "and found the money");
    assert_eq!(still_unaudited, 0, "with nothing left unread");

    let msg = match set_currency(&world, "admin_update_config", &btc) {
        Err(OpError::Err(m)) => m,
        other => panic!("still funded, so still refused; got {other:?}"),
    };
    assert!(
        msg.contains("owes players"),
        "and now it refuses for the plain reason: {msg}"
    );
}

/// FINDING 21, second half. `check_no_orphaned_custody` computed
/// `ledger_main - claims - uncredited_raw`, so a canister holding 7 ICP in its own
/// subaccounts and owing nobody anything reported ZERO orphaned e8s.
///
/// Driven on the harness's own instrument with the exact numbers from the finding,
/// so the gate cannot rot into a no-op the way the ones before it did.
#[test]
fn the_orphan_invariant_is_red_on_money_held_only_in_deposit_subaccounts() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let mut snap = world.snapshot();

    // The measured state from the finding: nothing owed, nothing in the main
    // account, 7 ICP in the canister's own deposit subaccounts, and the canister
    // does not know about any of it.
    snap.ledger_main = 0;
    snap.ledger_deposit_subaccounts = 700_000_000;
    snap.escrow_total = 0;
    snap.escrow.clear();
    snap.chips_total = 0;
    snap.table.pot = 0;
    snap.canister_unswept_deposits = 0;

    let vs = reachability::check_no_orphaned_custody(&snap, Exemptions::default());
    assert_eq!(
        vs.len(),
        1,
        "the orphan check reported {} violations on 7 ICP the canister holds and cannot name an \
         owner for. FINDING 21 measured this exact state and got 0 hits.",
        vs.len()
    );
    assert_eq!(vs[0].check, "money_belongs_to_nobody");
    assert_eq!(vs[0].severity, Severity::OrphanedCustody);
    assert_eq!(vs[0].delta_e8s, 700_000_000);

    // And the same 7 ICP, once the canister knows whose it is, is NOT orphaned:
    // it is owed. An instrument that cannot tell those apart is not measuring
    // custody, it is measuring surprise.
    snap.canister_unswept_deposits = 700_000_000;
    assert!(
        reachability::check_no_orphaned_custody(&snap, Exemptions::default()).is_empty(),
        "money the canister accounts for at its own deposit address belongs to somebody"
    );

    // M1's own re-anchoring, on the same two states.
    snap.canister_unswept_deposits = 0;
    assert!(
        !invariants::check_conservation(&snap, Exemptions::default()).is_empty(),
        "M1 must also be red: the ledger holds 7 ICP the canister's books do not mention"
    );
    let _ = &mut world;
}

/// `DrainReport::table_is_really_empty` was `fully_drained && nothing_orphaned`,
/// and both legs read `ledger_main`. A table whose escrow, chips and pot are all
/// zero while a player's money sits at a published deposit address reported EMPTY.
#[test]
fn the_drain_reports_not_empty_while_a_deposit_address_still_holds_money() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");

    // Sub-fee dust: the one thing a drain genuinely cannot move, so this is the
    // hardest case for "the report must not say empty".
    world
        .transfer_to_deposit_subaccount(alice, FEE - 1)
        .expect("send sub-fee dust to the published address");

    let report = reachability::drain(&mut world);
    eprintln!("drain transcript:\n  {}", report.log.join("\n  "));

    assert_eq!(
        report.ledger_deposit_subaccounts_after,
        FEE - 1,
        "the drain must MEASURE the subaccounts, not assume them empty"
    );
    assert!(
        !report.table_is_really_empty(),
        "the drain called this table empty while {} e8s of alice's sat at the address the \
         canister published to her (FINDING 28). owed_after={} ledger_main_after={} \
         subaccounts_after={}",
        FEE - 1,
        report.owed_after,
        report.ledger_main_after,
        report.ledger_deposit_subaccounts_after
    );
    assert!(
        !report.fully_drained(),
        "and it is `fully_drained` that must be false: the canister KNOWS about this money now, \
         so it is owed, not orphaned"
    );

    // Above the fee, the drain reaches it with player-only calls and the table
    // really is empty. The drain must be able to say both things.
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world
        .transfer_to_deposit_subaccount(alice, 3 * ICP)
        .expect("send a real amount");
    let report = reachability::drain(&mut world);
    eprintln!("drain transcript (sweepable):\n  {}", report.log.join("\n  "));
    assert_eq!(
        report.ledger_deposit_subaccounts_after, 0,
        "claim_external_deposit is a player call and belongs in a drain"
    );
    assert!(
        report.table_is_really_empty(),
        "owed_after={} orphaned={}",
        report.owed_after,
        report.orphaned_e8s()
    );
    assert!(
        report.returned_to_wallets[&alice] > 0,
        "and the money reached alice's own wallet on the ledger"
    );
}

/// The canister's claim and the ledger's fact must agree once the canister has
/// looked, exactly, in both directions, per principal.
///
/// This is the leg that answers the standing lesson. A total-only check passes
/// while the canister attributes alice's money to bob -- CORRECT TOTALS, WRONG
/// RECIPIENTS -- which is the signature four waves of this project have produced.
#[test]
fn the_canisters_deposit_books_match_the_ledger_per_principal() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");

    world.transfer_to_deposit_subaccount(alice, 3 * ICP).unwrap();
    world.transfer_to_deposit_subaccount(bob, 7 * ICP).unwrap();
    // carol sends nothing at all.

    // NOBODY here has ever called the canister, so `deposit_account_census()` is
    // EMPTY -- this is "THE ENUMERABILITY LIMIT" in the flesh, and an audit with
    // no `also` list correctly reads nothing and finds nothing.
    let (read, found, _) = world.admin_audit_deposit_custody(&[]).expect("audit");
    assert_eq!(
        (read, found),
        (0, 0),
        "an address nobody has ever asked this canister about is not in any list it can build;          the honest behaviour is to read nothing, not to claim the accounts are empty"
    );

    // The operator names them, which is what `also` is for.
    world
        .admin_audit_deposit_custody(&[alice, bob, carol])
        .expect("audit");

    let (total, list, unaudited) = world.admin_deposit_custody();
    assert_eq!(total, 10 * ICP);
    assert!(unaudited.is_empty(), "the audit read every account: {unaudited:?}");

    for (who, expected) in [(alice, 3 * ICP), (bob, 7 * ICP), (carol, 0)] {
        let on_ledger = ledger_at_deposit_address(&world, who);
        assert_eq!(
            on_ledger, expected,
            "harness setup wrong for {who}"
        );
        let in_books = list
            .iter()
            .find(|(p, _, _)| *p == who)
            .map(|(_, amount, _)| *amount)
            .unwrap_or(0);
        assert_eq!(
            in_books, on_ledger,
            "the canister attributes {in_books} e8s to {who} at their deposit address and the \
             LEDGER says {on_ledger}. A per-principal mismatch that nets to zero across the \
             table is the exact shape this project has produced four times: correct totals, \
             wrong recipients, every invariant silent."
        );
        assert_eq!(
            world.custody_status(who).unswept_deposit,
            on_ledger,
            "and each player's own surface must agree with the ledger for THEIR address"
        );
    }
    assert_no_new_violations(&invariants::check_world(&world), "after the audit");
}

/// THE GATE ON THE GATE for the leg that is not about totals.
///
/// Every other check here compares sums, and a canister that books alice's
/// deposit against bob's address satisfies all of them: the totals are right, the
/// recipients are wrong, and nothing fires. That is the signature this project has
/// produced four times. This drives `check_deposit_attribution` on exactly that
/// state and requires it to be red in both directions at once, so the instrument
/// cannot rot into a no-op.
#[test]
fn the_attribution_leg_is_red_when_the_totals_are_right_and_the_recipients_are_not() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let mut snap = world.snapshot();

    // The ledger holds 3 ICP at alice's address and nothing at bob's. The canister
    // books 3 ICP against bob. TOTALS AGREE EXACTLY: 3 ICP held, 3 ICP claimed.
    snap.ledger_deposit_by_principal.insert(alice, 3 * ICP);
    snap.ledger_deposit_by_principal.insert(bob, 0);
    snap.ledger_deposit_subaccounts = 3 * ICP;
    snap.canister_deposit_by_principal.clear();
    snap.canister_deposit_by_principal.insert(bob, 3 * ICP);
    snap.canister_unswept_deposits = 3 * ICP;

    assert!(
        invariants::check_conservation(&snap, Exemptions::default()).is_empty(),
        "the totals agree, so the conservation legs are silent -- which is exactly why this leg \
         has to exist"
    );

    let vs = invariants::check_deposit_attribution(&snap, &Exemptions::default());
    assert_eq!(
        vs.len(),
        2,
        "both directions must fire: bob is credited money that is not at his address, and \
         alice's money is at an address nothing accounts for. Got {vs:?}"
    );
    assert!(
        vs.iter()
            .any(|v| v.check == "deposit_custody_attributed_to_the_wrong_principal"
                && v.severity == Severity::FundCreation),
        "{vs:?}"
    );
    assert!(
        vs.iter().any(|v| v.check
            == "deposit_address_holds_money_the_canister_does_not_account_for"
            && v.severity == Severity::OrphanedCustody),
        "{vs:?}"
    );

    // And the correct attribution of the same total is silent.
    snap.canister_deposit_by_principal.clear();
    snap.canister_deposit_by_principal.insert(alice, 3 * ICP);
    assert!(
        invariants::check_deposit_attribution(&snap, &Exemptions::default()).is_empty(),
        "the same total, attributed to the principal whose address actually holds it"
    );
}

/// Custody survives an upgrade. `DEPOSIT_CUSTODY` is a liability record: if it
/// were dropped by `post_upgrade`, every surface would go back to reporting zero
/// on money still sitting in the canister's subaccounts, i.e. FINDING 28 would
/// re-open once per deploy.
#[test]
fn deposit_custody_survives_a_real_upgrade() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    world.transfer_to_deposit_subaccount(alice, 4 * ICP).unwrap();
    world.refresh_deposit_custody(alice).expect("observe");
    assert_eq!(world.custody_status(alice).unswept_deposit, 4 * ICP);

    world.upgrade().expect("a real --mode upgrade");

    assert_eq!(
        world.custody_status(alice).unswept_deposit,
        4 * ICP,
        "the upgrade forgot money the canister is holding at its own published address"
    );
    assert_eq!(world.admin_deposit_custody().0, 4 * ICP);
    assert_no_new_violations(&invariants::check_world(&world), "after the upgrade");

    // And it is still reachable afterwards.
    let credited = world.claim_external_deposit(alice).expect("sweep");
    assert_eq!(credited, 4 * ICP - FEE);
}

/// A player may only ever see or sweep their OWN deposit address. The record is
/// keyed by principal and every reader is caller-scoped; this asserts it rather
/// than trusting the derivation.
#[test]
fn one_players_deposit_custody_is_not_visible_or_claimable_by_another() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.transfer_to_deposit_subaccount(alice, 6 * ICP).unwrap();
    world.refresh_deposit_custody(alice).expect("observe");

    assert_eq!(world.custody_status(bob).unswept_deposit, 0);
    assert_eq!(world.deposit_custody(bob).observed_amount, 0);
    assert!(
        world.claim_external_deposit(bob).is_err(),
        "bob must not be able to sweep alice's deposit address"
    );
    assert_eq!(
        ledger_at_deposit_address(&world, alice),
        6 * ICP,
        "and alice's money must still be exactly where it was"
    );
    assert_eq!(world.custody_status(alice).unswept_deposit, 6 * ICP);
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Drive one of the three doors that take a whole `TableConfig` as the
/// controller. Same plumbing as `admin_custody.rs`, kept local so the two files
/// can be edited independently.
fn set_currency(world: &World, method: &str, cfg: &TableConfig) -> Result<String, OpError> {
    use candid::{decode_one, Encode};
    match world
        .pic
        .update_call(world.table, world.controller, method, Encode!(cfg).unwrap())
    {
        Err(reject) => Err(OpError::Trap(format!("{reject:?}"))),
        // `reset_table` and `admin_reinit_table` reply `Result<(), String>`;
        // `admin_update_config` replies `Result<TableConfig, String>`. Only the
        // Err arm is compared here, so decode the reply twice rather than
        // teaching this helper the shape of each one.
        Ok(bytes) => match decode_one::<Result<(), String>>(&bytes) {
            Ok(Ok(())) => Ok("Ok".to_string()),
            Ok(Err(msg)) => Err(OpError::Err(msg)),
            Err(_) => match decode_one::<Result<TableConfig, String>>(&bytes) {
                Ok(Ok(c)) => Ok(format!("{c:?}")),
                Ok(Err(msg)) => Err(OpError::Err(msg)),
                Err(e) => Err(OpError::Trap(format!(
                    "reply decode failed for {method}: {e}"
                ))),
            },
        },
    }
}
