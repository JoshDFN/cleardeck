/**
 * Each seat's LAST ACTION THIS STREET, as the one word the plate shows.
 *
 * On every reference client a seat that has acted this street says so on
 * its plate: CHECK, CALL, BET, RAISE. ClearDeck's plate carried the stack,
 * the fold (an X on the avatar and the word FOLD beside the stack) and the
 * all-in disc, and its bet disc shows chips that went in; a CHECK left no
 * trace anywhere but the log drawer, so a player reading the felt could not
 * tell a seat that had checked from one still to act.
 *
 * The source is the client's own action feed (PokerTable.svelte builds it
 * from the canister's `last_action` on every poll and inserts a STREET LINE
 * when the phase changes). The words are derived from that feed, never
 * invented: a seat whose action this client did not see gets no word. The
 * feed carries only what the chain reported, so the word is as certified as
 * the log line it came from.
 *
 * WHICH ENTRIES CARRY A WORD. Check, call, bet and raise. Not the fold: the
 * plate already says FOLD from the chain's own `has_folded`, and the fold
 * word must never disagree with it. Not the all-in: the avatar disc says
 * ALL IN from the chain's `is_all_in`. Not a blind post: posting is not a
 * decision. Not the showdown, winner or method lines.
 *
 * WHEN A WORD CLEARS. At the next street line: a new street starts with
 * every seat still to act. A word stays until then even after somebody
 * raises behind the seat (a seat that CALLED and now faces a raise still
 * shows CALL, the reference clients' grammar); the bet disc and the acting
 * ring say what is owed now. The hand boundary is the feed's own: the
 * feed is emptied on a new hand number, so a new hand has no words.
 *
 * Pure. Nothing here mutates the feed.
 */

/** The plate word for a feed entry's `type`, or null when the plate has its own. */
export const ACTION_WORDS = Object.freeze({
  check: 'Check',
  call: 'Call',
  bet: 'Bet',
  raise: 'Raise',
});

/**
 * @typedef {object} FeedEntry
 * @property {string} type       'fold'|'check'|'call'|'bet'|'raise'|'allin'|'blind'|'phase'|'showdown'|'winner'
 * @property {number} [seat]
 * @property {boolean} [street]  true on a phase line that opens a new street
 */

/**
 * The word each seat's plate should show, keyed by seat index, from the
 * feed's entries since the last street line.
 * @param {ReadonlyArray<FeedEntry>} feed
 * @returns {Map<number, string>}
 */
export function streetActionsOf(feed) {
  const words = new Map();
  if (!Array.isArray(feed)) return words;
  for (const entry of feed) {
    if (!entry) continue;
    if (entry.type === 'phase') {
      // Only a STREET line resets the plates: the feed also logs the equity
      // method as a phase-style line, and that is not a new street.
      if (entry.street === true) words.clear();
      continue;
    }
    const seat = Number(entry.seat);
    if (!Number.isInteger(seat) || seat < 0) continue;
    const word = ACTION_WORDS[entry.type];
    if (word) {
      words.set(seat, word);
    } else if (entry.type === 'fold' || entry.type === 'allin') {
      // The plate paints these from chain state; the word must not linger
      // beside them ("Call" next to an X, say).
      words.delete(seat);
    }
  }
  return words;
}
