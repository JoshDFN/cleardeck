// IS THE PLAYING SURFACE STILL THE SIZE THE WAVE SAID IT WAS?
//
// WHY THIS FILE EXISTS. Wave 5's headline is a number: the phone's playing
// surface went from 18.3% of a 390x844 frame to 60.6%. That number was measured
// once, by hand, by its author, with a script in a scratchpad that dies with the
// session. `grep -rn felt tools/shots` before this file returned only scrapers
// and comments: there was no felt assertion anywhere in the harness, so the
// headline could regress to wave 4's geometry with every gate in the repo green.
//
// It pairs with lib/protected-notices.mjs, and the pair is the point. The two
// failure modes are opposite and a gate for one invites the other:
//
//   notices hidden  -> felt goes UP. Caught by protected-notices.mjs.
//   felt shrinks    -> notices stay on screen. Caught here.
//
// Wave 4 shipped the first one and every gate was green. Nothing in the repo
// would have caught the second one at all.
//
// WHAT IT ASSERTS, and what it only records.
//   ASSERTED: the felt exists on a table view, its box is inside the viewport,
//             and its area is at least FELT_AREA_FLOOR of the frame.
//   RECORDED: the exact width, height, area fraction, aspect, the seat-pod count
//             and the ring class. The floor is deliberately loose (a wave-4
//             scale regression is 18.3%, a third of the floor); the RECORD is
//             what a reader compares wave to wave.
//
// The floors are set below the worst legal measured value with margin, because a
// gate that flakes gets switched off and then it is worse than no gate:
//
//   mobile  390x844   measured 60.6% (6-max, table_2) / 50.7% (9-max, table_3)
//   desktop 1440x900  measured 31.7% (6-max)          / 32.3% (9-max)
//
// The desktop and portrait numbers are not comparable to each other: a 2.10
// aspect felt in a 1.6 landscape window cannot fill it, and a 0.555 aspect felt
// in a 0.46 portrait window nearly can. That is geometry, not quality, which is
// why the floor is per viewport and never one number.

/** Area of the frame the felt must cover, per viewport name. */
export const FELT_AREA_FLOOR = {
  mobile: 45.0,
  desktop: 28.0,
};

/**
 * Measures the felt as rendered.
 *
 * `.felt` is the LAYOUT BOX of the visible green surface (PokerTable.svelte
 * draws the rail outside it with box-shadow rings so the rail costs no layout
 * height). Measuring `.poker-table` instead measures the whole stage including
 * the action dock and reads ~68% on a desktop where the surface is 31.7%, which
 * is how a felt figure gets overstated by a factor of two.
 *
 * @param {import('playwright').Page} page
 */
