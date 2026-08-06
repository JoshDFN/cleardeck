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
    /// `icrc1_balance_of(table, None)` afterwards. **The fact, as opposed to the
    /// claim.** Every other number in this struct is the canister describing
    /// itself; this one is the ledger describing the canister.
    pub ledger_main_after: u64,
    /// `icrc1_balance_of(table, deposit_subaccount(actor))` summed, afterwards.
    /// **The second fact, and the one no drain in this project measured until
    /// wave 8.** A table that has been emptied of chips and escrow while its own
    /// published deposit addresses still hold a player's money is not an empty
    /// table (docs/SECURITY-FINDINGS.md FINDING 28).
    pub ledger_deposit_subaccounts_after: u64,
    /// Money the harness pushed straight into the canister's main account with a
    /// raw transfer and has not asked it to credit yet, plus money it pushed into
    /// deposit subaccounts and has not asked the canister to look at.
    pub uncredited_raw: u64,
    /// Ledger money each actor gained, net of transfer fees.
    pub returned_to_wallets: BTreeMap<Principal, u64>,
    /// Calls the drain had to make, in order, for the transcript.
    pub log: Vec<String>,
}

impl DrainReport {
    /// The canister says it owes nobody anything.
    ///
    /// **This is a claim, not a fact, and FINDING 07 is what happens when it is
    /// mistaken for one.** A controller call that deletes seated chips satisfies it
    /// exactly, because it changes the quantity being measured rather than the
    /// money. Pair it with [`DrainReport::nothing_orphaned`], which the ledger
    /// answers, or use [`DrainReport::table_is_really_empty`].
    pub fn fully_drained(&self) -> bool {
        self.owed_after == 0
    }

    /// Money sitting in the canister's main ledger account that it owes to NOBODY.
    ///
    /// ClearDeck takes no rake, and every boundary-crossing path moves the ledger
    /// balance and the internal total by the same amount:
    ///
    /// * `deposit`: main `+a`, escrow `+a` (the payer pays the ledger fee);
    /// * `claim_external_deposit`: main `+(a-fee)`, escrow `+(a-fee)`;
    /// * `withdraw`: exactly `a` leaves the main account, escrow `-a`, and the
    ///   ledger burns the fee out of the amount in flight.
    ///
    /// So there is no such thing as a fee this canister "legitimately absorbs" into
    /// its main account: after a complete drain the honest value here is ZERO.
    ///
    /// # Deposit subaccounts are IN, and the old comment here was the defect
    ///
    /// This used to read: *"Deposit SUBACCOUNT balances are deliberately excluded.
    /// That money is not orphaned -- `claim_external_deposit` reaches it and only
    /// its owner can call that."* Both clauses were false in the state that
    /// mattered. `claim_external_deposit` refused with *"No claimable balance"*
    /// while the money sat at exactly that address, no balance surface reported a
    /// single e8 of it, and at or below the transfer fee nothing could move it at
    /// all. An exclusion argued from what a method is SUPPOSED to reach is not a
    /// measurement (docs/SECURITY-FINDINGS.md FINDING 21, FINDING 28).
    ///
    /// So both accounts are on the ledger side, and the canister's own
    /// deposit-custody figure is on the claims side via `owed_after`: subaccount
    /// money the canister KNOWS about is owed, subaccount money it does not know
    /// about is orphaned. That is the distinction the drain has to be able to make.
    pub fn orphaned_e8s(&self) -> u64 {
        self.ledger_holdings_after()
            .saturating_sub(self.owed_after)
            .saturating_sub(self.uncredited_raw)
    }

    /// Every e8 the ledger says the canister holds, across every account it owns.
    pub fn ledger_holdings_after(&self) -> u64 {
        self.ledger_main_after
            .saturating_add(self.ledger_deposit_subaccounts_after)
    }

    /// Nothing in any account the canister owns belongs to nobody.
    pub fn nothing_orphaned(&self) -> bool {
        self.orphaned_e8s() <= ORPHAN_TOLERANCE_E8S
    }

