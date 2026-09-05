/**
 * What one row of the hand list says, derived from the normalised hand.
 *
 * A row is a glance, not a report: the result, the pot, your two cards, the
 * board, who else was in, when. Every field here is a restatement of the
 * record, and the two things the record does not carry are not invented:
 *
 *   - your NET result. The record has what you were paid, not what you put in
 *     (a raise's amount is a street total, hand-record.js), so the row states
 *     the pot and whether you won it, never "+0.72".
 *   - your cards when you folded. The table reveals hole cards only at a
 *     showdown, so a hand you folded shows two backs.
 */

import { participatedIn, seatOfPrincipal } from './hand-history-records.js';

/** The list's filters, in the order the chips show them. */
export const FILTERS = [
  { id: 'mine', label: 'My hands' },
  { id: 'all', label: 'Every hand here' },
  { id: 'won', label: 'Won' },
  { id: 'showdown', label: 'Showdowns' },
];

/**
 * @param {object} hand
 * @param {string|null} principal
 * @returns {'won'|'lost'|'out'|'open'}
 *   won: the record pays this principal; lost: in the hand, paid nothing;
 *   out: not in the record; open: the hand has no winners yet
 */
export function heroResult(hand, principal) {
  if (!(hand?.winners || []).length) return 'open';
  if (!participatedIn(hand, principal)) return 'out';
  const paid = (hand.winners || []).some((w) => w.principal === principal && w.amount > 0);
  return paid ? 'won' : 'lost';
}

export const RESULT_WORD = { won: 'Won', lost: 'Lost', out: 'Not in', open: 'In play' };

/** The two cards the record shows for this principal, or null. */
export function heroCards(hand, principal) {
  const shown = (hand?.showdown || []).find((p) => p.principal === principal && p.cards);
  if (shown) return [shown.cards[0], shown.cards[1]];
  const won = (hand?.winners || []).find((w) => w.principal === principal && w.cards);
  return won ? [won.cards[0], won.cards[1]] : null;
}

/** How many OTHER seats the record saw. */
export function opponentCount(hand, principal) {
  const seats = hand?.seats || [];
  const mine = seatOfPrincipal(hand, principal);
  return seats.filter((s) => s !== mine).length;
}

/** The verdict badge for a row, from the local verification. */
export function verdictOf(hand) {
  const v = hand?.verification;
  if (!v) return { tone: 'pending', label: 'seed not revealed' };
  if (v.ok) return { tone: 'good', label: `${v.cardsMatched} cards re-derived here` };
  if (v.commitment?.match && v.cardsChecked === 0) {
    return { tone: 'partial', label: 'commitment checked here' };
  }
  return { tone: 'bad', label: 'DID NOT verify' };
}

/**
 * The hands a filter keeps. `all` keeps everything; the others keep what the
 * record says about this principal, never hiding a hand silently (the chip
 * counts say how many each keeps).
 */
export function applyFilter(hands, filterId, principal) {
  const list = hands || [];
  switch (filterId) {
    case 'mine': return list.filter((h) => participatedIn(h, principal));
    case 'won': return list.filter((h) => heroResult(h, principal) === 'won');
    case 'showdown': return list.filter((h) => (h.showdown || []).length > 0);
    default: return list;
  }
}

/** Counts per filter, for the chips. */
export function filterCounts(hands, principal) {
  return Object.fromEntries(FILTERS.map((f) => [f.id, applyFilter(hands, f.id, principal).length]));
}
