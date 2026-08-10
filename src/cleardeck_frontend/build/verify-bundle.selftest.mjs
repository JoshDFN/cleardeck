#!/usr/bin/env node
// CAN THE BUNDLE VERIFIER FAIL?
//
// ===========================================================================
// WHY THIS EXISTS
// ===========================================================================
//
// The standing lesson of this repository, five waves running: every serious
// defect was invisible to an instrument built by somebody who knew the answer.
// This wave produced one within the hour. `verify-bundle.mjs` shipped its first
// green run with
//
//     ✓ no local replica canister id in a mainnet bundle   checked 0 distinct canister id(s)
//
// on a bundle containing eleven of them, because the principal regex had one
// wrong repetition count. Twelve of twelve passed. Nothing was being checked.
//
// A gate that has never been seen to fail is not evidence. So this takes the
// real built bundle, copies it, breaks it in each of the specific ways a bad
// mainnet build breaks, and asserts the verifier says NO — naming which check
// has to be the one that catches it. If a mutation stops being caught, this
// fails, whatever the main run says.
//
// Usage:  node build/verify-bundle.selftest.mjs [--dist <dir>]

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { verifyBundle } from './verify-bundle.mjs';

const FRONTEND_DIR = fileURLToPath(new URL('..', import.meta.url));
const SRC_DIST = (() => {
  const i = process.argv.indexOf('--dist');
  return i === -1 ? path.join(FRONTEND_DIR, 'dist') : path.resolve(process.argv[i + 1]);
})();

const SCRATCH = process.env.CLEARDECK_SCRATCH
  || fs.mkdtempSync(path.join(os.tmpdir(), 'cleardeck-verify-selftest-'));

/** The biggest JS chunk: where every string this verifier reads actually lives. */
function mainChunk(dir) {
  const stack = [dir];
  let best = null;
  while (stack.length) {
    const d = stack.pop();
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) stack.push(p);
      else if (p.endsWith('.js')) {
        const size = fs.statSync(p).size;
        if (!best || size > best.size) best = { path: p, size };
      }
    }
  }
  if (!best) throw new Error(`no JavaScript under ${dir}`);
  return best.path;
}

function freshCopy(label) {
  const dest = path.join(SCRATCH, label);
  fs.rmSync(dest, { recursive: true, force: true });
  fs.cpSync(SRC_DIST, dest, { recursive: true });
  return dest;
}

function edit(dir, fn) {
  const file = mainChunk(dir);
  const before = fs.readFileSync(file, 'utf8');
  const after = fn(before);
  if (after === before) throw new Error(`mutation made no change to ${path.basename(file)}`);
  fs.writeFileSync(file, after);
}

const NET_ANCHOR = /\["icp0\.io","ic0\.app","internetcomputer\.org"\]/;

/**
 * Rewrite the network literal(s) the bundle compiled in, whichever shape the
 * minifier emitted, so the mutation keeps biting when the source changes.
 *
 * @param {string} text the bundle chunk
 * @param {'ic'|'local'|null} to the network to claim; null strips the literals
 *   entirely, which is the "nothing was compiled in" bundle.
 * @returns {string}
 */
function rewriteNetworkLiterals(text, to) {
  const at = text.search(NET_ANCHOR);
  if (at === -1) return text;
  const head = text.slice(0, at);
  const tail = text.slice(at + 600);
  let win = text.slice(at, at + 600);

  // A. folded: function x(){return"ic"}
  win = win.replace(
    /(function\s+\w+\(\)\{return)"(?:ic|local)"(\})/,
    to === null ? '$1 void 0$2' : `$1"${to}"$2`,
  );
  // B. wrapped: (()=>"ic")
  win = win.replace(
    /\(\s*\(\s*\)\s*=>\s*"(?:ic|local)"\s*\)/g,
    to === null ? '(()=>{})' : `(()=>"${to}")`,
  );
  return head + win + tail;
}

/**
 * Each mutation: what it simulates, how to make it, and the check that MUST go
 * red. Naming the check is the point — a mutation caught by some unrelated
 * assertion is not evidence that the intended one works.
 */
