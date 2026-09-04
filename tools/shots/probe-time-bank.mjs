#!/usr/bin/env node
// The time bank pill, photographed and pressed.
//
//   node tools/shots/probe-time-bank.mjs [--viewports desktop,mobile]
//
// The pill (TimeBankPill.svelte) exists only in the last fifteen seconds of
// the hero's own turn, so no resting scene photographs it deterministically:
// the facing-bet still lands at whatever second the preset clicks leave. This
// probe stages that scene, WAITS for the pill, measures it (on desktop under
// the turn indicator, on the phone on the hero's pod clock, never in the
// action row), checks the phone's hit area really is the touch floor (the
// visible pill is the plate row's height; a pseudo-element grows the target),
// checks the action row is still four cells, photographs it, presses it, and
// confirms the canister put the clock on the bank (the turn indicator takes
// `.time-bank`, the digits go up).
//
// Not a gate. Writes PROBE-time-bank-*.png beside the run's other PNGs.
// LOCAL REPLICA ONLY, like run.mjs (lib/ids.mjs refuses mainnet).

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { launchBrowser, newContext, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { scenesByName } from './scenarios/index.mjs';

const log = (msg) => console.log(msg);

/** The pill is offered at 15 s; the scene's clock starts at 45 s. */
const PILL_WAIT_MS = Number(process.env.SHOTS_TIME_BANK_WAIT_MS || 50_000);
const TOUCH_MIN = 44;

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

/** Where the pill is, what surrounds it, and whether a thumb can hit it. */
function measurePill(page) {
  return page.evaluate((touchMin) => {
    const box = (el) => {
      const r = el.getBoundingClientRect();
      return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };
    };
    const pill = document.querySelector('.time-bank-pill');
    if (!pill) return null;
    const b = box(pill);
    const cx = b.x + b.w / 2;
    const cy = b.y + b.h / 2;
    // The hit area: every point at half the touch floor from the centre must
    // land on the pill (the pseudo-element is part of the button for hit
    // testing), or the target is smaller than a thumb.
    const half = touchMin / 2 - 1;
    const probes = [[cx, cy], [cx - half, cy], [cx + half, cy], [cx, cy - half], [cx, cy + half]];
    const hits = probes.map(([x, y]) => {
      const el = document.elementFromPoint(x, y);
      return !!el && (el === pill || pill.contains(el));
    });
    // The pill must not stand on the clock digits it sits beside.
    const timer = document.querySelector('.player-nameplate.highlight-me .turn-timer');
    const timerBox = timer && timer.getClientRects().length ? box(timer) : null;
    const intersects = (p, q) => p && q && p.x < q.x + q.w && q.x < p.x + p.w && p.y < q.y + q.h && q.y < p.y + p.h;
    const rowButtons = [...document.querySelectorAll('.actions .action-btn')].filter((el) => el.getClientRects().length);
    const inActionRow = !!pill.closest('.actions');
    const onHeroPlate = !!pill.closest('.player-nameplate.highlight-me');
    const underTurnIndicator = !!pill.closest('.turn-cell');
    return {
      text: (pill.textContent || '').replace(/\s+/g, ' ').trim(),
      box: b,
      timerBox,
      overClockDigits: !!intersects(b, timerBox),
      hitsAtTouchFloor: hits.every(Boolean),
      hitProbes: hits,
      inActionRow,
      onHeroPlate,
      underTurnIndicator,
      actionRowCells: rowButtons.length,
      actionRowLabels: rowButtons.map((el) => (el.textContent || '').replace(/\s+/g, ' ').trim()),
      clock: (document.querySelector('.player-nameplate.highlight-me .turn-timer')?.textContent || '').trim() || null,
      turnIndicatorOnBank: !!document.querySelector('.turn-indicator.time-bank'),
    };
  }, TOUCH_MIN);
}

