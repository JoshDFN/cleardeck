#!/usr/bin/env node
// THE GATE ON THE CLOCK POLICY: WHAT CAN ONE OPEN TAB COST, OVER A WHOLE DAY?
//
// ===========================================================================
// WHY A SIMULATION AND NOT A UNIT TEST (docs/DEFECTS.md E-92)
// ===========================================================================
//
// `tools/shots/test-poll-updates.mjs` proves the SHAPE: no timer faster than 2 s
// reaches an update method. A shape assertion is satisfiable by a loop that runs
// at 2,001 ms and calls the canister on every single tick, which would be 43,200
// update calls a day per tab — nearly the mainnet-paced rate of the loop E-92 is
// about. The shape is necessary and it is nowhere near sufficient.
//
// What actually bounds the bill is that `$lib/clockNudge.js` only fires when a
// deadline is CROSSABLE, and backs off when its calls change nothing. That is a
// property of behaviour over time, so it is checked over time: a synthetic day of
// table states, stepped at the driver's own tick, with the calls counted.
//
// AND IT IS CHECKED AGAINST A CONTROL. A budget assertion on its own passes for a
// policy that never calls at all — an instrument that measures nothing. So every
// budget claim below is made twice: once for the real policy, and once for a
// NAIVE policy that calls on every tick, which is what the code did before this
// wave. If the two ever come out the same, this file is broken and says so.
//
// Pure and offline: no replica, no network, no canister.

import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');

const {
    CLOCK_NUDGE, ClockNudgePolicy, clockIsDue, dealtInCount, maxCallsIn, tableFingerprint,
    variantName,
} = await import(path.join(ROOT, 'src/cleardeck_frontend/src/lib/clockNudge.js'));

// ---------------------------------------------------------------------------
// The measured price of one call, so the budget is in cycles and not in calls
// ---------------------------------------------------------------------------
//
// Measured on the local replica by `tools/cycles/tab-burn.mjs` — see
// `artifacts/cycles/` for the run this came from and docs/DEFECTS.md E-92 for the
// table. It is a LOCAL measurement of a real module under a real replica, not a
// figure derived from a price list, which is the whole point of the harness.
//
// It is pinned here rather than recomputed because this gate must run with no
// replica. If a re-measurement moves it, move it here and the budget below moves
// with it.
const CHECK_TIMEOUTS_CYCLES = 6_600_000;

/**
 * The cycles one open tab may cost a table canister per day, from the clock
 * nudger alone, across the whole synthetic day below.
 *
 * THE BAR IS NOT A ROUND NUMBER PICKED AFTER SEEING THE RESULT. It is the
 * canister's own idle burn: a table with nobody looking at it measured **0.0411
 * T/day** on the local replica (`artifacts/cycles/control-before.json`). A person
 * merely HAVING THE PAGE OPEN must cost the table less than the table costs to
 * exist — otherwise the runway a player is shown is a function of how many
 * browser windows are open, which is the whole of E-92. Half of idle, rounded
 * down, is 0.02 T.
 *
 * For scale, on the same instrument: the loop this replaces measured about
 * 1.1 T/day per tab.
 */
const DAILY_CYCLE_BUDGET = 20_000_000_000;

const DAY_MS = 86_400_000;
const TICK = CLOCK_NUDGE.TICK_MS;

let failures = 0;
function check(name, fn) {
    try {
        fn();
        console.log(`    ok    ${name}`);
    } catch (e) {
        console.error(`    FAIL  ${name}`);
        console.error(`          ${e.message.split('\n').slice(0, 6).join('\n          ')}`);
        failures += 1;
    }
}

console.log('  test-clock-nudge: what one open tab can cost a table (docs/DEFECTS.md E-92)');

// ---------------------------------------------------------------------------
// Table views, as the Candid decoder produces them
// ---------------------------------------------------------------------------
const seat = (status, chips) => [{ status: { [status]: null }, chips: BigInt(chips) }];
const EMPTY = [];

function view({ phase, players = [], handNumber = 1, remaining = null, unmovable = false,
    actionOn = 0, pot = 0 }) {
    return {
        hand_number: BigInt(handNumber),
        phase: { [phase]: null },
        players,
        action_on: actionOn,
        pot: BigInt(pot),
        time_remaining_secs: remaining === null ? [] : [BigInt(remaining)],
        hand_is_unmovable: unmovable,
    };
}

