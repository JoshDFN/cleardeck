import { describe, expect, it } from 'vitest';
import {
  decimalToSmallest, depositCost, floatToSmallest, formatCashier, formatExact, formatPlain,
  formatUsd, toSmallest, usdValue, withdrawNet,
} from './cashier-format.js';

describe('formatExact / formatPlain', () => {
  it('never rounds a limit', () => {
    expect(formatExact(20_000n)).toBe('0.0002 ICP');
    expect(formatExact(10_000n)).toBe('0.0001 ICP');
    expect(formatExact(30_000n)).toBe('0.0003 ICP');
    expect(formatExact(10_000_000_000n)).toBe('100 ICP');
    expect(formatExact(123_456_789n)).toBe('1.23456789 ICP');
  });

  it('prints sats with a thousands separator', () => {
    expect(formatExact(1_000n, 'BTC')).toBe('1,000 sats');
    expect(formatExact(11n, 'BTC')).toBe('11 sats');
  });

  it('formatPlain is the bare decimal for an input attribute', () => {
    expect(formatPlain(20_000n)).toBe('0.0002');
    expect(formatPlain(200_000_000n)).toBe('2');
    expect(formatPlain(0n)).toBe('0');
  });
});

describe('toSmallest', () => {
  it('floors a Number and passes a BigInt through', () => {
    expect(toSmallest(123.9)).toBe(123n);
    expect(toSmallest(5n)).toBe(5n);
  });

  it('reads junk, null and negatives as nothing', () => {
    expect(toSmallest(null)).toBe(0n);
    expect(toSmallest('abc')).toBe(0n);
    expect(toSmallest(-4)).toBe(0n);
    expect(toSmallest(NaN)).toBe(0n);
  });
});

describe('decimalToSmallest (the withdraw modal\'s exact parse)', () => {
  it('does not lose a unit to a float', () => {
    expect(decimalToSmallest('1.15')).toBe(115_000_000n);
    expect(decimalToSmallest('0.29')).toBe(29_000_000n);
    expect(Math.floor(Number('1.15') * 1e8)).toBe(114_999_999);
  });

  it('reads whole, fractional and dotted forms', () => {
    expect(decimalToSmallest('2')).toBe(200_000_000n);
    expect(decimalToSmallest('.5')).toBe(50_000_000n);
    expect(decimalToSmallest('1.')).toBe(100_000_000n);
    expect(decimalToSmallest('1.234567891')).toBe(123_456_789n);
  });

  it('reads junk as nothing', () => {
    expect(decimalToSmallest('')).toBe(0n);
    expect(decimalToSmallest('abc')).toBe(0n);
    expect(decimalToSmallest(null)).toBe(0n);
  });

  it('MAX round-trips a balance to the e8', () => {
    const balance = 123_456_789n;
    expect(decimalToSmallest(formatPlain(balance))).toBe(balance);
  });
});

describe('floatToSmallest (the deposit modal\'s conversion, unchanged)', () => {
  it('is the float floor the deposit modal always used', () => {
    expect(floatToSmallest('2')).toBe(200_000_000n);
    expect(floatToSmallest('0.0002')).toBe(20_000n);
    expect(floatToSmallest('1.15')).toBe(114_999_999n);
  });

  it('reads junk as nothing rather than throwing', () => {
    expect(floatToSmallest('')).toBe(0n);
    expect(floatToSmallest('abc')).toBe(0n);
    expect(floatToSmallest('-1')).toBe(0n);
  });
});

describe('formatCashier', () => {
  it('prints four decimals with the unit', () => {
    expect(formatCashier(60_000)).toBe('0.0006 ICP');
    expect(formatCashier(1_200_000_000n)).toBe('12.0000 ICP');
    expect(formatCashier(0)).toBe('0.0000 ICP');
  });

  it('an unread balance is a placeholder, never zero', () => {
    expect(formatCashier(null)).toBe('…');
    expect(formatCashier(undefined, 'ICP', { placeholder: '...' })).toBe('...');
  });

  it('keeps the BTC modal\'s scale', () => {
    expect(formatCashier(150_000_000, 'BTC')).toBe('1.5000 BTC');
    expect(formatCashier(2_500, 'BTC')).toBe('2.5K sats');
    expect(formatCashier(11, 'BTC')).toBe('11 sats');
  });
});

describe('usdValue / formatUsd', () => {
  it('converts at the quote and prints under a cent at four places', () => {
    expect(usdValue(60_000, 2.5)).toBeCloseTo(0.0015, 8);
    expect(formatUsd(0.0015)).toBe('~$0.0015');
    expect(formatUsd(12.345)).toBe('~$12.35');
  });

  it('is null without a quote and empty for a non-number', () => {
    expect(usdValue(60_000, null)).toBeNull();
    expect(usdValue(60_000, 0)).toBeNull();
    expect(formatUsd(null)).toBe('');
  });
});

describe('depositCost', () => {
  it('charges the ledger fee twice and credits the amount whole', () => {
    const cost = depositCost({ amount: 20_000n, fee: 10_000n });
    expect(cost).toEqual({ amount: 20_000n, fees: 20_000n, total: 40_000n, credited: 20_000n });
  });
});

describe('withdrawNet', () => {
  it('takes one fee out of what arrives', () => {
    expect(withdrawNet({ amount: 100_000n, fee: 10_000n })).toEqual({
      amount: 100_000n, fee: 10_000n, net: 90_000n, feeDominates: false,
    });
  });

  it('flags a request the fee eats half of, and nets zero at or under the fee', () => {
    expect(withdrawNet({ amount: 20_000n, fee: 10_000n }).feeDominates).toBe(true);
    expect(withdrawNet({ amount: 10_000n, fee: 10_000n }).net).toBe(0n);
    expect(withdrawNet({ amount: 0n, fee: 10_000n }).feeDominates).toBe(false);
  });
});
