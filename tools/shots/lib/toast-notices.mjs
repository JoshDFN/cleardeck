// CAN THE APP'S OWN ERROR TOAST COVER A PLAYER-PROTECTION NOTICE? MEASURED, ON
// EVERY VIEW, AT EVERY VIEWPORT.
//
// WHY THIS MODULE EXISTS
// ----------------------
// docs/DEFECTS.md E-52. `lib/protected-notices.mjs` hit-tests each protected
// phrase on its own pixels and is run centrally by run.mjs for every scene, and
// it was green throughout a wave in which a portrait player who hit ANY error saw
// the no-rake property covered at 9 of 9 sample points and the other four
// protected phrases at 3 of 9. The probe was right. The problem was upstream of
// it: no scene in tools/shots/scenarios ever raises a toast, so the app has a
// state the harness never puts it into, and `lib/occlusion.mjs` only knows about
// `.modal-backdrop`.
//
// That is the wave-4 lesson recurring one layer out. Wave 4 measured the source
// instead of the screen. Wave 6 measured the screen, but only the screens the
// harness knew how to produce. So this module's job is to enlarge the SET OF
// RENDERED PAGES the notice gate sees, not to invent a new kind of check.
//
// WHY THE TOAST IS INJECTED RATHER THAN PROVOKED, AND WHY THAT IS NOT A CHEAT
// ---------------------------------------------------------------------------
// Provoking a real toast means breaking the agent's calls mid-scene (abort the
// fetches, let the app's own 500 ms poll fail). That is what the wave-6 probe
// did, and it is the right thing for a one-off reproducer. As a CENTRAL gate it
// is the wrong tool: it changes what is on screen for every other gate in the
// same run, it depends on each view having a call in flight, and a scene that
// cannot be provoked would quietly measure nothing.
//
// So the toast is injected -- but not as a lookalike. Svelte scopes every rule
// in `routes/+page.svelte` to a generated class (`.toast.svelte-1uha8ag`), so a
// hand-written `<div class="toast error">` would receive NO styling at all and
// the measurement would be worthless. The node built here carries the app's own
// scope class, read off `.alpha-warning-banner` (same component), and the module
// then PROVES the styling applied rather than assuming it:
//
//   * it collects, from the live CSSOM, every rule whose selector mentions
//     `.toast`, and requires the injected node to `matches()` at least one of
//     them -- so it is the app's own rule that is painting it, the same object
//     the real toast would match;
//   * it requires the computed `position` to be `fixed`, because a toast that is
//     not an overlay cannot cover anything and would make this gate vacuous;
//   * it records the computed `z-index`, `top`, `max-width` and `max-height` in
//     the run manifest, so a reader can compare them against the CSS by eye.
//
// The scenario `toast-notices` closes the last gap by raising a REAL toast
// through the app's own error path and asserting its geometry against the
// injected one.
//
// WHAT IS ASSERTED
// ----------------
//   1. every protected phrase still has a carrier that is on screen and wins the
//      hit test on its own pixels, with the toast up (the E-52 failure itself);
//   2. the toast's box is inside the viewport horizontally -- the E-52 toast was
//      624 px wide starting at x = -117 on a 390 px screen, overflowing both
//      edges so that no clear column existed;
//   3. the toast starts at or below the bottom edge of `.alpha-warning-banner`
//      whenever the banner is on screen. This is the positional lock; (1) would
//      still pass on paint order alone, so asserting it separately is what stops
//      the two locks silently collapsing into one.

import { foldProtectedNotices, probeProtectedNotices } from './protected-notices.mjs';

/**
 * Messages to raise. Both are worst cases, for different reasons.
 *
 * `prose` is the message the app really produced in the E-52 measurement: the
 * agent's own "failed to fetch" text, which is what made the toast 534 px tall.
 * `unbreakable` is a single token with no spaces, which is how a box with no
 * `max-width` grows past both edges of a phone screen.
 */
