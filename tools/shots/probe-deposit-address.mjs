#!/usr/bin/env node
// THE ADDRESS ROUTE, PHOTOGRAPHED AND PAID FOR REAL (a probe, not a gate).
//
//   node tools/shots/probe-deposit-address.mjs [--viewports desktop,mobile]
//
// Every harness player holds a funded wallet, so no resting scene ever lands
// on the deposit sheet's ADDRESS route (the QR card a new player sees when
// the connected wallet cannot pay the minimum plus both fees). This probe
// drains a dev player's ledger balance below that floor by a real transfer,
// signs in as that player, opens the sheet and checks that:
//
//   1. THE CARD. The route is `address`; the address on the card equals the
//      one lib/deposit-address.mjs derives on its own from the table canister
//      and the player's principal; the QR is painted; the limits line leads
//      with the address minimum; 5 of 5 protected notices; the touch floor on
//      the phone. Photographed.
//   2. THE ARRIVAL. A second identity (the hero) pays the derived subaccount
//      with a real icrc1_transfer. Within the card's poll the figure it names
//      ("Detected 0.0005 ICP") equals icrc1_balance_of on that subaccount.
//      Photographed.
//   3. THE SWEEP. Without a Claim button pressed, the sheet calls
//      claim_external_deposit itself and shows the receipt; the receipt's
//      "Table balance now" equals get_balance(), the escrow rose by the
//      amount less one ledger fee (the sweep pays the fee out of what
//      arrived), the subaccount is empty again. Photographed.
//
// The drained player is topped back up at the end. Writes
// PROBE-address-*.png and PROBE-address.json beside the run's other PNGs.
// LOCAL REPLICA ONLY (lib/ids.mjs refuses mainnet). Run after run.mjs has
// deployed the tree; never concurrently with run.mjs or another probe.

import fs from 'node:fs';
import path from 'node:path';
import {
  GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, REPO_ROOT, VIEWPORTS,
} from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import {
  devLogin, enterTable, launchBrowser, newContext, openApp, setAppOrigin, settle, watchPage,
} from './lib/browser.mjs';
import { foldProtectedNotices, probeProtectedNotices } from './lib/protected-notices.mjs';
import { foldTouchTargets, measureTouchTargets, TOUCH_MIN_PX } from './lib/touch-targets.mjs';
import { checkFigure, foldFigures } from './lib/money.mjs';
import {
  drainLedgerTo, ensureLedgerFunds, ledgerBalance, ledgerSubaccountBalance, ledgerTransferFee,
  ledgerTransferFrom, tableActorFor,
} from './lib/table-driver.mjs';
import { devPlayerPrincipal } from './lib/identities.mjs';
import { depositAddressHex, depositSubaccountBytes } from './lib/deposit-address.mjs';
import { emptyTable, tableDisplayName } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);
const TABLE = 'table_2';

/** The player whose wallet is drained (never the hero, who pays). */
const NEW_PLAYER = 4;
/** What the drained wallet keeps: below the wallet route's floor of minimum plus two fees. */
const LEAVE_E8S = 30_000n;
/** What the hero sends to the address, and the floor the hero is topped up to. */
const SEND_E8S = 50_000n;
const PAYER_FLOOR = 1_000_000n;
/** What the drained player is topped back up to afterwards. */
const RESTORE_FLOOR = 1_000_000n;

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

const textOf = (page, selector) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null;
}, selector);

const attrOf = (page, selector, attr) => page.evaluate(({ sel, attr }) => (
  document.querySelector(sel)?.getAttribute(attr) ?? null
), { sel: selector, attr });

const rowsOf = (page, selector, attr) => page.evaluate(({ sel, attr }) => (
  [...document.querySelectorAll(sel)].map((row) => ({
    id: row.getAttribute(attr),
    text: ((row.querySelector('dd .value') || row.querySelector('dd'))?.textContent || '').replace(/\s+/g, ' ').trim(),
  }))
), { sel: selector, attr });

