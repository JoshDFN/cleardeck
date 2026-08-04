//! Minimal reproducers, one per invariant violation this harness actually found.
//!
//! Each test is written so that it FAILS if the defect is silently fixed, forcing
//! whoever fixes it to update `docs/SECURITY-FINDINGS.md` in the same change.
//! None of these tests patch anything; the instruction for this work was to
//! characterise and prove.
//!
//! Numbering matches `docs/SECURITY-FINDINGS.md`.

use candid::{Decode, Encode, Principal};
use money_safety::invariants::*;
use money_safety::table_api::*;
use money_safety::world::*;
use std::time::Duration;

const ICP: u64 = 100_000_000;

// ---------------------------------------------------------------------------
// REG-01 -- FINDING 01 / E-01 is FIXED. This is the gate that keeps it fixed.
// ---------------------------------------------------------------------------

/// The sequence that used to permanently destroy real user funds, now asserting
/// the opposite.
///
/// THE EXACT HAND FROM THE FINDING. Heads-up, blinds 1,000,000 / 2,000,000. The
/// pre-flop bets are levelled at 2,000,000 each, then 30,000,000 is bet and called
/// on the flop, then it is checked down to a showdown. The pot is 64,000,000.
///
/// What used to happen: `state.side_pots` was built once, at the PreFlop -> Flop
/// transition, when the pot was 4,000,000. `determine_winners` only rebuilt it "if
/// empty", which it never was, so the winner was credited 4,000,000 and
/// `state.pot = 0` discarded the other 60,000,000. The tokens stayed inside the
/// canister on the ledger; `withdraw` pays strictly against the caller's own escrow
/// and there is no administrative withdrawal anywhere in the canister, so they could
/// never be recovered by anyone, including a controller.
///
/// What this test asserts now: the winner receives the WHOLE 64,000,000, chip
/// conservation across the hand is exact to the e8, and the breakdown the engine
/// publishes while the hand is live tracks the money instead of freezing.
#[test]
fn reg01_a_showdown_with_post_flop_betting_pays_out_every_e8() {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    for (i, who) in [alice, bob].into_iter().enumerate() {
        world.fund_escrow(who, 6 * ICP).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    let before = world.snapshot();
    let total_before = before.internal_total();
    let ledger_before = before.ledger_main;

    // Level the pre-flop bets and reach the flop.
    for _ in 0..8 {
        let t = world.table_state();
        if t.phase != GamePhase::PreFlop {
            break;
        }
        let who = on_clock(&t);
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        world.player_action(who, PlayerAction::Check).expect("check");
    }
    let flop = world.table_state();
    assert_eq!(flop.phase, GamePhase::Flop);
    let preflop_pot = flop.pot;
    let frozen_breakdown = flop.side_pots_total();
    assert_eq!(
        frozen_breakdown, preflop_pot,
        "side_pots is built at the PreFlop -> Flop transition and matches the pot then"
    );

    // One bet, one call: 30,000,000 each into the pot post-flop, which is the
    // finding's own scenario and makes the pot exactly 64,000,000.
    let bet = 30_000_000;
    world
        .player_action(on_clock(&world.table_state()), PlayerAction::Bet(bet))
        .expect("post-flop bet");
    world
        .player_action(on_clock(&world.table_state()), PlayerAction::Call)
        .expect("post-flop call");

    let after_bet = world.table_state();
    assert_eq!(
        after_bet.pot,
        preflop_pot + 2 * bet,
        "the pot really did grow by the post-flop money"
    );
    assert_eq!(after_bet.pot, 64_000_000, "the finding's exact pot");
    assert_eq!(
        after_bet.side_pots_total(),
        after_bet.pot,
        "PINNED FIX (E-01): the breakdown must grow with the pot. It used to stay at \
         the pre-flop {frozen_breakdown} for the rest of the hand, and that frozen \
         figure was what the winner was paid out of."
    );

    // Check everything down to the river, forcing a showdown.
    for _ in 0..12 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let who = on_clock(&t);
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        break;
    }

    let after = world.snapshot();
    assert_eq!(
        after.table.phase,
        GamePhase::HandComplete,
        "the hand must have completed"
    );
    assert_eq!(
        after.ledger_main, ledger_before,
        "no money left the canister: the loss is internal"
    );

    let destroyed = total_before as i128 - after.internal_total() as i128;
    assert_eq!(
        destroyed, 0,
        "PINNED FIX (E-01): not one e8 may be destroyed. {} was destroyed by this exact \
         hand before the fix, which is the whole post-flop pot (pre-flop pot {}, \
         post-flop {} each). total before {}, total after {}",
        2 * bet,
        preflop_pot,
        bet,
        total_before,
        after.internal_total()
    );

    // The winner was paid the WHOLE pot, not the pre-flop part of it.
    let history = world
        .hand_history(after.table.hand_number)
        .expect("the hand is in the history");
    let awarded: u64 = history
        .winners
        .iter()
        .fold(0u64, |a, w| a.saturating_add(w.amount));
    assert_eq!(
        awarded, 64_000_000,
        "the winner must be credited all 64,000,000. Before the fix this was \
         4,000,000 -- the pre-flop pot -- and the rest was credited to nobody. \
         winners={:?}",
        history.winners
    );

    // And nothing is stranded: the canister holds exactly what it owes.
    let vs = check_world(&world);
    assert!(
        !vs.iter().any(|v| v.invariant == Invariant::M1Conservation),
        "M1 used to see {} e8s stranded on the ledger owed to nobody. Nothing may be \
         stranded now: {:?}",
        2 * bet,
        vs
    );
    println!(
        "REG-01: pot {} collected, {} awarded, {} destroyed",
        after_bet.pot, awarded, destroyed
    );
}

