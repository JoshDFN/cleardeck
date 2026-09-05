// THE DEPOSIT SHEET'S AMOUNT HELPERS: what the field opens with, and the quick
// chips a thumb can tap. Pure, so the affordability rule is a unit test and
// not a screenshot.

import { floatToSmallest, formatPlain } from './cashier-format.js';

/**
 * What the field's text means in the smallest unit (sats or e8s): sats are
 * floored whole, everything else goes through the float floor the deposit
 * always used (lib/cashier-format.js floatToSmallest). Nothing usable is 0n.
 *
 * @param {string} text
 * @param {{isBTC?: boolean, inputUnit?: 'sats'|'btc'}} [unit]
 * @returns {bigint}
 */
export function typedToSmallest(text, { isBTC = false, inputUnit = 'sats' } = {}) {
  if (!text || !(Number(text) > 0)) return 0n;
  if (isBTC && inputUnit === 'sats') {
    const n = Math.floor(Number(text));
    return Number.isFinite(n) && n > 0 ? BigInt(n) : 0n;
  }
  return floatToSmallest(text);
}

/**
 * The input's own floor as the field is typed: sats whole, otherwise the
 * plain decimal. Also its placeholder.
 *
 * @param {bigint} floorSmallest
 * @param {{isBTC?: boolean, inputUnit?: 'sats'|'btc'}} [unit]
 * @returns {string}
 */
export function inputFloorText(floorSmallest, { isBTC = false, inputUnit = 'sats' } = {}) {
  return isBTC && inputUnit === 'sats' ? floorSmallest.toString() : formatPlain(floorSmallest);
}

/**
 * The preview line under the field: the amount with its unit, or on a BTC
 * table the amount in its other unit. Empty for nothing typed.
 *
 * @param {bigint} smallest
 * @param {{isBTC?: boolean, inputUnit?: 'sats'|'btc', format: (v: bigint) => string}} p
 * @returns {string}
 */
export function equivalentText(smallest, { isBTC = false, inputUnit = 'sats', format }) {
  if (smallest <= 0n) return '';
  if (!isBTC) return format(smallest);
  return inputUnit === 'sats'
    ? `= ${formatPlain(smallest)} BTC`
    : `= ${smallest.toLocaleString('en-US')} sats`;
}

/**
 * The cost summary's rows (CashierSummary.svelte) for a lib/cashier-format.js
 * depositCost, by `data-row`; the harness recomputes each from the field's
 * value and the ledger's own fee (chain-agreement.mjs).
 *
 * @param {{amount: bigint, fees: bigint, total: bigint, credited: bigint}|null} cost
 * @param {(v: bigint) => string} format
 * @returns {Array<{id: string, label: string, value: string, note?: string, tone?: string, strong?: boolean}>}
 */
export function costRows(cost, format) {
  if (!cost) return [];
  return [
    { id: 'send', label: 'You send', value: format(cost.amount) },
    { id: 'fees', label: 'Ledger fees', note: 'charged twice: the approval and the pull', value: format(cost.fees), tone: 'muted' },
    { id: 'total', label: 'Total from your wallet', value: format(cost.total), strong: true },
    { id: 'credited', label: 'The table credits', value: format(cost.credited), tone: 'money' },
  ];
}

/**
 * The text the amount field opens with: e8s to ICP for an ICP table (trailing
 * zeros dropped), sats as they are for a BTC table, '' for nothing usable.
 * A Sit tap on a seat the player cannot yet afford hands the shortfall here
 * (lib/join-gate.js) so the cashier is one tap from the seat.
 *
 * @param {bigint|number|string|null|undefined} initialAmount smallest units
 * @param {'ICP'|'BTC'} currency
 * @returns {string}
 */
export function openingAmountText(initialAmount, currency = 'ICP') {
  const n = initialAmount === null || initialAmount === undefined ? null : Number(initialAmount);
  if (n === null || !Number.isFinite(n) || n <= 0) return '';
  if (currency === 'BTC') return String(Math.ceil(n));
  return (n / 1e8).toFixed(8).replace(/\.?0+$/, '');
}

/**
 * Can this wallet pay for a chip's amount? An ICRC-2 deposit costs the amount
 * plus TWO ledger fees (the approval and the pull), both charged to the
 * depositor, so a chip is affordable only when amount + 2 x fee fits in the
 * balance. An unread balance (null) does not disable anything: the chip is
 * judged when the balance is known, and the deposit itself refuses later.
 *
 * @param {{amount: bigint, fee: bigint, balance: bigint|null}} p
 * @returns {boolean}
 */
export function chipAffordable({ amount, fee, balance }) {
  if (balance === null || balance === undefined) return true;
  return amount + fee * 2n <= balance;
}

/**
 * The quick chips: the table's minimum buy-in and twice it, the figure on the
 * face and the text the field would be typed in its current unit, disabled
 * with the reason in its hint when the wallet cannot cover it plus both fees.
 *
 * @param {{
 *   minBuyIn: bigint|number|string|null|undefined,
 *   fee: bigint,
 *   balance: bigint|null,
 *   isBTC?: boolean,
 *   inputUnit?: 'sats'|'btc',
 *   format: (smallest: bigint) => string,
 * }} p `format` renders a figure with its unit for the hint
 * @returns {Array<{id: string, label: string, figure: string, text: string, hint: string, disabled: boolean}>}
 *   `figure` is the amount with its unit, for the chip's face
 */
export function quickChips({ minBuyIn, fee, balance, isBTC = false, inputUnit = 'sats', format }) {
  if (minBuyIn === null || minBuyIn === undefined) return [];
  let buyIn;
  try { buyIn = BigInt(minBuyIn); } catch { return []; }
  if (buyIn <= 0n) return [];
  const text = (v) => (isBTC && inputUnit === 'sats' ? v.toString() : formatPlain(v));
  const chip = (id, label, amount, hint) => {
    const affordable = chipAffordable({ amount, fee, balance });
    return {
      id,
      label,
      figure: format(amount),
      text: text(amount),
      hint: affordable
        ? hint
        : `${hint}: ${format(amount)} plus two ledger fees is more than the ${format(balance)} in this wallet`,
      disabled: !affordable,
    };
  };
  return [
    chip('min', 'Min buy-in', buyIn, 'The table\'s minimum buy-in'),
    chip('double', '2x min', buyIn * 2n, 'Twice the table\'s minimum buy-in'),
  ];
}
