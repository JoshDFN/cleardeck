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
import {
  APP_ORIGIN, ASSET_CACHE_DIR, FREEZE_CSS, PRICE_FIXTURE_BODY,
  THIRD_PARTY_STATIC_HOSTS, THIRD_PARTY_VOLATILE_HOSTS, USE_PRICE_FIXTURE,
} from './config.mjs';
import { registerWatchers } from './page-health.mjs';

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

const matchesHost = (host, list) => list.some((h) => host === h || host.endsWith(`.${h}`));

/**
 * Deletes any on-disk cache entry for a volatile host. Earlier runs of this
 * harness cached the CoinGecko quote alongside the fonts; leaving those files
 * around means one wrong `if` away from replaying a stale market price as live.
 * @param {(msg:string)=>void} log
 */
function purgeVolatileCacheEntries(log) {
  let removed = 0;
  for (const name of fs.readdirSync(ASSET_CACHE_DIR)) {
    if (!name.endsWith('.json')) continue;
    const file = path.join(ASSET_CACHE_DIR, name);
    try {
      const { url } = JSON.parse(fs.readFileSync(file, 'utf8'));
      if (!url) continue;
      if (matchesHost(new URL(url).host, THIRD_PARTY_VOLATILE_HOSTS)) {
        fs.rmSync(file);
        removed += 1;
      }
    } catch {
      // An unreadable cache entry is not worth failing a run over.
    }
  }
  if (removed) log(`  purged ${removed} stale volatile third-party cache entr(ies)`);
}

/**
 * Provenance of every volatile (live-fact) third-party response served during a
 * run. run.mjs copies this into the manifest so a reader can tell whether the
 * fiat figure in a PNG was a real quote, a labelled fixture, or absent.
 *
 * @type {Array<{url:string, host:string, mode:'live'|'fixture'|'unavailable',
 *               observedAt:string, body:string|null, note?:string}>}
 */
const volatileObservations = [];

/** @returns {Array<object>} a copy of the volatile-response log. */
export const thirdPartyObservations = () => volatileObservations.map((o) => ({ ...o }));

/** Discards the volatile-response log and any in-memory quote (once per run). */
export const resetThirdPartyObservations = () => {
  volatileObservations.length = 0;
  resetLiveQuotes();
};

/**
 * Serves a VOLATILE third-party response. Never from the disk cache: a market
 * quote read last week is a false statement today, and it would be rendered next
 * to a real on-chain balance in an artifact people treat as evidence.
 */
/**
 * One live read per URL per run, held in memory only, NEVER written to disk.
 *
 * Reading it through `route.fetch()` inside the page's own request loses a race:
 * the app fires `loadPrices()` when the deposit modal opens, and the scene can
 * finish and close the page while that request is still in flight, at which point
 * the APIResponse is disposed and `.body()` throws "Response has been disposed".
 * That failed safe (503, and the reason recorded) but it meant the fiat figure was
 * absent for a reason that had nothing to do with the price feed.
 *
 * Fetching from Node instead, once, and reusing the result for the rest of the run
 * removes the race. It is still a real quote, and it is still dated — the whole
 * run shares one `observedAt`, which is exactly what the manifest reports.
 *
 * @type {Map<string, Promise<{ok:boolean, status?:number, body?:string, observedAt:string, error?:string}>>}
 */
const liveQuoteCache = new Map();

function fetchLiveQuoteOnce(url) {
  if (!liveQuoteCache.has(url)) {
    liveQuoteCache.set(
      url,
      fetch(url, { signal: AbortSignal.timeout(20_000) })
        .then(async (res) => ({
          ok: res.ok,
          status: res.status,
          body: await res.text(),
          observedAt: new Date().toISOString(),
        }))
        .catch((e) => ({
          ok: false,
          error: String(e.message || e),
          observedAt: new Date().toISOString(),
        })),
    );
  }
  return liveQuoteCache.get(url);
}

/** Discards the in-memory live quotes so a new run reads fresh ones. */
const resetLiveQuotes = () => liveQuoteCache.clear();

async function routeVolatile(route, request, host, log) {
  const observedAt = new Date().toISOString();

  if (USE_PRICE_FIXTURE) {
    volatileObservations.push({
      url: request.url(), host, mode: 'fixture', observedAt, body: PRICE_FIXTURE_BODY,
      note: 'SHOTS_PRICE_FIXTURE=1: any fiat figure in this run is a PLACEHOLDER, not a quote',
    });
    return route.fulfill({
      status: 200, headers: { 'content-type': 'application/json' }, body: PRICE_FIXTURE_BODY,
    });
  }

  try {
    const quote = await fetchLiveQuoteOnce(request.url());
    if (!quote.ok) throw new Error(quote.error || `HTTP ${quote.status}`);
    volatileObservations.push({
      url: request.url(), host, mode: 'live', observedAt: quote.observedAt,
      body: quote.body.slice(0, 400),
    });
    return route.fulfill({
      status: quote.status ?? 200,
      headers: { 'content-type': 'application/json' },
      body: quote.body,
    });
  } catch (e) {
    const note = `live fetch failed (${String(e.message).split('\n')[0]})`;
    log(`  volatile third-party fetch failed (${host}): ${note}`);
    log('    serving 503 so the app shows its own "no price" state; a stale quote is never replayed');
    volatileObservations.push({
      url: request.url(), host, mode: 'unavailable', observedAt, body: null, note,
    });
    // 503, not 204: the app's loadPrices() checks response.ok and renders its own
    // priceError state, so the screenshot shows an honest absence of a price.
    return route.fulfill({
      status: 503, headers: { 'content-type': 'application/json' }, body: '{}',
    });
  }
}

/** Serves a STATIC third-party asset from the disk cache, filling it on a miss. */
async function routeStatic(route, request, host, log) {
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
}

/**
 * Intercepts THIRD-PARTY traffic only. Canister traffic (same-origin /api/...)
 * is never intercepted, cached or faked.
 *
 * Static assets are cached to disk and replayed. Volatile facts (the CoinGecko
 * quote) are not — see THIRD_PARTY_VOLATILE_HOSTS in config.mjs for why.
 */
async function installThirdPartyCache(context, { log = () => {} } = {}) {
  fs.mkdirSync(ASSET_CACHE_DIR, { recursive: true });
  purgeVolatileCacheEntries(log);

  await context.route('**/*', async (route) => {
    const request = route.request();
    let host;
    try {
      host = new URL(request.url()).host;
    } catch {
      return route.continue();
    }
    if (matchesHost(host, THIRD_PARTY_VOLATILE_HOSTS)) {
      return routeVolatile(route, request, host, log);
    }
    if (matchesHost(host, THIRD_PARTY_STATIC_HOSTS)) {
      return routeStatic(route, request, host, log);
    }
    return route.continue();
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

/**
 * Attaches console/pageerror capture so scene reports can flag runtime breakage.
 *
 * The arrays are also registered against the page in `page-health.mjs`, which is
 * what turns them from a manifest field nobody reads into a GATE: `shoot()`
 * refuses to photograph a page that threw. See that file for why.
 */
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
  return registerWatchers(page, { consoleErrors, pageErrors, failedRequests });
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
