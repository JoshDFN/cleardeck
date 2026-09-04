#!/usr/bin/env node
// The decision loop's TRANSIENT states, photographed.
//
//   node tools/shots/probe-decision-loop.mjs [--viewports desktop,mobile]
//
// The gating harness (run.mjs) photographs RESTING states, and two states of
// the decision loop never rest long enough to be a scene:
//
//   1. THE PHONE SIZER, OPEN. Every phone raise passes through it, and when it
//      opened in flow it grew the dock ~130 px and shrank the felt under the
//      45% floor while the resting state was gated green. This probe opens it
//      and measures the felt box and every pod BEFORE and AFTER, so "opening
//      the sizer moves nothing" is a number, not a claim.
//
//   2. THE SENT ECHO. The optimistic state lasts ~200 ms on the local replica
//      and 1.35-1.75 s on mainnet. This probe HOLDS the update call at the
//      network layer for a mainnet-like interval (route delay; nothing is
//      mocked, the real call still lands) and photographs the felt mid-send:
//      the hero's plate tag, the chips at the bet spot, the quiet action row.
//      It then lets the call land and photographs the settled state.
//
// Not a gate. It writes PROBE-*.png beside the run's other PNGs and prints the
// measurements; a human (or the critic) reads them. LOCAL REPLICA ONLY, like
// run.mjs (lib/ids.mjs refuses mainnet).

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER, REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { launchBrowser, newContext, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { measureFelt } from './lib/felt-area.mjs';
import { scenesByName } from './scenarios/index.mjs';

const log = (msg) => console.log(msg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** How long the update call is held, so the echo is on screen when the still is taken. */
const HOLD_UPDATE_MS = Number(process.env.SHOTS_PROBE_HOLD_MS || 1500);
/** When, after the click, the mid-send still is taken. */
const ECHO_STILL_AT_MS = 350;

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

/** The boxes that must not move when the sizer opens: the felt and every pod. */
function measureLayout(page) {
  return page.evaluate(() => {
    const box = (el) => {
      const r = el.getBoundingClientRect();
      return { x: +r.x.toFixed(1), y: +r.y.toFixed(1), w: +r.width.toFixed(1), h: +r.height.toFixed(1) };
    };
    const felt = document.querySelector('.felt');
    return {
      felt: felt ? box(felt) : null,
      pods: [...document.querySelectorAll('.player-nameplate')].map(box),
      pot: (() => { const el = document.querySelector('.pot-amount'); return el ? box(el) : null; })(),
      sizer: (() => {
        const el = document.querySelector('.raise-slider-panel');
        return el && !el.hidden ? box(el) : null;
      })(),
      actionRow: (() => { const el = document.querySelector('.actions'); return el ? box(el) : null; })(),
    };
  });
}

function sameBoxes(a, b) {
  const same = (p, q) => p && q && p.x === q.x && p.y === q.y && p.w === q.w && p.h === q.h;
  if (!same(a.felt, b.felt)) return false;
  if (a.pods.length !== b.pods.length) return false;
  return a.pods.every((p, i) => same(p, b.pods[i]));
}

/** What the felt and the dock say while the send is open. */
function readEchoState(page) {
  return page.evaluate(() => {
    const txt = (sel) => {
      const el = document.querySelector(sel);
      return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null;
    };
    const tag = document.querySelector('.player-nameplate.highlight-me .plate-tag');
    return {
      plateTag: tag ? (tag.textContent || '').trim() : null,
      // Svelte scopes a hash class onto the element; only the tone words matter.
      plateTagTone: tag ? [...tag.classList].filter((c) => c === 'sent' || c === 'armed').join(' ') : null,
      plateTagSentE8s: tag ? tag.getAttribute('data-sent-e8s') : null,
      rowSent: !!document.querySelector('.actions.sent'),
      pressedButton: txt('.actions .action-btn.pressed'),
      spinner: !!document.querySelector('.actions .spinner'),
      turnIndicator: txt('.turn-indicator'),
      heroStack: txt('.player-nameplate.highlight-me .chips'),
      heroBet: txt('.seat.is-me .bet-amount, .seat.hero .bet-amount'),
      clock: txt('.turn-timer'),
    };
  });
}

async function probeViewport(vp, ctx, browser, outDir) {
  const scene = scenesByName(['table-facing-bet'])[0];
  const results = { viewport: vp.name };

  // ---- 1. the sizer, opened (phone) --------------------------------------
  log(`\n[${vp.name}] staging table-facing-bet`);
  const setup = await scene.setup(ctx);
  results.onChain = setup.onChain;
  let context = await newContext(browser, vp, { log });
  let page = await context.newPage();
  watchPage(page);
  try {
    await scene.stage(ctx, page);
    await settle(page);
    const before = await measureLayout(page);
    const caret = page.locator('.actions .action-btn.caret');
    if (await caret.count()) {
      await caret.first().click();
      await page.waitForSelector('.raise-slider-panel:not([hidden])', { timeout: 5000 });
      await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
      await settle(page);
      const after = await measureLayout(page);
      const file = path.join(outDir, `PROBE-sizer-open-${vp.name}.png`);
      await page.screenshot({ path: file, fullPage: false });
      const unmoved = sameBoxes(before, after);
      results.sizerOpen = {
        file: path.relative(REPO_ROOT, file),
        feltBefore: before.felt, feltAfter: after.felt,
        podsMoved: !unmoved,
        sizerBox: after.sizer, actionRow: after.actionRow,
        sizerAboveActionRow: !!(after.sizer && after.actionRow && after.sizer.y + after.sizer.h <= after.actionRow.y + 0.5),
        sizerInsideViewport: !!(after.sizer && after.sizer.y >= 0 && after.sizer.x >= 0
          && after.sizer.x + after.sizer.w <= vp.width + 0.5),
      };
      log(`  sizer open: felt ${JSON.stringify(before.felt)} -> ${JSON.stringify(after.felt)}; `
        + `pods moved: ${!unmoved}; sizer ${JSON.stringify(after.sizer)} above row at y ${after.actionRow?.y}`);
      // Close it again so the echo probe starts from rest.
      await page.locator('.actions .action-btn.caret').first().click();
      await page.waitForSelector('.raise-slider-panel[hidden]', { timeout: 5000 }).catch(() => {});
    } else {
      results.sizerOpen = { skipped: 'no caret at this viewport (the sizer is always open)' };
      log('  no caret at this viewport: the sizer is in the dock at rest');
    }

    // ---- 2. the echo, with the update held --------------------------------
    // The update call (player_action) travels as POST .../call. Holding it at
    // the network layer keeps the echo on screen for a mainnet-like interval.
    // Queries (.../query, .../read_state) are not delayed, so the poll keeps
    // running against the pre-click state exactly as a slow mainnet would.
    await page.route(/\/api\/v[0-9]+\/canister\/[^/]+\/call$/, async (route) => {
      await sleep(HOLD_UPDATE_MS);
      await route.continue();
    });
    const callBtn = page.locator('.actions:not(.disabled) .action-btn', { hasText: /^\s*Call\s+\d/i });
    if (!(await callBtn.count())) throw new Error('no live Call button to probe the echo with');
    const callLabel = (await callBtn.first().textContent()).trim();
    const t0 = Date.now();
    await callBtn.first().click();
    await sleep(ECHO_STILL_AT_MS);
    const mid = await readEchoState(page);
    const midFile = path.join(outDir, `PROBE-echo-mid-send-${vp.name}.png`);
    await page.screenshot({ path: midFile, fullPage: false });
    const midAt = Date.now() - t0;
    log(`  echo at +${midAt} ms: tag "${mid.plateTag}" (${mid.plateTagTone}, sent ${mid.plateTagSentE8s} e8s), `
      + `row sent ${mid.rowSent}, pressed "${mid.pressedButton}", spinner ${mid.spinner}, stack ${mid.heroStack}, clock ${mid.clock}`);

    // Let the held call land and the felt settle on the certified view.
    await page.waitForSelector('.actions.sent', { state: 'detached', timeout: HOLD_UPDATE_MS + 15_000 });
    await sleep(600);
    await settle(page);
    const after = await readEchoState(page);
    const afterFile = path.join(outDir, `PROBE-echo-settled-${vp.name}.png`);
    await page.screenshot({ path: afterFile, fullPage: false });
    log(`  settled: tag "${after.plateTag}", row sent ${after.rowSent}, stack ${after.heroStack}, turn "${after.turnIndicator}"`);
    results.echo = {
      clicked: callLabel,
      holdMs: HOLD_UPDATE_MS,
      midSendAtMs: midAt,
      midSend: mid,
      midSendFile: path.relative(REPO_ROOT, midFile),
      settled: after,
      settledFile: path.relative(REPO_ROOT, afterFile),
      echoPainted: mid.plateTagTone === 'sent' && mid.rowSent && !mid.spinner,
      echoReconciled: !after.rowSent && after.plateTagTone !== 'sent',
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
  const report = path.join(outDir, 'PROBE-decision-loop.json');
  // The scene's on-chain facts are BigInt (Candid nat); JSON needs strings.
  const bigintSafe = (_k, v) => (typeof v === 'bigint' ? v.toString() : v);
  fs.writeFileSync(report, JSON.stringify({ heroPlayer: HERO_PLAYER, holdMs: HOLD_UPDATE_MS, results: out }, bigintSafe, 2));
  log(`\nprobe report: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => (r.sizerOpen && r.sizerOpen.podsMoved) || (r.echo && !(r.echo.echoPainted && r.echo.echoReconciled)));
  if (bad.length) { log(`PROBE FOUND PROBLEMS in: ${bad.map((b) => b.viewport).join(', ')}`); return 1; }
  log('probe: the sizer opens without moving the felt; the echo paints and reconciles');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(`\nFATAL: ${e.message}`);
  if (process.env.SHOTS_DEBUG) console.error(e.stack);
  process.exit(2);
});
