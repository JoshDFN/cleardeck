#!/usr/bin/env node
// THE GATE: NO POLL LOOP MAY CALL AN UPDATE METHOD.
//
// ===========================================================================
// WHAT THIS EXISTS FOR (docs/DEFECTS.md E-92)
// ===========================================================================
//
// `src/cleardeck_frontend/src/routes/+page.svelte` drove this, for the whole of
// this project's life, and nothing anywhere was red about it:
//
//     const POLL_INTERVAL = 500;
//     pollInterval = setInterval(loadTableState, POLL_INTERVAL);
//
//     async function loadTableState() {
//       const timeoutResult = await tableActor.check_timeouts();   // FIRST statement
//
// `check_timeouts : () -> (TimeoutCheckResult);` in `table_canister.did` — no
// `query`. So the render rate was driving an UPDATE loop, one per open browser
// tab, at ~43,200 to 172,800 calls a day depending on finality, and every cycles
// figure in the tree omitted it. The runway numbers therefore all read HIGH,
// which is the dangerous direction.
//
// The reason it survived is that it is INVISIBLE TO EVERY INSTRUMENT THIS PROJECT
// HAD. It is not a bug: the code does exactly what it says. It is not a test
// failure: the game works. It is not a lint: the call is well-formed. The only
// thing that makes it wrong is a fact that lives in a DIFFERENT FILE — whether
// the Candid declares that method `query` — joined to a fact that lives in the
// frontend — how often the method is called. Nothing joined them.
//
// This joins them.
//
// ===========================================================================
// THE TWO ASSERTIONS
// ===========================================================================
//
//   A1  STRUCTURE. No repeating timer whose period is below FAST_TIMER_MS may
//       reach an update method on a table canister, transitively, through the
//       functions defined in the same file. This is the assertion that is RED on
//       the pre-fix tree.
//
//   A2  BUDGET. Summed over every repeating timer that CAN reach an update
//       method, the worst-case update calls one open tab can emit in a day must
//       stay under MAX_TAB_UPDATES_PER_DAY. A1 alone can be satisfied by moving
//       the same loop to 2,001 ms; A2 is what makes the bill the thing being
//       checked rather than the shape.
//
// Neither assertion knows anything about what the code MEANS. Both are read off
// the committed `.did` and the committed source, so they hold for a tree nobody
// can run.
//
// ===========================================================================
// WHAT IT CANNOT SEE, STATED SO NOBODY TRUSTS IT FURTHER THAN IT GOES
// ===========================================================================
//
//   * Dynamic dispatch. `actor[name]()` where `name` is computed is invisible
//     here. `$lib/cycleRunway.js` legitimately does this — and it is not on a
//     poll loop. If a poll loop ever does, this gate will not catch it.
//   * Calls made from another module. Only functions DEFINED IN THE SCANNED FILE
//     are followed; an imported helper that calls an update is invisible.
//   * `setTimeout` chains that reschedule themselves. Handled only when the
//     delay is a literal or a resolvable constant.
//
// Each of those is a way this gate could be defeated by somebody trying. It
// catches the way it actually happened, which is the bar a regression gate has to
// clear.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');

// ---------------------------------------------------------------------------
// The two numbers this gate enforces
// ---------------------------------------------------------------------------

/**
 * Below this period, a repeating timer is a POLL, and a poll may not send update
 * calls. 2 s is roughly one mainnet update finality: a loop faster than this
 * cannot even produce a second reply, so its extra calls buy latency that does
 * not exist and cost cycles that do.
 *
 * Deliberately a literal here and NOT imported from `$lib/clockNudge.js`, so the
 * gate still runs — and still fails — on a tree where that module does not exist.
 * When the module IS present, its floor is checked against this number below, so
 * the two cannot drift apart in the direction that would weaken this.
 */
const FAST_TIMER_MS = 2_000;

