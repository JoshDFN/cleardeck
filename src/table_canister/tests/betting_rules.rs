//! Betting-rules tests: does the engine agree with the rules of poker?
//!
//! These are NOT conservation tests. `tests/money_safety` already asks "did any
//! chips appear or vanish". These ask the different question a poker player asks:
//! **given who has what and who has acted, are the legal actions the legal
//! actions?** A rules deviation moves no money in aggregate -- it redistributes
//! it -- so every invariant in the money-safety harness is blind to it
//! (docs/DEFECTS.md H-12). That is why these exist.
//!
//! WHY THEY DRIVE THE REAL FUNCTIONS. docs/DEFECTS.md H-04: seven of seven
//! mutations to `src/table_canister/src/lib.rs` survived with the whole suite
//! green, because nothing tested the seam. So these call
//! `table_canister::apply_player_action`, `::is_betting_round_complete` and
//! `::resolve_expired_action_timer` -- the actual functions `player_action` and
//! `check_timeouts` run. `player_action` is `check_rate_limit()` +
//! `msg_caller()` + `time()` + the `TABLE` borrow + `apply_player_action`.
//! There is no re-implementation of the state machine anywhere in this file.
//!
//! Each test is named for the poker SITUATION, and its body states who holds
//! what, who has acted, and which actions are legal.
//!
//! Scope note: these stay inside one betting round on purpose. Showdown and the
//! single-winner payout call `ic_cdk::api::time()` and the history canister, so
//! they are not reachable from a host test; the pot-award path is covered by
//! `tests/money_safety` on PocketIC.

use candid::Principal;
use table_canister::{
    apply_player_action, is_betting_round_complete, resolve_expired_action_timer, ActionTimer,
    Currency, GamePhase, Player, PlayerAction, PlayerStatus, TableConfig, TableState,
};

// =============================================================================
// FIXTURE
// =============================================================================

/// Blinds 10/20, six seats, 30 s clock. `table_1`'s shape with round numbers so
/// the arithmetic in each test is readable.
const SMALL_BLIND: u64 = 10;
const BIG_BLIND: u64 = 20;
const TIMEOUT_SECS: u64 = 30;
const SEC: u64 = 1_000_000_000;

/// A fixed, distinguishable principal per seat.
fn seat_principal(seat: u8) -> Principal {
    Principal::from_slice(&[0xC0, 0xFF, 0xEE, seat])
}

fn config() -> TableConfig {
    TableConfig {
        small_blind: SMALL_BLIND,
        big_blind: BIG_BLIND,
        min_buy_in: 40,
        max_buy_in: 100_000,
        max_players: 6,
        action_timeout_secs: TIMEOUT_SECS,
        ante: 0,
        time_bank_secs: 30,
        currency: Currency::ICP,
    }
}

fn player(seat: u8, chips: u64) -> Player {
    Player {
        principal: seat_principal(seat),
        seat,
        chips,
        hole_cards: None,
        current_bet: 0,
        total_bet_this_hand: 0,
        has_folded: false,
        has_acted_this_round: false,
        is_all_in: false,
        status: PlayerStatus::Active,
        last_seen: 0,
        timeout_count: 0,
        time_bank_remaining: 30,
        is_sitting_out_next_hand: false,
        broke_at: None,
        sitting_out_since: None,
    }
}

/// A table mid-hand on the flop: no blinds in front of anyone, `current_bet` 0,
/// `min_raise` one big blind, everybody yet to act. The cleanest board on which
/// to state a betting-rules situation, because nothing is inherited from
/// pre-flop posting.
///
/// `stacks[i]` is seat `i`'s stack. Action starts on seat 0.
fn flop_table(stacks: &[u64], now: u64) -> TableState {
    let mut players: Vec<Option<Player>> = Vec::new();
    for (i, chips) in stacks.iter().enumerate() {
        players.push(Some(player(i as u8, *chips)));
    }
    while players.len() < 6 {
        players.push(None);
    }

    TableState {
        id: 0,
        config: config(),
        players,
        community_cards: Vec::new(),
        deck: poker_core::create_deck(),
        // The flop is already out: 3 dealt + 1 burn, and 2 hole cards each.
        deck_index: 2 * stacks.len() + 4,
        pot: 0,
        side_pots: Vec::new(),
        current_bet: 0,
        min_raise: BIG_BLIND,
        phase: GamePhase::Flop,
        dealer_seat: (stacks.len() - 1) as u8,
        small_blind_seat: 0,
        big_blind_seat: 1,
        action_on: 0,
        action_timer: Some(ActionTimer {
            player_seat: 0,
            started_at: now,
            expires_at: now + TIMEOUT_SECS * SEC,
            using_time_bank: false,
        }),
        shuffle_proof: None,
        hand_number: 1,
        last_aggressor: None,
        bb_has_option: false,
        first_hand: false,
        auto_deal_at: None,
        last_action: None,
        departed_stakes: Vec::new(),
    }
}

