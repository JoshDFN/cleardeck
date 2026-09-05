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
 * Svelte action: `<div class="actions" use:pinAfter={{ gates: ['.solvency', '.runway-notice'], scroller: '.modal-body' }}>`.
 * Toggles `pinned` on the node from the scroller's scroll and the window's
 * resize, read on the next frame; a gate that is not in the DOM (the solvency
 * block when the canister says covered) is simply not a gate. Does nothing
 * where the phone media query does not match, so the desktop dialog keeps
 * its flow row.
 *
 * @param {HTMLElement} node
 * @param {{gates: string[], scroller: string, pinnedClass?: string}} params
 */
export function pinAfter(node, params) {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return {};
  const media = window.matchMedia(PHONE_MEDIA);
  const pinnedClass = params?.pinnedClass ?? 'pinned';
  const gates = Array.isArray(params?.gates) ? params.gates : [];
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
  schedule();

  return {
    update(next) {
      if (Array.isArray(next?.gates)) gates.splice(0, gates.length, ...next.gates);
      schedule();
    },
    destroy() {
      if (frame) window.cancelAnimationFrame(frame);
      scroller?.removeEventListener('scroll', schedule);
      window.removeEventListener('resize', schedule);
      media.removeEventListener?.('change', schedule);
    },
  };
}
