#!/usr/bin/env node
// WHAT DOES ONE OPEN BROWSER TAB COST A TABLE CANISTER, PER DAY?
//
// ===========================================================================
// WHY THIS EXISTS (docs/DEFECTS.md E-92)
// ===========================================================================
//
// Every cycles figure in this tree was derived by adding up the price of the
// things somebody REMEMBERED the frontend does: a hand, a deposit, a withdrawal
// and the 10-second heartbeat. Nobody priced the 500 ms poll, whose first
// statement was `await tableActor.check_timeouts()` -- an `#[ic_cdk::update]`
// with no `query` in `table_canister.did`. An update loop, twice a second, per
// open tab, and no document in the repository counted it.
//
// A derived figure is a figure that omits whatever the deriver forgot. So this
// does not derive anything. It opens N simulated tabs against a REAL canister on
// the LOCAL replica, drives exactly the call pattern the page drives, and reads
// the canister's own cycle balance before and after. The number it prints is a
// measured difference in a balance the canister does not control, which is the
// only kind of cycles number this project should publish.
//
// ===========================================================================
// THE METHOD, SO IT CAN BE RE-RUN AND DISAGREED WITH
// ===========================================================================
//
//   1. Read `get_cycle_status` (a query -- costs the canister nothing) and take
//      `balance` and the wall clock. This is B0/t0.
//   2. Start N tabs. Each tab is its OWN Ed25519 identity, because a real tab is
//      a real principal and per-caller rate limits are per principal.
//   3. Run for --seconds. Every call each tab makes is counted, by method.
//   4. Stop the tabs, then WAIT --settle seconds with nothing running, so that
//      messages already in flight are charged inside the measured window rather
//      than after it.
//   5. Read `get_cycle_status` again. B1/t1.
//   6. burn/day = (B0 - B1) / (t1 - t0) * 86400.
//
// The N=0 run is the control: the canister's on-chain clock burns cycles whether
// or not anybody is looking, so the MARGINAL cost of a tab is
// (burn at N) - (burn at 0), divided by N. Both halves are printed; the control
// is never assumed.
//
// TWO CONFOUNDS, STATED RATHER THAN HIDDEN:
//
//   * Local replica update finality is much faster than mainnet's ~2 s. The poll
//     is re-entrancy-guarded (`loadingTableState`), so its real rate is one call
//     per round trip, NOT two per second. This harness therefore reports
//     CYCLES PER CALL as well as cycles per day, and prints the mainnet-paced
//     figure alongside the measured one. Cycles per call is the portable number;
//     calls per day depends on where you run it.
//   * A local replica's per-message prices are not guaranteed identical to
//     mainnet's. The RATIO between modes -- which is what E-92 is about -- is
//     robust to that; the absolute figure is a local measurement and is labelled
//     as one.
//
// ===========================================================================
// USAGE
// ===========================================================================
//
//   node tools/cycles/tab-burn.mjs --tabs 0 --seconds 120
//   node tools/cycles/tab-burn.mjs --tabs 10 --seconds 120 --mode legacy
//   node tools/cycles/tab-burn.mjs --tabs 10 --seconds 120 --mode fixed
//   node tools/cycles/tab-burn.mjs --sweep 0,1,3,10 --seconds 120 --mode both
//
//   --mode legacy   the poll as it was before E-92 was fixed: check_timeouts()
//                   (an UPDATE) as the first statement of every 500 ms poll.
//   --mode fixed    the poll as it is now: queries only on the render loop, with
//                   the clock advanced by tools/../lib/clockNudge.js's policy.
//
// LOCAL ONLY. There is no --network flag and no mainnet id anywhere in this
// file, on purpose.

import { Actor, HttpAgent } from '@dfinity/agent';
import { Ed25519KeyIdentity } from '@dfinity/identity';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');

const { idlFactory } = await import(
    path.join(ROOT, 'src/declarations/table_1/table_1.did.js'));
const { ClockNudgePolicy, CLOCK_NUDGE } = await import(
    path.join(ROOT, 'src/cleardeck_frontend/src/lib/clockNudge.js'));

// ---------------------------------------------------------------------------
// arguments
// ---------------------------------------------------------------------------
const argv = process.argv.slice(2);
const arg = (name, dflt) => {
    const i = argv.indexOf(`--${name}`);
    return i === -1 ? dflt : argv[i + 1];
};

const HOST = arg('host', 'http://localhost:8077');
const SECONDS = Number(arg('seconds', 120));
const SETTLE = Number(arg('settle', 10));
const MODE = arg('mode', 'legacy');
const SWEEP = arg('sweep', null);
const TABS = Number(arg('tabs', 1));
const OUT = arg('out', null);
const CANISTER = arg('canister', null);

// The render-loop period and the two background periods, copied from
// src/cleardeck_frontend/src/routes/+page.svelte. They are asserted against that
// file by tools/shots/test-poll-updates.mjs, so a drift here is caught there.
const POLL_INTERVAL = 500;
const HEARTBEAT_INTERVAL = 10_000;
const BALANCE_REFRESH_INTERVAL = 5_000;

