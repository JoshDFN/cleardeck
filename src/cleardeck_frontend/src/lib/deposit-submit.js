// THE TWO WALLET ROUTES OF THE DEPOSIT SHEET, PRESSED.
//
// Moved out of DepositModal.svelte statement for statement: the OISY route
// and the Internet Identity route, each painting its steps on the flow and
// returning an outcome the sheet turns into a receipt, its own sentence or
// the canister's answer. The calls are lib/deposit-flow.js's; the sheet has
// already refused an unpinned table and validated the amount before this runs.
// A thrown error propagates, as it did, to the sheet's catch.

import { depositSteps } from './cashier-steps.js';
import { depositViaOisy, depositViaWallet } from './deposit-flow.js';
import { oisyReceipt, walletReceipt } from './deposit-receipts.js';

/**
 * @typedef {{ok: true, receipt: object, source: 'ii'|'oisy'}
 *   | {ok: false, error: string}
 *   | {ok: false, failure: unknown, thrown?: boolean}} SubmitOutcome
 *   `error` is this sheet's own sentence, shown as written; `failure` is the
 *   canister's or the ledger's answer, for describeCashierFailure; `thrown`
 *   marks a failure that was THROWN by the wallet or the agent rather than
 *   returned as an Err, so the sheet can say the money may have moved.
 */

/**
 * @param {{
 *   source: 'ii'|'oisy',
 *   amountSmallest: bigint,
 *   approveAmount: bigint,
 *   amountTyped: string,
 *   currencySymbol: string,
 *   isBTC: boolean,
 *   tableActor: any,
 *   tableCanisterId: string|import('@dfinity/principal').Principal,
 *   ledgerCanisterId: string,
 *   auth: { getAgent: () => Promise<any> },
 *   oisy: any,
 *   oisyPrincipal: any,
 *   sessionPrincipal: string|null,
 *   flow: { begin: (steps: object[]) => void, advance: () => void, finish: () => void, fail: () => void },
 *   money: {
 *     format: (v: bigint) => string,
 *     fiatOf: (v: bigint) => string|null,
 *     feeDisplay: string,
 *     ledgerFees: bigint,
 *     ledgerFeesDisplay: string,
 *   },
 *   shortId: (id: string) => string,
 * }} p
 * @returns {Promise<SubmitOutcome>}
 */
export async function submitDeposit({
  source, amountSmallest, approveAmount, amountTyped, currencySymbol, isBTC,
  tableActor, tableCanisterId, ledgerCanisterId, auth, oisy, oisyPrincipal, sessionPrincipal,
  flow, money, shortId,
}) {
  const amountText = money.format(amountSmallest);

  if (source === 'oisy') {
    // THE OISY ROUTE (lib/deposit-flow.js depositViaOisy): derive the
    // subaccount from the SESSION principal, cross-check it against the
    // canister, transfer from OISY, claim. Its refusals are this sheet's
    // own sentences; a canister's answer goes through the humane mapping.
    flow.begin(depositSteps({ source: 'oisy', amountText }));
    try {
      const principal = await (await auth.getAgent()).getPrincipal();
      const outcome = await depositViaOisy({
        sessionPrincipal: principal, tableActor, tableCanisterId, ledgerCanisterId, isBTC, currencySymbol,
        amountSmallest, oisy, oisyPrincipal, onStep: flow.advance,
      });
      if (!outcome.ok) {
        flow.fail();
        return outcome.refusal ? { ok: false, error: outcome.refusal } : { ok: false, failure: outcome.failure };
      }
      flow.finish();
      return {
        ok: true,
        source,
        receipt: oisyReceipt({
          amountSmallest, amountText, balance: outcome.balance,
          paidBy: shortId(oisyPrincipal ?? ''), creditedTo: shortId(sessionPrincipal ?? ''),
          format: money.format, fiatOf: money.fiatOf,
        }),
      };
    } catch (oisyError) {
      console.error('OISY deposit failed:', oisyError);
      flow.fail();
      // A throw, not an Err: the transfer or the claim may have landed.
      return { ok: false, failure: oisyError, thrown: true };
    }
  }

  // THE INTERNET IDENTITY ROUTE (lib/deposit-flow.js depositViaWallet):
  // the approval of the amount plus one network charge with the table as
  // the spender, then the table's pull.
  flow.begin(depositSteps({ source: 'ii', amountText }));
  const agent = await auth.getAgent();
  const outcome = await depositViaWallet({
    agent, ledgerCanisterId, tableActor, tableCanisterId, amountSmallest, approveAmount,
    onApproved: flow.advance,
  });

  if (!outcome.ok && outcome.approveError) {
    const { key: errKey, value: errVal } = outcome.approveError;
    flow.fail();
    if (errKey === 'InsufficientFunds') {
      const balanceDisplay = money.format(errVal.balance);
      return {
        ok: false,
        error: `Insufficient funds. You have ${balanceDisplay} but need ${amountTyped} ${currencySymbol} plus the ${money.feeDisplay} ledger fee.`,
      };
    }
    if (errKey === 'GenericError') return { ok: false, failure: errVal.message };
    return { ok: false, failure: `Approval failed: ${errKey}` };
  }

  if (outcome.ok) {
    flow.finish();
    return {
      ok: true,
      source,
      receipt: walletReceipt({
        amountSmallest, amountText, ledgerFees: money.ledgerFees, ledgerFeesText: money.ledgerFeesDisplay,
        balance: outcome.balance, format: money.format, fiatOf: money.fiatOf,
      }),
    };
  }
  flow.fail();
  return { ok: false, failure: outcome.failure };
}
