// THE INVERTED MONEY GATE: every number on the screen must be accounted for.
//
// WHAT WAS WRONG WITH THE OLD GATE
// --------------------------------
// `chain-agreement.mjs` asserts a LIST of figures it knows about. That list is a
// numerator with no denominator: a run that printed "14 money figures on screen
// all equal the canister's" was reporting how many numbers the harness chose to
// look at, not how many numbers were on the screen. The wave-3 critic proved the
// gap is real and not theoretical — it rewrote `.current-table-name` (the largest
// teal string on every table scene, quoting the blinds) to "9.99/19.98", and the
// run still reported 14/14 agreement and filed the canonical PNG.
//
// A money surface nobody thought to assert is invisible in a green run. So the
// question is turned around here:
//
//     enumerate EVERY numeric-looking token the page renders, and require each
//     one to be either (a) matched to a figure the chain-agreement layer actually
//     compared with a canister value, or (b) declared non-monetary in
//     tools/shots/token-allowlist.mjs, which is small, in-repo and reviewed.
//
// A token that is neither FAILS the scene. Adding a money surface to the client
// and forgetting to assert it therefore turns a scene red instead of being
// silently absent from the count.
//
// WHY THE ALLOWLIST IS THE DANGEROUS PART, AND HOW IT IS KEPT HONEST
// ------------------------------------------------------------------
// An allowlist that grows to swallow real figures defeats the inversion, so:
//   * every rule is scoped to a CSS selector AND a token pattern, so a rule that
//     says "the countdown is seconds" cannot also excuse a pot that lands in the
//     same element;
//   * every rule carries a one-line reason, and the run manifest prints the whole
//     list plus how many tokens each rule excused, so growth is visible;
//   * the rule list is exported and counted in the census result, so "how big is
//     the allowlist" is a number in the artifact, not a claim in a document.
//
// MATCHING IS ONE-TO-ONE, NOT "SOME CHECK MENTIONED THIS NUMBER"
// ---------------------------------------------------------------
// Three seats showing 12.00 must be covered by three distinct passing checks, not
// by one check three times. Tokens and figures are therefore paired by maximum
// bipartite matching; a fourth 12.00 with only three checks behind it is
// UNASSERTED. That is what stops the count being inflated by coincidence.

import { ALLOWLIST } from '../token-allowlist.mjs';

/**
 * Sites whose tokens ARE asserted against the chain, and the figure label that
 * proves it. `label` is matched against `figures[].label` from
 * chain-agreement.mjs; if no check with that label ran on this scene, tokens at
 * this site are UNASSERTED even though the site is listed here. Listing a site
 * is a claim that something asserts it, and the claim is verified per scene.
 *
 * Order matters: the first rule whose selector matches (via `closest`) wins, so
 * specific selectors come before general ones.
 */
