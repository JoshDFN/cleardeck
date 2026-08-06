//! What a player can READ about money of theirs that is committed to a hand.
//!
//! # Why this module exists
//!
//! Every other reader in this harness is a controller. `get_table_state` is
//! controller-only, `admin_get_all_balances` is controller-only, and the ledger
//! is read from outside the canister entirely. That is the right anchor for
//! conservation -- and it is precisely why the harness could not see
//! [FINDING 18](../../../docs/SECURITY-FINDINGS.md#finding-18): the money was all
//! there, attributed to the right principal, in a field only an operator can
//! read, while **every surface the player themselves can call returned zero.**
//!
//! An independent auditor reached that state in ordinary play:
//!
//! ```text
//! cash_out(bob) -> Ok = 0        while 298_000_000 e8s of bob's was in the pot
//! get_balance() -> 0
//! withdraw(...) -> "Insufficient balance. Have: 0.00000000 ICP"
//! ```
//!
//! So this module reads the canister **as the player**, through the caller-scoped
//! surfaces and nothing else, and answers one question: *does any of them say the
//! money exists?*
//!
//! # It must work against a build that does not have the surfaces
//!
//! A probe that only compiles against the fixed canister cannot convict the
//! defect. Every read here therefore degrades to `None` -- "this surface reports
//! nothing" -- when the method is missing, the call is rejected, or the reply
//! cannot be decoded as something that carries the figure. On the pre-fix module
//! that is exactly what happens, and the invariant fires with a message that says
//! which doors it knocked on.

use candid::{decode_one, CandidType, Encode, Principal};
use serde::Deserialize;

use super::{phase_of, Invariant, Severity, Violation};
use crate::table_api::TableState;
use crate::world::{Snapshot, World};

/// Reply of `get_custody_status()`: everything the canister owes THIS CALLER,
/// including the part that is not in their escrow balance.
///
/// Mirrored from `src/table_canister/src/lib.rs`. Narrow on purpose -- Candid
/// record subtyping lets the decoder drop fields this does not declare, so a
/// display field added later does not break the harness.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct CustodyStatus {
    /// Withdrawable now: the same number `get_balance()` returns.
    pub escrow: u64,
    /// Chips in front of the caller at the table.
    pub chips_at_table: u64,
    /// The caller's own money in the CURRENT hand's pot: their live seat's stake
    /// plus any stake recorded for a seat of theirs that has already been
    /// vacated.
    pub committed_in_pot: u64,
    /// True when the hand holding `committed_in_pot` can no longer be moved by
    /// any message, so `abandon_stuck_hand()` would refund it right now.
    pub committed_is_stuck: bool,
    /// Nanoseconds until the hand becomes abandonable. `None` when it already is,
    /// or when nothing is committed.
    pub abandonable_in_ns: Option<u64>,
    /// Everything above, added up: what the canister holds for this caller.
    pub total: u64,
    /// Plain-language next step, naming the method by name when one is needed.
    pub advice: String,
}

/// The part of `get_table_view()` this module reads.
///
/// Deliberately NOT the full mirror: the view carries thirty display fields that
/// have nothing to do with custody, and a full mirror would break on every
/// unrelated addition. Only the custody figures are declared, so decoding
/// succeeds exactly when the view carries them.
///
/// # A build without the figure decodes as `null`, not as an error
///
/// The reply type is `opt TableView`, and Candid's rule for `opt t` is that a
/// value which cannot be read as `t` arrives as **null** rather than as a failure
/// -- the same rule that destroys a table's chips when a persisted record gains a
/// non-`opt` field (docs/SECURITY-FINDINGS.md FINDING 14). So on the pre-fix
/// module this decodes to `None` and looks exactly like "there is no table". It
/// is disambiguated against [`AnyView`] before anything is reported.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct CustodyView {
    pub pot: u64,
    pub my_seat: Option<u8>,
    /// The caller's own stake in this pot. Present only on a build that surfaces
    /// it; a build without it decodes as `null` per the note above, which is the
    /// honest answer to "does the table view report it".
    pub my_committed_in_pot: u64,
    /// True when the hand cannot be moved by any message.
    pub hand_is_unmovable: bool,
}

/// The narrowest possible view: enough to tell "there is a table" from "the view
/// does not carry the custody figures". Every build has `pot`.
#[derive(Clone, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub struct AnyView {
    pub pot: u64,
}

