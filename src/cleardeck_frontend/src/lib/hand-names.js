/**
 * The named hand on the hero's plate, compacted for a lower-third.
 *
 * `describeHand()` (equity.js) names a hand the long way ("Pair of Eights,
 * Ace kicker", "Full House, Aces full of Eights") and that is the right
 * sentence for a hand history. A plate row is 10 to 20 characters wide, and
 * an ellipsised hand name is worse than a shorter one: measured on the phone,
 * "Ace-Four offsuit" and "Pair of Eights, Ace kicker" both ended in "...".
 *
 * The grammar here only DELETES: the kicker clause goes, the category prefix
 * goes where the ranks say it already ("Aces full of Eights", "Aces and
 * Eights", "Three Sevens", "Four Sevens"), and the high card of a straight
 * flush goes. Nothing is abbreviated to a letter, so no digit is introduced
 * for the token census and the phrase stays a sentence a novice reads.
 * tools/shots/lib/chain-agreement.mjs accepts these shapes alongside the
 * long ones; keep the two in step.
 */

const RULES = [
  [/^Pair of (\w+), \w+ kicker$/, 'Pair of $1'],
  [/^Two Pair, (\w+) and (\w+)$/, '$1 and $2'],
  [/^Three of a Kind, (\w+)$/, 'Three $1'],
  [/^Four of a Kind, (\w+)$/, 'Four $1'],
  [/^Full House, (\w+) full of (\w+)$/, '$1 full of $2'],
  [/^Straight Flush, \w+ high$/, 'Straight Flush']
];

/**
 * @param {string|null|undefined} name  the long name from describeHand()
 * @returns {string|null}  the compact name, or null for no name
 */
export function compactHandName(name) {
  if (typeof name !== 'string') return null;
  const trimmed = name.trim();
  if (!trimmed) return null;
  const rule = RULES.find(([pattern]) => pattern.test(trimmed));
  return rule ? trimmed.replace(rule[0], rule[1]) : trimmed;
}
