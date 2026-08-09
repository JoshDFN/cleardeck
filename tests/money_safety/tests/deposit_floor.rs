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
//!
//! # Tests 7 onwards: THE OTHER DOOR, which this file did not touch for five waves
//!
//! docs/SECURITY-FINDINGS.md FINDING 31, docs/DEFECTS.md E-89. Everything above
//! goes through `deposit()` -- the ICRC-2 approve route, where the depositor's own
//! wallet pays the ledger fee alongside the amount, so a deposit of the floor
//! arrives as the floor. **Not one of those six tests sends anything to a deposit
//! subaccount**, and the canister publishes a deposit ADDRESS as well as a deposit
//! method. On that route the fee comes out of the money, which changes the
//! arithmetic every test above rests on:
//!
//! ```text
//! deposit(20_000)                       -> escrow 20_000   (wallet paid the fee)
//! transfer 20_000 to the address, claim -> escrow 10_000   (the sweep paid it)
//! ```
//!
//! and 10,000 is exactly the ledger fee, so it can never leave. The advertised
//! minimum through one door was a total loss through the other, and six green
//! tests and a compile-time invariant sat on top of it because they were all
//! measuring the door that worked. The fuzzer eventually found it at its own
//! default seeds, which is how a defect gets a gate five waves after its finding.
//!
//! What tests 7 onwards assert, for every amount in and around the dead band, is
//! the pair the register kept asking for: **what the player is TOLD, and whether
//! the money is RECOVERABLE** -- recoverable measured at the player's own wallet
//! on the ledger, never at an escrow row inside the canister.

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

/// `ICP_TRANSFER_FEE`. The one number in this file that is not a policy choice:
/// no transfer of any positive amount costs less, so an account holding this much
/// or less can deliver nothing to anybody.
const FEE: u64 = ledger::TRANSFER_FEE;

/// `ICP_MIN_WITHDRAWAL_AMOUNT`, the floor `withdraw` applies to a partial request.
const WITHDRAWAL_FLOOR: u64 = 20_000;

/// `ICP_MIN_EXTERNAL_DEPOSIT` = `min_withdrawal + transfer_fee`. The least this
/// canister will SWEEP from a published deposit address, and a different number
/// from [`ADVERTISED_MINIMUM_DEPOSIT`] for the reason in this file's header.
const MIN_EXTERNAL_DEPOSIT: u64 = WITHDRAWAL_FLOOR + FEE;

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

// ===========================================================================
// 7. THE PUBLISHED DEPOSIT ADDRESS -- the door none of the six tests above open
// ===========================================================================

/// What actually happened to one amount sent to a published deposit address.
///
/// Every field except the two `told_*` strings is read from the LEDGER or from an
/// escrow row, never inferred from a reply. `returned_to_wallet` in particular is
/// the only field that answers "did the player get their money back", and it is
/// the ledger's answer: the standing lesson of this repository is that a
/// conservation check measured against the canister's own figures is structurally
/// blind, and "the money is recoverable" is exactly the claim an internal figure
/// cannot support.
#[derive(Debug)]
struct BandOutcome {
    sent: u64,
    /// What `claim_external_deposit()` said, verbatim.
    told_by_claim: Result<u64, String>,
    /// What `refund_external_deposit()` said, verbatim. `None` when the claim
    /// already took the money, so there was nothing to ask about.
    told_by_refund: Option<Result<u64, String>>,
    /// e8s that arrived in the player's OWN ledger wallet, counted from after the
    /// send. **This is "recoverable", and nothing else is.**
    returned_to_wallet: u64,
    /// e8s the LEDGER still shows at the deposit address afterwards.
    still_at_the_address: u64,
    /// The escrow row at the end.
    escrow: u64,
}

