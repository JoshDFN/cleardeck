#!/usr/bin/env node
// THE ONE NAMED PATH THAT PRODUCES A MAINNET BUNDLE.
//
// ===========================================================================
// WHY THIS IS A FILE AND NOT A FLAG SOMEBODY REMEMBERS
// ===========================================================================
//
// docs/DEFECTS.md T-01 was a bare `npm run build` that silently produced a bundle
// pointing a local dev UI at the live fund-holding canisters, because the id
// fallback chain ended at a repo-root `.env` holding the mainnet ids. Wave 4 made
// that impossible by refusing any build that does not state its target.
//
// This is the other direction, and it has the same requirement: a MAINNET build
// must be a deliberate, visible, named act, not an environment variable that
// somebody exported in a shell three commands ago and forgot. Concretely:
//
//   * The target is in the COMMAND (`npm run build:mainnet`), so it is in the
//     shell history, in CI logs and in whatever the operator pastes into a
//     handover note. `DFX_NETWORK=ic npm run build` is a sentence you can say by
//     accident; this is not.
//   * A conflicting DFX_NETWORK in the ambient environment is a HARD ERROR, not
//     something quietly overridden. Two statements of intent that disagree mean
//     the operator does not know what they are building, and that is the state
//     to stop in.
//   * The bundle is VERIFIED before this script exits 0, and if verification
//     fails the dist is DELETED. A failed mainnet build must not leave behind a
//     directory that `icp deploy` would happily upload.
//
// The build itself is still `vite build`; nothing here reimplements it.
//
// Usage:
//   npm run build:mainnet              build, verify, and print the deploy command
//   npm run build:mainnet -- --keep    keep an unverified dist for inspection

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { readMainnetIds } from './network-env.mjs';
import { verifyBundle } from './verify-bundle.mjs';

const FRONTEND_DIR = fileURLToPath(new URL('..', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../../..', import.meta.url));
const DIST = path.join(FRONTEND_DIR, 'dist');
const KEEP = process.argv.includes('--keep');

const RULE = '='.repeat(72);

function die(message) {
  console.error(`\n${RULE}\nMAINNET BUILD REFUSED\n${RULE}\n\n${message}\n`);
  process.exit(1);
}

// --- 1. refuse a contradictory environment ---------------------------------
//
// dotenv inside network-env.mjs does not override variables already present, and
// `.env` on this checkout sets DFX_NETWORK='ic'. So an ambient DFX_NETWORK=local
// would not be corrected by anything downstream; it would simply win, and this
// script would print "mainnet" while building local. Refuse instead.
for (const key of ['DFX_NETWORK', 'ICP_NETWORK', 'VITE_ICP_NETWORK']) {
  const value = process.env[key];
  if (value && value !== 'ic') {
    die(
      `${key}=${value} is set in this shell, but this script builds for MAINNET.\n\n`
      + 'Two statements of intent that disagree mean nobody knows which bundle this\n'
      + `would be. Unset it (\`unset ${key}\`) and run this again, or run the local\n`
      + 'build instead:  ./scripts/dev.sh local-up',
    );
  }
}

// --- 2. the ids this build is going to bake in, stated BEFORE the build ----
let mapping;
try {
  mapping = readMainnetIds(REPO_ROOT);
} catch (e) {
  die(e.message);
}

console.log(`\n${RULE}`);
console.log('BUILDING THE CLEARDECK FRONTEND FOR **INTERNET COMPUTER MAINNET**');
console.log(RULE);
console.log('\nThese canisters custody REAL ICP and ckBTC. The bundle this produces');
console.log('addresses them directly.\n');
for (const [role, id] of Object.entries(mapping.byRole)) {
  console.log(`    ${role.padEnd(13)} ${id}`);
}
console.log(`\n  ids from ${mapping.source} (tracked; this is what \`icp -e ic\` resolves)`);
console.log(`  lobby + history are WIRED; table ids are fetched from the lobby at runtime\n`);

// --- 3. the build ----------------------------------------------------------
const env = { ...process.env, DFX_NETWORK: 'ic', NODE_ENV: 'production' };
// `CANISTER_CANDID_PATH` in the repo-root .env is an absolute path on one
// developer's laptop. network-env.mjs strips it; belt and braces here so the
// value is not even in this process's environment.
delete env.CANISTER_CANDID_PATH;

try {
  execFileSync('npm', ['run', 'build'], {
    cwd: FRONTEND_DIR,
    env,
    stdio: 'inherit',
    timeout: 15 * 60_000,
  });
} catch (e) {
  die(`vite build failed: ${e.message}`);
}

// --- 4. verification, on the bytes, before anybody can deploy them ---------
console.log(`\n${RULE}`);
console.log('VERIFYING THE BUILT BUNDLE');
console.log(RULE);

let result;
try {
  result = verifyBundle({ network: 'ic', distDir: DIST });
} catch (e) {
  die(`the bundle could not be verified: ${e.message}`);
}

const width = Math.max(...result.checks.map((c) => c.name.length));
console.log('');
for (const c of result.checks) {
  console.log(`  ${c.ok ? '✓' : '✗'} ${c.name.padEnd(width)}   ${c.detail}`);
}

if (!result.ok) {
  const failed = result.checks.filter((c) => !c.ok).length;
  if (!KEEP) {
    fs.rmSync(DIST, { recursive: true, force: true });
    console.error(`\n  dist DELETED so it cannot be deployed by accident (--keep to inspect it).`);
  }
  die(
    `${failed} of ${result.checks.length} bundle checks failed.\n\n`
    + 'This bundle would have gone to the asset canister in front of real money.',
  );
}

console.log(`\n  ${result.checks.length} of ${result.checks.length} checks passed.`);
console.log(`\n${RULE}`);
console.log('READY TO DEPLOY');
console.log(RULE);
console.log(`
  bundle:  ${path.relative(REPO_ROOT, DIST)}
  target:  ${mapping.byRole.frontend ?? '(no frontend id in the mapping)'} (asset canister)

  Deploy it with:

      icp deploy -e ic frontend --mode upgrade -y

  RUN THIS IMMEDIATELY BEFORE THE DEPLOY. There is ONE dist directory and both
  build paths write to it: \`./scripts/dev.sh local-up\` and \`./scripts/dev.sh shots\`
  each rebuild it with LOCAL ids. A mainnet bundle left sitting while either of
  those runs is silently replaced by a local one. Nothing is lost if that happens
  -- \`npm run verify:bundle -- --network ic\` says so in one line, and this script
  rebuilds in seconds -- but do not deploy a dist you did not just build.

  Then confirm what actually went live — from OUTSIDE this repo, against the
  canister rather than against the build:

      curl -s https://${mapping.byRole.frontend}.icp0.io/ | grep -c 'ClearDeck'
      curl -s https://${mapping.byRole.frontend}.icp0.io/_app/immutable/nodes/ \\
        > /dev/null   # confirms the asset canister is serving the new tree

  The exact greps are printed by:

      node build/verify-deployed.mjs
`);