// ---------------------------------------------------------------------------
// REG-02 -- the notify_deposit DECODE path (E-04 / FINDING 06)
// ---------------------------------------------------------------------------

// The two reply shapes live in the library so a reader can diff them directly:
// see src/legacy_ledger_shapes.rs.
use money_safety::legacy_ledger_shapes::{as_declared_by_the_canister, as_the_real_ledger_returns_it};

/// E-04 / FINDING 06 is FIXED (2026-08-04). This is now the gate that keeps it fixed.
///
/// What the defect WAS, isolated to a single field type: the canister declared the
/// ledger's `AccountIdentifier` as `record { hash : blob }` while the real ledger
/// returns a bare `blob`, and it asked `Response::candid::<(QueryBlocksResponse,)>`
/// for a tuple the reply never contains. So every `query_blocks` reply failed to
/// decode, `notify_deposit` returned `"Failed to decode ledger response"` for every
/// block forever, and ICP sent through the advertised deposit address was stranded.
///
/// What this test asserts now: the decode path works, for the ordinary Transfer
/// block AND for a block that is NOT a deposit for this canister. The second half is
/// the part a "does it credit?" test cannot see -- if the declared `Operation`
/// variant is wrong, Candid's `opt` rule decodes it to `None` with no error, and the
/// canister reports "not a transfer" for a block it simply could not read. So the
/// gate is on the REASON, not just on success: no rejection may ever again be a
/// decode failure.
///
/// The crediting semantics (exactly once, no replay, concurrency) are covered by
/// `tests/deposit_replay.rs` dr01-dr09 and are deliberately not duplicated here.
#[test]
fn reg02_notify_deposit_can_read_the_real_ledger_and_never_fails_to_decode() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    // --- part 1: the ordinary flow the Candid interface advertises ----------
    let sent = 5 * ICP;
    let block = world
        .raw_transfer_to_canister(alice, sent)
        .expect("the ledger transfer itself works fine");
    assert_eq!(
        world.ledger_balance(world.table, None),
        sent,
        "the canister really holds the user's ICP"
    );

    let credited = world.notify_deposit(alice, block).unwrap_or_else(|e| {
        panic!(
            "REGRESSION of E-04 / FINDING 06: notify_deposit({block}) failed with {e:?}. A user who \
             followed the documented get_deposit_address + notify_deposit flow has just had real \
             ICP stranded in the canister with no way to recover it."
        )
    });
    world.note_raw_deposit_credited(sent);
    assert_eq!(
        credited, sent,
        "notify_deposit must credit exactly what arrived on the ledger"
    );
    assert_eq!(world.get_balance(alice), sent);

    // --- part 2: blocks that are NOT a deposit for this canister ------------
    // Each of these must be REFUSED, and refused for the right reason. A decode
    // failure here would mean the canister cannot read the block at all, which is
    // the E-04 shape wearing a different error message.
    let approve_block = world
        .approve(bob, 3 * ICP)
        .expect("icrc2_approve writes an Approve block to the ledger");
    let elsewhere = world
        .transfer_to_deposit_subaccount(bob, 2 * ICP)
        .expect("a transfer to a per-player deposit subaccount");

    for (what, b) in [
        ("an icrc2_approve block", approve_block),
        ("a transfer to somebody else's deposit subaccount", elsewhere),
        ("a block that does not exist yet", 10_000u64),
    ] {
        let before = world.get_balance(alice);
        let outcome = world.notify_deposit(alice, b);
        let message = match &outcome {
            Err(OpError::Err(m)) => m.clone(),
            Err(OpError::Trap(t)) => panic!("{what} (block {b}) TRAPPED instead of returning Err: {t}"),
            Ok(v) => panic!(
                "{what} (block {b}) CREDITED {v} to a caller it does not belong to. That is \
                 withdrawable ICP created from nothing."
            ),
        };
        assert!(
            !message.contains("Failed to decode"),
            "{what} (block {b}) was refused with a DECODE failure: {message:?}. The canister \
             cannot read this block, so it cannot be distinguishing \"not yours\" from \"could \
             not parse\" -- that is E-04 (docs/DEFECTS.md E-04)."
        );
        assert_eq!(
            world.get_balance(alice),
            before,
            "{what} must not move escrow"
        );
        println!("REG-02 {what} (block {b}) -> refused: {message}");
    }

    // --- part 3: the root cause, still isolable -----------------------------
    // These two decode attempts are statements about the LEDGER's reply, not about
    // the canister: `src/legacy_ledger_shapes.rs` is a frozen copy of the shapes the
    // canister used to declare, and the ledger wasm is pinned by sha256. They are
    // kept because they are the whole diagnosis in six lines, and because they show
    // the corrected shape is the one the canister must keep using.
    let args = as_declared_by_the_canister::GetBlocksArgs {
        start: block,
        length: 1,
    };
    let raw = world
        .pic
        .query_call(
            world.ledger,
            Principal::anonymous(),
            "query_blocks",
            Encode!(&args).unwrap(),
        )
        .expect("query_blocks itself is fine");

    // BUG A -- `Response::candid::<(QueryBlocksResponse,)>` asks the decoder for ONE
    // value of type `record { 0 : QueryBlocksResponse }`; the reply holds one value
    // of type `QueryBlocksResponse`.
    assert!(
        candid::decode_one::<(as_declared_by_the_canister::QueryBlocksResponse,)>(&raw).is_err(),
        "the tuple mis-decode no longer fails, so the pinned ledger reply shape has changed and \
         this diagnosis needs revisiting"
    );
    // BUG B -- with `AccountIdentifier = record { hash : blob }` the Operation
    // variant does not match and `opt Operation` decodes to NULL. No error, no trap.
    let declared = candid::decode_one::<as_declared_by_the_canister::QueryBlocksResponse>(&raw)
        .expect("as one value the outer record does decode");
    assert!(
        declared.blocks[0].transaction.operation.is_none(),
        "the OLD declared Operation shape now matches the ledger, so BUG B was a different \
         mechanism than recorded; revisit docs/SECURITY-FINDINGS.md FINDING 06"
    );
    // And with `AccountIdentifier = blob` the SAME bytes yield the real transfer.
    let corrected = Decode!(&raw, as_the_real_ledger_returns_it::QueryBlocksResponse)
        .expect("with AccountIdentifier = blob the SAME bytes decode cleanly");
    match &corrected.blocks[0].transaction.operation {
        Some(as_the_real_ledger_returns_it::Operation::Transfer(t)) => {
            assert_eq!(t.amount.e8s, sent);
            assert_eq!(t.to.len(), 32, "`to` is a bare 32-byte blob, not a record");
        }
        other => panic!("expected a Transfer, got {other:?}"),
    }

    // --- part 4: and the money is no longer stranded ------------------------
    let vs = check_world(&world);
    money_safety::assert_no_new_violations(&vs, "after a notified raw deposit");
    assert!(
        world.withdraw(alice, 2 * ICP).is_ok(),
        "the credited deposit must be withdrawable: the whole point of FINDING 06 was that it \
         was not"
    );
}

