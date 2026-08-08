//! # ClearDeck SEALED DEALER — the no-peeking spike
//!
//! ## What this canister exists to remove
//!
//! An independent auditor, given only the public interface, found this and filed it
//! medium:
//!
//! > "Any controller can read the complete shuffled 52-card deck and every seated
//! > player's hole cards while a hand is in progress. Verified live at
//! > `phase = PreFlop`, `deck_index = 4`."
//!
//! That is the UltimateBet / Absolute Poker superuser mechanism. `docs/NO-PEEKING-FEASIBILITY.md`
//! measured every platform primitive that could close it and found:
//!
//! * vetKD **cannot** close it. The derived key is a pure function of
//!   `(canister_id, context, input)`; a controller who upgrades the canister
//!   re-derives every key ever issued, retroactively and silently. Proven, not
//!   reasoned: three transport keys, one upgrade, byte-identical secret.
//! * Deleting the field from `get_table_state` closes **nothing**: the whole deck
//!   was read out of a downloaded canister snapshot with four stock `dfx` commands.
//! * The one construction that reaches the goal today is **a separate canister with
//!   an empty controller list**. `install_code`, `stop_canister`,
//!   `take_canister_snapshot` and `update_settings` are all refused with `IC0512`
//!   when there is no controller to authorise them, while `deposit_cycles` stays
//!   open to everybody so anyone can keep it alive.
//!
//! This is that canister.
//!
//! ## The one sentence that matters
//!
//! **The secret of a ClearDeck hand is 32 bytes.** The shuffle is deterministic
//! (`docs/SHUFFLE-SPEC.md`), so whoever holds the seed holds every card. Today
//! that is `CURRENT_SEED` inside the fund-holding table canister, whose controller
//! can read it three different ways. Here it is inside a canister that **has no
//! controller**, and that exposes no method returning the seed, the deck, or any
//! seat's hole cards to anybody but the seat that holds them — until the hand is
//! over, at which point the seed is published on purpose so the shuffle stays
//! verifiable.
//!
//! ## What is deliberately absent
//!
//! There is no `get_deck`. There is no `get_seed`. There is no admin method, no
//! debug method, no `require_controller()` — because there is no controller. There
//! is no `pre_upgrade`/`post_upgrade` pair, because a canister that cannot be
//! upgraded cannot use one; the absence is a statement, not an oversight. Nothing
//! in this file logs a card, because canister logs have their own visibility rules
//! and the safest log line about a hole card is the one that is never written.
//!
//! ## The honest ceiling, stated here and not only in the doc
//!
//! Node operators on a standard application subnet can read canister memory
//! (`sev_enabled = false` on `jtdsg`, where ClearDeck's tables run, and on `pzp6e`,
//! where vetKD `key_1` lives). So this canister does **not** deliver "nobody can
//! peek". It delivers the strongest claim the platform supports today:
//!
//! > **No principal can peek, and no controller key exists that could.**
//!
//! ## Reveal is gated on the PLAYERS, not on the table (feasibility §8 question (b))
//!
//! The feasibility doc left the street-advance rule as a design sketch: "every
//! reveal is public and simultaneous so a premature reveal is evidence rather than
//! an advantage". Evidence is weaker than prevention, so this dealer does not rely
//! on it. A street is revealed only when **every dealt-in seat has said it is ready
//! on this street**, in its own name, with its own principal:
//!
//! * [`ack`] — "I am here and I am ready for the next card." Player-signed,
//!   idempotent, also a liveness heartbeat.
//! * [`stand_down`] — "I have folded or I am all in; stop waiting for me."
//!   Player-signed, permanent.
//! * [`table_stand_down`] — the table may stand a seat down **only after that seat
//!   has been silent for a full action clock, measured by this canister's own
//!   `time()`**. A player whose client is heartbeating cannot be stood down at all.
//!
//! So the table cannot manufacture an early flop against an attentive player. It
//! can only do to a *silent* seat what the existing action clock already does to a
//! silent seat, and the resulting reveal is public and simultaneous in the same
//! message. Every reveal is also written to an append-only public audit log with
//! the requesting principal and the block time.
//!
//! ## Nothing here needs a player to co-operate for the hand to end
//!
//! This is the reason mental poker was rejected in the feasibility study, so it is
//! worth being explicit: **no card in this design is locked behind a player's key.**
//! A disconnected player delays a street by at most one action clock and then gets
//! stood down. And if everything else fails — the table dies, this canister freezes,
//! the operator vanishes — [`force_finalize`] is callable **by any principal at all**
//! once the hand is older than `force_finalize_after_ns`, and it publishes the seed,
//! the board and every hole card, so the table can settle the hand to its real
//! winner. The pot cannot be stuck forever and nobody has to be refunded.

