/**
 * Bet sizing: the arithmetic behind the sizer, in one place.
 *
 * Every figure here is a RAISE TO (the player's total for the round), which is
 * what `player_action({ Raise })` takes (src/table_canister/src/lib.rs, the
 * Raise arm: `raise_amount = amount - current_bet` must be >= `min_raise`, and
 * `amount - my_current_bet` must be <= my chips). With no bet in front of the
 * player the same value is a Bet, floored at `min_bet`.
 *
 * A pot-sized raise is NOT "raise to the pot": it is call first, then raise by
 * the pot as it stands after that call. With `B` = table current_bet, `P` =
 * get_pot() (which already contains every live bet, docs/DEFECTS.md T-08) and
 * `c` = the amount to call:
 *
 *     pot        raise TO  B + P + c
 *     half pot   raise TO  B + floor((P + c) / 2)
 *     two thirds raise TO  B + floor(2 (P + c) / 3)
 *
 * tools/shots/lib/chain-agreement.mjs asserts the half-pot and pot presets
 * against these exact formulas from the canister's own get_pot(); a change here
 * needs the same change there.
 *
 * All amounts are integers in the table's smallest unit (e8s for ICP, sats for
 * BTC). Nothing here mutates its inputs.
 */

/** @typedef {{ currentBet: number, minRaise: number, minBet: number, myChips: number, myCurrentBet: number, pot: number, callAmount: number }} SizingContext */

/** The legal minimum raise-to (or minimum bet when nothing is in front). */
export function raiseFloor({ currentBet, minRaise, minBet }) {
  const bet = toInt(currentBet);
  return bet === 0 ? toInt(minBet) : bet + toInt(minRaise);
}

/** The most the player can put in: everything, as a raise-to figure. */
export function raiseCap({ myChips, myCurrentBet }) {
  return toInt(myChips) + toInt(myCurrentBet);
}

/** Clamp a proposed raise-to into [floor, cap]. A cap below the floor means all-in only. */
export function clampRaise(value, floor, cap) {
  const v = toInt(value);
  const lo = toInt(floor);
  const hi = toInt(cap);
  if (hi <= lo) return hi;
  return Math.min(Math.max(v, lo), hi);
}

/** The five presets, in the order they appear, with their labels. */
export const PRESETS = Object.freeze([
  Object.freeze({ id: 'min', label: 'Min', key: '1' }),
  Object.freeze({ id: 'half', label: '½ Pot', key: '2' }),
  Object.freeze({ id: 'twoThirds', label: '⅔ Pot', key: '3' }),
  Object.freeze({ id: 'pot', label: 'Pot', key: '4' }),
  Object.freeze({ id: 'allin', label: 'All in', key: '5' }),
]);

/**
 * The raise-to figure a preset proposes, clamped to the legal range.
 * @param {string} id one of PRESETS[].id
 * @param {SizingContext} ctx
 */
export function presetTarget(id, ctx) {
  const floor = raiseFloor(ctx);
  const cap = raiseCap(ctx);
  const bet = toInt(ctx.currentBet);
  const potAfterCall = toInt(ctx.pot) + toInt(ctx.callAmount);
  let target = floor;
  if (id === 'half') target = bet + Math.floor(potAfterCall / 2);
  else if (id === 'twoThirds') target = bet + Math.floor((2 * potAfterCall) / 3);
  else if (id === 'pot') target = bet + potAfterCall;
  else if (id === 'allin') target = cap;
  return clampRaise(target, floor, cap);
}

/** Every preset's target at once: { min, half, twoThirds, pot, allin }. */
export function presetTargets(ctx) {
  return Object.fromEntries(PRESETS.map((p) => [p.id, presetTarget(p.id, ctx)]));
}

/** Which preset (if any) the current value sits on, for the pressed look. */
export function presetAt(value, ctx) {
  const targets = presetTargets(ctx);
  const v = toInt(value);
  const hit = PRESETS.find((p) => targets[p.id] === v);
  return hit ? hit.id : null;
}

/** The slider's step: a quarter of the minimum raise, never below one unit. */
export function sliderStep(minRaise) {
  return Math.max(1, Math.round(toInt(minRaise) / 4) || 1);
}

/**
 * Step the value by one big blind in either direction, clamped. Stepping from
 * a value that is not on the blind grid lands on the next grid point past it,
 * so repeated presses walk the grid rather than carrying an odd offset.
 */
export function stepByBlind(value, bigBlind, direction, floor, cap) {
  const bb = Math.max(1, toInt(bigBlind));
  const v = toInt(value);
  const dir = direction < 0 ? -1 : 1;
  const onGrid = v % bb === 0;
  const next = onGrid
    ? v + dir * bb
    : dir > 0 ? Math.ceil(v / bb) * bb : Math.floor(v / bb) * bb;
  return clampRaise(next, floor, cap);
}

/** 'bet' when nothing is in front of the player, otherwise 'raise'. */
export function raiseKind(currentBet) {
  return toInt(currentBet) === 0 ? 'bet' : 'raise';
}

/**
 * Parse what the player typed into the amount field. Returns an integer in the
 * smallest unit, or null for anything that is not a plain positive number.
 * `decimals` is the table's display precision (0 for BTC sats).
 */
export function parseAmountInput(text, { isBTC = false, decimals = 2 } = {}) {
  const cleaned = String(text ?? '').replace(/[,\s]/g, '');
  if (!/^\d*(?:\.\d*)?$/.test(cleaned) || cleaned === '' || cleaned === '.') return null;
  const n = Number(cleaned);
  if (!Number.isFinite(n) || n < 0) return null;
  if (isBTC) return Math.round(n);
  void decimals;
  return Math.round(n * 100_000_000);
}

/** Format a smallest-unit amount for the amount field (no thousands separators). */
export function formatAmountInput(value, { isBTC = false, decimals = 2 } = {}) {
  const n = toInt(value);
  if (isBTC) return String(n);
  return (n / 100_000_000).toFixed(decimals);
}

/**
 * Explain why a proposed raise-to is not legal, in words the player can act on,
 * or null when it is legal. The canister would refuse the same figure with
 * "Minimum raise is ..." or "Not enough chips"; this stops it being sent.
 */
export function raiseProblem(value, ctx, fmt) {
  const v = toInt(value);
  const floor = raiseFloor(ctx);
  const cap = raiseCap(ctx);
  if (v < floor) return `Minimum ${raiseKind(ctx.currentBet)} is ${fmt(floor)}`;
  if (v > cap) return `You have ${fmt(cap)} behind`;
  return null;
}

function toInt(v) {
  const n = Number(v ?? 0);
  return Number.isFinite(n) ? Math.trunc(n) : 0;
}
