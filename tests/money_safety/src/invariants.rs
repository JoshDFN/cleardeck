//! The money invariants, as named assertions.
//!
//! Each one is a function that returns the violations it found instead of
//! panicking, so the fuzzer can keep exploring past a violation it already knows
//! about and still fail hard on a new one. The hand-written tests in
//! `tests/invariants.rs` call the same functions and assert on the result, so
//! there is exactly one definition of each invariant in the codebase.
//!
//! The anchor for the money invariants is the LEDGER, not the canister's own
//! bookkeeping. `icrc1_balance_of(table)` is a fact about the outside world; the
//! escrow map and the chip stacks are the canister's claims about it. Comparing
//! the canister against itself would make M1 vacuous.

use serde::Serialize;

use crate::table_api::{GamePhase, TableState};
use crate::world::{Snapshot, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Invariant {
    /// M1 CONSERVATION. No chips created, none destroyed.
    ///
    ///   ledger_main == escrow + chips + pot + uncredited_raw_deposits
    ///
    /// Every path that moves money across the canister boundary moves the ledger
    /// balance and the internal total by the same amount:
    ///   * `deposit`: main +a, escrow +a
    ///   * `claim_external_deposit`: main +(sub-fee), escrow +(sub-fee)
    ///   * `withdraw`: main -a (a-fee out, fee burnt), escrow -a
    ///   * `notify_deposit`: main unchanged now, main was +a earlier by a raw
    ///     transfer -- which is what `uncredited_raw_deposits` accounts for.
    /// So the identity is exact, to the e8, and it is exact BECAUSE ClearDeck
    /// takes no rake.
    M1Conservation,

    /// M1b POT BREAKDOWN. `side_pots` is a breakdown of `pot`, so whenever it is
    /// populated it must sum to `pot`. A stale breakdown is a payout basis that
    /// disagrees with the money actually collected.
    M1bPotBreakdown,

    /// M2 LEDGER REALITY. The canister's real ledger balance is at least what it
    /// owes players. It can never be short.
    M2LedgerReality,

    /// M3 NO RAKE. Over a completed hand, chips awarded == chips wagered. The
    /// house takes exactly zero.
    M3NoRake,

    /// M4 NO NEGATIVE / NO OVERFLOW. Nothing wraps; `saturating_*` never
    /// silently swallows a real deficit; hostile amounts are rejected rather
    /// than clamped.
    M4NoNegativeNoOverflow,

    /// M5 UPGRADE DURABILITY. An upgrade preserves every balance, every chip
    /// stack and the in-progress hand: nothing lost, nothing duplicated.
    M5UpgradeDurability,

    /// M6 NO DOUBLE PAY. A pot is awarded once; a withdrawal is paid at most
    /// once; a deposit block or allowance is credited at most once.
    M6NoDoublePay,
}

impl Invariant {
    pub fn name(&self) -> &'static str {
        match self {
            Invariant::M1Conservation => "M1_CONSERVATION",
            Invariant::M1bPotBreakdown => "M1b_POT_BREAKDOWN",
            Invariant::M2LedgerReality => "M2_LEDGER_REALITY",
            Invariant::M3NoRake => "M3_NO_RAKE",
            Invariant::M4NoNegativeNoOverflow => "M4_NO_NEGATIVE_NO_OVERFLOW",
            Invariant::M5UpgradeDurability => "M5_UPGRADE_DURABILITY",
            Invariant::M6NoDoublePay => "M6_NO_DOUBLE_PAY",
        }
    }
}

/// How bad a violation is, and -- critically -- whether it is one of the defects
/// already documented in `docs/SECURITY-FINDINGS.md`.
///
/// The distinction that matters for a custody canister is DIRECTION.
/// `FundDestruction` strands money inside the canister: bad, already documented,
/// and NOT exploitable to take somebody else's funds. `FundCreation` means the
/// canister believes it owes more than it holds, i.e. someone can withdraw money
/// that was never deposited. That is a fund-theft primitive and it is NOT a
/// known defect, so the fuzzer must fail on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    /// The canister owes MORE than it holds. Chips from nothing. FAIL.
    FundCreation,
    /// A pot, withdrawal or deposit was settled twice. FAIL.
    DoublePay,
    /// An upgrade lost or duplicated state. FAIL.
    DurabilityLoss,
    /// The canister holds MORE than it owes: money stranded, unrecoverable
    /// (there is no admin withdrawal path). Documented as FINDING 01/02.
    FundDestruction,
    /// `side_pots` disagrees with `pot`. The mechanism behind FINDING 01.
    BreakdownDrift,
}

