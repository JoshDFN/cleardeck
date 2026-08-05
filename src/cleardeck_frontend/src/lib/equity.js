/**
 * ClearDeck equity engine — runs entirely in the player's browser.
 *
 * WHY THIS FILE EXISTS, AND WHAT IT IS ALLOWED TO KNOW
 * =====================================================
 * docs/DESIGN-BAR.md bar 15: the reference clients dramatise all-in with
 * INFORMATION, not fireworks — PokerStars shows `0%` / `100%` beside the two
 * players, GGPoker shows `100.00%` plus a named hand-strength phrase. ClearDeck
 * showed neither, and the wave-3 critic scored the all-in moment a loss for it.
 *
 * The reason it showed neither is that the table canister exposes no equity, and
 * the client is not allowed to invent data. That constraint has NOT been relaxed.
 * What changed is that equity is now COMPUTED here, from cards the viewer can
 * already see on their own screen, and every figure states its own method.
 *
 * THE INFORMATION RULE — the important part.
 * `get_table_view` sets `hole_cards: None` for every player the caller may not
 * see (src/table_canister/src/lib.rs:5152-5164 — self, or showdown-and-not-
 * folded, or a voluntary show). So this module CANNOT leak an opponent's hand:
 * the bytes are not in the response. It computes in exactly two modes, and the
 * caller must say which one it is in:
 *
 *   SHOWDOWN  every live player's cards were revealed BY THE ENGINE. Equity is
 *             a fact about known cards and unknown board runouts. Post-flop it
 *             is an EXACT enumeration of every remaining runout; with a complete
 *             board it is a one-runout enumeration, which is where GGPoker's
 *             `100.00%` comes from.
 *
 *   HERO      only the viewer's own two cards are known. There is no honest
 *             per-opponent equity to show and none is produced. What is produced
 *             is the viewer's equity against N hands drawn UNIFORMLY AT RANDOM
 *             from the remaining deck — a stated model, labelled as one on
 *             screen ("vs 2 random"), never called plain "equity". It is a
 *             function of the viewer's own two cards, the board, and the number
 *             of live opponents: three things already on their screen.
 *
 * There is deliberately no third mode. Nothing here reads a folded player's
 * cards, and nothing infers a range from betting.
 *
 * AGREEMENT WITH THE ENGINE THAT ACTUALLY PAYS THE POT
 * ====================================================
 * A hand ranker that disagrees with `poker_core` would put a number on the felt
 * that contradicts the canister's own showdown. `scoreSeven` here is a direct
 * 7-card evaluator; `poker_core::evaluate_hand` is a max over all 21 five-card
 * combinations of `HandRank`'s derived `Ord`. They are checked to agree, not
 * assumed to:
 *
 *   tools/shots/equity-differential.mjs  vs  tools/differential (poker_core)
 *     - all 2,598,960 five-card hands, JS score == Rust score, and
 *     - 2,000,000 random 7-card deals, JS best-of-7 == poker_core best-of-21.
 *
 * The score encoding below is a lossless flattening of `HandRank`, in the same
 * order as its derived `Ord` (variant discriminant first, then payload, which is
 * why `HighCard` must stay first and `RoyalFlush` last in that enum).
 */

// ---------------------------------------------------------------------------
// 1. Cards
// ---------------------------------------------------------------------------

/** Candid variant name -> rank value, matching `poker_core::card::Rank::value`. */
const RANK_VALUE = {
  Two: 2, Three: 3, Four: 4, Five: 5, Six: 6, Seven: 7, Eight: 8,
  Nine: 9, Ten: 10, Jack: 11, Queen: 12, King: 13, Ace: 14,
};

const SUIT_INDEX = { Hearts: 0, Diamonds: 1, Clubs: 2, Spades: 3 };

const RANK_WORD = {
  2: 'Two', 3: 'Three', 4: 'Four', 5: 'Five', 6: 'Six', 7: 'Seven', 8: 'Eight',
  9: 'Nine', 10: 'Ten', 11: 'Jack', 12: 'Queen', 13: 'King', 14: 'Ace',
};

