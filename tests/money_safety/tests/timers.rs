//! THE ON-CHAIN CLOCK -- does anything move this table without an external caller?
//!
//! [FINDING 19](../../../docs/SECURITY-FINDINGS.md#finding-19): `ic-cdk-timers` was
//! a declared dependency that nothing imported. `set_timer` appeared nowhere in
//! `src/`, so every deadline in the engine -- the action clock, the disconnect
//! threshold, the sitting-out kick, the reload timer, the stuck-hand grace -- was
//! evaluated only inside a message somebody else sent, and a table whose clients
//! all closed their tabs froze.
//!
//! # What "no external caller" means here, exactly
//!
//! After the setup, the only things these tests do are:
//!
//! * `pic.tick()` -- the subnet producing a block, which it does whether or not
//!   anybody is watching; and
//! * `pic.query_call(...)` -- a read. A query cannot change canister state.
//!
//! No ingress update message is sent. If the phase changes under those two alone,
//! something ON CHAIN moved the game. The control arm
//! (`the_clock_and_check_timeouts_cannot_drift`) is the only test here that sends
//! an update, and it exists to prove the two paths agree.
//!
//! ```text
//! cd tests/money_safety
//! cargo test --test timers -- --nocapture --test-threads=1
//! ```

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use money_safety::table_api::*;
use money_safety::world::*;
use std::time::Duration;

const ICP: u64 = 100_000_000;
const SEC: u64 = 1_000_000_000;

/// The action clock every table in this file is configured with.
const ACTION_CLOCK_SECS: u64 = 30;
/// `DISCONNECT_TIMEOUT_SECS` in `src/table_canister/src/lib.rs`.
const DISCONNECT_SECS: u64 = 90;
/// `SITTING_OUT_KICK_SECS` in `src/table_canister/src/lib.rs`.
const KICK_SECS: u64 = 120;
/// `CLOCK_WATCHDOG_SECS` in `src/table_canister/src/lib.rs`. The most any deadline
/// can be late by, and therefore the slack every bound below is allowed.
const WATCHDOG_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// `get_cycle_status`, mirrored
// ---------------------------------------------------------------------------

/// Mirror of `CycleStatus` in `src/table_canister/src/lib.rs`.
///
/// A separate declaration on purpose: if the canister's record changes shape, the
/// decode here fails loudly rather than the test quietly reading a different
/// field. `clock_ticks` is the field that matters most -- it is the only externally
/// visible evidence that the clock is actually running, and a canister that has
/// been up for minutes with `clock_ticks == 0` has lost it.
#[derive(Clone, Debug, CandidType, Deserialize)]
struct CycleStatus {
    balance: u128,
    liquid_balance: u128,
    reserved_for_freezing: u128,
    observed_burn_per_day: u128,
    runway_days: Option<u64>,
    sample_window_secs: u64,
    measurement_is_meaningful: bool,
    clock_ticks: u64,
    clock_last_tick_at: u64,
    clock_watchdog_armed: bool,
    next_wake_at: Option<u64>,
}

fn cycle_status(world: &World) -> CycleStatus {
    let bytes = world
        .pic
        .query_call(
            world.table,
            Principal::anonymous(),
            "get_cycle_status",
            Encode!().unwrap(),
        )
        .expect("get_cycle_status must be callable by anybody, including anonymously");
    Decode!(&bytes, CycleStatus).expect("CycleStatus decode")
}

// ---------------------------------------------------------------------------
// silence
// ---------------------------------------------------------------------------

/// Let simulated time pass and let the replica produce blocks, WITHOUT sending
/// the canister any message.
///
/// `ticks_per_step` matters. `ic-cdk-timers` 1.0 does not run a callback inside
/// `canister_global_timer`; it makes a bounded-wait SELF-CALL and runs the
/// callback in that message, deliberately, so that a trapping callback is caught
/// at the call boundary. A fired timer therefore needs several rounds to complete:
/// the global-timer round, the self-call round, and the reply round. One `tick()`
/// per step would measure the replica's round budget rather than the canister's
/// clock.
fn quiet(world: &World, total: Duration, step: Duration, ticks_per_step: u32) {
    let steps = (total.as_nanos() / step.as_nanos().max(1)).max(1) as u32;
    for _ in 0..steps {
        world.pic.advance_time(step);
        for _ in 0..ticks_per_step {
            world.pic.tick();
        }
    }
}

fn seats(t: &TableState) -> String {
    t.players
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
        .map(|(i, p)| {
            format!(
                "seat{i} chips={} bet={} folded={} cards={} status={:?}",
                p.chips,
                p.total_bet_this_hand,
                p.has_folded,
                p.hole_cards.is_some(),
                p.status
            )
        })
        .collect::<Vec<_>>()
        .join("\n      ")
}

