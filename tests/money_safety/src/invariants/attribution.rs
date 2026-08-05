//! M8 PRINCIPAL ATTRIBUTION — the invariant every other gate in this repo was
//! blind to.
//!
//! # Why this file exists (docs/SECURITY-FINDINGS.md FINDING 13)
//!
//! Every money assertion in this harness before it was either an aggregate over
//! the whole table (M1, M2, M3) or a number the canister itself recorded. All of
//! them pass while one player's money is credited to another player, because
//! paying the wrong principal:
//!
//!   * conserves every total — the canister holds exactly what it owes;
//!   * awards exactly what the hand collected — no rake, no destruction;
//!   * moves the right AMOUNT into the right SEAT — so the settlement oracle's
//!     per-seat chip diff is zero as well.
//!
//! The only thing wrong is WHO HAS THE MONEY, and nothing was looking.
//!
//! # The invariant
//!
//! For a hand played with no deposit, withdrawal, buy-in or cash-out in flight,
//! measure each principal's **value** as `escrow + chips in front of them`. Counting
//! it that way rather than as a change in `Player::chips` is deliberate: a player
//! who leaves the table mid-hand has their stack moved into escrow, so `chips`
//! alone reads as a total loss where nothing was lost.
//!
//! Then, per principal:
//!
//!   value_after - value_before == what the hand owed THEM - what THEY staked
//!
//! The caller supplies the right-hand side. Two shapes of caller exist and both
//! matter:
//!
//!   * a deliberate scenario, which knows exactly who staked what and what the
//!     rules of poker owe each of them (`tests/invariants/principals.rs`);
//!   * the corollary that needs no oracle at all and is stated separately below:
//!     **a principal who staked nothing in a hand cannot come out of it richer.**
//!     You cannot win money from a hand you did not play. That single line convicts
//!     FINDING 13 without anybody having to derive a settlement.

use candid::Principal;
use std::collections::{BTreeMap, BTreeSet};

use crate::table_api::GamePhase;
use crate::world::{Snapshot, World};

use super::{Invariant, Severity, Violation};

/// What every principal in the world is worth to the canister: their escrow plus
/// whatever is in front of them at the table.
///
/// Chips in the POT belong to no principal while the hand is live -- that is what
/// makes this quantity dip by a player's stake mid-hand and come back at
/// settlement -- so the pot is deliberately not attributed to anybody here.
pub fn values_by_principal(snapshot: &Snapshot) -> BTreeMap<Principal, u64> {
    let mut out: BTreeMap<Principal, u64> = snapshot.escrow.clone();
    for p in snapshot.table.seated() {
        let value = out.entry(p.principal).or_insert(0);
        *value = value.saturating_add(p.chips);
    }
    out
}

/// Same, read live off the canister.
pub fn values_now(world: &World) -> BTreeMap<Principal, u64> {
    values_by_principal(&world.snapshot())
}

/// Signed change in each principal's value between two observations. Every
/// principal named in either observation appears.
pub fn value_deltas(
    before: &BTreeMap<Principal, u64>,
    after: &BTreeMap<Principal, u64>,
) -> BTreeMap<Principal, i128> {
    let mut who: BTreeSet<Principal> = before.keys().copied().collect();
    who.extend(after.keys().copied());
    who.into_iter()
        .map(|p| {
            let b = before.get(&p).copied().unwrap_or(0) as i128;
            let a = after.get(&p).copied().unwrap_or(0) as i128;
            (p, a - b)
        })
        .collect()
}

/// M8 PRINCIPAL ATTRIBUTION, against an explicit expectation.
///
/// `expected` is the net change each principal should show. A principal absent
/// from `expected` is expected to be unchanged, which is the strong form: it means
/// a windfall to somebody nobody was thinking about still fires.
pub fn check_principal_attribution(
    before: &BTreeMap<Principal, u64>,
    after: &BTreeMap<Principal, u64>,
    expected: &BTreeMap<Principal, i128>,
    phase: &GamePhase,
) -> Vec<Violation> {
    let observed = value_deltas(before, after);
    let mut out = Vec::new();
    for (who, got) in &observed {
        let want = expected.get(who).copied().unwrap_or(0);
        if *got == want {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "value_delta_per_principal",
            Severity::Misattribution,
            got - want,
            phase,
            format!(
                "principal {who} came out of this hand {got:+} e8s when the rules of poker owe \
                 them {want:+}. Every total may still balance: the money went to or came from \
                 the WRONG PERSON. See docs/SECURITY-FINDINGS.md FINDING 13. \
                 observed={observed:?} expected={expected:?}"
            ),
        ));
    }
    out
}