export const TOAST_MESSAGES = [
  {
    id: 'prose',
    text:
      'Failed to fetch HTTP request: error sending request for url '
      + '(http://127.0.0.1:8077/api/v2/canister/aaaaa-aa/call): client error (Connect): '
      + 'tcp connect error: Connection refused (os error 61). The call was not delivered '
      + 'to the canister and no state changed.',
  },
  {
    id: 'unbreakable',
    text: `Rejected:${'x'.repeat(240)}`,
  },
];

const MARKER = 'data-cd-toast-probe';

/**
 * Injects one toast styled by the app's own rule, and reports what it became.
 *
 * @param {import('playwright').Page} page
 * @param {string} text
 * @returns {Promise<object>} geometry + the proof that the app's CSS applied
 */
export function raiseToast(page, text) {
  return page.evaluate(
    ({ message, marker }) => {
      const rectOf = (el) => {
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return {
          x: Math.round(r.x), y: Math.round(r.y),
          w: Math.round(r.width), h: Math.round(r.height),
          right: Math.round(r.right), bottom: Math.round(r.bottom),
        };
      };

      const banner = document.querySelector('.alpha-warning-banner');
      const app = document.querySelector('.app') || document.body;
      // Svelte scopes every rule in the component to this class. Reading it off
      // an element of the SAME component is the only way an injected node can be
      // painted by the same rule the real toast is painted by.
      const scope = banner
        ? [...banner.classList].find((c) => /^svelte-/.test(c)) || null
        : null;

      const node = document.createElement('div');
      node.className = ['toast', 'error', scope].filter(Boolean).join(' ');
      node.setAttribute(marker, '1');
      const span = document.createElement('span');
      if (scope) span.classList.add(scope);
      span.textContent = message;
      node.appendChild(span);
      app.appendChild(node);

      // PROOF THAT THE APP'S OWN RULE IS PAINTING THIS NODE, not a resemblance.
      const rules = [];
      for (const sheet of [...document.styleSheets]) {
        let list;
        try { list = [...sheet.cssRules]; } catch { continue; } // cross-origin
        for (const rule of list) {
          if (!rule.selectorText || !rule.selectorText.includes('.toast')) continue;
          rules.push(rule.selectorText);
        }
      }
      const matched = rules.filter((sel) =>
        sel.split(',').some((part) => {
          try { return node.matches(part.trim()); } catch { return false; }
        }));

      const cs = getComputedStyle(node);
      return {
        scopeClass: scope,
        toastRulesInStylesheet: rules.length,
        toastRulesMatched: matched,
        computed: {
          position: cs.position,
          zIndex: cs.zIndex,
          top: cs.top,
          maxWidth: cs.maxWidth,
          maxHeight: cs.maxHeight,
          overflowY: cs.overflowY,
        },
        rect: rectOf(node),
        bannerRect: rectOf(banner),
        viewport: { w: window.innerWidth, h: window.innerHeight },
      };
    },
    { message: text, marker: MARKER },
  );
}

/** Removes every toast this module injected. */
export function removeToast(page) {
  return page.evaluate((marker) => {
    for (const el of document.querySelectorAll(`[${marker}]`)) el.remove();
  }, MARKER);
}

/**
 * Raises each message in turn, re-runs the protected-notice probe with it up, and
 * folds the whole thing into a scene verdict fragment.
 *
 * The page is left exactly as it was found: every injected node is removed in a
 * `finally`, so a throw here cannot leave a toast in the screenshot.
 *
 * @param {import('playwright').Page} page
 * @param {{scene:string, viewport:string}} where
 * @returns {Promise<{ok:boolean, problems:string[], cases:Array<object>}>}
 */
