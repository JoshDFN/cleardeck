#!/usr/bin/env node
// ClearDeck responsiveness measurement. Numbers, not vibes.
//
//   node tools/shots/perf.mjs                       # everything, default sample counts
//   node tools/shots/perf.mjs --only load            # load,entry,action,animation
//   node tools/shots/perf.mjs --loads 20 --actions 40 --hands 5
//   node tools/shots/perf.mjs --json artifacts/perf/perf.json
//
// Measures the REAL app against the REAL local canisters through the same port
// shim and the same asset canister the screenshot harness uses. Nothing is mocked
// and no canister response is intercepted.
//
// FOUR MEASUREMENTS
//   load       time to first meaningful paint on the LOBBY (cold navigation)
//   entry      time from clicking a lobby row to the TABLE showing on-chain state
//   action     click -> visible acknowledgement, and click -> settled visible state
//   animation  frame cadence through the all-in -> showdown transition
//
// WHAT "VISIBLE" MEANS HERE, EXACTLY
//   Every timestamp is taken inside the page, in `performance.now()`'s time base,
//   from the real `click` event's own `timeStamp`. An acknowledgement is counted
//   as VISIBLE at the first `requestAnimationFrame` callback after the DOM change
//   that carries it — that is the frame which will present it. The pre-paint
//   MutationObserver time is reported too (`domAckMs`), because the gap between
//   the two is itself interesting and hiding it would overstate the app.
//
// HEADLESS CAVEAT, STATED ONCE
//   This runs headless Chromium. rAF cadence in headless is driven by the
//   compositor and is a PROXY for on-screen frame rate, not a measurement of a
//   physical display. Load numbers come from the browser's own
//   PerformanceObserver (`paint`, `largest-contentful-paint`), which is the same
//   instrument a real user's Chrome reports. Third-party STATIC assets (fonts,
//   avatars) are served from the harness's on-disk cache, so load numbers are
//   warm-cache numbers for those hosts and cold for everything the replica serves.

import fs from 'node:fs';
import path from 'node:path';
import {
  APP_ORIGIN, GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER,
  PROXY_PORT, REPO_ROOT, VIEWPORTS, icp,
} from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { overrideDistDir, startGatewayProxy } from './lib/proxy.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import {
  doAct, phaseOf, sleep, startHand, tableActorFor, view, waitForState,
} from './lib/table-driver.mjs';
import {
  getAppOrigin, launchBrowser, setAppOrigin,
} from './lib/browser.mjs';
import { prepareTable } from './scenarios/_shared.mjs';
import { gitShortSha } from './lib/capture.mjs';
import { startLoadWatch } from './lib/load-watch.mjs';

const log = (m) => console.log(m);

const TABLE = 'table_2';
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const STACK = icp(40);

// ---------------------------------------------------------------------------
// statistics
// ---------------------------------------------------------------------------

/** @param {number[]} xs @param {number} q 0..1 */
function quantile(xs, q) {
  if (xs.length === 0) return null;
  const s = [...xs].sort((a, b) => a - b);
  if (s.length === 1) return s[0];
  // Nearest-rank, which is the honest choice for small n: every reported number
  // is an observation that really happened, not an interpolation between two.
  const rank = Math.min(s.length - 1, Math.max(0, Math.ceil(q * s.length) - 1));
  return s[rank];
}

function stats(xs, unit = 'ms') {
  const clean = xs.filter((x) => Number.isFinite(x));
  if (clean.length === 0) return { n: 0, unit, note: 'no samples' };
  const sum = clean.reduce((a, b) => a + b, 0);
  return {
    n: clean.length,
    unit,
    min: round(Math.min(...clean)),
    p50: round(quantile(clean, 0.5)),
    p95: round(quantile(clean, 0.95)),
    max: round(Math.max(...clean)),
    mean: round(sum / clean.length),
    samples: clean.map(round),
  };
}

const round = (x) => (x === null ? null : Math.round(x * 10) / 10);

// ---------------------------------------------------------------------------
// in-page instrumentation
// ---------------------------------------------------------------------------

/**
 * Installed before any app code runs. Records paint timings, the app's own
 * "meaningful" milestones, action acknowledgement/settle times and frame cadence.
 *
 * Everything here is passive observation of the app's real DOM. It changes no
 * app state and calls no canister.
 */
