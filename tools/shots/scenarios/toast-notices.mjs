// Scene: the app showing a REAL error, and the four protected notices under it.
//
// docs/DEFECTS.md E-52. This scene exists for one reason: to close the gap
// between the toast `lib/toast-notices.mjs` injects into every other scene and
// the toast the app raises on its own.
//
// The central gate is the injected one, because it is the only kind that can be
// raised on every view at every viewport without changing what every other gate
// in the same run is looking at. Its weakness is obvious and stated in that
// file: it is a node this harness created. So here, once per viewport, the app's
// OWN error path is driven -- every canister call is aborted at the network
// layer and the app's own sign-in is clicked, which is exactly the reproducer the
// wave-6 measurement used -- and the resulting toast is measured the same way.
// Then an injected toast is raised on the same page at the same viewport and the
// two geometries are compared, field by field. If they ever disagree, the central
// gate has been measuring something the player never sees, and this scene goes
// red and says so.
//
// The scene then reloads the page, so the artifact it files and every central
// gate that runs after it see an ordinary, error-free lobby.

import { openApp, settle } from '../lib/browser.mjs';
import { forgetConsoleFaults } from '../lib/page-health.mjs';
import { assertLobbyAgreement, withAgreement } from '../lib/chain-agreement.mjs';
import { foldProtectedNotices, probeProtectedNotices } from '../lib/protected-notices.mjs';
import { TOAST_MESSAGES, raiseToast, removeToast } from '../lib/toast-notices.mjs';
import { lobbyStateOf, waitForLobbySettled } from './_shared.mjs';

const API_ROUTES = ['**/api/v2/**', '**/api/v3/**'];

/** What `stage` measured, handed to `verify`, which the runner calls separately. */
let lastStage = null;

/** Geometry + computed style of whatever `.toast` is on screen, or null. */
function measureLiveToast(page) {
  return page.evaluate(() => {
    const el = document.querySelector('.toast');
    if (!el) return null;
    const r = el.getBoundingClientRect();
    const banner = document.querySelector('.alpha-warning-banner');
    const br = banner ? banner.getBoundingClientRect() : null;
    const cs = getComputedStyle(el);
    return {
      className: el.className,
      text: (el.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 200),
      computed: {
        position: cs.position,
        zIndex: cs.zIndex,
        top: cs.top,
        maxWidth: cs.maxWidth,
        maxHeight: cs.maxHeight,
        overflowY: cs.overflowY,
      },
      rect: {
        x: Math.round(r.x), y: Math.round(r.y),
        w: Math.round(r.width), h: Math.round(r.height),
        right: Math.round(r.right), bottom: Math.round(r.bottom),
      },
      bannerRect: br
        ? { y: Math.round(br.y), bottom: Math.round(br.bottom), h: Math.round(br.height) }
        : null,
      viewport: { w: window.innerWidth, h: window.innerHeight },
    };
  });
}

