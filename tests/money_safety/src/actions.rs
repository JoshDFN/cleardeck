//! The hostile action alphabet, and how each action is executed against the
//! world.
//!
//! Every public update method of the table canister that can move money is here,
//! plus time jumps, upgrades, and same-round concurrent submissions. Deliberately
//! illegal inputs are first-class members of the alphabet, not an afterthought:
//! raises below the min-raise, raises above the stack, acting out of turn, taking
//! an occupied seat, `u64::MAX` amounts, buy-ins outside `[min, max]`,
//! withdrawing more than escrow, re-entrant withdrawals and double-claimed
//! deposit blocks.

use candid::{encode_one, Principal};
use serde::Serialize;
use std::time::Duration;

use crate::table_api::PlayerAction;
use crate::world::{OpError, World};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Act {
    Fold,
    Check,
    Call,
    Bet(u64),
    Raise(u64),
    AllIn,
}

impl Act {
    pub fn to_candid(&self) -> PlayerAction {
        match self {
            Act::Fold => PlayerAction::Fold,
            Act::Check => PlayerAction::Check,
            Act::Call => PlayerAction::Call,
            Act::Bet(a) => PlayerAction::Bet(*a),
            Act::Raise(a) => PlayerAction::Raise(*a),
            Act::AllIn => PlayerAction::AllIn,
        }
    }
}

/// One step of a hostile sequence. `actor` is an index into `World::actors`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Op {
    /// `icrc2_approve` + `deposit`: the production ICRC-2 escrow top-up.
    FundEscrow { actor: usize, amount: u64 },
    /// `deposit` with NO approval, or with an approval that is too small.
    DepositWithoutAllowance { actor: usize, amount: u64 },
    /// Raw `icrc1_transfer` into the canister's main account, then
    /// `notify_deposit` for that block: the legacy block-verification flow.
    RawTransferThenNotify { actor: usize, amount: u64 },
    /// `notify_deposit` for a block index the actor did not produce, or for one
    /// that has already been credited.
    NotifyBlock { actor: usize, block: u64 },
    /// Transfer into the actor's per-player deposit subaccount, then
    /// `claim_external_deposit`.
    ExternalDepositThenClaim { actor: usize, amount: u64 },
    /// `claim_external_deposit` with nothing (or something) waiting.
    ClaimExternal { actor: usize },
    Withdraw { actor: usize, amount: u64 },
    /// Two `withdraw` calls submitted in the SAME round, before either executes.
    ReentrantWithdraw { actor: usize, amount: u64 },
    JoinTable { actor: usize, seat: u8 },
    BuyIn { actor: usize, seat: u8, amount: u64 },
    Reload { actor: usize, amount: u64 },
    CashOut { actor: usize },
    LeaveTable { actor: usize },
    /// Act as whoever is currently on the clock. This is what makes hands
    /// actually progress; without it almost every action is "Not your turn".
    ActInTurn { act: Act },
    /// Act as whoever is on the clock, with whatever is currently LEGAL
    /// (check, else call, else fold). The op is still a fixed symbol -- only its
    /// execution reads the state -- so a sequence stays replayable and shrinkable.
    /// Without this the generator spends its whole budget on rejected actions and
    /// almost no hand ever reaches a showdown.
    ActLegalInTurn,
    /// Top the actor's escrow up and take the lowest free seat. A recovery op:
    /// without it, one `SitOut` early in a heads-up run can wedge the table for
    /// the rest of the sequence.
    FundAndSeat { actor: usize },
    /// Act as a specific actor, in turn or not.
    ActAs { actor: usize, act: Act },
    /// The same in-turn action submitted by two actors in one round.
    ConcurrentActInTurn { act: Act },
    SitOut { actor: usize },
    SitIn { actor: usize },
    SitOutNextHand { actor: usize },
    UseTimeBank { actor: usize },
    ShowCards { actor: usize },
    Heartbeat { actor: usize },
    StartNewHand { actor: usize },
    CheckTimeouts { actor: usize },
    /// Jump the clock. Used to cross `action_timeout`, the auto-deal delay, the
    /// reload timer, the sitting-out kick and the withdrawal cooldown.
    AdvanceTime { nanos: u64 },
    /// A real `install_code --mode upgrade` with the same wasm.
    Upgrade,
    /// An actor stops responding entirely for the rest of the run. The generator
    /// emits no further ops for it; only timeouts can move it.
    GoSilent { actor: usize },
}

/// What executing an `Op` produced. Kept for the report so a failing sequence
/// reads like a transcript.
#[derive(Clone, Debug, Serialize)]
pub struct StepResult {
    pub op: Op,
    pub outcome: String,
}

