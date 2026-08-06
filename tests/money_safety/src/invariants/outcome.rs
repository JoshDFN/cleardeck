//! M11 OUTCOME — WHO WON, asked of the hand rather than of the arithmetic.
//!
//! # Why this file exists
//!
//! Three times now this project has produced a defect of exactly one shape:
//! **correct totals, wrong recipients, every invariant quiet.**
//!
//!   1. FINDING 13 — a departed player's stake refunded to whoever took their
//!      chair. Conserving, exact, right seat, wrong person.
//!   2. FINDING 16 / wave 6 — `plan_payouts`'s no-claimant branch handing every
//!      stake back to its funder. Conserving, exact, and the hand un-played.
//!   3. FINDING 17 / E-36 — a cardless `Active` seat propping up
//!      `count_active_players` so the fold-out fired on a seat `live_claims` could
//!      not pay. Measured: a 52,000,000 e8 pot, three seats, all three ending on
//!      exactly their buy-in; the folder refunded, the winner paid nothing. **No
//!      trap, no `CRITICAL:` line, exact conservation, M1 through M9 silent, drain
//!      clean.**
//!
//! M1..M6 ask whether the arithmetic is right. M8 asks whether the money reached
//! the person the RECORD names, and — through the settlement oracle — the person
//! the RULES name at the moment the hand ended. **None of them asks whether the
//! hand ended at the right moment**, and FINDING 17 is precisely a hand that was
//! allowed to run on past the point where it was decided. By the time it settled,
//! the last card-holder had been folded by his own clock, so the oracle looking at
//! the final state agreed with the canister: nobody held a claim, refund everybody.
//! The oracle was right about the state it was shown. The state should not have
//! existed.
//!
//! # The invariant, in poker terms
//!
//! The instant only one player still holds cards, the hand is over and that player
//! takes the pot. There is no such thing as a live hand with one claimant, and
//! there is certainly no such thing as a live hand with none. So:
//!
//!   **M11a** a live hand with money in it may never be OBSERVED with one claimant
//!           or with none. A correct engine settles inside the same message that
//!           takes the second-to-last claimant out, so this state is not merely
//!           wrong, it is unreachable between messages.
//!
//!   **M11b** if such a moment is observed anyway, the seat that held that last
//!           claim must be the seat that is paid, by exactly the pot that was
//!           contestable at that moment. This is the leg that convicts FINDING 17
//!           in money rather than in structure.
//!
//!   **M11b'** and, because a correct engine never lets M11a's state be observed
//!           at all, the same property stated from the other end and evaluated on
//!           EVERY settled hand: the claimant set of a hand only ever shrinks, so
//!           anybody who comes out of it AHEAD must have been holding a claim at
//!           the last moment it was seen live. This is the leg that gives the
//!           settlement side of M11 real coverage rather than a coverage counter
//!           that reads zero.
//!
//!   **M11c** a REFUND-EVERYONE settlement — every funder handed back exactly what
//!           they put in, including the ones who folded — is a violation unless the
//!           hand genuinely never had a claimant. Somebody who gave up their claim
//!           does not get their money back while somebody else still holds one.
//!
//! M11c is the dual of M8's `a_relinquished_stake_cannot_profit`, which fires on a
//! relinquished stake ending AHEAD. This one fires on it ending LEVEL, which is the
//! shape all three of the defects above actually had.
//!
//! # What this deliberately declines to answer
//!
//! A hand in which a seat was VACATED mid-hand (`leave_table`, `cash_out` of a
//! stuck hand) can legitimately show a claimant count that fell without a
//! settlement, because `cash_out` removes the seat without running the settlement
//! path at all — that is docs/SECURITY-FINDINGS.md FINDING 18, a different defect
//! with a different owner. M11a is skipped for those hands and the skip is COUNTED,
//! never swallowed: [`OutcomeCoverage::declined`] carries the reason and the fuzz
//! driver prints it. M11b and M11c still run on them.

use candid::Principal;
use std::collections::{BTreeMap, BTreeSet};

use crate::hand_attribution::HandAttribution;
use crate::table_api::{Player, TableState};
use crate::world::Snapshot;

use super::{Invariant, Severity, Violation};

// ---------------------------------------------------------------------------
// the claim, read off a snapshot with the ENGINE'S OWN RULE
// ---------------------------------------------------------------------------

