/**
 * Optimistic echo: render the player's own intent as fact for the 1.35-1.75 s
 * a mainnet update takes, then let the chain reconcile it.
 *
 * The leaders paint the action tag on the seat and slide the chips out of the
 * stack on mouse-up; the server reply only reconciles. This module is the pure
 * part of that: a pending record built from the certified view at the moment
 * of the click, the projected hero figures while it is open, the test that a
 * later certified view has absorbed it, and the projection that keeps the dock
 * from re-enabling while it is open.
 *
 * THE RULE THAT MATTERS: nothing here changes a figure the chain reported. The
 * echo replaces the hero's own stack and bet with the values the chain WILL
 * report once the action lands, and only while the send is open; every other
 * figure on the felt stays the certified one. On `Err` the echo is dropped and
 * the certified figures are back on the next paint.
 *
 * Nothing here mutates its inputs.
 */

/**
 * @typedef {object} PendingAction
 * @property {'fold'|'check'|'call'|'raise'|'bet'|'allin'} kind
 * @property {number|null} amount   raise-to / bet total, smallest unit, or null
 * @property {number} seat
 * @property {number} handNumber
 * @property {number} sentAt        Date.now() at the click
 * @property {number} callAmount    what was owed at the click
 * @property {{ chips: number, currentBet: number, folded: boolean, allIn: boolean }} snapshot
 */

const KINDS = new Set(['fold', 'check', 'call', 'raise', 'bet', 'allin']);

/** The hero's own player record out of a table view, or null. */
export function heroRecord(view) {
  const seat = view?.my_seat?.length > 0 ? Number(view.my_seat[0]) : null;
  if (seat === null) return null;
  const opt = view.players?.[seat];
  const player = opt && opt.length > 0 ? opt[0] : null;
  return player ? { seat, player } : null;
}

/**
 * Build the pending record at the click. Returns null when the view cannot
 * support an echo (no seat, unknown kind), in which case the caller sends
 * without one.
 */
export function beginPending(kind, amount, view, now) {
  if (!KINDS.has(kind)) return null;
  const hero = heroRecord(view);
  if (!hero) return null;
  const { seat, player } = hero;
  return Object.freeze({
    kind,
    amount: amount === null || amount === undefined ? null : Number(amount),
    seat,
    handNumber: Number(view.hand_number ?? 0),
    sentAt: Number(now),
    callAmount: Number(view.call_amount ?? 0),
    snapshot: Object.freeze({
      chips: Number(player.chips ?? 0),
      currentBet: Number(player.current_bet ?? 0),
      folded: player.has_folded === true,
      allIn: player.is_all_in === true,
    }),
  });
}

/**
 * The hero figures to paint while the send is open: what the chain will show
 * once the action lands. `tag` is the seat's action word; `amountShown` is the
 * money in the tag, or null.
 */
export function echoFor(pending) {
  const { chips, currentBet } = pending.snapshot;
  switch (pending.kind) {
    case 'fold':
      return { chips, currentBet, folded: true, allIn: false, tag: 'Fold', amountShown: null };
    case 'check':
      return { chips, currentBet, folded: false, allIn: false, tag: 'Check', amountShown: null };
    case 'call': {
      const debit = Math.min(pending.callAmount, chips);
      return {
        chips: chips - debit, currentBet: currentBet + debit, folded: false,
        allIn: chips - debit === 0, tag: 'Call', amountShown: debit,
      };
    }
    case 'raise':
    case 'bet': {
      const total = Math.min(Number(pending.amount ?? 0), chips + currentBet);
      const add = Math.max(0, total - currentBet);
      return {
        chips: chips - add, currentBet: currentBet + add, folded: false,
        allIn: chips - add === 0, tag: pending.kind === 'bet' ? 'Bet' : 'Raise to', amountShown: currentBet + add,
      };
    }
    case 'allin':
      return { chips: 0, currentBet: currentBet + chips, folded: false, allIn: true, tag: 'All in', amountShown: currentBet + chips };
    default:
      return { chips, currentBet, folded: false, allIn: false, tag: null, amountShown: null };
  }
}

/** The quiet 'sent' wording for the action bar. */
export function sentLabel(pending, fmt) {
  const echo = echoFor(pending);
  switch (pending.kind) {
    case 'fold': return 'Folding';
    case 'check': return 'Checking';
    case 'call': return `Calling ${fmt(echo.amountShown)}`;
    case 'bet': return `Betting ${fmt(echo.amountShown)}`;
    case 'raise': return `Raising to ${fmt(echo.amountShown)}`;
    case 'allin': return 'Going all in';
    default: return 'Sent';
  }
}

/**
 * Has a certified view absorbed the pending action? A view has when the hand
 * moved on, the clock left the hero's seat, or the hero's own record changed
 * from the snapshot taken at the click. A view that still shows the exact
 * pre-click state is a replica that has not seen the update yet: still open.
 * @returns {'open'|'absorbed'}
 */
export function pendingStatus(pending, view) {
  if (!pending || !view) return 'open';
  if (Number(view.hand_number ?? 0) !== pending.handNumber) return 'absorbed';
  if (view.is_my_turn !== true) return 'absorbed';
  if (Number(view.action_on) !== pending.seat) return 'absorbed';
  const hero = heroRecord(view);
  if (!hero) return 'absorbed';
  const { player } = hero;
  const s = pending.snapshot;
  if (Number(player.chips ?? 0) !== s.chips) return 'absorbed';
  if (Number(player.current_bet ?? 0) !== s.currentBet) return 'absorbed';
  if ((player.has_folded === true) !== s.folded) return 'absorbed';
  if ((player.is_all_in === true) !== s.allIn) return 'absorbed';
  return 'open';
}

/** A send older than this is treated as lost and rolled back with a message. */
export const PENDING_TTL_MS = 12_000;

export function pendingExpired(pending, now, ttlMs = PENDING_TTL_MS) {
  return !!pending && Number(now) - pending.sentAt > ttlMs;
}

/**
 * The view the app should render while a send is open: the certified view
 * with `is_my_turn` held false, so the dock stays in its sent state and a
 * pre-action cannot fire, until a view that has absorbed the action arrives.
 * Returns the same object when nothing is pending or the action is absorbed.
 */
export function projectPending(view, pending) {
  if (!view || !pending) return view;
  if (pendingStatus(pending, view) === 'absorbed') return view;
  if (view.is_my_turn !== true) return view;
  return { ...view, is_my_turn: false };
}

/**
 * A canister refusal, in words for the strip beside the buttons. The
 * canister's own messages are precise about WHY (docs/DEFECTS.md), so most
 * are kept; the ones that read like a stack trace are rephrased.
 */
export function humaneActionError(err) {
  const text = String(err?.message ?? err ?? '').trim();
  if (!text) return 'The table did not accept that action.';
  if (/timer expired|hand has moved on/i.test(text)) {
    return 'The clock ran out before your action arrived; the hand has moved on.';
  }
  if (/not your turn/i.test(text)) return 'Not your turn yet. Your action was not sent.';
  if (/rate limit/i.test(text)) return 'Too many actions in a row. Wait a moment and try again.';
  if (/signature|delegation|expired/i.test(text)) return 'Your session has expired. Sign in again to keep playing.';
  return text.replace(/\.$/, '') + '. Nothing was sent.';
}

/** The player record with the echo applied (a new object). */
export function echoedPlayer(player, pending) {
  if (!player || !pending) return player;
  const echo = echoFor(pending);
  return { ...player, chips: echo.chips, current_bet: echo.currentBet, has_folded: echo.folded, is_all_in: echo.allIn };
}
