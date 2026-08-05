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