export function measureFelt(page) {
  return page.evaluate(() => {
    const felt = document.querySelector('.felt');
    if (!felt) return null;
    const wrap = document.querySelector('.poker-table-wrapper');
    const inner = document.querySelector('.table-inner');
    const r = felt.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const cs = inner ? getComputedStyle(inner) : null;
    return {
      w: +r.width.toFixed(1),
      h: +r.height.toFixed(1),
      x: +r.x.toFixed(1),
      y: +r.y.toFixed(1),
      viewport: { w: vw, h: vh },
      areaPct: +((r.width * r.height) / (vw * vh) * 100).toFixed(1),
      aspect: +(r.width / r.height).toFixed(3),
      fullyInViewport:
        r.top >= -1 && r.bottom <= vh + 1 && r.left >= -1 && r.right <= vw + 1,
      // The ring the client actually drew. A 6-max table that renders nine pods
      // is mid-flight: `maxPlayers` falls back to 9 until the canister answers
      // (PokerTable.svelte:313), so the first paint of every table is a nine-seat
      // ring and the felt jumps when the real config lands (docs/DEFECTS.md T-32).
      seatPods: document.querySelectorAll('.seat').length,
      ringClass: wrap
        ? ['ring-crowded', 'ring-sparse'].filter((c) => wrap.classList.contains(c)).join(' ') || 'default'
        : null,
      fw: cs ? cs.getPropertyValue('--fw').trim() : null,
      ar: cs ? cs.getPropertyValue('--ar').trim() : null,
      cdAvail: wrap ? getComputedStyle(wrap).getPropertyValue('--cd-avail').trim() : null,
      scrollY: Math.round(window.scrollY),
      // IS THE FELT THE SUBJECT OF THIS SHOT? Decided by looking at the PAGE, not
      // by the scene's name, so a scene added tomorrow cannot escape the floor by
      // being called something else. A dialog or the fairness side panel legally
      // reflows or covers the table (`shuffleproof` desktop is 24.8% BECAUSE the
      // proof panel is beside it, and `deposit` mobile scrolls the table half out
      // of frame behind the modal), so on those shots the geometry is RECORDED and
      // not asserted. Every plain table view is asserted.
      obstructedBy: (() => {
        const sels = ['.modal-backdrop', '.modal-content', '[role="dialog"]', '.shuffle-proof'];
        for (const sel of sels) {
          for (const el of document.querySelectorAll(sel)) {
            const b = el.getBoundingClientRect();
            if (b.width < 2 || b.height < 2) continue;
            const s = getComputedStyle(el);
            if (s.display === 'none' || s.visibility === 'hidden' || Number(s.opacity) < 0.05) continue;
            if (b.bottom <= 0 || b.top >= vh || b.right <= 0 || b.left >= vw) continue;
            return sel;
          }
        }
        return null;
      })(),
    };
  });
}

/**
 * Folds a felt measurement into a scene verdict fragment.
 *
 * A scene with no `.felt` (the lobby, the deposit screen) is not a failure and
 * not a pass: it returns `ok: true` with `felt: null`, so the gate is silent
 * exactly where a felt would be meaningless.
 *
 * @param {object|null} felt result of `measureFelt`
 * @param {{viewport:string, scene:string}} where
 */
export function foldFeltArea(felt, { viewport, scene }) {
  if (!felt) {
    return { ok: true, felt: null, problems: [], notes: 'no .felt on this view (not a table)' };
  }
  const floor = FELT_AREA_FLOOR[viewport];
  const problems = [];
  // A dialog or the fairness panel legally changes the table's geometry, and a
  // scrolled page legally moves it out of frame. On those shots the numbers are
  // recorded and the floor is not applied — and the reason is named in the notes,
  // so a reader can see exactly which shots the floor did and did not cover.
  const asserted = !felt.obstructedBy && felt.scrollY === 0;
  if (asserted) {
    if (typeof floor === 'number' && felt.areaPct < floor) {
      problems.push(
        `the playing surface is ${felt.areaPct}% of the ${felt.viewport.w}x${felt.viewport.h} frame `
        + `(${felt.w}x${felt.h}), below the ${floor}% floor for ${viewport}. `
        + 'A shrinking felt with the notices still on screen is the regression no other '
        + 'gate in this repo can see; see docs/WAVE-05.md and lib/felt-area.mjs',
      );
    }
    if (!felt.fullyInViewport) {
      problems.push(
        `the playing surface is not fully inside the frame: ${felt.w}x${felt.h} at `
        + `(${felt.x},${felt.y}) in ${felt.viewport.w}x${felt.viewport.h}`,
      );
    }
  }
  const why = felt.obstructedBy
    ? `RECORDED not asserted (${felt.obstructedBy} is on screen)`
    : felt.scrollY !== 0
      ? `RECORDED not asserted (page scrolled to y=${felt.scrollY})`
      : `floor ${floor}%`;
  return {
    ok: problems.length === 0,
    asserted,
    felt,
    problems,
    notes: `felt ${felt.w}x${felt.h} = ${felt.areaPct}% of frame, aspect ${felt.aspect}, `
      + `${felt.seatPods} pods (${felt.ringClass}), ${why}`,
    scene,
  };
}