const INSTRUMENT = () => {
  const cd = {
    marks: {},
    action: null,
    actions: [],
    frames: null,
    loaf: [],
  };
  window.__cd = cd;

  const now = () => performance.now();

  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        if (e.name === 'first-paint') cd.marks.firstPaint ??= e.startTime;
        if (e.name === 'first-contentful-paint') cd.marks.fcp ??= e.startTime;
      }
    }).observe({ type: 'paint', buffered: true });
  } catch {}

  try {
    new PerformanceObserver((list) => {
      const es = list.getEntries();
      if (es.length) cd.marks.lcp = es[es.length - 1].startTime;
    }).observe({ type: 'largest-contentful-paint', buffered: true });
  } catch {}

  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        cd.loaf.push({ start: e.startTime, duration: e.duration, blocking: e.blockingDuration });
      }
    }).observe({ type: 'long-animation-frame', buffered: true });
  } catch {}

  // ---- app milestones -----------------------------------------------------
  // The lobby is meaningful when the app itself says it is settled AND there is
  // at least one real table row. The table is meaningful when the felt carries
  // on-chain state: a seated stack, or a pot, or a phase.
  const lobbyReady = () => {
    const main = document.querySelector('main');
    return !!main
      && main.getAttribute('data-lobby-state') === 'ready'
      && document.querySelectorAll('tbody tr').length > 0;
  };
  const tableShell = () => !!document.querySelector('.poker-table');
  // ON-CHAIN STATE, NOT THE SHELL. The first version of this predicate accepted
  // `.pot-amount`, which the felt renders as "--" the instant the shell mounts,
  // so "click to on-chain state" measured the same frame as "click to shell" and
  // flattered the app by however long the canister actually takes. A seated
  // player's CHIP COUNT is a figure that cannot exist until get_table_view() has
  // come back, so that is the milestone.
  const tableStateVisible = () =>
    [...document.querySelectorAll('.seat .chips')].some((el) => /\d/.test(el.textContent || ''));

  // ---- one rAF loop drives every "when was it painted" answer -------------
  let pendingPaintMarks = [];
  const markOnNextFrame = (key) => {
    if (cd.marks[key] !== undefined) return;
    pendingPaintMarks.push(key);
  };

  const tick = (ts) => {
    // Anything that mutated since the last frame is presented by THIS frame.
    if (pendingPaintMarks.length) {
      for (const key of pendingPaintMarks) cd.marks[key] ??= ts;
      pendingPaintMarks = [];
    }

    if (cd.marks.lobbyMeaningfulDom === undefined && lobbyReady()) {
      cd.marks.lobbyMeaningfulDom = now();
      markOnNextFrame('lobbyMeaningful');
    }
    if (cd.marks.tableShellDom === undefined && tableShell()) {
      cd.marks.tableShellDom = now();
      markOnNextFrame('tableShell');
    }
    if (cd.marks.tableStateDom === undefined && tableStateVisible()) {
      cd.marks.tableStateDom = now();
      markOnNextFrame('tableState');
    }

    // THREE MILESTONES, DELIBERATELY SEPARATE.
    //
    //   ack        the spinner is on screen: the client has heard the click
    //   callReturn the spinner is gone: `player_action` resolved
    //   settled    the FELT shows the new money: the hero's own stack or bet, or
    //              the pot, is different text from what it was at click time
    //
    // The first draft folded `callReturn` and `settled` together and used
    // `.actions.disabled` as the signal. That is wrong twice over: the action bar
    // stays disabled until it is the hero's turn AGAIN, so the number silently
    // included the OPPONENT's thinking time; and `actionPending` clears in a
    // `finally` the moment the update call resolves, which is before the client
    // has re-read the table. Separating them is what shows that the money on
    // screen arrives on the 500 ms poll rather than on the call's own reply.
    const a = cd.action;
    if (a) {
      const pending = !!document.querySelector('.action-pending');
      if (a.domAckAt === null && pending) {
        a.domAckAt = now();
        a.awaitingAckPaint = true;
      } else if (a.awaitingAckPaint) {
        a.paintedAckAt = ts;
        a.awaitingAckPaint = false;
      }
      if (a.domAckAt !== null && a.callReturnedAt === null && !pending) {
        a.callReturnedAt = ts;
      }
      if (a.settledAt === null && cd.money() !== a.moneyAtClick) {
        a.settledAt = ts;
      }
      if (a.callReturnedAt !== null && a.settledAt !== null) {
        cd.actions.push({ ...a });
        cd.action = null;
      } else if (a.callReturnedAt !== null && ts - a.clickAt > 8000) {
        // The felt never changed within 8 s. Recorded as such rather than
        // dropped, so a client that acknowledges and then never updates is
        // visible in the data instead of absent from it.
        a.settledAt = null;
        a.settleTimedOut = true;
        cd.actions.push({ ...a });
        cd.action = null;
      }
    }

    if (cd.frames) cd.frames.push(ts);
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);

  // THE MONEY ON THE FELT, as one string. Only chain-derived figures: the pot,
  // the hero's own stack and the hero's own live bet. When this string changes,
  // the canister's answer has reached the screen.
  const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : '');
  cd.money = () => {
    const mySeat = document.querySelector('.player-nameplate.highlight-me')?.closest('.seat');
    return [
      txt(document.querySelector('.pot-amount')),
      txt(mySeat?.querySelector('.chips')),
      txt(mySeat?.querySelector('.bet-amount')),
    ].join('|');
  };

  // ---- the real click, timestamped by the browser -------------------------
  document.addEventListener(
    'click',
    (e) => {
      const btn = e.target instanceof Element ? e.target.closest('.action-btn') : null;
      if (!btn) return;
      cd.action = {
        label: (btn.textContent || '').replace(/\s+/g, ' ').trim(),
        clickAt: e.timeStamp,
        moneyAtClick: cd.money(),
        domAckAt: null,
        paintedAckAt: null,
        callReturnedAt: null,
        settledAt: null,
        settleTimedOut: false,
        awaitingAckPaint: false,
      };
    },
    true,
  );

  window.__cdStartFrames = () => { cd.frames = []; };
  window.__cdStopFrames = () => { const f = cd.frames || []; cd.frames = null; return f; };
  window.__cdReset = () => { cd.actions = []; cd.action = null; };
};

