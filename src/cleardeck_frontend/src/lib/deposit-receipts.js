// THE DEPOSIT SHEET'S RECEIPTS, AS DATA. Pure: the rows a receipt shows for
// each of the three ways money reaches a table balance (the Internet Identity
// wallet, OISY, a sweep of the derived deposit address), so the arithmetic on
// a receipt (credited = arrived less one network charge) is a unit test and
// not a screenshot. CashierReceipt.svelte paints what comes back.

/**
 * @typedef {object} ReceiptRow
 * @property {string} id
 * @property {string} label
 * @property {string} value
 * @property {string|null} [fiat]
 * @property {boolean} [strong]
 */

/**
 * @typedef {object} Receipt
 * @property {string} title
 * @property {string} lead
 * @property {ReceiptRow[]} rows
 */

/**
 * The receipt of an ICRC-2 deposit from the signed-in wallet: the amount, the
 * two ledger fees the wallet paid, the balance the table now holds.
 *
 * @param {{
 *   amountSmallest: bigint, amountText: string,
 *   ledgerFees: bigint, ledgerFeesText: string,
 *   balance: bigint,
 *   format: (v: bigint) => string, fiatOf: (v: bigint) => string|null,
 * }} p
 * @returns {Receipt}
 */
export function walletReceipt({ amountSmallest, amountText, ledgerFees, ledgerFeesText, balance, format, fiatOf }) {
  return {
    title: 'Deposited',
    lead: `${amountText} moved from your Internet Identity wallet to your balance at this table.`,
    rows: [
      { id: 'sent', label: 'Deposited', value: amountText, fiat: fiatOf(amountSmallest) },
      { id: 'fees', label: 'Ledger fees, from your wallet', value: ledgerFeesText, fiat: fiatOf(ledgerFees) },
      { id: 'balance', label: 'Table balance now', value: format(balance), fiat: fiatOf(balance), strong: true },
    ],
  };
}

/**
 * The receipt of an OISY deposit: who paid, who is credited, the amount, the
 * balance now.
 *
 * @param {{
 *   amountSmallest: bigint, amountText: string, balance: bigint,
 *   paidBy: string, creditedTo: string,
 *   format: (v: bigint) => string, fiatOf: (v: bigint) => string|null,
 * }} p `paidBy` and `creditedTo` are already shortened principals
 * @returns {Receipt}
 */
export function oisyReceipt({ amountSmallest, amountText, balance, paidBy, creditedTo, format, fiatOf }) {
  return {
    title: 'Deposited from OISY',
    lead: `Paid by OISY ${paidBy}, credited to your signed-in identity ${creditedTo}.`,
    rows: [
      { id: 'sent', label: 'Deposited', value: amountText, fiat: fiatOf(amountSmallest) },
      { id: 'balance', label: 'Table balance now', value: format(balance), fiat: fiatOf(balance), strong: true },
    ],
  };
}

/**
 * The receipt of a sweep of the deposit address. What arrived is the card's
 * last reading of the address; the sweep pays one network charge out of it
 * (lib.rs claim_external_deposit), so the credit is that reading less the
 * charge. The three rows render only when the card had read the address
 * before the sweep (`arrived` above zero); the balance row always does.
 *
 * @param {{
 *   arrived: bigint|null, fee: bigint, feeText: string, balance: bigint,
 *   format: (v: bigint) => string, fiatOf: (v: bigint) => string|null,
 * }} p
 * @returns {Receipt}
 */
export function claimReceipt({ arrived, fee, feeText, balance, format, fiatOf }) {
  const known = arrived !== null && arrived !== undefined && arrived > 0n ? arrived : null;
  const sweptRows = known === null ? [] : [
    { id: 'arrived', label: 'Arrived at your address', value: format(known), fiat: fiatOf(known) },
    { id: 'fee', label: 'Network fee, out of it', value: feeText, fiat: fiatOf(fee) },
    { id: 'credited', label: 'Credited', value: format(known - fee), fiat: fiatOf(known - fee) },
  ];
  return {
    title: 'Deposit claimed',
    lead: 'What had arrived at your deposit address is in your table balance now.',
    rows: [
      ...sweptRows,
      { id: 'balance', label: 'Table balance now', value: format(balance), fiat: fiatOf(balance), strong: true },
    ],
  };
}
