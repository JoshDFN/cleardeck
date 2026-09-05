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
import { fileURLToPath, pathToFileURL } from 'node:url';
import { chromium } from 'playwright';
import * as sass from 'sass';

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

/**
 * The component's real stylesheet, compiled the way the build compiles it.
 *
 * The UI wave made the block `<style lang="scss">` and moved the dock's and the
 * geometry's rules into Sass MIXINS (`poker-table-dock.scss`,
 * `poker-table-tokens.scss`) that the block `@include`s where the rules used to
 * sit. So the text between the style tags is no longer CSS a browser can read:
 * it is compiled here with the same `sass` the frontend build uses, resolving
 * the partials from the component's own directory, so this fixture keeps
 * measuring the stylesheet the app ships and not a copy. Svelte's `:global(x)`
 * wrapper is unwrapped exactly as the compiler unwraps it (minus the scoping
 * hash, which this fixture never had).
 */
const INDEX_SCSS = path.resolve(HERE, '..', '..', 'src', 'cleardeck_frontend', 'src', 'index.scss');

/**
 * The app's token layer (`src/index.scss`), compiled. Every dock dimension the
 * fixture measures is a `var(--cd-*)` now (the gap under the stage, the dock's
 * padding, the touch floor), and a token the fixture does not define computes
 * to nothing, so the box it measured would be a box the app never paints.
 */
function tokenCss() {
    return sass.compileString(fs.readFileSync(INDEX_SCSS, 'utf8'), {
        loadPaths: [path.dirname(INDEX_SCSS)],
        url: pathToFileURL(INDEX_SCSS),
        style: 'expanded',
        silenceDeprecations: ['legacy-js-api'],
        logger: sass.Logger.silent,
    }).css;
}

