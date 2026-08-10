#!/usr/bin/env node
// DOES THE BUILT BUNDLE ACTUALLY POINT WHERE THE BUILD SAID IT WOULD?
//
// ===========================================================================
// WHY A SECOND CHECK, WHEN THE BUILD ALREADY CHECKS
// ===========================================================================
//
// `build/network-env.mjs` validates the ENVIRONMENT the build runs in. This
// validates the BYTES the build produced. They are different claims, and this
// project has already been bitten by treating the first as evidence of the
// second: docs/DEFECTS.md T-01 was a build whose inputs looked entirely ordinary
// and whose output silently addressed the live fund-holding canisters.
//
// The standing lesson of five waves here is that a gate written by somebody who
// knows the answer tends to ask the question the answer fits. So the checks below
// are deliberately written against the EMITTED JAVASCRIPT and its call sites, not
// against anything the build reports about itself, and two of them are designed
// to fail on the specific ways an id can be present but wrong:
//
//   PRESENCE IS NOT WIRING. All seven mainnet ids appear in EVERY bundle,
//   including local ones, because ic-config.js lists them as display-only text
//   for the "Verify the Code" panel. A check that greps for "is kpfcd-… in the
//   bundle?" passes on a local build. So the lobby and history ids are read out
//   of their actual call sites -- the `resolveCanisterId("LOBBY", <id>, …)`
//   arguments -- and compared there.
//
//   MEMBERSHIP IS NOT ROLE. `kpfcd-…` being "a ClearDeck mainnet id" does not
//   make it the LOBBY. Every id is checked against the role the committed
//   mapping gives it, so a bundle with history in the lobby slot fails.
//
// Exit code 0 means every assertion below passed on the files in `dist`.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { readMainnetIds } from './network-env.mjs';

const FRONTEND_DIR = fileURLToPath(new URL('..', import.meta.url));
const REPO_ROOT = fileURLToPath(new URL('../../..', import.meta.url));
const DEFAULT_DIST = path.join(FRONTEND_DIR, 'dist');

/**
 * Canister principals as they appear in a bundle.
 *
 * THE FIRST VERSION OF THIS LINE WAS `{4}` REPETITIONS, i.e. five groups before
 * `-cai`, and a canister id has FOUR (`kpfcd-kyaaa-aaaaj-qor3a-cai`). So it
 * matched nothing, `allPrincipals()` returned an empty map, and the
 * "no local replica canister id in a mainnet bundle" check reported
 *
 *     ✓ no local replica canister id in a mainnet bundle   checked 0 distinct canister id(s)
 *
 * on a bundle containing seven of them. A green tick over a scan of nothing.
 *
 * That is the standing lesson of this repo happening to the instrument written
 * to enforce it, so two things guard it now: the pattern is written to be
 * length-agnostic, and `verifyBundle` asserts a FLOOR on how many ids the scan
 * found. A scanner that finds nothing now fails instead of passing.
 */
const PRINCIPAL_RE = /\b[a-z0-9]{5}(?:-[a-z0-9]{5})*-cai\b/g;

/**
 * How many distinct canister ids any ClearDeck bundle must contain: the seven in
 * ic-config.js MAINNET_CANISTER_IDS, which ship in every build as display-only
 * text for the "Verify the Code" panel. Below this the scan is broken, whatever
 * it says about what it did not find.
 */
const MIN_PRINCIPALS_IN_ANY_BUNDLE = 7;

/**
 * Local pocket-ic principals. The literal ids come from the project's own local
 * mapping when it exists; the shape catches a local id from a DIFFERENT machine,
 * which is the case a literal list cannot see.
 */
const LOCAL_SHAPE_RE = /\b[a-z0-9]{5}(?:-[a-z0-9]{5})?-77775-[a-z0-9]{5}-cai\b/;

// ---------------------------------------------------------------------------
// Reading the bundle
// ---------------------------------------------------------------------------

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

/** @returns {{path:string, rel:string, text:string}[]} every text asset in dist */
export function readBundle(distDir) {
  if (!fs.existsSync(distDir)) {
    throw new Error(`No bundle at ${distDir}. Build it first.`);
  }
  const files = walk(distDir).filter((f) => /\.(js|html|css|json|json5|webmanifest)$/.test(f));
  if (files.length === 0) {
    throw new Error(`${distDir} contains no JavaScript, HTML or CSS. That is not a bundle.`);
  }
  return files.map((f) => ({
    path: f,
    rel: path.relative(distDir, f),
    text: fs.readFileSync(f, 'utf8'),
  }));
}

