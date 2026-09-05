import { describe, expect, it } from 'vitest';
import { claimReceipt, oisyReceipt, walletReceipt } from './deposit-receipts.js';

const format = (v) => `${(Number(v) / 1e8).toFixed(4)} ICP`;
const fiatOf = (v) => `~$${(Number(v) / 1e8 * 2.5).toFixed(4)}`;
const FEE = 10_000n;

describe('walletReceipt', () => {
  it('names the amount, both ledger fees and the balance now, in that order', () => {
    const r = walletReceipt({
      amountSmallest: 50_000n, amountText: '0.0005 ICP',
      ledgerFees: FEE * 2n, ledgerFeesText: '0.0002 ICP', balance: 1_050_000n, format, fiatOf,
    });
    expect(r.title).toBe('Deposited');
    expect(r.lead).toContain('0.0005 ICP');
    expect(r.rows.map((x) => x.id)).toEqual(['sent', 'fees', 'balance']);
    expect(r.rows[0].value).toBe('0.0005 ICP');
    expect(r.rows[1].value).toBe('0.0002 ICP');
    expect(r.rows[2]).toMatchObject({ value: '0.0105 ICP', strong: true });
  });

  it('carries a fiat figure on every row', () => {
    const r = walletReceipt({
      amountSmallest: 50_000n, amountText: '0.0005 ICP',
      ledgerFees: FEE * 2n, ledgerFeesText: '0.0002 ICP', balance: 1_050_000n, format, fiatOf,
    });
    for (const row of r.rows) expect(row.fiat).toMatch(/^~\$/);
  });

  it('shows no fiat when there is no quote', () => {
    const r = walletReceipt({
      amountSmallest: 50_000n, amountText: '0.0005 ICP',
      ledgerFees: FEE * 2n, ledgerFeesText: '0.0002 ICP', balance: 1_050_000n, format, fiatOf: () => null,
    });
    for (const row of r.rows) expect(row.fiat).toBeNull();
  });
});

describe('oisyReceipt', () => {
  it('says who paid and who is credited, with the amount and the balance', () => {
    const r = oisyReceipt({
      amountSmallest: 50_000n, amountText: '0.0005 ICP', balance: 60_000n,
      paidBy: 'abcde-…-cai', creditedTo: 'fghij-…-cai', format, fiatOf,
    });
    expect(r.lead).toBe('Paid by OISY abcde-…-cai, credited to your signed-in identity fghij-…-cai.');
    expect(r.rows.map((x) => x.id)).toEqual(['sent', 'balance']);
    expect(r.rows[1].value).toBe('0.0006 ICP');
  });
});

describe('claimReceipt', () => {
  it('credits what arrived less one network charge', () => {
    const r = claimReceipt({ arrived: 50_000n, fee: FEE, feeText: '0.0001 ICP', balance: 240_000n, format, fiatOf });
    expect(r.rows.map((x) => x.id)).toEqual(['arrived', 'fee', 'credited', 'balance']);
    expect(r.rows[0].value).toBe('0.0005 ICP');
    expect(r.rows[1].value).toBe('0.0001 ICP');
    expect(r.rows[2].value).toBe('0.0004 ICP');
    expect(r.rows[3]).toMatchObject({ value: '0.0024 ICP', strong: true });
  });

  it('shows only the balance when the card never read the address', () => {
    for (const arrived of [null, undefined, 0n]) {
      const r = claimReceipt({ arrived, fee: FEE, feeText: '0.0001 ICP', balance: 240_000n, format, fiatOf });
      expect(r.rows.map((x) => x.id)).toEqual(['balance']);
    }
  });
});
