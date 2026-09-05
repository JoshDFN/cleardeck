#!/usr/bin/env node
// PROBE: the heads-up ring at a showdown, measured.
//
// The `handreplay` scene stages table_1 (Heads Up, a 2-seat ring) and the
// occlusion gate runs on the LIVE table behind its dialog. The wave-6 round-2
// final sweep measured the winner readout ("Nakamoto wins 1.44 ICP") 100%
// covered by the top seat's revealed pair on that ring, the one ring no table
// scene photographs. This probe plays a completed hand on table_1, opens the
// table at both viewports, and measures the winner readout against the top
// seat's shown pair and the pot module, then photographs it.
//
//   node tools/shots/probe-heads-up.mjs [--viewports desktop,mobile]
//
// Report only: it prints the boxes and the overlap and exits 1 when the pair
// and the readout intersect, so a fix can be measured rather than eyeballed.

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, VIEWPORTS, icp } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { devLogin, enterTable, launchBrowser, newContext, openApp, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { playCompletedHand, tableDisplayName } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);
const TABLE = 'table_1';

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

function measure(page) {
  return page.evaluate(() => {
    const box = (el) => {
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };
    };
    const intersects = (p, q) => !!(p && q && p.x < q.x + q.w && q.x < p.x + p.w && p.y < q.y + q.h && q.y < p.y + p.h);
    const overlap = (p, q) => {
      if (!intersects(p, q)) return 0;
      const w = Math.min(p.x + p.w, q.x + q.w) - Math.max(p.x, q.x);
      const h = Math.min(p.y + p.h, q.y + q.h) - Math.max(p.y, q.y);
      return +((w * h) / (p.w * p.h)).toFixed(3);
    };
    const readout = document.querySelector('.winner-display .winner-text');
    const display = document.querySelector('.winner-display');
    const pairs = [...document.querySelectorAll('.seat .player-cards.shown')].map((el) => ({
      seat: el.closest('.seat')?.className.replace(/svelte-\S+/g, '').trim(),
      box: box(el),
    }));
    const r = box(readout);
    return {
      readoutText: (readout?.textContent || '').trim(),
      readout: r,
      display: box(display),
      potModule: box(document.querySelector('.pot-display, .main-pot')),
      board: box(document.querySelector('.community-cards')),
      pairs: pairs.map((p) => ({ ...p, overReadout: overlap(r, p.box) })),
      felt: box(document.querySelector('.felt')),
    };
  });
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
  const browser = await launchBrowser({ log });
  let red = 0;
  try {
    for (const name of args.viewports) {
      const vp = VIEWPORTS[name];
      if (!vp) throw new Error(`unknown viewport ${name}`);
      log(`\n[${name}] one completed hand on ${TABLE} (heads-up)`);
      const { view } = await playCompletedHand(ctx, TABLE, { heroChips: icp(6), oppChips: icp(8) });
      const winners = view.last_hand_winners.map((w) => `seat ${Number(w.seat) + 1}: ${w.amount}`);
      const context = await newContext(browser, vp, { log });
      const page = await context.newPage();
      watchPage(page);
      try {
        await openApp(page);
        await devLogin(page, HERO_PLAYER);
        await enterTable(page, tableDisplayName(ctx, TABLE));
        await page.waitForSelector('.winner-display', { timeout: 60_000 });
        await settle(page);
        const m = await measure(page);
        const file = path.join(outDir, `PROBE-heads-up-${name}.png`);
        await page.screenshot({ path: file, fullPage: false });
        log(`  winners on chain: ${winners.join(', ')}`);
        log(`  readout "${m.readoutText}" at ${JSON.stringify(m.readout)}; display ${JSON.stringify(m.display)}`);
        log(`  pot module ${JSON.stringify(m.potModule)}; board ${JSON.stringify(m.board)}; felt ${JSON.stringify(m.felt)}`);
        for (const p of m.pairs) {
          log(`  shown pair [${p.seat}] at ${JSON.stringify(p.box)}: ${(p.overReadout * 100).toFixed(1)}% of the readout under it${p.overReadout ? '  <-- COLLISION' : ''}`);
          if (p.overReadout > 0) red += 1;
        }
        log(`  still: ${path.relative(process.cwd(), file)}`);
      } finally {
        await context.close();
      }
    }
  } finally {
    await browser.close();
  }
  log(red ? `\n${red} collision(s): the readout is under a shown pair` : '\nno collision: the readout is clear of every shown pair');
  process.exit(red ? 1 : 0);
}

main().catch((e) => { console.error(e); process.exit(2); });
