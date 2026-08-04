//! Driving hands on the real canister.
//!
//! Two jobs:
//!
//! * put exact stacks in exact seats, so an all-in ladder is a decision and not an
//!   accident (`buy_in(seat, amount)` takes an exact amount, which is why the
//!   micro-stakes config exists);
//! * play a hand out under a stated betting policy, taking an observation after
//!   every message.
//!
//! Every `player_action` is preceded by a 250 ms clock advance. That is not
//! cosmetic: `check_rate_limit` allows ten actions per caller per second and
//! returns `Err` beyond that, so a hand driven as fast as the harness can send
//! would start being refused halfway through and the record would be of a
//! different hand than the one intended.

use std::time::Duration;

use crate::deal::{read_deal, DealPlan};
use crate::observe::{HandRecord, HandRecorder};
use crate::table_api::*;
use crate::world::{World, AUTO_DEAL_DELAY};

/// Clock advance before each action, to stay under `MAX_ACTIONS_PER_WINDOW`.
pub const ACTION_SPACING: Duration = Duration::from_millis(250);

/// How a seat should act when it is their turn.
///
/// Returning `None` means "no opinion": the driver then tries Check, then Call,
/// then Fold, which is the standard way to walk a hand to a showdown.
pub type Policy = Box<dyn FnMut(&TableState) -> Option<PlayerAction>>;

/// Check if possible, otherwise call. Produces showdowns with no post-flop money.
pub fn passive() -> Policy {
    Box::new(|_| None)
}

/// Everyone shoves at their first opportunity. With unequal stacks this is the
/// shortest route to a genuine side-pot ladder.
pub fn everyone_all_in() -> Policy {
    Box::new(|_| Some(PlayerAction::AllIn))
}

/// Bet `size` on every post-flop street when checked to, otherwise call.
/// This is the shape that reaches E-01: real money wagered after the flop.
pub fn bet_every_street(size: u64) -> Policy {
    Box::new(move |state: &TableState| match state.phase {
        GamePhase::PreFlop => None,
        _ => {
            if state.current_bet == 0 {
                Some(PlayerAction::Bet(size))
            } else {
                None
            }
        }
    })
}

/// Raise TO `total` for the street while the current bet is below it, then call.
/// `PlayerAction::Raise(x)` in the engine means "raise to x", not "raise by x".
pub fn raise_to(total: u64) -> Policy {
    Box::new(move |state: &TableState| {
        if state.current_bet < total {
            Some(PlayerAction::Raise(total))
        } else {
            None
        }
    })
}

/// `shover` shoves; everybody else raises to `total` and then calls. The shape
/// that puts a short stack all-in underneath a much bigger pot.
pub fn shove_one_else_raise_to(shover: u8, total: u64) -> Policy {
    Box::new(move |state: &TableState| {
        if state.action_on == shover {
            Some(PlayerAction::AllIn)
        } else if state.current_bet < total {
            Some(PlayerAction::Raise(total))
        } else {
            None
        }
    })
}

/// Result of driving a hand: the record plus whether the hand actually finished.
pub struct DrivenHand {
    pub record: HandRecord,
    pub completed: bool,
    pub actions_taken: usize,
}

/// Fund escrow for each named actor. Amount is per actor, pulled through the real
/// ICRC-2 approve + `deposit` path.
pub fn fund(world: &World, names: &[&str], each: u64) {
    for name in names {
        let who = world.actor(name);
        world
            .fund_escrow(who, each)
            .unwrap_or_else(|e| panic!("fund_escrow for {name} failed: {e}"));
    }
}

/// Seat `(name, seat, stack)` triples with EXACTLY `stack` chips each.
pub fn seat_exact(world: &World, seats: &[(&str, u8, u64)]) {
    for (name, seat, stack) in seats {
        let who = world.actor(name);
        world
            .buy_in(who, *seat, *stack)
            .unwrap_or_else(|e| panic!("buy_in({name}, seat {seat}, {stack}) failed: {e}"));
    }
    world.advance(AUTO_DEAL_DELAY + Duration::from_secs(1));
}

/// Start a hand and read the deal off the deck.
pub fn deal(world: &mut World, recorder: &mut HandRecorder) -> Result<DealPlan, String> {
    let dealer = world.actors[0].principal;
    world
        .start_new_hand(dealer)
        .map_err(|e| format!("start_new_hand failed: {e}"))?;
    recorder.observe(world);
    let state = world.table_state();
    let plan = read_deal(&state).ok_or_else(|| {
        format!(
            "could not read the deal: phase {:?}, {} community cards, deck_index {}",
            state.phase,
            state.community_cards.len(),
            state.deck_index
        )
    })?;
    recorder.note_predicted_board(plan.board.clone());
    Ok(plan)
}

/// Everyone in `keep` shoves; everyone else folds. The shortest route to an
/// all-in ladder among exactly the seats named.
pub fn all_in_only(keep: Vec<u8>) -> Policy {
    Box::new(move |state: &TableState| {
        if keep.contains(&state.action_on) {
            Some(PlayerAction::AllIn)
        } else {
            Some(PlayerAction::Fold)
        }
    })
}

/// Everyone folds. Used for the fold-out class.
pub fn everyone_folds() -> Policy {
    Box::new(|_| Some(PlayerAction::Fold))
}

/// Send one action for a named seat, spacing the clock and observing after.
pub fn act_seat(
    world: &mut World,
    recorder: &mut HandRecorder,
    seat: u8,
    action: PlayerAction,
) -> Result<(), String> {
    let state = world.table_state();
    let who = state
        .player_at(seat)
        .map(|p| p.principal)
        .ok_or_else(|| format!("seat {seat} is empty"))?;
    world.advance(ACTION_SPACING);
    let out = world
        .player_action(who, action)
        .map_err(|e| format!("seat {seat} {action:?}: {e}"));
    recorder.observe(world);
    out
}

