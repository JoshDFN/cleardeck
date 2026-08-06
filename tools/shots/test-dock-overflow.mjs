// Self-check for the ACTION DOCK's containment: node tools/shots/test-dock-overflow.mjs
//
// docs/DEFECTS.md E-63. The occlusion gate, at 390x844, on `table-allin`:
//
//     OCCLUSION FAILED: 1 of 25 figures on screen are covered —
//     8.7% of "0.20 ICP" (span.committed-value) by div.stage
//
// The recorded geometry says exactly what happened. The figure's box was
// `y=731 h=21`; `div.stage` was `y=132.59 h=605`, so its bottom edge was 737.59
// and it lay over the figure's top 6.6 px. `.stage` is `position: relative` and
// the wallet panel is not positioned at all, so the stage PAINTS AFTER it
// (`paintsAfterTarget: true`, paint index 45 against 40) and the top of a money
// figure went under the felt.
//
// The cause is one declaration: `.action-dock { height: var(--dock-h) }`, a FIXED
// height, with a wallet panel inside it that is taller than that whenever a
// committed stake is on show.
//
// WHY THIS FILE EXISTS RATHER THAN A NOTE IN THE WAVE REPORT. The gate that found
// this only runs inside a full screenshot sweep, against a live replica with a
// funded all-in hand on it, and that sweep has been unrunnable for two waves. A
// fix that cannot be measured is a guess. So this measures the same thing with no
// replica: it builds a page from the COMPONENT'S OWN STYLESHEET -- read out of
// PokerTable.svelte at run time, not copied -- and asserts the invariant whose
// violation the gate reported:
//
//     every part of the wallet panel is inside the action dock's own box.
//
// Case 2 puts the single offending declaration back and requires the fixture to
// convict it, which is what makes case 1's green mean something.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const COMPONENT = path.resolve(
    HERE, '..', '..', 'src', 'cleardeck_frontend', 'src', 'lib', 'components', 'PokerTable.svelte',
);

// The measured layout of the failing shot, so the fixture reproduces the box the
// gate was looking at rather than an invented one. `desktop` is here because the
// base rule changed too and one viewport is not a measurement: `.action-dock`
// is shared, and only the phone was in the recorded failure.
const VIEWPORTS = {
    mobile: { width: 390, height: 844, stageTop: 132.59, belowDock: 16, feltFloor: 45.0, aspect: 0.555 },
    desktop: { width: 1440, height: 900, stageTop: 132.59, belowDock: 16, feltFloor: 28.0, aspect: 2.1 },
};
const VIEWPORT = VIEWPORTS.mobile;

/** The component's real `<style>` block. Svelte scoping is added at build time, so this is plain CSS. */
function componentCss() {
    const src = fs.readFileSync(COMPONENT, 'utf8');
    const open = src.lastIndexOf('<style>');
    const close = src.lastIndexOf('</style>');
    if (open < 0 || close < 0) throw new Error('PokerTable.svelte has no <style> block');
    return src.slice(open + '<style>'.length, close);
}

/**
 * The real DOM path of the failing figure, from the manifest:
 *   div.game-layout > div.table-area > div.poker-table-wrapper.ring-crowded
 *     > div.poker-table > div.stage > ...
 *     > div.poker-table > div.action-dock > div.dock-aux.dock-right
 *         > div.wallet-panel > div.wallet-committed > span.committed-value
 */
