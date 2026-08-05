// ARE THE FOUR PLAYER-PROTECTION NOTICES ON SCREEN? MEASURED, NOT GREPPED.
//
// The lesson of wave 4, in one paragraph. `make hygiene` greps the SOURCE for the
// four protected phrases and was green throughout a wave in which a portrait
// player at a table saw NONE of them: `.alpha-warning-banner` was
// `display: none` and `.footer-disclaimer` was 248 px of scroll below the fold.
// An independent protected-phrase count was green too, for the same reason. A
// notice that is in the bundle and not on the screen is invisible to every check
// this repo had.
//
// docs/WAVE-04.md §2 fixed that and re-measured with a GEOMETRY probe: the banner
// at `y 0..268`, "fully in the viewport", "and behind the open Deposit modal".
// That last clause is the gap this module closes. Geometry cannot see OCCLUSION,
// and every dialog in this app is `position: fixed; inset: 0` over a scrim at 72%
// black with a 4 px blur. A notice under that sheet has a rectangle in the
// viewport and is unreadable.
//
// So each phrase is hit-tested on its OWN PIXELS with `elementFromPoint`. A
// phrase whose points answer `.modal-backdrop` is reported occluded, by name, and
// the scene fails. Nothing here can be satisfied by a phrase merely being in the
// DOM, in the bundle, or in the source.

/**
 * The four protected notices as a player reads them, plus the no-rake property.
 *
 * The first four strings are exactly `FRONTEND_NOTICES` in `scripts/dev.sh`, so
 * this probe and `make hygiene` are asking about the same sentences from two
 * different directions: hygiene asks whether they are in the source, this asks
 * whether they reached the screen. The fifth is the no-rake property, which
 * HARD RULE 2 protects alongside them.
 */
export const PROTECTED_PHRASES = [
  'Unaudited code with known bugs',
  'your funds are NOT safe',
  'illegal in many jurisdictions',
  '18+ only',
  'No rake is taken from any pot on any table',
];

/**
 * For each phrase: is at least one carrier of it visible, inside the viewport,
 * and not painted over?
 *
 * @param {import('playwright').Page} page
 * @param {string[]} [phrases]
 * @returns {Promise<Array<{phrase:string, carriersInDom:number, onScreen:boolean,
 *                          best:object|null, rejected:Array<object>}>>}
 */
export function probeProtectedNotices(page, phrases = PROTECTED_PHRASES) {
  return page.evaluate((list) => {
    const norm = (s) => (s || '').replace(/\s+/g, ' ');
    const describe = (el) => {
      const parts = [];
      for (let n = el, d = 0; n && d < 3 && n !== document.body; n = n.parentElement, d += 1) {
        parts.unshift(
          `${n.tagName.toLowerCase()}${[...n.classList].slice(0, 2).map((c) => `.${c}`).join('')}`,
        );
      }
      return parts.join(' > ');
    };

    return list.map((phrase) => {
      // The DEEPEST element that carries the whole phrase: an ancestor carries it
      // too, and measuring a wrapper would measure the wrong rectangle.
      const carriers = [...document.querySelectorAll('body *')].filter((el) => {
        if (!norm(el.textContent).includes(phrase)) return false;
        return ![...el.children].some((c) => norm(c.textContent).includes(phrase));
      });

      const measured = carriers.map((el) => {
        const visible = typeof el.checkVisibility === 'function'
          ? el.checkVisibility({
            visibilityProperty: true, opacityProperty: true, contentVisibilityAuto: true,
          })
          : true;
        const r = el.getBoundingClientRect();
        const inViewport = r.width > 1 && r.height > 1
          && r.bottom > 0 && r.top < window.innerHeight
          && r.right > 0 && r.left < window.innerWidth;
        let samples = 0;
        let ownPixels = 0;
        let occludedBy = null;
        if (visible && inViewport) {
          const ys = [r.top + 4, r.top + r.height / 2, r.bottom - 4]
            .filter((y) => y > 0 && y < window.innerHeight);
          const xs = [r.left + 4, r.left + r.width * 0.35, r.left + r.width * 0.65, r.right - 4]
            .filter((x) => x > 0 && x < window.innerWidth);
          for (const y of ys) {
            for (const x of xs) {
              samples += 1;
              const top = document.elementFromPoint(x, y);
              // The carrier itself, one of its children (a <strong> inside the
              // paragraph), or an ancestor if the carrier does not take hits.
              if (top && (top === el || el.contains(top) || top.contains(el))) ownPixels += 1;
              else if (!occludedBy) occludedBy = top ? describe(top) : 'nothing';
            }
          }
        }
        return {
          path: describe(el),
          visible,
          inViewport,
          samples,
          ownPixels,
          occludedBy,
          rect: {
            x: Math.round(r.x), y: Math.round(r.y),
            w: Math.round(r.width), h: Math.round(r.height),
          },
          // Most of the sampled points must land on the phrase's own pixels. A
          // rounded corner or a 1 px border can steal one; a scrim steals all.
          onScreen: visible && inViewport && samples > 0 && ownPixels / samples >= 0.6,
        };
      });

      const best = measured.find((m) => m.onScreen) || measured[0] || null;
      return {
        phrase,
        carriersInDom: carriers.length,
        onScreen: Boolean(best?.onScreen),
        best,
        // Kept so a reader can see what WAS there and why it did not count.
        rejected: measured.filter((m) => !m.onScreen).map((m) => ({
          path: m.path,
          visible: m.visible,
          inViewport: m.inViewport,
          ownPixels: `${m.ownPixels}/${m.samples}`,
          occludedBy: m.occludedBy,
          rect: m.rect,
        })),
      };
    });
  }, phrases);
}

/**
 * Folds the probe into a scene verdict fragment.
 *
 * @param {Array<object>} notices result of `probeProtectedNotices`
 * @param {string} where a human phrase naming the surface, for the failure text
 * @returns {{ok:boolean, onScreen:number, total:number, problems:string[], notices:Array<object>}}
 */
export function foldProtectedNotices(notices, where) {
  const problems = notices.filter((n) => !n.onScreen).map((n) =>
    `protected notice "${n.phrase}" is NOT on screen ${where} `
    + `(${n.carriersInDom} carrier(s) in the DOM; nearest was `
    + `${JSON.stringify(n.best?.occludedBy ?? null)} at ${JSON.stringify(n.best?.rect ?? null)})`);
  return {
    ok: problems.length === 0,
    onScreen: notices.filter((n) => n.onScreen).length,
    total: notices.length,
    problems,
    notices,
  };
}