const RANK_PLURAL = {
  2: 'Twos', 3: 'Threes', 4: 'Fours', 5: 'Fives', 6: 'Sixes', 7: 'Sevens',
  8: 'Eights', 9: 'Nines', 10: 'Tens', 11: 'Jacks', 12: 'Queens',
  13: 'Kings', 14: 'Aces',
};

/** The 52 distinct card codes, as `rank * 4 + suit`. */
export const DECK_SIZE = 52;

/** @param {number} code @returns {number} 2..14 */
const rankOf = (code) => (code >> 2) + 2;
/** @param {number} code @returns {number} 0..3 */
const suitOf = (code) => code & 3;

/**
 * A Candid `Card` (`{ rank: { Ace: null }, suit: { Spades: null } }`) as a
 * compact integer, or `null` if it is not a card.
 *
 * Tolerates the already-decoded `{ rank: 'Ace', suit: 'Spades' }` shape too, so
 * a Candid change degrades to a refusal to compute rather than to a wrong
 * number on the felt.
 *
 * @param {any} card
 * @returns {number|null} 0..51
 */
export function cardCode(card) {
  if (card === null || card === undefined) return null;
  if (typeof card === 'number') {
    return Number.isInteger(card) && card >= 0 && card < DECK_SIZE ? card : null;
  }
  const rawRank = card.rank;
  const rawSuit = card.suit;
  if (rawRank === undefined || rawSuit === undefined) return null;
  const rankName = typeof rawRank === 'string' ? rawRank : Object.keys(rawRank ?? {})[0];
  const suitName = typeof rawSuit === 'string' ? rawSuit : Object.keys(rawSuit ?? {})[0];
  const rank = RANK_VALUE[rankName];
  const suit = SUIT_INDEX[suitName];
  if (rank === undefined || suit === undefined) return null;
  return (rank - 2) * 4 + suit;
}

/**
 * A list of Candid cards as codes. Returns `null` if ANY entry fails to decode
 * or if the list contains a duplicate — both mean the caller's picture of the
 * table is wrong, and a wrong picture must not produce a confident percentage.
 *
 * @param {any[]} cards
 * @returns {number[]|null}
 */
export function cardCodes(cards) {
  if (!Array.isArray(cards)) return null;
  const out = [];
  const seen = new Set();
  for (const c of cards) {
    const code = cardCode(c);
    if (code === null || seen.has(code)) return null;
    seen.add(code);
    out.push(code);
  }
  return out;
}

// ---------------------------------------------------------------------------
// 2. Hand scoring — order-identical to poker_core::hand::HandRank's derived Ord
// ---------------------------------------------------------------------------

/**
 * Category indices. These are `HandRank`'s variant positions, and the derived
 * `Ord` compares by discriminant first, so this order is load-bearing.
 */
export const CATEGORY = {
  HIGH_CARD: 0, PAIR: 1, TWO_PAIR: 2, THREE_OF_A_KIND: 3, STRAIGHT: 4,
  FLUSH: 5, FULL_HOUSE: 6, FOUR_OF_A_KIND: 7, STRAIGHT_FLUSH: 8, ROYAL_FLUSH: 9,
};

export const CATEGORY_NAME = [
  'High Card', 'Pair', 'Two Pair', 'Three of a Kind', 'Straight',
  'Flush', 'Full House', 'Four of a Kind', 'Straight Flush', 'Royal Flush',
];

/**
 * Packs a category plus up to five tiebreak ranks into one comparable number.
 *
 * Six base-16 digits: `cat t1 t2 t3 t4 t5`. Every tiebreak is a rank 0..14, so
 * 16 is enough and the whole thing stays under 2^24 — an exact double, safely
 * comparable with `<`.
 */
function pack(cat, t1 = 0, t2 = 0, t3 = 0, t4 = 0, t5 = 0) {
  return ((((((cat * 16 + t1) * 16 + t2) * 16 + t3) * 16 + t4) * 16) + t5);
}

/** The category of a packed score. @param {number} score */
export const categoryOf = (score) => Math.floor(score / 16 ** 5);

