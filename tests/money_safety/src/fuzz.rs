//! The seeded hostile-sequence fuzzer.
//!
//! A run is a pure function of `(seed, config, actor_names, steps)`: the
//! generator is a SplitMix64 stream and PocketIC is deterministic, so a failing
//! seed replays exactly. Every generated sequence is also written into the report
//! verbatim, so a reproducer never depends on regenerating it.
//!
//! AND `config` AND `actor_names` ARE THEMSELVES A FUNCTION OF THE SEED
//! (`run_shape`), which is docs/DEFECTS.md H-26. That sentence above was true of
//! this function and false of the only thing that calls it: the driver picked the
//! table shape from the seed's POSITION IN THE LIST, so `MONEY_FUZZ_SEEDS=<seed>`
//! -- the reproducer command this repository prints, records in
//! `money-fuzz-report.json` and quotes in its register -- replayed a *different
//! game* from the one that found the violation. Seed `212967420072194` played
//! heads-up and found nothing when it was first in the list, and 6-max and found
//! two fund-creation defects when it was second. Nothing about the process, the
//! machine or the replica was involved; the tuple was simply incomplete.
//!
//! After EVERY step the point-in-time invariants (M1, M1b, M2, M4) are evaluated,
//! and M3 is evaluated at every hand boundary. An upgrade step additionally
//! evaluates M5 across itself.
//!
//! Whether a violation stops the run is decided by the NAMED register in
//! `crate::documented`, never by the sign of the delta. Only the handful of
//! (invariant, check, direction, magnitude) tuples listed there -- each carrying a
//! defect id from `docs/DEFECTS.md` -- are counted, reported and allowed to
//! continue, because otherwise the fuzzer would trip on E-01 within the first hand
//! and never explore anything else. Everything else -- chips from nothing, a double
//! payout, state lost across an upgrade, a rake, an unregistered `BUG:`/`CRITICAL:`
//! line from the canister itself -- fails the run and is shrunk to a minimal
//! reproducer.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::actions::{apply, Act, Op, StepResult};
use crate::fault;
use crate::hand_attribution::HandAttributionWatch;
use crate::invariants::reachability;
use crate::invariants::outcome::{check_hand_outcome, OutcomeCoverage, OutcomeWatch};
use crate::invariants::record::{check_archived_participants, ArchiveCoverage};
use crate::invariants::{
    check_custody_is_visible, check_hand_attribution, check_hand_payout_total,
    check_insolvency_is_reported, check_no_rake, external_money_moved,
    check_no_settlement_trap, check_point_in_time, check_self_reported_inconsistency,
    check_upgrade_durability, Violation,
};
use crate::rng::Rng;
use crate::table_api::{GamePhase, TableConfig};
use crate::world::{Snapshot, World};

pub const DEFAULT_STEPS: usize = 220;

// ---------------------------------------------------------------------------
// WHAT A SEED RUNS AGAINST -- docs/DEFECTS.md H-26
// ---------------------------------------------------------------------------

/// The actor pool every run draws its seats from, longest table first.
pub const ACTOR_POOL: [&str; 4] = ["alice", "bob", "carol", "attacker"];

/// The three table shapes a run can take, in the order this project has always
/// numbered them. Named, because a reproducer that says only `seed = N` does not
/// describe a game.
pub const SHAPES: [&str; 3] = ["heads_up_icp", "six_max_icp", "six_max_with_ante"];

/// Everything about a run that is not the seed, the step count or the wasm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunShape {
    /// Which of [`SHAPES`], by name. Recorded in the report so a reproducer is
    /// self-describing.
    pub shape: &'static str,
    pub config: TableConfig,
    pub actors: Vec<&'static str>,
}

/// THE ONE PLACE THAT DECIDES WHAT A SEED PLAYS, AND IT IGNORES `position`.
///
/// docs/DEFECTS.md H-26. `position` is the index of this seed in `MONEY_FUZZ_SEEDS`
/// and it is deliberately still a parameter: the gate
/// `a_seeds_table_shape_does_not_depend_on_where_it_appears_in_the_list` in
/// `tests/fuzz.rs` calls this with two different positions and the same seed and
/// requires the same answer, which is a check that cannot be written against a
/// function that does not take the argument it must not use. Delete the parameter
/// and you delete the gate.
///
/// **Why `seed - 1` and not `seed`.** The mapping is chosen so that every seed set
/// this repository actually runs keeps the exact shape it has always had, which
/// means every figure recorded against `fuzz-default`, against the `fast` smoke row
/// (`MONEY_FUZZ_SEEDS=1`) and against the 9x600 deep sweep stays comparable across
/// this change:
///
/// ```text
///   seed                  old (position % 3)   new ((seed-1) % 3)
///   1                     0 heads_up           0 heads_up
///   0xC1EA_2DEC_0001      0 heads_up           0 heads_up
///   0xC1EA_2DEC_0002      1 six_max            1 six_max
///   0xC1EA_2DEC_0003      2 six_max_ante       2 six_max_ante
///   212967420072193..201  0,1,2,0,1,2,0,1,2    0,1,2,0,1,2,0,1,2
/// ```
///
/// The deep sweep's nine seeds are consecutive integers starting at one where
/// `(seed - 1) % 3 == 0`, so consecutive positions and consecutive seeds agree
/// term for term. Nothing was tuned to make that true; it is why this mapping was
/// picked over `seed % 3`, which would have permuted all of them.
pub fn run_shape(position: usize, seed: u64) -> RunShape {
    // Read and discarded ON PURPOSE. See the doc comment: the gate needs an
    // argument to vary.
    let _ = position;
    let index = (seed.wrapping_sub(1) % 3) as usize;
    let config = match index {
        0 => TableConfig::heads_up_icp(),
        1 => TableConfig::six_max_icp(),
        _ => TableConfig::six_max_with_ante(),
    };
    let actors: Vec<&'static str> = ACTOR_POOL
        .iter()
        .copied()
        .take((config.max_players as usize + 2).min(ACTOR_POOL.len()))
        .collect();
    RunShape {
        shape: SHAPES[index],
        config,
        actors,
    }
}

