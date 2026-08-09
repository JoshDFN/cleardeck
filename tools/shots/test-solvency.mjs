#!/usr/bin/env node
// THE SOLVENCY READER, ON FIXTURES WHOSE ANSWER IS KNOWN BY CONSTRUCTION.
//
// ===========================================================================
// WHY THIS EXISTS AND WHAT IT IS AFRAID OF
// ===========================================================================
//
// `src/cleardeck_frontend/src/lib/solvency.js` renders the answer to "can this
// table pay back what it is holding?" on the two screens a player moves money
// from. Today the canister has no solvency surface at all
// (docs/SECURITY-FINDINGS.md FINDING 35 is open), so the ONLY state the running
// app can produce is `unsupported`.
//
// That is the trap. The other four states — `short`, `covered`, `unknown`,
// `error` — are the ones that will matter the day the canister half lands, and
// on that day nobody will re-derive whether the interpreter reads the record
// correctly. They would ship having never been executed. This repository has a
// name for a check like that: an instrument built by somebody who knew the
// answer.
//
// So each state is driven here against a SYNTHETIC CANDID SERVICE, built with
// the same `@dfinity/candid` the app uses, and the assertions are chosen so that
// the dangerous direction fails loudly:
//
//   * a canister that has never read its ledger must be `unknown`, NEVER `covered`
//   * a record the interpreter cannot understand must be `unknown`, NEVER `covered`
//   * `covered` must require an actual observation with an actual timestamp
//
// Usage:  node tools/shots/test-solvency.mjs

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { IDL } from '@dfinity/candid';
import { Principal } from '@dfinity/principal';

// `solvency.js` imports `declarations/table_1/table_1.did.js`, a VITE ALIAS that
// Node knows nothing about. Rather than mock the Candid — which would make this
// test agree with a fiction instead of with the shipped interface — resolve the
// alias the way Node can: a one-file package shim inside the frontend workspace's
// node_modules, pointing at the real generated declarations.
//
// Created here rather than committed, because `npm install` deletes anything in
// node_modules it does not own, and a test that fails after an install with
// ERR_MODULE_NOT_FOUND is a test people delete.
const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
const SHIM = path.join(REPO_ROOT, 'src/cleardeck_frontend/node_modules/declarations');
fs.mkdirSync(SHIM, { recursive: true });
fs.writeFileSync(
  path.join(SHIM, 'package.json'),
  `${JSON.stringify({ name: 'declarations', type: 'module', exports: { './*': './*' } }, null, 2)}\n`,
);
for (const name of fs.readdirSync(path.join(REPO_ROOT, 'src/declarations'))) {
  const link = path.join(SHIM, name);
  const target = path.join(REPO_ROOT, 'src/declarations', name);
  try { fs.rmSync(link, { recursive: true, force: true }); } catch { /* first run */ }
  fs.symlinkSync(target, link);
}

const {
  SOLVENCY_STATES, describeSolvencySurface, interpretSolvency, observationAge,
  readTableSolvency, severityOf, shouldWarnBeforeDeposit,
} = await import('../../src/cleardeck_frontend/src/lib/solvency.js');

let passed = 0;
const failures = [];
function check(name, fn) {
  try {
    fn();
    passed += 1;
    console.log(`  ✓ ${name}`);
  } catch (e) {
    failures.push(`${name}\n      ${e.message.split('\n')[0]}`);
    console.error(`  ✗ ${name}\n      ${e.message.split('\n').slice(0, 4).join('\n      ')}`);
  }
}
async function checkAsync(name, fn) {
  try {
    await fn();
    passed += 1;
    console.log(`  ✓ ${name}`);
  } catch (e) {
    failures.push(`${name}\n      ${e.message.split('\n')[0]}`);
    console.error(`  ✗ ${name}\n      ${e.message.split('\n').slice(0, 4).join('\n      ')}`);
  }
}

// ---------------------------------------------------------------------------
// Fixture services. These are what the canister half is expected to look like,
// written three plausible ways, because the point of discovery-by-Candid is that
// it must not depend on one exact spelling.
// ---------------------------------------------------------------------------

