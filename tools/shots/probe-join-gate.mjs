#!/usr/bin/env node
// A SIT TAP SIGNED OUT OPENS SIGN-IN, AND A SIT TAP SHORT OF THE BUY-IN OPENS
// THE CASHIER: the join gate, photographed.
//
//   node tools/shots/probe-join-gate.mjs [--viewports desktop,mobile]
//
// The audit's critical mobile finding: an anonymous tap on an empty seat went
// straight to the canister and came back as a red "Insufficient balance"
// panel over the whole phone header. The gate (routes/+page.svelte, decided by
// lib/join-gate.js from the TABLE canister's own config and the escrow as last
// read) is unit-tested at the decision level; no resting scene taps a seat
// signed out, so this probe does, on the real app against the real canisters:
//
//   1. ANONYMOUS. Open the table as a visitor, tap "Sit". Asserts that NO
//      update call reached the table canister (the request log carries no
//      POST .../canister/<table>/call), and that the sign-in path ran: on a
//      build that knows an Internet Identity a popup opens to it; on this
//      local build (no II canister, docs/DEFECTS.md T-39) auth.login() rejects
//      with its own "sign-in cannot be started" message, which is the proof
//      that auth.login() was invoked rather than join_table. The canister's
//      "Insufficient balance" wording anywhere on screen fails the probe.
//   2. SIGNED IN. Dev-login as the hero, whose escrow at this table the probe
//      reads through the agent, and tap "Sit" again. The client must do what
//      the chain's own figures say: escrow under config.min_buy_in opens the
//      Deposit sheet with the field on the exact shortfall; escrow at or over
//      it takes the seat.
//
// Not a gate. Writes PROBE-join-gate-*.png and PROBE-join-gate.json beside the
// run's other PNGs. LOCAL REPLICA ONLY, like run.mjs (lib/ids.mjs refuses
// mainnet). Do not run it concurrently with run.mjs or the other probes: they
// all drive table_2.

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { devLogin, enterTable, launchBrowser, newContext, openApp, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { tableActorFor, view } from './lib/table-driver.mjs';
import { emptyTable, tableDisplayName } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);
const TABLE = 'table_2';

function parseArgs(argv) {
  const out = { viewports: ['desktop', 'mobile'] };
  for (let i = 2; i < argv.length; i += 1) {
    if (argv[i] === '--viewports') out.viewports = String(argv[++i] || '').split(',').filter(Boolean);
    else throw new Error(`Unknown argument: ${argv[i]}`);
  }
  return out;
}

async function requireReplica() {
  const res = await fetch(`${GATEWAY_ORIGIN}/api/v2/status`, { signal: AbortSignal.timeout(8000) });
  if (!res.ok && res.status !== 400) throw new Error(`gateway HTTP ${res.status}`);
}

async function resolveTableNames(ids) {
  const lobby = await lobbyActor(ids.lobby);
  const tables = await lobby.get_tables();
  const byCanister = new Map();
  for (const t of tables) {
    const cid = optional(t.canister_id);
    if (cid) byCanister.set(cid.toText(), t.name);
  }
  const names = {};
  for (const key of ['table_1', 'table_2', 'table_3', 'btc_table_1']) {
    if (ids[key] && byCanister.has(ids[key])) names[key] = byCanister.get(ids[key]);
  }
  return names;
}

/**
 * Every `join_table` update call the page sends to one canister. The page
 * also sends `check_timeouts` updates on its own timer (even as a spectator),
 * so the method name in the CBOR envelope, not the URL, is what is counted.
 */
function watchJoinCalls(page, canisterId) {
  const calls = [];
  page.on('request', (req) => {
    const url = req.url();
    if (req.method() !== 'POST' || !url.includes(`/canister/${canisterId}/call`)) return;
    const body = req.postDataBuffer();
    if (body && body.includes('join_table')) calls.push(url);
  });
  return calls;
}