/// Send `sent` to the player's own published deposit address, then try every door
/// out of it that a player has, and report what the ledger says happened.
///
/// The order is the player's order: sweep it into the table if the table will
/// take it, otherwise ask for it back, otherwise withdraw whatever the sweep
/// credited. No controller call and no admin call appears here.
fn every_door_out_of_the_deposit_address(
    world: &mut World,
    who: candid::Principal,
    sent: u64,
) -> BandOutcome {
    world
        .transfer_to_deposit_subaccount(who, sent)
        .unwrap_or_else(|e| panic!("the ledger refused a transfer of {sent} to the address: {e}"));

    // From here, not from before the send: the send itself costs the player a
    // ledger fee out of their wallet, and that fee is not what is under test.
    let wallet_after_send = world.ledger_balance(who, None);

    let told_by_claim = world
        .claim_external_deposit(who)
        .map_err(|e| format!("{e:?}"));

    let told_by_refund = if told_by_claim.is_ok() {
        None
    } else {
        Some(
            world
                .refund_external_deposit(who)
                .map_err(|e| format!("{e:?}")),
        )
    };

    // Whatever the sweep did credit has to come out too, or "recoverable" is
    // being claimed for money that only moved between two accounts this canister
    // owns.
    if let Ok(credited) = &told_by_claim {
        if *credited > 0 {
            let _ = world.withdraw(who, *credited);
        }
    }

    BandOutcome {
        sent,
        told_by_claim,
        told_by_refund,
        returned_to_wallet: world
            .ledger_balance(who, None)
            .saturating_sub(wallet_after_send),
        still_at_the_address: world
            .ledger_balance(world.table, Some(ledger::deposit_subaccount(&who))),
        escrow: world.get_balance(who),
    }
}

/// **THE GATE FINDING 31 NEVER HAD.** Every amount in and around the dead band,
/// sent to the address the canister publishes, driven to whatever end a player can
/// reach, and measured at the player's wallet.
///
/// The rule being enforced is one sentence: **money above the ledger's own fee,
/// sent to an address this canister published, comes back.** Not "is credited",
/// not "is reported" -- comes back, to the wallet, on the ledger. Below the fee it
/// cannot come back and the canister has to say so instead of pretending.
///
/// The six amounts are the boundary and one e8 either side of it, twice over: the
/// ledger fee (`10_000`), where physics takes over, and the advertised
/// `deposit()` minimum (`20_000`), which is the number the product prints and the
/// number the fifth auditor lost in full.
#[test]
fn every_amount_sent_to_the_published_deposit_address_comes_back_or_is_impossible() {
    let names = [
        "under-fee", "at-fee", "over-fee", "under-advertised", "advertised", "over-advertised",
        "at-minimum",
    ];
    let mut world = World::new(TableConfig::heads_up_icp(), &names);

    let band: [(u64, &str); 7] = [
        (FEE - 1, "under-fee"),
        (FEE, "at-fee"),
        (FEE + 1, "over-fee"),
        (ADVERTISED_MINIMUM_DEPOSIT - 1, "under-advertised"),
        (ADVERTISED_MINIMUM_DEPOSIT, "advertised"),
        (ADVERTISED_MINIMUM_DEPOSIT + 1, "over-advertised"),
        (MIN_EXTERNAL_DEPOSIT, "at-minimum"),
    ];

    let mut report: Vec<String> = Vec::new();
    for (sent, name) in band {
        let who = world.actor(name);
        let out = every_door_out_of_the_deposit_address(&mut world, who, sent);
        report.push(format!(
            "sent {:>6} -> claim {:?} / refund {:?} -> wallet +{} , address {} , escrow {}",
            out.sent,
            out.told_by_claim.as_ref().map(|n| *n),
            out.told_by_refund
                .as_ref()
                .map(|r| r.as_ref().map(|n| *n).map_err(|_| "REFUSED")),
            out.returned_to_wallet,
            out.still_at_the_address,
            out.escrow,
        ));

        if sent <= FEE {
            // ------ PHYSICS. No canister can move this, and the honest answer
            //        is to say so rather than to report a healthy balance.
            assert!(
                out.told_by_claim.is_err(),
                "{sent} e8s is at or below the {FEE} e8 ledger fee and must NOT be swept: \
                 sweeping it would cost more than the amount"
            );
            let refund = out
                .told_by_refund
                .as_ref()
                .expect("the claim refused, so the refund door must have been tried");
            assert!(
                refund.is_err(),
                "{sent} e8s cannot be sent anywhere by anybody, so a refund must be refused \
                 rather than attempted: {refund:?}"
            );
            let text = format!("{refund:?}");
            assert!(
                text.contains("ledger charges") && text.contains(&FEE.to_string()),
                "THE REFUSAL MUST NAME THE LEDGER FEE AS THE REASON. A player told only 'no' \
                 concludes the canister has taken their money; a player told 'the fee is larger \
                 than the amount' knows it is arithmetic and not a policy: {text}"
            );
            assert_eq!(
                out.returned_to_wallet, 0,
                "nothing can come back from {sent} e8s, and any test that says otherwise is \
                 measuring something that did not happen"
            );
            assert_eq!(
                out.still_at_the_address, sent,
                "and the money must still be AT THE ADDRESS, in full: a refusal that also \
                 spends the money is the worst of both"
            );
            assert_eq!(out.escrow, 0, "nothing may be credited either");
        } else if sent < MIN_EXTERNAL_DEPOSIT {
            // ------ THE DEAD BAND. The ledger can move it, the sweep will not
            //        take it, and it has to come home.
            assert!(
                out.told_by_claim.is_err(),
                "{sent} e8s sweeps to {} in escrow, which is under the {WITHDRAWAL_FLOOR} \
                 withdrawal floor, so the claim must refuse rather than take money it cannot \
                 pay back",
                sent - FEE
            );
            let claim_text = format!("{:?}", out.told_by_claim);
            assert!(
                claim_text.contains("refund_external_deposit"),
                "A REFUSAL IS NOT A REMEDY. The refusal must name the door that gets the money \
                 back, not only tell the player to send more -- 'send more' asks somebody to \
                 spend a second ledger fee to rescue the first: {claim_text}"
            );
            let refund = out
                .told_by_refund
                .as_ref()
                .expect("the claim refused, so the refund door must have been tried");
            assert!(
                refund.is_ok(),
                "{sent} e8s is above the {FEE} e8 ledger fee, so the LEDGER can move it and \
                 this canister must send it back. This is docs/SECURITY-FINDINGS.md FINDING 31: \
                 the money is at an address this canister published, it is nobody's but the \
                 sender's, and refusing to sweep it is not the same as returning it: {refund:?}"
            );
            assert_eq!(
                out.returned_to_wallet,
                sent - FEE,
                "the whole balance less ONE ledger fee must arrive in the player's own wallet. \
                 One fee, not two: the money never entered escrow, so it never had to be \
                 withdrawn out again. ClearDeck keeps none of it"
            );
            assert_eq!(
                out.still_at_the_address, 0,
                "and the address must be empty afterwards -- money left at an address the \
                 canister published is money it is holding"
            );
            assert_eq!(
                out.escrow, 0,
                "a refund must not touch escrow in either direction: it is not a deposit and \
                 not a withdrawal"
            );
        } else {
            // ------ AT AND ABOVE THE PUBLISHED MINIMUM. The ordinary path.
            let credited = out
                .told_by_claim
                .as_ref()
                .copied()
                .unwrap_or_else(|e| panic!("{sent} e8s is the published minimum for this address \
                     and the sweep must accept it: {e}"));
            assert_eq!(
                credited,
                sent - FEE,
                "the sweep credits what survives the ledger fee, and nothing else is taken"
            );
            assert_eq!(
                out.returned_to_wallet,
                sent - 2 * FEE,
                "and it withdraws again: the round trip through the address costs exactly two \
                 ledger fees -- the sweep's and the withdrawal's -- and no principal"
            );
            assert_eq!(out.still_at_the_address, 0);
            assert_eq!(out.escrow, 0);
        }
    }

    eprintln!(
        "the published deposit address, every amount in the band:\n  {}",
        report.join("\n  ")
    );
}

