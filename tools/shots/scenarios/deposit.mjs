// Scene: the deposit flow (modal), opened from a real table view.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { phaseOf, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

export default {
  name: 'deposit',
  title: 'Deposit modal (ICRC-2 approve + transfer_from flow)',
  auth: true,

  async setup(ctx) {
    // Hero alone at the table: no hand in progress, so the modal is not racing a
    // 500 ms poll that changes the page underneath it.
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: 0, chips: icp(12) },
    ]);
    const v = await view(HERO_PLAYER, tableId);
    return {
      table: TABLE,
      tableId,
      onChain: { phase: phaseOf(v), seated: 1 },
      notes: 'hero seated alone; deposit modal opened from the table wallet panel',
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // The wallet panel can start collapsed on a narrow viewport.
    const deposit = page.locator('.wallet-action-btn.deposit');
    if (!(await deposit.isVisible().catch(() => false))) {
      const toggle = page.locator('.panel-toggle');
      if (await toggle.isVisible().catch(() => false)) await toggle.click();
    }
    await deposit.first().waitFor({ timeout: 30_000 });
    await deposit.first().click();
    await page.waitForSelector('.modal-content', { timeout: 30_000 });
    await page.waitForSelector('#deposit-modal-title', { timeout: 30_000 });
    await settle(page);
  },

  async verify(ctx, page) {
    const title = ((await page.locator('#deposit-modal-title').textContent()) || '')
      .replace(/\s+/g, ' ').trim();
    const modal = await page.locator('.modal-content').count();
    const sourceToggle = await page.locator('.wallet-source-toggle button').allTextContents();
    return {
      verified: modal >= 1 && /deposit/i.test(title),
      checks: {
        modals: modal,
        title,
        walletSources: sourceToggle.map((t) => t.replace(/\s+/g, ' ').trim()).filter(Boolean),
      },
      notes: title,
    };
  },
};