/// The exact command that replays one run of this fuzzer, printed next to every
/// finding and recorded in every reproducer.
///
/// It is a whole command rather than a seed because a seed was not enough: see
/// H-26. If this string ever stops being sufficient, that is the same defect
/// again and this is where it shows up.
pub fn replay_command(seed: u64, steps: usize) -> String {
    format!(
        "cd tests/money_safety && MONEY_FUZZ_SEEDS={seed} MONEY_FUZZ_STEPS={steps} \
         cargo test --test fuzz -- --nocapture"
    )
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub signature: String,
    pub first_step: usize,
    pub occurrences: usize,
    pub worst_delta_e8s: i128,
    pub violation: Violation,
    /// The op that immediately preceded the first observation.
    pub triggering_op: Option<Op>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunReport {
    pub seed: u64,
    pub steps_planned: usize,
    pub steps_executed: usize,
    pub config: TableConfigReport,
    pub hands_completed: u64,
    pub upgrades: u32,
    pub blocking_findings: Vec<Finding>,
    pub documented_findings: Vec<Finding>,
    /// Total e8s the run observed stranded inside the canister (money nobody owns
    /// and nobody can withdraw).
    pub max_stranded_e8s: i128,
    /// M8 PRINCIPAL ATTRIBUTION coverage over this run. Reported, not merely
    /// counted: an attribution gate that quietly declines to measure is
    /// indistinguishable from one that passes.
    pub attribution: AttributionCoverage,
    /// M11 OUTCOME coverage over this run: how many hands had their OUTCOME
    /// checked, as opposed to their totals. Reported for the same reason.
    pub outcome: OutcomeCoverage,
    /// M12 ARCHIVE FIDELITY coverage: how many settled hands had their PERMANENT
    /// RECORD checked, how many of those were cross-checked against the stakes the
    /// harness watched, and how many involved a mid-hand departure -- the shape
    /// FINDING 30 needs. A run with zero departures has not tested it.
    pub archive: ArchiveCoverage,
    pub transcript_tail: Vec<StepResult>,
    pub final_ledger_main: u64,
    pub final_internal_total: u64,
}

/// How much of the run M8 actually got to speak about.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AttributionCoverage {
    /// Hands that finished with the table idle again.
    pub hands_seen: u64,
    /// Hands the four attribution legs ran on.
    pub hands_measured: u64,
    /// Hands the independent settlement-oracle leg ran on. Always a subset of
    /// `hands_measured`: the rules of poker do not define an answer for a chair
    /// carrying two people's money (docs/DEFECTS.md E-36).
    pub hands_oracle_checked: u64,
    /// Why the rest were declined, counted by reason.
    pub declined: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TableConfigReport {
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_players: u8,
    pub ante: u64,
    pub action_timeout_secs: u64,
}

impl From<&TableConfig> for TableConfigReport {
    fn from(c: &TableConfig) -> Self {
        Self {
            small_blind: c.small_blind,
            big_blind: c.big_blind,
            min_buy_in: c.min_buy_in,
            max_buy_in: c.max_buy_in,
            max_players: c.max_players,
            ante: c.ante,
            action_timeout_secs: c.action_timeout_secs,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FuzzReport {
    pub harness: &'static str,
    pub ledger: &'static str,
    pub ledger_sha256: &'static str,
    /// sha256 of the table canister module the run executed against. Without this a
    /// report cannot be attributed to a build (docs/DEFECTS.md H-01).
    pub table_wasm_sha256: &'static str,
    pub runs: Vec<RunReport>,
    pub minimal_reproducers: Vec<Reproducer>,
    pub totals: Totals,
}

#[derive(Clone, Debug, Serialize)]
pub struct Totals {
    pub runs: usize,
    pub steps: usize,
    pub blocking_signatures: Vec<String>,
    pub documented_signatures: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Reproducer {
    pub signature: String,
    pub seed: u64,
    /// Which of [`SHAPES`] the run that found this was playing. Recorded because
    /// for eleven waves it was not, and the seed alone did not identify the game
    /// (docs/DEFECTS.md H-26).
    pub shape: &'static str,
    /// The literal command that replays it.
    pub replay: String,
    pub ops: Vec<Op>,
    pub violation: Violation,
}

// ---------------------------------------------------------------------------
// generation
// ---------------------------------------------------------------------------

/// Mutable generator state: which actors have stopped responding, and which
/// deposit blocks the sequence has produced (so it can double-claim them).
#[derive(Clone, Debug, Default)]
struct GenState {
    silent: Vec<usize>,
    seen_blocks: Vec<u64>,
}

/// Amounts chosen to sit exactly on the boundaries the canister validates.
fn hostile_amount(rng: &mut Rng, config: &TableConfig) -> u64 {
    let choices: [(u64, u64); 12] = [
        (6, 0),
        (6, 1),
        (8, config.min_buy_in.saturating_sub(1)),
        (10, config.min_buy_in),
        (8, config.max_buy_in),
        (8, config.max_buy_in.saturating_add(1)),
        (4, u64::MAX),
        (4, u64::MAX / 2),
        (6, 100_000),         // exactly ICP_MIN_WITHDRAWAL_AMOUNT
        (6, 99_999),          // one below it
        (6, 10_000_000_001),  // one above ICP_MAX_WITHDRAWAL_PER_TX
        (28, 0),              // replaced below by a random plausible amount
    ];
    let picked = *rng.weighted(&choices);
    if picked == 0 && rng.chance(1, 2) {
        // A plausible amount in the stake's neighbourhood.
        rng.range(config.big_blind, config.max_buy_in.saturating_mul(2).max(2))
    } else {
        picked
    }
}

fn hostile_act(rng: &mut Rng, config: &TableConfig) -> Act {
    let choices: [(u64, u8); 6] = [(14, 0), (26, 1), (26, 2), (10, 3), (14, 4), (10, 5)];
    match *rng.weighted(&choices) {
        0 => Act::Fold,
        1 => Act::Check,
        2 => Act::Call,
        3 => Act::Bet(bet_amount(rng, config)),
        4 => Act::Raise(bet_amount(rng, config)),
        _ => Act::AllIn,
    }
}

fn bet_amount(rng: &mut Rng, config: &TableConfig) -> u64 {
    let choices: [(u64, u64); 7] = [
        (8, 0),
        (8, 1),
        (10, config.big_blind.saturating_sub(1)),
        (16, config.big_blind),
        (16, config.big_blind.saturating_mul(3)),
        (6, u64::MAX),
        (36, 0),
    ];
    let picked = *rng.weighted(&choices);
    if picked == 0 && rng.chance(3, 4) {
        rng.range(1, config.max_buy_in.max(2))
    } else {
        picked
    }
}

/// Mostly a real seat, sometimes out of range (an out-of-range seat must be
/// refused, and that refusal is worth exercising).
fn pick_seat(rng: &mut Rng, config: &TableConfig) -> u8 {
    if rng.chance(1, 6) {
        (config.max_players as u64 + rng.below(3)) as u8
    } else {
        rng.below(config.max_players as u64) as u8
    }
}

fn live_actor(rng: &mut Rng, gen: &GenState, actors: usize) -> usize {
    for _ in 0..8 {
        let a = rng.index(actors);
        if !gen.silent.contains(&a) {
            return a;
        }
    }
    rng.index(actors)
}

/// Generate the next op. `gen` carries the little state the generator needs.
fn next_op(rng: &mut Rng, gen: &mut GenState, config: &TableConfig, actors: usize) -> Op {
    // The weights matter: the sequence has to make real progress through hands,
    // or every invariant check runs against an idle table.
    let kinds: [(u64, u8); 27] = [
        (18, 0),  // ActInTurn (hostile amounts, often illegal)
        (30, 24), // ActLegalInTurn -- the engine of progress
        (6, 1),   // ActAs (frequently out of turn)
        (3, 2),   // ConcurrentActInTurn
        (10, 3),  // AdvanceTime
        (8, 4),   // CheckTimeouts
        (9, 5),   // StartNewHand
        (5, 6),   // FundEscrow
        (4, 7),   // JoinTable
        (4, 8),   // BuyIn
        (8, 25),  // FundAndSeat -- recovery, keeps the table dealable
        (3, 9),   // Reload
        (3, 10),  // Withdraw
        (2, 11),  // ReentrantWithdraw
        (3, 12),  // CashOut
        (3, 13),  // LeaveTable
        (2, 14),  // SitOut
        (5, 15),  // SitIn
        (2, 16),  // SitOutNextHand
        (2, 17),  // Heartbeat
        (2, 18),  // UseTimeBank -- a RANDOM actor, so it is nearly always refused
        // UseTimeBankOnClock -- the same call by the seat the engine's pointer
        // names, so it LANDS. Weighted well above the random-actor form because
        // that form measured nothing: `use_time_bank` refuses anybody who is not
        // `action_on`, and a call that is always refused is not coverage. This is
        // the op that reaches docs/DEFECTS.md E-41 / FINDING 39 -- the only
        // canister surface that armed an action clock with no hand in progress --
        // and it is deliberately generated at every point of the sequence, not
        // only inside a live hand, because the stale-pointer states BETWEEN hands
        // are the ones nothing could reach. See docs/DEFECTS.md H-26.
        (6, 26),  // UseTimeBankOnClock
        (2, 19),  // ShowCards
        (3, 20),  // RawTransferThenNotify
        (3, 21),  // NotifyBlock (double-claim / forged)
        (3, 22),  // ExternalDepositThenClaim
        (2, 23),  // Upgrade
    ];
    let actor = live_actor(rng, gen, actors);
    match *rng.weighted(&kinds) {
        24 => Op::ActLegalInTurn,
        26 => Op::UseTimeBankOnClock,
        25 => Op::FundAndSeat { actor },
        0 => Op::ActInTurn {
            act: hostile_act(rng, config),
        },
        1 => Op::ActAs {
            actor,
            act: hostile_act(rng, config),
        },
        2 => Op::ConcurrentActInTurn {
            act: hostile_act(rng, config),
        },
        3 => Op::AdvanceTime {
            nanos: *rng.weighted(&[
                (40, 150_000_000u64),        // 0.15s -- normal think time
                (16, 1_100_000_000),         // just past the 1s rate-limit window
                (12, 3_100_000_000),         // just past AUTO_DEAL_DELAY_NS
                (12, 31_000_000_000),        // just past a 30s action_timeout
                (8, 61_000_000_000),         // just past RELOAD_TIMEOUT / withdrawal cooldown
                (8, 121_000_000_000),        // just past SITTING_OUT_KICK_SECS
                (4, 3_600_000_000_000),      // an hour
            ]),
        },
        4 => Op::CheckTimeouts { actor },
        5 => Op::StartNewHand { actor },
        6 => Op::FundEscrow {
            actor,
            amount: *rng.weighted(&[
                (10, config.min_buy_in.saturating_mul(3)),
                (6, config.min_buy_in),
                (4, 20_000),  // exactly the minimum deposit
                (3, 19_999),  // one below it
                (3, 0),
                (2, u64::MAX),
            ]),
        },
        7 => Op::JoinTable {
            actor,
            seat: pick_seat(rng, config),
        },
        8 => Op::BuyIn {
            actor,
            seat: pick_seat(rng, config),
            amount: hostile_amount(rng, config),
        },
        9 => Op::Reload {
            actor,
            amount: hostile_amount(rng, config),
        },
        10 => Op::Withdraw {
            actor,
            amount: hostile_amount(rng, config),
        },
        11 => Op::ReentrantWithdraw {
            actor,
            amount: *rng.weighted(&[(6, 100_000_000u64), (4, 200_000_000), (2, u64::MAX)]),
        },
        12 => Op::CashOut { actor },
        13 => Op::LeaveTable { actor },
        14 => Op::SitOut { actor },
        15 => Op::SitIn { actor },
        16 => Op::SitOutNextHand { actor },
        17 => Op::Heartbeat { actor },
        18 => Op::UseTimeBank { actor },
        19 => Op::ShowCards { actor },
        20 => {
            let amount = *rng.weighted(&[
                (8, config.min_buy_in),
                (6, 50_000_000u64),
                (2, 20_000),
            ]);
            gen.seen_blocks.push(gen.seen_blocks.len() as u64);
            Op::RawTransferThenNotify { actor, amount }
        }
        21 => Op::NotifyBlock {
            actor,
            block: if gen.seen_blocks.is_empty() || rng.chance(1, 3) {
                rng.below(24)
            } else {
                *rng.pick(&gen.seen_blocks)
            },
        },
        22 => Op::ExternalDepositThenClaim {
            actor,
            amount: *rng.weighted(&[
                (8, config.min_buy_in),
                (6, 30_000_000u64),
                (3, 10_001),
                (2, 10_000), // exactly the fee: nothing is claimable
            ]),
        },
        _ => Op::Upgrade,
    }
}

/// Build a whole sequence up front. Generating ahead of execution (rather than
/// reacting to state) is what makes a sequence a self-contained artifact that can
/// be shrunk and replayed.
pub fn generate(seed: u64, steps: usize, config: &TableConfig, actors: usize) -> Vec<Op> {
    let mut rng = Rng::new(seed);
    let mut gen = GenState::default();
    let mut ops = Vec::with_capacity(steps + 8);

    // A short deterministic prologue so the run does not spend its whole budget
    // discovering how to seat players. Everything after this is generated.
    for a in 0..actors.min(3) {
        ops.push(Op::FundEscrow {
            actor: a,
            amount: config.min_buy_in.saturating_mul(4),
        });
        ops.push(Op::JoinTable {
            actor: a,
            seat: a as u8,
        });
    }
    ops.push(Op::AdvanceTime {
        nanos: 3_500_000_000,
    });
    ops.push(Op::StartNewHand { actor: 0 });

    while ops.len() < steps {
        // A player who simply stops responding, once, part way through.
        if ops.len() == steps / 2 && actors > 2 {
            let victim = 2 % actors;
            gen.silent.push(victim);
            ops.push(Op::GoSilent { actor: victim });
            continue;
        }
        let op = next_op(&mut rng, &mut gen, config, actors);
        // LET THE CLOCK HAVE A GO AT WHAT THE SEQUENCE JUST ARMED.
        //
        // Every clock defect in this register has the same two-part shape: one
        // message ARMS a deadline or changes who the table can deal to, and a LATER
        // deadline-crossing acts on it. E-54 (no clock at all), E-56 (a stalled
        // hand voided instead of played out), E-59 (two beliefs about one stall),
        // E-41 (an action clock armed on a finished hand). A generator that emits
        // `AdvanceTime` and `CheckTimeouts` independently of the ops that arm
        // something has to draw the pair in the right order by luck, and across 220
        // steps at three seeds it mostly does not.
        //
        // So a state-arming op sometimes carries its own deadline crossing. This is
        // a SHAPE, not a recipe: it knows nothing about what the crossing will find,
        // only that a deadline nobody crosses is a deadline nobody tests. The ops it
        // adds are counted against the same step budget, so a run costs the same.
        let arms_a_deadline_or_changes_who_can_be_dealt_to = matches!(
            op,
            Op::UseTimeBankOnClock
                | Op::UseTimeBank { .. }
                | Op::SitOut { .. }
                | Op::SitOutNextHand { .. }
                | Op::LeaveTable { .. }
                | Op::CashOut { .. }
        );
        ops.push(op);
        if arms_a_deadline_or_changes_who_can_be_dealt_to
            && rng.chance(1, 2)
            && ops.len() + 2 <= steps
        {
            ops.push(Op::AdvanceTime {
                nanos: 31_000_000_000,
            });
            ops.push(Op::CheckTimeouts { actor: 0 });
        }
    }
    ops
}

// ---------------------------------------------------------------------------
// execution
// ---------------------------------------------------------------------------

struct HandWatch {
    /// Snapshot at the last moment the table was idle with an empty pot.
    idle: Option<Snapshot>,
    last_hand_number: u64,
    /// Who held a seat at the first in-progress observation of the current hand.
    hand_seats: Option<Vec<candid::Principal>>,
    /// True once ANY of those seats has been vacated during this hand.
    ///
    /// Endpoint comparison is not enough: a player can `leave_table` mid-hand and
    /// then re-seat before the hand ends, which leaves the seated set identical at
    /// the two endpoints while the departure still erased their contribution from
    /// `collect_contributions`. The fuzzer found exactly that sequence.
    seat_churn: bool,
}

/// Run one sequence and collect every invariant violation it produces.
pub fn run_sequence(
    seed: u64,
    ops: &[Op],
    config: TableConfig,
    actor_names: &[&str],
) -> (RunReport, Vec<Violation>) {
    let mut world = World::new(config.clone(), actor_names);
    let mut findings: BTreeMap<String, Finding> = BTreeMap::new();
    let mut all: Vec<Violation> = Vec::new();
    let mut transcript: Vec<StepResult> = Vec::new();
    let mut watch = HandWatch {
        idle: None,
        last_hand_number: 0,
        hand_seats: None,
        seat_churn: false,
    };
    let mut max_stranded: i128 = 0;
    let mut executed = 0usize;
    // M8 PRINCIPAL ATTRIBUTION, in the per-step loop.
    //
    // It used to live only in two hand-written fixtures, so the class it exists to
    // catch was covered exactly where somebody had scripted it -- the same shape of
    // hole that let FINDING 13 through wave 2. The watch is fed the snapshot the
    // loop already takes, so it costs no extra messages, and it answers on every
    // hand a randomised hostile sequence happens to complete.
    let mut attribution = HandAttributionWatch::new();
    // M11 OUTCOME, in the same per-step loop and off the same snapshot.
    //
    // The legs M8 already runs all ask about money at the moment a hand ENDED. This
    // one asks whether the hand ended at the right moment, which is the question
    // FINDING 17 got wrong while every money assertion in this file stayed silent.
    let mut outcome_watch = OutcomeWatch::new();
    // Which step of THIS run gets a discarded continuation, if this run is one of
    // the fault-injecting ones. Derived from the seed, so a failing run replays
    // exactly, and placed past the opening steps so the table has money in it.
    let fault_step = if ops.is_empty() {
        usize::MAX
    } else {
        (ops.len() / 4).max(1) + ((seed >> 17) as usize % ops.len().max(1)) / 2
    };
    // M12 ARCHIVE FIDELITY, on every hand this run settles. The legs M8 and M11
    // run ask where the money went and whether the hand ended right; this one asks
    // whether the PERMANENT RECORD of the hand names the people who played it,
    // which is the product's central claim and which no instrument in this project
    // measured until docs/SECURITY-FINDINGS.md FINDING 30.
    let mut archive_coverage = ArchiveCoverage::default();

    let record = |findings: &mut BTreeMap<String, Finding>,
                      all: &mut Vec<Violation>,
                      max_stranded: &mut i128,
                      step: usize,
                      op: Option<&Op>,
                      vs: Vec<Violation>| {
        for v in vs {
            if v.delta_e8s > *max_stranded {
                *max_stranded = v.delta_e8s;
            }
            let sig = v.signature();
            let entry = findings.entry(sig.clone()).or_insert_with(|| Finding {
                signature: sig,
                first_step: step,
                occurrences: 0,
                worst_delta_e8s: 0,
                violation: v.clone(),
                triggering_op: op.cloned(),
            });
            entry.occurrences += 1;
            if v.delta_e8s.abs() > entry.worst_delta_e8s.abs() {
                entry.worst_delta_e8s = v.delta_e8s;
            }
            all.push(v);
        }
    };

    // Baseline: an empty table must already satisfy everything.
    let base = world.snapshot();
    record(
        &mut findings,
        &mut all,
        &mut max_stranded,
        0,
        None,
        check_point_in_time(&base, world.uncredited_raw_deposits),
    );

    for (i, op) in ops.iter().enumerate() {
        let before = if matches!(op, Op::Upgrade) {
            Some(world.snapshot())
        } else {
            None
        };

        let result = apply(&mut world, op);
        executed += 1;

        let after = world.snapshot();

        // --- M9 FUND REACHABILITY, in the per-step loop --------------------
        //
        // Costs zero extra messages: it reads the outcome of the op that just ran.
        // The property is that no update on the settlement path may TRAP while the
        // canister is holding money, because a trap rolls the message back and the
        // door stays shut for that state. docs/SECURITY-FINDINGS.md FINDING 15.
        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            check_no_settlement_trap(&result, after.internal_total(), &after.table.phase),
        );
        transcript.push(result);

        if let Some(before) = before {
            record(
                &mut findings,
                &mut all,
                &mut max_stranded,
                i + 1,
                Some(op),
                check_upgrade_durability(&before, &after),
            );
        }

        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            check_point_in_time(&after, world.uncredited_raw_deposits),
        );

        // --- M14 LEDGER/BOOKS COHERENCE, per step ---------------------------
        //
        // One extra query per step. In ordinary play it must be silent, which is
        // most of what running it every step buys: it is the fault runs below
        // that make it speak, and an invariant that only ever runs in the state
        // it was written for is an invariant nobody trusts.
        // docs/SECURITY-FINDINGS.md FINDING 29.
        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            fault::check_ledger_books_coherence(&world, &after),
        );

        // --- FAULT INJECTION AT THE LEDGER BOUNDARY -------------------------
        //
        // On one run in `FAULT_INJECTION_SEED_MODULUS`, at one deterministic step
        // chosen from the seed, discard the post-await continuation of a real
        // money-moving call and ask M12 about the result -- then make the OWNER
        // recover it with a player-only call and ask again. Both readings are
        // recorded, so a regression that makes the injected state incoherent, or
        // that makes it unrecoverable, fails the run.
        if fault::run_is_fault_injecting(seed) && i == fault_step {
            let run = fault::inject_and_recover(&mut world, seed ^ (i as u64));
            transcript.push(StepResult {
                op: Op::CheckTimeouts { actor: 0 },
                outcome: format!(
                    "[FAULT INJECTION] {} | resolved: {}",
                    run.description, run.resolution
                ),
            });
            record(
                &mut findings,
                &mut all,
                &mut max_stranded,
                i + 1,
                Some(op),
                run.during,
            );
            record(
                &mut findings,
                &mut all,
                &mut max_stranded,
                i + 1,
                Some(op),
                run.after_recovery,
            );
        }

        // M10 CUSTODY VISIBILITY, per step. Reads the canister AS EACH PLAYER whose
        // money is in the pot and who has no ordinary way of seeing it -- a stake
        // that outlived its seat, or a hand that can no longer be moved. Costs
        // nothing on the steps where nobody is in that position, which is almost
        // all of them, and it is the one check here that is not a controller's
        // view of the table. docs/SECURITY-FINDINGS.md FINDING 18.
        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            check_custody_is_visible(&world, &after),
        );

        // M2 SOLVENCY, per step: OWES <= HOLDS across every account this canister
        // owns, anchored to `icrc1_balance_of` -- and, when it is NOT, the canister
        // must be able to say so to an ordinary player.
        //
        // The query-only legs run inside `check_point_in_time` above and cost
        // nothing. This one takes an update call (`refresh_solvency`) and is
        // therefore gated on there actually being a shortfall, which on a healthy
        // run is never and on the state docs/SECURITY-FINDINGS.md FINDING 35
        // describes is every step.
        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            check_insolvency_is_reported(&world, &after),
        );

        // The engine's own testimony about its accounting.
        let logs = world.new_canister_logs();
        record(
            &mut findings,
            &mut all,
            &mut max_stranded,
            i + 1,
            Some(op),
            check_self_reported_inconsistency(&logs, &after.table.phase),
        );

        // --- M8 PRINCIPAL ATTRIBUTION, per hand -----------------------------
        //
        // Fed every step so it sees the hand while it is live (`departed_stakes` is
        // cleared at settlement and is the only record of a departed player's
        // stake), and it answers the moment the hand is over.
        {
            // Lazily: the history is one query, and it is only needed on the step a
            // hand settles. See `HandAttributionWatch::observe_lazily`.
            let attributed = attribution.observe_lazily(
                &after,
                world.uncredited_raw_deposits,
                |n| world.hand_history(n),
            );
            if let Some(hand) = attributed.as_ref() {
                record(
                    &mut findings,
                    &mut all,
                    &mut max_stranded,
                    i + 1,
                    Some(op),
                    check_hand_attribution(hand),
                );

                // --- M12 ARCHIVE FIDELITY, on this same settled hand ---------
                //
                // One more query, on the step a hand settles only. It reads the
                // table's own copy of the permanent record -- the SAME participant
                // list `record_hand_to_history` sends to the archive canister,
                // cloned from one build, so the two cannot disagree -- and holds
                // it against the stakes this harness watched being made.
                // docs/SECURITY-FINDINGS.md FINDING 30.
                if let Some(h) = world.hand_history(hand.hand_number) {
                    archive_coverage.observe(hand, &h);
                    record(
                        &mut findings,
                        &mut all,
                        &mut max_stranded,
                        i + 1,
                        Some(op),
                        check_archived_participants(hand, &h),
                    );
                }
            }

            // --- M11 OUTCOME, per step AND per hand -------------------------
            //
            // The per-step half is structural and needs nothing but the snapshot:
            // a live hand with money in it may never be observed with one claimant
            // or with none. The per-hand half needs the attribution's measured
            // value deltas, so it runs on the same step, with the same numbers.
            let (structural, finished) = outcome_watch.observe(&after);
            record(
                &mut findings,
                &mut all,
                &mut max_stranded,
                i + 1,
                Some(op),
                structural,
            );
            if let (Some(o), Some(hand)) = (finished.as_ref(), attributed.as_ref()) {
                let vs = check_hand_outcome(o, hand, &mut outcome_watch.coverage);
                record(&mut findings, &mut all, &mut max_stranded, i + 1, Some(op), vs);
            }
        }

        // --- seat churn tracking, for the sharp M3 basis --------------------
        if after.table.phase.hand_in_progress() {
            let now_seated = seated_set(&after);
            match &watch.hand_seats {
                None => watch.hand_seats = Some(now_seated),
                Some(at_start) => {
                    if at_start.iter().any(|p| !now_seated.contains(p)) {
                        watch.seat_churn = true;
                    }
                }
            }
        }

        // --- M3, at hand boundaries ---------------------------------------
        let idle_now = !after.table.phase.hand_in_progress() && after.table.pot == 0;
        if after.table.hand_number > watch.last_hand_number && idle_now {
            if let Some(idle_before) = watch.idle.clone() {
                // Only meaningful when nothing external moved money, and the test
                // for that has to cover EVERY ledger account the canister owns --
                // its main account AND its published deposit subaccounts -- because
                // `internal_total()` counts both. This read `ledger_main` alone and
                // therefore convicted a table of creating the 10,000 e8s somebody
                // had just deposited into a subaccount mid-hand. See
                // `external_money_moved` and docs/SECURITY-FINDINGS.md FINDING 44.
                if !external_money_moved(&idle_before, &after) {
                    record(
                        &mut findings,
                        &mut all,
                        &mut max_stranded,
                        i + 1,
                        Some(op),
                        check_no_rake(&idle_before, &after),
                    );
                }
            }
            // Sharper M3: the recorded winner amounts must sum to the pot the
            // hand collected. Only valid when no seat was vacated during the hand
            // -- see the note on `check_hand_payout_total`.
            let hand = after.table.hand_number;
            let seats_stable = !watch.seat_churn
                && watch
                    .idle
                    .as_ref()
                    .map(|b| seated_set(b) == seated_set(&after))
                    .unwrap_or(false);
            if seats_stable {
                if let Some(history) = world.hand_history(hand) {
                    let awarded: u64 = history
                        .winners
                        .iter()
                        .fold(0u64, |a, w| a.saturating_add(w.amount));
                    let collected = wagered_last_hand(&after);
                    if collected > 0 {
                        record(
                            &mut findings,
                            &mut all,
                            &mut max_stranded,
                            i + 1,
                            Some(op),
                            check_hand_payout_total(hand, collected, awarded, &after.table.phase),
                        );
                    }
                }
            }
            watch.last_hand_number = after.table.hand_number;
        }
        if idle_now {
            watch.idle = Some(after.clone());
            watch.last_hand_number = after.table.hand_number;
            watch.hand_seats = None;
            watch.seat_churn = false;
        }
    }

    // --- M9 FUND REACHABILITY, the constructive half ------------------------
    //
    // The sequence is over; whatever state 600 hostile steps left the table in is
    // the state a real player would be sitting in. Now take everybody's money out
    // for real, using only calls a player can make, and check it reaches the
    // ledger. A structural check can only fail on the failure modes somebody
    // imagined. This one fails on any of them.
    //
    // Runs LAST on purpose: it is destructive (it ends hands and empties seats),
    // so nothing else may observe the world afterwards except the drain's own
    // verdict and the final snapshot, which is taken after it and reported as
    // `final_internal_total` -- that number is now the answer to "how much could
    // not be got out", not merely "how much was left lying about".
    // Anything the canister started for a player and did not finish is finished
    // BY THAT PLAYER now, with a call any player may make, so the drain below
    // measures money a player can actually reach. docs/SECURITY-FINDINGS.md
    // FINDING 29: without this the drain would report an unfinished deposit as
    // unreachable, which would be true of the sequence but not of the canister.
    for line in fault::resolve_everyones_intents(&mut world) {
        transcript.push(StepResult {
            op: Op::CheckTimeouts { actor: 0 },
            outcome: format!("[RESOLVE INTENTS] {line}"),
        });
    }

    let (drain_report, drain_violations) = reachability::drain_and_check(&mut world);
    let drain_summary = format!(
        "owed {} -> {} e8s after draining",
        drain_report.owed_before, drain_report.owed_after
    );
    transcript.push(StepResult {
        op: Op::CheckTimeouts { actor: 0 },
        outcome: format!("[M9 DRAIN] {drain_summary}"),
    });
    record(
        &mut findings,
        &mut all,
        &mut max_stranded,
        executed + 1,
        None,
        drain_violations,
    );

    let final_snap = world.snapshot();
    let (blocking, documented): (Vec<Finding>, Vec<Finding>) = findings
        .into_values()
        .partition(|f| !crate::documented::is_documented(&f.violation));

    let tail_start = transcript.len().saturating_sub(40);
    let report = RunReport {
        seed,
        steps_planned: ops.len(),
        steps_executed: executed,
        config: TableConfigReport::from(&config),
        hands_completed: final_snap.table.hand_number,
        upgrades: world.upgrades,
        blocking_findings: blocking,
        documented_findings: documented,
        max_stranded_e8s: max_stranded,
        attribution: AttributionCoverage {
            hands_seen: attribution.hands_seen,
            hands_measured: attribution.hands_measured,
            hands_oracle_checked: attribution.hands_oracled,
            declined: attribution.declined.clone(),
        },
        outcome: outcome_watch.coverage.clone(),
        archive: archive_coverage.clone(),
        transcript_tail: transcript[tail_start..].to_vec(),
        final_ledger_main: final_snap.ledger_main,
        final_internal_total: final_snap.internal_total(),
    };
    (report, all)
}

