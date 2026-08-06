//! M12 ARCHIVE FIDELITY -- the permanent record names the people who played.
//!
//! # Why this had to exist (docs/SECURITY-FINDINGS.md FINDING 30)
//!
//! Every other invariant in this harness asks about MONEY: is it conserved, is it
//! reachable, did the right PERSON get it, did the hand end when the rules say.
//! None of them asks whether the record ClearDeck publishes about the hand is
//! true, and that record is the entire product claim.
//!
//! `record_hand_to_history` built its player list from `state.players` AS THEY
//! STAND AT SETTLEMENT. Seats are not people and they do not even stay put inside
//! one hand: a player can leave a live hand with money in the pot, and somebody
//! else can buy the empty chair before it settles. So the permanent record
//! **omitted** the player who left and **invented** the one who arrived -- giving
//! the arrival a position and the departed player's starting stack, because
//! `STARTING_CHIPS` is keyed by seat. An auditor read archived hand 4 and found a
//! principal who never played it recorded as the small blind.
//!
//! Not one e8 moves wrongly in any of that. M1 through M11 are silent, correctly.
//!
//! # The three legs
//!
//! * [`check_archived_participants`] leg A -- STRUCTURAL, on every settled hand,
//!   needing nothing but the record: the record must say who played, must say who
//!   was dealt in, must add up, and must not name somebody who neither took a card
//!   nor put in a chip.
//! * leg B -- CROSS-CHECKED, on every hand the attribution watch could measure:
//!   what the record says each person contributed must equal what the harness
//!   watched them put in, reconstructed step by step from `total_bet_this_hand`
//!   and `departed_stakes` and never from the record.
//! * leg C -- THE VERIFIER'S QUESTION: `P` must be stated, and the people it names
//!   must be in the player list. docs/SHUFFLE-SPEC.md section 4 offsets the board
//!   by `P`; a hand whose `P` cannot be read off the record cannot be checked by
//!   anybody, which is what "provably fair" means here.
//!
//! Leg A is what makes this a gate on EVERY hand rather than on the ones the
//! harness happened to be able to attribute. Leg B is what makes leg A more than
//! the record agreeing with itself.
//!
//! # Where this gate CANNOT see, stated rather than discovered later
//!
//! * **It reads the TABLE's copy of the record, not the archive's.** Installing an
//!   archive canister in every fuzz world would cost a canister per run.
//!   `record_hand_to_history` builds the participant list ONCE and clones it into
//!   both, so they cannot differ -- and because "these two are built the same way"
//!   is exactly the claim that has been false four times in this project, it is
//!   asserted on the wire rather than argued:
//!   `the_tables_copy_of_the_record_and_the_archives_copy_are_the_same_list` in
//!   `tests/invariants/archive.rs` reads both and compares them.
//! * **Leg C does not check that `dealt_in` matches the cards actually dealt.** It
//!   checks that everybody in it is in the player list. A mutation that dropped a
//!   seat from `DEALT_IN` at deal time would give a coherent record with the wrong
//!   `P`, and only `the_board_can_be_reproduced_from_the_archived_record_alone`
//!   would convict it -- which it does, by re-deriving the board.
//! * **Leg B's `staked` is reconstructed from `total_bet_this_hand`**, so a defect
//!   in that field itself would fool both sides. That is M8's question, answered
//!   by the settlement oracle, not this one's.

use candid::Principal;
use std::collections::{BTreeMap, BTreeSet};

use crate::hand_attribution::HandAttribution;
use crate::invariants::{Invariant, Severity, Violation};
use crate::table_api::{GamePhase, HandHistoryAmounts};

fn v(check: &'static str, phase: &GamePhase, detail: String) -> Violation {
    // Zero delta, on purpose and like M10 and M11: nothing has gone missing, which
    // is exactly why every conservation invariant is quiet about it.
    Violation::new(
        Invariant::M12ArchiveFidelity,
        check,
        Severity::FalseRecord,
        0,
        phase,
        detail,
    )
}

/// Everything the record says each principal put in.
fn contributed_by_principal(h: &HandHistoryAmounts) -> BTreeMap<Principal, u64> {
    let mut out: BTreeMap<Principal, u64> = BTreeMap::new();
    for p in h.participants.iter().flatten() {
        let e = out.entry(p.principal).or_insert(0);
        *e = e.saturating_add(p.contributed.unwrap_or(0));
    }
    out
}