/**
 * The worst-case update calls one open browser tab may cost a table canister in
 * a day, from the SHAPE of the code alone.
 *
 * Where it comes from: the fixed poll's two update drivers are the clock nudger
 * at its 2 s floor (43,200/day) and the connection heartbeat at 10 s
 * (8,640/day) = 51,840. The cap is set just above that, at 60,000, so that
 * adding a third update loop, or halving either period, fails here rather than
 * on somebody's cycles bill. The measured figure is far lower because the nudger
 * is conditional — see `tools/shots/test-clock-nudge.mjs` for the simulated
 * number and `tools/cycles/tab-burn.mjs` for the measured one. This is the bound
 * a reader of the source can verify without running anything.
 */
const MAX_TAB_UPDATES_PER_DAY = 60_000;

const DAY_MS = 86_400_000;

/** The frontend files whose timers are checked, and the interface they call. */
const SCANNED = [
    {
        source: 'src/cleardeck_frontend/src/routes/+page.svelte',
        did: 'src/table_canister/table_canister.did',
    },
];

// ---------------------------------------------------------------------------
// Candid: which methods are queries?
// ---------------------------------------------------------------------------
//
// The service block is the authority, not a hand-kept list. A method declared
// `name : (args) -> (ret) query;` is a query; anything else on the wire is an
// update. Comments are stripped first so a commented-out declaration cannot
// register as an interface.

/**
 * @param {string} didText
 * @returns {{queries: Set<string>, updates: Set<string>}}
 */