export const CHAIN_SITES = [
    {
        id: 'pot-headline',
        selector: '.pot-amount',
        label: /^pot \(headline\) vs get_pot\(\)$/,
        why: 'the headline pot',
    },
    {
        id: 'pot-breakdown',
        selector: '.pot-breakdown',
        label: /^pot breakdown/,
        why: 'the two legs shown under the pot, which must sum to get_pot()',
    },
    {
        id: 'side-pot',
        selector: '.side-pot-amount',
        label: /^side pot \d+$/,
        why: 'each side pot, in order, against side_pots[i].amount',
    },
    {
        id: 'hero-plate-tag',
        selector: '.player-nameplate.highlight-me .plate-tag',
        label: /^hero plate tag /,
        why: "the tag on the hero's plate: an armed \"Call X\" pre-action against "
            + 'call_amount, or the sent echo ("Raise to X") against the e8s the echo '
            + 'recorded and will send (data-sent-e8s)',
    },
    {
        id: 'seat-stack',
        selector: '.seat .chips, .chips',
        label: /^seat \d+ stack$/,
        why: "each seat's stack against that player's chips",
    },
    {
        id: 'seat-bet',
        selector: '.seat .bet-amount, .bet-amount',
        label: /^seat \d+ bet$/,
        why: "each seat's live bet against that player's current_bet",
    },
    {
        id: 'wallet-balance',
        selector: '.wallet-panel .balance-value, .wallet-collapsed-info .collapsed-balance, .collapsed-balance',
        label: /^table balance vs get_balance\(\)$/,
        why: 'the escrow balance in the wallet panel',
    },
    {
        id: 'committed-stake',
        selector: '.wallet-committed .committed-value',
        label: /^committed stake vs get_table_view\(\)\.my_committed_in_pot$/,
        why: "the caller's own money in the pot, the FINDING 18 readout",
    },
    {
        id: 'committed-note-hand',
        selector: '.wallet-committed .committed-note',
        label: /^the hand named in the committed note vs get_table_view\(\)\.hand_number$/,
        why: 'the hand number the committed-stake sentence names',
    },
    {
        id: 'pot-odds-strip',
        selector: '.pot-odds-explanation',
        label: /^pot-odds "/,
        why: 'the "Call X to win Y" strip',
    },
    {
        id: 'pot-odds-ratio',
        selector: '.pot-odds-value',
        label: /^pot-odds ratio vs/,
        why: 'the pot-odds ratio, derived from get_pot()/call_amount',
    },
    {
        id: 'equity-hint',
        selector: '.equity-hint',
        label: /^required-equity %/,
        why: 'the required-equity percentage, derived from the same two fields',
    },
    {
        id: 'header-stakes-pill',
        selector: '.current-table-name',
        label: /^table header pill .* blind$/,
        why: 'the blinds quoted in the table header pill (docs/DEFECTS.md T-11)',
    },
    {
        id: 'raise-button',
        selector: '.actions .action-btn.raise',
        label: /^action button "Raise to X" vs the value that would be SENT$/,
        why: 'the figure on the primary "Raise to X" / "Bet X" button: the sizer\'s '
            + 'value, which the click SENDS, and which is itself asserted against the '
            + "canister's legal floor at rest and against get_pot() at the presets",
    },
    {
        id: 'raise-sizer-field',
        selector: '.raise-slider-panel .raise-input',
        label: /^bet preset .* vs the value that would be SENT$/,
        why: 'the typed amount field of the dock sizer (BetSizer.svelte): the same '
            + 'figure the range holds, asserted at rest and at the presets',
    },
    {
        id: 'pre-action-call',
        selector: '.pre-actions .pre-btn',
        label: /^pre-action "Call X" vs call_amount$/,
        why: 'the "Call X" pre-action toggle: the call amount promised in advance',
    },
    {
        id: 'call-button',
        selector: '.actions .action-btn',
        label: /^action button "Call X" vs call_amount$/,
        why: 'the amount on the Call button — money the player is about to commit',
    },
    {
        id: 'turn-hint',
        selector: '.turn-hint',
        label: /^turn hint "Call X" vs call_amount$/,
        why: 'the same call amount repeated in the turn hint',
    },
    {
        id: 'winner-amount',
        selector: '.winner-display',
        label: /^winner amount vs last_hand_winners$/,
        why: 'the amount in the winner banner',
    },
    {
        id: 'winner-award-chip',
        selector: '.winner-award .stack-delta, .stack-delta',
        label: /^seat \d+ award chip vs last_hand_winners$/,
        why: 'the "+X" delta chip on the winner\'s pod — the money that seat just received',
    },
    {
        id: 'equity-badge',
        selector: '.equity-badge',
        label: /^seat \d+ (equity vs independent exact enumeration|modelled equity vs independent Monte Carlo)/,
        why: 'the all-in / showdown equity percentage, recomputed by a second evaluator '
            + 'lineage in lib/equity-oracle.mjs from the canister\'s own cards. Not a money '
            + 'figure and not allowlisted either: a percentage a player calls off has to be '
            + 'checked by something',
    },
    {
        id: 'raise-slider-readout',
        selector: '.raise-slider-panel .slider-amount, .raise-slider-panel .confirm-raise',
        label: /^bet preset .* vs the value that would be SENT$/,
        why: 'what the bet-sizing popover says it will wager',
    },
    {
        // Before lobby-stakes / lobby-buyin / lobby-preview-fact: the hint is
        // INSIDE those cells, and the first matching selector wins.
        id: 'lobby-fiat',
        selector: '.fiat-num',
        label: /^lobby (".*"|preview) fiat (sb|bb|min|max) vs /,
        why: 'the dollar hint under a lobby stakes or buy-in figure: the chain figure '
            + 'times the quote the harness served (assertFiatHints)',
    },
    {
        id: 'lobby-row-clock',
        selector: 'tbody tr .clock-value',
        label: /^lobby ".*" clock (action timeout|time bank)$/,
        why: "the lobby card's clock cell, seconds against the table config",
    },
    {
        id: 'lobby-stakes',
        selector: 'tbody tr .c-stakes, tbody tr .stakes-value, tbody tr td:nth-child(2)',
        label: /^lobby ".*" (small|big) blind/,
        why: "the lobby row's stakes cell",
    },
    {
        id: 'lobby-buyin',
        selector: 'tbody tr .c-buyin, tbody tr .buyin-value',
        label: /^lobby ".*" (min|max) buy-in/,
        why: "the lobby row's buy-in range",
    },
    {
        id: 'lobby-row-name',
        selector: 'tbody tr .table-name',
        label: /^lobby row NAME .* quotes a (small|big) blind$/,
        why: 'the blinds baked into the lobby row NAME string',
    },
    {
        id: 'lobby-live-pot',
        selector: 'tbody tr .now-detail',
        label: /^lobby ".*" live pot vs get_pot\(\)$/,
        why: "a live pot rendered on a lobby row",
    },
    {
        id: 'lobby-preview-heading',
        selector: '.preview-heading h3',
        label: /^lobby preview heading .* quotes a (small|big) blind$/,
        why: 'the blinds in the preview pane heading (the lobby NAME string again)',
    },
    {
        id: 'lobby-preview-felt-pot',
        selector: '.mini-felt .felt-pot',
        label: /^lobby preview live pot vs get_pot\(\)$/,
        why: "the live pot on the preview pane's mini-felt",
    },
    {
        id: 'lobby-preview-seated-stack',
        selector: '.seated .seated-stack',
        label: /^lobby preview seat \d+ stack$/,
        why: 'each seated player’s stack in the preview pane',
    },
    {
        id: 'lobby-preview-fact',
        selector: '.facts dd',
        label: /^lobby preview fact "/,
        why: 'the preview facts list: blinds, buy-in range, ante, clock, hands dealt, last pot',
    },
    {
        id: 'lobby-preview-rake-line',
        selector: '.rake-line',
        label: /^lobby preview rake line last pot$/,
        why: 'the last pot repeated in the no-rake sentence',
    },
    {
        id: 'deposit-wallet-balance',
        selector: '.balance-crypto',
        label: /^deposit modal wallet balance vs ledger/,
        why: 'the wallet balance quoted by the deposit modal',
    },
    {
        id: 'deposit-fiat',
        selector: '.usd-value, .usd-amount',
        label: /^deposit modal fiat value vs/,
        why: 'the fiat conversion of that balance',
    },
    {
        id: 'deposit-cost-summary',
        selector: '.cost-summary dd',
        label: /^deposit modal cost row "/,
        why: 'the cost of the typed deposit, row by row (you send / ledger fees / total from '
            + 'your wallet / the table credits), each recomputed from the field\'s value and the '
            + 'ledger\'s own icrc1_fee(); rendered only once an amount is typed',
    },
    {
        id: 'deposit-button-amount',
        selector: '.modal-content .actions .btn-primary',
        label: /^deposit modal button amount vs/,
        why: 'the amount the Deposit button names, which is the typed amount',
    },
    {
        id: 'solvency-figures',
        selector: '.solvency .figures dd, .solvency .advice, .solvency .exact dd',
        label: /^solvency /,
        why: 'the money a table owes, holds and is short by (the rounded figure on the row, '
            + 'the exact e8s integer once under "What the table said"), and the canister\'s own '
            + 'advice sentence quoting them, against get_solvency() (docs/SECURITY-FINDINGS.md FINDING 35)',
    },
    {
        id: 'deposit-quick-chip',
        selector: '.quick-amounts .quick-amount .chip-figure',
        label: /^deposit modal quick chip "/,
        why: 'the figure on a quick chip\'s face (the table\'s minimum buy-in, twice it), '
            + 'derived from the TABLE canister\'s config.min_buy_in and asserted against '
            + 'get_table_view().config by chain-agreement.mjs; the "2x" factor beside it is '
            + 'the allowlist\'s deposit-quick-multiple',
    },
    {
        id: 'deposit-detected',
        selector: '.deposit-address-section .detected-amount',
        label: /^deposit modal detected at the address vs/,
        why: 'what the address route says has arrived at the derived deposit subaccount '
            + '("Detected 0.0005 ICP"), read from the ledger every few seconds and swept in by '
            + 'itself; asserted against icrc1_balance_of on that subaccount',
    },
    {
        id: 'deposit-minimum-and-fee',
        selector: '.minimum-notice',
        // The third alternative is the wallet requirement T-30 added ("charged twice
        // by the ledger, so you need 0.0004 ICP in your wallet"). The copy grew and
        // this label did not, so the census reported that token as asserted by
        // nothing and BOTH deposit shots were filed UNVERIFIED (docs/DEFECTS.md H-41).
        //
        // There are now FOUR figures in this notice, not three: 134550e split the
        // minimum into the ADDRESS floor and the connected-wallet floor, which are
        // different numbers with different consequences (docs/DEFECTS.md E-86). Both
        // are labelled `deposit modal "Minimum deposit"`, disambiguated in the rest
        // of the label, so this pattern still matches all four.
        label: /^deposit modal "(Minimum deposit|Network fee|above the N network fee|At or below N nothing can move it|you need N in your wallet)"/,
        why: 'the address minimum, the connected-wallet minimum, the network fee, and '
            + 'the wallet balance the modal says is needed to deposit that minimum — '
            + 'all four stated as fact',
    },
    {
        id: 'action-feed-amount',
        selector: '.action-feed .action-amount',
        label: /^feed line \d+ amount vs/,
        why: 'every amount in the live table\'s LOG drawer (ActionFeed.svelte): each blind '
            + 'post, each action\'s amount and each "won" line. The `table-log` scene matches '
            + 'every one of them to the table canister\'s own get_hand_history record for the '
            + 'hand on screen (blinds to get_table_view().config, awards to winners), in '
            + 'order; a line whose amount matches nothing fails the scene. The drawer is '
            + 'closed on every other table scene, so before that scene these figures had '
            + 'never been censused',
    },
    {
        id: 'history-row-pot',
        selector: '.hand-row .pot',
        label: /^history row \d+ pot vs/,
        why: "each hand-history row's pot",
    },
    {
        id: 'replay-equity',
        selector: '.replayer .replay-equity',
        label: /^replay seat \d+ equity/,
        why: 'the equity percentage on a seat pod in the hand REPLAYER, at the stop on '
            + 'screen. Shown only when every live hand at that stop is on its face (a '
            + 'showdown hand; lib/replay-stops.js equityAllowedAt), computed by '
            + 'lib/equity.js, and recomputed by the handreplay scene with the independent '
            + 'oracle (lib/equity-oracle.mjs) from the hand record\'s own cards and board. '
            + 'Never a money figure; listed before the money site because it is inside '
            + 'the same replayer',
    },
    {
        id: 'hand-replay-money',
        selector: '.replayer .replay-money',
        // Every replay figure EXCEPT the equity ones, which the site above owns.
        label: /^replay (?!seat \d+ equity)/,
        why: 'every amount the hand REPLAYER renders: each blind post, each action\'s '
            + 'amount, the log\'s own sum-vs-pot audit, the final pot, each winner\'s award '
            + 'and each showdown player\'s result. The `handreplay` scene compares every one '
            + "of them with the table canister's own get_hand_history record for that hand "
            + '(and the blind level with get_table_view().config), one figure per element, in '
            + 'DOM order. Added with that scene: until it existed the replayer had never been '
            + 'opened by the harness, so not one of these numbers had ever been looked at '
            + '(docs/DEFECTS.md H-31).',
    },
];

