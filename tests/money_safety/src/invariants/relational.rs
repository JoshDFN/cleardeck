//! The invariants that need more than one observation.
//!
//! M3 (NO RAKE) compares the table before and after a hand. M5 (UPGRADE
//! DURABILITY) compares it across an `install_code --mode upgrade`. M6 (NO DOUBLE
//! PAY) compares it across one money-moving call. The self-report matcher reads the
//! canister's own log.
//!
//! Split out of `super` so neither file grows past the point where a reader stops
//! reading. Everything here is re-exported by `super`, so callers see one module.

use crate::table_api::GamePhase;
use crate::world::Snapshot;

use super::{phase_of, Invariant, Severity, Violation};

// ---------------------------------------------------------------------------
// M3 -- no rake, over a hand
// ---------------------------------------------------------------------------

/// **DID MONEY CROSS THE CANISTER'S BOUNDARY BETWEEN THESE TWO OBSERVATIONS?**
///
/// [`check_no_rake`] is only meaningful over a window in which nothing external
/// moved money, and this is the test for that. It has to be asked of the SAME set
/// of accounts [`Snapshot::internal_total`] is built from, or the two sides of the
/// equation are drawn from different worlds.
///
/// # Why this is not `ledger_main`, which is what it used to be
///
/// The canister owns **two** classes of ledger account: its main account, and one
/// published deposit subaccount per principal. `internal_total()` has counted the
/// second class since wave 8 (`canister_unswept_deposits`, added because a canister
/// holding 5 ICP for a player at an address it had published to them reported that
/// it owed nobody anything -- FINDING 21 / FINDING 28). The window guard was never
/// updated with it, so it went on asking only about the MAIN account.
///
/// The consequence is a **false conviction**, and it was filed as a real one. A
/// third party pays 10,000 e8s into their own published deposit address while a
/// hand is running. `ledger_main` does not move -- the money is in a subaccount --
/// so the window was admitted; `canister_unswept_deposits` does move, so
/// `internal_total` grew by 10,000 across the hand; and M3 reported
/// `FundCreation`, *"value at the table changed across a hand with no external
/// money movement"*, about 10,000 e8s that had just arrived from outside. That is
/// [FINDING 44](../../../../docs/SECURITY-FINDINGS.md), and its 7-op minimal
/// reproducer is literally `ExternalDepositThenClaim { amount: 10_000 }` inside a
/// hand. It reproduces identically on the wave-12 canister, so it was never a
/// canister defect at all.
///
/// An instrument that convicts the innocent is not a safe instrument: the register
/// gained a `high` fund-creation finding against a module that had not created
/// anything, and a real red in the same row now has to be told apart from it.
///
/// # What this costs
///
/// Windows in which subaccount money moved are now SKIPPED by M3 rather than
/// mis-decided, exactly as windows in which main-account money moved always were.
/// A genuine chip creation that happened to coincide with a deposit is therefore
/// not seen *by M3* in that window -- it is still seen by M1 conservation, M8
/// attribution, M9 reachability, M11 outcome and the settlement oracle, none of
/// which are gated on this predicate.
pub fn external_money_moved(before: &Snapshot, after: &Snapshot) -> bool {
    before.ledger_holdings() != after.ledger_holdings()
}

/// M3 NO RAKE, measured over a completed hand.
///
/// `before` must be taken with the table idle (`WaitingForPlayers` or
/// `HandComplete`, pot 0) and `after` once the next `HandComplete` is reached,
/// with no deposit / withdraw / buy-in / cash-out in between -- which is
/// [`external_money_moved`]'s question, and asking it of the wrong accounts is
/// FINDING 44. Under those conditions the total value at the table cannot change:
/// the pot is chips taken out of stacks and handed back to stacks. Any change is
/// the house taking (or giving) something.
pub fn check_no_rake(before: &Snapshot, after: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let b = before.internal_total();
    let a = after.internal_total();
    if a != b {
        let delta = a as i128 - b as i128;
        out.push(Violation::new(
            Invariant::M3NoRake,
            "value_conserved_across_hand",
            if delta < 0 {
                Severity::FundDestruction
            } else {
                Severity::FundCreation
            },
            delta,
            phase_of(&after.table),
            format!(
                "value at the table changed across a hand with no external money movement: \
                 before escrow={} chips={} pot={} (total {}), after escrow={} chips={} pot={} \
                 (total {}), delta={}",
                before.escrow_total,
                before.chips_total,
                before.table.pot,
                b,
                after.escrow_total,
                after.chips_total,
                after.table.pot,
                a,
                delta
            ),
        ));
    }
    out
}

