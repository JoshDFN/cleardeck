// node tools/shots/selftest.mjs   (`./scripts/dev.sh shots-selftest`, `npm run selftest`)
//
// ONE LIST OF THE SCREENSHOT HARNESS'S OWN SELF-TESTS — docs/DEFECTS.md H-47.
//
// There were two, and they had drifted:
//
//   tools/shots/package.json  "selftest":
//       occlusion  census  money  rake  solvency
//   scripts/dev.sh  cmd_shots_selftest  (what `./scripts/dev.sh test` runs):
//       occlusion  census  money  rake  dock-overflow
//
// `test-solvency` was in neither the primary gate nor `make test`; `test-dock-overflow`,
// the E-63 gate, was not in the command the harness's own package file offers. H-38
// was closed by wiring `cmd_shots_selftest` into `cmd_test`, and the wiring COPIED the
// list rather than calling the one that already existed. Wave 12 then added
// `test-table-in-frame` and, if nothing had changed, would have made it three lists.
//
// So there is now one runner and both callers invoke it.
//
// IT DISCOVERS, AND IT ALSO REQUIRES. Discovery alone is how six money-safety suites
// came to be cargo-auto-discovered and named by no target (H-45) -- discovery answers
// "did anything get missed", not "did anything disappear". So `REQUIRED` names every
// self-test that must exist, and a missing one is a failure even though a missing file
// cannot be discovered. Discovery catches the file nobody wired; the list catches the
// file somebody deleted.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const HERE = path.dirname(fileURLToPath(import.meta.url));

/**
 * Every self-test that must be on disk, with the gate it holds. Adding a
 * `test-*.mjs` file does NOT require touching this list -- discovery will run it
 * either way -- but deleting one that is named here fails.
 */
const REQUIRED = {
    'test-occlusion.mjs': 'the pixel gate, on overlaps whose answer is in the fixture',
    'test-census.mjs': 'the inverted money gate: a token nothing asserts must fail a scene',
    'test-money.mjs': 'the display-vs-e8s parser',
    'test-rake.mjs': 'THE NO-RAKE GATE (docs/DEFECTS.md E-61): red on a rake of 1 e8',
    'test-dock-overflow.mjs': "the action dock's containment (docs/DEFECTS.md E-63)",
    'test-solvency.mjs': 'the solvency banner\'s own arithmetic',
    'test-table-in-frame.mjs':
        'the pot, board, seat pods and dock buttons in frame (docs/DEFECTS.md H-51)',
    'test-cycle-runway.mjs':
        'THE CYCLE-RUNWAY WARNING (docs/DEFECTS.md E-55): red when a null runway, an '
        + 'unreachable canister or a reply from the older module deployed on mainnet reads as '
        + '"fine" on the screen a player deposits from',
    'test-hand-identity.mjs':
        'THE HISTORY-CHAIN JOIN (docs/DEFECTS.md E-71): red when a hand is matched to the '
        + 'archive by hand_number, which answers to 70 records',
};

const discovered = fs.readdirSync(HERE)
    .filter((f) => f.startsWith('test-') && f.endsWith('.mjs'))
    .sort();

const missing = Object.keys(REQUIRED).filter((f) => !discovered.includes(f));
if (missing.length) {
    console.error(`  FATAL: these self-tests are named as required and are not on disk: ${
        missing.join(', ')}`);
    console.error('  A gate that has been deleted cannot be discovered, which is why the list exists.');
    process.exit(1);
}

console.log(`  ${discovered.length} self-test file(s) discovered in tools/shots:`);
for (const f of discovered) {
    console.log(`    ${f}${REQUIRED[f] ? `  — ${REQUIRED[f]}` : '  — (not on the required list)'}`);
}
console.log('');

const failed = [];
for (const f of discovered) {
    const r = spawnSync(process.execPath, [path.join(HERE, f)], { stdio: 'inherit' });
    if (r.status !== 0) failed.push(f);
}

if (failed.length) {
    console.error(`\n  FAILED: ${failed.join(', ')}`);
    process.exit(1);
}
console.log(`\n  all ${discovered.length} screenshot-harness self-tests green`);
process.exit(0);
