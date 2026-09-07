// One quote for the whole page.
//
// The cashier (DepositModal.svelte) has always fetched CoinGecko itself. The
// lobby wants the same figure for its fiat hints, and two fetches of the same
// URL on one page is one too many, so this module owns the read: one in-flight
// promise, one cached result, the same URL the cashier uses (the screenshot
// harness intercepts that exact URL and records the quote it served, which is
// what lets every fiat figure on screen be asserted against a real number).
//
// A quote that could not be read is `null`, and every caller renders nothing in
// that case. A missing fiat hint is honest; a stale or invented one is not.

import logger from './logger.js';

export const PRICE_URL =
  'https://api.coingecko.com/api/v3/simple/price?ids=internet-computer,bitcoin&vs_currencies=usd';

/** How long a served quote is reused before it is read again. */
export const PRICE_TTL_MS = 5 * 60_000;

/**
 * @typedef {object} Prices
 * @property {number|null} icpUsd
 * @property {number|null} btcUsd
 * @property {number} readAt   Date.now() when the body was parsed
 */

/**
 * Turns CoinGecko's body into the two figures, or null when neither is a
 * usable number. Pure, so it is unit-tested against real and hostile bodies.
 *
 * @param {unknown} body
 * @param {number} [now]
 * @returns {Prices|null}
 */
export function parsePriceBody(body, now = Date.now()) {
  if (!body || typeof body !== 'object') return null;
  const num = (v) => (typeof v === 'number' && Number.isFinite(v) && v > 0 ? v : null);
  const icpUsd = num(body['internet-computer']?.usd);
  const btcUsd = num(body['bitcoin']?.usd);
  if (icpUsd === null && btcUsd === null) return null;
  return { icpUsd, btcUsd, readAt: now };
}

let cached = null;
let inFlight = null;

/**
 * The current quote, read at most once per TTL. Never throws: a network or
 * parse failure resolves to null and is logged at debug level, because a
 * missing price is an expected state of this page, not an error a player can
 * act on.
 *
 * @param {{fetchImpl?: typeof fetch, now?: () => number}} [deps]
 * @returns {Promise<Prices|null>}
 */
export async function loadPrices({ fetchImpl = globalThis.fetch, now = Date.now } = {}) {
  if (cached && now() - cached.readAt < PRICE_TTL_MS) return cached;
  if (inFlight) return inFlight;
  if (typeof fetchImpl !== 'function') return null;
  inFlight = (async () => {
    try {
      const res = await fetchImpl(PRICE_URL);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const parsed = parsePriceBody(await res.json(), now());
      if (parsed) cached = parsed;
      return parsed;
    } catch (e) {
      logger.debug('price quote unavailable', e);
      return null;
    } finally {
      inFlight = null;
    }
  })();
  return inFlight;
}

/** Test hook: forget the cached quote. */
export function resetPricesForTests() {
  cached = null;
  inFlight = null;
}
