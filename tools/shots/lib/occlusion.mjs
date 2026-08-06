// THE PIXEL GATE: nothing may cover a money figure, an equity figure or a card.
//
// WHY THIS EXISTS
// ---------------
// Every other gate in this repo reads `textContent`. `lib/chain-agreement.mjs`
// asks whether the string equals the canister's number; `lib/token-census.mjs`
// asks whether every string on the page is accounted for. Both are blind to the
// only question a player's eye asks: IS IT ON SCREEN, OR IS SOMETHING ON TOP OF
// IT.
//
// Two defects proved the gap is not theoretical. Both were photographed by this
// harness and both were filed as VERIFIED, because the text was right:
//
//   docs/DEFECTS.md T-22  on a phone the winner's `100.00%` equity badge renders
//                         as a visible `0%` — the hero's own card covers the rest
//                         of it. `.equity-badge` sits inside `.player-nameplate`
//                         (z-index 6, a stacking context) at z-index 3, and the
//                         cards paint at z-index 7. From in there the badge can
//                         never win.
//   docs/DEFECTS.md T-23  the `+24.00` award chip covers the winner's revealed
//                         pair — the cards that justify the award — and on
//                         desktop overlaps `.community-cards` by a quarter of its
//                         own area when the winner sits at seat 1 or 2.
//
// HOW A VERDICT IS REACHED, AND WHY IT IS NOT A z-index COMPARISON
// ----------------------------------------------------------------
// A naive `zIndex(a) > zIndex(b)` gate produces false reds — z-index only orders
// siblings inside one stacking context, and this component nests them four deep
// (`.seat` 10 / `.player-nameplate` 6 / `.player-cards` 7 / `.equity-badge` 3,
// with transforms and container queries opening further contexts). A gate that
// cries wolf gets switched off, which is worse than no gate. So a finding must
// survive three independent stages:
//
//   1. GEOMETRY        the candidate's client rect intersects the target's, in
//                      the visible part of the viewport. Necessary, never
//                      sufficient — most intersections are containers.
//   2. EFFECTIVE PAINT ORDER  CSS 2.1 Appendix E is simulated over the real
//                      computed styles: stacking contexts (transform, opacity,
//                      filter, container-type, contain, fixed/sticky, positioned
//                      with a z-index, flex/grid items with a z-index), the
//                      "pseudo stacking contexts" that positioned `z-index: auto`
//                      elements form, and the tree-order layers for positioned
//                      versus non-positioned descendants. Every element gets a
//                      single integer paint index, so "is above" is one
//                      comparison that is right across contexts.
//   3. OCCLUSION EVIDENCE, which is what actually gates:
//        a. HIT TESTING — `document.elementFromPoint` at a grid of points across
//           the target's rect. What answers at each point is recorded.
//        b. PIXEL DIFFERENTIAL — four clipped screenshots per (target, occluder):
//                A both visible          C target hidden, occluder visible
//                B occluder hidden       D both hidden
//           A pixel belongs to the target when hiding the target changes it, so
//           the target is VISIBLE at p in A iff A[p] != C[p], and visible at p in
//           B iff B[p] != D[p]. The occluder COVERS p iff the target is visible
//           there once the occluder is hidden and not visible while it is shown.
//           Everything is hidden with `visibility: hidden`, which paints nothing
//           and moves nothing, so no reflow can contaminate the comparison.
//
//    Stage 3b is the authority. It cannot be fooled by a stacking-context
//    subtlety, by `pointer-events: none` (which makes an occluder invisible to
//    stage 3a — `.board-cluster` in portrait is exactly that), or by an
//    intersection that covers only padding.
//
// WHAT IS REPORTED, AND WHAT FAILS
// --------------------------------
// The manifest names, per finding: the OCCLUDED element (path, its text, its
// rect), the OCCLUDER (path, its text, its rect), and the FRACTION — of the
// target's own visible ink and of its box — plus the hit-test answers and a
// crop of the figure with and without the occluder, written next to the PNGs.
// A reader can fix it without re-deriving anything.
//
// The threshold is on the target's own INK (the pixels the target actually
// paints), not on its box, and it is 2%:
//   * total occlusion of a money figure is 100% and can never pass;
//   * hiding ONE digit of a seven-glyph badge is ~1/7 of its ink = ~14%, seven
//     times the threshold, so no covered digit can slip through;
//   * a decorative overlap that only touches padding, a shadow or a rounded
//     corner covers no ink at all and measures exactly 0.0%.
// The measured distribution across the full sweep is printed in the manifest
// (`geometryOnlyIntersections`), so the gap between real findings and harmless
// overlap is a number in the artifact rather than a claim in a comment.

