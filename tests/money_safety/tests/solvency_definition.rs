//! **ONE DEFINITION OF HELD, ONE DEFINITION OF OWED, AND NOBODY OUTSIDE CAN MOVE
//! THE BOUNDARY BETWEEN THEM.**
//!
//! docs/SECURITY-FINDINGS.md FINDING 43 and FINDING 38, which are the same defect
//! seen from two sides.
//!
//! # What was wrong
//!
//! `get_solvency()` computed `held` and `owed` from two different definitions,
//! and `total_liability()` -- the number the currency guard reads -- from a
//! third:
//!
//! ```text
//!   held  = main + deposit_subaccounts + pulls_in_flight - exit_leakage
//!   owed  = escrow + chips + pot + unswept + unfinished_incoming + payouts
//!   guard = escrow + chips + unswept + unfinished_incoming + main_uncredited
//! ```
//!
//! `held` carried the main-account balance and `owed` did not, so every e8 at the
//! shared main account was an asset and never a liability, and the difference
//! reported it as **surplus** -- in the same reply whose `unattributed_at_main`
//! field calls it *"money held for somebody this canister cannot yet name"*
//! (FINDING 43). And `pulls_in_flight` was added to BOTH sides on the strength of
//! the main-account reading not yet containing the money, while
//! `refresh_solvency()` -- public, unpermissioned, anonymous-callable -- exists
//! precisely to make the reading contain it (FINDING 38).
//!
//! # What is true now, and what these tests hold
//!
//! ```text
//!   held  = main_balance_now + deposit_subaccounts_observed - exit_leakage
//!   owed  = max(escrow + chips + pot + payouts + pulls, main_balance_now)
//!           + (unswept_deposits + sweeps_in_flight)
//!   guard = owed, the same function, not a second sum
//! ```
//!
//! so
//!
//! ```text
//!   difference = min(0, main_balance_now - (escrow + chips + pot + payouts + pulls))
//! ```
//!
//! **This canister can never report a surplus.** Money at its own accounts that no
//! player is credited with is somebody's, and it is a liability. That is the whole
//! of FINDING 43, and it makes the FINDING 38 lever inert: the pull is on the owed
//! side, so the reading that reveals its arrival reveals the liability with it.
//!
//! Run: `cd tests/money_safety && cargo test --test solvency_definition -- --nocapture`

use candid::{decode_one, Decode, Encode, Principal};
use money_safety::invariants::{self, Exemptions};
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{fault, ledger};

const ICP: u64 = 100_000_000;

fn main_account_address(world: &World) -> String {
    ledger::account_identifier_hex(&world.table, None)
}

fn solvency_of(world: &World) -> SolvencyReport {
    world.solvency().expect(
        "get_solvency() must answer an ANONYMOUS caller: a player has to be able to ask \
         whether the table can pay them without permission",
    )
}

