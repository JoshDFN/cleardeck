//! The money invariants, as named assertions.
//!
//! Each one is a function that returns the violations it found instead of
//! panicking, so the fuzzer can keep exploring past a violation it already knows
//! about and still fail hard on a new one. The hand-written tests in
//! `tests/invariants.rs` call the same functions and assert on the result, so
//! there is exactly one definition of each invariant in the codebase.
//!
//! Split in two by WHAT AN OBSERVATION IS:
//!
//!   * this file: the checks evaluable from ONE snapshot -- M1, M1b, M2, M4.
//!   * [`relational`]: the checks that need two observations or the canister's own
//!     log -- M3, M5, M6, and the self-report matcher. Re-exported here, so
//!     `use money_safety::invariants::*` still names everything.
//!
//! The anchor for the money invariants is the LEDGER, not the canister's own
//! bookkeeping. `icrc1_balance_of(table)` is a fact about the outside world; the
//! escrow map and the chip stacks are the canister's claims about it. Comparing
//! the canister against itself would make M1 vacuous.

use serde::Serialize;

pub mod attribution;
pub mod custody;
pub mod outcome;
pub mod reachability;
pub mod relational;
pub use attribution::*;
pub use custody::*;
pub use outcome::*;
pub use reachability::*;
pub use relational::*;

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

    /// M9 FUND REACHABILITY. For any state a sequence of legal calls can reach,
    /// there is a sequence of legal calls by each funded player that returns that
    /// player's balance to the ledger.
    ///
    /// # Why this had to exist (docs/SECURITY-FINDINGS.md FINDING 15)
    ///
    /// M1..M8 all ask whether the arithmetic is right. **None of them asks whether
    /// the player can still get the money out.** An independent auditor reached a
    /// state, in ordinary play, where `check_timeouts`, `player_action` and
    /// `leave_table` all trapped in the hand evaluator while `withdraw` and
    /// `cash_out` refused with "Cannot withdraw while in a hand" -- about 420 ICP
    /// with no door open at all -- and every invariant in this harness was silent,
    /// correctly, because not one chip had gone missing. Chips conserved inside a
    /// canister nobody can withdraw from is not safety.
    ///
    /// Checked two ways, because either one alone is a hole:
    ///
    /// * [`reachability::check_no_settlement_trap`], on EVERY step: no update on
    ///   the settlement path may trap while the table holds money. A trap rolls
    ///   the message back, so the state that caused it is still there and the next
    ///   call takes the identical path -- a trap here is not a failed call, it is
    ///   a closed door, permanently.
    /// * [`reachability::drain`], at the end of every run: actually take every
    ///   player's money out to the ledger and check it arrives. A structural check
    ///   can only fail on the failure modes somebody imagined; the drain fails on
    ///   any of them.
    /// * [`reachability::check_no_orphaned_custody`], on every snapshot, and again
    ///   on the drain's own result: **the LEDGER, not the canister's books, decides
    ///   whether the table is empty.**
    ///
    /// # The third leg was added because the first two could both be fooled at once
    ///
    /// docs/SECURITY-FINDINGS.md FINDING 07. Both checks above measure against
    /// `internal_total()` -- what the canister SAYS it owes. One controller call
    /// (`reset_table`) rebuilt `TableState`, so the canister stopped saying it owed
    /// the seated chips, and:
    ///
    /// * no settlement call had trapped, so leg one was silent;
    /// * `owed_after == 0`, so leg two reported `fully_drained`;
    /// * and 4.00000000 ICP sat on the ledger inside a canister that owed it to
    ///   nobody.
    ///
    /// An attack that changes what is owed cannot be caught by an instrument
    /// anchored to what is owed. The third leg is anchored to `icrc1_balance_of`.
    M9FundReachability,

    /// M10 CUSTODY VISIBILITY. If a principal has a positive stake in a pot, at
    /// least one surface that principal can read must say so.
    ///
    /// # Why this had to exist (docs/SECURITY-FINDINGS.md FINDING 18)
    ///
    /// M9 asks whether the money can be got out. **It does not ask whether the
    /// owner has any way of knowing it is still in there.** An auditor cashed out
    /// of a stuck hand, was told `Ok = 0` while 298,000,000 e8s of theirs sat in
    /// the pot, read `get_balance() -> 0`, withdrew "everything", and left. The
    /// money was recoverable the whole time -- by `abandon_stuck_hand`, which
    /// nothing on the withdrawal path names -- so M9's drain was GREEN, M1
    /// balanced, and every instrument in this harness agreed the table was fine.
    /// The only reader that could see the stake was a controller.
    ///
    /// A door nobody is told about is not a door. This invariant reads the
    /// canister **as the player** and fails when their own money is invisible to
    /// them (see [`custody::check_custody_is_visible`]).
    M10CustodyVisibility,

    /// M11 OUTCOME. The hand ended when the rules of poker say it ended, and the
    /// seat that held the last live claim is the seat that was paid.
    ///
    /// # Why this had to exist (docs/SECURITY-FINDINGS.md FINDING 17)
    ///
    /// Every other invariant here asks about MONEY: is it conserved, is it
    /// reachable, did it reach the person the record names, did it reach the person
    /// the rules name AT THE MOMENT THE HAND ENDED. None of them asks whether the
    /// hand ended at the right moment. FINDING 17 is a hand allowed to run on past
    /// the point where it was decided: by the time it settled the last card-holder
    /// had been folded by his own clock, so an oracle shown the final state agreed
    /// with the canister that nobody held a claim and every stake should go back.
    /// The oracle was right about the state it was shown. **The state should not
    /// have existed.** Measured: a 52,000,000 e8 pot, three seats, all three ending
    /// on exactly their buy-in, and M1 through M9 silent.
    ///
    /// See [`outcome`] for the three legs.
    M11Outcome,

    /// M8 PRINCIPAL ATTRIBUTION. The money reached the right PERSON, not merely
    /// the right seat and the right total. See [`attribution`] and
    /// docs/SECURITY-FINDINGS.md FINDING 13: paying a departed player's stake to
    /// whoever took their chair conserves every total, awards exactly what was
    /// collected, and lands the right amount in the right seat, so M1..M6 and the
    /// settlement oracle's per-seat diff are all silent.
    M8PrincipalAttribution,
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
            Invariant::M8PrincipalAttribution => "M8_PRINCIPAL_ATTRIBUTION",
            Invariant::M9FundReachability => "M9_FUND_REACHABILITY",
            Invariant::M10CustodyVisibility => "M10_CUSTODY_VISIBILITY",
            Invariant::M11Outcome => "M11_OUTCOME",
        }
    }
}

