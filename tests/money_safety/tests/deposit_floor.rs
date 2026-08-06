//! THE FLOOR INVARIANT, driven through the real canister on PocketIC.
//!
//! > **The rule.** This canister must never accept an amount it will not return.
//!
//! docs/SECURITY-FINDINGS.md FINDING 27, docs/DEFECTS.md E-62. The third
//! independent auditor deposited **exactly the amount the product advertises as its
//! minimum** -- 20,000 e8s -- and could not get it out again:
//!
//! ```text
//! deposit(20_000)   Ok            `deposit()`'s floor was 20_000
//! withdraw(20_000)  Err("Minimum withdrawal is 0.0010 ICP")
//!                                 ICP_MIN_WITHDRAWAL_AMOUNT was 100_000
//! buy_in(...)       Err(...)      below min_buy_in by four orders of magnitude
//! get_custody_status()            reports it as an ordinary balance, advice = ""
//! ```
//!
//! Every conservation invariant in this harness was silent, and correctly so: not
//! one e8 had gone missing. It was all still in the canister, on the ledger, in the
//! player's own escrow row. **Conservation cannot see a balance nobody can move**,
//! which is the same blind spot M9 was written for one boundary over.
//!
//! What these tests assert is the reachability half, at the money doors rather than
//! at the table:
//!
//!   1. a deposit of exactly the advertised minimum leaves again, to the ledger;
//!   2. every amount in the old dead band leaves;
//!   3. a balance BELOW the floor -- which no floor choice can prevent, because the
//!      pot and the ledger fee both produce them -- leaves too, via the whole-balance
//!      sweep; and
//!   4. the sweep is narrow: a partial request below the floor is still refused, and
//!      the refusal names the way out instead of ending the conversation.
//!
//! Test 5 is the honest boundary: below the LEDGER's own transfer fee nothing can
//! move, whatever this canister does, and the canister has to say so rather than
//! report a healthy balance.

use money_safety::ledger;
use money_safety::table_api::*;
use money_safety::world::*;
use std::time::Duration;

/// The advertised minimum deposit, `ICP_MIN_DEPOSIT_AMOUNT`. Written here as the
/// number a player reads on the deposit screen, because that is the thing under
/// test: the promise the product makes out loud.
const ADVERTISED_MINIMUM_DEPOSIT: u64 = 20_000;

/// `ICP_MIN_WITHDRAWAL_AMOUNT` before the fix. Not a limit any more -- it is the
/// top of the band that used to be a one-way door, and these tests walk it.
const OLD_WITHDRAWAL_FLOOR: u64 = 100_000;

/// One withdrawal per `WITHDRAWAL_COOLDOWN_NS`.
fn past_the_cooldown(world: &World) {
    world.advance(Duration::from_secs(61));
}

/// Everything this suite needs: one funded actor, nobody seated, no hand.
fn lone_depositor(name: &str) -> (World, candid::Principal) {
    let world = World::new(TableConfig::heads_up_icp(), &[name]);
    let who = world.actor(name);
    (world, who)
}

// ---------------------------------------------------------------------------
// 1. the auditor's exact sequence, now ending the other way
// ---------------------------------------------------------------------------

