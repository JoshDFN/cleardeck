//! # ClearDeck NO-PEEKING SPIKE — the table that holds no cards
//!
//! This is the other half of the sealed-dealer construction in
//! `docs/NO-PEEKING-FEASIBILITY.md` §6 Option A. It is **not** a replacement for
//! `src/table_canister`, it is not deployed, and it deliberately has no ledger, no
//! deposits and no withdrawals: chips here are plain numbers. What it does have is
//! everything the auditor's finding is about — seats, blinds, a betting round,
//! all-ins, side pots, a showdown and a settlement — so that "the table can still
//! run a hand without ever holding a card" is a thing you can execute rather than
//! a thing you can be told.
//!
//! ## The negative this exists to make checkable
//!
//! Search this file for a deck. There is no `deck` field. There is no `seed`
//! field. There is no `hole_cards` field on [`Seat`]. `TableView` returns
//! everything this canister knows, to anybody, and until the showdown is published
//! that includes no card belonging to any player. The controller of this canister
//! has nothing to read, and — this is the part the feasibility study proved matters
//! more than the interface — a **canister snapshot** of it has nothing in it
//! either, because the 32 bytes that generate every card are in a different
//! canister that has no controller.
//!
//! ## What it costs
//!
//! One inter-canister call to open the hand, one per street to reveal, one at
//! showdown, one to publish the seed. `docs/NO-PEEKING-FEASIBILITY.md` §7.5 budgets
//! that at 260,000 cycles per hop against a deal that costs ~5.03 M cycles today,
//! i.e. about 1.05×, versus about 31,000× for the vetKD design that does not even
//! close the hole. `tests/no_peeking/tests/measurements.rs` measures the real
//! number on a replica rather than trusting that arithmetic.
//!
//! ## Settlement is unchanged, and that is the point
//!
//! Feasibility §8 left the showdown split as an open question, because
//! `evaluate_hand` and the side-pot code read hole cards directly. The answer this
//! file demonstrates is the boring one: they keep reading hole cards directly, the
//! cards just arrive as an **argument** from [`reveal_showdown`] instead of being
//! fetched out of the table's own state. `poker_core` is untouched, so the golden
//! vectors, the settlement oracle and the money-safety invariants all still bind to
//! the same code.

use candid::{CandidType, Deserialize, Principal};
use dealer_types::{DealtInSeat, ForcedFinalize, HandOpened, SeatCards};
use ic_cdk::call::Call;
use poker_core::{build_side_pots_from_contributions, Card, HandRank, SidePot};
use std::cell::RefCell;

mod settlement;
use settlement::{contributions, settle_foldout, settle_showdown};

pub(crate) const MAX_SEATS: usize = 10;

// ---------------------------------------------------------------------------
// wire types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Idle,
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
    Complete,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize, PartialEq, Eq)]
pub enum Action {
    Fold,
    Check,
    Call,
    /// Raise TO this total for the street (the same convention the live engine
    /// uses for `Raise`).
    RaiseTo(u64),
    AllIn,
}

/// A seat.
///
/// **Read the fields.** There is no `hole_cards` here and there is nowhere else in
/// this canister for one to hide. In `src/table_canister/src/lib.rs` the
/// equivalent struct carries `pub hole_cards: Option<(Card, Card)>`, and that
/// field, plus `TableState::deck`, plus `CURRENT_SEED`, are the three things the
/// auditor read.
#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct Seat {
    pub seat: u8,
    pub principal: Principal,
    pub chips: u64,
    pub current_bet: u64,
    pub total_bet_this_hand: u64,
    pub has_folded: bool,
    pub has_acted_this_round: bool,
    pub is_all_in: bool,
    pub in_hand: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Winner {
    pub seat: u8,
    pub principal: Principal,
    pub amount: u64,
    pub rank: Option<HandRank>,
}

