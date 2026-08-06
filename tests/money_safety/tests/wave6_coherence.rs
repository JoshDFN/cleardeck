//! WAVE 6 COHERENCE PROBES — the reproducers for FINDING 17, 18 and 07.
//!
//! Written by the wave-6 reconciling pass, not by any of the three agents whose
//! work it reconciled. These drive sequences none of the builders' own tests
//! drive, and they are the evidence behind docs/WAVE-06.md.
//!
//! # WHAT IS WIRED IN, AND WHAT IS NOT
//!
//! `scripts/dev.sh cmd_test` names its money-safety targets explicitly —
//! `invariants`, `regressions`, `deposit_replay`, `ui_limits`, `fuzz` — precisely
//! so that a target cannot go unrun by accident (docs/DEFECTS.md: `deposit_replay`
//! carried the only proven fund-theft primitive in the project and was named by no
//! make target for a whole wave).
//!
//! * `probe1` is a genuine gate and asserts.
//! * `probe4` is a genuine gate as of the FINDING 17 fix: it now asserts that the
//!   fold-out winner IS paid the pot, and it goes RED if `is_in_hand` starts
//!   accepting a cardless seat again. **This file is named by `cmd_test` for
//!   `probe1` and `probe4`'s sake.**
//! * `probe5` is a genuine gate as of the FINDING 07 fix. It used to PIN the
//!   defect — it asserted that `reset_table` destroyed every seated chip and that
//!   M9 stayed silent about it — and was `#[ignore]`d for that reason. It now
//!   asserts that `reset_table` REFUSES, that `admin_reinit_table` CONSERVES, and
//!   that the drain leaves nothing belonging to nobody. The `#[ignore]` is gone
//!   with the pin, because leaving a marker pinned after its defect is fixed is
//!   how a gate goes red for a whole wave and teaches everyone to ignore it.
//!
//! Run them by name:
//!
//! ```text
//! cd tests/money_safety
//! CLEARDECK_TABLE_WASM=../../target/wasm32-unknown-unknown/release/table_canister.wasm \
//!   cargo test --test wave6_coherence -- --nocapture --test-threads=1
//! ```

use candid::{Decode, Encode};
use money_safety::invariants::reachability;
use money_safety::table_api::*;
use money_safety::world::*;
use std::time::Duration;

const ICP: u64 = 100_000_000;

fn on_clock(t: &TableState) -> Option<candid::Principal> {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
}

fn seats(t: &TableState) -> String {
    t.players
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
        .map(|(i, p)| {
            format!(
                "seat{i} chips={} bet={} folded={} cards={} status={:?}",
                p.chips,
                p.total_bet_this_hand,
                p.has_folded,
                p.hole_cards.is_some(),
                p.status
            )
        })
        .collect::<Vec<_>>()
        .join("\n    ")
}

// ---------------------------------------------------------------------------
// PROBE 1 — the first auditor's fund lock, reached by REAL SILENCE
// ---------------------------------------------------------------------------