/**
 * Highest card of a straight inside a rank bitmask, or 0.
 *
 * The mask has bit `r` set for rank r (2..14). The wheel is handled by also
 * testing an ace as a one, exactly as `poker_core::hand::detect_straight` does
 * with its explicit A-2-3-4-5 case.
 *
 * @param {number} mask
 * @returns {number} 5..14, or 0
 */
function straightHigh(mask) {
  // Ace plays low as well: an ace (bit 14) also sets the "rank one" bit, so the
  // wheel A-2-3-4-5 is found by the same window test as every other straight.
  const m = mask | (((mask >> 14) & 1) << 1);
  for (let high = 14; high >= 5; high -= 1) {
    const need = (1 << high) | (1 << (high - 1)) | (1 << (high - 2))
      | (1 << (high - 3)) | (1 << (high - 4));
    if ((m & need) === need) return high;
  }
  return 0;
}

/**
 * Ranks the best five-card poker hand inside 5, 6 or 7 cards.
 *
 * Mirrors `poker_core::hand::evaluate_five_cards_unchecked`'s decision order
 * (straight flush, quads, full house, flush, straight, trips, two pair, pair,
 * high card) and its tiebreaks, generalised to seven cards. `poker_core` gets
 * the same answer by taking the max over all 21 five-card subsets; that the two
 * agree is proved by differential test, not asserted here.
 *
 * @param {number[]} codes distinct card codes, length 5..7
 * @returns {number} packed score, comparable with `<`
 */
export function scoreHand(codes) {
  const rankCount = new Uint8Array(15);
  const suitCount = new Uint8Array(4);
  const suitMask = new Int32Array(4);
  let rankMask = 0;

  for (let i = 0; i < codes.length; i += 1) {
    const code = codes[i];
    const r = rankOf(code);
    const s = suitOf(code);
    rankCount[r] += 1;
    suitCount[s] += 1;
    suitMask[s] |= 1 << r;
    rankMask |= 1 << r;
  }

  // ---- straight flush / royal flush ----
  let flushSuit = -1;
  for (let s = 0; s < 4; s += 1) if (suitCount[s] >= 5) { flushSuit = s; break; }

  if (flushSuit >= 0) {
    const sfHigh = straightHigh(suitMask[flushSuit]);
    if (sfHigh === 14) return pack(CATEGORY.ROYAL_FLUSH);
    if (sfHigh > 0) return pack(CATEGORY.STRAIGHT_FLUSH, sfHigh);
  }

  // ---- rank groups, high to low ----
  let quad = 0;
  let trip1 = 0;
  let trip2 = 0;
  let pair1 = 0;
  let pair2 = 0;
  for (let r = 14; r >= 2; r -= 1) {
    const c = rankCount[r];
    if (c === 4) { if (quad === 0) quad = r; }
    else if (c === 3) { if (trip1 === 0) trip1 = r; else if (trip2 === 0) trip2 = r; }
    else if (c === 2) { if (pair1 === 0) pair1 = r; else if (pair2 === 0) pair2 = r; }
  }

  /** Highest `n` ranks present, skipping `exclude` ranks. */
  const kickers = (n, exclude) => {
    const out = [];
    for (let r = 14; r >= 2 && out.length < n; r -= 1) {
      if (rankCount[r] === 0 || exclude.includes(r)) continue;
      // A rank appearing k times contributes k kicker slots (poker_core sorts
      // the raw five ranks, so a pair among the kickers occupies two of them).
      for (let k = 0; k < rankCount[r] && out.length < n; k += 1) out.push(r);
    }
    while (out.length < n) out.push(0);
    return out;
  };

  // ---- four of a kind ----
  if (quad > 0) {
    // poker_core: kicker is the highest rank in the five that is not the quad.
    const k = kickers(1, [quad]);
    return pack(CATEGORY.FOUR_OF_A_KIND, quad, k[0]);
  }

  // ---- full house ----
  // With seven cards there can be two trips; the lower one plays as the pair,
  // which is what poker_core's best-of-21 picks.
  if (trip1 > 0 && (pair1 > 0 || trip2 > 0)) {
    const pairRank = trip2 > pair1 ? trip2 : pair1;
    return pack(CATEGORY.FULL_HOUSE, trip1, pairRank);
  }

  // ---- flush ----
  if (flushSuit >= 0) {
    const out = [];
    for (let r = 14; r >= 2 && out.length < 5; r -= 1) {
      if (suitMask[flushSuit] & (1 << r)) out.push(r);
    }
    return pack(CATEGORY.FLUSH, out[0], out[1], out[2], out[3], out[4]);
  }

  // ---- straight ----
  const sHigh = straightHigh(rankMask);
  if (sHigh > 0) return pack(CATEGORY.STRAIGHT, sHigh);

  // ---- three of a kind ----
  if (trip1 > 0) {
    const k = kickers(2, [trip1]);
    return pack(CATEGORY.THREE_OF_A_KIND, trip1, k[0], k[1]);
  }

  // ---- two pair ----
  if (pair1 > 0 && pair2 > 0) {
    const k = kickers(1, [pair1, pair2]);
    return pack(CATEGORY.TWO_PAIR, pair1, pair2, k[0]);
  }

  // ---- one pair ----
  if (pair1 > 0) {
    const k = kickers(3, [pair1]);
    return pack(CATEGORY.PAIR, pair1, k[0], k[1], k[2]);
  }

  // ---- high card ----
  const k = kickers(5, []);
  return pack(CATEGORY.HIGH_CARD, k[0], k[1], k[2], k[3], k[4]);
}

