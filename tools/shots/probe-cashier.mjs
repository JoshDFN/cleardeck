#!/usr/bin/env node
// THE CASHIER, PHOTOGRAPHED AND CHECKED AGAINST THE LEDGER (a probe, not a gate).
//
//   node tools/shots/probe-cashier.mjs [--viewports desktop,mobile]
//
// The `deposit` scene files the deposit sheet at rest. This probe drives the
// rest of the cashier on the REAL app against the REAL local canisters, with
// real money moving on the real local ledger, and checks every figure it
// photographs against the chain:
//
//   1. DEPOSIT. An amount typed: the cost summary's rows (you send / ledger
//      fees / total from your wallet / the table credits) against the typed
//      amount and the ledger's own icrc1_fee(); the button naming the amount;
//      5 of 5 protected notices; the solvency block above the button. Then
//      Deposit pressed: the stepper mid-flight, the receipt, and THE CLAIM ON
//      THE RECEIPT PROVED ON THE LEDGER: the hero's ledger balance fell by
//      exactly the amount plus two fees, the escrow rose by exactly the amount,
//      the receipt's "table balance now" equals get_balance().
//   2. WITHDRAW. The sheet at rest: the balance against get_balance(), the
//      destination against the account identifier derived from the hero's
//      principal, 5 of 5 notices, the touch floor on the phone. An amount
//      typed: the net rows against the fee. Withdraw pressed: the stepper, the
//      receipt with the ledger block, the cooldown counting down; the ledger
//      rose by amount minus fee and the escrow fell by the amount. When the
//      previous viewport's withdrawal is inside the canister's cooldown, the
//      refusal is photographed as the humane sentence it must be, and the
//      probe waits it out.
//
// Writes PROBE-cashier-*.png and PROBE-cashier.json beside the run's other
// PNGs. LOCAL REPLICA ONLY (lib/ids.mjs refuses mainnet). Run after
// `node tools/shots/run.mjs` has deployed the tree; never concurrently with
// run.mjs or another probe (they all drive table_2).

import fs from 'node:fs';
import path from 'node:path';
import { AccountIdentifier } from '@dfinity/ledger-icp';
import { Principal } from '@dfinity/principal';
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
  ensureLedgerFunds, ensureTableBalance, ledgerBalance, ledgerTransferFee, sleep, tableActorFor,
} from './lib/table-driver.mjs';
import { devPlayerPrincipal } from './lib/identities.mjs';
import { emptyTable, tableDisplayName } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);
const TABLE = 'table_2';

/** What the probe deposits and withdraws, in e8s, and as the field is typed. */
const DEPOSIT_E8S = 50_000n;
const DEPOSIT_TYPED = '0.0005';
const WITHDRAW_E8S = 100_000n;
const WITHDRAW_TYPED = '0.001';
/** The escrow and wallet floors the probe tops the hero up to first. */
const HERO_ESCROW_FLOOR = 1_000_000n;
const HERO_WALLET_FLOOR = 1_000_000n;

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

const rectOf = (page, selector) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), bottom: Math.round(r.bottom) };
}, selector);

const textOf = (page, selector) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null;
}, selector);

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
  const file = path.join(outDir, `PROBE-cashier-${name}-${vp.name}.png`);
  await settle(page);
  await page.screenshot({ path: file, fullPage: false });
  return path.relative(REPO_ROOT, file);
}

/** Opens the deposit or withdraw sheet from the dock's wallet panel. */
async function openCashier(page, kind) {
  const btn = page.locator(`.wallet-action-btn.${kind}`);
  if (!(await btn.first().isVisible().catch(() => false))) {
    const toggle = page.locator('.panel-toggle');
    if (await toggle.first().isVisible().catch(() => false)) await toggle.first().click();
  }
  await btn.first().waitFor({ timeout: 30_000 });
  await btn.first().click();
  await page.waitForSelector(kind === 'deposit' ? '#deposit-modal-title' : '#withdraw-modal-title', { timeout: 30_000 });
  // The solvency and runway reads are queries fired on mount; the resting
  // still waits for the block so the disclosures column is photographed as
  // a player sees it a beat later, not as an empty heading.
  await page.waitForSelector('.modal-content .solvency', { timeout: 10_000 }).catch(() => {});
  await page.waitForTimeout(400);
  await settle(page);
}

