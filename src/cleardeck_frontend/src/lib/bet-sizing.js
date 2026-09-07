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
 *     half pot   raise TO  B + (P + c) / 2
 *     two thirds raise TO  B + 2 (P + c) / 3
 *
 * Pre-flop the row is multiples instead (PokerStars, GGPoker): 2.5x / 3x / 4x
 * of the bet in front (`B`), or of the big blind when only the blinds are in.
 *
 * THE DISPLAY UNIT. Every figure the sizer proposes is QUANTISED to the unit
 * the screen shows (0.01 ICP = 1,000,000 e8s at two decimals; one sat on a
 * BTC table), rounded DOWN, then clamped into the legal range. Before this,
 * the two-thirds preset on a 0.30 pot proposed 46,666,666 e8s, the field and
 * the Raise button read "0.47", and 0.46666666 ICP would have been sent: a
 * money figure on screen that differs from what is sent. The quantum is part
 * of the SizingContext so the canister's own figures (the floor, the cap) are
 * never rounded: only proposals are.
 *
 * THE CAP IS EXACT. The all-in figure is the whole stack (chips behind plus
 * the bet already in front), and a stack is not on the display grid after an
 * odd pot has been split: 123,456,789 e8s quantised down is 123,000,000, and
 * a "raise to 1.23" from that stack leaves 456,789 e8s behind, which is a
 * raise SHORT of all-in that the table then treats as a plain raise. So a
 * proposal at or above the cap IS the cap, unquantised (`snapRaise`), the
 * primary button reads "All in" with the exact figure (`formatExact`), and
 * the figure sent equals the one shown. Only the fractions, the multiples
 * and the blind steps are quantised.
 *
 * tools/shots/lib/chain-agreement.mjs asserts the presets against these exact
 * formulas from the canister's own get_pot(); a change here needs the same
 * change there.
 *
 * All amounts are integers in the table's smallest unit (e8s for ICP, sats for
 * BTC). Nothing here mutates its inputs.
 */

/**
 * @typedef {object} SizingContext
 * @property {number} currentBet    the table's current_bet
 * @property {number} minRaise      the canister's min_raise
 * @property {number} minBet        the canister's min_bet
 * @property {number} myChips       the hero's chips behind
 * @property {number} myCurrentBet  what the hero already has in this round
 * @property {number} pot           get_pot()
 * @property {number} callAmount    what the hero owes
 * @property {number} [bigBlind]    the table's big blind (pre-flop multiples)
 * @property {number} [quantum]     the display unit in the smallest unit (1 = no quantisation)
 */

const E8S_PER_ICP = 100_000_000;
const ICP_DECIMALS = 8;

/** The display unit in the smallest unit: 10^(8 - decimals) e8s for ICP, one sat for BTC. */
export function displayQuantum({ isBTC = false, decimals = 2 } = {}) {
  if (isBTC) return 1;
  const d = Math.max(0, Math.min(ICP_DECIMALS, Math.trunc(Number(decimals) || 0)));
  return 10 ** (ICP_DECIMALS - d);
}

/** Round a smallest-unit value DOWN onto the display grid. */
export function quantise(value, quantum) {
  const q = Math.max(1, toInt(quantum));
  return Math.floor(toInt(value) / q) * q;
}

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

/**
 * A proposal onto the display grid and into the legal range, with ONE
 * exception: at or above the cap the answer is the cap itself, never
 * quantised, because the cap is the exact stack and anything under it is a
 * raise short of all-in. Below the cap the value is rounded down to the grid
 * and lifted to the floor if it fell under it.
 */
export function snapRaise(value, floor, cap, quantum = 1) {
  const v = toInt(value);
  const hi = toInt(cap);
  if (v >= hi) return clampRaise(hi, floor, hi);
  return clampRaise(quantise(v, quantum), floor, hi);
}

/**
 * Whether a raise-to figure IS the player's whole stack: equality with the
 * cap, not "at or above it". setRaise, snapRaise, stepByBlind and the presets
 * all land exactly on the cap, so equality is the test; a figure ABOVE the
 * cap is not all in, it is illegal (raiseProblem says "You have X behind"),
 * and the case where that happens is a short stack whose legal floor exceeds
 * its stack, where the button must not read "All in 0.50" over "You have
 * 0.35 behind".
 */
export function isAllInRaise(value, ctx) {
  return toInt(value) === raiseCap(ctx);
}

const MIN = Object.freeze({ id: 'min', label: 'Min', hint: 'The minimum raise' });
const POT = Object.freeze({ id: 'pot', label: 'Pot', hint: 'Call, then raise by the pot' });
const ALL_IN = Object.freeze({ id: 'allin', label: 'All in', hint: 'Everything' });

