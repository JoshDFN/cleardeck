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
// REG-01 -- FINDING 01: every showdown with post-flop money destroys it
// ---------------------------------------------------------------------------

/// The shortest sequence that permanently destroys real user funds.
///
/// Two players, one hand, one post-flop bet that is called, then the board runs
/// out to a showdown. The winner is paid the PRE-FLOP pot only; everything
/// wagered on the flop, turn and river is debited from the stacks and credited to
/// nobody. The tokens stay in the canister on the ledger, and because `withdraw`
/// pays strictly against the caller's own escrow and there is no administrative
/// withdrawal function anywhere in the canister, they can never be recovered by
/// anyone -- including a controller.
#[test]
fn reg01_a_showdown_with_post_flop_betting_destroys_the_post_flop_money() {
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

    // One bet, one call: 1 ICP each into the pot post-flop.
    let bet = ICP;
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
    assert_eq!(
        after_bet.side_pots_total(),
        frozen_breakdown,
        "but the payout basis did NOT grow -- this is the defect"
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
        destroyed,
        (2 * bet) as i128,
        "exactly the post-flop money must have been destroyed: pre-flop pot {}, post-flop {} \
         each, total before {}, total after {}",
        preflop_pot,
        bet,
        total_before,
        after.internal_total()
    );

    // And it is unreachable: the canister holds it on the ledger but owes it to
    // nobody.
    let vs = check_world(&world);
    let m1 = vs
        .iter()
        .find(|v| v.invariant == Invariant::M1Conservation)
        .expect("M1 must be violated");
    assert_eq!(m1.severity, Severity::FundDestruction);
    assert_eq!(m1.delta_e8s, (2 * bet) as i128);
    println!("REG-01: {}", m1.detail);
}

// ---------------------------------------------------------------------------
// REG-02 -- notify_deposit cannot decode the real ICP ledger
// ---------------------------------------------------------------------------

// The two reply shapes live in the library so this file stays readable and so a
// reader can diff them directly: see src/legacy_ledger_shapes.rs.
use money_safety::legacy_ledger_shapes::{as_declared_by_the_canister, as_the_real_ledger_returns_it};

/// `notify_deposit` can NEVER credit a deposit against the real ICP ledger, and
/// the ICP a user sent through the advertised deposit address is stranded in the
/// canister forever.
///
/// Root cause, isolated to a single field type: the canister declares the ledger's
/// `AccountIdentifier` as `record { hash : blob }`, but the real ledger returns a
/// bare `blob`. Every `query_blocks` reply therefore fails to decode, and
/// `notify_deposit` returns "Failed to decode ledger response" for every block,
/// forever.
#[test]
fn reg02_notify_deposit_cannot_decode_the_real_ledger_and_strands_the_money() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");

    // A user follows the documented flow: transfer to the canister's account,
    // then call notify_deposit.
    let sent = 5 * ICP;
    let block = world
        .raw_transfer_to_canister(alice, sent)
        .expect("the ledger transfer itself works fine");
    assert_eq!(
        world.ledger_balance(world.table, None),
        sent,
        "the canister really holds the user's ICP"
    );

    let outcome = world.notify_deposit(alice, block);
    let message = match &outcome {
        Err(OpError::Err(m)) => m.clone(),
        other => panic!(
            "notify_deposit unexpectedly returned {other:?}. If it now SUCCEEDS the defect has \
             been fixed -- update docs/SECURITY-FINDINGS.md and this test together."
        ),
    };
    assert!(
        message.contains("Failed to decode ledger response"),
        "expected the decode failure, got: {message}"
    );
    assert_eq!(
        world.get_balance(alice),
        0,
        "the user was credited nothing for real money the canister now holds"
    );

    // The money is unreachable: withdraw pays strictly against escrow.
    assert!(
        world.withdraw(alice, ICP).is_err(),
        "the user cannot withdraw money they really sent"
    );
    assert_eq!(
        world.ledger_balance(world.table, None),
        sent,
        "and it is still sitting in the canister"
    );

    // --- root cause, isolated -----------------------------------------------
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

    // BUG A -- the tuple mis-decode. `Response::candid::<R>()` in ic-cdk 0.19 is
    // `decode_one::<R>()`, so asking for `(QueryBlocksResponse,)` asks the decoder
    // for ONE value of type `record { 0 : QueryBlocksResponse }`. The reply holds
    // one value of type `QueryBlocksResponse`. It can never match.
    let as_the_canister_asks =
        candid::decode_one::<(as_declared_by_the_canister::QueryBlocksResponse,)>(&raw);
    assert!(
        as_the_canister_asks.is_err(),
        "BUG A appears to be fixed: decode_one::<(QueryBlocksResponse,)> now succeeds. Update \
         docs/SECURITY-FINDINGS.md and this test together."
    );
    // The error carries the whole reply as hex; keep the readable part only.
    let bug_a = format!("{:?}", as_the_canister_asks.err());
    let bug_a_reason = bug_a
        .rfind("is not a tuple type")
        .map(|i| {
            let tail = &bug_a[..i + "is not a tuple type".len()];
            let head = tail.rfind("Subtyping error").unwrap_or(0);
            tail[head..].replace('\n', " ").replace("  ", " ")
        })
        .unwrap_or_else(|| bug_a.chars().take(160).collect());
    println!("REG-02 BUG A -- decode_one::<(QueryBlocksResponse,)> failed: {bug_a_reason}");

    // BUG B -- even decoded as ONE value, the canister's declared shapes silently
    // lose the transaction. `AccountIdentifier` is declared as
    // `record { hash : blob }` but the ledger returns a bare `blob`, so the
    // `Operation` variant does not match and `opt Operation` decodes to NULL under
    // Candid's opt rule. No error, no trap: `operation` is simply None, and
    // notify_deposit's next branch says "Transaction is not a transfer".
    let declared = candid::decode_one::<as_declared_by_the_canister::QueryBlocksResponse>(&raw)
        .expect("as one value the outer record does decode");
    assert_eq!(declared.blocks.len(), 1);
    assert!(
        declared.blocks[0].transaction.operation.is_none(),
        "BUG B appears to be fixed: the declared Operation shape now matches the ledger. Update \
         docs/SECURITY-FINDINGS.md and this test together."
    );
    println!(
        "REG-02 BUG B -- with the canister's declared types the operation decodes to {:?}",
        declared.blocks[0].transaction.operation
    );

    // With `AccountIdentifier = blob` the SAME bytes yield the real transfer.
    let corrected = Decode!(&raw, as_the_real_ledger_returns_it::QueryBlocksResponse)
        .expect("with AccountIdentifier = blob the SAME bytes decode cleanly");
    assert_eq!(corrected.blocks.len(), 1);
    match &corrected.blocks[0].transaction.operation {
        Some(as_the_real_ledger_returns_it::Operation::Transfer(t)) => {
            assert_eq!(t.amount.e8s, sent);
            assert_eq!(t.to.len(), 32, "`to` is a bare 32-byte blob, not a record");
        }
        other => panic!("expected a Transfer, got {other:?}"),
    }

    // M1 sees exactly this money as stranded, once the harness stops excusing it
    // as an outstanding raw deposit.
    world.note_raw_deposit_credited(sent);
    let vs = check_world(&world);
    let m1 = vs
        .iter()
        .find(|v| v.invariant == Invariant::M1Conservation)
        .expect("M1 must flag the stranded money");
    assert_eq!(m1.delta_e8s, sent as i128);
    println!("REG-02: {}", m1.detail);
}

