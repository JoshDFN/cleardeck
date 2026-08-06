//! WAVE 6 COHERENCE PROBES — the reproducers for FINDING 17, 18 and 07.
//!
//! Written by the wave-6 reconciling pass, not by any of the three agents whose
//! work it reconciled. These drive sequences none of the builders' own tests
//! drive, and they are the evidence behind docs/WAVE-06.md.
//!
//! # THIS FILE IS NOT WIRED INTO ANY GATE, AND THAT IS A KNOWN HOLE
//!
//! `scripts/dev.sh cmd_test` names its money-safety targets explicitly —
//! `invariants`, `regressions`, `deposit_replay`, `ui_limits`, `fuzz` — precisely
//! so that a target cannot go unrun by accident (docs/DEFECTS.md: `deposit_replay`
//! carried the only proven fund-theft primitive in the project and was named by no
//! make target for a whole wave). This file is deliberately NOT added to that list,
//! because `probe2`/`probe4`/`probe5` currently RECORD defective behaviour rather
//! than forbid it, and a target that passes while the defect is present teaches
//! nobody anything.
//!
//! **Wiring it in is part of closing the findings, not a separate chore:**
//!
//! * `probe1` is a genuine gate today and asserts. It could be added to
//!   `cmd_test` as-is.
//! * `probe4` becomes a gate the moment [FINDING 17](../../../docs/SECURITY-FINDINGS.md)
//!   is fixed: invert its final assertion from "everyone got their buy-in back" to
//!   "the fold-out winner was paid the pot", and it convicts the defect returning.
//! * `probe5` becomes a gate the moment [FINDING 07](../../../docs/SECURITY-FINDINGS.md)
//!   is fixed: assert that `reset_table` either refuses or conserves.
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

/// **RECORDS A DEFECT. See docs/SECURITY-FINDINGS.md FINDING 17.**
///
/// `is_in_hand` (participation) accepts a seat that is `Active` and holds NO
/// CARDS; `live_claims` (eligibility) does not. `count_active_players(state) == 1`
/// is what calls `end_hand_single_winner`, and the one seat it counts can be the
/// cardless one — at which point `live_claims` is empty, `plan_payouts` takes its
/// no-claimant branch, and every stake goes back to its funder.
///
/// **Nothing in this sequence is hostile.** A mid-hand arrival takes an empty
/// chair and sits in (two ordinary `Ok` calls); one player folds (ordinary poker);
/// and the LAST CARD-HOLDER — the seat that has just won by fold-out — is folded by
/// its own action clock, which is exactly what this wave's headline fix now does to
/// a dropped client.
///
/// When FINDING 17 is fixed, invert the final assertion: the fold-out winner must
/// be paid the pot, and this becomes the gate that convicts the defect returning.
#[test]
fn probe4_finding_17_a_foldout_winner_loses_the_pot_to_their_own_clock() {
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
    let winner_stack = if folder == alice { stack(1) } else { stack(0) };

    // THE DEFECT, PINNED. Everybody ends on exactly their buy-in: the hand was
    // un-played, the folder was refunded, and the winner was paid nothing.
    assert_eq!(
        (stack(0), stack(1), stack(2)),
        (buy_in, buy_in, buy_in),
        "FINDING 17 pin: if this no longer holds, the defect has been FIXED. Invert this \
         assertion to `winner_stack > buy_in` and update docs/SECURITY-FINDINGS.md FINDING 17 \
         and docs/DEFECTS.md E-36 in the same change."
    );
    assert_eq!(
        winner_stack, buy_in,
        "FINDING 17: the seat that won by fold-out was paid nothing for a {pot} e8 pot"
    );
    eprintln!(
        "PROBE4 FINDING 17 REPRODUCED: a {pot} e8 pot was refunded to its funders, including \
         the seat that FOLDED; the fold-out winner got nothing."
    );
}

// ---------------------------------------------------------------------------
// PROBE 5 — FINDING 07: one controller call destroys every seated chip
// ---------------------------------------------------------------------------

/// **RECORDS A DEFECT. See docs/SECURITY-FINDINGS.md FINDING 07.**
///
/// `reset_table` and `admin_reinit_table` have byte-identical bodies and both call
/// `init_table_state`, which builds a fresh `TableState` with empty seats. Every
/// seated chip ceases to exist: not returned to escrow, not withdrawable, and
/// `admin_restore_balance` was deliberately removed so nobody can put it back.
///
/// The part that matters most for the harness: **M9's drain reports
/// `fully_drained` afterwards**, because `internal_total` counts what the canister
/// SAYS it owes and after the reset it says it owes nothing. Every invariant in
/// this suite is silent while 4 ICP is destroyed.
#[test]
fn probe5_finding_07_reset_table_destroys_every_seated_chip_and_m9_is_silent() {
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
    eprintln!("PROBE5 reset_table        -> {}", admin("reset_table"));
    eprintln!("PROBE5 admin_reinit_table -> {}", admin("admin_reinit_table"));

    let chips_after = world.chips_total();
    let held_after = world.ledger_balance(world.table, None);
    eprintln!(
        "PROBE5 chips {chips_before} -> {chips_after}; canister still holds {held_after} \
         (was {held_before}); owed was {owed_before}"
    );

    // THE DEFECT, PINNED.
    assert_eq!(
        chips_after, 0,
        "FINDING 07 pin: if reset_table now refuses or conserves, the defect has been FIXED. \
         Assert that instead and update docs/SECURITY-FINDINGS.md FINDING 07."
    );
    assert_eq!(
        held_after, held_before,
        "the canister still holds every e8 it held before; the chips did not leave, they \
         stopped being owed to anybody"
    );

    // AND THE HARNESS DOES NOT NOTICE.
    let mut w = world;
    let report = reachability::drain(&mut w);
    assert!(
        report.fully_drained(),
        "unexpected: M9 caught this. If it now does, say so in FINDING 07."
    );
    let unreachable = held_before.saturating_sub(report.returned_to_wallets.values().sum::<u64>());
    eprintln!(
        "PROBE5 FINDING 07 REPRODUCED: M9 reports fully_drained, and {unreachable} e8s \
         ({} ICP) are inside a canister that owes them to nobody.",
        unreachable as f64 / ICP as f64
    );
    assert!(
        unreachable >= chips_before,
        "at least the destroyed chips must be unreachable; got {unreachable}"
    );
}