const BODY = `
<div class="game-layout"><div class="table-area">
  <div class="poker-table-wrapper ring-crowded">
    <div class="poker-table">
      <div class="stage"><div class="table-inner all-in-moment">
        <div class="seat seat-center"><span class="equity-badge modelled">62%</span></div>
      </div></div>
      <div class="action-dock">
        <div class="dock-aux dock-left"><button class="log-toggle">LOG</button></div>
        <div class="dock-aux dock-right">
          <div class="wallet-panel">
            <div class="wallet-balance">
              <span class="balance-label">Table balance</span>
              <span class="balance-value">110.20 ICP</span>
            </div>
            <div class="wallet-committed">
              <span class="committed-label">In the pot</span>
              <span class="committed-value">0.20 ICP</span>
              <span class="committed-note">Yours, in hand 1, until the hand settles.</span>
            </div>
            <div class="wallet-actions">
              <button class="wallet-action-btn deposit">Deposit</button>
              <button class="wallet-action-btn withdraw">Withdraw</button>
            </div>
          </div>
        </div>
        <div class="dock-center"><div class="actions">
          <button class="action-btn secondary">Fold</button>
          <button class="action-btn primary">Check</button>
          <button class="action-btn raise">Raise</button>
        </div></div>
      </div>
    </div>
  </div>
</div></div>`;

/** Boxes for the dock and everything in the wallet panel, at 390x844. */
async function measure(browser, { reintroduceTheDefect, viewport = VIEWPORT }) {
    const context = await browser.newContext({
        viewport: { width: viewport.width, height: viewport.height },
        deviceScaleFactor: 1,
        colorScheme: 'dark',
    });
    const page = await context.newPage();
    const css = componentCss();
    // `--cd-avail` is what the app sets on the wrapper; here it is the measured
    // stage + dock so the fixture's poker-table column is the failing one.
    const availablePx = viewport.height - viewport.stageTop - viewport.belowDock;
    // Case 2 only: put the ONE declaration back that the fix removed.
    const defect = reintroduceTheDefect
        ? '.action-dock { height: var(--dock-h) !important; }'
        : '';
    await page.setContent(
        `<!doctype html><html><head><style>
           html, body { margin: 0; padding: 0; background: #07100c; color: #fff;
                        font: 14px/1.2 system-ui, sans-serif; }
           .game-layout { padding-top: ${viewport.stageTop}px; }
           .poker-table-wrapper { --cd-avail: ${availablePx}px; }
           ${css}
           ${defect}
         </style></head><body>${BODY}</body></html>`,
        { waitUntil: 'load' },
    );
    await page.evaluate(async () => { if (document.fonts?.ready) await document.fonts.ready; });

    return page.evaluate(() => {
        const box = (sel) => {
            const el = document.querySelector(sel);
            if (!el) return null;
            const r = el.getBoundingClientRect();
            return { top: r.top, bottom: r.bottom, left: r.left, right: r.right, h: r.height };
        };
        // THE FELT'S SIZE, WITHOUT RENDERING A FELT. `.table-inner` sets
        // `font-size: var(--ui)` and `--ui: calc(var(--fw) * var(--ui-r))`, so the
        // computed font size divided by `--ui-r` IS `--fw` -- the one number every
        // felt dimension is derived from. That is how this file can tell whether
        // giving the dock its room costs the felt anything, without a canister.
        const inner = document.querySelector('.table-inner');
        let fw = null;
        if (inner) {
            const cs = getComputedStyle(inner);
            const uiR = parseFloat(cs.getPropertyValue('--ui-r'));
            const px = parseFloat(cs.fontSize);
            if (Number.isFinite(uiR) && uiR > 0 && Number.isFinite(px)) fw = px / uiR;
        }
        return {
            dock: box('.action-dock'),
            stage: box('.stage'),
            panel: box('.wallet-panel'),
            committedValue: box('.committed-value'),
            committedNote: box('.committed-note'),
            feltWidth: fw,
        };
    }).finally(async () => { await context.close(); });
}

const cases = [];
const check = (name, condition, detail) => cases.push({ name, ok: Boolean(condition), detail });

const browser = await chromium.launch();

