#!/usr/bin/env node
// ===========================================================================
// FINDING 42, REPRODUCED ON RENDERED PIXELS:
// THE DEPOSIT ADDRESS IS DERIVED FROM A CANISTER ID THAT ARRIVED OVER AN
// UNCERTIFIED QUERY, AND EVERY CHECK ON THE SCREEN STILL READS GREEN.
// ===========================================================================
//
//   node tools/shots/repro-finding42.mjs
//
//   exit 1  the substituted canister id reached the deposit address. The player
//           is shown, and told to pay, an account inside a canister the build
//           never named.  <-- THIS IS THE PRE-FIX BUILD
//   exit 0  the deposit path refused the unpinned id and published no address.
//
// WHAT IS SUBSTITUTED, AND WHY THIS IS THE REAL THING
// ---------------------------------------------------------------------------
// `routes/+page.svelte` takes the table canister id out of `lobby.get_tables()`
// and hands it to `<DepositModal tableCanisterId={...}>`, which derives the
// player's deposit address from it. `get_tables` is declared `query` in
// `lobby_canister.did`. On the Internet Computer a query is answered by ONE
// replica and carries no certificate this client verifies, so the bytes that
// name the table canister are attacker-controllable without any canister bug,
// any majority, or any key.
//
// This script does not pretend to own a replica. It produces the SAME BYTES on
// the client's side of the wire the cheap way: it stands up a second lobby
// canister (the real lobby wasm), tells it that table 1 lives at a canister the
// attacker controls, and builds the real frontend against it. From the client's
// point of view those two situations are indistinguishable -- a query reply is
// a query reply -- which is exactly the property FINDING 40 closed for
// `get_deposit_address()` and left open one call upstream.
//
// Nothing here touches the deployed lobby, the deployed tables or the deployed
// asset canister: the hostile canisters are new, and the substituted bundle is
// served off local disk through the harness's existing SHOTS_SERVE_DIST path
// while every /api call still goes to the real local replica.
//
// LOCAL REPLICA ONLY. Every `icp` invocation goes through lib/ids.mjs, which
// refuses `-e ic` and refuses any argument naming a ClearDeck mainnet canister.

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import {
  APP_ORIGIN, FRONTEND_DIR, FRONTEND_DIST, GATEWAY_ORIGIN, ICP_LEDGER_CANISTER_ID,
  REPO_ROOT, VIEWPORTS,
} from './lib/config.mjs';
import { icp, readLocalIds } from './lib/ids.mjs';
import { startGatewayProxy } from './lib/proxy.mjs';
import { lobbyActor, ledgerActor, tableActor } from './lib/agent.mjs';
import { devPlayer, devPlayerPrincipal } from './lib/identities.mjs';
import { devLogin, launchBrowser, newContext, openApp, setAppOrigin, settle } from './lib/browser.mjs';

const HOSTILE_STATE = path.join(REPO_ROOT, '.icp', 'cache', 'cleardeck-finding42.json');
const OUT_DIR = path.join(REPO_ROOT, 'artifacts', 'finding42');
const SUBSTITUTED_DIST = path.join(OUT_DIR, 'dist-substituted');
const TABLE_WASM = path.join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/table_canister.wasm');
const LOBBY_WASM = path.join(REPO_ROOT, 'target/wasm32-unknown-unknown/release/lobby_canister.wasm');

// The table the substitution is planted at, and the identity that will be shown
// a deposit address. Player 4 is the one no screenshot scene seats.
const VICTIM_PLAYER = 4;

// table_1's init args, copied from icp.yaml. The hostile canister runs the SAME
// wasm on purpose: the point of FINDING 42 is that a perfectly honest table
// canister at an id the build never pinned is still a thief's account, because
// the account identifier is a function of the canister id.
const TABLE_1_INIT_ARGS =
  '(record { small_blind = 1000000 : nat64; big_blind = 2000000 : nat64; '
  + 'min_buy_in = 200000000 : nat64; max_buy_in = 1000000000 : nat64; max_players = 2 : nat8; '
  + 'action_timeout_secs = 30 : nat64; ante = 0 : nat64; time_bank_secs = 30 : nat64; '
  + 'currency = variant { ICP } })';

const log = (m) => console.log(m);
const step = (m) => console.log(`\n==> ${m}`);

function die(message) {
  console.error(`\nFATAL: ${message}`);
  process.exit(2);
}