/**
 * Scores seven cards given as two hole cards plus a board, without allocating.
 * The hot path of every enumeration below.
 */
function scoreSeven(h0, h1, b0, b1, b2, b3, b4) {
  SEVEN[0] = h0; SEVEN[1] = h1; SEVEN[2] = b0; SEVEN[3] = b1;
  SEVEN[4] = b2; SEVEN[5] = b3; SEVEN[6] = b4;
  return scoreHand(SEVEN);
}
const SEVEN = new Int32Array(7);

// ---------------------------------------------------------------------------
// 3. Naming a hand, in words
// ---------------------------------------------------------------------------

/**
 * The named strength of a hand, GGPoker-style ("Two Pair, Aces and Nines").
 *
 * Works with 2 cards (pre-flop, where the only honest statement is what you are
 * holding) through 7. Returns `null` when there is nothing to name.
 *
 * @param {number[]} codes distinct card codes
 * @returns {{ name: string, category: number|null }|null}
 */
export function describeHand(codes) {
  if (!Array.isArray(codes) || codes.length < 2) return null;

  if (codes.length < 5) {
    // Pre-flop: name the holding, not a five-card hand that does not exist yet.
    const [a, b] = codes.slice(0, 2).sort((x, y) => rankOf(y) - rankOf(x));
    const ra = rankOf(a);
    const rb = rankOf(b);
    if (ra === rb) return { name: `Pocket ${RANK_PLURAL[ra]}`, category: null };
    const suited = suitOf(a) === suitOf(b) ? 'suited' : 'offsuit';
    return { name: `${RANK_WORD[ra]}-${RANK_WORD[rb]} ${suited}`, category: null };
  }

  const score = scoreHand(codes);
  const cat = categoryOf(score);
  const rankCount = new Uint8Array(15);
  const suitCount = new Uint8Array(4);
  const suitMask = new Int32Array(4);
  let rankMask = 0;
  for (const code of codes) {
    const r = rankOf(code);
    const s = suitOf(code);
    rankCount[r] += 1;
    suitCount[s] += 1;
    suitMask[s] |= 1 << r;
    rankMask |= 1 << r;
  }

  const groups = (n) => {
    const out = [];
    for (let r = 14; r >= 2; r -= 1) if (rankCount[r] === n) out.push(r);
    return out;
  };
  const topKicker = (exclude) => {
    for (let r = 14; r >= 2; r -= 1) if (rankCount[r] > 0 && !exclude.includes(r)) return r;
    return 0;
  };

  switch (cat) {
    case CATEGORY.ROYAL_FLUSH:
      return { name: 'Royal Flush', category: cat };
    case CATEGORY.STRAIGHT_FLUSH: {
      let suit = 0;
      for (let s = 0; s < 4; s += 1) if (suitCount[s] >= 5) suit = s;
      return { name: `Straight Flush, ${RANK_WORD[straightHigh(suitMask[suit])]} high`, category: cat };
    }
    case CATEGORY.FOUR_OF_A_KIND:
      return { name: `Four of a Kind, ${RANK_PLURAL[groups(4)[0]]}`, category: cat };
    case CATEGORY.FULL_HOUSE: {
      const trips = groups(3);
      const pairs = groups(2);
      const over = trips[0];
      const under = trips.length > 1 && trips[1] > (pairs[0] ?? 0) ? trips[1] : pairs[0];
      return { name: `Full House, ${RANK_PLURAL[over]} full of ${RANK_PLURAL[under]}`, category: cat };
    }
    case CATEGORY.FLUSH: {
      let suit = 0;
      for (let s = 0; s < 4; s += 1) if (suitCount[s] >= 5) suit = s;
      let high = 0;
      for (let r = 14; r >= 2; r -= 1) if (suitMask[suit] & (1 << r)) { high = r; break; }
      return { name: `Flush, ${RANK_WORD[high]} high`, category: cat };
    }
    case CATEGORY.STRAIGHT:
      return { name: `Straight, ${RANK_WORD[straightHigh(rankMask)]} high`, category: cat };
    case CATEGORY.THREE_OF_A_KIND:
      return { name: `Three of a Kind, ${RANK_PLURAL[groups(3)[0]]}`, category: cat };
    case CATEGORY.TWO_PAIR: {
      const pairs = groups(2);
      return { name: `Two Pair, ${RANK_PLURAL[pairs[0]]} and ${RANK_PLURAL[pairs[1]]}`, category: cat };
    }
    case CATEGORY.PAIR: {
      const p = groups(2)[0];
      return { name: `Pair of ${RANK_PLURAL[p]}, ${RANK_WORD[topKicker([p])]} kicker`, category: cat };
    }
    default:
      return { name: `${RANK_WORD[topKicker([])]} high`, category: cat };
  }
}

