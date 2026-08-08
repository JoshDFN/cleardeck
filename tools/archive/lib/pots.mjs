// Pot layering, and who is eligible for which layer.
//
// Written from the rule, not from src/poker_core/src/side_pots.rs: a player can
// only win as much from each opponent as they themselves put in. Money a folded
// player left behind stays in the layers it reached, and they are eligible for
// none of them.

/**
 * @param {Map<number, number>} contributed seat -> settled contribution
 * @param {Set<number>|number[]} eligibleSeats seats still live at the showdown
 * @returns {Array<{amount:number, eligible:number[]}>} layers, lowest first
 */
export function buildPots(contributed, eligibleSeats) {
  const live = new Set(eligibleSeats);
  const entries = [...contributed.entries()].filter(([, v]) => v > 0);
  const levels = [...new Set(entries.map(([, v]) => v))].sort((a, b) => a - b);
  const pots = [];
  let prev = 0;
  for (const level of levels) {
    const band = level - prev;
    if (band <= 0) { prev = level; continue; }
    const contributors = entries.filter(([, v]) => v >= level);
    const amount = band * contributors.length;
    const eligible = contributors.map(([s]) => s).filter((s) => live.has(s));
    if (amount > 0) pots.push({ amount, eligible });
    prev = level;
  }
  return pots;
}

/**
 * Awards layered pots to the best hand(s) in each, splitting evenly and giving the
 * indivisible remainder to the lowest seat index. The remainder rule matters only
 * for exactness of the total; it is stated so the number is reproducible.
 *
 * @param {Array<{amount:number, eligible:number[]}>} pots
 * @param {Map<number, number>} scores seat -> hand score (bigger is better)
 * @returns {Map<number, number>} seat -> chips won
 */
export function awardPots(pots, scores) {
  const won = new Map();
  const add = (seat, n) => won.set(seat, (won.get(seat) ?? 0) + n);
  for (const pot of pots) {
    const contenders = pot.eligible.filter((s) => scores.has(s));
    if (contenders.length === 0) continue;
    const best = Math.max(...contenders.map((s) => scores.get(s)));
    const winners = contenders.filter((s) => scores.get(s) === best).sort((a, b) => a - b);
    const share = Math.floor(pot.amount / winners.length);
    let remainder = pot.amount - share * winners.length;
    for (const w of winners) {
      add(w, share + (remainder > 0 ? 1 : 0));
      if (remainder > 0) remainder -= 1;
    }
  }
  return won;
}
