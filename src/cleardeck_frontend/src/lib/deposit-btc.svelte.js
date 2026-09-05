// THE NATIVE-BTC PATH OF THE DEPOSIT SHEET, AS RUNE STATE: the minter's
// address for this player and the check for what has been minted. Moved out
// of DepositModal.svelte statement for statement; the calls themselves live
// in lib/deposit-btc.js.

import { checkBtcDeposits, fetchBtcDepositAddress } from './deposit-btc.js';

/**
 * @param {{
 *   tableActor: any,
 *   isBTC: boolean,
 *   isTrusted: () => boolean,
 *   untrustedReason: () => string|null,
 *   isAuthenticated: () => boolean,
 *   format: (v: bigint) => string,
 *   onFailure: (failure: unknown, opts?: {thrown: boolean}) => void,
 *   onMinted: () => Promise<void>,
 * }} p `onFailure` receives the canister's answer for the humane sentence
 * @returns {{
 *   readonly address: string,
 *   readonly loading: boolean,
 *   readonly error: string|null,
 *   readonly updating: boolean,
 *   readonly result: string|null,
 *   loadAddress: () => Promise<void>,
 *   check: () => Promise<void>,
 * }}
 */
export function createBtcDeposit({
  tableActor, isBTC, isTrusted, untrustedReason, isAuthenticated, format, onFailure, onMinted,
}) {
  let btcDepositAddress = $state('');
  let loadingBtcAddress = $state(false);
  let btcAddressError = $state(null);
  let updatingBtcBalance = $state(false);
  let btcUpdateResult = $state(null);

  // Get BTC deposit address from the table canister
  async function loadAddress() {
    if (!isBTC || !tableActor) return;

    // THE FOURTH MONEY DOOR (docs/SECURITY-FINDINGS.md FINDING 45): the BTC
    // address is FETCHED, not derived, so a substituted table id would put an
    // attacker's address on screen. Refused for an unpinned table.
    if (!isTrusted()) {
      btcAddressError = untrustedReason();
      return;
    }

    if (!isAuthenticated()) {
      btcAddressError = 'Please log in with Internet Identity to get a BTC deposit address';
      return;
    }

    loadingBtcAddress = true;
    btcAddressError = null;
    const outcome = await fetchBtcDepositAddress(tableActor);
    if ('address' in outcome) btcDepositAddress = outcome.address;
    else btcAddressError = outcome.error;
    loadingBtcAddress = false;
  }

  // Update BTC balance after sending Bitcoin
  async function check() {
    if (!tableActor) return;

    updatingBtcBalance = true;
    btcUpdateResult = null;
    const outcome = await checkBtcDeposits(tableActor, format);
    if ('failure' in outcome) {
      onFailure(outcome.failure, { thrown: outcome.thrown === true });
    } else {
      btcUpdateResult = outcome.message;
      if (outcome.minted) await onMinted();
    }
    updatingBtcBalance = false;
  }

  return {
    get address() { return btcDepositAddress; },
    get loading() { return loadingBtcAddress; },
    get error() { return btcAddressError; },
    get updating() { return updatingBtcBalance; },
    get result() { return btcUpdateResult; },
    loadAddress,
    check,
  };
}
