// Self-check for the PIXEL gate: node tools/shots/test-occlusion.mjs
//
// The gate in lib/occlusion.mjs is the only thing in this repo that judges what a
// player SEES rather than what the DOM says, so it needs its own ground truth.
// Every case below is a page whose answer is known by construction: the overlap
// was written into the fixture, so a wrong verdict is a defect in the gate and
// not a matter of opinion about the product.
//
// The two failure modes are weighted deliberately:
//
//   A FALSE RED is fatal. A gate that flags decoration gets switched off, and
//   then the next T-22 ships. Cases 4, 5, 6 and 9 are overlaps that must PASS —
//   an element that only touches padding, one that is genuinely behind an opaque
//   figure, a translucent veil the figure still shows through, and the case a
//   naive z-index comparison gets wrong in the direction of crying wolf.
//
//   A FALSE GREEN is the bug the gate exists to remove. Cases 1, 2, 3, 7, 8 and
//   10 are real occlusions, including the exact stacking shape of T-22, an
//   occluder with `pointer-events: none` (invisible to hit testing, which is why
//   hit testing alone is not the authority), and the case a naive z-index
//   comparison MISSES.
//
// This runs on a stub page built with setContent: no replica, no canisters, no
// app build. Chromium comes from tools/shots/node_modules.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';
import { assertNothingCoversAFigure, INK_FRACTION_THRESHOLD } from './lib/occlusion.mjs';

const OUT = fs.mkdtempSync(path.join(os.tmpdir(), 'cleardeck-occl-'));

const page$ = async (browser, body) => {
  const context = await browser.newContext({
    viewport: { width: 420, height: 320 }, deviceScaleFactor: 1, colorScheme: 'dark',
  });
  const page = await context.newPage();
  await page.setContent(`<!doctype html><html><head><style>
    html, body { margin: 0; padding: 0; background: #07100c; }
    body { position: relative; width: 420px; height: 320px;
           font: 16px/1.2 ui-monospace, monospace; color: #fff; }
    .card { width: 44px; height: 62px; background: #fff; border-radius: 4px; }
    .card-front { display: flex; flex-direction: column; align-items: center; color: #2C2C2C; }
    .rank { font-size: 22px; font-weight: 800; }
    .pip { font-size: 20px; }
  </style></head><body>${body}</body></html>`, { waitUntil: 'load' });
  await page.evaluate(async () => { if (document.fonts?.ready) await document.fonts.ready; });
  return { page, context };
};

const cases = [];
const results = [];

/**
 * @param {string} name
 * @param {string} body the fixture
 * @param {(r:{ok:boolean,checks:object,notes:string}) => {pass:boolean, detail:string}} expect
 * @param {{scrollTo?:number}} [opts] page state to establish before the gate runs
 */
const scenario = (name, body, expect, opts = {}) => cases.push({ name, body, expect, opts });

/** Every finding whose occluded element matches `sel` in its path. */
const on = (r, sel) => r.checks.findings.filter((f) => f.occluded.path.includes(sel));
const cleared = (r, sel) => r.checks.geometryOnlyIntersections.filter((f) => f.occluded.path.includes(sel));
const pct = (f) => `${(f.pixels.coveredInkFraction * 100).toFixed(1)}%`;

