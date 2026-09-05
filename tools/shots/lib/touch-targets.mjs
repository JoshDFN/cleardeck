// CAN A THUMB HIT EVERY CONTROL? AND CAN AN EYE READ EVERY FIGURE?
//
// The audit measured every phone tap target except the five action buttons at
// 24-36 px (Apple's floor is 44 pt, Material's 48 dp) and the action clock at
// 13 px of type plus a 3 px line. `table-in-frame.mjs` RECORDS the smallest
// action button; nothing asserted the rest. This module measures every
// interactive element on the page and folds the measurement into a verdict.
//
// WHAT "44 PX" MEANS HERE. Not the painted box alone: a control may paint a
// 30 px chip and grow its hit area with a pseudo-element (the empty seat's
// `.join-seat::before`, the time bank pill), and Material's dense toolbars do
// the same. A pseudo-element has no bounding rectangle, so the EFFECTIVE target
// is measured the way a thumb finds it: `elementFromPoint` at the centre and at
// the four points 21 px out along each axis. All five must resolve to the
// control (or a descendant). A control whose painted box is already 44x44 but
// whose probes hit something else is COVERED by a neighbour, which is a worse
// failure than a small box, and it fails here too.
//
// Off-viewport controls (the lobby's rows below the fold) cannot be probed with
// elementFromPoint; for those the painted box alone decides.

export const TOUCH_MIN_PX = 44;

/**
 * Type floors, CSS px. The felt tokens floor every on-felt label at 11 px
 * (index.scss `--cd-felt-label`); these are the figures a player reads at
 * arm's length and the sizes below which the audit called them unreadable.
 */
export const TYPE_FLOORS = [
  { selector: '.seat .player-name', label: 'seat name', minPx: 11 },
  { selector: '.seat .chips', label: 'seat stack', minPx: 13 },
  { selector: '.seat .bet-amount, .bet-amount', label: 'bet amount', minPx: 11 },
  { selector: '.player-cards .card .rank, .community-cards .card .rank', label: 'card rank', minPx: 11 },
  { selector: '.turn-timer', label: 'action clock digits', minPx: 11 },
  { selector: '.pot-amount', label: 'pot figure', minPx: 16 },
  { selector: '.wallet-panel .balance-value, .collapsed-balance', label: 'table balance', minPx: 13 },
  { selector: '.actions .action-btn', label: 'action button label', minPx: 13 },
];

