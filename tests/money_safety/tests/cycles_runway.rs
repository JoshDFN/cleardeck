//! THE RUNWAY -- how long until this canister stops honouring withdrawals?
//!
//! [E-55](../../../docs/DEFECTS.md#e-55),
//! [FINDING 24](../../../docs/SECURITY-FINDINGS.md#finding-24),
//! [FINDING 26](../../../docs/SECURITY-FINDINGS.md#finding-26).
//!
//! A canister below its freezing threshold rejects **every** update call at once:
//! `deposit`, `withdraw`, `cash_out`, `player_action`, `reload`,
//! `abandon_stuck_hand`. Every player at the table loses access to their own money
//! at the same instant, with no attacker involved. It is the only total custody
//! failure in this register that arrives on a schedule.
//!
//! `tests/timers.rs::idle_table_cycle_burn_and_runway` measures the **idle** burn.
//! Idle is not the number that matters. A table with players at it runs the clock,
//! settles hands, writes archive records and makes ledger calls, and every one of
//! those is an ingress message the IC charges to THE CANISTER, not to the caller.
//! This file measures the loaded figure and turns it into days.
//!
//! ```text
//! cd tests/money_safety
//! cargo test --test cycles_runway -- --nocapture --test-threads=1
//! ```
//!
//! # What each test here is for
//!
//! | test | question |
//! |------|----------|
//! | `the_instrument_itself_is_free` | do the harness's own queries perturb the measurement? |
//! | `burn_per_hand_under_realistic_load` | cycles per hand, per deposit, per withdrawal |
//! | `runway_at_real_balances` | those rates turned into days, at balances an operator holds |
//! | `a_frozen_table_answers_nothing_and_is_fully_recoverable` | FINDING 24, ported into the tree |
//! | `the_gauge_reads_high_after_a_quiet_start` | the runway number itself, under the load it exists for |
//!
//! # The standing lesson, applied to this file
//!
//! Every number below is a difference of two `pic.cycle_balance` reads taken from
//! OUTSIDE the canister. Nothing here trusts `get_cycle_status` to measure itself;
//! `get_cycle_status` is a SUBJECT of the last test, not an instrument of any of
//! them. That distinction is the entire reason the last test finds anything.

use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use money_safety::scenario;
use money_safety::table_api::*;
use money_safety::world::*;
use ic_management_canister_types::CanisterSettings;
use std::time::Duration;

const ICP: u64 = 100_000_000;
const SEC: u64 = 1_000_000_000;
const T: u128 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// `get_cycle_status`, mirrored
// ---------------------------------------------------------------------------

/// Mirror of `CycleStatus` in `src/table_canister/src/lib.rs`.
///
/// A separate declaration on purpose, exactly as in `tests/timers.rs`: if the
/// canister's record changes shape, the decode here fails loudly rather than this
/// file quietly reading a different field.
#[derive(Clone, Debug, CandidType, Deserialize)]
struct CycleStatus {
    balance: u128,
    liquid_balance: u128,
    reserved_for_freezing: u128,
    observed_burn_per_day: u128,
    recent_burn_per_day: Option<u128>,
    runway_days: Option<u64>,
    sample_window_secs: u64,
    recent_window_secs: Option<u64>,
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

/// `recent_burn_per_day`, which is an `opt` ON THE WIRE and must be.
///
/// The canister declares it optional so that a client built from this tree can
/// still decode a reply from the OLDER module running on mainnet -- Candid
/// refuses a record missing a non-optional field, and the frontend reports a
/// failed decode as "this table is not answering", which is the alarm for a
/// canister that has run out of cycles. That was measured, not imagined: see the
/// doc comment on `CycleStatus::recent_burn_per_day` in the canister.
///
/// THIS build always sets it, so a null reaching here is a real defect rather
/// than version skew, and it is loud.
fn recent_burn(s: &CycleStatus) -> u128 {
    s.recent_burn_per_day.expect(
        "this build always sets recent_burn_per_day; a null here means the sliding window \
         was dropped from the reply, and runway_days is a lifetime average again",
    )
}

/// Let simulated time pass and let the replica produce blocks, WITHOUT sending
/// the canister any message. Same shape as `tests/timers.rs::quiet` and for the
/// same reason: a fired timer needs several rounds to complete, so one tick per
/// step would measure the replica's round budget rather than the canister's clock.
fn quiet(world: &World, total: Duration, step: Duration, ticks_per_step: u32) {
    let steps = (total.as_nanos() / step.as_nanos().max(1)).max(1) as u32;
    for _ in 0..steps {
        world.pic.advance_time(step);
        for _ in 0..ticks_per_step {
            world.pic.tick();
        }
    }
}

/// Cycles per day, from a burn over a window of simulated seconds.
fn per_day(burned: u128, secs: u64) -> u128 {
    burned.saturating_mul(86_400) / (secs as u128).max(1)
}

fn t_per_day(v: u128) -> f64 {
    v as f64 / 1e12
}

/// Days of runway a `balance` buys at `burn_per_day`. Saturates rather than
/// dividing by zero, and a zero burn rate is reported as `None` because "we could
/// not measure a burn" and "there is no burn" are different claims.
fn runway_days(balance: u128, burn_per_day: u128) -> Option<u128> {
    if burn_per_day == 0 {
        None
    } else {
        Some(balance / burn_per_day)
    }
}

/// The balances worth reporting at. 10 T is the figure E-55 uses for a table.
const REPORT_BALANCES_T: [u128; 5] = [1, 5, 10, 50, 100];

/// Can a hand actually be dealt right now?
///
/// `scenario::play_one_hand` PANICS when `start_new_hand` refuses, so a
/// measurement loop that just calls it in a `for` loop dies the first time a
/// player busts and reports whatever partial burn it had. Checked rather than
/// assumed, because "the load ran" is the premise of every number below it.
fn can_deal(world: &World) -> bool {
    let t = world.table_state();
    t.seated()
        .filter(|p| p.chips >= t.config.big_blind && matches!(p.status, PlayerStatus::Active))
        .count()
        >= 2
}

/// Keep the seats filled and the stacks topped up between hands, from each
/// player's OWN escrow.
///
/// This is what a real table does -- players rebuy -- and without it the
/// measurement is over however few hands it takes for somebody to bust, which is
/// a different and much noisier quantity. Every call here is an ordinary
/// permissionless player call, so the cycles it costs belong in the bill.
fn refill_seats(world: &World, players: &[&str]) {
    for (i, name) in players.iter().enumerate() {
        let who = world.actor(name);
        let t = world.table_state();
        let seated = t
            .players
            .get(i)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal == who);
        match seated {
            Some(true) => {
                let p = t.players[i].as_ref().expect("checked");
                // THE HEARTBEAT IS NOT OPTIONAL, and leaving it out is how the
                // first version of this loop died. The disconnect threshold is
                // 90 s and the sitting-out kick is 120 s, so any gap between
                // hands longer than that marks every player Disconnected and then
                // empties the table -- correctly, that is the clock doing its job.
                // A real client heartbeats; so does this one, and it is an
                // ordinary permissionless update whose cycles belong in the bill.
                let _ = world.heartbeat(who);
                if !matches!(p.status, PlayerStatus::Active) {
                    let _ = world.sit_in(who);
                }
                let short = t.config.min_buy_in.saturating_sub(p.chips);
                if short > 0 && world.get_balance(who) > short {
                    let _ = world.reload(who, short);
                }
            }
            _ => {
                let _ = world.join_table(who, i as u8);
            }
        }
    }
}

