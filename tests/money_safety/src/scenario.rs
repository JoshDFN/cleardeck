//! Canister-level hand scenarios, and the MEASURED facts about each hand.
//!
//! Everything here drives the real `table_canister` state machine on PocketIC
//! through its public update methods. Nothing re-implements a rule.
//!
//! This module exists because of docs/DEFECTS.md H-04: seven of seven mutations to
//! the seam in `src/table_canister/src/lib.rs` survived with 101 tests green. You
//! could make `calculate_side_pots` a no-op, pass `state.pot / 2` as the pot, write
//! the pots into a throwaway `Vec`, pass an empty contributions slice, stub
//! `evaluate_hand` to return `RoyalFlush` for everybody, or shuffle every hand from
//! the constant seed `b"CONSTANT"`, and nothing went red. The reason was simple: no
//! test ever ran a hand through the canister and looked at the numbers.
//!
//! So the unit of measurement here is a whole hand, and the facts recorded are the
//! ones a mutation has to change: what went in, what the engine used as its payout
//! basis, what came out, which physical cards were dealt, and which hand the engine
//! believed each player held.

use std::time::Duration;

use crate::table_api::*;
use crate::world::World;

/// How much of the money in a hand went in AFTER the flop was dealt. This is the
/// quantity E-01 destroys, so tests state it explicitly rather than inferring it.
#[derive(Clone, Debug)]
pub struct HandOutcome {
    pub hand_number: u64,
    /// Phase the table ended in.
    pub final_phase: GamePhase,
    /// `pot` at the instant the flop was dealt, i.e. all pre-flop money.
    pub preflop_pot: u64,
    /// Money the harness itself put in after the flop, counted from the state
    /// deltas rather than from what it asked for.
    pub post_flop_in: u64,
    /// Sum of the seated players' `total_bet_this_hand` at `HandComplete`: every e8
    /// the hand collected from a player who is still seated.
    pub collected: u64,
    /// Sum of the winner amounts the canister RECORDED for this hand.
    pub awarded: u64,
    /// The basis the engine paid out of: `sum(side_pots)` if it had a breakdown at
    /// the last live observation, otherwise `pot`. `determine_winners` uses the
    /// breakdown when it is non-empty and `state.pot` when it is not, so this is
    /// the number the engine itself chose.
    pub payout_basis: u64,
    /// Whether the engine had a side-pot breakdown when the hand ended.
    pub used_side_pot_breakdown: bool,
    /// `side_pots` as last seen while the hand was live.
    pub basis_side_pots: Vec<SidePot>,
    /// `pot` as last seen while the hand was live.
    pub last_live_pot: u64,
    /// Total chips at the table before `start_new_hand` and after `HandComplete`.
    pub chips_before: u64,
    pub chips_after: u64,
    /// The shuffled deck for this hand, straight out of `get_table_state`.
    pub deck: Vec<Card>,
    pub community: Vec<Card>,
    /// Everyone who reached the showdown, as the canister recorded them.
    pub showdown: Vec<ShowdownRecord>,
    /// Canister log lines produced during the hand.
    pub logs: Vec<String>,
}

impl HandOutcome {
    /// Money collected from players that nobody was paid. Positive means destroyed.
    pub fn unpaid(&self) -> i128 {
        self.collected as i128 - self.awarded as i128
    }

    pub fn reached_showdown(&self) -> bool {
        self.showdown.len() >= 2
    }
}

/// Fund `names` and seat them in order, then let the auto-deal delay pass.
pub fn seat_players(world: &World, names: &[&str], escrow_each: u64) {
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world
            .fund_escrow(who, escrow_each)
            .unwrap_or_else(|e| panic!("fund_escrow for {name} failed: {e:?}"));
        world
            .join_table(who, i as u8)
            .unwrap_or_else(|e| panic!("join_table for {name} failed: {e:?}"));
    }
    world.advance(Duration::from_secs(4));
}

pub fn on_clock(t: &TableState) -> Option<candid::Principal> {
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
}

