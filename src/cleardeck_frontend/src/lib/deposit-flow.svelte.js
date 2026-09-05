// THE DEPOSIT SHEET'S FLOW AND ITS ADDRESS ROUTE, AS RUNE STATE.
//
// Two factories, each called once during DepositModal.svelte's init: the
// cashier flow (which step of a deposit is running, since when, and the
// receipt shown until Done) and the address watch (the derived deposit
// subaccount's ledger balance, re-read every few seconds while the card is
// up, swept in by itself once per ready reading with the same
// `claim_external_deposit` the button makes). The decisions are pure and
// tested in lib/cashier-steps.js and lib/deposit-detect.js; this module holds
// the state, runs the effects and makes the one call.

import { FLOW } from './cashier-steps.js';
import { DETECT, DETECT_POLL_MS, classifyDetected, shouldAutoClaim } from './deposit-detect.js';

const idle = () => ({ phase: FLOW.IDLE, steps: [], current: 0, startedAt: null });

/** The sweep's two steps, painted on the flow while it runs. */
const CLAIM_STEPS = [
  { id: 'claim', title: 'Claiming what arrived at your deposit address', hint: 'The table sweeps it into your balance', expectedMs: 1750 },
  { id: 'credited', title: 'Credited to your table balance', hint: '', expectedMs: null },
];

/**
 * The flow and the receipt. Every transition builds a new flow object.
 *
 * @returns {{
 *   readonly flow: {phase: string, steps: Array<object>, current: number, startedAt: number|null},
 *   receipt: object|null,
 *   begin: (steps: Array<object>) => void,
 *   advance: () => void,
 *   finish: () => void,
 *   fail: () => void,
 *   reset: () => void,
 * }}
 */
export function createCashierFlow() {
  let flow = $state(idle());
  let receipt = $state(null);
  return {
    get flow() { return flow; },
    get receipt() { return receipt; },
    set receipt(next) { receipt = next; },
    begin(steps) {
      flow = { phase: FLOW.RUNNING, steps, current: 0, startedAt: Date.now() };
    },
    advance() {
      flow = { ...flow, current: Math.min(flow.current + 1, flow.steps.length - 1), startedAt: Date.now() };
    },
    finish() {
      flow = { ...flow, phase: FLOW.DONE };
    },
    fail() {
      flow = { ...flow, phase: FLOW.FAILED };
    },
    reset() {
      flow = idle();
    },
  };
}

/**
 * The address watches itself and sweeps what arrives (lib/deposit-detect.js).
 * While `isWatching()` holds, `readBalance()` is called at once and then every
 * `pollMs`; a reading at or above the route's floor is claimed once per
 * distinct reading. `checkNow()` is the button on the address route: read
 * the address now and sweep if it is ready. The sweep is the same
 * `claim_external_deposit` call in the same order with the same refusal as
 * the button always made: nothing at an unpinned canister is yours to sweep.
 *
 * @param {{
 *   tableActor: any,
 *   readBalance: () => Promise<bigint>,
 *   isWatching: () => boolean,
 *   isTrusted: () => boolean,
 *   untrustedReason: () => string|null,
 *   flow: ReturnType<typeof createCashierFlow>,
 *   setError: (message: string|null) => void,
 *   onFailure: (failure: unknown, opts?: {thrown: boolean}) => void,
 *   receiptFor: (arrived: bigint|null, balance: bigint) => object,
 *   onCredited: () => Promise<void>,
 *   fee: bigint,
 *   minExternal: bigint,
 *   pollMs?: number,
 * }} p `setError` shows this sheet's own sentence; `onFailure` maps the
 *   canister's answer; `receiptFor` builds the receipt from the card's last
 *   reading and the new balance; `onCredited` runs after a successful sweep.
 * @returns {{
 *   readonly detected: bigint|null,
 *   readonly status: 'empty'|'stuck'|'short'|'ready',
 *   readonly claiming: boolean,
 *   readonly claimFailed: boolean,
 *   read: () => Promise<void>,
 *   claim: () => Promise<void>,
 *   checkNow: () => Promise<void>,
 * }}
 */
export function createAddressWatch({
  tableActor, readBalance, isWatching, isTrusted, untrustedReason, flow, setError, onFailure,
  receiptFor, onCredited, fee, minExternal, pollMs = DETECT_POLL_MS,
}) {
  let detected = $state(null);
  let attemptedFor = $state(null);
  let claiming = $state(false);
  let claimFailed = $state(false);
  const status = $derived(classifyDetected({ balance: detected, fee, minExternal }));

  async function read() {
    try {
      const balance = await readBalance();
      detected = balance;
      // An emptied address is a new address: the next arrival is a new reading.
      if (balance === 0n) attemptedFor = null;
    } catch (e) {
      console.error('could not read the deposit address:', e);
    }
  }

  // Sweep whatever is at YOUR deposit address into your table balance.
  async function claim() {
    // Nothing at an unpinned canister is yours to sweep.
    if (!isTrusted()) {
      setError(untrustedReason());
      return;
    }
    claiming = true;
    claimFailed = false;
    setError(null);
    flow.begin(CLAIM_STEPS);
    try {
      const result = await tableActor.claim_external_deposit();
      if ('Ok' in result) {
        flow.finish();
        // What arrived is the card's last reading of the address
        // (lib/deposit-receipts.js claimReceipt does the arithmetic).
        flow.receipt = receiptFor(detected, result.Ok);
        await onCredited();
      } else {
        flow.fail();
        claimFailed = true;
        onFailure(result.Err);
      }
    } catch (e) {
      console.error('claim_external_deposit failed:', e);
      flow.fail();
      claimFailed = true;
      // A throw on the reply leg: the sweep may have landed. Said so.
      onFailure(e, { thrown: true });
    }
    claiming = false;
  }

  $effect(() => {
    if (!isWatching()) return undefined;
    read();
    const timer = setInterval(read, pollMs);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!isWatching()) return;
    if (shouldAutoClaim({ status, claiming, balance: detected, attemptedFor })) {
      attemptedFor = detected;
      claim();
    }
  });

  async function checkNow() {
    if (claiming) return;
    await read();
    if (status === DETECT.READY) {
      attemptedFor = detected;
      await claim();
    }
  }

  return {
    get detected() { return detected; },
    get status() { return status; },
    get claiming() { return claiming; },
    get claimFailed() { return claimFailed; },
    read,
    claim,
    checkNow,
  };
}
