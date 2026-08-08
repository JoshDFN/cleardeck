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
//
// RULE 3, ADDED IN WAVE 12 (docs/DEFECTS.md E-87), because rules 1 and 2 were
// both green over a defect nobody had ever seen.
//
//   An acknowledgement used to be granted per (scene, viewport). So once a shot
//   was red for ANY reason, every OTHER failure that later appeared on the same
//   shot was absorbed in silence. That is not hypothetical: `table-in-frame.mjs`
//   landed in wave 12 and its first real conviction was a Leave-table control
//   rendering 6-7 px outside a 1440 px window on `table-preflop` and
//   `table-facing-bet` at desktop -- a control a player cannot reach. Both shots
//   were already acknowledged under E-76 for a felt-area shortfall of 0.2 points.
//   The new failure joined the same `problems` array and this gate stayed green.
//   Correct count of reds, wrong reds, every invariant silent.
//
//   So an entry must now list `covers`: the substrings of the recorded problems
//   it actually accounts for. Every recorded problem on an acknowledged shot must
//   match at least one of them, and every `covers` string must match at least one
//   recorded problem. The second half is rule 2 at the level of the problem
//   rather than the shot: a cover that stops matching is a cover that is excusing
//   nothing, and it fails rather than lingering.

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
        // `defect` may name more than one id, because one shot can carry more than
        // one distinct failure -- which is the whole of rule 3 below.
        const ids = Array.isArray(entry.defect) ? entry.defect
            : (entry.defect ? [entry.defect] : []);
        if (ids.length === 0) {
            problems.push(
                `${shot.scene} / ${shot.viewport} is acknowledged with no defect id. An entry `
                + 'without one is a shrug, not an acknowledgement.',
            );
        }
        for (const id of ids) {
            if (defectsText && !defectsText.includes(`### ${id},`)
                && !defectsText.includes(`### ${id} `)
                && !defectsText.includes(`### ${id} —`)) {
                problems.push(
                    `${shot.scene} / ${shot.viewport} is acknowledged under ${id}, which `
                    + 'docs/DEFECTS.md does not carry. Filing the defect is the point of the '
                    + 'acknowledgement.',
                );
            }
        }

        // RULE 3. What, exactly, is being excused.
        const recorded = shot.problems || [];
        const covers = Array.isArray(entry.covers) ? entry.covers : null;
        if (!covers || covers.length === 0) {
            problems.push(
                `${shot.scene} / ${shot.viewport} (${ids.join(', ')}) has no "covers" list. An `
                + 'acknowledgement is granted per PROBLEM, not per shot: without one, the next '
                + 'failure to appear on this shot is excused by an entry that was never about it '
                + '(docs/DEFECTS.md E-87). Recorded problems:\n        - '
                + (recorded.length ? recorded.join('\n        - ') : '(none recorded)'),
            );
            continue;
        }
        // A cover so short it matches anything is an acknowledgement of nothing.
        for (const c of covers) {
            if (typeof c !== 'string' || c.trim().length < 12) {
                problems.push(
                    `${shot.scene} / ${shot.viewport} (${ids.join(', ')}) has the cover ${
                        JSON.stringify(c)}, which is too short to identify a failure. Quote enough `
                    + 'of the recorded problem to name it (12 characters minimum).',
                );
            }
        }
        for (const p of recorded) {
            if (!covers.some((c) => typeof c === 'string' && c.length >= 12 && p.includes(c))) {
                problems.push(
                    `UNCOVERED PROBLEM on an acknowledged shot: ${shot.scene} / ${shot.viewport} `
                    + `is acknowledged under ${ids.join(', ')}, and this recorded problem is not `
                    + `accounted for by any of its "covers":\n        - ${p}`,
                );
            }
        }
        for (const c of covers) {
            if (typeof c === 'string' && c.length >= 12
                && !recorded.some((p) => p.includes(c))) {
                problems.push(
                    `STALE COVER: ${shot.scene} / ${shot.viewport} (${ids.join(', ')}) lists the `
                    + `cover ${JSON.stringify(c)}, and no recorded problem on that shot contains `
                    + 'it. Rule 2 at the level of the problem: delete it.',
                );
            }
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
