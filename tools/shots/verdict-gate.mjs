// node tools/shots/verdict-gate.mjs   (`./scripts/dev.sh shots-verdict`)
//
// THE LAST RECORDED SCREENSHOT VERDICT, AS A GATE.
//
// The screenshot sweep already exits 1 when a scene is red. That was not enough,
// and the way it failed is worth stating exactly, because it is not carelessness:
//
//   * the sweep is the ONLY thing that can see a rendered-pixel failure;
//   * the sweep needs a live local replica;
//   * the replica has been unstartable for two waves;
//   * so no gate anybody could run was red, and a real regression -- 8.7% of a
//     money figure painted over by the felt (docs/DEFECTS.md E-63) -- sat inside
//     a 7.6 MB artifact for a whole wave with nobody acting on it.
//
// A gate that can only run in conditions that do not hold is not a gate. This one
// reads the verdict the LAST sweep recorded, which is a tracked file, so it runs
// on any machine with no replica, no browser and no canister -- and it stays red
// until somebody writes the failure down.
//
// THE TWO RULES, and the second matters as much as the first:
//
//   1. a red with no entry in artifacts/screens/acknowledged-reds.json FAILS, and
//      an entry must name a defect id that exists in docs/DEFECTS.md, so the only
//      way past this gate is to file the defect; and
//   2. an ENTRY THAT IS NO LONGER RED also fails. Otherwise the ledger becomes a
//      list of permanently-excused failures, which is how a suite everybody
//      expects red teaches everybody to ignore red -- the wave-6 REG-09 lesson,
//      and HARD RULE 7.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const VERDICTS = path.join(ROOT, 'artifacts', 'screens', 'latest', 'verdicts.json');
const LEDGER = path.join(ROOT, 'artifacts', 'screens', 'acknowledged-reds.json');
const DEFECTS = path.join(ROOT, 'docs', 'DEFECTS.md');

const problems = [];
const notes = [];

function readJson(file, what) {
    if (!fs.existsSync(file)) {
        problems.push(
            `${what} is missing: ${path.relative(ROOT, file)}. There is no recorded screenshot `
            + 'verdict to stand on, which is indistinguishable from a sweep that was never run. '
            + 'Run `./scripts/dev.sh shots`, or `node tools/shots/backfill-verdicts.mjs` if the '
            + "run's manifest.json is still on disk.",
        );
        return null;
    }
    try {
        return JSON.parse(fs.readFileSync(file, 'utf8'));
    } catch (e) {
        problems.push(`${what} is not readable JSON: ${e.message}`);
        return null;
    }
}

const verdicts = readJson(VERDICTS, 'the recorded screenshot verdict');
const ledger = readJson(LEDGER, 'the acknowledged-reds ledger');

if (verdicts && ledger) {
    const entries = Array.isArray(ledger.reds) ? ledger.reds : [];
    const defectsText = fs.existsSync(DEFECTS) ? fs.readFileSync(DEFECTS, 'utf8') : '';

    const matches = (entry, shot) =>
        entry.scene === shot.scene
        && (entry.viewport === '*' || entry.viewport === shot.viewport);

    const red = (verdicts.shots || []).filter((s) => !s.verified);
    const used = new Set();

    for (const shot of red) {
        const entry = entries.find((e) => matches(e, shot));
        if (!entry) {
            problems.push(
                `UNACKNOWLEDGED RED: ${shot.scene} / ${shot.viewport} is not verified in the last `
                + 'recorded sweep and nothing in artifacts/screens/acknowledged-reds.json names it. '
                + `Reasons recorded by the run:\n        - ${
                    (shot.problems || ['(no reason recorded)']).join('\n        - ')}`,
            );
            continue;
        }
        used.add(entry);
        if (!entry.defect) {
            problems.push(
                `${shot.scene} / ${shot.viewport} is acknowledged with no defect id. An entry `
                + 'without one is a shrug, not an acknowledgement.',
            );
        } else if (defectsText && !defectsText.includes(`### ${entry.defect},`)
            && !defectsText.includes(`### ${entry.defect} `)) {
            problems.push(
                `${shot.scene} / ${shot.viewport} is acknowledged under ${entry.defect}, which `
                + 'docs/DEFECTS.md does not carry. Filing the defect is the point of the '
                + 'acknowledgement.',
            );
        }
    }

    // Rule 2. An acknowledgement that outlives its red is a pin on a fixed defect.
    for (const entry of entries) {
        if (used.has(entry)) continue;
        const stillListed = (verdicts.shots || []).some((s) => matches(entry, s));
        problems.push(
            `STALE ACKNOWLEDGEMENT: ${entry.scene} / ${entry.viewport} (${entry.defect}) is `
            + `${stillListed ? 'GREEN in' : 'not present in'} the last recorded sweep, so this `
            + 'entry excuses nothing. Delete it. A ledger of failures that are no longer '
            + 'failing is how everybody learns to skim the ledger (HARD RULE 7).',
        );
    }

    notes.push(
        `recorded sweep: ${verdicts.gitSha}${verdicts.gitDirty ? ' (dirty)' : ''} at `
        + `${verdicts.startedAt}`,
    );
    // WHERE THIS GATE IS BLIND, SAID OUT LOUD RATHER THAN LEFT TO BE DISCOVERED.
    // It judges the last sweep, not this tree. A wave can change every pixel and
    // still pass on a verdict recorded three commits ago. It is not made a
    // FAILURE -- that would put a red on every dirty working tree and teach
    // people to skip the gate, which is the disease -- but it is never silent,
    // because "the screenshot gate is green" and "the screenshots describe this
    // code" are different claims and only the first one is being made.
    let head = null;
    try {
        head = (await import('node:child_process'))
            .execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: ROOT, encoding: 'utf8' })
            .trim();
    } catch { /* not a git tree; the note below just says so */ }
    if (head && verdicts.gitSha && !head.startsWith(verdicts.gitSha) && !verdicts.gitSha.startsWith(head)) {
        notes.push(
            `NOTE: this verdict describes ${verdicts.gitSha}, and HEAD is ${head}. It is evidence `
            + 'about that tree, not this one. Nothing here has SEEN the current code render.',
        );
    } else if (verdicts.gitDirty) {
        notes.push(
            'NOTE: the recorded sweep ran on a DIRTY tree, so it describes a state no commit '
            + 'holds.',
        );
    }
    notes.push(
        `${verdicts.counts?.verified ?? '?'}/${verdicts.counts?.shots ?? '?'} shots verified, `
        + `${red.length} red, ${entries.length} acknowledged`,
    );
    if (verdicts.faultInjection) {
        notes.push(`FAULT-INJECTION RUN (${verdicts.faultInjection.join(', ')}) — every scene is expected red`);
    }
}

for (const n of notes) console.log(`    ${n}`);
if (problems.length) {
    for (const p of problems) console.error(`    ! ${p}`);
    console.error(`\n    ${problems.length} problem(s) with the recorded screenshot verdict`);
    process.exit(1);
}
console.log('    every recorded red is acknowledged, and every acknowledgement is still red');
process.exit(0);
