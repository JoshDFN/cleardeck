#!/usr/bin/env node
// THE CYCLE-RUNWAY WARNING, ON RECORDS WHOSE ANSWER IS KNOWN BY CONSTRUCTION.
//
// ===========================================================================
// WHY THIS EXISTS AND WHAT IT IS AFRAID OF
// ===========================================================================
//
// `src/cleardeck_frontend/src/lib/cycleRunway.js` decides whether a player about
// to move money is told that this table is running out of cycles. Getting that
// decision wrong in the reassuring direction is docs/DEFECTS.md E-55 arriving
// with no warning at all: a canister below its freezing threshold rejects EVERY
// update call, so `deposit`, `withdraw`, `cash_out` and `player_action` all fail
// at the same instant and every player's escrow is unreachable until somebody
// tops it up.
//
// The states that matter most are the ones a running system almost never
// produces on demand:
//
//   * UNREACHABLE -- what a FROZEN canister looks like from outside. It rejects
//     QUERIES too (docs/SECURITY-FINDINGS.md FINDING 24, reproduced live in
//     `tests/money_safety/tests/cycles_runway.rs`), so the endpoint built to
//     raise this alarm goes dark at the exact moment the alarm is true. A reader
//     that catches the rejection and shows nothing is the defect.
//   * UNKNOWN -- `runway_days` is null for the first five minutes after every
//     install and every upgrade. Reading null as "fine" makes the warning go
//     quiet exactly when the fleet was last touched.
//   * UNSUPPORTED -- the mainnet module is older than these declarations. As of
//     this writing the deployed table module hash does not match a build of this
//     tree, so this is not a hypothetical.
//
// Every assertion below is chosen so the DANGEROUS direction fails loudly: no
// input may produce `ok` unless the canister positively said it has more than
// WARN_DAYS of MEASURED runway.
//
// Usage:  node tools/shots/test-cycle-runway.mjs

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';

// `cycleRunway.js` imports `declarations/table_1/table_1.did.js`, a VITE ALIAS
// Node knows nothing about. Same shim as test-solvency.mjs, and created here
// rather than committed for the same reason: `npm install` deletes anything in
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
  RUNWAY_STATES,
  WARN_DAYS,
  CRITICAL_DAYS,
  interpretCycleStatus,
  readCycleRunway,
  describeRunwaySurface,
  severityOf,
  shouldWarnBeforeDeposit,
  headlineFor,
  adviceFor,
  formatCycles,
  topUpHint,
} = await import(
  new URL('../../src/cleardeck_frontend/src/lib/cycleRunway.js', import.meta.url)
);

let checks = 0;
const ok = (msg) => { checks += 1; console.log(`    ok  ${msg}`); };

// ---------------------------------------------------------------------------
// A well-formed CycleStatus, as the canister actually encodes it.
// ---------------------------------------------------------------------------
//
// `opt nat64` decodes to [] or [v]; every `nat` and `nat64` decodes to a BigInt.
// Built from a factory so a test can change ONE field and nothing else, which is
// what makes each assertion below a statement about that field.
function status(over = {}) {
  return {
    balance: 10_000_000_000_000n,
    liquid_balance: 9_997_800_000_000n,
    reserved_for_freezing: 2_200_000_000n,
    observed_burn_per_day: 44_247_843_312n,
    recent_burn_per_day: [44_247_843_312n],
    runway_days: [225n],
    sample_window_secs: 7200n,
    recent_window_secs: [3570n],
    measurement_is_meaningful: true,
    clock_ticks: 240n,
    clock_last_tick_at: 1_767_225_600_000_000_000n,
    clock_watchdog_armed: true,
    next_wake_at: [],
    ...over,
  };
}

