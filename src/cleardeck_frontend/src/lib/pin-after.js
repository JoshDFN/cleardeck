/**
 * A ROW THAT PINS ONLY AFTER THE WARNINGS ABOVE IT HAVE BEEN SEEN.
 *
 * The phone cashier's Deposit row. docs/SECURITY-FINDINGS.md FINDING 23 / 35
 * / 42 put the custody, network and solvency disclosures BEFORE every control
 * that moves money, and tools/shots/scenarios/deposit.mjs measures that
 * geometrically (the solvency block's bottom edge above the primary button)
 * at rest. A row pinned to the sheet's foot from the first screen breaks the
 * rule on the first screen: the button is on screen and the warning is not.
 * A row that is never pinned scrolls away under the alerts and the runway
 * panel once the warnings ARE read.
 *
 * So the row is `position: sticky` only from the moment every gate element
 * (the solvency block, the runway panel) has its bottom edge ABOVE the line
 * the pinned row's top would sit on. Before that it is an ordinary block in
 * flow, below the fold; after that it rides the foot of the sheet and never
 * stands over a figure it was meant to follow. Pure decision here (tested),
 * one Svelte action to apply it.
 */

/**
 * Should the row pin? All gates' bottom edges must be at or above the line
 * where the pinned row's top edge would be (viewport bottom less the row's
 * height). No gate found means never pin: an unmeasured warning is a warning
 * that has not been read.
 *
 * @param {{gateBottoms: number[], rowHeight: number, viewportBottom: number}} m
 *   client-coordinate pixels, as getBoundingClientRect reports them
 * @returns {boolean}
 */
export function shouldPin({ gateBottoms, rowHeight, viewportBottom }) {
  if (!Array.isArray(gateBottoms) || gateBottoms.length === 0) return false;
  if (!Number.isFinite(rowHeight) || !Number.isFinite(viewportBottom)) return false;
  const pinLine = viewportBottom - Math.max(0, rowHeight);
  return gateBottoms.every((b) => Number.isFinite(b) && b <= pinLine + 0.5);
}

/** The phone conditions the money sheets use (money-sheet-phone.scss). */
export const PHONE_MEDIA = '(max-aspect-ratio: 1/1), (max-height: 560px)';

/**
 * Watches the scroller's CONTENT for growth, not only its scroll position.
 *
 * The cashier wave's second round photographed the row pinned OVER the runway
 * panel's headline: the panel is the result of a query that lands a beat after
 * the sheet opens, so it rendered under a row that had already measured the
 * one gate then in the DOM (the solvency block, above the line) and pinned. A
 * row that measures only on scroll and resize cannot see a warning that
 * arrives after it. So every direct child of the scroller is under a
 * ResizeObserver (a panel growing anywhere inside one grows that child), and a
 * MutationObserver re-collects the children when the DOM changes. Both are
 * optional in the environment; without them the action behaves as it did.
 *
 * @param {HTMLElement|null} scroller
 * @param {() => void} onChange
 * @returns {() => void} disconnect
 */
function watchContent(scroller, onChange) {
  if (!scroller) return () => {};
  const hasResize = typeof window.ResizeObserver === 'function';
  const hasMutation = typeof window.MutationObserver === 'function';
  const resize = hasResize ? new window.ResizeObserver(onChange) : null;
  const observeChildren = () => {
    if (!resize) return;
    resize.disconnect();
    for (const child of Array.from(scroller.children)) resize.observe(child);
  };
  const mutation = hasMutation
    ? new window.MutationObserver(() => { observeChildren(); onChange(); })
    : null;
  observeChildren();
  mutation?.observe(scroller, { childList: true, subtree: true, characterData: true });
  return () => {
    resize?.disconnect();
    mutation?.disconnect();
  };
}

/**
 * Svelte action: `<div class="actions" use:pinAfter={{ gates: ['.solvency', '.runway-notice'], scroller: '.modal-body' }}>`.
 * Toggles `pinned` on the node from the scroller's scroll, the window's
 * resize and any growth of the scroller's content, read on the next frame; a
 * gate that is not in the DOM (the solvency block when the canister says
 * covered) is simply not a gate. Does nothing where the phone media query
 * does not match, so the desktop dialog keeps its footer row.
 *
 * @param {HTMLElement} node
 * @param {{gates: string[], scroller: string, pinnedClass?: string}} params
 */
export function pinAfter(node, params) {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return {};
  const media = window.matchMedia(PHONE_MEDIA);
  const pinnedClass = params?.pinnedClass ?? 'pinned';
  // A COPY of the caller's list, never an alias: the action reads `gates` on
  // every frame, and `update` replaces the list with a new array rather
  // than splicing into whatever the caller handed over.
  let gates = Array.isArray(params?.gates) ? [...params.gates] : [];
  const scroller = node.closest(params?.scroller ?? '') ?? node.parentElement;
  let frame = 0;

  const measure = () => {
    frame = 0;
    if (!media.matches) { node.classList.remove(pinnedClass); return; }
    const root = scroller ?? document;
    const found = gates.map((sel) => root.querySelector(sel)).filter(Boolean);
    const gateBottoms = found.map((el) => el.getBoundingClientRect().bottom);
    const viewportBottom = scroller ? scroller.getBoundingClientRect().bottom : window.innerHeight;
    const pin = shouldPin({ gateBottoms, rowHeight: node.getBoundingClientRect().height, viewportBottom });
    node.classList.toggle(pinnedClass, pin);
  };
  const schedule = () => { if (!frame) frame = window.requestAnimationFrame(measure); };

  scroller?.addEventListener('scroll', schedule, { passive: true });
  window.addEventListener('resize', schedule);
  media.addEventListener?.('change', schedule);
  const unwatch = watchContent(scroller, schedule);
  schedule();

  return {
    update(next) {
      if (Array.isArray(next?.gates)) gates = [...next.gates];
      schedule();
    },
    destroy() {
      if (frame) window.cancelAnimationFrame(frame);
      unwatch();
      scroller?.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
      media.removeEventListener?.('change', schedule);
    },
  };
}
