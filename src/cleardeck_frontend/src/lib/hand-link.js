// A hand's stable name and its deep link.
//
// The lobby's invite link (invite-link.js) opens ONE TABLE. A hand needs one
// more coordinate: its number on that table. `?table=<canister>&hand=N` opens
// the table and its replayer on hand N, so a proof can be pasted anywhere and
// land on the exact hand it talks about.
//
// The visible id, "table_1#2", is the lobby's name for the table slugged plus
// the hand number: readable in a chat message, and stable for as long as the
// table keeps its name. The LINK carries the canister id, which is the table's
// only permanent identity; the visible id is a label for people.

import { TABLE_PARAM, isCanisterId } from './invite-link.js';

export const HAND_PARAM = 'hand';

/** The largest hand number a link is allowed to ask for. */
const MAX_HAND_NUMBER = 1_000_000_000;

/**
 * "6-Max Table 1" -> "6_max_table_1". Lowercase, runs of anything that is not a
 * letter or a digit collapse to one underscore, and nothing leads or trails.
 *
 * @param {unknown} name
 * @returns {string} the slug, or "table" when there is nothing to slug
 */
export function tableSlug(name) {
  const text = typeof name === 'string' ? name : '';
  const slug = text.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '');
  return slug || 'table';
}

/**
 * The stable, human id of one hand: "<slug>#<n>".
 *
 * @param {unknown} tableName
 * @param {unknown} handNumber
 * @returns {string|null} null unless the hand number is a positive integer
 */
export function handIdFor(tableName, handNumber) {
  const n = Number(handNumber);
  if (!Number.isInteger(n) || n < 1) return null;
  return `${tableSlug(tableName)}#${n}`;
}

/**
 * A positive integer hand number, or null for anything else.
 * @param {unknown} value
 */
export function parseHandNumber(value) {
  if (value === null || value === undefined) return null;
  const text = String(value).trim();
  if (!/^\d{1,10}$/.test(text)) return null;
  const n = Number(text);
  return n >= 1 && n <= MAX_HAND_NUMBER ? n : null;
}

/**
 * The hand a URL asks for, or null. Meaningless without a table, so a `hand`
 * with no valid `table` beside it is ignored.
 *
 * @param {string} search  window.location.search
 */
export function handNumberFromSearch(search) {
  if (typeof search !== 'string' || !search) return null;
  const params = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  if (!isCanisterId(params.get(TABLE_PARAM))) return null;
  return parseHandNumber(params.get(HAND_PARAM));
}

/**
 * The query string with the hand set (or cleared when `handNumber` is null),
 * every other parameter kept. A NEW string; the input is never mutated.
 *
 * @param {string} search
 * @param {number|null} handNumber
 */
export function searchWithHand(search, handNumber) {
  const params = new URLSearchParams(
    typeof search === 'string' && search ? search.replace(/^\?/, '') : '',
  );
  params.delete(HAND_PARAM);
  const n = parseHandNumber(handNumber);
  if (n !== null && isCanisterId(params.get(TABLE_PARAM))) params.set(HAND_PARAM, String(n));
  const text = params.toString();
  return text ? `?${text}` : '';
}

/**
 * The share link for one hand, on the page's own origin and path.
 *
 * @param {{origin:string, pathname?:string}} location
 * @param {string} canisterId
 * @param {number} handNumber
 * @returns {string|null} null unless both coordinates are valid
 */
export function handLinkFor(location, canisterId, handNumber) {
  const n = parseHandNumber(handNumber);
  if (!location?.origin || !isCanisterId(canisterId) || n === null) return null;
  const path = location.pathname && location.pathname !== '/' ? location.pathname : '/';
  return `${location.origin}${path}?${TABLE_PARAM}=${canisterId}&${HAND_PARAM}=${n}`;
}