/** What the screen says after the tap. */
function readOutcome(page) {
  return page.evaluate(() => {
    const text = (sel) => (document.querySelector(sel)?.textContent || '').replace(/\s+/g, ' ').trim() || null;
    const field = document.querySelector('#deposit-amount');
    return {
      toast: text('.toast'),
      depositSheetOpen: !!document.querySelector('#deposit-modal-title'),
      depositField: field ? field.value : null,
      seatedAsHero: !!document.querySelector('.player-nameplate.highlight-me'),
      emptySeats: document.querySelectorAll('.join-seat').length,
    };
  });
}

async function tapFirstSeat(page) {
  const seat = page.locator('.join-seat').first();
  await seat.waitFor({ timeout: 30_000 });
  await seat.click();
}

async function probeViewport(vp, ctx, browser, outDir) {
  const tableId = ctx.tableIds[TABLE];
  const results = { viewport: vp.name, anonymous: null, signedIn: null };
  log(`\n[${vp.name}] emptying ${TABLE}, opening it as a visitor`);
  await emptyTable(ctx, TABLE);

  // ---- 1. anonymous --------------------------------------------------------
  {
    const context = await newContext(browser, vp, { log });
    const page = await context.newPage();
    watchPage(page);
    const calls = watchJoinCalls(page, tableId);
    let popupUrl = null;
    page.on('popup', (p) => { popupUrl = p.url(); });
    try {
      await openApp(page);
      await enterTable(page, tableDisplayName(ctx, TABLE));
      await settle(page);
      const callsBeforeTap = calls.length;
      await tapFirstSeat(page);
      await page.waitForTimeout(2000);
      await settle(page);
      const outcome = await readOutcome(page);
      const file = path.join(outDir, `PROBE-join-gate-anonymous-${vp.name}.png`);
      await page.screenshot({ path: file, fullPage: false });
      const canisterCalls = calls.length - callsBeforeTap;
      const canisterError = /insufficient balance|deposit .* first/i.test(outcome.toast || '');
      const loginPathRan = !!popupUrl || /internet identity|sign-in/i.test(outcome.toast || '');
      results.anonymous = {
        file: path.relative(REPO_ROOT, file),
        joinCallsToTableAfterTap: canisterCalls,
        popupUrl,
        toast: outcome.toast,
        canisterErrorShown: canisterError,
        loginPathRan,
        ok: canisterCalls === 0 && !canisterError && loginPathRan,
      };
      log(`  anonymous tap: ${canisterCalls} join_table call(s) to the table; popup ${popupUrl ? popupUrl : 'none'}; `
        + `toast "${outcome.toast ?? ''}"`);
      log(`  ${results.anonymous.ok ? '✓' : '✗'} sign-in path ${loginPathRan ? 'ran' : 'DID NOT run'}, `
        + `canister error ${canisterError ? 'SHOWN' : 'absent'}`);
    } finally {
      await page.close();
      await context.close();
    }
  }

  // ---- 2. signed in ---------------------------------------------------------
  {
    const table = await tableActorFor(HERO_PLAYER, tableId);
    const escrow = BigInt(await table.get_balance());
    const v = await view(HERO_PLAYER, tableId);
    const minBuyIn = BigInt(v.config.min_buy_in);
    const expectDeposit = escrow < minBuyIn;
    const shortfall = expectDeposit ? minBuyIn - escrow : 0n;
    log(`  hero escrow ${escrow} vs min_buy_in ${minBuyIn}: the chain says ${expectDeposit ? `deposit (shortfall ${shortfall})` : 'seat'}`);

    const context = await newContext(browser, vp, { log });
    const page = await context.newPage();
    watchPage(page);
    const calls = watchJoinCalls(page, tableId);
    try {
      await openApp(page);
      await devLogin(page, HERO_PLAYER);
      await enterTable(page, tableDisplayName(ctx, TABLE));
      // The balance must have been READ before the tap (null is "unknown",
      // and unknown lets the canister decide): wait for the dock's figure.
      await page.waitForFunction(() => {
        const el = document.querySelector('.wallet-panel .balance-value, .collapsed-balance');
        return !!el && /\d/.test(el.textContent || '');
      }, null, { timeout: 30_000 });
      await settle(page);
      const callsBeforeTap = calls.length;
      await tapFirstSeat(page);
      await page.waitForFunction(
        () => !!document.querySelector('#deposit-modal-title') || !!document.querySelector('.player-nameplate.highlight-me') || !!document.querySelector('.toast'),
        null, { timeout: 20_000 },
      ).catch(() => {});
      await page.waitForTimeout(800);
      await settle(page);
      const outcome = await readOutcome(page);
      const file = path.join(outDir, `PROBE-join-gate-signed-in-${vp.name}.png`);
      await page.screenshot({ path: file, fullPage: false });
      const canisterCalls = calls.length - callsBeforeTap;
      const fieldE8s = outcome.depositField ? BigInt(Math.round(Number(outcome.depositField) * 1e8)) : null;
      const ok = expectDeposit
        ? outcome.depositSheetOpen && fieldE8s === shortfall && canisterCalls === 0
        : outcome.seatedAsHero;
      results.signedIn = {
        file: path.relative(REPO_ROOT, file),
        escrow: escrow.toString(),
        minBuyIn: minBuyIn.toString(),
        chainSays: expectDeposit ? 'deposit' : 'seat',
        shortfall: shortfall.toString(),
        joinCallsToTableAfterTap: canisterCalls,
        depositSheetOpen: outcome.depositSheetOpen,
        depositField: outcome.depositField,
        seatedAsHero: outcome.seatedAsHero,
        toast: outcome.toast,
        ok,
      };
      log(`  signed-in tap: deposit sheet ${outcome.depositSheetOpen ? `open on "${outcome.depositField}"` : 'closed'}, `
        + `seated ${outcome.seatedAsHero}, ${canisterCalls} join_table call(s), toast "${outcome.toast ?? ''}"`);
      log(`  ${ok ? '✓' : '✗'} the client did what the chain's figures say`);
    } finally {
      await page.close();
      await context.close();
    }
  }
  return results;
}