async function closeReceipt(page) {
  await page.click('.cashier-receipt .done');
  await page.waitForSelector('.modal-content', { state: 'detached', timeout: 15_000 });
}

/** The account identifier `withdraw` pays: the principal's default account. */
function defaultAccountHex(principalText) {
  return AccountIdentifier.fromPrincipal({ principal: Principal.fromText(principalText) }).toHex();
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

async function solvencyAboveButton(page) {
  const s = await rectOf(page, '.modal-content .solvency');
  const b = await rectOf(page, '.modal-content .actions .btn-primary');
  if (!s || !b) return { ok: s === null, solvency: s, button: b };
  return { ok: s.bottom <= b.y + 1, solvency: s, button: b };
}

// ---------------------------------------------------------------------------
// 1. THE DEPOSIT
// ---------------------------------------------------------------------------
async function probeDeposit(page, vp, ctx, outDir, tableId) {
  const problems = [];
  const hero = devPlayerPrincipal(HERO_PLAYER);
  const table = await tableActorFor(HERO_PLAYER, tableId);
  const fee = BigInt(await ledgerTransferFee());
  const ledgerBefore = BigInt(await ledgerBalance(hero));
  const escrowBefore = BigInt(await table.get_balance());

  await openCashier(page, 'deposit');
  const states = {};
  states.noticesAtRest = await noticesUnder(page, 'the deposit sheet at rest', problems);

  await page.fill('#deposit-amount', DEPOSIT_TYPED);
  await page.waitForTimeout(300);
  await settle(page);
  const geometry = await solvencyAboveButton(page);
  if (!geometry.ok) problems.push(`deposit: the solvency block is not above the Deposit button with an amount typed (${JSON.stringify(geometry)})`);
  states.summary = {
    file: await shoot(page, outDir, 'deposit-summary', vp),
    rows: await rowsOf(page, '.modal-content .cost-summary [data-row]', 'data-row'),
    button: await textOf(page, '.modal-content .actions .btn-primary'),
    noticesOnScreen: await noticesUnder(page, 'the deposit sheet with an amount typed', problems),
    touch: await touchUnder(page, vp, 'deposit-summary', problems),
    solvencyAboveButton: geometry.ok,
  };
  const expectedByRow = {
    send: DEPOSIT_E8S, fees: fee * 2n, total: DEPOSIT_E8S + fee * 2n, credited: DEPOSIT_E8S,
  };
  const figures = [];
  for (const id of Object.keys(expectedByRow)) {
    const row = states.summary.rows.find((r) => r.id === id);
    figures.push(checkFigure(`deposit cost row "${id}"`, expectedByRow[id], row?.text ?? null, { currency: 'ICP' }));
  }
  figures.push(checkFigure('deposit button amount', DEPOSIT_E8S, states.summary.button, { currency: 'ICP' }));
  log(`  deposit summary: ${states.summary.rows.map((r) => `${r.id}=${r.text}`).join(', ')}; button "${states.summary.button}"; notices ${states.summary.noticesOnScreen}; solvency above button: ${geometry.ok}`);

  // Press Deposit: the stepper mid-flight, then the receipt.
  await page.click('.modal-content .actions .btn-primary');
  await page.waitForTimeout(450);
  states.steps = {
    file: await shoot(page, outDir, 'deposit-steps', vp),
    statuses: await stepStatuses(page),
  };
  if (!states.steps.statuses.includes('active') && !states.steps.statuses.every((s) => s === 'done')) {
    problems.push(`deposit: no active step was painted mid-flight (${JSON.stringify(states.steps.statuses)})`);
  }
  const receiptUp = await page.waitForSelector('.cashier-receipt', { timeout: 60_000 }).then(() => true).catch(() => false);
  if (!receiptUp) {
    const err = await textOf(page, '.modal-content .alert.error');
    problems.push(`deposit: no receipt within 60 s (error on screen: ${JSON.stringify(err)})`);
    states.receipt = { file: await shoot(page, outDir, 'deposit-receipt', vp), error: err };
    return { ok: false, problems, figures: foldFigures(figures), ...states };
  }
  await page.waitForTimeout(300);
  const escrowAfter = BigInt(await table.get_balance());
  const ledgerAfter = BigInt(await ledgerBalance(hero));
  states.receipt = {
    file: await shoot(page, outDir, 'deposit-receipt', vp),
    rows: await rowsOf(page, '.cashier-receipt [data-receipt-row]', 'data-receipt-row'),
    statuses: await stepStatuses(page),
    ledgerDeltaE8s: (ledgerBefore - ledgerAfter).toString(),
    escrowDeltaE8s: (escrowAfter - escrowBefore).toString(),
  };
  const rrow = (id) => states.receipt.rows.find((r) => r.id === id)?.text ?? null;
  figures.push(checkFigure('deposit receipt "sent"', DEPOSIT_E8S, rrow('sent'), { currency: 'ICP' }));
  figures.push(checkFigure('deposit receipt "fees"', fee * 2n, rrow('fees'), { currency: 'ICP' }));
  figures.push(checkFigure('deposit receipt "balance" vs get_balance()', escrowAfter, rrow('balance'), { currency: 'ICP' }));
  // THE CLAIM ON THE LEDGER: the wallet paid the amount plus two fees, the
  // table credited the amount whole.
  if (ledgerBefore - ledgerAfter !== DEPOSIT_E8S + fee * 2n) {
    problems.push(`deposit: the ledger fell by ${ledgerBefore - ledgerAfter} e8s, not the amount plus two fees (${DEPOSIT_E8S + fee * 2n})`);
  }
  if (escrowAfter - escrowBefore !== DEPOSIT_E8S) {
    problems.push(`deposit: the escrow rose by ${escrowAfter - escrowBefore} e8s, not the amount (${DEPOSIT_E8S})`);
  }
  if (!states.receipt.statuses.every((s) => s === 'done')) {
    problems.push(`deposit: the stepper is not all done on the receipt (${JSON.stringify(states.receipt.statuses)})`);
  }
  log(`  deposit receipt: ${states.receipt.rows.map((r) => `${r.id}=${r.text}`).join(', ')}; ledger -${states.receipt.ledgerDeltaE8s} e8s, escrow +${states.receipt.escrowDeltaE8s} e8s`);
  await closeReceipt(page);

  const folded = foldFigures(figures);
  if (!folded.ok) problems.push(...folded.mismatches.map((m) => `deposit figure: ${m}`));
  return { ok: problems.length === 0, problems, figures: folded, ...states };
}

// ---------------------------------------------------------------------------
// 2. THE WITHDRAWAL
// ---------------------------------------------------------------------------
async function probeWithdraw(page, vp, ctx, outDir, tableId) {
  const problems = [];
  const hero = devPlayerPrincipal(HERO_PLAYER);
  const table = await tableActorFor(HERO_PLAYER, tableId);
  const fee = BigInt(await ledgerTransferFee());
  const escrowBefore = BigInt(await table.get_balance());
  const ledgerBefore = BigInt(await ledgerBalance(hero));

  await openCashier(page, 'withdraw');
  const states = {};
  const figures = [];
  states.rest = {
    file: await shoot(page, outDir, 'withdraw', vp),
    balance: await textOf(page, '.modal-content .balance-info .amount'),
    destination: await textOf(page, '.modal-content .dest-id'),
    noticesOnScreen: await noticesUnder(page, 'the withdraw sheet at rest', problems),
    touch: await touchUnder(page, vp, 'withdraw', problems),
  };
  figures.push(checkFigure('withdraw balance vs get_balance()', escrowBefore, states.rest.balance, { currency: 'ICP' }));
  const expectedDestination = defaultAccountHex(hero);
  if ((states.rest.destination || '').toLowerCase() !== expectedDestination) {
    problems.push(`withdraw: the destination shown (${states.rest.destination}) is not the hero principal's default account (${expectedDestination})`);
  }
  log(`  withdraw at rest: balance "${states.rest.balance}", destination ${states.rest.destination === expectedDestination ? 'is the derived account id' : 'DIFFERS'}, notices ${states.rest.noticesOnScreen}`);

  await page.fill('#withdraw-amount', WITHDRAW_TYPED);
  await page.waitForTimeout(300);
  await settle(page);
  states.summary = {
    file: await shoot(page, outDir, 'withdraw-summary', vp),
    rows: await rowsOf(page, '.modal-content .cost-summary [data-row]', 'data-row'),
    button: await textOf(page, '.modal-content .actions .btn-primary'),
    touch: await touchUnder(page, vp, 'withdraw-summary', problems),
  };
  const expectedByRow = { withdraw: WITHDRAW_E8S, fee, net: WITHDRAW_E8S - fee };
  for (const id of Object.keys(expectedByRow)) {
    const row = states.summary.rows.find((r) => r.id === id);
    figures.push(checkFigure(`withdraw row "${id}"`, expectedByRow[id], row?.text ?? null, { currency: 'ICP' }));
  }
  figures.push(checkFigure('withdraw button amount', WITHDRAW_E8S, states.summary.button, { currency: 'ICP' }));
  log(`  withdraw summary: ${states.summary.rows.map((r) => `${r.id}=${r.text}`).join(', ')}; button "${states.summary.button}"`);

  // Press Withdraw. Inside the canister's cooldown from the previous
  // viewport's withdrawal the call is refused: that refusal must read as the
  // humane sentence, and the probe waits it out and presses again.
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await page.click('.modal-content .actions .btn-primary');
    await page.waitForTimeout(450);
    const mid = await shoot(page, outDir, attempt === 0 ? 'withdraw-steps' : `withdraw-steps-${attempt}`, vp);
    const midStatuses = await stepStatuses(page);
    const outcome = await Promise.race([
      page.waitForSelector('.cashier-receipt', { timeout: 60_000 }).then(() => 'receipt'),
      page.waitForSelector('.modal-content .alert.error', { timeout: 60_000 }).then(() => 'error'),
    ]).catch(() => 'timeout');
    if (outcome === 'receipt') {
      states.steps = { file: mid, statuses: midStatuses };
      break;
    }
    if (outcome === 'error') {
      const message = await textOf(page, '.modal-content .alert.error .alert-text');
      const detail = await textOf(page, '.modal-content .alert.error .alert-detail');
      const m = /Try again in (\d+) s/.exec(message || '');
      states.cooldown = {
        file: await shoot(page, outDir, 'withdraw-cooldown', vp),
        message,
        detail,
        humane: /^You withdrew less than a minute ago\. Try again in \d+ s\. Nothing moved\./.test(message || ''),
      };
      if (!m) {
        problems.push(`withdraw: refused with a sentence that is not the cooldown (${JSON.stringify(message)})`);
        return { ok: false, problems, figures: foldFigures(figures), ...states };
      }
      if (!states.cooldown.humane) problems.push(`withdraw: the cooldown refusal is not the humane sentence (${JSON.stringify(message)})`);
      if (!detail) problems.push('withdraw: the cooldown refusal carries no detail line with the canister\'s own words');
      log(`  withdraw refused inside the cooldown: "${message}"; waiting ${m[1]} s`);
      await sleep((Number(m[1]) + 2) * 1000);
      continue;
    }
    problems.push('withdraw: neither a receipt nor a refusal within 60 s');
    return { ok: false, problems, figures: foldFigures(figures), ...states };
  }
  if (!states.steps) {
    problems.push('withdraw: no receipt after three attempts');
    return { ok: false, problems, figures: foldFigures(figures), ...states };
  }
  await page.waitForTimeout(300);
  const escrowAfter = BigInt(await table.get_balance());
  const ledgerAfter = BigInt(await ledgerBalance(hero));
  states.receipt = {
    file: await shoot(page, outDir, 'withdraw-receipt', vp),
    rows: await rowsOf(page, '.cashier-receipt [data-receipt-row]', 'data-receipt-row'),
    lead: await textOf(page, '.cashier-receipt .lead'),
    statuses: await stepStatuses(page),
    ledgerDeltaE8s: (ledgerAfter - ledgerBefore).toString(),
    escrowDeltaE8s: (escrowBefore - escrowAfter).toString(),
  };
  const rrow = (id) => states.receipt.rows.find((r) => r.id === id)?.text ?? null;
  figures.push(checkFigure('withdraw receipt "withdrawn"', WITHDRAW_E8S, rrow('withdrawn'), { currency: 'ICP' }));
  figures.push(checkFigure('withdraw receipt "fee" vs icrc1_fee()', fee, rrow('fee'), { currency: 'ICP' }));
  figures.push(checkFigure('withdraw receipt "received"', WITHDRAW_E8S - fee, rrow('received'), { currency: 'ICP' }));
  if (!/^\d+$/.test(rrow('block') || '')) problems.push(`withdraw: the receipt's ledger block is not an index (${JSON.stringify(rrow('block'))})`);
  if (!/opens in \d:\d\d\.$/.test(states.receipt.lead || '')) problems.push(`withdraw: the receipt does not count the cooldown down (${JSON.stringify(states.receipt.lead)})`);
  if (ledgerAfter - ledgerBefore !== WITHDRAW_E8S - fee) {
    problems.push(`withdraw: the ledger rose by ${ledgerAfter - ledgerBefore} e8s, not the amount minus the fee (${WITHDRAW_E8S - fee})`);
  }
  if (escrowBefore - escrowAfter !== WITHDRAW_E8S) {
    problems.push(`withdraw: the escrow fell by ${escrowBefore - escrowAfter} e8s, not the amount (${WITHDRAW_E8S})`);
  }
  log(`  withdraw receipt: ${states.receipt.rows.map((r) => `${r.id}=${r.text}`).join(', ')}; "${states.receipt.lead}"; ledger +${states.receipt.ledgerDeltaE8s} e8s, escrow -${states.receipt.escrowDeltaE8s} e8s`);
  await closeReceipt(page);

  const folded = foldFigures(figures);
  if (!folded.ok) problems.push(...folded.mismatches.map((m) => `withdraw figure: ${m}`));
  return { ok: problems.length === 0, problems, figures: folded, ...states };
}

