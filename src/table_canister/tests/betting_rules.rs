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
//! Scope note: sections 1 to 5 stay inside one betting round on purpose. Since
//! E-106 the settlement runs on the host too (`deal_hand` + `open_the_action`
//! is the deal, `apply_player_action` closes a hand through the real payout,
//! and an unwired archive buffers the record), so sections 6 to 8 play hands
//! to completion; the money side of the pot award is still `tests/money_safety`
//! on PocketIC.

use candid::Principal;
use table_canister::{
    apply_player_action, build_table_view, can_still_act, deal_hand, is_betting_round_complete,
    is_in_hand, open_the_action, plan_payouts, record_voluntary_show,
    resolve_expired_action_timer, vacate_seat, ActionTimer, Card, Currency, GamePhase, Player,
    PlayerAction, PlayerStatus, Rank, ShuffleProof, Suit, TableConfig, TableState, TableView,
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

/// A seat with no cards yet. Every caller is [`flop_table`], which deals them:
/// a seat in a live betting round that holds no cards is not in the hand at all
/// (see the note there and docs/SECURITY-FINDINGS.md FINDING 17).
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
    // EVERY SEAT IN A LIVE BETTING ROUND HOLDS CARDS, and this fixture has to
    // reflect that or it is not describing a state the engine can reach.
    //
    // These players used to be built with `hole_cards: None` while `deck_index`
    // was already advanced past `2 * stacks.len()` hole cards -- the accounting
    // said they had been dealt in and the seats said they had not. It compiled and
    // it passed, because `is_in_hand` accepted a seat that was `Active` and held no
    // cards, which is exactly docs/SECURITY-FINDINGS.md FINDING 17. So eighteen
    // betting-rules tests, including `every_seat_the_engine_waits_for_is_a_seat_
    // that_can_win_the_pot`, were asserting the rules of poker against a table of
    // seats that could not win anything.
    //
    // The cards come off the same deck `deck_index` is counted against, so the
    // fixture is now internally consistent: seat `i` holds `deck[2i]` and
    // `deck[2i+1]`, and the index starts after them.
    let deck = poker_core::create_deck();
    let mut players: Vec<Option<Player>> = Vec::new();
    for (i, chips) in stacks.iter().enumerate() {
        let mut p = player(i as u8, *chips);
        p.hole_cards = Some((deck[2 * i], deck[2 * i + 1]));
        players.push(Some(p));
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
        last_hand_went_to_showdown: None,
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

// =============================================================================
// 6. A DEAL THAT LEAVES FEWER THAN TWO SEATS ABLE TO ACT RUNS THE BOARD OUT, AND
//    THE CLOCK NEVER FOLDS A SEAT THAT CANNOT ACT
//
// docs/DEFECTS.md E-106 (docs/CODEBASE-REVIEW-2026-09-28.md gap 4). Posting a
// blind or an ante can take a seat to zero and mark it all-in, and when that
// leaves nobody owed an action the hand has nothing to wait for: the board runs
// out and the pot goes by hand strength. The deal used to arm the clock
// unconditionally, `find_next_active_seat_with_chips` handed it the big blind
// when no seat had chips, and `resolve_expired_action_timer` folded whatever
// seat the clock named without asking whether that seat could act. So two short
// stacks all-in from the posts became: the big blind is folded by its own clock
// and the small blind takes the whole pot. Conservation holds, so the
// money-safety invariants could not see it. These tests assert the WINNER.
//
// They drive `deal_hand` + `open_the_action`, which is `start_new_hand` with the
// platform pulled out, on a deck stacked so the winner is known.
// =============================================================================

/// A table between hands: the seats are occupied and funded, nobody holds cards,
/// the button has never moved. What `start_new_hand` finds on the first hand.
fn waiting_table(stacks: &[u64], ante: u64) -> TableState {
    let mut state = flop_table(stacks, 0);
    for p in state.players.iter_mut().flatten() {
        p.hole_cards = None;
    }
    state.config.ante = ante;
    state.phase = GamePhase::WaitingForPlayers;
    state.community_cards.clear();
    state.deck_index = 0;
    state.action_timer = None;
    state.hand_number = 0;
    state.first_hand = true;
    state.dealer_seat = 0;
    state.small_blind_seat = 0;
    state.big_blind_seat = 0;
    state.current_bet = 0;
    state
}

/// A deck with `top` on top, in that order, and the rest of the pack after it.
/// `deal_hand` deals seat by seat in seat order (two cards each), then burns one
/// and deals the flop, burns one and deals the turn, burns one and deals the
/// river.
fn stacked_deck(top: &[Card]) -> Vec<Card> {
    let mut deck: Vec<Card> = top.to_vec();
    for c in poker_core::create_deck() {
        if !deck.iter().any(|d| d.rank == c.rank && d.suit == c.suit) {
            deck.push(c);
        }
    }
    assert_eq!(deck.len(), 52, "the stacked deck is still one pack");
    deck
}

/// The real deal, as `start_new_hand` runs it: post, deal, then open the action.
fn run_the_deal(state: &mut TableState, deck: Vec<Card>, now: u64) {
    let proof = ShuffleProof {
        seed_hash: "test".to_string(),
        revealed_seed: None,
        timestamp: now,
    };
    deal_hand(state, deck, &proof, now).expect("the deal is legal");
    open_the_action(state, now);
}

/// `A♠ A♥` for one seat, `7♣ 2♦` for the other, over `3♦ 8♣ J♥ 4♠ 9♦`: no
/// straight, no flush, nothing on board that beats a pair of aces.
const BOARD_BLANKS: [(Rank, Suit); 5] = [
    (Rank::Three, Suit::Diamonds),
    (Rank::Eight, Suit::Clubs),
    (Rank::Jack, Suit::Hearts),
    (Rank::Four, Suit::Spades),
    (Rank::Nine, Suit::Diamonds),
];
const ACES: [(Rank, Suit); 2] = [(Rank::Ace, Suit::Spades), (Rank::Ace, Suit::Hearts)];
const KINGS: [(Rank, Suit); 2] = [(Rank::King, Suit::Spades), (Rank::King, Suit::Hearts)];
const SEVEN_DEUCE: [(Rank, Suit); 2] = [(Rank::Seven, Suit::Clubs), (Rank::Two, Suit::Diamonds)];

fn cards(spec: &[&[(Rank, Suit)]], burn: (Rank, Suit)) -> Vec<Card> {
    // hole cards, seat by seat; then burn, flop; burn, turn; burn, river.
    let hole: Vec<Card> = spec.iter().flat_map(|h| h.iter().map(|&(r, s)| card(r, s))).collect();
    let board: Vec<Card> = BOARD_BLANKS.iter().map(|&(r, s)| card(r, s)).collect();
    let burn = card(burn.0, burn.1);
    let mut top = hole;
    top.push(burn);
    top.extend_from_slice(&board[0..3]);
    top.push(burn);
    top.extend_from_slice(&board[3..4]);
    top.push(burn);
    top.extend_from_slice(&board[4..5]);
    // the burn card repeats in `top`; stacked_deck keeps its first appearance and
    // the later ones are replaced by whatever the pack holds next, which is fine:
    // burns are never read.
    let spare: Vec<Card> = poker_core::create_deck()
        .into_iter()
        .filter(|c| !top.iter().any(|t| t.rank == c.rank && t.suit == c.suit))
        .collect();
    let mut spare = spare.into_iter();
    let mut dedup: Vec<Card> = Vec::new();
    for c in top {
        if dedup.iter().any(|d| d.rank == c.rank && d.suit == c.suit) {
            dedup.push(spare.next().expect("pack has a spare card"));
        } else {
            dedup.push(c);
        }
    }
    stacked_deck(&dedup)
}

fn chips(state: &TableState) -> Vec<u64> {
    state.players.iter().flatten().map(|p| p.chips).collect()
}

/// HEADS-UP, first hand, so the button lands on seat 1 (small blind) and seat 0
/// posts the big blind. Seat 1 has 10 and seat 0 has 15: the posts leave BOTH
/// all-in. Seat 0 holds aces. Nobody can act, so the board runs out and seat 0
/// wins the 20 that was contested; its uncalled 5 comes back.
///
/// Before the fix the deal armed a clock on seat 0 (the one `find_next_active_
/// seat_with_chips` falls back to when no seat has chips), and the clock folded
/// them: the 7-2 took everything.
#[test]
fn a_heads_up_deal_where_the_posts_leave_both_seats_all_in_runs_the_board_out() {
    let now = 1_000 * SEC;
    let mut state = waiting_table(&[15, 10], 0);
    run_the_deal(&mut state, cards(&[&ACES, &SEVEN_DEUCE], (Rank::Six, Suit::Clubs)), now);

    assert_eq!((state.small_blind_seat, state.big_blind_seat), (1, 0), "sanity: heads-up, button on seat 1");
    assert_eq!(seat(&state, 1).total_bet_this_hand, 10, "sanity: the small blind posted its whole 10");
    assert!(seat(&state, 0).total_bet_this_hand >= 10, "sanity: the big blind posted at least that");

    // THE DEFECT, played out: whatever the deal left, let the clock expire on it.
    let armed = state.action_timer.clone();
    let folded = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1);

    assert_eq!(
        chips(&state),
        vec![25, 0],
        "seat 0, all-in for 15 from the big blind holding aces, must end with 25 \
         (the 20 contested + its 5 uncalled). Instead: the deal armed {armed:?}, the \
         clock folded {folded:?}, phase {:?}, board {:?}",
        state.phase,
        state.community_cards
    );
    assert!(armed.is_none(), "the deal must not arm a clock when no seat can act: {armed:?}");
    assert_eq!(folded, None, "and so there was nothing for the clock to fold");
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_eq!(state.community_cards.len(), 5, "the board ran out");
    assert!(!seat(&state, 0).has_folded && !seat(&state, 1).has_folded, "nobody was folded");
    assert_eq!(seat(&state, 0).timeout_count, 0, "and nobody was charged a timeout");
}

/// ANTE TABLE, three-handed, ante 50 over 10/20 blinds. Stacks 30, 50, 50: the
/// antes take every seat all-in before a blind is posted (the blinds then post
/// zero). Seat 0 is short with aces, seat 1 has kings, seat 2 has 7-2. Main pot
/// 90 (30 x 3) to the aces, side pot 40 (20 x 2) to the kings, 7-2 nothing.
#[test]
fn an_ante_table_whose_posts_leave_every_seat_all_in_runs_the_board_out_and_pays_by_hand_strength() {
    let now = 1_000 * SEC;
    let mut state = waiting_table(&[30, 50, 50], 50);
    run_the_deal(&mut state, cards(&[&ACES, &KINGS, &SEVEN_DEUCE], (Rank::Six, Suit::Clubs)), now);

    assert!(state.players.iter().flatten().all(|p| p.is_all_in), "sanity: the antes took everyone all-in");

    let armed = state.action_timer.clone();
    let folded = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1);

    assert_eq!(
        chips(&state),
        vec![90, 40, 0],
        "aces take the 90 main pot and kings the 40 side pot. Instead: the deal \
         armed {armed:?}, the clock folded {folded:?}, phase {:?}, board {:?}",
        state.phase,
        state.community_cards
    );
    assert!(armed.is_none(), "no seat can act, so no clock: {armed:?}");
    assert_eq!(folded, None);
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_eq!(state.community_cards.len(), 5);
    assert!(state.players.iter().flatten().all(|p| !p.has_folded && p.timeout_count == 0));
}

