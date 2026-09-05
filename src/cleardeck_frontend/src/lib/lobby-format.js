// Pure formatting for the lobby: amounts, blinds, buy-in ranges, stake tiers,
// short identifiers and the fiat hint under a money figure.
//
// Everything here is a function of numbers the canisters gave us. Nothing
// fetches, nothing reads the DOM, nothing rounds a money figure differently
// from the way the rest of the client does (formatAmount is the lobby's
// long-standing rule, moved here unchanged so the harness's chain-agreement
// parser keeps reading the same shapes).

export const E8S = 100_000_000;

/** The unit word a player reads beside a figure. */
export const unitOf = (currency) => (currency === 'BTC' ? 'sats' : 'ICP');

/**
 * A chain amount (e8s or sats) as the lobby prints it.
 *
 * ICP: two decimals from 0.01 up, four below, K above 1000. Sats: integral,
 * K/M above a thousand / a million, BTC above 1e8.
 */
export function formatAmount(raw, currency) {
  const num = Number(raw);
  if (currency === 'BTC') {
    if (num >= 100_000_000) return `${(num / 100_000_000).toFixed(2)} BTC`;
    if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
    if (num >= 1_000) return `${(num / 1_000).toFixed(0)}K`;
    return `${num}`;
  }
  const icp = num / E8S;
  if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K`;
  if (icp >= 0.01) return icp.toFixed(2);
  if (icp === 0) return '0.00';
  return icp.toFixed(4);
}

export const formatBlinds = (sb, bb, currency) =>
  `${formatAmount(sb, currency)}/${formatAmount(bb, currency)}`;

// Spaces around the dash matter: the screenshot harness reads two numbers out
// of this cell (tools/shots/lib/dom-scrape.mjs) and a bare "2.00-10.00" parses
// the second one as negative.
export const formatBuyIn = (min, max, currency) =>
  `${formatAmount(min, currency)} – ${formatAmount(max, currency)}`;

/** Stake tier by small blind, in the table's own unit. */
export function stakeTier(smallBlind, currency) {
  const sb = Number(smallBlind);
  if (currency === 'BTC') {
    if (sb <= 200) return 'Micro';
    if (sb <= 1000) return 'Low';
    if (sb <= 5000) return 'Mid';
    if (sb <= 20000) return 'High';
    return 'Nosebleed';
  }
  const icp = sb / E8S;
  if (icp <= 0.02) return 'Micro';
  if (icp <= 0.10) return 'Low';
  if (icp <= 0.50) return 'Mid';
  if (icp <= 2) return 'High';
  return 'Nosebleed';
}

/** The ICP small-blind ceiling of each tier, in ICP (the rule above, inverted). */
const ICP_TIER_CEILING = { Micro: 0.02, Low: 0.10, Mid: 0.50, High: 2 };

/**
 * What a tier means in dollars, for the filter pill's tooltip. Null without a
 * quote: the pill then explains itself in ICP only.
 */
export function tierDefinition(tier, icpUsd) {
  const ceiling = ICP_TIER_CEILING[tier];
  if (ceiling === undefined) return `${tier}: small blind above ${ICP_TIER_CEILING.High} ICP`;
  const inIcp = `small blind up to ${ceiling} ICP`;
  if (!icpUsd) return `${tier}: ${inIcp}`;
  return `${tier}: ${inIcp} (about ${formatUsd(ceiling * icpUsd)})`;
}

export const shortId = (id) => (id && id.length > 16 ? `${id.slice(0, 5)}…${id.slice(-9)}` : id || '');

// Short enough to sit on one line in a half-width proof box: a hash that wraps
// mid-digit reads as corrupted rather than as an elision.
export const shortHash = (h) => (h && h.length > 16 ? `${h.slice(0, 8)}…${h.slice(-6)}` : h || '');

/** "Heads-up" for two seats, "N-max" otherwise. */
export function tableFormat(maxPlayers) {
  const max = Number(maxPlayers);
  return max === 2 ? 'Heads-up' : `${max}-max`;
}

/**
 * A chain amount in dollars, or null when there is no quote for its currency.
 *
 * @param {number|bigint} raw   e8s (ICP) or sats (BTC)
 * @param {'ICP'|'BTC'} currency
 * @param {{icpUsd?:number|null, btcUsd?:number|null}|null} prices
 */
export function fiatUsd(raw, currency, prices) {
  if (!prices) return null;
  const perToken = currency === 'BTC' ? prices.btcUsd : prices.icpUsd;
  if (typeof perToken !== 'number' || !(perToken > 0)) return null;
  return (Number(raw) / E8S) * perToken;
}

/**
 * A dollar figure as the lobby prints it: whole dollars from $100, cents below
 * that, four places under a cent so a micro blind is not shown as $0.00.
 * Rounded (never floored) to the shown precision, so the screenshot harness's
 * half-resolution window contains the exact product.
 */
export function formatUsd(usd) {
  if (typeof usd !== 'number' || !Number.isFinite(usd)) return '';
  const abs = Math.abs(usd);
  const decimals = abs >= 100 ? 0 : abs >= 0.01 || abs === 0 ? 2 : 4;
  return `$${usd.toFixed(decimals)}`;
}

/**
 * The muted line under a pair of money figures: "≈ $0.06 / $0.12" for blinds,
 * "≈ $12.00 – $60.00" for a range. Null when either side has no quote, so the
 * line is all-or-nothing and never half a hint.
 */
export function fiatPair(lowRaw, highRaw, currency, prices, joiner = '/') {
  const low = fiatUsd(lowRaw, currency, prices);
  const high = fiatUsd(highRaw, currency, prices);
  if (low === null || high === null) return null;
  return { low: formatUsd(low), high: formatUsd(high), joiner };
}