/// A pre-flop table with the blinds already posted, exactly as `start_new_hand`
/// leaves it: `current_bet` and `min_raise` at one big blind, SB and BB money in
/// front of them, `bb_has_option` live, action left of the big blind.
fn preflop_table(stacks: &[u64], now: u64) -> TableState {
    let mut state = flop_table(stacks, now);
    state.phase = GamePhase::PreFlop;
    state.deck_index = 2 * stacks.len();
    state.dealer_seat = (stacks.len() - 1) as u8;
    state.small_blind_seat = 0;
    state.big_blind_seat = 1;
    state.current_bet = BIG_BLIND;
    state.min_raise = BIG_BLIND;
    state.bb_has_option = true;

    for (seat, blind) in [(0usize, SMALL_BLIND), (1usize, BIG_BLIND)] {
        let p = state.players[seat].as_mut().expect("blind seat is occupied");
        let posted = blind.min(p.chips);
        p.chips -= posted;
        p.current_bet = posted;
        p.total_bet_this_hand = posted;
        state.pot += posted;
        if p.chips == 0 {
            p.is_all_in = true;
            if seat == 1 {
                state.bb_has_option = false;
            }
        }
    }

    // Action starts left of the big blind.
    let first = if stacks.len() > 2 { 2 } else { 0 };
    state.action_on = first as u8;
    state.action_timer = Some(ActionTimer {
        player_seat: first as u8,
        started_at: now,
        expires_at: now + TIMEOUT_SECS * SEC,
        using_time_bank: false,
    });
    state
}

/// Drive the real engine as seat `seat`.
fn act(state: &mut TableState, seat: u8, now: u64, action: PlayerAction) -> Result<(), String> {
    apply_player_action(state, seat_principal(seat), now, action)
}

fn seat(state: &TableState, s: u8) -> &Player {
    state.players[s as usize].as_ref().expect("seat occupied")
}

// =============================================================================
// 1. AN INCOMPLETE ALL-IN RAISE MUST NOT REOPEN THE BETTING
//
// The rule, and the source. Robert's Rules of Poker (Ciaffone), Section 3
// "Betting and Raising": if a player goes all-in for less than the amount needed
// for a full raise, the betting is NOT reopened for players who have already
// acted. TDA 2022 Rule 41 (Raises) and Illustration Addendum 3 say the same: a
// player who has already acted and is not facing at least a full raise may only
// call or fold.
//
// The engine used to reset `has_acted_this_round` for every other player on ANY
// all-in above the current bet, so a player who had already acted got a fresh
// raise out of a raise that was never legal to raise into.
// =============================================================================

