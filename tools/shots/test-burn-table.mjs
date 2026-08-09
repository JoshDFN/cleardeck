#!/usr/bin/env node
// THE GATE ON THE RUNWAY NUMBERS THEMSELVES.
//
// ===========================================================================
// WHY (docs/DEFECTS.md E-92)
// ===========================================================================
//
// One measurement, six transcriptions:
//
//     scripts/cycles-runway.sh                 header comment + FALLBACK_BURN_PER_DAY
//     src/cleardeck_frontend/src/lib/cycleRunway.js   header comment
//     src/cleardeck_frontend/src/lib/components/DepositModal.svelte    comment
//     src/cleardeck_frontend/src/lib/components/WithdrawModal.svelte   comment
//     .github/workflows/cycles-monitor.yml     header comment
//     docs/DEFECTS.md E-55                     the register's table
//
// Every one of them said an open browser tab costs a 10-second heartbeat. Every
// one of them was wrong by more than an order of magnitude, in the direction that
// makes a canister look safer than it is, and NOTHING COMPARED ANY COPY TO ANY
// OTHER. That is the whole defect: not an arithmetic error, a copying discipline.
//
// So this gate does three things, and none of them is "is the number right":
//
//   1. THE SOURCE EXISTS AND IS SELF-CONSISTENT. `tools/cycles/burn-table.json`
//      parses, carries its provenance, and every scenario's burn and runway can
//      be recomputed from its own components. A table whose rows do not follow
//      from its own inputs is a table somebody typed.
//
//   2. THE CONSUMERS READ IT. `scripts/cycles-runway.sh` must not carry a
//      hardcoded fallback burn rate; it must read one.
//
//   3. NO STALE COPY HAS COME BACK. The retired figures are named, and any
//      appearance of one in code or in operator-facing configuration fails —
//      the same shape as the retracted-custody-claim check in `dev.sh hygiene`,
//      and for the same reason: a number that is wrong in the operator's favour
//      is worse than no number.
//
// `docs/` is deliberately NOT scanned. The register narrates the history of these
// figures and has to be able to quote what they used to be.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const TABLE_PATH = path.join(ROOT, 'tools/cycles/burn-table.json');

let failures = 0;
const fail = (m) => { console.error(`    FAIL  ${m}`); failures += 1; };
const ok = (m) => console.log(`    ok    ${m}`);

console.log('  test-burn-table: one source for every runway figure (docs/DEFECTS.md E-92)');

// ---------------------------------------------------------------------------
// 1. The source
// ---------------------------------------------------------------------------
if (!fs.existsSync(TABLE_PATH)) {
    fail('tools/cycles/burn-table.json does not exist. Every runway figure in this tree is '
        + 'supposed to come from it; without it, each consumer is back to its own copy.\n'
        + '          Regenerate: ./tools/cycles/run-matrix.sh && node tools/cycles/build-burn-table.mjs');
    console.error('\n  test-burn-table FAILED (1 problem)');
    process.exit(1);
}

const table = JSON.parse(fs.readFileSync(TABLE_PATH, 'utf8'));

for (const key of ['schema', 'generated_by', 'generated_at', 'method', 'measured', 'inherited',
    'scenarios', 'fallback_burn_per_day']) {
    if (!(key in table)) fail(`burn-table.json has no "${key}"`);
}
if (!failures) ok('burn-table.json parses and carries its provenance');

const m = table.measured;
if (!(m.idle_burn_per_day > 0)) {
    fail('the measured idle burn is zero or missing: a control run that measured nothing '
        + 'makes every marginal figure below it meaningless');
} else {
    ok(`idle burn measured at ${(m.idle_burn_per_day / 1e12).toFixed(4)} T/day`);
}

const perTab = m.per_tab_per_day || {};
if (!(perTab.legacy_poll > 0) || !(perTab.fixed_poll_ceiling > 0)) {
    fail('the per-tab figures are missing; the whole point of the table is that an open '
        + 'browser tab has a price');
} else if (!(perTab.legacy_poll > perTab.fixed_poll_ceiling)) {
    // THE CONTROL ON THE MEASUREMENT ITSELF. If the fixed poll did not come out
    // cheaper than the loop it replaced, either the fix does nothing or the
    // harness is not measuring the canister.
    fail(`the fixed poll (${(perTab.fixed_poll_ceiling / 1e12).toFixed(4)} T/day/tab) did not `
        + `measure cheaper than the loop it replaced `
        + `(${(perTab.legacy_poll / 1e12).toFixed(4)} T/day/tab). Either the fix is not a fix `
        + 'or the harness is measuring something other than the canister.');
} else {
    ok(`an open tab: ${(perTab.legacy_poll / 1e12).toFixed(4)} T/day before, `
        + `${(perTab.fixed_poll_ceiling / 1e12).toFixed(4)} T/day after `
        + `(${(perTab.legacy_poll / perTab.fixed_poll_ceiling).toFixed(1)}x)`);
}

