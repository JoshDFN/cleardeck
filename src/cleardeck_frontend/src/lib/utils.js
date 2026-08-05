/**
 * Candid helpers and the CANONICAL money layer for the ClearDeck client.
 *
 * WHY A CANONICAL MONEY LAYER EXISTS
 * ----------------------------------
 * As of wave 3 there are six near-identical, independently maintained copies of
 * "divide by 1e8 and round" in this frontend:
 *
 *   PokerTable.svelte    formatAmount / formatICP / formatChips / formatWithUnit
 *   Lobby.svelte         formatAmount / formatBlinds / formatBuyIn
 *   HandHistory.svelte   formatChips        (appends " ICP", others do not)
 *   ActionFeed.svelte    formatChips
 *   DepositModal.svelte  formatBalance / formatWithUnit  (always 4 dp for ICP)
 *   WithdrawModal.svelte formatBalance / formatWithUnit
 *   routes/+page.svelte  formatICP          (dead code, kept in step by nobody)
 *
 * They already disagree with each other about how many decimals an ICP figure
 * gets, so the same on-chain number renders differently depending on which
 * component you are looking at. That is the soil the real defects grew in:
 *
 *   docs/DEFECTS.md T-08  the headline POT is rendered as `pot + liveBets`, but
 *                         `state.pot` in the canister ALREADY includes every
 *                         live bet, so the pot is displayed at 2x during every
 *                         betting round — while the pot-odds strip on the same
 *                         screen renders the same field correctly.
 *   docs/DEFECTS.md T-09  `hole_cards` is a Candid `opt (Card, Card)`, i.e.
 *                         `[] | [[Card, Card]]`. Indexing it as if it were the
 *                         tuple gives the whole tuple and `undefined`, so the
 *                         villain's revealed hand renders as two blanks; and
 *                         `Object.keys(optHandRank)[0]` on an `opt` array yields
 *                         the literal string "0" where the hand name belongs.
 *
 * Everything below reads the canister's own fields and nothing else. There is no
 * arithmetic here that could turn one on-chain number into a different displayed
 * number: that is the property the screenshot harness now asserts scene by scene
 * (tools/shots/lib/chain-agreement.mjs), and the only way to keep it true is to
 * have one place where money becomes text.
 */

/** Smallest unit per whole token: e8s per ICP, and sats per ckBTC. Both 1e8. */
export const SMALLEST_UNITS_PER_TOKEN = 100_000_000;

/**
 * Unwraps a Candid optional type (which comes as an array: [] for None, [value] for Some)
 * @param {Array} candidOpt - The Candid optional value
 * @returns {*} The unwrapped value or null
 */
export function unwrapOpt(candidOpt) {
    return candidOpt && candidOpt.length > 0 ? candidOpt[0] : null;
}

/**
 * Unwraps a Candid optional number and converts BigInt to Number
 * @param {Array} candidOpt - The Candid optional value
 * @returns {number|null} The unwrapped number or null
 */
export function unwrapOptNum(candidOpt) {
    if (!candidOpt || candidOpt.length === 0) return null;
    const val = candidOpt[0];
    return typeof val === 'bigint' ? Number(val) : val;
}

/**
 * Converts a Principal to string, handling both Principal objects and strings
 * @param {Principal|string} principal - The principal to convert
 * @returns {string} The principal as a string
 */
export function principalToString(principal) {
    if (!principal) return '';
    if (typeof principal === 'string') return principal;
    if (principal.toText) return principal.toText();
    return principal.toString();
}

/**
 * Formats chip amounts for display (e.g. 1000 -> "1K", 1000000 -> "1M").
 *
 * NOTE this one formats the RAW SMALLEST UNIT, so on an ICP table it renders e8s
 * and not ICP. Nothing in the app calls it. Prefer formatTokenAmount().
 *
 * @param {number|bigint} amount - The chip amount
 * @returns {string} Formatted string
 */
export function formatChips(amount) {
    const num = Number(amount);
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
    return num.toLocaleString();
}

/**
 * The one formatter. Turns a smallest-unit on-chain amount into display text.
 *
 * Deliberately identical in behaviour to PokerTable.svelte's `formatAmount`,
 * because that is the copy the screenshots and the design bar were derived from;
 * adopting it elsewhere therefore changes no pixel on the felt while removing the
 * five divergent copies.
 *
 * @param {number|bigint|null|undefined} smallestUnit e8s on an ICP table, sats on a BTC table
 * @param {{currency?: 'ICP'|'BTC', includeUnit?: boolean, placeholder?: string}} [opts]
 * @returns {string}
 */