fn describe<T: std::fmt::Debug>(r: &Result<T, OpError>) -> String {
    match r {
        Ok(v) => format!("Ok({v:?})"),
        Err(OpError::Err(m)) => format!("Err({m})"),
        Err(OpError::Trap(m)) => format!("TRAP({m})"),
    }
}

/// Execute one op. Never panics on a canister-level failure: rejecting hostile
/// input is the expected behaviour and the invariants are what judge the result.
pub fn apply(world: &mut World, op: &Op) -> StepResult {
    let outcome = match op {
        Op::FundEscrow { actor, amount } => {
            let who = actor_principal(world, *actor);
            describe(&world.fund_escrow(who, *amount))
        }
        Op::DepositWithoutAllowance { actor, amount } => {
            let who = actor_principal(world, *actor);
            describe(&world.deposit(who, *amount))
        }
        Op::RawTransferThenNotify { actor, amount } => {
            let who = actor_principal(world, *actor);
            match world.raw_transfer_to_canister(who, *amount) {
                Ok(block) => {
                    let r = world.notify_deposit(who, block);
                    if r.is_ok() {
                        world.note_raw_deposit_credited(*amount);
                    }
                    format!("block {block} -> {}", describe(&r))
                }
                Err(e) => format!("ledger transfer failed: {e}"),
            }
        }
        Op::NotifyBlock { actor, block } => {
            let who = actor_principal(world, *actor);
            // A successful credit here means a raw deposit was consumed. The
            // harness cannot know WHICH, so it conservatively assumes it was one
            // of its own outstanding raw deposits; if the canister credited money
            // that was never transferred, M1 catches it on the next check.
            let before = world.get_balance(who);
            let r = world.notify_deposit(who, *block);
            let after = world.get_balance(who);
            if r.is_ok() {
                world.note_raw_deposit_credited(after.saturating_sub(before));
            }
            describe(&r)
        }
        Op::ExternalDepositThenClaim { actor, amount } => {
            let who = actor_principal(world, *actor);
            match world.transfer_to_deposit_subaccount(who, *amount) {
                Ok(block) => {
                    let r = world.claim_external_deposit(who);
                    format!("block {block} -> {}", describe(&r))
                }
                Err(e) => format!("ledger transfer failed: {e}"),
            }
        }
        Op::ClaimExternal { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.claim_external_deposit(who))
        }
        Op::Withdraw { actor, amount } => {
            let who = actor_principal(world, *actor);
            describe(&world.withdraw(who, *amount))
        }
        Op::ReentrantWithdraw { actor, amount } => {
            let who = actor_principal(world, *actor);
            reentrant_withdraw(world, who, *amount)
        }
        Op::JoinTable { actor, seat } => {
            let who = actor_principal(world, *actor);
            describe(&world.join_table(who, *seat))
        }
        Op::BuyIn {
            actor,
            seat,
            amount,
        } => {
            let who = actor_principal(world, *actor);
            describe(&world.buy_in(who, *seat, *amount))
        }
        Op::Reload { actor, amount } => {
            let who = actor_principal(world, *actor);
            describe(&world.reload(who, *amount))
        }
        Op::CashOut { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.cash_out(who))
        }
        Op::LeaveTable { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.leave_table(who))
        }
        Op::ActInTurn { act } => match on_the_clock(world) {
            Some(who) => describe(&world.player_action(who, act.to_candid())),
            None => "no seat on the clock".to_string(),
        },
        Op::ActLegalInTurn => match on_the_clock(world) {
            Some(who) => {
                let mut tried = Vec::new();
                for candidate in [PlayerAction::Check, PlayerAction::Call, PlayerAction::Fold] {
                    let r = world.player_action(who, candidate.clone());
                    let text = describe(&r);
                    if r.is_ok() {
                        return StepResult {
                            op: op.clone(),
                            outcome: format!("{candidate:?} -> {text}"),
                        };
                    }
                    tried.push(format!("{candidate:?}:{text}"));
                }
                format!("nothing legal ({})", tried.join(", "))
            }
            None => "no seat on the clock".to_string(),
        },
        Op::FundAndSeat { actor } => {
            let who = actor_principal(world, *actor);
            let min = world.config.min_buy_in;
            let escrow = world.get_balance(who);
            let mut parts = Vec::new();
            if escrow < min {
                parts.push(format!(
                    "fund={}",
                    describe(&world.fund_escrow(who, min.saturating_mul(3)))
                ));
            }
            let t = world.table_state();
            let free = t.players.iter().position(|p| p.is_none());
            match free {
                Some(seat) => parts.push(format!(
                    "join({seat})={}",
                    describe(&world.join_table(who, seat as u8))
                )),
                None => parts.push("no free seat".to_string()),
            }
            // A seated player who is sitting out cannot be dealt in.
            parts.push(format!("sit_in={}", describe(&world.sit_in(who))));
            parts.join(" ")
        }
        Op::ActAs { actor, act } => {
            let who = actor_principal(world, *actor);
            describe(&world.player_action(who, act.to_candid()))
        }
        Op::ConcurrentActInTurn { act } => concurrent_act(world, act),
        Op::SitOut { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.sit_out(who))
        }
        Op::SitIn { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.sit_in(who))
        }
        Op::SitOutNextHand { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.sit_out_next_hand(who))
        }
        Op::UseTimeBank { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.use_time_bank(who))
        }
        Op::ShowCards { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.show_cards(who))
        }
        Op::Heartbeat { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.heartbeat(who))
        }
        Op::StartNewHand { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.start_new_hand(who))
        }
        Op::CheckTimeouts { actor } => {
            let who = actor_principal(world, *actor);
            describe(&world.check_timeouts(who))
        }
        Op::AdvanceTime { nanos } => {
            world.advance(Duration::from_nanos(*nanos));
            format!("advanced {nanos} ns")
        }
        Op::Upgrade => match world.upgrade() {
            Ok(()) => "upgraded".to_string(),
            Err(e) => format!("upgrade rejected: {e}"),
        },
        Op::GoSilent { actor } => format!("actor {actor} stops responding"),
    };

    StepResult {
        op: op.clone(),
        outcome,
    }
}

