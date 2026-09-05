// Scene: the lobby as a brand-new visitor sees it (not signed in).

import { openApp, settle } from '../lib/browser.mjs';
import { assertLobbyAgreement, withAgreement } from '../lib/chain-agreement.mjs';
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
    // Signed out = a sign-in control in the wallet slot. On a phone a local
    // build shows ONE button (the dev button reading "Connect", its menu
    // holding Internet Identity and the dev players; WalletButton.svelte) and
    // hides the wide Connect Wallet button, so either painted button counts.
    const anonymous = await page.locator('.login-buttons .wallet-btn:visible').first().isVisible().catch(() => false);
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
    // The lobby's money figures are the stakes and the buy-in range: the numbers a
    // player uses to choose a table. They are asserted against the table
    // canisters' own configs, not merely counted.
    const agreement = await assertLobbyAgreement(ctx, page);
    return withAgreement({
      verified: rows > 0 && anonymous && disclaimer && lobbyState === 'ready' && !spinnerVisible,
      checks,
      notes:
        `${rows} live table rows; signed out; lobby state="${lobbyState}"; ` +
        `loading spinner ${spinnerVisible ? 'VISIBLE (defect H-09 is back)' : 'absent'}`,
    }, agreement);
  },
};