console.log('\n  === the four thresholds ===');
{
  assert.equal(interpretCycleStatus(status({ runway_days: [225n] })).state, RUNWAY_STATES.OK);
  ok(`225 days -> ok (warn at ${WARN_DAYS})`);

  assert.equal(
    interpretCycleStatus(status({ runway_days: [BigInt(WARN_DAYS)] })).state,
    RUNWAY_STATES.OK,
  );
  ok(`exactly ${WARN_DAYS} days -> ok (the boundary is exclusive)`);

  assert.equal(
    interpretCycleStatus(status({ runway_days: [BigInt(WARN_DAYS - 1)] })).state,
    RUNWAY_STATES.LOW,
  );
  ok(`${WARN_DAYS - 1} days -> low`);

  assert.equal(
    interpretCycleStatus(status({ runway_days: [BigInt(CRITICAL_DAYS - 1)] })).state,
    RUNWAY_STATES.CRITICAL,
  );
  ok(`${CRITICAL_DAYS - 1} days -> critical`);

  assert.equal(interpretCycleStatus(status({ runway_days: [0n] })).state, RUNWAY_STATES.CRITICAL);
  ok('0 days -> critical');
}

console.log('\n  === null is not fine ===');
{
  // The state of EVERY instance for the first five minutes after an install or
  // an upgrade. If this ever returns `ok`, the warning is silent across every
  // deploy this project makes.
  const r = interpretCycleStatus(status({ runway_days: [], measurement_is_meaningful: false }));
  assert.equal(r.state, RUNWAY_STATES.UNKNOWN);
  assert.notEqual(r.state, RUNWAY_STATES.OK);
  ok('runway_days = null -> unknown, never ok');

  // A canister with a huge balance and no measurement is still unknown. The
  // balance is not the question; the burn rate is.
  const rich = interpretCycleStatus(status({
    runway_days: [],
    liquid_balance: 500_000_000_000_000n,
    observed_burn_per_day: 0n,
    recent_burn_per_day: [0n],
  }));
  assert.equal(rich.state, RUNWAY_STATES.UNKNOWN);
  ok('500 T and no measured burn rate -> unknown, not ok');

  assert.equal(interpretCycleStatus(null).state, RUNWAY_STATES.UNKNOWN);
  assert.equal(interpretCycleStatus(undefined).state, RUNWAY_STATES.UNKNOWN);
  assert.equal(interpretCycleStatus('not a record').state, RUNWAY_STATES.UNKNOWN);
  ok('a missing or unparseable record -> unknown, never ok');
}

console.log('\n  === the pessimistic burn rate ===');
{
  // THE DEFECT THE CANISTER WAS CHANGED FOR (docs/DEFECTS.md E-55). A table that
  // sat idle for months and has just got busy has a LIFETIME average far below
  // its RECENT one. Anything that recomputes a runway must use the larger rate,
  // or it reproduces the gauge that reads high.
  const r = interpretCycleStatus(status({
    observed_burn_per_day: 44_000_000_000n,
    recent_burn_per_day: [234_287_086_846n],
  }));
  assert.equal(r.burnPerDay, 234_287_086_846n);
  ok('burnPerDay takes the larger of lifetime and recent, not the lifetime average');

  const quiet = interpretCycleStatus(status({
    observed_burn_per_day: 234_287_086_846n,
    recent_burn_per_day: [44_000_000_000n],
  }));
  assert.equal(quiet.burnPerDay, 234_287_086_846n);
  ok('and the larger one when it is the lifetime figure, not whichever is newer');
}

console.log('\n  === a reply from the OLDER module running on mainnet ===');
{
  // MEASURED, NOT IMAGINED. The first build of this change declared
  // `recent_burn_per_day` and `recent_window_secs` as bare `nat`/`nat64` in the
  // Candid. Candid will not decode a record that is MISSING a non-optional
  // field, so every reply from the module actually deployed on mainnet -- an
  // older build than this tree -- became undecodable, `readCycleRunway` caught
  // the decode error, and the deposit screen of a table with 960 DAYS of runway
  // rendered "This table is not answering", which is the alarm for a canister
  // that has run out of cycles. The screenshot gate went green: it checks the
  // protected notices and the money figures, not this banner.
  //
  // Both fields are `opt` now and this is the gate on it. The fixture is the
  // record the OLD module emits: the two new fields simply are not there.
  const oldModule = {
    balance: 1_408_677_590_920n,
    liquid_balance: 1_379_728_176_820n,
    reserved_for_freezing: 28_949_414_100n,
    observed_burn_per_day: 54_999_400_485n,
    runway_days: [25n],
    sample_window_secs: 117_239n,
    measurement_is_meaningful: true,
    clock_ticks: 3_942n,
    clock_last_tick_at: 1_786_255_737_361_817_000n,
    clock_watchdog_armed: true,
    next_wake_at: [],
  };
  const r = interpretCycleStatus(oldModule);
  assert.notEqual(r.state, RUNWAY_STATES.UNREACHABLE);
  assert.equal(r.state, RUNWAY_STATES.LOW);
  assert.equal(r.days, 25);
  assert.equal(r.burnPerDay, 54_999_400_485n);
  ok('an old-module reply is read, not treated as a dead canister');
}