/// Everything this canister knows, returned to anybody who asks.
///
/// The auditor's finding was against a method of exactly this shape. Run it
/// mid-hand and count the cards: `board` holds only what the dealer has already
/// made public to everyone, and `showdown` is empty until the showdown is public
/// too.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct TableView {
    pub dealer_canister: Principal,
    pub phase: Phase,
    pub hand_id: Option<u64>,
    pub seed_hash: Option<String>,
    pub revealed_seed: Option<String>,
    pub dealt_in: Vec<DealtInSeat>,
    pub seats: Vec<Seat>,
    pub board: Vec<Card>,
    pub showdown: Vec<SeatCards>,
    pub pot: u64,
    pub side_pots: Vec<SidePot>,
    pub current_bet: u64,
    pub min_raise: u64,
    pub action_on: Option<u8>,
    pub dealer_seat: u8,
    pub winners: Vec<Winner>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct StubConfig {
    pub small_blind: u64,
    pub big_blind: u64,
}

#[derive(Clone, Copy, Debug, CandidType, Deserialize)]
pub struct StubInit {
    pub dealer: Principal,
    pub config: StubConfig,
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// Note what is NOT in here: no `deck`, no `deck_index`, no `seed`.
pub(crate) struct Table {
    pub(crate) dealer: Principal,
    pub(crate) config: StubConfig,
    pub(crate) seats: Vec<Option<Seat>>,
    pub(crate) phase: Phase,
    pub(crate) hand_id: Option<u64>,
    pub(crate) seed_hash: Option<String>,
    pub(crate) revealed_seed: Option<String>,
    pub(crate) dealt_in: Vec<DealtInSeat>,
    pub(crate) board: Vec<Card>,
    pub(crate) showdown: Vec<SeatCards>,
    pub(crate) pot: u64,
    pub(crate) current_bet: u64,
    pub(crate) min_raise: u64,
    pub(crate) action_on: Option<u8>,
    pub(crate) dealer_seat: u8,
    pub(crate) winners: Vec<Winner>,
    pub(crate) last_error: Option<String>,
}

thread_local! {
    static TABLE: RefCell<Option<Table>> = const { RefCell::new(None) };
}

pub(crate) fn with_table<R>(f: impl FnOnce(&mut Table) -> R) -> Result<R, String> {
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        let table = t.as_mut().ok_or("table not initialised")?;
        Ok(f(table))
    })
}

#[ic_cdk::init]
fn init(args: StubInit) {
    TABLE.with(|t| {
        *t.borrow_mut() = Some(Table {
            dealer: args.dealer,
            config: args.config,
            seats: vec![None; MAX_SEATS],
            phase: Phase::Idle,
            hand_id: None,
            seed_hash: None,
            revealed_seed: None,
            dealt_in: Vec::new(),
            board: Vec::new(),
            showdown: Vec::new(),
            pot: 0,
            current_bet: 0,
            min_raise: 0,
            action_on: None,
            dealer_seat: 0,
            winners: Vec::new(),
            last_error: None,
        });
    });
}

// ---------------------------------------------------------------------------
// seating
// ---------------------------------------------------------------------------

/// Spike shortcut: chips are minted on request. There is no ledger here on
/// purpose — this canister must never be mistaken for something that could hold
/// money, and the money path is explicitly out of scope for this wave.
#[ic_cdk::update]
fn sit(seat: u8, chips: u64) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("anonymous cannot sit".to_string());
    }
    with_table(|t| {
        if t.phase != Phase::Idle && t.phase != Phase::Complete {
            return Err("a hand is in progress".to_string());
        }
        let idx = seat as usize;
        if idx >= MAX_SEATS {
            return Err("no such seat".to_string());
        }
        if t.seats[idx].is_some() {
            return Err("seat taken".to_string());
        }
        if t.seats.iter().flatten().any(|s| s.principal == caller) {
            return Err("already seated".to_string());
        }
        t.seats[idx] = Some(Seat {
            seat,
            principal: caller,
            chips,
            current_bet: 0,
            total_bet_this_hand: 0,
            has_folded: false,
            has_acted_this_round: false,
            is_all_in: false,
            in_hand: false,
        });
        Ok(())
    })?
}

// ---------------------------------------------------------------------------
// the deal
// ---------------------------------------------------------------------------

