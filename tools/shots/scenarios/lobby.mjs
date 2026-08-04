// Scene: the lobby as a brand-new visitor sees it (not signed in).

import { openApp, settle } from '../lib/browser.mjs';
import { lobbyStateOf, waitForLobbySettled } from './_shared.mjs';

export default {
  name: 'lobby',
  title: 'Lobby — table list as a new visitor sees it',
  auth: false,

  async setup() {
    // Nothing to stage: the lobby renders whatever the lobby canister and the
    // live table canisters report. Player counts are real.
    return { notes: 'anonymous visitor; table rows and player counts come from the lobby + table canisters' };
  },

  async stage(ctx, page) {
    await openApp(page);
    await waitForLobbySettled(page);
    await settle(page);
  },

  async verify(ctx, page) {
    const rows = await page.locator('tbody tr').count();
    const namedTables = await page.locator('.table-name').allTextContents();
    const anonymous = await page.locator('.wallet-btn.connect').isVisible().catch(() => false);
    const disclaimer = await page.locator('.alpha-warning-banner').isVisible().catch(() => false);
    // docs/DEFECTS.md H-09: the spinner and the loaded lobby used to be able to
    // render together. They are now mutually exclusive, and this asserts it, so
    // the defect cannot silently come back.
    const spinnerVisible = await page.locator('.loading-state').isVisible().catch(() => false);
    const lobbyState = await lobbyStateOf(page);
    const checks = {
      rows,
      namedTables,
      showsConnectWallet: anonymous,
      disclaimerVisible: disclaimer,
      lobbyState,
      loadingSpinnerVisible: spinnerVisible,
    };
    return {
      verified: rows > 0 && anonymous && disclaimer && lobbyState === 'ready' && !spinnerVisible,
      checks,
      notes:
        `${rows} live table rows; signed out; lobby state="${lobbyState}"; ` +
        `loading spinner ${spinnerVisible ? 'VISIBLE (defect H-09 is back)' : 'absent'}`,
    };
  },
};
