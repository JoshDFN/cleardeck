/**
 * Pre-actions: the checkbox row a player arms while it is NOT their turn.
 *
 * The industry convention (PokerStars, GGPoker): while waiting, the player
 * pre-selects what they intend to do; the client sends the real action the
 * instant a certified state shows it is their turn AND the selection is still
 * legal; a selection that depended on the amount to call is dropped the moment
 * that amount changes (a raise behind them), so nobody calls a bigger bet than
 * the one they agreed to.
 *
 * Two contexts, three choices each:
 *
 *   nothing owed   Check / Fold     check if still free, else fold
 *                  Check            check if still free, else cancel
 *                  Call any         call whatever it is, check if free
 *   facing a bet   Fold to any bet  fold if still facing a bet, check if free
 *                  Call X           call exactly X, cancel if it changes
 *                  Call any         call whatever it is, check if free
 *
 * An armed choice is a plain object `{ id, callAmount, handNumber }` so the
 * rules can see what the player agreed to. Nothing here mutates anything, and
 * nothing here sends anything: the caller does, with a freshly polled state.
 */

/** @typedef {{ id: string, callAmount: number, handNumber: number }} ArmedPreAction */
/** @typedef {{ callAmount: number, canCheck: boolean, handNumber: number }} TurnFacts */

const CATALOGUE = Object.freeze({
  checkFold: Object.freeze({ id: 'checkFold', label: 'Check / Fold', hint: 'Check if free, otherwise fold' }),
  check: Object.freeze({ id: 'check', label: 'Check', hint: 'Check; cleared if a bet arrives' }),
  callAny: Object.freeze({ id: 'callAny', label: 'Call any', hint: 'Call any amount' }),
  foldAny: Object.freeze({ id: 'foldAny', label: 'Fold to any bet', hint: 'Fold if still facing a bet' }),
  call: Object.freeze({ id: 'call', label: 'Call', hint: 'Call this amount; cleared if it changes' }),
});

/**
 * The choices offered in the current context. `fmt` formats the call amount
 * for the Call button's label; the label is `Call X` so the harness can assert
 * it against call_amount like the primary Call button.
 * @param {{ callAmount: number, fmt: (v: number) => string }} facts
 */
export function availablePreActions({ callAmount, fmt }) {
  const owed = Number(callAmount ?? 0);
  if (owed <= 0) return [CATALOGUE.checkFold, CATALOGUE.check, CATALOGUE.callAny];
  return [
    CATALOGUE.foldAny,
    { ...CATALOGUE.call, label: `Call ${fmt(owed)}` },
    CATALOGUE.callAny,
  ];
}

/** Arm a choice against the facts it was made under. */
export function armPreAction(id, { callAmount, handNumber }) {
  if (!CATALOGUE[id]) return null;
  return Object.freeze({ id, callAmount: Number(callAmount ?? 0), handNumber: Number(handNumber ?? 0) });
}

/**
 * Should an armed choice survive a change in the certified state while it is
 * still not the hero's turn? Returns the (same) armed object to keep it, or
 * null to cancel it.
 * @param {ArmedPreAction|null} armed
 * @param {TurnFacts} facts
 */
export function keepPreAction(armed, facts) {
  if (!armed) return null;
  if (Number(facts.handNumber) !== armed.handNumber) return null;
  const owed = Number(facts.callAmount ?? 0);
  switch (armed.id) {
    case 'check':
      return owed > 0 ? null : armed;
    case 'call':
      return owed === armed.callAmount ? armed : null;
    default:
      // checkFold, foldAny and callAny are context-free promises.
      return armed;
  }
}

/**
 * Resolve an armed choice against a certified state that says it is the
 * hero's turn. Returns `{ fire, armed }`: `fire` is the action to send
 * ('fold' | 'check' | 'call') or null, and `armed` is what remains armed
 * (always null: a choice is one-shot, whether it fired or was cancelled).
 * @param {ArmedPreAction|null} armed
 * @param {TurnFacts & { isMyTurn: boolean }} facts
 */
export function resolvePreAction(armed, facts) {
  if (!armed || !facts.isMyTurn) return { fire: null, armed };
  const kept = keepPreAction(armed, facts);
  if (!kept) return { fire: null, armed: null };
  const owed = Number(facts.callAmount ?? 0);
  const free = facts.canCheck === true || owed === 0;
  let fire = null;
  switch (kept.id) {
    case 'checkFold':
    case 'foldAny':
      fire = free ? 'check' : 'fold';
      break;
    case 'check':
      fire = free ? 'check' : null;
      break;
    case 'call':
      fire = owed > 0 ? 'call' : null;
      break;
    case 'callAny':
      fire = free ? 'check' : 'call';
      break;
    default:
      fire = null;
  }
  return { fire, armed: null };
}

/** The short tag shown on the hero's plate while a choice is armed. */
export function armedTag(armed, fmt) {
  if (!armed) return null;
  if (armed.id === 'call') return `Call ${fmt(armed.callAmount)}`;
  return CATALOGUE[armed.id]?.label ?? null;
}

export const PRE_ACTION_CATALOGUE = CATALOGUE;
