//! **Can this canister pay everyone it owes, and can it SAY so?**
//!
//! docs/SECURITY-FINDINGS.md FINDING 35, docs/DEFECTS.md E-70.
//!
//! # Why this module exists and M2 was not enough
//!
//! M2 (`check_conservation`'s `canister_is_short`) already compares what the
//! LEDGER holds against what the canister's books say it owes, and it is anchored
//! correctly. It has never been able to convict the defect this module exists
//! for, because it asks the question **from outside**. It reads
//! `icrc1_balance_of` with the harness's own agent, on a replica the harness owns.
//!
//! A player cannot do that. A player can only ask the canister. And on mainnet
//! table_1, on 2026-08-06:
//!
//! ```text
//!   escrow claimed   940,640,001 e8s
//!   ledger main      740,640,001 e8s
//!   SHORTFALL        200,000,000 e8s   (2.00 ICP)
//!
//!   get_balance()             -> correct
//!   get_custody_status()      -> correct, every field
//!   admin_get_all_balances()  -> correct
//!   admin_audit_deposit_custody() -> (5 audited, 0 held, 0 unaudited)
//! ```
//!
//! Every surface was right and not one of them could say the money was not there.
//! **A canister custodying funds that cannot tell anyone it is insolvent is the
//! defect**, and no instrument anchored to the harness's own ledger reads will
//! ever see it, because the harness can always look.
//!
//! # The four legs
//!
//! 1. [`check_solvency_report_is_coherent`] -- the report's own arithmetic, term
//!    by term against figures the harness computes independently. This is
//!    docs/SECURITY-FINDINGS.md FINDING 37's "one query": `total_liability()` is
//!    the only number the last custody guard reads and it had no query, so the
//!    only way to sample it was to attempt the destructive operation it guards.
//!    It is published as `guard_liability` now and it is checked here.
//! 2. [`check_unknown_is_not_zero`] -- a never-observed account must read as
//!    `null` and force the verdict to `Unknown`. Wave 8 learned this at the
//!    subaccounts; the same rule is the difference between "the main account is
//!    empty" and "nobody has ever looked at the main account", and those two were
//!    the same answer until FINDING 35.
//! 3. [`check_written_down_holdings_are_not_overstated`] -- the canister may never
//!    claim to hold more in an account than the ledger says is there. This is the
//!    one-directional property the whole report rests on, asserted rather than
//!    argued, exactly as `check_deposit_attribution` asserts it for the
//!    subaccounts.
//! 4. [`check_insolvency_is_reported`] -- when the LEDGER says the canister is
//!    short, the canister's own public surface must say so too. This is the only
//!    leg that costs an update call, and it is taken **only when there is a
//!    shortfall to report**, which in a healthy run is never.

use candid::Principal;

use super::{phase_of, Invariant, Severity, Violation};
use crate::table_api::SolvencyVerdict;
use crate::world::{Snapshot, World};

/// What the HARNESS says the canister owes, computed from the canister's booked
/// custody and nothing else.
///
/// Open ledger intents are deliberately **excluded**. An intent names a movement
/// whose ledger effect is indeterminate by construction -- that is what makes it
/// an intent -- so counting it against a ledger balance that may or may not
/// already reflect it manufactures a shortfall on one side or a surplus on the
/// other every time a `deposit()` or a `withdraw()` is in flight. Money at the
/// ledger boundary is M14's subject, and M14 is the invariant that can see it.
///
/// `pot.max(committed_stake)` and not `pot`, which is the one term
/// [`Snapshot::internal_total`] gets weaker than the canister's own figure: a
/// stake left behind by a departed player is owed to that player whether or not
/// `state.pot` still names it.
pub fn harness_owed(snap: &Snapshot) -> u64 {
    let committed = snap
        .canister_solvency
        .as_ref()
        .map(|r| r.committed_stake)
        .unwrap_or(0);
    snap.escrow_total
        .saturating_add(snap.chips_total)
        .saturating_add(snap.table.pot.max(committed))
        .saturating_add(snap.canister_unswept_deposits)
}

