// THE NATIVE-BITCOIN CALLS OF THE DEPOSIT SHEET. The address is FETCHED from
// the table canister (it is the ckBTC minter's, not derivable here), which is
// why DepositModal.svelte refuses to call `fetchBtcDepositAddress` for a table
// outside the trust root (docs/SECURITY-FINDINGS.md FINDING 45) before this
// module is reached. Outcome objects, never component state.

import { logger } from './logger.js';

/**
 * @param {any} tableActor
 * @returns {Promise<{address: string} | {error: string}>}
 */
export async function fetchBtcDepositAddress(tableActor) {
  try {
    const result = await tableActor.get_btc_deposit_address();
    if ('Ok' in result) return { address: result.Ok };
    return { error: result.Err };
  } catch (e) {
    logger.error('Failed to get BTC deposit address:', e);
    return { error: e?.message || 'Failed to get deposit address' };
  }
}

/**
 * Asks the table to look for new UTXOs at the minter's address and mint
 * ckBTC for them.
 *
 * @param {any} tableActor
 * @param {(smallest: number) => string} formatWithUnit
 * @returns {Promise<{message: string, minted: boolean} | {failure: unknown}>}
 */
export async function checkBtcDeposits(tableActor, formatWithUnit) {
  try {
    const result = await tableActor.update_btc_balance();
    if ('Err' in result) return { failure: result.Err };
    const statuses = result.Ok;
    const minted = statuses.filter((s) => 'Minted' in s);
    if (minted.length > 0) {
      const totalMinted = minted.reduce((sum, s) => sum + Number(s.Minted.minted_amount), 0);
      return { message: `Success! ${formatWithUnit(totalMinted)} minted to your wallet.`, minted: true };
    }
    if (statuses.some((s) => 'Checked' in s)) {
      return { message: 'UTXOs found and being processed. Please wait and try again.', minted: false };
    }
    return { message: 'No new deposits found yet.', minted: false };
  } catch (e) {
    logger.error('Failed to update BTC balance:', e);
    // A throw, not an Err: the minter may have credited the wallet anyway.
    return { failure: e, thrown: true };
  }
}