/** A context with animations LEFT ON — the opposite of the screenshot harness. */
async function perfContext(browser, vp) {
  const context = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: 1,
    isMobile: vp.isMobile,
    hasTouch: vp.isMobile,
    locale: 'en-US',
    timezoneId: 'UTC',
    colorScheme: 'dark',
    // MEASURING ANIMATION, so reducedMotion must NOT be 'reduce' (that is what
    // the screenshot harness sets, and it would suppress the very transitions
    // this file exists to time).
    reducedMotion: 'no-preference',
  });
  await context.addInitScript(() => {
    try {
      localStorage.setItem('poker_sound_muted', 'true');
      localStorage.setItem('poker_avatar_style', 'bottts');
      localStorage.setItem('lobby_view', 'list');
      localStorage.removeItem('poker_custom_name');
      localStorage.removeItem('poker_wallet_collapsed');
    } catch {}
  });
  await context.addInitScript(INSTRUMENT);
  return context;
}

const marks = (page) => page.evaluate(() => ({ ...window.__cd.marks }));

async function devLogin(page, playerNum) {
  await page.waitForSelector('.wallet-btn.dev', { timeout: 30_000 });
  await page.click('.wallet-btn.dev');
  await page.waitForSelector('.dev-menu', { timeout: 15_000 });
  await page.click(`.dev-menu button:text-is("Player ${playerNum}")`);
  await page.waitForSelector('.wallet-btn.connected', { timeout: 30_000 });
}

// ---------------------------------------------------------------------------
// 1. lobby: time to first meaningful paint
// ---------------------------------------------------------------------------

async function measureLobbyLoad(browser, vp, runs) {
  const out = { firstPaint: [], fcp: [], lcp: [], meaningful: [], meaningfulDom: [] };
  for (let i = 0; i < runs; i += 1) {
    const context = await perfContext(browser, vp);
    const page = await context.newPage();
    try {
      await page.goto(getAppOrigin(), { waitUntil: 'commit', timeout: 60_000 });
      await page.waitForFunction(() => window.__cd?.marks?.lobbyMeaningful !== undefined, undefined, {
        timeout: 60_000,
      });
      // Give LCP one more beat to settle on its final candidate.
      await page.waitForTimeout(400);
      const m = await marks(page);
      out.firstPaint.push(m.firstPaint);
      out.fcp.push(m.fcp);
      out.lcp.push(m.lcp);
      out.meaningful.push(m.lobbyMeaningful);
      out.meaningfulDom.push(m.lobbyMeaningfulDom);
    } finally {
      await page.close();
      await context.close();
    }
  }
  return {
    firstPaint: stats(out.firstPaint),
    firstContentfulPaint: stats(out.fcp),
    largestContentfulPaint: stats(out.lcp),
    firstMeaningfulPaint: stats(out.meaningful),
    firstMeaningfulDomMutation: stats(out.meaningfulDom),
  };
}