// ---------------------------------------------------------------------------
// preflight
// ---------------------------------------------------------------------------

async function gatewayIsUp() {
  try {
    const res = await fetch(`${GATEWAY_ORIGIN}/api/v2/status`, { signal: AbortSignal.timeout(3000) });
    return res.ok;
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// the substitution: a table canister at an id this build never named, announced
// by a lobby over an ordinary query
// ---------------------------------------------------------------------------

function readHostileState() {
  try {
    return JSON.parse(fs.readFileSync(HOSTILE_STATE, 'utf8'));
  } catch {
    return {};
  }
}

function writeHostileState(state) {
  fs.mkdirSync(path.dirname(HOSTILE_STATE), { recursive: true });
  fs.writeFileSync(HOSTILE_STATE, `${JSON.stringify(state, null, 2)}\n`);
}

function canisterExists(id) {
  try {
    icp(['canister', 'status', id, '-e', 'local'], { timeoutMs: 30_000 });
    return true;
  } catch {
    return false;
  }
}

function createDetached() {
  const out = icp(['canister', 'create', '--detached', '-q', '--cycles', '3t', '-e', 'local']);
  const id = out.trim().split('\n').filter(Boolean).pop();
  if (!id || !/^[a-z0-9-]+-cai$/.test(id)) {
    throw new Error(`could not read a canister id out of \`icp canister create\`:\n${out}`);
  }
  return id;
}

function installWasm(id, wasm, args) {
  const argv = ['canister', 'install', id, '-e', 'local', '--mode', 'install', '--wasm', wasm];
  if (args) argv.push('--args', args);
  argv.push('-y');
  icp(argv, { timeoutMs: 300_000 });
}

/**
 * Ensures a hostile table canister and a lobby that names it exist, and that the
 * lobby's `get_tables()` really returns the hostile id. Idempotent: re-running
 * reuses whatever is already on the replica.
 */
async function ensureSubstitution(localIds) {
  const state = readHostileState();

  if (!state.hostileTable || !canisterExists(state.hostileTable)) {
    if (!fs.existsSync(TABLE_WASM)) die(`no table wasm at ${TABLE_WASM} -- run ./scripts/dev.sh wasm`);
    state.hostileTable = createDetached();
    installWasm(state.hostileTable, TABLE_WASM, TABLE_1_INIT_ARGS);
    log(`  hostile TABLE canister created and installed: ${state.hostileTable}`);
  } else {
    log(`  hostile TABLE canister reused: ${state.hostileTable}`);
  }

  if (!state.hostileLobby || !canisterExists(state.hostileLobby)) {
    if (!fs.existsSync(LOBBY_WASM)) die(`no lobby wasm at ${LOBBY_WASM} -- run ./scripts/dev.sh wasm`);
    state.hostileLobby = createDetached();
    installWasm(state.hostileLobby, LOBBY_WASM, null);
    log(`  hostile LOBBY canister created and installed: ${state.hostileLobby}`);
  } else {
    log(`  hostile LOBBY canister reused: ${state.hostileLobby}`);
  }
  writeHostileState(state);

  // Table 1 -> the hostile canister. Tables 2 and 3 stay honest, because ONE
  // substituted id in an otherwise ordinary reply is the whole finding.
  try {
    icp([
      'canister', 'call', state.hostileLobby, 'init_microstakes_tables',
      `(principal "${state.hostileTable}", principal "${localIds.table_2}", principal "${localIds.table_3}")`,
      '-e', 'local',
    ]);
  } catch {
    /* already initialised on a re-run; the postcondition below is what counts */
  }

  // POSTCONDITION, READ BACK OFF THE CANISTER. A call that returned Ok is not
  // the same fact as the reply carrying the substituted id.
  const lobby = await lobbyActor(state.hostileLobby, devPlayer(VICTIM_PLAYER));
  const tables = await lobby.get_tables();
  const rows = tables
    .map((t) => ({
      id: Number(t.id),
      name: t.name,
      canisterId: t.canister_id?.[0] ? t.canister_id[0].toText() : null,
    }))
    .sort((a, b) => a.id - b.id);
  const planted = rows.find((r) => r.canisterId === state.hostileTable);
  if (!planted) {
    die(
      `the hostile lobby ${state.hostileLobby} does not return ${state.hostileTable} from `
      + `get_tables(). Rows: ${JSON.stringify(rows)}`,
    );
  }
  log(`  get_tables() row ${planted.id} "${planted.name}" -> ${planted.canisterId}  (SUBSTITUTED)`);
  for (const r of rows.filter((x) => x !== planted)) {
    log(`  get_tables() row ${r.id} "${r.name}" -> ${r.canisterId}`);
  }
  return { ...state, plantedRow: planted, rows };
}

// ---------------------------------------------------------------------------
// the bundle: the real frontend, built against the lobby that lies
// ---------------------------------------------------------------------------

function buildSubstitutedBundle(localIds, hostileLobby) {
  const backup = `${FRONTEND_DIST}.finding42-backup`;
  fs.rmSync(SUBSTITUTED_DIST, { recursive: true, force: true });
  fs.rmSync(backup, { recursive: true, force: true });
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const hadDist = fs.existsSync(FRONTEND_DIST);
  if (hadDist) fs.renameSync(FRONTEND_DIST, backup);
  try {
    execFileSync('npm', ['run', 'build'], {
      cwd: FRONTEND_DIR,
      stdio: 'inherit',
      timeout: 15 * 60_000,
      env: {
        ...process.env,
        NODE_ENV: 'production',
        DFX_NETWORK: 'local',
        // THE SUBSTITUTION IS ONLY IN THE LOBBY. Everything else about this
        // build is the honest local build, including the table ids the build was
        // produced against -- which is the trust root the fix uses.
        VITE_CANISTER_ID_LOBBY: hostileLobby,
        VITE_CANISTER_ID_HISTORY: localIds.history,
        CANISTER_ID_LOBBY: hostileLobby,
        CANISTER_ID_HISTORY: localIds.history,
        CANISTER_ID_LEDGER: ICP_LEDGER_CANISTER_ID,
        CANISTER_ID_TABLE_1: localIds.table_1,
        CANISTER_ID_TABLE_2: localIds.table_2,
        CANISTER_ID_TABLE_3: localIds.table_3,
        CANISTER_ID_BTC_TABLE_1: localIds.btc_table_1,
        VITE_CANISTER_ID_TABLE_1: localIds.table_1,
        VITE_CANISTER_ID_TABLE_2: localIds.table_2,
        VITE_CANISTER_ID_TABLE_3: localIds.table_3,
        VITE_CANISTER_ID_BTC_TABLE_1: localIds.btc_table_1,
        CANISTER_ID: undefined,
        CANISTER_CANDID_PATH: undefined,
      },
    });
    if (!fs.existsSync(path.join(FRONTEND_DIST, 'index.html'))) {
      throw new Error('the build produced no dist/index.html');
    }
    fs.renameSync(FRONTEND_DIST, SUBSTITUTED_DIST);
  } finally {
    if (hadDist && !fs.existsSync(FRONTEND_DIST)) fs.renameSync(backup, FRONTEND_DIST);
    fs.rmSync(backup, { recursive: true, force: true });
  }

  // Prove the substitution really is in the bytes that will be served.
  const files = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (/\.(js|html)$/.test(p)) files.push(p);
    }
  };
  walk(SUBSTITUTED_DIST);
  const carriesHostileLobby = files.some((f) => fs.readFileSync(f, 'utf8').includes(hostileLobby));
  if (!carriesHostileLobby) {
    throw new Error(
      `the built bundle does not contain ${hostileLobby}; the substituted lobby id did not land, `
      + 'so this run would prove nothing.',
    );
  }
  const trustedIdsInBundle = [localIds.table_1, localIds.table_2, localIds.table_3]
    .filter((id) => files.some((f) => fs.readFileSync(f, 'utf8').includes(id)));
  log(`  bundle carries the substituted lobby id: yes`);
  log(`  bundle carries pinned local table ids:   ${trustedIdsInBundle.length}/3 `
    + `${trustedIdsInBundle.length === 0 ? '(NO TRUST ROOT COMPILED IN -- pre-fix build)' : ''}`);
  return { dist: SUBSTITUTED_DIST, trustedIdsInBundle };
}

