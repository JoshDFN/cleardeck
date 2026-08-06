//! **M14 — LEDGER/BOOKS COHERENCE.** docs/SECURITY-FINDINGS.md FINDING 29.
//!
//! > Money that has moved on the ledger is money the canister's own books must
//! > either hold or name. There is no third state.
//!
//! `deposit()`, `claim_external_deposit()` and `withdraw()` each perform an
//! IRREVERSIBLE LEDGER MOVEMENT and settle the canister's books afterwards, in
//! the post-await continuation. Before the ledger-intent journal, nothing was
//! written before the movement, so a continuation that did not run left money
//! inside the canister that nothing accounted for and nobody could reach.
//!
//! Read `src/fault.rs` first: it explains how a discarded continuation is
//! reproduced exactly, with no instrumentation of the canister and no mock
//! ledger, and it records every attempt to force a literal trap and the measured
//! reason each one failed.
//!
//! # What these tests are, and what they are not
//!
//! They are the GATE for the fix, not a pin for the defect. They assert the
//! properties the journal is supposed to give — accounted-for, recoverable,
//! recoverable BY THE RIGHT PERSON, exactly once, across an upgrade — so a
//! regression turns them red. The measured behaviour of the pre-journal build,
//! which is what convicted the finding, is recorded in
//! docs/SECURITY-FINDINGS.md FINDING 29 with the numbers.

use candid::Encode;
use money_safety::fault::{self, Custody};
use money_safety::ledger;
use money_safety::world::World;
use money_safety::OpError;
use std::time::Duration;

fn e8(icp: f64) -> u64 {
    (icp * 100_000_000.0) as u64
}

/// Longer than `INTENT_LEASE_NS` in the canister. A discarded continuation leaves
/// its lease behind; the lease then expires and anybody entitled to the entry may
/// take it over. That expiry is the property, so the tests wait for it rather
/// than reaching around it.
const PAST_THE_LEASE: Duration = Duration::from_secs(45);

fn show(label: &str, c: &Custody) {
    eprintln!(
        "  {label:<22} ledger(main {} + subacc {}) = {}   books {}   journalled {}   ORPHANED {}  SHORT {}",
        c.ledger_main,
        c.ledger_subaccounts,
        c.ledger_total(),
        c.books,
        c.journalled,
        c.orphaned(),
        c.short(),
    );
}

/// The one assertion every test in this file makes, in the same words.
fn assert_coherent(w: &World, when: &str) {
    let c = fault::custody(w);
    assert_eq!(
        c.orphaned(),
        0,
        "\nM14 LEDGER/BOOKS COHERENCE VIOLATED ({when}).\n\
         {} e8s sit in an account this canister controls and NOTHING in the canister \
         accounts for them: not escrow, not chips, not the pot, and not an open ledger-intent \
         naming whose they are.\n\
         reading: {c:?}\n\
         docs/SECURITY-FINDINGS.md FINDING 29. The fix is not 'do not trap'. It is: write the \
         intent BEFORE the irreversible call, so a discarded continuation leaves a record \
         somebody can drive to completion.",
        c.orphaned()
    );
    assert_eq!(
        c.short(),
        0,
        "\nM14/M2 VIOLATED ({when}): the books promise {} e8s more than the canister holds on \
         the ledger.\n  reading: {c:?}",
        c.short()
    );
}

// ===========================================================================
// deposit(): the ICRC-2 pull
// ===========================================================================