// ---------------------------------------------------------------------------
// 4. Deterministic randomness
// ---------------------------------------------------------------------------

/**
 * xorshift32 seeded from a string.
 *
 * DETERMINISM IS A REQUIREMENT, not a convenience. A Monte Carlo figure that
 * moves every poll would make the badge flicker while the player stares at it,
 * and would make the screenshot gate non-reproducible. The seed is derived from
 * the exact card set being evaluated, so the same table state always yields the
 * same number, on every machine, forever.
 */
function makeRng(seedString) {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < seedString.length; i += 1) {
    h ^= seedString.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  let x = (h || 0x9e3779b9) >>> 0;
  return () => {
    x ^= x << 13; x >>>= 0;
    x ^= x >>> 17;
    x ^= x << 5; x >>>= 0;
    return x;
  };
}

// ---------------------------------------------------------------------------
// 5. Equity
// ---------------------------------------------------------------------------

/**
 * Runouts remaining for `k` more board cards out of `n` unknown cards.
 * @returns {number}
 */
export function runoutCount(n, k) {
  if (k <= 0) return 1;
  if (k > n) return 0;
  let c = 1;
  for (let i = 0; i < k; i += 1) c = (c * (n - i)) / (i + 1);
  return Math.round(c);
}

/** Splits one runout's pot share across the winners of that runout. */
function award(scores, share, live) {
  let best = -1;
  let winners = 0;
  for (let i = 0; i < live; i += 1) {
    const s = scores[i];
    if (s > best) { best = s; winners = 1; } else if (s === best) { winners += 1; }
  }
  for (let i = 0; i < live; i += 1) {
    if (scores[i] === best) share[i] += 1 / winners;
  }
  return winners;
}

/**
 * EXACT equity for a set of KNOWN hands over every remaining board runout.
 *
 * "Equity" here is the standard definition and the one the pot actually pays:
 * the share of runouts a player wins, plus a chopped share of the runouts they
 * tie. With a complete board there is exactly one runout, so the answer is
 * 100 / 0 — which is the `100.00%` GGPoker prints at an all-in showdown.
 *
 * @param {number[][]} hands one two-card array per live player
 * @param {number[]} board 0, 3, 4 or 5 board cards
 * @returns {{ method:'exact', trials:number, share:number[] }}
 */
