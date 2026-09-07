#!/usr/bin/env node
// PROBE: the 9-max ring at a showdown with the deck seal, and a four-seat replay.
//
// Two things no gating scene photographs:
//   1. the crowded (9-max) PHONE at a showdown with the deck seal on screen.
//      The seal moved from the felt's near corner (where the hero's award chip
//      lands on a phone) to the dock row in portrait; this measures the seal's
//      box against every money figure, percentage and card on the table.
//   2. a replay with MORE than two seats. Every gating replay is heads-up, and
//      the handoff expected the mini table's flank plates to overhang the
//      dialog column by half a plate on a 6-max ring; this seats the four dev
//      players on table_3, plays a four-way all-in to a showdown, opens the
//      hand's replay at both viewports on the Showdown stop and measures every
//      pod's box against the replayer's own column.
//
//   node tools/shots/probe-nine-max.mjs [--viewports desktop,mobile]
//
// Report only: prints the boxes, writes the stills under artifacts/screens/<sha>/probe/,
// exits 1 on a seal collision or a pod outside the replayer's box.

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, VIEWPORTS, icp } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { devLogin, enterTable, launchBrowser, newContext, openApp, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { doAct, phaseOf, playUntil, sitOutAfterHand, startHand } from './lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);
const TABLE = 'table_3'; // 9-max
const SEATS = [
  { player: HERO_PLAYER, seat: 0, chips: icp(20) },
  { player: 2, seat: 2, chips: icp(45) },
  { player: 3, seat: 5, chips: icp(70) },
  { player: 4, seat: 7, chips: icp(95) },
];

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

/** Four players all in: a multi-way showdown with every hand on its face. */
async function playFourWayShowdown(ctx) {
  const { tableId } = await prepareTable(ctx, TABLE, SEATS);
  await startHand(2, tableId);
  await sitOutAfterHand(SEATS.slice(1).map((s) => s.player), tableId);
  const plan = {};
  for (const s of SEATS) plan[s.seat] = { player: s.player, act: (t) => doAct.allIn(t) };
  const view = await playUntil(
    tableId, plan,
    (s) => phaseOf(s) === 'HandComplete' && s.last_hand_winners.length > 0,
    'a four-way all-in to a showdown',
    { timeoutMs: 120_000 },
  );
  return { tableId, view };
}

/** The deck seal's box against every figure on the table. */
function measureTable(page) {
  return page.evaluate(() => {
    const box = (el) => {
      const r = el.getBoundingClientRect();
      return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };
    };
    const hit = (p, q) => {
      const w = Math.min(p.x + p.w, q.x + q.w) - Math.max(p.x, q.x);
      const h = Math.min(p.y + p.h, q.y + q.h) - Math.max(p.y, q.y);
      return w > 0.5 && h > 0.5 ? +((w * h) / (q.w * q.h)).toFixed(3) : 0;
    };
    const seal = document.querySelector('.deck-seal');
    if (!seal) return { seal: null, collisions: [], figures: 0 };
    const s = box(seal);
    const money = /\d[\d,]*\.\d/;
    const pct = /\d(?:[.,]\d+)?\s*%/;
    const collisions = [];
    let figures = 0;
    for (const el of document.querySelectorAll('.poker-table *')) {
      if (seal.contains(el) || el.children.length) continue;
      const text = (el.textContent || '').trim();
      const card = el.classList.contains('rank') || el.classList.contains('pip');
      if (!card && !money.test(text) && !pct.test(text)) continue;
      const b = box(el);
      if (!b.w || !b.h) continue;
      figures += 1;
      const f = hit(s, b);
      if (f > 0) collisions.push({ text, cls: String(el.className).replace(/svelte-\S+/g, '').trim(), box: b, covered: f });
    }
    return {
      seal: { box: s, state: seal.getAttribute('data-seal'), variant: [...seal.classList].find((c) => c === 'in-dock' || c === 'on-felt') },
      figures,
      collisions,
      felt: (() => { const f = document.querySelector('.felt'); return f ? box(f) : null; })(),
      seats: document.querySelectorAll('.seat').length,
      ring: [...(document.querySelector('.poker-table-wrapper')?.classList || [])].filter((c) => c.startsWith('ring-')).join(' ') || 'default',
    };
  });
}

/** Every pod on the mini table against the replayer's own box. */
function measureReplay(page) {
  return page.evaluate(() => {
    const box = (el) => {
      const r = el.getBoundingClientRect();
      return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };
    };
    const replayer = document.querySelector('.replayer');
    const scene = document.querySelector('.replayer .scene');
    const modal = document.querySelector('.modal-content');
    if (!replayer || !scene) return null;
    const col = box(replayer);
    const pods = [...document.querySelectorAll('.replayer .player-row')].map((el) => {
      const b = box(el.querySelector('.plate') || el);
      const overhangLeft = +(col.x - b.x).toFixed(1);
      const overhangRight = +((b.x + b.w) - (col.x + col.w)).toFixed(1);
      return {
        seat: (el.querySelector('.seat-tag')?.textContent || '').replace(/\s+/g, ' ').trim(),
        side: [...el.classList].find((c) => c.startsWith('side-')),
        box: b,
        overhang: Math.max(0, overhangLeft, overhangRight),
        equity: (el.querySelector('.replay-equity')?.textContent || '').trim() || null,
      };
    });
    return {
      column: col, scene: box(scene), modal: modal ? box(modal) : null,
      felt: (() => { const f = document.querySelector('.replayer .felt'); return f ? box(f) : null; })(),
      boardCard: (() => { const c = document.querySelector('.replayer .board .card'); return c ? box(c).w : null; })(),
      pods,
      lineCells: document.querySelectorAll('.equity-line .replay-equity').length,
      lineRows: document.querySelectorAll('.equity-line .eq-row').length,
      stop: (document.querySelector('.transport-label')?.textContent || '').trim(),
    };
  });
}

