//! Regressions landed by the wave-2 COHERENCE PASS.
//!
//! Every test here pins a correction made to `apply_player_action` after the
//! betting work had been reviewed and rejected. They exist because the two
//! corrections are one-liners: without a test naming the rule, the next reader
//! sees a guard that looks arbitrary and "simplifies" it straight back into the
//! defect. See docs/DEFECTS.md "E-30 -- correction" and "E-31 -- correction".
//!
//! Originally written as adversarial probes by the betting reviewer; the
//! characterisation of the out-of-turn defect has been inverted into an assertion
//! that it is fixed.
//!
//! Rule sources used here, quoted verbatim:
//!
//! TDA 2022/2024 Rule 47-A (Re-Opening the Bet), No-Limit & Pot-Limit:
//!   "An all-in wager (or CUMULATIVE MULTIPLE SHORT ALL-INS) totaling less than
//!    a full bet or raise will not reopen betting for players who have already
//!    acted and are not facing at least a full bet or raise when the action
//!    returns to them."
//!   Illustration: A opens 100, B all-in 125, C calls 125, D all-in 200, E calls
//!   200. When action returns to A, A faces a cumulative 100 (B's 25 + D's 75),
//!   which IS a full raise, so the betting IS reopened for A.
//!
//! Robert's Rules of Poker, No-Limit and Pot-Limit Betting:
//!   "Multiple all-in wagers, each of an amount too small to qualify as a raise,
//!    still act as a raise and reopen the betting if the resulting wager size to
//!    a player qualifies as a raise."
//!
//! Both rulesets agree: the test is the amount the closed player FACES versus
//! the last full bet/raise -- not "is there anything at all in front of them".

use candid::Principal;
use table_canister::{
    apply_player_action, is_betting_round_complete, resolve_expired_action_timer, ActionTimer,
    Currency, GamePhase, Player, PlayerAction, PlayerStatus, TableConfig, TableState,
};

const SMALL_BLIND: u64 = 10;
const BIG_BLIND: u64 = 20;
const TIMEOUT_SECS: u64 = 30;
const SEC: u64 = 1_000_000_000;

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

fn act(state: &mut TableState, seat: u8, now: u64, action: PlayerAction) -> Result<(), String> {
    apply_player_action(state, seat_principal(seat), now, action)
}

fn seat(state: &TableState, s: u8) -> &Player {
    state.players[s as usize].as_ref().expect("seat occupied")
}

// =========================================================================
// 1. THE RULE THE BUILDER DID NOT IMPLEMENT: CUMULATIVE SHORT ALL-INS
// =========================================================================

/// TDA 47-A illustration, transcribed to this engine's numbers.
///
/// FLOP, four handed. min_raise starts at one big blind (20).
///   seat 0 (5000) bets 100          -> the last FULL bet of the round is 100
///   seat 1 (exactly 150) shoves     -> raise of 50. INCOMPLETE (50 < 100)
///   seat 2 (exactly 200) shoves     -> raise of 50 over 150. INCOMPLETE alone
///   seat 3 (5000) calls 200
/// Action returns to seat 0, who put in 100 and now must call 100 more.
///
/// 100 owed >= the 100 last-full-bet, so per TDA 47-A and per Robert's Rules the
/// two short all-ins COMBINE into a full raise and the betting IS REOPENED for
/// seat 0: seat 0 may raise (to at least 200 + 100 = 300).
///
/// This test asserts THE RULE. If it fails, the engine deviates from the rule.
#[test]
fn tda_47a_cumulative_short_all_ins_must_reopen_the_betting_to_seat_0() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 200, 5_000], now);

    act(&mut state, 0, now, PlayerAction::Bet(100)).expect("seat 0 bets 100");
    assert_eq!(state.min_raise, 100, "the last full bet of the round is 100");

    act(&mut state, 1, now, PlayerAction::AllIn).expect("seat 1 shoves 150");
    assert_eq!(state.current_bet, 150);
    assert_eq!(state.min_raise, 100, "50 was not a full raise");

    assert_eq!(state.action_on, 2);
    act(&mut state, 2, now, PlayerAction::AllIn).expect("seat 2 shoves 200");
    assert_eq!(state.current_bet, 200);
    assert_eq!(state.min_raise, 100, "50 more was not a full raise either");

    assert_eq!(state.action_on, 3);
    act(&mut state, 3, now, PlayerAction::Call).expect("seat 3 calls 200");

    assert_eq!(state.action_on, 0, "action returns to seat 0");
    let owed = state.current_bet - seat(&state, 0).current_bet;
    assert_eq!(owed, 100, "seat 0 faces 100, which equals the last full bet");

    // THE RULE: seat 0 is facing at least a full bet, so the betting is reopened.
    let outcome = act(&mut state, 0, now, PlayerAction::Raise(300));
    assert!(
        outcome.is_ok(),
        "TDA 47-A / Robert's Rules: the cumulative short all-ins total a full \
         raise (100 >= 100), so seat 0 MAY raise. Engine said: {outcome:?}"
    );
}

