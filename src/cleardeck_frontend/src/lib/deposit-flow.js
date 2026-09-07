// THE TWO WALLET ROUTES OF THE DEPOSIT SHEET, AS FUNCTIONS.
//
// Moved out of DepositModal.svelte statement for statement: the same calls
// in the same order with the same refusals. Each function returns an outcome
// object instead of touching component state; the modal turns the outcome
// into the flow's step, the receipt or the humane sentence. A thrown error
// propagates, as it did, to the modal's catch.
//
// docs/SECURITY-FINDINGS.md FINDING 40 / 42 are honoured here by
// construction: the OISY destination is DERIVED from the session principal
// and cross-checked against the canister, never fetched; the approval names
// the table the caller already checked against the trust root.

import { Principal } from '@dfinity/principal';
import { depositSubaccount } from './depositAddress.js';
import { approveSpender } from './deposit-icrc2.js';
import { logger } from './logger.js';

const toHex = (bytes) => Array.from(bytes ?? [], (b) => b.toString(16).padStart(2, '0')).join('');

/**
 * The OISY route: derive the subaccount, cross-check it, transfer from OISY
 * to it, claim it.
 *
 * @param {{
 *   sessionPrincipal: import('@dfinity/principal').Principal,
 *   tableActor: any,
 *   tableCanisterId: string|import('@dfinity/principal').Principal,
 *   ledgerCanisterId: string,
 *   isBTC: boolean,
 *   currencySymbol: string,
 *   amountSmallest: bigint,
 *   oisy: { getWallet: () => any },
 *   oisyPrincipal: any,
 *   onStep: () => void,
 * }} p `onStep` advances the painted stepper (called twice: before the transfer, before the claim)
 * @returns {Promise<{ok: true, balance: bigint} | {ok: false, refusal?: string, failure?: unknown}>}
 *   `refusal` is this sheet's own sentence, shown as written; `failure` is
 *   the canister's answer, for describeCashierFailure.
 */
export async function depositViaOisy({
  sessionPrincipal, tableActor, tableCanisterId, ledgerCanisterId, isBTC, currencySymbol,
  amountSmallest, oisy, oisyPrincipal, onStep,
}) {
  // Step 1: DERIVE the destination. Never fetch it (FINDING 40). The
  // subaccount is a pure function of the SESSION principal, the one that will
  // call `claim_external_deposit()` and be credited, which is NOT the OISY
  // wallet principal paying for it.
  const depositSubaccountBytes = depositSubaccount(sessionPrincipal);

  // The canister is asked ONLY to catch a drift between the two derivations,
  // and its answer is never preferred. A disagreement aborts: the very next
  // statement moves real money.
  try {
    const reportedSub = await tableActor.get_deposit_subaccount();
    if (toHex(reportedSub) !== toHex(depositSubaccountBytes)) {
      return {
        ok: false,
        refusal:
          'Refusing to send: this table reported a different deposit subaccount from the ' +
          'one derived from your principal, so one of the two answers is wrong and paying ' +
          'either would be guessing with your money. Nothing has been sent.',
      };
    }
  } catch (checkError) {
    logger.error('could not cross-check the deposit subaccount:', checkError);
  }

  // Step 2: Transfer from OISY wallet directly to the canister's deposit subaccount
  onStep();

  const wallet = oisy.getWallet();
  if (!wallet) return { ok: false, refusal: 'OISY wallet not connected' };

  const canisterPrincipal = typeof tableCanisterId === 'string'
    ? Principal.fromText(tableCanisterId)
    : tableCanisterId;

  if (!wallet.transfer) {
    return {
      ok: false,
      refusal: `OISY wallet does not support direct transfers. Please transfer ${currencySymbol} to your Internet Identity wallet first, then deposit from there.`,
    };
  }

  const destination = {
    to: { owner: canisterPrincipal, subaccount: [depositSubaccountBytes] },
    amount: amountSmallest,
  };

  await wallet.transfer({
    ...(isBTC ? { params: destination } : { request: destination }),
    owner: oisyPrincipal,
    ledgerCanisterId,
    options: { timeoutInMilliseconds: 300000 },
  });

  // Step 3: Claim the deposit (sweeps from subaccount to main balance)
  onStep();
  const claimResult = await tableActor.claim_external_deposit();
  if ('Ok' in claimResult) return { ok: true, balance: claimResult.Ok };
  return { ok: false, failure: claimResult.Err };
}

/**
 * The Internet Identity route: approve the table for the amount plus one
 * network charge, then let the table pull the amount.
 *
 * @param {{
 *   agent: import('@dfinity/agent').HttpAgent,
 *   ledgerCanisterId: string,
 *   tableActor: any,
 *   tableCanisterId: string|import('@dfinity/principal').Principal,
 *   amountSmallest: bigint,
 *   approveAmount: bigint,
 *   onApproved: () => void,
 * }} p `onApproved` advances the painted stepper between the two commits
 * @returns {Promise<{ok: true, balance: bigint}
 *   | {ok: false, approveError: {key: string, value: any}}
 *   | {ok: false, failure: unknown}>}
 */
export async function depositViaWallet({
  agent, ledgerCanisterId, tableActor, tableCanisterId, amountSmallest, approveAmount, onApproved,
}) {
  const tableCanisterPrincipal = typeof tableCanisterId === 'string'
    ? Principal.fromText(tableCanisterId)
    : tableCanisterId;

  const approveResult = await approveSpender(agent, ledgerCanisterId, {
    amount: approveAmount,
    spender: tableCanisterPrincipal,
  });

  if ('Err' in approveResult) {
    const key = Object.keys(approveResult.Err)[0];
    return { ok: false, approveError: { key, value: approveResult.Err[key] } };
  }

  onApproved();

  const depositResult = await tableActor.deposit(amountSmallest);
  if ('Ok' in depositResult) return { ok: true, balance: depositResult.Ok };
  return { ok: false, failure: depositResult.Err };
}