/// M3 NO RAKE, in its sharpest form: the amounts the canister recorded as
/// awarded for hand `hand_number` must sum to the money that was collected.
///
/// IMPORTANT about the basis. `pot_collected` is normally read as the sum of the
/// seated players' `total_bet_this_hand`, which is exact ONLY if no seat was
/// vacated during the hand. `leave_table` and a post-fold `cash_out` remove the
/// seat AND the record of what that player put in, so after one of those the sum
/// UNDERSTATES what was collected and this check would report money appearing from
/// nowhere when in fact it is FINDING 05's orphaned stake. Callers must therefore
/// only apply this check across a hand whose seated set did not change; the
/// conservation form ([`check_no_rake`]) covers the rest and needs no attribution.
pub fn check_hand_payout_total(
    hand_number: u64,
    pot_collected: u64,
    awarded: u64,
    phase: &GamePhase,
) -> Vec<Violation> {
    if awarded == pot_collected {
        return Vec::new();
    }
    let delta = awarded as i128 - pot_collected as i128;
    vec![Violation::new(
        Invariant::M3NoRake,
        "awarded_equals_collected",
        if delta < 0 {
            Severity::FundDestruction
        } else {
            Severity::FundCreation
        },
        delta,
        phase,
        format!(
            "hand {hand_number}: chips awarded = {awarded} but chips wagered = {pot_collected} \
             (delta {delta}). ClearDeck takes no rake, so these must be equal to the e8."
        ),
    )]
}

// ---------------------------------------------------------------------------
// M5 -- upgrade durability
// ---------------------------------------------------------------------------

/// M5 UPGRADE DURABILITY: everything the canister is responsible for must be
/// byte-identical across an upgrade. The two ledger-side fields are excluded
/// only because an upgrade cannot touch them.
pub fn check_upgrade_durability(before: &Snapshot, after: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();

    if before.escrow != after.escrow {
        out.push(Violation::new(
            Invariant::M5UpgradeDurability,
            "escrow_survives_upgrade",
            Severity::DurabilityLoss,
            after.escrow_total as i128 - before.escrow_total as i128,
            phase_of(&after.table),
            format!(
                "escrow map changed across the upgrade: before={:?} after={:?}",
                before.escrow, after.escrow
            ),
        ));
    }

    if before.table != after.table {
        // Report the money-bearing differences explicitly; a raw struct diff of
        // a 52-card deck is unreadable.
        let mut diffs: Vec<String> = Vec::new();
        if before.table.pot != after.table.pot {
            diffs.push(format!(
                "pot {} -> {}",
                before.table.pot, after.table.pot
            ));
        }
        if before.table.side_pots != after.table.side_pots {
            diffs.push(format!(
                "side_pots {:?} -> {:?}",
                before.table.side_pots, after.table.side_pots
            ));
        }
        if before.table.hand_number != after.table.hand_number {
            diffs.push(format!(
                "hand_number {} -> {}",
                before.table.hand_number, after.table.hand_number
            ));
        }
        if before.table.phase != after.table.phase {
            diffs.push(format!(
                "phase {:?} -> {:?}",
                before.table.phase, after.table.phase
            ));
        }
        if before.table.players != after.table.players {
            for (i, (bp, ap)) in before
                .table
                .players
                .iter()
                .zip(after.table.players.iter())
                .enumerate()
            {
                if bp != ap {
                    diffs.push(format!("seat {i}: {bp:?} -> {ap:?}"));
                }
            }
        }
        if before.table.deck != after.table.deck {
            diffs.push("deck changed".to_string());
        }
        if before.table.community_cards != after.table.community_cards {
            diffs.push(format!(
                "community_cards {:?} -> {:?}",
                before.table.community_cards, after.table.community_cards
            ));
        }
        if before.table.action_timer != after.table.action_timer {
            diffs.push(format!(
                "action_timer {:?} -> {:?}",
                before.table.action_timer, after.table.action_timer
            ));
        }
        if diffs.is_empty() {
            diffs.push(format!(
                "table state differs: before={:?} after={:?}",
                before.table, after.table
            ));
        }
        out.push(Violation::new(
            Invariant::M5UpgradeDurability,
            "table_state_survives_upgrade",
            Severity::DurabilityLoss,
            after.internal_total() as i128 - before.internal_total() as i128,
            phase_of(&after.table),
            format!("table state changed across the upgrade: {}", diffs.join("; ")),
        ));
    }

    if before.chips_total != after.chips_total {
        out.push(Violation::new(
            Invariant::M5UpgradeDurability,
            "chips_survive_upgrade",
            Severity::DurabilityLoss,
            after.chips_total as i128 - before.chips_total as i128,
            phase_of(&after.table),
            format!(
                "total chips at table changed across the upgrade: {} -> {}",
                before.chips_total, after.chips_total
            ),
        ));
    }

    out
}