const solvencyRecord = IDL.Record({
  observed_main_balance: IDL.Nat64,
  total_liability: IDL.Nat64,
  shortfall: IDL.Nat64,
  observed_at_ns: IDL.Opt(IDL.Nat64),
  advice: IDL.Text,
});

const FIXTURES = {
  canonical: () => IDL.Service({
    get_solvency: IDL.Func([], [solvencyRecord], ['query']),
    refresh_main_account_custody: IDL.Func([], [IDL.Nat64], []),
    get_balance: IDL.Func([], [IDL.Nat64], ['query']),
  }),
  // No `shortfall`; the two figures only. The interpreter must derive it.
  derived: () => IDL.Service({
    get_reserve_status: IDL.Func([], [IDL.Record({
      main_account_balance: IDL.Nat64,
      liabilities: IDL.Nat64,
      checked_at_ns: IDL.Opt(IDL.Nat64),
    })], ['query']),
  }),
  // Wrapped in a Result, which a controller-scoped surface would be.
  wrapped: () => IDL.Service({
    get_table_solvency: IDL.Func([], [IDL.Variant({
      Ok: solvencyRecord,
      Err: IDL.Text,
    })], ['query']),
  }),
  // TODAY. No solvency surface anywhere.
  none: () => IDL.Service({
    get_balance: IDL.Func([], [IDL.Nat64], ['query']),
    get_custody_status: IDL.Func([], [IDL.Record({
      escrow: IDL.Nat64, chips_at_table: IDL.Nat64, total: IDL.Nat64, advice: IDL.Text,
    })], ['query']),
  }),
  // A NEAR MISS: the name matches, the payload does not carry the two numbers.
  // Must NOT be adopted — wiring the UI to this would show a confident nothing.
  decoy: () => IDL.Service({
    get_solvency_docs_url: IDL.Func([], [IDL.Text], ['query']),
  }),
};

// ---------------------------------------------------------------------------
// THE SHIPPED SURFACE, field for field.
//
// Transcribed from `src/table_canister/table_canister.did` (`SolvencyReport`,
// `SolvencyVerdict`, `get_solvency`, `refresh_solvency`). If the canister renames
// a field, these assertions go red, which is the point: the frontend discovers
// this surface structurally and the discovery has to be pinned to something.
// ---------------------------------------------------------------------------

const SolvencyVerdictT = IDL.Variant({
  CanPayEveryone: IDL.Null, CannotPayEveryone: IDL.Null, Unknown: IDL.Null,
});

const SolvencyReportT = IDL.Record({
  currency: IDL.Text,
  ledger: IDL.Principal,
  as_of_ns: IDL.Nat64,
  escrow: IDL.Nat64,
  chips_at_table: IDL.Nat64,
  pot: IDL.Nat64,
  committed_stake: IDL.Nat64,
  unswept_deposits: IDL.Nat64,
  unfinished_incoming: IDL.Nat64,
  pulls_in_flight: IDL.Nat64,
  sweep_fees_in_flight: IDL.Nat64,
  payouts_in_flight: IDL.Nat64,
  owed: IDL.Nat64,
  main_account: IDL.Opt(IDL.Nat64),
  main_observed_at_ns: IDL.Opt(IDL.Nat64),
  main_ledger: IDL.Opt(IDL.Principal),
  main_credited_since_reading: IDL.Nat64,
  main_debited_since_reading: IDL.Nat64,
  deposit_subaccounts: IDL.Nat64,
  deposit_accounts_observed: IDL.Nat64,
  deposit_oldest_observed_at_ns: IDL.Opt(IDL.Nat64),
  deposit_accounts_never_observed: IDL.Vec(IDL.Principal),
  held: IDL.Opt(IDL.Nat64),
  difference_e8s: IDL.Opt(IDL.Int),
  shortfall_e8s: IDL.Opt(IDL.Nat64),
  unattributed_at_main: IDL.Opt(IDL.Nat64),
  guard_liability: IDL.Nat64,
  verdict: SolvencyVerdictT,
  summary: IDL.Text,
});