// ---------------------------------------------------------------------------
// the victim's wallet: the deposit-address panel only renders for a player who
// cannot cover the minimum deposit, so put them there and put them back after.
// ---------------------------------------------------------------------------

// MIN_DEPOSIT (20_000) + two ledger fees (20_000) is the modal's own threshold.
const MODAL_WALLET_THRESHOLD = 40_000n;
// What a dev player's wallet holds on a freshly brought-up local stack, and what
// this script puts back afterwards so a second run starts from the same place.
const DEV_WALLET_TARGET = 60_000n;
const LEDGER_FEE = 10_000n;

async function victimBalance() {
  const identity = devPlayer(VICTIM_PLAYER);
  const ledger = await ledgerActor(ICP_LEDGER_CANISTER_ID, identity);
  return ledger.icrc1_balance_of({ owner: identity.getPrincipal(), subaccount: [] });
}

async function drainVictimWallet() {
  const identity = devPlayer(VICTIM_PLAYER);
  const ledger = await ledgerActor(ICP_LEDGER_CANISTER_ID, identity);
  const before = await victimBalance();
  if (before < MODAL_WALLET_THRESHOLD) {
    log(`  player ${VICTIM_PLAYER} wallet already at ${before} e8s (below the modal's threshold)`);
    return { before };
  }
  const moved = before - MODAL_WALLET_THRESHOLD + LEDGER_FEE + 1n;
  const res = await ledger.icrc1_transfer({
    to: { owner: devPlayer(1).getPrincipal(), subaccount: [] },
    fee: [LEDGER_FEE], memo: [], from_subaccount: [], created_at_time: [], amount: moved,
  });
  if ('Err' in res) throw new Error(`could not drain the victim wallet: ${JSON.stringify(res.Err)}`);
  log(`  player ${VICTIM_PLAYER} wallet: ${before} -> ${await victimBalance()} e8s `
    + '(below the modal\'s threshold, which is what makes the address panel render)');
  return { before };
}