fn print_runway_table(label: &str, burn_per_day: u128) {
    eprintln!(
        "\n  runway at {burn_per_day} cycles/day ({:.5} T/day) -- {label}",
        t_per_day(burn_per_day)
    );
    for t in REPORT_BALANCES_T {
        let bal = t * T;
        match runway_days(bal, burn_per_day) {
            Some(d) => eprintln!(
                "    {t:>4} T -> {d:>7} days  ({:.1} years)",
                d as f64 / 365.0
            ),
            None => eprintln!("    {t:>4} T -> unmeasured"),
        }
    }
}

/// The pause between hands at a real table, with the clients still attached.
///
/// A bare `quiet` longer than the 90 s disconnect threshold is not "a table
/// between hands", it is "a table everybody walked away from", and the clock
/// empties it. Heartbeating through the gap is what a live client does and it is
/// what makes the load in these tests SUSTAINED rather than a burst followed by an
/// eviction.
fn gap(world: &World, players: &[&str], secs: u64) {
    let mut left = secs;
    while left > 0 {
        let step = left.min(30);
        quiet(world, Duration::from_secs(step), Duration::from_secs(10), 4);
        for name in players {
            let _ = world.heartbeat(world.actor(name));
        }
        left -= step;
    }
}

// ===========================================================================
// 0 -- IS THE INSTRUMENT FREE?
// ===========================================================================

/// **The blind-spot check that has to run first.**
///
/// Every measurement in this file is a difference of two cycle-balance reads with
/// activity in between, and the harness inspects the table with QUERIES while that
/// activity happens. If a query costs the canister cycles in this environment, then
/// every "burn per hand" figure below is partly a measurement of the harness
/// watching, and the number an operator would compute from it is wrong.
///
/// On mainnet, query calls are not charged to the canister today. That is a claim
/// about the IC's pricing, and this project does not take pricing claims on trust:
/// it is measured here, on the same replica, in the same shape.
#[test]
fn the_instrument_itself_is_free() {
    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    quiet(&world, Duration::from_secs(120), Duration::from_secs(10), 4);

    // No ticks at all in the window: a tick would run the clock and the clock
    // burns. This measures queries and nothing else.
    let before = world.pic.cycle_balance(world.table);
    const N: u32 = 300;
    for _ in 0..N {
        let _ = world.pic.query_call(
            world.table,
            Principal::anonymous(),
            "get_cycle_status",
            Encode!().unwrap(),
        );
    }
    let after = world.pic.cycle_balance(world.table);
    let burned = before.saturating_sub(after);

    eprintln!(
        "\n=== INSTRUMENT ===\n  {N} anonymous queries, zero subnet ticks: {burned} cycles \
         ({} per query)",
        burned / N as u128
    );

    assert_eq!(
        burned, 0,
        "a query cost the canister {} cycles each in this environment, so every burn figure in \
         this file includes the harness's own observation and is not a statement about \
         production. Re-measure with the queries removed from the window before believing any \
         of it.",
        burned / N as u128
    );
}

// ===========================================================================
// 1 -- WHAT A HAND COSTS
// ===========================================================================

