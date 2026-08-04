// Scene: hand history with a completed hand.
//
// HandHistory.svelte reads the table canister's own get_hand_history for hands
// 1..hand_number, so the table needs at least one finished hand. Because the
// table is reset at the start of the scene, that hand is always hand #1.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER } from '../lib/config.mjs';
import { phaseOf } from '../lib/table-driver.mjs';
import { playCompletedHand, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

export default {
  name: 'handhistory',
  title: 'Hand history with a completed hand',
  auth: true,

  async setup(ctx) {
    const { tableId, view: v } = await playCompletedHand(ctx, TABLE);
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        handNumber: v.hand_number,
        winners: v.last_hand_winners.map((w) => ({ seat: Number(w.seat), amount: w.amount })),
      },
      notes: `hand #${v.hand_number} recorded on the table canister`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.history-btn', { timeout: 30_000 });
    await page.click('.history-btn');
    await page.waitForSelector('.hand-history-modal', { timeout: 30_000 });
    await page.waitForSelector('.hand-row', { timeout: 30_000 });
    await settle(page);
  },

  async verify(ctx, page) {
    const rows = await page.locator('.hand-row').count();
    const rowText = ((await page.locator('.hand-row').first().textContent()) || '')
      .replace(/\s+/g, ' ').trim();
    const modal = await page.locator('.hand-history-modal').count();
    return {
      verified: modal === 1 && rows >= 1,
      checks: { modal, handRows: rows, firstRow: rowText.slice(0, 160) },
      notes: `${rows} hand row(s); first: ${rowText.slice(0, 80)}`,
    };
  },
};