// ---------------------------------------------------------------------------
// 1. A REAL OCCLUSION: an opaque lid over half a money figure.
// ---------------------------------------------------------------------------
scenario(
  'an opaque lid over half a money figure is reported, with the fraction',
  `<span class="money" style="position:absolute;left:20px;top:20px;background:#111">12.34</span>
   <div class="lid" style="position:absolute;left:20px;top:20px;width:35px;height:24px;
        background:#e8543a;z-index:5"></div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.lid')
        && f.pixels.coveredInkFraction > 0.2 && f.occluder.paintsAfterTarget,
      detail: f ? `${pct(f)} by ${f.occluder.path}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 2. T-22's EXACT SHAPE. `.equity-badge` at z-index 3 inside a `.player-nameplate`
//    at z-index 6, with `.player-cards` at z-index 7 as a sibling of the plate.
//    Three nested stacking contexts; the badge cannot win from in there.
// ---------------------------------------------------------------------------
scenario(
  'T-22 shape: a badge inside a z-index 6 plate, covered by z-index 7 cards',
  `<div class="seat" style="position:absolute;left:120px;top:140px;z-index:10;width:0;height:0">
     <div class="player-nameplate" style="position:absolute;left:-60px;top:-20px;z-index:6;
          width:120px;height:40px;background:#262832;overflow:visible">
       <span class="equity-badge" style="position:absolute;right:2px;top:-14px;z-index:3;
             background:#7ee2b8;color:#06231a;font-size:11px;font-weight:800">100.00%</span>
     </div>
     <div class="player-cards" style="position:absolute;left:0px;top:-46px;z-index:7;
          width:70px;height:40px;background:#fff"></div>
   </div>`,
  (r) => {
    const f = on(r, '.equity-badge')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.player-cards')
        && f.pixels.coveredInkFraction > 0.3,
      detail: f ? `${pct(f)} by ${f.occluder.path}; hit=${JSON.stringify(f.hitTest.centre)}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 3. THE NAIVE GATE'S FALSE NEGATIVE. The figure's own z-index is 99 and the
//    occluder's is 1, so a z-index comparison says "below". Effective paint order
//    says otherwise, because the occluder's stacking context (2) is above the
//    figure's (1), and the pixels agree.
// ---------------------------------------------------------------------------
scenario(
  'a z-index 1 element over a z-index 99 figure IS caught (naive compare misses it)',
  `<div style="position:absolute;left:0;top:0;z-index:1">
     <span class="money" style="position:absolute;left:30px;top:60px;z-index:99">66.60</span>
   </div>
   <div style="position:absolute;left:0;top:0;z-index:2">
     <div class="lid" style="position:absolute;left:30px;top:58px;width:60px;height:24px;
          background:#e9ff63;z-index:1"></div>
   </div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.lid')
        && f.occluder.naiveZIndexSaysAbove === false && f.occluder.paintsAfterTarget === true,
      detail: f ? `${pct(f)}; naiveSaysAbove=${f.occluder.naiveZIndexSaysAbove}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 4. THE NAIVE GATE'S FALSE RED. The occluder's z-index is 99 and the figure's is
//    2, but the figure's stacking context is above the occluder's, so nothing is
//    covered. This case is the reason the gate cannot be a z-index comparison.
// ---------------------------------------------------------------------------
scenario(
  'a z-index 99 element UNDER a z-index 2 figure is not reported (naive gate false red)',
  `<div style="position:absolute;left:0;top:0;z-index:1">
     <div class="blob" style="position:absolute;left:30px;top:118px;width:80px;height:24px;
          background:#ff00ff;z-index:99"></div>
   </div>
   <div style="position:absolute;left:0;top:0;z-index:9">
     <span class="money" style="position:absolute;left:30px;top:120px;background:#111;z-index:2">55.50</span>
   </div>`,
  (r) => {
    const g = cleared(r, '.money').find((x) => x.occluder.path.includes('.blob'));
    return {
      pass: r.ok && Boolean(g) && g.pixels.coveredPixels === 0
        && g.occluder.naiveZIndexSaysAbove === true && g.occluder.paintsAfterTarget === false,
      detail: g ? `cleared: ${g.why}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 5. DECORATION. The overlap lands entirely inside the figure's padding, so it
//    takes no ink. Must pass — this is the case that decides whether the gate is
//    tolerable to live with.
// ---------------------------------------------------------------------------
scenario(
  'an overlap that only touches padding covers 0.0% of the ink and passes',
  `<span class="money" style="position:absolute;left:200px;top:40px;padding:12px">7.00</span>
   <div class="deco" style="position:absolute;left:200px;top:40px;width:10px;height:10px;
        background:#49A16E;z-index:5"></div>`,
  (r) => {
    const g = cleared(r, '.money').find((x) => x.occluder.path.includes('.deco'));
    return {
      pass: r.ok && Boolean(g) && g.pixels.coveredPixels === 0 && g.geometry.intersectionPx >= 90,
      detail: g ? `intersects ${g.geometry.intersectionPx}px, covers ${g.pixels.coveredPixels}px` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 6. BEHIND. An element under an opaque figure intersects it completely and takes
//    nothing away.
// ---------------------------------------------------------------------------
scenario(
  'an element painted behind an opaque figure is not reported',
  `<div class="under" style="position:absolute;left:250px;top:150px;width:90px;height:26px;
        background:#e8543a"></div>
   <span class="money" style="position:absolute;left:250px;top:150px;width:90px;height:26px;
        background:#111;z-index:3">9.99</span>`,
  (r) => {
    const g = cleared(r, '.money').find((x) => x.occluder.path.includes('.under'));
    return {
      pass: r.ok && Boolean(g) && g.pixels.coveredPixels === 0,
      detail: g ? `covers ${g.pixels.coveredPixels}px of ${g.pixels.inkPixels}px ink` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 7. A TRANSLUCENT VEIL. The figure still shows through, so this does NOT fail —
//    and the gate says so out loud with alteredInkFraction rather than pretending
//    it did not happen. (Documented limitation, asserted here so it stays true.)
// ---------------------------------------------------------------------------
scenario(
  'a translucent veil is measured as altered, not as covered, and passes',
  `<span class="money" style="position:absolute;left:30px;top:200px;background:#111">44.44</span>
   <div class="veil" style="position:absolute;left:30px;top:200px;width:80px;height:24px;
        background:rgba(255,60,60,0.35);z-index:5"></div>`,
  (r) => {
    const g = cleared(r, '.money').find((x) => x.occluder.path.includes('.veil'));
    return {
      pass: r.ok && Boolean(g) && g.pixels.coveredPixels === 0 && g.pixels.alteredInkFraction > 0.5,
      detail: g ? `altered ${(g.pixels.alteredInkFraction * 100).toFixed(0)}% of ink, covered ${g.pixels.coveredPixels}px` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 8. `pointer-events: none`. HIT TESTING CANNOT SEE THIS OCCLUDER AT ALL — every
//    probe point still answers the figure — and the pixels catch it anyway. This
//    is why the pixel differential is the authority and not the corroboration.
//    (`.board-cluster` is pointer-events: none in portrait, so this is the real
//    configuration, not a hypothetical.)
// ---------------------------------------------------------------------------
scenario(
  'an occluder with pointer-events:none is invisible to hit testing and still caught',
  `<span class="money" style="position:absolute;left:150px;top:240px;background:#111">88.88</span>
   <div class="ghost" style="position:absolute;left:150px;top:240px;width:80px;height:24px;
        background:#0b3d91;z-index:5;pointer-events:none"></div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.ghost')
        && f.hitTest.foreign === 0 && f.pixels.coveredInkFraction > 0.9,
      detail: f ? `${pct(f)} covered, hit-test foreign points: ${f.hitTest.foreign}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 9. T-23's SHAPE, plus the ASYMMETRY. The chip covers the card; the card does
//    not cover the chip. A gate that reported both would be reporting noise.
// ---------------------------------------------------------------------------
scenario(
  'T-23 shape: an award chip over a card is reported on the CARD only, not on the chip',
  `<div class="player-cards" style="position:absolute;left:40px;top:110px;z-index:7">
     <div class="card"><div class="card-front"><span class="rank">8</span><span class="pip">&spades;</span></div></div>
   </div>
   <div class="winner-award" style="position:absolute;left:52px;top:132px;z-index:20">
     <span class="stack-delta" style="background:#E9FF63;color:#1d2000;padding:2px 6px;
           border-radius:99px;font-size:13px;font-weight:800">+24.00</span>
   </div>`,
  (r) => {
    const card = on(r, '.card').find((f) => f.occluded.kind === 'card');
    const pip = on(r, '.pip')[0];
    const chip = on(r, '.stack-delta');
    // The occluder named is `.winner-award` — the positioned element carrying the
    // transform and the z-index, i.e. the line a fixer acts on — with the
    // `.stack-delta` inside it folded under it rather than reported twice.
    const folded = (card?.alsoAccountedForBy || []).some((x) => x.path.includes('.stack-delta'));
    return {
      pass: !r.ok && Boolean(card) && Boolean(pip)
        && card.occluder.path.includes('.winner-award') && folded && chip.length === 0,
      detail: card
        ? `card ${pct(card)} and pip ${pct(pip)} by ${card.occluder.path.split(' > ').pop()}; `
          + `descendants folded in: ${(card.alsoAccountedForBy || []).length}; findings on the chip: ${chip.length}`
        : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 10. A TRANSFORM opens a stacking context with z-index auto. Both the figure and
//     the transformed box are z-index-auto step-6 items, so TREE ORDER decides
//     and the later one wins. Nothing in a z-index comparison can see this.
// ---------------------------------------------------------------------------
scenario(
  'a transformed sibling later in tree order covers the figure (paint order, not z-index)',
  `<span class="money" style="position:absolute;left:300px;top:250px;background:#111">31.10</span>
   <div style="transform: translate(0px, 0px)">
     <div class="lid2" style="position:absolute;left:300px;top:250px;width:60px;height:22px;
          background:#7ee2b8"></div>
   </div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.lid2')
        && f.occluder.zIndex === 'auto' && f.occluder.paintsAfterTarget,
      detail: f ? `${pct(f)} by a z-index:${f.occluder.zIndex} element` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 11a. A MODAL OVER THE PAGE. The single most dangerous false red: a dialog is
//      SUPPOSED to cover what is behind it, and the first full sweep reported 37
//      covered figures on the hand-history scene for exactly that reason. The
//      page's figures must be reported as `behind-an-overlay` and NOT fail...
// ---------------------------------------------------------------------------
scenario(
  'a dialog covering the page behind it does not fail the scene',
  `<span class="money" style="position:absolute;left:20px;top:20px;background:#111">12.34</span>
   <div class="modal-backdrop" style="position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:50"></div>
   <div class="modal" role="dialog" aria-modal="true"
        style="position:fixed;left:60px;top:80px;width:280px;height:160px;background:#151a17;z-index:51">
     <span class="money" style="color:#fff">99.99</span>
   </div>`,
  (r) => {
    const behind = r.checks.behindAnOverlay.filter((b) => b.text === '12.34');
    return {
      pass: r.ok && behind.length > 0 && r.checks.counts.occlusionsFound === 0
        && r.checks.counts.figuresBehindAnOverlay === 1,
      detail: `behind an overlay: ${behind.map((b) => `${b.text} ${(b.coveredInkFraction * 100).toFixed(0)}% by ${b.overlay}`).join(', ')}`,
    };
  },
);

// ---------------------------------------------------------------------------
// 11b. ...and the converse is NOT excused: a figure INSIDE the dialog that the
//      dialog's own chrome covers is a defect, and the overlay rule must not
//      launder it.
// ---------------------------------------------------------------------------
scenario(
  'a figure INSIDE a dialog, covered by the dialog\'s own chrome, still fails',
  `<div class="modal" role="dialog" aria-modal="true"
        style="position:fixed;left:0;top:0;width:390px;height:300px;background:#151a17;z-index:51">
     <span class="money" style="position:absolute;left:30px;top:40px;background:#111">55.10</span>
     <div class="modal-header" style="position:absolute;left:30px;top:40px;width:50px;height:24px;
          background:#2b3630;z-index:9"></div>
   </div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.modal-header'),
      detail: f ? `${pct(f)} by ${f.occluder.path.split(' > ').pop()}` : r.notes,
    };
  },
);

// ---------------------------------------------------------------------------
// 11c. A SCROLLED PAGE. `page.screenshot({ clip })` trims the clip against the
//      VIEWPORT, not the document, so a clip built in document coordinates either
//      throws (which is how this was found, on the deposit scene at mobile, where
//      the open modal leaves the page scrolled) or -- far worse -- lands somewhere
//      else inside the viewport and measures a different rectangle with total
//      confidence. This is the regression test for that arithmetic.
// ---------------------------------------------------------------------------
scenario(
  'a figure on a SCROLLED page is measured at the right rectangle',
  `<div style="height:1400px"></div>
   <span class="money" style="position:absolute;left:30px;top:1180px;background:#111">77.70</span>
   <div class="lid3" style="position:absolute;left:30px;top:1180px;width:44px;height:24px;
        background:#e8543a;z-index:5"></div>
   <div style="height:600px"></div>`,
  (r) => {
    const f = on(r, '.money')[0];
    return {
      pass: !r.ok && Boolean(f) && f.occluder.path.includes('.lid3')
        && f.pixels.inkPixels > 50 && f.pixels.coveredInkFraction > 0.3
        && r.checks.figuresNotMeasurable.length === 0,
      detail: f
        ? `${pct(f)} of ${f.pixels.inkPixels}px ink, clip-sanity failures: ${r.checks.figuresNotMeasurable.length}`
        : `notes: ${r.notes}; unmeasurable: ${JSON.stringify(r.checks.figuresNotMeasurable)}`,
    };
  },
  { scrollTo: 1100 },
);

// ---------------------------------------------------------------------------
// 11d. A BACKDROP-FILTER PILL OVER A BACKGROUND. The live false red: hiding the
//      felt changes what a translucent, blurred pill SAMPLES, so the pixels the
//      figure sits on change and part of its ink measures as suppressed while
//      nothing is on top of it at all. Measured 14.0% on `table-preflop` mobile.
//      The scene must stay GREEN and the disagreement must be reported.
// ---------------------------------------------------------------------------
scenario(
  'a background under a backdrop-filter pill cannot fail the scene, and is reported',
  `<div class="felt" style="position:absolute;left:0;top:0;width:420px;height:200px;
        background:radial-gradient(circle at 40% 40%, #2d7a52, #0d3a26)"></div>
   <div class="pill" style="position:absolute;left:40px;top:60px;padding:6px 14px;border-radius:99px;
        background:rgba(6,14,11,0.78);backdrop-filter:blur(6px)">
     <span class="money" style="color:#f8d97a;font-size:26px;font-weight:800">0.20</span>
   </div>`,
  (r) => {
    const dis = r.checks.paintOrderModelDisagreements;
    return {
      pass: r.ok && r.checks.counts.occlusionsFound === 0,
      detail: `disagreements reported: ${dis.length}`
        + (dis.length ? ` (${(dis[0].coveredInkFraction * 100).toFixed(1)}% by ${dis[0].occluder.split(' > ').pop()})` : ''),
    };
  },
);

// ---------------------------------------------------------------------------
// 11. A CLEAN PAGE. Money, equity and cards with nothing over them: the gate must
//     be silent. A gate that cannot be green on a correct page is useless.
// ---------------------------------------------------------------------------
scenario(
  'a page where nothing overlaps anything is GREEN',
  `<span class="money" style="position:absolute;left:20px;top:20px">12.34</span>
   <span class="equity-badge" style="position:absolute;left:20px;top:60px;background:#7ee2b8;
         color:#06231a">100.00%</span>
   <div class="player-cards" style="position:absolute;left:20px;top:100px;display:flex;gap:6px">
     <div class="card"><div class="card-front"><span class="rank">A</span><span class="pip">&hearts;</span></div></div>
     <div class="card"><div class="card-front"><span class="rank">K</span><span class="pip">&spades;</span></div></div>
   </div>`,
  (r) => ({
    pass: r.ok && r.checks.counts.occlusionsFound === 0 && r.checks.counts.figuresOnScreen >= 7,
    detail: `${r.checks.counts.figuresOnScreen} figures, ${r.checks.counts.occlusionsFound} occlusions, `
      + `${r.checks.counts.pairsPixelTested} pairs tested`,
  }),
);

// ---------------------------------------------------------------------------
// 12. THE PAGE IS LEFT EXACTLY AS IT WAS FOUND. The probe tags elements and
//     injects a stylesheet; if either survived, every PNG after it would be of a
//     page the harness had modified.
// ---------------------------------------------------------------------------
scenario(
  'the probe leaves no attribute and no stylesheet behind',
  `<span class="money" style="position:absolute;left:20px;top:20px;background:#111">12.34</span>
   <div class="lid" style="position:absolute;left:20px;top:20px;width:30px;height:24px;
        background:#e8543a;z-index:5"></div>`,
  (r, page) => ({ pass: true, detail: '', deferred: true, page }),
);

const browser = await chromium.launch();
let failed = 0;
try {
  for (const c of cases) {
    const { page, context } = await page$(browser, c.body);
    if (c.opts?.scrollTo) {
      await page.evaluate((y) => window.scrollTo(0, y), c.opts.scrollTo);
      await page.evaluate(() => new Promise((res) => requestAnimationFrame(() => res(undefined))));
    }
    const r = await assertNothingCoversAFigure(page, {
      scene: `selftest-${cases.indexOf(c) + 1}`, viewport: 'fixture', outDir: OUT,
    });
    let verdict;
    if (c.name.startsWith('the probe leaves no')) {
      const leftovers = await page.evaluate(() => ({
        ids: document.querySelectorAll('[data-occl-id]').length,
        hidden: document.querySelectorAll('[data-occl-hide]').length,
        style: document.getElementById('occl-probe-style') ? 1 : 0,
      }));
      verdict = {
        pass: leftovers.ids === 0 && leftovers.hidden === 0 && leftovers.style === 0,
        detail: JSON.stringify(leftovers),
      };
    } else {
      verdict = c.expect(r);
    }
    results.push({ name: c.name, ...verdict, notes: r.notes });
    await context.close();
  }
} finally {
  await browser.close();
}

for (const r of results) {
  if (!r.pass) failed += 1;
  console.log(`${r.pass ? 'ok  ' : 'FAIL'} ${r.name}`);
  if (r.detail) console.log(`       ${r.detail}`);
  if (!r.pass) console.log(`       notes: ${r.notes}`);
}
fs.rmSync(OUT, { recursive: true, force: true });
console.log(
  failed === 0
    ? `\nALL ${results.length} PIXEL-GATE CASES PASS (ink threshold ${(INK_FRACTION_THRESHOLD * 100).toFixed(0)}%)`
    : `\n${failed} of ${results.length} PIXEL-GATE CASES FAILED`,
);
process.exit(failed === 0 ? 0 : 1);