/// Ask the sealed dealer for a hand.
///
/// The whole exchange with the dealer is: "here are the seats". The reply is a
/// commitment and a seat order. No card crosses this boundary, so there is nothing
/// for this canister to store even if it wanted to, and nothing for its controller
/// to find.
#[ic_cdk::update]
async fn start_hand() -> Result<u64, String> {
    let (dealer, seats) = TABLE.with(|t| {
        let t = t.borrow();
        let table = t.as_ref().ok_or("table not initialised")?;
        if table.phase != Phase::Idle && table.phase != Phase::Complete {
            return Err("a hand is in progress".to_string());
        }
        let seats: Vec<DealtInSeat> = table
            .seats
            .iter()
            .flatten()
            .filter(|s| s.chips > 0)
            .map(|s| DealtInSeat {
                seat: s.seat,
                principal: s.principal,
            })
            .collect();
        if seats.len() < 2 {
            return Err("need at least two seats with chips".to_string());
        }
        Ok((table.dealer, seats))
    })?;

    let opened: HandOpened = Call::unbounded_wait(dealer, "open_hand")
        .with_arg(&seats)
        .await
        .map_err(|e| format!("dealer unreachable: {e:?}"))?
        .candid::<Result<HandOpened, String>>()
        .map_err(|e| format!("dealer reply undecodable: {e:?}"))??;

    with_table(|t| {
        t.phase = Phase::PreFlop;
        t.hand_id = Some(opened.hand_id);
        t.seed_hash = Some(opened.seed_hash.clone());
        t.revealed_seed = None;
        t.dealt_in = opened.dealt_in.clone();
        t.board.clear();
        t.showdown.clear();
        t.winners.clear();
        t.last_error = None;
        t.pot = 0;

        let in_hand: Vec<u8> = opened.dealt_in.iter().map(|d| d.seat).collect();
        for seat in t.seats.iter_mut().flatten() {
            seat.current_bet = 0;
            seat.total_bet_this_hand = 0;
            seat.has_folded = false;
            seat.has_acted_this_round = false;
            seat.is_all_in = false;
            seat.in_hand = in_hand.contains(&seat.seat);
        }

        // Button, blinds, action. Deliberately the same shape as the live engine
        // so the settlement inputs are the ones `poker_core` expects.
        t.dealer_seat = in_hand[0];
        let (sb, bb) = if in_hand.len() == 2 {
            (in_hand[0], in_hand[1])
        } else {
            (in_hand[1], in_hand[2 % in_hand.len()])
        };
        let small = t.config.small_blind;
        let big = t.config.big_blind;
        post(t, sb, small);
        post(t, bb, big);
        t.current_bet = big;
        t.min_raise = big;
        t.action_on = Some(next_to_act_from(t, bb));
        Ok::<u64, String>(opened.hand_id)
    })?
}

fn post(t: &mut Table, seat: u8, amount: u64) {
    if let Some(Some(p)) = t.seats.get_mut(seat as usize) {
        let paid = amount.min(p.chips);
        p.chips = p.chips.saturating_sub(paid);
        p.current_bet = p.current_bet.saturating_add(paid);
        p.total_bet_this_hand = p.total_bet_this_hand.saturating_add(paid);
        if p.chips == 0 {
            p.is_all_in = true;
        }
        t.pot = t.pot.saturating_add(paid);
    }
}

fn live_seats(t: &Table) -> Vec<u8> {
    t.seats
        .iter()
        .flatten()
        .filter(|s| s.in_hand && !s.has_folded)
        .map(|s| s.seat)
        .collect()
}

fn actionable(t: &Table) -> Vec<u8> {
    t.seats
        .iter()
        .flatten()
        .filter(|s| s.in_hand && !s.has_folded && !s.is_all_in)
        .map(|s| s.seat)
        .collect()
}

fn next_to_act_from(t: &Table, from: u8) -> u8 {
    let act = actionable(t);
    if act.is_empty() {
        return from;
    }
    for step in 1..=MAX_SEATS {
        let candidate = ((from as usize + step) % MAX_SEATS) as u8;
        if act.contains(&candidate) {
            return candidate;
        }
    }
    from
}

pub(crate) fn seat_mut<'a>(t: &'a mut Table, seat: u8) -> Option<&'a mut Seat> {
    t.seats.get_mut(seat as usize)?.as_mut()
}

fn seat_of(t: &Table, who: Principal) -> Option<u8> {
    t.seats
        .iter()
        .flatten()
        .find(|s| s.principal == who)
        .map(|s| s.seat)
}

// ---------------------------------------------------------------------------
// betting
// ---------------------------------------------------------------------------

