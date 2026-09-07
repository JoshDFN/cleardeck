import { describe, expect, it } from 'vitest';
import {
  fiatPair, fiatUsd, formatAmount, formatBlinds, formatBuyIn, formatUsd, shortHash, shortId,
  stakeTier, tableFormat, tierDefinition, unitOf,
} from './lobby-format.js';

const ICP = (n) => Math.round(n * 100_000_000);

describe('formatAmount', () => {
  it('prints ICP to two decimals from a cent up and four below', () => {
    expect(formatAmount(ICP(0.05), 'ICP')).toBe('0.05');
    expect(formatAmount(ICP(12.5), 'ICP')).toBe('12.50');
    expect(formatAmount(ICP(0.0006), 'ICP')).toBe('0.0006');
    expect(formatAmount(0, 'ICP')).toBe('0.00');
    expect(formatAmount(ICP(1500), 'ICP')).toBe('1.5K');
  });

  it('prints sats integral with K/M and BTC above 1e8', () => {
    expect(formatAmount(200, 'BTC')).toBe('200');
    expect(formatAmount(10_000, 'BTC')).toBe('10K');
    expect(formatAmount(2_500_000, 'BTC')).toBe('2.5M');
    expect(formatAmount(150_000_000, 'BTC')).toBe('1.50 BTC');
  });

  it('formats blinds and a buy-in range with the word "to" (no dash in copy; two numbers for the harness)', () => {
    expect(formatBlinds(ICP(0.05), ICP(0.10), 'ICP')).toBe('0.05/0.10');
    expect(formatBuyIn(ICP(2), ICP(10), 'ICP')).toBe('2.00 to 10.00');
    expect(formatBuyIn(ICP(2), ICP(10), 'ICP')).not.toMatch(/[\u2013\u2014-]/);
    // the harness's own number scan reads exactly two figures, the second positive
    expect(formatBuyIn(ICP(2), ICP(10), 'ICP').match(/-?\d[\d.,]*\s*[KM]?/g).map((t) => t.trim())).toEqual(['2.00', '10.00']);
    expect(unitOf('BTC')).toBe('sats');
    expect(unitOf('ICP')).toBe('ICP');
  });
});

describe('stakeTier and tierDefinition', () => {
  it('tiers by small blind in the table unit', () => {
    expect(stakeTier(ICP(0.01), 'ICP')).toBe('Micro');
    expect(stakeTier(ICP(0.05), 'ICP')).toBe('Low');
    expect(stakeTier(ICP(0.5), 'ICP')).toBe('Mid');
    expect(stakeTier(ICP(5), 'ICP')).toBe('Nosebleed');
    expect(stakeTier(100, 'BTC')).toBe('Micro');
    expect(stakeTier(50_000, 'BTC')).toBe('Nosebleed');
  });

  it('defines a tier in dollars only when a quote exists', () => {
    expect(tierDefinition('Micro', null)).toBe('Micro: small blind up to 0.02 ICP');
    expect(tierDefinition('Low', 5)).toBe('Low: small blind up to 0.1 ICP (about $0.50)');
    expect(tierDefinition('Nosebleed', 5)).toBe('Nosebleed: small blind above 2 ICP');
  });
});

describe('identifiers', () => {
  it('shortens a canister id and a hash without touching short ones', () => {
    expect(shortId('4fbx2-kt777-77775-aaabq-cai')).toBe('4fbx2…aaabq-cai');
    expect(shortId('short')).toBe('short');
    expect(shortId(null)).toBe('');
    expect(shortHash('1ba47aa1deadbeefdeadbeef00f984cf')).toBe('1ba47aa1…f984cf');
  });

  it('names the table shape', () => {
    expect(tableFormat(2)).toBe('Heads-up');
    expect(tableFormat(6n)).toBe('6-max');
  });
});

describe('fiat', () => {
  const prices = { icpUsd: 6.25, btcUsd: 100_000 };

  it('converts e8s and sats by their own quote', () => {
    expect(fiatUsd(ICP(2), 'ICP', prices)).toBeCloseTo(12.5);
    expect(fiatUsd(100, 'BTC', prices)).toBeCloseTo(0.1);
  });

  it('is null without a quote for that currency', () => {
    expect(fiatUsd(ICP(2), 'ICP', null)).toBeNull();
    expect(fiatUsd(ICP(2), 'ICP', { icpUsd: null, btcUsd: 1 })).toBeNull();
    expect(fiatUsd(100, 'BTC', { icpUsd: 6, btcUsd: 0 })).toBeNull();
  });

  it('prints dollars at the precision a player reads, rounded not floored', () => {
    expect(formatUsd(0.0625)).toBe('$0.06');
    expect(formatUsd(0.125)).toBe('$0.13');
    expect(formatUsd(12.5)).toBe('$12.50');
    expect(formatUsd(625)).toBe('$625');
    expect(formatUsd(0.004)).toBe('$0.0040');
    expect(formatUsd(0)).toBe('$0.00');
    expect(formatUsd(NaN)).toBe('');
  });

  it('renders a pair all-or-nothing', () => {
    expect(fiatPair(ICP(0.01), ICP(0.02), 'ICP', prices)).toEqual({ low: '$0.06', high: '$0.13', joiner: '/' });
    expect(fiatPair(ICP(2), ICP(10), 'ICP', prices, 'to')).toEqual({ low: '$12.50', high: '$62.50', joiner: 'to' });
    expect(fiatPair(ICP(2), ICP(10), 'ICP', null)).toBeNull();
  });
});