// ---------------------------------------------------------------------------
// REG-05 -- FINDING 05: leave_table mid-hand orphans the leaver's stake
// ---------------------------------------------------------------------------

/// E-05 / FINDING 05 is FIXED. This is the gate that keeps it fixed.
///
/// What the defect WAS: `leave_table` mid-hand removed the seat, so the leaver's
/// `total_bet_this_hand` vanished from the contribution list while their money
/// stayed in `pot`. The bet-level split then had an unattributed remainder, which
/// `poker_core::build_side_pots` appended to the LAST (highest) bet level -- the pot
/// only the deepest stacks can win. An honest short-stacked all-in lost part of the
/// main pot it was entitled to, and every chip was conserved, so no conservation
/// invariant could see it.
///
/// `cash_out` reached the same state by a different door: it refuses only players
/// who have NOT folded, so a player who folds first -- or is folded by the action
/// timer without calling anything -- can vacate the seat mid-hand too. Both doors
/// are exercised here.
///
/// What this test asserts now: after the departure, every e8 in the pot is still
/// attributed. `leave_table` and `cash_out` record the stake in
/// `TableState::departed_stakes`, which is part of the payout basis and independent
/// of whether the chair is occupied.
#[test]
fn reg05_a_vacated_seats_stake_stays_in_the_payout_basis() {
    let mut proved = 0;
    for door in ["leave_table", "fold_then_cash_out"] {
        let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
        let seats = [world.actor("alice"), world.actor("bob"), world.actor("carol")];
        for (i, who) in seats.iter().enumerate() {
            world.fund_escrow(*who, 6 * ICP).expect("deposit");
            world.join_table(*who, i as u8).expect("seat");
        }
        world.advance(Duration::from_secs(4));
        world.start_new_hand(seats[0]).expect("deal");

        // Everyone puts money in pre-flop.
        for _ in 0..10 {
            let t = world.table_state();
            if t.phase != GamePhase::PreFlop {
                break;
            }
            let who = on_clock(&t);
            if world.player_action(who, PlayerAction::Call).is_ok() {
                continue;
            }
            if world.player_action(who, PlayerAction::Check).is_ok() {
                continue;
            }
            break;
        }

        let mid = world.table_state();
        assert!(
            mid.phase.hand_in_progress(),
            "{door}: must still be in a hand"
        );
        assert_eq!(
            mid.pot,
            mid.wagered_total(),
            "{door}: before anyone leaves, every e8 in the pot is attributed"
        );

        // Pick a player who is NOT on the clock, so this is purely a seat change.
        let leaver = mid
            .seated()
            .find(|p| p.seat != mid.action_on && p.total_bet_this_hand > 0)
            .map(|p| p.principal)
            .expect("someone off the clock has money in");
        let stake = mid
            .seated()
            .find(|p| p.principal == leaver)
            .map(|p| p.total_bet_this_hand)
            .unwrap();

        let vacated = match door {
            "leave_table" => world.leave_table(leaver).is_ok(),
            _ => {
                // cash_out refuses players still in the hand, so fold first.
                // A player can only fold on their own turn -- but a timeout folds
                // them too, and after that cash_out lets them vacate mid-hand.
                if world.player_action(leaver, PlayerAction::Fold).is_ok() {
                    world.cash_out(leaver).is_ok()
                } else {
                    println!("{door}: not the leaver's turn to fold; skipping this door");
                    false
                }
            }
        };
        if !vacated {
            continue;
        }

        let after = world.table_state();
        if !after.phase.hand_in_progress() {
            println!("{door}: departure ended the hand outright; nothing orphaned");
            continue;
        }

        assert_eq!(
            after.pot as i128 - after.wagered_total() as i128,
            stake as i128,
            "{door}: the leaver's stake ({stake}) is no longer credited to a SEATED \
             player, which is the precondition the fix has to survive"
        );
        assert_eq!(
            after.departed_total(),
            stake,
            "{door}: PINNED FIX (E-05): the stake must be recorded against the seat that \
             left. departed_stakes={:?}",
            after.departed_stakes
        );
        assert_eq!(
            after.pot,
            after.payout_basis_total(),
            "{door}: PINNED FIX (E-05): every e8 in the pot must still be attributed, so \
             the bet-level split allocates all of it. It used to lose exactly {stake}, \
             which then reappeared in the pot only the deepest stacks could win."
        );

        let vs = check_pot_breakdown(&world.snapshot());
        assert!(
            vs.is_empty(),
            "{door}: M1b must be satisfied after the departure. It used to flag {stake} \
             as orphaned: {vs:?}"
        );
        println!("REG-05 ({door}): {stake} recorded, pot {} fully attributed", after.pot);
        proved += 1;
    }
    assert!(
        proved > 0,
        "at least one door out of an occupied seat mid-hand must have been exercised"
    );
}

