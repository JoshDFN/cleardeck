// Scene: a table with open seats — the state the README screenshot shows.
//
// docs/screenshots/table.png (the README shot) is the hero seated alone with an
// open "Seat 2" placeholder and the WAITING FOR PLAYERS badge, so that is what
// this scene reproduces: one real seated player, the rest of the seats open.
// table_3 is 9-max, so eight open seats are visible.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { optional } from '../lib/agent.mjs';
import { phaseOf, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_3';

export default {
  name: 'table-empty',
  title: 'Table with open seats (hero seated alone, waiting for players)',
  auth: true,

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: 0, chips: icp(25) },
    ]);
    const v = await view(HERO_PLAYER, tableId);
    const seated = v.players.filter((p) => p.length > 0).length;
    const open = v.players.length - seated;
    if (seated !== 1) throw new Error(`Expected exactly the hero seated, found ${seated}`);
    if (phaseOf(v) !== 'WaitingForPlayers') {
      throw new Error(`Expected WaitingForPlayers, table is at ${phaseOf(v)}`);
    }
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v), seated, openSeats: open, handNumber: v.hand_number,
        heroSeat: optional(v.my_seat),
      },
      notes: `${TABLE} reset; hero alone at seat 0, ${open} open seats, phase WaitingForPlayers`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.join-seat', { timeout: 30_000 });
    await page.waitForSelector('.player-nameplate.highlight-me', { timeout: 30_000 });
    await settle(page);
  },

  async verify(ctx, page) {
    const openSeats = await page.locator('.join-seat').count();
    const heroSeated = await page.locator('.player-nameplate.highlight-me').count();
    const phaseText = ((await page.locator('.phase-indicator').first().textContent()) || '').trim();
    const walletPanel = await page.locator('.wallet-panel').count();
    return {
      verified: openSeats > 0 && heroSeated === 1,
      checks: { openSeatsRendered: openSeats, heroNameplate: heroSeated, phaseText, walletPanel },
      notes: `${openSeats} open seats, hero seated, phase "${phaseText}"`,
    };
  },
};