#[ic_cdk::update]
fn act(action: Action) -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    with_table(|t| {
        let seat = seat_of(t, caller).ok_or("not seated")?;
        if t.action_on != Some(seat) {
            return Err(format!("it is seat {:?}'s turn", t.action_on));
        }
        let target = t.current_bet;
        let min_raise = t.min_raise;
        let p = seat_mut(t, seat).ok_or("no seat")?;
        if p.has_folded || p.is_all_in || !p.in_hand {
            return Err("you have no action".to_string());
        }
        let owed = target.saturating_sub(p.current_bet);

        match action {
            Action::Fold => {
                p.has_folded = true;
            }
            Action::Check => {
                if owed > 0 {
                    return Err(format!("cannot check, {owed} to call"));
                }
            }
            Action::Call => {
                let pay = owed.min(p.chips);
                p.chips -= pay;
                p.current_bet += pay;
                p.total_bet_this_hand += pay;
                if p.chips == 0 {
                    p.is_all_in = true;
                }
                t.pot += pay;
            }
            Action::RaiseTo(to) => {
                if to < target.saturating_add(min_raise) {
                    return Err(format!(
                        "raise must be to at least {}",
                        target.saturating_add(min_raise)
                    ));
                }
                let pay = to.saturating_sub(p.current_bet);
                if pay > p.chips {
                    return Err("not enough chips".to_string());
                }
                p.chips -= pay;
                p.current_bet += pay;
                p.total_bet_this_hand += pay;
                if p.chips == 0 {
                    p.is_all_in = true;
                }
                t.pot += pay;
                t.min_raise = to.saturating_sub(target).max(min_raise);
                t.current_bet = to;
                // A genuine raise re-opens the action for everybody else.
                for other in t.seats.iter_mut().flatten() {
                    if other.seat != seat && !other.has_folded && !other.is_all_in {
                        other.has_acted_this_round = false;
                    }
                }
            }
            Action::AllIn => {
                let pay = p.chips;
                p.chips = 0;
                p.current_bet += pay;
                p.total_bet_this_hand += pay;
                p.is_all_in = true;
                t.pot += pay;
                let to = seat_mut(t, seat).map(|s| s.current_bet).unwrap_or(0);
                if to > t.current_bet {
                    t.min_raise = to.saturating_sub(t.current_bet).max(t.min_raise);
                    t.current_bet = to;
                    for other in t.seats.iter_mut().flatten() {
                        if other.seat != seat && !other.has_folded && !other.is_all_in {
                            other.has_acted_this_round = false;
                        }
                    }
                }
            }
        }

        if let Some(p) = seat_mut(t, seat) {
            p.has_acted_this_round = true;
        }
        t.action_on = Some(next_to_act_from(t, seat));
        Ok(())
    })?
}

/// Has the current betting round closed?
fn round_closed(t: &Table) -> bool {
    let live = live_seats(t);
    if live.len() <= 1 {
        return true;
    }
    t.seats
        .iter()
        .flatten()
        .filter(|s| s.in_hand && !s.has_folded && !s.is_all_in)
        .all(|s| s.has_acted_this_round && s.current_bet == t.current_bet)
}

/// Whether this table currently believes the hand can move on. Public so a test —
/// and a client — can see the same thing the table sees.
#[ic_cdk::query]
fn betting_round_closed() -> bool {
    TABLE.with(|t| t.borrow().as_ref().map(round_closed).unwrap_or(false))
}

// ---------------------------------------------------------------------------
// advancing, which is where the dealer is asked for cards
// ---------------------------------------------------------------------------