FIXTURES.shipped = () => IDL.Service({
  get_balance: IDL.Func([], [IDL.Nat64], ['query']),
  get_solvency: IDL.Func([], [SolvencyReportT], ['query']),
  refresh_solvency: IDL.Func([], [IDL.Variant({ Ok: SolvencyReportT, Err: IDL.Text })], []),
  refresh_main_account_custody: IDL.Func([], [IDL.Variant({
    Ok: IDL.Record({ amount: IDL.Nat64, observed_at_ns: IDL.Nat64 }), Err: IDL.Text,
  })], []),
});

/** A decoded `SolvencyReport`, as @dfinity/candid would hand it over. */
function shippedReport(over = {}) {
  return {
    currency: 'ICP',
    as_of_ns: 1_786_000_500_000_000_000n,
    escrow: 940_640_001n,
    chips_at_table: 0n,
    pot: 0n,
    committed_stake: 0n,
    unswept_deposits: 0n,
    unfinished_incoming: 0n,
    pulls_in_flight: 0n,
    sweep_fees_in_flight: 0n,
    payouts_in_flight: 0n,
    owed: 940_640_001n,
    main_account: [740_640_001n],
    main_observed_at_ns: [1_786_000_000_000_000_000n],
    main_credited_since_reading: 0n,
    main_debited_since_reading: 0n,
    deposit_subaccounts: 0n,
    deposit_accounts_observed: 5n,
    deposit_oldest_observed_at_ns: [1_786_000_000_000_000_000n],
    deposit_accounts_never_observed: [],
    held: [740_640_001n],
    difference_e8s: [-200_000_000n],
    shortfall_e8s: [200_000_000n],
    unattributed_at_main: [],
    guard_liability: 940_640_001n,
    verdict: { CannotPayEveryone: null },
    summary: 'this table cannot pay everyone',
    ...over,
  };
}

console.log('\n  solvency reader — THE SHIPPED CANISTER SURFACE\n');

check('the shipped get_solvency / refresh_solvency pair is discovered', () => {
  const s = describeSolvencySurface(FIXTURES.shipped);
  assert.equal(s.read, 'get_solvency');
  assert.equal(s.refresh, 'refresh_solvency',
    'refresh_solvency re-reads every account; refresh_main_account_custody only one');
});

check('THE MAINNET table_1 NUMBERS read as SHORT by 2.00 ICP', () => {
  const r = interpretSolvency(shippedReport());
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
  assert.equal(r.owed, 940_640_001n);
  assert.equal(r.held, 740_640_001n);
  assert.equal(r.shortfall, 200_000_000n);
  assert.equal(r.observedAtNs, 1_786_000_000_000_000_000n);
});

check('a CanPayEveryone verdict is COVERED', () => {
  const r = interpretSolvency(shippedReport({
    held: [1_000_000_000n], difference_e8s: [59_359_999n], shortfall_e8s: [],
    verdict: { CanPayEveryone: null }, summary: 'covered',
  }));
  assert.equal(r.state, SOLVENCY_STATES.COVERED);
});

check('AN UNKNOWN VERDICT IS UNKNOWN even though shortfall_e8s is null', () => {
  // The Candid says shortfall_e8s is null both when there is no shortfall AND
  // when the answer is not known. Reading null as "no shortfall" is the
  // reassuring-direction failure this whole module exists to avoid.
  const r = interpretSolvency(shippedReport({
    main_account: [], main_observed_at_ns: [], held: [],
    difference_e8s: [], shortfall_e8s: [],
    deposit_accounts_never_observed: ['aaaaa-aa'],
    verdict: { Unknown: null }, summary: 'never read the main account',
  }));
  assert.equal(r.state, SOLVENCY_STATES.UNKNOWN,
    'null shortfall + Unknown verdict must NOT read as covered');
});

check('`as_of_ns` is NOT mistaken for a ledger observation', () => {
  // as_of_ns is always set. If it were matched as "when was the ledger read",
  // a canister that has never looked would report a fresh observation.
  const r = interpretSolvency(shippedReport({
    main_account: [], main_observed_at_ns: [], held: [], shortfall_e8s: [],
    verdict: { Unknown: null },
  }));
  assert.equal(r.observedAtNs, null,
    `observedAtNs should be null, got ${r.observedAtNs} (as_of_ns is 1786000500000000000)`);
});

