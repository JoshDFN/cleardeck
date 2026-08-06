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
    apply_player_action, can_still_act, is_betting_round_complete, is_in_hand, plan_payouts,
    resolve_expired_action_timer, ActionTimer, Card, Currency, GamePhase, Player, PlayerAction,
    PlayerStatus, Rank, Suit, TableConfig, TableState,
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
        departed_stakes: None,
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

fn card(rank: Rank, suit: Suit) -> Card {
    Card { rank, suit }
}

/// Put a known hand in front of a seat.
///
/// The section-4 and -5 tests need real hole cards, because the PAYOUT side of the
/// engine -- `live_claims`, and therefore `plan_payouts` -- will not pay a seat
/// that holds none. That asymmetry is the whole subject of those tests: the money
/// question and the whose-turn question have to be asked of the same seats.
fn deal(state: &mut TableState, s: u8, hand: (Card, Card)) {
    state.players[s as usize]
        .as_mut()
        .expect("seat occupied")
        .hole_cards = Some(hand);
}

/// Put a fixed five-card board out and move the hand to the river, so
/// `plan_payouts` can be asked, on a real showdown, who is eligible for what.
///
/// `A♠ K♠ 7♦ 2♣ 9♥`: no pair, no flush and no straight on board, so the winner is
/// decided entirely by the hole cards the test dealt.
fn board_to_the_river(state: &mut TableState) {
    state.community_cards = vec![
        card(Rank::Ace, Suit::Spades),
        card(Rank::King, Suit::Spades),
        card(Rank::Seven, Suit::Diamonds),
        card(Rank::Two, Suit::Clubs),
        card(Rank::Nine, Suit::Hearts),
    ];
    state.phase = GamePhase::River;
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
// 4. A SEAT THAT STOPS RESPONDING IS STILL IN THE HAND -- AND MUST PAY FOR IT
//
// THE DEFECT (docs/DEFECTS.md E-32, and E-06 which is the same mistake on a table
// whose two thresholds coincide). `check_timeouts` marks a seat `Disconnected`
// after a lull with no heartbeat. Every count that drove the betting round --
// `find_next_active_seat`, `count_active_players`, `count_players_can_act` and
// `is_betting_round_complete` -- then filtered on `status == PlayerStatus::Active`,
// so that seat was never offered the action and never blocked the street. But
// ELIGIBILITY for the pot is decided by `live_claims`: seated, holds cards, has
// not folded, and nothing about `status`. The seat therefore kept the chips it
// would have had to call AND its claim on every pot layer it had already paid
// into.
//
// It is client-controlled -- stop sending heartbeats -- and it is worth roughly a
// big blind a hand, in a game whose entire edge is a fraction of one. It cut the
// other way too: with a `Disconnected` seat still holding cards,
// `count_active_players` could reach 1 while TWO seats still had a claim, and the
// pot was handed over without the showdown the other seat had paid for.
//
// REPRODUCED ON THE RUNNING LOCAL `table_2` BEFORE IT WAS FIXED:
//   hand 3: exactly ONE action for the whole hand (seat 3, a forced Fold), a full
//           five-card board dealt, and two non-responding seats carried to a
//           showdown neither had acted in;
//   hand 5: a seat called `sit_out()` on the flop -- the same defect with no wait
//           at all -- was never asked to match a 2 ICP bet, kept its stack, and
//           was still `has_folded = false` holding cards at the showdown.
//
// THE RULE, AND THE SOURCE. Poker gives a player facing a bet three options --
// call, raise, fold -- and no fourth; there is no "skip me but keep my claim".
// Robert's Rules of Poker (Ciaffone) is the source the wave-4 audit already
// recorded against E-32: a player called upon to act who fails to act has a folded
// hand. Every online room implements exactly that -- your clock runs whether or
// not your client is connected, and the hand is folded when it expires -- and the
// "disconnect protection" a few rooms offered in the early 2000s, which capped a
// dropped player at what they had already put in while they kept the rest of their
// stack, was withdrawn because players triggered it on purpose. That cap is
// precisely what this engine was handing out by accident.
//
// THE FIX. Participation is now asked of the SAME predicate as eligibility
// (`is_in_hand` / `can_still_act`). A seat that has stopped responding is offered
// the action like anybody else, its own clock runs, and
// `resolve_expired_action_timer` folds it when the clock expires. `Disconnected`
// no longer moves money.
//
// WHAT THAT DOES TO AN HONEST DISCONNECTION: nothing at all until their clock
// expires, and the street can no longer close behind their back, so a pot they
// have chips in cannot be given away while they are still in it
// (`a_pot_is_not_given_away_while_a_seat_that_stopped_responding_still_holds_cards`).
// They get the full `action_timeout_secs` -- 30 s on `table_1`, 45 on `table_2`,
// 60 on `table_3` -- and can act the moment they reconnect inside it
// (`a_seat_that_comes_back_inside_its_own_clock_plays_the_hand_out`). The
// heartbeat threshold itself, which is all a flaky connection trips, was raised
// from 30 s to 90 s and now decides nothing but the badge and the auto-kick clock.
// =============================================================================

/// **THE EXPLOIT, and the money it moved.** Three-handed on the flop, everybody in
/// for 20, pot 60. Seat 0 leads for 200. Seat 2 stops heartbeating -- which is all
/// it takes; `check_timeouts` marks them `Disconnected` and no message from them
/// is ever needed again -- and seat 1 calls.
///
/// Seat 2 holds `A♥ A♦`, so on the `A♠ K♠ 7♦ 2♣ 9♥` board they have the nuts and
/// WILL be paid anything they are eligible for.
///
/// BEFORE THE FIX: the flop closed with seat 2 never asked to call. Phase `Turn`,
/// seat 2 `has_folded = false`, stack still 5,000, and `plan_payouts` handed them
/// the whole 60 main pot -- 40 of it other players' money -- for a hand they had
/// declined to put another chip into.
///
/// AFTER THE FIX: the flop does NOT close. Seat 2 is on the clock owing 200, and
/// when the clock runs out they are folded and paid nothing.
#[test]
fn a_seat_that_stops_responding_cannot_win_a_pot_it_declined_to_match() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    for s in 0..3u8 {
        let p = state.players[s as usize].as_mut().unwrap();
        p.total_bet_this_hand = 20;
    }
    state.pot = 60;
    deal(&mut state, 0, (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)));
    deal(&mut state, 1, (card(Rank::Six, Suit::Diamonds), card(Rank::Four, Suit::Spades)));
    // The quiet seat has the best hand, so eligibility is the only thing between
    // them and the money.
    deal(&mut state, 2, (card(Rank::Ace, Suit::Hearts), card(Rank::Ace, Suit::Diamonds)));

    act(&mut state, 0, now, PlayerAction::Bet(200)).expect("seat 0 leads the flop");

    // The whole attack. No message from seat 2 is required, then or later.
    state.players[2].as_mut().unwrap().status = PlayerStatus::Disconnected;

    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls 200");

    // THE RULES ASSERTION. Before the fix this was `GamePhase::Turn`: the street
    // closed with a live seat never asked for the 200.
    assert_eq!(
        state.phase,
        GamePhase::Flop,
        "the flop cannot close while a seat that can still win the pot owes 200"
    );
    assert!(
        !is_betting_round_complete(&state),
        "seat 2 has not acted and has not matched the bet"
    );
    assert_eq!(state.action_on, 2, "the action is OFFERED to the quiet seat");
    assert_eq!(seat(&state, 2).current_bet, 0, "and they still owe the whole 200");

    // THE MONEY ASSERTION, part one. Even in the state the exploit used to reach --
    // a showdown with the quiet seat unfolded -- being unfolded is now the ONLY way
    // to be eligible, and they are not going to stay unfolded.
    let quiet = seat_principal(2);
    let mut showdown = state.clone();
    board_to_the_river(&mut showdown);
    assert_eq!(
        plan_payouts(&showdown).amount_for_principal(quiet),
        60,
        "sanity: an UNFOLDED seat with the nuts is eligible for the 60 it paid \
         into -- which is exactly the 60 the exploit used to collect"
    );

    // THE MONEY ASSERTION, part two. Their clock runs out, as it must for a seat
    // that will not act, and folds them.
    let folded_seat = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1)
        .expect("the quiet seat's clock expires and is resolved");
    assert_eq!(folded_seat, 2);
    assert!(
        seat(&state, 2).has_folded,
        "a seat that does not act on its hand is folded: Robert's Rules of Poker"
    );

    let mut showdown = state.clone();
    board_to_the_river(&mut showdown);
    let plan = plan_payouts(&showdown);
    assert_eq!(
        plan.amount_for_principal(quiet),
        0,
        "THE FIX: the seat that declined to match wins nothing, holding the nuts. \
         plan={:?}",
        plan.payouts
    );
    assert!(plan.conserves(), "and every chip collected is still paid out");
    assert_eq!(
        seat(&state, 2).total_bet_this_hand,
        20,
        "the 20 they had in is forfeited to the pot, exactly as a fold forfeits it"
    );
}