if (/\bic\b/.test(HOST) && !HOST.includes('localhost') && !HOST.includes('127.0.0.1')) {
    console.error('this harness is local-only; refusing host ' + HOST);
    process.exit(2);
}

function localCanisterId() {
    if (CANISTER) return CANISTER;
    const map = JSON.parse(fs.readFileSync(
        path.join(ROOT, '.icp/cache/mappings/local.ids.json'), 'utf8'));
    // table_3 by default: `dev.sh shots` drives table_1 and table_2, and a
    // measurement whose canister somebody else is also poking is not a
    // measurement.
    const id = map.table_3 || map.table_1;
    if (!id) throw new Error('no local table id in .icp/cache/mappings/local.ids.json');
    return id;
}

const CANISTER_ID = localCanisterId();

async function makeActor(identity) {
    const agent = await HttpAgent.create({ host: HOST, identity, shouldFetchRootKey: true });
    return Actor.createActor(idlFactory, { agent, canisterId: CANISTER_ID });
}

const anonActor = await makeActor(undefined);

async function readBalance() {
    const s = await anonActor.get_cycle_status();
    return {
        balance: BigInt(s.balance),
        clockTicks: BigInt(s.clock_ticks),
        at: Date.now(),
    };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// ONE TAB
// ---------------------------------------------------------------------------
//
// This is deliberately a transcription of `+page.svelte`, not an abstraction of
// it. The re-entrancy guard, the ordering, the `Promise.all` of the two queries
// and the two background intervals are all as the page has them, because the
// thing being priced is the page and not an idealisation of it.
class Tab {
    constructor(actor, mode, counts) {
        this.actor = actor;
        this.mode = mode;
        this.counts = counts;
        this.loading = false;
        this.timers = [];
        this.view = null;
        this.policy = new ClockNudgePolicy();
        this.stopped = false;
        this.nudging = false;
    }

    bump(method) {
        this.counts[method] = (this.counts[method] || 0) + 1;
    }

    async loadTableState() {
        if (this.loading || this.stopped) return;
        this.loading = true;
        try {
            if (this.mode === 'legacy') {
                // THE DEFECT: an UPDATE call as the first statement of a 500 ms poll.
                this.bump('check_timeouts');
                await this.actor.check_timeouts();
            }
            const [viewResult] = await Promise.all([
                this.actor.get_table_view(),
                this.actor.get_shuffle_proof(),
            ]);
            this.bump('get_table_view');
            this.bump('get_shuffle_proof');
            this.view = (viewResult && viewResult.length > 0) ? viewResult[0] : null;
        } catch (e) {
            this.bump('poll_error');
        } finally {
            this.loading = false;
        }
    }

    // The fixed mode's clock driver. Conditional and rate-limited: it asks the
    // shared policy whether a deadline is actually crossable, and the policy
    // refuses to fire faster than its floor however often it is asked.
    //
    // THE TWO SYNTHETIC MODES ARE NOT CHEATING, THEY ARE THE CEILING.
    //
    // An idle table is the easy case for this policy and measuring only that
    // would flatter it — which is precisely the error E-92 is about. So:
    //
    //   fixed-stuck  the table is permanently "due" and never moves: a table that
    //                is between hands with two dealable seats forever because
    //                `start_new_hand` keeps failing. The backoff should walk the
    //                gap out to MAX_GAP_MS and hold it there.
    //   fixed-max    "due" AND the table moves on every call, so the backoff never
    //                engages and every tick fires at MIN_GAP_MS. This is the
    //                absolute most this policy can ever cost a canister per tab,
    //                and it is the figure the runway numbers are set from.
    async nudgeClock() {
        if (this.stopped || this.nudging) return;
        // A view that is always "due": between hands with two dealt-in seats.
        const stuckView = {
            hand_number: 1n,
            phase: { HandComplete: null },
            action_on: 0,
            pot: 0n,
            hand_is_unmovable: false,
            players: [
                [{ status: { Active: null }, chips: 100n }],
                [{ status: { Active: null }, chips: 100n }],
            ],
        };
        let view = this.view;
        if (this.mode === 'fixed-stuck') view = stuckView;
        if (this.mode === 'fixed-max') {
            // Same, but the fingerprint changes every time, so the backoff never
            // engages: the table really is moving on every call.
            view = { ...stuckView, hand_number: BigInt(this.policy.calls + 1) };
        }
        const decision = this.policy.decide(view, Date.now());
        if (!decision.call) return;
        this.nudging = true;
        try {
            this.bump('check_timeouts');
            const r = await this.actor.check_timeouts();
            this.policy.observe(view, r, Date.now());
        } catch (e) {
            this.bump('nudge_error');
            this.policy.observe(view, null, Date.now());
        } finally {
            this.nudging = false;
        }
    }

    async heartbeat() {
        if (this.stopped) return;
        try {
            this.bump('heartbeat');
            await this.actor.heartbeat();
        } catch (e) { /* best effort, exactly as the page has it */ }
    }

    async balance() {
        if (this.stopped) return;
        try {
            this.bump('get_balance');
            await this.actor.get_balance();
        } catch (e) { /* ignore */ }
    }

    start() {
        this.loadTableState();
        this.heartbeat();
        this.balance();
        this.timers.push(setInterval(() => this.loadTableState(), POLL_INTERVAL));
        this.timers.push(setInterval(() => this.heartbeat(), HEARTBEAT_INTERVAL));
        this.timers.push(setInterval(() => this.balance(), BALANCE_REFRESH_INTERVAL));
        if (this.mode.startsWith('fixed')) {
            // Its own timer, at the policy's floor, so the render rate and the
            // clock rate are two different numbers -- which is the whole fix.
            this.timers.push(setInterval(() => this.nudgeClock(), CLOCK_NUDGE.TICK_MS));
        }
    }

    stop() {
        this.stopped = true;
        for (const t of this.timers) clearInterval(t);
        this.timers = [];
    }
}

// ---------------------------------------------------------------------------
// ONE MEASUREMENT
// ---------------------------------------------------------------------------
async function measure(nTabs, mode) {
    const counts = {};
    const tabs = [];
    for (let i = 0; i < nTabs; i++) {
        const identity = Ed25519KeyIdentity.generate();
        tabs.push(new Tab(await makeActor(identity), mode, counts));
    }

    // Quiesce: nothing running, so the "before" reading is not contaminated by
    // the previous run's tail.
    await sleep(2000);
    const before = await readBalance();

    for (const t of tabs) t.start();
    await sleep(SECONDS * 1000);
    for (const t of tabs) t.stop();

    // In-flight messages are charged when they execute, not when they were sent.
    await sleep(SETTLE * 1000);
    const after = await readBalance();

    const spentCycles = before.balance - after.balance;
    const elapsedMs = after.at - before.at;
    const perDay = Number(spentCycles) * 86_400_000 / elapsedMs;

    return {
        tabs: nTabs,
        mode: nTabs === 0 ? 'control' : mode,
        elapsed_s: +(elapsedMs / 1000).toFixed(1),
        cycles_spent: spentCycles.toString(),
        burn_per_day: Math.round(perDay),
        burn_per_day_T: +(perDay / 1e12).toFixed(4),
        clock_ticks: Number(after.clockTicks - before.clockTicks),
        calls: { ...counts },
        update_calls: (counts.check_timeouts || 0) + (counts.heartbeat || 0),
    };
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------
const runs = [];
const plan = [];
if (SWEEP) {
    const ns = SWEEP.split(',').map((s) => Number(s.trim()));
    const modes = MODE === 'both' ? ['legacy', 'fixed'] : [MODE];
    for (const n of ns) {
        if (n === 0) { plan.push([0, modes[0]]); continue; }
        for (const m of modes) plan.push([n, m]);
    }
} else {
    plan.push([TABS, MODE]);
}

console.log(`# tab-burn: canister ${CANISTER_ID} on ${HOST}`);
console.log(`# ${plan.length} run(s) x ${SECONDS}s + ${SETTLE}s settle`
    + `  (~${Math.ceil(plan.length * (SECONDS + SETTLE + 4) / 60)} min)`);
console.log('');

for (const [n, m] of plan) {
    process.stdout.write(`  measuring ${n} tab(s), mode=${n === 0 ? 'control' : m} ... `);
    const r = await measure(n, m);
    runs.push(r);
    console.log(`${r.burn_per_day_T} T/day  (${r.update_calls} update calls in ${r.elapsed_s}s)`);
}

// -------- report -----------------------------------------------------------
const control = runs.find((r) => r.tabs === 0);
console.log('');
console.log('  tabs  mode     burn T/day   marginal T/day/tab   updates   cycles/update');
console.log('  ----  -------  ----------   ------------------   -------   -------------');
for (const r of runs) {
    const marginal = control && r.tabs > 0
        ? (r.burn_per_day - control.burn_per_day) / r.tabs / 1e12
        : null;
    const perUpdate = r.update_calls > 0 && control
        ? Math.round((r.burn_per_day - control.burn_per_day) * (r.elapsed_s / 86400)
            / r.update_calls)
        : null;
    console.log(`  ${String(r.tabs).padStart(4)}  ${r.mode.padEnd(7)}  `
        + `${r.burn_per_day_T.toFixed(4).padStart(10)}   `
        + `${(marginal === null ? '-' : marginal.toFixed(4)).padStart(18)}   `
        + `${String(r.update_calls).padStart(7)}   `
        + `${(perUpdate === null ? '-' : perUpdate.toLocaleString('en-US')).padStart(13)}`);
}

const report = {
    measured_at: new Date().toISOString(),
    host: HOST,
    canister: CANISTER_ID,
    seconds: SECONDS,
    settle: SETTLE,
    runs,
};
if (OUT) {
    fs.writeFileSync(OUT, JSON.stringify(report, null, 2));
    console.log(`\n  wrote ${OUT}`);
}
process.exit(0);