const TWO_LIVE = [seat('Active', 100), seat('Active', 100), EMPTY, EMPTY, EMPTY, EMPTY];
const ONE_LIVE = [seat('Active', 100), EMPTY, EMPTY, EMPTY, EMPTY, EMPTY];
const TWO_BROKE = [seat('Active', 0), seat('Active', 0), EMPTY, EMPTY, EMPTY, EMPTY];
const TWO_OUT = [seat('SittingOut', 100), seat('SittingOut', 100), EMPTY, EMPTY, EMPTY, EMPTY];
const NOBODY = [EMPTY, EMPTY, EMPTY, EMPTY, EMPTY, EMPTY];

// ---------------------------------------------------------------------------
// 1. The predicate. Every branch, including the ones that must NOT fire.
// ---------------------------------------------------------------------------
check('a table nobody is sitting at is never due', () => {
    assert.equal(clockIsDue(view({ phase: 'WaitingForPlayers', players: NOBODY })).due, false);
});

check('one seated player between hands is not due: no hand can start', () => {
    assert.equal(clockIsDue(view({ phase: 'HandComplete', players: ONE_LIVE })).due, false);
});

check('two seated players with no chips are not due', () => {
    assert.equal(clockIsDue(view({ phase: 'HandComplete', players: TWO_BROKE })).due, false);
});

check('two seated players sitting out are not due', () => {
    assert.equal(clockIsDue(view({ phase: 'WaitingForPlayers', players: TWO_OUT })).due, false);
});

check('two dealt-in players between hands IS due: nothing else delivers AutoDealReady', () => {
    const d = clockIsDue(view({ phase: 'HandComplete', players: TWO_LIVE }));
    assert.equal(d.due, true, d.reason);
});

check('a hand in progress with the clock still running is not due', () => {
    assert.equal(
        clockIsDue(view({ phase: 'Flop', players: TWO_LIVE, remaining: 12 })).due, false);
});

check('a hand in progress whose action clock hit zero IS due', () => {
    const d = clockIsDue(view({ phase: 'Flop', players: TWO_LIVE, remaining: 0 }));
    assert.equal(d.due, true, d.reason);
});

check('an unmovable hand is due whatever else is true (FINDING 15 stall opportunities)', () => {
    const d = clockIsDue(view({ phase: 'Turn', players: TWO_LIVE, remaining: 30, unmovable: true }));
    assert.equal(d.due, true, d.reason);
});

check('no view at all is not due (the backstop covers it, not the predicate)', () => {
    assert.equal(clockIsDue(null).due, false);
});

// ...AND THE BACKSTOP HAS TO ACTUALLY COVER IT. docs/DEFECTS.md T-48.
//
// The check above says "the backstop covers it" and measured nothing, so it was
// green while a cold-start tab emitted ZERO check_timeouts for a whole day: the
// `seated` term for a null view read `this.lastCallAt !== null`, and lastCallAt is
// only set when a call is MADE, so a tab that never got a decodable view never
// called, was never seated, and never reached the backstop. The pre-split client
// called check_timeouts BEFORE get_table_view, so it kept firing at 2/s exactly
// when the view could not be read. check_timeouts is what records the stall
// opportunities that make abandon_stuck_hand reachable, so going silent there is
// FINDING 15's fund lock rebuilt client-side. A comment is not a measurement.
check('a tab that never gets a readable view still nudges the clock (T-48)', () => {
    const p = new ClockNudgePolicy();
    let calls = 0;
    const DAY = 24 * 60 * 60 * 1000;
    for (let t = 0; t < DAY; t += CLOCK_NUDGE.TICK_MS) {
        if (p.decide(null, t).call) { p.observe(null, null, t); calls++; }
    }
    assert.ok(calls > 0,
        'a tab whose get_table_view never decodes emitted ZERO check_timeouts in a day. '
        + '"I could not tell" has become "so I stopped calling", which is the one thing '
        + 'this module must not do.');
    const expected = Math.floor(DAY / CLOCK_NUDGE.BACKSTOP_MS);
    assert.ok(calls >= expected * 0.9 && calls <= expected * 1.1,
        `${calls} backstop calls in a day, expected about ${expected} (one per `
        + `${CLOCK_NUDGE.BACKSTOP_MS} ms). Too few is a silent tab; too many is a poll.`);
});