fn read_every_account(world: &mut World, also: &[Principal]) -> SolvencyReport {
    world
        .admin_audit_deposit_custody(also)
        .expect("the controller audit must run");
    solvency_of(world)
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

fn show(tag: &str, r: &SolvencyReport) {
    println!(
        "{tag}: verdict={:?} owed={} guard_liability={} held={:?} diff={:?} \
         shortfall={:?} unattributed={:?} pulls={} payouts={}",
        r.verdict,
        r.owed,
        r.guard_liability,
        r.held,
        r.difference_e8s,
        r.shortfall_e8s,
        r.unattributed_at_main,
        r.pulls_in_flight,
        r.payouts_in_flight
    );
}

/// The three properties that ARE "one definition", asserted on every report this
/// file produces so no test can pass while the instrument has two of anything.
fn assert_one_definition(r: &SolvencyReport, where_: &str) {
    assert_one_definition_numbers(r, where_);
    assert!(
        !r.summary.contains("a surplus of"),
        "{where_}: the player-facing summary claims a surplus. This canister takes no rake \
         and keeps nothing, so it has none, ever: what it holds and cannot attribute is \
         somebody's money. {}",
        r.summary
    );
}

/// The arithmetic half of [`assert_one_definition`], without the claim about the
/// words. Split out only so a report taken BEFORE the defect is planted can be
/// checked without the check that the fix rewrites.
fn assert_one_definition_numbers(r: &SolvencyReport, where_: &str) {
    // 1. The guard and the instrument are the same number. `guard_liability` is
    //    `total_liability()`, the single input to
    //    `refuse_currency_change_while_funded`; `owed` is what the verdict and the
    //    player-facing summary are computed from. Two totals of one liability in
    //    one reply is FINDING 43.
    assert_eq!(
        r.owed, r.guard_liability,
        "{where_}: the public `owed` ({}) and the guard's `total_liability()` ({}) are two \
         different totals of the same liability, in one reply. \
         docs/SECURITY-FINDINGS.md FINDING 43.",
        r.owed, r.guard_liability
    );

    // 2. The total is rebuildable from the published terms, INCLUDING the
    //    main-account residual. A term in the total and not in the breakdown is a
    //    term no reader can audit.
    let term_sum = r
        .escrow
        .saturating_add(r.chips_at_table)
        .saturating_add(r.pot.max(r.committed_stake))
        .saturating_add(r.unswept_deposits)
        .saturating_add(r.unfinished_incoming)
        .saturating_add(r.payouts_in_flight)
        .saturating_add(r.unattributed_at_main.unwrap_or(0));
    assert_eq!(
        term_sum, r.owed,
        "{where_}: owed={} and its own published terms sum to {term_sum} (escrow {} + chips \
         {} + max(pot {}, committed {}) + unswept {} + unfinished_incoming {} + payouts {} + \
         unattributed_at_main {:?})",
        r.owed,
        r.escrow,
        r.chips_at_table,
        r.pot,
        r.committed_stake,
        r.unswept_deposits,
        r.unfinished_incoming,
        r.payouts_in_flight,
        r.unattributed_at_main
    );

    // 3. THE PROPERTY FINDING 43 IS. Money this canister holds and cannot
    //    attribute is a liability, so the difference can never be positive and the
    //    word "surplus" can never be true of it.
    if let Some(d) = r.difference_e8s {
        assert!(
            d <= 0,
            "{where_}: get_solvency() reports a SURPLUS of {d} e8s. This canister takes no \
             rake and keeps nothing: an e8 at its own accounts that no player is credited \
             with is money held for somebody it cannot yet name, which is a liability. \
             docs/SECURITY-FINDINGS.md FINDING 43."
        );
    }
}

// ===========================================================================
// FINDING 43 -- the money at the shared main account
// ===========================================================================

/// **Plant an unattributed balance at the shared main account and the public
/// reply must not call it a surplus.**
///
/// The door is the one FINDING 34's published address sent every player to for
/// eleven waves: the canister's MAIN account, addressed by its 64-hex account
/// identifier through the ICP ledger's legacy `transfer`. There is ICP at that
/// account on mainnet today.
///
/// The table is NOT empty here -- alice has 5 ICP of real escrow -- because an
/// empty table makes the wrong arithmetic look like a rounding question. With
/// escrow present the unfixed build reports `owed = 500000000`, `held =
/// 600000000` and *"a surplus of 100000000 e8s"* about a stranger's ICP.
#[test]
fn money_at_the_main_account_that_nobody_is_credited_with_is_never_a_surplus() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    world.fund_escrow(alice, 5 * ICP).expect("alice's deposit");
    let before = read_every_account(&mut world, &[alice, bob]);
    show("BEFORE", &before);
    assert_one_definition_numbers(&before, "before the stray transfer");
    assert_eq!(
        before.difference_e8s,
        Some(0),
        "a table holding exactly what it owes must report a difference of zero"
    );

    // bob sends 1 ICP to the SHARED main account. No message reaches the canister.
    let main = main_account_address(&world);
    world
        .legacy_transfer_to_address(bob, &main, ICP, None)
        .expect("legacy transfer to the shared main account");

    // The public, unpermissioned, moves-no-money button the deposit screen offers.
    let r = world
        .refresh_solvency(bob)
        .expect("refresh_solvency is public and moves no money");
    show("AFTER ", &r);
    println!("summary = {}", r.summary);

    // The datum is present and always was: the canister knows it holds money for
    // somebody it cannot name.
    assert_eq!(
        r.unattributed_at_main,
        Some(ICP),
        "the canister must still name the money it cannot attribute"
    );

    // THE FIX. It is on the OWED side now, in the one total, in both readers.
    assert_one_definition(&r, "with 1 ICP unattributed at the main account");
    assert_eq!(
        r.owed,
        5 * ICP + ICP,
        "owed must carry alice's 5 ICP of escrow AND the 1 ICP the canister is holding for \
         somebody it cannot name"
    );
    assert_eq!(
        r.difference_e8s,
        Some(0),
        "the canister holds exactly what it owes -- 6 ICP against 6 ICP -- so the difference \
         is zero. It is NOT 1 ICP of profit."
    );
    assert_eq!(r.verdict, SolvencyVerdict::CanPayEveryone);

    // And the sentence a player reads must NAME it rather than book it as profit.
    assert!(
        r.summary.contains("100000000")
            && (r.summary.contains("cannot yet name") || r.summary.contains("cannot name")),
        "the summary must say, in words, that 1 ICP of what it holds is for somebody it \
         cannot name -- publishing the number in a field nobody reads is what FINDING 43 \
         already did. summary = {}",
        r.summary
    );

    // Every wired-in leg must be silent on the corrected report: a fix that
    // trips the fuzzer's own instrument is not a fix.
    let snap = world.snapshot();
    let vs = invariants::check_point_in_time(&snap, Exemptions::of(&world));
    assert!(vs.is_empty(), "nothing may go red on the corrected report: {vs:#?}");
}