/// After a hand completes the engine zeroes `pot` and clears `total_bet_this_hand`
/// only on the NEXT `start_new_hand`, so the seated players still carry what they
/// wagered. That is the money the hand collected.
/// The set of principals holding a seat, sorted so it compares as a set.
fn seated_set(snap: &Snapshot) -> Vec<candid::Principal> {
    let mut v: Vec<candid::Principal> = snap.table.seated().map(|p| p.principal).collect();
    v.sort();
    v
}

fn wagered_last_hand(snap: &Snapshot) -> u64 {
    if snap.table.phase == GamePhase::HandComplete {
        snap.table.wagered_total()
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// shrinking
// ---------------------------------------------------------------------------

/// Delta-debugging shrink: try to delete chunks of the sequence and keep any
/// deletion that still reproduces `signature`. Each candidate runs in a FRESH
/// world, so a surviving sequence is genuinely self-contained.
pub fn shrink(
    seed: u64,
    ops: &[Op],
    config: &TableConfig,
    actor_names: &[&str],
    signature: &str,
    max_attempts: usize,
) -> Vec<Op> {
    let mut best = ops.to_vec();
    let mut attempts = 0usize;
    let mut chunk = (best.len() / 2).max(1);

    while chunk >= 1 && attempts < max_attempts {
        let mut i = 0usize;
        let mut progressed = false;
        while i < best.len() && attempts < max_attempts {
            let end = (i + chunk).min(best.len());
            let mut candidate = Vec::with_capacity(best.len() - (end - i));
            candidate.extend_from_slice(&best[..i]);
            candidate.extend_from_slice(&best[end..]);
            attempts += 1;
            if candidate.is_empty() {
                i = end;
                continue;
            }
            let (_, vs) = run_sequence(seed, &candidate, config.clone(), actor_names);
            if vs.iter().any(|v| v.signature() == signature) {
                best = candidate;
                progressed = true;
                // Do not advance `i`: the chunk that used to be here is gone.
            } else {
                i = end;
            }
        }
        if !progressed && chunk == 1 {
            break;
        }
        chunk /= 2;
    }
    best
}

/// Where the fuzz report is written. Overridable so CI can put it somewhere else.
pub fn report_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("MONEY_FUZZ_REPORT") {
        return std::path::PathBuf::from(p);
    }
    crate::wasms::cache_dir().join("money-fuzz-report.json")
}

pub fn write_report(report: &FuzzReport) -> std::path::PathBuf {
    let path = report_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let json = serde_json::to_string_pretty(report).expect("report serialisation");
    std::fs::write(&path, json).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    path
}