#[test]
fn a_deposit_of_exactly_the_advertised_minimum_can_be_withdrawn() {
    let (world, alice) = lone_depositor("alice");

    let wallet_before = world.ledger_balance(alice, None);
    world
        .fund_escrow(alice, ADVERTISED_MINIMUM_DEPOSIT)
        .expect("a deposit of exactly the advertised minimum must be accepted");
    assert_eq!(
        world.get_balance(alice),
        ADVERTISED_MINIMUM_DEPOSIT,
        "escrow must hold what was deposited"
    );

    let block = world.withdraw(alice, ADVERTISED_MINIMUM_DEPOSIT).expect(
        "THE AUDITOR'S BLOCKER: a deposit of exactly the advertised minimum must be \
         withdrawable. If this is an Err naming a minimum withdrawal, the two floors have \
         drifted apart again -- see THE FLOOR INVARIANT in src/table_canister/src/lib.rs.",
    );
    assert!(block > 0, "a successful withdrawal returns a ledger block index");

    assert_eq!(world.get_balance(alice), 0, "escrow must be empty afterwards");

    // The money is back on the LEDGER, not merely off the escrow row. The round
    // trip costs exactly three LEDGER fees and nothing else: `icrc2_approve`,
    // the `icrc2_transfer_from` the deposit pulls with, and the `icrc1_transfer`
    // the withdrawal sends (`transfer_tokens` sends `amount - fee`). The 20,000
    // e8s of principal itself comes back whole -- ClearDeck keeps none of it,
    // which is the no-rake property applied to the money doors.
    let wallet_after = world.ledger_balance(alice, None);
    let expected = wallet_before - 3 * ledger::TRANSFER_FEE;
    assert_eq!(
        wallet_after, expected,
        "the wallet should be {expected} (down exactly three ledger fees, no principal), \
         not {wallet_after}"
    );
}

// ---------------------------------------------------------------------------
// 2. the whole dead band, walked
// ---------------------------------------------------------------------------

#[test]
fn every_amount_in_the_old_dead_band_can_leave() {
    // Each of these was accepted by `deposit()` and refused by `withdraw()`.
    let band = [
        ADVERTISED_MINIMUM_DEPOSIT,
        ADVERTISED_MINIMUM_DEPOSIT + 1,
        50_000,
        OLD_WITHDRAWAL_FLOOR - 1,
        OLD_WITHDRAWAL_FLOOR,
    ];

    let (world, alice) = lone_depositor("alice");
    for amount in band {
        world
            .fund_escrow(alice, amount)
            .unwrap_or_else(|e| panic!("deposit of {amount} e8s refused: {e:?}"));
        let before = world.ledger_balance(alice, None);
        world
            .withdraw(alice, amount)
            .unwrap_or_else(|e| panic!("{amount} e8s went in and cannot come out: {e:?}"));
        let after = world.ledger_balance(alice, None);
        assert_eq!(
            after - before,
            amount - ledger::TRANSFER_FEE,
            "withdrawing {amount} e8s must put {} e8s back on the ledger",
            amount - ledger::TRANSFER_FEE
        );
        assert_eq!(world.get_balance(alice), 0, "escrow must be empty after {amount}");
        past_the_cooldown(&world);
    }
}

// ---------------------------------------------------------------------------
// 3. below the floor, which is where a floor alone would still trap money
// ---------------------------------------------------------------------------

/// A consistent pair of floors is not enough on its own, and this is the state that
/// shows it. Escrow balances are not only made of deposits: a partial withdrawal
/// leaves the remainder, an odd-chip split leaves a few e8s, and
/// `claim_external_deposit` credits the swept amount minus a ledger fee. Every one
/// of those can leave a balance under the floor, and every one of them is money the
/// canister took custody of.
#[test]
fn a_balance_below_the_floor_can_still_be_swept_whole() {
    let (world, alice) = lone_depositor("alice");

    // Deposit 40,000 and take 25,000 out. What is left is 15,000 e8s: under the
    // 20,000 floor, over the 10,000 ledger fee, and nothing to do with a deposit.
    world.fund_escrow(alice, 40_000).expect("deposit");
    world.withdraw(alice, 25_000).expect("a withdrawal above the floor");
    past_the_cooldown(&world);

    let residue = world.get_balance(alice);
    assert_eq!(residue, 15_000, "the state under test is a sub-floor residue");

    let before = world.ledger_balance(alice, None);
    world.withdraw(alice, residue).unwrap_or_else(|e| {
        panic!(
            "a sub-floor residue of {residue} e8s must be sweepable in one call. This is the \
             half of FINDING 27 that making the two floors equal does NOT fix, because the pot \
             and the ledger fee both produce balances the deposit door never saw: {e:?}"
        )
    });

    assert_eq!(world.get_balance(alice), 0, "the sweep must empty the row");
    assert_eq!(
        world.ledger_balance(alice, None) - before,
        residue - ledger::TRANSFER_FEE,
        "the swept residue must arrive on the ledger, less the one fee"
    );
}