/// The single number this finding is about, on its own, so a failure names it.
///
/// 20,000 e8s is `ICP_MIN_DEPOSIT_AMOUNT` -- what `deposit()` accepts, what
/// `DepositModal.svelte` prints on its approve branch, and what test 1 above
/// proves comes back through THAT door. Sent to the ADDRESS instead it was a 100%
/// loss: swept to exactly 10,000 in escrow, which equals the ledger fee, which
/// `withdraw` can never pay out. The fifth auditor lost it by following the
/// product's own instructions.
#[test]
fn the_advertised_minimum_sent_to_the_published_address_is_not_a_total_loss() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice"]);
    let alice = world.actor("alice");

    let out = every_door_out_of_the_deposit_address(&mut world, alice, ADVERTISED_MINIMUM_DEPOSIT);

    assert_eq!(
        out.returned_to_wallet,
        ADVERTISED_MINIMUM_DEPOSIT - FEE,
        "THE FIFTH AUDITOR'S EXACT DEPOSIT. {} e8s went to the address this canister published \
         and {} came back to their wallet. If this is 0, FINDING 31 is live again and the \
         number on the deposit screen is the largest fully unrecoverable deposit this product \
         accepts. claim said: {:?}. refund said: {:?}",
        ADVERTISED_MINIMUM_DEPOSIT,
        out.returned_to_wallet,
        out.told_by_claim,
        out.told_by_refund,
    );
    assert_eq!(out.still_at_the_address, 0);
}

