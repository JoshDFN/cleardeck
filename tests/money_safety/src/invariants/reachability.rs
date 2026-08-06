//! M9 FUND REACHABILITY.
//!
//! > For any state a sequence of legal calls can reach, there exists a sequence of
//! > legal calls by each funded player that returns that player's balance to the
//! > ledger.
//!
//! # Why this file exists
//!
//! Every other invariant in this harness asks whether the arithmetic is right:
//! chips conserved (M1), the canister solvent (M2), no rake (M3), no double pay
//! (M6), the right principal paid (M8). **None of them asks whether the player can
//! still get the money out**, and a table can satisfy all of them while being
//! permanently frozen with funded seats.
//!
//! That is not hypothetical. An independent auditor, playing an ordinary hand on
//! the running local stack, reached exactly that state:
//!
//! ```text
//! check_timeouts   IC0503 trap: IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or
//!                  5-card board (flop/turn/river), got 0 community cards
//! player_action    the same trap
//! leave_table      the same trap
//! withdraw         Err("Cannot withdraw while in a hand")
//! cash_out         Err("Cannot cash out while in a hand")
//! ```
//!
//! About 420 ICP, and not one door open. Every invariant in this harness was
//! silent, correctly: nothing had gone missing. **Chips conserved inside a
//! canister nobody can withdraw from is not safety.**
//!
//! # How it is checked, and why it takes two forms
//!
//! [`check_no_settlement_trap`] runs on EVERY step and costs no extra messages: it
//! reads the outcome of the op the loop just executed. The rule it enforces is
//! blunt and exactly right for this canister: **no update on the settlement path
//! may trap while the table is holding money.** A trap is not a failed call here,
//! it is a closed door: the IC rolls the message back, so the state that produced
//! the trap is still there and the next identical call takes the identical path.
//! One trap is therefore permanent for that door, and the doors are not
//! independent -- `check_timeouts`, `player_action` and `leave_table` all run
//! settlement, and `withdraw`/`cash_out` refuse while a hand is live -- so one
//! trap shuts all five.
//!
//! [`drain`] runs at the end of every run and is the constructive half: it takes
//! every actor's money out for real and checks it lands on the ledger. A
//! structural check can only fail on the failure modes somebody thought of; a
//! drain fails on all of them, including the ones nobody has invented yet.

use candid::Principal;
use std::collections::BTreeMap;
use std::time::Duration;

use super::{phase_of, Invariant, Severity, Violation};
use crate::actions::{Op, StepResult};
use crate::world::{OpError, World};

/// The endpoints a player has to reach their own money, or to make the hand that
/// is holding it move on.
///
/// This list IS the finding. Every one of these was closed at once, and the first
/// three were closed by the same single unguarded evaluator call.
pub const MONEY_DOORS: &[&str] = &[
    "check_timeouts",
    "player_action",
    "leave_table",
    "cash_out",
    "withdraw",
    "abandon_stuck_hand",
];

/// Did this op run the settlement path?
///
/// `Upgrade` is excluded on purpose: a rejected upgrade is a different property
/// (M5), and `post_upgrade` traps deliberately rather than restore a half-decoded
/// table.
fn is_settlement_path(op: &Op) -> bool {
    matches!(
        op,
        Op::CheckTimeouts { .. }
            | Op::ActInTurn { .. }
            | Op::ActLegalInTurn
            | Op::ActAs { .. }
            | Op::ConcurrentActInTurn { .. }
            | Op::LeaveTable { .. }
            | Op::CashOut { .. }
            | Op::Withdraw { .. }
            | Op::ReentrantWithdraw { .. }
            | Op::SitOut { .. }
            | Op::SitIn { .. }
            | Op::StartNewHand { .. }
    )
}

