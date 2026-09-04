// Reading a money figure OFF THE SCREEN and deciding whether it can possibly be
// the number the canister reported.
//
// WHY THIS IS NOT JUST STRING COMPARISON
// -------------------------------------
// Nothing in the client displays an e8s integer. Every money figure goes through
// one of six near-identical formatters (PokerTable, Lobby, HandHistory,
// ActionFeed, DepositModal, WithdrawModal — see docs and the audit in
// docs/RESPONSIVENESS.md's sibling report) that divide by 1e8 and then
// `toFixed(2)` or `toFixed(4)`, or fold into `K`/`M`. The display is therefore
// LOSSY, and "does the screen agree with the chain" has to be asked at the
// precision the screen actually chose.
//
// So a displayed figure is decoded back into the CLOSED INTERVAL of chain values
// that could have produced it, and the chain value must lie inside it. That is
// exact — it makes no assumption about which formatter ran — and it is honest
// about its own resolution, which is recorded per figure.
//
// A check that cannot distinguish `x` from `2x` is not a check. Every non-zero
// figure therefore also reports `discriminates2x`, and a scene fails if a money
// figure is asserted at a precision too coarse to have caught the very defect
// this file exists for (docs/DEFECTS.md T-08). Silence from a blind assertion is
// worse than no assertion at all.

/** Smallest unit per whole token. e8s for ICP, sats for BTC — both 1e8. */
export const SMALLEST_PER_TOKEN = 100_000_000;

/** Floating-point slack, in smallest units. Sub-1-e8 and therefore harmless. */
const EPS = 1e-6;

/**
 * One number as it appears on screen, decoded to a chain-value interval.
 *
 * @typedef {object} ParsedAmount
 * @property {string} text          the raw text it was read from
 * @property {number} display       the numeric literal on screen (e.g. 0.30)
 * @property {number} decimals      decimal places shown (drives the resolution)
 * @property {string} suffix        '', 'K' or 'M'
 * @property {string} unit          'ICP' | 'BTC' | 'sats' | '' (as written)
 * @property {number} scale         smallest units per displayed unit
 * @property {number} low           lowest chain value that renders as `text`
 * @property {number} high          highest chain value that renders as `text`
 * @property {number} resolution    high - low, in smallest units
 */

const NUMBER_RE = /(-?\d[\d,]*(?:\.\d+)?)\s*([KkMm])?\s*(ICP|BTC|sats)?/;

/**
 * Decodes a displayed money string into the interval of chain values that could
 * have produced it.
 *
 * @param {string|null|undefined} text raw DOM text, e.g. "0.30", "1.5K", "12.00 ICP"
 * @param {{currency?: 'ICP'|'BTC'}} [opts] currency of the table the figure belongs to
 * @returns {ParsedAmount|null} null when there is no number in `text` (e.g. "--")
 */
export function parseDisplayedAmount(text, { currency = 'ICP' } = {}) {
    if (text === null || text === undefined) return null;
    const raw = String(text).replace(/ /g, ' ').trim();
    const m = NUMBER_RE.exec(raw);
    if (!m) return null;

    const literal = m[1].replace(/,/g, '');
    const display = Number(literal);
    if (!Number.isFinite(display)) return null;

    const dot = literal.indexOf('.');
    const decimals = dot === -1 ? 0 : literal.length - dot - 1;
    const suffix = (m[2] || '').toUpperCase();
    const unit = m[3] || '';

    // Smallest units per one unit of what is written on screen.
    let scale;
    if (unit === 'BTC') scale = SMALLEST_PER_TOKEN;
    else if (unit === 'sats') scale = 1;
    else if (currency === 'BTC') scale = 1; // BTC tables display bare sats
    else scale = SMALLEST_PER_TOKEN; // ICP tables display bare ICP

    if (suffix === 'K') scale *= 1_000;
    else if (suffix === 'M') scale *= 1_000_000;

    // Every formatter in this app rounds with toFixed(), i.e. to nearest. A
    // literal with `decimals` places therefore stands for a half-ulp window.
    const centre = display * scale;
    const half = 0.5 * Math.pow(10, -decimals) * scale;

    return {
        text: raw,
        display,
        decimals,
        suffix,
        unit,
        scale,
        low: centre - half,
        high: centre + half,
        resolution: 2 * half,
    };
}