/**
 * Everything the census knows how to attribute.
 *
 * A token is tried against its CHAIN site first and falls back to the allowlist
 * only if no unused check covers it. The order is deliberate and it is the
 * conservative one: an allowlist rule can never shadow a check that would have
 * failed, because the check is consulted first; it can only excuse a leftover
 * token in the same element (a seat count inside a "6-Max · 0.05/0.10" pill, the
 * "+30s" on a button next to the Call amount).
 */
function ruleTable() {
    // THE CENSUS MUST NOT BE ABLE TO EXCUSE A TOKEN BY ACCIDENT. The first draft
    // of this file dropped `kind` on the way into the page, so every site was
    // treated as an allowlist rule, and `new RegExp(undefined)` is `/(?:)/` —
    // which matches every string. The result was a run reporting "74 tokens, 74
    // allowlisted, 0 unaccounted for": a green gate that checked nothing, the
    // exact failure this module exists to remove. Malformed rules now throw.
    for (const r of ALLOWLIST) {
        if (!r.id || !r.selector || typeof r.tokens !== 'string' || !r.why) {
            throw new Error(
                `token-allowlist.mjs: rule ${JSON.stringify(r.id ?? r)} is missing id, selector, `
                + 'tokens or why. An allowlist rule without an explicit token pattern would '
                + 'excuse every number in its element.',
            );
        }
        // THE SHAPE INVARIANT. Every ICP figure this client renders goes through
        // a `toFixed(2)` or `toFixed(4)` (and a BTC one through `toFixed(1)` +
        // K/M), so a money figure ALWAYS carries a decimal point. An allowlist
        // rule that accepts a decimal token could therefore excuse an amount, and
        // must say so out loud with `moneyShaped: true`. Everything else is
        // mechanically incapable of hiding money.
        const moneyProbes = ['0.30', '12.00', '155.00', '0.0001', '1.5', '1,000.00'];
        const hits = moneyProbes.filter((p) => new RegExp(r.tokens).test(p));
        if (hits.length && !r.moneyShaped) {
            throw new Error(
                `token-allowlist.mjs: rule "${r.id}" accepts money-shaped token(s) `
                + `${hits.join(', ')}. Every amount in this client renders with a decimal point, `
                + 'so this rule could excuse a real figure. Tighten the pattern, or set '
                + 'moneyShaped: true and justify it.',
            );
        }
    }
    for (const r of CHAIN_SITES) {
        if (!r.id || !r.selector || !(r.label instanceof RegExp)) {
            throw new Error(`token-census.mjs: CHAIN_SITES entry ${JSON.stringify(r.id ?? r)} needs id, selector and a RegExp label`);
        }
    }
    return [
        ...CHAIN_SITES.map((r) => ({ ...r, kind: 'chain' })),
        ...ALLOWLIST.map((r) => ({ ...r, kind: 'allow' })),
    ];
}