export function formatTokenAmount(smallestUnit, { currency = 'ICP', includeUnit = false, placeholder = '--' } = {}) {
    if (smallestUnit === null || smallestUnit === undefined) return placeholder;
    const num = typeof smallestUnit === 'bigint' ? Number(smallestUnit) : Number(smallestUnit);
    if (!Number.isFinite(num)) return placeholder;

    if (currency === 'BTC') {
        if (num >= SMALLEST_UNITS_PER_TOKEN) return `${(num / SMALLEST_UNITS_PER_TOKEN).toFixed(2)} BTC`;
        if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M${includeUnit ? ' sats' : ''}`;
        if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K${includeUnit ? ' sats' : ''}`;
        return `${num}${includeUnit ? ' sats' : ''}`;
    }

    const tokens = num / SMALLEST_UNITS_PER_TOKEN;
    const unit = includeUnit ? ' ICP' : '';
    if (tokens >= 1000) return `${(tokens / 1000).toFixed(1)}K${unit}`;
    if (tokens >= 0.01) return tokens.toFixed(2) + unit;
    return tokens.toFixed(4) + unit;
}

/** The currency name out of a TableConfig's `opt variant { ICP; BTC }`. */
export function currencyOf(config) {
    const raw = config?.currency;
    const inner = Array.isArray(raw) ? (raw.length ? raw[0] : null) : raw;
    if (!inner) return 'ICP';
    if (typeof inner === 'string') return inner.toUpperCase() === 'BTC' ? 'BTC' : 'ICP';
    return Object.keys(inner)[0] === 'BTC' ? 'BTC' : 'ICP';
}

/** `players` from a TableView, with each `opt PlayerView` unwrapped to a value or null. */
export function seatedPlayers(tableState) {
    return (tableState?.players || []).map((p) => (p && p.length > 0 ? p[0] : null));
}

/**
 * THE POT. The number the canister will pay out, in smallest units.
 *
 * `state.pot` is incremented the instant a player posts a blind, calls, bets,
 * raises or goes all in (src/table_canister/src/lib.rs — every one of those arms
 * does `state.pot = state.pot.saturating_add(...)`), and `player.current_bet` is
 * set in the same arm. The live bets are therefore ALREADY INSIDE `pot`, and
 * `get_pot()` returns exactly this field.
 *
 * So the pot to display is `pot`, full stop. Adding the live bets to it is
 * docs/DEFECTS.md T-08 and doubles the headline figure during every betting
 * round.
 *
 * @param {object} tableState a TableView from get_table_view()
 * @returns {number} smallest units
 */
export function potTotal(tableState) {
    return Number(tableState?.pot ?? 0);
}

/**
 * The part of the pot that is committed in the CURRENT betting round: the chips
 * still sitting in front of the players rather than in the middle.
 *
 * This is a presentation split of `potTotal()`, never an addition to it.
 *
 * @param {object} tableState
 * @returns {number} smallest units
 */
export function liveBetsTotal(tableState) {
    return seatedPlayers(tableState).reduce(
        (total, player) => total + Number(player?.current_bet ?? 0),
        0,
    );
}

/**
 * The part of the pot collected in PREVIOUS betting rounds, i.e. the pot minus
 * the live bets. `potBeforeThisRound + liveBetsTotal === potTotal` by
 * construction, which is what the screenshot harness asserts about the on-screen
 * breakdown.
 *
 * @param {object} tableState
 * @returns {number} smallest units
 */
export function potBeforeThisRound(tableState) {
    return Math.max(0, potTotal(tableState) - liveBetsTotal(tableState));
}

/**
 * A player's two hole cards, or null when they are not visible to us.
 *
 * `hole_cards` is a Candid `opt (Card, Card)` and therefore arrives as
 * `[] | [[Card, Card]]`. docs/DEFECTS.md T-09: `player.hole_cards[0]` is the
 * whole TUPLE and `player.hole_cards[1]` is `undefined`, which renders the
 * villain's revealed hand as two blank rectangles at showdown.
 *
 * @param {object} playerView a PlayerView from get_table_view()
 * @returns {[object, object]|null}
 */
export function holeCardsOf(playerView) {
    const opt = playerView?.hole_cards;
    if (!Array.isArray(opt) || opt.length === 0) return null;
    const pair = opt[0];
    if (!Array.isArray(pair) || pair.length < 2) return null;
    return [pair[0], pair[1]];
}

/**
 * The human name of a hand rank that arrives as a Candid `opt variant`.
 *
 * docs/DEFECTS.md T-09: `Object.keys(handRank)[0]` on the `opt` ARRAY returns the
 * array index `"0"`, so the winner banner prints `0` where "Pair" belongs. The
 * hand-history modal, which unwraps first, has always printed it correctly.
 *
 * @param {Array|object|null} optHandRank
 * @returns {string} e.g. "Two Pair", or '' when there is no rank
 */
export function handRankName(optHandRank) {
    const variant = Array.isArray(optHandRank)
        ? (optHandRank.length ? optHandRank[0] : null)
        : optHandRank;
    if (!variant || typeof variant !== 'object') return '';
    const key = Object.keys(variant)[0];
    if (!key) return '';
    return key.replace(/([A-Z])/g, ' $1').trim();
}

/**
 * Gets the phase name from a Candid variant
 * @param {Object} phase - The phase variant
 * @returns {string} The phase name
 */
export function getPhaseName(phase) {
    if (!phase) return 'Unknown';
    return Object.keys(phase)[0];
}