/// What KIND of failure a violation is.
///
/// Severity used to double as the blocking decision, via
/// `is_documented_defect() = matches!(self, FundDestruction | BreakdownDrift)`.
/// That made a whole direction of defect structurally incapable of failing a run:
/// a planted 1% house rake was recorded as `documented ... FundDestruction` and the
/// fuzz test reported `ok` (docs/DEFECTS.md H-03). Blocking is now decided by the
/// NAMED register in [`crate::documented`], and the severities below that are never
/// excusable are listed there in one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    /// The canister owes MORE than it holds. Chips from nothing. Never excusable.
    FundCreation,
    /// Money was collected from players and not paid back out to them, and the
    /// harness can prove it against the canister's OWN payout basis rather than
    /// against a sign. This is the shape of an operator skimming pots.
    /// Never excusable.
    RakeTaken,
    /// A pot, withdrawal or deposit was settled twice. Never excusable.
    DoublePay,
    /// An upgrade lost or duplicated state. Never excusable.
    DurabilityLoss,
    /// The canister itself logged that it had detected an inconsistency, and the
    /// line is not the one enumerated in the register. `pre_upgrade`'s
    /// `CRITICAL: Failed to save state to stable memory` is this: a
    /// total-fund-loss-on-upgrade event. Never excusable.
    SelfReportedFailure,
    /// The canister holds MORE than it owes: money stranded, unrecoverable
    /// (there is no admin withdrawal path). Excusable ONLY where
    /// [`crate::documented::REGISTER`] names it.
    FundDestruction,
    /// `side_pots` disagrees with `pot`. Excusable ONLY where the register names it.
    BreakdownDrift,
    /// The right amount reached the WRONG PRINCIPAL. Totals balance, seats balance,
    /// and one player has another player's money. Never excusable: there is no
    /// magnitude at which paying the wrong person is acceptable, and no direction
    /// -- the loser's side of it looks like a shortfall and the winner's like a
    /// windfall, and they are the same defect.
    Misattribution,
    /// Money the canister accounts for perfectly and no player can withdraw. The
    /// arithmetic is right and the door is shut. Never excusable, and note that it
    /// is the ONE severity here with a zero `delta_e8s`: nothing has gone missing,
    /// which is exactly why every other invariant is silent about it.
    /// docs/SECURITY-FINDINGS.md FINDING 15.
    FundsUnreachable,
    /// **Money inside the canister that belongs to NOBODY.** The ledger says the
    /// canister holds it; the canister's own books say it owes it to no one; so no
    /// call by any player and no call by a controller can move it out. Never
    /// excusable, and deliberately its own severity rather than a
    /// [`Severity::FundDestruction`], which the register is allowed to excuse.
    ///
    /// # Why this exists (docs/SECURITY-FINDINGS.md FINDING 07)
    ///
    /// `reset_table` replaced `TableState`, so every seated chip stopped being
    /// owed to anybody while the ICP stayed on the ledger. M2 was silent because
    /// holding MORE than you owe is not insolvency. M9's drain reported
    /// `fully_drained` because `internal_total` counts what the canister SAYS it
    /// owes, and after the reset it said it owed nothing -- so the attack passed
    /// by changing the very quantity the instrument measured against. The rule
    /// that binds does not ask the canister what it owes: it asks the LEDGER what
    /// the canister holds, and calls every unowned e8 of it a defect.
    OrphanedCustody,
    /// **The player's own money, invisible to the player.** A principal has a
    /// positive stake in a pot and every surface they can read reports zero, so
    /// the recovery path exists and nobody can be expected to find it. Never
    /// excusable: a door nobody is told about is not a door.
    ///
    /// Zero `delta_e8s`, like [`Severity::FundsUnreachable`], and for the same
    /// reason -- nothing has gone missing, which is precisely why every
    /// conservation invariant is quiet. docs/SECURITY-FINDINGS.md FINDING 18.
    CustodyInvisible,
    /// **The right totals, and the wrong result.** The hand did not end when the
    /// rules of poker say it ended, or the seat that held the last live claim was
    /// not the seat that was paid. Never excusable.
    ///
    /// Its own severity, and not a [`Severity::Misattribution`], because the two
    /// convict different things: misattribution is one person holding another
    /// person's money, and this is nobody holding money that somebody WON. In the
    /// FINDING 17 measurement every e8 went back to the principal who put it in,
    /// which is the most defensible-looking wrong answer this engine can give: the
    /// arithmetic is exact and the outcome is a hand that was never played.
    /// docs/SECURITY-FINDINGS.md FINDING 17.
    WrongOutcome,
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
    pub(crate) fn new(
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

pub(crate) fn phase_of(t: &TableState) -> &GamePhase {
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
///  2. mid-hand, `pot` equals the PAYOUT BASIS: the seated players'
///     `total_bet_this_hand` plus the stakes recorded for seats that left mid-hand.
///     Every e8 in the pot is attributed to somebody, so the bet-level split can
///     allocate all of it.
///
/// (2) used to be measured against the seated players alone, because that was all
/// the engine had: vacating a seat mid-hand deleted the record of what that player
/// put in while the money stayed in `pot`, and the split handed the orphaned amount
/// to the highest bet level -- the deepest stack's own pot (FINDING 05 / E-05).
/// The fix records the stake independently of seat occupancy
/// (`TableState::departed_stakes`), so the basis is complete again and this leg
/// measures against the complete basis. Against the seated set alone it would now
/// flag every departure as an orphan, hiding a real one.
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
        let wagered = t.payout_basis_total();
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
                    "pot={} but the payout basis (seated total_bet_this_hand {} + {} recorded \
                     for seats that left) sums to {} (delta {}). {}",
                    t.pot,
                    t.wagered_total(),
                    t.departed_total(),
                    wagered,
                    delta,
                    if delta > 0 {
                        "There is money in the pot that nobody is credited with contributing, \
                         so the bet-level split cannot allocate it correctly."
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
/// u64 cannot be negative, so the reachable failure modes are (a) a single field
/// that WRAPPED to an absurd magnitude, and (b) a total the canister computed with
/// `saturating_add` that clamped and thereby hid a deficit.
///
/// # What used to be here, and why it could never fire (docs/DEFECTS.md H-02/H-03)
///
/// * `escrow_sum_does_not_overflow` needed the escrow map to sum past `u64::MAX`
///   -- 1.8e19 e8s, about 184 billion ICP. Every e8 in this world is minted by the
///   real ledger at genesis into four wallets, so the whole world holds
///   `4 * ACTOR_START_E8S = 4e12` e8s. Seven orders of magnitude short. Decoration.
/// * `chip_sum_does_not_overflow`: identical, and worse -- it re-added the seated
///   players' chips and then threw the sum away without ever comparing it to
///   `admin_get_table_chips`, the number M1 and M2 are computed from.
/// * `escrow_total_is_exact` compared the canister's `admin_get_all_balances` total
///   against a re-fold of the very list that same call returned, i.e. it recomputed
///   the canister's own expression over the canister's own inputs -- a value against
///   itself -- and then only in the `Some(..)` arm, which excludes the saturation
///   case that was the stated reason for the comparison.
///
/// What replaces them are checks with an input that makes them red: two DIFFERENT
/// canister query methods, computed over two different code paths, must agree about
/// the same money.
pub fn check_arithmetic(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let t = &snap.table;

    // (1) CROSS-METHOD, chips. `admin_get_table_chips` folds `state.players`;
    //     `get_table_state` serialises `state` and the harness folds the seats it
    //     got back. `internal_total()` -- the left-hand side of M1 and M2 -- uses
    //     the first; `wagered_total()` -- the basis of M1b and of M3's sharp form --
    //     uses the second. If they ever disagree, every one of those results is
    //     measured against a different table than the one that holds the money, so
    //     this is load-bearing rather than cosmetic. It fires if either method
    //     miscounts, if `get_table_state` starts omitting or redacting a seat, or if
    //     a chip stack wrapped.
    let seats_sum = t.seated().try_fold(0u64, |a: u64, p| a.checked_add(p.chips));
    match seats_sum {
        None => out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "seated_chips_are_addable",
            Severity::FundCreation,
            0,
            phase_of(t),
            format!(
                "the seated players' chip stacks cannot be added without overflowing u64, so at \
                 least one has WRAPPED: {:?}",
                t.seated().map(|p| (p.seat, p.chips)).collect::<Vec<_>>()
            ),
        )),
        Some(sum) if sum != snap.chips_total => out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "chips_total_is_exact",
            Severity::FundCreation,
            snap.chips_total as i128 - sum as i128,
            phase_of(t),
            format!(
                "the canister reports two different totals for the chips at the table: \
                 admin_get_table_chips()={} but the seats returned by get_table_state() sum to {} \
                 ({:?}). M1/M2 use the first and M1b/M3 use the second, so they are no longer \
                 talking about the same money.",
                snap.chips_total,
                sum,
                t.seated().map(|p| (p.seat, p.chips)).collect::<Vec<_>>()
            ),
        )),
        _ => {}
    }

    // (2) The composite total M1 and M2 are built from must be EXACT. `Snapshot::
    //     internal_total()` folds with `saturating_add`; if it ever clamps, M1
    //     compares the ledger against `u64::MAX` and quietly stops being an
    //     invariant. Recompute it with checked arithmetic and say so if it cannot
    //     be represented.
    let exact_internal = snap
        .escrow_total
        .checked_add(snap.chips_total)
        .and_then(|a| a.checked_add(t.pot));
    if exact_internal.is_none() {
        out.push(Violation::new(
            Invariant::M4NoNegativeNoOverflow,
            "internal_total_is_representable",
            Severity::FundCreation,
            0,
            phase_of(t),
            format!(
                "escrow({}) + chips({}) + pot({}) OVERFLOWS u64, so the saturating total M1 and M2 \
                 compare against the ledger is a clamped number and both invariants are vacuous",
                snap.escrow_total, snap.chips_total, t.pot
            ),
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
    // M9's standing half. Fires on exactly the same arithmetic condition as M1's
    // positive direction, on purpose and by construction -- so it adds no NEW red
    // anywhere -- but it says the thing M1 does not: a surplus is not a surplus,
    // it is money that belongs to nobody, and the register may never excuse it.
    out.extend(reachability::check_no_orphaned_custody(
        snap,
        uncredited_raw_deposits,
    ));
    out
}

pub fn check_world(world: &World) -> Vec<Violation> {
    let snap = world.snapshot();
    check_point_in_time(&snap, world.uncredited_raw_deposits)
}