/**
 * Every numeric-looking token the page is currently rendering.
 *
 * Text nodes plus the VALUE and PLACEHOLDER of visible inputs, because a number
 * typed into (or suggested by) a deposit field is a number on the screen. Hidden
 * subtrees are collected but flagged, and only visible tokens are gated: a scene
 * cannot be failed by a figure nobody can see.
 *
 * @param {import('playwright').Page} page
 * @param {Array<{id:string, selector:string}>} rules
 */
function collectTokens(page, rules) {
    return page.evaluate((ruleList) => {
        // A token ends on a digit, so "Published at 8/5/2026, 3:15" yields
        // "2026" and not "2026," — a trailing separator would make the same
        // number unrecognisable to the matcher.
        const NUM = /\d+(?:[.,]\d+)*/g;
        const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'TITLE']);

        const visibleOf = (el) => {
            if (!el || !el.isConnected) return false;
            if (typeof el.checkVisibility === 'function') {
                return el.checkVisibility({
                    visibilityProperty: true, opacityProperty: true, contentVisibilityAuto: true,
                });
            }
            const rect = el.getBoundingClientRect();
            return rect.width > 0 && rect.height > 0;
        };

        const describe = (el) => {
            const parts = [];
            let node = el;
            for (let depth = 0; node && depth < 4 && node !== document.body; depth += 1) {
                const cls = [...node.classList].slice(0, 3).map((c) => `.${c}`).join('');
                parts.unshift(`${node.tagName.toLowerCase()}${cls}`);
                node = node.parentElement;
            }
            return parts.join(' > ');
        };

        const rulesFor = (el) => {
            let chain = null;
            const allow = [];
            for (const r of ruleList) {
                let hit = false;
                try {
                    hit = Boolean(el.closest(r.selector));
                } catch { /* an invalid selector is a harness bug: reported as unattributed */ }
                if (!hit) continue;
                if (r.kind === 'chain') { if (chain === null) chain = r.id; }
                else allow.push(r.id);
            }
            return { chain, allow };
        };

        const out = [];
        const push = (el, text, source) => {
            const matches = String(text).match(NUM);
            if (!matches) return;
            const visible = visibleOf(el);
            const rect = el.getBoundingClientRect();
            const style = window.getComputedStyle(el);
            const rules = rulesFor(el);
            const path = describe(el);
            const elementText = (el.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 160);
            for (const token of matches) {
                out.push({
                    token,
                    source,
                    visible,
                    chainRuleId: rules.chain,
                    allowRuleIds: rules.allow,
                    path,
                    elementText,
                    context: String(text).replace(/\s+/g, ' ').trim().slice(0, 160),
                    fontSizePx: Math.round(parseFloat(style.fontSize) || 0),
                    rect: { x: Math.round(rect.x), y: Math.round(rect.y), w: Math.round(rect.width), h: Math.round(rect.height) },
                });
            }
        };

        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
            const el = node.parentElement;
            if (!el || SKIP_TAGS.has(el.tagName)) continue;
            const text = node.nodeValue || '';
            if (!/\d/.test(text)) continue;
            push(el, text, 'text');
        }

        for (const input of document.querySelectorAll('input, textarea')) {
            if (input.type === 'range' || input.type === 'hidden') continue;
            if (input.value) push(input, input.value, 'input-value');
            else if (input.placeholder) push(input, input.placeholder, 'input-placeholder');
        }

        return out;
    }, rules.map((r) => ({ id: r.id, selector: r.selector, kind: r.kind })));
}