import fs from 'node:fs';
import path from 'node:path';
import { REPO_ROOT } from './config.mjs';
import { decodePng, encodePng, pixelDiffers } from './png.mjs';
import { scanInPage } from './occlusion-scan.mjs';

/**
 * Fraction of a figure's own visible ink that may be covered before it fails.
 *
 * TWO NUMBERS, EACH WITH A REASON, BECAUSE THE TWO KINDS OF FIGURE FAIL
 * DIFFERENTLY.
 *
 * `figure` — a money amount, an equity percentage, a card's rank or its suit
 *   pip: 2%. These are read GLYPH BY GLYPH, and half a covered glyph is a
 *   different number, not a missing one — T-22's `100.00%` was read off the
 *   screen as `0%`. One glyph of a seven-glyph badge is ~14% of its ink, so a 2%
 *   budget cannot hide any part of any digit, while an overlap that only touches
 *   padding, a shadow or a rounded corner measures exactly 0.0% and passes. The
 *   sweep's measured near-misses are in `geometryOnlyIntersections`, so the gap
 *   between decoration and a defect is a number in the artifact.
 *
 * `card` — the white FACE of a playing card: 20%. A card is identified by its
 *   rank and its pip, and both are gated separately at 2% as figures in their own
 *   right. What is left is a large blank plate; a chip clipping a corner of it
 *   does not make the card ambiguous, and gating that at 2% would fire on the
 *   hero's own bet disc touching the corner of the hero's own card (measured:
 *   2.8%) — a false red on the first scene, which is how a gate gets deleted.
 *   Above 20% the face itself is being hidden and the card stops reading as a
 *   card, so the ceiling is still there.
 */
export const INK_FRACTION_THRESHOLD = 0.02;
export const CARD_FACE_INK_FRACTION_THRESHOLD = 0.20;

/** The threshold that applies to one target kind. */
export const thresholdFor = (kind) => (kind === 'card'
  ? CARD_FACE_INK_FRACTION_THRESHOLD
  : INK_FRACTION_THRESHOLD);

/** Below this many covered pixels nothing is reported: antialiasing floor. */
export const MIN_COVERED_PIXELS = 8;

/** Per-channel tolerance when deciding two screenshot pixels differ. */
export const CHANNEL_TOLERANCE = 8;

/** Most intersecting candidates pixel-tested per target (highest overlap first). */
const MAX_CANDIDATES_PER_TARGET = 12;

/**
 * How many intersecting elements the paint-order model says are BELOW the figure
 * are pixel-tested anyway. They should never show coverage; testing a few per
 * figure is what turns "the model is right" from an assumption into a measured
 * number (`paintOrderModelDisagreements`).
 */
const MAX_BELOW_CANDIDATES_PER_TARGET = 3;

/**
 * Every amount this client renders goes through `toFixed(...)`, so a money
 * figure ALWAYS carries a decimal point — the same shape invariant
 * lib/token-census.mjs relies on.
 */
const MONEY_PATTERN = '\\d[\\d,]*\\.\\d';
const PERCENT_PATTERN = '\\d(?:[.,]\\d+)?\\s*%';


/** Style rules used to hide a probe subject without moving anything. */
const PROBE_CSS = `
[data-occl-hide="1"], [data-occl-hide="1"] * { visibility: hidden !important; }
`;

async function installProbe(page) {
  await page.evaluate((css) => {
    if (document.getElementById('occl-probe-style')) return;
    const style = document.createElement('style');
    style.id = 'occl-probe-style';
    style.textContent = css;
    document.head.appendChild(style);
  }, PROBE_CSS);
}

/** Leaves the page exactly as it was found. */
async function removeProbe(page) {
  await page.evaluate(() => {
    document.getElementById('occl-probe-style')?.remove();
    for (const el of document.querySelectorAll('[data-occl-hide]')) el.removeAttribute('data-occl-hide');
    for (const el of document.querySelectorAll('[data-occl-id]')) el.removeAttribute('data-occl-id');
  });
}

