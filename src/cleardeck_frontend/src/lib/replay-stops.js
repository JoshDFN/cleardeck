/**
 * The replayer's stops: one per ACTION, not one per street.
 *
 * A street scrubber with five stops cannot show a hand: the board arrives, but
 * nobody bets, nobody folds and the pot never moves. PokerStars, PokerNow and
 * every broadcast replay step one action at a time, with a longer beat when a
 * street is dealt. This module turns a hand record (already normalised by
 * hand-record.js) into that sequence, and folds the record's amounts into the
 * state of the table at each stop: whose turn it was, what each seat has in
 * front of it on the current street, what the pot has collected, who has
 * folded, who is all in.
 *
 * WHAT IS EXACT AND WHAT IS NOT. The record carries every action's amount, and
 * hand-record.js says what each amount MEANS: a Call or a Bet is the increment
 * that action added; a Raise or an AllIn is the total the seat has in for the
 * street. Folding per seat and per street handles both exactly, with one gap:
 * a pre-flop Raise or AllIn by a seat that posted a blind is a street total
 * that INCLUDES the blind, so the pot after it is only exact when the record
 * says which seats posted (`blinds.seats`, hand-record.js `blindsForHand`).
 * When it does not, `exact` turns false from that stop on and the pot figure
 * is withheld rather than guessed. The FINAL stop always shows the pot the
 * canister paid, which is the record's own figure.
 *
 * Nothing here is a claim about the chain: it is the record, replayed.
 */

import { BOARD_AFTER, STREET_ORDER, STREET_UNKNOWN } from './hand-record.js';

/** Bar 11 of docs/DESIGN-BAR.md: 500 ms is the house tempo for a moving object. */
export const ACTION_BEAT_MS = 500;

/** A street reveal gets two beats, so the board can be read before play resumes. */
export const REVEAL_BEAT_MS = 1000;

export const SHOWDOWN_LABEL = 'Showdown';
export const NO_SHOWDOWN_LABEL = 'Won without showdown';

/**
 * @typedef {object} Stop
 * @property {number} index        position in the sequence
 * @property {string} street       the scrubber label this stop belongs to
 * @property {'deal'|'blind'|'action'|'reveal'|'showdown'|'end'} kind
 * @property {number} board        community cards on the felt at this stop
 * @property {number|null} line    the 1-based log line this stop plays, if any
 * @property {number|null} seat    the seat that acted, if any
 * @property {string|null} verb    the log line's verb, if any
 * @property {number|null} pot     chips the pot has collected, or null when not exact
 * @property {Record<string, number>} bets  each seat's contribution on the current street
 * @property {number[]} folded
 * @property {number[]} allIn
 * @property {boolean} reveal      every showdown hand face up
 * @property {boolean} exact       pot and bets are exact at this stop
 * @property {boolean} paid        `pot` is the pot the canister paid, not a fold
 */

const sum = (bets) => Object.values(bets).reduce((n, v) => n + v, 0);

/** The log's lines in play order, flattened out of the street groups. */
export function flattenLog(logGroups) {
  return (logGroups || []).flatMap((g) => g.lines.map((l) => ({ ...l, street: g.street })));
}

/**
 * @param {object} args
 * @param {{community:any[], showdown:any[], potTotal:number}} args.hand
 * @param {{level:{small:number,big:number,ante:number}|null, seats:{sb:number,bb:number}|null}|null} args.blinds
 * @param {Array<{street:string, lines:object[]}>} args.logGroups
 * @returns {Stop[]}
 */
