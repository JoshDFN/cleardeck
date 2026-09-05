/**
 * The two hand-record shapes on the wire, normalised into ONE model, plus the
 * facts the live table view adds and the per-seat card checks. Pure: nothing
 * here touches a canister, a store or the DOM. HandHistory.svelte loads and
 * renders; this file reasons.
 *
 * Moved out of HandHistory.svelte word for word (the shapes are documented in
 * $lib/hand-record.js and docs/DEFECTS.md H-32); the only additions are the
 * helpers at the foot that the list and the replayer both need.
 */

import { normalizeAction } from './hand-record.js';

/** Phases in which the current hand number has no record yet, and should not. */
export const SETTLED_PHASES = ['HandComplete', 'WaitingForPlayers'];

/** Candid `opt T` arrives as [] or [value]. */
export const opt = (v) => (Array.isArray(v) ? (v.length ? v[0] : null) : v ?? null);
export const text = (p) => (p?.toString ? p.toString() : p ?? null);
export const num = (v) => (typeof v === 'bigint' ? Number(v) : Number(v ?? 0));

const RANK_LETTER = {
  Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8',
  Nine: '9', Ten: 'T', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
};
const SUIT_LETTER = { Hearts: 'h', Diamonds: 'd', Clubs: 'c', Spades: 's' };

/** A Candid card as the two-character code shuffle-verify.js speaks. */
export function codeOf(card) {
  if (!card) return null;
  const rank = Object.keys(card.rank || {})[0];
  const suit = Object.keys(card.suit || {})[0];
  return rank && suit ? RANK_LETTER[rank] + SUIT_LETTER[suit] : null;
}

/** The table canister's own `HandHistory`. */
export function fromTableRecord(record, requestedNumber) {
  const proof = record.shuffle_proof || {};
  const revealed = opt(proof.revealed_seed);
  const showdown = (record.showdown_players || []).map((p) => ({
    seat: Number(p.seat),
    principal: text(p.principal),
    cards: opt(p.cards),
    rank: opt(p.hand_rank),
    won: num(p.amount_won),
  }));
  const winners = (record.winners || []).map((w) => ({
    seat: Number(w.seat),
    principal: text(w.principal),
    amount: num(w.amount),
    potType: null,
    rank: opt(w.hand_rank),
    cards: opt(w.cards),
  }));
  const actions = (record.actions || []).map(normalizeAction);
  // WHO PLAYED (FINDING 30): the table record's own two lists, when the
  // canister recorded them (`opt`; null is "not recorded", not "nobody").
  const dealtIn = (opt(record.dealt_in) || []).map((d) => ({ seat: Number(d.seat), principal: text(d.principal) }));
  const participants = (opt(record.participants) || []).map((p) => ({ seat: Number(p.seat), principal: text(p.principal) }));
  const players = seatPrincipals([...dealtIn, ...participants, ...showdown, ...winners]);
  const seats = new Set([
    ...players.map((p) => p.seat),
    ...actions.map((a) => a.seat),
    ...showdown.map((p) => p.seat),
    ...winners.map((w) => w.seat),
  ]);
  return {
    handNumber: Number(record.hand_number ?? requestedNumber),
    requestedNumber,
    timestamp: proof.timestamp ?? 0n,
    proof: { seedHash: proof.seed_hash || null, revealedSeed: revealed },
    community: record.community_cards || [],
    actions,
    showdown,
    winners,
    // Every (seat, principal) the record attributes to the hand, folds included.
    players,
    seats: [...seats].sort((a, b) => a - b),
    potTotal: winners.reduce((sum, w) => sum + w.amount, 0),
    // The TABLE record carries no blind level of its own. Saying so is the
    // difference between a level read off this hand and a level read off the
    // table as it is configured today.
    blinds: null,
    source: 'table canister',
    verification: null,
  };
}

/** The history canister's richer `HandHistoryRecord` (blinds and streets included). */
export function fromHistoryRecord(record) {
  const proof = record.shuffle_proof || {};
  const flop = opt(record.flop);
  const community = [
    ...(flop ? [flop[0], flop[1], flop[2]] : []),
    ...(opt(record.turn) ? [opt(record.turn)] : []),
    ...(opt(record.river) ? [opt(record.river)] : []),
  ];
  const players = (record.players || []).map((p) => ({
    seat: Number(p.seat),
    principal: text(p.principal),
    cards: opt(p.hole_cards),
    rank: opt(p.final_hand_rank),
    won: num(p.amount_won),
  }));
  const winners = (record.winners || []).map((w) => ({
    seat: Number(w.seat),
    principal: text(w.principal),
    amount: num(w.amount),
    potType: w.pot_type || null,
    rank: opt(w.hand_rank),
    cards: null,
  }));
  const actions = (record.actions || []).map(normalizeAction);
  return {
    handNumber: Number(record.hand_number),
    requestedNumber: Number(record.hand_id ?? record.hand_number),
    timestamp: record.timestamp ?? proof.timestamp ?? 0n,
    proof: { seedHash: proof.seed_hash || null, revealedSeed: proof.revealed_seed || null },
    community,
    actions,
    showdown: players.filter((p) => p.cards),
    winners,
    // Every player the archive lists, folded or not (hole_cards is null for a fold).
    players: seatPrincipals(players),
    seats: players.map((p) => p.seat).sort((a, b) => a - b),
    potTotal: num(record.total_pot),
    // This record DOES carry the level that was in force for this hand.
    blinds: {
      small: num(record.small_blind), big: num(record.big_blind), ante: num(record.ante),
    },
    source: 'history canister',
    verification: null,
  };
}

