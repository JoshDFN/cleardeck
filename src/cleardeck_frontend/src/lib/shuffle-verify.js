/**
 * ClearDeck shuffle verifier — runs entirely in the player's browser.
 *
 * This is a port of `docs/SHUFFLE-SPEC.md` v1, kept deliberately independent of
 * everything else in this app:
 *
 *   - it imports NOTHING (no ClearDeck module, no agent, no canister actor);
 *   - it makes no network request of any kind;
 *   - the only primitive it borrows is the browser's own `crypto.subtle`
 *     SHA-256, which is the platform's implementation, not ours.
 *
 * That independence is the whole point. The table canister's `verify_shuffle`
 * query only recomputes a hash, and it is run by the exact party a player is
 * supposed to distrust. Everything in this file runs on the player's machine
 * and re-derives the CARDS, which is the claim that actually matters.
 *
 * The two reference implementations this was checked against live in
 * `src/poker_core/tests/verify/` (Node and Python, standard library only), and
 * both reproduce all 2,000 wasm32-generated golden vectors. So does this file:
 * see `$SCRATCH` harness notes in the wave report.
 *
 * WHAT VERIFYING PROVES, exactly:
 *   - the 52-card order was fixed by a seed whose SHA-256 was published before
 *     any card was dealt, and
 *   - the cards the player was shown are the cards that seed produces.
 * WHAT IT DOES NOT PROVE:
 *   - that the seed was unpredictable (that is the IC subnet's `raw_rand`, and
 *     it is trust, not arithmetic), and
 *   - anything at all about settlement, betting rules or the rest of the engine.
 */

// --- section 2 of the spec: deck construction -------------------------------

/** Suits in the normative pre-shuffle order. */
export const SUITS = ['Hearts', 'Diamonds', 'Clubs', 'Spades'];

/** Ranks in the normative pre-shuffle order (Candid variant names). */
export const RANKS = [
  'Two', 'Three', 'Four', 'Five', 'Six', 'Seven', 'Eight',
  'Nine', 'Ten', 'Jack', 'Queen', 'King', 'Ace',
];

const SUIT_LETTER = { Hearts: 'h', Diamonds: 'd', Clubs: 'c', Spades: 's' };
const RANK_LETTER = {
  Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8',
  Nine: '9', Ten: 'T', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
};

const TWO_POW_64 = 1n << 64n;
const DECK_SIZE = 52;

/**
 * The 52 cards before shuffling, index 0 first: `deck[13*suit + (rank - 2)]`.
 * Getting this order wrong changes every card, so it is pinned by its own test
 * on both the host and wasm32 (`create_deck_order_is_pinned`).
 * @returns {Array<{rank:string, suit:string, code:string}>}
 */
export function buildDeck() {
  const deck = [];
  for (const suit of SUITS) {
    for (const rank of RANKS) {
      deck.push({ rank, suit, code: RANK_LETTER[rank] + SUIT_LETTER[suit] });
    }
  }
  return deck;
}

/**
 * Two-character code ("Ah", "Td") for a card in either shape: this module's
 * `{rank:'Ace', suit:'Hearts'}` or the canister's Candid variant
 * `{rank:{Ace:null}, suit:{Hearts:null}}`.
 * @param {any} card
 * @returns {string|null}
 */
export function cardCode(card) {
  if (!card) return null;
  const rank = typeof card.rank === 'string' ? card.rank : firstKey(card.rank);
  const suit = typeof card.suit === 'string' ? card.suit : firstKey(card.suit);
  if (!rank || !suit || !RANK_LETTER[rank] || !SUIT_LETTER[suit]) return null;
  return RANK_LETTER[rank] + SUIT_LETTER[suit];
}

/** Renders one of this module's cards in the Candid shape `<Card>` expects. */
export function toCandidCard(card) {
  if (!card) return null;
  return { rank: { [card.rank]: null }, suit: { [card.suit]: null } };
}

function firstKey(variant) {
  if (!variant || typeof variant !== 'object') return null;
  const keys = Object.keys(variant);
  return keys.length ? keys[0] : null;
}

