//! The seeded hostile-sequence fuzzer.
//!
//! A run is a pure function of `(seed, config, actor_names, steps)`: the
//! generator is a SplitMix64 stream and PocketIC is deterministic, so a failing
//! seed replays exactly. Every generated sequence is also written into the report
//! verbatim, so a reproducer never depends on regenerating it.
//!
//! After EVERY step the point-in-time invariants (M1, M1b, M2, M4) are evaluated,
//! and M3 is evaluated at every hand boundary. An upgrade step additionally
//! evaluates M5 across itself.
//!
//! Violations are classified by `Severity`. Violations that are already
//! characterised in `docs/SECURITY-FINDINGS.md` (money stranded inside the
//! canister, and the stale `side_pots` breakdown that causes it) are counted and
//! reported but do not stop the run -- otherwise the fuzzer would trip on FINDING
//! 01 within the first hand and never explore anything else. Anything else --
//! chips created from nothing, a double payout, state lost across an upgrade --
//! fails the run and is shrunk to a minimal reproducer.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::actions::{apply, Act, Op, StepResult};
use crate::invariants::{
    check_hand_payout_total, check_no_rake, check_point_in_time,
    check_self_reported_inconsistency, check_upgrade_durability, Violation,
};
use crate::rng::Rng;
use crate::table_api::{GamePhase, TableConfig};
use crate::world::{Snapshot, World};

pub const DEFAULT_STEPS: usize = 220;

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
    pub transcript_tail: Vec<StepResult>,
    pub final_ledger_main: u64,
    pub final_internal_total: u64,
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
    let kinds: [(u64, u8); 26] = [
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
        (2, 18),  // UseTimeBank
        (2, 19),  // ShowCards
        (3, 20),  // RawTransferThenNotify
        (3, 21),  // NotifyBlock (double-claim / forged)
        (3, 22),  // ExternalDepositThenClaim
        (2, 23),  // Upgrade
    ];
    let actor = live_actor(rng, gen, actors);
    match *rng.weighted(&kinds) {
        24 => Op::ActLegalInTurn,
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
        ops.push(next_op(&mut rng, &mut gen, config, actors));
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
        transcript.push(result);

        let after = world.snapshot();

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
                // Only meaningful when nothing external moved money. The cheap,
                // sound test for that: the ledger position did not change.
                if idle_before.ledger_main == after.ledger_main {
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

    let final_snap = world.snapshot();
    let (blocking, documented): (Vec<Finding>, Vec<Finding>) = findings
        .into_values()
        .partition(|f| !f.violation.severity.is_documented_defect());

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
