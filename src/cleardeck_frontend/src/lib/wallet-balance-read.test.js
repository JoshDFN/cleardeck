import { describe, expect, it } from 'vitest';
import { WALLET_BALANCE_READ_FAILED, settleBalanceRead } from './wallet-balance-read.js';
import { chipAffordable, quickChips } from './deposit-amounts.js';

describe('settleBalanceRead: a failed read is UNREAD, never zero', () => {
  it('keeps a successful read as a number with no error', () => {
    expect(settleBalanceRead({ ok: true, value: 1_234_567n })).toEqual({ balance: 1_234_567, error: null });
    expect(settleBalanceRead({ ok: true, value: 0n })).toEqual({ balance: 0, error: null });
  });

  it('a failed read leaves the balance null and says the read failed', () => {
    const out = settleBalanceRead({ ok: false, error: new Error('Failed to fetch') });
    expect(out.balance).toBeNull();
    expect(out.balance).not.toBe(0);
    expect(out.error).toBe(WALLET_BALANCE_READ_FAILED);
    expect(settleBalanceRead(null).balance).toBeNull();
    expect(settleBalanceRead({ ok: true, value: NaN }).balance).toBeNull();
  });

  it('an unread balance disables no quick chip', () => {
    const { balance } = settleBalanceRead({ ok: false });
    const b = balance === null ? null : BigInt(balance);
    expect(chipAffordable({ amount: 200_000_000n, fee: 10_000n, balance: b })).toBe(true);
    const chips = quickChips({ minBuyIn: 1_000_000_000n, fee: 10_000n, balance: b, format: (v) => String(v) });
    expect(chips.length).toBeGreaterThan(0);
    expect(chips.every((c) => !c.disabled)).toBe(true);
  });
});