// --- hex and hashing --------------------------------------------------------

/** @param {string} hex @returns {Uint8Array} */
export function hexToBytes(hex) {
  const clean = String(hex ?? '').trim();
  if (!/^([0-9a-fA-F]{2})+$/.test(clean)) {
    throw new Error('seed must be a non-empty, even-length hex string');
  }
  return Uint8Array.from(clean.match(/../g).map((byte) => parseInt(byte, 16)));
}

/** @param {Uint8Array} bytes @returns {string} */
export function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

function subtle() {
  const api = globalThis.crypto?.subtle;
  if (!api) {
    throw new Error(
      'This browser did not expose crypto.subtle, so the shuffle cannot be ' +
      'verified locally. WebCrypto requires a secure context (https, or ' +
      'localhost during development).',
    );
  }
  return api;
}

/** SHA-256 of raw bytes, via the browser's own WebCrypto. */
export async function sha256Bytes(bytes) {
  const digest = await subtle().digest('SHA-256', bytes);
  return new Uint8Array(digest);
}

/** SHA-256 of raw bytes as lowercase hex. */
export async function sha256Hex(bytes) {
  return bytesToHex(await sha256Bytes(bytes));
}

// --- section 3 of the spec: the shuffle -------------------------------------

/**
 * `floor(2^64 / n) * n` — the largest multiple of `n` that is <= 2^64. Draws at
 * or above this are REJECTED rather than reduced, which is what makes the
 * modulo unbiased. See spec 3.3: this fires with probability ~2.2e-18, so a
 * player will never see it, but a verifier that omits it would one day disagree
 * with the table and be unable to tell that from cheating.
 * @param {bigint} n
 */
export function drawBound(n) {
  return (TWO_POW_64 / n) * n;
}

function littleEndianU64(bytes) {
  let value = 0n;
  for (let k = 7; k >= 0; k -= 1) value = (value << 8n) | BigInt(bytes[k]);
  return value;
}

/**
 * One Fisher-Yates pass from the top of the deck down, every swap index drawn
 * from a SHA-256 chain rooted at the seed (spec 3.5).
 *
 * @param {Uint8Array} seedBytes raw seed bytes (NOT the hex text)
 * @param {{traceSteps?:number}} [opts] how many opening steps to record for display
 * @returns {Promise<{deck:Array, rejections:number, trace:Array}>}
 */
export async function shuffleFromSeed(seedBytes, { traceSteps = 3 } = {}) {
  const deck = buildDeck();
  let chain = seedBytes;
  let rejections = 0;
  const trace = [];

  for (let i = DECK_SIZE - 1; i >= 1; i -= 1) {
    const n = BigInt(i + 1);
    const limit = drawBound(n);
    let draw;
    let redraws = 0;
    for (;;) {
      const input = new Uint8Array(chain.length + 1);
      input.set(chain, 0);
      input[chain.length] = i % 256;
      // eslint-disable-next-line no-await-in-loop -- the chain is inherently serial
      chain = await sha256Bytes(input);
      draw = littleEndianU64(chain.subarray(0, 8));
      if (draw < limit) break;
      rejections += 1;
      redraws += 1;
    }
    const j = Number(draw % n);

    if (trace.length < traceSteps) {
      trace.push({
        step: DECK_SIZE - i,
        i,
        n: i + 1,
        counterByte: i % 256,
        chainHex: bytesToHex(chain),
        drawBytesHex: bytesToHex(chain.subarray(0, 8)),
        draw: draw.toString(),
        j,
        redraws,
        movedToTop: deck[j].code,
        movedFromTop: deck[i].code,
      });
    }

    const swap = deck[i];
    deck[i] = deck[j];
    deck[j] = swap;
  }

  return { deck, rejections, trace };
}

// --- section 4 of the spec: dealing order -----------------------------------

/**
 * Where every visible card of a hand sits in the shuffled deck, for `players`
 * seats dealt in. Hole cards go round in ASCENDING SEAT INDEX (not left of the
 * button, which is the usual casino convention but is not what this engine
 * does), then burn / flop / burn / turn / burn / river.
 *
 * @param {number} players number of seats DEALT IN
 */