export function buildStops({ hand, blinds, logGroups }) {
  const lines = flattenLog(logGroups);
  const byStreet = (street) => lines.filter((l) => l.street === street);
  const boardCount = (hand?.community || []).length;
  const level = blinds?.level || null;
  const seatsKnown = !!(level && blinds?.seats);

  let exact = !!level && !(level.ante > 0);
  let collected = 0;
  let bets = {};
  let folded = [];
  let allIn = [];
  let board = 0;

  if (level && seatsKnown) {
    bets = { [blinds.seats.sb]: level.small, [blinds.seats.bb]: level.big };
  } else if (level) {
    // The level is known but not who posted: the blinds are in the pot, unattributed.
    collected = level.small + level.big;
  }

  const stops = [];
  const push = (fields) => {
    stops.push({
      index: stops.length,
      line: null,
      seat: null,
      verb: null,
      reveal: false,
      paid: false,
      ...fields,
      board,
      pot: exact ? collected : null,
      bets: { ...bets },
      folded: [...folded],
      allIn: [...allIn],
      exact,
    });
  };

  const play = (line) => {
    const seat = line.seat ?? null;
    switch (line.kind) {
      case 'PostBlind':
      case 'PostAnte':
        break; // already in the opening state
      case 'Fold':
        if (seat !== null) folded = [...folded, seat];
        break;
      case 'Check':
        break;
      case 'Call':
      case 'Bet':
        if (seat !== null && line.amount) bets = { ...bets, [seat]: (bets[seat] || 0) + line.amount };
        break;
      case 'Raise':
      case 'AllIn':
        if (line.street === 'Pre-flop' && level && !seatsKnown) exact = false;
        if (seat !== null && line.amount) bets = { ...bets, [seat]: line.amount };
        if (line.kind === 'AllIn' && seat !== null) allIn = [...allIn, seat];
        break;
      default:
        exact = false;
    }
    push({ street: line.street, kind: line.kind === 'PostBlind' || line.kind === 'PostAnte' ? 'blind' : 'action', line: line.index, seat, verb: line.word ?? null });
  };

  const sweep = () => {
    collected += sum(bets);
    bets = {};
  };

  push({ street: 'Pre-flop', kind: 'deal' });
  byStreet('Blinds').forEach(play);
  byStreet('Pre-flop').forEach(play);

  for (const street of STREET_ORDER.slice(1)) {
    if (BOARD_AFTER[street] > boardCount) break;
    sweep();
    board = BOARD_AFTER[street];
    push({ street, kind: 'reveal' });
    byStreet(street).forEach(play);
  }

  const unknown = byStreet(STREET_UNKNOWN);
  if (unknown.length) exact = false;
  unknown.forEach(play);

  sweep();
  const paidPot = Number(hand?.potTotal || 0);
  const showdown = (hand?.showdown || []).length > 0;
  stops.push({
    index: stops.length,
    street: showdown ? SHOWDOWN_LABEL : NO_SHOWDOWN_LABEL,
    kind: showdown ? 'showdown' : 'end',
    board,
    line: null,
    seat: null,
    verb: null,
    pot: paidPot,
    bets: {},
    folded: [...folded],
    allIn: [...allIn],
    reveal: showdown,
    exact,
    paid: true,
  });
  return stops;
}

/**
 * The scrubber's entries: the first stop of every street, plus the final one.
 * @param {Stop[]} stops
 * @returns {Array<{label:string, index:number}>}
 */
export function streetStops(stops) {
  const out = [];
  for (const stop of stops) {
    const known = stop.kind === 'deal' || stop.kind === 'reveal' || stop.kind === 'showdown' || stop.kind === 'end';
    if (!known) continue;
    if (out.some((s) => s.label === stop.street)) continue;
    out.push({ label: stop.street, index: stop.index });
  }
  return out;
}

/** The scrubber entry a stop belongs to: the last street entry at or before it. */
export function streetOfStop(stops, index) {
  const entries = streetStops(stops);
  let current = entries[0] || null;
  for (const e of entries) if (e.index <= index) current = e;
  return current;
}

/** How long autoplay rests on a stop. */
export function stopBeatMs(stop) {
  if (!stop) return ACTION_BEAT_MS;
  return stop.kind === 'action' || stop.kind === 'blind' ? ACTION_BEAT_MS : REVEAL_BEAT_MS;
}

/** The seats still in the hand at a stop. */
export function liveSeatsAt(stop, seats) {
  return (seats || []).filter((s) => !(stop?.folded || []).includes(s));
}

/**
 * Equity may be shown at a stop only when EVERY seat still live there has cards
 * the record reveals: the same rule the live table follows (equity.js), so the
 * replay never models a hand it was not shown.
 */
export function equityAllowedAt(stop, seats, showdownSeats) {
  const live = liveSeatsAt(stop, seats);
  return live.length >= 2 && live.every((s) => (showdownSeats || []).includes(s));
}

/**
 * Where each seat's pod sits on the mini table, as fractions of the scene box.
 *
 * The anchor seat (the hero, or the lowest seat when the viewer was not in the
 * hand) takes the bottom of the ring; the others take the slots their seat
 * numbers give them around a `maxPlayers` ring, clockwise, so two seats are
 * bottom and top, six are the familiar 6-max ring.
 *
 * @param {{seats:number[], heroSeat:number|null, maxPlayers:number, rx?:number, ry?:number}} args
 * @returns {Array<{seat:number, x:number, y:number, nx:number, ny:number, side:'top'|'bottom'|'left'|'right'}>}
 */
export function seatSpots({ seats, heroSeat = null, maxPlayers = 9, rx = 0.41, ry = 0.44 }) {
  const ring = Math.max(2, Number(maxPlayers) || 2, ...(seats || []).map((s) => s + 1));
  const anchor = heroSeat !== null && (seats || []).includes(heroSeat)
    ? heroSeat
    : Math.min(...(seats || [0]));
  return (seats || []).map((seat) => {
    const slot = ((seat - anchor) % ring + ring) % ring;
    const theta = Math.PI / 2 + (slot / ring) * Math.PI * 2;
    const cx = Math.cos(theta);
    const cy = Math.sin(theta);
    const x = 0.5 + rx * cx;
    const y = 0.5 + ry * cy;
    let side = 'bottom';
    if (Math.abs(cx) > 0.6) side = cx < 0 ? 'left' : 'right';
    else if (cy < 0) side = 'top';
    return { seat, x, y, nx: -cx, ny: -cy, side };
  });
}