/// M8b NO FREE MONEY — the oracle-free corollary.
///
/// A principal who put nothing into a hand cannot come out of it with more than
/// they went in with. This needs no settlement derivation at all, and it is what
/// convicts FINDING 13: a stranger who takes a departed player's chair mid-hand is
/// dealt no cards, stakes nothing, and was nonetheless credited that player's
/// refunded stake.
///
/// `staked` is the set of principals who actually had money in this hand. A
/// principal in that set is not checked here -- what they are owed is a question
/// for the settlement rules, i.e. for [`check_principal_attribution`].
pub fn check_no_free_money(
    before: &BTreeMap<Principal, u64>,
    after: &BTreeMap<Principal, u64>,
    staked: &BTreeSet<Principal>,
    phase: &GamePhase,
) -> Vec<Violation> {
    let mut out = Vec::new();
    for (who, delta) in value_deltas(before, after) {
        if staked.contains(&who) || delta <= 0 {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "no_free_money_for_a_principal_that_staked_nothing",
            Severity::Misattribution,
            delta,
            phase,
            format!(
                "principal {who} staked NOTHING in this hand and came out of it {delta:+} e8s \
                 richer. You cannot win money from a hand you did not play; that money belongs \
                 to whoever put it in. See docs/SECURITY-FINDINGS.md FINDING 13."
            ),
        ));
    }
    out
}

/// Convenience for a scenario that expects every principal to end where it
/// started -- e.g. a hand in which every stake was refunded to its own owner.
pub fn expect_all_flat<'a>(
    who: impl IntoIterator<Item = &'a Principal>,
) -> BTreeMap<Principal, i128> {
    who.into_iter().map(|p| (*p, 0i128)).collect()
}

// ===========================================================================
// M8 ON AN ORDINARY HAND
// ===========================================================================
//
// Everything above needs a caller who already knows the answer, which is why it
// only ever ran on two hand-written fixtures. What follows takes a
// [`HandAttribution`] -- measured by `crate::hand_attribution` from the snapshots
// the fuzzer already takes -- and asks the same question of EVERY hand.
//
// Four legs, deliberately of different kinds, because the misdirection defects
// they convict are of different kinds:
//
//   1. `value_delta_matches_the_recorded_award`
//        the canister paid the person its own record names. Convicts everything on
//        the APPLY side, where the plan and the history are right and the money
//        goes somewhere else -- including a `push_winner` that merges two people's
//        money into one name, which is the same defect seen from the record.
//   2. `value_delta_matches_the_settlement_oracle`
//        the person paid is the person the RULES name. Convicts the defects that
//        are internally consistent: a plan that names the wrong live player, or
//        that lets a folded seat win a layer.
//   3. `no_free_money_for_a_principal_that_staked_nothing` (see above; applied
//        per hand here)
//   4. `a_relinquished_stake_cannot_profit`
//        somebody who folded, or who left the chair, can get their own uncalled
//        money back and nothing else. No oracle needed.
//
// Legs 1, 3 and 4 need no derivation of the settlement at all. Leg 2 does, and
// it is skipped -- counted, never silently -- on hands the rules of poker do not
// define an answer for.

use crate::hand_attribution::HandAttribution;

/// Every attribution leg that applies to this hand.
///
/// Returns nothing at all for a hand the harness could not fully observe. That is
/// deliberate and it is why [`HandAttribution::complete`] exists: reporting a
/// misattribution from an incomplete reconstruction would teach a reader to
/// ignore this invariant.
pub fn check_hand_attribution(a: &HandAttribution) -> Vec<Violation> {
    if !a.is_measurable() {
        return Vec::new();
    }
    let mut out = check_award_reaches_the_person_it_names(a);
    out.extend(check_no_free_money_in_hand(a));
    out.extend(check_relinquished_cannot_profit(a));
    out.extend(check_against_the_oracle(a));
    out
}