/// Cycles per hand, per deposit and per withdrawal, on a table with three funded
/// players playing real hands with real post-flop money.
///
/// # Why this is the number and the idle figure is not
///
/// E-55's runway table is built from an EMPTY table: 0.0442 T/day, all of it the
/// on-chain clock. That is the number for a table nobody is using. The moment
/// somebody sits down, every action is an ingress message, every hand writes an
/// archive record, and every deposit and withdrawal is an inter-canister call to
/// the ICP ledger with a reservation for the reply. The IC charges all of it to
/// the canister. A runway computed from the idle rate is therefore an upper bound
/// that a busy table never sees, and the whole point of this defect is what
/// happens on the day the balance runs out.
///
/// The per-hand figure is reported both RAW (everything that happened in the
/// window, divided by hands) and NET of the idle burn for the same simulated
/// seconds, because a hand takes wall time and the clock is ticking through it.
/// Net is the honest marginal cost of one more hand; raw is what an operator's
/// bill actually looks like.
#[test]
fn burn_per_hand_under_realistic_load() {
    let mut world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "dave"],
    );

    // --- the idle baseline, on THIS instance, so the two rates are comparable --
    quiet(&world, Duration::from_secs(120), Duration::from_secs(10), 4);
    let idle_t0 = world.now_nanos();
    let idle_before = world.pic.cycle_balance(world.table);
    quiet(&world, Duration::from_secs(1800), Duration::from_secs(10), 4);
    let idle_secs = (world.now_nanos() - idle_t0) / SEC;
    let idle_burned = idle_before.saturating_sub(world.pic.cycle_balance(world.table));
    let idle_per_day = per_day(idle_burned, idle_secs);
    let idle_per_sec = idle_burned / (idle_secs as u128).max(1);
    eprintln!(
        "\n=== IDLE BASELINE (empty table, this instance) ===\n  {idle_secs}s, {idle_burned} \
         cycles => {idle_per_day} cycles/day ({:.5} T/day)",
        t_per_day(idle_per_day)
    );

    // --- what a DEPOSIT costs -----------------------------------------------
    //
    // Two ingress messages (`icrc2_approve` to the ledger by the player, then
    // `deposit` to the table) and one inter-canister call with a reply reservation.
    // Only the table's balance is measured; the player's approve is charged to the
    // LEDGER canister, which is a different operator's bill.
    let players = ["alice", "bob", "carol"];
    let mut deposit_costs: Vec<u128> = Vec::new();
    for name in players {
        let who = world.actor(name);
        let before = world.pic.cycle_balance(world.table);
        world
            .fund_escrow(who, 50 * ICP)
            .unwrap_or_else(|e| panic!("fund_escrow for {name}: {e:?}"));
        deposit_costs.push(before.saturating_sub(world.pic.cycle_balance(world.table)));
    }
    let deposit_avg = deposit_costs.iter().sum::<u128>() / deposit_costs.len() as u128;
    eprintln!(
        "\n=== DEPOSIT ===\n  {:?}\n  => {deposit_avg} cycles per deposit (approve + deposit + \
         ledger round trip)",
        deposit_costs
    );

    // --- seat everybody ------------------------------------------------------
    for (i, name) in players.iter().enumerate() {
        world
            .join_table(world.actor(name), i as u8)
            .unwrap_or_else(|e| panic!("join_table for {name}: {e:?}"));
    }
    world.advance(Duration::from_secs(4));

    // --- what a HAND costs ---------------------------------------------------
    // A post-flop bet of two big blinds. Deliberately NOT a big number: at this
    // table's 2.00 ICP starting stack anything near an ICP is an all-in shove,
    // everybody busts inside five hands, and the measurement ends up being an
    // average over three hands and a lot of table-emptying. Two big blinds is
    // ordinary poker and it still puts real money in after the flop, which is
    // what makes the settlement path -- side pots, showdown, archive record --
    // actually run.
    let post_flop_bet = 2 * world.config.big_blind;
    const HANDS: usize = 10;
    let play_t0 = world.now_nanos();
    let play_before = world.pic.cycle_balance(world.table);
    let mut showdowns = 0usize;
    let mut hands_played = 0usize;
    for _ in 0..HANDS {
        refill_seats(&world, &players);
        if !can_deal(&world) {
            eprintln!("  (no dealable hand after {hands_played}; stopping the load early)");
            break;
        }
        let outcome = scenario::play_one_hand(&mut world, post_flop_bet);
        hands_played += 1;
        if outcome.reached_showdown() {
            showdowns += 1;
        }
        // The auto-deal delay between hands, as a real table experiences it.
        world.advance(Duration::from_secs(4));
    }
    let play_secs = (world.now_nanos() - play_t0) / SEC;
    let play_burned = play_before.saturating_sub(world.pic.cycle_balance(world.table));

    assert!(
        hands_played >= 4,
        "only {hands_played} hands were playable, which is too few to average over. The \
         measurement below would be dominated by whatever went wrong."
    );

    let raw_per_hand = play_burned / hands_played as u128;
    let idle_share = idle_per_sec.saturating_mul(play_secs as u128);
    let net_per_hand = play_burned.saturating_sub(idle_share) / hands_played as u128;
    let loaded_per_day = per_day(play_burned, play_secs);

    eprintln!(
        "\n=== HANDS ===\n  {hands_played} hands ({showdowns} to showdown) over {play_secs}s, \
         {play_burned} cycles\n  raw          : {raw_per_hand} cycles/hand\n  \
         idle share   : {idle_share} cycles ({idle_per_sec}/s x {play_secs}s)\n  \
         net of clock : {net_per_hand} cycles/hand\n  \
         while playing: {loaded_per_day} cycles/day ({:.4} T/day) -- a table dealing \
         continuously",
        t_per_day(loaded_per_day)
    );

    // --- what a HEARTBEAT costs, and why it turns out to be the whole bill ----
    //
    // `src/cleardeck_frontend/src/routes/+page.svelte` sends `heartbeat()` every
    // 10 SECONDS for every open tab at the table. That is 8,640 permissionless
    // update calls per player per day, each one an ingress message the IC charges
    // to THIS canister. Nothing in E-55, in FINDING 19 or in the idle measurement
    // sees it, because the idle measurement is of a table with nobody at it.
    //
    // MEASURED FROM A SEATED PLAYER, and the first version of this was not.
    // `heartbeat` returns Err for a principal who is not at the table, so the
    // first attempt measured sixty REFUSALS, divided by a `max(1)` guard, and
    // reported 429,696,279 cycles per heartbeat -- thirty times the truth, in a
    // block that printed "0/60 accepted" one line above it. Both halves are kept
    // now: the accepted cost, and the refused cost, which is not zero.
    let hb_who = world.actor(players[0]);
    const HEARTBEATS: u32 = 60;
    let hb_before = world.pic.cycle_balance(world.table);
    let mut hb_ok = 0u32;
    for _ in 0..HEARTBEATS {
        if world.heartbeat(hb_who).is_ok() {
            hb_ok += 1;
        }
        // One per simulated second: MAX_HEARTBEATS_PER_SECOND is 2, and a
        // measurement of the rate limiter is not a measurement of a heartbeat.
        world.advance(Duration::from_secs(1));
    }
    let hb_burned = hb_before.saturating_sub(world.pic.cycle_balance(world.table));
    assert!(
        hb_ok >= HEARTBEATS / 2,
        "only {hb_ok}/{HEARTBEATS} heartbeats were accepted, so this block measured refusals \
         rather than heartbeats and the per-heartbeat figure below would be nonsense"
    );
    // The clock ran through this window too; take it out.
    let hb_idle = idle_per_sec.saturating_mul(HEARTBEATS as u128);
    let per_heartbeat = hb_burned.saturating_sub(hb_idle) / (hb_ok as u128);
    // 10 s cadence, per seated player, all day.
    let hb_per_player_day = per_heartbeat.saturating_mul(8_640);
    eprintln!(
        "\n=== HEARTBEAT ===\n  {hb_ok}/{HEARTBEATS} accepted, {hb_burned} cycles gross, \
         {hb_idle} of it the clock => {per_heartbeat} cycles each\n  at the frontend's 10 s \
         cadence that is {hb_per_player_day} cycles/day ({:.4} T/day) PER SEATED PLAYER\n  a \
         full 6-max table of open tabs: {:.4} T/day, before a single card is dealt",
        t_per_day(hb_per_player_day),
        t_per_day(hb_per_player_day.saturating_mul(6))
    );

    // A REFUSED update is not free either, which is the half of
    // docs/SECURITY-FINDINGS.md FINDING 26 that rate limiting does not fix: the
    // ingress is induced and charged before the canister decides to say no.
    let stranger = world.actor("dave");
    let refused_before = world.pic.cycle_balance(world.table);
    let mut refused = 0u32;
    for _ in 0..20 {
        if world.heartbeat(stranger).is_err() {
            refused += 1;
        }
        world.advance(Duration::from_secs(1));
    }
    let refused_burn = refused_before.saturating_sub(world.pic.cycle_balance(world.table));
    let refused_idle = idle_per_sec.saturating_mul(20);
    eprintln!(
        "  a heartbeat from a principal who is NOT at the table is REFUSED and still costs \
         {} cycles ({refused}/20 refused). Rate limiting moves the price, it does not remove \
         it.",
        refused_burn.saturating_sub(refused_idle) / (refused.max(1) as u128)
    );

    // --- what a WITHDRAWAL costs ---------------------------------------------
    //
    // The custody-critical call, and the one that is rejected first when the
    // balance runs out.
    let mut withdraw_costs: Vec<u128> = Vec::new();
    for name in players {
        let who = world.actor(name);
        // Leave the seat so the whole balance is in escrow and withdrawable.
        let _ = world.leave_table(who);
        world.advance(Duration::from_secs(1));
        let bal = world.get_balance(who);
        if bal < 2 * ICP {
            continue;
        }
        let before = world.pic.cycle_balance(world.table);
        match world.withdraw(who, ICP) {
            Ok(_) => {
                withdraw_costs.push(before.saturating_sub(world.pic.cycle_balance(world.table)))
            }
            Err(e) => eprintln!("  withdraw for {name} refused: {e:?}"),
        }
    }
    assert!(
        !withdraw_costs.is_empty(),
        "not one withdrawal completed, so the cost of the call this whole defect is about was \
         never measured"
    );
    let withdraw_avg = withdraw_costs.iter().sum::<u128>() / withdraw_costs.len() as u128;
    eprintln!(
        "\n=== WITHDRAW ===\n  {:?}\n  => {withdraw_avg} cycles per withdrawal (ledger \
         icrc1_transfer round trip)",
        withdraw_costs
    );

    // --- the runway, at rates an operator can actually reason about -----------
    //
    // A "session" is priced rather than a hand: a player deposits once, plays, and
    // withdraws once. `HANDS_PER_DAY` is the operator's dial.
    eprintln!("\n=== RUNWAY ===");
    print_runway_table("idle, nobody at the table", idle_per_day);
    for hands_per_day in [50u128, 200, 1000] {
        let burn = idle_per_day + net_per_hand.saturating_mul(hands_per_day);
        print_runway_table(&format!("{hands_per_day} hands/day, no tabs open"), burn);
    }
    for seats in [2u128, 6] {
        let burn = idle_per_day
            + net_per_hand.saturating_mul(500)
            + hb_per_player_day.saturating_mul(seats);
        print_runway_table(
            &format!("500 hands/day with {seats} tabs open (heartbeats at 10 s)"),
            burn,
        );
    }
    print_runway_table(
        "dealing continuously, as measured above",
        loaded_per_day,
    );

    eprintln!(
        "\n  per-unit costs: deposit {deposit_avg}, hand {net_per_hand} (net) / {raw_per_hand} \
         (raw), withdrawal {withdraw_avg}, heartbeat {per_heartbeat}\n"
    );

    // --- the assertions ------------------------------------------------------
    assert!(
        net_per_hand > 0,
        "a hand measured as costing NOTHING. Either the load never ran or the cycle balance is \
         not moving in this environment, and every runway figure in this file is then a \
         statement about an instrument rather than about the canister."
    );
    assert!(
        loaded_per_day > idle_per_day,
        "a table playing hands burned no more than the same table sitting empty ({loaded_per_day} \
         vs {idle_per_day} cycles/day). That cannot be true, so the measurement is wrong."
    );
    // The headline claim of E-55's runway table is that idle is the OPTIMISTIC
    // case. Pin it: if a busy table were cheaper than an idle one, the whole
    // remediation would be aimed at the wrong number.
    assert!(
        net_per_hand > 1_000_000,
        "a hand cost {net_per_hand} cycles net, which is less than a single ingress message. \
         The hands almost certainly did not play."
    );
}

