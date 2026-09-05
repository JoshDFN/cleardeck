// The lobby's rows, as data: one plain object per table, built from the lobby
// canister's record and the table canister's own live view.
//
// Two sources, and the rule about which one wins:
//   1. the lobby canister's TableInfo: name, stakes, buy-in range, seat count,
//      currency, per-table canister id, the action clock;
//   2. the table canister's `get_table_view()` (an anonymous query): pot, hand
//      number, phase, the board, the seat map with real stacks and per-seat
//      status, the last hand's payout, the deck commitment.
// The money a player posts is set by the TABLE canister, so its config is
// what a row renders; the lobby record is the fallback for a table not
// reached yet, and a disagreement between the two is named on the row.
//
// Pure functions of Candid values. Nothing here fetches or touches the DOM.

import { formatAmount, formatBlinds, stakeTier, tableFormat, unitOf } from './lobby-format.js';

/** `opt T` decodes to [] or [value]; everything here tolerates both. */
export const opt = (v) => (Array.isArray(v) ? (v.length ? v[0] : null) : (v ?? null));

/** The variant tag of a Candid variant, or null. */
export function tagOf(variant) {
  const v = opt(variant);
  if (!v || typeof v !== 'object') return typeof v === 'string' ? v : null;
  const keys = Object.keys(v);
  return keys.length ? keys[0] : null;
}

export function canisterIdOf(table) {
  const p = opt(table?.canister_id);
  if (!p) return null;
  return typeof p === 'string' ? p : (p.toText ? p.toText() : String(p));
}

export function currencyOf(table) {
  const direct = tagOf(table?.currency);
  if (direct) return direct.toUpperCase();
  const fromConfig = tagOf(table?.config?.currency);
  return fromConfig ? fromConfig.toUpperCase() : 'ICP';
}

/** The config a player will actually be charged by. */
export function effectiveConfig(table, view) {
  return view?.config ?? table.config;
}

const DRIFT_KEYS = [
  ['small_blind', 'small blind'],
  ['big_blind', 'big blind'],
  ['min_buy_in', 'minimum buy-in'],
  ['max_buy_in', 'maximum buy-in'],
  ['max_players', 'seat count'],
  ['ante', 'ante'],
];

/**
 * Keys where the lobby canister's registration disagrees with the table
 * canister's own config. Empty means the two on-chain sources agree.
 */
export function configDrift(table, view) {
  const live = view?.config;
  if (!live) return [];
  return DRIFT_KEYS
    .filter(([key]) => Number(live[key]) !== Number(table.config[key]))
    .map(([, label]) => label);
}

/**
 * Seat map: one entry per seat, null when empty. `status` is the seat's own
 * PlayerStatus off the table canister, so a player sitting out is a taken
 * seat that cannot be dealt in.
 */
export function seatsOf(table, view) {
  const max = Number(effectiveConfig(table, view).max_players);
  if (view?.players?.length) {
    return Array.from({ length: max }, (_, i) => {
      const p = opt(view.players[i]);
      if (!p) return null;
      const status = tagOf(p.status) ?? 'Active';
      return {
        seat: i,
        name: opt(p.display_name) || null,
        chips: Number(p.chips),
        status,
        active: status === 'Active',
      };
    });
  }
  // No live view yet: the count the lobby resolved, never overstated.
  const filled = Number(table.player_count);
  return Array.from({ length: max }, (_, i) => (
    i < filled ? { seat: i, name: null, chips: null, status: null, active: true } : null
  ));
}

export const PHASE_LABEL = Object.freeze({
  PreFlop: 'Pre-flop',
  Flop: 'Flop',
  Turn: 'Turn',
  River: 'River',
  Showdown: 'Showdown',
  HandComplete: 'Hand complete',
  WaitingForPlayers: 'Waiting',
});

export const isDealing = (phase) =>
  Boolean(phase) && phase !== 'WaitingForPlayers' && phase !== 'HandComplete';

/**
 * The "Now" cell. A hand in progress is stated with its street and pot; with
 * no hand running, how close the table is to one; with nobody seated, an
 * invitation rather than a zero.
 */
export function nowOf(table, view) {
  const currency = currencyOf(table);
  const seats = seatsOf(table, view);
  const ready = seats.filter((s) => s && s.active).length;
  const seated = seats.filter(Boolean).length;
  const phase = view ? tagOf(view.phase) : null;

  if (isDealing(phase)) {
    const pot = Number(view.pot);
    return {
      kind: 'live',
      label: PHASE_LABEL[phase] ?? phase,
      detail: pot > 0 ? `${formatAmount(pot, currency)} ${unitOf(currency)}` : null,
    };
  }
  if (ready >= 2) return { kind: 'ready', label: 'Ready to deal', detail: null };
  if (ready === 1) return { kind: 'waiting', label: 'Waiting for one more', detail: null };
  if (seated > 0) return { kind: 'waiting', label: 'All sitting out', detail: null };
  return { kind: 'open', label: 'No one seated yet', detail: null };
}

