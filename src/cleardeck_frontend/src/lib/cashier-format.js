// THE CASHIER'S ARITHMETIC AND FORMATTING, ONCE.
//
// DepositModal.svelte and WithdrawModal.svelte each carried their own copies of
// "exact limit as text", "balance to BigInt", "decimal text to smallest units"
// and "four decimals with the unit". Two copies drift; this module is the one.
//
// Nothing here rounds a limit: a limit rendered with toFixed is a limit that
// lies (docs/DEFECTS.md T-26). A BALANCE is rendered at four decimals with its
// unit, which is what a cashier shows and what the screenshot harness compares
// with the ledger at that precision. The two input conversions are kept as the
// modals had them: the withdraw's exact decimal parse (MAX must read back as the
// whole balance, to the e8) and the deposit's float floor.

/** Smallest units per whole token: e8s per ICP, sats per ckBTC. */
export const SMALLEST_PER_TOKEN = 100_000_000n;

const FRACTION_DIGITS = 8;

/**
 * Exact, never rounded: "0.0002 ICP", "1,000 sats".
 * @param {bigint|number|string} smallestUnit
 * @param {'ICP'|'BTC'} [currency]
 */
export function formatExact(smallestUnit, currency = 'ICP') {
  const v = BigInt(smallestUnit);
  if (currency === 'BTC') return `${v.toLocaleString('en-US')} sats`;
  return `${formatPlain(v)} ICP`;
}

/** The same value as a bare decimal, for an input's min / max attribute. */
export function formatPlain(smallestUnit) {
  const v = BigInt(smallestUnit);
  const whole = v / SMALLEST_PER_TOKEN;
  const frac = (v % SMALLEST_PER_TOKEN).toString().padStart(FRACTION_DIGITS, '0').replace(/0+$/, '');
  return frac ? `${whole}.${frac}` : `${whole}`;
}

/**
 * A balance as a BigInt of smallest units, tolerating whatever the caller has.
 * Balances reach the cashier as Number(...) from three places (the ledger
 * query, oisy.js, a Candid Nat) and BigInt() throws on a non-integer Number.
 * Anything that is not a positive finite number is 0n.
 */
export function toSmallest(value) {
  if (typeof value === 'bigint') return value;
  const n = Number(value);
  return Number.isFinite(n) && n > 0 ? BigInt(Math.floor(n)) : 0n;
}

/**
 * Exact decimal text to smallest units WITHOUT a float in between.
 * Number('0.00012345') * 1e8 is 12344.999999999998, which floors one unit
 * short; at the bottom of a balance that turns a whole-balance sweep into a
 * sub-floor amount the canister refuses. Junk is 0n.
 */
export function decimalToSmallest(text) {
  const m = /^\s*(\d*)(?:\.(\d*))?\s*$/.exec(String(text ?? ''));
  if (!m || (m[1] === '' && (m[2] ?? '') === '')) return 0n;
  const whole = m[1] === '' ? '0' : m[1];
  const frac = (m[2] ?? '').padEnd(FRACTION_DIGITS, '0').slice(0, FRACTION_DIGITS);
  return BigInt(whole) * SMALLEST_PER_TOKEN + BigInt(frac);
}

/**
 * The deposit modal's conversion, unchanged: a float multiply, floored. Kept
 * separate from decimalToSmallest on purpose so the deposit's arithmetic is
 * byte-for-byte what it was; a non-number is 0n rather than a throw.
 */
export function floatToSmallest(amount) {
  const n = Math.floor(Number(amount) * 100_000_000);
  return Number.isFinite(n) && n > 0 ? BigInt(n) : 0n;
}

/**
 * A balance as the cashier prints it: four decimals with the unit for ICP,
 * the BTC modal's own scale for sats. Null is an unread balance, never zero.
 * @param {bigint|number|null|undefined} smallestUnit
 * @param {'ICP'|'BTC'} [currency]
 * @param {{placeholder?: string}} [opts]
 */
export function formatCashier(smallestUnit, currency = 'ICP', { placeholder = '…' } = {}) {
  if (smallestUnit === null || smallestUnit === undefined) return placeholder;
  const num = Number(smallestUnit);
  if (!Number.isFinite(num)) return placeholder;
  if (currency === 'BTC') {
    const btc = num / 100_000_000;
    if (btc >= 1) return `${btc.toFixed(4)} BTC`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K sats`;
    return `${num} sats`;
  }
  return `${(num / 100_000_000).toFixed(4)} ICP`;
}

/** Dollars for an amount at a per-token quote, or null without a usable quote. */
export function usdValue(smallestUnit, perToken) {
  if (typeof perToken !== 'number' || !(perToken > 0)) return null;
  const num = Number(smallestUnit);
  if (!Number.isFinite(num)) return null;
  return (num / 100_000_000) * perToken;
}

/** "~$12.34", four places under a cent so a micro amount is not "$0.00". */
export function formatUsd(value) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '';
  if (value < 0.01) return `~$${value.toFixed(4)}`;
  return `~$${value.toFixed(2)}`;
}

/**
 * What an ICRC-2 deposit costs the depositor. The ledger charges its fee on
 * the approve AND on the table's transfer_from, both to the depositor, so the
 * wallet is debited the amount plus two fees while the table credits the
 * amount whole.
 * @param {{amount: bigint, fee: bigint}} p
 */
export function depositCost({ amount, fee }) {
  const fees = fee * 2n;
  return { amount, fees, total: amount + fees, credited: amount };
}

/**
 * What a withdrawal delivers. The canister sends amount minus the fee
 * (src/table_canister/src/lib.rs transfer_tokens), so the wallet receives
 * that much less; feeDominates when the fee is at least half of the request.
 * @param {{amount: bigint, fee: bigint}} p
 */
export function withdrawNet({ amount, fee }) {
  const net = amount > fee ? amount - fee : 0n;
  return {
    amount,
    fee,
    net,
    feeDominates: amount > 0n && fee * 2n >= amount,
  };
}