/// LEG 1. What the canister's own hand history says it paid a person must be what
/// that person's escrow and chips actually did.
///
/// ```text
///   value_after - value_before  ==  awarded_to_them - what_they_staked
/// ```
///
/// Both sides are measured, and neither is computed by the payout code: the left
/// from `admin_get_all_balances` + `get_table_state`, the right from the hand
/// history and from `total_bet_this_hand`/`departed_stakes`. A defect that credits
/// the right amount to the wrong wallet moves the left side and not the right.
pub fn check_award_reaches_the_person_it_names(a: &HandAttribution) -> Vec<Violation> {
    let mut out = Vec::new();
    for who in a.everyone() {
        let measured = a.measured.get(&who).copied().unwrap_or(0);
        let claimed = a.claimed.get(&who).copied().unwrap_or(0) as i128;
        let staked = a.staked.get(&who).copied().unwrap_or(0) as i128;
        let expected = claimed - staked;
        if measured == expected {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "value_delta_matches_the_recorded_award",
            Severity::Misattribution,
            measured - expected,
            &a.phase,
            format!(
                "hand #{}: the canister's own record says it paid {who} {claimed} e8s against a \
                 stake of {staked}, so their escrow+chips should have moved {expected:+}. It \
                 moved {measured:+}. Somebody has money the record does not say they have, and \
                 the totals can still balance to the e8. See docs/SECURITY-FINDINGS.md \
                 FINDING 13. staked={:?} claimed={:?} measured={:?}",
                a.hand_number, a.staked, a.claimed, a.measured
            ),
        ));
    }
    out
}

/// LEG 2. The person paid must be the person the RULES OF POKER name.
///
/// The oracle is `settlement_oracle::oracle`, the independent derivation written
/// for `tests/settlement`: it never calls `determine_winners`, `plan_payouts` or
/// `poker_core::side_pots`. Silent on hands it cannot rule on -- those are counted
/// by the watch, not swallowed.
pub fn check_against_the_oracle(a: &HandAttribution) -> Vec<Violation> {
    let Some(oracle) = a.oracle.as_ref() else {
        return Vec::new();
    };
    if !oracle.anomalies.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for who in a.everyone() {
        let measured = a.measured.get(&who).copied().unwrap_or(0);
        let owed = oracle.net.get(&who).copied().unwrap_or(0);
        if measured == owed {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "value_delta_matches_the_settlement_oracle",
            Severity::Misattribution,
            measured - owed,
            &a.phase,
            format!(
                "hand #{}: the rules of poker owe {who} {owed:+} e8s across this hand and the \
                 canister moved them {measured:+}. Every total may balance and the record may \
                 agree with itself: what is wrong is WHO HAS THE MONEY. oracle={:?} \
                 measured={:?} staked={:?}",
                a.hand_number, oracle.net, a.measured, a.staked
            ),
        ));
    }
    out
}

/// LEG 3. [`check_no_free_money`], applied to one measured hand.
pub fn check_no_free_money_in_hand(a: &HandAttribution) -> Vec<Violation> {
    let staked: BTreeSet<Principal> = a
        .staked
        .iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(who, _)| *who)
        .collect();
    let mut out = Vec::new();
    for who in a.everyone() {
        let delta = a.measured.get(&who).copied().unwrap_or(0);
        if staked.contains(&who) || delta <= 0 {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "no_free_money_for_a_principal_that_staked_nothing",
            Severity::Misattribution,
            delta,
            &a.phase,
            format!(
                "hand #{}: principal {who} staked NOTHING and came out of it {delta:+} e8s \
                 richer. You cannot win money from a hand you did not play. \
                 See docs/SECURITY-FINDINGS.md FINDING 13.",
                a.hand_number
            ),
        ));
    }
    out
}

/// LEG 4. Somebody who gave up their claim cannot come out ahead.
///
/// Folding gives up the claim; so does leaving the chair. Such a player can be
/// handed back money nobody covered -- an uncalled bet, or their whole stake when
/// no layer has a claimant -- and that is the most they can ever receive. A net
/// GAIN means they were paid out of somebody else's stake.
///
/// No oracle, no settlement derivation, no knowledge of the cards.
pub fn check_relinquished_cannot_profit(a: &HandAttribution) -> Vec<Violation> {
    let mut out = Vec::new();
    for who in &a.relinquished_only {
        let delta = a.measured.get(who).copied().unwrap_or(0);
        if delta <= 0 {
            continue;
        }
        out.push(Violation::new(
            Invariant::M8PrincipalAttribution,
            "a_relinquished_stake_cannot_profit",
            Severity::Misattribution,
            delta,
            &a.phase,
            format!(
                "hand #{}: principal {who} folded or left the chair -- every stake of theirs in \
                 this hand was given up -- and they came out {delta:+} e8s AHEAD. A relinquished \
                 stake can only ever be handed its own money back; a profit is somebody else's \
                 stake. staked={:?} claimed={:?}",
                a.hand_number, a.staked, a.claimed
            ),
        ));
    }
    out
}