async function openReplayOfHand(page, handNumber) {
  await page.waitForSelector('.history-btn', { timeout: 30_000 });
  await page.click('.history-btn');
  await page.waitForSelector('.hand-row', { timeout: 30_000 });
  const row = page.locator('.hand-row', { has: page.locator('.hand-number', { hasText: `Hand #${handNumber}` }) }).first();
  if (!(await row.count())) throw new Error(`no list row for Hand #${handNumber}`);
  await row.click();
  await page.waitForSelector('.replayer', { timeout: 20_000 });
  await page.locator('.scrubber .stop').last().click();
  await settle(page, { extraFrames: 2 });
  // the resting frame: the replayer at its top, the felt first
  await page.evaluate(() => document.querySelector('.replayer')?.scrollTo(0, 0));
  await settle(page, { extraFrames: 1 });
}

async function main() {
  const args = parseArgs(process.argv);
  await requireReplica();
  const ids = readLocalIds();
  const frontendId = requireId(ids, 'frontend');
  setAppOrigin(`http://${frontendId}.${GATEWAY_HOST}:${GATEWAY_PORT}`);
  const ctx = {
    ids,
    tableIds: { table_1: ids.table_1, table_2: ids.table_2, table_3: ids.table_3, btc_table_1: ids.btc_table_1 },
    tableNames: await resolveTableNames(ids),
  };
  const { shaDir } = runDirs(gitShortSha());
  const outDir = path.join(shaDir, 'probe');
  fs.mkdirSync(outDir, { recursive: true });

  log(`\none four-way all-in to a showdown on ${TABLE} (9-max ring, ${SEATS.length} seats)`);
  const { view } = await playFourWayShowdown(ctx);
  const handNumber = Number(view.hand_number);
  const winners = view.last_hand_winners.map((w) => `seat ${Number(w.seat) + 1}: ${w.amount}`);
  log(`  hand #${handNumber}, winners on chain: ${winners.join(', ')}`);

  const browser = await launchBrowser({ log });
  let red = 0;
  try {
    for (const name of args.viewports) {
      const vp = VIEWPORTS[name];
      if (!vp) throw new Error(`unknown viewport ${name}`);
      log(`\n[${name}]`);
      const context = await newContext(browser, vp, { log });
      const page = await context.newPage();
      watchPage(page);
      try {
        await openApp(page);
        await devLogin(page, HERO_PLAYER);
        await enterTable(page, tableDisplayName(ctx, TABLE));
        await page.waitForSelector('.winner-display', { timeout: 60_000 });
        await page.waitForSelector('.deck-seal[data-seal="checked"]', { timeout: 30_000 }).catch(() => {});
        await settle(page);
        const t = await measureTable(page);
        const tableFile = path.join(outDir, `PROBE-nine-max-table-${name}.png`);
        await page.screenshot({ path: tableFile, fullPage: false });
        log(`  table: ${t.seats} seats on the ${t.ring} ring, felt ${JSON.stringify(t.felt)}`);
        log(`  deck seal ${t.seal ? `${t.seal.variant} ${t.seal.state} at ${JSON.stringify(t.seal.box)}` : 'NOT ON SCREEN'}; ${t.figures} figures measured`);
        for (const c of t.collisions) {
          log(`    COLLISION: "${c.text}" (${c.cls}) at ${JSON.stringify(c.box)}, ${(c.covered * 100).toFixed(1)}% under the seal`);
          red += 1;
        }
        if (!t.seal) red += 1;
        if (!t.collisions.length && t.seal) log('    the seal touches no figure');
        log(`  still: ${path.relative(process.cwd(), tableFile)}`);

        await openReplayOfHand(page, handNumber);
        const r = await measureReplay(page);
        const replayFile = path.join(outDir, `PROBE-nine-max-replay-${name}.png`);
        await page.screenshot({ path: replayFile, fullPage: false });
        if (!r) { log('  NO REPLAYER'); red += 1; continue; }
        log(`  replay at "${r.stop}": column ${JSON.stringify(r.column)}, scene ${JSON.stringify(r.scene)}, felt ${JSON.stringify(r.felt)}, board card ${r.boardCard} px`);
        log(`  equity line: ${r.lineRows} rows, ${r.lineCells} cells`);
        for (const p of r.pods) {
          const flag = p.overhang > 0 ? `  <-- OVERHANG ${p.overhang} px outside the replayer` : '';
          log(`    pod "${p.seat}" (${p.side}) at ${JSON.stringify(p.box)} equity ${p.equity ?? 'none'}${flag}`);
          if (p.overhang > 0) red += 1;
        }
        log(`  still: ${path.relative(process.cwd(), replayFile)}`);
      } finally {
        await context.close();
      }
    }
  } finally {
    await browser.close();
  }
  log(red ? `\n${red} finding(s): see COLLISION / OVERHANG above` : '\nclean: the seal touches no figure and every replay pod is inside the replayer');
  process.exit(red ? 1 : 0);
}

main().catch((e) => { console.error(e); process.exit(2); });