// ===========================================================================
// FINDING 38 -- the lever anybody could pull
// ===========================================================================

/// **An anonymous caller hammering the public refresh button must never turn a
/// real shortfall into an all-clear.**
///
/// The exact FINDING 38 sequence, driven with the FINDING 29 injector:
///
/// 1. alice funds 8 ICP of escrow, and 1.0001 ICP is moved out of the canister's
///    own main account behind its back. The canister is genuinely short.
/// 2. bob deposits 2 ICP and his post-await continuation is discarded: the ICRC-2
///    pull STANDS, the money is at the main account, bob is credited with
///    nothing, and the `pull` intent stays open. Past `INTENT_RETRY_WINDOW_NS`
///    that entry is kept forever pending reconciliation, so the window is durable
///    rather than a race.
/// 3. Anybody -- including `Principal::anonymous()` -- calls `refresh_solvency()`
///    until the rate limit bites.
///
/// On the unfixed build step 3 moved `main_account` up by bob's 2 ICP, added the
/// same 2 ICP to `held` a second time through `pulls_in_flight`, and published
/// `CanPayEveryone` with a surplus, on a canister that was still 1.0001 ICP short.
///
/// The anchor here is OUTSIDE the canister: the true shortfall is computed from
/// `icrc1_balance_of` across every account the canister owns and from the amount
/// the ledger really moved, never from a figure the canister collected.
#[test]
fn an_anonymous_refresh_can_never_flip_a_real_shortfall_into_an_all_clear() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let anonymous = Principal::anonymous();

    world.fund_escrow(alice, 8 * ICP).expect("alice's deposit");
    read_every_account(&mut world, &[alice, bob]);

    // A real hole: money leaves the canister's own main account with no message
    // to it. This is the local stand-in for the mainnet residue.
    let hole = ICP + 10_000;
    let args = ledger::TransferArg {
        from_subaccount: None,
        to: ledger::Account {
            owner: alice,
            subaccount: None,
        },
        amount: candid::Nat::from(hole),
        fee: Some(candid::Nat::from(ledger::TRANSFER_FEE)),
        memo: None,
        created_at_time: None,
    };
    let bytes = world
        .pic
        .update_call(
            world.ledger,
            world.table,
            "icrc1_transfer",
            Encode!(&args).expect("encode"),
        )
        .expect("drain the main account behind the canister's back");
    Decode!(&bytes, ledger::LedgerResult)
        .expect("decode")
        .block()
        .expect("the drain must land");

    let short_before = world
        .refresh_solvency(anonymous)
        .expect("refresh_solvency must answer an ANONYMOUS caller");
    show("STEP 1 truly short", &short_before);
    assert_eq!(
        short_before.verdict,
        SolvencyVerdict::CannotPayEveryone,
        "the fixture must start from a REAL, correctly reported shortfall or it tests nothing"
    );
    assert_one_definition(&short_before, "step 1");

    // STEP 2. bob's 2 ICP lands at the main account and his credit is discarded.
    let pull = 2 * ICP;
    world
        .approve(bob, pull + ledger::TRANSFER_FEE)
        .expect("approve");
    let inj = fault::trap_deposit_tail(&mut world, bob, pull)
        .expect("the FINDING 29 injector must reach the await");
    println!(
        "STEP 2 injection: ledger_main {} -> {}, bob's escrow = {}",
        inj.before.ledger_main,
        inj.after.ledger_main,
        world.get_balance(bob)
    );
    assert_eq!(
        inj.after.ledger_main - inj.before.ledger_main,
        pull,
        "the pull must really have moved and stayed moved"
    );
    assert_eq!(
        world.get_balance(bob),
        0,
        "and bob's credit must really have been discarded"
    );
    let journal = fault::ledger_intents(&world);
    assert_eq!(journal.len(), 1, "exactly one open intent: {journal:?}");
    assert_eq!(journal[0].kind, "pull");
    assert_eq!(journal[0].who, bob);

    // THE ANCHOR, TAKEN OUTSIDE THE CANISTER. What the ledger holds across every
    // account this canister owns, against what is really owed: alice's escrow as
    // the canister's own books state it, plus the 2 ICP of bob's that arrived and
    // was credited to nobody.
    let snap = world.snapshot();
    let true_owed = snap.escrow_total + pull;
    let true_held = snap.ledger_holdings();
    let true_short = true_owed - true_held;
    println!(
        "ANCHOR: ledger holds {true_held} (main {} + subaccounts {}), really owes {true_owed} \
         (escrow {} + bob's arrived-and-uncredited {pull}) => SHORT {true_short}",
        snap.ledger_main, snap.ledger_deposit_subaccounts, snap.escrow_total
    );
    assert_eq!(
        true_short,
        hole + ledger::TRANSFER_FEE,
        "the hole must be exactly what was drained, plus the fee the ledger burned to do it"
    );

    // STEP 3. Hammer the public button as an ANONYMOUS principal.
    //
    // `MAX_SOLVENCY_REFRESH_PER_MINUTE` is 30 per caller, so ten is well inside
    // the limit and the refusal is not what is being measured.
    for round in 1..=10 {
        let r = world
            .refresh_solvency(anonymous)
            .expect("refresh_solvency must stay available to an anonymous caller");
        if round == 1 || round == 10 {
            show(&format!("STEP 3 refresh #{round}"), &r);
            println!("summary = {}", r.summary);
        }
        assert_one_definition(&r, &format!("anonymous refresh #{round}"));
        assert_eq!(
            r.verdict,
            SolvencyVerdict::CannotPayEveryone,
            "ROUND {round}: one free, unpermissioned call turned a canister that is really \
             SHORT {true_short} e8s into {:?}. docs/SECURITY-FINDINGS.md FINDING 38. \
             summary = {}",
            r.verdict,
            r.summary
        );
        assert_eq!(
            r.shortfall_e8s,
            Some(true_short),
            "ROUND {round}: the published shortfall must be the one the LEDGER implies, to \
             the e8. The pull is on the owed side, so the reading that reveals its arrival \
             reveals the liability with it."
        );
        assert!(
            r.summary.contains("CANNOT PAY EVERYONE"),
            "ROUND {round}: the sentence a player reads must say so. summary = {}",
            r.summary
        );
    }

    // And the query half, read as anonymous, says the same thing.
    let q = solvency_of(&world);
    show("QUERY ", &q);
    assert_eq!(q.verdict, SolvencyVerdict::CannotPayEveryone);
    assert_eq!(q.shortfall_e8s, Some(true_short));

    // The instrument that is anchored outside the canister must agree, and it
    // must not have had to be told.
    let snap = world.snapshot();
    let vs = invariants::check_insolvency_is_reported(&world, &snap);
    let unreported: Vec<_> = vs
        .iter()
        .filter(|v| v.check.contains("insolvency_is_not_reported"))
        .collect();
    assert!(
        unreported.is_empty(),
        "the canister must report the insolvency the ledger can see: {unreported:#?}"
    );
}

