// THE REVIEWED NON-MONETARY ALLOWLIST.
//
// Every numeric token the client renders must be either matched to a canister
// figure or declared here (tools/shots/lib/token-census.mjs enforces it). This
// file is the ONLY escape hatch, which makes it the only place the inverted gate
// can be defeated, so five rules apply to it:
//
//   1. THE SHAPE INVARIANT. Every amount this client renders goes through a
//      `toFixed(2)` / `toFixed(4)` (or `toFixed(1)` + K/M), so a money figure
//      ALWAYS carries a decimal point. No rule below accepts a decimal token
//      unless it is marked `moneyShaped: true` — and the census REFUSES TO RUN if
//      one does without saying so. That is a mechanical guarantee, not a promise:
//      a pot, a stack, a bet, a blind or a buy-in cannot be excused by this file.
//   2. A rule is scoped to a CSS SELECTOR **and** a TOKEN PATTERN, and where the
//      element can hold both kinds of number, a CONTEXT pattern as well.
//   3. The chain checks are consulted FIRST. A rule here can only ever excuse a
//      token no check claimed; it can never shadow a check that would have failed.
//   4. Every rule states WHY the number is not money, in terms a reader can check
//      against the component.
//   5. Every rule was written against an OBSERVED token. Nothing is speculative:
//      each one names a number a real scene really rendered. The list length and
//      the tokens each rule excused are printed into every run's manifest.
//
// If a money figure ever needs an entry here, the correct change is an assertion
// in chain-agreement.mjs and a site in CHAIN_SITES — not a rule in this file.

/**
 * @typedef {object} AllowRule
 * @property {string} id       stable name, printed in the manifest
 * @property {string} selector matched with `element.closest(selector)`
 * @property {string} tokens   regex source; the token must match to be excused
 * @property {string} [context] regex source; the element's whole text must match too
 * @property {boolean} [moneyShaped] the pattern accepts a decimal token, and that is deliberate
 * @property {string} why      why this number is not a money figure
 */