export default {
  name: 'toast-notices',
  title: 'Error toast up — the four protected notices must still be readable',
  auth: false,

  async setup() {
    return {
      notes:
        "no on-chain staging: the state under test is the app's own error state, "
        + 'produced by aborting every canister call at the network layer',
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await waitForLobbySettled(page);
    await settle(page);

    const baseline = foldProtectedNotices(
      await probeProtectedNotices(page),
      'on the lobby with no error on screen (the baseline for this scene)',
    );

    // ---- the app's OWN error path -----------------------------------------
    // Every canister call fails at the transport layer. `loadTables()` catches
    // it and assigns `error = e.message`, which is the app's own toast. Nothing
    // is injected and no app code is patched.
    for (const route of API_ROUTES) {
      await page.route(route, (r) => r.abort('failed'));
    }

    let realToast = null;
    let realNotices = null;
    let raisedBy = null;
    try {
      // A reload with the transport dead is the app's most deterministic error:
      // `loadTables()` runs on mount, its await rejects, and its own catch does
      // `error = e.message`. Nothing is clicked, nothing is patched, and it does
      // not depend on any control being present at this viewport.
      await page.reload({ waitUntil: 'domcontentloaded', timeout: 60_000 });
      await page.waitForSelector('.brand', { timeout: 30_000 });
      await page.waitForSelector('.toast.error', { timeout: 20_000 });
      raisedBy = "the app's own error path (every canister call aborted, then a reload: "
        + 'loadTables() rejects and assigns error = e.message)';
      await settle(page);
      realToast = await measureLiveToast(page);
      realNotices = foldProtectedNotices(
        await probeProtectedNotices(page),
        "with the app's OWN error toast on screen (nothing injected)",
      );
    } catch (e) {
      raisedBy = `FAILED to raise a real toast: ${e.message.split('\n')[0]}`;
    }

    // ---- the injected toast, same page, same viewport ----------------------
    // Raised while the real one is still up would measure two toasts, so the
    // real one is dismissed first through its own close button.
    // Its OWN close control: the error toast also carries a Retry button now,
    // and a bare `button` would press that, re-read the dead transport and
    // raise the toast again.
    await page.click('.toast.error .toast-close').catch(() => {});
    await page.waitForSelector('.toast', { state: 'detached', timeout: 10_000 }).catch(() => {});
    const injected = await raiseToast(page, TOAST_MESSAGES[0].text);
    await removeToast(page);

    // ---- back to an ordinary lobby ----------------------------------------
    for (const route of API_ROUTES) await page.unroute(route);
    await openApp(page);
    await waitForLobbySettled(page);
    await settle(page);

    // The app behaved CORRECTLY during the outage this scene caused: it caught
    // the rejection and logged it. `page-health.mjs` classifies that line as an
    // escaped throw only because the transport's message quotes the words
    // "TypeError: Failed to fetch". Forgive exactly that line, on this page
    // only, and record what was forgiven in the manifest. Nothing here can
    // forgive an uncaught EXCEPTION -- `forgetConsoleFaults` throws if the
    // pattern matches one.
    const forgiven = forgetConsoleFaults(
      page,
      /Failed to load tables:.*TransportError/,
      'this scene aborts every canister call on purpose; the app catches the rejection and '
      + 'logs it, which is the handled path, not an escaped throw (pageErrors stayed empty)',
    );

    lastStage = { baseline, raisedBy, realToast, realNotices, injected, forgiven };
    return lastStage;
  },

  async verify(ctx, page) {
    const { baseline, raisedBy, realToast, realNotices, injected, forgiven } = lastStage || {};
    const problems = [];

    if (!realToast) {
      problems.push(
        'the app never showed a toast when every canister call was aborted, so this scene '
        + `proved nothing about the real error path (${raisedBy}). Do not read the central `
        + 'injected-toast gate as validated.',
      );
    }
    if (realNotices && !realNotices.ok) {
      for (const p of realNotices.problems) problems.push(p);
    }

    // THE BRIDGE. The central gate injects a node; this asserts that the node and
    // the app's own toast are the same object as far as the layout is concerned.
    const compared = [];
    if (realToast && injected) {
      // A BOTTOM-ANCHORED TOAST (the phone lobby's snackbar, routes/app-phone
      // .scss) declares `top: auto`, so its computed `top` is derived from its
      // own height and two messages of different length never agree on it.
      // The anchor that IS declared is the bottom edge, so `top` is judged as
      // the same when both boxes end within 2 px of each other.
      const sameBottom = Math.abs(realToast.rect.bottom - injected.rect.bottom) <= 2;
      for (const key of ['position', 'zIndex', 'top', 'maxWidth', 'maxHeight', 'overflowY']) {
        const real = realToast.computed[key];
        const fake = injected.computed[key];
        const same = real === fake || (key === 'top' && sameBottom);
        compared.push({ property: key, real, injected: fake, same });
        if (!same) {
          problems.push(
            `the injected toast and the app's own toast disagree on ${key}: `
            + `real="${real}" injected="${fake}". The central gate in run.mjs is measuring `
            + 'a node the player never sees.',
          );
        }
      }
      // Same left edge and same width budget: the geometry E-52 was about.
      if (Math.abs(realToast.rect.x - injected.rect.x) > 2) {
        problems.push(
          `the two toasts start at different x (real=${realToast.rect.x}, `
          + `injected=${injected.rect.x}).`,
        );
      }
    }

    // The scene's own screenshot is of a clean lobby, so the ordinary lobby
    // assertions still have to hold — otherwise this scene would file a PNG of a
    // lobby nobody checked.
    const rows = await page.locator('tbody tr').count();
    const lobbyState = await lobbyStateOf(page);
    const agreement = await assertLobbyAgreement(ctx, page);

    return withAgreement({
      verified: problems.length === 0 && rows > 0 && lobbyState === 'ready',
      checks: {
        raisedBy,
        baselineNoticesOnScreen: baseline ? `${baseline.onScreen}/${baseline.total}` : null,
        realToast,
        realNoticesOnScreen: realNotices ? `${realNotices.onScreen}/${realNotices.total}` : null,
        realNotices,
        injectedToast: injected,
        computedStyleComparison: compared,
        consoleFaultsForgiven: forgiven ?? null,
        problems,
        rows,
        lobbyState,
      },
      notes: problems.length === 0
        ? `real error toast raised by ${raisedBy}; `
          + `${realNotices ? realNotices.onScreen : 0}/${realNotices ? realNotices.total : 0} `
          + 'protected notices still on screen under it; the injected toast the central gate '
          + 'uses matches it on every computed property compared'
        : problems.join(' | '),
    }, agreement);
  },
};