/// FLOP, three-handed. Seat 0 has 5,000 and bets 100. Seat 1 has EXACTLY 150 and
/// shoves: that is a raise of 50 into a min-raise of 100, so it is an INCOMPLETE
/// raise. Seat 2 has 5,000 and has not acted yet.
///
/// Legal actions after the shove:
///   seat 2 (has not acted)  -> fold, call 150, or raise to >= 250
///   seat 0 (already acted)  -> fold or call the extra 50. NOT raise.
#[test]
fn an_incomplete_all_in_raise_does_not_reopen_the_betting_to_a_player_who_already_acted() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 5_000], now);

    act(&mut state, 0, now, PlayerAction::Bet(100)).expect("seat 0 may open for 100");
    assert_eq!(state.current_bet, 100);
    assert_eq!(state.min_raise, 100, "the opening bet sets the min raise");
    assert!(seat(&state, 0).has_acted_this_round);
    assert_eq!(state.action_on, 1, "action moves to seat 1");

    act(&mut state, 1, now, PlayerAction::AllIn).expect("seat 1 may always shove");

    // The shove: 150 total, 50 more than the 100 bet, against a 100 min raise.
    assert_eq!(state.current_bet, 150, "the amount owed does go up to 150");
    assert_eq!(
        state.min_raise, 100,
        "an incomplete all-in must not become the new min-raise increment"
    );
    assert!(seat(&state, 1).is_all_in);

    // THE RULE. Seat 0 had acted; an incomplete all-in raise does not give them
    // a new raise.
    assert!(
        seat(&state, 0).has_acted_this_round,
        "seat 0 already acted and the raise was incomplete: the betting must NOT \
         be reopened to them"
    );
    assert!(
        !seat(&state, 2).has_acted_this_round,
        "seat 2 has not acted yet, so their option is untouched"
    );
    assert!(
        !is_betting_round_complete(&state),
        "seat 2 still owes an action and seat 0 still owes 50"
    );

    // Seat 2 has not acted, so seat 2 keeps a FULL option and calls.
    assert_eq!(state.action_on, 2);
    act(&mut state, 2, now, PlayerAction::Call).expect("seat 2 may call 150");
    assert_eq!(seat(&state, 2).current_bet, 150);

    // Action comes back to seat 0, who still owes 50.
    assert_eq!(state.action_on, 0, "seat 1 is all-in, so the action skips them");
    assert!(!is_betting_round_complete(&state));

    // THE MONEY ASSERTION. Seat 0 may not raise.
    let rejected = act(&mut state, 0, now, PlayerAction::Raise(300))
        .expect_err("seat 0 must not be allowed to raise into an incomplete all-in");
    assert!(
        rejected.contains("not reopened"),
        "the rejection must explain the rule, got: {rejected}"
    );
    assert_eq!(state.current_bet, 150, "the rejected raise changed nothing");
    assert_eq!(seat(&state, 0).chips, 5_000 - 100, "and cost seat 0 nothing");

    // Call and fold are still legal, and calling closes the round -- which resets
    // `current_bet` for the new street, so the wager is read off
    // `total_bet_this_hand`.
    act(&mut state, 0, now, PlayerAction::Call).expect("seat 0 may call the extra 50");
    assert_eq!(seat(&state, 0).total_bet_this_hand, 150);
    assert_eq!(state.phase, GamePhase::Turn, "matching all round closes the flop");
    assert_eq!(state.pot, 150 * 3);
}

/// The same shape, except seat 1 has EXACTLY 200 -- a raise of 100 into a
/// min-raise of 100, which is a FULL raise. The betting IS reopened.
///
/// This is the guard against over-correcting the test above into "an all-in never
/// reopens the betting".
#[test]
fn a_full_all_in_raise_does_reopen_the_betting() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 200, 5_000], now);

    act(&mut state, 0, now, PlayerAction::Bet(100)).expect("seat 0 opens 100");
    act(&mut state, 1, now, PlayerAction::AllIn).expect("seat 1 shoves 200");

    assert_eq!(state.current_bet, 200);
    assert_eq!(
        state.min_raise, 100,
        "a full all-in raise of exactly 100 sets the increment to 100"
    );
    assert!(
        !seat(&state, 0).has_acted_this_round,
        "a FULL raise reopens the betting to seat 0"
    );

    act(&mut state, 2, now, PlayerAction::Call).expect("seat 2 calls 200");
    assert_eq!(state.action_on, 0);

    // And seat 0's raise is legal: 200 + 100.
    act(&mut state, 0, now, PlayerAction::Raise(300))
        .expect("a full all-in raise reopens seat 0's option to re-raise");
    assert_eq!(state.current_bet, 300);
}