/// THE REFINEMENT. "Fewer than two can act" is not the whole rule: the one seat
/// that can still act may OWE a call, and then it must be asked. Heads-up, seat 0
/// (big blind) has 15 and is all-in from the post; seat 1 (small blind) has 1,000,
/// posted 10 and owes 5 more. Seat 1 gets a clock and a decision, and calling runs
/// the board out.
#[test]
fn a_deal_that_leaves_one_seat_owing_a_call_still_asks_it() {
    let now = 1_000 * SEC;
    let mut state = waiting_table(&[15, 1_000], 0);
    run_the_deal(&mut state, cards(&[&ACES, &SEVEN_DEUCE], (Rank::Six, Suit::Clubs)), now);

    assert_eq!(state.phase, GamePhase::PreFlop, "seat 1 still owes 5, so the hand waits");
    assert_eq!(state.action_on, 1);
    let timer = state.action_timer.as_ref().expect("a clock on the seat that owes the call");
    assert_eq!(timer.player_seat, 1);
    assert!(!is_betting_round_complete(&state));

    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 calls the 5");
    assert_eq!(state.phase, GamePhase::HandComplete, "nobody can act after the call");
    assert_eq!(state.community_cards.len(), 5);
    assert_eq!(chips(&state), vec![30, 985], "aces take 30; the 7-2 paid 15");
}