console.log('\n  === unreachable is the alarm, not an error to swallow ===');
{
  // The exact reject text a frozen canister produces, from the live
  // reproduction in tests/money_safety/tests/cycles_runway.rs.
  const frozenQuery = {
    get_cycle_status: async () => {
      throw new Error(
        "Canister kieex-haaaa-aaaaj-qor3q-cai is unable to process query calls because "
        + "it's frozen. Please top up the canister with cycles and try again.",
      );
    },
  };
  const r = await readCycleRunway(frozenQuery, { surface: { read: 'get_cycle_status' } });
  assert.equal(r.state, RUNWAY_STATES.UNREACHABLE);
  assert.equal(severityOf(r.state), 'danger');
  ok('a frozen canister rejecting the query -> unreachable, at danger severity');

  const outOfCycles = {
    get_cycle_status: async () => { throw new Error('Canister ... is out of cycles'); },
  };
  assert.equal(
    (await readCycleRunway(outOfCycles, { surface: { read: 'get_cycle_status' } })).state,
    RUNWAY_STATES.UNREACHABLE,
  );
  ok('"is out of cycles" -> unreachable');

  // Any other transport failure is ALSO unreachable, on purpose. A table that is
  // not answering is not paying anybody out either, and from outside a frozen
  // canister and a sick subnet are indistinguishable -- which the advice says.
  const dead = { get_cycle_status: async () => { throw new Error('fetch failed'); } };
  const r3 = await readCycleRunway(dead, { surface: { read: 'get_cycle_status' } });
  assert.equal(r3.state, RUNWAY_STATES.UNREACHABLE);
  assert.match(adviceFor(r3.state), /out of cycles or its subnet/i);
  ok('any transport failure -> unreachable, and the copy states both possibilities');
}

console.log('\n  === an older module than this bundle ===');
{
  const old = {
    get_cycle_status: async () => {
      throw new Error("Canister has no query method 'get_cycle_status'");
    },
  };
  const r = await readCycleRunway(old, { surface: { read: 'get_cycle_status' } });
  assert.equal(r.state, RUNWAY_STATES.UNSUPPORTED);
  assert.notEqual(r.state, RUNWAY_STATES.OK);
  ok('"has no query method" -> unsupported, never ok');

  const noSurface = await readCycleRunway({}, { surface: { read: null } });
  assert.equal(noSurface.state, RUNWAY_STATES.UNSUPPORTED);
  ok('a Candid without get_cycle_status -> unsupported');

  const noActor = await readCycleRunway(null, { surface: { read: 'get_cycle_status' } });
  assert.equal(noActor.state, RUNWAY_STATES.UNKNOWN);
  ok('no actor yet -> unknown, not ok');
}

console.log('\n  === the surface is discovered from the SHIPPED Candid ===');
{
  // Not a mock. This reads `src/declarations/table_1/table_1.did.js`, the file
  // the app builds its actor from. If `get_cycle_status` is ever dropped from the
  // declarations again -- it was missing from all three of them until this wave,
  // and was carried as a known-drift line in
  // scripts/candid-declarations-baseline.txt -- this goes red instead of the
  // banner silently becoming permanently `unsupported`.
  const surface = describeRunwaySurface();
  assert.equal(surface.read, 'get_cycle_status');
  ok('get_cycle_status is on the declarations the frontend actually builds from');
}