async function setHidden(page, ids) {
  await page.evaluate((wanted) => {
    for (const el of document.querySelectorAll('[data-occl-hide]')) el.removeAttribute('data-occl-hide');
    for (const id of wanted) {
      document.querySelector(`[data-occl-id="${id}"]`)?.setAttribute('data-occl-hide', '1');
    }
  }, ids);
}

/** One clipped screenshot, decoded. */
async function shot(page, clip) {
  const buf = await page.screenshot({ clip, animations: 'disabled', caret: 'initial' });
  return decodePng(buf);
}

/**
 * The clip Playwright will accept for this figure.
 *
 * THE COORDINATE SPACE IS THE VIEWPORT, NOT THE DOCUMENT. `page.screenshot`
 * with `fullPage: false` renders the visible viewport and then trims the clip
 * against `{0, 0, viewportWidth, viewportHeight}` (Playwright's
 * `trimClipToSize`), throwing "Clipped area is either empty or outside the
 * resulting image" when nothing is left. The first draft added `scrollX/scrollY`
 * to make document coordinates: identical while a page happens to be at scroll
 * zero, and on the deposit scene at mobile -- where the open modal leaves the
 * page scrolled -- it threw. The dangerous version of that mistake is the one
 * that does NOT throw: a shifted clip that still lands inside the viewport
 * measures a DIFFERENT REGION of the page and reports a confident number about
 * it. `inkPixels` is asserted per figure for exactly that reason.
 *
 * The viewport size is re-read here rather than trusted from the scan.
 */