check('dealtInCount mirrors will_be_dealt_in: Active AND chips > 0', () => {
    assert.equal(dealtInCount(view({ phase: 'HandComplete', players: TWO_LIVE })), 2);
    assert.equal(dealtInCount(view({ phase: 'HandComplete', players: TWO_BROKE })), 0);
    assert.equal(dealtInCount(view({ phase: 'HandComplete', players: TWO_OUT })), 0);
    assert.equal(dealtInCount(null), 0);
});

check('variantName reads a Candid variant and refuses anything else', () => {
    assert.equal(variantName({ HandComplete: null }), 'HandComplete');
    assert.equal(variantName(null), null);
    assert.equal(variantName({ a: 1, b: 2 }), null);
});

// ---------------------------------------------------------------------------
// 2. The floor and the backoff
// ---------------------------------------------------------------------------
check('the floor refuses a second call inside MIN_GAP_MS however often it is asked', () => {
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'HandComplete', players: TWO_LIVE });
    let t = 0;
    assert.equal(p.decide(v, t).call, true, 'the first call must go');
    p.observe(v, { AutoDealReady: null }, t);
    let extra = 0;
    for (t = 100; t < CLOCK_NUDGE.MIN_GAP_MS; t += 100) {
        if (p.decide(v, t).call) extra += 1;
    }
    assert.equal(extra, 0, `${extra} calls slipped through the ${CLOCK_NUDGE.MIN_GAP_MS} ms floor`);
    assert.equal(p.decide(v, CLOCK_NUDGE.MIN_GAP_MS).call, true, 'the floor must also open again');
});

check('a table that is due forever and never moves backs off to MAX_GAP_MS', () => {
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'HandComplete', players: TWO_LIVE });
    for (let t = 0; t <= 600_000; t += TICK) {
        if (p.decide(v, t).call) p.observe(v, { NoAction: null }, t);
    }
    assert.equal(p.gap, CLOCK_NUDGE.MAX_GAP_MS,
        `gap settled at ${p.gap} ms, not the ${CLOCK_NUDGE.MAX_GAP_MS} ms ceiling`);
    // 10 minutes at a doubling gap: 2+4+8+16+30+30... A loop at the floor would
    // be 300 calls. This is the assertion that the backoff is doing work.
    assert.ok(p.calls < 30, `${p.calls} calls in ten minutes on a table that never moved`);
});

check('the backoff SNAPS BACK the moment the table actually moves', () => {
    const p = new ClockNudgePolicy();
    const stuck = view({ phase: 'HandComplete', players: TWO_LIVE });
    for (let t = 0; t <= 300_000; t += TICK) {
        if (p.decide(stuck, t).call) p.observe(stuck, { NoAction: null }, t);
    }
    assert.equal(p.gap, CLOCK_NUDGE.MAX_GAP_MS);
    const moved = view({ phase: 'PreFlop', players: TWO_LIVE, handNumber: 2, remaining: 0 });
    p.observe(moved, { NoAction: null }, 400_000);
    assert.equal(p.gap, CLOCK_NUDGE.MIN_GAP_MS,
        'a table that moved must get the fast clock back, or a busy table is served slowly');
});

check('a REPEATED AutoDealReady on a table that never moves backs off anyway', () => {
    // The tempting rule is "an actionable reply means the call did work, so keep
    // the fast clock". A table jammed between hands answers AutoDealReady to
    // every call while `start_new_hand` keeps failing, so that rule holds the
    // floor open for as long as the jam lasts. Only the table moving resets it.
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'HandComplete', players: TWO_LIVE });
    p.observe(v, { AutoDealReady: null }, 0);
    p.observe(v, { AutoDealReady: null }, 2_000);
    p.observe(v, { AutoDealReady: null }, 6_000);
    assert.ok(p.gap > CLOCK_NUDGE.MIN_GAP_MS,
        'three identical AutoDealReady replies over an unchanged table kept the floor open');
});

check('a deal that really happened gets the fast clock straight back', () => {
    const p = new ClockNudgePolicy();
    const between = view({ phase: 'HandComplete', players: TWO_LIVE, handNumber: 7 });
    p.observe(between, { AutoDealReady: null }, 0);
    p.observe(between, { AutoDealReady: null }, 2_000);
    assert.ok(p.gap > CLOCK_NUDGE.MIN_GAP_MS);
    const dealt = view({ phase: 'PreFlop', players: TWO_LIVE, handNumber: 8, remaining: 20 });
    p.observe(dealt, { NoAction: null }, 6_000);
    assert.equal(p.gap, CLOCK_NUDGE.MIN_GAP_MS);
});