/// And the mirror: the one seat that can still act is owed NOTHING. Seat 1 (small
/// blind) has 5 and is all-in from the post; seat 0 (big blind) has 1,000 and
/// posted 20. Nobody can call anything, so the big blind has no decision: its 15
/// uncalled comes back and the board runs out for the 10, so it ends on 995.
#[test]
fn a_deal_where_the_only_seat_that_can_act_is_owed_nothing_runs_the_board_out() {
    let now = 1_000 * SEC;
    let mut state = waiting_table(&[1_000, 5], 0);
    run_the_deal(&mut state, cards(&[&SEVEN_DEUCE, &ACES], (Rank::Six, Suit::Clubs)), now);

    let armed = state.action_timer.clone();
    let folded = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1);

    assert_eq!(
        chips(&state),
        vec![995, 10],
        "the aces, all-in for 5, take 10; the big blind's 15 uncalled comes back. \
         Instead: the deal armed {armed:?}, the clock folded {folded:?}, phase {:?}",
        state.phase
    );
    assert!(armed.is_none(), "{armed:?}");
    assert_eq!(folded, None);
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_eq!(state.community_cards.len(), 5);
}

/// THE CLOCK HALF. Three-handed on the flop, seat 0 is all-in (it cannot act) and
/// the clock is pointed at it: the shape every table dealt before this fix can be
/// sitting in when the fix is upgraded in. Expiring it folds nobody, charges no
/// timeout, and moves the action to the next seat that can act. Seat 0 holds the
/// nuts and is still eligible for the pot.
#[test]
fn the_clock_never_folds_a_seat_that_cannot_act() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[0, 5_000, 5_000], now);
    for s in 0..3usize {
        state.players[s].as_mut().unwrap().total_bet_this_hand = 300;
    }
    state.pot = 900;
    state.players[0].as_mut().unwrap().is_all_in = true;
    deal(&mut state, 0, (card(Rank::Ace, Suit::Hearts), card(Rank::Ace, Suit::Diamonds)));
    deal(&mut state, 1, (card(Rank::Six, Suit::Clubs), card(Rank::Four, Suit::Hearts)));
    deal(&mut state, 2, (card(Rank::Six, Suit::Diamonds), card(Rank::Four, Suit::Spades)));
    assert!(!can_still_act(seat(&state, 0)), "sanity: seat 0 is all-in");
    assert_eq!(state.action_timer.as_ref().map(|t| t.player_seat), Some(0), "sanity: the clock names seat 0");

    let folded = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1);

    assert!(
        !seat(&state, 0).has_folded,
        "a seat that cannot act cannot be folded by its clock (resolved as {folded:?})"
    );
    assert_eq!(folded, None, "nobody timed out");
    assert_eq!(seat(&state, 0).timeout_count, 0);
    assert_eq!(seat(&state, 0).status, PlayerStatus::Active);
    assert_eq!(state.phase, GamePhase::Flop, "the street is still open: seats 1 and 2 have not acted");
    assert_eq!(state.action_on, 1, "the action moved to the next seat that can act");
    let timer = state.action_timer.as_ref().expect("a fresh clock on seat 1");
    assert_eq!(timer.player_seat, 1);
    assert_eq!(timer.started_at, now + TIMEOUT_SECS * SEC + 1, "started now, not back-dated");

    let mut showdown = state.clone();
    board_to_the_river(&mut showdown);
    let plan = plan_payouts(&showdown);
    assert_eq!(
        plan.amount_for_principal(seat_principal(0)),
        900,
        "and the all-in seat is still paid for the nuts: {:?}",
        plan.payouts
    );
    assert!(plan.conserves());
}