/// Does this seat hold a live claim on the pot?
///
/// The harness's own statement of `table_canister::is_in_hand`, written out here
/// rather than imported so the gate does not agree with the engine by construction:
/// dealt into this hand (`hole_cards.is_some()`) and has not given the claim up
/// (`!has_folded`). Nothing about `status`. If the engine's predicate ever drifts
/// from this one, M11a fires, which is the point.
pub fn holds_a_live_claim(p: &Player) -> bool {
    !p.has_folded && p.hole_cards.is_some()
}

/// Every seat holding a live claim, as (seat, principal).
pub fn live_claims_of(t: &TableState) -> Vec<(u8, Principal)> {
    t.seated()
        .filter(|p| holds_a_live_claim(p))
        .map(|p| (p.seat, p.principal))
        .collect()
}

/// What every funder has in this hand right now: seated stakes plus the stakes
/// recorded for seats that have been vacated.
fn funders_now(t: &TableState) -> Vec<(u8, Principal, u64)> {
    let mut out: Vec<(u8, Principal, u64)> = t
        .seated()
        .filter(|p| p.total_bet_this_hand > 0)
        .map(|p| (p.seat, p.principal, p.total_bet_this_hand))
        .collect();
    for d in t.departed() {
        if d.hand_number == t.hand_number && d.contributed > 0 {
            out.push((d.seat, d.principal, d.contributed));
        }
    }
    out
}

/// The money that can actually be WON out of a set of contributions.
///
/// Anything the single largest contributor put in above the second largest was
/// never covered by anybody, so it is not contestable and comes back to them. This
/// is the uncalled-bet rule stated independently of the engine's
/// `return_uncalled_bet`, so the two are not the same code agreeing with itself.
fn capped(contributions: &[u64]) -> Vec<u64> {
    let mut sorted = contributions.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let cap = sorted.get(1).copied().unwrap_or(0);
    contributions.iter().map(|c| (*c).min(cap)).collect()
}

// ---------------------------------------------------------------------------
// the per-hand evidence
// ---------------------------------------------------------------------------

/// The moment a hand became decided: one seat left holding a claim.
#[derive(Clone, Debug)]
pub struct SoleClaimant {
    pub seat: u8,
    pub who: Principal,
    /// What this principal must gain over the whole hand if they are paid the pot
    /// they had just won: the contestable pot minus their own contestable share.
    pub owed: i128,
    /// Total contributions observed at that moment, for the message.
    pub pot_then: u64,
}

/// What one hand looked like from the outcome's point of view.
#[derive(Clone, Debug)]
pub struct HandOutcome {
    pub hand_number: u64,
    pub phase: crate::table_api::GamePhase,
    /// The last observed moment at which exactly one seat held a claim on a
    /// non-empty pot.
    pub sole_claimant: Option<SoleClaimant>,
    /// True if a live hand with money in it was ever observed with NO claimant.
    pub saw_no_claimant: bool,
    /// True if any seat held a claim at any point while the pot was non-empty.
    pub ever_had_a_claimant: bool,
    /// Principals holding a claim at the LAST observation in which the hand was
    /// live. Empty for a hand that ended with nobody able to win it.
    pub claimants_at_the_end: BTreeSet<Principal>,
    /// A seat was vacated during this hand, so M11a's structural leg does not
    /// apply. See the module docs.
    pub seat_vacated: bool,
    /// One principal had money at two seats in this hand: their per-principal
    /// arithmetic is ambiguous and M11b declines.
    pub double_seated: bool,
}

/// How much of a run M11 got to speak about.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct OutcomeCoverage {
    /// Hands that finished with the table idle again.
    pub hands_seen: u64,
    /// Hands at least one OUTCOME leg was evaluated on.
    pub hands_outcome_checked: u64,
    /// Hands that reached a decided-by-fold-out moment the harness could see, and
    /// therefore had M11b (the sharpest leg) evaluated on them.
    pub hands_foldout_checked: u64,
    /// Steps at which the structural leg M11a was evaluated.
    pub steps_structurally_checked: u64,
    /// Why hands were declined, counted by reason.
    pub declined: BTreeMap<String, u64>,
}

// ---------------------------------------------------------------------------
// the watch
// ---------------------------------------------------------------------------

/// Watches hands go by and produces one [`HandOutcome`] per completed hand, plus
/// the per-step structural violations.
///
/// Fed the snapshot the fuzz loop already takes, so it costs no extra messages.
#[derive(Clone, Debug, Default)]
pub struct OutcomeWatch {
    hand_number: u64,
    sole_claimant: Option<SoleClaimant>,
    saw_no_claimant: bool,
    ever_had_a_claimant: bool,
    claimants_at_the_end: BTreeSet<Principal>,
    seat_vacated: bool,
    double_seated: bool,
    seats_seen: BTreeMap<u8, Principal>,
    pub coverage: OutcomeCoverage,
}