/**
 * Puts the victim's wallet back where a fresh `local-up` leaves it.
 *
 * Funded from the LOCAL DEPLOYER identity, not from another dev player. The
 * first version of this took the money off dev player 1 and put it back the same
 * way, and after three runs the fee had eaten enough that the refill itself was
 * refused for InsufficientFunds -- silently, because `icrc1_transfer` returns
 * `Err` in the reply rather than throwing. A fixture that degrades every time it
 * is used is a fixture that stops reproducing.
 */
async function refillVictimWallet() {
  const now = await victimBalance();
  if (now >= DEV_WALLET_TARGET) return;
  const missing = DEV_WALLET_TARGET - now;
  const icpAmount = (Number(missing) / 1e8).toFixed(8);
  try {
    icp([
      'token', 'transfer', icpAmount, devPlayer(VICTIM_PLAYER).getPrincipal().toText(),
      '-e', 'local', '--identity', 'cd-local-deployer', '-q',
    ]);
  } catch (e) {
    log(`  ! could not restore player ${VICTIM_PLAYER}'s wallet: ${String(e).split('\n')[0]}`);
  }
  log(`  player ${VICTIM_PLAYER} wallet restored to ${await victimBalance()} e8s`);
}

// ---------------------------------------------------------------------------
// the screen
// ---------------------------------------------------------------------------