// Every scenario must follow from the components. This is what makes the table a
// derivation rather than a transcription.
let arithmeticBad = 0;
for (const s of table.scenarios || []) {
    const basisPrice = {
        legacy: perTab.legacy_poll,
        fixed_typical: perTab.fixed_poll_measured_idle_table,
        fixed_ceiling: perTab.fixed_poll_ceiling,
    }[s.tab_cost_basis];
    if (s.tabs > 0 && basisPrice === undefined) {
        fail(`scenario "${s.name}" names an unknown tab_cost_basis "${s.tab_cost_basis}". `
            + 'A row whose per-tab price cannot be identified cannot be checked, and an '
            + 'unchecked row is the thing this gate exists to stop.');
        arithmeticBad += 1;
        continue;
    }
    const perTabTerm = s.tabs === 0 ? 0 : s.tabs * basisPrice;
    const expected = Math.round(m.idle_burn_per_day
        + s.hands_per_day * table.inherited.hand_cycles
        + perTabTerm);
    if (Math.abs(expected - s.burn_per_day) > 1) {
        fail(`scenario "${s.name}": burn_per_day is ${s.burn_per_day}, but its own components `
            + `give ${expected}`);
        arithmeticBad += 1;
        continue;
    }
    for (const [label, days] of Object.entries(s.runway_days)) {
        const balance = Number(label.replace('T', '')) * 1e12;
        const want = Math.floor(balance / s.burn_per_day);
        if (want !== days) {
            fail(`scenario "${s.name}": ${label} runway says ${days} days, arithmetic says ${want}`);
            arithmeticBad += 1;
        }
    }
}
if (!arithmeticBad && (table.scenarios || []).length) {
    ok(`all ${table.scenarios.length} scenarios recompute from their own components`);
}

if (table.fallback_burn_per_day < m.idle_burn_per_day * 5) {
    // The floor a monitor alarms on when a canister cannot measure itself must be
    // the burn of a BUSY table. An idle-sized floor is how "I cannot say" becomes
    // "you have plenty".
    fail(`fallback_burn_per_day (${(table.fallback_burn_per_day / 1e12).toFixed(4)} T/day) is `
        + 'within five times the idle burn. A canister that cannot measure itself would be '
        + 'assumed idle, which flatters exactly the canister that has just been touched.');
} else {
    ok(`unknown-burn floor is ${(table.fallback_burn_per_day / 1e12).toFixed(4)} T/day, `
        + `${(table.fallback_burn_per_day / m.idle_burn_per_day).toFixed(0)}x idle`);
}

// ---------------------------------------------------------------------------
// 2. The consumers read it
// ---------------------------------------------------------------------------
const shPath = path.join(ROOT, 'scripts/cycles-runway.sh');
if (!fs.existsSync(shPath)) {
    fail('scripts/cycles-runway.sh is missing');
} else {
    const sh = fs.readFileSync(shPath, 'utf8');
    const hardcoded = sh.match(/^\s*FALLBACK_BURN_PER_DAY\s*=\s*[0-9_]+\s*$/m);
    if (hardcoded) {
        fail(`scripts/cycles-runway.sh hardcodes its fallback burn rate `
            + `(\`${hardcoded[0].trim()}\`). That literal is how the wrong number survived: `
            + 'nothing could correct it. It must read tools/cycles/burn-table.json.');
    } else if (!sh.includes('burn-table.json')) {
        fail('scripts/cycles-runway.sh does not read tools/cycles/burn-table.json');
    } else {
        ok('scripts/cycles-runway.sh reads the burn table rather than carrying a copy');
    }
}

// ---------------------------------------------------------------------------
// 3. No retired figure has come back
// ---------------------------------------------------------------------------
//
// Each entry is a string that was published as a burn rate or a runway and is now
// known to be wrong. Matching is literal: these are distinctive enough that a
// coincidental hit is not a real risk, and a coincidental hit is anyway a
// prompt to look.
const RETIRED = [
    ['0.4994', 'the "fully occupied table" burn that priced six open tabs as six heartbeats'],
    ['499412781032', 'the fallback burn rate derived from that figure'],
    ['499_412_781_032', 'the same, with separators'],
    ['0.0779', 'the 200 hands/day row from the same table'],
    ['0.2126', 'the 1000 hands/day row from the same table'],
];

// Code and operator-facing configuration only. docs/ narrates the history of
// these numbers and has to be able to quote them.
const SCANNED = [
    'scripts/cycles-runway.sh',
    'scripts/dev.sh',
    '.github/workflows/cycles-monitor.yml',
    'src/cleardeck_frontend/src/lib/cycleRunway.js',
    'src/cleardeck_frontend/src/routes/+page.svelte',
    'src/cleardeck_frontend/src/lib/components/DepositModal.svelte',
    'src/cleardeck_frontend/src/lib/components/WithdrawModal.svelte',
];

let stale = 0;
for (const rel of SCANNED) {
    const p = path.join(ROOT, rel);
    if (!fs.existsSync(p)) continue;
    const text = fs.readFileSync(p, 'utf8');
    for (const [needle, why] of RETIRED) {
        if (!text.includes(needle)) continue;
        const line = text.split('\n').findIndex((l) => l.includes(needle)) + 1;
        fail(`${rel}:${line} still states \`${needle}\` — ${why}.\n`
            + '          It reads LOW, which is the direction that makes a table look safer '
            + 'than it is. Take the figure from tools/cycles/burn-table.json.');
        stale += 1;
    }
}
if (!stale) ok(`no retired burn figure appears in any of the ${SCANNED.length} scanned files`);

if (failures) {
    console.error(`\n  test-burn-table FAILED (${failures} problem(s))`);
    process.exit(1);
}
console.log('  test-burn-table passed');
process.exit(0);