impl OutcomeWatch {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset_for(&mut self, hand_number: u64) {
        self.hand_number = hand_number;
        self.sole_claimant = None;
        self.saw_no_claimant = false;
        self.ever_had_a_claimant = false;
        self.claimants_at_the_end = BTreeSet::new();
        self.seat_vacated = false;
        self.double_seated = false;
        self.seats_seen = BTreeMap::new();
    }

    /// Observe one snapshot.
    ///
    /// Returns `(per-step violations, Some(outcome) when a hand has just ended)`.
    pub fn observe(&mut self, snap: &Snapshot) -> (Vec<Violation>, Option<HandOutcome>) {
        let t = &snap.table;
        let mut out = Vec::new();

        if t.hand_number != self.hand_number {
            self.reset_for(t.hand_number);
        }

        // A chair that changes hands, or empties, during a live hand.
        if t.phase.hand_in_progress() {
            let occupied: BTreeMap<u8, Principal> =
                t.seated().map(|p| (p.seat, p.principal)).collect();
            for (seat, who) in &self.seats_seen {
                if occupied.get(seat) != Some(who) {
                    self.seat_vacated = true;
                }
            }
            for (seat, who) in occupied {
                self.seats_seen.insert(seat, who);
            }
            if !t.departed().is_empty() {
                self.seat_vacated = true;
            }
        }

        let live = t.phase.hand_in_progress();
        let funders = funders_now(t);
        let staked_total: u64 = funders.iter().fold(0u64, |a, (_, _, c)| a.saturating_add(*c));
        let claims = live_claims_of(t);

        if live && staked_total > 0 {
            if !claims.is_empty() {
                self.ever_had_a_claimant = true;
            }
            self.claimants_at_the_end = claims.iter().map(|(_, who)| *who).collect();

            // Is one principal funding two chairs in this hand? Their per-principal
            // net cannot be split between the chairs, so M11b declines.
            let mut owners: Vec<Principal> = funders.iter().map(|(_, who, _)| *who).collect();
            owners.sort();
            let unique: BTreeSet<Principal> = owners.iter().copied().collect();
            if unique.len() != owners.len() {
                self.double_seated = true;
            }

            // --- M11a, THE STRUCTURAL LEG -------------------------------------
            //
            // Skipped, and counted, on a hand somebody walked out of: `cash_out`
            // of a stuck hand removes the seat WITHOUT running the settlement
            // path, so a claimant count that falls with no settlement is FINDING
            // 18's question and not this one.
            if !self.seat_vacated {
                self.coverage.steps_structurally_checked += 1;
                if claims.len() == 1 {
                    out.push(Violation::new(
                        Invariant::M11Outcome,
                        "a_live_hand_may_not_run_on_with_one_claimant",
                        Severity::WrongOutcome,
                        0,
                        &t.phase,
                        format!(
                            "hand #{}: the hand is STILL LIVE with {staked_total} e8s in it and \
                             exactly ONE seat holding a claim (seat {}, {}). The instant only one \
                             player still holds cards the hand is over and that player takes the \
                             pot, so a correct engine settles inside the message that took the \
                             second-to-last claimant out and this state is unreachable between \
                             messages. It is reachable when some OTHER predicate is propping up \
                             the fold-out count: docs/SECURITY-FINDINGS.md FINDING 17. seats={:?}",
                            t.hand_number,
                            claims[0].0,
                            claims[0].1,
                            t.seated()
                                .map(|p| (
                                    p.seat,
                                    p.total_bet_this_hand,
                                    p.has_folded,
                                    p.hole_cards.is_some()
                                ))
                                .collect::<Vec<_>>()
                        ),
                    ));
                }
                if claims.is_empty() {
                    self.saw_no_claimant = true;
                    out.push(Violation::new(
                        Invariant::M11Outcome,
                        "a_live_hand_may_not_run_on_with_no_claimant_at_all",
                        Severity::WrongOutcome,
                        0,
                        &t.phase,
                        format!(
                            "hand #{}: the hand is STILL LIVE with {staked_total} e8s in it and \
                             NOBODY holding a claim on any of it. Whatever settles this hand can \
                             only refund it, and the money belongs to whoever won it before the \
                             last claim disappeared. seats={:?}",
                            t.hand_number,
                            t.seated()
                                .map(|p| (
                                    p.seat,
                                    p.total_bet_this_hand,
                                    p.has_folded,
                                    p.hole_cards.is_some()
                                ))
                                .collect::<Vec<_>>()
                        ),
                    ));
                }
            }

            // The decided-by-fold-out moment, recorded whether or not M11a fired.
            if claims.len() == 1 {
                let (seat, who) = claims[0];
                let amounts: Vec<u64> = funders.iter().map(|(_, _, c)| *c).collect();
                let contestable = capped(&amounts);
                let total: i128 = contestable.iter().fold(0i128, |a, c| a + *c as i128);
                let theirs: i128 = funders
                    .iter()
                    .zip(contestable.iter())
                    .filter(|((s, w, _), _)| *s == seat && *w == who)
                    .fold(0i128, |a, (_, c)| a + *c as i128);
                self.sole_claimant = Some(SoleClaimant {
                    seat,
                    who,
                    owed: total - theirs,
                    pot_then: staked_total,
                });
            }
        }

        // The hand is over when the table is idle with an empty pot.
        let produced = if !live && t.pot == 0 && self.hand_number > 0 {
            let outcome = HandOutcome {
                hand_number: self.hand_number,
                phase: t.phase.clone(),
                sole_claimant: self.sole_claimant.clone(),
                saw_no_claimant: self.saw_no_claimant,
                ever_had_a_claimant: self.ever_had_a_claimant,
                claimants_at_the_end: self.claimants_at_the_end.clone(),
                seat_vacated: self.seat_vacated,
                double_seated: self.double_seated,
            };
            // Only report a hand once.
            let fresh = self.ever_had_a_claimant
                || self.saw_no_claimant
                || self.sole_claimant.is_some();
            self.reset_for(0);
            if fresh {
                Some(outcome)
            } else {
                None
            }
        } else {
            None
        };

        (out, produced)
    }
}

