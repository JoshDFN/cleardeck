// THE TWO QUESTIONS EVERY MONEY SHEET ASKS ON MOUNT, AS RUNE STATE.
//
// CAN THIS TABLE PAY BACK WHAT IT ALREADY HOLDS? Asked before, not after
// (docs/SECURITY-FINDINGS.md FINDING 35 / docs/DEFECTS.md E-70): a query run
// on mount, since a warning that appears after the button is pressed is a
// receipt. HOW LONG CAN THIS TABLE KEEP HONOURING WITHDRAWALS?
// (docs/DEFECTS.md E-55): a canister below its freezing threshold rejects
// EVERY update call at once. Both sheets (DepositModal, WithdrawModal) call
// these once during init and render SolvencyNotice / CycleRunwayNotice from
// what comes back.

import { readTableSolvency, refreshTableSolvency } from './solvency.js';
import { readCycleRunway } from './cycleRunway.js';

/**
 * @param {any} tableActor
 * @returns {{
 *   readonly solvency: object|null,
 *   readonly refreshing: boolean,
 *   load: () => Promise<void>,
 *   refresh: () => Promise<void>,
 * }}
 */
export function createSolvencyRead(tableActor) {
  let solvency = $state(null);
  let refreshing = $state(false);

  async function load() {
    solvency = await readTableSolvency(tableActor);
  }

  async function refresh() {
    refreshing = true;
    try {
      solvency = await refreshTableSolvency(tableActor);
    } finally {
      refreshing = false;
    }
  }

  return {
    get solvency() { return solvency; },
    get refreshing() { return refreshing; },
    load,
    refresh,
  };
}

/**
 * @param {any} tableActor
 * @returns {{ readonly runway: object|null, load: () => Promise<void> }}
 */
export function createRunwayRead(tableActor) {
  let runway = $state(null);

  async function load() {
    runway = await readCycleRunway(tableActor);
  }

  return {
    get runway() { return runway; },
    load,
  };
}