async function captureDepositModal(tableName, outPrefix) {
  const browser = await launchBrowser({ log: () => {} });
  const context = await newContext(browser, VIEWPORTS.desktop, { log: () => {} });
  const page = await context.newPage();
  const consoleErrors = [];
  page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push(m.text()); });
  try {
    await openApp(page);
    await devLogin(page, VICTIM_PLAYER);

    const row = page.locator('tr', { has: page.locator('.table-name', { hasText: tableName }) });
    await row.first().waitFor({ timeout: 60_000 });
    await row.first().click();
    await page.waitForSelector('.poker-table', { timeout: 60_000 });

    const deposit = page.locator('.wallet-action-btn.deposit');
    if (!(await deposit.first().isVisible().catch(() => false))) {
      const toggle = page.locator('.panel-toggle');
      if (await toggle.first().isVisible().catch(() => false)) await toggle.first().click();
    }
    await deposit.first().click({ timeout: 60_000 });
    await page.waitForSelector('.modal-content', { timeout: 60_000 });
    await page.waitForSelector('#deposit-modal-title', { timeout: 60_000 });
    // The address is derived after the wallet balance query resolves.
    await page.waitForFunction(
      () => !document.querySelector('.deposit-address-section .address-hint')
        ?.textContent?.includes('Deriving'),
      undefined,
      { timeout: 60_000 },
    ).catch(() => {});
    // The modal body scrolls: the address panel is below the fold, and a
    // screenshot that cannot show the address is not evidence about the address.
    await page.locator('.deposit-address-section, .untrusted-table').first()
      .scrollIntoViewIfNeeded({ timeout: 10_000 })
      .catch(() => {});
    await settle(page);

    const text = (sel) => page.locator(sel).first().textContent().catch(() => null);
    const observed = {
      trustAttribute: await page.locator('.modal-content').first()
        .getAttribute('data-table-trust').catch(() => null),
      addressShown: ((await text('.address-value')) || '').trim() || null,
      addressSectionPresent: (await page.locator('.deposit-address-section').count()) > 0,
      warning: ((await text('.address-mismatch')) || '').replace(/\s+/g, ' ').trim() || null,
      refusal: ((await text('.untrusted-table')) || '').replace(/\s+/g, ' ').trim() || null,
      networkLine: ((await text('.network-line')) || '').replace(/\s+/g, ' ').trim() || null,
      claimDisabled: await page.locator('.deposit-address-section ~ .actions .btn-primary')
        .first().isDisabled().catch(() => null),
      consoleErrors,
    };

    fs.mkdirSync(OUT_DIR, { recursive: true });
    await page.screenshot({ path: `${outPrefix}-full.png`, fullPage: true });
    await page.locator('.modal-content').first()
      .screenshot({ path: `${outPrefix}-modal.png` })
      .catch(() => {});
    return observed;
  } finally {
    await context.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

async function main() {
  if (!(await gatewayIsUp())) {
    die(`the local replica is not answering at ${GATEWAY_ORIGIN}. Run ./scripts/dev.sh local-up`);
  }
  const localIds = readLocalIds();
  for (const need of ['lobby', 'history', 'table_1', 'table_2', 'table_3', 'frontend']) {
    if (!localIds[need]) die(`local canister "${need}" is not deployed; run ./scripts/dev.sh local-up`);
  }

  step('1/5  stand up the substitution (a lobby that names a table this build never pinned)');
  const sub = await ensureSubstitution(localIds);

  step('2/5  what each canister says the victim\'s deposit address is');
  const victim = devPlayerPrincipal(VICTIM_PLAYER);
  const honest = await tableActor(localIds.table_1, devPlayer(VICTIM_PLAYER));
  const hostile = await tableActor(sub.hostileTable, devPlayer(VICTIM_PLAYER));
  const honestAddress = await honest.get_deposit_address();
  const attackerAddress = await hostile.get_deposit_address();
  log(`  victim principal                     ${victim}`);
  log(`  address at the PINNED table_1        ${honestAddress}`);
  log(`  address at the SUBSTITUTED canister  ${attackerAddress}   <- the attacker's account`);
  if (honestAddress === attackerAddress) {
    die('the two canisters report the same address; the fixture is wrong and this run proves nothing');
  }

  step('3/5  build the real frontend against the lobby that lies');
  const built = buildSubstitutedBundle(localIds, sub.hostileLobby);

  step('4/5  open the deposit modal in a browser and photograph it');
  await drainVictimWallet();
  process.env.SHOTS_SERVE_DIST = built.dist;
  const proxy = await startGatewayProxy({ frontendCanisterId: localIds.frontend, log });
  setAppOrigin(APP_ORIGIN);
  let observed;
  let control;
  try {
    observed = await captureDepositModal(sub.plantedRow.name, path.join(OUT_DIR, 'deposit-substituted'));
    // THE CONTROL ARM. A deposit path that refuses EVERYTHING would satisfy the
    // assertion above and would be a worse product than the defect. The same
    // lobby also names two HONEST tables -- the ids this bundle was built with --
    // so the same run opens one of them and requires a real address to appear.
    const honestRow = sub.rows.find((r) => r.canisterId === localIds.table_2);
    if (honestRow) {
      control = await captureDepositModal(honestRow.name, path.join(OUT_DIR, 'deposit-pinned'));
      control.tableCanisterId = honestRow.canisterId;
    }
  } finally {
    await proxy.close().catch(() => {});
    await refillVictimWallet();
  }

  step('5/5  verdict');
  const shown = observed.addressShown;
  const substitutionSucceeded = Boolean(shown) && shown.toLowerCase() === attackerAddress.toLowerCase();
  const refused = !shown && observed.trustAttribute === 'refused';

  // The control arm's answer, checked against the canister rather than against
  // this script's own arithmetic.
  let controlOk = null;
  let controlExpected = null;
  if (control) {
    const pinnedTable = await tableActor(localIds.table_2, devPlayer(VICTIM_PLAYER));
    controlExpected = await pinnedTable.get_deposit_address();
    const addressOk = Boolean(control.addressShown)
      && control.addressShown.toLowerCase() === String(controlExpected).toLowerCase();
    // `pinned` only exists on a build that HAS a trust root; a pre-fix build has
    // no attribute at all, which is a true observation about it and not a
    // failure of the pinned table.
    const trustOk = control.trustAttribute === 'pinned';
    controlOk = addressOk && trustOk;
    control.addressOk = addressOk;
    control.trustOk = trustOk;
    log(`  control: pinned table ${localIds.table_2} published `
      + `${control.addressShown ?? '(none)'} (canister says ${controlExpected}) `
      + `-> address ${addressOk ? 'OK' : 'MISMATCH'}, `
      + `trust attribute ${control.trustAttribute ?? 'ABSENT (no trust root in this build)'}`);
  }

  const evidence = {
    finding: 42,
    ranAt: new Date().toISOString(),
    revision: (() => {
      try { return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: REPO_ROOT, encoding: 'utf8' }).trim(); }
      catch { return 'unknown'; }
    })(),
    hostileLobby: sub.hostileLobby,
    hostileTable: sub.hostileTable,
    pinnedTable1: localIds.table_1,
    victimPrincipal: victim,
    addressAtPinnedTable: honestAddress,
    addressAtSubstitutedCanister: attackerAddress,
    pinnedTableIdsFoundInBundle: built.trustedIdsInBundle,
    observed,
    control: control ? { ...control, expectedAddress: controlExpected, ok: controlOk } : null,
    substitutionSucceeded,
    refused,
    screenshots: [
      path.relative(REPO_ROOT, path.join(OUT_DIR, 'deposit-substituted-full.png')),
      path.relative(REPO_ROOT, path.join(OUT_DIR, 'deposit-substituted-modal.png')),
    ],
  };
  fs.writeFileSync(path.join(OUT_DIR, 'evidence.json'), `${JSON.stringify(evidence, null, 2)}\n`);

  log(`  address rendered to the player       ${shown ?? '(none)'}`);
  log(`  warning rendered beside it           ${observed.warning ?? '(none -- every check reads green)'}`);
  log(`  refusal rendered instead             ${observed.refusal ?? '(none)'}`);
  log(`  network line                         ${observed.networkLine ?? '(none)'}`);
  log(`  evidence                             ${path.relative(REPO_ROOT, path.join(OUT_DIR, 'evidence.json'))}`);

  if (substitutionSucceeded) {
    console.error(
      '\n  ✗ FINDING 42 REPRODUCED. The modal published '
      + `${shown}\n    which is the deposit account of ${sub.hostileTable}, a canister this build `
      + 'never named.\n    The address panel showed no warning, so every check on the screen read green.',
    );
    process.exit(1);
  }
  if (refused) {
    log(`\n  ✓ REFUSED. The deposit path would not derive an address from ${sub.hostileTable}.`);
    log(`    "${observed.refusal}"`);
    if (controlOk === false) {
      console.error(
        '\n  ✗ BUT THE CONTROL ARM FAILED: the PINNED table did not publish the address its own '
        + `canister reports (screen=${control.addressShown ?? '(none)'}, canister=${controlExpected}, `
        + `trust=${control.trustAttribute}). A deposit path that refuses everything is not a fix.`,
      );
      process.exit(1);
    }
    if (controlOk === null) {
      console.error('\n  ! the control arm did not run; the refusal above is unaccompanied.');
      process.exit(3);
    }
    log('  ✓ CONTROL: the pinned table still publishes the address its own canister reports.');
    process.exit(0);
  }
  console.error(
    '\n  ? INCONCLUSIVE. The modal published '
    + `${shown ?? '(no address)'} and the trust attribute read `
    + `${observed.trustAttribute ?? '(absent)'}.\n    Read ${path.relative(REPO_ROOT, OUT_DIR)} before believing anything.`,
  );
  process.exit(3);
}

main().catch((e) => {
  console.error(e?.stack || String(e));
  process.exit(2);
});

export { fileURLToPath };