/** Hands dealt at this table, straight off the table canister. */
export const handsOf = (view) => (view ? Number(view.hand_number) : null);

/** What the last completed hand paid out, summed across its winners. */
export function lastPotOf(view) {
  const winners = view?.last_hand_winners;
  if (!winners?.length) return null;
  return winners.reduce((n, w) => n + Number(w.amount), 0);
}

const RANK_TEXT = {
  Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8',
  Nine: '9', Ten: '10', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
};
const SUIT_TEXT = { Spades: '♠', Hearts: '♥', Diamonds: '♦', Clubs: '♣' };

/** The community board as it stands. Public state; no hole cards. */
export function boardOf(view) {
  const cards = view?.community_cards;
  if (!cards?.length) return [];
  return cards.map((c) => {
    const suit = tagOf(c.suit);
    return {
      rank: RANK_TEXT[tagOf(c.rank)] ?? '?',
      suit: SUIT_TEXT[suit] ?? '?',
      red: suit === 'Hearts' || suit === 'Diamonds',
    };
  });
}

/** A "<sb>/<bb>" pair anywhere in a registered table name. */
const NAME_STAKES_RE = /(\d[\d.,]*)\s*\/\s*(\d[\d.,]*)/;
const sameQuote = (a, b) => String(a).replace(/\s+/g, '') === String(b).replace(/\s+/g, '');

/**
 * Splits a registered name around the price it quotes and says whether that
 * price is the one the TABLE contract enforces. The name is never rewritten:
 * a figure the contract contradicts is struck through by the row.
 */
export function nameQuote(table, view) {
  const name = String(table?.name ?? '');
  const m = NAME_STAKES_RE.exec(name);
  if (!m) return { before: name, quoted: null, after: '', stale: false, chain: null };
  const cfg = effectiveConfig(table, view);
  const chain = formatBlinds(cfg.small_blind, cfg.big_blind, currencyOf(table));
  return {
    before: name.slice(0, m.index),
    quoted: m[0],
    after: name.slice(m.index + m[0].length),
    stale: !sameQuote(m[0], chain),
    chain,
  };
}

/**
 * @typedef {object} LobbyRow
 * @property {string} id
 * @property {object} table        the lobby record, untouched
 * @property {string} name
 * @property {string|null} canisterId
 * @property {'ICP'|'BTC'} currency
 * @property {object} cfg          the effective config
 * @property {Array<object|null>} seats
 * @property {number} filled
 * @property {number} idle
 * @property {boolean} isFull
 * @property {{kind:string,label:string,detail:string|null}} now
 * @property {number|null} hands
 * @property {string[]} drift
 * @property {ReturnType<typeof nameQuote>} nameq
 * @property {string} format
 * @property {string} tier
 * @property {string} clockText
 */

/** Everything a row card renders, computed once per table per refresh. */
export function buildRow(table, view) {
  const cfg = effectiveConfig(table, view);
  const seats = seatsOf(table, view);
  const filled = seats.filter(Boolean).length;
  const currency = currencyOf(table);
  return {
    id: String(table.id),
    table,
    name: String(table.name ?? ''),
    canisterId: canisterIdOf(table),
    currency,
    cfg,
    seats,
    filled,
    idle: seats.filter((s) => s && !s.active).length,
    isFull: filled >= Number(cfg.max_players),
    now: nowOf(table, view),
    hands: handsOf(view),
    drift: configDrift(table, view),
    nameq: nameQuote(table, view),
    format: tableFormat(cfg.max_players),
    tier: stakeTier(cfg.small_blind, currency),
    clockText: `${cfg.action_timeout_secs}s + ${cfg.time_bank_secs}s`,
  };
}

/**
 * Where a first-time visitor should start: the cheapest ICP table by minimum
 * buy-in (an ICP table over a sats one, because ICP is the unit the cashier
 * and the stakes are read in), or the cheapest table of any kind.
 *
 * @param {object[]} tables
 * @param {(table: object) => object|null} viewOf
 */
export function starterTable(tables, viewOf) {
  const byBuyIn = (a, b) =>
    Number(effectiveConfig(a, viewOf(a)).min_buy_in) - Number(effectiveConfig(b, viewOf(b)).min_buy_in);
  const icp = tables.filter((t) => currencyOf(t) === 'ICP').sort(byBuyIn);
  if (icp.length) return icp[0];
  return [...tables].sort(byBuyIn)[0] ?? null;
}
