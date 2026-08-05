// AN INDEPENDENT EQUITY, so the percentage on the felt is CHECKED and not merely
// allowlisted.
//
// WHY THIS FILE EXISTS
// --------------------
// The inverted money gate (lib/token-census.mjs) says every number the client
// renders must be either compared with a canister figure or declared
// non-monetary in a reviewed list. An equity badge is neither: the canister
// exposes no equity, so there is nothing to compare it with, and a percentage a
// player uses to decide whether to call is not the same class of thing as a seat
// ordinal. Allowlisting it would have put a four-digit decimal on the felt that
// nothing on earth checks — exactly the hole the census was built to close.
//
// So the harness recomputes it. This module is a SECOND LINEAGE, deliberately
// written to a different shape than src/cleardeck_frontend/src/lib/equity.js:
//
//   equity.js       one direct 7-card pass: rank histogram + suit bitmasks,
//                   categories decided in place.
//   this file       poker_core's shape: enumerate all 21 five-card subsets,
//                   rank each one on its own, take the max. Different straight
//                   detector (sorted-dedup window, not a bitmask), different
//                   flush test, different tiebreak assembly.
//
// If both lineages agree on a real hand, on a real board, from a real canister
// read, the badge is not a decoration.
//
// The five-card ranking below reproduces `poker_core::hand::HandRank`'s ORDER,
// which is the order that decides who takes the pot. It does not have to
// reproduce poker_core's exact payloads, only its comparisons.

// ---------------------------------------------------------------------------
// cards
// ---------------------------------------------------------------------------

const RANK_VALUE = {
    Two: 2, Three: 3, Four: 4, Five: 5, Six: 6, Seven: 7, Eight: 8,
    Nine: 9, Ten: 10, Jack: 11, Queen: 12, King: 13, Ace: 14,
};
const SUIT_INDEX = { Hearts: 0, Diamonds: 1, Clubs: 2, Spades: 3 };

/** Candid `Card` -> `{ rank: 2..14, suit: 0..3 }`, or null. */
export function toCard(card) {
    if (!card) return null;
    const rankName = typeof card.rank === 'string' ? card.rank : Object.keys(card.rank ?? {})[0];
    const suitName = typeof card.suit === 'string' ? card.suit : Object.keys(card.suit ?? {})[0];
    const rank = RANK_VALUE[rankName];
    const suit = SUIT_INDEX[suitName];
    if (rank === undefined || suit === undefined) return null;
    return { rank, suit };
}

/** A whole deck as `{rank,suit}` objects, minus the cards already seen. */
function remainingDeck(seen) {
    const used = new Set(seen.map((c) => c.rank * 4 + c.suit));
    const out = [];
    for (let r = 2; r <= 14; r += 1) {
        for (let s = 0; s < 4; s += 1) {
            if (!used.has(r * 4 + s)) out.push({ rank: r, suit: s });
        }
    }
    return out;
}

// ---------------------------------------------------------------------------
// five-card ranking (poker_core's shape, written independently)
// ---------------------------------------------------------------------------

/** Straight high card among five ranks, or 0. Sorted-dedup window + wheel. */
function straightHighOfFive(ranks) {
    const s = [...new Set(ranks)].sort((a, b) => b - a);
    if (s.length < 5) return 0;
    if (s[0] === 14 && s[1] === 5 && s[2] === 4 && s[3] === 3 && s[4] === 2) return 5;
    for (let i = 0; i + 4 < s.length; i += 1) {
        if (s[i] - s[i + 4] === 4) return s[i];
    }
    return 0;
}

export const CATEGORY_NAMES = [
    'High Card', 'Pair', 'Two Pair', 'Three of a Kind', 'Straight',
    'Flush', 'Full House', 'Four of a Kind', 'Straight Flush', 'Royal Flush',
];

/**
 * Ranks exactly five cards as `[category, t1, t2, t3, t4, t5]`, comparable
 * lexicographically. Category indices are `HandRank`'s variant positions.
 */