export function dealPositions(players) {
  if (!Number.isInteger(players) || players < 2) {
    throw new Error('a hand needs at least 2 players dealt in');
  }
  if (2 * players + 8 > DECK_SIZE) {
    throw new Error('not enough cards for that many players');
  }
  const base = 2 * players;
  const holes = [];
  for (let k = 0; k < players; k += 1) holes.push([2 * k, 2 * k + 1]);
  return {
    players,
    holes,
    burnBeforeFlop: base,
    flop: [base + 1, base + 2, base + 3],
    burnBeforeTurn: base + 4,
    turn: base + 5,
    burnBeforeRiver: base + 6,
    river: base + 7,
    /** Board positions in dealt order, for a board of `n` cards. */
    boardFor(n) {
      return [base + 1, base + 2, base + 3, base + 5, base + 7].slice(0, n);
    },
  };
}

// --- putting it together ----------------------------------------------------

/**
 * Finds every (table size, seat order) pair under which EVERY card the player
 * actually saw lands exactly where the dealing rule puts it.
 *
 * The number of seats dealt in is not recorded in the hand log (a seat that was
 * sitting out or busted took no cards, and blinds are not action records), so
 * rather than guess it, the verifier tries each possible table size and reports
 * which ones the deal itself is consistent with. The deck is already pinned by
 * the commitment at this point, so this is a check, not a fit: for a solution to
 * exist, seven independent cards have to land on seven specific indices.
 *
 * @param {string[]} deckCodes 52 two-character card codes, in shuffled order
 * @param {{myCards?:string[], board?:string[], maxPlayers?:number}} observed
 * @returns {Array<{players:number, dealIndex:number, positions:object}>}
 */
export function solveLayout(deckCodes, { myCards = null, board = null, maxPlayers = 9 } = {}) {
  const solutions = [];
  const cap = Math.max(2, Math.min(Number(maxPlayers) || 9, 22));

  for (let players = 2; players <= cap; players += 1) {
    const pos = dealPositions(players);
    const boardPositions = pos.boardFor(board ? board.length : 0);
    const boardOk = !board || board.length === 0
      || boardPositions.every((p, at) => deckCodes[p] === board[at]);
    if (!boardOk) continue;

    if (!myCards || myCards.length !== 2) {
      solutions.push({ players, dealIndex: null, positions: pos, boardPositions });
      continue;
    }
    for (let k = 0; k < players; k += 1) {
      const [a, b] = pos.holes[k];
      if (deckCodes[a] === myCards[0] && deckCodes[b] === myCards[1]) {
        solutions.push({ players, dealIndex: k, positions: pos, boardPositions });
      }
    }
  }
  return solutions;
}

/**
 * The whole client-side verification, start to finish.
 *
 * Step 0 is the commitment: SHA-256 of the revealed seed, computed here, must
 * equal the hash the table published BEFORE the deal. Then the deck is
 * re-derived from that seed and the player's own cards and the board are looked
 * up at the positions the dealing rule predicts.
 *
 * Nothing in this function talks to a canister. If it returns `cardsMatched > 0`
 * then those cards were genuinely re-derived on this machine.
 *
 * @param {object} args
 * @param {string} args.seedHashHex the commitment published before the deal
 * @param {string} args.revealedSeedHex the seed revealed after the hand
 * @param {string[]|null} [args.myCards] the player's two dealt cards, as codes
 * @param {string[]} [args.board] the community cards actually seen, as codes
 * @param {number} [args.maxPlayers] table capacity, bounds the layout search
 * @param {number|null} [args.expectedPlayers] a table size read from the hand
 *        record, used only to corroborate the solved one
 */
