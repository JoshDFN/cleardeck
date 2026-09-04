#!/usr/bin/env node
// EVERY CONTROL UNDER A THUMB, EVERY FIGURE UNDER AN EYE: the phone ergonomics gate.
//
//   node tools/shots/touch-targets.mjs [--scenes a,b] [--viewports mobile,landscape]
//
// Stages the same scenes run.mjs stages (real canisters, real login, the same
// scenario modules), then measures the EFFECTIVE hit area of every interactive
// element on the page against the 44 px touch floor (lib/touch-targets.mjs:
// elementFromPoint at the centre and 21 px out along each axis, so a control
// that grows its target with a pseudo-element passes and a 44 px control
// covered by a neighbour fails), the type floors of the figures a player reads
// at arm's length, the document's width (no horizontal scroll), the scroll lock
// behind an open dialog, and the sideways-phone rotate prompt.
//
// Extra states the resting scenes do not photograph: on table-preflop the
// phone's bet sizer is opened (the caret) and its sheet measured; on the lobby
// the developer login menu is opened and measured.
//
// Exit 1 on any violation. Writes TOUCH-TARGETS.json and a PNG per scene, with
// every violation outlined in red, under artifacts/screens/<sha>/probe/.
//
// Not a run.mjs gate (it re-stages the scenes and takes minutes), but it is
// re-runnable by anyone and is the evidence the mobile phase's claims rest on.
// LOCAL REPLICA ONLY, like run.mjs (lib/ids.mjs refuses mainnet). Do not run it
// concurrently with run.mjs or the probes: they all drive table_2.

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import { launchBrowser, newContext, setAppOrigin, settle, watchPage } from './lib/browser.mjs';
import { probeProtectedNotices } from './lib/protected-notices.mjs';
import { raiseToast, removeToast, TOAST_MESSAGES } from './lib/toast-notices.mjs';
import { scenesByName } from './scenarios/index.mjs';
import { foldTouchTargets, measureTouchTargets, TOUCH_MIN_PX } from './lib/touch-targets.mjs';

const log = (msg) => console.log(msg);

const DEFAULT_SCENES = ['table-preflop', 'table-facing-bet', 'table-showdown', 'table-sidepots', 'lobby', 'deposit'];

/** The phone held sideways (iPhone 13 landscape, minus browser chrome). */
const LANDSCAPE = { name: 'landscape', width: 750, height: 342, deviceScaleFactor: 1, isMobile: true };