/** The post-flop row: pot fractions. Keys 1-5. */
export const POSTFLOP_PRESETS = withKeys([
  MIN,
  Object.freeze({ id: 'half', label: '½ Pot', hint: 'Call, then raise by half the pot' }),
  Object.freeze({ id: 'twoThirds', label: '⅔ Pot', hint: 'Call, then raise by two thirds of the pot' }),
  POT,
  ALL_IN,
]);

/** The pre-flop row: multiples of the bet in front (or the big blind). Keys 1-6. */
export const PREFLOP_PRESETS = withKeys([
  MIN,
  Object.freeze({ id: 'x2_5', label: '2.5x', hint: '2.5 times the bet in front, or the big blind' }),
  Object.freeze({ id: 'x3', label: '3x', hint: '3 times the bet in front, or the big blind' }),
  Object.freeze({ id: 'x4', label: '4x', hint: '4 times the bet in front, or the big blind' }),
  POT,
  ALL_IN,
]);

/** The default row (post-flop), kept for callers that do not know the street. */
export const PRESETS = POSTFLOP_PRESETS;

/** The row for a street: 'PreFlop' gets the multiples, everything else the fractions. */
export function presetsForPhase(phaseKey) {
  return phaseKey === 'PreFlop' ? PREFLOP_PRESETS : POSTFLOP_PRESETS;
}

const MULTIPLES = Object.freeze({ x2_5: 2.5, x3: 3, x4: 4 });

/**
 * The raise-to figure a preset proposes: quantised to the display unit (down),
 * then clamped to the legal range. All in is the exact cap, never quantised.
 * @param {string} id one of PREFLOP_PRESETS[].id or POSTFLOP_PRESETS[].id
 * @param {SizingContext} ctx
 */
export function presetTarget(id, ctx) {
  const floor = raiseFloor(ctx);
  const cap = raiseCap(ctx);
  if (id === 'allin') return clampRaise(cap, floor, cap);
  const bet = toInt(ctx.currentBet);
  const potAfterCall = toInt(ctx.pot) + toInt(ctx.callAmount);
  let target = floor;
  if (id === 'half') target = bet + potAfterCall / 2;
  else if (id === 'twoThirds') target = bet + (2 * potAfterCall) / 3;
  else if (id === 'pot') target = bet + potAfterCall;
  else if (MULTIPLES[id]) target = MULTIPLES[id] * Math.max(bet, toInt(ctx.bigBlind));
  return snapRaise(Math.floor(target), floor, cap, ctx.quantum);
}

/** Every preset's target at once, keyed by id, for the row given (default post-flop). */
export function presetTargets(ctx, presets = POSTFLOP_PRESETS) {
  return Object.fromEntries(presets.map((p) => [p.id, presetTarget(p.id, ctx)]));
}

/** Which preset (if any) the current value sits on, for the pressed look. */
export function presetAt(value, ctx, presets = POSTFLOP_PRESETS) {
  const targets = presetTargets(ctx, presets);
  const v = toInt(value);
  const hit = presets.find((p) => targets[p.id] === v);
  return hit ? hit.id : null;
}

/**
 * The slider's step: ONE DISPLAY UNIT. A range input snaps its value to
 * min + k * step, so any coarser step makes the range report a figure the
 * state does not hold (measured: a 0.03 step read 0.51 where the 2.5x preset
 * had set 0.50, and the harness read the range). One display unit reaches
 * every quantised proposal exactly; the +/- steppers walk the big blind.
 */
export function sliderStep(minRaise, quantum = 1) {
  void minRaise;
  return Math.max(1, toInt(quantum));
}

/**
 * Step the value by one big blind in either direction, on the display grid,
 * clamped. Stepping from a value that is not on the blind grid lands on the
 * next grid point past it, so repeated presses walk the grid rather than
 * carrying an odd offset.
 */
export function stepByBlind(value, bigBlind, direction, floor, cap, quantum = 1) {
  const bb = Math.max(1, toInt(bigBlind));
  const v = toInt(value);
  const dir = direction < 0 ? -1 : 1;
  const onGrid = v % bb === 0;
  const next = onGrid
    ? v + dir * bb
    : dir > 0 ? Math.ceil(v / bb) * bb : Math.floor(v / bb) * bb;
  return snapRaise(next, floor, cap, quantum);
}

/** 'bet' when nothing is in front of the player, otherwise 'raise'. */
export function raiseKind(currentBet) {
  return toInt(currentBet) === 0 ? 'bet' : 'raise';
}