/// LEG 1. The report's own arithmetic, and its agreement with every other surface
/// the harness can read.
///
/// A report nobody can check is the same instrument `total_liability()` was: a
/// number with one caller, no query and no gate, which is how a whole account
/// stayed outside it for eight waves (FINDING 37).
pub fn check_solvency_report_is_coherent(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(r) = snap.canister_solvency.as_ref() else {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "no_solvency_surface_at_all",
            Severity::InsolvencyUnreported,
            0,
            phase_of(&snap.table),
            "this build has no get_solvency() a caller can read, so there is no way for anybody \
             -- player, operator or auditor -- to ask the canister whether it can pay what it \
             owes. That is the state mainnet table_1 was in while it was 2.00 ICP short. \
             docs/SECURITY-FINDINGS.md FINDING 35."
                .to_string(),
        ));
        return out;
    };

    let mut mismatch = |check: &'static str, said: u64, is: u64, what: &str| {
        if said != is {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                check,
                Severity::InsolvencyUnreported,
                said as i128 - is as i128,
                phase_of(&snap.table),
                format!(
                    "get_solvency() says {what} is {said} and every other surface on this \
                     canister says {is}. A solvency report that disagrees with the surfaces it \
                     is built from cannot be used to convict or to clear anybody."
                ),
            ));
        }
    };
    mismatch(
        "solvency_escrow_disagrees_with_admin_balances",
        r.escrow,
        snap.escrow_total,
        "escrow",
    );
    mismatch(
        "solvency_chips_disagree_with_table",
        r.chips_at_table,
        snap.chips_total,
        "chips at the table",
    );
    mismatch("solvency_pot_disagrees_with_table", r.pot, snap.table.pot, "the pot");
    mismatch(
        "solvency_deposits_disagree_with_deposit_census",
        r.unswept_deposits,
        snap.canister_unswept_deposits,
        "unswept deposit custody",
    );

    // The terms must add up to the total the verdict is computed from. If they do
    // not, one of them is not in the sum and the whole point of publishing the
    // terms separately is gone.
    let term_sum = r
        .escrow
        .saturating_add(r.chips_at_table)
        .saturating_add(r.pot.max(r.committed_stake))
        .saturating_add(r.unswept_deposits)
        .saturating_add(r.unfinished_incoming)
        .saturating_add(r.payouts_in_flight);
    if term_sum != r.owed {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "solvency_owed_is_not_the_sum_of_its_terms",
            Severity::InsolvencyUnreported,
            r.owed as i128 - term_sum as i128,
            phase_of(&snap.table),
            format!(
                "get_solvency() reports owed={} and its own published terms sum to {} \
                 (escrow {} + chips {} + max(pot {}, committed {}) + unswept {} + \
                 unfinished_incoming {} + payouts_in_flight {}). A term that is in the total \
                 and not in the breakdown is a term no reader can audit, which is the shape \
                 of docs/SECURITY-FINDINGS.md FINDING 37.",
                r.owed,
                term_sum,
                r.escrow,
                r.chips_at_table,
                r.pot,
                r.committed_stake,
                r.unswept_deposits,
                r.unfinished_incoming,
                r.payouts_in_flight
            ),
        ));
    }

    // THE GUARD'S NUMBER. `guard_liability` is `total_liability()`, the single
    // input to `refuse_currency_change_while_funded`, and the relation below is
    // the whole of what makes the report and the guard the same instrument:
    //
    //   total_liability() = owed - payouts_in_flight + unattributed_at_main
    //
    // (a payout is money leaving and is not in the guard's sum; money at the main
    // account that nobody is credited with IS, and that fifth term is FINDING 35's
    // own prescription.)
    let expected_guard = r
        .owed
        .saturating_sub(r.payouts_in_flight)
        .saturating_add(r.unattributed_at_main.unwrap_or(0));
    if r.guard_liability != expected_guard {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "guard_liability_disagrees_with_the_published_terms",
            Severity::InsolvencyUnreported,
            r.guard_liability as i128 - expected_guard as i128,
            phase_of(&snap.table),
            format!(
                "get_solvency() publishes guard_liability={} -- the ONLY number \
                 refuse_currency_change_while_funded reads -- while its own terms imply {}. \
                 The guard and the report have come apart, and the guard is the one nobody \
                 can sample without attempting the destructive operation it protects. \
                 docs/SECURITY-FINDINGS.md FINDING 37.",
                r.guard_liability, expected_guard
            ),
        ));
    }

    // A pull cannot exceed everything in flight that is arriving.
    if r.pulls_in_flight > r.unfinished_incoming {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "solvency_pulls_exceed_unfinished_incoming",
            Severity::InsolvencyUnreported,
            r.pulls_in_flight as i128 - r.unfinished_incoming as i128,
            phase_of(&snap.table),
            format!(
                "get_solvency() reports pulls_in_flight={} inside unfinished_incoming={}, so \
                 the pull half of the journal is larger than the whole of it.",
                r.pulls_in_flight, r.unfinished_incoming
            ),
        ));
    }

    // The held side, and the difference. Both are EXACT: the report publishes
    // every part it assembles `held` from, so there is nothing here the harness
    // has to take on trust.
    if let (Some(main), Some(held)) = (r.main_account, r.held) {
        let expected_held = main
            .saturating_add(r.deposit_subaccounts)
            .saturating_add(r.pulls_in_flight)
            .saturating_sub(r.sweep_fees_in_flight);
        if held != expected_held {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "solvency_held_is_not_built_from_its_published_parts",
                Severity::InsolvencyUnreported,
                held as i128 - expected_held as i128,
                phase_of(&snap.table),
                format!(
                    "get_solvency() reports held={held}, and its own published parts give \
                     main_account({main}) + deposit_subaccounts({}) + pulls_in_flight({}) - \
                     sweep_fees_in_flight({}) = {expected_held}. A total a reader cannot \
                     rebuild from the record is a total they have to take on trust from the \
                     party being audited.",
                    r.deposit_subaccounts, r.pulls_in_flight, r.sweep_fees_in_flight
                ),
            ));
        }
        let expected_difference = held as i128 - r.owed as i128;
        if r.difference_e8s != Some(expected_difference) {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "solvency_difference_is_not_held_minus_owed",
                Severity::InsolvencyUnreported,
                r.difference_e8s.unwrap_or(0) - expected_difference,
                phase_of(&snap.table),
                format!(
                    "get_solvency() reports difference_e8s={:?} while held({held}) - \
                     owed({}) = {expected_difference}. The one number a reader takes away \
                     from this record is the one that does not follow from it.",
                    r.difference_e8s, r.owed
                ),
            ));
        }
    }

    // A verdict of CanPayEveryone is a claim, and it has preconditions.
    if r.verdict == SolvencyVerdict::CanPayEveryone {
        let bad = r.main_account.is_none()
            || r.main_observed_at_ns.is_none()
            || r.deposit_accounts_never_observed_count > 0
            || r.difference_e8s.map(|d| d < 0).unwrap_or(true);
        if bad {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "solvency_says_it_can_pay_on_inputs_it_has_not_got",
                Severity::InsolvencyUnreported,
                r.difference_e8s.unwrap_or(0),
                phase_of(&snap.table),
                format!(
                    "get_solvency() answers CanPayEveryone with main_account={:?}, \
                     main_observed_at_ns={:?}, {} never-observed deposit account(s) and \
                     difference={:?}. An all-clear assembled from an account nobody has read \
                     is the exact failure wave 8 named UNKNOWN IS NOT ZERO.",
                    r.main_account,
                    r.main_observed_at_ns,
                    r.deposit_accounts_never_observed_count,
                    r.difference_e8s
                ),
            ));
        }
    }
    out
}