/// **DELIVERABLE 4, DRIVEN.** `sweepable` must not claim reachable what the sweep
/// refuses.
///
/// The flag's own documentation says *"True when `claim_external_deposit()` would
/// sweep this amount right now"*, and it was `observed_amount > transfer_fee`,
/// which stopped being that sentence the moment the Rule-3 floor was added. The
/// cost was not cosmetic: the money-safety drain prints this flag beside every
/// balance it cannot move, so the transcript that convicted the canister of
/// stranding 20,002 e8s carried `sweepable=true` on the same line
/// (docs/DEFECTS.md E-89).
///
/// This test asks the canister the question and then performs the action, at
/// every boundary, and requires the two to agree.
#[test]
fn sweepable_and_refundable_predict_what_the_two_doors_actually_do() {
    let names = ["a", "b", "c", "d"];
    let mut world = World::new(TableConfig::heads_up_icp(), &names);

    for (sent, name) in [
        (FEE, "a"),
        (FEE + 1, "b"),
        (MIN_EXTERNAL_DEPOSIT - 1, "c"),
        (MIN_EXTERNAL_DEPOSIT, "d"),
    ] {
        let who = world.actor(name);
        world
            .transfer_to_deposit_subaccount(who, sent)
            .expect("send to the published address");
        let custody = world
            .refresh_deposit_custody(who)
            .expect("the canister reads its own address");

        assert_eq!(
            custody.observed_amount, sent,
            "the canister must see what the ledger holds"
        );
        assert_eq!(
            custody.minimum_deposit, MIN_EXTERNAL_DEPOSIT,
            "the canister has to PUBLISH the minimum it applies to this address, or a client \
             can only print the deposit() floor beside it -- which is the number that cost the \
             fifth auditor a whole deposit"
        );

        // The claim, performed. This is the fact `sweepable` is a claim about.
        let swept = world.claim_external_deposit(who).is_ok();
        assert_eq!(
            custody.sweepable, swept,
            "sweepable said {} for {sent} e8s and claim_external_deposit() {}. A flag that \
             says money is reachable through a door that will not open is worse than no flag: \
             it is what the drain transcript quotes back as evidence",
            custody.sweepable,
            if swept { "took it" } else { "refused" }
        );

        if !swept {
            let refunded = world.refund_external_deposit(who).is_ok();
            assert_eq!(
                custody.refundable, refunded,
                "refundable said {} for {sent} e8s and refund_external_deposit() {}",
                custody.refundable,
                if refunded { "sent it back" } else { "refused" }
            );
        }
    }
}

/// **BEFORE THEY SEND, not after.** The address surface has to state the minimum
/// and the dust rule to somebody who has sent nothing yet.
///
/// Every other sentence this canister has about a deposit address is a
/// post-mortem: it is produced from an observation, and there is no observation
/// until money has already moved. A player looking at the address they were given
/// is in exactly the state where a number would still help them.
#[test]
fn the_address_surface_states_the_minimum_and_the_dust_rule_before_anything_is_sent() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice"]);
    let alice = world.actor("alice");

    let custody = world.deposit_custody(alice);
    assert_eq!(
        custody.observed_at_ns, None,
        "the state under test is an address nobody has looked at and nothing has been sent to"
    );
    assert_eq!(
        custody.minimum_deposit, MIN_EXTERNAL_DEPOSIT,
        "the published minimum for THIS route must be min_withdrawal + fee. Publishing the \
         deposit() floor here is FINDING 31: it is one e8 inside the band that cannot be \
         recovered by a sweep at all"
    );
    assert!(
        custody.minimum_deposit >= WITHDRAWAL_FLOOR + FEE,
        "a published minimum below min_withdrawal + fee is a number that strands the player \
         who follows it exactly"
    );
    assert!(
        custody.note.contains(&MIN_EXTERNAL_DEPOSIT.to_string()),
        "the note has to carry the figure, not only the concept: {}",
        custody.note
    );
    assert!(
        custody.note.contains("unrecoverable dust"),
        "AND IT HAS TO SAY THE UGLY HALF. At or below the ledger fee the money cannot be \
         moved by this canister or by anybody, and the player can only act on that before \
         they send: {}",
        custody.note
    );
}