// ---------------------------------------------------------------------------
// 4. the waiver is narrow, and the refusal points at the exit
// ---------------------------------------------------------------------------

#[test]
fn a_partial_request_below_the_floor_is_still_refused_and_names_the_way_out() {
    let (world, alice) = lone_depositor("alice");
    world.fund_escrow(alice, 40_000).expect("deposit");

    let err = world
        .withdraw(alice, 15_000)
        .expect_err("a PARTIAL request below the floor is not the sweep and must be refused");
    let text = format!("{err:?}");
    assert!(
        text.contains("Minimum withdrawal"),
        "the refusal must still state the floor: {text}"
    );
    assert!(
        text.contains("whole remaining balance"),
        "THE REFUSAL HAS TO NAME THE RECOVERY. A player under the floor who is told only \
         'minimum withdrawal is X' concludes their money is stuck, which is exactly what \
         happened to the third auditor. It must say the whole balance can always leave: {text}"
    );

    // ...and the balance is untouched by the refusal.
    assert_eq!(world.get_balance(alice), 40_000);
}

// ---------------------------------------------------------------------------
// 5. the honest boundary: the LEDGER's fee, which no floor can argue with
// ---------------------------------------------------------------------------

/// At or below the transfer fee there is no amount the ledger will move, so this
/// canister cannot return it however the floors are set. The requirement here is not
/// that the money leaves -- it cannot -- but that the canister SAYS SO at the door
/// instead of reporting an ordinary balance and refusing with a number that looks
/// like a policy choice.
#[test]
fn a_residue_at_the_transfer_fee_is_refused_in_words_that_explain_it() {
    let (world, alice) = lone_depositor("alice");
    world.fund_escrow(alice, 30_000).expect("deposit");
    world.withdraw(alice, 20_000).expect("a withdrawal at the floor");
    past_the_cooldown(&world);

    let residue = world.get_balance(alice);
    assert_eq!(residue, ledger::TRANSFER_FEE, "the state under test is a fee-sized residue");

    let err = world
        .withdraw(alice, residue)
        .expect_err("the ledger cannot move an amount equal to its own fee");
    let text = format!("{err:?}");
    assert!(
        text.contains("network fee"),
        "the refusal must name the LEDGER FEE as the reason, not a policy minimum, because \
         the player cannot act on a policy minimum here: {text}"
    );
}

// ---------------------------------------------------------------------------
// 6. the sweep did not open a door anywhere else
// ---------------------------------------------------------------------------

/// The waiver relaxes exactly one comparison. Everything else `withdraw` refuses,
/// it must still refuse -- especially "more than you have", which is the one an
/// `amount == balance` special case could plausibly break.
#[test]
fn the_sweep_does_not_weaken_any_other_refusal() {
    let (world, alice) = lone_depositor("alice");
    world.fund_escrow(alice, 40_000).expect("deposit");

    world
        .withdraw(alice, 40_001)
        .expect_err("more than the balance must still be refused");
    world
        .withdraw(alice, 0)
        .expect_err("zero must still be refused");
    assert_eq!(world.get_balance(alice), 40_000, "no refusal may move money");

    // The cooldown still binds after a successful sweep.
    world.withdraw(alice, 40_000).expect("the sweep");
    world.fund_escrow(alice, 40_000).expect("deposit again");
    world
        .withdraw(alice, 40_000)
        .expect_err("the withdrawal cooldown must still bind immediately after a sweep");
    past_the_cooldown(&world);
    world
        .withdraw(alice, 40_000)
        .expect("and must lift when it expires");
}
