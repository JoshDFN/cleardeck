import { beforeEach, describe, expect, it } from 'vitest';
import { PRICE_URL, loadPrices, parsePriceBody, resetPricesForTests } from './prices.js';

const body = { 'internet-computer': { usd: 6.25 }, bitcoin: { usd: 100_000 } };

describe('parsePriceBody', () => {
  it('reads both figures off CoinGecko\'s shape', () => {
    expect(parsePriceBody(body, 1000)).toEqual({ icpUsd: 6.25, btcUsd: 100_000, readAt: 1000 });
  });

  it('keeps the one figure that is usable and nulls the other', () => {
    expect(parsePriceBody({ 'internet-computer': { usd: 6.25 }, bitcoin: { usd: '1' } }, 1))
      .toEqual({ icpUsd: 6.25, btcUsd: null, readAt: 1 });
  });

  it('is null for junk, zero, negative and non-object bodies', () => {
    expect(parsePriceBody(null)).toBeNull();
    expect(parsePriceBody('6.25')).toBeNull();
    expect(parsePriceBody({ 'internet-computer': { usd: 0 } })).toBeNull();
    expect(parsePriceBody({ 'internet-computer': { usd: -3 } })).toBeNull();
    expect(parsePriceBody({})).toBeNull();
  });
});

describe('loadPrices', () => {
  beforeEach(() => resetPricesForTests());

  const fetchOk = (calls) => async (url) => {
    calls.push(url);
    return { ok: true, json: async () => body };
  };

  it('fetches the cashier\'s URL once and caches the result', async () => {
    const calls = [];
    let t = 1000;
    const now = () => t;
    const first = await loadPrices({ fetchImpl: fetchOk(calls), now });
    const second = await loadPrices({ fetchImpl: fetchOk(calls), now });
    expect(first).toEqual({ icpUsd: 6.25, btcUsd: 100_000, readAt: 1000 });
    expect(second).toBe(first);
    expect(calls).toEqual([PRICE_URL]);
    t += 10 * 60_000;
    await loadPrices({ fetchImpl: fetchOk(calls), now });
    expect(calls.length).toBe(2);
  });

  it('shares one in-flight request', async () => {
    const calls = [];
    const [a, b] = await Promise.all([
      loadPrices({ fetchImpl: fetchOk(calls) }),
      loadPrices({ fetchImpl: fetchOk(calls) }),
    ]);
    expect(a).toEqual(b);
    expect(calls.length).toBe(1);
  });

  it('resolves null on a failed read and never throws', async () => {
    const failing = async () => ({ ok: false, status: 503, json: async () => ({}) });
    expect(await loadPrices({ fetchImpl: failing })).toBeNull();
    const throwing = async () => { throw new Error('offline'); };
    expect(await loadPrices({ fetchImpl: throwing })).toBeNull();
    expect(await loadPrices({ fetchImpl: null })).toBeNull();
  });
});
