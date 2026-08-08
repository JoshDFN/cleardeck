// The archive bundle: the ONLY thing the offline half of this tool reads.
//
// `fetch/fetch-archive.mjs` writes one of these from a canister. Everything
// after that -- reconstruction, statistics, EV, collusion -- reads a plain JSON
// file and never opens a socket. That separation is the point: the analysis does
// not require trusting ClearDeck's server, or ClearDeck's client library, or
// this tool's own fetcher. Anyone can produce a bundle from any source and the
// answers must come out the same.
//
// This module validates one at the boundary, hard, before a single number is
// computed from it. External data is not trusted, including data this tool's own
// fetcher wrote a second ago.

import { isCard } from './cards.mjs';

export const BUNDLE_VERSION = 1;

/**
 * Chip amounts arrive as decimal strings because they are `nat64` on the wire.
 * They become JS numbers here, and anything that cannot be a number EXACTLY is
 * refused rather than rounded. e8s amounts at any plausible table are far below
 * 2^53; a value above it means either a hostile bundle or a defect, and either
 * way this tool must not print a number about it.
 */
export function chips(value, where) {
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new BundleError(`${where}: chip amount ${value} is not a safe non-negative integer`);
    }
    return value;
  }
  if (typeof value !== 'string' || !/^\d+$/.test(value)) {
    throw new BundleError(`${where}: chip amount must be a decimal string, got ${JSON.stringify(value)}`);
  }
  const n = Number(value);
  if (!Number.isSafeInteger(n)) {
    throw new BundleError(
      `${where}: chip amount ${value} exceeds 2^53 and cannot be represented exactly. ` +
      'Refusing to compute anything from it.',
    );
  }
  return n;
}

export class BundleError extends Error {}

const isPlainObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

function need(obj, key, where) {
  if (!isPlainObject(obj) || !(key in obj)) throw new BundleError(`${where}: missing field "${key}"`);
  return obj[key];
}

function optCard(v, where) {
  if (v === null || v === undefined) return null;
  if (!isCard(v)) throw new BundleError(`${where}: not a card: ${JSON.stringify(v)}`);
  return v;
}

function u64(v, where) {
  if (typeof v === 'number' && Number.isSafeInteger(v) && v >= 0) return v;
  if (typeof v === 'string' && /^\d+$/.test(v)) return Number(v);
  throw new BundleError(`${where}: expected a non-negative integer, got ${JSON.stringify(v)}`);
}

const ACTION_KINDS = new Set(['Fold', 'Check', 'Call', 'Bet', 'Raise', 'AllIn', 'PostBlind']);
const AMOUNT_KINDS = new Set(['Call', 'Bet', 'Raise', 'AllIn', 'PostBlind']);
const PHASES = new Set(['preflop', 'flop', 'turn', 'river']);

function validateAction(a, where) {
  const kind = need(a, 'kind', where);
  if (!ACTION_KINDS.has(kind)) throw new BundleError(`${where}: unknown action kind ${JSON.stringify(kind)}`);
  const amount = AMOUNT_KINDS.has(kind) ? chips(need(a, 'amount', where), `${where}.amount`) : 0;
  const phase = need(a, 'phase', `${where}`);
  if (typeof phase !== 'string') throw new BundleError(`${where}: phase must be a string`);
  return {
    seat: u64(need(a, 'seat', where), `${where}.seat`),
    principal: String(need(a, 'principal', where)),
    kind,
    amount,
    phase,
    // A phase string the canister never writes is not fatal to reading the record,
    // but every consumer must know it cannot place the action on a street.
    phaseKnown: PHASES.has(phase),
    timestamp: String(need(a, 'timestamp', where)),
  };
}

function validatePlayer(p, where) {
  const hole = need(p, 'hole_cards', where);
  let holeCards = null;
  if (hole !== null && hole !== undefined) {
    if (!Array.isArray(hole) || hole.length !== 2) {
      throw new BundleError(`${where}.hole_cards: expected null or two cards`);
    }
    holeCards = [optCard(hole[0], `${where}.hole_cards[0]`), optCard(hole[1], `${where}.hole_cards[1]`)];
  }
  const dealtIn = p.dealt_in;
  if (dealtIn !== null && dealtIn !== undefined && typeof dealtIn !== 'boolean') {
    throw new BundleError(`${where}.dealt_in: expected boolean or null`);
  }
  const contributed = p.contributed === null || p.contributed === undefined
    ? null
    : chips(p.contributed, `${where}.contributed`);
  const leftMidHand = p.left_mid_hand;
  if (leftMidHand !== null && leftMidHand !== undefined && typeof leftMidHand !== 'boolean') {
    throw new BundleError(`${where}.left_mid_hand: expected boolean or null`);
  }
  return {
    seat: u64(need(p, 'seat', where), `${where}.seat`),
    principal: String(need(p, 'principal', where)),
    startingChips: chips(need(p, 'starting_chips', where), `${where}.starting_chips`),
    endingChips: chips(need(p, 'ending_chips', where), `${where}.ending_chips`),
    holeCards,
    finalHandRank: p.final_hand_rank ?? null,
    amountWon: chips(need(p, 'amount_won', where), `${where}.amount_won`),
    position: String(need(p, 'position', where)),
    dealtIn: dealtIn ?? null,
    contributed,
    leftMidHand: leftMidHand ?? null,
  };
}