/// What each caller-readable surface says about `who`'s committed stake.
///
/// `None` means the surface does not report the figure at all -- the method does
/// not exist on this build, the call was rejected, or the reply does not carry
/// it. That is a stronger statement than `Some(0)` and the two must not be
/// confused: `Some(0)` is the canister answering "nothing of yours is in a pot",
/// which is a claim it can be wrong about.
#[derive(Clone, Debug, Default)]
pub struct CustodySurfaces {
    /// `get_balance()`. Always decodable; reports escrow only, by construction.
    pub escrow: u64,
    /// `get_custody_status().committed_in_pot`.
    pub custody_status: Option<u64>,
    /// `get_table_view().my_committed_in_pot`.
    pub table_view: Option<u64>,
    /// Every door that was knocked on and what it answered, for the failure
    /// message. A gate that says "invisible" without saying where it looked is
    /// not evidence.
    pub log: Vec<String>,
}

impl CustodySurfaces {
    /// The largest committed figure any surface reports. Zero when every surface
    /// reports zero or reports nothing at all.
    pub fn best_report(&self) -> u64 {
        self.custody_status
            .unwrap_or(0)
            .max(self.table_view.unwrap_or(0))
    }

    /// True when NO surface reports a positive committed stake.
    pub fn reports_nothing(&self) -> bool {
        self.best_report() == 0
    }
}

/// Read every caller-scoped surface as `who`. Queries only: this is called from
/// the fuzzer's per-step invariant loop and must not move a single e8.
pub fn read_surfaces(world: &World, who: Principal) -> CustodySurfaces {
    let mut out = CustodySurfaces {
        escrow: world.get_balance(who),
        ..Default::default()
    };
    out.log
        .push(format!("get_balance() -> {} (escrow only)", out.escrow));

    match world.query_as(who, "get_custody_status", Encode!().unwrap()) {
        Ok(bytes) => match decode_one::<CustodyStatus>(&bytes) {
            Ok(status) => {
                out.custody_status = Some(status.committed_in_pot);
                out.log.push(format!(
                    "get_custody_status() -> committed_in_pot={} stuck={} advice={:?}",
                    status.committed_in_pot, status.committed_is_stuck, status.advice
                ));
            }
            Err(e) => out
                .log
                .push(format!("get_custody_status() reply does not decode: {e}")),
        },
        Err(e) => out.log.push(format!(
            "get_custody_status() is not answerable on this build: {e:?}"
        )),
    }

    match world.query_as(who, "get_table_view", Encode!().unwrap()) {
        Ok(bytes) => match decode_one::<Option<CustodyView>>(&bytes) {
            Ok(Some(view)) => {
                out.table_view = Some(view.my_committed_in_pot);
                out.log.push(format!(
                    "get_table_view() -> my_committed_in_pot={} unmovable={} pot={}",
                    view.my_committed_in_pot, view.hand_is_unmovable, view.pot
                ));
            }
            Ok(None) => {
                // `null` here is ambiguous: no table, or a view that does not
                // carry the custody figures (see the note on `CustodyView`). Ask
                // the narrower question to find out which.
                let any = decode_one::<Option<AnyView>>(&bytes).ok().flatten();
                out.log.push(match any {
                    Some(v) => format!(
                        "get_table_view() answers (pot={}) but carries NO custody figure",
                        v.pot
                    ),
                    None => "get_table_view() -> null (no table)".to_string(),
                });
            }
            Err(e) => out.log.push(format!(
                "get_table_view() carries no custody figure on this build: {e}"
            )),
        },
        Err(e) => out
            .log
            .push(format!("get_table_view() was rejected: {e:?}")),
    }

    out
}

/// The GROUND TRUTH: what each principal has in the current hand's pot, read
/// from the controller-only `get_table_state`.
///
/// A live seat's `total_bet_this_hand` plus every `departed_stakes` entry for
/// THIS hand, by owner. Both halves are needed: the whole shape of FINDING 18 is
/// a stake whose seat is gone, and the whole shape of FINDING 13 is a stake whose
/// owner is not the seat's occupant, so the sum is taken by PRINCIPAL and never
/// by seat.
pub fn committed_by_principal(t: &TableState) -> Vec<(Principal, u64)> {
    if !t.phase.hand_in_progress() {
        return Vec::new();
    }
    let mut out: Vec<(Principal, u64)> = Vec::new();
    let mut add = |who: Principal, amount: u64| {
        if amount == 0 {
            return;
        }
        match out.iter_mut().find(|(p, _)| *p == who) {
            Some((_, a)) => *a = a.saturating_add(amount),
            None => out.push((who, amount)),
        }
    };
    for p in t.players.iter().flatten() {
        add(p.principal, p.total_bet_this_hand);
    }
    for d in t.departed().iter().filter(|d| d.hand_number == t.hand_number) {
        add(d.principal, d.contributed);
    }
    out
}