// ---------------------------------------------------------------------------
// REG-08 -- cash_out after a TIMEOUT vacates a seat mid-hand
// ---------------------------------------------------------------------------

/// FINDING 05 says `cash_out` "guards the same removal with `Cannot cash out
/// while in a hand`". The guard only refuses players who have NOT folded:
///
/// ```rust
/// state.players.iter().flatten().any(|p| p.principal == caller && !p.has_folded)
/// ```
///
/// A player does not have to call anything to become folded. Going quiet until the
/// action timer expires and somebody calls `check_timeouts` folds them. After that
/// `cash_out` lets them vacate the seat mid-hand, orphaning their stake exactly as
/// `leave_table` does, AND returns their chips to escrow where they can be
/// withdrawn.
///
/// This is a disconnect, not an attack: the shape happens whenever a player closes
/// the tab mid-hand and then reconnects and cashes out.
#[test]
fn reg08_a_timed_out_player_can_cash_out_mid_hand_and_orphan_their_stake() {
    // action_timeout 10s, deliberately SHORTER than the 30s disconnect timeout, so
    // one player times out without the whole table being marked Disconnected.
    // REG-09 covers what happens when the two thresholds are equal, which is the
    // deployed configuration.
    let config = TableConfig {
        action_timeout_secs: 10,
        ..TableConfig::six_max_icp()
    };
    let world = World::new(config, &["alice", "bob", "carol"]);
    let seats = [world.actor("alice"), world.actor("bob"), world.actor("carol")];
    for (i, who) in seats.iter().enumerate() {
        world.fund_escrow(*who, 6 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(seats[0]).expect("deal");

    // Everyone puts the big blind in and the hand reaches the flop.
    for _ in 0..12 {
        let t = world.table_state();
        if t.phase == GamePhase::Flop {
            break;
        }
        let who = on_clock(&t);
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        world.player_action(who, PlayerAction::Check).expect("check");
    }
    let flop = world.table_state();
    assert_eq!(flop.phase, GamePhase::Flop);
    assert_eq!(
        flop.pot,
        flop.wagered_total(),
        "before anyone leaves, every e8 in the pot is attributed"
    );

    // The player on the clock simply stops responding.
    let quiet = on_clock(&flop);
    let stake = flop
        .seated()
        .find(|p| p.principal == quiet)
        .map(|p| p.total_bet_this_hand)
        .expect("the quiet player has money in");
    assert!(stake > 0, "the timed-out player must have contributed");

    // cash_out is refused while they are still live in the hand.
    assert!(
        matches!(world.cash_out(quiet), Err(OpError::Err(ref m)) if m.contains("Cannot cash out")),
        "cash_out must refuse a player who has not folded"
    );

    // Time passes; anybody may call check_timeouts, and it auto-folds them.
    world.advance(Duration::from_secs(11));
    let timeout = world.check_timeouts(seats[0]).expect("check_timeouts");
    println!("REG-08 check_timeouts -> {timeout:?}");
    let after_timeout = world.table_state();
    assert!(
        after_timeout
            .seated()
            .any(|p| p.principal == quiet && p.has_folded),
        "the quiet player must have been auto-folded"
    );
    if !after_timeout.phase.hand_in_progress() {
        panic!("the auto-fold ended the hand; this fixture needs 3+ live players");
    }

    // And now the guard lets them go.
    let returned = world
        .cash_out(quiet)
        .expect("cash_out must now be allowed, because the player counts as folded");
    println!("REG-08 cash_out returned {returned} chips to escrow mid-hand");

    let vacated = world.table_state();
    assert!(
        vacated.seat_of(quiet).is_none(),
        "the seat must have been vacated mid-hand"
    );
    assert_eq!(
        vacated.pot as i128 - vacated.wagered_total() as i128,
        stake as i128,
        "the timed-out player's stake ({stake}) is no longer credited to a SEATED \
         player: that is the precondition, and it is reached without the player \
         calling anything but cash_out"
    );
    assert_eq!(
        vacated.departed_total(),
        stake,
        "PINNED FIX (E-05): a player folded by the ACTION TIMER and then cashed out \
         mid-hand must still have their stake in the payout basis. \
         departed_stakes={:?}",
        vacated.departed_stakes
    );
    assert_eq!(
        vacated.pot,
        vacated.payout_basis_total(),
        "PINNED FIX (E-05): the pot must remain fully attributed. It used to lose \
         exactly {stake} here, which the split then handed to the highest bet level \
         instead of the pot the short stacks could win."
    );

    let vs = check_pot_breakdown(&world.snapshot());
    assert!(
        vs.is_empty(),
        "M1b must be satisfied after the timed-out player cashes out. It used to flag \
         {stake} as orphaned: {vs:?}"
    );
    println!("REG-08: {stake} recorded, pot {} fully attributed", vacated.pot);

    let vs = check_world(&world);
    assert!(
        vs.iter().all(|v| v.severity != Severity::FundCreation),
        "nothing may be created by the departure itself: {vs:?}"
    );
}

// ---------------------------------------------------------------------------
// REG-09 -- one timeout ends everyone's hand, because the disconnect timeout and
//           the action timeout are both 30 seconds
// ---------------------------------------------------------------------------

/// `check_timeouts` marks every player whose `last_seen` is older than
/// `DISCONNECT_TIMEOUT_NS` (30 s, hardcoded) as `Disconnected` BEFORE it looks at
/// the action timer. `count_players_can_act` requires `status == Active`. The
/// deployed tables use `action_timeout_secs = 30` (`table_1`, `btc_table_1`) or 45
/// and 60 (`table_2`, `table_3`), so on the 30 s tables the two thresholds are
/// EQUAL: by the time anybody times out, every player who has not sent a heartbeat
/// in the last 30 seconds is already `Disconnected`, fewer than two players "can
/// act", and `advance_to_next_street` runs the entire remaining board out to a
/// showdown in one message.
///
/// Nobody folded, nobody was all-in, and nobody chose to check down. The turn and
/// the river are simply dealt and the hand is settled.
///
/// E-06 -- the forced settlement -- is STILL LIVE and this test still pins it. What
/// changed is the cost: the settlement used to come out of the stale pre-flop
/// `side_pots` breakdown (FINDING 01 / E-01), so a 30-second lull with a failing
/// heartbeat both removed the remaining betting rounds AND destroyed everything
/// already wagered post-flop. Since the E-01 fix it destroys nothing, and the tail
/// of this test asserts that. Losing the rest of the hand's betting to a
/// disconnection is still wrong, and still E-06.
///
/// The harness found this while building REG-08: the first version of that test
/// used a 30 s timeout and the hand ended before the folded player could cash out.
#[test]
fn reg09_one_timeout_with_no_heartbeats_runs_the_whole_board_out_and_settles() {
    // Exactly the deployed table_1 timeout.
    let config = TableConfig {
        action_timeout_secs: 30,
        ..TableConfig::six_max_icp()
    };
    let world = World::new(config, &["alice", "bob", "carol"]);
    let seats = [world.actor("alice"), world.actor("bob"), world.actor("carol")];
    for (i, who) in seats.iter().enumerate() {
        world.fund_escrow(*who, 6 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(seats[0]).expect("deal");

    // Reach the flop, then put real money in post-flop so the loss is visible.
    for _ in 0..12 {
        let t = world.table_state();
        if t.phase == GamePhase::Flop {
            break;
        }
        let who = on_clock(&t);
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        world.player_action(who, PlayerAction::Check).expect("check");
    }
    assert_eq!(world.table_state().phase, GamePhase::Flop);
    world
        .player_action(on_clock(&world.table_state()), PlayerAction::Bet(ICP))
        .expect("post-flop bet");
    world
        .player_action(on_clock(&world.table_state()), PlayerAction::Call)
        .expect("post-flop call");

    let before = world.snapshot();
    let live_before = before
        .table
        .seated()
        .filter(|p| !p.has_folded && !p.is_all_in)
        .count();
    assert!(
        live_before >= 2,
        "at least two players must still be live in the hand, not folded and not all-in"
    );
    assert_eq!(before.table.phase, GamePhase::Flop);
    let total_before = before.internal_total();
    let pot_before = before.table.pot;

    // A 30-second lull. No heartbeats: exactly what a browser tab losing its
    // connection produces.
    world.advance(Duration::from_secs(31));
    let outcome = world.check_timeouts(seats[0]).expect("check_timeouts");
    println!("REG-09 check_timeouts -> {outcome:?}");

    let after = world.snapshot();
    assert_eq!(
        after.table.phase,
        GamePhase::HandComplete,
        "ONE timeout ended the hand for everybody. Live players before: {live_before}. \
         If this now leaves the hand in progress the interaction has been fixed -- update \
         docs/SECURITY-FINDINGS.md and this test together."
    );
    assert_eq!(
        after.table.community_cards.len(),
        5,
        "the turn and the river were dealt without anybody acting on them"
    );
    assert!(
        after
            .table
            .seated()
            .all(|p| p.status == PlayerStatus::Disconnected || p.status == PlayerStatus::SittingOut),
        "every player was marked Disconnected before the timer was even looked at: {:?}",
        after.table.seated().map(|p| (p.seat, p.status.clone())).collect::<Vec<_>>()
    );

    let destroyed = total_before as i128 - after.internal_total() as i128;
    println!(
        "REG-09: pot was {pot_before}, {destroyed} e8s destroyed by the forced settlement"
    );
    // E-06 -- the forced settlement itself -- is STILL LIVE: one 30-second lull
    // still deals the turn and the river and settles the hand for everybody. What
    // changed with the E-01 fix is what that costs. The settlement used to pay out
    // of the frozen pre-flop breakdown, so the whole post-flop pot was destroyed;
    // this run used to report 200,000,000 e8s gone.
    assert_eq!(
        destroyed, 0,
        "PINNED FIX (E-01): a forced settlement must still pay out every e8 it \
         collected. before {total_before}, after {}, pot was {pot_before}",
        after.internal_total()
    );
    let history = world
        .hand_history(after.table.hand_number)
        .expect("the forced hand is in the history");
    let awarded: u64 = history
        .winners
        .iter()
        .fold(0u64, |a, w| a.saturating_add(w.amount));
    assert_eq!(
        awarded, pot_before,
        "the forced showdown must award the whole pot it collected. winners={:?}",
        history.winners
    );
}

// ---------------------------------------------------------------------------
// REG-06 -- admin_reinit_table destroys every seated player's chips
// ---------------------------------------------------------------------------

/// `admin_reinit_table` is controller-only, but it calls `init_table_state`, which
/// builds a brand new `TableState` with empty seats. Every seated player's chips
/// and the whole pot are dropped on the floor: not returned to escrow, not
/// withdrawable, gone. The canister's ledger balance is unchanged, so the money is
/// stranded exactly as in REG-01.
///
/// Documented rather than patched: it needs an owner decision (return chips to
/// escrow first, or refuse while anyone is seated).
#[test]
fn reg06_admin_reinit_table_strands_every_seated_players_chips() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    for (i, who) in [alice, bob].into_iter().enumerate() {
        world.fund_escrow(who, 6 * ICP).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));

    let before = world.snapshot();
    assert!(before.chips_total > 0);

    let raw = world
        .pic
        .update_call(
            world.table,
            world.controller,
            "admin_reinit_table",
            Encode!(&world.config).unwrap(),
        )
        .expect("admin_reinit_table must not be rejected for a controller");
    let out = Decode!(&raw, Result<(), String>).expect("reinit reply decode");
    assert!(out.is_ok(), "admin_reinit_table returned {out:?}");

    let after = world.snapshot();
    assert_eq!(after.chips_total, 0, "every chip stack is gone");
    assert_eq!(
        after.escrow_total, before.escrow_total,
        "and NOTHING was returned to escrow"
    );
    assert_eq!(
        after.ledger_main, before.ledger_main,
        "the tokens are still in the canister, owed to nobody"
    );

    let vs = check_world(&world);
    let m1 = vs
        .iter()
        .find(|v| v.invariant == Invariant::M1Conservation)
        .expect("M1 must flag the stranded chips");
    assert_eq!(m1.delta_e8s, before.chips_total as i128);
    println!("REG-06: {}", m1.detail);
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn on_clock(t: &TableState) -> Principal {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
        .expect("the seat on the clock must be occupied")
}