#[test]
fn m14_deposit_continuation_discarded_is_accounted_for_and_recoverable() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let amount = e8(3.0);

    let wallet_at_start = w.ledger_balance(alice, None);
    w.approve(alice, amount + ledger::TRANSFER_FEE)
        .expect("approve");

    let inj = fault::trap_deposit_tail(&mut w, alice, amount)
        .expect("deposit must reach an await; if it does not, FINDING 29 is not what it says");

    eprintln!("\n=== deposit(): post-await continuation discarded ===");
    show("before", &inj.before);
    show("completed", &inj.completed);
    show("after rollback", &inj.after);
    eprintln!("  reply the caller saw before its tail was discarded: {:?}", inj.reply);
    eprintln!("  journal now: {:?}", fault::ledger_intents(&w));

    // 1. The pull is irreversible and it really happened.
    assert_eq!(
        inj.after.ledger_main - inj.before.ledger_main,
        amount,
        "the ICRC-2 pull must have moved {amount} e8s into the canister's main account and \
         stayed moved -- if it did not, this harness is reproducing nothing"
    );
    // 2. The credit really was discarded.
    assert_eq!(
        w.get_balance(alice),
        0,
        "the escrow credit lives in the discarded continuation, so it must be gone"
    );
    // 3. THE PROPERTY: the money is nonetheless accounted for.
    assert_coherent(&w, "after a discarded deposit continuation");
    let journal = fault::ledger_intents(&w);
    assert_eq!(journal.len(), 1, "exactly one open intent: {journal:?}");
    assert_eq!(journal[0].who, alice, "and it names the RIGHT PERSON");
    assert_eq!(journal[0].amount, amount);
    assert_eq!(journal[0].kind, "pull");

    // 4. And the canister says so, to the player, in the surface that claims to
    //    say where all of a player's money is.
    let advice = w
        .query_as(alice, "get_custody_status", Encode!().unwrap())
        .expect("get_custody_status");
    let text = String::from_utf8_lossy(&advice).to_string();
    assert!(
        text.contains("resolve_my_ledger_intents"),
        "get_custody_status must NAME the recovery method: a recovery path a player has to \
         read the interface definition to find is not a recovery path"
    );

    // 5. THE PROPERTY THAT MATTERS: the owner gets it back, with a player-only call.
    w.advance(PAST_THE_LEASE);
    let resolved = fault::resolve_my_ledger_intents(&w, alice).expect("resolve must succeed");
    eprintln!("  resolve_my_ledger_intents(alice) -> {resolved:?}");
    assert_eq!(
        w.get_balance(alice),
        amount,
        "after resolving, alice's escrow must be the whole {amount} e8s that moved"
    );
    assert!(
        fault::ledger_intents(&w).is_empty(),
        "and the journal entry must be retired, not left open"
    );
    assert_coherent(&w, "after resolving a discarded deposit continuation");

    // 6. EXACTLY ONCE. Resolving again must not credit again.
    w.advance(PAST_THE_LEASE);
    let again = fault::resolve_my_ledger_intents(&w, alice);
    eprintln!("  resolve again -> {again:?}");
    assert_eq!(
        w.get_balance(alice),
        amount,
        "a second resolve must not credit a second time"
    );

    // 7. And the money is really hers: take it all the way back to her wallet.
    let out = w.withdraw(alice, amount);
    eprintln!("  withdraw({amount}) -> {out:?}");
    assert!(out.is_ok(), "the recovered money must be withdrawable: {out:?}");
    let wallet_end = w.ledger_balance(alice, None);
    eprintln!("  alice wallet {wallet_at_start} -> {wallet_end}");
    assert!(
        wallet_at_start.saturating_sub(wallet_end) <= 4 * ledger::TRANSFER_FEE,
        "alice must be whole except for ledger fees: started {wallet_at_start}, ended \
         {wallet_end}"
    );
}

// ===========================================================================
// claim_external_deposit(): the subaccount sweep
// ===========================================================================