async function main() {
  const args = parseArgs(process.argv);
  await requireReplica();
  const ids = readLocalIds();
  const frontendId = requireId(ids, 'frontend');
  setAppOrigin(`http://${frontendId}.${GATEWAY_HOST}:${GATEWAY_PORT}`);
  const tableNames = await resolveTableNames(ids);
  const ctx = {
    ids,
    tableIds: { table_1: ids.table_1, table_2: ids.table_2, table_3: ids.table_3, btc_table_1: ids.btc_table_1 },
    tableNames,
    frontendId,
    log,
  };
  const { shaDir } = runDirs(gitShortSha());
  const outDir = path.join(shaDir, 'probe');
  fs.mkdirSync(outDir, { recursive: true });

  const browser = await launchBrowser({ log });
  const out = [];
  try {
    for (const name of args.viewports) {
      const vp = VIEWPORTS[name];
      if (!vp) throw new Error(`Unknown viewport ${name}`);
      out.push(await probeViewport(vp, ctx, browser, outDir));
    }
  } finally {
    await browser.close();
  }
  const report = path.join(outDir, 'PROBE-join-gate.json');
  fs.writeFileSync(report, JSON.stringify(out, null, 2));
  log(`\nreport: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => !(r.anonymous?.ok && r.signedIn?.ok));
  if (bad.length) {
    log(`FAILED at: ${bad.map((b) => b.viewport).join(', ')}`);
    return 1;
  }
  log('the join gate: signed out, Sit opens sign-in and nothing reaches the canister; signed in, Sit does what the chain\'s figures say');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(`\nFATAL: ${e.message}`);
  if (process.env.SHOTS_DEBUG) console.error(e.stack);
  process.exit(2);
});
