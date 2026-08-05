// Reads every money figure and every card OFF THE RENDERED PAGE.
//
// This file is deliberately the ONLY place that knows a CSS selector for another
// agent's component. Three agents are redesigning PokerTable / Lobby /
// HandHistory / ShuffleProof while this harness asserts on their output, so the
// contract between us is here, in one list, and a selector that stops matching
// is reported as a FAILED SCRAPE rather than as "nothing to check".
//
// That distinction is the whole point. A verifier that silently checks zero
// figures when a class name changes is exactly the failure mode wave 2 caught:
// a scene recording `verified: true` while quoting a wrong number in its notes.

/**
 * @typedef {object} SeatDom
 * @property {number} index         DOM order, which is the seat index
 * @property {boolean} occupied
 * @property {string|null} name
 * @property {string|null} chipsText
 * @property {string|null} betText
 * @property {boolean} isMe
 * @property {boolean} allIn
 * @property {boolean} folded
 * @property {boolean} dealerBadge
 * @property {boolean} sbBadge
 * @property {boolean} bbBadge
 * @property {number} cardCount
 * @property {Array<{faceDown:boolean, empty:boolean, rank:string|null, suit:string|null}>} cards
 */

/**
 * Everything the table page is currently claiming, as plain JSON.
 *
 * Runs entirely inside the page, so it is a snapshot of one frame.
 *
 * @param {import('playwright').Page} page
 * @returns {Promise<object>}
 */
export function scrapeTable(page) {
    return page.evaluate(() => {
        const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
        const one = (root, sel) => txt(root.querySelector(sel));

        // CARD FACE CONTRACT. Card.svelte has already been rewritten once during
        // this wave: `.corner.top-left .corner-rank` / `.corner-suit` became
        // `.rank` / `.pip`. Both spellings are accepted, and when a card is
        // plainly FACE UP (`.card-front` present) yet no known selector yields a
        // rank, the element's own markup is captured so the failure names itself
        // instead of being reported as "the client rendered a blank card".
        const readCard = (el) => {
            const faceDown = !!el.querySelector('.card-back');
            const front = !!el.querySelector('.card-front');
            const empty = !!el.querySelector('.card-empty') || el.classList.contains('empty');
            const rank = one(el, '.rank')
                || one(el, '.corner.top-left .corner-rank') || one(el, '.corner-rank');
            const suit = one(el, '.pip')
                || one(el, '.corner.top-left .corner-suit') || one(el, '.corner-suit');
            const unreadable = front && !faceDown && !empty && (!rank || !suit);
            return {
                faceDown,
                empty,
                front,
                rank,
                suit,
                unreadable,
                markup: unreadable ? el.innerHTML.replace(/\s+/g, ' ').trim().slice(0, 240) : undefined,
            };
        };

        // SEAT ROOT. `.poker-table`, not `.felt`. In the wave-3 redesign `.felt`
        // became an empty decorative div and the seat ring moved out to be its
        // sibling, which made a `.felt .seat` query return nothing — reported
        // loudly as SCRAPE FAILED rather than as "no seats to check". The root has
        // to be the outermost thing that only ever exists on the table page.
        const root = document.querySelector('.poker-table') || document.querySelector('.felt');
        const seatEls = root ? [...root.querySelectorAll('.seat')] : [];

        const seats = seatEls.map((el, index) => {
            const plate = el.querySelector('.player-nameplate');
            return {
                index,
                occupied: el.classList.contains('occupied') || !!plate,
                name: one(el, '.player-name'),
                chipsText: one(el, '.chips'),
                betText: one(el, '.bet-amount'),
                isMe: !!el.querySelector('.player-nameplate.highlight-me'),
                allIn: !!el.querySelector('.avatar-overlay.allin'),
                folded: !!el.querySelector('.avatar-overlay.folded'),
                dealerBadge: !!el.querySelector('.position-badge.dealer'),
                sbBadge: !!el.querySelector('.position-badge.sb'),
                bbBadge: !!el.querySelector('.position-badge.bb'),
                cards: [...el.querySelectorAll('.player-cards .card')].map(readCard),
            };
        });

        const sidePots = [...document.querySelectorAll('.side-pot')].map((el) => ({
            label: one(el, '.side-pot-label'),
            amountText: one(el, '.side-pot-amount'),
        }));

        const board = [...document.querySelectorAll('.community-cards .card')].map(readCard);

        return {
            found: {
                felt: !!root,
                seats: seatEls.length,
                potDisplay: !!document.querySelector('.pot-display'),
                potAmount: !!document.querySelector('.pot-amount'),
                winnerDisplay: !!document.querySelector('.winner-display'),
                communityCards: board.length,
                walletPanel: !!document.querySelector('.wallet-panel, .wallet-collapsed-info'),
            },
            potAmountText: one(document, '.main-pot .pot-amount') ?? one(document, '.pot-amount'),
            potBreakdownText: one(document, '.pot-breakdown'),
            sidePots,
            board,
            seats,
            phaseText: one(document, '.phase-indicator'),
            winnerText: one(document, '.winner-display .winner-text') ?? one(document, '.winner-display'),
            winnerHandRank: one(document, '.winner-display .winner-hand-rank'),
            splitInfo: one(document, '.winner-display .split-info'),
            tableBalanceText:
                one(document, '.wallet-panel .balance-value')
                ?? one(document, '.wallet-collapsed-info .collapsed-balance'),
            potOddsText: one(document, '.pot-odds-explanation'),
            potOddsValue: one(document, '.pot-odds-value'),
            // THE HEADER STAKES PILL IS A MONEY FIGURE.
            // `.current-table-name` sits in the table header on every table
            // scene and quotes the blinds. Nothing scraped it, so for the whole
            // of wave 3 it read "6-Max - 0.01/0.02" above a felt whose blinds
            // were 0.10, and 18 scenes were filed "agrees with chain: yes"
            // around it. It is now read here and asserted against the table's
            // own config, like every other number on the screen.
            headerStakesText: one(document, '.current-table-name'),
            equityHint: one(document, '.equity-hint'),
            potOddsPresent: !!document.querySelector('.pot-odds-display'),
            turnHint: one(document, '.turn-hint'),
            actionButtons: [...document.querySelectorAll('.actions .action-btn')]
                .map((b) => (b.textContent || '').replace(/\s+/g, ' ').trim())
                .filter(Boolean),
            // The bet-sizing popover. `.slider-amount` and the confirm button are
            // the two places the client shows what it is ABOUT TO WAGER, so they
            // are money figures even though the amount is client-side state.
            raiseSliderPresent: !!document.querySelector('.raise-slider-panel'),
            raiseSliderAmountText: one(document, '.raise-slider-panel .slider-amount'),
            raiseConfirmText: one(document, '.raise-slider-panel .confirm-raise'),
            raiseSliderRange: (() => {
                const el = document.querySelector('.raise-slider-panel .raise-slider');
                return el ? { min: el.min, max: el.max, value: el.value } : null;
            })(),
            presetButtons: [...document.querySelectorAll('.raise-slider-panel .preset-buttons button')]
                .map((b) => (b.textContent || '').replace(/\s+/g, ' ').trim()),
        };
    });
}