/**
 * @typedef {object} FigureCheck
 * @property {string} label            what this figure is, e.g. "pot"
 * @property {number|null} chain       the canister's value in smallest units
 * @property {string|null} domText     what the screen said
 * @property {boolean} agrees          chain value lies inside the displayed interval
 * @property {boolean} discriminates2x the assertion could have caught a 2x error
 * @property {boolean} ok              agrees && (chain === 0 || discriminates2x)
 * @property {string} detail           human-readable verdict
 */

/**
 * Checks one money figure on screen against one number from the canister.
 *
 * @param {string} label
 * @param {bigint|number|null|undefined} chainValue smallest units
 * @param {string|null|undefined} domText
 * @param {{currency?: 'ICP'|'BTC', allowAbsentWhenZero?: boolean}} [opts]
 * @returns {FigureCheck}
 */
export function checkFigure(label, chainValue, domText, opts = {}) {
    const { currency = 'ICP', allowAbsentWhenZero = false } = opts;
    const chain = chainValue === null || chainValue === undefined ? null : Number(chainValue);
    const parsed = parseDisplayedAmount(domText, { currency });

    if (chain === null) {
        return {
            label, chain: null, domText: domText ?? null, agrees: false, discriminates2x: false,
            ok: false, detail: 'no canister value to compare against (harness bug)',
        };
    }

    if (!parsed) {
        // An absent figure is correct only when the canister says the figure is
        // zero AND the caller said absence is how zero is rendered.
        const ok = allowAbsentWhenZero && chain === 0;
        return {
            label,
            chain,
            domText: domText ?? null,
            agrees: ok,
            discriminates2x: false,
            ok,
            detail: ok
                ? `absent on screen, and the canister says ${chain} — consistent`
                : `NOTHING NUMERIC ON SCREEN (${JSON.stringify(domText ?? null)}) but the canister says ${chain}`,
        };
    }

    const agrees = chain >= parsed.low - EPS && chain <= parsed.high + EPS;

    // "Would a wrong value have LOOKED different?" — asked by re-rendering the
    // alternative at the precision this figure was actually shown at, rather than
    // by interval containment, so a boundary hit is not mistaken for blindness.
    const renders = (v) => (v / parsed.scale).toFixed(parsed.decimals);
    const shown = renders(chain);
    // Gate on the OVERSTATEMENT direction: that is the shape of T-08 (a pot shown
    // at 2x) and the direction that flatters the house.
    const discriminates2x = chain === 0 ? true : renders(chain * 2) !== shown;
    // Understatement at a coarse precision is a property of the client's own
    // formatter, not of this check, so it is reported and not gated.
    const detectsHalving = chain === 0 ? true : renders(chain / 2) !== shown;
    const ok = agrees && (chain === 0 || discriminates2x);

    let detail;
    if (!agrees) {
        const ratio = chain === 0 ? Infinity : ((parsed.low + parsed.high) / 2) / chain;
        detail =
            `DISAGREES: screen "${parsed.text}" means ${fmt(parsed.low)}..${fmt(parsed.high)}, ` +
            `canister says ${fmt(chain)}` +
            (Number.isFinite(ratio) ? ` (screen is ${ratio.toFixed(3)}x the chain)` : '');
    } else if (!discriminates2x) {
        detail =
            `agrees, but the check is BLIND: "${parsed.text}" is displayed so coarsely ` +
            `(window ${fmt(parsed.resolution)}) that ${fmt(chain * 2)} would render identically`;
    } else {
        detail = `screen "${parsed.text}" == canister ${fmt(chain)} (±${fmt(parsed.resolution / 2)})`
            + (detectsHalving ? '' : `; NOTE the client's precision here cannot distinguish ${fmt(chain / 2)}`);
    }

    return {
        label, chain, domText: parsed.text, agrees, discriminates2x, detectsHalving, ok, detail,
        window: [parsed.low, parsed.high], resolution: parsed.resolution,
    };
}

/**
 * Checks a displayed figure that is NOT a chain amount and needs no unit scaling
 * — a fiat conversion, a player count, a percentage.
 *
 * @param {string} label
 * @param {number|null} expected
 * @param {string|null|undefined} domText
 * @param {{unit?:string}} [opts]
 * @returns {FigureCheck}
 */