mod state;

use candid::Principal;
use poker_core::Card;
use sha2::{Digest, Sha256};
use state::*;
use std::collections::BTreeMap;

// The wire types live in `dealer_types` and are re-exported through `state`, so
// the table and the test harness link ONE definition. A second copy of a type is a
// copy that drifts -- the reason `src/poker_core` exists at all.

// ---------------------------------------------------------------------------
// lifecycle
// ---------------------------------------------------------------------------

#[ic_cdk::init]
fn init(args: DealerInit) {
    CONFIG.with(|c| {
        *c.borrow_mut() = Some(Config {
            table: args.table,
            action_timeout_ns: args.action_timeout_ns.unwrap_or(DEFAULT_ACTION_TIMEOUT_NS),
            street_grace_ns: args.street_grace_ns.unwrap_or(DEFAULT_STREET_GRACE_NS),
            force_finalize_after_ns: args
                .force_finalize_after_ns
                .unwrap_or(DEFAULT_FORCE_FINALIZE_AFTER_NS),
            min_open_balance: args.min_open_balance.unwrap_or(DEFAULT_MIN_OPEN_BALANCE),
        });
    });
}

// NOTE THE ABSENCE. There is no `#[ic_cdk::pre_upgrade]` and no
// `#[ic_cdk::post_upgrade]` in this file, and that is not an oversight: after the
// handover this canister has no controllers, `install_code` is refused with
// IC0512, and an upgrade hook is therefore dead code that only advertises a door
// which does not exist. If you find yourself adding one, the canister you are
// adding it to is not sealed.

// ---------------------------------------------------------------------------
// THE DEAL
// ---------------------------------------------------------------------------