/// The top-up the canister names must be a top-up that WORKS.
///
/// FINDING 11's remedy for sub-fee dust is "send more to the same address", and
/// the canister prints the exact figure. That figure was `fee + 1 - amount` --
/// correct for as long as the sweep took anything the ledger could move, and
/// wrong from the moment the Rule-3 floor was added, because at `fee + 1` the
/// claim refuses for a different reason with a different message. An instruction
/// the canister will not honour costs the player a second ledger fee and a second
/// refusal.
///
/// So this test does not read the sentence. It extracts the number, sends exactly
/// that, and requires the claim to succeed.
#[test]
fn the_top_up_the_dust_refusal_names_actually_makes_the_balance_claimable() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice"]);
    let alice = world.actor("alice");
    let dust = FEE - 1;

    world
        .transfer_to_deposit_subaccount(alice, dust)
        .expect("send sub-fee dust to the published address");
    let refusal = format!(
        "{:?}",
        world
            .claim_external_deposit(alice)
            .expect_err("sub-fee dust cannot be swept")
    );

    // "sending N e8s or more to the SAME address" -- take N from the canister's
    // own words rather than recomputing it here, because recomputing it is how a
    // test agrees with a bug.
    let named = refusal
        .split(" e8s or more")
        .next()
        .and_then(|before| before.split_whitespace().next_back().map(str::to_string))
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or_else(|| panic!("the dust refusal must name a top-up in e8s: {refusal}"));

    world
        .transfer_to_deposit_subaccount(alice, named)
        .expect("send exactly what the canister asked for");
    let credited = world.claim_external_deposit(alice).unwrap_or_else(|e| {
        panic!(
            "THE CANISTER'S OWN INSTRUCTION DID NOT WORK. It said to send {named} e8s more to \
             the same address; that was done, the address now holds {}, and the claim still \
             refused: {e:?}",
            dust + named
        )
    });
    assert_eq!(
        credited,
        dust + named - FEE,
        "and the dust comes out WITH the top-up: it was never lost, only immovable alone"
    );
}

/// The refund can only ever pay the caller, and can only ever drain the caller's
/// own address.
///
/// Neither the source nor the destination is a parameter, so there is nothing to
/// substitute -- but "there is no parameter" is an argument, and this project has
/// lost a wave to an argument standing in for a measurement. Driven: bob asks for
/// a refund while alice's address is the one holding money.
#[test]
fn a_refund_reaches_no_address_but_the_callers_own() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    world
        .transfer_to_deposit_subaccount(alice, ADVERTISED_MINIMUM_DEPOSIT)
        .expect("alice funds her own deposit address");
    world
        .refresh_deposit_custody(alice)
        .expect("the canister reads alice's address");

    let bob_wallet_before = world.ledger_balance(bob, None);
    world
        .refund_external_deposit(bob)
        .expect_err("bob's own deposit address is empty, so there is nothing to refund to him");
    assert_eq!(
        world.ledger_balance(bob, None),
        bob_wallet_before,
        "and bob's wallet must not have moved"
    );
    assert_eq!(
        world.ledger_balance(world.table, Some(ledger::deposit_subaccount(&alice))),
        ADVERTISED_MINIMUM_DEPOSIT,
        "ALICE'S MONEY MUST STILL BE ALICE'S. A refund that reads the caller and writes \
         somebody else's account is the shape of every completed theft in this project"
    );

    // And alice's own refund still works afterwards.
    let alice_before = world.ledger_balance(alice, None);
    world
        .refund_external_deposit(alice)
        .expect("alice can always get her own money back");
    assert_eq!(
        world.ledger_balance(alice, None) - alice_before,
        ADVERTISED_MINIMUM_DEPOSIT - FEE
    );
}

// ===========================================================================
// 8. THE DRAIN'S OWN TOLERANCE -- E-89's second defect, which is in the instrument
// ===========================================================================
//
// These four are pure functions over a hand-built `DrainReport`: no replica, no
// wasm, milliseconds. They live in THIS file rather than beside the code they test
// because `cargo test --lib` in this crate is invoked by no dev.sh target and no
// CI job, and a gate nothing runs is this project's most-repeated defect
// (docs/DEFECTS.md H-45, H-17). `deposit_floor` is named explicitly by
// `./scripts/dev.sh test`, and the question is the same question: a floor, and the
// money it decides to excuse.