export function rankFive(cards) {
    const ranks = cards.map((c) => c.rank).sort((a, b) => b - a);
    const suits = cards.map((c) => c.suit);
    const isFlush = suits.every((s) => s === suits[0]);
    const high = straightHighOfFive(ranks);

    if (isFlush && high === 14) return [9, 0, 0, 0, 0, 0];
    if (isFlush && high > 0) return [8, high, 0, 0, 0, 0];

    const counts = new Map();
    for (const r of ranks) counts.set(r, (counts.get(r) ?? 0) + 1);
    const by = (n) => [...counts.entries()].filter(([, c]) => c === n)
        .map(([r]) => r).sort((a, b) => b - a);
    const quads = by(4);
    const trips = by(3);
    const pairs = by(2);
    const kickersExcluding = (excl) => ranks.filter((r) => !excl.includes(r));

    if (quads.length) return [7, quads[0], kickersExcluding(quads)[0] ?? 0, 0, 0, 0];
    if (trips.length && pairs.length) return [6, trips[0], pairs[0], 0, 0, 0];
    if (isFlush) return [5, ranks[0], ranks[1], ranks[2], ranks[3], ranks[4]];
    if (high > 0) return [4, high, 0, 0, 0, 0];
    if (trips.length) {
        const k = kickersExcluding(trips);
        return [3, trips[0], k[0] ?? 0, k[1] ?? 0, 0, 0];
    }
    if (pairs.length >= 2) {
        const k = kickersExcluding([pairs[0], pairs[1]]);
        return [2, pairs[0], pairs[1], k[0] ?? 0, 0, 0];
    }
    if (pairs.length === 1) {
        const k = kickersExcluding(pairs);
        return [1, pairs[0], k[0] ?? 0, k[1] ?? 0, k[2] ?? 0, 0];
    }
    return [0, ranks[0], ranks[1], ranks[2], ranks[3], ranks[4]];
}

const cmp = (a, b) => {
    for (let i = 0; i < 6; i += 1) if (a[i] !== b[i]) return a[i] - b[i];
    return 0;
};

/** All 21 five-card subsets of seven, precomputed once. */
const SUBSETS_7C5 = (() => {
    const out = [];
    for (let a = 0; a < 7; a += 1)
        for (let b = a + 1; b < 7; b += 1)
            for (let c = b + 1; c < 7; c += 1)
                for (let d = c + 1; d < 7; d += 1)
                    for (let e = d + 1; e < 7; e += 1) out.push([a, b, c, d, e]);
    return out;
})();

/** The best five-card rank inside seven cards, poker_core-style. */
export function rankSeven(seven) {
    let best = null;
    const buf = [null, null, null, null, null];
    for (const idx of SUBSETS_7C5) {
        for (let i = 0; i < 5; i += 1) buf[i] = seven[idx[i]];
        const r = rankFive(buf);
        if (best === null || cmp(r, best) > 0) best = r;
    }
    return best;
}

/**
 * The CATEGORY the hero's best hand falls in, as a word. Fewer than five cards
 * has no five-card hand, so it returns null and the caller checks nothing.
 */
export function bestCategoryName(cards) {
    if (cards.length < 5) return null;
    let best = null;
    const n = cards.length;
    const buf = [null, null, null, null, null];
    for (let a = 0; a < n; a += 1)
        for (let b = a + 1; b < n; b += 1)
            for (let c = b + 1; c < n; c += 1)
                for (let d = c + 1; d < n; d += 1)
                    for (let e = d + 1; e < n; e += 1) {
                        buf[0] = cards[a]; buf[1] = cards[b]; buf[2] = cards[c];
                        buf[3] = cards[d]; buf[4] = cards[e];
                        const r = rankFive(buf);
                        if (best === null || cmp(r, best) > 0) best = r.slice();
                    }
    return best === null ? null : CATEGORY_NAMES[best[0]];
}

// ---------------------------------------------------------------------------
// equity
// ---------------------------------------------------------------------------