// ---------------------------------------------------------------------------
// 0. the fixture is measuring the right thing at all
// ---------------------------------------------------------------------------
const fixed = await measure(browser, { reintroduceTheDefect: false });
check('the fixture renders the dock', fixed.dock !== null, JSON.stringify(fixed));
check('the fixture renders the committed money figure', fixed.committedValue !== null, JSON.stringify(fixed));
// `.poker-table` is a flex column with `gap: 8px`, so the two boxes are adjacent
// with exactly that gap between them and nothing else.
const COLUMN_GAP = 8;
check(
    'and the stage sits directly above the dock, as it does in the app',
    fixed.stage && fixed.dock
        && Math.abs((fixed.dock.top - fixed.stage.bottom) - COLUMN_GAP) < 1,
    `stage.bottom=${fixed.stage?.bottom} dock.top=${fixed.dock?.top}`,
);

// ---------------------------------------------------------------------------
// 1. THE INVARIANT. Nothing in the wallet panel may leave the dock's box.
// ---------------------------------------------------------------------------
const EPS = 0.5;
for (const [what, b] of Object.entries({
    'the wallet panel': fixed.panel,
    'the committed money figure': fixed.committedValue,
    'the committed note': fixed.committedNote,
})) {
    check(
        `${what} stays inside the action dock`,
        b && fixed.dock && b.top >= fixed.dock.top - EPS && b.bottom <= fixed.dock.bottom + EPS,
        `dock=[${fixed.dock?.top}, ${fixed.dock?.bottom}]  ${what}=[${b?.top}, ${b?.bottom}]`,
    );
}
check(
    'so no part of it is under the stage',
    fixed.committedValue && fixed.stage && fixed.committedValue.top >= fixed.stage.bottom - EPS,
    `stage.bottom=${fixed.stage?.bottom} figure.top=${fixed.committedValue?.top}`,
);
check(
    'and the dock is still at least as tall as --dock-h',
    fixed.dock && fixed.dock.h >= 90 - EPS,
    `dock height ${fixed.dock?.h}`,
);

// ---------------------------------------------------------------------------
// 2. THE SAME FIXTURE, WITH THE DEFECT PUT BACK. It has to convict.
// ---------------------------------------------------------------------------
const broken = await measure(browser, { reintroduceTheDefect: true });
const overflowPx = broken.dock && broken.panel ? broken.dock.top - broken.panel.top : 0;
check(
    '`height: var(--dock-h)` pushes the wallet panel OUT of the dock',
    overflowPx > 1,
    `the panel's top is ${overflowPx.toFixed(1)}px above the dock's top`,
);
check(
    '...and into the stage, which paints over it — the shape the gate reported',
    broken.panel && broken.stage && broken.panel.top < broken.stage.bottom - 1,
    `stage.bottom=${broken.stage?.bottom} panel.top=${broken.panel?.top} `
    + `(${(broken.stage.bottom - broken.panel.top).toFixed(1)}px of the panel is under the stage)`,
);

// ---------------------------------------------------------------------------
// 3. AND IT DOES NOT TRADE ONE RED FOR ANOTHER.
//
//    The dock takes its room from the stage, and the stage is what sizes the
//    felt. `felt-area.mjs` fails any mobile shot whose felt is under 45% of the
//    frame; the last recorded run measured 50.7%. A fix that bought an occlusion
//    green with a felt red would be a straight swap, so it is measured here.
// ---------------------------------------------------------------------------
const FELT_AREA_FLOOR_MOBILE = 45.0;
const FELT_ASPECT = 0.555; // `--ar` for the portrait ring
const frameArea = VIEWPORT.width * VIEWPORT.height;
const feltPct = (fw) => (fw === null ? null : ((fw * (fw / FELT_ASPECT)) / frameArea) * 100);
const before = feltPct(broken.feltWidth);
const after = feltPct(fixed.feltWidth);
check(
    'the felt is still measurable in the fixture',
    before !== null && after !== null,
    `before=${broken.feltWidth} after=${fixed.feltWidth}`,
);
// THE TRADE, STATED AND BOUNDED. The dock's room comes out of the stage, and
// past 14 px of it the felt starts shrinking, so this fix is not free: on a
// mobile shot with a stake outstanding the felt goes 50.7% -> 46.4%. That is a
// real cost and it is the right side of the trade -- the alternative is the top
// of a money figure sliced off under the felt -- but it must never be silent,
// and it must never creep. The margin below the floor check is what stops the
// next line added to the committed note being paid for out of the felt: the
// first draft of this fix measured 44.1% and would have swapped one red for
// another.
const REQUIRED_MARGIN = 1.0;
check(
    `the felt clears the ${FELT_AREA_FLOOR_MOBILE}% mobile floor by ${REQUIRED_MARGIN} point`,
    after !== null && after >= FELT_AREA_FLOOR_MOBILE + REQUIRED_MARGIN,
    `felt ${after?.toFixed(1)}% of the frame (was ${before?.toFixed(1)}% with the fixed-height `
    + 'dock). The committed block is taking more stage height than there is spare. Tighten the '
    + 'block -- padding, gap, wording -- rather than letting it eat the felt.',
);
console.log(
    `note  felt width ${broken.feltWidth?.toFixed(1)}px -> ${fixed.feltWidth?.toFixed(1)}px, `
    + `area ${before?.toFixed(1)}% -> ${after?.toFixed(1)}% of the frame `
    + `(floor ${FELT_AREA_FLOOR_MOBILE}%); the dock took ${
        (broken.stage.h - fixed.stage.h).toFixed(1)}px of stage height`,
);