/**
 * The facts the live table view adds: the blind level, which seats the button
 * had on the blinds, the display names of the principals currently seated.
 * Everything here is about the table NOW; `blindsForHand` decides how far that
 * is evidence about the hand being replayed.
 */
export function factsFrom(view) {
  if (!view) return null;
  const cfg = view.config || {};
  const phase = view.phase && typeof view.phase === 'object' ? Object.keys(view.phase)[0] : null;
  return {
    config: {
      smallBlind: num(cfg.small_blind), bigBlind: num(cfg.big_blind), ante: num(cfg.ante),
    },
    blindSeats: {
      sb: Number(view.small_blind_seat), bb: Number(view.big_blind_seat),
      dealer: Number(view.dealer_seat),
    },
    liveHandNumber: num(view.hand_number),
    phase,
    settled: phase !== null && SETTLED_PHASES.includes(phase),
    // Seat -> {principal, name}. Only ever used when the principal in this map
    // is the same principal the hand record has for that seat.
    seated: (view.players || []).map((p) => opt(p)).filter(Boolean).map((p) => ({
      seat: Number(p.seat),
      principal: text(p.principal),
      name: opt(p.display_name),
    })),
  };
}

/**
 * Every card any player showed, checked against the locally derived deck.
 *
 * The hand log does not record which seats were dealt in, so this assumes the
 * seats it saw, in ascending order, are that list. If the assumption is wrong
 * the codes simply will not match and no tick is shown: a wrong guess can never
 * produce a false confirmation.
 */
export function seatCardChecks(report, hand) {
  if (!report?.layout || report.layout.players !== hand.seats.length) return [];
  return hand.showdown
    .filter((p) => p.cards)
    .map((p) => {
      const k = hand.seats.indexOf(p.seat);
      if (k < 0) return null;
      const [a, b] = report.layout.positions.holes[k];
      const derived = [report.deckCodes[a], report.deckCodes[b]];
      const dealt = [p.cards[0], p.cards[1]].map(codeOf);
      return {
        seat: p.seat,
        positions: [a, b],
        derived,
        match: derived[0] === dealt[0] && derived[1] === dealt[1],
      };
    })
    .filter(Boolean);
}

/** The principal the hand record itself attributes to a seat, if any. */
export function principalForSeat(hand, seat) {
  const hit = playersOf(hand).find((p) => p.seat === seat);
  return hit ? hit.principal : null;
}

/**
 * A name for a seat, used only when it can be justified: the live table's name
 * for that chair, and only while the principal sitting there now is the one the
 * record attributes to it.
 */
export function seatNameFor(hand, seat, tableFacts) {
  const recorded = principalForSeat(hand, seat);
  if (!recorded) return null;
  const live = (tableFacts?.seated || []).find((p) => p.seat === seat);
  if (!live || live.principal !== recorded) return null;
  return live.name || null;
}

/** One entry per (seat, principal), in first-seen order, nameless entries dropped. */
function seatPrincipals(list) {
  const seen = new Set();
  const out = [];
  for (const p of list) {
    if (!p || p.principal === null || p.principal === undefined) continue;
    const key = `${Number(p.seat)}:${p.principal}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push({ seat: Number(p.seat), principal: p.principal });
  }
  return out;
}

/**
 * Every (seat, principal) the record attributes to the hand: the showdown,
 * the winners, the dealt-in / participant lists, and the ACTIONS (the
 * archive's action records name who acted). A hand the hero folded before
 * showdown is in none of the first two, and read as "Not in" until the
 * other lists were consulted.
 */
export function playersOf(hand) {
  return seatPrincipals([
    ...(hand?.showdown || []),
    ...(hand?.winners || []),
    ...(hand?.players || []),
    ...(hand?.actions || []),
  ]);
}

/** Whether this principal was in the hand: showed down, won, was dealt in, or acted. */
export function participatedIn(hand, principal) {
  return !!principal && playersOf(hand).some((p) => p.principal === principal);
}

/** The seat the record attributes to a principal, or null. */
export function seatOfPrincipal(hand, principal) {
  if (!principal) return null;
  const hit = playersOf(hand).find((p) => p.principal === principal);
  return hit ? hit.seat : null;
}

/** A hand-rank variant as words ("Full House"). */
export function rankName(rank) {
  const value = Array.isArray(rank) ? rank[0] : rank;
  if (!value || typeof value !== 'object') return null;
  const key = Object.keys(value)[0];
  return key ? key.replace(/([A-Z])/g, ' $1').trim() : null;
}
