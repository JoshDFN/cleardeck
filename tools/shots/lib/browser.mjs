// Playwright plumbing: a deterministic browser pointed at the real app.
//
// The only things intercepted here are THIRD-PARTY assets (Google Fonts,
// dicebear avatars, the CoinGecko price ticker), which are cached to disk so a
// run is repeatable and works offline. Canister traffic is never intercepted,
// never cached and never faked.

import { chromium } from 'playwright';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { APP_ORIGIN, ASSET_CACHE_DIR, FREEZE_CSS, THIRD_PARTY_HOSTS } from './config.mjs';

export async function launchBrowser({ log = () => {} } = {}) {
  // The full Chromium build renders backdrop-filter/blur the way a user sees it;
  // fall back to the default headless shell if that build is not cached.
  try {
    const browser = await chromium.launch({ headless: true, channel: 'chromium' });
    log('  chromium: bundled full build (new headless)');
    return browser;
  } catch (e) {
    log(`  chromium channel unavailable (${e.message.split('\n')[0]}), using default headless`);
    return chromium.launch({ headless: true });
  }
}

function cachePathFor(url) {
  const hash = crypto.createHash('sha1').update(url).digest('hex');
  return path.join(ASSET_CACHE_DIR, `${hash}.json`);
}

/**
 * Caches third-party responses to disk and replays them on later runs.
 * Canister traffic (same-origin /api/...) is untouched.
 */
async function installThirdPartyCache(context, { log = () => {} } = {}) {
  fs.mkdirSync(ASSET_CACHE_DIR, { recursive: true });

  await context.route('**/*', async (route) => {
    const request = route.request();
    let host;
    try {
      host = new URL(request.url()).host;
    } catch {
      return route.continue();
    }
    if (!THIRD_PARTY_HOSTS.some((h) => host === h || host.endsWith(`.${h}`))) {
      return route.continue();
    }

    const file = cachePathFor(request.url());
    if (fs.existsSync(file)) {
      const cached = JSON.parse(fs.readFileSync(file, 'utf8'));
      return route.fulfill({
        status: cached.status,
        headers: cached.headers,
        body: Buffer.from(cached.bodyBase64, 'base64'),
      });
    }

    try {
      const response = await route.fetch({ timeout: 20_000 });
      const body = await response.body();
      const headers = response.headers();
      delete headers['content-encoding'];
      delete headers['content-length'];
      fs.writeFileSync(
        file,
        JSON.stringify({
          url: request.url(),
          status: response.status(),
          headers,
          bodyBase64: body.toString('base64'),
        }),
      );
      return route.fulfill({ status: response.status(), headers, body });
    } catch (e) {
      log(`  third-party fetch failed (${host}): ${e.message.split('\n')[0]}`);
      return route.fulfill({ status: 204, body: '' });
    }
  });
}

/**
 * @param {import('playwright').Browser} browser
 * @param {{name:string,width:number,height:number,deviceScaleFactor:number,isMobile:boolean}} vp
 * @param {{recordVideoDir?:string, log?:Function}} [opts]
 */
export async function newContext(browser, vp, { recordVideoDir, log = () => {} } = {}) {
  const context = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: vp.deviceScaleFactor,
    isMobile: vp.isMobile,
    hasTouch: vp.isMobile,
    locale: 'en-US',
    timezoneId: 'UTC',
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    forcedColors: 'none',
    ...(recordVideoDir
      ? { recordVideo: { dir: recordVideoDir, size: { width: vp.width, height: vp.height } } }
      : {}),
  });

  // Deterministic client-side settings, applied through the same localStorage
  // keys the app's own UI writes (mute toggle, avatar picker).
  await context.addInitScript(() => {
    try {
      localStorage.setItem('poker_sound_muted', 'true');
      localStorage.setItem('poker_avatar_style', 'bottts');
      localStorage.setItem('lobby_view', 'list');
      localStorage.removeItem('poker_custom_name');
    } catch {}
    // Seeded PRNG so anything random-driven renders the same every run.
    let seed = 0x2f6e2b1;
    Math.random = () => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed / 0x7fffffff;
    };
  });

  await installThirdPartyCache(context, { log });
  return context;
}

/** Attaches console/pageerror capture so scene reports can flag runtime breakage. */
export function watchPage(page) {
  const consoleErrors = [];
  const pageErrors = [];
  const failedRequests = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') consoleErrors.push(msg.text().slice(0, 400));
  });
  page.on('pageerror', (err) => pageErrors.push(String(err.message).slice(0, 400)));
  page.on('requestfailed', (req) => {
    failedRequests.push(`${req.method()} ${req.url()} :: ${req.failure()?.errorText}`);
  });
  return { consoleErrors, pageErrors, failedRequests };
}

/** Records every canister id the page actually talked to (runtime wiring proof). */
export function watchCanisterCalls(page) {
  const seen = new Set();
  page.on('request', (req) => {
    const m = req.url().match(/\/api\/v\d+\/canister\/([a-z0-9-]+)\//);
    if (m) seen.add(m[1]);
  });
  return seen;
}

/**
 * Origin the app is served from. Defaults to the 4943 port shim; run.mjs
 * overrides it when the gateway already listens on the port the app hardcodes,
 * in which case no shim is needed and the asset canister is reached directly.
 */
let appOrigin = APP_ORIGIN;
export const setAppOrigin = (origin) => { appOrigin = origin; };
export const getAppOrigin = () => appOrigin;

export async function openApp(page) {
  await page.goto(appOrigin, { waitUntil: 'domcontentloaded', timeout: 60_000 });
  await page.waitForSelector('.brand', { timeout: 60_000 });
}

/** Clicks the app's own local-dev login (real Ed25519 identity, real code path). */
export async function devLogin(page, playerNum) {
  await page.waitForSelector('.wallet-btn.dev', { timeout: 30_000 });
  await page.click('.wallet-btn.dev');
  await page.waitForSelector('.dev-menu', { timeout: 15_000 });
  await page.click(`.dev-menu button:text-is("Player ${playerNum}")`);
  await page.waitForSelector('.wallet-btn.connected', { timeout: 30_000 });
}

/** Clicks the lobby row for a table by its lobby-registered name. */
export async function enterTable(page, tableName) {
  const row = page.locator('tr', { has: page.locator('.table-name', { hasText: tableName }) });
  await row.first().waitFor({ timeout: 30_000 });
  await row.first().click();
  await page.waitForSelector('.poker-table', { timeout: 30_000 });
}

/**
 * Freezes animations and waits for fonts + two frames so a still is stable.
 * This is cosmetic only; no DOM content is altered.
 */
export async function settle(page, { extraFrames = 2 } = {}) {
  await page.addStyleTag({ content: FREEZE_CSS }).catch(() => {});
  await page.evaluate(async (frames) => {
    if (document.fonts?.ready) await document.fonts.ready;
    for (let i = 0; i < frames; i += 1) {
      await new Promise((r) => requestAnimationFrame(() => r(undefined)));
    }
  }, extraFrames);
}

/**
 * Waits until a DOM text node reads exactly `target`, so live countdowns land on
 * the same digits every run. Returns the value actually observed.
 */
export async function stabilizeTimer(page, selector, target, timeoutMs = 40_000) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    last = await page.locator(selector).first().textContent().catch(() => null);
    if (last && last.trim() === target) return { matched: true, value: last.trim() };
    await page.waitForTimeout(200);
  }
  return { matched: false, value: last ? last.trim() : null };
}