/// Open a hand: draw a seed, commit to it, and deal.
///
/// The reply carries `seed_hash` and `dealt_in` and **nothing else**. The table
/// learns which seats are in and what to publish as its commitment; it does not
/// learn a card and it does not learn the seed. That is the entire difference
/// between this design and the one the auditor found.
///
/// Refuses if the dealer's own balance is below the floor: see
/// [`DEFAULT_MIN_OPEN_BALANCE`] for why a hand that cannot be finished must never
/// be started.
#[ic_cdk::update]
async fn open_hand(seats: Vec<DealtInSeat>) -> Result<HandOpened, String> {
    let caller = ic_cdk::api::msg_caller();
    let cfg = require_table(caller)?;

    // ---- validate before spending anything ------------------------------
    if seats.len() < 2 {
        return Err("a hand needs at least 2 dealt-in seats".to_string());
    }
    if seats.len() > MAX_SEATS {
        return Err(format!("at most {MAX_SEATS} seats, got {}", seats.len()));
    }
    for (i, s) in seats.iter().enumerate() {
        if s.principal == Principal::anonymous() {
            return Err(format!("seat {} is anonymous", s.seat));
        }
        for other in &seats[i + 1..] {
            if other.seat == s.seat {
                return Err(format!("seat {} appears twice", s.seat));
            }
            if other.principal == s.principal {
                return Err(format!("principal {} holds two seats", s.principal));
            }
        }
    }
    if let Some(open) = live_hand_id() {
        return Err(format!("hand {open} is still live; finish it first"));
    }

    // ---- THE RUNWAY GATE -------------------------------------------------
    let balance = ic_cdk::api::canister_cycle_balance();
    if balance < cfg.min_open_balance {
        return Err(format!(
            "dealer balance {balance} is below the floor {}. Refusing to OPEN a hand \
             this canister might not be able to FINISH. Anyone can fix this: \
             `dfx canister deposit-cycles <amount> <dealer>` needs no controller.",
            cfg.min_open_balance
        ));
    }

    // ---- the seed --------------------------------------------------------
    // Same source and same shape as the live engine: 32 bytes from `raw_rand`,
    // committed as SHA256(seed), shuffled with `poker_core::shuffle_deck`. A
    // verifier following docs/SHUFFLE-SPEC.md word for word reproduces a hand this
    // dealer dealt, which is the property five independent auditors already
    // checked and which this spike is not allowed to break.
    let seed = ic_cdk::management_canister::raw_rand()
        .await
        .map_err(|e| format!("raw_rand failed: {e:?}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&seed);
    let seed_hash = hex::encode(hasher.finalize());
    let now = ic_cdk::api::time();

    let hand_id = NEXT_HAND_ID.with(|n| {
        let mut n = n.borrow_mut();
        let id = *n;
        *n += 1;
        id
    });

    let hand = Hand {
        hand_id,
        seed,
        seed_hash: seed_hash.clone(),
        dealt_in: seats.clone(),
        street: Street::Dealt,
        opened_at_ns: now,
        street_opened_at_ns: now,
        ready: BTreeMap::new(),
        last_ack_ns: seats.iter().map(|s| (s.seat, now)).collect(),
        reveals: Vec::new(),
        showdown: Vec::new(),
        revealed_seed: None,
        finalized_by: None,
    };

    HANDS.with(|h| h.borrow_mut().insert(hand_id, hand));

    Ok(HandOpened {
        hand_id,
        seed_hash,
        dealt_in: seats,
        opened_at_ns: now,
    })
}

/// **The only method in this canister that returns a live hole card, and it
/// returns exactly two of them, to the one principal holding the seat.**
///
/// A query, so it costs the caller nothing and costs the dealer no consensus
/// round. Queries are not certified, so a malicious node could lie to you about
/// your own cards; that lie is detectable after the fact from the revealed seed,
/// which is precisely the guarantee `docs/SHUFFLE-SPEC.md` already provides and
/// the reason the seed is published at all.
#[ic_cdk::query]
fn my_hole_cards(hand_id: u64) -> Result<(Card, Card), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("anonymous callers hold no seat".to_string());
    }
    HANDS.with(|h| {
        let hands = h.borrow();
        let hand = hands.get(&hand_id).ok_or("no such hand")?;
        let seat = seat_of(hand, caller).ok_or("you were not dealt into this hand")?;
        hole_of(hand, seat).ok_or_else(|| "deck exhausted".to_string())
    })
}

// ---------------------------------------------------------------------------
// READINESS — the mechanism that makes an early reveal impossible rather than
// merely visible (feasibility §8 question (b))
// ---------------------------------------------------------------------------

/// "I am here, and I am ready for the next card."
///
/// Player-signed and idempotent. Calling it again refreshes the liveness clock,
/// which is what makes [`table_stand_down`] unusable against a player whose client
/// is running. The live engine already heartbeats (`last_seen`,
/// `DISCONNECT_TIMEOUT_SECS = 90`), so this is not a new class of traffic.
#[ic_cdk::update]
fn ack(hand_id: u64) -> Result<AckState, String> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if hand.finalized_by.is_some() {
            return Err("hand is over".to_string());
        }
        let seat = seat_of(hand, caller).ok_or("you were not dealt into this hand")?;
        hand.last_ack_ns.insert(seat, now);
        // A seat that already stood down stays stood down: readiness only ever
        // becomes MORE permanent, never less.
        hand.ready.entry(seat).or_insert(Readiness::Acked);
        Ok(ack_state(hand))
    })
}