fn actor_principal(world: &World, index: usize) -> Principal {
    let n = world.actors.len();
    world.actors[index % n].principal
}

/// Whose turn it is, if a hand is in progress and the seat is occupied.
pub fn on_the_clock(world: &World) -> Option<Principal> {
    let t = world.table_state();
    if !t.phase.hand_in_progress() {
        return None;
    }
    t.players
        .get(t.action_on as usize)
        .and_then(|p| p.as_ref())
        .map(|p| p.principal)
}

/// Two `withdraw` calls for the same principal submitted into the SAME round.
/// This is the reentrancy shape `PENDING_WITHDRAWALS` exists to stop.
fn reentrant_withdraw(world: &World, who: Principal, amount: u64) -> String {
    let arg = encode_one(amount).expect("withdraw arg encode");
    let first = world
        .pic
        .submit_call(world.table, who, "withdraw", arg.clone());
    let second = world.pic.submit_call(world.table, who, "withdraw", arg);
    let mut parts = Vec::new();
    for (label, submitted) in [("A", first), ("B", second)] {
        match submitted {
            Ok(id) => match world.pic.await_call(id) {
                Ok(bytes) => parts.push(format!(
                    "{label}={:?}",
                    candid::decode_one::<Result<u64, String>>(&bytes)
                )),
                Err(r) => parts.push(format!("{label}=TRAP({r:?})")),
            },
            Err(r) => parts.push(format!("{label}=SUBMIT_REJECTED({r:?})")),
        }
    }
    parts.join(" ")
}

/// The same in-turn action submitted by the player on the clock AND by another
/// seated player, in one round.
fn concurrent_act(world: &World, act: &Act) -> String {
    let t = world.table_state();
    if !t.phase.hand_in_progress() {
        return "no hand in progress".to_string();
    }
    let seated: Vec<Principal> = t.seated().map(|p| p.principal).collect();
    if seated.len() < 2 {
        return "fewer than two seated".to_string();
    }
    let arg = encode_one(act.to_candid()).expect("player_action arg encode");
    // Submit ALL of them before awaiting any, so they land in the same round.
    let mut submitted = Vec::new();
    let mut parts = Vec::new();
    for who in seated.iter().take(3) {
        match world
            .pic
            .submit_call(world.table, *who, "player_action", arg.clone())
        {
            Ok(id) => submitted.push((*who, id)),
            // A rejected submission executed nothing; there is no reply to await.
            Err(r) => parts.push(format!("{}=SUBMIT_REJECTED({r:?})", short(who))),
        }
    }
    for (who, id) in submitted {
        match world.pic.await_call(id) {
            Ok(bytes) => parts.push(format!(
                "{}={:?}",
                short(&who),
                candid::decode_one::<Result<(), String>>(&bytes)
            )),
            Err(r) => parts.push(format!("{}=TRAP({r:?})", short(&who))),
        }
    }
    parts.join(" ")
}

fn short(p: &Principal) -> String {
    p.to_text().chars().take(5).collect()
}
