#!/usr/bin/env node
// THE ONE PLACE THE RUNWAY NUMBERS COME FROM.
//
// ===========================================================================
// WHY (docs/DEFECTS.md E-92, E-55)
// ===========================================================================
//
// Before this file, the runway table existed in six places: a Rust test that
// derived it, a shell script's header comment, a JS module's header comment, two
// Svelte modals' comments, a GitHub workflow's comment and the register itself.
// Every one of them was a hand-copied transcription, and every one of them was
// wrong in the same way — they priced an open browser tab as a 10-second
// heartbeat stream and left out the 500 ms UPDATE poll, which was larger by more
// than an order of magnitude.
//
// Six copies of a number is six chances to be wrong and one guarantee: whichever
// one somebody reads, nothing checks it. So there is now ONE file of numbers,
// this script writes it FROM MEASUREMENTS, and every consumer reads it:
//
//     tools/cycles/burn-table.json          <- the single source
//       scripts/cycles-runway.sh              reads FALLBACK_BURN_PER_DAY + prints the table
//       src/cleardeck_frontend/src/lib/cycleRunway.js
//                                             cites it; states no burn figure of its own
//       tools/shots/test-burn-table.mjs       the gate: arithmetic, wiring, and no stale copies
//
// ===========================================================================
// WHAT IS MEASURED HERE AND WHAT IS INHERITED
// ===========================================================================
//
// MEASURED THIS WAVE, on the local replica, by `tools/cycles/tab-burn.mjs`:
//   * the idle burn of a table nobody is looking at (the control run)
//   * the marginal cost of ONE OPEN BROWSER TAB, before and after the E-92 fix,
//     and the ceiling the fixed policy can reach
//   * the cycles cost of one `check_timeouts` call
//
// INHERITED from wave 13 (`tests/money_safety/tests/cycles_runway.rs`) and NOT
// re-measured here: the cost of a hand, a deposit and a withdrawal. They are
// carried with a `source` field saying exactly that, because a number whose
// provenance is not attached is a number that will be re-copied without it.
//
// USAGE
//   ./tools/cycles/run-matrix.sh 120        # produce artifacts/cycles/*.json
//   node tools/cycles/build-burn-table.mjs  # fold them into burn-table.json

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const ARTIFACTS = path.join(ROOT, 'artifacts/cycles');
const OUT = path.join(HERE, 'burn-table.json');

/**
 * Costs measured in wave 13 on the real module under PocketIC and NOT re-measured
 * by this pass. Carried so the scenario arithmetic is complete; labelled so
 * nobody mistakes them for this wave's work.
 */
const INHERITED = {
    source: 'tests/money_safety/tests/cycles_runway.rs (wave 13); not re-measured in wave 14',
    hand_cycles: 168_300_000,
    deposit_cycles: 12_700_000,
    withdrawal_cycles: 12_700_000,
    heartbeat_cycles: 7_160_000,
};

/** Balances the runway table is expressed against. */
const BALANCES_T = [1, 10, 20, 51.4];

function readRun(name) {
    const p = path.join(ARTIFACTS, `${name}.json`);
    if (!fs.existsSync(p)) return null;
    const j = JSON.parse(fs.readFileSync(p, 'utf8'));
    return j.runs[0];
}

function must(name) {
    const r = readRun(name);
    if (!r) {
        console.error(`  missing artifacts/cycles/${name}.json — run ./tools/cycles/run-matrix.sh first`);
        process.exit(1);
    }
    return r;
}

const controlBefore = must('control-before');
const controlAfter = readRun('control-after');

// THE CONTROL IS TAKEN TWICE AND CHECKED, because every marginal figure below is
// a difference against it. A baseline that moved during the run is a baseline
// that cannot support a difference, and publishing one anyway is exactly the
// species of error E-92 is about.
const idle = controlAfter
    ? Math.round((controlBefore.burn_per_day + controlAfter.burn_per_day) / 2)
    : controlBefore.burn_per_day;
let controlDrift = null;
if (controlAfter) {
    const spread = Math.abs(controlAfter.burn_per_day - controlBefore.burn_per_day);
    controlDrift = spread / idle;
    if (controlDrift > 0.25) {
        console.error(`  the control moved ${(controlDrift * 100).toFixed(0)}% between the start `
            + 'and the end of the matrix. Every marginal figure is a difference against it, so '
            + 'this run cannot support one. Re-run the matrix on a quiet replica.');
        process.exit(1);
    }
}