// ---------------------------------------------------------------------------
// the settlement legs
// ---------------------------------------------------------------------------

/// M11b and M11c, evaluated once per completed hand.
///
/// Takes the [`HandAttribution`] measured over the same hand, because "what
/// actually happened to this person's money" is already reconstructed there from
/// `admin_get_all_balances` + `get_table_state` and there must be exactly one
/// definition of it.
pub fn check_hand_outcome(
    o: &HandOutcome,
    a: &HandAttribution,
    coverage: &mut OutcomeCoverage,
) -> Vec<Violation> {
    coverage.hands_seen += 1;
    let mut out = Vec::new();
    let mut checked = false;

    if !a.closed_world {
        *coverage
            .declined
            .entry("money crossed the canister boundary during the hand".to_string())
            .or_insert(0) += 1;
        return out;
    }

    // --- M11b: the seat that held the last claim is the seat that is paid ----
    if let Some(sc) = o.sole_claimant.as_ref() {
        if a.tainted.contains(&sc.who) {
            *coverage
                .declined
                .entry("the fold-out winner deposited or withdrew during the hand".to_string())
                .or_insert(0) += 1;
        } else if o.double_seated {
            *coverage
                .declined
                .entry("one principal funded two chairs in the hand".to_string())
                .or_insert(0) += 1;
        } else {
            checked = true;
            coverage.hands_foldout_checked += 1;
            let measured = a.measured.get(&sc.who).copied().unwrap_or(0);
            if measured != sc.owed {
                out.push(Violation::new(
                    Invariant::M11Outcome,
                    "the_seat_that_held_the_last_claim_is_the_seat_that_is_paid",
                    Severity::WrongOutcome,
                    measured - sc.owed,
                    &o.phase,
                    format!(
                        "hand #{}: seat {} ({}) was the LAST seat holding a claim on a {} e8 pot, \
                         so it won the hand and the rules owe it {:+} e8s across the hand. Its \
                         escrow+chips moved {:+}. Every total can still balance to the e8 and \
                         every stake can still be accounted for: what is wrong is WHO WON. \
                         See docs/SECURITY-FINDINGS.md FINDING 17. measured={:?} staked={:?} \
                         claimed={:?}",
                        o.hand_number,
                        sc.seat,
                        sc.who,
                        sc.pot_then,
                        sc.owed,
                        measured,
                        a.measured,
                        a.staked,
                        a.claimed
                    ),
                ));
            }
        }
    }

    // --- M11b (general): only a seat that HELD A CLAIM can come out ahead ----
    //
    // The sharp leg above only has something to say about a hand the harness saw
    // decided by fold-out, and on a correct engine that state is unobservable --
    // which would leave the settlement side of M11 running on nothing. This leg
    // runs on EVERY hand and states the same property from the other end.
    //
    // The claimant set of a hand only ever SHRINKS: folding is irreversible and
    // cards are cleared only when the next hand is dealt. So whoever the pot
    // eventually goes to must already have been holding a claim at the last moment
    // the harness saw the hand live. Anybody else who comes out ahead was paid out
    // of money they had given up their claim on -- which is FINDING 17's shape
    // (the folder refunded) and FINDING 13's (the stranger in the chair) at once.
    //
    // Needs no oracle, no cards, and no knowledge of who was winning.
    if o.ever_had_a_claimant && !o.claimants_at_the_end.is_empty() {
        checked = true;
        for (who, delta) in &a.measured {
            if *delta <= 0 || a.tainted.contains(who) {
                continue;
            }
            if o.claimants_at_the_end.contains(who) {
                continue;
            }
            out.push(Violation::new(
                Invariant::M11Outcome,
                "only_a_seat_that_held_a_claim_can_come_out_ahead",
                Severity::WrongOutcome,
                *delta,
                &o.phase,
                format!(
                    "hand #{}: principal {who} came out of this hand {delta:+} e8s ahead and was                      NOT holding a live claim on it at the last moment it was seen live. The                      claimants then were {:?}. A hand's claimant set only ever shrinks, so the                      winner must be one of them; anybody else who profits was paid out of money                      they had given up. See docs/SECURITY-FINDINGS.md FINDING 17 and FINDING 13.                      measured={:?} staked={:?} claimed={:?}",
                    o.hand_number, o.claimants_at_the_end, a.measured, a.staked, a.claimed
                ),
            ));
        }
    }

    // --- M11c: refund-everyone requires that nobody ever held a claim --------
    //
    // Needs the stake reconstruction, so it asks for a fully measurable hand.
    if a.is_measurable() {
        let funders: Vec<Principal> = a
            .staked
            .iter()
            .filter(|(who, amount)| **amount > 0 && !a.tainted.contains(*who))
            .map(|(who, _)| *who)
            .collect();
        let everybody_level = !funders.is_empty()
            && funders
                .iter()
                .all(|who| a.measured.get(who).copied().unwrap_or(0) == 0);
        if everybody_level {
            checked = true;
            if o.ever_had_a_claimant {
                // A chop of exactly equal contributions also leaves every funder
                // level, and it is legitimate. It is distinguished by there being
                // NOBODY who gave their claim up: in a refund-everyone the folders
                // are handed their money back, which is the whole defect.
                let relinquished_and_whole: Vec<&Principal> = a
                    .relinquished_only
                    .iter()
                    .filter(|who| a.staked.get(who).copied().unwrap_or(0) > 0)
                    .collect();
                if !relinquished_and_whole.is_empty() {
                    out.push(Violation::new(
                        Invariant::M11Outcome,
                        "a_refund_everyone_settlement_needs_nobody_to_have_held_a_claim",
                        Severity::WrongOutcome,
                        a.staked_total() as i128,
                        &o.phase,
                        format!(
                            "hand #{}: every funder came out of this hand EXACTLY LEVEL on a pot \
                             of {} e8s -- the hand was un-played and every stake went back to the \
                             person who put it in -- and {:?} had already given up their claim by \
                             folding or leaving. A player who folds does not get their money back \
                             while somebody else holds a claim on it. This hand had a claimant; \
                             the claimants at the last live observation were {:?}. \
                             See docs/SECURITY-FINDINGS.md FINDING 17. staked={:?} claimed={:?}",
                            o.hand_number,
                            a.staked_total(),
                            relinquished_and_whole,
                            o.claimants_at_the_end,
                            a.staked,
                            a.claimed
                        ),
                    ));
                }
            }
        }
    } else if o.sole_claimant.is_none() {
        *coverage
            .declined
            .entry(
                a.incompleteness
                    .clone()
                    .unwrap_or_else(|| "the hand could not be attributed".to_string())
                    .chars()
                    .take(64)
                    .collect::<String>(),
            )
            .or_insert(0) += 1;
    }

    if checked {
        coverage.hands_outcome_checked += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uncalled_excess_is_not_contestable() {
        // One player put in 50, another 2: only 2 of the 50 was ever covered.
        assert_eq!(capped(&[50, 2]), vec![2, 2]);
        // Equal contributions are entirely contestable.
        assert_eq!(capped(&[2, 2, 2]), vec![2, 2, 2]);
        // A single funder's money is all uncalled.
        assert_eq!(capped(&[7]), vec![0]);
        // Side-pot shape: the short stack caps nothing above the second largest.
        assert_eq!(capped(&[10, 6, 1]), vec![6, 6, 1]);
    }
}