check('an unrecognised verdict tag degrades to UNKNOWN, never COVERED', () => {
  const r = interpretSolvency(shippedReport({ verdict: { SomeNewTag: null } }));
  assert.equal(r.state, SOLVENCY_STATES.UNKNOWN);
});

check("the canister's own summary is the sentence shown", () => {
  const r = interpretSolvency(shippedReport({ summary: 'call refresh_solvency()' }));
  assert.equal(r.advice, 'call refresh_solvency()');
});

console.log('\n  solvency reader — discovery from Candid\n');

check('canonical surface is discovered, query + refresh', () => {
  const s = describeSolvencySurface(FIXTURES.canonical);
  assert.equal(s.read, 'get_solvency');
  assert.equal(s.refresh, 'refresh_main_account_custody');
});

check('a differently-named surface with the same MEANING is discovered', () => {
  const s = describeSolvencySurface(FIXTURES.derived);
  assert.equal(s.read, 'get_reserve_status');
});

check('a Result-wrapped surface is discovered', () => {
  const s = describeSolvencySurface(FIXTURES.wrapped);
  assert.equal(s.read, 'get_table_solvency');
});

check('a name-only decoy is NOT adopted', () => {
  const s = describeSolvencySurface(FIXTURES.decoy);
  assert.equal(s.read, null, 'a method whose payload carries no figures must not be wired');
});

// ---------------------------------------------------------------------------
// THE SHIPPED DECLARATIONS MUST EXPOSE WHAT THE COMMITTED CANISTER EXPORTS.
// ---------------------------------------------------------------------------
//
// This module DISCOVERS its surface from `declarations/table_1/table_1.did.js`
// rather than naming a method, and an Actor only has the methods its IDL
// declares. So a declarations bundle that is older than the canister does not
// degrade the solvency screen -- it DELETES it: `tableActor.get_solvency` is
// undefined no matter what the canister exports, `readTableSolvency()` answers
// `unsupported`, and every table on earth shows the loudest warning this app has,
// permanently, including tables that can answer perfectly.
//
// **That state was live in this repository until 2026-08-09.** Measured, by
// running the app's own discovery against the committed bundle:
// `describeSolvencySurface()` returned `{read: null, refresh: null}` while
// `src/table_canister/table_canister.did` exported `get_solvency` and
// `refresh_solvency`. It fails SAFE, which is why it is a defect and not a
// finding -- but a warning that is always on is a warning nobody reads, and this
// repository has already paid for that twice.
//
// The two halves are asserted separately so a failure says which one moved.
check('the shipped declarations expose the solvency surface the canister exports', () => {
  const s = describeSolvencySurface();
  const canisterDid = fs.readFileSync(
    path.join(REPO_ROOT, 'src/table_canister/table_canister.did'), 'utf8',
  );
  const canisterExports = /(^|\n)\s*get_solvency\s*:/.test(canisterDid);
  console.log(`      canister .did exports get_solvency : ${canisterExports}`);
  console.log(`      declarations expose it             : ${s.read !== null}`);
  assert.ok(s.methods.length > 20, 'the shipped service should have been enumerated');
  assert.ok(s.methods.includes('get_custody_status'), 'enumeration should see real methods');

  // Whatever else is true, an undiscoverable surface must warn and must never
  // read as an all-clear.
  assert.equal(severityOf(SOLVENCY_STATES.UNSUPPORTED), 'warning');
  assert.equal(shouldWarnBeforeDeposit(SOLVENCY_STATES.UNSUPPORTED), true,
    'a table whose solvency cannot be read must discourage a deposit');

  assert.equal(canisterExports, true,
    'the committed canister interface must still export get_solvency; if it was renamed, '
    + 'rename it here and regenerate src/declarations in the same change');
  assert.equal(s.read, 'get_solvency',
    "the app builds its table actor from src/declarations/table_1/table_1.did.js. If that "
    + 'bundle does not declare get_solvency, the running app CANNOT CALL IT, and every '
    + 'player sees "this table cannot say whether it holds your money" on a table that can. '
    + 'Regenerate the declarations from src/table_canister/table_canister.did.');
  assert.equal(s.refresh, 'refresh_solvency',
    'and the public, permissionless re-read must be reachable too, or the warning has no '
    + 'action attached to it');
});