/** Selector for everything a finger can operate. */
export const INTERACTIVE_SELECTOR = [
  'button', 'a[href]', 'input', 'select', 'textarea', 'summary',
  '[role="button"]', '[role="link"]', '[role="tab"]', '[role="menuitem"]', '[role="switch"]',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

/**
 * Measures every interactive element and the type floors.
 *
 * @param {import('playwright').Page} page
 * @param {{touchMin?: number, typeFloors?: Array<{selector:string,label:string,minPx:number}>, scope?: string|null}} [opts]
 *   `scope`: measure only the controls inside this selector (an opened
 *   sheet or menu), since an open layer legitimately covers what is under it.
 */
export function measureTouchTargets(page, opts = {}) {
  const touchMin = opts.touchMin ?? TOUCH_MIN_PX;
  const floors = opts.typeFloors ?? TYPE_FLOORS;
  const scope = opts.scope ?? null;
  return page.evaluate(({ selector, touchMin, floors, scope }) => {
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const box = (el) => {
      const r = el.getBoundingClientRect();
      return {
        x: +r.x.toFixed(1), y: +r.y.toFixed(1),
        w: +r.width.toFixed(1), h: +r.height.toFixed(1),
        right: +r.right.toFixed(1), bottom: +r.bottom.toFixed(1),
      };
    };
    // A CONTROL SCROLLED OUT OF ITS OWN SCROLLER IS NOT A TARGET. The cashier
    // sheet's body is the scroller under a fixed header; with the body
    // scrolled to its end the amount field sits above the body's top edge,
    // clipped away, and checkVisibility() still says visible (it knows
    // nothing about overflow clipping). A probe there hits the header and
    // the field was reported "covered by a neighbour" (the cashier wave's
    // first touch run). So an element whose box no longer intersects an
    // overflow-clipping ancestor's box is not painted.
    const clippedAway = (el) => {
      const r = el.getBoundingClientRect();
      for (let n = el.parentElement; n && n !== document.documentElement; n = n.parentElement) {
        const o = getComputedStyle(n);
        const clips = (v) => v === 'hidden' || v === 'auto' || v === 'scroll' || v === 'clip';
        if (!clips(o.overflowX) && !clips(o.overflowY)) continue;
        const c = n.getBoundingClientRect();
        if (r.bottom <= c.top + 0.5 || r.top >= c.bottom - 0.5 || r.right <= c.left + 0.5 || r.left >= c.right - 0.5) return true;
      }
      return false;
    };
    const painted = (el) => {
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) return false;
      if (clippedAway(el)) return false;
      if (typeof el.checkVisibility === 'function') {
        return el.checkVisibility({ visibilityProperty: true, opacityProperty: true });
      }
      const s = getComputedStyle(el);
      return s.display !== 'none' && s.visibility !== 'hidden' && Number(s.opacity) > 0.05;
    };
    const path = (el) => {
      const parts = [];
      for (let n = el, d = 0; n && d < 3 && n !== document.body; n = n.parentElement, d += 1) {
        parts.unshift(`${n.tagName.toLowerCase()}${[...n.classList]
          .filter((c) => !c.startsWith('svelte-')).slice(0, 2).map((c) => `.${c}`).join('')}`);
      }
      return parts.join(' > ');
    };
    const text = (el) => {
      const own = (el.getAttribute('aria-label') || el.getAttribute('title') || el.textContent || '')
        .replace(/\s+/g, ' ').trim();
      return own.slice(0, 40);
    };

    // A dialog on screen: only what is inside it (and the page's fixed toast)
    // can be tapped; everything under the backdrop is out of reach by design.
    const dialog = document.querySelector('[role="dialog"], .modal-content, .verify-modal');
    const backdrop = document.querySelector('.modal-backdrop');
    // The sideways phone: the rotate prompt stands over the whole table area,
    // so nothing under it is a target until the phone turns.
    const rotate = document.querySelector('.rotate-prompt');
    const rotateUp = !!rotate && painted(rotate);
    const scopeEl = scope ? document.querySelector(scope) : null;
    const reachable = (el) => {
      if (scopeEl && !scopeEl.contains(el)) return false;
      if (rotateUp && el.closest('.table-area') && el !== rotate) return false;
      if (!dialog || !backdrop) return true;
      return dialog.contains(el) || el.closest('.toast') || el.closest('.alpha-warning-banner.expanded');
    };
    // A probe that lands on the label wrapping an input has found the input:
    // the label is the control's own hit area.
    const owns = (el, at) => {
      if (!at) return false;
      if (at === el || el.contains(at)) return true;
      const label = el.closest('label');
      return !!label && (label === at || label.contains(at));
    };

    const half = touchMin / 2 - 1;
    const items = [];
    for (const el of document.querySelectorAll(selector)) {
      if (!painted(el)) continue;
      if (!reachable(el)) continue;
      const b = box(el);
      const cx = b.x + b.w / 2;
      const cy = b.y + b.h / 2;
      const inViewport = b.x >= 0 && b.y >= 0 && b.right <= vw && b.bottom <= vh;
      const boxOk = b.w >= touchMin - 0.5 && b.h >= touchMin - 0.5;
      // A probe that would land outside the viewport (an edge control, or a
      // control at the bottom of a scrolled page) cannot be hit-tested; that
      // direction is judged on the painted box alone.
      const inside = (x, y) => x >= 0.5 && x <= vw - 0.5 && y >= 0.5 && y <= vh - 0.5;
      const centreInside = inside(cx, cy);
      const probes = centreInside
        ? [[cx, cy, true], [cx - half, cy, b.x <= cx - half], [cx + half, cy, b.right >= cx + half],
          [cx, cy - half, b.y <= cy - half], [cx, cy + half, b.bottom >= cy + half]]
        : null;
      // A probe within 1.5 px of the element's own edge can land on the
      // neighbour through sub-pixel rounding (a 690.8 px top edge probed at
      // 691.8); that is rounding, not a covered control, and it is judged on
      // the box. A real overlap covers far more than 1.5 px.
      const nearOwnEdge = (x, y) => x >= b.x && x <= b.right && y >= b.y && y <= b.bottom
        && Math.min(x - b.x, b.right - x, y - b.y, b.bottom - y) < 1.5;
      const hits = probes
        ? probes.map(([x, y, boxHas]) => {
          if (!inside(x, y)) return boxHas;
          return owns(el, document.elementFromPoint(x, y)) || nearOwnEdge(x, y);
        })
        : null;
      const effectiveOk = hits ? hits.every(Boolean) : boxOk;
      const disabled = el.matches(':disabled') || el.getAttribute('aria-disabled') === 'true';
      items.push({
        path: path(el),
        text: text(el),
        tag: el.tagName.toLowerCase(),
        type: el.getAttribute('type'),
        box: b,
        inViewport,
        probed: !!hits,
        hits: hits ? hits.map((h) => (h ? 'y' : 'n')).join('') : null,
        boxOk,
        effectiveOk,
        disabled,
        // Small box AND probes hit: a pseudo-element or a parent grows it.
        ok: effectiveOk,
      });
    }

    // Under the rotate prompt nothing on the felt is readable by design; the
    // floors are measured on the portrait table.
    const type = (rotateUp ? [] : floors).map((f) => {
      const els = [...document.querySelectorAll(f.selector)].filter(painted);
      const sizes = els.map((el) => parseFloat(getComputedStyle(el).fontSize));
      const min = sizes.length ? Math.min(...sizes) : null;
      return {
        label: f.label, selector: f.selector, minPx: f.minPx, count: els.length,
        smallestPx: min === null ? null : +min.toFixed(1),
        ok: min === null || min >= f.minPx - 0.05,
      };
    });

    // THE POT MODULE NEVER STANDS ON A PLATE. The side-pot pills sit in the
    // band between the board and the lower flank plates; a pill that reaches
    // a plate covers no money figure (the occlusion gate is silent) but reads
    // as a collision to a player. Every painted `.side-pot` box against every
    // painted `.player-nameplate` box.
    const intersects = (p, q) => p.x < q.right && q.x < p.right && p.y < q.bottom && q.y < p.bottom;
    const plates = [...document.querySelectorAll('.player-nameplate')].filter(painted).map((el) => ({ el, box: box(el) }));
    const collisions = [];
    for (const pill of [...document.querySelectorAll('.side-pot')].filter(painted)) {
      const pb = box(pill);
      for (const plate of plates) {
        if (!intersects(pb, plate.box)) continue;
        collisions.push({
          pill: text(pill), pillBox: pb,
          plate: text(plate.el.querySelector('.player-name') || plate.el), plateBox: plate.box,
        });
      }
    }

    // A STICKY ROW NEVER STANDS ON A MONEY FIGURE. The phone cashier pins its
    // Deposit row to the sheet's foot (DepositModal.svelte's phone block), the
    // one pinned control in the app; the round that first tried it measured
    // the row over the solvency advice and dropped it. So: every painted
    // element inside the dialog whose computed position is sticky, against
    // every painted money surface in the dialog it does not itself contain.
    // Measured at rest and again with the body scrolled to its end
    // (touch-targets.mjs's "scrolled to end" state).
    const MONEY_IN_DIALOG = [
      '.solvency .figures dd', '.solvency .advice', '.balance-crypto', '.balance-value', '.usd-value',
      '.minimum-notice', '.conversion-preview', '.cd-money', 'input[type="number"]',
    ].join(', ');
    // ...AND NEVER ON A DISCLOSURE. The row pins only after the warnings have
    // been seen (lib/pin-after.js); the cashier wave's second round photographed
    // it over the runway panel's "Do not deposit" headline, which the money
    // list above cannot see (a headline is not a figure). So every painted
    // disclosure block in the dialog, whole, against every sticky element.
    const DISCLOSURES_IN_DIALOG = [
      '.solvency', '.runway-notice', '.custody-notice', '.network-line', '.untrusted-table',
    ].join(', ');
    const stickyCover = [];
    const stickyOverDisclosure = [];
    if (dialog) {
      const stickies = [...dialog.querySelectorAll('*')]
        .filter((el) => painted(el) && getComputedStyle(el).position === 'sticky');
      const figures = [...dialog.querySelectorAll(MONEY_IN_DIALOG)].filter(painted);
      const disclosures = [...dialog.querySelectorAll(DISCLOSURES_IN_DIALOG)].filter(painted);
      for (const st of stickies) {
        const sb = box(st);
        for (const fig of figures) {
          if (st.contains(fig) || fig.contains(st)) continue;
          const fb = box(fig);
          if (!intersects(sb, fb)) continue;
          stickyCover.push({ sticky: text(st), stickyBox: sb, figure: text(fig), figureBox: fb });
        }
        for (const block of disclosures) {
          if (st.contains(block) || block.contains(st)) continue;
          const bb = box(block);
          if (!intersects(sb, bb)) continue;
          stickyOverDisclosure.push({ sticky: text(st), stickyBox: sb, block: text(block), blockBox: bb });
        }
      }
    }

    return {
      viewport: { w: vw, h: vh },
      coarsePointer: window.matchMedia('(pointer: coarse)').matches,
      documentScrollWidth: document.documentElement.scrollWidth,
      documentScrollHeight: document.documentElement.scrollHeight,
      scrollLocked: document.documentElement.classList.contains('cd-scroll-lock'),
      dialogOpen: !!(dialog && backdrop),
      rotatePromptVisible: rotateUp,
      items,
      type,
      collisions,
      stickyCover,
      stickyOverDisclosure,
    };
  }, { selector: INTERACTIVE_SELECTOR, touchMin, floors, scope });
}