    /// The honest end-state: the canister neither claims to owe anything nor holds
    /// anything it cannot name an owner for.
    ///
    /// `fully_drained()` reads `owed_after`, which is `Snapshot::internal_total()`,
    /// which now includes the canister's deposit-subaccount custody -- so a table
    /// still holding a player's money at an address it published reports NOT
    /// empty, whether or not it knows about it: if it knows, `owed_after` is
    /// non-zero; if it does not, `orphaned_e8s()` is.
    pub fn table_is_really_empty(&self) -> bool {
        self.fully_drained() && self.nothing_orphaned()
    }
}

/// How much unowned money in the main account is not a finding.
///
/// ZERO. Stated as a named constant rather than a bare `> 0` so that anybody who
/// ever wants to relax it has to change a line that says what it is giving up.
/// The dust exemption in [`check_drain`] is about money the canister still OWES
/// and cannot move (FINDING 11 / E-12); this is about money it owes to nobody, and
/// no economic floor makes that acceptable.
pub const ORPHAN_TOLERANCE_E8S: u64 = 0;

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
///
/// # The deposit-subaccount pass runs FIRST, before the early exit
///
/// The loop's exit condition is "the canister says it owes nothing", and a
/// canister that has never looked at its own deposit addresses says exactly that
/// while holding a player's money at one. Measured: a table holding 9,999 e8s of
/// alice's at the address it published to her exited at round 0 with an empty
/// transcript and reported `table_is_really_empty`. So the drain makes the
/// canister LOOK before it is allowed to conclude anything
/// (docs/SECURITY-FINDINGS.md FINDING 28).
/// Make the canister READ every actor's deposit address, then sweep whatever is
/// there into escrow.
///
/// Both calls are player-callable with no privilege check, so they belong in a
/// drain exactly as `cash_out` does. `refresh_deposit_custody` runs even when
/// there is nothing to sweep, because the drain's verdict is "this canister holds
/// nothing that belongs to anybody" and a canister that has not looked at an
/// account it published cannot support that sentence.
fn sweep_deposit_addresses(
    world: &mut World,
    actors: &[Principal],
    log: &mut Vec<String>,
    stage: &str,
) {
    for who in actors {
        match world.refresh_deposit_custody(*who) {
            Err(OpError::Trap(m)) => {
                log.push(format!("{stage}: refresh_deposit_custody TRAPPED: {m}"))
            }
            Ok(c) if c.observed_amount > 0 => log.push(format!(
                "{stage}: deposit address of {who} holds {} (sweepable={})",
                c.observed_amount, c.sweepable
            )),
            _ => {}
        }
        match world.claim_external_deposit(*who) {
            Ok(n) => log.push(format!("{stage}: claim_external_deposit -> escrow {n}")),
            Err(OpError::Trap(m)) => {
                log.push(format!("{stage}: claim_external_deposit TRAPPED: {m}"))
            }
            Err(OpError::Err(_)) => {}
        }
    }
}

pub fn drain(world: &mut World) -> DrainReport {
    let before = world.snapshot();
    let owed_before = before.internal_total();
    let mut log: Vec<String> = Vec::new();
    let wallets_before: BTreeMap<Principal, u64> = before.actor_wallets.clone();
    let actors: Vec<Principal> = world.actors.iter().map(|a| a.principal).collect();

    sweep_deposit_addresses(world, &actors, &mut log, "pre");

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

        // Money that arrived at a published deposit address, into escrow. Only
        // when the canister's own books say there is some: the pre-pass above has
        // already made it look once, so a second sweep is only worth its messages
        // if something is there.
        if world.snapshot().canister_unswept_deposits > 0 {
            sweep_deposit_addresses(world, &actors, &mut log, &format!("round {round}"));
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
        ledger_main_after: after.ledger_main,
        ledger_deposit_subaccounts_after: after.ledger_deposit_subaccounts,
        uncredited_raw: crate::invariants::Exemptions::of(world).total(),
        returned_to_wallets: returned,
        log,
    }
}