/** Numeric value of a token as written ("1,000" -> 1000). */
const valueOf = (token) => Number(String(token).replace(/,/g, ''));

/** Every numeric literal inside a figure's on-screen text. */
function figureValues(screen) {
    const matches = String(screen ?? '').match(/\d+(?:[.,]\d+)*/g) || [];
    return matches.map(valueOf);
}

/**
 * How many tokens one check may legitimately account for.
 *
 * A check whose on-screen text is a COMPOSITE asserts every number in it: the
 * pot-odds ratio "3.5:1" is compared as a ratio, the pot breakdown
 * "0.10 + 0.20" is compared as a sum, a commitment hash is compared whole. Such
 * a check covers each of its own literals — but no more, so it still cannot be
 * stretched over a number it never saw.
 */
const capacityOf = (screen) => Math.max(1, figureValues(screen).length);

/**
 * Maximum bipartite matching (Kuhn's algorithm) between tokens and figures.
 *
 * One-to-one on purpose. Three seats showing the same stack need three distinct
 * passing checks behind them, or the third is unasserted.
 *
 * @param {number[][]} adjacency adjacency[t] = indices of figures token t may use
 * @param {number} figureCount
 * @returns {number[]} matchOf[t] = figure index, or -1
 */
function matchTokens(adjacency, figureCount) {
    const figureToToken = new Array(figureCount).fill(-1);
    const tryAssign = (t, seen) => {
        for (const f of adjacency[t]) {
            if (seen.has(f)) continue;
            seen.add(f);
            if (figureToToken[f] === -1 || tryAssign(figureToToken[f], seen)) {
                figureToToken[f] = t;
                return true;
            }
        }
        return false;
    };
    for (let t = 0; t < adjacency.length; t += 1) tryAssign(t, new Set());
    const matchOf = new Array(adjacency.length).fill(-1);
    figureToToken.forEach((t, f) => { if (t !== -1) matchOf[t] = f; });
    return matchOf;
}