function componentCss() {
    const src = fs.readFileSync(COMPONENT, 'utf8');
    const openTag = src.match(/<style(?:\s+lang="scss")?>/g);
    const close = src.lastIndexOf('</style>');
    if (!openTag || close < 0) throw new Error('PokerTable.svelte has no <style> block');
    const last = openTag[openTag.length - 1];
    const open = src.lastIndexOf(last);
    const block = src.slice(open + last.length, close);
    const plain = last.includes('scss')
        ? sass.compileString(block, {
            loadPaths: [path.dirname(COMPONENT)],
            url: pathToFileURL(COMPONENT),
            style: 'expanded',
            // The frontend build is on the same legacy warning; it is not a defect here.
            silenceDeprecations: ['legacy-js-api'],
            logger: sass.Logger.silent,
        }).css
        : block;
    // `:global(.a .b)` -> `.a .b` (one level of parentheses inside is enough for this file).
    return plain.replace(/:global\(([^()]*(?:\([^()]*\)[^()]*)*)\)/g, '$1');
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
    const css = tokenCss() + '\n' + componentCss();
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
        const table = document.querySelector('.poker-table');
        const gap = table ? parseFloat(getComputedStyle(table).rowGap) : NaN;
        return {
            columnGap: Number.isFinite(gap) ? gap : NaN,
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
//
// THE GAP IS READ, NOT HARDCODED. It was written here as a literal `8`, and the
// moment the portrait rule changed it the fixture went red on the ADJACENCY
// claim -- which is still true -- instead of on anything that had broken. A
// self-test that fails for a reason it is not testing is a self-test people
// switch off, so the number comes from the same stylesheet the assertion is
// about and only the ADJACENCY is asserted.
const COLUMN_GAP = fixed.columnGap;
check(
    'and the stage sits directly above the dock, as it does in the app',
    fixed.stage && fixed.dock && Number.isFinite(COLUMN_GAP)
        && Math.abs((fixed.dock.top - fixed.stage.bottom) - COLUMN_GAP) < 1,
    `stage.bottom=${fixed.stage?.bottom} dock.top=${fixed.dock?.top} `
    + `(.poker-table row-gap ${COLUMN_GAP}px, read from the component's own stylesheet)`,
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
// WHICH EDGE IT LEAVES BY IS THE DOCK'S LAYOUT, NOT THE DEFECT. The recorded
// failure (E-63) had the panel spill out of the TOP of a flex dock and under the
// stage. The UI wave's dock is a grid that keeps the panel's top on the dock's
// top, so the same fixed height spills it out of the BOTTOM instead, under the
// viewport's edge. Either way the money figure has left the one box that is
// laid out to hold it, which is the thing this arm has to convict.
const overflowAbove = broken.dock && broken.panel ? broken.dock.top - broken.panel.top : 0;
const overflowBelow = broken.dock && broken.panel ? broken.panel.bottom - broken.dock.bottom : 0;
check(
    '`height: var(--dock-h)` pushes the wallet panel OUT of the dock',
    overflowAbove > 1 || overflowBelow > 1,
    `the panel's top is ${overflowAbove.toFixed(1)}px above the dock's top and its bottom `
    + `${overflowBelow.toFixed(1)}px below the dock's bottom`,
);
// ...and the spilled part lands where the app paints something else: under the
// stage (the shape E-63 recorded, the felt painting over a money row) or past
// the dock's bottom edge (in the app, under the phone's home indicator). Which
// edge depends on how the grid centres the panel; both are content the layout
// no longer accounts for.
check(
    '...and the spilled part sits under the stage or past the dock\'s bottom edge',
    broken.panel && broken.dock && broken.stage
        && (broken.panel.top < broken.stage.bottom - 1 || broken.panel.bottom > broken.dock.bottom + 1),
    `dock=[${broken.dock?.top}, ${broken.dock?.bottom}] panel=[${broken.panel?.top}, ${broken.panel?.bottom}] `
    + `stage.bottom=${broken.stage?.bottom} figure=[${broken.committedValue?.top}, ${broken.committedValue?.bottom}]`,
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
    check(
        'desktop: the wallet panel is inside the dock with the fix',
        d.panel && d.dock && d.panel.top >= d.dock.top - EPS && d.panel.bottom <= d.dock.bottom + EPS,
        `dock=[${d.dock?.top}, ${d.dock?.bottom}] panel=[${d.panel?.top}, ${d.panel?.bottom}]`,
    );
    // MEASURED ANSWER, RE-MEASURED AFTER THE UI WAVE. The wave's desktop wallet
    // panel is three rows when a stake is outstanding (the balance, the IN THE
    // POT line on its own row, the two buttons), taller than --dock-h, so the
    // fixed height convicts on desktop too and the fix costs the stage the
    // difference (about 13 px at 1440x900). The old pin here ("a no-op on
    // desktop") described the one-row panel and is not a measurement of this
    // dock; what is asserted instead is what the trade has to satisfy: the
    // defect is convicted, and the felt keeps the desktop floor with a margin
    // (the real run records 39.8% on the facing-bet scene with the stake row up).
    const dSpill = dBroken.panel && dBroken.dock
        && (dBroken.panel.top < dBroken.dock.top - 1 || dBroken.panel.bottom > dBroken.dock.bottom + 1);
    check(
        'desktop: the same fixed height convicts on desktop too (the panel leaves the dock)',
        Boolean(dSpill),
        `dock=[${dBroken.dock?.top}, ${dBroken.dock?.bottom}] panel=[${dBroken.panel?.top}, ${dBroken.panel?.bottom}]`,
    );
    const stolen = dBroken.stage.h - d.stage.h;
    const desktopFrame = vp.width * vp.height;
    const desktopFeltPct = (fw) => (fw === null ? null : ((fw * (fw / vp.aspect)) / desktopFrame) * 100);
    const dAfter = desktopFeltPct(d.feltWidth);
    check(
        `desktop: the room the dock takes (${stolen.toFixed(1)}px) leaves the felt over the ${vp.feltFloor}% floor by ${REQUIRED_MARGIN} point`,
        dAfter !== null && dAfter >= vp.feltFloor + REQUIRED_MARGIN,
        `felt ${dAfter?.toFixed(1)}% of the frame with the fix (the fixture's desktop stage is larger `
        + 'than the app\'s, so this is a bound on the trade, not the recorded figure)',
    );
    // The felt is NOT identical between the arms any more (the one-row panel's
    // no-op is gone with it); the floor-with-margin check above is the bound on
    // that trade, and the delta is printed so it can never creep in silence.
    // The real run records 41.0% without the stake row and 39.8% with it.
    console.log(
        `note  desktop: felt width ${dBroken.feltWidth?.toFixed(1)}px -> ${d.feltWidth?.toFixed(1)}px `
        + `between the fixed-height arm and the fix (the stake row's cost at 1440x900)`,
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