async function probeViewport(vp, ctx, browser, outDir) {
  const scene = scenesByName(['table-facing-bet'])[0];
  if (!scene) throw new Error('scene table-facing-bet is not registered');
  const results = { viewport: vp.name };
  log(`\n[${vp.name}] staging table-facing-bet, then waiting for the last fifteen seconds`);
  await scene.setup(ctx);
  const context = await newContext(browser, vp, { log });
  const page = await context.newPage();
  watchPage(page);
  try {
    await scene.stage(ctx, page);
    await page.waitForSelector('.time-bank-pill', { timeout: PILL_WAIT_MS });
    await settle(page);
    const before = await measurePill(page);
    const file = path.join(outDir, `PROBE-time-bank-${vp.name}.png`);
    await page.screenshot({ path: file, fullPage: false });
    log(`  pill "${before.text}" at ${JSON.stringify(before.box)}; `
      + `${before.onHeroPlate ? 'on the hero plate' : before.underTurnIndicator ? 'under the turn indicator' : 'ELSEWHERE'}; `
      + `in the action row: ${before.inActionRow}; row cells ${before.actionRowCells} [${before.actionRowLabels.join(' | ')}]; `
      + `hit area at the touch floor: ${before.hitsAtTouchFloor} (${before.hitProbes.map((h) => (h ? 'y' : 'n')).join('')}); clock ${before.clock}; `
      + `over the clock digits: ${before.overClockDigits}`);

    // Press it: the canister's use_time_bank moves the clock onto the bank.
    await page.locator('.time-bank-pill').first().click();
    await page.waitForSelector('.turn-indicator.time-bank, .player-nameplate.highlight-me .turn-timer', { timeout: 10_000 }).catch(() => {});
    await page.waitForTimeout(1500);
    await settle(page);
    const after = await measurePill(page).catch(() => null);
    const afterState = await page.evaluate(() => ({
      onBank: !!document.querySelector('.turn-indicator.time-bank'),
      clock: (document.querySelector('.player-nameplate.highlight-me .turn-timer')?.textContent || '').trim() || null,
      pillStillOffered: !!document.querySelector('.time-bank-pill'),
    }));
    const secsBefore = Number.parseInt(before.clock || '0', 10);
    const secsAfter = Number.parseInt(afterState.clock || '0', 10);
    const clockWentUp = secsAfter > secsBefore;
    log(`  pressed: clock ${before.clock} -> ${afterState.clock} (${clockWentUp ? 'went up' : 'did NOT go up'}), `
      + `turn indicator on the bank: ${afterState.onBank}, pill still offered: ${afterState.pillStillOffered}`);
    const afterFile = path.join(outDir, `PROBE-time-bank-pressed-${vp.name}.png`);
    await page.screenshot({ path: afterFile, fullPage: false });
    results.pill = {
      file: path.relative(REPO_ROOT, file),
      pressedFile: path.relative(REPO_ROOT, afterFile),
      before,
      after: { ...afterState, pill: after },
      placedRight: !before.inActionRow && (vp.name === 'mobile' ? before.onHeroPlate : before.underTurnIndicator),
      rowIsFourCells: before.actionRowCells === 4 || (before.actionRowCells === 5 && before.actionRowLabels.some((l) => /^Choose|^Close|^$/.test(l))),
      hitsAtTouchFloor: before.hitsAtTouchFloor,
      // The touch floor is the PHONE's rule; on desktop the pill is a
      // pointer target and its hit area is recorded, not required.
      hitsWhereRequired: vp.name === 'mobile' ? before.hitsAtTouchFloor : true,
      clearOfClockDigits: !before.overClockDigits,
      clockWentUp,
    };
  } finally {
    await page.close();
    await context.close();
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
  const report = path.join(outDir, 'PROBE-time-bank.json');
  const bigintSafe = (_k, v) => (typeof v === 'bigint' ? v.toString() : v);
  fs.writeFileSync(report, JSON.stringify({ heroPlayer: HERO_PLAYER, results: out }, bigintSafe, 2));
  log(`\nprobe report: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => !r.pill || !(r.pill.placedRight && r.pill.hitsWhereRequired && r.pill.rowIsFourCells
    && r.pill.clearOfClockDigits && r.pill.clockWentUp));
  if (bad.length) { log(`PROBE FOUND PROBLEMS in: ${bad.map((b) => b.viewport).join(', ')}`); return 1; }
  log('probe: the time bank pill stands off the action row, hits at the touch floor, and puts the clock on the bank');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(`\nFATAL: ${e.message}`);
  if (process.env.SHOTS_DEBUG) console.error(e.stack);
  process.exit(2);
});