/// LEG 2. **UNKNOWN IS NOT ZERO.**
///
/// An account nobody has read has an unknown balance. It must be reported as
/// `null`, it must not be folded into a total as a zero, and it must force the
/// verdict to `Unknown` rather than to an all-clear.
pub fn check_unknown_is_not_zero(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(r) = snap.canister_solvency.as_ref() else {
        return out; // Leg 1 has already convicted the absence of the surface.
    };

    if r.main_observed_at_ns.is_none() {
        let mut wrong: Vec<String> = Vec::new();
        if r.main_account.is_some() {
            wrong.push(format!("main_account={:?}", r.main_account));
        }
        if r.held.is_some() {
            wrong.push(format!("held={:?}", r.held));
        }
        if r.difference_e8s.is_some() {
            wrong.push(format!("difference_e8s={:?}", r.difference_e8s));
        }
        if r.verdict != SolvencyVerdict::Unknown {
            wrong.push(format!("verdict={:?}", r.verdict));
        }
        if !wrong.is_empty() {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "never_observed_main_account_is_not_reported_as_unknown",
                Severity::InsolvencyUnreported,
                0,
                phase_of(&snap.table),
                format!(
                    "this canister has NEVER asked the ledger what its own main account holds \
                     (main_observed_at_ns is null) and yet reports {}. The ledger says that \
                     account holds {} e8s. A never-read account rendered as a zero is how the \
                     currency guard came to read a liability of zero on a canister holding 5 \
                     ICP. docs/SECURITY-FINDINGS.md FINDING 35.",
                    wrong.join(", "),
                    snap.ledger_main
                ),
            ));
        }
    }

    // THE COUNT, not the caller-scoped roster. `World::solvency()` reads this as
    // an ANONYMOUS caller, so `deposit_accounts_never_observed` is empty by
    // design there; branching on it would make this leg permanently silent, which
    // is the exact shape of a gate that measures nothing.
    if r.deposit_accounts_never_observed_count > 0
        && r.verdict == SolvencyVerdict::CanPayEveryone
    {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "never_observed_deposit_account_reported_as_solvent",
            Severity::InsolvencyUnreported,
            0,
            phase_of(&snap.table),
            format!(
                "get_solvency() answers CanPayEveryone while {} published deposit address(es) \
                 have never been read (of which this caller may see {:?})",
                r.deposit_accounts_never_observed_count,
                r.deposit_accounts_never_observed
            ),
        ));
    }
    out
}