export function checkPlainNumber(label, expected, domText, { unit = '' } = {}) {
    const parsed = parseDisplayedAmount(domText, { currency: 'BTC' }); // scale 1
    if (expected === null || expected === undefined) {
        return {
            label, chain: null, domText: domText ?? null, agrees: false,
            discriminates2x: false, ok: false, detail: 'no expected value supplied',
        };
    }
    if (!parsed) {
        return {
            label, chain: expected, domText: domText ?? null, agrees: false,
            discriminates2x: false, ok: false,
            detail: `no number on screen (${JSON.stringify(domText ?? null)}) but expected ${expected}${unit}`,
        };
    }
    // parseDisplayedAmount with currency BTC gives scale 1 for a bare literal,
    // which is what a fiat figure or a count needs.
    const agrees = expected >= parsed.low - EPS && expected <= parsed.high + EPS;
    const renders = (v) => (v / parsed.scale).toFixed(parsed.decimals);
    const discriminates2x = expected === 0 ? true : renders(expected * 2) !== renders(expected);
    return {
        label,
        chain: expected,
        domText: parsed.text,
        agrees,
        discriminates2x,
        ok: agrees && (expected === 0 || discriminates2x),
        detail: agrees
            ? `screen "${parsed.text}" == expected ${expected}${unit}`
            : `DISAGREES: screen "${parsed.text}" means ${parsed.low}..${parsed.high}, expected ${expected}${unit}`,
        window: [parsed.low, parsed.high],
    };
}

/**
 * The DISPLAY UNIT the client quantises its bet-sizing proposals to, in the
 * smallest unit. Mirrors PokerTable.svelte's precision rule (`decimals`: 0 on
 * a BTC table, 4 when the big blind is under 0.01 ICP, else 2) and
 * $lib/bet-sizing.js `displayQuantum`: 10^(8 - decimals) e8s for ICP, one
 * sat for BTC. A preset is expected to be the formula's figure rounded DOWN
 * onto this grid, so the figure on the button is the figure sent; before the
 * client quantised, the two-thirds preset showed 0.47 and sent 0.46666666.
 */
export function displayQuantumFor(currency, bigBlindSmallest) {
    if (currency === 'BTC') return 1;
    const bb = Number(bigBlindSmallest);
    const decimals = bb > 0 && bb < 1_000_000 ? 4 : 2;
    return 10 ** (8 - decimals);
}

/** Round a smallest-unit figure DOWN onto the display grid. */
export function quantiseDown(value, quantum) {
    const q = Math.max(1, Math.trunc(Number(quantum) || 1));
    return Math.floor(Number(value) / q) * q;
}

/** Compact rendering of a smallest-unit amount for verdict strings. */
export function fmt(smallest) {
    if (!Number.isFinite(smallest)) return String(smallest);
    return `${smallest} e8s`;
}

/**
 * Checks a list of figures and folds them into one verdict.
 * @param {FigureCheck[]} figures
 */
export function foldFigures(figures) {
    const bad = figures.filter((f) => !f.ok);
    return {
        ok: bad.length === 0,
        checked: figures.length,
        mismatches: bad.map((f) => `${f.label}: ${f.detail}`),
        figures,
    };
}

/** Suit symbol the app renders -> the canister's suit name. */
export const SUIT_BY_SYMBOL = { '♥': 'Hearts', '♦': 'Diamonds', '♣': 'Clubs', '♠': 'Spades' };

/** Rank glyph the app renders -> the canister's rank name. */
export const RANK_BY_GLYPH = {
    2: 'Two', 3: 'Three', 4: 'Four', 5: 'Five', 6: 'Six', 7: 'Seven', 8: 'Eight',
    9: 'Nine', 10: 'Ten', J: 'Jack', Q: 'Queen', K: 'King', A: 'Ace',
};

/** "As" style short form of a canister card, for readable verdicts. */
export function cardToText(card) {
    if (!card) return '--';
    const rank = Object.keys(card.rank ?? {})[0] ?? '?';
    const suit = Object.keys(card.suit ?? {})[0] ?? '?';
    const glyph = Object.entries(RANK_BY_GLYPH).find(([, v]) => v === rank)?.[0] ?? '?';
    const sym = Object.entries(SUIT_BY_SYMBOL).find(([, v]) => v === suit)?.[0] ?? '?';
    return `${glyph}${sym}`;
}