/// The same stale shape heads-up with BOTH seats all-in from the posts, as a
/// table dealt on the old engine is left when the fix is upgraded in: the clock
/// on the big blind expires, and instead of folding it the board runs out.
#[test]
fn a_clock_left_pointing_at_an_all_in_seat_runs_the_board_out_instead_of_folding_it() {
    let now = 1_000 * SEC;
    // Build the pre-fix state by hand: both posts all-in, clock on the big blind.
    let mut state = waiting_table(&[15, 10], 0);
    let deck = cards(&[&ACES, &SEVEN_DEUCE], (Rank::Six, Suit::Clubs));
    let proof = ShuffleProof { seed_hash: "test".to_string(), revealed_seed: None, timestamp: now };
    deal_hand(&mut state, deck, &proof, now).expect("the deal is legal");
    state.action_on = state.big_blind_seat;
    state.action_timer = Some(ActionTimer {
        player_seat: state.big_blind_seat,
        started_at: now,
        expires_at: now + TIMEOUT_SECS * SEC,
        using_time_bank: false,
    });

    let folded = resolve_expired_action_timer(&mut state, now + TIMEOUT_SECS * SEC + 1);

    assert_eq!(
        chips(&state),
        vec![25, 0],
        "the aces, all-in from the big blind, take the pot. Instead the clock folded \
         {folded:?}; phase {:?}",
        state.phase
    );
    assert_eq!(folded, None);
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_eq!(state.community_cards.len(), 5);
    assert!(state.action_timer.is_none());
}

// =============================================================================
// 7. A POT WON BECAUSE EVERYBODY FOLDED DOES NOT SHOW THE WINNER'S CARDS
//
// docs/DEFECTS.md E-107 (docs/CODEBASE-REVIEW-2026-09-28.md gap 3). The engine's
// own rule is in `end_hand_single_winner`: "they take the pot without showing a
// hand", and the archive keeps it (`push_winner` withholds the cards on a
// fold-out). The live view did not: `get_table_view` read `HandComplete` as a
// showdown and turned every unfolded seat face up, and after a fold-out the
// last player standing is exactly that -- unfolded, still holding cards until
// the next deal. So every viewer, seated or not, saw the winner's hand for the
// whole between-hands pause. At a real-money table that is a free read on every
// hand that ends by folds.
//
// These drive `build_table_view`, which is `get_table_view` with the platform
// pulled out, from every seat's point of view and from a stranger's, at every
// phase of a hand played through the real engine.
// =============================================================================

/// Somebody who is not at the table: the lobby, a railbird, a scraper.
fn stranger() -> Principal {
    Principal::from_slice(&[0xBA, 0xD0, 0x00, 0x01])
}

/// What `viewer` sees in front of seat `s`.
fn cards_seen_by(state: &TableState, viewer: Principal, s: u8, now: u64) -> Option<(Card, Card)> {
    let view: TableView = build_table_view(state, viewer, now);
    view.players[s as usize]
        .as_ref()
        .expect("seat occupied")
        .hole_cards
}

/// Every occupied seat, and a stranger, looks at every occupied seat: a seat
/// sees its own cards, and sees another seat's cards only if `revealed` says so.
fn assert_visibility(state: &TableState, now: u64, revealed: &[u8], when: &str) {
    let seats: Vec<u8> = state.players.iter().flatten().map(|p| p.seat).collect();
    for &looking in &seats {
        for &at in &seats {
            let seen = cards_seen_by(state, seat_principal(looking), at, now);
            let expected = if looking == at || revealed.contains(&at) {
                seat(state, at).hole_cards
            } else {
                None
            };
            assert_eq!(
                seen, expected,
                "{when}: seat {looking} looking at seat {at} (phase {:?}, folded {}, revealed set {revealed:?})",
                state.phase,
                seat(state, at).has_folded
            );
        }
    }
    for &at in &seats {
        let seen = cards_seen_by(state, stranger(), at, now);
        let expected = if revealed.contains(&at) { seat(state, at).hole_cards } else { None };
        assert_eq!(
            seen, expected,
            "{when}: a stranger looking at seat {at} (phase {:?}, folded {})",
            state.phase,
            seat(state, at).has_folded
        );
    }
}

/// Act as whoever the engine says is on action.
fn act_in_turn(state: &mut TableState, now: u64, action: PlayerAction) -> u8 {
    let s = state.action_on;
    let what = format!("seat {s} {action:?}");
    act(state, s, now, action).unwrap_or_else(|e| panic!("{what} at {:?}: {e}", state.phase));
    s
}