/// **The same defect with no waiting at all.** `sit_out()` used to set
/// `PlayerStatus::SittingOut` with no phase check, so one call from a player facing
/// a bet took them out of the betting round while leaving them holding cards.
/// Reproduced on the running local `table_2`, hand 5.
///
/// This test drives the ENGINE half, which is what makes the exploit pay: whatever
/// door sets a non-`Active` status on a seat that is in the hand, the street must
/// not close behind it. (`sit_out()` itself is an `#[ic_cdk::update]`; the
/// deferral it now performs is verified against the deployed canister.)
#[test]
fn a_seat_that_sits_out_mid_hand_does_not_leave_the_betting_round_it_is_in() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    for s in 0..3u8 {
        state.players[s as usize].as_mut().unwrap().total_bet_this_hand = 20;
        deal(
            &mut state,
            s,
            (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)),
        );
    }
    state.pot = 60;

    act(&mut state, 0, now, PlayerAction::Bet(200)).expect("seat 0 leads the flop");
    state.players[2].as_mut().unwrap().status = PlayerStatus::SittingOut;
    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls 200");

    assert_eq!(
        state.phase,
        GamePhase::Flop,
        "sitting out cannot close a street you still hold cards in"
    );
    assert_eq!(state.action_on, 2, "the seat is still owed an action");
    assert!(can_still_act(seat(&state, 2)));
}

