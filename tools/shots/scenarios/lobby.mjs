// Scene: the lobby as a brand-new visitor sees it (not signed in).

import { openApp, settle } from '../lib/browser.mjs';

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
    await page.waitForSelector('.table-name', { timeout: 60_000 });
    // Wait for the per-table player counts (each is a real canister query).
    await page.waitForFunction(
      () => document.querySelectorAll('tbody tr').length > 0,
      undefined,
      { timeout: 60_000 },
    );
    await settle(page);
  },

  async verify(ctx, page) {
    const rows = await page.locator('tbody tr').count();
    const namedTables = await page.locator('.table-name').allTextContents();
    const anonymous = await page.locator('.wallet-btn.connect').isVisible().catch(() => false);
    const disclaimer = await page.locator('.alpha-warning-banner').isVisible().catch(() => false);
    const checks = { rows, namedTables, showsConnectWallet: anonymous, disclaimerVisible: disclaimer };
    return {
      verified: rows > 0 && anonymous && disclaimer,
      checks,
      notes: `${rows} live table rows; signed out`,
    };
  },
};