/// Three seats of 1,000, first hand, dealt from a stacked deck: aces, kings,
/// seven-deuce, in seat order, over the blank board.
fn three_handed_deal(now: u64) -> TableState {
    let mut state = waiting_table(&[1_000, 1_000, 1_000], 0);
    let deck = cards(&[&ACES, &KINGS, &SEVEN_DEUCE], (Rank::Six, Suit::Clubs));
    run_the_deal(&mut state, deck, now);
    assert_eq!(state.phase, GamePhase::PreFlop);
    for s in 0..3 {
        assert!(seat(&state, s).hole_cards.is_some(), "seat {s} was dealt in");
    }
    state
}

/// Everybody calls or checks to the river, asserting at every street that a
/// seat sees only its own cards. Returns with the river open and nobody acted.
fn check_down_to_the_river(state: &mut TableState, now: u64) {
    assert_visibility(state, now, &[], "pre-flop, before anybody acts");
    // Pre-flop: two calls and the big blind's check close the street.
    for _ in 0..2 {
        act_in_turn(state, now, PlayerAction::Call);
    }
    act_in_turn(state, now, PlayerAction::Check);
    assert_eq!(state.phase, GamePhase::Flop);
    assert_visibility(state, now, &[], "on the flop");
    for _ in 0..3 {
        act_in_turn(state, now, PlayerAction::Check);
    }
    assert_eq!(state.phase, GamePhase::Turn);
    assert_visibility(state, now, &[], "on the turn");
    for _ in 0..3 {
        act_in_turn(state, now, PlayerAction::Check);
    }
    assert_eq!(state.phase, GamePhase::River);
    assert_visibility(state, now, &[], "on the river");
}

/// A hand played to the river, where the first seat to act bets and the other
/// two fold: the bettor takes the pot without showing a hand. Every phase, from
/// every seat's point of view and a stranger's: a seat sees its own cards and
/// nobody else's, and that stays true after the fold-out, for the winner's
/// cards above all.
///
/// Before the fix, `HandComplete` was read as a showdown and the winner -- the
/// one unfolded seat, still holding cards -- was face up to the whole table.
#[test]
fn a_pot_won_because_everybody_folded_does_not_show_the_winners_cards() {
    let now = 1_000 * SEC;
    let mut state = three_handed_deal(now);
    check_down_to_the_river(&mut state, now);

    let bettor = act_in_turn(&mut state, now, PlayerAction::Bet(100));
    let first_fold = act_in_turn(&mut state, now, PlayerAction::Fold);
    let second_fold = act_in_turn(&mut state, now, PlayerAction::Fold);
    assert_eq!(state.phase, GamePhase::HandComplete, "two folds end the hand");
    assert!(!seat(&state, bettor).has_folded);
    assert!(seat(&state, first_fold).has_folded && seat(&state, second_fold).has_folded);
    assert!(
        seat(&state, bettor).hole_cards.is_some(),
        "the winner still holds cards until the next deal, which is the whole hazard"
    );
    assert_eq!(seat(&state, bettor).chips, 1_000 + 2 * BIG_BLIND, "sanity: the bettor took the pot");

    assert_visibility(&state, now, &[], "after the fold-out, between hands");

    // The winner record agrees: no hand and no cards, as the archive has always said.
    let view = build_table_view(&state, stranger(), now);
    let winner = view
        .last_hand_winners
        .iter()
        .find(|w| w.seat == bettor)
        .expect("the bettor is the recorded winner");
    assert_eq!(winner.cards, None, "a fold-out winner's record carries no cards");
    assert_eq!(winner.hand_rank, None, "and no hand");
}

/// The other ending. Checked down to a showdown, every seat that did not fold is
/// face up to everyone, the stranger included, and the seat that folded on the
/// flop stays hidden from everybody but itself. A guard that the fix hides only
/// what a fold-out should hide.
#[test]
fn a_showdown_still_turns_every_unfolded_hand_face_up() {
    let now = 1_000 * SEC;
    let mut state = three_handed_deal(now);
    assert_visibility(&state, now, &[], "pre-flop, before anybody acts");
    for _ in 0..2 {
        act_in_turn(&mut state, now, PlayerAction::Call);
    }
    act_in_turn(&mut state, now, PlayerAction::Check);
    assert_eq!(state.phase, GamePhase::Flop);

    // Three-handed on the first hand the button is seat 1, so the flop opens on
    // seat 2 (the seven-deuce): it checks, the aces bet, the kings call, and the
    // seven-deuce folds.
    assert_eq!(state.action_on, 2, "sanity: the flop opens on the small blind");
    let folder = act_in_turn(&mut state, now, PlayerAction::Check);
    let bettor = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    let caller = act_in_turn(&mut state, now, PlayerAction::Call);
    assert_eq!(act_in_turn(&mut state, now, PlayerAction::Fold), folder);
    assert_eq!((bettor, caller, folder), (0, 1, 2));
    assert_eq!(state.phase, GamePhase::Turn);
    assert_visibility(&state, now, &[], "on the turn, a seat folded");
    for _ in 0..2 {
        act_in_turn(&mut state, now, PlayerAction::Check);
    }
    assert_eq!(state.phase, GamePhase::River);
    assert_visibility(&state, now, &[], "on the river");
    for _ in 0..2 {
        act_in_turn(&mut state, now, PlayerAction::Check);
    }
    assert_eq!(state.phase, GamePhase::HandComplete, "the river checks through to a showdown");

    assert_visibility(&state, now, &[bettor, caller], "after the showdown, between hands");
    assert!(seat(&state, folder).has_folded);
    assert_eq!(
        cards_seen_by(&state, stranger(), folder, now),
        None,
        "the mucked hand stays mucked"
    );
    // The aces win, and a showdown winner's record carries the hand and the cards.
    let view = build_table_view(&state, stranger(), now);
    let winner = view.last_hand_winners.iter().find(|w| w.seat == bettor).expect("the aces win");
    assert!(winner.cards.is_some() && winner.hand_rank.is_some(), "a showdown winner's record shows the hand");
    assert_eq!(seat(&state, bettor).chips, 1_000 + 2 * BIG_BLIND + 40, "sanity: the aces took the pot");
}