/**
 * Parse what the player typed into the amount field. Returns an integer in the
 * smallest unit ON THE DISPLAY GRID (a third decimal typed on a two-decimal
 * table is dropped, so what the button shows is what is sent), or null for
 * anything that is not a plain positive number.
 */
export function parseAmountInput(text, { isBTC = false, decimals = 2 } = {}) {
  const cleaned = String(text ?? '').replace(/[,\s]/g, '');
  if (!/^\d*(?:\.\d*)?$/.test(cleaned) || cleaned === '' || cleaned === '.') return null;
  const n = Number(cleaned);
  if (!Number.isFinite(n) || n < 0) return null;
  if (isBTC) return Math.round(n);
  return quantise(Math.round(n * E8S_PER_ICP), displayQuantum({ isBTC, decimals }));
}

/**
 * Whether what the player typed carried more precision than the display grid
 * keeps: "0.075" on a two-decimal table parses to 0.07, and the field should
 * say so ("sizes in 0.01") rather than look as if it ate a digit. False for
 * anything unparseable (the field has other words for that) and for BTC
 * (sats are integral; a fraction of a sat is the same case, and is reported).
 */
export function typedPrecisionDropped(text, { isBTC = false, decimals = 2 } = {}) {
  const parsed = parseAmountInput(text, { isBTC, decimals });
  if (parsed === null) return false;
  const cleaned = String(text ?? '').replace(/[,\s]/g, '');
  const n = Number(cleaned);
  if (isBTC) return !Number.isInteger(n);
  return Math.round(n * E8S_PER_ICP) !== parsed;
}

/** The display unit as the field would write it: "0.01" at two decimals, "1" for sats. */
export function displayUnitLabel({ isBTC = false, decimals = 2 } = {}) {
  return formatAmountInput(displayQuantum({ isBTC, decimals }), { isBTC, decimals });
}

/**
 * How many decimals a smallest-unit ICP figure needs to be written EXACTLY:
 * the display's own count when it is on the grid, more (up to eight) when it
 * is not. An off-grid figure is only ever the stack cap; every proposal is on
 * the grid.
 */
export function exactDecimals(value, decimals = 2) {
  const n = Math.abs(toInt(value));
  const floor = Math.max(0, Math.min(ICP_DECIMALS, Math.trunc(Number(decimals) || 0)));
  let d = floor;
  while (d < ICP_DECIMALS && n % 10 ** (ICP_DECIMALS - d) !== 0) d += 1;
  return d;
}

/**
 * Format a smallest-unit amount for the amount field (no thousands
 * separators), with every digit the figure has: "1.23" on the grid,
 * "1.23456789" for an off-grid cap, so the field never shows a rounded
 * figure for a value that would be sent unrounded.
 */
export function formatAmountInput(value, { isBTC = false, decimals = 2 } = {}) {
  const n = toInt(value);
  if (isBTC) return String(n);
  return (n / E8S_PER_ICP).toFixed(exactDecimals(n, decimals));
}

/**
 * A smallest-unit amount as the dock writes it (thousands separators, the
 * table's decimals) but EXACT: an off-grid figure keeps every digit it has.
 * This is the figure on the primary button, which is the figure sent.
 */
export function formatExact(value, { isBTC = false, decimals = 2 } = {}) {
  const n = toInt(value);
  if (isBTC) return n.toLocaleString('en-US');
  const d = exactDecimals(n, decimals);
  return (n / E8S_PER_ICP).toLocaleString('en-US', {
    minimumFractionDigits: d,
    maximumFractionDigits: d,
  });
}

/**
 * The primary button's words for the sizer's figure: "All in" at the cap,
 * "Bet" with nothing in front, "Raise to" otherwise, and the exact figure.
 * @param {number} value the raise-to figure in the smallest unit
 * @param {SizingContext} ctx
 * @param {{ isBTC?: boolean, decimals?: number }} [display]
 * @returns {{ word: 'All in'|'Bet'|'Raise to', figure: string, text: string, allIn: boolean }}
 */
export function primaryRaiseLabel(value, ctx, display = {}) {
  const allIn = isAllInRaise(value, ctx);
  const word = allIn ? 'All in' : raiseKind(ctx.currentBet) === 'bet' ? 'Bet' : 'Raise to';
  const figure = formatExact(value, display);
  return { word, figure, text: `${word} ${figure}`, allIn };
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

function withKeys(list) {
  return Object.freeze(list.map((p, i) => Object.freeze({ ...p, key: String(i + 1) })));
}

function toInt(v) {
  const n = Number(v ?? 0);
  return Number.isFinite(n) ? Math.trunc(n) : 0;
}