/** Validates one normalized hand record. Throws BundleError with the field name. */
export function validateHand(h, index) {
  const where = `hands[${index}]`;
  const proof = need(h, 'shuffle_proof', where);
  const dealtInRaw = need(h, 'dealt_in', where);
  let dealtIn = null;
  if (dealtInRaw !== null && dealtInRaw !== undefined) {
    if (!Array.isArray(dealtInRaw)) throw new BundleError(`${where}.dealt_in: expected a list or null`);
    dealtIn = dealtInRaw.map((d, k) => ({
      seat: u64(need(d, 'seat', `${where}.dealt_in[${k}]`), `${where}.dealt_in[${k}].seat`),
      principal: String(need(d, 'principal', `${where}.dealt_in[${k}]`)),
    }));
  }

  const flopRaw = need(h, 'flop', where);
  let flop = null;
  if (flopRaw !== null && flopRaw !== undefined) {
    if (!Array.isArray(flopRaw) || flopRaw.length !== 3) {
      throw new BundleError(`${where}.flop: expected null or three cards`);
    }
    flop = flopRaw.map((c, k) => optCard(c, `${where}.flop[${k}]`));
  }

  return {
    handId: u64(need(h, 'hand_id', where), `${where}.hand_id`),
    tableId: String(need(h, 'table_id', where)),
    handNumber: u64(need(h, 'hand_number', where), `${where}.hand_number`),
    timestamp: String(need(h, 'timestamp', where)),
    smallBlind: chips(need(h, 'small_blind', where), `${where}.small_blind`),
    bigBlind: chips(need(h, 'big_blind', where), `${where}.big_blind`),
    ante: chips(need(h, 'ante', where), `${where}.ante`),
    shuffleProof: {
      seedHash: String(need(proof, 'seed_hash', `${where}.shuffle_proof`)),
      revealedSeed: String(need(proof, 'revealed_seed', `${where}.shuffle_proof`)),
      timestamp: String(need(proof, 'timestamp', `${where}.shuffle_proof`)),
    },
    dealtIn,
    dealerSeat: u64(need(h, 'dealer_seat', where), `${where}.dealer_seat`),
    players: need(h, 'players', where).map((p, k) => validatePlayer(p, `${where}.players[${k}]`)),
    flop,
    turn: optCard(need(h, 'turn', where), `${where}.turn`),
    river: optCard(need(h, 'river', where), `${where}.river`),
    actions: need(h, 'actions', where).map((a, k) => validateAction(a, `${where}.actions[${k}]`)),
    totalPot: chips(need(h, 'total_pot', where), `${where}.total_pot`),
    rake: chips(need(h, 'rake', where), `${where}.rake`),
    winners: need(h, 'winners', where).map((w, k) => ({
      seat: u64(need(w, 'seat', `${where}.winners[${k}]`), `${where}.winners[${k}].seat`),
      principal: String(need(w, 'principal', `${where}.winners[${k}]`)),
      amount: chips(need(w, 'amount', `${where}.winners[${k}]`), `${where}.winners[${k}].amount`),
      handRank: w.hand_rank ?? null,
      potType: String(need(w, 'pot_type', `${where}.winners[${k}]`)),
    })),
    wentToShowdown: Boolean(need(h, 'went_to_showdown', where)),
  };
}

/** Validates a whole bundle. Returns the parsed form; throws BundleError otherwise. */
export function parseBundle(raw) {
  if (!isPlainObject(raw)) throw new BundleError('bundle: not a JSON object');
  const version = raw.cleardeck_archive_bundle;
  if (version !== BUNDLE_VERSION) {
    throw new BundleError(
      `bundle: expected cleardeck_archive_bundle=${BUNDLE_VERSION}, got ${JSON.stringify(version)}`,
    );
  }
  if (!Array.isArray(raw.hands)) throw new BundleError('bundle: "hands" must be a list');
  return {
    version,
    fetchedAt: String(raw.fetched_at ?? ''),
    source: raw.source ?? null,
    totalHandsReported: raw.total_hands_reported ?? null,
    hands: raw.hands.map((h, i) => validateHand(h, i)),
  };
}