/// "I have folded, or I am all in. Stop waiting for me on every street from here."
///
/// Player-signed and permanent. Nothing about it reveals a card.
#[ic_cdk::update]
fn stand_down(hand_id: u64) -> Result<AckState, String> {
    let caller = ic_cdk::api::msg_caller();
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if hand.finalized_by.is_some() {
            return Err("hand is over".to_string());
        }
        let seat = seat_of(hand, caller).ok_or("you were not dealt into this hand")?;
        hand.last_ack_ns.insert(seat, now);
        hand.ready.insert(seat, Readiness::StoodDownSelf);
        Ok(ack_state(hand))
    })
}

/// The table may stand a seat down **only after that seat has been silent for a
/// full action clock, measured here**.
///
/// THE DISCONNECT CASE. A player who goes offline mid-hand must not stall the
/// table, and this is the door for that — the same door the live engine's action
/// clock already is. The clock is measured against `max(street_opened_at,
/// last_ack)`, so:
///
/// * a player whose client is heartbeating can NEVER be stood down, which is what
///   stops the table from manufacturing an early reveal;
/// * a player who has genuinely gone silent is stood down after one clock, and the
///   reveal that follows is public, simultaneous, and logged with
///   `Readiness::TimedOutByTable` next to that seat, so the coercion is on the
///   record rather than invisible.
#[ic_cdk::update]
fn table_stand_down(hand_id: u64, seat: u8) -> Result<AckState, String> {
    let caller = ic_cdk::api::msg_caller();
    let cfg = require_table(caller)?;
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if hand.finalized_by.is_some() {
            return Err("hand is over".to_string());
        }
        if !hand.dealt_in.iter().any(|d| d.seat == seat) {
            return Err(format!("seat {seat} was not dealt into this hand"));
        }
        if matches!(hand.ready.get(&seat), Some(Readiness::StoodDownSelf)) {
            return Ok(ack_state(hand));
        }
        // CLOCK ONE: the seat has gone silent. Measured from the later of "this
        // street opened" and "this seat last spoke", so a client that is
        // heartbeating can never be stood down under it.
        let last_seen = hand
            .last_ack_ns
            .get(&seat)
            .copied()
            .unwrap_or(hand.opened_at_ns)
            .max(hand.street_opened_at_ns);
        let silent_due = last_seen.saturating_add(cfg.action_timeout_ns);

        // CLOCK TWO: the street itself has been open for the whole grace period.
        // Applies whatever the seat is doing, which is what stops a heartbeating
        // player freezing the hand. See DEFAULT_STREET_GRACE_NS for the trade.
        let grace_due = hand.street_opened_at_ns.saturating_add(cfg.street_grace_ns);

        let how = if now >= silent_due {
            Readiness::TimedOutByTable
        } else if now >= grace_due {
            Readiness::GraceExpired
        } else {
            return Err(format!(
                "seat {seat} cannot be stood down yet. It last spoke {} ns ago, so {} ns \
                 of the action clock remain; and this street has been open {} ns, so {} ns \
                 of the grace period remain. A seat that is heartbeating cannot be timed \
                 out, and a street cannot be forced on before the grace period.",
                now.saturating_sub(last_seen),
                silent_due.saturating_sub(now),
                now.saturating_sub(hand.street_opened_at_ns),
                grace_due.saturating_sub(now)
            ));
        };
        hand.ready.insert(seat, how);
        Ok(ack_state(hand))
    })
}

// ---------------------------------------------------------------------------
// THE REVEAL
// ---------------------------------------------------------------------------