/// A player closed to raising may still CALL, and may still FOLD. The fix must
/// not turn "cannot raise" into "cannot act".
#[test]
fn a_player_closed_by_an_incomplete_all_in_may_still_call_or_fold() {
    let now = 1_000 * SEC;

    // Calling.
    let mut state = flop_table(&[5_000, 150, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::Call).unwrap();
    act(&mut state, 0, now, PlayerAction::Call).expect("call must stay legal");
    assert_eq!(seat(&state, 0).total_bet_this_hand, 150);
    assert_eq!(seat(&state, 0).chips, 5_000 - 150);

    // Folding.
    let mut state = flop_table(&[5_000, 150, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::Call).unwrap();
    act(&mut state, 0, now, PlayerAction::Fold).expect("fold must stay legal");
    assert!(seat(&state, 0).has_folded);
    assert_eq!(
        seat(&state, 0).total_bet_this_hand,
        100,
        "folding leaves the 100 already bet in the pot and adds nothing"
    );
}

/// A closed player must not be able to launder a raise through `AllIn` either.
/// Seat 0 has 5,000 behind after betting 100; shoving would be a raise to 5,100,
/// which the rule does not allow. Refuse it rather than silently shrink the wager
/// to a call: this canister holds funds and quietly resizing somebody's bet is
/// not a safe default.
#[test]
fn a_closed_player_cannot_launder_a_raise_through_all_in() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::Call).unwrap();

    let rejected = act(&mut state, 0, now, PlayerAction::AllIn)
        .expect_err("a closed player shoving 5,000 behind is an illegal raise");
    assert!(rejected.contains("not reopened"), "got: {rejected}");
    assert_eq!(seat(&state, 0).chips, 4_900, "the refused shove cost nothing");
    assert!(!seat(&state, 0).is_all_in);
    assert_eq!(state.current_bet, 150);
}

/// The other side of that: a closed player whose whole stack is LESS than the
/// call is putting in a call for less, which is always legal.
///
/// Seat 0 starts with 130, bets 100, and has 30 behind. The shove to 130 does not
/// exceed the 150 owed, so it is a call for less, not a raise.
#[test]
fn a_closed_player_may_still_shove_a_short_stack_as_a_call_for_less() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[130, 150, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::Call).unwrap();
    assert_eq!(state.current_bet, 150, "seat 0 owes 150 and has 30 behind");

    act(&mut state, 0, now, PlayerAction::AllIn)
        .expect("shoving less than the amount owed is a call for less, always legal");
    assert!(seat(&state, 0).is_all_in);
    assert_eq!(
        seat(&state, 0).total_bet_this_hand,
        130,
        "130 went in: the 100 bet plus the 30 behind"
    );
    assert_eq!(
        seat(&state, 2).total_bet_this_hand,
        150,
        "a call for less must not have made anyone else pay more"
    );

    // The call for less closed the betting with only seat 2 able to act, so the
    // board ran out and the hand settled inside this same call. `flop_table` deals
    // no hole cards, so it settled as a showdown at which no seat could be ranked,
    // and the payout path now hands every seat back exactly what it put in.
    //
    // It used to return early there, leaving all 430 sitting in `state.pot` of a
    // HandComplete hand, which the next `start_new_hand` zeroed: destroyed. The
    // numbers below therefore changed with the E-01/E-03/E-05 payout fix, and the
    // rule this test exists for -- the call for less itself -- is asserted above.
    assert_eq!(state.pot, 0, "the hand settled, so the pot is empty");
    assert_eq!(
        seat(&state, 0).chips,
        130,
        "a settlement nobody can win refunds each seat its own stake, exactly"
    );
    assert_eq!(seat(&state, 1).chips, 150);
    assert_eq!(seat(&state, 2).chips, 5_000);
}

/// A player who has NOT yet acted keeps a full option over an incomplete all-in,
/// and the minimum raise is measured off the last FULL raise, not off the
/// incomplete shove. TDA 2022 Rule 41: the raise must be at least the size of the
/// largest previous FULL bet or raise of the round.
///
/// Seat 0 bets 100 (full raise increment 100). Seat 1 shoves 150 (incomplete).
/// The amount owed is 150; the increment in force is still 100; so seat 2's
/// minimum legal raise is to 250, not to 200 and not to 300.
#[test]
fn the_minimum_raise_over_an_incomplete_all_in_is_measured_off_the_last_full_raise() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    assert_eq!(state.current_bet, 150);
    assert_eq!(state.min_raise, 100);

    for too_small in [151u64, 200, 249] {
        let mut probe = state.clone();
        let rejected = act(&mut probe, 2, now, PlayerAction::Raise(too_small))
            .expect_err(&format!("a raise to {too_small} is under the minimum"));
        assert!(
            rejected.contains("Minimum raise"),
            "raise to {too_small}: {rejected}"
        );
        assert_eq!(probe.current_bet, 150, "the refused raise changed nothing");
    }

    let mut ok = state.clone();
    act(&mut ok, 2, now, PlayerAction::Raise(250))
        .expect("150 owed + the 100 full-raise increment = 250 is the minimum legal raise");
    assert_eq!(ok.current_bet, 250);
    assert_eq!(ok.min_raise, 100, "a raise of exactly the increment keeps it at 100");
}