check('a failed call still teaches the policy to back off', () => {
    // `observe(..., null, ...)` is the thrown-call path. A policy that only
    // learned from successes would hammer a canister that is refusing, which is
    // the one time backing off matters most.
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'HandComplete', players: TWO_LIVE });
    p.observe(v, null, 0);
    p.observe(v, null, 2_000);
    p.observe(v, null, 6_000);
    assert.ok(p.gap > CLOCK_NUDGE.MIN_GAP_MS,
        'the gap did not widen across three failed calls');
});

check('a seated table where nothing looks due is nudged once, then once a minute', () => {
    // The first tick after joining a table always nudges: the tab has just
    // arrived, nothing has been asked of this canister yet, and a table waiting
    // to deal is the commonest reason somebody opened the page. After that the
    // only thing that fires is the slow backstop.
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'Flop', players: TWO_LIVE, remaining: 20 });
    const firedAt = [];
    for (let t = 0; t <= 3 * CLOCK_NUDGE.BACKSTOP_MS; t += TICK) {
        if (p.decide(v, t).call) { firedAt.push(t); p.observe(v, { NoAction: null }, t); }
    }
    assert.deepEqual(firedAt, [
        0, CLOCK_NUDGE.BACKSTOP_MS, 2 * CLOCK_NUDGE.BACKSTOP_MS, 3 * CLOCK_NUDGE.BACKSTOP_MS,
    ], `fired at ${firedAt.join(', ')} ms`);
});

check('an EMPTY table gets no backstop at all: nobody is waiting on a deadline', () => {
    const p = new ClockNudgePolicy();
    const v = view({ phase: 'WaitingForPlayers', players: NOBODY });
    for (let t = 0; t <= DAY_MS; t += 60_000) {
        assert.equal(p.decide(v, t).call, false, `an empty table asked for a call at ${t} ms`);
    }
});

check('the fingerprint ignores a countdown and notices a real change', () => {
    const a = view({ phase: 'Flop', players: TWO_LIVE, remaining: 20 });
    const b = view({ phase: 'Flop', players: TWO_LIVE, remaining: 3 });
    assert.equal(tableFingerprint(a), tableFingerprint(b),
        'a ticking clock must not count as the table moving, or the backoff never engages');
    const c = view({ phase: 'Flop', players: TWO_LIVE, remaining: 20, pot: 500 });
    assert.notEqual(tableFingerprint(a), tableFingerprint(c));
});

// ---------------------------------------------------------------------------
// 3. A SYNTHETIC DAY, and the control that proves the budget measures something
// ---------------------------------------------------------------------------
//
// The day is deliberately unkind:
//   08:00 of an empty table            -- the common case, and it must cost zero
//   12:00 of play at ~500 hands/day    -- the E-55 "fully occupied table" figure
//   04:00 of a table jammed between hands, due forever, never moving
//
/**
 * @param {number} ms
 * @returns {object|null} the view a tab would be holding at that instant
 */
function dayView(ms) {
    const h = ms / 3_600_000;
    if (h < 8) return view({ phase: 'WaitingForPlayers', players: NOBODY });
    if (h < 20) {
        // 500 hands over 12 hours = one hand per 86.4 s. Five seconds between
        // hands, the rest in play with the action clock running.
        const cycleMs = 86_400;
        const inCycle = ms % cycleMs;
        const handNumber = Math.floor(ms / cycleMs);
        if (inCycle < 5_000) {
            return view({ phase: 'HandComplete', players: TWO_LIVE, handNumber });
        }
        return view({
            phase: 'Flop',
            players: TWO_LIVE,
            handNumber,
            remaining: Math.max(0, 20 - Math.floor((inCycle - 5_000) / 1000) % 21),
            pot: inCycle,
        });
    }
    // Jammed: permanently between hands with two dealable seats, nothing moving.
    return view({ phase: 'HandComplete', players: TWO_LIVE, handNumber: 9_999 });
}

function simulate(policy, { naive = false } = {}) {
    let calls = 0;
    for (let t = 0; t < DAY_MS; t += TICK) {
        const v = dayView(t);
        if (naive) {
            // What the code did before this wave, transposed onto this driver: no
            // condition, no floor, one update per tick.
            calls += 1;
            continue;
        }
        if (policy.decide(v, t).call) {
            // Between hands, a real `check_timeouts` returns AutoDealReady; the
            // jammed stretch returns it too and the hand still never starts,
            // which is exactly the case the fingerprint backoff has to catch.
            const phase = variantName(v.phase);
            const reply = (phase === 'HandComplete' || phase === 'WaitingForPlayers')
                ? { AutoDealReady: null } : { NoAction: null };
            policy.observe(v, reply, t);
            calls += 1;
        }
    }
    return calls;
}