/// M10 CUSTODY VISIBILITY.
///
/// > If a principal has a positive stake in a pot, at least one surface that
/// > principal can read must say so.
///
/// # What it fires on, and what it deliberately does not
///
/// A **seated** player in a hand that is still moving is not the subject. Their
/// stake is on the table in front of them, the pot is on the same screen, and the
/// money is contested in the ordinary way: folding and losing it is poker, not a
/// custody failure. Firing there would make the invariant noise.
///
/// It fires on the two shapes where the player is told nothing at all:
///
///  1. **the stake outlived the seat.** The principal is no longer at the table
///     and their money is still in the pot. Every ordinary surface is now scoped
///     to a chair they do not occupy;
///  2. **the hand cannot move.** Nobody can win the pot, so the stake is not
///     contested any more -- it is a refund waiting for somebody to ask for it,
///     and the player has to know it is there to ask.
///
/// Plus the state that produces both: a live hand with money in it and **nobody
/// seated**. Nothing can advance it, no client is polling it, and the only reason
/// it is not already lost is a method (`abandon_stuck_hand`) that no surface names.
///
/// # Cost
///
/// Two queries per affected principal, and affected principals are rare: the
/// early return costs nothing on the overwhelming majority of fuzz steps.
pub fn check_custody_is_visible(world: &World, snap: &Snapshot) -> Vec<Violation> {
    let t = &snap.table;
    let mut out = Vec::new();

    if !t.phase.hand_in_progress() {
        return out;
    }

    let committed = committed_by_principal(t);
    let seated_count = t.seated().count();

    // The state that should not persist: money in a live hand and nobody in it.
    if seated_count == 0 && (t.pot > 0 || !committed.is_empty()) {
        out.push(Violation::new(
            Invariant::M10CustodyVisibility,
            "live_hand_with_no_players",
            Severity::CustodyInvisible,
            0,
            phase_of(t),
            format!(
                "the canister is holding a LIVE hand ({:?}) with a pot of {} e8s and ZERO seated \
                 players. Nothing can advance it, nobody is polling it, and every principal whose \
                 money is in it has already been told by cash_out/leave_table that they got \
                 everything back. stakes={:?}",
                t.phase,
                t.pot,
                committed
                    .iter()
                    .map(|(p, a)| format!("{}={}", p.to_text(), a))
                    .collect::<Vec<_>>()
            ),
        ));
    }

    if committed.is_empty() {
        return out;
    }

    // One query, and only when somebody has a stake at all.
    let unmovable = world.stuck_hand_status().is_stuck;

    for (who, amount) in committed {
        let seated = t.seat_of(who).is_some();
        if seated && !unmovable {
            continue;
        }
        let surfaces = read_surfaces(world, who);
        let where_they_are = if seated { "seated" } else { "NOT AT THE TABLE" };
        let and_the_hand = if unmovable {
            ", and the hand can no longer be moved by any message"
        } else {
            ""
        };

        if surfaces.reports_nothing() {
            out.push(Violation::new(
                Invariant::M10CustodyVisibility,
                "committed_stake_is_reported_to_its_owner",
                Severity::CustodyInvisible,
                0,
                phase_of(t),
                format!(
                    "{} e8s of {}'s is in hand {}'s pot ({}{}), and every surface they can read \
                     reports zero:\n      {}",
                    amount,
                    who.to_text(),
                    t.hand_number,
                    where_they_are,
                    and_the_hand,
                    surfaces.log.join("\n      ")
                ),
            ));
            continue;
        }

        // A FIGURE IS NOT A REPORT UNLESS IT IS THE RIGHT FIGURE.
        //
        // "Some surface said something non-zero" is the weak form of this
        // invariant, and it is the form that would let a wrong number through:
        // a surface reporting somebody ELSE's stake, or the pot instead of the
        // caller's share, satisfies it while telling the player something false
        // about their own money. That is docs/SECURITY-FINDINGS.md FINDING 13's
        // shape applied to a display field, so the check is equality against the
        // ground truth, per surface.
        for (name, reported) in [
            ("get_custody_status().committed_in_pot", surfaces.custody_status),
            ("get_table_view().my_committed_in_pot", surfaces.table_view),
        ] {
            let Some(reported) = reported else { continue };
            if reported == amount {
                continue;
            }
            out.push(Violation::new(
                Invariant::M10CustodyVisibility,
                "committed_stake_is_reported_ACCURATELY",
                Severity::CustodyInvisible,
                reported as i128 - amount as i128,
                phase_of(t),
                format!(
                    "{name} reports {reported} e8s for {} in hand {} ({}{}), but the payout basis \
                     says {amount}. A figure that is not the player's own stake is worse than no \
                     figure: it is a number they will act on.\n      {}",
                    who.to_text(),
                    t.hand_number,
                    where_they_are,
                    and_the_hand,
                    surfaces.log.join("\n      ")
                ),
            ));
        }
    }

    out
}