/// Two incomplete all-ins in a row must not accumulate into a reopened round.
///
/// Seat 0 bets 100. Seat 1 shoves 150 (incomplete). Seat 2 shoves 190
/// (incomplete against the 100 increment: 190 - 150 = 40). Seat 0 has acted and
/// still may not raise.
#[test]
fn two_successive_incomplete_all_ins_still_do_not_reopen_the_betting() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 190], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::AllIn).unwrap();

    assert_eq!(state.current_bet, 190);
    assert_eq!(state.min_raise, 100, "still the last FULL raise increment");
    assert!(seat(&state, 0).has_acted_this_round);
    assert_eq!(state.action_on, 0, "seats 1 and 2 are all-in");

    let rejected = act(&mut state, 0, now, PlayerAction::Raise(400))
        .expect_err("neither shove was a full raise, so seat 0 is still closed");
    assert!(rejected.contains("not reopened"), "got: {rejected}");
}

/// PRE-FLOP, blinds 10/20. Seat 2 (UTG) shoves 25: a raise of 5 into a 20
/// min-raise, so incomplete. The big blind has posted but has NOT acted, so the
/// big blind keeps a full option -- and specifically may still raise.
///
/// This is the short-all-in-versus-the-blinds case. Posting a blind is not an
/// action, so `has_acted_this_round` is false for the blinds and the rule above
/// does not close them.
#[test]
fn the_big_blind_still_has_a_live_option_over_an_incomplete_all_in_preflop() {
    let now = 1_000 * SEC;
    let mut state = preflop_table(&[5_000, 5_000, 25], now);
    assert_eq!(state.current_bet, BIG_BLIND);
    assert_eq!(state.action_on, 2, "UTG acts first pre-flop three-handed");

    act(&mut state, 2, now, PlayerAction::AllIn).expect("UTG shoves 25");
    assert_eq!(state.current_bet, 25);
    assert_eq!(state.min_raise, 20, "5 is not a full raise");
    assert!(
        !state.bb_has_option,
        "the big blind now faces a bet, so the free check is gone"
    );

    // Small blind, who has not acted, calls to 25.
    assert_eq!(state.action_on, 0);
    act(&mut state, 0, now, PlayerAction::Call).expect("SB calls to 25");

    // The big blind never acted, so the big blind may raise: 25 + 20 = 45.
    assert_eq!(state.action_on, 1);
    assert!(!seat(&state, 1).has_acted_this_round);
    act(&mut state, 1, now, PlayerAction::Raise(45))
        .expect("the big blind posted but never acted, so the option is live");
    assert_eq!(state.current_bet, 45);
}

// =============================================================================
// 2. AN EXPIRED ACTION TIMER MUST RESOLVE THE HAND, NOT JUST REFUSE THE ACTION
//
// `player_action` used to reject an action whose timer had passed and change
// nothing else, and only `check_timeouts` ever applied the timeout. Nothing in
// the canister calls `check_timeouts` by itself, so once the clock passed, the
// hand could not progress and the seat could not act: the table wedged.
// =============================================================================

/// FLOP, three-handed. Seat 0 is on the clock and lets it run out, then sends a
/// Check anyway. The action must be refused AND the hand must have moved on.
#[test]
fn an_expired_action_timer_folds_the_seat_and_moves_the_action_on() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    let late = start + (TIMEOUT_SECS + 1) * SEC;

    let rejected = act(&mut state, 0, late, PlayerAction::Check)
        .expect_err("an action that arrives after the clock must not be accepted");
    assert!(
        rejected.contains("expired"),
        "the rejection must say the clock ran out, got: {rejected}"
    );

    // THE WEDGE ASSERTION. The hand has to be somewhere different from where it
    // started, or nothing can ever move it.
    assert!(
        seat(&state, 0).has_folded,
        "the forfeited action must be resolved, not merely refused"
    );
    assert_eq!(seat(&state, 0).timeout_count, 1);
    assert_ne!(state.action_on, 0, "the action must have moved off seat 0");
    assert_eq!(state.action_on, 1);

    let timer = state.action_timer.as_ref().expect("a fresh clock for seat 1");
    assert_eq!(timer.player_seat, 1);
    assert!(
        timer.expires_at > late,
        "seat 1 must get a full clock starting now, not inherit seat 0's expiry"
    );

    // And the table is live: seat 1 can act.
    act(&mut state, 1, late, PlayerAction::Check).expect("seat 1 can now act");
}