/** marginal T/day per tab for one mode, from every cell that measured it. */
function marginal(prefix) {
    const cells = [];
    for (const n of [1, 3, 10]) {
        const r = readRun(`${prefix}-${n}tab`);
        if (!r) continue;
        cells.push({
            tabs: n,
            burn_per_day: r.burn_per_day,
            per_tab: Math.round((r.burn_per_day - idle) / n),
            update_calls: r.update_calls,
            elapsed_s: r.elapsed_s,
            cycles_per_update: r.update_calls
                ? Math.round((r.burn_per_day - idle) * (r.elapsed_s / 86400) / r.update_calls)
                : null,
        });
    }
    if (!cells.length) return null;
    // THE WORST CELL, NOT THE MEAN. A fuel gauge resolves a disagreement upwards
    // when the quantity is a cost.
    const worst = cells.reduce((a, b) => (b.per_tab > a.per_tab ? b : a));
    return { cells, per_tab_per_day: worst.per_tab, from_tabs: worst.tabs };
}

const legacy = marginal('legacy');
const fixed = marginal('fixed');
const fixedMax = marginal('fixed-max');
const fixedStuck = readRun('fixed-stuck-10tab');

if (!legacy || !fixed || !fixedMax) {
    console.error('  the matrix is incomplete: need legacy-*, fixed-* and fixed-max-* cells');
    process.exit(1);
}

// cycles per check_timeouts, taken from the cell with the most calls behind it.
const perUpdateCells = legacy.cells.filter((c) => c.cycles_per_update);
const cyclesPerUpdate = perUpdateCells.length
    ? Math.round(perUpdateCells.reduce((s, c) => s + c.cycles_per_update, 0) / perUpdateCells.length)
    : null;

// ---------------------------------------------------------------------------
// The scenarios, DERIVED. Nothing below is typed in.
// ---------------------------------------------------------------------------
//
// THREE PER-TAB PRICES, PUBLISHED SEPARATELY, BECAUSE ONE NUMBER HERE IS A LIE
// WHICHEVER ONE YOU PICK.
//
//   legacy          what the 500 ms poll cost, measured. The comparison.
//   fixed_typical   the fixed client measured against a table nobody is playing
//                   at: the policy correctly sends almost nothing and the figure
//                   is essentially the 10-second heartbeat. This is what a real
//                   tab costs most of the time and it is the FLATTERING number.
//   fixed_ceiling   the policy pinned at its 2 s floor with the table moving on
//                   every single call, all day. The most one tab can ever cost,
//                   and NOT reachable by an ordinary game — but it is the only
//                   figure safe to alarm on, so it is what
//                   `fallback_burn_per_day` is built from.
//
// Publishing only the typical figure would repeat the error this whole entry is
// about. Publishing only the ceiling would be a different dishonesty: quoting an
// unreachable worst case as the bill. Both are in the table, both are labelled.
const PER_TAB = {
    legacy: legacy.per_tab_per_day,
    fixed_typical: fixed.per_tab_per_day,
    fixed_ceiling: fixedMax.per_tab_per_day,
};
const BASIS_LABEL = {
    legacy: 'legacy 500 ms poll (pre-E-92), measured',
    fixed_typical: 'fixed poll, measured on a table nobody is playing at',
    fixed_ceiling: 'fixed poll, policy pinned at its 2 s floor all day',
};

function scenario(name, { tabs = 0, handsPerDay = 0, basis = 'fixed_ceiling', note = null }) {
    const burn = idle
        + handsPerDay * INHERITED.hand_cycles
        + tabs * (tabs === 0 ? 0 : PER_TAB[basis]);
    const runway = {};
    for (const t of BALANCES_T) {
        runway[`${t}T`] = Math.floor((t * 1e12) / burn);
    }
    return {
        name,
        tabs,
        hands_per_day: handsPerDay,
        tab_cost_basis: tabs === 0 ? null : basis,
        tab_cost_basis_label: tabs === 0 ? null : BASIS_LABEL[basis],
        burn_per_day: Math.round(burn),
        burn_per_day_T: +(burn / 1e12).toFixed(4),
        runway_days: runway,
        note,
    };
}