/// **A GATE.** The auditor's table got where it got because a client stopped
/// sending heartbeats. `m9_the_auditors_disconnect_sequence_leaves_every_door_open`
/// takes the `sit_out()` shortcut to the same status flag — but `sit_out()`'s
/// mid-hand behaviour is something THIS WAVE CHANGED, so the shortcut and the
/// original are no longer the same sequence. This drives the original: nobody calls
/// anything on carol's behalf, ever.
///
/// Asserts: no update traps for anybody at any point, and every funded player's
/// money reaches their own on-ledger wallet.
#[test]
fn probe1_the_auditors_lock_reached_by_real_silence() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for (i, who) in [alice, bob, carol].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    let staked = world.snapshot().internal_total();
    assert!(staked > 0, "the table must actually hold money");

    // Carol's client goes quiet. Nobody calls anything for her. Alice and bob keep
    // their clients alive, which is what a real table looks like.
    for _ in 0..4 {
        world.advance(Duration::from_secs(30));
        let _ = world.heartbeat(alice);
        let _ = world.heartbeat(bob);
        if let Err(OpError::Trap(m)) = world.check_timeouts(alice) {
            panic!("PROBE1 M9 VIOLATED: check_timeouts TRAPPED while carol went silent: {m}");
        }
    }
    eprintln!("after silence:\n    {}", seats(&world.table_state()));

    // Ordinary play: whoever is on the clock folds; carol's clock simply runs out.
    for _ in 0..8 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else { break };
        if who == carol {
            world.advance(Duration::from_secs(120));
            if let Err(OpError::Trap(m)) = world.check_timeouts(alice) {
                panic!("PROBE1 M9 VIOLATED: check_timeouts TRAPPED with {staked} e8s staked: {m}");
            }
            continue;
        }
        if let Err(OpError::Trap(m)) = world.player_action(who, PlayerAction::Fold) {
            panic!("PROBE1 M9 VIOLATED: player_action TRAPPED with {staked} e8s staked: {m}");
        }
    }

    // Every door, every player, exactly as the auditor tried them.
    for who in [alice, bob, carol] {
        if let Err(OpError::Trap(m)) = world.check_timeouts(who) {
            panic!("PROBE1 M9 VIOLATED: check_timeouts TRAPPED: {m}");
        }
        if let Err(OpError::Trap(m)) = world.leave_table(who) {
            panic!("PROBE1 M9 VIOLATED: leave_table TRAPPED: {m}");
        }
    }

    let report = reachability::drain(&mut world);
    assert!(
        report.fully_drained(),
        "PROBE1 M9 VIOLATED: {} of {} e8s could not be got out.\n  {}",
        report.owed_after,
        report.owed_before,
        report.log.join("\n  ")
    );
    for who in [alice, bob, carol] {
        assert!(
            report.returned_to_wallets.get(&who).copied().unwrap_or(0) > 0,
            "PROBE1: {who} got no ledger money back.\n  {}",
            report.log.join("\n  ")
        );
    }
}

// ---------------------------------------------------------------------------
// PROBE 4 — FINDING 17: the hand that is settled with NO live claim on it
// ---------------------------------------------------------------------------