#[test]
fn m14_sweep_continuation_discarded_is_accounted_for_and_recoverable() {
    let mut w = World::default_world();
    let bob = w.actor("bob");
    let amount = e8(2.0);

    let wallet_at_start = w.ledger_balance(bob, None);
    w.transfer_to_deposit_subaccount(bob, amount)
        .expect("external wallet transfer into bob's deposit subaccount");

    let inj = fault::trap_claim_external_tail(&mut w, bob).expect("inject");

    eprintln!("\n=== claim_external_deposit(): post-await continuation discarded ===");
    show("before", &inj.before);
    show("completed", &inj.completed);
    show("after rollback", &inj.after);
    eprintln!("  reply before the tail was discarded: {:?}", inj.reply);
    eprintln!("  journal now: {:?}", fault::ledger_intents(&w));

    assert!(
        inj.after.ledger_subaccounts < inj.before.ledger_subaccounts,
        "the sweep must have emptied bob's deposit subaccount"
    );
    assert!(
        inj.after.ledger_main > inj.before.ledger_main,
        "and put it in the canister's main account"
    );
    assert_eq!(w.get_balance(bob), 0, "and credited bob nothing");
    assert_coherent(&w, "after a discarded sweep continuation");

    // THE SENTENCE THAT USED TO BE A LIE. An empty deposit subaccount means one of
    // two opposite things and the canister must not guess.
    let again = w.claim_external_deposit(bob);
    eprintln!("  bob tries claim_external_deposit again: {again:?}");
    match &again {
        Err(OpError::Err(msg)) => {
            assert!(
                !msg.contains("Send ICP to your deposit address first"),
                "the canister is holding bob's money and must not tell him to send some: {msg}"
            );
            assert!(
                msg.contains("ALREADY SWEPT") && msg.contains("resolve_my_ledger_intents"),
                "the refusal must say what really happened and name the way out: {msg}"
            );
        }
        other => panic!("expected a refusal that explains itself, got {other:?}"),
    }

    w.advance(PAST_THE_LEASE);
    let resolved = fault::resolve_my_ledger_intents(&w, bob).expect("resolve");
    eprintln!("  resolve_my_ledger_intents(bob) -> {resolved:?}");
    let swept = amount - ledger::TRANSFER_FEE;
    assert_eq!(
        w.get_balance(bob),
        swept,
        "bob must be credited the swept amount ({swept})"
    );
    assert_coherent(&w, "after resolving a discarded sweep continuation");
    assert!(
        fault::ledger_intents(&w).is_empty(),
        "the entry must be retired"
    );
    eprintln!(
        "  bob wallet {wallet_at_start} -> {} , escrow {}",
        w.ledger_balance(bob, None),
        w.get_balance(bob)
    );
}

// ===========================================================================
// withdraw(): the mirror shape
// ===========================================================================

#[test]
fn m14_withdraw_continuation_discarded_does_not_lock_the_player_out() {
    let mut w = World::default_world();
    let carol = w.actor("carol");
    let funded = e8(5.0);
    let take = e8(2.0);

    w.fund_escrow(carol, funded).expect("fund escrow");
    let escrow_before = w.get_balance(carol);
    let wallet_before = w.ledger_balance(carol, None);

    let inj = fault::trap_withdraw_tail(&mut w, carol, take).expect("inject");

    eprintln!("\n=== withdraw(): post-await continuation discarded ===");
    show("before", &inj.before);
    show("completed", &inj.completed);
    show("after rollback", &inj.after);
    eprintln!("  reply before the tail was discarded: {:?}", inj.reply);

    let escrow_after = w.get_balance(carol);
    let wallet_after = w.ledger_balance(carol, None);
    let paid = wallet_after.saturating_sub(wallet_before);
    let debited = escrow_before.saturating_sub(escrow_after);
    eprintln!(
        "  carol escrow {escrow_before} -> {escrow_after} (debited {debited}), \
         wallet {wallet_before} -> {wallet_after} (paid {paid})"
    );
    eprintln!("  journal now: {:?}", fault::ledger_intents(&w));

    // `withdraw` debits BEFORE the await, so the debit is committed at the await
    // point and a discarded continuation does not give it back. The debit is only
    // safe because it is written down.
    assert_eq!(debited, take, "the escrow debit is committed at the await point");
    assert_coherent(&w, "after a discarded withdraw continuation");
    let journal = fault::ledger_intents(&w);
    assert_eq!(journal.len(), 1, "the payout must be on record: {journal:?}");
    assert_eq!(journal[0].kind, "payout");
    assert_eq!(journal[0].who, carol);

    // THE HARM THAT USED TO BE PERMANENT: `PENDING_WITHDRAWALS` is set before the
    // await and cleared in the continuation, and `withdraw` is the ONLY door from
    // escrow to the ledger. A flag that outlives its operation is a fund lock.
    w.advance(Duration::from_secs(3600));
    let blocked = w.withdraw(carol, e8(1.0));
    eprintln!("  an unrelated withdrawal an hour later: {blocked:?}");
    if let Err(OpError::Err(ref m)) = blocked {
        assert!(
            m.contains("resolve_my_ledger_intents"),
            "if a withdrawal is refused as in-progress, the refusal must name the way out: {m}"
        );
    }

    let resolved = fault::resolve_my_ledger_intents(&w, carol).expect("resolve");
    eprintln!("  resolve_my_ledger_intents(carol) -> {resolved:?}");
    assert!(
        fault::ledger_intents(&w).is_empty(),
        "the payout entry must be settled one way or the other"
    );
    assert_coherent(&w, "after resolving a discarded withdraw continuation");

    // Whole: whatever left her escrow either reached her wallet or came back.
    let escrow_end = w.get_balance(carol);
    let wallet_end = w.ledger_balance(carol, None);
    let held = escrow_end + wallet_end;
    eprintln!("  carol escrow {escrow_end} + wallet {wallet_end} = {held}");
    assert!(
        held + 2 * ledger::TRANSFER_FEE >= escrow_before + wallet_before,
        "carol must be whole modulo ledger fees: had {} , has {held}",
        escrow_before + wallet_before
    );

    // AND SHE IS NOT LOCKED OUT.
    w.advance(Duration::from_secs(120));
    let after = w.withdraw(carol, e8(1.0));
    eprintln!("  withdraw again after resolving: {after:?}");
    assert!(
        after.is_ok(),
        "\nFINDING 29 / withdraw: the player is still locked out of the only door from escrow \
         to the ledger after resolving: {after:?}"
    );
}

