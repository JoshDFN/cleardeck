// Scene: hand history with a completed hand.
//
// HandHistory.svelte reads the table canister's own get_hand_history for hands
// 1..hand_number, so the table needs at least one finished hand. Because the
// table is reset at the start of the scene, that hand is always hand #1.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER } from '../lib/config.mjs';
import { phaseOf } from '../lib/table-driver.mjs';
import {
  assertChainAgreement, assertHandHistoryAgreement, named, withAgreement,
} from '../lib/chain-agreement.mjs';
import { heroPrincipal, playCompletedHand, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

export default {
  name: 'handhistory',
  title: 'Hand history with a completed hand',
  auth: true,

  async setup(ctx) {
    // One real hand to completion is enough. Verified on this replica: the panel
    // lists a hand the hero LOST as well as one it won ("You lost / Showdown"),
    // because the table's record carries every showdown player's principal, not
    // just the winners'. So there is no need to fish for a win, and the hand the
    // shot shows is whichever one the real shuffle produced.
    const { tableId, view: v } = await playCompletedHand(ctx, TABLE);
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        handNumber: v.hand_number,
        winners: v.last_hand_winners.map((w) => ({
          seat: Number(w.seat),
          principal: w.principal?.toText ? w.principal.toText() : String(w.principal),
          amount: String(w.amount),
        })),
        heroPrincipal: heroPrincipal(),
      },
      notes: `hand #${v.hand_number} recorded on the table canister`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // WAIT ON REAL STATE, NOT A SLEEP, and specifically on the state HandHistory
    // depends on. `+page.svelte` passes `handNumber={tableState?.hand_number || 0}`,
    // and HandHistory only queries the TABLE canister when `handNumber > 0`;
    // otherwise it falls through to the HISTORY canister, which is empty on this
    // network because nothing in any deploy path ever calls the history
    // canister's `authorize_table`, so `record_hand` is rejected as
    // "Unauthorized: table not registered".
    //
    // Opening the panel before the first `get_table_view` has landed therefore
    // renders "No hands recorded yet" for a hand that demonstrably exists. The
    // five community cards of the finished hand are the viewport-independent
    // signal that the app has the hand: they are on the felt at both 1440px and
    // 390px, unlike `.feed-title` ("Hand #N"), which lives in `.feed-container`
    // and is hidden below 900px.
    await page.waitForFunction(
      () => document.querySelectorAll('.community-cards .card').length >= 5,
      undefined,
      { timeout: 60_000 },
    );

    await page.waitForSelector('.history-btn', { timeout: 30_000 });
    await page.click('.history-btn');
    await page.waitForSelector('.hand-history-modal', { timeout: 30_000 });

    // One bounded re-open: HandHistory kicks off its load from an effect on mount,
    // so a panel opened a beat too early can settle on the empty result. Closing
    // and re-opening remounts it against the now-populated table state. Bounded,
    // and any remaining emptiness is reported by verify() rather than retried away.
    try {
      await page.waitForSelector('.hand-row', { timeout: 8_000 });
    } catch {
      await page.locator('.hand-history-modal .close-btn, .hand-history-modal button')
        .first().click().catch(() => {});
      await page.click('.history-btn');
      await page.waitForSelector('.hand-history-modal', { timeout: 15_000 });
      await page.waitForSelector('.hand-row', { timeout: 20_000 });
    }
    await settle(page);
  },

  async verify(ctx, page) {
    const rows = await page.locator('.hand-row').count();
    const rowText = ((await page.locator('.hand-row').first().textContent()) || '')
      .replace(/\s+/g, ' ').trim();
    const modal = await page.locator('.hand-history-modal').count();
    // Two surfaces state money here: the table behind the modal, and the modal's
    // own per-hand pot. Both are compared with the canister.
    const tableAgreement = named('table', await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    }));
    const historyAgreement = named('history', await assertHandHistoryAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER,
    }));
    return withAgreement({
      verified: modal === 1 && rows >= 1,
      checks: { modal, handRows: rows, firstRow: rowText.slice(0, 160) },
      notes: `${rows} hand row(s); first: ${rowText.slice(0, 80)}`,
    }, tableAgreement, historyAgreement);
  },
};