/**
 * Clicks a bet-sizing preset and reports the amount the client then proposes.
 *
 * WHY THIS IS IN THE HARNESS AT ALL. A preset is not a display: "Pot" writes a
 * number into the raise field that the very next click SENDS TO THE CANISTER as
 * real money. A pot figure that is wrong on the felt is a wrong number the
 * player reads; the same wrong figure behind the preset is a wrong number the
 * player WAGERS. It is read here and asserted in chain-agreement.mjs, and
 * nothing is committed — the popover is closed again afterwards.
 *
 * @param {import('playwright').Page} page
 * @param {string} label button text, e.g. 'Pot' or '½ Pot'
 * @returns {Promise<{available:boolean, amountText:string|null, sliderValue:string|null}>}
 */
export async function readBetPreset(page, label) {
    const opened = await page.evaluate(() => {
        const already = !!document.querySelector('.raise-slider-panel');
        if (already) return true;
        const btn = [...document.querySelectorAll('.actions .action-btn')]
            .find((b) => /raise|bet/i.test(b.textContent || ''));
        if (!btn) return false;
        btn.click();
        return true;
    });
    if (!opened) return { available: false, amountText: null, sliderValue: null };
    await page.waitForSelector('.raise-slider-panel', { timeout: 5_000 }).catch(() => {});

    const clicked = await page.evaluate((wanted) => {
        const norm = (s) => (s || '').replace(/\s+/g, ' ').replace(/½/g, '1/2').trim().toLowerCase();
        const btn = [...document.querySelectorAll('.raise-slider-panel .preset-buttons button')]
            .find((b) => norm(b.textContent) === norm(wanted));
        if (!btn) return false;
        btn.click();
        return true;
    }, label);
    if (!clicked) return { available: false, amountText: null, sliderValue: null };

    // SVELTE 5 FLUSHES ON A MICROTASK AND PAINTS ON THE NEXT FRAME. Reading the
    // DOM in the same evaluate() as the click returns the PREVIOUS value, which
    // silently compares the wrong number. Two rAFs is "the click has been applied
    // and presented".
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));

    return page.evaluate(() => {
        const amt = document.querySelector('.raise-slider-panel .slider-amount');
        const slider = document.querySelector('.raise-slider-panel .raise-slider');
        const confirm = document.querySelector('.raise-slider-panel .confirm-raise');
        return {
            available: true,
            amountText: amt ? (amt.textContent || '').replace(/\s+/g, ' ').trim() : null,
            confirmText: confirm ? (confirm.textContent || '').replace(/\s+/g, ' ').trim() : null,
            sliderValue: slider ? slider.value : null,
        };
    });
}