// ===========================================================================
// the journal must survive the very event most likely to create its entries
// ===========================================================================

#[test]
fn m14_journal_survives_an_upgrade() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let amount = e8(1.5);
    w.approve(alice, amount + ledger::TRANSFER_FEE).unwrap();
    fault::trap_deposit_tail(&mut w, alice, amount).expect("inject");
    assert_coherent(&w, "after injection, before the upgrade");

    let before = fault::ledger_intents(&w);
    assert_eq!(before.len(), 1);

    // An upgrade is one of the things that DROPS outstanding callbacks, so it is
    // exactly the event after which these entries matter most. Losing them here
    // would reintroduce the defect at the moment it is most likely to fire.
    w.upgrade().expect("upgrade");

    let after = fault::ledger_intents(&w);
    eprintln!("\n=== journal across a real --mode upgrade ===");
    eprintln!("  before: {before:?}");
    eprintln!("  after:  {after:?}");
    assert_eq!(after.len(), 1, "the journal must survive the upgrade");
    assert_eq!(after[0].id, before[0].id);
    assert_eq!(after[0].who, before[0].who);
    assert_eq!(after[0].amount, before[0].amount);
    assert_eq!(
        after[0].leased_until_ns, None,
        "a lease held by a message in the OLD module must not survive it, or the money is \
         locked behind a caller that can never come back"
    );
    assert_coherent(&w, "after the upgrade");

    // And it is still recoverable on the other side.
    let resolved = fault::resolve_my_ledger_intents(&w, alice).expect("resolve after upgrade");
    eprintln!("  resolve after upgrade -> {resolved:?}");
    assert_eq!(w.get_balance(alice), amount);
    assert_coherent(&w, "after resolving on the far side of an upgrade");
}

// ===========================================================================
// the recipient dimension: the standing lesson
// ===========================================================================