/// The same expiry resolved by ANY player's message, not only the timed-out
/// player's. Seat 2 acts out of turn while seat 0's clock has run out: seat 0 is
/// folded, the action reaches seat 1, and seat 2 is told it is not their turn --
/// but the table is no longer stuck.
#[test]
fn any_players_message_unwedges_a_table_whose_clock_has_run_out() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    let late = start + (TIMEOUT_SECS + 1) * SEC;

    let rejected = act(&mut state, 2, late, PlayerAction::Check)
        .expect_err("seat 2 is not on the clock");
    assert_eq!(rejected, "Not your turn");

    assert!(seat(&state, 0).has_folded, "seat 0's expiry was still resolved");
    assert_eq!(state.action_on, 1);
    assert!(!seat(&state, 2).has_folded, "seat 2 was not punished for asking");
}

/// If the expiry resolution hands the action to the caller, their message is
/// accepted in the same call. Seat 1 sends a Bet while seat 0's clock has passed:
/// seat 0 folds, the action lands on seat 1, and seat 1's bet stands.
#[test]
fn a_message_that_becomes_in_turn_after_the_expiry_is_resolved_is_accepted() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    let late = start + (TIMEOUT_SECS + 1) * SEC;

    act(&mut state, 1, late, PlayerAction::Bet(200))
        .expect("seat 1 is next after seat 0's expiry, so their bet is in turn");

    assert!(seat(&state, 0).has_folded);
    assert_eq!(state.current_bet, 200);
    assert_eq!(seat(&state, 1).current_bet, 200);
    assert_eq!(state.action_on, 2);
}

/// The clock is not resolved a nanosecond early. At exactly `expires_at` the
/// action is still good: `now > expires_at` is the boundary the engine uses and
/// the tests must pin it, or a fix could quietly start eating live actions.
#[test]
fn an_action_arriving_exactly_on_the_expiry_instant_is_still_accepted() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    let exactly = start + TIMEOUT_SECS * SEC;

    act(&mut state, 0, exactly, PlayerAction::Check).expect("on the instant, still in time");
    assert!(!seat(&state, 0).has_folded);
    assert_eq!(state.action_on, 1);
}

/// `check_timeouts` and `player_action` must resolve an expiry IDENTICALLY --
/// they are one function now, and this pins that they stay one. Two copies of the
/// same table, one resolved through the shared resolver (what `check_timeouts`
/// calls) and one through a late `player_action`, must land in the same state.
#[test]
fn the_timer_path_resolves_the_same_way_whichever_message_triggers_it() {
    let start = 1_000 * SEC;
    let late = start + (TIMEOUT_SECS + 1) * SEC;

    let mut via_check_timeouts = flop_table(&[5_000, 5_000, 5_000], start);
    let seat_out = resolve_expired_action_timer(&mut via_check_timeouts, late);
    assert_eq!(seat_out, Some(0));

    let mut via_player_action = flop_table(&[5_000, 5_000, 5_000], start);
    let _ = act(&mut via_player_action, 0, late, PlayerAction::Check);

    for s in 0..3u8 {
        let a = seat(&via_check_timeouts, s);
        let b = seat(&via_player_action, s);
        assert_eq!(a.has_folded, b.has_folded, "seat {s} has_folded");
        assert_eq!(a.chips, b.chips, "seat {s} chips");
        assert_eq!(a.current_bet, b.current_bet, "seat {s} current_bet");
        assert_eq!(a.timeout_count, b.timeout_count, "seat {s} timeout_count");
        assert_eq!(a.status, b.status, "seat {s} status");
    }
    assert_eq!(via_check_timeouts.action_on, via_player_action.action_on);
    assert_eq!(via_check_timeouts.pot, via_player_action.pot);
    assert_eq!(via_check_timeouts.current_bet, via_player_action.current_bet);
}