/// The voluntary show. A seat that folded may not turn its cards up while the
/// hand is still live -- that tells the seats still playing what is out of the
/// deck -- but once the hand is over it may, and so may the fold-out winner, and
/// only then does the table see them.
#[test]
fn show_cards_is_refused_while_the_hand_is_live_and_reveals_to_everyone_once_it_is_over() {
    let now = 1_000 * SEC;
    let mut state = three_handed_deal(now);
    for _ in 0..2 {
        act_in_turn(&mut state, now, PlayerAction::Call);
    }
    act_in_turn(&mut state, now, PlayerAction::Check);
    assert_eq!(state.phase, GamePhase::Flop);
    let bettor = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    let folder = act_in_turn(&mut state, now, PlayerAction::Fold);
    let caller = act_in_turn(&mut state, now, PlayerAction::Call);
    assert_eq!(state.phase, GamePhase::Turn);

    // Two seats are still playing. The folded seat may not show.
    let refused = record_voluntary_show(&state, seat_principal(folder));
    assert!(
        refused.is_err(),
        "seat {folder} folded on the flop and the hand is live at {:?}: its show must be refused, got {refused:?}",
        state.phase
    );
    assert_visibility(&state, now, &[], "on the turn, after the refused show");
    // And neither may a seat that is still in the hand.
    assert!(record_voluntary_show(&state, seat_principal(bettor)).is_err());

    // Turn: the bettor bets again and the caller gives up. Fold-out.
    let _ = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    let _ = act_in_turn(&mut state, now, PlayerAction::Fold);
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_visibility(&state, now, &[], "after the fold-out, nobody has shown");

    // Now the folded seat may show ("I had it"), and so may the winner.
    record_voluntary_show(&state, seat_principal(folder)).expect("a show after the hand is over");
    assert_visibility(&state, now, &[folder], "after the folder's voluntary show");
    record_voluntary_show(&state, seat_principal(bettor)).expect("the winner may show too");
    assert_visibility(&state, now, &[folder, bettor], "after the winner's voluntary show");
    let _ = caller;
}

/// A table restored from state written before the flag existed (docs/
/// SECURITY-FINDINGS.md FINDING 14: the field is `opt`, and `None` is what the
/// previous release's state decodes to) sits at `HandComplete` with no record of
/// how the hand ended. The safe reading is "not a showdown": hidden until the
/// next deal, which clears the cards anyway.
#[test]
fn a_table_restored_without_the_flag_at_hand_complete_hides_every_hand() {
    let now = 1_000 * SEC;
    let mut state = three_handed_deal(now);
    check_down_to_the_river(&mut state, now);
    for _ in 0..3 {
        act_in_turn(&mut state, now, PlayerAction::Check);
    }
    assert_eq!(state.phase, GamePhase::HandComplete);
    assert_visibility(&state, now, &[0, 1, 2], "after a showdown, with the flag");

    state.last_hand_went_to_showdown = None;
    assert_visibility(&state, now, &[], "after a showdown, restored from state without the flag");
}

// =============================================================================
// 8. LEAVING FROM THE SEAT ON ACTION MOVES THE HAND ON THE WAY AN ACTION WOULD
//
// docs/DEFECTS.md E-108 (docs/CODEBASE-REVIEW-2026-09-28.md gap 5). `leave_table`
// marks the leaver folded and vacates the chair; when the clock was pointing at
// that chair it used to hand the clock to the next seat unconditionally, without
// asking `is_betting_round_complete` the way every action does through
// `advance_game`. A bets, B calls, C leaves on action: the street is closed, but
// the clock landed on A, whose only legal replies were Check or Fold on a bet
// they had made themselves -- and if A was away, the clock folded A out of a pot
// they had fully matched. Conservation holds, so the money-safety invariants
// could not see it. These tests assert WHAT THE HAND IS WAITING FOR after a
// departure, and drive `vacate_seat`, which is `leave_table` with the platform
// pulled out, on a hand dealt and bet through the real engine.
//
// Three-handed first hand: dealer seat 1, small blind seat 2, big blind seat 0,
// so the flop action runs 2, 0, 1. Seat 2 is "A", seat 0 "B", seat 1 "C".
// =============================================================================

/// Three seats of 1,000, pre-flop closed by two calls and the big blind's check:
/// the flop is out, the pot is 60, and seat 2 is first to act.
fn flop_three_handed(now: u64) -> TableState {
    let mut state = three_handed_deal(now);
    for _ in 0..2 {
        act_in_turn(&mut state, now, PlayerAction::Call);
    }
    act_in_turn(&mut state, now, PlayerAction::Check);
    assert_eq!(state.phase, GamePhase::Flop, "sanity: pre-flop closed");
    assert_eq!(state.pot, 60, "sanity: three big blinds in the middle");
    assert_eq!(state.action_on, 2, "sanity: first to act on the flop is left of the button");
    state
}