/// M9, per step, from the transcript the loop already has.
///
/// `money_inside` is the canister's own total liability at the moment after the
/// step: escrow + chips + pot. Zero means there is nothing to be unable to reach,
/// and a trap on an empty table is a robustness bug rather than a custody one.
pub fn check_no_settlement_trap(
    step: &StepResult,
    money_inside: u64,
    phase: &crate::table_api::GamePhase,
) -> Vec<Violation> {
    if money_inside == 0 || !is_settlement_path(&step.op) {
        return Vec::new();
    }
    if !step.outcome.starts_with("TRAP(") && !step.outcome.contains(" -> TRAP(") {
        return Vec::new();
    }
    vec![Violation::new(
        Invariant::M9FundReachability,
        "settlement_call_trapped",
        Severity::FundsUnreachable,
        // Deliberately zero. Nothing has gone missing -- that is the whole point.
        0,
        phase,
        format!(
            "a state-advancing call TRAPPED while the canister held {money_inside} e8s for \
             players. A trap rolls the message back, so this door is now closed permanently for \
             this state, and check_timeouts / player_action / leave_table all run the same path \
             while withdraw and cash_out refuse during a hand. op={:?} outcome={}",
            step.op, step.outcome
        ),
    )]
}

/// A live hand whose clock has been expired for a long time, or that has no clock
/// at all, must be endable by somebody. Cheap structural companion to the trap
/// check: it fires on a hand that has silently stopped moving without anything
/// having trapped in this run's window.
pub fn check_stuck_hand_is_escapable(
    status: &crate::table_api::StuckHandStatus,
    money_inside: u64,
    phase: &crate::table_api::GamePhase,
    abandon_would_work: bool,
) -> Vec<Violation> {
    if money_inside == 0 || !status.is_stuck || abandon_would_work {
        return Vec::new();
    }
    vec![Violation::new(
        Invariant::M9FundReachability,
        "stuck_hand_has_no_escape",
        Severity::FundsUnreachable,
        0,
        phase,
        format!(
            "the canister reports a stuck hand holding {} e8s in the pot ({money_inside} e8s in \
             total) and abandon_stuck_hand did not end it",
            status.refundable_pot
        ),
    )]
}

/// What a drain attempt found.
#[derive(Clone, Debug)]
pub struct DrainReport {
    /// Everything the canister said it owed before the drain started.
    pub owed_before: u64,
    /// Everything it still says it owes afterwards.
    pub owed_after: u64,
    /// Ledger money each actor gained, net of transfer fees.
    pub returned_to_wallets: BTreeMap<Principal, u64>,
    /// Calls the drain had to make, in order, for the transcript.
    pub log: Vec<String>,
}

impl DrainReport {
    pub fn fully_drained(&self) -> bool {
        self.owed_after == 0
    }
}