/**
 * THE GATE. Enumerates the page's numbers and demands an account of each.
 *
 * @param {import('playwright').Page} page
 * @param {Array<{label:string, chain:number|null, screen:string|null, ok:boolean, detail:string}>} figures
 *        every figure the scene's chain-agreement layer compared this run
 * @param {{scene:string, viewport:string}} meta
 * @returns {Promise<{ok:boolean, checks:object, notes:string}>}
 */
export async function assertEveryTokenAccountedFor(page, figures, meta = {}) {
    const rules = ruleTable();
    const byId = new Map(rules.map((r) => [r.id, r]));
    const raw = await collectTokens(page, rules);

    const visible = raw.filter((t) => t.visible);
    const hidden = raw.filter((t) => !t.visible);

    const classified = visible.map((t) => ({ ...t, verdict: 'pending', site: t.chainRuleId }));

    // ---- chain-matched ----------------------------------------------------
    // Per site, pair the tokens rendered there with the checks that ran there,
    // one-to-one. Three stacks showing 12.00 need three distinct checks.
    const perSite = new Map();
    for (const t of classified) {
        if (!t.chainRuleId) continue;
        if (!perSite.has(t.chainRuleId)) perSite.set(t.chainRuleId, []);
        perSite.get(t.chainRuleId).push(t);
    }

    const siteReport = [];
    for (const [ruleId, tokens] of perSite) {
        const rule = byId.get(ruleId);
        const checks = figures.filter((f) => rule.label.test(f.label));
        // One slot per literal a check quotes: a check may cover its own
        // composite, and nothing else.
        const candidates = [];
        for (const f of checks) {
            for (let n = 0; n < capacityOf(f.screen); n += 1) candidates.push(f);
        }
        const values = candidates.map((f) => figureValues(f.screen));
        const adjacency = tokens.map((t) =>
            candidates.map((_, i) => i).filter((i) => values[i].some((v) => v === valueOf(t.token))));
        const matched = matchTokens(adjacency, candidates.length);
        tokens.forEach((t, i) => {
            const f = matched[i] === -1 ? null : candidates[matched[i]];
            if (!f) {
                // Left for the allowlist to explain, and UNASSERTED if it cannot.
                t.unmatchedReason = checks.length === 0
                    ? `site "${ruleId}" (${rule.why}) is declared chain-asserted, but NO check `
                      + `matching /${rule.label.source}/ ran on this scene`
                    : `site "${ruleId}" rendered ${tokens.length} number(s) but the `
                      + `${checks.length} check(s) that ran there cover ${candidates.length} of `
                      + `them, and no unused check quotes "${t.token}"`;
                return;
            }
            t.verdict = f.ok ? 'chain-matched' : 'chain-DISAGREES';
            t.figureLabel = f.label;
            t.chain = f.chain;
            t.why = f.detail;
        });
        siteReport.push({
            site: ruleId,
            tokens: tokens.length,
            checksAvailable: candidates.length,
            unmatched: tokens.filter((t) => t.verdict === 'pending').length,
        });
    }

    // ---- allowlisted ------------------------------------------------------
    // A token is excused only when a rule's SELECTOR, its TOKEN PATTERN and its
    // optional CONTEXT pattern all match. A rule cannot excuse a number it was
    // not written for.
    for (const t of classified) {
        if (t.verdict !== 'pending') continue;
        for (const id of t.allowRuleIds) {
            const rule = byId.get(id);
            if (!rule || rule.kind !== 'allow' || typeof rule.tokens !== 'string') continue;
            if (!new RegExp(rule.tokens).test(t.token)) continue;
            if (rule.context && !new RegExp(rule.context).test(t.elementText)) continue;
            if (rule.source && rule.source !== t.source) continue;
            t.verdict = 'allowlisted';
            t.site = id;
            t.why = rule.why;
            break;
        }
    }

    // ---- anything nothing accounts for ------------------------------------
    for (const t of classified) {
        if (t.verdict !== 'pending') continue;
        t.verdict = 'UNASSERTED';
        t.why = t.unmatchedReason
            || (t.allowRuleIds.length
                ? `allowlist rule(s) ${t.allowRuleIds.join(', ')} match this element but not the `
                  + `token "${t.token}" (or not its context), so this number is excused by nothing`
                : 'no rule in CHAIN_SITES or the allowlist attributes this element, so nothing '
                  + 'in the harness has ever looked at this number');
    }

    const counts = classified.reduce((acc, t) => {
        acc[t.verdict] = (acc[t.verdict] || 0) + 1;
        return acc;
    }, {});
    const unasserted = classified.filter((t) => t.verdict === 'UNASSERTED');

    // Allowlist usage, so a rule that quietly starts excusing 40 tokens is visible.
    const allowlistUsage = ALLOWLIST.map((r) => ({
        rule: r.id,
        selector: r.selector,
        tokens: r.tokens,
        context: r.context ?? null,
        moneyShaped: Boolean(r.moneyShaped),
        why: r.why,
        excusedThisScene: classified.filter((t) => t.verdict === 'allowlisted' && t.site === r.id).length,
    }));

    const reportOnly = process.env.SHOTS_CENSUS === 'report';
    const ok = reportOnly || unasserted.length === 0;

    const summary = {
        mode: reportOnly ? 'REPORT ONLY (SHOTS_CENSUS=report) — not gating' : 'gating',
        scene: meta.scene ?? null,
        viewport: meta.viewport ?? null,
        totalTokensOnScreen: visible.length,
        chainMatched: counts['chain-matched'] || 0,
        chainDisagrees: counts['chain-DISAGREES'] || 0,
        allowlisted: counts.allowlisted || 0,
        unasserted: unasserted.length,
        hiddenTokensNotGated: hidden.length,
        chainAgreementFiguresAvailable: figures.length,
        allowlistRuleCount: ALLOWLIST.length,
        allowlist: allowlistUsage,
        perSite: siteReport,
        unassertedTokens: unasserted.map((t) => ({
            token: t.token,
            path: t.path,
            context: t.context,
            fontSizePx: t.fontSizePx,
            at: t.rect,
            why: t.why,
        })),
        // The whole enumeration, so a reader can redo the arithmetic from the
        // artifact instead of trusting the summary.
        tokens: classified.map((t) => ({
            token: t.token,
            verdict: t.verdict,
            site: t.site,
            path: t.path,
            figure: t.figureLabel ?? null,
        })),
    };

    const notes = unasserted.length === 0
        ? `token census: ${visible.length} numeric tokens on screen, `
          + `${summary.chainMatched} matched to a canister figure, `
          + `${summary.allowlisted} allowlisted non-monetary (${ALLOWLIST.length} rules), 0 unaccounted for`
        // In report-only mode the scene is NOT failed, so the note has to say so:
        // a line reading "TOKEN CENSUS FAILED" beside a green verdict is exactly
        // the kind of quiet contradiction this module exists to remove.
        : `${reportOnly ? 'TOKEN CENSUS (REPORT ONLY, NOT GATING — SHOTS_CENSUS=report): ' : 'TOKEN CENSUS FAILED: '}`
          + `${unasserted.length} of ${visible.length} numeric tokens on screen are `
          + `asserted by nothing — ${unasserted.slice(0, 4).map((t) => `"${t.token}" in ${t.path}`).join(' | ')}`;

    return { ok, checks: summary, notes };
}
