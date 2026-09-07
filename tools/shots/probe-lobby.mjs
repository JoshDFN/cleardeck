#!/usr/bin/env node
// THE LOBBY'S OTHER STATES, PHOTOGRAPHED (a probe, not a gate).
//
// The `lobby` scene files one still per viewport: a signed-out visitor, the
// tables as the canisters report them. This probe photographs the states that
// still is not: the error toast up, the FULL TERMS overlay, the page scrolled
// to its foot (the trust bar is sticky and must still be on screen), an
// invite link (`?table=<id>`) opening its table, and the lobby signed in.
//
// Run AFTER `node tools/shots/run.mjs` has deployed the tree you want measured
// (this does not build), never concurrently with it or another probe. Writes
// `artifacts/screens/<sha>/probe/PROBE-lobby-*.png` and `PROBE-lobby.json`,
// exits 1 on any failed assertion.

import fs from 'node:fs';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, REPO_ROOT, VIEWPORTS } from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { gitShortSha, runDirs } from './lib/capture.mjs';
import {
  devLogin, getAppOrigin, launchBrowser, newContext, openApp, setAppOrigin, settle, watchPage,
} from './lib/browser.mjs';
import { foldProtectedNotices, probeProtectedNotices } from './lib/protected-notices.mjs';
import { TOAST_MESSAGES, raiseToast, removeToast } from './lib/toast-notices.mjs';
import { waitForLobbySettled } from './scenarios/_shared.mjs';

const log = (msg) => console.log(msg);

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

const rectOf = (page, selector) => page.evaluate((sel) => {
  const el = document.querySelector(sel);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height), bottom: Math.round(r.bottom) };
}, selector);

async function shoot(page, outDir, name, vp) {
  const file = path.join(outDir, `PROBE-lobby-${name}-${vp.name}.png`);
  await settle(page);
  await page.screenshot({ path: file, fullPage: false });
  return path.relative(REPO_ROOT, file);
}