// ===========================================================================
// 2 -- FROZEN: WHAT ACTUALLY HAPPENS, AND WHETHER IT COMES BACK
// ===========================================================================

/// **FINDING 24, ported into the tree.** The finding says a probe was written
/// outside this repository and belongs in it, because the sentence it falsifies is
/// repeated in four places and is the basis of the whole cycles plan.
///
/// Two separate claims are measured here, and they are the two an operator has to
/// know apart:
///
/// 1. **Below the freezing threshold, the canister answers NOTHING.** Not "queries
///    keep working". Not "degraded". Update calls AND query calls are rejected at
///    the boundary, before any canister code runs. So `get_cycle_status` -- the
///    endpoint built to raise the alarm -- goes dark at the exact moment the alarm
///    is true, and any watcher that only alerts on a low number it manages to READ
///    will report nothing at all. Unreachability must itself be the alarm.
///
/// 2. **It is fully recoverable.** Freezing does not destroy state. Topping the
///    canister back up (or lowering the threshold) restores every method and every
///    balance, to the e8. That is the difference between this and running to true
///    zero, at which point the IC uninstalls the module and the state is gone --
///    which is why the alarm has to fire with days of margin and not hours.
#[test]
fn a_frozen_table_answers_nothing_and_is_fully_recoverable() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    world.fund_escrow(alice, 10 * ICP).expect("alice deposit");
    world.fund_escrow(bob, 10 * ICP).expect("bob deposit");
    quiet(&world, Duration::from_secs(600), Duration::from_secs(10), 4);

    let before_balance = world.get_balance(alice);
    let before_status = cycle_status(&world);
    let cycles = world.pic.cycle_balance(world.table);
    eprintln!(
        "\n=== BEFORE FREEZING ===\n  alice escrow={before_balance} e8s, canister holds \
         {cycles} cycles\n  get_cycle_status: liquid={} reserved={} runway_days={:?}",
        before_status.liquid_balance,
        before_status.reserved_for_freezing,
        before_status.runway_days
    );
    assert!(before_balance > 0, "alice must have money in the canister");

    // --- how big is the reserve, really, in units of the burn we measured? ----
    //
    // The freezing threshold is `freezing_threshold_seconds x IDLE resource
    // consumption`, and idle resource consumption counts memory and compute
    // ALLOCATION only. It knows nothing about a timer that sends messages. So the
    // reserve is nominally 30 days of storage-only burn while the canister is in
    // fact burning ~630x that. This prints the ratio rather than asserting a
    // constant, because the constant is a system parameter that can change.
    let observed_per_day = if before_status.observed_burn_per_day > 0 {
        before_status.observed_burn_per_day
    } else {
        1
    };
    let reserve_hours = (before_status.reserved_for_freezing * 24) / observed_per_day.max(1);
    eprintln!(
        "  the freezing RESERVE ({} cycles) is nominally 30 days of storage-only burn.\n  At the \
         burn this canister actually shows ({} cycles/day) it is worth ~{reserve_hours} HOURS.",
        before_status.reserved_for_freezing, before_status.observed_burn_per_day
    );

    // --- freeze it -----------------------------------------------------------
    //
    // Raising `freezing_threshold` puts the canister in exactly the state it
    // reaches by burning its balance down to the reserve, and unlike burning it
    // down it is reversible on the same instance, so recovery is provable here too.
    //
    // THE THRESHOLD HAS TO BE BIG ENOUGH, and "big" here is not intuitive. The
    // reserve is `freezing_threshold_seconds x IDLE resource consumption`, and
    // idle consumption on this canister is about 860 cycles/second -- storage,
    // nothing else, because the freezing formula knows nothing about a timer that
    // sends messages. The first version of this test used 400,000,000 seconds,
    // which reserves ~0.35 T against a balance of ~100 T, and the canister
    // cheerfully answered everything. It PASSED nothing and it would have
    // "disproved" FINDING 24 if the assertion had been the other way round.
    // Derived from the balance instead of guessed:
    let idle_per_sec = (before_status.reserved_for_freezing / 2_592_000).max(1); // 30-day default
    let huge_threshold: u64 = (((cycles / idle_per_sec) * 4) as u64).max(400_000_000);
    eprintln!(
        "  freezing by raising the threshold to {huge_threshold} s (idle ~{idle_per_sec} \
         cycles/s, so the reserve becomes ~{} T against a balance of {:.2} T)",
        (idle_per_sec * huge_threshold as u128) / T,
        cycles as f64 / 1e12
    );
    world
        .pic
        .update_canister_settings(
            world.table,
            Some(world.controller),
            CanisterSettings {
                freezing_threshold: Some(huge_threshold.into()),
                ..Default::default()
            },
        )
        .expect("raising freezing_threshold must be accepted from the controller");
    world.pic.tick();

    eprintln!("\n=== FROZEN ===");
    let mut query_rejections = 0usize;
    let mut update_rejections = 0usize;

    for method in [
        "get_cycle_status",
        "get_table_state",
        "get_balance",
        "get_custody_status",
        "get_stuck_hand_status",
    ] {
        let out = world
            .pic
            .query_call(world.table, alice, method, Encode!().unwrap());
        match out {
            Ok(_) => eprintln!("  QUERY  {method:<22} -> ANSWERED"),
            Err(e) => {
                query_rejections += 1;
                eprintln!(
                    "  QUERY  {method:<22} -> REJECTED: {}",
                    format!("{e:?}").chars().take(110).collect::<String>()
                );
            }
        }
    }

    for (method, arg) in [
        ("withdraw", Encode!(&(1_000_000u64)).unwrap()),
        ("cash_out", Encode!().unwrap()),
        ("check_timeouts", Encode!().unwrap()),
        ("abandon_stuck_hand", Encode!().unwrap()),
    ] {
        let out = world.pic.update_call(world.table, alice, method, arg);
        match out {
            Ok(_) => eprintln!("  UPDATE {method:<22} -> ACCEPTED"),
            Err(e) => {
                update_rejections += 1;
                eprintln!(
                    "  UPDATE {method:<22} -> REJECTED: {}",
                    format!("{e:?}").chars().take(110).collect::<String>()
                );
            }
        }
    }

    assert_eq!(
        update_rejections, 4,
        "a frozen canister must reject every update call. {update_rejections}/4 were rejected, so \
         either the freeze did not take or the IC's behaviour has changed and E-55's premise \
         needs re-deriving."
    );
    assert_eq!(
        query_rejections, 5,
        "THE LOAD-BEARING SENTENCE OF THE CYCLES PLAN. {query_rejections}/5 queries were \
         rejected. If queries keep answering, an off-chain watcher polling get_cycle_status is a \
         sufficient alarm; if they do not -- which is FINDING 24 -- then unreachability itself \
         has to be the alarm, and every monitor in this repo is built on that assumption."
    );

    // --- and it comes back ---------------------------------------------------
    world
        .pic
        .update_canister_settings(
            world.table,
            Some(world.controller),
            CanisterSettings {
                freezing_threshold: Some(2_592_000u64.into()), // the 30-day default
                ..Default::default()
            },
        )
        .expect("lowering freezing_threshold must be accepted");
    world.pic.tick();

    let after_balance = world.get_balance(alice);
    let after_status = cycle_status(&world);
    eprintln!(
        "\n=== AFTER TOP-UP (threshold restored) ===\n  alice escrow={after_balance} e8s, \
         get_cycle_status answers again (liquid={})",
        after_status.liquid_balance
    );
    assert_eq!(
        after_balance, before_balance,
        "freezing must not move a single e8. Before {before_balance}, after {after_balance}."
    );
    let withdrawn = world
        .withdraw(alice, 2 * ICP)
        .expect("after a top-up, withdraw must work again: freezing is recoverable, and if it is \
                 not then the remedy in E-55 does not exist");
    eprintln!("  withdraw after recovery: block {withdrawn}\n");
}