/// M12, all three legs, for one settled hand.
///
/// `attribution` is the harness's own reconstruction of the hand. Leg B runs only
/// when it is [`HandAttribution::complete`] -- the watch says so when its
/// reconstruction balances against what the canister paid out -- because an
/// instrument that cannot see the whole hand must abstain rather than invent a
/// finding. Legs A and C run always: they need nothing but the record.
pub fn check_archived_participants(
    attribution: &HandAttribution,
    h: &HandHistoryAmounts,
) -> Vec<Violation> {
    let mut out = Vec::new();
    let phase = &attribution.phase;
    let hand = attribution.hand_number;

    // ---- leg A: the record must exist and be internally true ---------------
    let Some(players) = h.participants.as_ref() else {
        out.push(v(
            "record_says_who_played",
            phase,
            format!(
                "hand {hand} settled and the permanent record does not say who played it. The \
                 archive is the only durable artifact behind 'provably fair'; a hand with no \
                 participant list cannot be checked by anybody. \
                 docs/SECURITY-FINDINGS.md FINDING 30."
            ),
        ));
        return out;
    };

    let awarded: u64 = h
        .winners
        .iter()
        .fold(0u64, |a, w| a.saturating_add(w.amount));
    let contributed_total: u64 = players
        .iter()
        .fold(0u64, |a, p| a.saturating_add(p.contributed.unwrap_or(0)));
    if contributed_total != awarded {
        out.push(v(
            "record_balances",
            phase,
            format!(
                "hand {hand}: the record says {contributed_total} e8s went in and {awarded} e8s \
                 came out. A record that does not balance is describing a hand that did not \
                 happen. participants={:?}",
                players
            ),
        ));
    }

    for p in players {
        if p.contributed == Some(0) && p.dealt_in == Some(false) {
            out.push(v(
                "record_invents_nobody",
                phase,
                format!(
                    "hand {hand}: the record names {} at seat {} who was not dealt a card and \
                     put in no money. Somebody who bought that chair after the deal is not a \
                     player in the hand, and recording them as one is a confident false record.",
                    p.principal, p.seat
                ),
            ));
        }
    }

    // ---- leg C: the verifier's question ------------------------------------
    match h.dealt_in.as_ref() {
        None => out.push(v(
            "record_states_the_deal",
            phase,
            format!(
                "hand {hand}: the record does not say how many players were dealt in, so \
                 docs/SHUFFLE-SPEC.md section 4 cannot offset the board and the shuffle cannot \
                 be verified from the record. Counting the players instead is what produced the \
                 WRONG BOARD on every hand somebody left."
            ),
        )),
        Some(dealt) => {
            if dealt.is_empty() && awarded > 0 {
                out.push(v(
                    "record_states_the_deal",
                    phase,
                    format!("hand {hand}: a pot of {awarded} e8s was played for and the record \
                             says nobody was dealt in."),
                ));
            }
            let named: BTreeSet<(u8, Principal)> =
                players.iter().map(|p| (p.seat, p.principal)).collect();
            for d in dealt {
                if !named.contains(&(d.seat, d.principal)) {
                    out.push(v(
                        "record_names_everyone_dealt_in",
                        phase,
                        format!(
                            "hand {hand}: {} was dealt in at seat {} and is not in the player \
                             list. Every hand this happens to is a hand a verifier will \
                             reproduce with the wrong number of players.",
                            d.principal, d.seat
                        ),
                    ));
                }
            }
        }
    }

    // ---- leg B: against what the harness watched, not what the record says --
    if !attribution.complete {
        return out;
    }
    let recorded = contributed_by_principal(h);
    let watched: BTreeMap<Principal, u64> = attribution
        .staked
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, v)| (*k, *v))
        .collect();
    let recorded_nonzero: BTreeMap<Principal, u64> = recorded
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, v)| (*k, *v))
        .collect();

    for (who, amount) in &watched {
        match recorded_nonzero.get(who) {
            None => out.push(v(
                "record_names_everyone_who_paid",
                phase,
                format!(
                    "hand {hand}: {who} put {amount} e8s into the pot and the permanent record \
                     does not name them. Measured from total_bet_this_hand and departed_stakes, \
                     never from the record. record={recorded_nonzero:?}"
                ),
            )),
            Some(recorded_amount) if recorded_amount != amount => out.push(v(
                "record_states_what_each_person_paid",
                phase,
                format!(
                    "hand {hand}: {who} put {amount} e8s in and the record says {recorded_amount}."
                ),
            )),
            Some(_) => {}
        }
    }
    for (who, amount) in &recorded_nonzero {
        if !watched.contains_key(who) {
            out.push(v(
                "record_names_only_who_paid",
                phase,
                format!(
                    "hand {hand}: the record says {who} put {amount} e8s in, and the harness \
                     watched them put in nothing. watched={watched:?}"
                ),
            ));
        }
    }

    out
}

/// How often M12 could ask its sharp question, so a green run cannot be a run in
/// which the sharp leg never fired -- the hole the wave-7 review found in M11.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ArchiveCoverage {
    /// Settled hands the structural legs ran on.
    pub hands_checked: u64,
    /// Of those, hands the cross-check against the watched stakes also ran on.
    pub hands_cross_checked: u64,
    /// Of those, hands where somebody left the table mid-hand -- the shape the
    /// defect needs. A run with zero of these has not tested FINDING 30 at all.
    pub hands_with_a_departure: u64,
}

impl ArchiveCoverage {
    pub fn observe(&mut self, attribution: &HandAttribution, h: &HandHistoryAmounts) {
        self.hands_checked += 1;
        if attribution.complete {
            self.hands_cross_checked += 1;
        }
        if h.participants
            .iter()
            .flatten()
            .any(|p| p.left_mid_hand == Some(true))
        {
            self.hands_with_a_departure += 1;
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "M12 ARCHIVE: {} settled hand(s) checked, {} cross-checked against the watched \
             stakes, {} with a mid-hand departure (the shape FINDING 30 needs)",
            self.hands_checked, self.hands_cross_checked, self.hands_with_a_departure
        )
    }
}