/// M9's standing leg: **the canister may never hold money it owes to nobody.**
///
/// Read off any snapshot, and anchored to `icrc1_balance_of` rather than to the
/// canister's own books, which is the whole point. See
/// [`Severity::OrphanedCustody`] and docs/SECURITY-FINDINGS.md FINDING 07.
pub fn check_no_orphaned_custody(
    snap: &crate::world::Snapshot,
    exempt: impl Into<crate::invariants::Exemptions>,
) -> Vec<Violation> {
    let exempt = exempt.into();
    let claims = snap.internal_total();
    let holdings = snap.ledger_holdings();
    let orphaned = holdings
        .saturating_sub(claims)
        .saturating_sub(exempt.total());
    if orphaned <= ORPHAN_TOLERANCE_E8S {
        return Vec::new();
    }
    vec![Violation::new(
        Invariant::M9FundReachability,
        "money_belongs_to_nobody",
        Severity::OrphanedCustody,
        orphaned as i128,
        phase_of(&snap.table),
        format!(
            "the ledger says this canister holds {} e8s across every account it owns (main {} + \
             deposit subaccounts {}) and the canister says it owes {} (escrow {} + chips {} + \
             pot {} + unswept deposit custody {}), with {} of raw deposits not yet credited and \
             {} in subaccounts the harness has not yet asked it to look at. That leaves {} e8s \
             inside a canister that owes them to NOBODY: no player call can withdraw them, no \
             controller call can return them, and no invariant anchored to what the canister \
             SAYS it owes will ever see them. ClearDeck takes no rake, so the honest value here \
             is zero. docs/SECURITY-FINDINGS.md FINDING 07, FINDING 21, FINDING 28.",
            holdings,
            snap.ledger_main,
            snap.ledger_deposit_subaccounts,
            claims,
            snap.escrow_total,
            snap.chips_total,
            snap.table.pot,
            snap.canister_unswept_deposits,
            exempt.uncredited_raw,
            exempt.unobserved_total(),
            orphaned
        ),
    )]
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
    let mut out = Vec::new();

    if report.owed_after > UNMOVABLE_DUST_E8S {
        out.push(Violation::new(
            Invariant::M9FundReachability,
            "money_left_behind_after_drain",
            Severity::FundsUnreachable,
            report.owed_after as i128,
            phase,
            format!(
                "after every legal player-side exit was driven to exhaustion, the canister still \
                 owes {} of the {} e8s it started with, and no player call can move it. Drain \
                 transcript:\n  {}",
                report.owed_after,
                report.owed_before,
                report.log.join("\n  ")
            ),
        ));
    }

    // THE LEG THE AUDITOR'S ATTACK WALKED PAST (docs/SECURITY-FINDINGS.md
    // FINDING 07). The check above asks the canister whether it still owes
    // anything, and the attack works by making it stop owing. This one asks the
    // LEDGER how much the canister is holding, and subtracts only what somebody
    // can name an owner for. `reset_table` on a funded table leaves this at the
    // full value of the destroyed chips while `owed_after` reads a clean zero.
    let orphaned = report.orphaned_e8s();
    if orphaned > ORPHAN_TOLERANCE_E8S {
        out.push(Violation::new(
            Invariant::M9FundReachability,
            "drain_left_money_that_belongs_to_nobody",
            Severity::OrphanedCustody,
            orphaned as i128,
            phase,
            format!(
                "the drain finished and the canister says it owes {} e8s -- but the LEDGER says \
                 it is holding {} e8s across every account it owns, of which {} is raw deposits \
                 and unread deposit subaccounts the harness has not asked it about. {} e8s are \
                 therefore inside a canister that owes them to nobody: unwithdrawable by every \
                 player and unreturnable by every controller. A drain that only asks the canister \
                 what it owes reports `fully_drained` here, which is exactly how FINDING 07 \
                 survived four waves. Drain transcript:\n  {}",
                report.owed_after,
                report.ledger_holdings_after(),
                report.uncredited_raw,
                orphaned,
                report.log.join("\n  ")
            ),
        ));
    }

    out
}

/// Convenience for the fuzz loop: the phase to tag a drain violation with.
pub fn drain_and_check(world: &mut World) -> (DrainReport, Vec<Violation>) {
    let report = drain(world);
    let phase = world.table_state().phase;
    let vs = check_drain(&report, &phase);
    let _ = phase_of;
    (report, vs)
}
