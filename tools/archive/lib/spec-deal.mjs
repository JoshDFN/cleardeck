// Dealing order, from docs/SHUFFLE-SPEC.md section 4.
//
// The shuffled deck is consumed strictly front to back. The only input other
// than the deck is P, THE NUMBER OF SEATS THE DEAL FED -- and section 4 is
// emphatic that P is READ from the record's `dealt_in`, never counted from the
// player list. Counting it is FINDING 30: on any hand somebody left or joined,
// the count is wrong, the hole cards still match (they come off the front of the
// deck) and the board silently comes out wrong.
//
// So this module takes P as a number and refuses to invent one.

/**
 * Lays a shuffled deck out as a hand with `players` seats dealt in.
 *
 * @param {string[]} deck 52 cards, index 0 first
 * @param {number} players P, from `dealt_in.length`
 */
export function deal(deck, players) {
  if (!Number.isInteger(players) || players < 2) {
    throw new Error(`P must be an integer >= 2, got ${players}`);
  }
  if (2 * players + 8 > deck.length) {
    throw new Error(`a ${deck.length}-card deck cannot feed ${players} players plus a board`);
  }

  const holeCards = [];
  for (let k = 0; k < players; k += 1) holeCards.push([deck[2 * k], deck[2 * k + 1]]);

  const base = 2 * players;
  return {
    players,
    holeCards,
    burnBeforeFlop: deck[base],
    flop: [deck[base + 1], deck[base + 2], deck[base + 3]],
    burnBeforeTurn: deck[base + 4],
    turn: deck[base + 5],
    burnBeforeRiver: deck[base + 6],
    river: deck[base + 7],
    /** Where each published card came from, so the reconstruction can show its work. */
    indices: {
      hole: Array.from({ length: players }, (_, k) => [2 * k, 2 * k + 1]),
      burnBeforeFlop: base,
      flop: [base + 1, base + 2, base + 3],
      burnBeforeTurn: base + 4,
      turn: base + 5,
      burnBeforeRiver: base + 6,
      river: base + 7,
    },
  };
}

/**
 * How far the board got, from what the record published.
 * A hand that ended pre-flop has no flop, and reconstructing one and comparing
 * it to `null` would be a false mismatch.
 */
export function boardDepth(record) {
  if (record.river) return 5;
  if (record.turn) return 4;
  if (record.flop) return 3;
  return 0;
}