impl Severity {
    /// Whether a violation of this kind is already characterised in
    /// `docs/SECURITY-FINDINGS.md`. Anything else is new and must fail the run.
    pub fn is_documented_defect(&self) -> bool {
        matches!(self, Severity::FundDestruction | Severity::BreakdownDrift)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub invariant: Invariant,
    /// Which concrete assertion inside the invariant fired. An invariant can be
    /// approached from more than one direction (M3, for instance, is checked both
    /// as value-conservation across a hand and as awarded-equals-collected), and
    /// the report must not merge them.
    pub check: &'static str,
    pub severity: Severity,
    /// Signed size of the discrepancy in e8s, from the canister's point of view:
    /// positive = the canister holds more than it owes.
    pub delta_e8s: i128,
    pub detail: String,
    /// Phase the table was in when the violation was observed.
    pub phase: String,
}

impl Violation {
    fn new(
        invariant: Invariant,
        check: &'static str,
        severity: Severity,
        delta_e8s: i128,
        phase: &GamePhase,
        detail: String,
    ) -> Self {
        Self {
            invariant,
            check,
            severity,
            delta_e8s,
            detail,
            phase: format!("{phase:?}"),
        }
    }

    /// A stable key for "the same kind of violation", used to deduplicate the
    /// fuzz report so a million steps do not produce a million identical rows.
    pub fn signature(&self) -> String {
        format!(
            "{}:{}|{:?}|{}|{}",
            self.invariant.name(),
            self.check,
            self.severity,
            self.phase,
            if self.delta_e8s >= 0 { "+" } else { "-" }
        )
    }
}

fn phase_of(t: &TableState) -> &GamePhase {
    &t.phase
}

// ---------------------------------------------------------------------------
// M1 / M2 -- ledger-anchored conservation
// ---------------------------------------------------------------------------

/// M1 CONSERVATION and M2 LEDGER REALITY, both read off the same snapshot.
pub fn check_conservation(snap: &Snapshot, uncredited_raw_deposits: u64) -> Vec<Violation> {
    let mut out = Vec::new();
    let internal = snap.internal_total();
    let expected_main = internal.saturating_add(uncredited_raw_deposits);

    // M2 first: being short is the worst possible state.
    if snap.ledger_main < internal {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "canister_is_short",
            Severity::FundCreation,
            snap.ledger_main as i128 - internal as i128,
            phase_of(&snap.table),
            format!(
                "canister is SHORT: ledger_main={} but it owes escrow={} + chips={} + pot={} = {}",
                snap.ledger_main, snap.escrow_total, snap.chips_total, snap.table.pot, internal
            ),
        ));
    }

    if snap.ledger_main != expected_main {
        let delta = snap.ledger_main as i128 - expected_main as i128;
        let severity = if delta > 0 {
            Severity::FundDestruction
        } else {
            Severity::FundCreation
        };
        out.push(Violation::new(
            Invariant::M1Conservation,
            "ledger_equals_owed",
            severity,
            delta,
            phase_of(&snap.table),
            format!(
                "ledger_main={} != escrow({}) + chips({}) + pot({}) + uncredited_raw({}) = {}; \
                 delta={} ({})",
                snap.ledger_main,
                snap.escrow_total,
                snap.chips_total,
                snap.table.pot,
                uncredited_raw_deposits,
                expected_main,
                delta,
                if delta > 0 {
                    "chips DESTROYED: money stranded in the canister that nobody owns"
                } else {
                    "chips CREATED: the canister owes money it does not hold"
                }
            ),
        ));
    }

    out
}

