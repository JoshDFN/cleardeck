// THE DEPOSIT SHEET'S AMOUNT HELPERS: what the field opens with, and the quick
// chips a thumb can tap. Pure, so the affordability rule is a unit test and
// not a screenshot.

import { formatPlain } from './cashier-format.js';

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
 * The quick chips: the table's minimum buy-in and twice it, each as the field
 * would be typed in its current unit, disabled with the reason in its hint
 * when the wallet cannot cover it plus both fees.
 *
 * @param {{
 *   minBuyIn: bigint|number|string|null|undefined,
 *   fee: bigint,
 *   balance: bigint|null,
 *   isBTC?: boolean,
 *   inputUnit?: 'sats'|'btc',
 *   format: (smallest: bigint) => string,
 * }} p `format` renders a figure with its unit for the hint
 * @returns {Array<{id: string, label: string, text: string, hint: string, disabled: boolean}>}
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