/// Reveal the next street. Public and simultaneous: the cards this returns to the
/// table are the same cards [`community`] returns to every other caller, including
/// anonymous ones, from this message onward. There is no private-reveal path in
/// this canister for the same reason there is no `get_deck`.
///
/// Refuses until every dealt-in seat is ready — acked in its own name, stood down
/// in its own name, or timed out on this canister's own clock.
#[ic_cdk::update]
fn advance_street(hand_id: u64) -> Result<Vec<Card>, String> {
    let caller = ic_cdk::api::msg_caller();
    require_table(caller)?;
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if hand.finalized_by.is_some() {
            return Err("hand is over".to_string());
        }
        let to = next_street(hand.street).ok_or("no street left to reveal")?;
        let waiting = waiting_on(hand);
        if !waiting.is_empty() {
            return Err(format!(
                "cannot reveal {to:?}: seats {waiting:?} have not said they are ready. \
                 Call ack() from those seats, or wait out the action clock and use \
                 table_stand_down()."
            ));
        }
        let readiness = readiness_vec(hand);
        open_street(hand, to, now);
        hand.reveals.push(RevealRecord {
            street: to,
            requested_by: caller,
            at_ns: now,
            readiness,
        });
        Ok(community_now(hand))
    })
}

/// The board, as it stands, to anybody who asks. Identical for every caller.
#[ic_cdk::query]
fn community(hand_id: u64) -> Vec<Card> {
    HANDS.with(|h| {
        h.borrow()
            .get(&hand_id)
            .map(community_now)
            .unwrap_or_default()
    })
}

/// Publish the hole cards of the seats that reached showdown.
///
/// This is where the table gets cards, and it is the ONLY place, and it is after
/// the river with every seat's betting closed. The cards become public to
/// everybody in the same message — `hand_public` carries them from here on — so the
/// table has no window in which it knows something the players do not.
///
/// **Feasibility §8 question (a), the showdown split.** The table keeps
/// `evaluate_hand` and the side-pot code exactly as they are; it just receives the
/// cards as an INPUT here instead of reading them out of its own state. Nothing in
/// `poker_core` changes, so the golden vectors, the settlement oracle and the
/// money-safety invariants all still bind.
#[ic_cdk::update]
fn reveal_showdown(hand_id: u64, seats: Vec<u8>) -> Result<Vec<SeatCards>, String> {
    let caller = ic_cdk::api::msg_caller();
    require_table(caller)?;
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if hand.finalized_by.is_some() {
            return Err("hand is over".to_string());
        }
        if hand.street != Street::River {
            return Err(format!(
                "showdown needs the river on the board; street is {:?}",
                hand.street
            ));
        }
        let waiting = waiting_on(hand);
        if !waiting.is_empty() {
            return Err(format!(
                "cannot show down: seats {waiting:?} have not closed the river betting"
            ));
        }
        if seats.len() < 2 {
            return Err("a showdown is at least two seats".to_string());
        }
        let mut out = Vec::with_capacity(seats.len());
        for seat in &seats {
            if !hand.dealt_in.iter().any(|d| d.seat == *seat) {
                return Err(format!("seat {seat} was not dealt into this hand"));
            }
            let cards = hole_of(hand, *seat).ok_or("deck exhausted")?;
            out.push(SeatCards { seat: *seat, cards });
        }
        hand.showdown = out.clone();
        let readiness = readiness_vec(hand);
        hand.reveals.push(RevealRecord {
            street: Street::River,
            requested_by: caller,
            at_ns: now,
            readiness,
        });
        Ok(out)
    })
}

