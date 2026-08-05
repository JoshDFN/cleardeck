// THE IN-PAGE HALF OF THE PIXEL GATE: what is on screen, and what is over it.
//
// This module is the part that runs INSIDE the browser (`page.evaluate`), split
// out of occlusion.mjs so neither half is an 800-line file. It answers three
// questions and returns plain data:
//
//   1. WHAT IS A FIGURE. Every element carrying a money amount, an equity
//      percentage, or a card's face / rank / pip, minus anything the app itself
//      declares decorative with `aria-hidden`.
//   2. EFFECTIVE PAINT ORDER. CSS 2.1 Appendix E simulated over the real
//      computed styles, so "is above" is one integer comparison that is right
//      ACROSS stacking contexts. A naive z-index comparison is wrong in both
//      directions here and the numbers in the run manifest prove it.
//   3. WHAT INTERSECTS EACH FIGURE, and what a hit test answers at points
//      across it.
//
// It decides nothing. The verdict is the four-shot pixel differential in
// occlusion.mjs, because a stacking-context subtlety, an occluder with
// `pointer-events: none` or an overlap that covers only padding all defeat
// reasoning about the DOM.
//
// `scanInPage` MUST stay self-contained: Playwright serialises the function
// source, so anything it referenced from this module's scope would be undefined
// inside the page.

/**
 * The scan that runs inside the page: paint order, figures, candidates, hit
 * tests. Every element it cares about is tagged with `data-occl-id` so the Node
 * side can address it for the pixel differential.
 *
 * @param {object} cfg
 */