async function clipFor(page, viewportRect) {
  const view = await page.evaluate(() => ({ w: window.innerWidth, h: window.innerHeight }));
  const x0 = Math.max(0, Math.floor(viewportRect.x));
  const y0 = Math.max(0, Math.floor(viewportRect.y));
  const x1 = Math.min(view.w, Math.ceil(viewportRect.x + viewportRect.w));
  const y1 = Math.min(view.h, Math.ceil(viewportRect.y + viewportRect.h));
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

/**
 * Compares four images and reports what the occluder took away from the target.
 *
 * A: both visible   B: occluder hidden   C: target hidden   D: both hidden
 *
 * THE MEASURE IS A SUPPRESSION RATIO, NOT AN ABSOLUTE COLOUR DIFFERENCE.
 * `d(p)` is how much the target changes pixel p when nothing is over it
 * (`|B-D|`), and `dUnder(p)` is how much it still changes it with the occluder
 * present (`|A-C|`). The target paints ink at p when `d(p)` clears the
 * tolerance, and it is COVERED there when the occluder has suppressed at least
 * `1 - SURVIVAL_RATIO` of that contribution.
 *
 * An absolute test was tried first and is wrong in a way that matters: a
 * translucent veil over a figure whose own background is close to the page's
 * pushed `|A-C|` under a fixed tolerance and the whole figure was reported
 * covered, when a player can still read every digit. The ratio is scale-free, so
 * it gives the same answer on a bright badge and on a dark plate.
 */
const SURVIVAL_RATIO = 0.25;

function measureCoverage(A, B, C, D) {
  const n = A.width * A.height;
  let inkPixels = 0;
  let covered = 0;
  let altered = 0;
  const maxDiff = (x, y, i) => {
    const o = i * 4;
    return Math.max(
      Math.abs(x.data[o] - y.data[o]),
      Math.abs(x.data[o + 1] - y.data[o + 1]),
      Math.abs(x.data[o + 2] - y.data[o + 2]),
    );
  };
  for (let i = 0; i < n; i += 1) {
    const d = maxDiff(B, D, i);
    if (d <= CHANNEL_TOLERANCE) continue; // the target paints nothing legible here
    inkPixels += 1;
    const dUnder = maxDiff(A, C, i);
    if (dUnder <= Math.max(2, SURVIVAL_RATIO * d)) covered += 1;
    else if (pixelDiffers(A.data, B.data, i, CHANNEL_TOLERANCE)) altered += 1;
  }
  return {
    pixels: n,
    inkPixels,
    coveredPixels: covered,
    alteredPixels: altered,
    survivalRatio: SURVIVAL_RATIO,
  };
}

const frac = (a, b) => (b > 0 ? Math.round((a / b) * 10000) / 10000 : 0);

/**
 * Retires every crop this (scene, viewport) wrote on an earlier run, BEFORE any
 * new one is written.
 *
 * A CROP IS EVIDENCE. A run that fixed a scene writes no crops at all, so
 * without this the ones from the failing run survive beside a manifest that
 * reports zero findings — 976 of them accumulated while this gate was being
 * built, describing occlusions that no longer existed. Same reasoning as
 * `clearLatestVariants` in capture.mjs.
 */
function clearEvidence(outDir, scene, viewport) {
  if (!fs.existsSync(outDir)) return;
  const prefix = `${scene}-${viewport}-`;
  for (const name of fs.readdirSync(outDir)) {
    if (name.startsWith(prefix)) fs.rmSync(path.join(outDir, name), { force: true });
  }
}

/**
 * Writes the two crops that prove one finding: the figure as a player sees it,
 * and the same rect with the occluder hidden.
 */
function writeEvidence(outDir, base, A, B) {
  fs.mkdirSync(outDir, { recursive: true });
  const asShown = path.join(outDir, `${base}-as-shown.png`);
  const withoutOccluder = path.join(outDir, `${base}-occluder-hidden.png`);
  fs.writeFileSync(asShown, encodePng(A.width, A.height, A.data));
  fs.writeFileSync(withoutOccluder, encodePng(B.width, B.height, B.data));
  return {
    asShown: path.relative(REPO_ROOT, asShown),
    occluderHidden: path.relative(REPO_ROOT, withoutOccluder),
  };
}

/**
 * THE GATE.
 *
 * @param {import('playwright').Page} page
 * @param {{scene:string, viewport:string, outDir:string}} meta
 * @returns {Promise<{ok:boolean, checks:object, notes:string}>}
 */
export async function assertNothingCoversAFigure(page, meta = {}) {
  const reportOnly = process.env.SHOTS_OCCLUSION === 'report';
  const scan = await page.evaluate(scanInPage, {
    moneyPattern: MONEY_PATTERN,
    percentPattern: PERCENT_PATTERN,
    equitySelector: '.equity-badge',
    cardSelector: '.card:not(.empty):not(.face-down)',
    cardFigureSelector: '.card:not(.empty):not(.face-down) .rank, .card:not(.empty):not(.face-down) .pip',
    maxCandidates: MAX_CANDIDATES_PER_TARGET,
    maxBelowCandidates: MAX_BELOW_CANDIDATES_PER_TARGET,
    hitCols: 5,
    hitRows: 3,
    minIntersectionPx: 1,
  });

  await installProbe(page);

  const evidenceDir = meta.outDir
    || path.join(REPO_ROOT, 'artifacts', 'screens', 'latest', 'occlusion');
  clearEvidence(evidenceDir, meta.scene ?? 'scene', meta.viewport ?? 'viewport');

  const findings = [];
  const geometryOnly = [];
  const behindAnOverlay = [];
  const modelDisagreements = [];
  const unmeasurable = [];
  let pairsMeasured = 0;
  let targetsProbed = 0;

  try {
    for (const t of scan.targets) {
      if (!t.candidates.length) continue;
      const clip = await clipFor(page, t.rect);
      if (clip.width < 2 || clip.height < 2) {
        unmeasurable.push({
          path: t.path, text: t.text, rect: t.rect, clip,
          why: 'the figure is outside the viewport\'s current scroll window, so no clipped '
            + 'screenshot of it can be taken; it is not judged either way',
        });
        continue;
      }
      targetsProbed += 1;

      await setHidden(page, []);
      const A = await shot(page, clip);
      await setHidden(page, [t.id]);
      const C = await shot(page, clip);

      const confirmed = [];
      let sawInk = false;
      for (const c of t.candidates) {
        await setHidden(page, [c.id]);
        const B = await shot(page, clip);
        await setHidden(page, [c.id, t.id]);
        const D = await shot(page, clip);
        pairsMeasured += 1;

        if (A.width !== B.width || A.width !== C.width || A.width !== D.width
          || A.height !== B.height || A.height !== C.height || A.height !== D.height) {
          throw new Error('occlusion: the four probe screenshots have different sizes');
        }

        const m = measureCoverage(A, B, C, D);
        if (m.inkPixels > 0) sawInk = true;
        const coveredInkFraction = frac(m.coveredPixels, m.inkPixels);
        const coveredBoxFraction = frac(m.coveredPixels, m.pixels);
        const record = {
          occluded: {
            id: t.id, kind: t.kind, path: t.path, text: t.text, rect: t.rect,
            paintIndex: t.paintIndex, zIndex: t.zIndex, glyphCount: t.glyphCount,
            offScreenPart: t.rect.offScreen,
          },
          occluder: {
            id: c.id, path: c.path, text: c.text, rect: c.rect, zIndex: c.zIndex,
            position: c.position, paintIndex: c.paintIndex, pointerEvents: c.pointerEvents,
            paintsAfterTarget: c.paintsAfterTarget,
            naiveZIndexSaysAbove: c.naiveZIndexSaysAbove,
            containsAnotherCandidate: c.containsAnotherCandidate,
            overlayRoot: c.overlayRoot ?? null,
          },
          geometry: {
            intersectionPx: c.intersectionPx,
            intersectionFractionOfTarget: c.intersectionFractionOfTarget,
          },
          pixels: {
            ...m,
            coveredInkFraction,
            coveredBoxFraction,
            alteredInkFraction: frac(m.alteredPixels, m.inkPixels),
          },
          hitTest: t.hit,
          scene: meta.scene ?? null,
          viewport: meta.viewport ?? null,
        };

        const limit = thresholdFor(t.kind);
        record.threshold = limit;
        // A figure that is not hit-testable ANYWHERE has something over all of
        // it -- unless the figure itself is `pointer-events: none`, in which case
        // hit testing can never reach it and the signal means nothing. Every
        // board card is in that position (`.board-cluster` is pointer-events:
        // none in portrait, and pointer-events inherits), and without this test
        // the rule fired on eight stray pixels of a community card.
        const hitTestable = t.pointerEvents !== 'none';
        const totallyUnhittable = hitTestable && t.hit.foreignFraction === 1;
        const measuredAsOccluded = coveredInkFraction >= limit
          ? m.coveredPixels >= MIN_COVERED_PIXELS
          : totallyUnhittable && m.coveredPixels >= MIN_COVERED_PIXELS;

        // AN ELEMENT THE PAINT MODEL PUTS *BELOW* THE FIGURE CANNOT FAIL A SCENE.
        //
        // The four-shot differential is not sound for an element BEHIND a figure,
        // because hiding a background changes what a TRANSLUCENT foreground
        // samples. `.main-pot` is `rgba(6,14,11,0.78)` with
        // `backdrop-filter: blur(6px)`, so hiding `.felt` changes the pill the
        // pot amount is printed on, and 118 of that figure's 841 ink pixels
        // (14.0%) then measure as suppressed while nothing whatsoever is on top
        // of it. That is a live false red — it appeared on `table-preflop`
        // mobile — and a false red is how a gate gets switched off.
        //
        // Hit testing cannot arbitrate it either: in portrait `.board-cluster` is
        // `pointer-events: none`, so `elementFromPoint` over the pot answers
        // `.felt` at all 15 probe points while the pot is perfectly legible.
        //
        // So this is where the paint-order model is trusted, and the trust is
        // measured rather than assumed: across the sweep 2,895 pairs were
        // pixel-tested and there is no case of an ABOVE-painting element whose
        // pixels contradicted the model. What the concession costs is a figure
        // covered by an element the model wrongly believes is below — bounded,
        // and never silent: every such case is reported here with its fraction as
        // a `paintOrderModelDisagreement`, in the manifest and in the scene notes.
        const belowButMeasured = !c.paintsAfterTarget && measuredAsOccluded;
        if (belowButMeasured) {
          record.verdict = 'below-the-figure';
          record.modelDisagreement = 'the pixels show suppression but the simulated paint order '
            + 'puts this element BELOW the figure, so it cannot be on top of it. Either the '
            + 'paint-order model in lib/occlusion-scan.mjs is wrong here, or -- far more likely '
            + '-- this is the translucency artifact described in lib/occlusion.mjs: hiding a '
            + 'background changes what a translucent foreground samples. Reported, never gated.';
          record.why = `${(coveredInkFraction * 100).toFixed(1)}% of this figure's ink measures as `
            + `suppressed by ${c.path}, which paints BELOW it (paint index ${c.paintIndex} vs `
            + `${t.paintIndex}). Not a failure; see modelDisagreement.`;
          modelDisagreements.push(record);
          geometryOnly.push(record);
          continue;
        }

        // A DIALOG COVERING THE PAGE BEHIND IT IS THE FEATURE, NOT THE DEFECT.
        // Measured, named and counted; never a failure. The converse is not
        // excused: a figure INSIDE an overlay is gated like any other.
        const crossLayer = Boolean(c.overlayRoot) && !t.overlayRoot;
        const fails = measuredAsOccluded && !crossLayer;

        if (measuredAsOccluded && crossLayer) {
          record.verdict = 'behind-an-overlay';
          record.why = `${(coveredInkFraction * 100).toFixed(1)}% of this figure is covered by `
            + `${c.path}, which is inside the overlay ${c.overlayRoot}. A dialog covering the `
            + 'page behind it is deliberate layering, so this is reported and not gated. The '
            + 'figures INSIDE the overlay are gated normally.';
          behindAnOverlay.push(record);
          continue;
        }

        if (fails) {
          record.verdict = 'OCCLUDED';
          record.why = `${(coveredInkFraction * 100).toFixed(1)}% of this figure's own visible ink `
            + `(${m.coveredPixels} of ${m.inkPixels} px, threshold ${(limit * 100).toFixed(0)}%) `
            + `is painted over by ${c.path}`
            + (totallyUnhittable ? '; no point on it answers a hit test' : '');
          confirmed.push({ candidate: c, record, B });
        } else {
          record.verdict = m.coveredPixels > 0 ? 'below-threshold' : 'no-coverage';
          record.why = m.coveredPixels === 0
            ? 'the rects intersect but no pixel of the target is taken away: a container, a '
              + 'sibling behind it, or an overlap that only touches padding'
            : `${m.coveredPixels} px covered = ${(coveredInkFraction * 100).toFixed(2)}% of this `
              + `figure's ink, under the ${(limit * 100).toFixed(0)}% threshold for a ${t.kind}`;
          geometryOnly.push(record);
        }
      }

      // THE CROP HAS TO CONTAIN THE FIGURE. Hiding a figure that is on screen
      // MUST change pixels inside its own clip; if it changes none, either the
      // figure paints nothing (a zero-contrast text colour) or the crop is not
      // looking at it — and a pixel gate that measures the wrong rectangle
      // reports confident nonsense in both directions. Recorded per figure rather
      // than assumed.
      if (!sawInk) {
        unmeasurable.push({
          path: t.path,
          text: t.text,
          rect: t.rect,
          clip,
          why: 'hiding this figure changed no pixel inside its own client rect. It paints nothing '
            + 'measurable there, so nothing is claimed about it either way — and if this appears '
            + 'for a figure that is plainly on screen, the clip arithmetic in lib/occlusion.mjs '
            + 'is wrong and every measurement in this run is suspect.',
        });
      }

      // ONE FINDING PER OVERLAP, NAMING THE ELEMENT A FIXER EDITS.
      //
      // A card covering a badge is confirmed three times over — `.player-cards`,
      // the `.card` inside it, the `.card-front` inside that — for a single
      // overlap of the same pixels. The one worth printing is the OUTERMOST,
      // because that is where the position and the z-index live. An inner
      // candidate is folded into its container when the container accounts for
      // essentially the same pixels; if the inner one covers materially MORE it
      // stands on its own, so nothing is lost by the folding.
      const byId = new Map(confirmed.map((c) => [c.candidate.id, c]));
      const surviving = [];
      for (const item of confirmed) {
        const swallowedBy = (item.candidate.containedByCandidateIds || [])
          .map((id) => byId.get(id))
          .filter(Boolean)
          .find((p) => p.record.pixels.coveredPixels >= item.record.pixels.coveredPixels * 0.9);
        if (swallowedBy) {
          swallowedBy.record.alsoAccountedForBy = [
            ...(swallowedBy.record.alsoAccountedForBy || []),
            {
              path: item.candidate.path,
              coveredInkFraction: item.record.pixels.coveredInkFraction,
              note: 'a descendant of the occluder above, covering the same pixels',
            },
          ];
        } else {
          surviving.push(item);
        }
      }
      surviving.sort((a, b) => b.record.pixels.coveredPixels - a.record.pixels.coveredPixels);
      for (const [i, item] of surviving.entries()) {
        const base = `${meta.scene}-${meta.viewport}-${t.id}-${item.candidate.id}`;
        item.record.evidence = writeEvidence(evidenceDir, base, A, item.B);
        item.record.rank = i + 1;
        findings.push(item.record);
      }
    }
  } finally {
    await setHidden(page, []);
    await removeProbe(page);
  }

  // Worst first, so the manifest, the INDEX row and the console line all quote
  // the same, most serious finding.
  findings.sort((a, b) => b.pixels.coveredInkFraction - a.pixels.coveredInkFraction);

  const byTarget = new Map();
  for (const f of findings) {
    const prev = byTarget.get(f.occluded.id);
    if (!prev || f.pixels.coveredInkFraction > prev.pixels.coveredInkFraction) byTarget.set(f.occluded.id, f);
  }
  const worst = [...byTarget.values()]
    .sort((a, b) => b.pixels.coveredInkFraction - a.pixels.coveredInkFraction);

  const ok = reportOnly || findings.length === 0;

  const checks = {
    mode: reportOnly
      ? 'REPORT ONLY (SHOTS_OCCLUSION=report) — NOT GATING'
      : 'gating',
    scene: meta.scene ?? null,
    viewport: meta.viewport ?? null,
    // Where the page was when it was judged. Occlusion is a fact about the
    // RENDERED viewport, so the scroll position is part of the claim.
    viewportPixels: scan.viewport,
    threshold: {
      moneyEquityAndCardGlyphs: INK_FRACTION_THRESHOLD,
      cardFace: CARD_FACE_INK_FRACTION_THRESHOLD,
      minCoveredPixels: MIN_COVERED_PIXELS,
      channelTolerance: CHANNEL_TOLERANCE,
      why: 'the fraction is of the FIGURE\'S OWN VISIBLE INK, so an overlap that touches only '
        + 'padding, a shadow or a rounded corner measures 0.0%, while hiding one digit of a '
        + 'seven-glyph badge measures ~14% — seven times the threshold. A card\'s blank white '
        + 'face gets 20% because its rank and its pip are gated separately at 2%',
    },
    model: {
      elementsPainted: scan.elementsPainted,
      stages: [
        'geometry: client-rect intersection inside the visible viewport',
        'effective paint order: CSS 2.1 Appendix E simulated over computed styles',
        'hit testing: elementFromPoint on a 5x3 grid across the target',
        'pixel differential: 4 clipped screenshots per (target, occluder) pair',
      ],
      simplifications: [
        'CSS 2.1 steps 3-5 (non-positioned block backgrounds, floats, inline content) are '
        + 'walked as one tree-order pass; this can only mis-order non-positioned siblings that '
        + 'overlap each other, and no verdict rests on paint order alone',
        'a translucent overlay that still lets the figure show through is reported as '
        + 'alteredInkFraction and does NOT fail the scene',
        'an ancestor\'s ::before/::after cannot be hidden independently of the target, so '
        + 'pseudo-element overlays drawn by an ANCESTOR of a figure are outside this gate',
        'an element the paint model puts BELOW the figure can never fail a scene: hiding a '
        + 'background changes what a TRANSLUCENT foreground samples (a backdrop-filter pill over '
        + 'the felt measured 14% suppression with nothing on top of it), and hit testing cannot '
        + 'arbitrate a figure whose container is pointer-events: none. Such a measurement is '
        + 'reported as a paintOrderModelDisagreement with its fraction instead',
        'a figure whose computed `pointer-events` is none can never answer a hit test, so the '
        + '"nothing on it is hit-testable" rule is not applied to one',
      ],
    },
    counts: {
      figuresOnScreen: scan.targets.length,
      // Money/equity/card text inside an `aria-hidden="true"` subtree: the app's
      // own declaration that it is decoration. Listed so the exclusion is
      // auditable rather than invisible.
      decorativeFiguresSkipped: scan.decorativeSkipped.length,
      figuresByKind: scan.targets.reduce((acc, t) => {
        acc[t.kind] = (acc[t.kind] || 0) + 1;
        return acc;
      }, {}),
      figuresWithAnIntersectingElement: targetsProbed,
      pairsPixelTested: pairsMeasured,
      occlusionsFound: findings.length,
      figuresOccluded: byTarget.size,
      geometryOnlyIntersections: geometryOnly.length,
      figuresBehindAnOverlay: new Set(behindAnOverlay.map((f) => f.occluded.id)).size,
      figuresNotMeasurable: unmeasurable.length,
      // THE FALSE-POSITIVE ACCOUNTING, per scene, in the artifact.
      //
      // `naiveZIndexWouldFlag`  what a gate that compared the two z-index values
      //                         and the two rects would have reported. Every one
      //                         of those beyond `occlusionsFound` is a false red
      //                         that would have got the gate switched off.
      // `geometryPlusPaintOrderWouldFlag`  the same with the paint order done
      //                         properly but no pixel evidence: still mostly
      //                         containers and elements that overlap padding.
      naiveZIndexWouldFlag: scan.targets.reduce((n, t) => n + (t.naiveZIndexCandidates || 0), 0),
      geometryPlusPaintOrderWouldFlag: scan.targets.reduce((n, t) => n + (t.candidatesAbove || 0), 0),
      candidatesDroppedByCap: scan.targets.reduce((n, t) => n + (t.candidatesDropped || 0), 0),
      paintOrderModelDisagreements: modelDisagreements.length,
    },
    // Every case where the pixels and the paint-order model disagree, with the
    // measurement. Non-gating by design (see the comment at the decision), and
    // printed so the concession is a number a reader can audit.
    paintOrderModelDisagreements: modelDisagreements.map((f) => ({
      occluded: f.occluded.path,
      text: f.occluded.text,
      occluder: f.occluder.path,
      occluderPaintIndex: f.occluder.paintIndex,
      figurePaintIndex: f.occluded.paintIndex,
      coveredInkFraction: f.pixels.coveredInkFraction,
      note: f.modelDisagreement,
    })),
    findings,
    // Figures the page shows UNDER an open dialog. Not failures; measured so that
    // "the modal dims the player-protection notices to a fifth of their contrast"
    // is a number somebody can read rather than a thing nobody looked at.
    behindAnOverlay: behindAnOverlay
      .sort((a, b) => b.pixels.coveredInkFraction - a.pixels.coveredInkFraction)
      .map((f) => ({
        occluded: f.occluded.path,
        text: f.occluded.text,
        kind: f.occluded.kind,
        overlay: f.occluder.overlayRoot,
        occluder: f.occluder.path,
        coveredInkFraction: f.pixels.coveredInkFraction,
      })),
    figuresNotMeasurable: unmeasurable,
    decorativeFiguresSkipped: scan.decorativeSkipped,
    // Every intersection the gate declined to report, with the measurement that
    // cleared it. This is the list that makes a false red visible: if the gate
    // ever starts firing on decoration, it shows up here first as a near-miss.
    geometryOnlyIntersections: geometryOnly
      .sort((a, b) => b.pixels.coveredPixels - a.pixels.coveredPixels)
      .slice(0, 40),
  };

  const overlayNote = checks.counts.figuresBehindAnOverlay
    ? `, ${checks.counts.figuresBehindAnOverlay} behind an open dialog (not gated)`
    : '';
  const notes = findings.length === 0
    ? `pixel gate: ${scan.targets.length} money/equity/card figures on screen, `
      + `${targetsProbed} with something overlapping them, ${pairsMeasured} pairs pixel-tested, `
      + `0 occluded${overlayNote}`
    : `${reportOnly ? 'OCCLUSION (REPORT ONLY, NOT GATING — SHOTS_OCCLUSION=report): ' : 'OCCLUSION FAILED: '}`
      + `${byTarget.size} of ${scan.targets.length} figures on screen are covered — `
      + worst.slice(0, 3).map((f) => `${(f.pixels.coveredInkFraction * 100).toFixed(1)}% of `
        + `"${f.occluded.text || f.occluded.kind}" (${f.occluded.path.split(' > ').pop()}) by `
        + `${f.occluder.path.split(' > ').pop()}`).join(' | ');

  return { ok, checks, notes };
}
