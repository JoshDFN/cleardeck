// THE PAYING WALLET AND YOUR DEPOSIT ADDRESS, AS RUNE STATE.
//
// Moved out of DepositModal.svelte statement for statement. NOTHING IN THE
// VERIFICATION LOGIC CHANGED: the deposit address is DERIVED from the pinned
// canister id and the signed-in principal (the trust root is checked inside
// that call, docs/SECURITY-FINDINGS.md FINDING 42), then CHECKED against what
// the canister says and never taken from it (FINDING 34 / 40); a canister
// that will not answer cannot hide the derived address, and a disagreement
// other than the pre-FINDING-34 shared account shows nothing at all.

import { Principal } from '@dfinity/principal';
import {
  accountIdentifierHex, checkAgainstCanister, depositSubaccount, deriveTrustedDepositAddress,
} from './depositAddress.js';
import { readLedgerBalance } from './deposit-icrc2.js';
import { logger } from './logger.js';
import { settleBalanceRead } from './wallet-balance-read.js';

/**
 * @param {{
 *   auth: { getAgent: () => Promise<any> },
 *   tableActor: any,
 *   tableCanisterId: string|import('@dfinity/principal').Principal|null,
 *   ledgerCanisterId: string,
 *   isBTC: boolean,
 *   currencySymbol: string,
 * }} p
 * @returns {{
 *   readonly balance: number|null,
 *   readonly balanceError: string|null,
 *   readonly loading: boolean,
 *   readonly address: string,
 *   readonly warning: string|null,
 *   load: () => Promise<void>,
 *   readAddressBalance: () => Promise<bigint>,
 * }}
 */
export function createWalletReader({ auth, tableActor, tableCanisterId, ledgerCanisterId, isBTC, currencySymbol }) {
  // null is UNREAD (never a stand-in zero: a zero is a claim the ledger did
  // not make, and the sheet's route and floor checks read it as one).
  let walletBalance = $state(null);
  // Why the last read gave no figure, or null when it did.
  let balanceError = $state(null);
  let loadingBalance = $state(true);
  // YOUR table deposit address, derived locally. Empty until it is derived, and
  // deliberately left empty when the canister's own answer disagrees with it.
  let tableDepositAddress = $state('');
  let depositAddressWarning = $state(null);

  // Derive YOUR deposit address locally, then ask the canister and compare.
  // The comparison NEVER prefers the canister's answer.
  async function deriveAndVerifyDepositAddress(principal) {
    depositAddressWarning = null;
    tableDepositAddress = '';
    if (!tableCanisterId) {
      depositAddressWarning = 'This table has no canister id in this build, so no deposit address can be derived.';
      return;
    }
    let derived;
    try {
      // THE TRUST ROOT IS CHECKED INSIDE THIS CALL, NOT HERE (FINDING 42).
      derived = deriveTrustedDepositAddress(tableCanisterId, principal).address;
    } catch (e) {
      depositAddressWarning = e.message || 'Could not derive your deposit address.';
      return;
    }
    // Show the derived address first: it is the trustworthy one, and a canister
    // that will not answer must not be able to hide it.
    tableDepositAddress = derived;
    try {
      const reported = await tableActor.get_deposit_address();
      const shared = accountIdentifierHex(tableCanisterId, null);
      const { agrees, safeToShow, reason } = checkAgainstCanister(derived, reported, shared);
      if (!agrees) {
        depositAddressWarning = reason;
        // `safeToShow` is the whole judgement: any disagreement other than the
        // pre-FINDING-34 shared account means the derivations differ, so show nothing.
        if (!safeToShow) tableDepositAddress = '';
      }
    } catch (e) {
      logger.error('could not cross-check the deposit address:', e);
    }
  }

  // Get user's balance from their wallet (ICP or ckBTC)
  async function load() {
    loadingBalance = true;
    try {
      const agent = await auth.getAgent();
      const principal = await agent.getPrincipal();

      // YOUR table deposit address. Derived from the canister id and your own
      // principal, then CHECKED against what the canister says, never taken
      // from it. docs/SECURITY-FINDINGS.md FINDING 34, FINDING 40.
      if (!isBTC) {
        await deriveAndVerifyDepositAddress(principal);
      }

      const balance = await readLedgerBalance(agent, ledgerCanisterId, { owner: principal });
      ({ balance: walletBalance, error: balanceError } = settleBalanceRead({ ok: true, value: balance }));
    } catch (e) {
      logger.error(`Failed to load ${currencySymbol} wallet balance:`, e);
      // The balance stays UNREAD (null), and the card says the read failed.
      ({ balance: walletBalance, error: balanceError } = settleBalanceRead({ ok: false, error: e }));
    }
    loadingBalance = false;
  }

  /** The ledger's balance of the derived deposit subaccount at this table. */
  async function readAddressBalance() {
    const agent = await auth.getAgent();
    const principal = await agent.getPrincipal();
    const canister = typeof tableCanisterId === 'string' ? Principal.fromText(tableCanisterId) : tableCanisterId;
    return readLedgerBalance(agent, ledgerCanisterId, {
      owner: canister, subaccount: depositSubaccount(principal),
    });
  }

  return {
    get balance() { return walletBalance; },
    get balanceError() { return balanceError; },
    get loading() { return loadingBalance; },
    get address() { return tableDepositAddress; },
    get warning() { return depositAddressWarning; },
    load,
    readAddressBalance,
  };
}