/// The same shape, one chip short, where the correct answer IS "closed".
/// seat 2 shoves 199 -> seat 0 faces 99 < 100, so seat 0 is genuinely closed.
/// This is the case the builder's `two_successive_incomplete_all_ins...` test
/// happens to sit in, which is why that test passes for the wrong reason.
#[test]
fn one_chip_below_a_full_raise_the_cumulative_short_all_ins_do_not_reopen() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 150, 199, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 2, now, PlayerAction::AllIn).unwrap();
    act(&mut state, 3, now, PlayerAction::Call).unwrap();
    assert_eq!(state.action_on, 0);
    let owed = state.current_bet - seat(&state, 0).current_bet;
    assert_eq!(owed, 99);
    act(&mut state, 0, now, PlayerAction::Raise(299))
        .expect_err("99 < 100, so seat 0 is correctly closed");
}

/// TASK ITEM 3, literally: three players, one has already CALLED, then an
/// incomplete all-in raise. The already-acted CALLER must not be able to
/// re-raise. (Four seats so the caller is not the bettor.)
#[test]
fn a_player_who_already_called_cannot_reraise_an_incomplete_all_in() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 150, 5_000], now);

    act(&mut state, 0, now, PlayerAction::Bet(100)).expect("seat 0 bets 100");
    act(&mut state, 1, now, PlayerAction::Call).expect("seat 1 CALLS 100");
    assert!(seat(&state, 1).has_acted_this_round);
    assert_eq!(seat(&state, 1).current_bet, 100);

    act(&mut state, 2, now, PlayerAction::AllIn).expect("seat 2 shoves 150");
    assert_eq!(state.current_bet, 150);

    // seat 3 has not acted: full option, may raise to >= 250.
    assert_eq!(state.action_on, 3);
    let mut probe = state.clone();
    act(&mut probe, 3, now, PlayerAction::Raise(250)).expect("seat 3 keeps a full option");

    act(&mut state, 3, now, PlayerAction::Call).expect("seat 3 calls 150");

    // Back to seat 0 (bettor) then seat 1 (caller). Both face 50 < 100 -> closed.
    assert_eq!(state.action_on, 0);
    act(&mut state, 0, now, PlayerAction::Call).expect("seat 0 may call the extra 50");

    assert_eq!(state.action_on, 1, "now the already-CALLED seat");
    let owed = state.current_bet - seat(&state, 1).current_bet;
    assert_eq!(owed, 50);
    let refused = act(&mut state, 1, now, PlayerAction::Raise(400))
        .expect_err("a player who already called may not re-raise a short all-in");
    assert!(refused.contains("not reopened"), "got: {refused}");
    // and may not launder it through AllIn either
    act(&mut state, 1, now, PlayerAction::AllIn)
        .expect_err("nor through AllIn, which would be a raise");
    // but MAY call
    act(&mut state, 1, now, PlayerAction::Call).expect("call is legal");
    assert!(is_betting_round_complete(&state) || state.phase != GamePhase::Flop);
}

// =========================================================================
// 2. THE TIMER
// =========================================================================

/// An expired clock must resolve, not wedge. Three handed, flop.
#[test]
fn an_expired_clock_advances_the_hand_and_does_not_wedge() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::Call).unwrap();
    assert_eq!(state.action_on, 2);

    let late = now + 31 * SEC;
    // Nobody called check_timeouts. Seat 0 -- NOT the seat on the clock --
    // sends a message.
    let before_phase = state.phase.clone();
    let res = act(&mut state, 0, late, PlayerAction::Check);
    println!("PROBE out-of-turn message during an expired clock -> {res:?}");
    println!("PROBE phase {before_phase:?} -> {:?}", state.phase);
    println!("PROBE seat 2 folded={} status={:?}", seat(&state, 2).has_folded, seat(&state, 2).status);
    println!("PROBE action_on={} timer={:?}", state.action_on, state.action_timer);

    assert!(seat(&state, 2).has_folded, "the expired seat is folded");
    assert_eq!(state.phase, GamePhase::Turn, "the hand moved to the turn");
    assert!(
        state.action_timer.as_ref().unwrap().expires_at > late,
        "a fresh clock was started, so the same expiry cannot resolve twice"
    );
}