/// Leave as seat `s`, asserting the chair is empty afterwards.
fn leave(state: &mut TableState, s: u8, now: u64) -> u64 {
    let chips = vacate_seat(state, seat_principal(s), now)
        .unwrap_or_else(|e| panic!("seat {s} leaving at {:?}: {e}", state.phase));
    assert!(state.players[s as usize].is_none(), "seat {s} has left the table");
    chips
}

/// A bets 40, B calls, and C -- on action, owing 40 -- leaves. Every seat still
/// holding cards has matched the bet, so the flop betting is CLOSED: the turn is
/// dealt, the pot is the 140 that was put in, and nobody is asked to act on the
/// flop again.
///
/// Before the fix the clock was handed to A on the flop with A's own 40 still
/// standing as the bet: A could only Check or Fold, and an expiry folded A out of
/// a pot A had matched. The last line is the general statement of the rule: a
/// clock is never armed on a closed street.
#[test]
fn a_leaver_on_action_after_the_street_closed_does_not_hand_the_street_back() {
    let now = 1_000 * SEC;
    let mut state = flop_three_handed(now);
    let a = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    let b = act_in_turn(&mut state, now, PlayerAction::Call);
    assert_eq!((a, b), (2, 0));
    assert_eq!(state.action_on, 1, "sanity: the action is on C");
    assert!(!is_betting_round_complete(&state), "sanity: C still owes 40, the street is open");

    let left_with = leave(&mut state, 1, now);
    assert_eq!(left_with, 980, "C leaves with the stack behind; the 20 posted stays in the pot");

    assert_eq!(
        state.phase,
        GamePhase::Turn,
        "A bet, B called, C left on action: the flop betting is closed and the turn is dealt. \
         Instead the hand is at {:?} with the clock on {:?}, current bet {}",
        state.phase,
        state.action_timer.as_ref().map(|t| t.player_seat),
        state.current_bet
    );
    assert_eq!(state.community_cards.len(), 4, "the turn card is out");
    assert_eq!(state.pot, 140, "60 pre-flop, 40 from A, 40 from B: nothing came back and nothing vanished");
    assert_eq!(state.current_bet, 0, "a fresh street");
    for s in [a, b] {
        let p = seat(&state, s);
        assert!(!p.has_folded && !p.is_all_in, "seat {s} is still in the hand");
        assert_eq!(p.current_bet, 0, "seat {s} has nothing in front of them on the turn");
        assert!(!p.has_acted_this_round, "seat {s} has not acted on the turn");
        assert_eq!(p.chips, 940, "seat {s} paid 20 pre-flop and 40 on the flop");
    }
    // THE RULE. Whatever the phase, a clock is never armed on a closed street.
    assert!(
        !(is_betting_round_complete(&state) && state.action_timer.is_some()),
        "the clock {:?} is armed on a street the engine itself says is closed",
        state.action_timer
    );
    // On the turn the first seat after the (now empty) button is A, by position.
    // That is a fresh street, not the flop handed back: A may BET, which was
    // refused on the flop where A's own bet already stood.
    let timer = state.action_timer.as_ref().expect("the turn is open for betting");
    assert_eq!((state.action_on, timer.player_seat), (a, a));
    act(&mut state, a, now, PlayerAction::Bet(40)).expect("a bet opens the turn");
    assert_eq!(state.action_on, b, "and B is asked to answer it");
}

/// A bets 40 and B -- on action, owing 40 -- leaves. C has not acted, so the
/// street is still OPEN: the clock moves to C, A's bet stays standing, and the
/// hand waits for C exactly as it would after B folded.
///
/// Green before and after: this is the case the old branch handled. It is here
/// so the fix cannot be "always deal the next street".
#[test]
fn a_leaver_on_action_with_the_street_still_open_hands_the_clock_to_the_seat_that_owes_action() {
    let now = 1_000 * SEC;
    let mut state = flop_three_handed(now);
    let a = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    assert_eq!(state.action_on, 0, "sanity: the action is on B");

    let left_with = leave(&mut state, 0, now);
    assert_eq!(left_with, 980);

    assert_eq!(state.phase, GamePhase::Flop, "C has not answered the bet: the flop is still open");
    assert_eq!(state.action_on, 1, "the clock moved to C");
    let timer = state.action_timer.as_ref().expect("C is on the clock");
    assert_eq!((timer.player_seat, timer.expires_at), (1, now + TIMEOUT_SECS * SEC));
    assert_eq!(state.current_bet, 40, "A's bet stands");
    assert_eq!(seat(&state, a).current_bet, 40, "A's 40 is still in front of A, not returned: C can still call it");
    assert_eq!(state.pot, 100, "60 pre-flop and A's 40");
    assert!(!is_betting_round_complete(&state), "C owes 40");

    // C calls: the street closes and the turn is dealt, as after any call.
    act(&mut state, 1, now, PlayerAction::Call).expect("C may call");
    assert_eq!(state.phase, GamePhase::Turn);
    assert_eq!(state.pot, 140);
}