// ---------------------------------------------------------------------------
// M6 -- no double pay
// ---------------------------------------------------------------------------

/// M6, withdrawal leg: a successful `withdraw(amount)` debits escrow by exactly
/// `amount`, moves exactly `amount` out of the canister's ledger account, and
/// delivers exactly `amount - fee` to the player. Once.
pub fn check_withdraw_effect(
    who: candid::Principal,
    amount: u64,
    fee: u64,
    before: &Snapshot,
    after: &Snapshot,
) -> Vec<Violation> {
    let mut out = Vec::new();
    let escrow_before = before.escrow.get(&who).copied().unwrap_or(0);
    let escrow_after = after.escrow.get(&who).copied().unwrap_or(0);
    let wallet_before = before.actor_wallets.get(&who).copied().unwrap_or(0);
    let wallet_after = after.actor_wallets.get(&who).copied().unwrap_or(0);

    let escrow_debit = escrow_before as i128 - escrow_after as i128;
    let wallet_credit = wallet_after as i128 - wallet_before as i128;
    let main_debit = before.ledger_main as i128 - after.ledger_main as i128;

    if escrow_debit != amount as i128 {
        out.push(Violation::new(
            Invariant::M6NoDoublePay,
            "withdraw_debits_escrow_once",
            Severity::DoublePay,
            escrow_debit - amount as i128,
            phase_of(&after.table),
            format!("withdraw({amount}) debited escrow by {escrow_debit}, expected {amount}"),
        ));
    }
    if wallet_credit != (amount - fee) as i128 {
        out.push(Violation::new(
            Invariant::M6NoDoublePay,
            "withdraw_delivers_once",
            Severity::DoublePay,
            wallet_credit - (amount - fee) as i128,
            phase_of(&after.table),
            format!(
                "withdraw({amount}) delivered {wallet_credit} to the player, expected {}",
                amount - fee
            ),
        ));
    }
    if main_debit != amount as i128 {
        out.push(Violation::new(
            Invariant::M6NoDoublePay,
            "withdraw_moves_ledger_once",
            Severity::DoublePay,
            main_debit - amount as i128,
            phase_of(&after.table),
            format!(
                "withdraw({amount}) moved {main_debit} out of the canister's ledger account, \
                 expected {amount} ({} to the player + {fee} burnt as the transfer fee)",
                amount - fee
            ),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// self-report: the canister telling you it is inconsistent
// ---------------------------------------------------------------------------

/// The engine detects some of its own accounting inconsistencies and then logs
/// them and settles anyway. `calculate_side_pots` prints
/// `BUG: Side pots (...) exceed total pot (...). Capping to pot amount.` and pays
/// out of the capped figure; `pre_upgrade` prints
/// `CRITICAL: Failed to save state to stable memory` and lets the upgrade proceed.
///
/// On a custody canister a detected inconsistency should trap, not warn, so any
/// such line is a violation.
///
/// # Why this used to be unable to fail a run (docs/DEFECTS.md H-02)
///
/// Every matching line was hardcoded to `Severity::BreakdownDrift`, and
/// `is_documented_defect()` returned true for `BreakdownDrift`, and `tests/fuzz.rs`
/// only fails on non-documented severities. So the canister could announce its own
/// inconsistency in the log and the run still reported `ok`. The same branch
/// swallowed `pre_upgrade`'s `CRITICAL: Failed to save state to stable memory` --
/// a TOTAL-FUND-LOSS-ON-UPGRADE event -- into the same non-blocking bucket.
///
/// Now the pattern is split, and since E-03 was fixed the tolerated list
/// ([`crate::documented::TOLERATED_SELF_REPORTS`]) is EMPTY: every `BUG:` line and
/// every `CRITICAL:` line is [`Severity::SelfReportedFailure`], which
/// [`crate::documented`] can never excuse, so it stops the run. The only line the
/// payout path can still write is `CRITICAL: pot accounting disagreement`, and
/// honest play cannot produce it.
/// `WARNING:` is matched as well, and that was added by the coherence pass for a
/// reason worth keeping in view. The payout fix wrote E-36's dual-stake condition
/// as `WARNING: seat {} carries both a live stake and a departed stake` rather than
/// as `CRITICAL:`, defensibly -- nothing about the engine's ACCOUNTING is
/// inconsistent there -- but the effect was that 296 occurrences of a real open
/// defect executing against the real canister were reported as
/// `0 documented finding(s)`. Tolerance had moved into a string the classifier did
/// not read, which is precisely the H-03 pathology this module exists to prevent.
/// A `WARNING:` line is now a finding like any other; the ones we accept are named
/// in [`crate::documented::TOLERATED_SELF_REPORTS`] where they can be counted and
/// deleted.
pub fn check_self_reported_inconsistency(logs: &[String], phase: &GamePhase) -> Vec<Violation> {
    logs.iter()
        .filter(|line| {
            line.contains("BUG:") || line.contains("CRITICAL:") || line.contains("WARNING:")
        })
        .map(|line| {
            let registered = !line.contains("CRITICAL:")
                && crate::documented::TOLERATED_SELF_REPORTS
                    .iter()
                    .any(|tolerated| line.contains(tolerated));
            if registered {
                Violation::new(
                    Invariant::M1bPotBreakdown,
                    "canister_reports_its_own_inconsistency",
                    Severity::BreakdownDrift,
                    0,
                    phase,
                    format!(
                        "the canister logged a self-report that is on the tolerated list and \
                         settled anyway: {line}"
                    ),
                )
            } else {
                Violation::new(
                    Invariant::M1bPotBreakdown,
                    "canister_reports_an_unregistered_failure",
                    Severity::SelfReportedFailure,
                    0,
                    phase,
                    format!(
                        "the canister logged a failure that is NOT in the documented register and \
                         carried on instead of trapping: {line}"
                    ),
                )
            }
        })
        .collect()
}

/// M3 NO RAKE, in the one form that survives E-01: the money paid out must equal
/// the payout BASIS the canister itself chose.
///
/// This is the assertion that a house rake cannot pass. E-01 makes the basis wrong
/// (it is the frozen pre-flop `side_pots` breakdown rather than the pot), and while
/// that is true, "awarded < collected" cannot distinguish E-01 from an operator
/// skimming every pot -- which is precisely how a planted 1% rake passed the fuzz
/// suite. But whatever basis the engine picks, it must pay out ALL of it: a rake is
/// visible as awarded < basis even when awarded < collected is expected.
///
/// `basis` is the sum of `side_pots` observed while the hand was still live (the
/// engine clears them at `HandComplete`), or `pot` when the engine used the
/// no-side-pots branch.
pub fn check_awarded_equals_payout_basis(
    hand_number: u64,
    basis: u64,
    awarded: u64,
    phase: &GamePhase,
) -> Vec<Violation> {
    if awarded == basis {
        return Vec::new();
    }
    let delta = awarded as i128 - basis as i128;
    vec![Violation::new(
        Invariant::M3NoRake,
        "awarded_equals_payout_basis",
        Severity::RakeTaken,
        delta,
        phase,
        format!(
            "hand {hand_number}: the canister's own payout basis was {basis} e8s but it awarded \
             {awarded} (delta {delta}). Whatever basis the engine picks it must pay out all of it; \
             a shortfall here is money collected from players and kept, and it is NOT explained by \
             E-01, which affects how the basis is COMPUTED and not whether it is fully paid."
        ),
    )]
}

/// M6, deposit leg: a call that must NOT credit anything did not change escrow.
pub fn check_no_credit(
    who: candid::Principal,
    what: &str,
    before: &Snapshot,
    after: &Snapshot,
) -> Vec<Violation> {
    let b = before.escrow.get(&who).copied().unwrap_or(0);
    let a = after.escrow.get(&who).copied().unwrap_or(0);
    if a == b {
        return Vec::new();
    }
    vec![Violation::new(
        Invariant::M6NoDoublePay,
        "no_second_credit",
        Severity::DoublePay,
        a as i128 - b as i128,
        phase_of(&after.table),
        format!("{what} credited {who} a second time: escrow {b} -> {a}"),
    )]
}
