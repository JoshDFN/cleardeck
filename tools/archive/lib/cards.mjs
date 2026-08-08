// Cards, as this tool models them.
//
// Written from docs/SHUFFLE-SPEC.md section 2, NOT from src/poker_core/src/card.rs.
// The whole point of this tool is that a stranger can run it against data they
// fetched themselves and check ClearDeck's arithmetic without running ClearDeck's
// code, so nothing in tools/archive imports poker_core, the canister, or any
// generated declaration.
//
// A card is a two-character string, rank then suit: "Ah", "Td", "2c", "Ks".
// That is the whole model. Everything else here converts to or from it.

/** Suit letters in the deck's own order. Normative: SHUFFLE-SPEC section 2. */
export const SUITS = 'hdcs';

/** Rank letters, low to high. `rank_value = index + 2`. */
export const RANKS = '23456789TJQKA';

/** The Candid variant names the canister uses, in the order that fixes their ordinal. */
const CANDID_SUITS = ['Hearts', 'Diamonds', 'Clubs', 'Spades'];
const CANDID_RANKS = [
  'Two', 'Three', 'Four', 'Five', 'Six', 'Seven', 'Eight',
  'Nine', 'Ten', 'Jack', 'Queen', 'King', 'Ace',
];

const CANDID_SUIT_TO_LETTER = new Map(CANDID_SUITS.map((n, i) => [n, SUITS[i]]));
const CANDID_RANK_TO_LETTER = new Map(CANDID_RANKS.map((n, i) => [n, RANKS[i]]));

/**
 * The 52 cards before shuffling, index 0 first: `deck[13 * suit + rank - 2]`.
 * @returns {string[]} a fresh array; callers mutate their own copy, never this one
 */
export function buildDeck() {
  const deck = [];
  for (const suit of SUITS) for (const rank of RANKS) deck.push(rank + suit);
  return deck;
}

/** Absolute deck index of a card, the same number section 2 pins. */
export function cardIndex(card) {
  const r = RANKS.indexOf(card[0]);
  const s = SUITS.indexOf(card[1]);
  if (r < 0 || s < 0 || card.length !== 2) throw new Error(`not a card: ${JSON.stringify(card)}`);
  return 13 * s + r;
}

/** 2..14, aces high. */
export function rankValue(card) {
  const r = RANKS.indexOf(card[0]);
  if (r < 0) throw new Error(`not a card: ${JSON.stringify(card)}`);
  return r + 2;
}

/** 0..3, in SUITS order. */
export function suitOrdinal(card) {
  const s = SUITS.indexOf(card[1]);
  if (s < 0) throw new Error(`not a card: ${JSON.stringify(card)}`);
  return s;
}

/** True for a well-formed two-character card string. */
export function isCard(v) {
  return typeof v === 'string' && v.length === 2
    && RANKS.includes(v[0]) && SUITS.includes(v[1]);
}

/**
 * Converts one decoded Candid `Card` record into this tool's string form.
 *
 * The wire form is `{ rank: { Ace: null }, suit: { Hearts: null } }`. Anything
 * else is a decoding mistake and must be loud, because a silently wrong card is
 * the one failure this tool exists to make impossible.
 */
export function cardFromCandid(v) {
  if (!v || typeof v !== 'object') throw new Error(`not a Candid Card: ${JSON.stringify(v)}`);
  const rankName = Object.keys(v.rank ?? {})[0];
  const suitName = Object.keys(v.suit ?? {})[0];
  const r = CANDID_RANK_TO_LETTER.get(rankName);
  const s = CANDID_SUIT_TO_LETTER.get(suitName);
  if (!r) throw new Error(`unknown Candid Rank variant: ${JSON.stringify(rankName)}`);
  if (!s) throw new Error(`unknown Candid Suit variant: ${JSON.stringify(suitName)}`);
  return r + s;
}

/** The compact symbol encoding golden_vectors.txt uses: A-Z then a-z by deck index. */
const ALPHA = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
export const deckSymbols = (deck) => deck.map((c) => ALPHA[cardIndex(c)]).join('');

/** Pretty suit glyphs, for reading a reconstruction on a terminal. */
const GLYPH = { h: '♥', d: '♦', c: '♣', s: '♠' };
export const pretty = (card) => `${card[0]}${GLYPH[card[1]] ?? card[1]}`;
export const prettyList = (cards) => cards.map(pretty).join(' ');