async function probeViewport(vp, ctx, browser, outDir) {
  const tableId = ctx.tableIds[TABLE];
  log(`\n[${vp.name}] the cashier on ${TABLE}`);
  // The hero's wallet and escrow, topped up through the real doors so a
  // deposit and a withdrawal can both happen; no seat, so nothing at the
  // table moves underneath the sheet.
  await emptyTable(ctx, TABLE);
  const funds = await ensureLedgerFunds(HERO_PLAYER, HERO_WALLET_FLOOR);
  const escrow = await ensureTableBalance(HERO_PLAYER, tableId, HERO_ESCROW_FLOOR);
  log(`  hero wallet ${funds.balance} e8s (topped ${funds.topped}), escrow ${escrow.balance} e8s (deposited ${escrow.deposited})`);

  const context = await newContext(browser, vp, { log });
  const page = await context.newPage();
  watchPage(page);
  const result = { viewport: vp.name, deposit: null, withdraw: null };
  try {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await settle(page);
    result.deposit = await probeDeposit(page, vp, ctx, outDir, tableId);
    result.withdraw = await probeWithdraw(page, vp, ctx, outDir, tableId);
  } finally {
    await page.close();
    await context.close();
  }
  const ok = result.deposit?.ok && result.withdraw?.ok;
  log(`  ${ok ? '✓' : '✗'} ${vp.name}: deposit ${result.deposit?.ok ? 'green' : 'RED'}, withdraw ${result.withdraw?.ok ? 'green' : 'RED'}`);
  for (const p of [...(result.deposit?.problems || []), ...(result.withdraw?.problems || [])]) log(`    ! ${p}`);
  return result;
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
  const report = path.join(outDir, 'PROBE-cashier.json');
  fs.writeFileSync(report, JSON.stringify(out, (_k, v) => (typeof v === 'bigint' ? v.toString() : v), 2));
  log(`\nreport: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => !(r.deposit?.ok && r.withdraw?.ok));
  if (bad.length) {
    log(`FAILED at: ${bad.map((b) => b.viewport).join(', ')}`);
    return 1;
  }
  log('the cashier: every figure on the summary and the receipt equals what the ledger and the table did');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(process.env.SHOTS_DEBUG ? e : `FATAL: ${e.message}`);
  process.exit(2);
});