/// LEG 3. **The written-down holdings may never exceed what the ledger holds --
/// beyond what is provably in flight.**
///
/// The report rests on a one-directional property, and the FIRST version of this
/// check stated it too strongly, which is worth recording because it is the same
/// mistake in the same place as everything else in this file: it asserted
/// `main_account <= ledger_main` unconditionally, and that is FALSE in exactly one
/// reachable state. A `payout` that the ledger executed and whose continuation was
/// discarded (docs/SECURITY-FINDINGS.md FINDING 29) leaves the main account lower
/// on the chain and unchanged in the record, because the record is only adjusted
/// at `settle_intent`. Same shape for a `sweep`: the deposit subaccount is drained
/// on the chain and the observation still names the money.
///
/// **The difference the report publishes is unaffected**, and that is the point
/// worth checking rather than asserting away. A payout debits the main account by
/// exactly `intent.amount`, and exactly `intent.amount` is on the owed side as
/// `payouts_in_flight` until the same instant the record is adjusted, so
///
/// ```text
///   (main + A) - (owed + A)  ==  main - owed
/// ```
///
/// whether or not the movement has landed. What the per-account field can be is
/// stale-high by AT MOST what is in flight, and that is what is asserted here:
/// tight everywhere else, and exactly as loose as the journal says it must be.
/// Anything beyond that bound is a canister claiming to hold money that is not
/// there, which is a false all-clear waiting to be issued to somebody deciding
/// whether to sit down at this table.
pub fn check_written_down_holdings_are_not_overstated(snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(r) = snap.canister_solvency.as_ref() else {
        return out;
    };

    // A payout hands `amount` to the ledger out of the main account and is only
    // subtracted from the record when the ledger's answer comes back.
    let main_allowance = r.payouts_in_flight;
    if let Some(main) = r.main_account {
        if main > snap.ledger_main.saturating_add(main_allowance) {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "canister_claims_more_in_its_main_account_than_the_ledger_holds",
                Severity::InsolvencyUnreported,
                main as i128 - snap.ledger_main as i128,
                phase_of(&snap.table),
                format!(
                    "get_solvency() says the main account holds {main} e8s, the LEDGER says {}, \
                     and only {main_allowance} of that gap is a payout in flight. The \
                     written-down balance is allowed to be stale LOW and is allowed to be high \
                     ONLY by money the journal already names: a figure that can be freely too \
                     high is a false all-clear waiting to be issued. (raw reading {} + \
                     credited_since {} - debited_since {})",
                    snap.ledger_main,
                    main.saturating_sub(r.main_credited_since_reading)
                        .saturating_add(r.main_debited_since_reading),
                    r.main_credited_since_reading,
                    r.main_debited_since_reading,
                ),
            ));
        }
    }

    // A sweep drains `amount + fee` from a deposit subaccount, and the observation
    // is only reset when the ledger's answer comes back. `unfinished_incoming` is
    // pulls plus sweeps, so the sweep half is the difference.
    let deposit_allowance = r
        .unfinished_incoming
        .saturating_sub(r.pulls_in_flight)
        .saturating_add(r.sweep_fees_in_flight);
    if r.deposit_subaccounts > snap.ledger_deposit_subaccounts.saturating_add(deposit_allowance) {
        out.push(Violation::new(
            Invariant::M2LedgerReality,
            "canister_claims_more_in_its_deposit_subaccounts_than_the_ledger_holds",
            Severity::InsolvencyUnreported,
            r.deposit_subaccounts as i128 - snap.ledger_deposit_subaccounts as i128,
            phase_of(&snap.table),
            format!(
                "get_solvency() says the deposit subaccounts hold {} e8s, the LEDGER says {}, \
                 and only {deposit_allowance} of that gap is a sweep in flight.",
                r.deposit_subaccounts, snap.ledger_deposit_subaccounts
            ),
        ));
    }

    // AND THE THING THE TWO ALLOWANCES ABOVE MUST NOT BE ABLE TO HIDE.
    //
    // Both are exactly cancelled on the owed side, so the TOTAL is not allowed to
    // be loose even while the parts are. `held` may never exceed what the ledger
    // holds across every account plus what is genuinely arriving from outside
    // (an open `pull` is money in a player's wallet or in ours, and the report
    // counts it on both sides on purpose).
    if let Some(held) = r.held {
        let bound = snap
            .ledger_holdings()
            .saturating_add(r.payouts_in_flight)
            .saturating_add(r.pulls_in_flight);
        if held > bound {
            out.push(Violation::new(
                Invariant::M2LedgerReality,
                "canister_claims_to_hold_more_than_the_ledger_holds",
                Severity::InsolvencyUnreported,
                held as i128 - snap.ledger_holdings() as i128,
                phase_of(&snap.table),
                format!(
                    "get_solvency() reports held={held} against a LEDGER total of {} across \
                     every account this canister owns, with {} in payouts and {} in pulls in \
                     flight. Every e8 of the in-flight allowance is counted on the OWED side \
                     too, so it cannot be what closes this gap: the canister is claiming money \
                     that is not there.",
                    snap.ledger_holdings(),
                    r.payouts_in_flight,
                    r.pulls_in_flight
                ),
            ));
        }
    }
    out
}