/// **The honest player, direction one: a pot they already have chips in cannot be
/// given away while they are still in it.**
///
/// Three-handed, everybody in for 20. Seat 2's connection drops. Seat 0 folds.
///
/// `count_active_players` used to count only `Active` seats, so it saw ONE player
/// left and `advance_game` would have called `end_hand_single_winner` -- handing
/// seat 1 the pot without a showdown, while seat 2 was still unfolded and holding
/// cards that might have won it. Now two seats are in the hand, so the hand stays
/// in progress and seat 2 keeps the clock it is entitled to.
#[test]
fn a_pot_is_not_given_away_while_a_seat_that_stopped_responding_still_holds_cards() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    for s in 0..3u8 {
        state.players[s as usize].as_mut().unwrap().total_bet_this_hand = 20;
        deal(
            &mut state,
            s,
            (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)),
        );
    }
    state.pot = 60;

    state.players[2].as_mut().unwrap().status = PlayerStatus::Disconnected;
    act(&mut state, 0, now, PlayerAction::Fold).expect("seat 0 folds");

    assert_eq!(
        state.phase,
        GamePhase::Flop,
        "two seats still hold cards, so this is not a fold-out"
    );
    assert!(
        is_in_hand(seat(&state, 2)),
        "the seat that stopped responding is still in the hand"
    );
    assert!(
        !seat(&state, 2).has_folded,
        "and nothing has folded them: only their own clock can"
    );
}