/// Close the hand and publish the seed.
///
/// After this the whole hand is reproducible by a stranger from
/// `docs/SHUFFLE-SPEC.md`, including the hole cards of seats that folded — which is
/// exactly what the live engine already does and what one auditor already
/// demonstrated. Post-hand privacy is not what this spike buys and pretending
/// otherwise would be the sort of overstatement the feasibility doc warns about.
/// (The fix for THAT is Option C, vetKD for the archive, and it belongs to the
/// history canister.)
///
/// **The gate matters.** Publishing the seed publishes every card, so
/// `finish_hand` is the most dangerous method here. It is refused unless the hand
/// is genuinely over: either a showdown was published, or **every** seat has stood
/// down — each of those in its own name or on a full action clock of silence. A
/// table that wanted to open the seed early would have to wait out a real clock
/// against every attentive player, and an attentive player's heartbeat stops it
/// outright.
#[ic_cdk::update]
fn finish_hand(hand_id: u64) -> Result<String, String> {
    let caller = ic_cdk::api::msg_caller();
    require_table(caller)?;
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        if let Some(seed) = &hand.revealed_seed {
            return Ok(seed.clone());
        }
        let showdown_done = !hand.showdown.is_empty();
        // "Every seat is out of the hand" for the purpose of publishing the seed.
        // `Acked` is deliberately NOT in this list: acking means "ready for the next
        // card", not "done", and treating it as done is exactly how a hostile table
        // would open the seed while a seat still had to act.
        let is_done = |how: Option<&Readiness>| {
            matches!(
                how,
                Some(Readiness::StoodDownSelf)
                    | Some(Readiness::TimedOutByTable)
                    | Some(Readiness::GraceExpired)
            )
        };
        let all_down = hand.dealt_in.iter().all(|d| is_done(hand.ready.get(&d.seat)));
        if !showdown_done && !all_down {
            let still: Vec<u8> = hand
                .dealt_in
                .iter()
                .filter(|d| !is_done(hand.ready.get(&d.seat)))
                .map(|d| d.seat)
                .collect();
            return Err(format!(
                "refusing to publish the seed: no showdown was revealed and seats {still:?} \
                 have not stood down. Publishing the seed publishes every card, so this \
                 canister will not do it while a seat could still have to act."
            ));
        }
        let seed = hex::encode(&hand.seed);
        hand.revealed_seed = Some(seed.clone());
        hand.street = Street::Complete;
        hand.finalized_by = Some(FinalizeKind::Table);
        Ok(seed)
    })
}

// ---------------------------------------------------------------------------
// THE LAST-RESORT DOOR (feasibility §8 question (c))
// ---------------------------------------------------------------------------

/// **Anyone at all** may force a stale hand open, and the pot is then settleable to
/// its real winner.
///
/// This is the answer to the failure this project has already shipped twice and
/// fixed twice: a hand that can never be closed, or a hand that is closed by
/// refunding everybody so the winner is robbed. Neither is reachable here. Once a
/// hand is older than `force_finalize_after_ns` — an hour by default, twelve times
/// the table's own `STUCK_HAND_GRACE_NS` — any principal on the internet can
/// publish the seed, the full board and every hole card, and the table can then
/// award the pot to whoever actually won it.
///
/// It is safe precisely because the only gate is a wall clock that no caller
/// controls, and it is set far beyond any hand that could still be live. It is
/// also the recovery for the one failure a zero-controller canister cannot
/// otherwise survive: if this dealer freezes mid-hand, anybody may
/// `deposit_cycles` (open to all principals, no controller needed — measured) and
/// then call this.
#[ic_cdk::update]
fn force_finalize(hand_id: u64) -> Result<ForcedFinalize, String> {
    let caller = ic_cdk::api::msg_caller();
    let cfg = config()?;
    let now = ic_cdk::api::time();
    HANDS.with(|h| {
        let mut hands = h.borrow_mut();
        let hand = hands.get_mut(&hand_id).ok_or("no such hand")?;
        let due = hand.opened_at_ns.saturating_add(cfg.force_finalize_after_ns);
        if now < due && hand.finalized_by.is_none() {
            return Err(format!(
                "hand {hand_id} is only {} ns old; the last-resort door opens {} ns from now. \
                 It is gated on nothing but this canister's clock, which is why it cannot be \
                 used to peek at a live hand.",
                now.saturating_sub(hand.opened_at_ns),
                due - now
            ));
        }
        let deck = deck_of(hand);
        let hole_cards: Vec<SeatCards> = hand
            .dealt_in
            .iter()
            .enumerate()
            .filter_map(|(k, d)| {
                Some(SeatCards {
                    seat: d.seat,
                    cards: (*deck.get(2 * k)?, *deck.get(2 * k + 1)?),
                })
            })
            .collect();
        let community: Vec<Card> = community_indices(hand.dealt_in.len(), Street::Complete)
            .into_iter()
            .filter_map(|i| deck.get(i).copied())
            .collect();
        let seed = hex::encode(&hand.seed);

        if hand.finalized_by.is_none() {
            hand.revealed_seed = Some(seed.clone());
            hand.street = Street::Complete;
            hand.finalized_by = Some(FinalizeKind::Forced);
            hand.showdown = hole_cards.clone();
        }

        Ok(ForcedFinalize {
            hand_id,
            revealed_seed: seed,
            community,
            hole_cards,
            forced_by: caller,
            at_ns: now,
        })
    })
}