export function classifyDidMethods(didText) {
    const noComments = didText
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/\/\/[^\n]*/g, '');
    const queries = new Set();
    const updates = new Set();
    // `name : ( ... ) -> ( ... ) [query|composite_query|oneway] ;`
    // The arrow is what makes it a method rather than a type alias.
    const re = /(^|[\s{,;])([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*\(([\s\S]*?)\)\s*->\s*\(([\s\S]*?)\)\s*(query|composite_query|oneway)?\s*;/g;
    let m;
    while ((m = re.exec(noComments)) !== null) {
        const name = m[2];
        const annotation = m[5];
        if (annotation === 'query' || annotation === 'composite_query') queries.add(name);
        else updates.add(name);
    }
    return { queries, updates };
}

// ---------------------------------------------------------------------------
// Source: strip strings and comments, then everything else is structure
// ---------------------------------------------------------------------------

/**
 * Blank out comments and string/template literals, preserving offsets so that
 * every index into the result is an index into the original.
 *
 * Necessary because this file brace-matches: a `}` inside a comment or a string
 * would otherwise close a function body early and hide everything after it —
 * which is the failure mode that turns a gate into decoration.
 *
 * @param {string} src
 * @returns {string}
 */
export function blankNonCode(src) {
    const out = src.split('');
    let i = 0;
    const n = src.length;
    const blank = (from, to) => {
        for (let k = from; k < to && k < n; k++) if (out[k] !== '\n') out[k] = ' ';
    };
    while (i < n) {
        const c = src[i];
        const next = src[i + 1];
        if (c === '/' && next === '/') {
            const end = src.indexOf('\n', i);
            blank(i, end === -1 ? n : end);
            i = end === -1 ? n : end;
            continue;
        }
        if (c === '/' && next === '*') {
            const end = src.indexOf('*/', i + 2);
            blank(i, end === -1 ? n : end + 2);
            i = end === -1 ? n : end + 2;
            continue;
        }
        if (c === '"' || c === "'" || c === '`') {
            let j = i + 1;
            while (j < n) {
                if (src[j] === '\\') { j += 2; continue; }
                if (src[j] === c) break;
                j += 1;
            }
            blank(i + 1, j);
            i = j + 1;
            continue;
        }
        i += 1;
    }
    return out.join('');
}

/**
 * Numeric constants declared at any level: `const NAME = 500;`
 * @param {string} code blanked source
 * @returns {Map<string, number>}
 */
function numericConsts(code) {
    const map = new Map();
    const re = /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(\d[\d_]*)\s*[;,\n]/g;
    let m;
    while ((m = re.exec(code)) !== null) {
        map.set(m[1], Number(m[2].replace(/_/g, '')));
    }
    return map;
}

/**
 * Every function DEFINED in this source, as name -> body text.
 *
 * Handles `function f() {}`, `async function f() {}`, `const f = () => {}` and
 * `const f = async (a) => {}`. Brace-matched over blanked code.
 *
 * @param {string} code blanked source
 * @returns {Map<string, string>}
 */
export function functionBodies(code) {
    const bodies = new Map();
    const record = (name, openIdx) => {
        if (openIdx === -1) return;
        let depth = 0;
        for (let k = openIdx; k < code.length; k++) {
            if (code[k] === '{') depth += 1;
            else if (code[k] === '}') {
                depth -= 1;
                if (depth === 0) { bodies.set(name, code.slice(openIdx, k + 1)); return; }
            }
        }
    };
    let m;
    const declRe = /\b(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*\{/g;
    while ((m = declRe.exec(code)) !== null) record(m[1], code.indexOf('{', m.index));
    const arrowRe = /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>\s*\{/g;
    while ((m = arrowRe.exec(code)) !== null) {
        record(m[1], code.indexOf('{', code.indexOf('=>', m.index)));
    }
    return bodies;
}

/**
 * Repeating timers: `setInterval(<fn>, <period>)`.
 *
 * `<fn>` may be a bare identifier or an arrow that immediately calls one, which
 * are the two shapes this codebase uses. `<period>` may be a literal, a local
 * numeric const, or a member expression resolved from `extraConsts`.
 *
 * @param {string} code blanked source
 * @param {Map<string, number>} consts
 * @param {Map<string, number>} extraConsts dotted names, e.g. CLOCK_NUDGE.TICK_MS
 * @returns {Array<{entry: string, period: number|null, raw: string}>}
 */
export function repeatingTimers(code, consts, extraConsts) {
    const found = [];
    const re = /setInterval\s*\(\s*([^,]+?)\s*,\s*([A-Za-z_$][\w$.]*|\d[\d_]*)\s*\)/g;
    let m;
    while ((m = re.exec(code)) !== null) {
        const fnExpr = m[1].trim();
        const periodExpr = m[2].trim();
        let entry = null;
        const bare = fnExpr.match(/^([A-Za-z_$][\w$]*)$/);
        if (bare) entry = bare[1];
        else {
            const arrow = fnExpr.match(/=>\s*(?:this\.)?([A-Za-z_$][\w$]*)\s*\(/);
            if (arrow) entry = arrow[1];
        }
        let period = null;
        if (/^\d/.test(periodExpr)) period = Number(periodExpr.replace(/_/g, ''));
        else if (consts.has(periodExpr)) period = consts.get(periodExpr);
        else if (extraConsts.has(periodExpr)) period = extraConsts.get(periodExpr);
        found.push({ entry, period, raw: m[0] });
    }
    return found;
}

/**
 * Every canister method named anywhere reachable from `entry`, following calls
 * to functions defined in the same file.
 *
 * @param {string} entry
 * @param {Map<string,string>} bodies
 * @param {Set<string>} methodNames every method on the interface
 * @returns {{methods: Map<string, string>, visited: Set<string>}}
 *          methods: method name -> the function it was found in
 */
export function reachableMethods(entry, bodies, methodNames) {
    const methods = new Map();
    const visited = new Set();
    const stack = [entry];
    while (stack.length) {
        const fn = stack.pop();
        if (visited.has(fn)) continue;
        visited.add(fn);
        const body = bodies.get(fn);
        if (!body) continue;
        // Any `.method(` whose name is on the interface. Broader than matching an
        // actor variable by name on purpose: a method declared in the Candid is
        // not a plausible unrelated member call, and matching the variable name
        // would be defeated by renaming it.
        const memberRe = /\.([A-Za-z_][\w]*)\s*\(/g;
        let mm;
        while ((mm = memberRe.exec(body)) !== null) {
            if (methodNames.has(mm[1]) && !methods.has(mm[1])) methods.set(mm[1], fn);
        }
        // Local calls, followed transitively.
        const callRe = /(^|[^.\w$])([A-Za-z_$][\w$]*)\s*\(/g;
        let cm;
        while ((cm = callRe.exec(body)) !== null) {
            if (bodies.has(cm[2])) stack.push(cm[2]);
        }
    }
    return { methods, visited };
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

let failed = 0;
const fail = (msg) => { console.error(`    FAIL  ${msg}`); failed += 1; };
const ok = (msg) => console.log(`    ok    ${msg}`);

console.log('  test-poll-updates: no poll loop may call an update method (docs/DEFECTS.md E-92)');

// If the shared policy module is present, its floor must not undercut this gate.
const extraConsts = new Map();
const nudgePath = path.join(ROOT, 'src/cleardeck_frontend/src/lib/clockNudge.js');
if (fs.existsSync(nudgePath)) {
    const { CLOCK_NUDGE } = await import(nudgePath);
    for (const [k, v] of Object.entries(CLOCK_NUDGE)) {
        if (typeof v === 'number') extraConsts.set(`CLOCK_NUDGE.${k}`, v);
    }
    if (CLOCK_NUDGE.MIN_GAP_MS < FAST_TIMER_MS) {
        fail(`clockNudge.js MIN_GAP_MS is ${CLOCK_NUDGE.MIN_GAP_MS} ms, under this gate's `
            + `${FAST_TIMER_MS} ms floor: the policy would authorise a call rate this gate `
            + 'exists to forbid');
    } else {
        ok(`clockNudge.js MIN_GAP_MS ${CLOCK_NUDGE.MIN_GAP_MS} ms >= ${FAST_TIMER_MS} ms floor`);
    }
    if (CLOCK_NUDGE.TICK_MS < CLOCK_NUDGE.MIN_GAP_MS) {
        fail(`clockNudge.js TICK_MS (${CLOCK_NUDGE.TICK_MS} ms) is faster than its own floor `
            + `(${CLOCK_NUDGE.MIN_GAP_MS} ms): the driver would be a fast update loop with a `
            + 'runtime excuse, and a runtime excuse is not statically checkable');
    } else {
        ok(`clockNudge.js TICK_MS ${CLOCK_NUDGE.TICK_MS} ms is not faster than its floor`);
    }
} else {
    console.log('    note  no $lib/clockNudge.js in this tree; checking timers only');
}

for (const { source, did } of SCANNED) {
    const srcPath = path.join(ROOT, source);
    const didPath = path.join(ROOT, did);
    if (!fs.existsSync(srcPath)) { fail(`${source} does not exist`); continue; }
    if (!fs.existsSync(didPath)) { fail(`${did} does not exist`); continue; }

    const { queries, updates } = classifyDidMethods(fs.readFileSync(didPath, 'utf8'));
    if (updates.size === 0 || queries.size === 0) {
        fail(`${did} parsed to ${queries.size} queries and ${updates.size} updates; `
            + 'a classifier that finds nothing would pass everything');
        continue;
    }
    const allMethods = new Set([...queries, ...updates]);
    ok(`${did}: ${queries.size} query methods, ${updates.size} update methods`);

    const code = blankNonCode(fs.readFileSync(srcPath, 'utf8'));
    const consts = numericConsts(code);
    const bodies = functionBodies(code);
    const timers = repeatingTimers(code, consts, extraConsts);

    if (timers.length === 0) {
        fail(`${source}: no setInterval found at all. Either the poll moved to a shape this `
            + 'gate cannot see, or the gate is broken. Both are failures.');
        continue;
    }

    let budget = 0;
    for (const t of timers) {
        if (!t.entry) {
            fail(`${source}: could not identify the callback in \`${t.raw}\``);
            continue;
        }
        if (t.period === null) {
            fail(`${source}: could not resolve the period in \`${t.raw}\`. An unresolvable `
                + 'period is an unbounded loop as far as this gate can tell.');
            continue;
        }
        const { methods } = reachableMethods(t.entry, bodies, allMethods);
        const reachedUpdates = [...methods].filter(([m]) => updates.has(m));

        if (reachedUpdates.length === 0) {
            ok(`${t.entry} every ${t.period} ms: queries only `
                + `(${[...methods.keys()].join(', ') || 'no canister calls'})`);
            continue;
        }

        const listed = reachedUpdates.map(([m, fn]) => `${m}() in ${fn}()`).join(', ');
        if (t.period < FAST_TIMER_MS) {
            // ============================================================
            // A1. THIS IS THE E-92 ASSERTION.
            // ============================================================
            fail(`${source}: \`setInterval(${t.entry}, ${t.period})\` reaches UPDATE `
                + `method(s): ${listed}.\n`
                + `          A timer at ${t.period} ms is a POLL. `
                + `${allMethods.size} methods are declared in ${did} and these are not `
                + 'queries there, so every open browser tab drives an update loop at up to '
                + `${Math.round(DAY_MS / t.period).toLocaleString('en-US')} calls a day. `
                + 'Move the update onto its own schedule (see $lib/clockNudge.js).');
        } else {
            ok(`${t.entry} every ${t.period} ms reaches updates (${listed}) `
                + '— slower than the poll floor, counted against the budget');
        }
        budget += DAY_MS / t.period;
    }

    // ---------------------------------------------------------------------
    // THE HARNESS HAS TO BE PRICING THE LOOP THE PAGE ACTUALLY RUNS.
    // ---------------------------------------------------------------------
    //
    // `tools/cycles/tab-burn.mjs` copies the three periods so it can drive the
    // same call pattern the page drives, and every runway figure in the tree is
    // derived from what it measured. A copy that drifts is a measurement of a
    // client nobody ships — which is a subtler version of the same defect this
    // gate exists for: a number whose provenance stopped being true.
    const harnessPath = path.join(ROOT, 'tools/cycles/tab-burn.mjs');
    if (fs.existsSync(harnessPath) && source.endsWith('+page.svelte')) {
        const harness = numericConsts(blankNonCode(fs.readFileSync(harnessPath, 'utf8')));
        let drifted = 0;
        for (const name of ['POLL_INTERVAL', 'HEARTBEAT_INTERVAL', 'BALANCE_REFRESH_INTERVAL']) {
            if (!consts.has(name)) { fail(`${source} no longer defines ${name}`); drifted += 1; continue; }
            if (!harness.has(name)) { fail(`tab-burn.mjs no longer defines ${name}`); drifted += 1; continue; }
            if (consts.get(name) !== harness.get(name)) {
                fail(`tools/cycles/tab-burn.mjs prices ${name} at ${harness.get(name)} ms and `
                    + `${source} runs it at ${consts.get(name)} ms. The burn measurements in `
                    + 'tools/cycles/burn-table.json describe a client this repository does not ship.');
                drifted += 1;
            }
        }
        if (!drifted) ok('tools/cycles/tab-burn.mjs prices the same three periods the page runs');
    }

    const worst = Math.round(budget);
    if (worst > MAX_TAB_UPDATES_PER_DAY) {
        // ============================================================
        // A2. THE BILL, NOT THE SHAPE.
        // ============================================================
        fail(`${source}: one open tab can emit ${worst.toLocaleString('en-US')} update calls a `
            + `day from its repeating timers, over the ${MAX_TAB_UPDATES_PER_DAY.toLocaleString('en-US')} `
            + 'budget. Every one of those is charged to the table canister, and the runway '
            + 'figures in docs/DEFECTS.md E-55 are computed from this bound.');
    } else {
        ok(`worst-case ${worst.toLocaleString('en-US')} update calls/day/tab, under the `
            + `${MAX_TAB_UPDATES_PER_DAY.toLocaleString('en-US')} budget`);
    }
}

if (failed) {
    console.error(`\n  test-poll-updates FAILED (${failed} problem(s))`);
    process.exit(1);
}
console.log('  test-poll-updates passed');
process.exit(0);