// ---------------------------------------------------------------------------
// THE PICK IS ORDERED. docs/SECURITY-FINDINGS.md FINDING 43's frontend half.
// ---------------------------------------------------------------------------
check('`held` wins over `main_account`, and `owed` over `guard_liability`', () => {
  // Round-tripped through the REAL Candid, so the key order is the one
  // `@dfinity/candid` actually produces: hash order, in which `main_account`
  // comes BEFORE `held`. Every fixture above happens to give those two fields
  // the same value, so none of them could ever have caught this.
  const record = shippedReport({
    ledger: Principal.fromText('ryjl3-tyaaa-aaaaa-aaaba-cai'),
    main_ledger: [Principal.fromText('ryjl3-tyaaa-aaaaa-aaaba-cai')],
    main_account: [400_000_000n],     // the main account alone
    deposit_subaccounts: 200_000_000n, // plus money at published deposit addresses
    held: [600_000_000n],              // = what this canister actually holds
    unswept_deposits: 200_000_000n,
    owed: 900_000_000n,
    guard_liability: 950_000_000n,     // the second total FINDING 43 published
    difference_e8s: [-300_000_000n],
    shortfall_e8s: [300_000_000n],
  });
  const [decoded] = IDL.decode([SolvencyReportT], IDL.encode([SolvencyReportT], [record]));
  const order = Object.keys(decoded);
  assert.ok(
    order.indexOf('main_account') < order.indexOf('held'),
    'the trap this test exists for is that Candid hash order puts main_account first; '
    + `it did not here (${order.join(', ')}), so this test is measuring nothing`,
  );

  const r = interpretSolvency(decoded);
  assert.equal(r.held, 600_000_000n,
    'the screen labels this "Held on the ledger". Picking `main_account` instead drops '
    + 'every deposit subaccount from the figure a player uses to decide whether to deposit.');
  assert.equal(r.owed, 900_000_000n,
    'and `owed` must be the published `owed`, not whichever field matched the loose '
    + '/liabilit/ fallback first');
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
});

console.log('\n  solvency reader — interpreting the answer\n');

check('a stated shortfall > 0 is SHORT', () => {
  const r = interpretSolvency({
    observed_main_balance: 740_640_001n,
    total_liability: 940_640_001n,
    shortfall: 200_000_000n,
    observed_at_ns: [1_786_000_000_000_000_000n],
    advice: '',
  });
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
  assert.equal(r.shortfall, 200_000_000n);
  assert.equal(r.held, 740_640_001n);
  assert.equal(r.owed, 940_640_001n);
});

check('held >= owed with a real observation is COVERED', () => {
  const r = interpretSolvency({
    observed_main_balance: 1_000n, total_liability: 900n,
    observed_at_ns: [1_786_000_000_000_000_000n], advice: '',
  });
  assert.equal(r.state, SOLVENCY_STATES.COVERED);
  assert.equal(r.shortfall, 0n);
});

check('held < owed with NO stated shortfall is still SHORT (derived)', () => {
  const r = interpretSolvency({
    main_account_balance: 900n, liabilities: 1_000n,
    checked_at_ns: [1_786_000_000_000_000_000n],
  });
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
  assert.equal(r.shortfall, 100n);
});

check('NEVER OBSERVED is UNKNOWN, not covered — the whole point of FINDING 35', () => {
  const r = interpretSolvency({
    observed_main_balance: 0n, total_liability: 0n, shortfall: 0n,
    observed_at_ns: [], advice: '',
  });
  assert.equal(r.state, SOLVENCY_STATES.UNKNOWN,
    'a canister that has never read its ledger reports zero and zero; reading that as '
    + '"covered" is exactly the defect');
});

check('an Err reply is UNKNOWN and carries the reason', () => {
  const r = interpretSolvency({ Err: 'not a controller' });
  assert.equal(r.state, SOLVENCY_STATES.UNKNOWN);
  assert.match(r.advice, /not a controller/);
});

check('an Ok wrapper is looked through', () => {
  const r = interpretSolvency({
    Ok: {
      observed_main_balance: 1n, total_liability: 5n, shortfall: 4n,
      observed_at_ns: [1n], advice: 'x',
    },
  });
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
  assert.equal(r.shortfall, 4n);
});

