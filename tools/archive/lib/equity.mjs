// Equity: what share of a pot a set of known hands is worth, over the board
// cards that had not been turned over yet.
//
// WHICH CARDS COUNT AS UNKNOWN, AND WHY IT MATTERS
//
// This tool can see the whole deck, so "what would have come" is not a
// probability at all: the seed already fixed it. Equity is therefore a
// COUNTERFACTUAL, and a counterfactual needs its randomisation stated or the
// number means nothing.
//
// The default here is the one every poker tool uses and the only one that
// answers "was this a good spot": uniform over the cards NOT VISIBLE TO THE
// PLAYERS AT THAT MOMENT -- the 52 minus the board so far minus the contesting
// players' hole cards. Folded players' cards and burn cards stay in the pool,
// because from the table's point of view any of them could have landed.
//
// `deadCards` lets a caller remove more (the folded hands this tool reconstructed,
// say) to see how much that assumption is worth. It is not the default, and the
// output always says which pool was used.

import { score } from './evaluator.mjs';
import { buildDeck } from './cards.mjs';

/** Above this many hand evaluations, enumeration gives way to sampling. */
export const EXACT_EVAL_BUDGET = 6_000_000;

function* combinations(pool, k) {
  const idx = Array.from({ length: k }, (_, i) => i);
  const n = pool.length;
  if (k === 0) { yield []; return; }
  for (;;) {
    yield idx.map((i) => pool[i]);
    let i = k - 1;
    while (i >= 0 && idx[i] === n - k + i) i -= 1;
    if (i < 0) return;
    idx[i] += 1;
    for (let j = i + 1; j < k; j += 1) idx[j] = idx[j - 1] + 1;
  }
}

export function countCombinations(n, k) {
  if (k < 0 || k > n) return 0;
  let c = 1;
  for (let i = 0; i < k; i += 1) c = (c * (n - i)) / (i + 1);
  return Math.round(c);
}

/** mulberry32, so a sampled equity is reproducible from its seed. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6D2B79F5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * @param {Array<[string,string]>} hands contesting hole cards, in a fixed order
 * @param {string[]} board cards already out (0, 3, 4 or 5)
 * @param {{deadCards?: string[], samples?: number, seed?: number}} [opts]
 * @returns {{equity:number[], wins:number[], ties:number[], boards:number, method:'exact'|'sampled', stderr:number|null, pool:number}}
 */
export function equity(hands, board, { deadCards = [], samples = 200_000, seed = 1 } = {}) {
  if (hands.length < 2) throw new Error('equity(): need at least two contesting hands');
  const seen = new Set([...board, ...hands.flat(), ...deadCards]);
  const pool = buildDeck().filter((c) => !seen.has(c));
  const need = 5 - board.length;
  if (need < 0) throw new Error(`equity(): board already has ${board.length} cards`);

  const n = hands.length;
  const wins = new Array(n).fill(0);
  const ties = new Array(n).fill(0);
  const share = new Array(n).fill(0);

  const settle = (full) => {
    let best = -1;
    let bestCount = 0;
    const scores = new Array(n);
    for (let i = 0; i < n; i += 1) {
      const s = score([...hands[i], ...full]);
      scores[i] = s;
      if (s > best) { best = s; bestCount = 1; } else if (s === best) bestCount += 1;
    }
    for (let i = 0; i < n; i += 1) {
      if (scores[i] !== best) continue;
      share[i] += 1 / bestCount;
      if (bestCount === 1) wins[i] += 1; else ties[i] += 1;
    }
  };

  const total = countCombinations(pool.length, need);
  const exact = total * n <= EXACT_EVAL_BUDGET;

  let boards = 0;
  if (exact) {
    for (const extra of combinations(pool, need)) {
      settle([...board, ...extra]);
      boards += 1;
    }
  } else {
    const rand = rng(seed);
    const work = pool.slice();
    for (let s = 0; s < samples; s += 1) {
      // Partial Fisher-Yates: draw `need` distinct cards.
      for (let i = 0; i < need; i += 1) {
        const j = i + Math.floor(rand() * (work.length - i));
        const t = work[i]; work[i] = work[j]; work[j] = t;
      }
      settle([...board, ...work.slice(0, need)]);
      boards += 1;
    }
  }

  const eq = share.map((s) => s / boards);
  // Binomial standard error of the largest equity, the honest error bar on a
  // sampled number. Exact enumeration has none.
  const stderr = exact ? null
    : Math.sqrt(Math.max(...eq) * (1 - Math.max(...eq)) / boards);

  return {
    equity: eq,
    wins: wins.map((w) => w / boards),
    ties: ties.map((t) => t / boards),
    boards,
    method: exact ? 'exact' : 'sampled',
    stderr,
    pool: pool.length,
  };
}