/// **The honest player, direction two: reconnect inside your clock and you play.**
///
/// A dropped connection costs nothing at all until the action clock expires. The
/// street cannot close without them, so they come back to the same decision they
/// left, with their pot equity intact.
#[test]
fn a_seat_that_comes_back_inside_its_own_clock_plays_the_hand_out() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    for s in 0..3u8 {
        state.players[s as usize].as_mut().unwrap().total_bet_this_hand = 20;
    }
    state.pot = 60;
    deal(&mut state, 0, (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)));
    deal(&mut state, 1, (card(Rank::Six, Suit::Diamonds), card(Rank::Four, Suit::Spades)));
    deal(&mut state, 2, (card(Rank::Ace, Suit::Hearts), card(Rank::Ace, Suit::Diamonds)));

    act(&mut state, 0, now, PlayerAction::Bet(200)).expect("seat 0 leads");
    state.players[2].as_mut().unwrap().status = PlayerStatus::Disconnected;
    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls");
    assert_eq!(state.action_on, 2, "the action waited for them");

    // `heartbeat()` puts a reconnecting player back to Active; the decision they
    // come back to is the one they left.
    let back = now + (TIMEOUT_SECS - 5) * SEC;
    state.players[2].as_mut().unwrap().status = PlayerStatus::Active;
    act(&mut state, 2, back, PlayerAction::Call).expect("they call the 200 they owe");

    assert_eq!(state.phase, GamePhase::Turn, "and now the street closes");
    assert_eq!(seat(&state, 2).total_bet_this_hand, 220);
    assert_eq!(seat(&state, 2).chips, 5_000 - 200);
    let mut showdown = state.clone();
    board_to_the_river(&mut showdown);
    assert_eq!(
        plan_payouts(&showdown).amount_for_principal(seat_principal(2)),
        660,
        "having called, they are eligible for the whole 660 their aces win: a \
         disconnection that ends inside the clock costs nothing"
    );
}

/// **The class, not the instance.** The bug was never "check_timeouts is wrong"; it
/// was that PARTICIPATION and ELIGIBILITY were asked of different predicates. This
/// pins the relationship for every status a seat can hold: a seat the engine will
/// wait for is always a seat that can win, and a seat that can win is either owed
/// an action or all-in. There is no third state.
///
/// If someone reintroduces a `status ==` filter into the betting round, one of
/// these fails.
#[test]
fn every_seat_the_engine_waits_for_is_a_seat_that_can_win_the_pot() {
    let now = 1_000 * SEC;
    for status in [
        PlayerStatus::Active,
        PlayerStatus::Disconnected,
        PlayerStatus::SittingOut,
    ] {
        for folded in [false, true] {
            for all_in in [false, true] {
                let mut state = flop_table(&[5_000, 5_000, 5_000], now);
                for s in 0..3u8 {
                    deal(
                        &mut state,
                        s,
                        (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)),
                    );
                }
                {
                    let p = state.players[2].as_mut().unwrap();
                    p.status = status.clone();
                    p.has_folded = folded;
                    p.is_all_in = all_in;
                }
                let p = seat(&state, 2);

                assert_eq!(
                    is_in_hand(p),
                    !folded,
                    "a dealt-in seat is in the hand exactly while it has not folded \
                     (status {status:?}, all_in {all_in})"
                );
                if can_still_act(p) {
                    assert!(
                        is_in_hand(p),
                        "the engine must never wait for a seat that cannot win \
                         (status {status:?}, folded {folded}, all_in {all_in})"
                    );
                }
                if is_in_hand(p) && !can_still_act(p) {
                    assert!(
                        p.is_all_in,
                        "the only way to be eligible without being asked for more \
                         money is to be all-in (status {status:?})"
                    );
                }
            }
        }
    }
}

/// An all-in seat is the ONE legitimate way to be eligible for a pot you cannot
/// keep matching, and the fix must not have broken it: it is not folded, it is not
/// waited for, and the side-pot layering caps what it can win at what it paid.
#[test]
fn an_all_in_seat_is_still_capped_rather_than_folded() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 150], now);
    for s in 0..3u8 {
        deal(
            &mut state,
            s,
            (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)),
        );
    }
    deal(
        &mut state,
        2,
        (card(Rank::Ace, Suit::Hearts), card(Rank::Ace, Suit::Diamonds)),
    );

    act(&mut state, 0, now, PlayerAction::Bet(1_000)).expect("seat 0 leads");
    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls");
    act(&mut state, 2, now, PlayerAction::AllIn).expect("seat 2 is all-in for 150");

    assert!(seat(&state, 2).is_all_in);
    assert!(!seat(&state, 2).has_folded, "all-in is not folded");
    assert!(is_in_hand(seat(&state, 2)), "and it is still in the hand");
    assert!(!can_still_act(seat(&state, 2)), "but is owed no further action");
    assert_eq!(state.phase, GamePhase::Turn, "the street closes: nobody else owes");

    let mut showdown = state.clone();
    board_to_the_river(&mut showdown);
    let plan = plan_payouts(&showdown);
    assert_eq!(
        plan.amount_for_principal(seat_principal(2)),
        450,
        "the nuts all-in for 150 wins 150 from each of the three stakes and no \
         more: capped at what it matched. plan={:?}",
        plan.payouts
    );
    assert!(plan.conserves());
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