// ---------------------------------------------------------------------------
// 2. table entry: lobby row click -> table showing on-chain state
// ---------------------------------------------------------------------------

async function measureTableEntry(browser, vp, tableName, runs, ctx) {
  // Two seats really occupied, so "on-chain state visible" has something to be
  // visible ABOUT: an empty table renders no stack, and the milestone would
  // never fire.
  await prepareTable(ctx, TABLE, [
    { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
    { player: OPP, seat: OPP_SEAT, chips: STACK },
  ]);
  const out = { clickToShell: [], clickToState: [], navToState: [] };
  for (let i = 0; i < runs; i += 1) {
    const context = await perfContext(browser, vp);
    const page = await context.newPage();
    try {
      await page.goto(getAppOrigin(), { waitUntil: 'commit', timeout: 60_000 });
      await page.waitForFunction(() => window.__cd?.marks?.lobbyMeaningful !== undefined, undefined, { timeout: 60_000 });
      await devLogin(page, HERO_PLAYER);

      const row = page.locator('tr', { has: page.locator('.table-name', { hasText: tableName }) });
      await row.first().waitFor({ timeout: 30_000 });
      const clickAt = await page.evaluate(() => performance.now());
      await row.first().click();

      await page.waitForFunction(() => window.__cd?.marks?.tableState !== undefined, undefined, { timeout: 60_000 });
      const m = await marks(page);
      out.clickToShell.push(m.tableShell - clickAt);
      out.clickToState.push(m.tableState - clickAt);
      out.navToState.push(m.tableState);
    } finally {
      await page.close();
      await context.close();
    }
  }
  return {
    rowClickToTableShell: stats(out.clickToShell),
    rowClickToOnChainStateVisible: stats(out.clickToState),
    navigationStartToOnChainStateVisible: stats(out.navToState),
  };
}

// ---------------------------------------------------------------------------
// 3. action latency
// ---------------------------------------------------------------------------

/**
 * Plays the opponent's seat from Node while the browser plays the hero, so the
 * hero gets a fresh decision every few seconds. Real update calls, real engine.
 */
function startOpponentBot(tableId, stopFlag, counters) {
  const loop = (async () => {
    while (!stopFlag.done) {
      try {
        const v = await view(OPP, tableId);
        if (!v) { await sleep(250); continue; }
        const phase = phaseOf(v);
        if (phase === 'WaitingForPlayers' || phase === 'HandComplete') {
          try { await startHand(OPP, tableId); counters.handsStarted += 1; } catch {}
          await sleep(400);
          continue;
        }
        if (Number(v.action_on) === OPP_SEAT && v.is_my_turn) {
          const t = await tableActorFor(OPP, tableId);
          // BET WHEN IT CAN. The hero's "settled" milestone is "the hero's own
          // stack or the pot changed on screen", which a Check cannot produce.
          // An opponent that only ever checks starves the measurement of the
          // samples it exists to take. So the bot opens for the minimum whenever
          // the betting round is fresh, putting a real amount in front of the
          // hero on nearly every decision. Minimum-sized, so 40 ICP stacks
          // survive dozens of hands.
          try {
            if (Number(v.current_bet) === 0 && v.can_raise) {
              await doAct.bet(Number(v.min_bet ?? v.config.big_blind))(t);
              counters.oppBets += 1;
            } else {
              await doAct.checkOrCall(t, v);
            }
            counters.oppActions += 1;
          } catch {
            try { await doAct.checkOrCall(t, v); counters.oppActions += 1; } catch {}
          }
        }
      } catch {
        // The bot is a fixture, not the thing under test; a transient replica
        // error must not end the measurement.
      }
      await sleep(200);
    }
  })();
  return loop;
}

async function measureActions(browser, vp, ctx, wanted) {
  const tableId = ctx.tableIds[TABLE];
  await prepareTable(ctx, TABLE, [
    { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
    { player: OPP, seat: OPP_SEAT, chips: STACK },
  ]);

  const context = await perfContext(browser, vp);
  const page = await context.newPage();
  const stopFlag = { done: false };
  const counters = { handsStarted: 0, oppActions: 0, oppBets: 0 };
  const bot = startOpponentBot(tableId, stopFlag, counters);
  const samples = [];

  try {
    await page.goto(getAppOrigin(), { waitUntil: 'commit', timeout: 60_000 });
    await page.waitForFunction(() => window.__cd?.marks?.lobbyMeaningful !== undefined, undefined, { timeout: 60_000 });
    await devLogin(page, HERO_PLAYER);
    const row = page.locator('tr', { has: page.locator('.table-name', { hasText: ctx.tableNames[TABLE] }) });
    await row.first().waitFor({ timeout: 30_000 });
    await row.first().click();
    await page.waitForSelector('.poker-table', { timeout: 30_000 });

    const deadline = Date.now() + Number(process.env.PERF_ACTION_BUDGET_MS || 480_000);
    while (samples.length < wanted && Date.now() < deadline) {
      // Wait for the hero to really be on the clock, with live buttons.
      try {
        await page.waitForSelector('.actions:not(.disabled) .action-btn', { timeout: 20_000 });
      } catch {
        continue;
      }
      // PREFER CALL. A Check moves no money, so "the felt shows the new state"
      // has nothing to detect; a Call changes the hero's own stack and the pot,
      // which is exactly the milestone being timed. Check is the fallback when
      // there is nothing to call.
      const labels = await page.locator('.actions .action-btn').allTextContents();
      const callIdx = labels.findIndex((t) => /^call/i.test(t.trim()));
      const checkIdx = labels.findIndex((t) => /check/i.test(t));
      const pick = callIdx >= 0 ? callIdx : checkIdx;
      if (pick < 0) { await page.waitForTimeout(250); continue; }

      const before = await page.evaluate(() => window.__cd.actions.length);
      await page.locator('.actions .action-btn').nth(pick).click();
      try {
        await page.waitForFunction(
          (n) => window.__cd.actions.length > n,
          before,
          { timeout: 45_000 },
        );
      } catch {
        continue;
      }
      const rec = await page.evaluate(() => window.__cd.actions[window.__cd.actions.length - 1]);
      samples.push({
        label: rec.label,
        domAckMs: rec.domAckAt - rec.clickAt,
        paintedAckMs: rec.paintedAckAt - rec.clickAt,
        callReturnedMs: rec.callReturnedAt === null ? NaN : rec.callReturnedAt - rec.clickAt,
        settledMs: rec.settledAt === null ? NaN : rec.settledAt - rec.clickAt,
        settleTimedOut: Boolean(rec.settleTimedOut),
      });
      log(`    action ${samples.length}/${wanted} "${rec.label}": ack ${round(rec.paintedAckAt - rec.clickAt)}ms, `
        + `call returned ${round(rec.callReturnedAt - rec.clickAt)}ms, `
        + `felt updated ${rec.settledAt === null ? 'NEVER (>8s)' : `${round(rec.settledAt - rec.clickAt)}ms`}`);
    }
    const loaf = await page.evaluate(() => window.__cd.loaf.slice(0, 200));
    return { samples, counters, loaf };
  } finally {
    stopFlag.done = true;
    await bot.catch(() => {});
    await page.close();
    await context.close();
  }
}

// ---------------------------------------------------------------------------
// 4. animation cadence through all-in -> showdown
// ---------------------------------------------------------------------------

async function measureShowdownAnimation(browser, vp, ctx, hands, sampleMs) {
  const tableId = ctx.tableIds[TABLE];
  const perHand = [];

  for (let h = 0; h < hands; h += 1) {
    await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: icp(12) },
      { player: OPP, seat: OPP_SEAT, chips: icp(18) },
    ]);
    await startHand(OPP, tableId);

    // Drive to "hero all in, opponent still to act" from Node, so the browser is
    // watching a genuine engine transition rather than a scripted one.
    const heroActor = await tableActorFor(HERO_PLAYER, tableId);
    const oppActor = await tableActorFor(OPP, tableId);
    let staged = false;
    const stageDeadline = Date.now() + 60_000;
    while (!staged && Date.now() < stageDeadline) {
      const v = await view(HERO_PLAYER, tableId);
      if (!v) { await sleep(200); continue; }
      const seat = Number(v.action_on);
      const heroView = await view(HERO_PLAYER, tableId);
      const allInHero = optional(v.players[HERO_SEAT])?.is_all_in;
      if (allInHero && seat === OPP_SEAT) { staged = true; break; }
      if (seat === HERO_SEAT && heroView.is_my_turn) {
        try { await doAct.allIn(heroActor); } catch {}
      } else if (seat === OPP_SEAT) {
        const ov = await view(OPP, tableId);
        if (ov?.is_my_turn && !allInHero) {
          try { await doAct.checkOrCall(oppActor, ov); } catch {}
        }
      }
      await sleep(200);
    }
    if (!staged) { log(`    hand ${h + 1}: could not reach the all-in moment, skipped`); continue; }

    const context = await perfContext(browser, vp);
    const page = await context.newPage();
    try {
      await page.goto(getAppOrigin(), { waitUntil: 'commit', timeout: 60_000 });
      await page.waitForFunction(() => window.__cd?.marks?.lobbyMeaningful !== undefined, undefined, { timeout: 60_000 });
      await devLogin(page, HERO_PLAYER);
      const row = page.locator('tr', { has: page.locator('.table-name', { hasText: ctx.tableNames[TABLE] }) });
      await row.first().waitFor({ timeout: 30_000 });
      await row.first().click();
      await page.waitForSelector('.avatar-overlay.allin', { timeout: 30_000 });

      // CONTROL SAMPLE. "60 fps during the showdown" means nothing without
      // knowing what this browser does when nothing is happening: in headless
      // Chromium the compositor is not vsync-locked to a display and ticks
      // faster than 60 Hz. The idle cadence is measured on the same page, in the
      // same run, immediately before the transition, so the animation figure is
      // read as a DELTA against it rather than as an absolute frame rate.
      await page.evaluate(() => window.__cdStartFrames());
      await page.waitForTimeout(1000);
      const idleFrames = await page.evaluate(() => window.__cdStopFrames());
      const idleIntervals = [];
      for (let i = 1; i < idleFrames.length; i += 1) idleIntervals.push(idleFrames[i] - idleFrames[i - 1]);

      await page.evaluate(() => window.__cdStartFrames());
      const t0 = await page.evaluate(() => performance.now());
      await doAct.call(oppActor);
      let winnerAt = null;
      try {
        await page.waitForSelector('.winner-display', { timeout: 30_000 });
        winnerAt = await page.evaluate(() => performance.now());
      } catch {}
      await page.waitForTimeout(sampleMs);
      const frames = await page.evaluate(() => window.__cdStopFrames());
      const loaf = await page.evaluate(() => window.__cd.loaf.slice(0, 200));

      const intervals = [];
      for (let i = 1; i < frames.length; i += 1) intervals.push(frames[i] - frames[i - 1]);
      perHand.push({
        hand: h + 1,
        frames: frames.length,
        windowMs: round(frames.length ? frames[frames.length - 1] - frames[0] : 0),
        callToWinnerBannerMs: winnerAt === null ? null : round(winnerAt - t0),
        intervals,
        idleIntervals,
        loafCount: loaf.length,
        loafWorstMs: loaf.length ? round(Math.max(...loaf.map((l) => l.duration))) : null,
      });
      log(`    hand ${h + 1}: ${frames.length} frames, banner at ${perHand[perHand.length - 1].callToWinnerBannerMs}ms`);
    } finally {
      await page.close();
      await context.close();
    }
  }

  const allIntervals = perHand.flatMap((h) => h.intervals);
  const allIdle = perHand.flatMap((h) => h.idleIntervals || []);
  const fps = allIntervals.map((ms) => 1000 / ms);
  return {
    hands: perHand.map(({ intervals, idleIntervals, ...rest }) => ({
      ...rest, intervalStats: stats(intervals), idleIntervalStats: stats(idleIntervals || []),
    })),
    frameIntervalMs: stats(allIntervals),
    // The control: the same page, the same browser, nothing happening.
    idleFrameIntervalMs: stats(allIdle),
    compositorTickCeiling: {
      note: 'headless Chromium is not vsync-locked to a display; the shortest interval observed '
        + 'is this browser\'s tick period, and no measurement here can exceed it',
      shortestIntervalMs: allIntervals.length ? round(Math.min(...allIntervals)) : null,
      impliedTickHz: allIntervals.length ? round(1000 / Math.min(...allIntervals)) : null,
    },
    // Effective FPS is reported as the reciprocal of the interval quantiles, not
    // as the mean of per-frame FPS: p95 of the INTERVAL is the slow frame, which
    // is what a player notices.
    effectiveFps: {
      p50: allIntervals.length ? round(1000 / quantile(allIntervals, 0.5)) : null,
      worst5pct: allIntervals.length ? round(1000 / quantile(allIntervals, 0.95)) : null,
      meanOfPerFrameFps: fps.length ? round(fps.reduce((a, b) => a + b, 0) / fps.length) : null,
    },
    droppedFrames: {
      over20ms: allIntervals.filter((x) => x > 20).length,
      over34ms: allIntervals.filter((x) => x > 34).length,
      over100ms: allIntervals.filter((x) => x > 100).length,
      total: allIntervals.length,
    },
    callToWinnerBannerMs: stats(perHand.map((h) => h.callToWinnerBannerMs)),
  };
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const out = {
    only: null, loads: 15, entries: 10, actions: 30, hands: 4, sampleMs: 2500,
    viewport: 'desktop',
    json: path.join(REPO_ROOT, 'artifacts', 'perf', `perf-${gitShortSha()}.json`),
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--only') out.only = String(argv[++i]).split(',');
    else if (a === '--loads') out.loads = Number(argv[++i]);
    else if (a === '--entries') out.entries = Number(argv[++i]);
    else if (a === '--actions') out.actions = Number(argv[++i]);
    else if (a === '--hands') out.hands = Number(argv[++i]);
    else if (a === '--sample-ms') out.sampleMs = Number(argv[++i]);
    else if (a === '--viewport') out.viewport = String(argv[++i]);
    else if (a === '--json') out.json = path.resolve(argv[++i]);
    else throw new Error(`Unknown argument: ${a}`);
  }
  return out;
}