/// M1b POT BREAKDOWN. Two things must be true of the money in the middle:
///
///  1. `side_pots`, when populated, sums to `pot` -- it is a breakdown of `pot`,
///     not additional money.
///  2. mid-hand, `pot` equals the sum of the seated players' `total_bet_this_hand`
///     -- every e8 in the pot is attributed to somebody who is still at the table
///     and therefore still appears in the bet-level split.
///
/// (2) is the precondition for FINDING 05: when a seat is vacated mid-hand its
/// stake stays in `pot` but disappears from the contribution list, and the split
/// then hands the orphaned amount to the highest bet level, i.e. to the deepest
/// stack alone.
pub fn check_pot_breakdown(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let t = &snap.table;

    if !t.side_pots.is_empty() {
        let sum = t.side_pots_total();
        if sum != t.pot {
            out.push(Violation::new(
                Invariant::M1bPotBreakdown,
                "side_pots_sum_to_pot",
                Severity::BreakdownDrift,
                t.pot as i128 - sum as i128,
                phase_of(t),
                format!(
                    "sum(side_pots)={} != pot={} (side_pots={:?}); the payout basis disagrees \
                     with the money actually collected",
                    sum, t.pot, t.side_pots
                ),
            ));
        }
    }

    if t.phase.hand_in_progress() {
        let wagered = t.wagered_total();
        if t.pot != wagered {
            let delta = t.pot as i128 - wagered as i128;
            out.push(Violation::new(
                Invariant::M1bPotBreakdown,
                "pot_is_fully_attributed",
                if delta < 0 {
                    // Chips left stacks and never reached the pot.
                    Severity::FundDestruction
                } else {
                    Severity::BreakdownDrift
                },
                delta,
                phase_of(t),
                format!(
                    "pot={} but the seated players' total_bet_this_hand sums to {} (delta {}). {}",
                    t.pot,
                    wagered,
                    delta,
                    if delta > 0 {
                        "There is money in the pot that no seated player is credited with \
                         contributing, so the bet-level split cannot allocate it correctly."
                    } else {
                        "Chips left stacks without reaching the pot."
                    }
                ),
            ));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// M4 -- arithmetic sanity
// ---------------------------------------------------------------------------

/// M4 NO NEGATIVE / NO OVERFLOW, as far as it can be seen from a snapshot.
///
/// u64 cannot be negative, so the reachable failure modes are (a) a wrap, and
/// (b) a `saturating_*` that clamped and thereby hid a real deficit. Both show
/// up as an implausible magnitude or as an unchecked-sum disagreement.
pub fn check_arithmetic(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let t = &snap.table;

    // A checked re-sum: if the canister's saturating fold clamped, the two
    // disagree.
    let escrow_checked = snap
        .escrow
        .values()
        .try_fold(0u64, |a: u64, v| a.checked_add(*v));
    match escrow_checked {
        None => out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "escrow_sum_does_not_overflow",
            Severity::FundCreation,
            0,
            phase_of(t),
            "sum of escrow balances OVERFLOWS u64; admin_get_all_balances reports a \
             saturating_add total that hides it"
                .to_string(),
        )),
        Some(sum) if sum != snap.escrow_total => out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "escrow_total_is_exact",
            Severity::FundCreation,
            sum as i128 - snap.escrow_total as i128,
            phase_of(t),
            format!(
                "escrow total mismatch: canister reports {} but the entries sum to {}",
                snap.escrow_total, sum
            ),
        )),
        _ => {}
    }

    let chips_checked = t
        .seated()
        .try_fold(0u64, |a: u64, p| a.checked_add(p.chips));
    if chips_checked.is_none() {
        out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "chip_sum_does_not_overflow",
            Severity::FundCreation,
            0,
            phase_of(t),
            "sum of chip stacks OVERFLOWS u64".to_string(),
        ));
    }

    // Nothing anyone owns can exceed the money the canister actually holds.
    let holdings = snap
        .ledger_main
        .saturating_add(snap.ledger_deposit_subaccounts);
    for (who, bal) in &snap.escrow {
        if *bal > holdings {
            out.push(Violation::new(
                Invariant::M4NoNegativeNoOverflow,
                "escrow_within_holdings",
                Severity::FundCreation,
                *bal as i128 - holdings as i128,
                phase_of(t),
                format!(
                    "escrow balance of {who} is {bal}, more than the canister's entire ledger \
                     holdings {holdings}"
                ),
            ));
        }
    }
    for p in t.seated() {
        if p.chips > holdings {
            out.push(Violation::new(
                Invariant::M4NoNegativeNoOverflow,
                "chips_within_holdings",
                Severity::FundCreation,
                p.chips as i128 - holdings as i128,
                phase_of(t),
                format!(
                    "seat {} holds {} chips, more than the canister's entire ledger holdings {}",
                    p.seat, p.chips, holdings
                ),
            ));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// the composite point-in-time check
// ---------------------------------------------------------------------------

/// Every invariant that can be evaluated from a single observation: M1, M1b, M2,
/// M4. M3, M5 and M6 are relational and have their own entry points.
pub fn check_point_in_time(snap: &Snapshot, uncredited_raw_deposits: u64) -> Vec<Violation> {
    let mut out = check_conservation(snap, uncredited_raw_deposits);
    out.extend(check_pot_breakdown(snap));
    out.extend(check_arithmetic(snap));
    out
}

pub fn check_world(world: &World) -> Vec<Violation> {
    let snap = world.snapshot();
    check_point_in_time(&snap, world.uncredited_raw_deposits)
}

// ---------------------------------------------------------------------------
// M3 -- no rake, over a hand
// ---------------------------------------------------------------------------

/// M3 NO RAKE, measured over a completed hand.
///
/// `before` must be taken with the table idle (`WaitingForPlayers` or
/// `HandComplete`, pot 0) and `after` once the next `HandComplete` is reached,
/// with no deposit / withdraw / buy-in / cash-out in between. Under those
/// conditions the total value at the table cannot change: the pot is chips taken
/// out of stacks and handed back to stacks. Any change is the house taking (or
/// giving) something.
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
/// such line is treated as a violation. It is classified as a documented defect
/// (FINDING 02 records that the routine logs and pays anyway) rather than as a new
/// one, but it is recorded verbatim, because it is the engine's own testimony that
/// its payout basis was wrong.
pub fn check_self_reported_inconsistency(logs: &[String], phase: &GamePhase) -> Vec<Violation> {
    logs.iter()
        .filter(|line| line.contains("BUG:") || line.contains("CRITICAL:"))
        .map(|line| {
            Violation::new(
                Invariant::M1bPotBreakdown,
                "canister_reports_its_own_inconsistency",
                Severity::BreakdownDrift,
                0,
                phase,
                format!("the canister logged an accounting inconsistency and settled anyway: {line}"),
            )
        })
        .collect()
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