// ===========================================================================
// 3 -- IS THE GAUGE HONEST UNDER THE LOAD IT EXISTS FOR?
// ===========================================================================

/// **The gauge is the subject here, not the instrument.**
///
/// `get_cycle_status` measures its own burn rate between two samples taken at
/// clock ticks. Which two samples it picks is the whole question. The shipped
/// version anchored `CYCLES_ORIGIN` at the FIRST tick of the instance and moved it
/// only on a top-up, so `observed_burn_per_day` was a **lifetime average from
/// install** and `runway_days` was computed from it alone.
///
/// A lifetime average is the one shape a fuel gauge must not have. A table sits
/// empty for months at the idle rate, then fills with players -- or gets pointed at
/// by the free permissionless ingress flood of
/// [FINDING 26](../../../docs/SECURITY-FINDINGS.md#finding-26) -- and the lifetime
/// average barely moves, because the months of quiet are still in it. The gauge
/// goes on reporting the runway of a table nobody was using. That is exactly the
/// failure the source names as the only one that matters:
///
/// > a cycles gauge that reads high is the one failure mode that matters here: it
/// > is the gauge saying "plenty of fuel" to a canister that is about to stop
/// > honouring withdrawals.
///
/// # The comparison has to be over the SAME window, and the first version was not
///
/// The first version of this test ran a burst of hands, waited, and compared the
/// gauge's one-hour window against an external measurement over a *different*,
/// shorter window that happened to be mostly burst. It reported a 2.24x
/// "overstatement" and would have failed any honest implementation: two averages
/// over two different windows are not the same quantity, and a gauge that tracked
/// a 40-second burst instantly would be a worse gauge, not a better one.
///
/// So the load here is **sustained** for longer than the gauge's own window, and
/// the external truth is measured over exactly the window the gauge reports. What
/// is being asserted is the property that matters to an operator: a table whose
/// burn rate has CHANGED AND STAYED CHANGED must say so, within an hour.
#[test]
fn the_gauge_reads_high_after_a_quiet_start() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);

    // --- a long quiet start, so the lifetime average is an IDLE average -------
    const QUIET_HOURS: u64 = 4;
    quiet(
        &world,
        Duration::from_secs(3_600 * QUIET_HOURS),
        Duration::from_secs(10),
        4,
    );
    let quiet_status = cycle_status(&world);
    eprintln!(
        "\n=== AFTER {QUIET_HOURS} QUIET HOURS ===\n  observed_burn_per_day={} \
         recent_burn_per_day={} runway_days={:?} window={}s recent_window={}s",
        quiet_status.observed_burn_per_day,
        recent_burn(&quiet_status),
        quiet_status.runway_days,
        quiet_status.sample_window_secs,
        quiet_status.recent_window_secs.unwrap_or(0)
    );
    assert!(
        quiet_status.measurement_is_meaningful,
        "hours of uptime must be enough for the gauge to say something"
    );
    let idle_gauge = quiet_status.observed_burn_per_day;

    // --- then SUSTAINED load, for longer than the gauge's own window ----------
    let names = ["alice", "bob", "carol"];
    for (i, name) in names.iter().enumerate() {
        let who = world.actor(name);
        world.fund_escrow(who, 500 * ICP).expect("deposit");
        world.join_table(who, i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));

    let post_flop_bet = 2 * world.config.big_blind;
    // A hand every ~2 simulated minutes for ~75 simulated minutes: an ordinary
    // busy table, and long enough that the gauge's one-hour window is entirely
    // inside the loaded period by the end.
    const LOAD_ROUNDS: usize = 38;
    const GAP_SECS: u64 = 110;
    let mut hands = 0usize;
    for _ in 0..LOAD_ROUNDS {
        refill_seats(&world, &names);
        if can_deal(&world) {
            let _ = scenario::play_one_hand(&mut world, post_flop_bet);
            hands += 1;
        }
        gap(&world, &names, GAP_SECS);
    }
    assert!(
        hands >= LOAD_ROUNDS / 2,
        "only {hands} of {LOAD_ROUNDS} rounds dealt a hand, so the 'sustained load' was not \
         sustained and the comparison below is not the one this test claims to make"
    );

    // --- the truth, over EXACTLY the window the gauge reports ------------------
    let loaded_status = cycle_status(&world);
    let window = loaded_status.recent_window_secs.unwrap_or(0).max(1);
    let t_end = world.now_nanos();
    let bal_end = world.pic.cycle_balance(world.table);
    // Replay the same span backwards is impossible, so the window is measured
    // forwards from a mark taken `window` seconds ago -- which is what the
    // bookkeeping below reconstructs: the load has been running for longer than
    // `window`, so any sub-window of it is a sample of the same steady state.
    // Measure one more `window` of the SAME load and compare that.
    let mut hands_in_window = 0usize;
    let rounds = (window / GAP_SECS).max(1) as usize;
    for _ in 0..rounds {
        refill_seats(&world, &names);
        if can_deal(&world) {
            let _ = scenario::play_one_hand(&mut world, post_flop_bet);
            hands_in_window += 1;
        }
        gap(&world, &names, GAP_SECS);
    }
    let measured_secs = (world.now_nanos() - t_end) / SEC;
    let measured_burn = bal_end.saturating_sub(world.pic.cycle_balance(world.table));
    let truth_per_day = per_day(measured_burn, measured_secs);

    let final_status = cycle_status(&world);
    eprintln!(
        "\n=== UNDER SUSTAINED LOAD ===\n  {hands} hands over the ramp, {hands_in_window} more \
         over the {measured_secs}s comparison window\n  TRUTH (external, same window): \
         {truth_per_day} cycles/day ({:.4} T/day)\n  gauge recent  : {} cycles/day (window \
         {}s)\n  gauge lifetime: {} cycles/day\n  idle, before the load: {idle_gauge} \
         cycles/day",
        t_per_day(truth_per_day),
        recent_burn(&final_status),
        final_status.recent_window_secs.unwrap_or(0),
        final_status.observed_burn_per_day
    );

    let honest_runway = runway_days(final_status.liquid_balance, truth_per_day)
        .expect("the external measurement produced a zero burn rate, which cannot be right");
    let reported = final_status
        .runway_days
        .expect("the gauge reported no runway at all after hours of uptime and real load")
        as u128;
    // What the gauge would have said WITHOUT the sliding window -- i.e. the number
    // this canister shipped before this change. Computed from the field the old
    // code used, so it is not a guess about the old behaviour.
    let lifetime_only = runway_days(final_status.liquid_balance, final_status.observed_burn_per_day)
        .unwrap_or(u128::MAX);

    eprintln!(
        "  runway the canister REPORTS      : {reported} days\n  runway the load implies      \
         : {honest_runway} days\n  runway a LIFETIME AVERAGE gives  : {lifetime_only} days  \
         <- what shipped before this change\n"
    );

    // --- the gates ------------------------------------------------------------
    //
    // A fuel gauge may read low: that costs an operator an unnecessary top-up. It
    // must not read high, because reading high costs every player at the table
    // access to their own money on a day nobody predicted.
    assert!(
        reported <= honest_runway.saturating_mul(3) / 2,
        "THE GAUGE READS HIGH. It reports {reported} days of runway on a table whose burn over \
         the same window it is averaging implies {honest_runway}. docs/DEFECTS.md E-55."
    );
    // And the sliding window has to be doing the work. If this fails, the fix is
    // inert and the test above is passing for some other reason.
    assert!(
        recent_burn(&final_status) > final_status.observed_burn_per_day,
        "the one-hour window ({}) is not above the lifetime average ({}) on a table that was \
         quiet for {QUIET_HOURS} hours and has been busy for the last two. The window is not a \
         window, and runway_days is still a lifetime average wearing a new name.",
        recent_burn(&final_status),
        final_status.observed_burn_per_day
    );
    assert!(
        lifetime_only > reported,
        "the lifetime-average runway ({lifetime_only}) is not worse than the shipped one \
         ({reported}), so this scenario does not actually exercise the defect and the gate is \
         proving nothing"
    );
    assert!(
        recent_burn(&final_status) > idle_gauge,
        "the gauge did not notice the load at all: recent burn {} vs the idle rate {idle_gauge} \
         it was reporting before three players sat down",
        recent_burn(&final_status)
    );
}