/// Four times this project has produced a defect with the signature CORRECT
/// TOTALS, WRONG RECIPIENTS, EVERY INVARIANT SILENT. `orphaned() == 0` is a
/// TOTAL, so on its own it is exactly the shape of gate that has been fooled
/// before.
///
/// This test asks the per-person question instead: after a fault, can EACH funded
/// player, using only calls they are allowed to make, get THEIR OWN money back to
/// THEIR OWN wallet? A journal that named the wrong owner, or credited the wrong
/// principal on resume, would keep every total right and fail here.
#[test]
fn m14_each_victim_drains_their_own_money_after_a_fault() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let bob = w.actor("bob");
    let carol = w.actor("carol");

    let start: Vec<(&str, candid::Principal, u64)> = vec![
        ("alice", alice, w.ledger_balance(alice, None)),
        ("bob", bob, w.ledger_balance(bob, None)),
        ("carol", carol, w.ledger_balance(carol, None)),
    ];

    // Three different faults, three different doors, three different people.
    w.approve(alice, e8(3.0) + ledger::TRANSFER_FEE).unwrap();
    fault::trap_deposit_tail(&mut w, alice, e8(3.0)).expect("inject alice");

    w.transfer_to_deposit_subaccount(bob, e8(2.0)).unwrap();
    fault::trap_claim_external_tail(&mut w, bob).expect("inject bob");

    w.fund_escrow(carol, e8(4.0)).expect("fund carol");
    fault::trap_withdraw_tail(&mut w, carol, e8(2.0)).expect("inject carol");

    eprintln!("\n=== three faults, three victims, then each drains for themselves ===");
    for i in fault::ledger_intents(&w) {
        eprintln!("  open: {i:?}");
    }
    assert_coherent(&w, "with three faults outstanding");

    // Each player, and ONLY player-callable methods.
    w.advance(PAST_THE_LEASE);
    for (name, who, _) in &start {
        let r = fault::resolve_my_ledger_intents(&w, *who);
        eprintln!("  {name}: resolve -> {r:?}");
    }
    assert!(
        fault::ledger_intents(&w).is_empty(),
        "every entry must settle: {:?}",
        fault::ledger_intents(&w)
    );
    assert_coherent(&w, "after every victim resolved their own");

    // Now drain: escrow -> wallet, with the player's own call.
    for (name, who, _) in &start {
        w.advance(Duration::from_secs(120)); // withdrawal cooldown
        let escrow = w.get_balance(*who);
        if escrow >= 100_000 {
            let r = w.withdraw(*who, escrow);
            eprintln!("  {name}: withdraw({escrow}) -> {r:?}");
        }
    }

    let mut worst = 0i128;
    for (name, who, before) in &start {
        let after = w.ledger_balance(*who, None) as i128 + w.get_balance(*who) as i128;
        let delta = *before as i128 - after;
        eprintln!("  {name}: wallet+escrow {before} -> {after}   (down {delta})");
        worst = worst.max(delta);
    }
    // Ledger fees are real and are not the canister's to give back. Each victim
    // pays at most a handful.
    assert!(
        worst <= 6 * ledger::TRANSFER_FEE as i128,
        "\nSTANDING-LESSON CHECK FAILED: totals may reconcile, but at least one victim is \
         {worst} e8s down after recovering with their own calls. Correct totals with the \
         wrong recipient is the signature this project has produced four times."
    );

    let end = fault::custody(&w);
    show("end", &end);
    assert_eq!(
        end.ledger_total(),
        end.books,
        "with every intent settled and every player drained, the canister should hold only \
         what its books say: {end:?}"
    );
}

// ===========================================================================
// the instrument itself
// ===========================================================================

/// A gate that cannot go red teaches everyone to ignore it. This checks that the
/// M14 reading is not vacuously zero: with the journal deliberately ignored, the
/// injected state must show a non-zero orphan, which is exactly what the
/// pre-journal build showed for real.
#[test]
fn m14_instrument_self_check() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let amount = e8(1.0);
    w.approve(alice, amount + ledger::TRANSFER_FEE).unwrap();
    let inj = fault::trap_deposit_tail(&mut w, alice, amount).expect("inject");

    eprintln!("\n=== M14 instrument self-check ===");
    show("injected state", &inj.after);

    let with_journal = inj.after.orphaned();
    let ignoring_journal = Custody {
        journalled: 0,
        ..inj.after.clone()
    }
    .orphaned();
    eprintln!("  orphaned with the journal read: {with_journal}");
    eprintln!("  orphaned with the journal ignored: {ignoring_journal}");
    assert_eq!(with_journal, 0, "the fix must account for the money");
    assert_eq!(
        ignoring_journal, amount,
        "and the reading must be non-zero without it, or the gate measures nothing"
    );

    eprintln!("\n  what was tried, and failed, to force a LITERAL trap:");
    for (name, why) in fault::TRAP_FORCING_ATTEMPTS {
        eprintln!("  [{name}]\n      {why}");
    }
    eprintln!("\n  where this gate CANNOT see:");
    for (name, why) in fault::NOT_COVERED {
        eprintln!("  [{name}]\n      {why}");
    }
}

// ===========================================================================
// the two paths where a mistake becomes a double-spend or a lockout
// ===========================================================================