// ---------------------------------------------------------------------------
// 4. THE OTHER VIEWPORT.
//
//    The recorded failure was on a phone, but the declaration that caused it is
//    in the BASE rule, which desktop shares. Fixing one viewport and measuring
//    one viewport is how the next wave inherits the other half.
//
//    MEASURED ANSWER: at 1440 wide the panel is a single row with room for the
//    note, it already fits inside the 78 px dock, and the change is a NO-OP --
//    the dock takes 0 px of stage height either way. That is the claim being
//    pinned. It is the containment and the no-op that are asserted here and not
//    an absolute felt figure: this fixture reuses the phone's chrome offsets, so
//    its desktop stage is larger than the app's (it computes ~50% against the
//    33.2% the real run recorded) and an absolute number off it would be a
//    number made up. The DELTA is still sound, because both arms share the
//    fixture.
// ---------------------------------------------------------------------------
{
    const vp = VIEWPORTS.desktop;
    const d = await measure(browser, { reintroduceTheDefect: false, viewport: vp });
    const dBroken = await measure(browser, { reintroduceTheDefect: true, viewport: vp });
    for (const [arm, m] of Object.entries({ 'with the fix': d, 'with the defect': dBroken })) {
        check(
            `desktop: the wallet panel is inside the dock ${arm}`,
            m.panel && m.dock && m.panel.top >= m.dock.top - EPS && m.panel.bottom <= m.dock.bottom + EPS,
            `dock=[${m.dock?.top}, ${m.dock?.bottom}] panel=[${m.panel?.top}, ${m.panel?.bottom}]`,
        );
    }
    const stolen = dBroken.stage.h - d.stage.h;
    check(
        'desktop: the change costs the stage nothing at all',
        Math.abs(stolen) < 1,
        `the dock took ${stolen.toFixed(1)}px of stage height at 1440x900`,
    );
    check(
        'desktop: and the felt is byte-identical between the two arms',
        d.feltWidth !== null && Math.abs(d.feltWidth - dBroken.feltWidth) < 0.5,
        `${dBroken.feltWidth?.toFixed(1)}px -> ${d.feltWidth?.toFixed(1)}px`,
    );
}

await browser.close();

for (const c of cases) {
    console.log(`${c.ok ? 'ok  ' : 'FAIL'} ${c.name}${c.ok ? '' : `\n     ${c.detail}`}`);
}
const failed = cases.filter((c) => !c.ok);
if (failed.length) {
    console.error(`\n${failed.length} of ${cases.length} DOCK CONTAINMENT CASES FAILED`);
    process.exit(1);
}
console.log(`\nALL ${cases.length} DOCK CONTAINMENT CASES PASS`);