/**
 * The canister id a `resolveCanisterId(<NAME>, <id>, …)` call site was compiled
 * with. Minification renames the function but never the string literals, so the
 * call is found by its first argument.
 *
 * @returns {{values:string[], sites:number}} distinct ids seen, and how many sites
 */
export function wiredIdFor(name, files) {
  const re = new RegExp(`\\(\\s*["']${name}["']\\s*,\\s*["']([a-z0-9-]+)["']`, 'g');
  const values = new Set();
  let sites = 0;
  for (const f of files) {
    for (const m of f.text.matchAll(re)) {
      sites += 1;
      values.add(m[1]);
    }
  }
  return { values: [...values], sites };
}

/**
 * The display-only mainnet id map compiled from ic-config.js MAINNET_CANISTER_IDS.
 * Anchored on `btc_table_1`, the one role name that appears nowhere else.
 * @returns {Record<string,string>|null}
 */
export function displayedMainnetMap(files) {
  const re = /Object\.freeze\(\{((?:\s*\w+\s*:\s*"[a-z0-9-]+"\s*,?){5,})\}\)/g;
  for (const f of files) {
    for (const m of f.text.matchAll(re)) {
      const body = m[1];
      if (!body.includes('btc_table_1')) continue;
      /** @type {Record<string,string>} */
      const out = {};
      for (const pair of body.matchAll(/(\w+)\s*:\s*"([a-z0-9-]+)"/g)) out[pair[1]] = pair[2];
      if (out.lobby && out.history && out.btc_table_1) return out;
    }
  }
  return null;
}

/**
 * The network literal `compiledNetwork()` was built with.
 *
 * Anchored on MAINNET_HOSTNAMES, which is emitted verbatim and immediately
 * precedes the function. Anchoring on a bare `"ic"` would match any string in
 * the bundle.
 *
 * TWO EMITTED SHAPES, AND WHY BOTH ARE HERE
 * ---------------------------------------------------------------------------
 * This check read for ONE shape and the source stopped producing it, so
 * `npm run build:mainnet` refused every correct bundle for as long as that was
 * true -- 1 of 13 checks failing, CI red on every push, and T-01's only gate
 * measuring nothing while appearing to be strict. A false red is not the safe
 * direction: it is a gate everyone learns to route around.
 *
 *   A. FOLDED.  `function x(){return"ic"}`
 *      What vite emits when the value is read directly and the minifier can
 *      constant-fold the whole function.
 *
 *   B. WRAPPED.  `function x(){const e=As(()=>"ic")||As(()=>{})||As(()=>"ic");…}`
 *      What it emits today. `compiledNetwork()` in ic-config.js reads through
 *      `buildValue()`, which carries a try/catch so the module can also be
 *      imported by bare node -- its gates run the real derivation module
 *      outside vite. A try/catch is not foldable, so the literals survive as
 *      arrow bodies and the function itself never collapses.
 *
 * Shape B is detected by collecting the network literals that appear as arrow
 * returns inside the window. One distinct value is the compiled target. NONE
 * means nothing was compiled in and the app would fall back to sniffing the
 * hostname. MORE THAN ONE cannot be trusted at all -- a bundle does not target
 * two networks -- so it reports null rather than picking a winner.
 *
 * @returns {'ic'|'local'|null}
 */
export function compiledNetwork(files) {
  const anchor = /\["icp0\.io","ic0\.app","internetcomputer\.org"\]/;
  for (const f of files) {
    const at = f.text.search(anchor);
    if (at === -1) continue;
    // Wide enough to hold `buildValue` plus the wrapped function that follows
    // it. Shape B measured at ~230 chars; the surplus is safe because what is
    // matched below is specific, not "any occurrence of the word ic".
    const window = f.text.slice(at, at + 600);

    // A. folded
    const folded = window.match(/function\s+\w+\(\)\{return"(ic|local)"\}/);
    if (folded) return /** @type {'ic'|'local'} */ (folded[1]);

    // B. wrapped
    const wrapped = [...window.matchAll(/\(\s*\(\s*\)\s*=>\s*"(ic|local)"\s*\)/g)];
    const distinct = [...new Set(wrapped.map((m) => m[1]))];
    if (distinct.length === 1) return /** @type {'ic'|'local'} */ (distinct[0]);
    if (distinct.length > 1) return null;

    // Neither shape carries a literal: the `||` chain fell through to nothing.
    return null;
  }
  return null;
}

