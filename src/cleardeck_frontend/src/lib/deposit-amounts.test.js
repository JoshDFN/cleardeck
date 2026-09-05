import { describe, expect, it } from 'vitest';
import {
  chipAffordable, costRows, equivalentText, inputFloorText, openingAmountText, quickChips, typedToSmallest,
} from './deposit-amounts.js';

const FEE = 10_000n;
const format = (v) => `${(Number(v) / 1e8).toFixed(4)} ICP`;

describe('openingAmountText', () => {
  it('turns a shortfall in e8s into the ICP the field would be typed in', () => {
    expect(openingAmountText(200_000_000n, 'ICP')).toBe('2');
    expect(openingAmountText(150_000_000, 'ICP')).toBe('1.5');
    expect(openingAmountText('12345', 'ICP')).toBe('0.00012345');
  });

  it('keeps sats whole on a BTC table', () => {
    expect(openingAmountText(1_000n, 'BTC')).toBe('1000');
    expect(openingAmountText(999.2, 'BTC')).toBe('1000');
  });

  it('opens empty on nothing usable', () => {
    expect(openingAmountText(null)).toBe('');
    expect(openingAmountText(undefined)).toBe('');
    expect(openingAmountText(0)).toBe('');
    expect(openingAmountText(-5)).toBe('');
    expect(openingAmountText('junk')).toBe('');
  });
});

describe('chipAffordable', () => {
  it('needs the amount plus two ledger fees in the wallet', () => {
    expect(chipAffordable({ amount: 200_000_000n, fee: FEE, balance: 200_020_000n })).toBe(true);
    expect(chipAffordable({ amount: 200_000_000n, fee: FEE, balance: 200_019_999n })).toBe(false);
    expect(chipAffordable({ amount: 200_000_000n, fee: FEE, balance: 200_000_000n })).toBe(false);
  });

  it('does not disable a chip on an unread balance', () => {
    expect(chipAffordable({ amount: 200_000_000n, fee: FEE, balance: null })).toBe(true);
  });
});

describe('quickChips', () => {
  it('names the minimum and twice it, in the unit the field is typed in', () => {
    const chips = quickChips({ minBuyIn: 200_000_000n, fee: FEE, balance: 10_000_000_000n, format });
    expect(chips.map((c) => c.id)).toEqual(['min', 'double']);
    expect(chips.map((c) => c.label)).toEqual(['Min buy-in', '2x min']);
    expect(chips.map((c) => c.text)).toEqual(['2', '4']);
    expect(chips.every((c) => !c.disabled)).toBe(true);
  });

  it('puts the figure with its unit on the chip face', () => {
    const chips = quickChips({ minBuyIn: 1_000_000_000n, fee: FEE, balance: null, format });
    expect(chips.map((c) => c.figure)).toEqual(['10.0000 ICP', '20.0000 ICP']);
  });

  it('disables a chip the wallet cannot cover and says why in the hint', () => {
    const chips = quickChips({ minBuyIn: 200_000_000n, fee: FEE, balance: 300_000_000n, format });
    expect(chips[0].disabled).toBe(false);
    expect(chips[1].disabled).toBe(true);
    expect(chips[1].hint).toContain('4.0000 ICP plus two ledger fees');
    expect(chips[1].hint).toContain('3.0000 ICP in this wallet');
    expect(chips[0].hint).toBe('The table\'s minimum buy-in');
  });

  it('types sats whole on a BTC table in sats mode', () => {
    const chips = quickChips({ minBuyIn: 1_000n, fee: 10n, balance: null, isBTC: true, inputUnit: 'sats', format });
    expect(chips.map((c) => c.text)).toEqual(['1000', '2000']);
  });

  it('renders nothing without a table minimum', () => {
    expect(quickChips({ minBuyIn: null, fee: FEE, balance: 1n, format })).toEqual([]);
    expect(quickChips({ minBuyIn: 0, fee: FEE, balance: 1n, format })).toEqual([]);
    expect(quickChips({ minBuyIn: 'junk', fee: FEE, balance: 1n, format })).toEqual([]);
  });
});

describe('typedToSmallest', () => {
  it('floors ICP text to e8s the way the deposit always did', () => {
    expect(typedToSmallest('0.0005')).toBe(50_000n);
    expect(typedToSmallest('2')).toBe(200_000_000n);
  });

  it('keeps sats whole and floors a fraction of one', () => {
    expect(typedToSmallest('1000', { isBTC: true, inputUnit: 'sats' })).toBe(1000n);
    expect(typedToSmallest('1000.9', { isBTC: true, inputUnit: 'sats' })).toBe(1000n);
  });

  it('is zero for nothing usable', () => {
    for (const text of ['', '0', '-1', 'junk', null, undefined]) expect(typedToSmallest(text)).toBe(0n);
  });
});

describe('inputFloorText', () => {
  it('is the plain decimal for ICP and whole sats in sats mode', () => {
    expect(inputFloorText(20_000n)).toBe('0.0002');
    expect(inputFloorText(1_000n, { isBTC: true, inputUnit: 'sats' })).toBe('1000');
    expect(inputFloorText(1_000n, { isBTC: true, inputUnit: 'btc' })).toBe('0.00001');
  });
});

describe('equivalentText', () => {
  it('names the amount with its unit on an ICP table and the other unit on a BTC table', () => {
    expect(equivalentText(50_000n, { format })).toBe('0.0005 ICP');
    expect(equivalentText(1_000n, { isBTC: true, inputUnit: 'sats', format })).toBe('= 0.00001 BTC');
    expect(equivalentText(1_000n, { isBTC: true, inputUnit: 'btc', format })).toBe('= 1,000 sats');
    expect(equivalentText(0n, { format })).toBe('');
  });
});

describe('costRows', () => {
  it('lays the cost out by row id, the total strong and the credit in money tone', () => {
    const rows = costRows({ amount: 50_000n, fees: 20_000n, total: 70_000n, credited: 50_000n }, format);
    expect(rows.map((r) => r.id)).toEqual(['send', 'fees', 'total', 'credited']);
    expect(rows.map((r) => r.value)).toEqual(['0.0005 ICP', '0.0002 ICP', '0.0007 ICP', '0.0005 ICP']);
    expect(rows[2].strong).toBe(true);
    expect(rows[3].tone).toBe('money');
    expect(costRows(null, format)).toEqual([]);
  });
});