function parseArgs(argv) {
  const out = { scenes: DEFAULT_SCENES, viewports: ['mobile', 'landscape'] };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--scenes') out.scenes = String(argv[++i] || '').split(',').filter(Boolean);
    else if (a === '--viewports') out.viewports = String(argv[++i] || '').split(',').filter(Boolean);
    else throw new Error(`Unknown argument: ${a}`);
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

/** Outlines every failing control in red, photographs, then cleans up. */
async function photograph(page, verdict, file) {
  const bad = (verdict.measurement?.items || []).filter((i) => !i.ok).map((i) => i.box);
  await page.evaluate((boxes) => {
    const root = document.createElement('div');
    root.id = 'touch-target-overlay';
    root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:99999';
    for (const b of boxes) {
      const d = document.createElement('div');
      d.style.cssText = `position:absolute;left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px;`
        + 'outline:2px solid #ff3b30;outline-offset:1px;background:rgba(255,59,48,0.18)';
      root.appendChild(d);
    }
    document.body.appendChild(root);
  }, bad);
  await page.screenshot({ path: file, fullPage: false });
  await page.evaluate(() => document.getElementById('touch-target-overlay')?.remove());
}

/**
 * The extra states per scene: what to open after staging, and what to expect.
 * Each step returns a label; the measurement runs once per step.
 */
const EXTRA_STATES = {
  'table-preflop': [
    {
      label: 'sizer open',
      scope: '.raise-slider-panel',
      async open(page) {
        const caret = page.locator('.actions .action-btn.caret');
        if (!(await caret.count())) return false;
        await caret.first().click();
        await page.waitForSelector('.raise-slider-panel:not([hidden])', { timeout: 5000 });
        await settle(page);
        return true;
      },
      async close(page) {
        const close = page.locator('.raise-slider-panel .close-slider');
        if (await close.count()) await close.first().click().catch(() => {});
      },
    },
  ],
  lobby: [
    {
      label: 'dev login menu open',
      scope: '.dev-menu',
      async open(page) {
        const dev = page.locator('.wallet-btn.dev');
        if (!(await dev.count())) return false;
        await dev.first().click();
        await page.waitForSelector('.dev-menu', { timeout: 5000 });
        await settle(page);
        return true;
      },
      async close(page) {
        await page.locator('.wallet-btn.dev').first().click().catch(() => {});
      },
    },
  ],
};

async function measureState(page, { scene, viewportName, label, expect, scope = null }) {
  const m = await measureTouchTargets(page, { touchMin: TOUCH_MIN_PX, scope });
  const verdict = foldTouchTargets(m, { scene: `${scene}${label ? ` (${label})` : ''}`, viewport: viewportName, ...expect });
  return verdict;
}

async function runScene(scene, vp, ctx, browser, outDir, results) {
  const viewportName = vp.name;
  log(`\n[${viewportName}] ${scene.name}`);
  await scene.setup(ctx);
  const context = await newContext(browser, vp, { log });
  const page = await context.newPage();
  watchPage(page);
  const isTable = scene.name.startsWith('table-') || scene.name === 'deposit';
  const sideways = viewportName === 'landscape';
  try {
    await scene.stage(ctx, page);
    await settle(page);
    const expect = {
      expectDialogLock: true,
      // The rotate prompt shows on a coarse-pointer phone held sideways on the
      // TABLE view only; the lobby never shows it.
      expectRotatePrompt: sideways ? isTable : false,
      // The table view is one screen on a phone (routes/app-phone.scss): the
      // document may be no taller than the viewport. The lobby scrolls by
      // design (its rows are below the fold).
      expectNoPageScroll: isTable,
    };
    const verdicts = [];
    const first = await measureState(page, { scene: scene.name, viewportName, label: '', expect });
    verdicts.push(first);
    log(`  ${first.ok ? '✓' : '✗'} ${first.notes}`);
    for (const p of first.problems) log(`    ⚠ ${p}`);
    const file = path.join(outDir, `TOUCH-${scene.name}-${viewportName}.png`);
    await photograph(page, first, file);
    first.file = path.relative(REPO_ROOT, file);
    first.coarsePointer = first.measurement?.coarsePointer ?? null;

    // The protected notices under the sideways prompt: the prompt stands over
    // the table area only, so all five must still be on screen.
    if (sideways) {
      const notices = await probeProtectedNotices(page);
      const off = notices.filter((n) => !n.onScreen).map((n) => n.phrase);
      first.notices = { onScreen: notices.length - off.length, total: notices.length, off };
      if (off.length) {
        first.ok = false;
        first.problems.push(`protected notice(s) off screen under the rotate prompt: ${off.join(' | ')}`);
        log(`    ⚠ notices off screen: ${off.join(' | ')}`);
      } else {
        log(`  ▪ ${notices.length}/${notices.length} protected notices on screen with the prompt up`);
      }
    }

    // THE TOAST'S CLOSE CONTROL, measured. No resting scene raises a toast, so
    // the app's own error toast is raised the way lib/toast-notices.mjs raises
    // it for the notice gate (the app's own rule paints it, the real
    // `.toast-close` button included) and its control is measured at the
    // touch floor, then it is removed.
    if (!sideways) {
      const raised = await raiseToast(page, TOAST_MESSAGES[0].text).catch(() => null);
      if (raised && raised.toastRulesMatched.length) {
        try {
          const v = await measureState(page, { scene: scene.name, viewportName, label: 'toast up', expect, scope: '.toast' });
          log(`  ${v.ok ? '✓' : '✗'} toast up: ${v.notes}`);
          for (const p of v.problems) log(`    ⚠ ${p}`);
          const f = path.join(outDir, `TOUCH-${scene.name}-toast-up-${viewportName}.png`);
          await photograph(page, v, f);
          v.file = path.relative(REPO_ROOT, f);
          verdicts.push(v);
        } finally {
          await removeToast(page);
        }
      } else {
        log('    (could not raise the app\'s toast; its close control was not measured)');
      }
    }

    if (!sideways) {
      for (const extra of EXTRA_STATES[scene.name] || []) {
        const opened = await extra.open(page).catch((e) => { log(`    (could not open ${extra.label}: ${e.message})`); return false; });
        if (!opened) continue;
        const v = await measureState(page, { scene: scene.name, viewportName, label: extra.label, expect, scope: extra.scope ?? null });
        log(`  ${v.ok ? '✓' : '✗'} ${extra.label}: ${v.notes}`);
        for (const p of v.problems) log(`    ⚠ ${p}`);
        const f = path.join(outDir, `TOUCH-${scene.name}-${extra.label.replace(/\s+/g, '-')}-${viewportName}.png`);
        await photograph(page, v, f);
        v.file = path.relative(REPO_ROOT, f);
        verdicts.push(v);
        await extra.close(page);
      }
    }
    for (const v of verdicts) {
      results.push({
        scene: v.scene, viewport: v.viewport, ok: v.ok, notes: v.notes, problems: v.problems, file: v.file,
        coarsePointer: v.measurement?.coarsePointer ?? null,
        notices: v.notices ?? null,
        items: v.measurement?.items ?? [],
        type: v.measurement?.type ?? [],
        document: v.measurement
          ? { w: v.measurement.documentScrollWidth, h: v.measurement.documentScrollHeight, viewport: v.measurement.viewport }
          : null,
      });
    }
  } catch (e) {
    log(`  ✗ FAILED: ${String(e.message || e).split('\n')[0]}`);
    results.push({ scene: scene.name, viewport: viewportName, ok: false, problems: [String(e.message || e)], notes: 'staging failed', items: [], type: [] });
    try { await page.screenshot({ path: path.join(outDir, `TOUCH-FAILED-${scene.name}-${viewportName}.png`) }); } catch {}
  } finally {
    await page.close();
    await context.close();
  }
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
  const scenes = scenesByName(args.scenes);
  const missing = args.scenes.filter((n) => !scenes.some((s) => s.name === n));
  if (missing.length) throw new Error(`unknown scene(s): ${missing.join(', ')}`);

  const { shaDir } = runDirs(gitShortSha());
  const outDir = path.join(shaDir, 'probe');
  fs.mkdirSync(outDir, { recursive: true });

  const browser = await launchBrowser({ log });
  const results = [];
  try {
    for (const name of args.viewports) {
      const vp = name === 'landscape' ? LANDSCAPE : VIEWPORTS[name];
      if (!vp) throw new Error(`Unknown viewport ${name}`);
      // Sideways: the table and the lobby are enough to prove the prompt and
      // the notices; the dialogs are portrait-only surfaces to a phone.
      const list = name === 'landscape'
        ? scenes.filter((s) => ['table-preflop', 'lobby'].includes(s.name))
        : scenes;
      for (const scene of list) await runScene(scene, vp, ctx, browser, outDir, results);
    }
  } finally {
    await browser.close();
  }

  const report = path.join(outDir, 'TOUCH-TARGETS.json');
  fs.writeFileSync(report, JSON.stringify({ touchMinPx: TOUCH_MIN_PX, results }, null, 2));
  log(`\nreport: ${path.relative(REPO_ROOT, report)}`);
  const bad = results.filter((r) => !r.ok);
  log(`\n${results.length - bad.length}/${results.length} states clean`);
  if (bad.length) {
    log(`VIOLATIONS in: ${bad.map((b) => `${b.scene} @ ${b.viewport}`).join('; ')}`);
    return 1;
  }
  log(`every interactive element meets the ${TOUCH_MIN_PX} px touch floor; type floors met; no horizontal scroll; dialogs lock the page`);
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(`\nFATAL: ${e.message}`);
  if (process.env.SHOTS_DEBUG) console.error(e.stack);
  process.exit(2);
});