/// A timer with nothing expired must not touch the table.
#[test]
fn a_live_clock_is_left_alone() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    assert_eq!(resolve_expired_action_timer(&mut state, start + SEC), None);
    assert!(!seat(&state, 0).has_folded);
    assert_eq!(state.action_on, 0);
}

/// Two expiries in a row resolve one seat each, and the second resolution starts
/// from the clock the first one installed. Repeatedly timing out must walk the
/// table, not fold everybody at once.
///
/// Four-handed so that folding two seats still leaves a live hand: the
/// single-winner payout is not reachable from a host test (it reads
/// `ic_cdk::api::time()` and the history canister) and is covered on PocketIC by
/// `tests/money_safety`.
#[test]
fn each_expiry_resolves_exactly_one_seat() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000, 5_000], start);

    let first = start + (TIMEOUT_SECS + 1) * SEC;
    assert_eq!(resolve_expired_action_timer(&mut state, first), Some(0));
    assert!(seat(&state, 0).has_folded);
    assert!(!seat(&state, 1).has_folded, "only the seat on the clock folds");
    assert!(!seat(&state, 2).has_folded);

    // The new clock is seat 1's, and it is not yet expired.
    assert_eq!(resolve_expired_action_timer(&mut state, first), None);

    let second = first + (TIMEOUT_SECS + 1) * SEC;
    assert_eq!(resolve_expired_action_timer(&mut state, second), Some(1));
    assert!(seat(&state, 1).has_folded);
}

/// A player who times out twice is sat out (`MAX_TIMEOUTS_BEFORE_SITOUT` is 2),
/// through the shared path as much as through `check_timeouts`.
#[test]
fn a_second_timeout_sits_the_player_out() {
    let start = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], start);
    state.players[0].as_mut().unwrap().timeout_count = 1;

    let late = start + (TIMEOUT_SECS + 1) * SEC;
    let rejected = act(&mut state, 0, late, PlayerAction::Check).expect_err("too late");
    assert!(rejected.contains("expired"), "got: {rejected}");
    assert_eq!(seat(&state, 0).timeout_count, 2);
    assert_eq!(seat(&state, 0).status, PlayerStatus::SittingOut);
}

// =============================================================================
// 3. THE UNCHANGED RULES STILL HOLD
//
// A guard band. The two fixes above touched `player_action`'s AllIn and Raise
// arms and its entry sequence, so the ordinary actions need pinning too or a
// regression in them would go unnoticed.
// =============================================================================

#[test]
fn an_ordinary_flop_round_of_checks_closes_the_betting() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Check).unwrap();
    assert!(!is_betting_round_complete(&state));
    act(&mut state, 1, now, PlayerAction::Check).unwrap();
    assert!(!is_betting_round_complete(&state));
    act(&mut state, 2, now, PlayerAction::Check).unwrap();
    assert_eq!(state.phase, GamePhase::Turn, "three checks close the flop");
}

#[test]
fn a_bet_reopens_the_action_to_everyone_who_had_already_checked() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Check).unwrap();
    act(&mut state, 1, now, PlayerAction::Bet(100)).unwrap();
    assert!(
        !seat(&state, 0).has_acted_this_round,
        "a bet gives the earlier checker a full option again"
    );
    assert_eq!(state.min_raise, 100);
}

#[test]
fn a_check_facing_a_bet_is_refused() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    let rejected = act(&mut state, 1, now, PlayerAction::Check).expect_err("cannot check a bet");
    assert!(rejected.contains("bet to call"), "got: {rejected}");
}

#[test]
fn an_under_sized_raise_is_refused_with_the_legal_amount_named() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    let rejected = act(&mut state, 1, now, PlayerAction::Raise(150))
        .expect_err("150 is a raise of 50 into a 100 min raise");
    assert!(rejected.contains("Minimum raise"), "got: {rejected}");
}

#[test]
fn acting_out_of_turn_is_refused() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    assert_eq!(
        act(&mut state, 2, now, PlayerAction::Check),
        Err("Not your turn".to_string())
    );
}