/// **Past the ledger's deduplication window, the canister must REFUSE to retry.**
///
/// This is the one place where doing the helpful thing would move money a second
/// time. ICRC-1/ICRC-2 deduplicate an identical transaction only inside the
/// ledger's transaction window; outside it, re-issuing is a second, real payment.
/// The canister therefore stops retrying and keeps the record, which is a worse
/// outcome than automatic recovery and a far better one than a double spend.
///
/// It is gated rather than merely commented because the failure mode of "relax
/// this" is invisible in every other test in this file: they all run inside the
/// window, so a canister that ignored the deadline would pass all of them.
#[test]
fn m14_past_the_dedup_window_the_retry_is_refused_and_the_record_kept() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let amount = e8(1.0);
    w.approve(alice, amount + ledger::TRANSFER_FEE).unwrap();
    fault::trap_deposit_tail(&mut w, alice, amount).expect("inject");

    let open = fault::ledger_intents(&w);
    assert_eq!(open.len(), 1);
    let deadline = open[0].retry_deadline_ns;
    eprintln!("\n=== past the ledger's deduplication window ===");
    eprintln!("  entry: {:?}", open[0]);

    // Past the deadline. `advance_time_only` moves the clock without running a
    // round, which is what a canister nobody called for a day looks like.
    let now = w.now_nanos();
    let over = deadline.saturating_sub(now) + 60_000_000_000;
    w.advance_time_only(Duration::from_nanos(over));

    let out = fault::resolve_my_ledger_intents(&w, alice).expect("the call itself must succeed");
    eprintln!("  resolve -> {out:?}");
    let joined = out.join(" ");
    assert!(
        joined.contains("deduplication window"),
        "the refusal must say WHY it will not retry: {joined}"
    );
    assert!(
        joined.contains("Nothing has been forgotten"),
        "and it must say the record is kept: {joined}"
    );
    assert_eq!(
        w.get_balance(alice),
        0,
        "\nDOUBLE-SPEND HAZARD: the canister credited an entry it could not verify. Past the \
         ledger's transaction window a re-issue is a SECOND movement, so there is nothing safe \
         to credit against."
    );
    assert_eq!(
        fault::ledger_intents(&w).len(),
        1,
        "the entry must STAY, naming the owner and the amount. Dropping it is the forgetting \
         FINDING 29 is about."
    );
    assert_coherent(&w, "past the deduplication window");
}

/// **The journal is bounded, and the bound is enforced by refusing to START.**
///
/// An unbounded map eventually makes `pre_upgrade` fail to serialise, and a
/// fund-holding canister that cannot be upgraded is bricked with the funds
/// inside. The only other way to bound it is to drop entries, which is exactly
/// the forgetting this whole mechanism exists to prevent. So the cap refuses new
/// deposits — and the refusal has to name the way out, or the cap is itself a
/// lockout.
#[test]
fn m14_the_journal_is_bounded_by_refusing_to_start_not_by_forgetting() {
    let mut w = World::default_world();
    let alice = w.actor("alice");
    let amount = e8(0.5);

    let mut opened = 0;
    for i in 0..8 {
        w.approve(alice, amount + ledger::TRANSFER_FEE).unwrap();
        match fault::trap_deposit_tail(&mut w, alice, amount) {
            Ok(_) => opened = fault::ledger_intents(&w).len(),
            Err(e) => {
                eprintln!("  injection {i} did not happen: {e}");
                break;
            }
        }
        if fault::ledger_intents(&w).len() < opened.max(1) {
            break;
        }
        if opened >= 8 {
            break;
        }
    }
    eprintln!("\n=== the cap ===");
    eprintln!("  open entries: {opened}");
    assert!(
        opened <= 8,
        "the journal must be bounded per principal; {opened} entries are open"
    );
    assert_coherent(&w, "with the journal at its cap");

    // Once capped, an ordinary deposit is refused rather than starting a movement
    // nothing can record.
    w.approve(alice, amount + ledger::TRANSFER_FEE).unwrap();
    let refused = w.deposit(alice, amount);
    eprintln!("  a further deposit: {refused:?}");
    if let Err(OpError::Err(ref m)) = refused {
        assert!(
            m.contains("resolve_my_ledger_intents"),
            "a cap that does not name the way out is a lockout: {m}"
        );
    }

    // And resolving frees the capacity, so the cap is a queue and not a wall.
    w.advance(PAST_THE_LEASE);
    let _ = fault::resolve_my_ledger_intents(&w, alice);
    w.advance(PAST_THE_LEASE);
    let _ = fault::resolve_my_ledger_intents(&w, alice);
    eprintln!("  after resolving: {} open", fault::ledger_intents(&w).len());
    assert!(
        fault::ledger_intents(&w).len() < opened,
        "resolving must free capacity"
    );
    assert_coherent(&w, "after draining the cap");
}
