/**
 * DID THE HAND THAT JUST ENDED GO TO A SHOWDOWN? -- docs/DEFECTS.md E-107.
 *
 * `get_table_view` turns the unfolded seats face up at `Showdown`, and at
 * `HandComplete` only when the hand that just ended went to a showdown. A pot
 * won because everybody folded is taken without showing a hand, so between
 * hands the winner's `hole_cards` are null for everyone but the winner. The
 * felt has to read the phase the same way, or it dramatises a showdown (the
 * lit seats, the equity badges, the "shows a pair" log line) around a hand
 * nobody showed.
 *
 * The client derives the answer from what the view already carries rather
 * than a new wire field: the canister's own winner record (`push_winner`)
 * attaches `cards` only at a showdown -- a fold-out's winner is recorded with
 * `cards: null` -- so "any winner record carries cards" IS "the hand went to a
 * showdown". `last_hand_winners` is the hand that just ended for the whole of
 * `HandComplete`; it is only consulted in that phase.
 *
 * On the wire an `opt record { Card; Card }` is `[]` or `[[a, b]]`.
 */

/** @param {Array<{cards?: unknown}>|null|undefined} lastWinners */
export function lastHandWentToShowdown(lastWinners) {
  if (!Array.isArray(lastWinners)) return false;
  return lastWinners.some(w => Array.isArray(w?.cards) && w.cards.length > 0);
}

/**
 * Are the engine-revealed hands on the felt right now?
 *
 * @param {string|null|undefined} phaseKey the `GamePhase` variant name
 * @param {Array<{cards?: unknown}>|null|undefined} lastWinners the view's `last_hand_winners`
 */
export function handsAreFaceUp(phaseKey, lastWinners) {
  if (phaseKey === 'Showdown') return true;
  return phaseKey === 'HandComplete' && lastHandWentToShowdown(lastWinners);
}