export async function assertNoticesSurviveAToast(page, { scene, viewport }) {
  const problems = [];
  const cases = [];
  try {
    for (const message of TOAST_MESSAGES) {
      await removeToast(page); // never measure two at once
      const raised = await raiseToast(page, message.text);

      if (!raised.scopeClass) {
        problems.push(
          'could not find the component scope class on .alpha-warning-banner, so the injected '
          + 'toast would not be styled by the app\'s own rule. Refusing to report a pass from a '
          + 'measurement that proves nothing.',
        );
      }
      if (raised.toastRulesMatched.length === 0) {
        problems.push(
          `the injected toast matches NONE of the ${raised.toastRulesInStylesheet} .toast rule(s) `
          + 'in the stylesheet, so it is not being painted by the app\'s toast CSS. '
          + 'This gate is measuring the wrong thing until that is fixed.',
        );
      }
      if (raised.computed.position !== 'fixed') {
        problems.push(
          `.toast computes position: ${raised.computed.position}, not fixed. An in-flow toast `
          + 'cannot cover anything, which makes this gate vacuous rather than green.',
        );
      }

      // WHAT SCROLL POSITION THE RULE IS ABOUT -- the same answer run.mjs gives
      // for the toast-free probe, and for the same reason.
      //
      // Some scenes scroll deliberately to frame something: `shuffleproof` puts
      // the proof panel in shot on a phone, which leaves the banner at y = -90.
      // A notice that is off screen because the SCENE scrolled has not been
      // covered by anything, and failing it here would be a false red -- the
      // kind that gets a gate switched off. So probe where the scene left the
      // page, and if anything is off screen, probe again at scroll 0 and let
      // THAT be the verdict, recording both. A notice the toast covers is
      // covered at scroll 0 too.
      let notices = foldProtectedNotices(
        await probeProtectedNotices(page),
        `with the app's error toast on screen (scene ${scene}, ${viewport}, `
        + `message "${message.id}")`,
      );
      let measuredAtScrollTop = false;
      let sceneScrollY = 0;
      if (!notices.ok) {
        sceneScrollY = await page.evaluate(() => window.scrollY);
        if (sceneScrollY !== 0) {
          await page.evaluate(() => window.scrollTo(0, 0));
          notices = foldProtectedNotices(
            await probeProtectedNotices(page),
            `with the app's error toast on screen, at the top of the view `
            + `(scene ${scene}, ${viewport}, message "${message.id}"; `
            + `the scene had scrolled to y=${sceneScrollY})`,
          );
          measuredAtScrollTop = true;
          await page.evaluate((y) => window.scrollTo(0, y), sceneScrollY);
        }
      }
      for (const p of notices.problems) problems.push(p);

      // E-52's other half: the box itself. A toast wider than the screen has no
      // clear column beside it at any position, so this is asserted directly and
      // not left to follow from the hit test.
      const { rect, bannerRect, viewport: vp } = raised;
      if (rect && (rect.x < 0 || rect.right > vp.w)) {
        problems.push(
          `the toast box [${rect.x}, ${rect.y}, ${rect.w}, ${rect.h}] overflows the ${vp.w}px `
          + `viewport (message "${message.id}"): it spans x ${rect.x}..${rect.right}. `
          + 'That is the E-52 shape exactly.',
        );
      }
      // The positional lock, asserted independently of paint order.
      const bannerOnScreen = bannerRect && bannerRect.bottom > 0 && bannerRect.y < vp.h;
      if (rect && bannerOnScreen && rect.y < bannerRect.bottom - 1) {
        problems.push(
          `the toast starts at y=${rect.y}, above the bottom edge of .alpha-warning-banner `
          + `(y=${bannerRect.bottom}) (message "${message.id}"). The notices may still win on `
          + 'paint order, but the positional lock described in +page.svelte is broken.',
        );
      }

      cases.push({
        message: message.id,
        messageLength: message.text.length,
        scopeClass: raised.scopeClass,
        toastRulesMatched: raised.toastRulesMatched,
        computed: raised.computed,
        toastRect: rect,
        bannerRect,
        viewport: vp,
        noticesOnScreen: `${notices.onScreen}/${notices.total}`,
        measuredAtScrollTop,
        sceneScrollY,
        notices: notices.notices,
      });
    }
  } finally {
    await removeToast(page);
  }

  return { ok: problems.length === 0, problems, cases };
}