async function resolveTableNames(ids) {
  const lobby = await lobbyActor(ids.lobby);
  const tables = await lobby.get_tables();
  const names = {};
  for (const t of tables) {
    const cid = optional(t.canister_id);
    if (!cid) continue;
    for (const key of ['table_1', 'table_2', 'table_3', 'btc_table_1']) {
      if (ids[key] === cid.toText()) names[key] = t.name;
    }
  }
  return names;
}

async function main() {
  const args = parseArgs(process.argv);
  const wants = (name) => !args.only || args.only.includes(name);
  const vp = VIEWPORTS[args.viewport];
  if (!vp) throw new Error(`Unknown viewport "${args.viewport}"`);

  log('ClearDeck responsiveness measurement');
  log(`  gateway: ${GATEWAY_ORIGIN}`);

  const res = await fetch(`${GATEWAY_ORIGIN}/api/v2/status`, { signal: AbortSignal.timeout(8000) });
  if (!res.ok && res.status !== 400) throw new Error(`replica gateway not answering: HTTP ${res.status}`);

  const ids = readLocalIds();
  const frontendId = requireId(ids, 'frontend');

  // docs/DEFECTS.md T-14: same as run.mjs. The bundle carries the real gateway
  // port now, so a perf number is measured against the URL a human opens rather
  // than through a reverse proxy that adds a hop nobody else has.
  let proxy = { stats: () => ({ skipped: true }), close: async () => {} };
  if (overrideDistDir()) {
    proxy = await startGatewayProxy({ frontendCanisterId: frontendId, log });
    setAppOrigin(APP_ORIGIN);
  } else {
    setAppOrigin(`http://${frontendId}.${GATEWAY_HOST}:${GATEWAY_PORT}`);
  }

  const tableNames = await resolveTableNames(ids);
  const ctx = {
    ids,
    tableIds: {
      table_1: ids.table_1, table_2: ids.table_2, table_3: ids.table_3, btc_table_1: ids.btc_table_1,
    },
    tableNames,
    log,
  };

  // WHAT ELSE THE MACHINE WAS DOING. Five agents share this laptop; the wave-3
  // numbers were taken with cargo builds and extra PocketIC instances running,
  // which inflates every latency here by an unknown amount. Sampled throughout
  // and recorded in the JSON, so a reader can judge the run instead of trusting
  // the word "quiet". See lib/load-watch.mjs.
  const loadWatch = startLoadWatch({ everyMs: 10_000 });
  const browser = await launchBrowser({ log });
  const out = {
    startedAt: new Date().toISOString(),
    gitSha: gitShortSha(),
    gateway: GATEWAY_ORIGIN,
    appOrigin: getAppOrigin(),
    frontendCanisterId: frontendId,
    viewport: vp,
    table: { name: tableNames[TABLE], canisterId: ids[TABLE] },
    method: {
      timeBase: "performance.now() inside the page; click times are the browser's own event.timeStamp",
      visible: 'first requestAnimationFrame callback after the DOM change that carries the state',
      headless: 'headless Chromium; rAF cadence is a proxy for presented frames',
      thirdParty: 'no interception in this run: fonts/avatars are fetched live or fail, exactly as the app would',
      quantiles: 'nearest-rank, so every reported value is an observation that happened',
    },
  };

  try {
    if (wants('load')) {
      log(`\n[1] lobby first meaningful paint  (${args.loads} cold navigations)`);
      out.lobbyLoad = await measureLobbyLoad(browser, vp, args.loads);
      log(`    FMP p50 ${out.lobbyLoad.firstMeaningfulPaint.p50}ms  p95 ${out.lobbyLoad.firstMeaningfulPaint.p95}ms`);
    }
    if (wants('entry')) {
      log(`\n[2] lobby row -> table on-chain state  (${args.entries} runs)`);
      out.tableEntry = await measureTableEntry(browser, vp, tableNames[TABLE], args.entries, ctx);
      log(`    click->state p50 ${out.tableEntry.rowClickToOnChainStateVisible.p50}ms  p95 ${out.tableEntry.rowClickToOnChainStateVisible.p95}ms`);
    }
    if (wants('action')) {
      log(`\n[3] action feedback + settle  (target ${args.actions} real actions)`);
      const a = await measureActions(browser, vp, ctx, args.actions);
      const moneyMoving = a.samples.filter((s) => /^call/i.test(s.label));
      out.action = {
        visibleAcknowledgementMs: stats(a.samples.map((s) => s.paintedAckMs)),
        domAcknowledgementMs: stats(a.samples.map((s) => s.domAckMs)),
        updateCallReturnedMs: stats(a.samples.map((s) => s.callReturnedMs)),
        settledVisibleStateMs: stats(a.samples.map((s) => s.settledMs)),
        // The headline number: only actions that move the hero's own money, so
        // "the felt shows the new state" has something real to detect.
        settledVisibleStateMoneyMovingOnly: stats(moneyMoving.map((s) => s.settledMs)),
        // How much of the settle time is the client waiting for its own poll
        // rather than for the canister.
        pollWaitAfterCallReturnedMs: stats(
          a.samples.map((s) => s.settledMs - s.callReturnedMs).filter((x) => Number.isFinite(x) && x >= 0),
        ),
        settleTimeouts: a.samples.filter((s) => s.settleTimedOut).length,
        byLabel: Object.fromEntries(
          [...new Set(a.samples.map((s) => s.label))].map((label) => [
            label,
            {
              visibleAcknowledgementMs: stats(a.samples.filter((s) => s.label === label).map((s) => s.paintedAckMs)),
              updateCallReturnedMs: stats(a.samples.filter((s) => s.label === label).map((s) => s.callReturnedMs)),
              settledVisibleStateMs: stats(a.samples.filter((s) => s.label === label).map((s) => s.settledMs)),
            },
          ]),
        ),
        fixture: a.counters,
        longAnimationFrames: a.loaf.length,
        samples: a.samples.map((s) => ({
          label: s.label,
          paintedAckMs: round(s.paintedAckMs),
          callReturnedMs: round(s.callReturnedMs),
          settledMs: Number.isFinite(s.settledMs) ? round(s.settledMs) : null,
        })),
      };
      log(`    ack p50 ${out.action.visibleAcknowledgementMs.p50}ms p95 ${out.action.visibleAcknowledgementMs.p95}ms; `
        + `call returned p50 ${out.action.updateCallReturnedMs.p50}ms; `
        + `felt updated p50 ${out.action.settledVisibleStateMs.p50}ms p95 ${out.action.settledVisibleStateMs.p95}ms`);
    }
    if (wants('animation')) {
      log(`\n[4] all-in -> showdown frame cadence  (${args.hands} hands, ${args.sampleMs}ms each)`);
      out.animation = await measureShowdownAnimation(browser, vp, ctx, args.hands, args.sampleMs);
      log(`    p50 ${out.animation.effectiveFps.p50} fps, worst 5% ${out.animation.effectiveFps.worst5pct} fps`);
    }
  } finally {
    await browser.close();
    await proxy.close();
  }

  out.finishedAt = new Date().toISOString();
  out.machineLoad = loadWatch.stop();
  log(`\nload while measuring: ${out.machineLoad.verdict}`);
  fs.mkdirSync(path.dirname(args.json), { recursive: true });
  fs.writeFileSync(args.json, JSON.stringify(out, null, 2));
  log(`wrote ${path.relative(REPO_ROOT, args.json)}`);
  return 0;
}

main()
  .then((c) => process.exit(c))
  .catch((e) => {
    console.error(`\nFATAL: ${e.message}`);
    if (process.env.PERF_DEBUG) console.error(e.stack);
    process.exit(2);
  });