const scenarios = [
    scenario('idle, nobody at the table', {}),
    scenario('500 hands/day, no tab open', { handsPerDay: 500 }),
    scenario('500 hands/day, 6 tabs open (typical)', {
        handsPerDay: 500, tabs: 6, basis: 'fixed_typical',
    }),
    scenario('500 hands/day, 10 tabs open (typical)', {
        handsPerDay: 500, tabs: 10, basis: 'fixed_typical',
    }),
    scenario('500 hands/day, 6 tabs open (policy ceiling)', {
        handsPerDay: 500, tabs: 6, basis: 'fixed_ceiling',
    }),
    scenario('500 hands/day, 10 tabs open (policy ceiling)', {
        handsPerDay: 500, tabs: 10, basis: 'fixed_ceiling',
        note: 'the figure fallback_burn_per_day is taken from.',
    }),
    scenario('500 hands/day, 6 tabs open — BEFORE the E-92 fix', {
        handsPerDay: 500, tabs: 6, basis: 'legacy',
        note: 'what the poll cost until wave 14. Kept as the comparison, not as a live figure.',
    }),
    scenario('500 hands/day, 10 tabs open — BEFORE the E-92 fix', {
        handsPerDay: 500, tabs: 10, basis: 'legacy',
        note: 'the headline of E-92: ten open browser tabs and 10 T is under a day.',
    }),
];

// THE FLOOR THE CI MONITOR ALARMS ON when a canister cannot yet measure its own
// burn. It has to be the burn of a table that is BUSY, not idle, or a canister
// that cannot say gets flattered. Taken from the CEILING scenario, not the
// typical one: an alarm threshold is the one place the pessimistic figure is the
// right figure.
const fallback = scenarios
    .find((s) => s.name === '500 hands/day, 10 tabs open (policy ceiling)').burn_per_day;

const table = {
    schema: 1,
    generated_by: 'tools/cycles/build-burn-table.mjs',
    generated_at: new Date().toISOString(),
    defect: 'docs/DEFECTS.md E-92 (the poll), E-55 (the runway)',
    method: 'tools/cycles/tab-burn.mjs, local replica, real module, control-differenced. '
        + 'See that file\'s header for the procedure and its two stated confounds.',
    environment: 'local replica (icp 1.0.2). Absolute prices are a local measurement; '
        + 'the ratio between the legacy and fixed polls is what E-92 is about and is robust '
        + 'to the price list.',
    measured: {
        idle_burn_per_day: idle,
        control_before_per_day: controlBefore.burn_per_day,
        control_after_per_day: controlAfter ? controlAfter.burn_per_day : null,
        control_drift: controlDrift === null ? null : +controlDrift.toFixed(3),
        cycles_per_check_timeouts: cyclesPerUpdate,
        per_tab_per_day: {
            legacy_poll: PER_TAB.legacy,
            fixed_poll_measured_idle_table: PER_TAB.fixed_typical,
            fixed_poll_ceiling: PER_TAB.fixed_ceiling,
        },
        cells: { legacy: legacy.cells, fixed: fixed.cells, fixed_max: fixedMax.cells },
        fixed_stuck_10tab: fixedStuck
            ? { burn_per_day: fixedStuck.burn_per_day, update_calls: fixedStuck.update_calls }
            : null,
    },
    inherited: INHERITED,
    scenarios,
    fallback_burn_per_day: fallback,
    fallback_rationale:
        'What a canister that cannot yet measure its own burn is ASSUMED to be burning. '
        + 'It is the busiest shipped-client scenario (500 hands/day, 10 tabs open), not the '
        + 'idle figure, because "I cannot say" must not be turned into "you have plenty".',
};

fs.writeFileSync(OUT, `${JSON.stringify(table, null, 2)}\n`);

console.log(`  wrote ${path.relative(ROOT, OUT)}`);
console.log(`  idle                            ${(idle / 1e12).toFixed(4)} T/day`);
console.log(`  per tab, legacy poll            ${(PER_TAB.legacy / 1e12).toFixed(4)} T/day`);
console.log(`  per tab, fixed poll (typical)   ${(PER_TAB.fixed_typical / 1e12).toFixed(4)} T/day`
    + `   -> ${(PER_TAB.legacy / PER_TAB.fixed_typical).toFixed(1)}x cheaper`);
console.log(`  per tab, fixed poll (ceiling)   ${(PER_TAB.fixed_ceiling / 1e12).toFixed(4)} T/day`
    + `   -> ${(PER_TAB.legacy / PER_TAB.fixed_ceiling).toFixed(1)}x cheaper`);
console.log(`  cycles per check_timeouts       ${cyclesPerUpdate?.toLocaleString('en-US')}`);
console.log('');
console.log('  scenario                                                  T/day    10 T    51.4 T');
for (const s of scenarios) {
    console.log(`  ${s.name.padEnd(56)}${s.burn_per_day_T.toFixed(4).padStart(7)}`
        + `${String(s.runway_days['10T']).padStart(8)}d`
        + `${String(s.runway_days['51.4T']).padStart(9)}d`);
}
