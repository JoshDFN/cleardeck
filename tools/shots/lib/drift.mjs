// Deliberate UI/chain disagreement, for proving the verifier has teeth.
//
// A verifier nobody has ever seen fail is not evidence, it is decoration. This
// module makes the rendered page lie about a specific number — the chain is
// untouched, the canister still says what it said — and the run must then refuse
// the canonical filename and write UNVERIFIED-<scene>-<viewport>.png.
//
//   SHOTS_INJECT_DRIFT=pot        node tools/shots/run.mjs --scenes table-preflop
//   SHOTS_INJECT_DRIFT=stack,board node tools/shots/run.mjs --scenes table-sidepots
//
// OFF by default and never used by a real run. It edits the DOM after render; it
// does not intercept, mock or alter a single canister response, which is what
// makes it a fair test of "does the screen agree with the chain".

/** Every fault this module can inject, and what it does to the page. */
export const DRIFT_TARGETS = {
    pot: { selector: '.pot-amount', how: 'double the headline pot' },
    breakdown: { selector: '.pot-breakdown', how: 'double the first leg of the pot breakdown' },
    stack: { selector: '.chips', how: "double the first seat's stack" },
    bet: { selector: '.bet-amount', how: "double the first seat's live bet" },
    sidepot: { selector: '.side-pot-amount', how: 'double side pot 1' },
    balance: { selector: '.balance-value', how: 'double the table balance' },
    // Card.svelte's face markup was rewritten mid-wave: `.corner-rank` became
    // `.rank`. Both are listed so this module keeps injecting across that change,
    // and a target whose selector matches NOTHING is a hard failure (see
    // injectDrift) rather than a quietly skipped fault.
    board: {
        selector: '.community-cards .card .rank, .community-cards .card .corner-rank',
        how: 'change the first board card rank',
    },
    winner: { selector: '.winner-display .winner-text', how: 'double the amount in the winner banner' },
    potodds: { selector: '.pot-odds-value', how: 'double the pot-odds ratio' },
    headerstakes: {
        selector: '.current-table-name',
        how: 'double the blinds quoted in the table header pill',
    },
    callbutton: { selector: '.actions .action-btn.primary', how: 'double the amount on the Call button' },
};

/** @returns {string[]} the drift targets requested for this run (possibly empty). */
export function requestedDrift() {
    const raw = (process.env.SHOTS_INJECT_DRIFT || '').trim();
    if (!raw) return [];
    const names = raw.split(',').map((s) => s.trim()).filter(Boolean);
    const unknown = names.filter((n) => !DRIFT_TARGETS[n]);
    if (unknown.length) {
        throw new Error(
            `SHOTS_INJECT_DRIFT: unknown target(s) ${unknown.join(', ')}. `
            + `Known: ${Object.keys(DRIFT_TARGETS).join(', ')}`,
        );
    }
    return names;
}

/**
 * Rewrites the requested figures in the live page and keeps them rewritten.
 *
 * @param {import('playwright').Page} page
 * @param {string[]} targets
 * @returns {Promise<Array<{target:string, selector:string, before:string, after:string}>>}
 */
export async function injectDrift(page, targets) {
    if (!targets.length) return [];
    return page.evaluate(({ targets: names, table }) => {
        const doubleNumbers = (text) =>
            text.replace(/\d[\d,]*(?:\.\d+)?/, (m) => {
                const n = Number(m.replace(/,/g, ''));
                if (!Number.isFinite(n)) return m;
                const decimals = m.includes('.') ? m.split('.')[1].length : 0;
                return (n * 2).toFixed(decimals);
            });

        // The header pill reads "<shape> · <sb>/<bb>", and its FIRST number is
        // the seat count ("6-Max"), not money. Doubling that would leave the
        // blinds correct and the fault would not fire, so the two blinds are
        // targeted by position instead.
        const doubleBlinds = (text) =>
            text.replace(/(\d[\d,]*(?:\.\d+)?)\s*\/\s*(\d[\d,]*(?:\.\d+)?)/, (_m, a, b) => {
                const twice = (s) => {
                    const n = Number(String(s).replace(/,/g, ''));
                    if (!Number.isFinite(n)) return s;
                    const d = String(s).includes('.') ? String(s).split('.')[1].length : 0;
                    return (n * 2).toFixed(d);
                };
                return `${twice(a)}/${twice(b)}`;
            });

        const bumpRank = (text) => {
            const order = ['2', '3', '4', '5', '6', '7', '8', '9', '10', 'J', 'Q', 'K', 'A'];
            const i = order.indexOf(text.trim());
            return i === -1 ? 'A' : order[(i + 1) % order.length];
        };

        const applied = [];
        const plan = [];
        for (const name of names) {
            const selector = table[name].selector;
            const el = document.querySelector(selector);
            if (!el) {
                applied.push({ target: name, selector, before: null, after: null, note: 'selector not present on this page' });
                continue;
            }
            const before = el.textContent;
            let after;
            if (name === 'board') after = bumpRank(before);
            else if (name === 'headerstakes') after = doubleBlinds(before);
            else after = doubleNumbers(before);
            plan.push({ selector, after });
            applied.push({ target: name, selector, before: before.trim(), after: after.trim() });
        }

        // Svelte only rewrites a text node when its own value changes, and the
        // table is at rest, so a single write would probably survive. "Probably"
        // is not good enough for a fault-injection proof, so it is re-applied.
        const reapply = () => {
            for (const p of plan) {
                const el = document.querySelector(p.selector);
                if (el && el.textContent !== p.after) el.textContent = p.after;
            }
        };
        reapply();
        const observer = new MutationObserver(reapply);
        observer.observe(document.body, { childList: true, subtree: true, characterData: true });
        window.__cleardeckDriftInterval = setInterval(reapply, 100);

        return applied;
    }, { targets, table: DRIFT_TARGETS });
}