/**
 * Folds one measurement into a verdict fragment.
 *
 * @param {object|null} m result of `measureTouchTargets`
 * @param {{scene:string, viewport:string, touchMin?:number, expectDialogLock?:boolean, expectRotatePrompt?:boolean, expectNoPageScroll?:boolean}} where
 *   `expectNoPageScroll`: the table view is one screen on a phone
 *   (routes/app-phone.scss), so the document may be no taller than the
 *   viewport; a taller one is the scroll trap the audit measured at 1040 px.
 */
export function foldTouchTargets(m, where) {
  const touchMin = where.touchMin ?? TOUCH_MIN_PX;
  if (!m) return { ok: false, problems: ['no measurement (the page did not answer)'], notes: '', scene: where.scene, viewport: where.viewport };
  const problems = [];
  const items = Array.isArray(m.items) ? m.items : [];
  if (items.length === 0) {
    problems.push('no interactive element was measured: a probe that walks nothing passes nothing');
  }
  for (const it of items) {
    if (it.ok) continue;
    const size = `${it.box.w}x${it.box.h} at (${it.box.x},${it.box.y})`;
    if (it.probed && it.boxOk) {
      problems.push(`"${it.text}" (${it.path}) is ${size} but a probe at the touch floor hits something else (${it.hits}): covered by a neighbour`);
    } else if (it.probed) {
      problems.push(`"${it.text}" (${it.path}) is ${size}, under the ${touchMin} px floor, and no hit area grows it (${it.hits})`);
    } else {
      problems.push(`"${it.text}" (${it.path}) is ${size}, under the ${touchMin} px floor (off viewport, box only)`);
    }
  }
  for (const t of m.type || []) {
    if (!t.ok) problems.push(`${t.label} renders at ${t.smallestPx} px, under its ${t.minPx} px floor (${t.count} elements)`);
  }
  if (m.documentScrollWidth > m.viewport.w) {
    problems.push(`the document is ${m.documentScrollWidth} px wide in a ${m.viewport.w} px viewport (horizontal scroll)`);
  }
  if (where.expectNoPageScroll && m.documentScrollHeight > m.viewport.h + 1) {
    problems.push(`the document is ${m.documentScrollHeight} px tall in a ${m.viewport.h} px viewport: the table view scrolls (a flick on the felt moves the action row)`);
  }
  for (const c of m.collisions || []) {
    problems.push(`the side-pot pill "${c.pill}" (${c.pillBox.w}x${c.pillBox.h} at ${c.pillBox.x},${c.pillBox.y}) stands on the plate of "${c.plate}" (${c.plateBox.w}x${c.plateBox.h} at ${c.plateBox.x},${c.plateBox.y})`);
  }
  for (const c of m.stickyCover || []) {
    problems.push(`the sticky row "${c.sticky}" (${c.stickyBox.w}x${c.stickyBox.h} at ${c.stickyBox.x},${c.stickyBox.y}) stands on the money figure "${c.figure}" (${c.figureBox.w}x${c.figureBox.h} at ${c.figureBox.x},${c.figureBox.y})`);
  }
  for (const c of m.stickyOverDisclosure || []) {
    problems.push(`the sticky row "${c.sticky}" (${c.stickyBox.w}x${c.stickyBox.h} at ${c.stickyBox.x},${c.stickyBox.y}) stands on the disclosure "${c.block}" (${c.blockBox.w}x${c.blockBox.h} at ${c.blockBox.x},${c.blockBox.y}): it pinned before that warning was read`);
  }
  if (where.expectDialogLock && m.dialogOpen && !m.scrollLocked) {
    problems.push('a dialog is open and the page behind it is not scroll-locked (html.cd-scroll-lock missing)');
  }
  if (where.expectRotatePrompt !== undefined && m.rotatePromptVisible !== where.expectRotatePrompt) {
    problems.push(`the rotate prompt is ${m.rotatePromptVisible ? 'showing' : 'absent'}; expected it ${where.expectRotatePrompt ? 'showing' : 'absent'}`);
  }
  const under = items.filter((i) => !i.ok).length;
  const smallest = items.reduce((acc, i) => {
    const s = Math.min(i.box.w, i.box.h);
    return acc === null || s < acc.s ? { s, text: i.text } : acc;
  }, null);
  const notes = `${items.length} interactive elements, ${under} under the ${touchMin} px floor`
    + (smallest ? `; smallest painted box ${smallest.s} px ("${smallest.text}")` : '')
    + `; document ${m.documentScrollWidth}x${m.documentScrollHeight} in ${m.viewport.w}x${m.viewport.h}`
    + `; type floors ${(m.type || []).filter((t) => !t.ok).length ? 'BROKEN' : 'met'}`
    + ((m.collisions || []).length ? `; ${m.collisions.length} pot pill(s) on a plate` : '')
    + ((m.stickyCover || []).length ? `; ${m.stickyCover.length} money figure(s) under a sticky row` : '')
    + ((m.stickyOverDisclosure || []).length ? `; ${m.stickyOverDisclosure.length} disclosure(s) under a sticky row` : '')
    + (m.dialogOpen ? `; dialog open, scroll ${m.scrollLocked ? 'locked' : 'NOT locked'}` : '')
    + (m.rotatePromptVisible ? '; rotate prompt showing' : '');
  return { ok: problems.length === 0, measurement: m, problems, notes, scene: where.scene, viewport: where.viewport };
}
