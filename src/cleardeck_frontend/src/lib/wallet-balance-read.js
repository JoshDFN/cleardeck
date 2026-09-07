/**
 * What a ledger balance read leaves behind: a figure, or NO figure.
 *
 * The deposit sheet's contract is that `null` means UNREAD. A failed read
 * used to write 0, and a zero is a claim the ledger never made: the card
 * then said the wallet was empty, the route fell back to the address path,
 * and the "below the minimum" sentence appeared under a wallet that may be
 * full. So a failed read keeps the balance at null and carries a sentence
 * saying the read failed, and nothing downstream judges a null (the quick
 * chips stay enabled, lib/deposit-amounts.js chipAffordable; the deposit
 * itself refuses later if the money is not there).
 *
 * Pure: the decision is here so it can be tested without the rune module.
 */

/** The card's sentence for a balance the ledger would not report. */
export const WALLET_BALANCE_READ_FAILED =
  'Could not read this wallet\'s balance. Nothing is assumed from that: re-read, or send to your deposit address instead.';

/**
 * @param {{ ok: true, value: bigint|number } | { ok: false, error?: unknown }} read
 * @returns {{ balance: number|null, error: string|null }}
 *   `balance` is the figure in the smallest unit, or null when unread
 */
export function settleBalanceRead(read) {
  if (read && read.ok === true) {
    const n = Number(read.value);
    return Number.isFinite(n) ? { balance: n, error: null } : { balance: null, error: WALLET_BALANCE_READ_FAILED };
  }
  return { balance: null, error: WALLET_BALANCE_READ_FAILED };
}