// ---------------------------------------------------------------------------
// REG-05 -- FINDING 05: leave_table mid-hand orphans the leaver's stake
// ---------------------------------------------------------------------------

/// `leave_table` mid-hand removes the seat, so the leaver's `total_bet_this_hand`
/// vanishes from the contribution list while their money stays in `pot`. The
/// bet-level split then has an unattributed remainder, which
/// `poker_core::build_side_pots` appends to the LAST (highest) bet level -- the
/// pot only the deepest stacks can win.
///
/// `cash_out` reaches the same state by a different door: it refuses only for
/// players who have NOT folded, so a player who folds first can vacate the seat
/// mid-hand too.
#[test]
fn reg05_vacating_a_seat_mid_hand_orphans_the_leavers_stake() {
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
            "{door}: exactly the leaver's stake ({stake}) is now unattributed"
        );

        let vs = check_pot_breakdown(&world.snapshot());
        let orphan = vs
            .iter()
            .find(|v| v.delta_e8s == stake as i128)
            .unwrap_or_else(|| panic!("{door}: M1b must flag the orphaned stake: {vs:?}"));
        println!("REG-05 ({door}): {}", orphan.detail);
        proved += 1;
    }
    assert!(
        proved > 0,
        "at least one door into the orphaned-stake state must have been demonstrated"
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
        "exactly the timed-out player's stake ({stake}) is now unattributed, so the bet-level \
         split will hand it to the highest bet level instead of the pot the short stacks can win"
    );

    let vs = check_pot_breakdown(&world.snapshot());
    let orphan = vs
        .iter()
        .find(|v| v.delta_e8s == stake as i128)
        .unwrap_or_else(|| panic!("M1b must flag the orphaned stake: {vs:?}"));
    println!("REG-08: {}", orphan.detail);

    // Conservation still holds at this instant: the money has not been misallocated
    // yet, it is only mis-ATTRIBUTED. The loss happens at payout.
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
/// the river are simply dealt and the hand is settled, out of the stale pre-flop
/// `side_pots` breakdown, which is FINDING 01. So a 30-second lull with a failing
/// heartbeat both removes the remaining betting rounds and destroys whatever was
/// already wagered post-flop.
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
    assert!(
        destroyed > 0,
        "the forced settlement must have destroyed the post-flop money (FINDING 01): \
         before {total_before}, after {}",
        after.internal_total()
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