const MUTATIONS = [
  {
    name: 'local lobby id wired into a mainnet bundle (docs/DEFECTS.md T-01, reversed)',
    apply: (dir) => edit(dir, (t) =>
      t.replace(/\("LOBBY","kpfcd-kyaaa-aaaaj-qor3a-cai"/, '("LOBBY","4zfnl-5t777-77775-aaadq-cai"')),
    mustFail: [
      'LOBBY is wired to the ic lobby (kpfcd-kyaaa-aaaaj-qor3a-cai)',
      'no local replica canister id in a mainnet bundle',
    ],
  },
  {
    name: 'HISTORY wired into the LOBBY slot (a real mainnet id, the wrong role)',
    apply: (dir) => edit(dir, (t) =>
      t.replace(/\("LOBBY","kpfcd-kyaaa-aaaaj-qor3a-cai"/, '("LOBBY","kggj7-4qaaa-aaaaj-qor2q-cai"')),
    mustFail: ['LOBBY is wired to the ic lobby (kpfcd-kyaaa-aaaaj-qor3a-cai)'],
  },
  {
    // Both emitted shapes, because the source has produced both and this
    // mutation silently stopped applying when it changed. See compiledNetwork()
    // in verify-bundle.mjs for the two shapes and why.
    name: 'the compiled network says local while everything else says mainnet',
    apply: (dir) => edit(dir, (t) => rewriteNetworkLiterals(t, 'local')),
    mustFail: ['compiled network is "ic"'],
  },
  {
    // THE DEFECT THAT WAS ACTUALLY LIVE, from the other side. A bundle with no
    // network compiled in falls back to sniffing window.location.hostname,
    // which is the guess T-01 forbids. The check that should have caught it was
    // reading for a shape the source no longer emitted, so it reported this
    // state on every CORRECT bundle instead -- 1 of 13 failing, `build:mainnet`
    // refusing, and nobody able to tell a real miss from the false one.
    name: 'nothing is compiled in at all (the app would sniff the hostname)',
    apply: (dir) => edit(dir, (t) => rewriteNetworkLiterals(t, null)),
    mustFail: ['compiled network is "ic"'],
  },
  {
    name: 'the mainnet agent gateway is gone (agentHost would have nothing to return)',
    apply: (dir) => edit(dir, (t) => t.replaceAll('https://icp-api.io', 'https://example.invalid')),
    mustFail: ['mainnet agent gateway is compiled in'],
  },
  {
    name: 'the Internet Identity URL lost its /authorize path',
    apply: (dir) => edit(dir, (t) => t.replaceAll('https://id.ai/authorize', 'https://id.ai')),
    mustFail: ['mainnet Internet Identity provider is compiled in, with /authorize'],
  },
  {
    name: 'a mainnet id is displayed under the wrong role in the Verify panel',
    apply: (dir) => edit(dir, (t) =>
      t.replace('table_2:"lfkaz-iiaaa-aaaaj-qor4a-cai"', 'table_2:"lclgn-fqaaa-aaaaj-qor4q-cai"')),
    mustFail: ['every mainnet id is shown under its real role'],
  },
  {
    name: "a build machine's home directory in the bundle",
    apply: (dir) => edit(dir, (t) =>
      `${t}\n//# sourceRoot=/Users/somebody/Desktop/cleardeck/src/cleardeck_frontend\n`),
    mustFail: ['no absolute path from the build machine'],
  },
  {
    name: 'the canister-id scan finds nothing (the bug this file was written for)',
    apply: (dir) => edit(dir, (t) => t.replaceAll(/-cai\b/g, '-caj')),
    mustFail: ['the canister-id scan actually found canister ids'],
  },
];

function run() {
  console.log(`\n  verifier self-test — mutating a copy of ${path.relative(FRONTEND_DIR, SRC_DIST)}\n`);

  // 0. The unmutated bundle must PASS, or every red below is meaningless.
  const clean = verifyBundle({ network: 'ic', distDir: SRC_DIST });
  if (!clean.ok) {
    console.error('  ✗ the unmutated bundle does not pass. Fix that before reading anything else:');
    for (const c of clean.checks.filter((x) => !x.ok)) console.error(`      ${c.name}: ${c.detail}`);
    process.exit(1);
  }
  console.log(`  ✓ baseline: the real bundle passes ${clean.checks.length} checks\n`);

  let failures = 0;
  for (const [i, m] of MUTATIONS.entries()) {
    const dir = freshCopy(`m${i}`);
    try {
      m.apply(dir);
    } catch (e) {
      console.error(`  ✗ ${m.name}\n      could not apply the mutation: ${e.message}`);
      failures += 1;
      continue;
    }
    const result = verifyBundle({ network: 'ic', distDir: dir });
    const red = new Set(result.checks.filter((c) => !c.ok).map((c) => c.name));
    const missed = m.mustFail.filter((name) => !red.has(name));

    if (result.ok) {
      console.error(`  ✗ ${m.name}\n      the verifier PASSED a bundle that is broken.`);
      failures += 1;
    } else if (missed.length) {
      console.error(
        `  ✗ ${m.name}\n      caught, but not by the check that is supposed to catch it.\n`
        + `      expected red: ${missed.join(', ')}\n`
        + `      actually red: ${[...red].join(', ')}`,
      );
      failures += 1;
    } else {
      console.log(`  ✓ ${m.name}\n      caught by: ${m.mustFail.join(', ')}`);
    }
  }

  console.log('');
  if (failures) {
    console.error(`  ${failures} of ${MUTATIONS.length} mutations were not caught properly.\n`);
    process.exit(1);
  }
  console.log(`  ${MUTATIONS.length} of ${MUTATIONS.length} mutations caught by the named check.\n`);
}

run();
