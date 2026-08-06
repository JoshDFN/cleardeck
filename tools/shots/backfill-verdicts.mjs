// node tools/shots/backfill-verdicts.mjs [dir ...]
//
// Projects an existing `manifest.json` into the small tracked `verdicts.json`.
//
// One-shot in intent but kept in the tree, because the situation it was written
// for recurs: `manifest.json` is now gitignored build output (docs/DEFECTS.md
// E-60), so a run whose manifest is on disk but whose verdicts were never written
// -- an older wave's artifacts, or a sweep interrupted after the manifest --
// would otherwise lose its evidence trail the moment somebody cleaned the
// directory. Nothing here re-judges anything: every verdict comes from the
// filename the sweep chose at the time.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildVerdicts } from './lib/verdicts.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SCREENS = path.resolve(HERE, '..', '..', 'artifacts', 'screens');

const dirs = process.argv.slice(2).length
    ? process.argv.slice(2)
    : fs.readdirSync(SCREENS)
        .map((d) => path.join(SCREENS, d))
        .filter((d) => fs.existsSync(path.join(d, 'manifest.json')));

if (!dirs.length) {
    console.error('no run directory with a manifest.json found');
    process.exit(1);
}

for (const dir of dirs) {
    const manifestPath = path.join(dir, 'manifest.json');
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    const verdicts = buildVerdicts(manifest);
    const out = path.join(dir, 'verdicts.json');
    fs.writeFileSync(out, `${JSON.stringify(verdicts, null, 2)}\n`);
    const rel = path.relative(path.resolve(HERE, '..', '..'), out);
    console.log(
        `${rel}  ${verdicts.counts.verified}/${verdicts.counts.shots} verified, `
        + `${verdicts.counts.red} red  (${fs.statSync(manifestPath).size} -> ${fs.statSync(out).size} bytes)`,
    );
}