use money_safety::invariants::{reachability, Severity};
use std::collections::BTreeMap;

fn principal(n: u8) -> candid::Principal {
    candid::Principal::self_authenticating([n])
}

/// A finished drain that left `escrow` and `deposits` behind and nothing else.
fn drain_that_left(
    escrow: &[(u8, u64)],
    deposits: &[(u8, u64)],
    chips: u64,
    pot: u64,
) -> reachability::DrainReport {
    let escrow_after: BTreeMap<_, _> = escrow.iter().map(|(p, v)| (principal(*p), *v)).collect();
    let deposit_custody_after: BTreeMap<_, _> =
        deposits.iter().map(|(p, v)| (principal(*p), *v)).collect();
    let owed = escrow_after.values().sum::<u64>()
        + deposit_custody_after.values().sum::<u64>()
        + chips
        + pot;
    reachability::DrainReport {
        owed_before: owed,
        owed_after: owed,
        // The ledger holds exactly what the canister says it owes, so the
        // ORPHAN leg is silent and only the stranded leg can speak. That
        // separation is the point: these tests are about one of the two.
        ledger_main_after: escrow_after.values().sum::<u64>() + chips + pot,
        ledger_deposit_subaccounts_after: deposit_custody_after.values().sum::<u64>(),
        uncredited_raw: 0,
        returned_to_wallets: BTreeMap::new(),
        escrow_after,
        deposit_custody_after,
        chips_after: chips,
        pot_after: pot,
        log: vec!["(synthetic)".to_string()],
    }
}

fn stranded_verdict(report: &reachability::DrainReport) -> Option<i128> {
    reachability::check_drain(report, &GamePhase::WaitingForPlayers)
        .into_iter()
        .find(|v| v.check == "money_left_behind_after_drain")
        .map(|v| {
            assert_eq!(v.severity, Severity::FundsUnreachable);
            v.delta_e8s
        })
}

/// **ONE STRANDED PLAYER IS A FINDING.** The old tolerance was a single
/// `owed_after > 20_000`, so a lone player left holding 19,999 e8s of unreachable
/// money passed in silence -- and 19,999 is inside the exact band FINDING 31 is
/// about.
#[test]
fn one_player_stranded_under_the_old_aggregate_tolerance_is_now_convicted() {
    let report = drain_that_left(&[], &[(1, 19_999)], 0, 0);
    assert_eq!(
        stranded_verdict(&report),
        Some(19_999),
        "19,999 e8s at one published deposit address is money the ledger can move and the \
         player cannot reach. The old check compared this against a 20,000 e8 AGGREGATE \
         tolerance and said nothing"
    );
}

/// **AND THE VERDICT MUST NOT DEPEND ON SEAT COUNT.** The same per-account amount
/// at nine addresses is nine times the loss, not nine times more acceptable -- and
/// under the old rule the first one was free.
#[test]
fn the_drain_tolerance_does_not_scale_with_the_number_of_seats() {
    let nine: Vec<(u8, u64)> = (1..=9u8).map(|i| (i, 19_999)).collect();
    assert_eq!(
        stranded_verdict(&drain_that_left(&[], &nine, 0, 0)),
        Some(9 * 19_999),
        "every stranded account counts in full"
    );

    // The E-89 reproducer's own shape: two players, 10,001 each. The old rule
    // convicted this at 20,002 while tolerating ONE player at 19,999 -- the
    // threshold's answer depended on how the same money was distributed.
    assert_eq!(
        stranded_verdict(&drain_that_left(&[], &[(1, 10_001), (2, 10_001)], 0, 0)),
        Some(20_002)
    );

    // AND THE SIZE IT REPORTS HAS TO BE THE RECOVERABLE PART, not the total owed.
    // Two accounts at exactly the fee are immovable; one at 15,000 is not. The
    // aggregate rule fired on the sum -- 35,000 -- so it named 20,000 e8s of
    // physics as unreachable money and buried the 15,000 that actually was.
    assert_eq!(
        stranded_verdict(&drain_that_left(&[], &[(1, FEE), (2, FEE), (3, 15_000)], 0, 0)),
        Some(15_000),
        "the delta must be the money somebody could have been paid, so the number in the \
         report is the number an operator has to go and find"
    );
}