/** Closes the bet-sizing popover so the scene photographs its normal state. */
export async function closeBetPresets(page) {
    await page.evaluate(() => {
        document.querySelector('.raise-slider-panel .close-slider')?.click();
    });
}

/**
 * The lobby table list, as rendered.
 *
 * READ BY COLUMN HEADER, NOT BY CLASS NAME. The lobby is being redesigned while
 * this harness runs: within one hour the stakes cell went from
 * `.stakes-value` to `.c-stakes .num`. A scraper pinned to either one silently
 * checks nothing after the other lands, and "nothing checked" is the exact
 * failure this whole change exists to remove. The `<thead>` labels ("Stakes",
 * "Buy-in", "Seats") are the stable contract — they are what the user reads too —
 * so cells are located by matching the header text, with the legacy class names
 * kept only as a fallback for a layout that has no header row.
 */
export function scrapeLobby(page) {
    return page.evaluate(() => {
        const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);

        const table = document.querySelector('table');
        const headers = table
            ? [...table.querySelectorAll('thead th')].map((th) => (txt(th) || '').replace(/[^A-Za-z- ]/g, '').trim())
            : [];

        const findCol = (re) => headers.findIndex((h) => re.test(h));
        const col = {
            name: findCol(/table|game/i),
            stakes: findCol(/stake|blind/i),
            buyIn: findCol(/buy.?in/i),
            seats: findCol(/seat|player/i),
        };

        const rows = [...document.querySelectorAll('tbody tr')].map((tr) => {
            const cells = [...tr.children];
            const cellText = (i) => (i >= 0 && cells[i] ? txt(cells[i]) : null);
            // Inside a cell, the numeric span is what we want, not the unit label.
            const numeric = (i) => {
                if (i < 0 || !cells[i]) return null;
                const n = cells[i].querySelector('.num, .stakes-value, .buyin-value, .players-text, .seat-count');
                return txt(n) ?? cellText(i);
            };
            return {
                name: txt(tr.querySelector('.table-name')) ?? cellText(col.name),
                stakesText: numeric(col.stakes) ?? txt(tr.querySelector('.stakes-value')),
                buyInText: numeric(col.buyIn) ?? txt(tr.querySelector('.buyin-value')),
                playersText: numeric(col.seats) ?? txt(tr.querySelector('.players-text')),
                // The redesigned lobby renders a LIVE pot per row ("pot 0.20 ICP").
                nowLabel: txt(tr.querySelector('.now')),
                livePotText: txt(tr.querySelector('.now-detail')),
                currencyTag: txt(tr.querySelector('.currency-tag, .tag.currency')),
            };
        });

        return { rows, rowCount: rows.length, headers, columnMap: col };
    });
}

/** The deposit modal's money figures. */
export function scrapeDeposit(page) {
    return page.evaluate(() => {
        const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
        const cryptoBalances = [...document.querySelectorAll('.balance-crypto')].map((e) => txt(e));
        const usdValues = [...document.querySelectorAll('.usd-value')].map((e) => txt(e));
        return {
            found: {
                modal: !!document.querySelector('.modal-content'),
                balanceCrypto: cryptoBalances.length,
                usdValue: usdValues.length,
            },
            title: txt(document.querySelector('#deposit-modal-title')),
            cryptoBalances,
            usdValues,
            priceError: txt(document.querySelector('.price-error')),
            sourceButtons: [...document.querySelectorAll('.wallet-source-toggle button')]
                .map((b) => txt(b)).filter(Boolean),
        };
    });
}

/** The hand-history modal's money figures. */
export function scrapeHandHistory(page) {
    return page.evaluate(() => {
        const txt = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
        const rows = [...document.querySelectorAll('.hand-row')].map((el) => ({
            all: txt(el),
            potText: txt(el.querySelector('.pot')),
        }));
        return {
            found: { modal: !!document.querySelector('.hand-history-modal'), rows: rows.length },
            rows,
            detailPotText: txt(document.querySelector('.hand-detail .amount, .amount')),
            winAmounts: [...document.querySelectorAll('.win-amount')].map((e) => txt(e)),
        };
    });
}
