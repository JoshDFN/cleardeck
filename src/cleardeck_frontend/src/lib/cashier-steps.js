// THE CASHIER'S STEPS, AND HOW FAR ALONG EACH ONE IS.
//
// A deposit from the Internet Identity wallet is two update calls in a row
// (icrc2_approve on the ledger, then the table's deposit) and each commits in
// 1.35-1.75 s on mainnet (docs/RESPONSIVENESS.md). The old status line said
// "Requesting approval from your wallet..." for a call that shows no popup,
// then "Transferring..." with nothing that moved for four seconds. This module
// names the steps, says how long each one is expected to take, and turns a
// start time into a progress figure the stepper can paint every frame.
//
// Pure: no timers here. The component owns the clock and asks.

/** The measured mainnet update commit, the tempo every bar is tuned to. */
export const EXPECTED_COMMIT_MS = 1750;

/** oisy.js's popup timeout: the wait the OISY step counts down from. */
export const OISY_POPUP_TIMEOUT_MS = 300_000;

/** A local derivation, no network: the instant step. */
const LOCAL_STEP_MS = 300;

/**
 * @typedef {object} CashierStep
 * @property {string} id
 * @property {string} title       what is happening, in the player's words
 * @property {string} hint        one line under it (may be '')
 * @property {number|null} expectedMs  how long it usually takes; null for a terminal step
 * @property {number} [deadlineMs]     a wait with a hard timeout counts down instead
 */

/**
 * The deposit's steps for a wallet source.
 * @param {{source: 'ii'|'oisy', amountText: string}} p
 * @returns {CashierStep[]}
 */
export function depositSteps({ source, amountText }) {
  if (source === 'oisy') {
    return [
      {
        id: 'derive',
        title: 'Deriving your deposit address',
        hint: 'From your signed-in principal, no network call',
        expectedMs: LOCAL_STEP_MS,
      },
      {
        id: 'popup',
        title: 'Approve the transfer in the OISY popup',
        hint: 'OISY pays; your signed-in identity is credited',
        expectedMs: null,
        deadlineMs: OISY_POPUP_TIMEOUT_MS,
      },
      {
        id: 'claim',
        title: `Claiming ${amountText} into your table balance`,
        hint: 'The table sweeps your deposit address',
        expectedMs: EXPECTED_COMMIT_MS,
      },
      { id: 'credited', title: 'Credited', hint: '', expectedMs: null },
    ];
  }
  return [
    {
      id: 'approve',
      title: `Approving the table to pull ${amountText}`,
      hint: 'Signed by your session. No popup opens.',
      expectedMs: EXPECTED_COMMIT_MS,
    },
    {
      id: 'move',
      title: `Moving ${amountText} to the table`,
      hint: 'The table pulls it from your wallet',
      expectedMs: EXPECTED_COMMIT_MS,
    },
    { id: 'credited', title: 'Credited to your table balance', hint: '', expectedMs: null },
  ];
}

/**
 * The withdrawal's steps: one update call with the ledger transfer inside it.
 * @param {{amountText: string}} p
 * @returns {CashierStep[]}
 */
export function withdrawSteps({ amountText }) {
  return [
    {
      id: 'withdraw',
      title: `Sending ${amountText} to your wallet`,
      hint: 'One call; the ledger transfer happens inside it',
      expectedMs: EXPECTED_COMMIT_MS,
    },
    { id: 'sent', title: 'On the ledger', hint: '', expectedMs: null },
  ];
}

/** Where the whole flow is: not started, running, finished, or failed. */
export const FLOW = Object.freeze({
  IDLE: 'idle',
  RUNNING: 'running',
  DONE: 'done',
  FAILED: 'failed',
});

/**
 * One step's status, given the flow's phase and the index it is on.
 * @param {number} index      the step being asked about
 * @param {number} current    the step the flow is on
 * @param {string} phase      a FLOW value
 * @returns {'done'|'active'|'pending'|'failed'}
 */
export function stepStatus(index, current, phase) {
  if (phase === FLOW.DONE) return 'done';
  if (index < current) return 'done';
  if (index > current) return 'pending';
  if (phase === FLOW.FAILED) return 'failed';
  return phase === FLOW.RUNNING ? 'active' : 'pending';
}

/**
 * The bar's fill for a step that has been running since `startedAt`: it
 * reaches 0.9 at the expected duration and then creeps, never completing on
 * its own, so a slow commit reads as slow rather than as done. 0 with no
 * start or no expectation.
 * @param {{startedAt: number|null, now: number, expectedMs: number|null}} p
 */
export function progressAt({ startedAt, now, expectedMs }) {
  if (!Number.isFinite(startedAt) || !Number.isFinite(now) || !(expectedMs > 0)) return 0;
  const t = Math.max(0, now - startedAt) / expectedMs;
  if (t <= 1) return Math.min(0.9, t * 0.9);
  // Past the expectation: the last tenth, asymptotically.
  return Math.min(0.97, 0.9 + 0.07 * (1 - Math.exp(-(t - 1))));
}

/**
 * Whole seconds left on a wait with a hard timeout, floored at zero.
 * @param {{startedAt: number|null, now: number, deadlineMs: number}} p
 */
export function secondsLeft({ startedAt, now, deadlineMs }) {
  if (!Number.isFinite(startedAt) || !Number.isFinite(now) || !(deadlineMs > 0)) return 0;
  return Math.max(0, Math.ceil((startedAt + deadlineMs - now) / 1000));
}

/** "4:59", "0:07". Negative or junk reads as "0:00". */
export function formatCountdown(secs) {
  const s = Number.isFinite(secs) && secs > 0 ? Math.floor(secs) : 0;
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, '0')}`;
}

/**
 * Seconds until the next withdrawal is allowed, from when the last one landed.
 * @param {{lastAt: number|null, now: number, cooldownSecs: number}} p
 */
export function cooldownRemainingSecs({ lastAt, now, cooldownSecs }) {
  if (!Number.isFinite(lastAt) || !Number.isFinite(now) || !(cooldownSecs > 0)) return 0;
  return Math.max(0, Math.ceil((lastAt + cooldownSecs * 1000 - now) / 1000));
}