/* eslint-disable no-restricted-globals */
export function scanInPage(cfg) {
  const MONEY = new RegExp(cfg.moneyPattern);
  const PERCENT = new RegExp(cfg.percentPattern);
  const styles = new Map();
  const cs = (el) => {
    let s = styles.get(el);
    if (s === undefined) { s = getComputedStyle(el); styles.set(el, s); }
    return s;
  };

  // ---------------------------------------------------------------------------
  // EFFECTIVE PAINT ORDER (CSS 2.1 Appendix E, simulated)
  // ---------------------------------------------------------------------------
  const zOf = (s) => {
    if (s.zIndex === 'auto') return null;
    const n = parseInt(s.zIndex, 10);
    return Number.isFinite(n) ? n : null;
  };
  const isPositioned = (s) => s.position !== 'static';
  const set = (v) => Boolean(v) && v !== 'none' && v !== 'normal';

  function createsStackingContext(el, s) {
    if (el === document.documentElement) return true;
    if (s.position === 'fixed' || s.position === 'sticky') return true;
    if (isPositioned(s) && zOf(s) !== null) return true;
    const parent = el.parentElement;
    if (parent && zOf(s) !== null && /(flex|grid)/.test(cs(parent).display)) return true;
    if (parseFloat(s.opacity) < 1) return true;
    if (set(s.transform) || set(s.scale) || set(s.rotate) || set(s.translate)) return true;
    if (set(s.filter) || set(s.backdropFilter) || set(s.perspective)) return true;
    if (set(s.clipPath) || set(s.mask) || set(s.maskImage)) return true;
    if (s.isolation === 'isolate') return true;
    if (set(s.mixBlendMode)) return true;
    if (s.contain && /(paint|layout|strict|content)/.test(s.contain)) return true;
    // container-type: inline-size implies layout containment, which is a
    // stacking context. `.table-inner` is a query container, so getting this
    // wrong would mis-order the whole felt.
    if (s.containerType && s.containerType !== 'normal') return true;
    if (s.willChange && /(transform|opacity|filter|perspective|z-index)/.test(s.willChange)) return true;
    if (s.contentVisibility && s.contentVisibility !== 'visible') return true;
    if (s.viewTransitionName && s.viewTransitionName !== 'none') return true;
    return false;
  }

  const paintIndex = new Map();
  let paintCounter = 0;
  const assign = (el) => { if (!paintIndex.has(el)) paintIndex.set(el, paintCounter++); };

  /**
   * Partitions a subtree into the layers of ONE stacking context.
   *
   * `group` collects the non-positioned in-flow content painted atomically with
   * the element that owns it. Positioned `z-index: auto` descendants form their
   * own group (a "pseudo stacking context"): their non-positioned content is
   * atomic with them, but their positioned and context-forming descendants
   * ESCAPE to this stacking context, in tree order, exactly as the spec says.
   */
  function collect(el, lists, group) {
    for (const child of el.children) {
      const s = cs(child);
      if (s.display === 'none') continue;
      if (createsStackingContext(child, s)) {
        const z = zOf(s) ?? 0;
        const entry = { el: child, z, seq: lists.seq++, kind: 'context' };
        if (z < 0) lists.neg.push(entry);
        else if (z > 0) lists.pos.push(entry);
        else lists.zero.push(entry);
        continue;
      }
      if (isPositioned(s)) {
        const own = [];
        lists.zero.push({ el: child, z: 0, seq: lists.seq++, kind: 'pseudo', group: own });
        collect(child, lists, own);
        continue;
      }
      group.push(child);
      collect(child, lists, group);
    }
  }

  function paintContext(el) {
    assign(el); // step 1: this element's own background and borders
    const lists = { neg: [], zero: [], pos: [], seq: 0 };
    const group = [];
    collect(el, lists, group);
    lists.neg.sort((a, b) => a.z - b.z || a.seq - b.seq);
    lists.pos.sort((a, b) => a.z - b.z || a.seq - b.seq);
    for (const e of lists.neg) paintContext(e.el);
    // Steps 3-5 (block backgrounds, floats, inline content) are walked as ONE
    // tree-order pass. The spec paints all non-positioned block backgrounds
    // before any inline content, which only matters for non-positioned siblings
    // that overlap each other; nothing here decides a verdict on paint order
    // alone, so the simplification cannot turn a scene red. It is recorded in
    // `model.simplifications` so a reader is not misled about what was modelled.
    for (const child of group) assign(child);
    for (const e of lists.zero) {
      if (e.kind === 'context') paintContext(e.el);
      else { assign(e.el); for (const d of e.group) assign(d); }
    }
    for (const e of lists.pos) paintContext(e.el);
  }

  paintContext(document.documentElement);

  // ---------------------------------------------------------------------------
  // TARGETS: the figures a player reads
  // ---------------------------------------------------------------------------
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const round = (n) => Math.round(n * 100) / 100;

  const visible = (el) => {
    if (!el.isConnected) return false;
    if (typeof el.checkVisibility === 'function') {
      return el.checkVisibility({
        visibilityProperty: true, opacityProperty: true, contentVisibilityAuto: true,
      });
    }
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };

  const describe = (el) => {
    const parts = [];
    let node = el;
    for (let d = 0; node && d < 5 && node !== document.body; d += 1) {
      const cls = [...node.classList].slice(0, 3).map((c) => `.${c}`).join('');
      parts.unshift(`${node.tagName.toLowerCase()}${cls}`);
      node = node.parentElement;
    }
    return parts.join(' > ');
  };

  /** Viewport rect clamped to what is actually on screen. */
  const clamped = (el) => {
    const r = el.getBoundingClientRect();
    const x0 = Math.max(0, Math.floor(r.left));
    const y0 = Math.max(0, Math.floor(r.top));
    const x1 = Math.min(vw, Math.ceil(r.right));
    const y1 = Math.min(vh, Math.ceil(r.bottom));
    return {
      x: x0, y: y0, w: Math.max(0, x1 - x0), h: Math.max(0, y1 - y0),
      offScreen: r.width > 0 && (x1 - x0 < r.width - 1 || y1 - y0 < r.height - 1),
    };
  };

  /**
   * The OVERLAY LAYER an element belongs to, if any.
   *
   * A modal is SUPPOSED to cover the page it is over; a gate that reports "the
   * hand-history dialog covers 37 figures on the table behind it" is reporting
   * the feature, and after the third such run somebody switches the gate off. So
   * an overlay layer is identified structurally -- a `position: fixed` ancestor
   * (or self) that covers at least 40% of the viewport, or anything marked
   * `role="dialog"` / `aria-modal="true"` -- and a figure on the PAGE that an
   * element in an OVERLAY covers is reported as `behind-an-overlay` instead of
   * failing the scene.
   *
   * The test is deliberately one-directional. A figure INSIDE an overlay is
   * gated normally, so a dialog that covers its own numbers still fails, and so
   * does a second overlay covering the first one's.
   */
  const overlayCache = new Map();
  const overlayRootOf = (el) => {
    if (overlayCache.has(el)) return overlayCache.get(el);
    let node = el;
    let root = null;
    while (node && node !== document.body && node !== document.documentElement) {
      const s = cs(node);
      const role = node.getAttribute?.('role');
      if (role === 'dialog' || role === 'alertdialog' || node.getAttribute?.('aria-modal') === 'true') {
        root = node;
      } else if (s.position === 'fixed') {
        const r = node.getBoundingClientRect();
        if (r.width * r.height >= vw * vh * 0.4) root = node;
      }
      node = node.parentElement;
    }
    overlayCache.set(el, root);
    return root;
  };

  /** Text owned by this element directly (not by a child element). */
  const ownText = (el) => [...el.childNodes]
    .filter((n) => n.nodeType === 3)
    .map((n) => n.nodeValue)
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim();

  const all = [...document.body.querySelectorAll('*')];
  const targets = [];
  let idSeq = 0;

  /**
   * ONE PROBE ID PER ELEMENT, EVER.
   *
   * The award chip is both a money FIGURE and the occluder of a CARD (that is
   * exactly docs/DEFECTS.md T-23), and the first draft handed it a target id and
   * then overwrote it with a candidate id. The overwritten id no longer resolved,
   * so hiding "the target" hid nothing, the four screenshots came back identical
   * and the figure was reported as having no ink and no occlusion: a FALSE GREEN
   * on the very defect this gate was built for. Ids are interned.
   */
  const ids = new Map();
  const idOf = (el) => {
    let id = ids.get(el);
    if (!id) {
      id = `e${idSeq++}`;
      ids.set(el, id);
      el.setAttribute('data-occl-id', id);
    }
    return id;
  };

  const classify = (el) => {
    const text = ownText(el);
    if (el.matches(cfg.equitySelector)) return { kind: 'equity', text };
    if (el.matches(cfg.cardFigureSelector)) return { kind: 'card-figure', text };
    if (el.matches(cfg.cardSelector)) return { kind: 'card', text: el.textContent.replace(/\s+/g, ' ').trim() };
    if (!text) return null;
    if (MONEY.test(text)) return { kind: 'money', text };
    if (PERCENT.test(text)) return { kind: 'equity', text };
    return null;
  };

  // Content the app itself declares decorative is not a figure a player reads.
  // `.felt-marks` (the "100% ON-CHAIN · NO RAKE" watermark printed on the felt,
  // under the board by design) is `aria-hidden="true"`, and without this the gate
  // reports the board covering the felt's own watermark on every table scene —
  // true, intended, and not a figure. The skipped list is counted and printed, so
  // this exclusion cannot quietly grow to cover something that matters.
  const decorativeSkipped = [];
  for (const el of all) {
    const what = classify(el);
    if (!what) continue;
    if (!visible(el)) continue;
    if (el.closest('[aria-hidden="true"]')) {
      decorativeSkipped.push({ path: describe(el), text: what.text.slice(0, 60), kind: what.kind });
      continue;
    }
    const rect = clamped(el);
    if (rect.w < 2 || rect.h < 2) continue;
    targets.push({
      id: idOf(el),
      el,
      kind: what.kind,
      text: what.text.slice(0, 60),
      path: describe(el),
      rect,
      pageRect: { x: rect.x + window.scrollX, y: rect.y + window.scrollY, w: rect.w, h: rect.h },
      paintIndex: paintIndex.get(el) ?? -1,
      zIndex: cs(el).zIndex,
      pointerEvents: cs(el).pointerEvents,
      glyphCount: what.text.replace(/\s/g, '').length,
      overlayRoot: (() => { const r = overlayRootOf(el); return r ? describe(r) : null; })(),
    });
  }

  // ---------------------------------------------------------------------------
  // CANDIDATES: what intersects a target and is not part of it
  // ---------------------------------------------------------------------------
  // Every element that paints anything, measured once. Doing this per target
  // would be O(targets x elements) getBoundingClientRect calls and would force a
  // layout flush per figure.
  const boxes = [];
  for (const el of all) {
    const s = cs(el);
    if (s.display === 'none') continue;
    if (!visible(el)) continue;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    boxes.push({ el, r, s, paintIndex: paintIndex.get(el) ?? -1 });
  }

  /** z-index as a NAIVE gate would read it: 'auto' is 0 and context is ignored. */
  const naiveZ = (s) => (s.zIndex === 'auto' ? 0 : (parseInt(s.zIndex, 10) || 0));

  for (const t of targets) {
    const tr = t.rect;
    const tz = naiveZ(cs(t.el));
    const found = [];
    for (const b of boxes) {
      if (b.el === t.el) continue;
      if (b.el.contains(t.el) || t.el.contains(b.el)) continue; // ancestors and descendants
      const ix = Math.max(0, Math.min(tr.x + tr.w, b.r.right) - Math.max(tr.x, b.r.left));
      const iy = Math.max(0, Math.min(tr.y + tr.h, b.r.bottom) - Math.max(tr.y, b.r.top));
      const area = ix * iy;
      if (area < cfg.minIntersectionPx) continue;
      found.push({
        el: b.el,
        path: describe(b.el),
        text: (b.el.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 40),
        rect: { x: round(b.r.left), y: round(b.r.top), w: round(b.r.width), h: round(b.r.height) },
        paintIndex: b.paintIndex,
        zIndex: b.s.zIndex,
        position: b.s.position,
        pointerEvents: b.s.pointerEvents,
        paintsAfterTarget: b.paintIndex > t.paintIndex,
        // What a gate that just compared the two z-index values would conclude.
        naiveZIndexSaysAbove: naiveZ(b.s) >= tz,
        overlayRoot: (() => { const r = overlayRootOf(b.el); return r ? describe(r) : null; })(),
        intersectionPx: Math.round(area),
        intersectionFractionOfTarget: round(area / Math.max(1, tr.w * tr.h)),
      });
    }
    // An intersecting element that CONTAINS another intersecting element is a
    // container (`.felt` covers every badge on it); the specific element is the
    // actionable answer, so containers rank last.
    for (const c of found) {
      c.containsAnotherCandidate = found.some((o) => o !== c && c.el.contains(o.el));
    }
    // Ranked so the pixel budget is spent where an occlusion can actually be:
    // things that paint ABOVE the figure first, specific before container, then
    // by how much they overlap. Elements that paint below are kept at the tail so
    // the paint-order model itself is checked against the pixels a few times per
    // scene instead of being trusted.
    found.sort((a, b) => Number(b.paintsAfterTarget) - Number(a.paintsAfterTarget)
      || Number(a.containsAnotherCandidate) - Number(b.containsAnotherCandidate)
      || b.intersectionPx - a.intersectionPx);
    const above = found.filter((c) => c.paintsAfterTarget).slice(0, cfg.maxCandidates);
    const below = found.filter((c) => !c.paintsAfterTarget).slice(0, cfg.maxBelowCandidates);
    const kept = [...above, ...below];
    for (const c of kept) c.id = idOf(c.el);
    // Which other candidates for THIS figure contain this one. A card covering a
    // badge shows up three times — `.player-cards`, its `.card`, its
    // `.card-front` — for one overlap; the Node side keeps the outermost (the
    // element that actually carries the position and the z-index, i.e. the one a
    // fixer edits) and files the rest under it.
    for (const c of kept) {
      c.containedByCandidateIds = kept.filter((o) => o !== c && o.el.contains(c.el)).map((o) => o.id);
    }
    t.candidates = kept.map(({ el, ...rest }) => rest);
    t.candidatesDropped = found.length - kept.length;
    t.candidatesAbove = found.filter((c) => c.paintsAfterTarget).length;
    t.naiveZIndexCandidates = found.filter((c) => c.naiveZIndexSaysAbove).length;
  }

  // ---------------------------------------------------------------------------
  // HIT TESTING: what answers at points across the target
  // ---------------------------------------------------------------------------
  for (const t of targets) {
    const { x, y, w, h } = t.rect;
    const cols = cfg.hitCols;
    const rows = cfg.hitRows;
    const points = [];
    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        const px = x + ((c + 0.5) / cols) * w;
        const py = y + ((r + 0.5) / rows) * h;
        const hit = document.elementFromPoint(px, py);
        let verdict = 'foreign';
        if (!hit) verdict = 'nothing';
        else if (hit === t.el || t.el.contains(hit)) verdict = 'target';
        else if (hit.contains(t.el)) verdict = 'ancestor';
        points.push({
          x: Math.round(px), y: Math.round(py), verdict, top: hit ? describe(hit) : null,
        });
      }
    }
    const centre = document.elementFromPoint(x + w / 2, y + h / 2);
    const foreign = points.filter((p) => p.verdict === 'foreign');
    const tally = new Map();
    for (const p of foreign) tally.set(p.top, (tally.get(p.top) || 0) + 1);
    t.hit = {
      points: points.length,
      foreign: foreign.length,
      foreignFraction: round(foreign.length / points.length),
      centre: centre ? describe(centre) : null,
      centreIsForeign: Boolean(centre && centre !== t.el && !t.el.contains(centre) && !centre.contains(t.el)),
      topElements: [...tally.entries()].sort((a, b) => b[1] - a[1]).map(([p, n]) => ({ path: p, points: n })),
    };
  }

  return {
    viewport: { w: vw, h: vh, scrollX: window.scrollX, scrollY: window.scrollY },
    elementsPainted: paintCounter,
    decorativeSkipped,
    targets: targets.map(({ el, ...rest }) => rest),
  };
}
/* eslint-enable no-restricted-globals */