export function exactEquity(hands, board) {
  const live = hands.length;
  const used = new Uint8Array(DECK_SIZE);
  for (const hand of hands) for (const c of hand) used[c] = 1;
  for (const c of board) used[c] = 1;
  const deck = [];
  for (let c = 0; c < DECK_SIZE; c += 1) if (!used[c]) deck.push(c);

  const need = 5 - board.length;
  const share = new Float64Array(live);
  const scores = new Float64Array(live);
  const full = [board[0] ?? -1, board[1] ?? -1, board[2] ?? -1, board[3] ?? -1, board[4] ?? -1];
  let trials = 0;

  const evaluate = () => {
    for (let i = 0; i < live; i += 1) {
      scores[i] = scoreSeven(hands[i][0], hands[i][1], full[0], full[1], full[2], full[3], full[4]);
    }
    award(scores, share, live);
    trials += 1;
  };

  // A hand-rolled nest per depth beats a generic recursive combinator by enough
  // to keep the whole flop case (C(45,2) = 990) inside one animation frame.
  const n = deck.length;
  if (need === 0) {
    evaluate();
  } else if (need === 1) {
    for (let a = 0; a < n; a += 1) { full[4] = deck[a]; evaluate(); }
  } else if (need === 2) {
    for (let a = 0; a < n; a += 1) {
      full[3] = deck[a];
      for (let b = a + 1; b < n; b += 1) { full[4] = deck[b]; evaluate(); }
    }
  } else if (need === 5) {
    for (let a = 0; a < n; a += 1) {
      full[0] = deck[a];
      for (let b = a + 1; b < n; b += 1) {
        full[1] = deck[b];
        for (let c = b + 1; c < n; c += 1) {
          full[2] = deck[c];
          for (let d = c + 1; d < n; d += 1) {
            full[3] = deck[d];
            for (let e = d + 1; e < n; e += 1) { full[4] = deck[e]; evaluate(); }
          }
        }
      }
    }
  } else {
    throw new Error(`equity: a Hold'em board is 0, 3, 4 or 5 cards, got ${board.length}`);
  }

  return { method: 'exact', trials, share: Array.from(share, (s) => s / trials) };
}

/**
 * MONTE CARLO equity for a set of KNOWN hands.
 *
 * Used only when an exact enumeration would not fit in a frame — in Hold'em that
 * is the pre-flop all-in, where C(48,5) = 1,712,304 runouts. The trial count is
 * reported alongside the figure so nobody reads an estimate as exact.
 *
 * @param {number[][]} hands
 * @param {number[]} board
 * @param {number} trials
 * @param {string} seed
 */
export function monteCarloEquity(hands, board, trials, seed) {
  const live = hands.length;
  const used = new Uint8Array(DECK_SIZE);
  for (const hand of hands) for (const c of hand) used[c] = 1;
  for (const c of board) used[c] = 1;
  const deck = [];
  for (let c = 0; c < DECK_SIZE; c += 1) if (!used[c]) deck.push(c);

  const need = 5 - board.length;
  const share = new Float64Array(live);
  const scores = new Float64Array(live);
  const full = [board[0] ?? -1, board[1] ?? -1, board[2] ?? -1, board[3] ?? -1, board[4] ?? -1];
  const pool = Int32Array.from(deck);
  const rng = makeRng(seed);
  const n = pool.length;

  for (let t = 0; t < trials; t += 1) {
    // Partial Fisher-Yates: only the `need` cards actually drawn are shuffled.
    for (let i = 0; i < need; i += 1) {
      const j = i + (rng() % (n - i));
      const tmp = pool[i]; pool[i] = pool[j]; pool[j] = tmp;
      full[board.length + i] = pool[i];
    }
    for (let i = 0; i < live; i += 1) {
      scores[i] = scoreSeven(hands[i][0], hands[i][1], full[0], full[1], full[2], full[3], full[4]);
    }
    award(scores, share, live);
  }

  return { method: 'monte-carlo', trials, share: Array.from(share, (s) => s / trials) };
}