/// **`held` is built from ledger readings and from nothing else.**
///
/// The pull sits on the OWED side and on no other, and the two branches are:
///
/// * the money HAS arrived (this test, because the FINDING 29 injector lands it):
///   it is inside `main_account`, so it is held once and owed once and the
///   difference is EXACTLY ZERO -- which is what the run prints;
/// * the money has NOT arrived: it is in neither `main_account` nor the escrow
///   books, so the canister cries poor by the pull amount until somebody calls
///   `resolve_my_ledger_intents()`. That is the safe direction, it is named in
///   `summary` in words, and it is the trade FINDING 38 asks for: a false alarm
///   is recoverable, a false all-clear is what FINDING 35 and FINDING 38 were.
///
/// What must never happen is the third thing, which is what shipped: the pull on
/// BOTH sides, so that one anonymous `refresh_solvency()` -- whose whole purpose
/// is to put the arrival into `main_account` -- publishes the same e8s twice.
#[test]
fn an_open_pull_is_owed_and_never_held() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    world.fund_escrow(alice, 4 * ICP).expect("alice's deposit");
    let before = read_every_account(&mut world, &[alice, bob]);
    show("BEFORE", &before);
    assert_eq!(before.pulls_in_flight, 0);
    assert_eq!(before.difference_e8s, Some(0));

    let pull = 2 * ICP;
    world
        .approve(bob, pull + ledger::TRANSFER_FEE)
        .expect("approve");
    fault::trap_deposit_tail(&mut world, bob, pull).expect("inject");

    let during = world.refresh_solvency(bob).expect("refresh");
    show("DURING", &during);
    assert_eq!(during.pulls_in_flight, pull, "the intent must really be open");
    assert_one_definition(&during, "with a pull in flight");

    // `held` is built from the ledger readings and from NOTHING ELSE.
    let expected_held = during
        .main_account
        .expect("main account has been read")
        .saturating_add(during.deposit_subaccounts)
        .saturating_sub(during.sweep_fees_in_flight);
    assert_eq!(
        during.held,
        Some(expected_held),
        "held must be main_account + deposit_subaccounts - exit leakage, with no in-flight \
         term added to it. An open pull added to `held` is FINDING 38: `refresh_solvency()` \
         is public and its entire purpose is to make `main_account` already contain the \
         money."
    );

    // Resolving the intent closes the gap, because the money really did arrive.
    world.advance(std::time::Duration::from_secs(45));
    let resolved = fault::resolve_my_ledger_intents(&world, bob);
    println!("resolve_my_ledger_intents(bob) -> {resolved:?}");
    let after = world.refresh_solvency(bob).expect("refresh");
    show("AFTER ", &after);
    assert_eq!(after.pulls_in_flight, 0, "the intent must be retired");
    assert_eq!(
        after.difference_e8s,
        Some(0),
        "and the answer must be back to exact: bob is credited, the money is at the main \
         account, and the two sides agree"
    );
    assert_one_definition(&after, "after resolving the pull");

    let snap = world.snapshot();
    let vs = invariants::check_point_in_time(&snap, Exemptions::of(&world));
    assert!(vs.is_empty(), "nothing may go red: {vs:#?}");
}