export async function verifyHandLocally({
  seedHashHex,
  revealedSeedHex,
  myCards = null,
  board = [],
  maxPlayers = 9,
  expectedPlayers = null,
}) {
  const result = {
    ok: false,
    error: null,
    commitment: { computed: null, committed: (seedHashHex || '').toLowerCase(), match: false },
    deck: [],
    deckCodes: [],
    rejections: 0,
    trace: [],
    layout: null,
    ambiguous: false,
    checks: [],
    cardsChecked: 0,
    cardsMatched: 0,
    expectedPlayers,
    playersAgree: null,
  };

  let seedBytes;
  try {
    seedBytes = hexToBytes(revealedSeedHex);
  } catch (e) {
    result.error = e.message;
    return result;
  }

  try {
    result.commitment.computed = await sha256Hex(seedBytes);
  } catch (e) {
    result.error = e.message;
    return result;
  }
  result.commitment.match =
    !!result.commitment.committed &&
    result.commitment.computed === result.commitment.committed;

  // The deck is re-derived even when the commitment fails, so a player can see
  // exactly WHAT the revealed seed produces and compare it with what they got.
  const { deck, rejections, trace } = await shuffleFromSeed(seedBytes);
  result.deck = deck;
  result.deckCodes = deck.map((c) => c.code);
  result.rejections = rejections;
  result.trace = trace;

  const boardCodes = (board || []).map(cardCode).filter(Boolean);
  const mine = myCards && myCards.length === 2 ? myCards.map(cardCode) : null;
  const mineUsable = mine && mine.every(Boolean) ? mine : null;

  const solutions = solveLayout(result.deckCodes, {
    myCards: mineUsable,
    board: boardCodes,
    maxPlayers,
  });

  if (solutions.length === 0) {
    result.error = result.commitment.match
      ? 'The seed matches the commitment, but the cards this browser derived from ' +
        'it are NOT the cards that were dealt. Do not trust this table.'
      : 'The revealed seed does not hash to the published commitment, and the ' +
        'cards it produces are not the cards that were dealt.';
    return result;
  }

  // Prefer a solution that agrees with the table size the hand record implies.
  const preferred =
    solutions.find((s) => expectedPlayers != null && s.players === expectedPlayers) ||
    solutions[0];
  result.layout = preferred;
  result.allSolutions = solutions.map((s) => ({ players: s.players, dealIndex: s.dealIndex }));
  result.playersAgree = expectedPlayers == null ? null : preferred.players === expectedPlayers;

  // Two different questions, and conflating them would overstate the result.
  //
  //  - "how many seats were dealt in" is only pinned once there is a board:
  //    with no community cards, every table size larger than the player's own
  //    position fits equally well, because hole positions do not depend on it.
  //  - "where are MY two cards" is pinned by the deal index alone, so it can be
  //    certain even when the table size is not.
  result.playersDetermined = new Set(solutions.map((s) => s.players)).size === 1;
  result.holePositionsCertain =
    mineUsable != null && new Set(solutions.map((s) => s.dealIndex)).size === 1;
  result.ambiguous = !result.playersDetermined;

  if (mineUsable) {
    const [a, b] = preferred.positions.holes[preferred.dealIndex];
    result.checks.push({
      kind: 'hole',
      label: 'Your two cards',
      positions: [a, b],
      dealt: mineUsable,
      derived: [result.deckCodes[a], result.deckCodes[b]],
      derivedCards: [deck[a], deck[b]],
      match: result.deckCodes[a] === mineUsable[0] && result.deckCodes[b] === mineUsable[1],
    });
  }

  const boardLabels = ['Flop', 'Flop', 'Flop', 'Turn', 'River'];
  boardCodes.forEach((code, at) => {
    const p = preferred.boardPositions[at];
    result.checks.push({
      kind: 'board',
      label: boardLabels[at] || 'Board',
      positions: [p],
      dealt: [code],
      derived: [result.deckCodes[p]],
      derivedCards: [deck[p]],
      match: result.deckCodes[p] === code,
    });
  });

  result.cardsChecked = result.checks.reduce((n, c) => n + c.dealt.length, 0);
  result.cardsMatched = result.checks.reduce(
    (n, c) => n + c.dealt.filter((code, at) => code === c.derived[at]).length,
    0,
  );
  result.ok =
    result.commitment.match &&
    result.cardsChecked > 0 &&
    result.cardsMatched === result.cardsChecked;

  return result;
}