/**
 * HERO-ONLY equity against opponents drawn uniformly at random.
 *
 * THE MODEL IS THE POINT, AND IT IS STATED. This is not "the hero's equity" —
 * it is the hero's equity if every live opponent's two cards were a uniform draw
 * from the cards the hero cannot see. It uses ONLY the hero's own two cards, the
 * board, and the number of live opponents, so it cannot leak a hand the canister
 * withheld, and it is exactly as derivable as counting the players.
 *
 * @param {number[]} hero two card codes
 * @param {number[]} board
 * @param {number} opponents live opponents, >= 1
 * @param {number} trials
 * @param {string} seed
 */
export function heroEquityVsRandom(hero, board, opponents, trials, seed) {
  const used = new Uint8Array(DECK_SIZE);
  for (const c of hero) used[c] = 1;
  for (const c of board) used[c] = 1;
  const deck = [];
  for (let c = 0; c < DECK_SIZE; c += 1) if (!used[c]) deck.push(c);

  const need = 5 - board.length;
  const draw = need + opponents * 2;
  if (draw > deck.length) return null;

  const pool = Int32Array.from(deck);
  const rng = makeRng(seed);
  const n = pool.length;
  const full = [board[0] ?? -1, board[1] ?? -1, board[2] ?? -1, board[3] ?? -1, board[4] ?? -1];
  const scores = new Float64Array(opponents + 1);
  const share = new Float64Array(opponents + 1);

  for (let t = 0; t < trials; t += 1) {
    for (let i = 0; i < draw; i += 1) {
      const j = i + (rng() % (n - i));
      const tmp = pool[i]; pool[i] = pool[j]; pool[j] = tmp;
    }
    for (let i = 0; i < need; i += 1) full[board.length + i] = pool[i];
    scores[0] = scoreSeven(hero[0], hero[1], full[0], full[1], full[2], full[3], full[4]);
    for (let o = 0; o < opponents; o += 1) {
      const a = pool[need + o * 2];
      const b = pool[need + o * 2 + 1];
      scores[o + 1] = scoreSeven(a, b, full[0], full[1], full[2], full[3], full[4]);
    }
    award(scores, share, opponents + 1);
  }

  return {
    method: 'monte-carlo',
    trials,
    equity: share[0] / trials,
    opponents,
  };
}

// ---------------------------------------------------------------------------
// 6. The one entry point the table uses
// ---------------------------------------------------------------------------

/**
 * How many runouts an exact enumeration may walk before falling back to Monte
 * Carlo. Every post-flop case in Hold'em is far under it (turn: <= 45 runouts,
 * flop: <= C(45,2) = 990, river: 1); only the pre-flop all-in exceeds it.
 */
export const EXACT_BUDGET = 120_000;

/**
 * Trials for the Monte Carlo fallback.
 *
 * 200,000 is a deliberate point on the accuracy/latency curve, not a round
 * number: the standard error of a proportion at n = 200,000 is at most
 * 0.5/sqrt(n) = 0.11 percentage points, which is why `formatEquity` prints one
 * decimal for a Monte Carlo figure and two only for an exact enumeration.
 * Measured cost is in the wave report; it runs once per distinct card set, not
 * per poll.
 */
export const MC_TRIALS = 200_000;

/**
 * Equity for one table state, in whichever of the two legal modes applies.
 *
 * @param {object} input
 * @param {{seat:number, cards:any[]}[]} input.revealed live players whose cards
 *   the ENGINE has already revealed to this viewer. Pass every live player, or
 *   none — a partial reveal is not a showdown.
 * @param {number} input.liveCount how many players are still in the hand
 * @param {{seat:number, cards:any[]}|null} input.hero the viewer's own seat
 * @param {any[]} input.board community cards
 * @returns {null | {
 *   mode: 'showdown'|'hero',
 *   method: 'exact'|'monte-carlo',
 *   trials: number,
 *   bySeat: Map<number, number>,
 *   heroEquity: number|null,
 *   opponents: number,
 *   note: string,
 * }}
 */