/// Send one action for whoever is to act.
pub fn act_on_turn(
    world: &mut World,
    recorder: &mut HandRecorder,
    action: PlayerAction,
) -> Result<u8, String> {
    let seat = world.table_state().action_on;
    act_seat(world, recorder, seat, action).map(|_| seat)
}

/// Drive pre-flop so that ONE seat folds with real money in and then leaves the
/// table while the betting round is still open.
///
/// That window matters and it is narrow. `calculate_side_pots` runs once, at the
/// pre-flop-to-flop transition (or inside `run_out_board`), and nothing recomputes
/// it afterwards. So a seat that vacates AFTER the flop is harmless -- the frozen
/// breakdown still names it -- and only a seat that vacates BEFORE the pots are
/// built can make the breakdown disagree with the money. Reaching that state needs
/// an orbit where the last player to act raises, so the earlier callers get a
/// second turn and one of them can fold with a real stake already committed:
///
/// ```text
///   orbit 1   first deep seat raises to `first`      (a real commitment)
///             middle deep seats call
///             the short seat shoves
///             last deep seat raises to `second`
///   orbit 2   the first deep seat now faces `second` with only `first` in
///             -> it FOLDS and LEAVES, with two seats still able to act
///             the remaining caller calls `second`, the round closes, and the
///             side pots are built with that seat's money in the pot and the
///             seat itself gone
/// ```
///
/// Returns the seat that vacated, if the window was reached.
pub fn vacate_a_committed_seat_preflop(
    world: &mut World,
    recorder: &mut HandRecorder,
    short_seat: u8,
    first: u64,
    second: u64,
) -> Option<u8> {
    let deep_total = world
        .table_state()
        .seated()
        .filter(|p| p.seat != short_seat)
        .count();
    let mut acted: Vec<u8> = Vec::new();
    let mut vacated: Option<u8> = None;

    for _ in 0..32 {
        let state = world.table_state();
        if state.phase != GamePhase::PreFlop {
            return vacated;
        }
        let seat = state.action_on;
        let Some(player) = state.player_at(seat) else {
            return vacated;
        };
        let who = player.principal;

        if seat == short_seat {
            let _ = act_seat(world, recorder, seat, PlayerAction::AllIn);
            acted.push(seat);
            continue;
        }

        if !acted.contains(&seat) {
            let action = if acted.iter().all(|s| *s == short_seat) {
                PlayerAction::Raise(first)
            } else if acted.iter().filter(|s| **s != short_seat).count() + 1 == deep_total {
                PlayerAction::Raise(second)
            } else {
                PlayerAction::Call
            };
            if act_seat(world, recorder, seat, action).is_err() {
                let _ = act_seat(world, recorder, seat, PlayerAction::Call);
            }
            acted.push(seat);
            continue;
        }

        // Second time around. The first committed seat gives up and leaves.
        if vacated.is_none() && player.total_bet_this_hand >= first {
            let _ = act_seat(world, recorder, seat, PlayerAction::Fold);
            // Either door: `leave_table` has no phase guard at all, and `cash_out`
            // lets a folded player out. docs/DEFECTS.md E-05.
            if world.leave_table(who).is_err() {
                let _ = world.cash_out(who);
            }
            recorder.observe(world);
            vacated = Some(seat);
            continue;
        }

        let _ = act_seat(world, recorder, seat, PlayerAction::Call);
    }
    vacated
}

/// Play the hand to completion under `policy`.
pub fn play_out(
    world: &mut World,
    recorder: &mut HandRecorder,
    policy: Policy,
    budget: usize,
) -> (bool, usize) {
    play_until(world, recorder, policy, budget, None)
}

/// Play under `policy` until the table reaches `phase` (or the hand ends).
pub fn play_out_until_phase(
    world: &mut World,
    recorder: &mut HandRecorder,
    phase: GamePhase,
    policy: Policy,
    budget: usize,
) -> (bool, usize) {
    play_until(world, recorder, policy, budget, Some(phase))
}

fn play_until(
    world: &mut World,
    recorder: &mut HandRecorder,
    mut policy: Policy,
    budget: usize,
    stop_at: Option<GamePhase>,
) -> (bool, usize) {
    let mut actions = 0usize;
    for _ in 0..budget {
        let state = world.table_state();
        if !state.phase.hand_in_progress() {
            return (true, actions);
        }
        if Some(state.phase) == stop_at {
            return (false, actions);
        }
        let Some(who) = state.player_at(state.action_on).map(|p| p.principal) else {
            // Nobody can act. Nudge the engine the way the real deployment does.
            world.advance(Duration::from_secs(1));
            let _ = world.check_timeouts(world.actors[0].principal);
            recorder.observe(world);
            continue;
        };
        world.advance(ACTION_SPACING);

        let wanted = policy(&state);
        let mut sent = false;
        if let Some(action) = wanted {
            if world.player_action(who, action).is_ok() {
                sent = true;
            }
        }
        if !sent {
            for fallback in [PlayerAction::Check, PlayerAction::Call, PlayerAction::Fold] {
                if world.player_action(who, fallback).is_ok() {
                    sent = true;
                    break;
                }
            }
        }
        recorder.observe(world);
        if !sent {
            // Every legal action was refused. Let the clock do it.
            world.advance(Duration::from_secs(2));
            let _ = world.check_timeouts(world.actors[0].principal);
            recorder.observe(world);
        } else {
            actions += 1;
        }
    }
    let done = !world.table_state().phase.hand_in_progress();
    (done, actions)
}