/** @type {AllowRule[]} */
export const ALLOWLIST = [
    // ---- cards -------------------------------------------------------------
    {
        id: 'card-rank-glyph',
        selector: '.card .rank, .card .rank-mirror, .card .corner-rank, .mini-card .mc-rank, '
            + '.board-cell, .mini-board',
        tokens: '^(?:2|3|4|5|6|7|8|9|10)$',
        why: 'a playing-card rank glyph, not an amount. The felt board is asserted '
            + 'card-by-card against get_community_cards() as rank+suit. `.rank-mirror` is '
            + 'the same glyph repeated in the card\'s rotated bottom-right index '
            + '(Card.svelte, aria-hidden); `.rank` is the one the scraper reads',
    },

    // ---- clocks ------------------------------------------------------------
    {
        id: 'action-countdown',
        selector: '.turn-timer, .timer-value, .seat .timer, .turn-indicator',
        tokens: '^\\d{1,3}$',
        why: 'the action countdown in seconds. Wall-clock, not money; the harness '
            + 'stabilises it so stills are deterministic',
    },
    {
        id: 'deposit-quick-multiple',
        selector: '.quick-amounts .quick-amount',
        tokens: '^2$',
        why: 'the cashier\'s "2x" quick chip (DepositModal.svelte): the FACTOR of the '
            + 'table\'s minimum buy-in it fills the field with, never a money figure. The '
            + 'figure it produces lands in the amount field, which is read like any typed '
            + 'amount; the label is a factor',
    },
    {
        id: 'bet-preset-multiples',
        selector: '.raise-slider-panel .preset-buttons button',
        tokens: '^(2\\.5|3|4)$',
        why: 'the pre-flop preset labels "2.5x", "3x", "4x" (BetSizer.svelte): the '
            + 'MULTIPLE of the bet in front a preset proposes, never a money figure. The '
            + 'figure each one produces is asserted by assertBetPresetsAgree when the '
            + 'preset is clicked; the label itself is a factor',
    },
    {
        id: 'sizer-precision-cue',
        selector: '.raise-slider-panel .sizer-note',
        tokens: '^0\\.0*1$',
        moneyShaped: true,
        context: '^sizes in 0\\.0*1$',
        why: 'the typed field\'s cue "sizes in 0.01" (BetSizer.svelte) when a third '
            + 'decimal was dropped: the DISPLAY UNIT the table sizes in (10^-decimals), '
            + 'never an amount. Decimal-shaped by nature, so marked; it appears only '
            + 'while the player is typing past the grid, never in a resting still',
    },
    {
        id: 'keyboard-hint-digits',
        selector: '.key-hints',
        tokens: '^[1-6]$',
        why: 'the hotkey legend under the action row on pointer devices '
            + '(ActionBar.svelte): "1-5 sizes" (post-flop) or "1-6 sizes" (the pre-flop '
            + 'row has six presets) names the number keys that pick a bet preset. Key '
            + 'caps, never chips; a single digit 1 to 6 and nothing else',
    },
    {
        id: 'time-bank-button',
        selector: '.time-bank-pill',
        tokens: '^\\d{1,3}$',
        context: '^(?:Time bank\\s*)?\\+\\s*\\d+\\s*s$',
        why: 'the "+30s" time-bank pill (TimeBankPill.svelte): seconds added to the '
            + 'clock, never chips. It stands under the turn indicator on desktop and on '
            + 'the hero pod clock on the phone, never in the action row; the context '
            + 'pattern means this rule excuses nothing but "[Time bank] +NNs"',
    },
    {
        id: 'wall-clock-timestamp',
        selector: '.hand-time, .timestamp, .rung-note, .proof-time, .published-at',
        tokens: '^\\d{1,4}$',
        why: 'a date or clock time ("8/5/2026, 3:15:12 AM") saying when a hand was '
            + 'played or a commitment published',
    },

    // ---- ordinals and counts ------------------------------------------------
    {
        id: 'seat-ordinal',
        selector: '.sit-seat, .join-seat, .seat-label, .position-badge, .empty-seat, '
            + '.winner-display, .seated-name, .side-pot-label',
        tokens: '^\\d{1,2}$',
        why: 'a seat or pot ordinal ("Seat 3", "Side 1"). Seat mapping is asserted '
            + "structurally in mappingChecks(); each side pot's AMOUNT is a figure",
    },
    {
        id: 'occupancy-count',
        selector: '.players-text, .seat-count, .seat-note, .c-seats, .bar-sub, .notice, '
            + '.pot-label, .split-info, .seats, .chip, .layout-note, .tally, .state-block, '
            + '.pane-sub, .drift-strip, .row-badges .badge, .retention',
        tokens: '^\\d{1,3}$',
        why: 'a count of players, seats, rows, hands or cards ("2 at risk", "2 seats", '
            + '"5 of 17 seats taken", "My hands (1)", "7 of 7 cards", "7 cards re-derived '
            + 'here", "2 of 3 lobby records quote figures the table contracts do not '
            + 'charge", "keeps only its last 100 hands"). Counts, not amounts: '
            + 'player counts are reported against get_player_count(), the config drift is a '
            + 'structural failure in assertLobbyAgreement, and the card tally is the '
            + "fairness scene's own verdict, asserted there. `.retention` is the fairness "
            + "panel's durability block: its only number is the table's hand-retention cap, "
            + 'read live from get_fairness_retention() on the same canister the rest of the '
            + 'panel is asserted against, and the decimal-point shape invariant above still '
            + 'refuses any money figure that appears there',
    },
    {
        id: 'hands-dealt-count',
        selector: '.hands-value, .c-hands, .hand-number, .hand-counter, .hand-id, .live-note, '
            + '.gap-note',
        tokens: '^#?\\d{1,6}$',
        why: "the canister's hand_number — an ordinal, not an amount. The preview pane's "
            + 'copy of it IS compared with hand_number as a figure',
    },
    {
        id: 'table-shape',
        selector: '.current-table-name, .table-name, .preview-sub, .preview-heading h3, .tag',
        tokens: '^(?:2|6|9)$',
        why: 'the seat count in a table SHAPE label ("6-Max", "9-max"). The blinds in the '
            + "same string are asserted against the table canister's config",
    },
    {
        id: 'display-name-digits',
        selector: '.display-name, .player-name, .wallet-btn',
        tokens: '^\\d{1,4}$',
        why: 'digits inside a generated display name ("ShadowDragon71"). A label',
    },

    // ---- fixed copy ---------------------------------------------------------
    {
        id: 'protected-disclaimer-copy',
        // `.player-notice` and `.modal-notices` are the SAME protected copy restated
        // inside a dialog, which wave 5 had to do because a 72%-black scrim hides
        // the banner behind it (docs/DEFECTS.md T-31, H-36, T-36). Without them the
        // deposit and withdraw scenes fail the census on the "18" of "18+ only" —
        // a player-protection notice being read as an unasserted money figure.
        selector: '.alpha-warning-banner, .footer-disclaimer, .disclaimer-content, .legal, '
            + '.player-notice, .modal-notices',
        tokens: '^(?:0|18|21|100)$',
        why: 'the 18+ notice, the "100% on-chain" / "100% by AI" lines and the "0% rake" '
            + 'property of the unaudited-alpha disclaimer. Fixed legal copy, protected by '
            + '`make hygiene`. The `0` arrived with the portrait `.banner-strip` (the T-20 '
            + 'fix), which states the no-rake property verbatim on the table view; without it '
            + 'every portrait table scene failed the census on the word "0% rake" inside the '
            + 'player-protection notice itself',
    },
    {
        id: 'static-claim-copy',
        selector: '.intro, .intro-slim, .claims, .headline-claim, .rake-line strong, '
            + '.felt-marks, .method, .method-name, .proof-item .label, .list-foot, .norake',
        tokens: '^\\d{1,3}$',
        why: 'static claim copy: "0% rake", "SHA-256", "down to the last e8", the '
            + '"100% ON-CHAIN · NO RAKE" felt watermark, the numbered "1. Check the '
            + 'commitment…" method list, and the client\'s own poll interval. None of it '
            + 'quotes table state; the no-rake property itself is asserted per hand '
            + "against the history canister's rake field",
    },
    {
        id: 'app-version',
        selector: '.version',
        tokens: '^\\d+(?:\\.\\d+)*$',
        moneyShaped: true,
        why: 'THE ONE MONEY-SHAPED RULE: the footer version string "v0.1.0-alpha" '
            + 'tokenises as 0.1 and 0. `.version` is a footer span that renders a '
            + 'compile-time constant and has never held a canister value',
    },
    {
        id: 'static-explainer-copy',
        selector: '.how-it-works, .how-it-works-modal, .explainer, .steps, .step, .hiw-body, '
            + '.lobby-steps',
        tokens: '^\\d{1,4}$',
        why: 'numbered steps and worked examples in the static "How it works" copy, and the '
            + 'lobby\'s three-step strip under the list (LobbySteps.svelte: the step numerals '
            + '1, 2, 3 and the "SHA-256" of the committed deck): the same text for every '
            + 'table, quoting no live state',
    },

    // ---- the failure toast ---------------------------------------------------
    {
        id: 'lobby-failure-retry',
        selector: '.toast.error',
        tokens: '^\\d{1,3}$',
        context: '^(?:Could not reach the tables|The table list could not be read)\\. Retrying in \\d{1,3} s\\.',
        why: 'the seconds until the page re-reads the lobby on its own ("Retrying in 12 s.", '
            + 'lib/humane-errors.js describeLobbyFailure + retryDelayMs): a wall-clock '
            + 'interval the client chose, never a canister figure. The context pins the rule '
            + 'to that sentence, so a number in any other toast is excused by nothing',
    },
    {
        id: 'lobby-failure-detail',
        selector: '.toast.error .toast-detail',
        tokens: '^\\d+(?:[.,]\\d+)*$',
        moneyShaped: true,
        why: 'the raw agent text under the humane failure sentence (Toast.svelte '
            + '`.toast-detail`, lib/humane-errors.js detailOf): the gateway URL\'s host and '
            + 'port digits, an "os error 61", a line:column. MONEY-SHAPED because a URL '
            + 'carries dotted numbers (127.0.0.1); the element renders only what the agent '
            + 'threw, never a balance, and only while every canister read is failing',
    },

    // ---- how a computed figure was computed ----------------------------------
    {
        id: 'equity-method-counts',
        selector: '.equity-method, .replay-equity-method',
        tokens: '^\\d{1,3}(?:,\\d{3})*$',
        why: 'the sample size behind the equity badge ("Monte Carlo · 200,000 trials", '
            + '"exact · 990 runouts") and the count of random opponents in the modelled '
            + 'case ("vs 2 random"); `.replay-equity-method` is the same caption under the '
            + 'hand replayer\'s board (ReplayTable.svelte), the same non-money shape. '
            + 'Trial counts and player counts, never chips — and '
            + 'deliberately NOT money-shaped, so a decimal landing in this element could '
            + 'not be excused by this rule. The PERCENTAGE these describe is not '
            + 'allowlisted: it is recomputed by a second evaluator lineage and asserted '
            + '(lib/token-census.mjs, site `equity-badge`)',
    },
    // (`board-caption-count`, the "Board · 5 to come" caption, was retired here:
    // BoardStrip.svelte no longer renders a caption tag, so the rule excused
    // nothing. The list stays at its cap of 25 with the two failure-toast rules
    // above.)

    {
        id: 'log-equity-method-counts',
        selector: '.feed-item .phase-text',
        tokens: '^\\d{1,3}(?:,\\d{3})*$',
        why: 'the equity method line the felt no longer paints ("Equity vs 2 random · '
            + 'Monte Carlo · 200,000 trials") is logged once per computation in the '
            + 'action log as a phase-style line. Trial counts and opponent counts only, '
            + 'the same non-money shape as `equity-method-counts`; a decimal here is '
            + 'not excused',
    },

    // ---- identifiers and cryptographic material ------------------------------
    {
        id: 'identifier-digits',
        selector: '.hash, .commitment, .seed, .digest, .proof-hash, .mono, .principal, '
            + '.canister-id, .block-index, .deposit-address, .address, .linkish, .copy, '
            + 'code, .rederive-pos, .rung-mark',
        tokens: '^\\d{1,64}$',
        why: 'digits inside a hash, a shuffle seed, a principal, a canister id, a ledger '
            + 'block index, a deposit address, a deck position or a numbered proof rung. '
            + 'Identifiers and offsets, never amounts. The commitment hash on the fairness '
            + 'panel is verified WHOLE against the canister by the shuffleproof scene '
            + '(checks.hashMatchesCanister); the census does not re-match 64 hex digits '
            + 'one token at a time',
    },

    // ---- form affordances ----------------------------------------------------
    {
        id: 'empty-input-placeholder',
        selector: 'input, textarea',
        tokens: '^\\d+(?:\\.\\d+)?$',
        source: 'input-placeholder',
        moneyShaped: true,
        why: 'the PLACEHOLDER of an EMPTY amount field ("0.0000"): a hint about the '
            + 'format, not a value the client is asserting. The census only tags a token '
            + '`input-placeholder` when the field has no value at all, so a number the '
            + 'player has actually typed is never excused by this rule',
    },

    // ---- cycles, which are not player money ----------------------------------
    {
        id: 'cycle-runway-days',
        selector: '.runway-notice',
        tokens: '^\\d{1,6}$',
        why: 'DAYS of measured cycle runway, and the warning threshold in days, in the '
            + 'CycleRunwayNotice banner (docs/DEFECTS.md E-55). A duration, never an '
            + 'amount of anybody\'s money. The number itself comes from the canister\'s '
            + '`get_cycle_status().runway_days` and is gated by '
            + 'tools/shots/test-cycle-runway.mjs (32 checks, including that a null runway '
            + 'can never read as healthy) and by '
            + 'tests/money_safety/tests/cycles_runway.rs, which measures the burn the '
            + 'canister divides by against an external cycle-balance read',
    },
    {
        id: 'cycle-runway-trillions',
        selector: '.runway-notice',
        tokens: '^\\d+(?:\\.\\d+)?$',
        context: 'T ',
        moneyShaped: true,
        why: 'CYCLES, in trillions ("4.299 T spendable · burning 0.555 T/day"). Decimal '
            + 'and therefore money-SHAPED, and deliberately declared as such -- but cycles '
            + 'are not player money and are on no ledger: '
            + 'src/table_canister/src/lib.rs says so in as many words ("CYCLES. Not player '
            + 'money and not on any ledger"). No player balance, pot, stake, blind or '
            + 'buy-in can reach this selector: `.runway-notice` renders only the two '
            + 'figures `formatCycles()` produces from get_cycle_status, and the `context` '
            + 'pattern requires the unit T to be in the element text. The escrow and '
            + 'wallet figures in the same modal are asserted against the ledger by '
            + 'chain-agreement.mjs and are untouched by this rule',
    },
];