/// E-31 correction. An out-of-turn message sent on the FLOP must NOT be applied
/// on the TURN.
///
/// Resolving an expired timer before the whose-turn check is what unwedges a
/// stuck table, and it must keep doing that -- but the message that triggered the
/// resolution was composed against the OLD board. Applying it commits chips to a
/// street whose card the player has not seen. Measured before this fix: seat 0
/// pre-fires `Bet(500)` out of turn on the flop and it lands on the turn with
/// `Ok(())`, `current_bet` 500, pot 700.
///
/// The timeout itself must still have been applied, or the table is still wedged.
#[test]
fn an_out_of_turn_message_is_not_applied_to_a_street_the_player_has_not_seen() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::Call).unwrap();
    assert_eq!(state.action_on, 2, "seat 2 is on the clock, on the FLOP");

    let late = now + 31 * SEC;
    // Seat 0 fires Bet(500) while it is seat 2's turn on the flop.
    let res = act(&mut state, 0, late, PlayerAction::Bet(500));
    println!("PROBE pre-fired Bet(500) out of turn on the flop -> {res:?}");
    println!("PROBE phase now {:?} current_bet={} pot={}", state.phase, state.current_bet, state.pot);
    assert!(
        res.is_err(),
        "an action composed against the flop was applied to the turn: {res:?}"
    );
    // The unwedging half must still have happened: the stale timeout was resolved
    // and the hand moved on, so the table is not stuck on a seat that cannot act.
    assert_eq!(
        state.phase,
        GamePhase::Turn,
        "the stale timeout must still have been resolved -- refusing the action must \
         not also refuse to unwedge the table"
    );
    assert_eq!(
        state.current_bet, 0,
        "no chips from the flop-era message may land on the turn"
    );
    assert_eq!(state.pot, 200, "only the flop bet and its call are in the pot");
}

/// Can anyone force the fold early by supplying their own time? `now` is the
/// canister's clock, so no. But check strict-versus-loose at the boundary.
#[test]
fn the_expiry_boundary_is_strict() {
    let now = 1_000 * SEC;
    let mut base = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut base, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut base, 1, now, PlayerAction::Call).unwrap();
    let expires = base.action_timer.as_ref().unwrap().expires_at;

    let mut at = base.clone();
    assert_eq!(resolve_expired_action_timer(&mut at, expires), None, "not expired AT expires_at");
    let mut after = base.clone();
    assert_eq!(resolve_expired_action_timer(&mut after, expires + 1), Some(2));
}

/// A folded player, and a player who is not on the clock, can both trigger the
/// resolution. Who is allowed to?
#[test]
fn characterises_who_can_trigger_the_timeout_resolution() {
    let now = 1_000 * SEC;
    let mut state = flop_table(&[5_000, 5_000, 5_000, 5_000], now);
    act(&mut state, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut state, 1, now, PlayerAction::Fold).unwrap();
    assert_eq!(state.action_on, 2);

    let late = now + 31 * SEC;
    // Seat 1 has FOLDED. Their message still resolves seat 2's clock.
    let res = act(&mut state, 1, late, PlayerAction::Check);
    println!("PROBE folded seat 1's message -> {res:?}");
    println!("PROBE seat 2 folded={} action_on={}", seat(&state, 2).has_folded, state.action_on);
    assert!(seat(&state, 2).has_folded, "a FOLDED player's message folded seat 2");

    // A principal that is not at the table at all.
    let stranger = Principal::from_slice(&[9, 9, 9, 9]);
    let mut s2 = flop_table(&[5_000, 5_000, 5_000], now);
    act(&mut s2, 0, now, PlayerAction::Bet(100)).unwrap();
    act(&mut s2, 1, now, PlayerAction::Call).unwrap();
    let r = apply_player_action(&mut s2, stranger, now + 31 * SEC, PlayerAction::Check);
    println!("PROBE stranger's message -> {r:?}");
    assert!(!seat(&s2, 2).has_folded, "a stranger cannot reach the resolver");
}