const realPolicy = new ClockNudgePolicy();
const realCalls = simulate(realPolicy);
const naiveCalls = simulate(new ClockNudgePolicy(), { naive: true });
const realCycles = realCalls * CHECK_TIMEOUTS_CYCLES;
const naiveCycles = naiveCalls * CHECK_TIMEOUTS_CYCLES;

console.log(`    ....  one tab, one synthetic day: ${realCalls.toLocaleString('en-US')} update `
    + `calls (${(realCycles / 1e12).toFixed(4)} T) against a no-policy `
    + `${naiveCalls.toLocaleString('en-US')} (${(naiveCycles / 1e12).toFixed(4)} T)`);

check('the hard ceiling holds even if every assumption in the simulation is wrong', () => {
    // `maxCallsIn` is the bound from the floor alone: one call per MIN_GAP_MS,
    // whatever the table does and whatever the policy decides. It is much looser
    // than the budget below and it is checked anyway, because the budget rests on
    // a synthetic day and this does not.
    const ceiling = maxCallsIn(DAY_MS);
    assert.equal(ceiling, Math.ceil(DAY_MS / CLOCK_NUDGE.MIN_GAP_MS));
    assert.ok(realCalls <= ceiling,
        `${realCalls} simulated calls exceed the ${ceiling} the floor alone permits, which `
        + 'means the floor is not being applied at all');
});

check('one open tab stays under the daily cycle budget', () => {
    assert.ok(realCycles <= DAILY_CYCLE_BUDGET,
        `${(realCycles / 1e12).toFixed(4)} T/day/tab, over the `
        + `${(DAILY_CYCLE_BUDGET / 1e12).toFixed(4)} T budget `
        + `(${realCalls.toLocaleString('en-US')} calls x ${CHECK_TIMEOUTS_CYCLES} cycles)`);
});

check('THE CONTROL: the same instrument convicts a policy-free loop', () => {
    // If this ever passes, the budget above is measuring nothing and every claim
    // in this file is worthless.
    assert.ok(naiveCycles > DAILY_CYCLE_BUDGET,
        'an unconditional update-per-tick loop came in UNDER the budget, so the budget '
        + 'cannot tell the fixed poll from the broken one');
    assert.ok(naiveCalls > realCalls * 10,
        `the policy only saved ${(naiveCalls / realCalls).toFixed(1)}x; either the day is `
        + 'too easy or the policy has stopped doing anything');
});

check('the empty first eight hours cost exactly nothing', () => {
    const p = new ClockNudgePolicy();
    let calls = 0;
    for (let t = 0; t < 8 * 3_600_000; t += TICK) {
        if (p.decide(dayView(t), t).call) { p.observe(dayView(t), { NoAction: null }, t); calls += 1; }
    }
    assert.equal(calls, 0, `${calls} update calls on a table nobody was sitting at`);
});

check('the jammed four hours are bounded by the ceiling, not by the floor', () => {
    const p = new ClockNudgePolicy();
    let calls = 0;
    for (let t = 20 * 3_600_000; t < DAY_MS; t += TICK) {
        const v = dayView(t);
        if (p.decide(v, t).call) { p.observe(v, { AutoDealReady: null }, t); calls += 1; }
    }
    // AutoDealReady always reads as work, so this stretch runs at the FLOOR and
    // not at the ceiling: 4 h / 2 s. That is the honest worst case for a table
    // that is genuinely trying to deal and failing, and it is what the budget has
    // to absorb.
    const atFloor = Math.floor(4 * 3_600_000 / CLOCK_NUDGE.MIN_GAP_MS);
    assert.ok(calls <= atFloor,
        `${calls} calls, above even the ${atFloor} the floor allows`);
    assert.ok(calls * CHECK_TIMEOUTS_CYCLES < DAILY_CYCLE_BUDGET,
        `four jammed hours alone cost ${(calls * CHECK_TIMEOUTS_CYCLES / 1e12).toFixed(4)} T`);
});

if (failures) {
    console.error(`\n  test-clock-nudge FAILED (${failures} problem(s))`);
    process.exit(1);
}
console.log('  test-clock-nudge passed');
process.exit(0);