// =============================================================================
// 5. WHAT THE EXPLOIT WAS WORTH
// =============================================================================

/// **Quantify it.** The two states differ by one flag -- was the quiet seat folded
/// or not -- and `plan_payouts` is the engine's own answer to "who gets what", so
/// running both over the same deals measures the edge in chips rather than
/// asserting it.
///
/// The situation is the one reproduced above and on the running `table_2`: three
/// handed, everyone in for 20 pre-flop, seat 0 leads the flop for 200 and seat 1
/// calls. The quiet seat has 5,000 behind and does not want to put in 200.
///
///   * PLAYING BY THE RULES they fold: -20, every hand, with certainty.
///   * EXPLOITING they are skipped and stay eligible: they keep the 200 AND hold a
///     claim on the 60 main pot, which they collect whenever their two cards beat
///     both of the other two hands.
///
/// The deals are dealt from a real deck by a fixed LCG, so the number is
/// reproducible; `--nocapture` prints it.
#[test]
fn the_free_showdown_was_worth_about_a_big_blind_a_hand() {
    const HANDS: u64 = 20_000;
    let now = 1_000 * SEC;
    let quiet = seat_principal(2);
    let mut rng: u64 = 0x5EED_C1EA_2DEC_0003;
    let mut next = move || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng >> 33) as usize
    };

    let mut exploit_total: i64 = 0;
    let mut wins = 0u64;
    for _ in 0..HANDS {
        // A real deal: shuffle a real deck, take six hole cards and five board.
        let mut deck = poker_core::create_deck();
        for i in (1..deck.len()).rev() {
            deck.swap(i, next() % (i + 1));
        }

        let mut state = flop_table(&[5_000, 5_000, 5_000], now);
        for s in 0..3usize {
            let p = state.players[s].as_mut().unwrap();
            // seats 0 and 1 put in 20 pre-flop and then 200 on the flop; the quiet
            // seat put in the 20 and nothing since.
            p.total_bet_this_hand = if s == 2 { 20 } else { 220 };
            p.hole_cards = Some((deck[2 * s], deck[2 * s + 1]));
        }
        state.pot = 460;
        state.community_cards = deck[6..11].to_vec();
        state.phase = GamePhase::River;

        // THE EXPLOIT: skipped by the betting round, still unfolded.
        let exploiting = plan_payouts(&state).amount_for_principal(quiet) as i64;
        // BY THE RULES: a player who will not match the bet is folded.
        state.players[2].as_mut().unwrap().has_folded = true;
        let folding = plan_payouts(&state).amount_for_principal(quiet) as i64;

        assert_eq!(folding, 0, "a folded seat is paid nothing, always");
        if exploiting > 0 {
            wins += 1;
        }
        exploit_total += exploiting;
    }

    // Their stake is 20 either way; the edge is what the exploit collects on top.
    let edge_per_hand = exploit_total as f64 / HANDS as f64;
    println!(
        "E-32 measured over {HANDS} deals: the skipped seat collected the 60 main \
         pot on {wins} of them ({:.1}%), worth {:.2} chips a hand = {:.2} big \
         blinds a hand, against a certain -20 for folding. Net edge {:.2} chips \
         ({:.2} BB) per hand, taken from the two players who were still betting.",
        100.0 * wins as f64 / HANDS as f64,
        edge_per_hand,
        edge_per_hand / 20.0,
        edge_per_hand,
        edge_per_hand / 20.0,
    );

    assert!(
        edge_per_hand > 15.0,
        "the free showdown is worth close to a big blind a hand, measured {edge_per_hand}"
    );
    assert!(
        (wins as f64 / HANDS as f64) > 0.25,
        "and it converts on roughly a third of deals, measured {wins}/{HANDS}"
    );
}