/** Every canister principal anywhere in the bundle, with the file it is in. */
export function allPrincipals(files) {
  /** @type {Map<string,Set<string>>} */
  const found = new Map();
  for (const f of files) {
    for (const m of f.text.matchAll(PRINCIPAL_RE)) {
      if (!found.has(m[0])) found.set(m[0], new Set());
      found.get(m[0]).add(f.rel);
    }
  }
  return found;
}

function readLocalIds() {
  for (const rel of ['.icp/cache/mappings/local.ids.json', '.icp/data/mappings/local.ids.json']) {
    const p = path.join(REPO_ROOT, rel);
    if (!fs.existsSync(p)) continue;
    try {
      return Object.values(JSON.parse(fs.readFileSync(p, 'utf8'))).map(String);
    } catch {
      /* an unreadable local mapping is not a reason to pass or fail; the shape
         check below still runs */
    }
  }
  return [];
}

// ---------------------------------------------------------------------------
// The assertions
// ---------------------------------------------------------------------------

/**
 * @param {{distDir?:string, network:'ic'|'local', expectLobby?:string, expectHistory?:string}} opts
 * @returns {{ok:boolean, checks:{name:string, ok:boolean, detail:string}[]}}
 */
export function verifyBundle(opts) {
  const distDir = opts.distDir || DEFAULT_DIST;
  const network = opts.network;
  const files = readBundle(distDir);
  const { byRole: mainnetByRole } = readMainnetIds(REPO_ROOT);
  const mainnetIds = Object.values(mainnetByRole);
  const localIds = readLocalIds();

  /** @type {{name:string, ok:boolean, detail:string}[]} */
  const checks = [];
  const check = (name, ok, detail) => checks.push({ name, ok: Boolean(ok), detail });

  check(
    'bundle is present and non-trivial',
    files.length >= 3 && files.some((f) => f.rel.endsWith('index.html')),
    `${files.length} text asset(s) under ${path.relative(REPO_ROOT, distDir)}`,
  );

  // --- 1. the compiled target network -------------------------------------
  const net = compiledNetwork(files);
  check(
    `compiled network is "${network}"`,
    net === network,
    net === null
      ? 'no network literal compiled in: the bundle would fall back to sniffing '
        + 'window.location.hostname, which is exactly the guess docs/DEFECTS.md T-01 forbids'
      : `compiled network = ${net}`,
  );

  // --- 2. the wiring, read out of its call sites --------------------------
  const expectLobby = opts.expectLobby
    ?? (network === 'ic' ? mainnetByRole.lobby : undefined);
  const expectHistory = opts.expectHistory
    ?? (network === 'ic' ? mainnetByRole.history : undefined);

  for (const [role, expected] of [['LOBBY', expectLobby], ['HISTORY', expectHistory]]) {
    const wired = wiredIdFor(role, files);
    if (wired.sites === 0) {
      check(`${role} is wired`, false, 'no resolveCanisterId call site found in the bundle');
      continue;
    }
    check(
      `${role} is wired to exactly one id`,
      wired.values.length === 1,
      `${wired.sites} call site(s), ${wired.values.length} distinct id(s): ${wired.values.join(', ')}`,
    );
    if (expected) {
      check(
        `${role} is wired to the ${network} ${role.toLowerCase()} (${expected})`,
        wired.values.length === 1 && wired.values[0] === expected,
        `wired to ${wired.values.join(', ') || '(nothing)'}`,
      );
    }
  }

  // --- 3. the display map agrees with the committed mapping ---------------
  const displayed = displayedMainnetMap(files);
  if (!displayed) {
    check('mainnet id table is in the bundle', false, 'MAINNET_CANISTER_IDS not found in dist');
  } else {
    const drift = Object.entries(mainnetByRole)
      .filter(([role, id]) => displayed[role] !== id)
      .map(([role, id]) => `${role}: bundle says ${displayed[role] ?? '(absent)'}, mapping says ${id}`);
    check(
      'every mainnet id is shown under its real role',
      drift.length === 0,
      drift.length ? drift.join('; ') : `${Object.keys(mainnetByRole).length} role(s) agree with .icp/data/mappings/ic.ids.json`,
    );
  }

  // --- 4. no foreign canister ids -----------------------------------------
  const principals = allPrincipals(files);

  // THE SCANNER HAS TO PROVE IT CAN SEE BEFORE ITS SILENCE MEANS ANYTHING.
  // Without this line a broken pattern reports "no local ids found" and passes.
  // It did exactly that once; see the note on PRINCIPAL_RE.
  check(
    'the canister-id scan actually found canister ids',
    principals.size >= MIN_PRINCIPALS_IN_ANY_BUNDLE,
    `${principals.size} distinct id(s) found; every bundle carries at least `
      + `${MIN_PRINCIPALS_IN_ANY_BUNDLE} (the display-only mainnet table). Fewer means the `
      + 'scan is broken, and a broken scan finds no leaks either.',
  );

  if (network === 'ic') {
    const leaked = [...principals.keys()].filter(
      (id) => localIds.includes(id) || LOCAL_SHAPE_RE.test(id),
    );
    check(
      'no local replica canister id in a mainnet bundle',
      leaked.length === 0,
      leaked.length
        ? leaked.map((id) => `${id} in ${[...principals.get(id)].join(', ')}`).join('; ')
        : `checked ${principals.size} distinct canister id(s)`,
    );
  } else {
    const wiredValues = ['LOBBY', 'HISTORY'].flatMap((r) => wiredIdFor(r, files).values);
    const wiredMainnet = wiredValues.filter((id) => mainnetIds.includes(id));
    check(
      'no mainnet canister id is WIRED in a local bundle',
      wiredMainnet.length === 0,
      wiredMainnet.length ? wiredMainnet.join(', ') : `wired: ${wiredValues.join(', ')}`,
    );
  }

  // --- 5. the hosts the agent and the sign-in flow will use ---------------
  const joined = files.map((f) => f.text).join('\n');
  if (network === 'ic') {
    check(
      'mainnet agent gateway is compiled in',
      joined.includes('https://icp-api.io'),
      'IC_HOST = https://icp-api.io',
    );
    check(
      'mainnet Internet Identity provider is compiled in, with /authorize',
      joined.includes('https://id.ai/authorize'),
      'II_URL = https://id.ai/authorize (the bare origin opens account management '
        + 'and never completes the delegation handshake)',
    );
    // `agentHost()` is `isMainnet() ? IC_HOST : LOCAL_HOST`, so the local host
    // string is still in the file. Check 1 is what proves which branch is taken;
    // this only catches a build that has NO mainnet host to choose.
    check(
      'the local gateway is not the only host in the bundle',
      joined.includes('https://icp-api.io'),
      'the mainnet host must be present for agentHost() to return it',
    );
  }

  // --- 6. nothing from the build machine ----------------------------------
  const homePaths = [...joined.matchAll(/\/(?:Users|home)\/[A-Za-z0-9._-]+\/[^"'`\s)]{4,}/g)]
    .map((m) => m[0]);
  check(
    'no absolute path from the build machine',
    homePaths.length === 0,
    homePaths.length ? [...new Set(homePaths)].slice(0, 4).join('; ') : 'none',
  );

  return { ok: checks.every((c) => c.ok), checks };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const out = { network: undefined, distDir: undefined };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === '--network') out.network = argv[i + 1];
    if (argv[i] === '--dist') out.distDir = path.resolve(argv[i + 1]);
  }
  return out;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const network = args.network || process.env.DFX_NETWORK || process.env.ICP_NETWORK;
  if (network !== 'ic' && network !== 'local') {
    console.error(
      'verify-bundle: say which network the bundle claims to be for.\n'
      + '  node build/verify-bundle.mjs --network ic\n'
      + '  node build/verify-bundle.mjs --network local\n'
      + 'There is no default, for the same reason the build has none: a verifier '
      + 'that guesses its own expectation verifies nothing.',
    );
    process.exit(2);
  }

  let result;
  try {
    result = verifyBundle({ network, distDir: args.distDir });
  } catch (e) {
    console.error(`verify-bundle: ${e.message}`);
    process.exit(2);
    return;
  }

  const width = Math.max(...result.checks.map((c) => c.name.length));
  console.log(`\n  bundle verification — target network "${network}"\n`);
  for (const c of result.checks) {
    console.log(`  ${c.ok ? '✓' : '✗'} ${c.name.padEnd(width)}   ${c.detail}`);
  }
  const failed = result.checks.filter((c) => !c.ok);
  if (failed.length) {
    console.error(
      `\n  ${failed.length} of ${result.checks.length} checks FAILED. This bundle must not be `
      + 'deployed.\n',
    );
    process.exit(1);
  }
  console.log(`\n  ${result.checks.length} of ${result.checks.length} checks passed.\n`);
}

if (import.meta.url === `file://${process.argv[1]}`) main();