/// LEG 4. **OWES <= HOLDS, and when it does not, the canister must say so.**
///
/// The first half is anchored to `icrc1_balance_of` across EVERY account the
/// canister owns, and it is the never-excusable one: a canister that owes more
/// than it holds is a canister where somebody's withdrawal is going to fail.
///
/// The second half is the one no instrument in this project had. It costs an
/// update call, so it is taken **only when there is a shortfall to report** --
/// which in a healthy run is never, and on the state this wave exists for is
/// every step. `refresh_solvency()` is called as an ORDINARY ACTOR, not as the
/// controller, because the property being tested is that a player can find this
/// out without permission.
pub fn check_insolvency_is_reported(world: &World, snap: &Snapshot) -> Vec<Violation> {
    let mut out = Vec::new();
    let owed = harness_owed(snap);
    let holdings = snap.ledger_holdings();
    if owed <= holdings {
        return out;
    }
    let short = owed - holdings;

    out.push(Violation::new(
        Invariant::M2LedgerReality,
        "owes_more_than_it_holds_across_every_account",
        Severity::FundCreation,
        holdings as i128 - owed as i128,
        phase_of(&snap.table),
        format!(
            "the LEDGER says this canister holds {holdings} e8s across every account it owns \
             (main {} + deposit subaccounts {}) and its own books say it owes {owed} (escrow {} \
             + chips {} + pot {} + unswept deposit custody {}). It is SHORT {short} e8s: \
             somebody's withdrawal is going to fail at the ledger, and it will be whoever asks \
             last.",
            snap.ledger_main,
            snap.ledger_deposit_subaccounts,
            snap.escrow_total,
            snap.chips_total,
            snap.table.pot,
            snap.canister_unswept_deposits,
        ),
    ));

    // AND NOW THE PART THAT IS THE POINT. Give the canister the one thing it needs
    // -- a reading of its own main account, taken by a call any principal may make
    // -- and require it to say what the ledger has just said.
    let asker = world
        .actors
        .first()
        .map(|a| a.principal)
        .unwrap_or_else(Principal::anonymous);
    match world.refresh_solvency(asker) {
        Ok(r) => {
            if r.verdict != SolvencyVerdict::CannotPayEveryone {
                out.push(Violation::new(
                    Invariant::M2LedgerReality,
                    "insolvency_is_not_reported_after_a_refresh",
                    Severity::InsolvencyUnreported,
                    -(short as i128),
                    phase_of(&snap.table),
                    format!(
                        "the LEDGER says this canister is SHORT {short} e8s, and after \
                         refresh_solvency() -- which reads the main account and writes the \
                         answer down -- it still answers {:?} with summary {:?}. A canister \
                         custodying funds that cannot say it is insolvent is the defect. \
                         docs/SECURITY-FINDINGS.md FINDING 35.",
                        r.verdict, r.summary
                    ),
                ));
            }
            if r.verdict == SolvencyVerdict::CannotPayEveryone && r.shortfall_e8s.is_none() {
                out.push(Violation::new(
                    Invariant::M2LedgerReality,
                    "insolvency_is_reported_without_a_magnitude",
                    Severity::InsolvencyUnreported,
                    0,
                    phase_of(&snap.table),
                    "get_solvency() says CannotPayEveryone and carries no shortfall_e8s, so \
                     nobody reading it can tell a rounding error from a stolen table."
                        .to_string(),
                ));
            }
        }
        Err(e) => out.push(Violation::new(
            Invariant::M2LedgerReality,
            "refresh_solvency_is_not_available_to_an_ordinary_caller",
            Severity::InsolvencyUnreported,
            -(short as i128),
            phase_of(&snap.table),
            format!(
                "the LEDGER says this canister is SHORT {short} e8s and an ordinary player \
                 cannot even take the reading that would reveal it: refresh_solvency() answered \
                 {e:?}. A solvency check a player has to ask an operator for is not a solvency \
                 check."
            ),
        )),
    }
    out
}

/// Every query-only solvency leg, for [`super::check_point_in_time`].
///
/// Leg 4 is NOT here: it performs an update call, so it belongs where the caller
/// knows a round may be consumed ([`check_insolvency_is_reported`], driven from
/// the fuzz step loop and from the hand-written tests).
pub fn check_solvency_surfaces(snap: &Snapshot) -> Vec<Violation> {
    let mut out = check_solvency_report_is_coherent(snap);
    out.extend(check_unknown_is_not_zero(snap));
    out.extend(check_written_down_holdings_are_not_overstated(snap));
    out
}