/// **A GATE. It convicts docs/SECURITY-FINDINGS.md FINDING 17 returning.**
///
/// FINDING 17 was: `is_in_hand` (participation) accepted a seat that was `Active`
/// and held NO CARDS; `live_claims` (eligibility) did not.
/// `count_active_players(state) == 1` is what calls `end_hand_single_winner`, and
/// the one seat it counted could be the cardless one — at which point `live_claims`
/// was empty, `plan_payouts` took its no-claimant branch, and every stake went back
/// to its funder, including the folders'. Measured here as a 52,000,000 e8 pot with
/// all three seats ending on exactly their buy-in.
///
/// **Nothing in this sequence is hostile.** A mid-hand arrival takes an empty
/// chair and sits in (two ordinary `Ok` calls); one player folds (ordinary poker);
/// and the LAST CARD-HOLDER — the seat that has just won by fold-out — stops
/// answering and its own action clock runs.
///
/// The two predicates are now one function, so the hand settles the instant the
/// folder folds and the fold-out winner is paid. **This test asserts the OUTCOME,
/// not the totals:** the totals were exact while the defect was live. Restoring the
/// `|| p.status == PlayerStatus::Active` disjunct on `is_in_hand` makes it RED at
/// the `winner_stack > buy_in` assertion.
#[test]
fn probe4_finding_17_the_foldout_winner_is_paid_the_pot() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for who in [alice, bob, carol] {
        world.fund_escrow(who, 20 * ICP).expect("deposit");
    }
    world.join_table(alice, 0).expect("seat alice");
    world.join_table(bob, 1).expect("seat bob");
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");

    for _ in 0..4 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(w) = on_clock(&t) else { break };
        if world.player_action(w, PlayerAction::Raise(ICP / 2)).is_err() {
            let _ = world.player_action(w, PlayerAction::Call);
        }
        if world.pot() >= ICP / 2 {
            break;
        }
    }
    let pot = world.pot();
    let buy_in = world
        .table_state()
        .player_at(0)
        .map(|p| p.chips + p.total_bet_this_hand)
        .unwrap_or(0);
    eprintln!("PROBE4 pot={pot} buy_in={buy_in}");
    assert!(pot >= ICP / 4, "need a real pot, got {pot}");

    // A mid-hand arrival at an EMPTY chair. Both calls return Ok.
    world.join_table(carol, 2).expect("carol takes an empty chair");
    world.sit_in(carol).expect("carol sits in");

    // One player folds. Ordinary poker: the other has now won by fold-out.
    let t = world.table_state();
    let Some(folder) = on_clock(&t) else {
        panic!("nobody on the clock")
    };
    world.player_action(folder, PlayerAction::Fold).expect("fold");

    // Nobody answers again. Every remaining clock simply expires.
    for _ in 0..8 {
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
        world.advance(Duration::from_secs(120));
        if let Err(OpError::Trap(m)) = world.check_timeouts(alice) {
            panic!("PROBE4: check_timeouts TRAPPED: {m}");
        }
    }

    let t = world.table_state();
    eprintln!("PROBE4 FINAL phase={:?} pot={}\n    {}", t.phase, t.pot, seats(&t));
    let stack = |seat: u8| t.player_at(seat).map(|p| p.chips).unwrap_or(0);
    let (winner_seat, folder_seat) = if folder == alice { (1u8, 0u8) } else { (0u8, 1u8) };
    let winner_stack = stack(winner_seat);
    let folder_stack = stack(folder_seat);

    // THE OUTCOME, ASSERTED. The totals were EXACT while the defect was live —
    // every seat ended on precisely its buy-in and M1 through M9 were silent — so
    // an assertion about conservation cannot see this defect at all. What has to be
    // true is who was paid.
    assert!(
        winner_stack > buy_in,
        "FINDING 17 HAS RETURNED: seat {winner_seat} won a {pot} e8 pot by fold-out and came out \
         of the hand on {winner_stack}, no better than its {buy_in} buy-in. Stacks were \
         ({}, {}, {}). The likely cause is `is_in_hand` accepting a seat that holds no cards \
         again: see docs/SECURITY-FINDINGS.md FINDING 17 and the \"WHO IS IN THE HAND\" section \
         of src/table_canister/src/lib.rs.",
        stack(0),
        stack(1),
        stack(2)
    );
    assert!(
        folder_stack < buy_in,
        "FINDING 17 HAS RETURNED: seat {folder_seat} FOLDED and got its stake back \
         ({folder_stack} against a {buy_in} buy-in). A refund-everyone settlement happened on a \
         hand somebody had won."
    );
    // The chair that arrived mid-hand staked nothing and can win nothing.
    assert_eq!(
        stack(2),
        buy_in,
        "the mid-hand arrival staked nothing and must end exactly where it started"
    );
    assert_eq!(
        stack(0) + stack(1) + stack(2),
        3 * buy_in,
        "CONSERVATION: the outcome moved, the total must not have"
    );
    eprintln!(
        "PROBE4 OK: seat {winner_seat} won the fold-out and was paid ({buy_in} -> \
         {winner_stack}); seat {folder_seat} folded and paid for it ({buy_in} -> {folder_stack})."
    );
}

// ---------------------------------------------------------------------------
// PROBE 5 — FINDING 07: one controller call destroys every seated chip
// ---------------------------------------------------------------------------