async function probeViewport(vp, ids, browser, outDir) {
  const context = await newContext(browser, vp, { log });
  const page = await context.newPage();
  watchPage(page);
  const problems = [];
  const states = {};
  try {
    await openApp(page);
    await waitForLobbySettled(page);
    await settle(page);

    // ---- 1. the error toast, up ------------------------------------------
    const toast = await raiseToast(page, TOAST_MESSAGES[0].text);
    const notices = foldProtectedNotices(await probeProtectedNotices(page), 'lobby with the toast up');
    states.toast = {
      file: await shoot(page, outDir, 'toast', vp),
      rect: toast.rect,
      bannerRect: toast.bannerRect,
      noticesOnScreen: `${notices.onScreen}/${notices.total}`,
    };
    if (!notices.ok) problems.push(...notices.problems);
    if (toast.rect.x < 0 || toast.rect.right > vp.width) problems.push('the toast overflows the viewport horizontally');
    if (toast.bannerRect && toast.rect.y < toast.bannerRect.bottom) problems.push('the toast starts above the trust bar\'s bottom edge');
    await removeToast(page);
    log(`  toast: ${states.toast.rect.w}x${states.toast.rect.h} at y ${states.toast.rect.y}, notices ${states.toast.noticesOnScreen}`);

    // ---- 1b. the app's OWN failure toast, with its Retry ------------------
    // Every canister call is aborted at the transport and the page reloaded
    // (the toast-notices scene's reproducer): the toast must read the humane
    // sentence, carry the raw agent text as a detail line and a Retry, and
    // Retry must clear it once the transport is back.
    const API_ROUTES = ['**/api/v2/**', '**/api/v3/**'];
    for (const route of API_ROUTES) await page.route(route, (r) => r.abort('failed'));
    await page.reload({ waitUntil: 'domcontentloaded', timeout: 60_000 });
    await page.waitForSelector('.toast.error', { timeout: 20_000 });
    await settle(page);
    const realToast = await page.evaluate(() => {
      const el = document.querySelector('.toast.error');
      const r = el.getBoundingClientRect();
      const text = (node) => (node ? node.textContent.replace(/\s+/g, ' ').trim() : null);
      return {
        message: text(el.querySelector('span')),
        detail: text(el.querySelector('.toast-detail')),
        action: text(el.querySelector('.toast-action')),
        rect: { x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) },
      };
    });
    const realNotices = foldProtectedNotices(await probeProtectedNotices(page), 'the lobby under its own failure toast');
    states.realToast = { file: await shoot(page, outDir, 'real-toast', vp), ...realToast, noticesOnScreen: `${realNotices.onScreen}/${realNotices.total}` };
    if (!realNotices.ok) problems.push(...realNotices.problems);
    if (!/^Could not reach the tables\. Retrying in \d+ s\./.test(realToast.message || '')) problems.push(`the failure toast does not read the humane sentence (read "${realToast.message}")`);
    if (!realToast.detail) problems.push('the failure toast carries no detail line with the raw text');
    if (realToast.action !== 'Retry') problems.push(`the failure toast has no Retry (found "${realToast.action}")`);
    for (const route of API_ROUTES) await page.unroute(route);
    await page.click('.toast.error .toast-action');
    const cleared = await page.waitForSelector('.toast.error', { state: 'detached', timeout: 20_000 }).then(() => true).catch(() => false);
    states.realToast.clearedByRetry = cleared;
    if (!cleared) problems.push('Retry did not clear the failure toast once the transport was back');
    await waitForLobbySettled(page);
    await settle(page);
    log(`  real toast: "${realToast.message}" + Retry, detail ${realToast.detail ? 'present' : 'MISSING'}, ${realToast.rect.w}x${realToast.rect.h} at y ${realToast.rect.y}, notices ${states.realToast.noticesOnScreen}, cleared by Retry: ${cleared}`);

    // ---- 1c. a failed first read WITH an invite link ----------------------
    // Round 2's defect: `?table=` ran after a FAILED first read, replaced the
    // failure sentence with "That invite link points to a table this lobby
    // does not list" while the previous failure's stack trace stayed under
    // it, and the list flipped to "Nothing is wrong with your connection".
    // Now the link waits for a successful read: the failure toast keeps its
    // own sentence, detail and Retry, the list says the read failed, and the
    // table opens once Retry succeeds.
    const linkedTable = requireId(ids, 'table_2');
    for (const route of API_ROUTES) await page.route(route, (r) => r.abort('failed'));
    await page.goto(`${getAppOrigin()}/?table=${linkedTable}`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
    await page.waitForSelector('.toast.error', { timeout: 20_000 });
    await page.waitForTimeout(1500);
    await settle(page);
    const failedLink = await page.evaluate(() => {
      const el = document.querySelector('.toast.error');
      const text = (node) => (node ? node.textContent.replace(/\s+/g, ' ').trim() : null);
      const empty = document.querySelector('.list-pane .empty');
      return {
        message: text(el?.querySelector('span')),
        detail: text(el?.querySelector('.toast-detail')),
        action: text(el?.querySelector('.toast-action')),
        emptyKind: empty ? empty.getAttribute('data-kind') : null,
        search: window.location.search,
      };
    });
    states.failedLink = { file: await shoot(page, outDir, 'failed-link', vp), ...failedLink };
    const humane = /^(?:Could not reach the tables|The table list could not be read)\. Retrying in \d+ s\./;
    if (!humane.test(failedLink.message || '')) problems.push(`with ?table= and a dead transport the toast does not read the failure sentence (read "${failedLink.message}")`);
    if (/invite link/i.test(failedLink.message || '')) problems.push('the invite-link message replaced the failure toast after a failed first read');
    if (!failedLink.detail) problems.push('the failure toast under ?table= lost its detail line');
    if (failedLink.action !== 'Retry') problems.push(`the failure toast under ?table= has no Retry (found "${failedLink.action}")`);
    if (failedLink.emptyKind !== 'failed') problems.push(`the empty list under a failed read + ?table= reads kind "${failedLink.emptyKind}", not "failed"`);
    if (!failedLink.search.includes(`table=${linkedTable}`)) problems.push(`the invite link was dropped from the URL before a read succeeded (${failedLink.search})`);
    for (const route of API_ROUTES) await page.unroute(route);
    await page.click('.toast.error .toast-action');
    const openedAfterRetry = await page.waitForSelector('.felt', { timeout: 60_000 }).then(() => true).catch(() => false);
    states.failedLink.openedAfterRetry = openedAfterRetry;
    if (!openedAfterRetry) problems.push('the invite link did not open its table once Retry succeeded');
    log(`  failed read + invite link: "${failedLink.message}", detail ${failedLink.detail ? 'present' : 'MISSING'}, list kind ${failedLink.emptyKind}, opened after Retry: ${openedAfterRetry}`);
    await openApp(page);
    await waitForLobbySettled(page);
    await settle(page);

    // ---- 2. FULL TERMS ------------------------------------------------------
    await page.click('.banner-strip');
    await page.waitForSelector('.banner-content', { timeout: 10_000 });
    const terms = foldProtectedNotices(await probeProtectedNotices(page), 'the FULL TERMS overlay');
    states.terms = { file: await shoot(page, outDir, 'terms', vp), noticesOnScreen: `${terms.onScreen}/${terms.total}` };
    if (!terms.ok) problems.push(...terms.problems);
    await page.click('.banner-close');
    await page.waitForSelector('.banner-content', { state: 'detached', timeout: 10_000 });
    log(`  terms overlay: notices ${states.terms.noticesOnScreen}`);

    // ---- 3. scrolled to the foot: the sticky trust bar stays on screen ------
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
    await page.waitForTimeout(300);
    const scrolled = foldProtectedNotices(await probeProtectedNotices(page), 'the lobby scrolled to its foot');
    const bar = await rectOf(page, '.alpha-warning-banner');
    states.scrolled = {
      file: await shoot(page, outDir, 'scrolled', vp),
      scrollY: await page.evaluate(() => Math.round(window.scrollY)),
      barRect: bar,
      noticesOnScreen: `${scrolled.onScreen}/${scrolled.total}`,
    };
    if (!scrolled.ok) problems.push(...scrolled.problems);
    if (!bar || bar.y !== 0) problems.push(`the trust bar is not pinned to the top when scrolled (y ${bar?.y})`);
    log(`  scrolled ${states.scrolled.scrollY} px: bar at y ${bar?.y}, notices ${states.scrolled.noticesOnScreen}`);
    await page.evaluate(() => window.scrollTo(0, 0));

    // ---- 4. the invite link opens its table -------------------------------
    const tableId = requireId(ids, 'table_2');
    await page.goto(`${getAppOrigin()}/?table=${tableId}`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
    await page.waitForSelector('.felt', { timeout: 60_000 });
    await page.waitForTimeout(500);
    const search = await page.evaluate(() => window.location.search);
    const onTable = await page.evaluate(() => !!document.querySelector('header.compact'));
    states.deepLink = { file: await shoot(page, outDir, 'deeplink', vp), search, onTable };
    if (!onTable) problems.push('the invite link did not open the table view');
    if (!search.includes(`table=${tableId}`)) problems.push(`the URL lost the table while it was open (${search})`);
    await page.click('.back-btn');
    await page.waitForSelector('.tables-list', { timeout: 30_000 });
    const searchAfter = await page.evaluate(() => window.location.search);
    states.deepLink.searchAfterBack = searchAfter;
    if (searchAfter !== '') problems.push(`the URL still carries a table on the lobby (${searchAfter})`);
    log(`  invite link: table view ${onTable}, search "${search}", after Back "${searchAfter}"`);

    // ---- 5. the sign-in button while Internet Identity opens ---------------
    // Desktop only: the hero's button is the phone header's on a phone. The
    // II popup's document is held at the network layer so the button stays in
    // its "Opening Internet Identity…" state long enough to photograph; the
    // popup is then closed, which is the AuthClient's own interrupt path.
    if (vp.name === 'desktop') {
      const appHost = new URL(getAppOrigin()).hostname;
      const hold = (url) => url.hostname !== appHost && url.hostname.endsWith('.localhost');
      await context.route(hold, () => new Promise(() => {}));
      const popupPromise = context.waitForEvent('page', { timeout: 15_000 }).catch(() => null);
      await page.click('.intro .btn.primary');
      const popup = await popupPromise;
      await page.waitForTimeout(700);
      const label = (await page.locator('.intro .btn.primary').textContent().catch(() => '') || '').trim();
      const errorLine = (await page.locator('.intro-error').textContent().catch(() => '') || '').trim();
      const widthAfter = await rectOf(page, '.intro .btn.primary');
      states.signingIn = { file: await shoot(page, outDir, 'signing-in', vp), label, errorLine, popupOpened: Boolean(popup), rect: widthAfter };
      // A local stack with no Internet Identity canister rejects before any
      // popup (auth.js, T-39): the photographed state is then the failure
      // line under a button that kept its width, which is the other half of
      // the same claim. A stack WITH an II id must show the opening state.
      const opening = /^Opening Internet Identity/.test(label);
      if (!opening && !errorLine) problems.push(`the hero button neither read "Opening Internet Identity…" nor showed a failure line (read "${label}")`);
      if (popup) await popup.close().catch(() => {});
      await context.unroute(hold);
      await page.waitForTimeout(500);
      log(`  signing in: button "${label}", popup ${popup ? 'opened' : 'not seen'}${errorLine ? `, failure line "${errorLine}" (no II canister on this stack, T-39)` : ''}`);
    }

    // ---- 6. signed in ------------------------------------------------------
    await openApp(page);
    await waitForLobbySettled(page);
    await devLogin(page, 1);
    await page.waitForSelector('.intro-slim', { timeout: 30_000 });
    states.signedIn = { file: await shoot(page, outDir, 'signed-in', vp) };
    log('  signed in: slim claims row, Sit on every open card');
  } finally {
    await page.close();
    await context.close();
  }
  return { viewport: vp.name, ok: problems.length === 0, problems, states };
}

async function main() {
  const args = parseArgs(process.argv);
  await requireReplica();
  const ids = readLocalIds();
  const frontendId = requireId(ids, 'frontend');
  setAppOrigin(`http://${frontendId}.${GATEWAY_HOST}:${GATEWAY_PORT}`);
  const { shaDir } = runDirs(gitShortSha());
  const outDir = path.join(shaDir, 'probe');
  fs.mkdirSync(outDir, { recursive: true });

  const browser = await launchBrowser({ log });
  const out = [];
  try {
    for (const name of args.viewports) {
      const vp = VIEWPORTS[name];
      if (!vp) throw new Error(`Unknown viewport ${name}`);
      log(`\n[${vp.name}]`);
      out.push(await probeViewport(vp, ids, browser, outDir));
    }
  } finally {
    await browser.close();
  }
  const report = path.join(outDir, 'PROBE-lobby.json');
  fs.writeFileSync(report, JSON.stringify(out, null, 2));
  log(`\nreport: ${path.relative(REPO_ROOT, report)}`);
  const bad = out.filter((r) => !r.ok);
  if (bad.length) {
    for (const b of bad) log(`FAILED at ${b.viewport}: ${b.problems.join(' | ')}`);
    return 1;
  }
  log('the lobby: toast under the trust bar, FULL TERMS a superset, the bar pinned when scrolled, the invite link opens its table, the sign-in button says what it is doing, signed in the cards say Sit');
  return 0;
}

main().then((c) => process.exit(c)).catch((e) => {
  console.error(`\nFATAL: ${e.message}`);
  if (process.env.SHOTS_DEBUG) console.error(e.stack);
  process.exit(2);
});