#[test]
fn a_stranger_cannot_act() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    assert_eq!(
        apply_player_action(
            &mut state,
            Principal::from_slice(&[0xBA, 0xD0]),
            now,
            PlayerAction::Check
        ),
        Err("Not at table".to_string())
    );
}

#[test]
fn no_action_is_legal_once_the_hand_is_complete() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    state.phase = GamePhase::HandComplete;
    assert_eq!(
        act(&mut state, 0, now, PlayerAction::Check),
        Err("No hand in progress".to_string())
    );
}

// =============================================================================
// 4. CHARACTERISATION: A DEFECT FOUND BY THE AUDIT AND NOT FIXED HERE
//
// Same convention as `tools/differential/tests/fast_subset.rs`: assert TODAY'S
// wrong answer so it cannot change silently, and describe what the rules require.
// Fixing this one changes which hands reach showdown and who is eligible for
// which pot, i.e. it lands in the payout path another agent owns.
// =============================================================================

/// **DEFECT (docs/DEFECTS.md E-32), characterised not fixed.** A player marked
/// `Disconnected` mid-street is skipped by the betting round but is NOT folded, so
/// the street closes without them ever calling -- and they keep both their chips
/// and their eligibility for every pot they had already paid into.
///
/// `is_betting_round_complete`, `count_active_players` and `find_next_active_seat`
/// all require `status == Active`, so a `Disconnected` seat blocks nothing and is
/// offered nothing. But `determine_winners` evaluates every player with
/// `!has_folded`. Under the rules a player who does not act must be folded and
/// forfeit what they have in; here they get a free ride to showdown.
///
/// Reachable on `table_2` (45 s) and `table_3` (60 s), where the hardcoded 30 s
/// disconnect threshold in `check_timeouts` fires BEFORE the action clock, and
/// only for a player who is not the one on the clock. On `table_1` /
/// `btc_table_1` the two thresholds are equal, which is E-06 instead.
///
/// THE RULE. Robert's Rules of Poker, Section 1 #12: a player called upon to act
/// who fails to act has a folded hand. There is no "skip the player and keep them
/// live" state in poker.
#[test]
fn characterises_a_disconnected_player_being_skipped_yet_left_live_in_the_hand() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    // Everyone paid 20 pre-flop, so seat 2 has money in the main pot.
    for s in 0..3usize {
        let p = state.players[s].as_mut().unwrap();
        p.total_bet_this_hand = 20;
    }
    state.pot = 60;

    act(&mut state, 0, now, PlayerAction::Bet(200)).expect("seat 0 leads the flop");

    // Seat 2 stops heartbeating. This is exactly what `check_timeouts` does after
    // 30 s of silence, and it does it to a player who is not on the clock.
    state.players[2].as_mut().unwrap().status = PlayerStatus::Disconnected;

    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls 200");

    // TODAY'S ANSWER, asserted so it cannot drift.
    assert_eq!(
        state.phase,
        GamePhase::Turn,
        "the flop closed without seat 2 ever being asked to act"
    );
    let quiet = seat(&state, 2);
    assert!(
        !quiet.has_folded,
        "seat 2 is still live in the hand despite never calling the 200"
    );
    assert_eq!(quiet.chips, 5_000, "and kept every chip they should have had to call");
    assert_eq!(
        quiet.total_bet_this_hand, 20,
        "while still holding 20 in the pot, which keeps them eligible for the main pot"
    );
    assert_eq!(state.pot, 60 + 400);

    // What the rules require instead:
    //   quiet.has_folded == true, and the 20 forfeited.
}

/// Pre-flop, unraised: the big blind may check their option, and doing so closes
/// the round. Pinned because `bb_has_option` is read by the same entry sequence
/// the timer fix rearranged.
#[test]
fn the_big_blind_may_check_their_option_in_an_unraised_pot() {
    let now = 1_000 * SEC;
    let mut state = preflop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 2, now, PlayerAction::Call).expect("UTG limps");
    act(&mut state, 0, now, PlayerAction::Call).expect("SB completes");
    assert_eq!(state.action_on, 1, "the option is the big blind's");
    assert!(state.bb_has_option);
    assert!(!is_betting_round_complete(&state));
    act(&mut state, 1, now, PlayerAction::Check).expect("the big blind checks the option");
    assert_eq!(state.phase, GamePhase::Flop);
}