/// A bets 40, B folds, and C -- on action -- leaves. A is the only seat holding
/// cards: the hand ends by fold-out, A takes the pot without showing a hand
/// (docs/DEFECTS.md E-107), the clock is retired and the table waits for the
/// next deal.
///
/// Green before and after: `count_active_players == 1` was the first thing the
/// old branch asked. It is here because `advance_game` now asks it instead.
#[test]
fn a_leaver_on_action_who_leaves_one_player_ends_the_hand_by_fold_out() {
    let now = 1_000 * SEC;
    let mut state = flop_three_handed(now);
    let a = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    let b = act_in_turn(&mut state, now, PlayerAction::Fold);
    assert_eq!(state.action_on, 1, "sanity: the action is on C");

    let left_with = leave(&mut state, 1, now);
    assert_eq!(left_with, 980);

    assert_eq!(state.phase, GamePhase::HandComplete, "A is alone: the hand is over");
    assert_eq!(state.last_hand_went_to_showdown, Some(false), "a fold-out, not a showdown");
    assert!(state.action_timer.is_none(), "no clock between hands");
    assert_eq!(state.pot, 0, "the pot has been paid");
    assert_eq!(seat(&state, a).chips, 1_040, "A: 1,000 less 20 and 40 put in, plus the 100 pot");
    assert_eq!(seat(&state, b).chips, 980, "B folded and keeps the stack behind");
    assert_eq!(
        seat(&state, a).chips + seat(&state, b).chips + left_with,
        3_000,
        "every chip is accounted for"
    );
}

/// A bets 40 and the action is on B; C, who has NOT been asked yet, leaves.
/// Nothing about what the hand is waiting for has changed: B still owes 40 and
/// the clock stays exactly where it was.
///
/// Green before and after. It is here so the fix cannot be "always call
/// `advance_game`": that would move the clock off B, who has not acted.
#[test]
fn a_leaver_who_was_not_on_action_leaves_the_clock_where_it_was() {
    let now = 1_000 * SEC;
    let mut state = flop_three_handed(now);
    let a = act_in_turn(&mut state, now, PlayerAction::Bet(40));
    assert_eq!(state.action_on, 0, "sanity: the action is on B");
    let before = state.action_timer.clone().expect("B is on the clock");

    let left_with = leave(&mut state, 1, now);
    assert_eq!(left_with, 980);

    assert_eq!(state.phase, GamePhase::Flop);
    assert_eq!(state.action_on, 0, "B is still on action");
    let after = state.action_timer.as_ref().expect("B is still on the clock");
    assert_eq!((after.player_seat, after.started_at, after.expires_at), (before.player_seat, before.started_at, before.expires_at));
    assert_eq!(state.current_bet, 40, "A's bet stands");
    assert_eq!(seat(&state, a).current_bet, 40);
    assert_eq!(state.pot, 100);
    assert!(!is_betting_round_complete(&state), "B owes 40");

    // B calls: two seats can still act, so the turn is dealt and opened.
    act(&mut state, 0, now, PlayerAction::Call).expect("B may call");
    assert_eq!(state.phase, GamePhase::Turn);
    assert_eq!(state.pot, 140);
}

/// A table restored in a shape the engine no longer produces: the clock is on a
/// seat that has already matched the bet while another seat still owes it (the
/// E-106 kind of leftover -- a clock parked on a seat that should not be asked).
/// When the seat that still owed leaves, the street is closed and nothing is
/// waiting; the hand must move on rather than sit on a clock nobody should
/// answer.
///
/// Built by hand, because the reachable engine never parks the clock on a
/// matched seat. Before the fix a departure from anywhere but the clock's seat
/// only asked whether one player was left.
#[test]
fn a_clock_left_on_a_matched_seat_moves_on_when_the_last_seat_owing_action_leaves() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[1_000, 1_000, 1_000], now);
    state.pot = 60;
    for s in [0u8, 1] {
        let p = state.players[s as usize].as_mut().expect("seat occupied");
        p.chips -= 20 + 40;
        p.total_bet_this_hand = 60;
        p.current_bet = 40;
        p.has_acted_this_round = true;
    }
    {
        let p = state.players[2].as_mut().expect("seat occupied");
        p.chips -= 20;
        p.total_bet_this_hand = 20;
    }
    state.pot += 80;
    state.current_bet = 40;
    state.last_aggressor = Some(0);
    state.action_on = 0; // parked on a seat that has matched
    state.action_timer = Some(ActionTimer {
        player_seat: 0,
        started_at: now,
        expires_at: now + TIMEOUT_SECS * SEC,
        using_time_bank: false,
    });
    assert!(!is_betting_round_complete(&state), "sanity: seat 2 owes 40");

    let left_with = leave(&mut state, 2, now);
    assert_eq!(left_with, 980);

    assert_eq!(
        state.phase,
        GamePhase::Turn,
        "seats 0 and 1 have both matched: the flop is closed and the turn is dealt. \
         Instead the hand is at {:?} with the clock on {:?}",
        state.phase,
        state.action_timer.as_ref().map(|t| t.player_seat)
    );
    assert_eq!(state.pot, 140);
    assert!(
        !(is_betting_round_complete(&state) && state.action_timer.is_some()),
        "the clock {:?} is armed on a street the engine itself says is closed",
        state.action_timer
    );
}