const stepStatuses = (page) => page.evaluate(() => (
  [...document.querySelectorAll('.cashier-steps .step')].map((s) => s.getAttribute('data-status'))
));

async function shoot(page, outDir, name, vp) {
  const file = path.join(outDir, `PROBE-address-${name}-${vp.name}.png`);
  await settle(page);
  await page.screenshot({ path: file, fullPage: false });
  return path.relative(REPO_ROOT, file);
}

async function openDeposit(page) {
  const btn = page.locator('.wallet-action-btn.deposit');
  if (!(await btn.first().isVisible().catch(() => false))) {
    const toggle = page.locator('.panel-toggle');
    if (await toggle.first().isVisible().catch(() => false)) await toggle.first().click();
  }
  await btn.first().waitFor({ timeout: 30_000 });
  await btn.first().click();
  await page.waitForSelector('#deposit-modal-title', { timeout: 30_000 });
  await page.waitForSelector('.modal-content .solvency', { timeout: 10_000 }).catch(() => {});
  await page.waitForFunction(() => !document.querySelector('.runway-notice.pending'), null, { timeout: 15_000 }).catch(() => {});
  // The wallet balance read decides the route; wait for it.
  await page.waitForFunction(() => !!document.querySelector('.modal-content .balance-crypto'), null, { timeout: 15_000 }).catch(() => {});
  await page.waitForTimeout(400);
  await settle(page);
}

async function noticesUnder(page, where, problems) {
  const n = foldProtectedNotices(await probeProtectedNotices(page), where);
  if (!n.ok) problems.push(...n.problems);
  return `${n.onScreen}/${n.total}`;
}

async function touchUnder(page, vp, where, problems) {
  if (vp.name !== 'mobile') return null;
  const m = await measureTouchTargets(page, { touchMin: TOUCH_MIN_PX, scope: '.modal-content' });
  const f = foldTouchTargets(m, { scene: where, viewport: vp.name, touchMin: TOUCH_MIN_PX });
  if (!f.ok) problems.push(...f.problems.map((p) => `${where}: ${p}`));
  return f.notes ?? `${(m?.items || []).length} controls`;
}