check('a record the interpreter cannot understand is UNKNOWN, never covered', () => {
  const r = interpretSolvency({ some_unrelated_field: 3n });
  assert.equal(r.state, SOLVENCY_STATES.UNKNOWN);
});

check('an explicit boolean verdict is honoured', () => {
  assert.equal(interpretSolvency({ is_solvent: false }).state, SOLVENCY_STATES.SHORT);
  assert.equal(interpretSolvency({ is_solvent: true }).state, SOLVENCY_STATES.COVERED);
});

console.log('\n  solvency reader — reading a canister\n');

await checkAsync('no surface at all reports UNSUPPORTED, with advice', async () => {
  const r = await readTableSolvency({}, { surface: describeSolvencySurface(FIXTURES.none) });
  assert.equal(r.state, SOLVENCY_STATES.UNSUPPORTED);
  assert.match(r.advice, /cannot report whether it actually holds/i);
  assert.doesNotMatch(r.advice, /\d/, 'player-facing copy must carry no numeric token');
});

await checkAsync('a canister older than its declarations reports UNSUPPORTED', async () => {
  const actor = { get_solvency: async () => { throw new Error('IC0302: Canister has no query method \'get_solvency\''); } };
  const r = await readTableSolvency(actor, { surface: describeSolvencySurface(FIXTURES.canonical) });
  assert.equal(r.state, SOLVENCY_STATES.UNSUPPORTED);
});

await checkAsync('a transport failure is ERROR, not covered', async () => {
  const actor = { get_solvency: async () => { throw new Error('Network request timed out after 30s'); } };
  const r = await readTableSolvency(actor, { surface: describeSolvencySurface(FIXTURES.canonical) });
  assert.equal(r.state, SOLVENCY_STATES.ERROR);
});

await checkAsync('a real reply is read end to end', async () => {
  const actor = {
    get_solvency: async () => ({
      observed_main_balance: 740_640_001n,
      total_liability: 940_640_001n,
      shortfall: 200_000_000n,
      observed_at_ns: [1_786_000_000_000_000_000n],
      advice: 'this table is short',
    }),
  };
  const r = await readTableSolvency(actor, { surface: describeSolvencySurface(FIXTURES.canonical) });
  assert.equal(r.state, SOLVENCY_STATES.SHORT);
  assert.equal(r.method, 'get_solvency');
  assert.equal(r.canRefresh, true);
});

console.log('\n  solvency reader — how the UI is told to treat each state\n');

check('every state except covered warns before a deposit', () => {
  for (const s of [SOLVENCY_STATES.SHORT, SOLVENCY_STATES.UNKNOWN,
    SOLVENCY_STATES.UNSUPPORTED, SOLVENCY_STATES.ERROR]) {
    assert.equal(shouldWarnBeforeDeposit(s), true, `${s} must warn`);
  }
  assert.equal(shouldWarnBeforeDeposit(SOLVENCY_STATES.COVERED), false);
});

check('short is critical; unknown and unsupported are warnings', () => {
  assert.equal(severityOf(SOLVENCY_STATES.SHORT), 'critical');
  assert.equal(severityOf(SOLVENCY_STATES.UNKNOWN), 'warning');
  assert.equal(severityOf(SOLVENCY_STATES.UNSUPPORTED), 'warning');
  assert.equal(severityOf(SOLVENCY_STATES.COVERED), 'ok');
});

check('an absent observation reads as "never taken", not as "0s ago"', () => {
  assert.equal(observationAge(null), 'never taken');
  assert.equal(observationAge(undefined), 'never taken');
  // 1_786_000_000_000_000_000 ns == 1_786_000_000_000 ms.
  assert.equal(observationAge(1_786_000_000_000_000_000n, 1_786_000_010_000), '10s ago');
  assert.equal(observationAge(1_786_000_000_000_000_000n, 1_786_000_100_000), '2 min ago');
  assert.equal(observationAge(1_786_000_000_000_000_000n, 1_786_007_200_000), '2 h ago');
});

console.log('');
if (failures.length) {
  console.error(`  ${failures.length} failed, ${passed} passed\n`);
  process.exit(1);
}
console.log(`  ${passed} of ${passed} solvency checks passed.\n`);