export function computeEquity({ revealed, liveCount, hero, board }) {
  const boardCodes = cardCodes(board ?? []);
  if (boardCodes === null || (boardCodes.length !== 0 && (boardCodes.length < 3 || boardCodes.length > 5))) {
    return null;
  }

  // ---- showdown: every live hand is on its face ----
  if (Array.isArray(revealed) && revealed.length >= 2 && revealed.length === liveCount) {
    const hands = [];
    const seats = [];
    const seen = new Set(boardCodes);
    let ok = true;
    for (const r of revealed) {
      const codes = cardCodes(r.cards ?? []);
      if (!codes || codes.length !== 2 || seen.has(codes[0]) || seen.has(codes[1])) { ok = false; break; }
      seen.add(codes[0]);
      seen.add(codes[1]);
      hands.push(codes);
      seats.push(Number(r.seat));
    }
    if (ok) {
      const unknown = DECK_SIZE - boardCodes.length - hands.length * 2;
      const runouts = runoutCount(unknown, 5 - boardCodes.length);
      const seed = `sd:${boardCodes.join(',')}|${hands.map((h) => h.join('-')).join('/')}`;
      const res = runouts <= EXACT_BUDGET
        ? exactEquity(hands, boardCodes)
        : monteCarloEquity(hands, boardCodes, MC_TRIALS, seed);
      const bySeat = new Map();
      seats.forEach((s, i) => bySeat.set(s, res.share[i]));
      const heroSeat = hero ? Number(hero.seat) : null;
      return {
        mode: 'showdown',
        method: res.method,
        trials: res.trials,
        bySeat,
        heroEquity: heroSeat !== null && bySeat.has(heroSeat) ? bySeat.get(heroSeat) : null,
        opponents: hands.length - 1,
        note: res.method === 'exact'
          ? `Exact: all ${res.trials.toLocaleString('en-US')} remaining board runout${res.trials === 1 ? '' : 's'} enumerated over the revealed hands.`
          : `Monte Carlo: ${res.trials.toLocaleString('en-US')} random runouts over the revealed hands.`,
      };
    }
  }

  // ---- hero only: opponents' cards are withheld by the canister ----
  if (hero) {
    const heroCodes = cardCodes(hero.cards ?? []);
    const opponents = Math.max(0, Number(liveCount ?? 0) - 1);
    if (heroCodes && heroCodes.length === 2 && opponents >= 1
        && !boardCodes.includes(heroCodes[0]) && !boardCodes.includes(heroCodes[1])) {
      const seed = `hero:${boardCodes.join(',')}|${heroCodes.join('-')}|${opponents}`;
      const res = heroEquityVsRandom(heroCodes, boardCodes, opponents, MC_TRIALS, seed);
      if (!res) return null;
      return {
        mode: 'hero',
        method: res.method,
        trials: res.trials,
        bySeat: new Map([[Number(hero.seat), res.equity]]),
        heroEquity: res.equity,
        opponents,
        note: `Monte Carlo, ${res.trials.toLocaleString('en-US')} trials. Your two cards and the board are known; `
          + `each of the ${opponents} live opponent${opponents === 1 ? '' : 's'} is dealt a hand drawn uniformly at random `
          + `from the ${DECK_SIZE - 2 - boardCodes.length} cards you cannot see. This is a model, not a read: `
          + `ClearDeck never shows you a hand the canister has not revealed.`,
      };
    }
  }

  return null;
}

/**
 * The percentage as the felt should print it.
 *
 * Two decimals for an exact enumeration, because it IS exact to two decimals
 * (and because `100.00%` is the reference figure, bar 15). ONE decimal for a
 * Monte Carlo run at 40,000 trials, whose standard error is ~0.25 points —
 * printing more digits than that would be a lie about precision.
 *
 * @param {number} share 0..1
 * @param {'exact'|'monte-carlo'} method
 */
export function formatEquity(share, method) {
  if (!Number.isFinite(share)) return '--';
  const pct = share * 100;
  return method === 'exact' ? `${pct.toFixed(2)}%` : `${pct.toFixed(1)}%`;
}
