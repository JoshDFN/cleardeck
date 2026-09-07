// The invite link: a URL that opens one table.
//
// Since nobody has ever played on mainnet, the product's growth loop is the
// same as PokerNow's: sit down, send the link, the hand deals when your
// opponent arrives. The link carries the table's canister id, which is the
// one identifier that is the table (the lobby's numeric id is a registry row).

export const TABLE_PARAM = 'table';

/** A principal-shaped text: groups of base32 separated by dashes, ending -cai. */
const CANISTER_ID_RE = /^[a-z0-9]{5}(?:-[a-z0-9]{5})*-cai$/;

export const isCanisterId = (text) => typeof text === 'string' && CANISTER_ID_RE.test(text);

/**
 * The share link for a table, on the page's own origin and path.
 *
 * @param {{origin:string, pathname?:string}} location  window.location, or a stand-in
 * @param {string} canisterId
 * @returns {string|null} null for anything that is not a canister id
 */
export function inviteLinkFor(location, canisterId) {
  if (!location?.origin || !isCanisterId(canisterId)) return null;
  const path = location.pathname && location.pathname !== '/' ? location.pathname : '/';
  return `${location.origin}${path}?${TABLE_PARAM}=${canisterId}`;
}

/**
 * The table a URL asks for, or null. Anything that is not a canister id is
 * ignored rather than passed on, so a hostile query string opens nothing.
 *
 * @param {string} search  window.location.search
 */
export function tableIdFromSearch(search) {
  if (typeof search !== 'string' || !search) return null;
  const params = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  const id = params.get(TABLE_PARAM);
  return isCanisterId(id) ? id : null;
}

/**
 * The query string the page should carry while a table is open (or the lobby
 * is showing): a NEW string, never a mutation of the current one.
 *
 * @param {string} search  the current window.location.search
 * @param {string|null} canisterId  the open table, or null on the lobby
 */
export function searchWithTable(search, canisterId) {
  const params = new URLSearchParams(
    typeof search === 'string' && search ? search.replace(/^\?/, '') : '',
  );
  params.delete(TABLE_PARAM);
  if (isCanisterId(canisterId)) params.set(TABLE_PARAM, canisterId);
  const text = params.toString();
  return text ? `?${text}` : '';
}