function splitPot(scores, share) {
    let best = null;
    for (const s of scores) if (best === null || cmp(s, best) > 0) best = s;
    const winners = scores.filter((s) => cmp(s, best) === 0).length;
    scores.forEach((s, i) => { if (cmp(s, best) === 0) share[i] += 1 / winners; });
}

/**
 * EXACT equity over every remaining board runout, for known hands.
 * @param {{rank:number,suit:number}[][]} hands
 * @param {{rank:number,suit:number}[]} board
 * @returns {{ method:'exact', trials:number, share:number[] }}
 */
export function exactEquity(hands, board) {
    const deck = remainingDeck([...hands.flat(), ...board]);
    const need = 5 - board.length;
    const share = new Array(hands.length).fill(0);
    let trials = 0;

    const run = (full) => {
        const scores = hands.map((h) => rankSeven([...h, ...full]));
        splitPot(scores, share);
        trials += 1;
    };

    const rec = (depth, start, chosen) => {
        if (depth === need) { run([...board, ...chosen]); return; }
        for (let i = start; i < deck.length; i += 1) {
            chosen.push(deck[i]);
            rec(depth + 1, i + 1, chosen);
            chosen.pop();
        }
    };
    rec(0, 0, []);

    return { method: 'exact', trials, share: share.map((s) => s / trials) };
}

/** mulberry32 — a different generator to the client's xorshift32, on purpose. */
function mulberry32(seed) {
    let a = seed >>> 0;
    return () => {
        a = (a + 0x6D2B79F5) >>> 0;
        let t = Math.imul(a ^ (a >>> 15), 1 | a);
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
}

/**
 * The MODEL the client shows when the canister has NOT revealed opponents: the
 * hero's equity against hands drawn uniformly at random.
 *
 * Sampled independently of the client -- different generator, different seed,
 * different trial count -- so agreement is statistical, and the caller compares
 * inside a stated tolerance rather than for equality.
 */
export function heroEquityVsRandom(hero, board, opponents, trials, seed = 0x5eed1) {
    const deck = remainingDeck([...hero, ...board]);
    const need = 5 - board.length;
    const draw = need + opponents * 2;
    if (draw > deck.length) return null;
    const rnd = mulberry32(seed);
    const pool = deck.slice();
    const share = new Array(opponents + 1).fill(0);

    for (let t = 0; t < trials; t += 1) {
        for (let i = 0; i < draw; i += 1) {
            const j = i + Math.floor(rnd() * (pool.length - i));
            const tmp = pool[i]; pool[i] = pool[j]; pool[j] = tmp;
        }
        const full = [...board, ...pool.slice(0, need)];
        const scores = [rankSeven([...hero, ...full])];
        for (let o = 0; o < opponents; o += 1) {
            scores.push(rankSeven([pool[need + o * 2], pool[need + o * 2 + 1], ...full]));
        }
        splitPot(scores, share);
    }
    return { method: 'monte-carlo', trials, equity: share[0] / trials };
}

/**
 * Trials for the oracle's Monte Carlo. Smaller than the client's 200,000 because
 * this lineage ranks 21 subsets per hand; the tolerance below accounts for it.
 */
export const ORACLE_TRIALS = 25_000;

/**
 * How far apart the two Monte Carlo runs may be, in percentage points.
 *
 * Five combined standard errors. SE of a proportion is at most 0.5/sqrt(n), so
 * the client at 200,000 contributes 0.112 points and the oracle at 25,000
 * contributes 0.316; combined sqrt(0.112^2 + 0.316^2) = 0.335, and 5 sigma is
 * 1.68. Wide enough that two correct implementations effectively never disagree,
 * and narrow enough that any real defect -- the wrong opponent count, the wrong
 * board, a broken evaluator -- misses by many points, not by one.
 */
export const MC_TOLERANCE_POINTS = 5 * Math.sqrt((0.5 / Math.sqrt(200_000)) ** 2
    + (0.5 / Math.sqrt(ORACLE_TRIALS)) ** 2) * 100;