/// The money-carrying facts about a table, independent of which cards were dealt.
/// Used to compare the timer path against the `check_timeouts` path.
fn shape(t: &TableState) -> String {
    format!(
        "phase={:?} pot={} seats=[{}]",
        t.phase,
        t.pot,
        t.players
            .iter()
            .map(|p| match p {
                Some(p) => format!("{}:{}", p.chips, p.has_folded),
                None => "-".to_string(),
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Two funded seats and a real hand in progress, with a real pot.
fn table_mid_hand() -> (World, Principal, Principal) {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    for (i, who) in [alice, bob].iter().enumerate() {
        world.fund_escrow(*who, 20 * ICP).expect("deposit");
        world.join_table(*who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world.start_new_hand(alice).expect("deal");
    let t = world.table_state();
    assert!(t.phase.hand_in_progress(), "a hand must be live");
    assert!(t.pot > 0, "the table must actually hold money");
    (world, alice, bob)
}

// ===========================================================================
// GATE 1 -- a table with every client closed settles itself
// ===========================================================================

/// **THE HEADLINE, AND A GATE.**
///
/// Every client closes its tab mid-hand. Nothing sends the canister a message
/// ever again. The hand must resolve, and every player's money must become
/// reachable.
///
/// # Measured on the module WITHOUT the clock (2026-08-05)
///
/// ```text
///   t+  60s  phase=PreFlop pot=3000000 stuck=false abandonable_in=Some(269)s seated=2
///   ...
///   t+1200s  phase=PreFlop pot=3000000 stuck=true  abandonable_in=None      seated=2
///   RESULT: hand resolved at None, seats released at None
/// ```
///
/// Twenty simulated minutes, a live pre-flop hand holding 3,000,000 e8s, and both
/// seats still marked `Active` -- the disconnect threshold had not been evaluated
/// either. The dead window was not 5.5 minutes; it was unbounded. 5.5 minutes is
/// only how long until `abandon_stuck_hand` would have been ACCEPTED, and that
/// still needs a caller.
#[test]
fn the_table_settles_itself_with_no_external_caller() {
    let (world, _alice, _bob) = table_mid_hand();
    let t0 = world.now_nanos();
    let staked = world.snapshot().internal_total();
    let clock_secs = world
        .table_state()
        .action_timer
        .as_ref()
        .map(|a| a.expires_at.saturating_sub(t0) / SEC)
        .unwrap_or(0);
    eprintln!(
        "\n=== DEAD WINDOW ===\n  every client closes at t=0; pot={} action clock expires in {clock_secs}s",
        world.table_state().pot
    );

    let mut settled_at: Option<u64> = None;
    let mut released_at: Option<u64> = None;
    for _ in 0..120 {
        quiet(&world, Duration::from_secs(10), Duration::from_secs(10), 4);
        let elapsed = (world.now_nanos() - t0) / SEC;
        let t = world.table_state();
        if settled_at.is_none() && !t.phase.hand_in_progress() {
            settled_at = Some(elapsed);
            eprintln!(
                "  t+{elapsed:>4}s  HAND RESOLVED ITSELF, phase={:?}\n      {}",
                t.phase,
                seats(&t)
            );
        }
        if settled_at.is_some() && released_at.is_none() && t.seated().count() == 0 {
            released_at = Some(elapsed);
            eprintln!("  t+{elapsed:>4}s  every seat released, chips back in escrow");
            break;
        }
    }

    let status = cycle_status(&world);
    eprintln!(
        "  clock ran {} times over the window\n",
        status.clock_ticks
    );

    // --- the hand resolved on its own clock --------------------------------
    let settled = settled_at.expect(
        "NOTHING ON CHAIN MOVED THE GAME. The hand was still live after 20 simulated \
         minutes with no client attached. This is FINDING 19 and it is not fixed.",
    );
    assert!(
        settled <= ACTION_CLOCK_SECS + WATCHDOG_SECS + 10,
        "the hand took {settled}s to resolve itself; the action clock is {ACTION_CLOCK_SECS}s \
         and the watchdog is {WATCHDOG_SECS}s, so anything past {}s means the wake is not \
         being aimed at the action clock",
        ACTION_CLOCK_SECS + WATCHDOG_SECS + 10
    );

    // --- and the seats emptied themselves, which is what makes the money
    //     reachable by an ordinary `withdraw` -------------------------------
    let released = released_at.expect(
        "the hand resolved but the seats never emptied: the disconnect threshold and the \
         sitting-out kick are still waiting for a caller",
    );
    assert!(
        released <= DISCONNECT_SECS + KICK_SECS + WATCHDOG_SECS + 20,
        "seats took {released}s to release; disconnect {DISCONNECT_SECS}s + kick {KICK_SECS}s \
         + watchdog {WATCHDOG_SECS}s is the budget"
    );

    // --- and no money went anywhere on the way ------------------------------
    let t = world.table_state();
    assert_eq!(t.pot, 0, "a settled table must not still hold a pot");
    assert_eq!(
        world.snapshot().internal_total(),
        staked,
        "the clock must not create or destroy a single e8 while nobody is watching"
    );
    assert!(
        status.clock_ticks > 0,
        "the canister reports zero clock ticks, so whatever moved the table it was not the clock"
    );

    eprintln!(
        "  RESULT: hand resolved at t+{settled}s, seats released at t+{released}s, \
         internal total unchanged at {staked} e8s\n"
    );
}

// ===========================================================================
// GATE 2 -- the upgrade
// ===========================================================================

/// **A GATE. Timers do not survive upgrades.**
///
/// The CDK's task queue is heap and the system's global-timer field is cleared,
/// so an upgrade that does not re-arm leaves a canister holding every player's
/// money and a live hand, with nothing on chain able to move either -- and nothing
/// about it looks wrong from outside. This is the classic way an on-chain timer
/// silently stops existing.
///
/// Upgrades MID-HAND, then goes completely silent, and requires the same
/// self-settlement Gate 1 requires.
#[test]
fn the_clock_survives_a_mid_hand_upgrade() {
    let (mut world, _alice, _bob) = table_mid_hand();
    let before_state = world.table_state();
    let before_total = world.snapshot().internal_total();
    let ticks_before = cycle_status(&world).clock_ticks;

    world.upgrade().expect("a mid-hand upgrade must succeed");

    let after = world.table_state();
    assert_eq!(before_state.pot, after.pot, "the upgrade moved the pot");
    assert!(
        after.phase.hand_in_progress(),
        "the hand must still be live across the upgrade, or this test proves nothing"
    );

    let status = cycle_status(&world);
    assert!(
        status.clock_watchdog_armed,
        "post_upgrade did not re-arm the watchdog. The canister still holds {} e8s and a live \
         hand, and there is now nothing on chain that can move either.",
        before_total
    );
    assert_eq!(
        status.clock_ticks, 0,
        "clock_ticks is instance-scoped and must restart at 0 after an upgrade; a non-zero \
         value here means this test is reading a stale instance and cannot prove anything"
    );
    eprintln!(
        "\n=== MID-HAND UPGRADE ===\n  before: {} ticks. after: watchdog_armed={} ticks={} \
         next_wake_at={:?}",
        ticks_before,
        status.clock_watchdog_armed,
        status.clock_ticks,
        status.next_wake_at.map(|w| (w.saturating_sub(world.now_nanos())) / SEC)
    );

    // Now go silent. The re-armed clock has to do the whole job by itself.
    let t0 = world.now_nanos();
    let mut settled_at: Option<u64> = None;
    for _ in 0..30 {
        quiet(&world, Duration::from_secs(10), Duration::from_secs(10), 4);
        if !world.table_state().phase.hand_in_progress() {
            settled_at = Some((world.now_nanos() - t0) / SEC);
            break;
        }
    }
    let settled = settled_at.expect(
        "THE CLOCK DID NOT SURVIVE THE UPGRADE. The hand never resolved after the upgrade with \
         no client attached, which is exactly the silent-stop this gate exists for.",
    );
    assert!(
        settled <= ACTION_CLOCK_SECS + WATCHDOG_SECS + 10,
        "the post-upgrade hand took {settled}s to resolve"
    );
    assert_eq!(
        world.snapshot().internal_total(),
        before_total,
        "the upgrade plus the clock must conserve every e8"
    );
    let after_ticks = cycle_status(&world).clock_ticks;
    assert!(
        after_ticks > 0,
        "the hand resolved but the canister reports zero ticks since the upgrade"
    );
    eprintln!("  resolved at t+{settled}s on {after_ticks} post-upgrade ticks\n");
}

// ===========================================================================
// GATE 3 -- a STALLED table is played out, not voided
// ===========================================================================

/// **A GATE, and the one that convicts the defect this work introduced.**
///
/// A hand whose action clock expired an hour ago is *abandonable by the wall
/// clock*. It is not *unmovable*. Those two come apart after any stall in which the
/// canister does not execute: a subnet halt, or -- pointedly -- a canister frozen for
/// want of cycles and later topped up ([E-55](../../../docs/DEFECTS.md#e-55)).
///
/// The first version of the clock conflated them and refunded every stake in a
/// perfectly playable hand. The money-safety fuzzer convicted it on seed
/// `0xc1ea2dec0002`:
///
/// ```text
/// CRITICAL: hand 7 was ABANDONED as unmovable at phase flop: no message could
/// advance it. 4000000 e8s across 2 stakes returned ... nobody won the hand.
/// ```
///
/// Nothing was unmovable. The fuzzer had jumped an hour of simulated time, so the
/// clock was five minutes past expiry the *first* time anything looked at it.
///
/// The fuzzer catches that shape only incidentally, through the unregistered
/// `CRITICAL:` line, and a narrower regression slips past it -- verified: reverting
/// the fix so the grace is measured from `expires_at` again leaves
/// `cargo test --test fuzz` **green**. So the property gets its own gate here.
///
/// The stall is simulated the only honest way: advance time with **no ticks at
/// all**, which is exactly a canister that does not execute.
#[test]
fn a_hand_stalled_past_its_grace_is_played_out_not_voided() {
    let (mut world, _alice, _bob) = table_mid_hand();
    let staked = world.snapshot().internal_total();
    let pot = world.table_state().pot;
    let _ = world.new_canister_logs(); // drop setup chatter

    // AN HOUR IN WHICH THE CANISTER DOES NOT EXECUTE. No ticks: no timer can fire,
    // no message runs. This is a subnet halt, or a frozen canister.
    world.pic.advance_time(Duration::from_secs(3600));

    // Now it starts executing again. The clock sees a hand whose action timer
    // expired 3600 s ago, which is well past STUCK_HAND_GRACE_NS (300 s).
    for _ in 0..12 {
        quiet(&world, Duration::from_secs(10), Duration::from_secs(10), 4);
        if !world.table_state().phase.hand_in_progress() {
            break;
        }
    }

    let t = world.table_state();
    let logs = world.new_canister_logs();
    let abandoned: Vec<&String> = logs.iter().filter(|l| l.contains("ABANDONED")).collect();
    eprintln!(
        "\n=== STALLED TABLE ===\n  after a 3600s stall with no execution: phase={:?} pot={}\n      {}\n  abandon lines: {}\n",
        t.phase,
        t.pot,
        seats(&t),
        abandoned.len()
    );

    assert!(
        abandoned.is_empty(),
        "THE CLOCK VOIDED A PLAYABLE HAND. A stall is not a stuck hand: the seat whose \
         clock expired should have been folded and the hand played out. The grace period \
         must be measured from when the canister FIRST SAW the clock overdue, not from the \
         timer's own expires_at. See docs/DEFECTS.md E-56.\n  {}",
        abandoned
            .iter()
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    assert!(
        !t.phase.hand_in_progress(),
        "the stalled hand never resolved at all: phase={:?}",
        t.phase
    );

    // And it was decided, not refunded: exactly one seat folded, and somebody is up
    // by the pot rather than everyone being back where they started.
    let folded = t.seated().filter(|p| p.has_folded).count();
    assert_eq!(
        folded, 1,
        "a fold-out must fold exactly the seat whose clock ran out; seats: {:?}",
        t.seated().map(|p| (p.seat, p.has_folded)).collect::<Vec<_>>()
    );
    let winner_gain = t
        .seated()
        .map(|p| p.chips)
        .max()
        .unwrap_or(0)
        .saturating_sub(t.seated().map(|p| p.chips).min().unwrap_or(0));
    assert!(
        winner_gain > 0,
        "NOBODY WON. Every seat holds the same stack, which is what a refund looks like: \
         the hand was voided rather than decided. pot was {pot}"
    );
    assert_eq!(
        world.snapshot().internal_total(),
        staked,
        "playing out a stalled hand must conserve every e8"
    );
}

// ===========================================================================
// GATE 4 -- the two paths cannot drift
// ===========================================================================

/// **A GATE.** `check_timeouts` is kept callable so the system still works if a
/// timer is ever lost. That is only worth anything if the two paths do the same
/// thing.
///
/// Two identical tables. One is driven ONLY by the on-chain clock, with no ingress
/// at all. The other is driven ONLY by `check_timeouts` on the same simulated
/// schedule. The money-carrying shape of the two tables must end up identical.
///
/// This does not merely assert that one function calls another -- it drives both
/// entry points on a real replica and compares the outcome, because "they share an
/// implementation" is a claim about source and this is a claim about behaviour.
#[test]
fn the_clock_and_check_timeouts_cannot_drift() {
    // Arm A: nothing but ticks.
    let (timer_world, _a1, _b1) = table_mid_hand();
    for _ in 0..24 {
        quiet(&timer_world, Duration::from_secs(10), Duration::from_secs(10), 4);
    }

    // Arm B: an external caller doing what the frontend used to have to do.
    let (poll_world, alice, _b2) = table_mid_hand();
    for _ in 0..24 {
        poll_world.pic.advance_time(Duration::from_secs(10));
        poll_world.pic.tick();
        let _ = poll_world.check_timeouts(alice);
    }

    let a = shape(&timer_world.table_state());
    let b = shape(&poll_world.table_state());
    eprintln!("\n=== DRIFT ===\n  clock-only    : {a}\n  check_timeouts: {b}\n");
    assert_eq!(
        a, b,
        "the on-chain clock and check_timeouts reached DIFFERENT states from the same start. \
         They are supposed to run the same function; if this fails, one of them has grown a \
         second copy of the timeout rules."
    );
}

// ===========================================================================
// MEASUREMENT -- what the clock costs, and how long the tank lasts
// ===========================================================================

fn burn_over_an_hour(world: &World, label: &str) -> (u128, u64) {
    // Let the install settle so the measurement is of the steady state.
    quiet(world, Duration::from_secs(120), Duration::from_secs(10), 4);
    let ticks_before = cycle_status(world).clock_ticks;
    let before = world.pic.cycle_balance(world.table);
    let t0 = world.now_nanos();

    quiet(world, Duration::from_secs(3600), Duration::from_secs(10), 4);

    let after = world.pic.cycle_balance(world.table);
    let elapsed_s = (world.now_nanos() - t0) / SEC;
    let ticks = cycle_status(world).clock_ticks - ticks_before;
    let burned = before.saturating_sub(after);
    let per_day = burned * 86_400 / (elapsed_s as u128).max(1);
    eprintln!(
        "\n=== BURN: {label} ===\n  {elapsed_s}s, {ticks} clock ticks, {burned} cycles burned\n  \
         => {per_day} cycles/day ({:.5} T/day, {:.2} T/year){}",
        per_day as f64 / 1e12,
        (per_day as f64 * 365.0) / 1e12,
        if ticks > 0 {
            format!("\n  => {} cycles per tick", burned / ticks as u128)
        } else {
            String::new()
        }
    );
    (per_day, ticks)
}

/// An EMPTY table: no players, no hand, nothing to do. This is what most tables in
/// a lobby are doing most of the time, and it is the number the timer makes worse.
#[test]
fn idle_table_cycle_burn_and_runway() {
    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let (per_day, ticks) = burn_over_an_hour(&world, "EMPTY TABLE");
    assert!(
        ticks > 0,
        "an empty table must still run its clock: it is the thing that would notice a stuck \
         hand appearing"
    );

    let status = cycle_status(&world);
    eprintln!(
        "  get_cycle_status: balance={} liquid={} reserved_for_freezing={}\n  \
         observed_burn_per_day={} runway_days={:?} window={}s meaningful={}",
        status.balance,
        status.liquid_balance,
        status.reserved_for_freezing,
        status.observed_burn_per_day,
        status.runway_days,
        status.sample_window_secs,
        status.measurement_is_meaningful
    );
    // Runway at balances an operator would plausibly hold.
    for t in [1u128, 5, 10, 50] {
        let bal = t * 1_000_000_000_000;
        eprintln!(
            "  runway at {t:>3} T cycles: {} days",
            if per_day > 0 { bal / per_day } else { 0 }
        );
    }
    eprintln!();

    assert!(
        status.measurement_is_meaningful,
        "get_cycle_status must be able to report a real burn rate after an hour of uptime; \
         a runway nobody can see is a runway nobody will watch"
    );
    assert!(
        status.runway_days.is_some(),
        "get_cycle_status reported no runway despite a meaningful measurement window"
    );
    assert!(
        status.liquid_balance <= status.balance,
        "liquid balance cannot exceed the total balance"
    );
}

/// A table with players seated and a hand being played out by the clock. The
/// expensive case, and the one where the one-shot wake is doing real work.
#[test]
fn busy_table_cycle_burn() {
    let (world, _alice, _bob) = table_mid_hand();
    let (_per_day, ticks) = burn_over_an_hour(&world, "TABLE THAT PLAYED A HAND OUT AND EMPTIED");
    assert!(ticks > 0, "the clock must run on a table with players at it");
}