console.log('\n  === the shipped IDL can DECODE the old wire shape ===');
{
  // The check above is about interpretation. This one is about the wire: encode
  // the OLD record with the old field set and decode it with the CycleStatus
  // type the frontend actually builds its actor from. If the two new fields ever
  // stop being `opt`, this throws and the banner starts crying wolf on every
  // mainnet table again.
  // The bare specifier `declarations/...` is a vite alias; resolve the real path.
  const { IDL } = await import('@dfinity/candid');
  const { idlFactory } = await import(
    new URL('../../src/declarations/table_1/table_1.did.js', import.meta.url)
  );
  const svc = idlFactory({ IDL });
  const entry = svc._fields.find(([n]) => n === 'get_cycle_status');
  assert.ok(entry, 'get_cycle_status must be on the shipped declarations');
  const retTypes = entry[1].retTypes;
  assert.ok(retTypes && retTypes.length === 1, 'get_cycle_status must return one value');
  const CycleStatus = retTypes[0];

  const OldCycleStatus = IDL.Record({
    balance: IDL.Nat,
    liquid_balance: IDL.Nat,
    reserved_for_freezing: IDL.Nat,
    observed_burn_per_day: IDL.Nat,
    runway_days: IDL.Opt(IDL.Nat64),
    sample_window_secs: IDL.Nat64,
    measurement_is_meaningful: IDL.Bool,
    clock_ticks: IDL.Nat64,
    clock_last_tick_at: IDL.Nat64,
    clock_watchdog_armed: IDL.Bool,
    next_wake_at: IDL.Opt(IDL.Nat64),
  });
  const wire = IDL.encode([OldCycleStatus], [{
    balance: 1_408_677_590_920n,
    liquid_balance: 1_379_728_176_820n,
    reserved_for_freezing: 28_949_414_100n,
    observed_burn_per_day: 54_999_400_485n,
    runway_days: [25n],
    sample_window_secs: 117_239n,
    measurement_is_meaningful: true,
    clock_ticks: 3_942n,
    clock_last_tick_at: 1_786_255_737_361_817_000n,
    clock_watchdog_armed: true,
    next_wake_at: [],
  }]);
  const [decoded] = IDL.decode([CycleStatus], wire);
  assert.equal(decoded.liquid_balance, 1_379_728_176_820n);
  assert.deepEqual(decoded.recent_burn_per_day, []);
  ok('a reply from the deployed (older) module decodes against the shipped declarations');

  assert.equal(interpretCycleStatus(decoded).state, RUNWAY_STATES.LOW);
  ok('and it interprets as LOW (25 days), not as a dead canister');
}

console.log('\n  === every non-ok state is displayable and says something ===');
{
  for (const state of Object.values(RUNWAY_STATES)) {
    if (state === RUNWAY_STATES.OK) continue;
    const headline = headlineFor(state, 7);
    const advice = adviceFor(state, 'deposit');
    assert.ok(headline.length > 10, `${state} has no headline`);
    assert.ok(advice.length > 30, `${state} has no advice`);
    assert.ok(shouldWarnBeforeDeposit(state), `${state} does not warn before a deposit`);
    ok(`${state}: headline + advice + warns before deposit`);
  }
  assert.equal(shouldWarnBeforeDeposit(RUNWAY_STATES.OK), false);
  assert.equal(headlineFor(RUNWAY_STATES.OK), '');
  ok('ok renders nothing, which is the only state that may be silent');
}