async function probeViewport(vp, ctx, browser, outDir) {
  const tableId = ctx.tableIds[TABLE];
  const problems = [];
  const figures = [];
  const states = { viewport: vp.name };
  log(`\n[${vp.name}] the address route on ${TABLE}`);

  await emptyTable(ctx, TABLE);
  const payer = await ensureLedgerFunds(HERO_PLAYER, PAYER_FLOOR);
  const drained = await drainLedgerTo(NEW_PLAYER, LEAVE_E8S, HERO_PLAYER);
  const player = devPlayerPrincipal(NEW_PLAYER);
  const subaccount = depositSubaccountBytes(player);
  const expectedAddress = depositAddressHex(tableId, player);
  const fee = BigInt(await ledgerTransferFee());
  const table = await tableActorFor(NEW_PLAYER, tableId);
  const escrowBefore = BigInt(await table.get_balance());
  const arrivedBefore = BigInt(await ledgerSubaccountBalance(tableId, subaccount));
  log(`  player ${NEW_PLAYER} wallet ${drained.after} e8s (was ${drained.before}, moved ${drained.moved}); hero wallet ${payer.balance} e8s; escrow ${escrowBefore} e8s; at the address already ${arrivedBefore} e8s`);
  states.setup = {
    walletE8s: drained.after.toString(), escrowBeforeE8s: escrowBefore.toString(),
    arrivedBeforeE8s: arrivedBefore.toString(), expectedAddress,
  };

  const context = await newContext(browser, vp, { log });
  const page = await context.newPage();
  watchPage(page);
  try {
    await openApp(page);
    await devLogin(page, NEW_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await settle(page);
    await openDeposit(page);

    // 1. THE CARD.
    const route = await attrOf(page, '.modal-content', 'data-route');
    if (route !== 'address') problems.push(`the sheet opened on the "${route}" route, not the address route, with ${drained.after} e8s in the wallet`);
    const shown = (await textOf(page, '.modal-content .address-value') || '').replace(/\s+/g, '').toLowerCase();
    if (shown !== expectedAddress) problems.push(`the card shows ${shown || '(nothing)'}, not the address derived here (${expectedAddress})`);
    const qrPainted = await page.locator('.modal-content .qr svg').count();
    if (!qrPainted) problems.push('no QR is painted on the card');
    const limits = await textOf(page, '.modal-content .minimum-notice');
    if (!/^\s*Minimum to this address:/i.test(limits || '')) problems.push(`the limits line does not lead with the address minimum on the address route: "${limits}"`);
    const walletText = await textOf(page, '.modal-content .balance-crypto');
    figures.push(checkFigure('address route: wallet balance vs ledger', drained.after, walletText, { currency: 'ICP' }));
    states.card = {
      file: await shoot(page, outDir, 'card', vp),
      route, address: shown, qr: qrPainted > 0, limits,
      detect: await attrOf(page, '.modal-content', 'data-detect'),
      button: await textOf(page, '.modal-content .actions .btn-primary'),
      notices: await noticesUnder(page, 'the address card', problems),
      touch: await touchUnder(page, vp, 'deposit-address', problems),
    };
    if (/claim/i.test(states.card.button || '')) problems.push(`the button reads "${states.card.button}" before anything arrived: a new player should never see a Claim button`);
    log(`  card: route ${route}, address ${shown === expectedAddress ? 'equals the derived one' : 'DIFFERS'}, QR ${qrPainted ? 'painted' : 'MISSING'}, button "${states.card.button}", notices ${states.card.notices}`);

    // 2. THE ARRIVAL: the hero pays the derived subaccount.
    const block = await ledgerTransferFrom(HERO_PLAYER, { ownerText: tableId, subaccount, amountE8s: SEND_E8S });
    const arrived = BigInt(await ledgerSubaccountBalance(tableId, subaccount));
    log(`  hero paid ${SEND_E8S} e8s to the address (block ${block}); the address holds ${arrived} e8s`);
    // The figure is read the instant the card names it: the local replica
    // sweeps within a second of the detection, so a read after the still
    // can find the receipt in the card's place (the first run did).
    const seen = await page.waitForFunction(() => {
      const el = document.querySelector('.modal-content .detected-amount');
      if (!el) return null;
      const txt = (n) => (n ? (n.textContent || '').replace(/\s+/g, ' ').trim() : null);
      return {
        text: txt(el),
        line: txt(document.querySelector('.modal-content .detected')),
        detect: document.querySelector('.modal-content')?.getAttribute('data-detect') ?? null,
      };
    }, null, { timeout: 20_000 }).then((h) => h.jsonValue()).catch(() => null);
    if (!seen) {
      problems.push('the card did not detect the arrival within 20 s');
      states.detected = { file: await shoot(page, outDir, 'detected', vp) };
    } else {
      const file = path.join(outDir, `PROBE-address-detected-${vp.name}.png`);
      // The line sits at the phone's fold; bring it into the frame for the still.
      await page.evaluate(() => document.querySelector('.modal-content .detected')?.scrollIntoView({ block: 'center' })).catch(() => {});
      await page.screenshot({ path: file, fullPage: false });
      states.detected = { file: path.relative(REPO_ROOT, file), ...seen, block: String(block) };
      figures.push(checkFigure('address route: detected figure vs icrc1_balance_of(subaccount)', arrived, states.detected.text, { currency: 'ICP' }));
      log(`  detected: "${states.detected.line}"`);
    }

    // 3. THE SWEEP, with no button pressed.
    const receiptUp = await page.waitForSelector('.cashier-receipt', { timeout: 60_000 }).then(() => true).catch(() => false);
    if (!receiptUp) {
      const err = await textOf(page, '.modal-content .alert.error, .modal-content .alert');
      problems.push(`no receipt within 60 s of the arrival (error on screen: ${JSON.stringify(err)})`);
      states.receipt = { file: await shoot(page, outDir, 'receipt', vp), error: err };
    } else {
      await page.waitForTimeout(300);
      const escrowAfter = BigInt(await table.get_balance());
      const arrivedAfter = BigInt(await ledgerSubaccountBalance(tableId, subaccount));
      states.receipt = {
        file: await shoot(page, outDir, 'receipt', vp),
        title: await textOf(page, '.cashier-receipt h3'),
        rows: await rowsOf(page, '.cashier-receipt [data-receipt-row]', 'data-receipt-row'),
        statuses: await stepStatuses(page),
        escrowDeltaE8s: (escrowAfter - escrowBefore).toString(),
        leftAtAddressE8s: arrivedAfter.toString(),
      };
      const rrow = (id) => states.receipt.rows.find((r) => r.id === id)?.text ?? null;
      figures.push(checkFigure('address route: receipt "balance" vs get_balance()', escrowAfter, rrow('balance'), { currency: 'ICP' }));
      figures.push(checkFigure('address route: receipt "arrived" vs what the ledger held at the address', arrived, rrow('arrived'), { currency: 'ICP' }));
      figures.push(checkFigure('address route: receipt "fee" vs icrc1_fee()', fee, rrow('fee'), { currency: 'ICP' }));
      figures.push(checkFigure('address route: receipt "credited" vs arrived less one fee', arrived - fee, rrow('credited'), { currency: 'ICP' }));
      const expectedCredit = arrived - fee;
      if (escrowAfter - escrowBefore !== expectedCredit) {
        problems.push(`the escrow rose by ${escrowAfter - escrowBefore} e8s, not what arrived less one fee (${expectedCredit})`);
      }
      if (arrivedAfter !== 0n) problems.push(`the address still holds ${arrivedAfter} e8s after the sweep`);
      if (!states.receipt.statuses.every((s) => s === 'done')) problems.push(`the stepper is not all done on the receipt (${JSON.stringify(states.receipt.statuses)})`);
      log(`  receipt "${states.receipt.title}": ${states.receipt.rows.map((r) => `${r.id}=${r.text}`).join(', ')}; escrow +${states.receipt.escrowDeltaE8s} e8s; address holds ${arrivedAfter} e8s`);
      await page.click('.cashier-receipt .done');
      await page.waitForSelector('.modal-content', { state: 'detached', timeout: 15_000 });
    }
  } finally {
    await page.close();
    await context.close();
    // The drained player is a harness player: put the wallet back.
    const restored = await ensureLedgerFunds(NEW_PLAYER, RESTORE_FLOOR);
    log(`  player ${NEW_PLAYER} wallet restored to ${restored.balance} e8s`);
  }

  const folded = foldFigures(figures);
  if (!folded.ok) problems.push(...folded.mismatches.map((m) => `figure: ${m}`));
  const ok = problems.length === 0;
  log(`  ${ok ? '✓' : '✗'} ${vp.name}: ${ok ? 'green' : 'RED'}`);
  for (const p of problems) log(`    ! ${p}`);
  return { ...states, ok, problems, figures: folded };
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
  const report = path.join(outDir, 'PROBE-address.json');
  fs.writeFileSync(report, JSON.stringify(out, (_k, v) => (typeof v === 'bigint' ? v.toString() : v), 2));
  log(`\nreport: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => !r.ok);
  if (bad.length) {
    log(`FAILED at: ${bad.map((b) => b.viewport).join(', ')}`);
    return 1;
  }
  log('the address route: the card shows the derived address, the arrival is detected and swept in by itself, every figure equals the ledger and the table');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(process.env.SHOTS_DEBUG ? e : `FATAL: ${e.message}`);
  process.exit(2);
});