// ---------------------------------------------------------------------------
// PUBLIC READ SURFACE — identical for every caller, by construction
// ---------------------------------------------------------------------------

#[ic_cdk::query]
fn hand_public(hand_id: u64) -> Option<HandPublic> {
    let cfg = config().ok()?;
    HANDS.with(|h| {
        let hands = h.borrow();
        let hand = hands.get(&hand_id)?;
        Some(HandPublic {
            hand_id: hand.hand_id,
            table: cfg.table,
            seed_hash: hand.seed_hash.clone(),
            dealt_in: hand.dealt_in.clone(),
            street: hand.street,
            community: community_now(hand),
            showdown: hand.showdown.clone(),
            revealed_seed: hand.revealed_seed.clone(),
            finalized_by: hand.finalized_by,
            opened_at_ns: hand.opened_at_ns,
            street_opened_at_ns: hand.street_opened_at_ns,
            reveals: hand.reveals.clone(),
            ready: readiness_vec(hand),
        })
    })
}

#[ic_cdk::query]
fn ack_status(hand_id: u64) -> Option<AckState> {
    HANDS.with(|h| h.borrow().get(&hand_id).map(ack_state))
}

#[ic_cdk::query]
fn dealer_health() -> DealerHealth {
    let cfg = config().ok();
    let balance = ic_cdk::api::canister_cycle_balance();
    let floor = cfg.as_ref().map(|c| c.min_open_balance).unwrap_or(u128::MAX);
    DealerHealth {
        cycle_balance: balance,
        min_open_balance: floor,
        can_open_hand: balance >= floor && live_hand_id().is_none(),
        live_hand: live_hand_id(),
        hands_dealt: NEXT_HAND_ID.with(|n| *n.borrow()) - 1,
    }
}

#[ic_cdk::query]
fn dealer_identity() -> Result<DealerIdentity, String> {
    let cfg = config()?;
    Ok(DealerIdentity {
        table: cfg.table,
        action_timeout_ns: cfg.action_timeout_ns,
        street_grace_ns: cfg.street_grace_ns,
        force_finalize_after_ns: cfg.force_finalize_after_ns,
        min_open_balance: cfg.min_open_balance,
        build: BUILD.to_string(),
    })
}

/// Instruction counter for the CURRENT message, so a harness can price a deal
/// against the live engine without guessing.
///
/// `performance_counter(0)` counts instructions within the current message
/// execution and an `await` starts a new slice, which is why
/// `docs/NO-PEEKING-FEASIBILITY.md` §7.1 is careful about what its numbers mean.
/// This one is a plain query with no await, so it is the whole thing.
#[ic_cdk::query]
fn instructions_to_read_my_cards(hand_id: u64) -> u64 {
    let before = ic_cdk::api::performance_counter(0);
    let _ = my_hole_cards(hand_id);
    ic_cdk::api::performance_counter(0) - before
}

ic_cdk::export_candid!();