console.log('\n  === the copy names the remedy, and the remedy needs no permission ===');
{
  // THE COMMAND HAS TO EXIST. `dfx` spells this `deposit-cycles`; the pinned CLI
  // for this project is `icp` 1.0.2 and it has no such subcommand -- the first
  // draft of cycleRunway.js printed it anyway. A remedy a player cannot run is
  // worse than no remedy, because it looks like one. The spelling is pinned here
  // and cross-checked against the script that actually runs it.
  const hint = topUpHint('kieex-haaaa-aaaaj-qor3q-cai');
  assert.match(hint, /icp canister top-up/);
  assert.doesNotMatch(hint, /deposit-cycles/);
  assert.match(hint, /kieex-haaaa-aaaaj-qor3q-cai/);
  assert.match(hint, /--amount/);
  ok('the top-up command is `icp canister top-up`, names the canister, and is not the dfx spelling');

  const shSource = fs.readFileSync(path.join(REPO_ROOT, 'scripts/cycles-runway.sh'), 'utf8');
  assert.ok(
    shSource.includes('icp canister top-up'),
    'scripts/cycles-runway.sh must print the same top-up command the UI does',
  );
  assert.ok(
    !/icp canister deposit-cycles/.test(shSource),
    'scripts/cycles-runway.sh still prints the dfx spelling of the top-up command',
  );
  ok('scripts/cycles-runway.sh prints the same command, in the same spelling');

  assert.equal(formatCycles(10_000_000_000_000n), '10.000 T');
  assert.equal(formatCycles(2_654_911_270_080n), '2.654 T');
  assert.equal(formatCycles(null), 'unknown');
  ok('cycles format as T with three decimals, and null formats as "unknown"');
}

console.log('\n  === the thresholds are the ones the rest of the tree uses ===');
{
  // Three copies of these numbers exist by necessity -- Rust, this module and the
  // CI script -- so drift between them is checked rather than commented about.
  const ciSource = fs.readFileSync(
    path.join(REPO_ROOT, 'scripts/cycles-runway.sh'), 'utf8',
  );
  const ciWarn = /^WARN_DAYS=(\d+)/m.exec(ciSource);
  const ciCrit = /^CRITICAL_DAYS=(\d+)/m.exec(ciSource);
  assert.ok(ciWarn && ciCrit, 'scripts/cycles-runway.sh must declare both thresholds');
  assert.equal(Number(ciWarn[1]), WARN_DAYS, 'CI warn threshold differs from the UI');
  assert.equal(Number(ciCrit[1]), CRITICAL_DAYS, 'CI critical threshold differs from the UI');
  ok(`scripts/cycles-runway.sh agrees: warn ${WARN_DAYS}, critical ${CRITICAL_DAYS}`);

  const rustSource = fs.readFileSync(
    path.join(REPO_ROOT, 'tests/money_safety/tests/cycles_runway.rs'), 'utf8',
  );
  const rWarn = /const WARN_DAYS: u128 = (\d+)/.exec(rustSource);
  const rCrit = /const CRITICAL_DAYS: u128 = (\d+)/.exec(rustSource);
  assert.ok(rWarn && rCrit, 'the Rust measurement must declare both thresholds');
  assert.equal(Number(rWarn[1]), WARN_DAYS);
  assert.equal(Number(rCrit[1]), CRITICAL_DAYS);
  ok('tests/money_safety/tests/cycles_runway.rs agrees with both');
}

console.log('\n  === the notice cannot cover the four protected notices ===');
{
  // HARD RULE 2 is measured on rendered pixels by the screenshot harness, which
  // needs a replica and a browser. This is the cheap structural half that runs
  // everywhere: the component must not be able to overlay anything, and the
  // guarantee is that it declares no positioning at all.
  const svelte = fs.readFileSync(
    path.join(REPO_ROOT, 'src/cleardeck_frontend/src/lib/components/CycleRunwayNotice.svelte'),
    'utf8',
  );
  const style = svelte.slice(svelte.indexOf('<style>'));
  for (const forbidden of [
    /position\s*:\s*(fixed|absolute|sticky)/,
    /z-index\s*:/,
    /transform\s*:/,
  ]) {
    assert.ok(
      !forbidden.test(style),
      `CycleRunwayNotice.svelte declares ${forbidden} in its styles. An in-flow warning `
      + 'cannot occlude the unaudited-alpha disclaimer, the 18+ notice, the jurisdiction '
      + 'warning or the no-rake property; a positioned one can. HARD RULE 2.',
    );
  }
  ok('CycleRunwayNotice declares no position, no z-index and no transform');
}

console.log(`\n  cycle-runway self-test: ${checks} checks green\n`);