/// Move the hand on if the betting round has closed.
///
/// Permissionless on purpose. Nothing here is privileged, because nothing here can
/// see a card that the dealer has not already published to everybody; and a
/// permissionless advance is one fewer thing that stalls when the operator's
/// process is not running (FINDING 19).
#[ic_cdk::update]
async fn try_advance() -> Result<Phase, String> {
    let (dealer, hand_id, phase, closed, live) = TABLE.with(|t| {
        let t = t.borrow();
        let table = t.as_ref().ok_or("table not initialised")?;
        Ok::<_, String>((
            table.dealer,
            table.hand_id.ok_or("no hand")?,
            table.phase,
            round_closed(table),
            live_seats(table),
        ))
    })?;

    if phase == Phase::Idle || phase == Phase::Complete {
        return Err("no hand in progress".to_string());
    }
    if !closed {
        return Err("betting round is still open".to_string());
    }

    // Fold-out: one player left, no showdown, no cards revealed at all. Exactly
    // what the live engine does, and the reason `finish_hand` on the dealer has a
    // separate gate for it.
    if live.len() <= 1 {
        settle_foldout(live.first().copied())?;
        finish(dealer, hand_id).await?;
        return with_table(|t| t.phase);
    }

    if phase == Phase::River {
        // Showdown. THIS is the only moment a hole card enters this canister, and
        // by then the dealer has made it public to everybody in the same message.
        let seats: Vec<u8> = live.clone();
        let cards: Vec<SeatCards> = Call::unbounded_wait(dealer, "reveal_showdown")
            .with_args(&(hand_id, seats))
            .await
            .map_err(|e| format!("dealer unreachable: {e:?}"))?
            .candid::<Result<Vec<SeatCards>, String>>()
            .map_err(|e| format!("dealer reply undecodable: {e:?}"))??;
        settle_showdown(cards)?;
        finish(dealer, hand_id).await?;
        return with_table(|t| t.phase);
    }

    let board: Vec<Card> = Call::unbounded_wait(dealer, "advance_street")
        .with_arg(&hand_id)
        .await
        .map_err(|e| format!("dealer unreachable: {e:?}"))?
        .candid::<Result<Vec<Card>, String>>()
        .map_err(|e| format!("dealer reply undecodable: {e:?}"))??;

    with_table(|t| {
        t.board = board;
        t.phase = match t.phase {
            Phase::PreFlop => Phase::Flop,
            Phase::Flop => Phase::Turn,
            Phase::Turn => Phase::River,
            other => other,
        };
        t.current_bet = 0;
        t.min_raise = t.config.big_blind;
        for s in t.seats.iter_mut().flatten() {
            s.current_bet = 0;
            s.has_acted_this_round = false;
        }
        let button = t.dealer_seat;
        t.action_on = Some(next_to_act_from(t, button));
        t.phase
    })
}

/// Ask the dealer to time a silent seat out, so a disconnect cannot stall the
/// street.
///
/// The dealer refuses unless that seat has been silent for a full action clock BY
/// ITS OWN CLOCK, so this is not a "the table says so" door. Permissionless,
/// because any player has the same interest in the hand moving.
#[ic_cdk::update]
async fn nudge_timeout(seat: u8) -> Result<String, String> {
    let (dealer, hand_id) = TABLE.with(|t| {
        let t = t.borrow();
        let table = t.as_ref().ok_or("table not initialised")?;
        Ok::<_, String>((table.dealer, table.hand_id.ok_or("no hand")?))
    })?;
    let res = Call::unbounded_wait(dealer, "table_stand_down")
        .with_args(&(hand_id, seat))
        .await
        .map_err(|e| format!("dealer unreachable: {e:?}"))?
        .candid::<Result<dealer_types::AckState, String>>()
        .map_err(|e| format!("dealer reply undecodable: {e:?}"))?;
    match res {
        Ok(state) => Ok(format!("{:?}", state.waiting_on)),
        Err(e) => Err(e),
    }
}

/// A player who folded or went all-in tells the dealer to stop waiting for them.
///
/// Cannot be done on their behalf: the dealer only accepts `stand_down` from the
/// seat's own principal, which is what makes an early reveal impossible rather than
/// merely visible. The table's route is [`nudge_timeout`], and that one costs a
/// full action clock of measured silence.
#[ic_cdk::query]
fn how_a_player_stands_down() -> String {
    "call stand_down(hand_id) on the dealer canister with your own identity".to_string()
}

async fn finish(dealer: Principal, hand_id: u64) -> Result<(), String> {
    let seed = Call::unbounded_wait(dealer, "finish_hand")
        .with_arg(&hand_id)
        .await
        .map_err(|e| format!("dealer unreachable: {e:?}"))?
        .candid::<Result<String, String>>()
        .map_err(|e| format!("dealer reply undecodable: {e:?}"))?;
    with_table(|t| match seed {
        Ok(s) => {
            t.revealed_seed = Some(s);
            t.phase = Phase::Complete;
        }
        Err(e) => {
            // The hand is settled either way; the seed reveal is a separate
            // promise and its failure is recorded rather than swallowed.
            t.last_error = Some(format!("seed not published: {e}"));
            t.phase = Phase::Complete;
        }
    })
}

