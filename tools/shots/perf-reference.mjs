#!/usr/bin/env node
// Measures a THIRD-PARTY web poker client with the same instrument used on
// ClearDeck (tools/shots/perf.mjs), so the two sets of numbers are comparable.
//
//   node tools/shots/perf-reference.mjs
//   node tools/shots/perf-reference.mjs --loads 10 --idle-ms 4000
//
// WHAT IT DOES AND DOES NOT DO
//   * loads a PUBLIC page and times it, using the browser's own
//     PerformanceObserver — exactly the instrument used on our own lobby
//   * samples requestAnimationFrame cadence while the page sits idle
//   * creates NO account, enters NO credentials, joins NO game, accepts NO
//     consent banner and touches NOTHING involving money
//
// Because it never sits down at a table, it CANNOT measure click-to-acknowledge
// or action-to-settled on the reference client. Those cells stay empty in
// docs/RESPONSIVENESS.md rather than being filled with a guess.
//
// This is a measurement tool pointed at a public URL. It reaches the public
// internet and nothing else; it never touches the local replica or any
// ClearDeck canister.

import fs from 'node:fs';
import path from 'node:path';
import { REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { launchBrowser } from './lib/browser.mjs';
import { gitShortSha } from './lib/capture.mjs';

const log = (m) => console.log(m);

const TARGETS = [
  { key: 'pokernow-home', url: 'https://www.pokernow.club/', label: 'PokerNow — landing/lobby entry' },
];

const round = (x) => (x === null || x === undefined ? null : Math.round(x * 10) / 10);

function quantile(xs, q) {
  if (!xs.length) return null;
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.max(0, Math.ceil(q * s.length) - 1))];
}

function stats(xs, unit = 'ms') {
  const clean = xs.filter((x) => Number.isFinite(x));
  if (!clean.length) return { n: 0, unit, note: 'no samples' };
  return {
    n: clean.length,
    unit,
    min: round(Math.min(...clean)),
    p50: round(quantile(clean, 0.5)),
    p95: round(quantile(clean, 0.95)),
    max: round(Math.max(...clean)),
  };
}

/** Same shape of instrumentation as perf.mjs, minus the ClearDeck-specific marks. */
const INSTRUMENT = () => {
  const cd = { marks: {}, frames: null, loaf: [] };
  window.__ref = cd;
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) {
        if (e.name === 'first-paint') cd.marks.firstPaint ??= e.startTime;
        if (e.name === 'first-contentful-paint') cd.marks.fcp ??= e.startTime;
      }
    }).observe({ type: 'paint', buffered: true });
  } catch {}
  try {
    new PerformanceObserver((l) => {
      const es = l.getEntries();
      if (es.length) cd.marks.lcp = es[es.length - 1].startTime;
    }).observe({ type: 'largest-contentful-paint', buffered: true });
  } catch {}
  try {
    new PerformanceObserver((l) => {
      for (const e of l.getEntries()) cd.loaf.push({ duration: e.duration, blocking: e.blockingDuration });
    }).observe({ type: 'long-animation-frame', buffered: true });
  } catch {}
  const tick = (ts) => {
    if (cd.frames) cd.frames.push(ts);
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  window.__refStartFrames = () => { cd.frames = []; };
  window.__refStopFrames = () => { const f = cd.frames || []; cd.frames = null; return f; };
};

function parseArgs(argv) {
  const out = {
    loads: 10,
    idleMs: 4000,
    json: path.join(REPO_ROOT, 'artifacts', 'perf', `perf-reference-${gitShortSha()}.json`),
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--loads') out.loads = Number(argv[++i]);
    else if (a === '--idle-ms') out.idleMs = Number(argv[++i]);
    else if (a === '--json') out.json = path.resolve(argv[++i]);
    else throw new Error(`Unknown argument: ${a}`);
  }
  return out;
}

async function main() {
  const args = parseArgs(process.argv);
  const vp = VIEWPORTS.desktop;
  const browser = await launchBrowser({ log });
  const results = [];

  try {
    for (const target of TARGETS) {
      log(`\n▸ ${target.label}  ${target.url}`);
      const acc = { firstPaint: [], fcp: [], lcp: [] };
      let idleIntervals = [];
      let reachable = true;
      let failure = null;

      for (let i = 0; i < args.loads; i += 1) {
        const context = await browser.newContext({
          viewport: { width: vp.width, height: vp.height },
          deviceScaleFactor: 1,
          locale: 'en-US',
          timezoneId: 'UTC',
          reducedMotion: 'no-preference',
        });
        await context.addInitScript(INSTRUMENT);
        const page = await context.newPage();
        try {
          await page.goto(target.url, { waitUntil: 'commit', timeout: 45_000 });
          await page.waitForFunction(() => window.__ref?.marks?.fcp !== undefined, undefined, { timeout: 45_000 });
          await page.waitForTimeout(600);
          const m = await page.evaluate(() => ({ ...window.__ref.marks }));
          acc.firstPaint.push(m.firstPaint);
          acc.fcp.push(m.fcp);
          acc.lcp.push(m.lcp);

          // One idle cadence sample, on the last run only.
          if (i === args.loads - 1) {
            await page.evaluate(() => window.__refStartFrames());
            await page.waitForTimeout(args.idleMs);
            const frames = await page.evaluate(() => window.__refStopFrames());
            idleIntervals = [];
            for (let k = 1; k < frames.length; k += 1) idleIntervals.push(frames[k] - frames[k - 1]);
          }
        } catch (e) {
          reachable = false;
          failure = String(e.message || e).split('\n')[0];
          await page.close().catch(() => {});
          await context.close().catch(() => {});
          break;
        }
        await page.close();
        await context.close();
      }

      results.push({
        ...target,
        reachable,
        failure,
        firstPaint: stats(acc.firstPaint),
        firstContentfulPaint: stats(acc.fcp),
        largestContentfulPaint: stats(acc.lcp),
        idleFrameIntervalMs: stats(idleIntervals),
        notMeasured: [
          'click-to-acknowledgement (needs a seat at a table)',
          'action-to-settled (needs a seat at a table)',
          'showdown animation cadence (needs a hand in progress)',
        ],
      });
      const r = results[results.length - 1];
      if (reachable) {
        log(`  FCP p50 ${r.firstContentfulPaint.p50}ms  LCP p50 ${r.largestContentfulPaint.p50}ms  `
          + `idle frame interval p50 ${r.idleFrameIntervalMs.p50}ms`);
      } else {
        log(`  NOT REACHABLE: ${failure}`);
      }
    }
  } finally {
    await browser.close();
  }

  const out = {
    startedAt: new Date().toISOString(),
    gitSha: gitShortSha(),
    viewport: vp,
    method: {
      instrument: "the same PerformanceObserver + requestAnimationFrame code as tools/shots/perf.mjs",
      browser: 'headless Chromium via Playwright, same binary and same machine as the ClearDeck run',
      interaction: 'NONE. No account, no game joined, no consent banner clicked, no money involved.',
      networkCaveat: 'these are public-internet loads; the ClearDeck numbers are localhost loads. '
        + 'The two are NOT comparable as absolute load times and must not be presented as a race.',
    },
    targets: results,
  };
  fs.mkdirSync(path.dirname(args.json), { recursive: true });
  fs.writeFileSync(args.json, JSON.stringify(out, null, 2));
  log(`\nwrote ${path.relative(REPO_ROOT, args.json)}`);
  return 0;
}

main()
  .then((c) => process.exit(c))
  .catch((e) => {
    console.error(`\nFATAL: ${e.message}`);
    process.exit(2);
  });
