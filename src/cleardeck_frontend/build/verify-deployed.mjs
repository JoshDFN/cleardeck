#!/usr/bin/env node
// WHAT ACTUALLY WENT LIVE — the checks to run AFTER `icp deploy -e ic frontend`.
//
// ===========================================================================
// WHY THIS PRINTS COMMANDS INSTEAD OF RUNNING THEM
// ===========================================================================
//
// A deploy verification that runs inside the repo that produced the deploy is
// one party checking itself. Every command below reads the LIVE asset canister
// over ordinary HTTPS, from outside, with no ClearDeck code in the path, so the
// operator (or anyone) can run them from any machine and get an answer that does
// not depend on this checkout being what they think it is.
//
// It is also the honest division of labour for whoever built the bundle: the
// build machine in this project is not permitted to call mainnet, so it prints
// what to check rather than claiming to have checked it. `--run` exists for the
// operator, who is; it is never used by the build.
//
// Usage:
//   node build/verify-deployed.mjs          print the checks
//   node build/verify-deployed.mjs --run    run them (CONTACTS MAINNET)

import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { readMainnetIds } from './network-env.mjs';
import { verifyBundle } from './verify-bundle.mjs';

const FRONTEND_DIR = fileURLToPath(new URL('..', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../../..', import.meta.url));
const DIST = path.join(FRONTEND_DIR, 'dist');
const RUN = process.argv.includes('--run');

const { byRole } = readMainnetIds(REPO_ROOT);
const FRONTEND = byRole.frontend;
const ORIGIN = `https://${FRONTEND}.icp0.io`;

/**
 * Each check is a shell one-liner and the answer that means "the new bundle is
 * live". They are greps rather than eyeballing because the failure this catches
 * -- a deploy that reported success and left the OLD bundle in place -- looks
 * identical to a successful one in a browser with a warm cache.
 */
// Fetch every JS chunk the live index references, once, into one file. Every
// check below greps THAT, so the six checks read one snapshot of the deployment
// rather than six.
//
// COUNTING NOTE, and it is not a nit: the bundle is minified to a handful of very
// long lines, so `grep -c` (which counts LINES) answers 1 for a phrase that
// occurs five times and 1 for a phrase that occurs once. Every count below is
// `grep -o … | wc -l`, which counts OCCURRENCES. The first draft of this file
// used `grep -c` and would have reported "1" for the five protected phrases and
// called it a pass at `>= 5`... by failing. Same trap, opposite direction.
const SNAPSHOT = `
  # one snapshot of what is actually being served
  d=$(mktemp -d)
  curl -sL ${ORIGIN}/ > "$d/index.html"
  for f in $(grep -o '/_app/immutable/[^"]*\\.js' "$d/index.html" | sort -u); do
    curl -sL "${ORIGIN}$f"; echo
  done > "$d/bundle.js"
  wc -c "$d/index.html" "$d/bundle.js"`;

const CHECKS = [
  {
    what: 'the live page is wired to the live LOBBY',
    why: 'the single string whose absence means the bundle is unwired or stale',
    cmd: `grep -o '${byRole.lobby}' "$d/bundle.js" | wc -l`,
    expect: '>= 1',
  },
  {
    what: 'the live page is NOT wired to any local replica canister',
    why: 'docs/DEFECTS.md T-01, in the direction that reaches a stranger',
    cmd: `grep -oE '[a-z0-9]{5}(-[a-z0-9]{5})?-77775-[a-z0-9]{5}-cai' "$d/bundle.js" | wc -l`,
    expect: 'exactly 0',
  },
  {
    what: 'the live page says it is on mainnet',
    why: 'the compiled network literal reached the SERVED bytes, not just the build log',
    cmd: `grep -o 'IC mainnet' "$d/bundle.js" | wc -l`,
    expect: '>= 1',
  },
  {
    what: 'the agent host is the mainnet gateway and sign-in is mainnet II',
    why: 'a bundle served from mainnet that still points its agent at 127.0.0.1 renders '
      + 'perfectly and can do nothing',
    cmd: `grep -o 'https://icp-api.io' "$d/bundle.js" | wc -l; `
      + `grep -o 'https://id.ai/authorize' "$d/bundle.js" | wc -l`,
    expect: 'both >= 1',
  },
  {
    what: 'all five protected phrases reached the served page',
    why: 'HARD RULE 2, source half. The ON SCREEN half is the sweep, below — this project '
      + 'has already had a wave where all five were in the bundle and none were visible',
    cmd: `for p in 'Unaudited code with known bugs' 'your funds are NOT safe' \\\n`
      + `         'illegal in many jurisdictions' '18+ only' \\\n`
      + `         'No rake is taken from any pot on any table'; do\n`
      + `  printf '%-45s %s\\n' "$p" "$(grep -o "$p" "$d/bundle.js" | wc -l)"\n`
      + `done`,
    expect: 'every one >= 1',
  },
  {
    what: 'the served bundle is the one that was just built',
    why: 'a deploy can report success and leave the old assets in place',
    cmd: `diff <(grep -o '/_app/immutable/entry/[^"]*' "$d/index.html" | sort -u) \\\n`
      + `     <(grep -o '/_app/immutable/entry/[^"]*' ${path.relative(REPO_ROOT, DIST)}/index.html | sort -u)`,
    expect: 'no output (the live entry chunk is the built one)',
  },
];

function printPlan() {
  console.log(`
========================================================================
AFTER THE DEPLOY: what to run, and what the answer has to be
========================================================================

  asset canister : ${FRONTEND}
  served at      : ${ORIGIN}

  Deploy command (the one this bundle was built for):

      icp deploy -e ic frontend --mode upgrade -y

  Take ONE snapshot of what is being served, then run every check against it:
${SNAPSHOT}
`);
  for (const [i, c] of CHECKS.entries()) {
    console.log(`  ${i + 1}. ${c.what}`);
    console.log(`     why: ${c.why}`);
    console.log(`     $ ${c.cmd}`);
    console.log(`     expect: ${c.expect}\n`);
  }
  console.log(`  Then the on-screen half, which no grep can answer:

      ./scripts/dev.sh shots          (against the LOCAL build, for regressions)
      open ${ORIGIN}/    (and read the banner at 390x844)

  And the code-identity half, which anyone can run against mainnet:

      ./scripts/verify-build.sh --mainnet
`);
}

function localBundleState() {
  try {
    const r = verifyBundle({ network: 'ic', distDir: DIST });
    console.log(
      `  local dist: ${r.ok ? 'VERIFIED for mainnet' : 'FAILS mainnet verification'} `
      + `(${r.checks.filter((c) => c.ok).length}/${r.checks.length} checks)\n`,
    );
  } catch (e) {
    console.log(`  local dist: not verifiable — ${e.message}\n`);
  }
}

printPlan();
localBundleState();

if (RUN) {
  console.error(
    '  --run contacts mainnet. It is deliberately not implemented here: paste the\n'
    + '  commands above into a shell, so the thing being checked and the thing doing\n'
    + '  the checking are not the same process.\n',
  );
  process.exit(2);
}
