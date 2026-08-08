// An independent poker hand evaluator.
//
// Written from the rules of Texas Hold'em, not from src/poker_core/src/hand.rs.
// It exists for two reasons:
//
//   1. Nothing here may depend on ClearDeck's own engine agreeing with itself.
//      When this evaluator and the canister's recorded `final_hand_rank` disagree
//      about a showdown, that is a finding, and a tool that reuses the canister's
//      evaluator can never notice it.
//   2. EV needs to score millions of run-outs, so it evaluates seven cards
//      directly instead of scoring all 21 five-card subsets.
//
// A hand becomes ONE integer that orders exactly like poker: bigger is better,
// equal means a genuine tie (a chop), never "close enough".

import { RANKS, SUITS } from './cards.mjs';

export const CATEGORY = {
  HIGH_CARD: 0, PAIR: 1, TWO_PAIR: 2, TRIPS: 3, STRAIGHT: 4,
  FLUSH: 5, FULL_HOUSE: 6, QUADS: 7, STRAIGHT_FLUSH: 8,
};

export const CATEGORY_NAME = [
  'high card', 'a pair', 'two pair', 'three of a kind', 'a straight',
  'a flush', 'a full house', 'four of a kind', 'a straight flush',
];

/** Packs a category and up to five ordered tiebreakers into one comparable integer. */
function pack(category, k1 = 0, k2 = 0, k3 = 0, k4 = 0, k5 = 0) {
  return ((((category * 15 + k1) * 15 + k2) * 15 + k3) * 15 + k4) * 15 + k5;
}

/** Unpacks a score for display. */
export function describe(score) {
  const parts = [];
  let s = score;
  for (let i = 0; i < 5; i += 1) { parts.unshift(s % 15); s = Math.floor(s / 15); }
  return { category: s, categoryName: CATEGORY_NAME[s], kickers: parts };
}

const WHEEL = (1 << 12) | (1 << 3) | (1 << 2) | (1 << 1) | (1 << 0); // A 5 4 3 2

/** Highest card of a five-in-a-row inside a rank bitmask, or 0. Ace plays low for the wheel. */
function straightHigh(mask) {
  for (let high = 14; high >= 6; high -= 1) {
    const need = (1 << (high - 2)) | (1 << (high - 3)) | (1 << (high - 4))
      | (1 << (high - 5)) | (1 << (high - 6));
    if ((mask & need) === need) return high;
  }
  return (mask & WHEEL) === WHEEL ? 5 : 0;
}

/** The top `n` set bits of a rank mask, as rank values, high first. */
function topRanks(mask, n) {
  const out = [];
  for (let r = 14; r >= 2 && out.length < n; r -= 1) if (mask & (1 << (r - 2))) out.push(r);
  return out;
}

/**
 * Scores 5, 6 or 7 cards. Every card must be distinct: a duplicate means the
 * caller built an impossible hand, and silently scoring it is how a fabricated
 * flush gets paid (docs/DEFECTS.md E-09 is the canister's version of this).
 *
 * @param {string[]} cards
 * @returns {number} comparable score
 */
export function score(cards) {
  if (cards.length < 5 || cards.length > 7) {
    throw new Error(`score(): need 5..7 cards, got ${cards.length}`);
  }
  const counts = new Array(15).fill(0);
  const suitMasks = [0, 0, 0, 0];
  const suitCounts = [0, 0, 0, 0];
  let mask = 0;
  let seen = 0n;

  for (const c of cards) {
    const r = RANKS.indexOf(c[0]);
    const s = SUITS.indexOf(c[1]);
    if (r < 0 || s < 0 || c.length !== 2) throw new Error(`score(): not a card: ${JSON.stringify(c)}`);
    const bit = 1n << BigInt(13 * s + r);
    if (seen & bit) throw new Error(`score(): duplicate card ${c} in ${cards.join(' ')}`);
    seen |= bit;
    const rank = r + 2;
    counts[rank] += 1;
    mask |= 1 << r;
    suitMasks[s] |= 1 << r;
    suitCounts[s] += 1;
  }

  // Flush family first: a straight flush outranks everything.
  for (let s = 0; s < 4; s += 1) {
    if (suitCounts[s] >= 5) {
      const sf = straightHigh(suitMasks[s]);
      if (sf) return pack(CATEGORY.STRAIGHT_FLUSH, sf);
      const [a, b, c, d, e] = topRanks(suitMasks[s], 5);
      return pack(CATEGORY.FLUSH, a, b, c, d, e);
    }
  }

  // Rank multiplicities, high first.
  const quads = []; const trips = []; const pairs = [];
  for (let r = 14; r >= 2; r -= 1) {
    if (counts[r] === 4) quads.push(r);
    else if (counts[r] === 3) trips.push(r);
    else if (counts[r] === 2) pairs.push(r);
  }

  if (quads.length > 0) {
    const q = quads[0];
    const kicker = topRanks(mask & ~(1 << (q - 2)), 1)[0] ?? 0;
    return pack(CATEGORY.QUADS, q, kicker);
  }
  if (trips.length > 0 && (pairs.length > 0 || trips.length > 1)) {
    const t = trips[0];
    const pairRank = trips.length > 1 ? Math.max(trips[1], pairs[0] ?? 0) : pairs[0];
    return pack(CATEGORY.FULL_HOUSE, t, pairRank);
  }

  const st = straightHigh(mask);
  if (st) return pack(CATEGORY.STRAIGHT, st);

  if (trips.length > 0) {
    const t = trips[0];
    const [k1, k2] = topRanks(mask & ~(1 << (t - 2)), 2);
    return pack(CATEGORY.TRIPS, t, k1 ?? 0, k2 ?? 0);
  }
  if (pairs.length >= 2) {
    const [p1, p2] = pairs;
    const kicker = topRanks(mask & ~(1 << (p1 - 2)) & ~(1 << (p2 - 2)), 1)[0] ?? 0;
    return pack(CATEGORY.TWO_PAIR, p1, p2, kicker);
  }
  if (pairs.length === 1) {
    const p = pairs[0];
    const [k1, k2, k3] = topRanks(mask & ~(1 << (p - 2)), 3);
    return pack(CATEGORY.PAIR, p, k1 ?? 0, k2 ?? 0, k3 ?? 0);
  }
  const [a, b, c, d, e] = topRanks(mask, 5);
  return pack(CATEGORY.HIGH_CARD, a, b, c, d, e);
}

/** Convenience: score two hole cards plus a board. */
export const scoreHand = (hole, board) => score([...hole, ...board]);

/**
 * The canister's recorded `HandRank` variant name for a score, so a
 * reconstruction can compare its own verdict with the archive's.
 * `RoyalFlush` is the canister's separate name for an ace-high straight flush.
 */
export function canisterRankName(s) {
  const { category, kickers } = describe(s);
  switch (category) {
    case CATEGORY.STRAIGHT_FLUSH: return kickers[0] === 14 ? 'RoyalFlush' : 'StraightFlush';
    case CATEGORY.QUADS: return 'FourOfAKind';
    case CATEGORY.FULL_HOUSE: return 'FullHouse';
    case CATEGORY.FLUSH: return 'Flush';
    case CATEGORY.STRAIGHT: return 'Straight';
    case CATEGORY.TRIPS: return 'ThreeOfAKind';
    case CATEGORY.TWO_PAIR: return 'TwoPair';
    case CATEGORY.PAIR: return 'Pair';
    default: return 'HighCard';
  }
}