// ===========================================================================
// 4 -- THE NUMBER AN OPERATOR HAS TO ACT ON
// ===========================================================================

/// Not a defect gate: a report, printed so the runway figures in
/// `docs/DEFECTS.md` E-55 are reproducible from a command rather than quoted from
/// a wave that is over.
///
/// It asserts only the one thing that must never stop being true: that the warning
/// threshold the frontend and the CI job use leaves an operator more than a
/// weekend to act at the burn rates this file measures.
#[test]
fn runway_at_real_balances() {
    // Kept equal to `WARN_RUNWAY_DAYS` in
    // `src/cleardeck_frontend/src/lib/cycleRunway.js` and `CRITICAL_DAYS` /
    // `WARN_DAYS` in `scripts/cycles-runway.mjs`.
    const WARN_DAYS: u128 = 60;
    const CRITICAL_DAYS: u128 = 21;

    let world = World::new(TableConfig::six_max_icp(), &["alice"]);
    quiet(&world, Duration::from_secs(120), Duration::from_secs(10), 4);
    let t0 = world.now_nanos();
    let before = world.pic.cycle_balance(world.table);
    quiet(&world, Duration::from_secs(3600), Duration::from_secs(10), 4);
    let secs = (world.now_nanos() - t0) / SEC;
    let idle = per_day(before.saturating_sub(world.pic.cycle_balance(world.table)), secs);

    eprintln!("\n=== E-55 RUNWAY TABLE, REPRODUCED ===");
    print_runway_table("idle (the on-chain clock alone)", idle);

    // The days between the warning firing and the canister freezing, at idle.
    let warn_at = idle.saturating_mul(WARN_DAYS);
    let crit_at = idle.saturating_mul(CRITICAL_DAYS);
    eprintln!(
        "\n  warning fires below {warn_at} cycles liquid ({:.4} T) = {WARN_DAYS} days at idle\n  \
         critical fires below {crit_at} cycles liquid ({:.4} T) = {CRITICAL_DAYS} days at idle\n",
        warn_at as f64 / 1e12,
        crit_at as f64 / 1e12
    );

    assert!(idle > 0, "the idle burn measured as zero");
    assert!(
        CRITICAL_DAYS >= 14,
        "the critical threshold must leave a human more than a weekend. Under \
         docs/SECURITY-FINDINGS.md FINDING 26 a hostile ingress flood compresses whatever margin \
         this buys by ~65x, so anything under two weeks at idle is under five hours under attack."
    );
    assert!(
        WARN_DAYS > CRITICAL_DAYS,
        "the warning must fire before the critical threshold"
    );
}