/// THE CONSTRUCTIVE CHECK. Take everybody's money out and see whether it arrives.
///
/// Uses only calls a real player can make -- no controller, no admin, no
/// reinstall. In order, repeatedly until the table stops changing:
///
/// 1. `check_timeouts`, which is what a client polls, to let a hand that CAN
///    finish finish;
/// 2. `abandon_stuck_hand` once the clock has been expired long enough, which is
///    the door that does not depend on the settlement path working;
/// 3. `cash_out` to turn a stack into escrow;
/// 4. `withdraw` to turn escrow into real ledger money.
///
/// Time is advanced between rounds because both (1) and (2) are time-gated, and a
/// player waiting is a legal move.
pub fn drain(world: &mut World) -> DrainReport {
    let before = world.snapshot();
    let owed_before = before.internal_total();
    let mut log: Vec<String> = Vec::new();
    let wallets_before: BTreeMap<Principal, u64> = before.actor_wallets.clone();
    let actors: Vec<Principal> = world.actors.iter().map(|a| a.principal).collect();

    // Ten rounds is far more than enough: each round can finish at most one hand,
    // and the loop stops early when nothing is left owing.
    for round in 0..10 {
        if world.snapshot().internal_total() == 0 {
            break;
        }
        // Let a hand that can finish, finish.
        for who in &actors {
            let r = world.check_timeouts(*who);
            if let Err(OpError::Trap(m)) = &r {
                log.push(format!("round {round}: check_timeouts TRAPPED: {m}"));
            }
        }
        // Past the action clock, past the stuck-hand grace, past the withdrawal
        // cooldown. All three are things a player achieves by waiting.
        world.advance(Duration::from_secs(400));

        // The door that does not run the settlement path.
        if world.table_state().phase.hand_in_progress() {
            for who in &actors {
                match world.abandon_stuck_hand(*who) {
                    Ok(n) => {
                        log.push(format!("round {round}: abandon_stuck_hand returned {n} e8s"));
                        break;
                    }
                    Err(OpError::Trap(m)) => {
                        log.push(format!("round {round}: abandon_stuck_hand TRAPPED: {m}"))
                    }
                    Err(OpError::Err(_)) => {}
                }
            }
        }

        // Stacks out of the seats.
        for who in &actors {
            match world.cash_out(*who) {
                Ok(n) if n > 0 => log.push(format!("round {round}: cash_out {n}")),
                Err(OpError::Trap(m)) => log.push(format!("round {round}: cash_out TRAPPED: {m}")),
                _ => {}
            }
        }
        // Escrow out to the ledger. The withdrawal cooldown is one per actor per
        // 60 s, and the loop advances 400 s per round, so one call each is right.
        for who in &actors {
            let bal = world.get_balance(*who);
            if bal > 20_000 {
                match world.withdraw(*who, bal) {
                    Ok(n) => log.push(format!("round {round}: withdraw {bal} -> balance {n}")),
                    Err(OpError::Trap(m)) => {
                        log.push(format!("round {round}: withdraw TRAPPED: {m}"))
                    }
                    Err(OpError::Err(e)) => {
                        log.push(format!("round {round}: withdraw {bal} refused: {e}"))
                    }
                }
            }
        }
    }

    let after = world.snapshot();
    let returned = after
        .actor_wallets
        .iter()
        .map(|(p, v)| {
            (
                *p,
                v.saturating_sub(wallets_before.get(p).copied().unwrap_or(0)),
            )
        })
        .collect();

    DrainReport {
        owed_before,
        owed_after: after.internal_total(),
        returned_to_wallets: returned,
        log,
    }
}

/// M9 as an assertion over a finished [`drain`].
///
/// Dust below the ledger's own minimum withdrawal is not a violation: it is an
/// economic floor, it is documented as FINDING 11 / E-12, and no amount of
/// engineering makes an amount smaller than the transfer fee movable.
pub fn check_drain(report: &DrainReport, phase: &crate::table_api::GamePhase) -> Vec<Violation> {
    /// `Currency::ICP.min_withdrawal()` in the canister, plus one fee. Below this
    /// the ledger itself refuses.
    const UNMOVABLE_DUST_E8S: u64 = 20_000;
    if report.owed_after <= UNMOVABLE_DUST_E8S {
        return Vec::new();
    }
    vec![Violation::new(
        Invariant::M9FundReachability,
        "money_left_behind_after_drain",
        Severity::FundsUnreachable,
        report.owed_after as i128,
        phase,
        format!(
            "after every legal player-side exit was driven to exhaustion, the canister still owes \
             {} of the {} e8s it started with, and no player call can move it. Drain transcript:\n  {}",
            report.owed_after,
            report.owed_before,
            report.log.join("\n  ")
        ),
    )]
}

/// Convenience for the fuzz loop: the phase to tag a drain violation with.
pub fn drain_and_check(world: &mut World) -> (DrainReport, Vec<Violation>) {
    let report = drain(world);
    let phase = world.table_state().phase;
    let vs = check_drain(&report, &phase);
    let _ = phase_of;
    (report, vs)
}