/// Drive the hand with `Check`, then `Call`, then `Fold`, whichever the engine
/// accepts. Deliberately passive: produces showdowns with NO post-flop betting.
pub fn play_out_passively(world: &World, budget: usize) {
    for _ in 0..budget {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = on_clock(&t) else { return };
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
}

/// Bet `amount` whenever the player on the clock can, otherwise call/check.
/// This is what makes post-flop money exist.
pub fn play_out_with_betting(world: &World, amount: u64, budget: usize) {
    for _ in 0..budget {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = on_clock(&t) else { return };
        let post_flop = matches!(
            t.phase,
            GamePhase::Flop | GamePhase::Turn | GamePhase::River
        );
        if post_flop && world.player_action(who, PlayerAction::Bet(amount)).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
}

/// Level the pre-flop bets and stop on the flop. Returns the state on the flop.
fn reach_the_flop(world: &World, budget: usize) -> TableState {
    for _ in 0..budget {
        let t = world.table_state();
        if t.phase != GamePhase::PreFlop {
            return t;
        }
        let Some(who) = on_clock(&t) else { return t };
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        break;
    }
    world.table_state()
}

/// ONE full hand, with real pre-flop money AND one post-flop bet that gets called,
/// run all the way to a settled hand, measured throughout.
///
/// `post_flop_bet` of 0 plays the hand out passively instead, which is how a caller
/// gets the no-post-flop-money control case.
///
/// The tracking is done by polling `get_table_state` after every action, because the
/// numbers this returns must be the ones the ENGINE held, not the ones the caller
/// intended. `payout_basis` in particular has to be captured before the hand ends:
/// `determine_winners` clears `side_pots` on the way out.
pub fn play_one_hand(world: &mut World, post_flop_bet: u64) -> HandOutcome {
    let before = world.snapshot();
    let chips_before = before.chips_total;
    // Drop any log lines from set-up so `logs` is this hand's testimony only.
    let _ = world.new_canister_logs();

    world
        .start_new_hand(world.actors[0].principal)
        .unwrap_or_else(|e| panic!("start_new_hand failed: {e:?}"));
    let dealt = world.table_state();
    let hand_number = dealt.hand_number;
    let deck = dealt.deck.clone();

    let mut last_live = dealt.clone();
    let mut preflop_pot = 0u64;
    let mut post_flop_in = 0u64;

    let flop = reach_the_flop(world, 24);
    if flop.phase.hand_in_progress() {
        last_live = flop.clone();
    }
    if flop.phase == GamePhase::Flop {
        preflop_pot = flop.pot;
        if post_flop_bet > 0 {
            let pot_at_flop = flop.pot;
            play_out_with_betting(world, post_flop_bet, 40);
            // Whatever actually went in post-flop, measured, not assumed.
            let t = world.table_state();
            let collected_now = t.wagered_total();
            post_flop_in = collected_now.saturating_sub(pot_at_flop);
        }
    }

    // Run the rest out, recording the last state in which the hand was still live.
    for _ in 0..80 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        last_live = t.clone();
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            let _ = world.check_timeouts(world.actors[0].principal);
            continue;
        };
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(31));
        let _ = world.check_timeouts(world.actors[0].principal);
    }

    let after = world.snapshot();
    let logs = world.new_canister_logs();
    let history = world.hand_history(hand_number);
    let awarded = history.as_ref().map(|h| h.awarded_total()).unwrap_or(0);
    let showdown = history
        .as_ref()
        .map(|h| h.showdown_players.clone())
        .unwrap_or_default();

    let used_side_pot_breakdown = !last_live.side_pots.is_empty();
    let payout_basis = if used_side_pot_breakdown {
        last_live.side_pots_total()
    } else {
        last_live.pot
    };

    HandOutcome {
        hand_number,
        final_phase: after.table.phase.clone(),
        preflop_pot,
        post_flop_in,
        collected: after.table.wagered_total(),
        awarded,
        payout_basis,
        used_side_pot_breakdown,
        basis_side_pots: last_live.side_pots.clone(),
        last_live_pot: last_live.pot,
        chips_before,
        chips_after: after.chips_total,
        deck,
        community: after.table.community_cards.clone(),
        showdown,
        logs,
    }
}

/// Re-evaluate one showdown player's hand through the engine's own evaluator.
///
/// This is the oracle for "the canister really consulted `evaluate_hand`": the cards
/// come from the canister's hand history, the rank comes from a fresh call to
/// `poker_core::evaluate_hand`, and the two must agree.
pub fn recompute_rank(
    record: &ShowdownRecord,
    community: &[Card],
) -> Option<poker_core::HandRank> {
    let hole = record.cards?;
    let board: Vec<poker_core::Card> = community.iter().map(|c| c.to_engine()).collect();
    Some(poker_core::evaluate_hand(&hole, &board))
}