/// **A GATE SINCE 2026-08-05. It used to be the PIN for FINDING 07.**
///
/// What it recorded: `reset_table` and `admin_reinit_table` had byte-identical
/// bodies and both called `init_table_state`, which builds a fresh `TableState`
/// with empty seats. Every seated chip ceased to exist — not returned to escrow,
/// not withdrawable, and `admin_restore_balance` was deliberately removed so
/// nobody could put it back. This test ASSERTED `chips_after == 0` and asserted
/// that M9's drain still reported `fully_drained`, which was true and was the
/// finding: `internal_total` counts what the canister SAYS it owes, and after the
/// reset it said it owed nothing.
///
/// Both assertions are now inverted, exactly as the old comment instructed:
/// *"becomes a gate the moment FINDING 07 is fixed: assert that `reset_table`
/// either refuses or conserves."* It does both, because the two doors are no
/// longer the same function — `reset_table` REFUSES while money is at the table,
/// `admin_reinit_table` CONSERVES by paying every chip into its owner's escrow
/// first — and the drain is now checked with `table_is_really_empty()`, which asks
/// the LEDGER rather than the canister's own books.
///
/// The `#[ignore]` is gone with the pin. The wide coverage of the admin custody
/// surface, including the sweep over every controller-callable method, is in
/// `tests/admin_custody.rs`.
#[test]
fn probe5_finding_07_reset_table_can_no_longer_destroy_a_seated_chip() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    for (i, who) in [alice, bob].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    let chips_before = world.chips_total();
    let owed_before = world.snapshot().internal_total();
    let held_before = world.ledger_balance(world.table, None);
    assert!(chips_before > 0, "somebody must be sitting on chips");

    let admin = |m: &str| -> String {
        let arg = Encode!(&TableConfig::six_max_icp()).unwrap();
        match world.pic.update_call(world.table, world.controller, m, arg) {
            Ok(b) => format!("{:?}", Decode!(&b, Result<(), String>)),
            Err(e) => format!("REJECTED {e:?}"),
        }
    };
    let reset = admin("reset_table");
    let reinit = admin("admin_reinit_table");
    eprintln!("PROBE5 reset_table        -> {reset}");
    eprintln!("PROBE5 admin_reinit_table -> {reinit}");

    let chips_after = world.chips_total();
    let held_after = world.ledger_balance(world.table, None);
    let owed_after = world.snapshot().internal_total();
    eprintln!(
        "PROBE5 chips {chips_before} -> {chips_after}; canister holds {held_after} \
         (was {held_before}); owed {owed_before} -> {owed_after}"
    );

    // THE GATE. Door one must refuse rather than delete.
    assert!(
        reset.contains("Err"),
        "FINDING 07 HAS RETURNED: reset_table accepted a table holding {chips_before} e8s of \
         seated chips. Its body must refuse while the table holds custody. Got: {reset}"
    );

    // Door two must conserve: the money moved out of the seats and into escrow,
    // and the canister owes exactly as much as it did before.
    assert_eq!(
        owed_after, owed_before,
        "FINDING 07 HAS RETURNED: the two admin calls changed what this canister owes players \
         from {owed_before} to {owed_after} without paying anybody. reset={reset} reinit={reinit}"
    );
    assert_eq!(
        held_after, held_before,
        "no money should have left the canister"
    );

    // AND THE HARNESS NOW NOTICES. `fully_drained()` was true here before the fix
    // too -- that was the finding -- so the assertion is on the ledger-anchored
    // verdict, not on the canister's own claim.
    let mut w = world;
    let report = reachability::drain(&mut w);
    assert!(
        report.table_is_really_empty(),
        "PROBE5: after the two admin calls the table could not be emptied honestly. owed_after={} \
         orphaned={} (money inside a canister that owes it to nobody).\n  {}",
        report.owed_after,
        report.orphaned_e8s(),
        report.log.join("\n  ")
    );
    let returned: u64 = report.returned_to_wallets.values().sum();
    eprintln!(
        "PROBE5 FINDING 07 CLOSED: reset_table refused, admin_reinit_table conserved, and \
         {returned} e8s ({} ICP) reached real wallets with nothing left belonging to nobody.",
        returned as f64 / ICP as f64
    );
    assert!(
        returned >= chips_before,
        "at least the once-destroyed chips must reach a wallet; got {returned}"
    );
}