/// **THE EXCUSE IS PHYSICS AND NOTHING ELSE.** At or below the ledger's own
/// transfer fee no canister can deliver anything, at any number of accounts, so
/// that and only that is silent (docs/SECURITY-FINDINGS.md FINDING 11).
#[test]
fn only_amounts_the_ledger_itself_cannot_move_are_excused() {
    // Three accounts at exactly the fee: 30,000 e8s owed, all of it immovable.
    assert_eq!(
        stranded_verdict(&drain_that_left(
            &[(1, FEE)],
            &[(2, FEE), (3, FEE)],
            0,
            0
        )),
        None,
        "a balance at the ledger fee cannot produce a delivery: sending it costs exactly what \
         it is. Excusing it is arithmetic, not indulgence"
    );

    // One e8 more, in one account, and it is reachable and therefore a finding.
    assert_eq!(
        stranded_verdict(&drain_that_left(&[(1, FEE)], &[(2, FEE + 1), (3, FEE)], 0, 0)),
        Some(i128::from(FEE) + 1),
        "and the boundary is exact: one e8 above the fee is money that CAN be sent home"
    );
}

/// Chips and the pot have no ledger-fee argument at all: `cash_out` turns a stack
/// into escrow without a transfer. A single chip left in a seat after a full drain
/// is a hand that never ended, which is FINDING 15's shape.
#[test]
fn chips_and_the_pot_are_never_excused_at_any_size() {
    assert_eq!(stranded_verdict(&drain_that_left(&[], &[], 1, 0)), Some(1));
    assert_eq!(stranded_verdict(&drain_that_left(&[], &[], 0, 1)), Some(1));
    assert_eq!(
        stranded_verdict(&drain_that_left(&[], &[], 0, 0)),
        None,
        "and an empty table is not a finding"
    );
}

// ===========================================================================
// 9. THE SENTENCE THE STRANDED PLAYER READS  (docs/DEFECTS.md E-81)
// ===========================================================================

/// A refusal a player cannot act on must not look like one they can.
///
/// The whole-balance waiver's refusal read: *"Minimum withdrawal is 0.0002 ICP.
/// Your whole remaining balance can always be withdrawn in one call whatever its
/// size, as long as it is more than the 0.0001 ICP network fee -- you have 0.0001
/// ICP."* Every clause of that is true and the paragraph is unusable: it leads
/// with a policy number, promises a universal guarantee, and then quietly excludes
/// this exact balance in a subordinate clause. It is the message shown to the
/// player whom [FINDING 31](docs/SECURITY-FINDINGS.md) stranded, and the fifth
/// auditor's note on it was that a reader concludes they made a formatting mistake
/// and retries. There is no retry, no amount and no later.
///
/// So: when the balance ITSELF is at or below the ledger fee, the refusal has to
/// say that nothing can move it and why, and must not open with a policy minimum
/// that suggests otherwise.
#[test]
fn a_balance_at_the_ledger_fee_is_refused_as_arithmetic_and_not_as_a_policy_minimum() {
    let (world, alice) = lone_depositor("alice");
    world.fund_escrow(alice, 30_000).expect("deposit");
    world.withdraw(alice, 20_000).expect("a withdrawal at the floor");
    past_the_cooldown(&world);

    let residue = world.get_balance(alice);
    assert_eq!(residue, FEE, "the state under test is a balance equal to the ledger fee");

    let text = format!(
        "{:?}",
        world
            .withdraw(alice, residue)
            .expect_err("the ledger cannot move an amount equal to its own fee")
    );

    assert!(
        !text.contains("can always be withdrawn"),
        "A UNIVERSAL GUARANTEE MUST NOT BE STATED TO THE ONE PLAYER IT EXCLUDES. This refusal \
         is shown to somebody whose balance can never leave; telling them the whole balance can \
         always be withdrawn is what makes them retry: {text}"
    );
    assert!(
        !text.contains("Minimum withdrawal is"),
        "AND IT MUST NOT LEAD WITH A POLICY NUMBER. A policy minimum is something a player can \
         imagine clearing. This one cannot be cleared by any request: {text}"
    );
    assert!(
        text.contains("network fee"),
        "the reason is the LEDGER's fee and it has to be named: {text}"
    );
    assert!(
        text.contains("No withdrawal of any size"),
        "and it has to say plainly that nothing moves this -- not the player, not the table, \
         not a controller: {text}"
    );
}