// ===========================================================================
// THE HOLE THE ONE DEFINITION CLOSED ON THE WAY PAST
// ===========================================================================

/// **A currency flip must be refused while a `payout` is in flight, and it was
/// not.**
///
/// `total_liability()`'s own doc comment has argued since FINDING 29 that a table
/// with unfinished ledger operations is a funded table: the currency selects the
/// LEDGER, and re-issuing an open intent after a flip sends the retry to a chain
/// where the original transaction does not exist, so the ledger's deduplication
/// cannot fire and the movement happens a second time. That argument was applied
/// to `pull` and `sweep` -- `journalled_incoming_total()` -- and **never to
/// `payout`, which was in no term of the guard at all.**
///
/// So a table drained to zero escrow with a 5 ICP withdrawal open read a
/// liability of ZERO and accepted the flip. The one definition closes it without
/// a special case: `main_attributed_claims()` carries `open_payout_total()`, so
/// `total_liability()` cannot be zero while any intent is open.
///
/// ```text
/// PRE-FIX  (total_liability = escrow + claims + deposits + incoming + main_uncredited)
///   escrow 0, claims 0, deposits 0, incoming 0 (payouts are in no term),
///   main_uncredited = max(0, main_read - escrow - claims - payouts) = 0
///   => total_liability = 0  => ACCEPTED currency now BTC
/// POST-FIX (total_liability = max(escrow + claims + payouts + pulls, main) + deposits)
///   => 500000000  => REFUSED
/// ```
#[test]
fn the_currency_guard_refuses_while_a_payout_is_still_in_flight() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    let btc = TableConfig {
        currency: Currency::BTC,
        ..TableConfig::six_max_icp()
    };

    world.fund_escrow(alice, 5 * ICP).expect("alice's deposit");
    read_every_account(&mut world, &[alice]);

    // Withdraw the WHOLE escrow, with the post-await continuation discarded: the
    // ledger transfer stands, escrow is already 0, and the payout intent stays
    // open. This is the FINDING 29 end state on the outgoing side.
    let inj = fault::trap_withdraw_tail(&mut world, alice, 5 * ICP)
        .expect("the FINDING 29 injector must reach the await");
    println!(
        "after the discarded withdrawal continuation: ledger_main {} -> {}, alice's escrow = {}",
        inj.before.ledger_main,
        inj.after.ledger_main,
        world.get_balance(alice)
    );
    assert_eq!(
        world.get_balance(alice),
        0,
        "the fixture needs escrow at ZERO, or the guard refuses for an unrelated reason"
    );
    let journal = fault::ledger_intents(&world);
    assert_eq!(journal.len(), 1, "exactly one open intent: {journal:?}");
    assert_eq!(journal[0].kind, "payout");

    let r = solvency_of(&world);
    show("WITH A PAYOUT OPEN", &r);
    assert!(
        r.payouts_in_flight > 0,
        "the fixture must actually leave a payout open, or it is testing nothing"
    );

    // THE DESTRUCTIVE OPERATION ITSELF, FIRST, so that the conviction this test
    // exists for is the one that fires and not a bookkeeping assertion about the
    // report on the way to it.
    let verdict = flip(&world, &btc);
    println!("flip to BTC with a payout in flight -> {verdict}");
    assert!(
        verdict.starts_with("REFUSED"),
        "a currency flip while a ledger intent is open re-points its retry at a chain where \
         the original transaction does not exist, so the deduplication cannot fire and the \
         money moves a second time. That is the argument total_liability() has carried for \
         `pull` since FINDING 29; it was never applied to `payout`. {verdict}"
    );

    assert!(
        r.guard_liability > 0,
        "THE HOLE. total_liability() -- the ONLY number \
         refuse_currency_change_while_funded reads -- was ZERO on a canister with an \
         irreversible 5 ICP ledger movement still open in its own journal. \
         guard_liability={} payouts_in_flight={}",
        r.guard_liability,
        r.payouts_in_flight
    );
    assert_one_definition(&r, "with a payout in flight and escrow at zero");
}