// ---------------------------------------------------------------------------
// THE LAST-RESORT SETTLEMENT
// ---------------------------------------------------------------------------

/// Settle a hand nobody can otherwise close, from the dealer's last-resort reveal.
///
/// Permissionless, and this is the door that keeps the two failures this project
/// has already shipped out of the design: the pot is NOT stuck forever, and
/// everybody is NOT refunded so that the winner is robbed. The dealer publishes the
/// seed, the board and every hole card once the hand is older than its
/// `force_finalize_after_ns`, and the real winner is paid from the real cards.
#[ic_cdk::update]
async fn rescue_hand() -> Result<Vec<Winner>, String> {
    let (dealer, hand_id) = TABLE.with(|t| {
        let t = t.borrow();
        let table = t.as_ref().ok_or("table not initialised")?;
        Ok::<_, String>((table.dealer, table.hand_id.ok_or("no hand")?))
    })?;

    let forced: ForcedFinalize = Call::unbounded_wait(dealer, "force_finalize")
        .with_arg(&hand_id)
        .await
        .map_err(|e| format!("dealer unreachable: {e:?}"))?
        .candid::<Result<ForcedFinalize, String>>()
        .map_err(|e| format!("dealer reply undecodable: {e:?}"))??;

    with_table(|t| {
        t.board = forced.community.clone();
        t.revealed_seed = Some(forced.revealed_seed.clone());
    })?;

    // Only seats that had not folded contest the pot. Their cards are in the
    // forced reveal, and so is everybody else's, which is what makes this
    // settleable with nobody's co-operation.
    let contesting: Vec<u8> = with_table(|t| live_seats(t))?;
    let cards: Vec<SeatCards> = forced
        .hole_cards
        .into_iter()
        .filter(|sc| contesting.contains(&sc.seat))
        .collect();

    if cards.len() >= 2 {
        settle_showdown(cards)?;
    } else {
        settle_foldout(cards.first().map(|c| c.seat))?;
    }
    with_table(|t| {
        t.phase = Phase::Complete;
        t.winners.clone()
    })
}

// ---------------------------------------------------------------------------
// read surface — the thing the auditor's attack is pointed at
// ---------------------------------------------------------------------------

/// The analogue of `get_table_state`. It returns EVERYTHING this canister knows.
///
/// That is the claim under test: run it as the controller, mid-hand, at
/// `phase = PreFlop`, and there is no deck in it and no hole card in it, because
/// there is no deck in this canister and no hole card in this canister.
#[ic_cdk::query]
fn get_table_state() -> Option<TableView> {
    TABLE.with(|t| {
        let t = t.borrow();
        let table = t.as_ref()?;
        Some(TableView {
            dealer_canister: table.dealer,
            phase: table.phase,
            hand_id: table.hand_id,
            seed_hash: table.seed_hash.clone(),
            revealed_seed: table.revealed_seed.clone(),
            dealt_in: table.dealt_in.clone(),
            seats: table.seats.iter().flatten().copied().collect(),
            board: table.board.clone(),
            showdown: table.showdown.clone(),
            pot: table.pot,
            side_pots: build_side_pots_from_contributions(&contributions(table)),
            current_bet: table.current_bet,
            min_raise: table.min_raise,
            action_on: table.action_on,
            dealer_seat: table.dealer_seat,
            winners: table.winners.clone(),
            last_error: table.last_error.clone(),
        })
    })
}

#[ic_cdk::query]
fn chips_of(who: Principal) -> u64 {
    TABLE.with(|t| {
        t.borrow()
            .as_ref()
            .and_then(|table| {
                table
                    .seats
                    .iter()
                    .flatten()
                    .find(|s| s.principal == who)
                    .map(|s| s.chips)
            })
            .unwrap_or(0)
    })
}

#[ic_cdk::query]
fn total_chips() -> u64 {
    TABLE.with(|t| {
        t.borrow()
            .as_ref()
            .map(|table| {
                table
                    .seats
                    .iter()
                    .flatten()
                    .fold(0u64, |a, s| a.saturating_add(s.chips))
                    .saturating_add(if table.phase == Phase::Complete {
                        0
                    } else {
                        table.pot
                    })
            })
            .unwrap_or(0)
    })
}

ic_cdk::export_candid!();
