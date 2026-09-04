/**
 * WHAT A TAP ON AN EMPTY SEAT SHOULD DO, decided from certified facts only.
 *
 * The audit's critical mobile finding: an anonymous tap on "Sit" went straight
 * to the canister and came back as a red "Insufficient balance ... Deposit ICP
 * first" panel over the whole phone header. No leader lets an unauthenticated
 * tap reach the table: the seat opens sign-in, then the cashier with the
 * shortfall pre-filled, and only then the seat.
 *
 * Pure. The caller (routes/+page.svelte) supplies the facts and acts on the
 * decision; nothing here touches the DOM, the auth store or a canister.
 */

/** @typedef {{kind: 'login'} | {kind: 'deposit', shortfall: number} | {kind: 'join'}} JoinDecision */

/**
 * @param {{isAuthenticated: boolean, escrow: number|bigint|null|undefined, minBuyIn: number|bigint|null|undefined}} facts
 *   `escrow` is the player's balance at this table in the smallest unit (e8s or
 *   sats); `minBuyIn` the table's minimum buy-in in the same unit. Either may be
 *   unknown (null): an unknown balance is NOT treated as zero, because sending
 *   a funded player to the cashier is a worse mistake than one refused join.
 * @returns {JoinDecision}
 */
export function joinGateDecision({ isAuthenticated, escrow, minBuyIn }) {
  if (!isAuthenticated) return { kind: 'login' };
  const have = toNumber(escrow);
  const need = toNumber(minBuyIn);
  if (have === null || need === null || need <= 0) return { kind: 'join' };
  if (have >= need) return { kind: 'join' };
  return { kind: 'deposit', shortfall: need - have };
}

/**
 * The seat to resume after a sign-in that a Sit tap started, or null.
 * A NEW object every time: the caller replaces its state rather than mutating it.
 *
 * @param {{seat: number, tableKey: string}|null} remembered what the tap stored
 * @param {string} tableKey the table the player is looking at now
 * @returns {{seat: number}|null} the seat to join, or null when nothing applies
 */
export function seatToResume(remembered, tableKey) {
  if (!remembered) return null;
  if (remembered.tableKey !== tableKey) return null;
  if (!Number.isInteger(remembered.seat) || remembered.seat < 0) return null;
  return { seat: remembered.seat };
}

function toNumber(v) {
  if (v === null || v === undefined) return null;
  const n = typeof v === 'bigint' ? Number(v) : Number(v);
  return Number.isFinite(n) ? n : null;
}
