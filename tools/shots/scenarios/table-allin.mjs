// Scene: the all-in moment — two players committed, the third still to act.
//
// Three players go all in in turn order. The loop stops the instant two of them
// are all in and the action is pending on the third, which is a state the engine
// genuinely rests in (the pending player has the full action clock).

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { allInSeats, doAct, phaseOf, playUntil, startHand } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_3'; // 9-max, 0.10/0.20 ICP, 60s action clock
const SEATS = [
  { player: HERO_PLAYER, seat: 0, chips: icp(25) },
  { player: 2, seat: 1, chips: icp(40) },
  { player: 3, seat: 2, chips: icp(70) },
];

export default {
  name: 'table-allin',
  title: 'All-in moment — two players committed',
  auth: true,
  timerTarget: '50s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, SEATS);
    await startHand(2, tableId);

    const plan = {};
    for (const s of SEATS) plan[s.seat] = { player: s.player, act: (t) => doAct.allIn(t) };

    const v = await playUntil(
      tableId,
      plan,
      (s) => {
        const allIn = allInSeats(s);
        const phase = phaseOf(s);
        return (
          allIn.length >= 2 &&
          phase !== 'HandComplete' &&
          phase !== 'Showdown' &&
          !allIn.includes(Number(s.action_on))
        );
      },
      'two players all in with the action pending on the third',
    );

    const allIn = allInSeats(v);
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v), allInSeats: allIn, actionOn: Number(v.action_on),
        pot: v.pot, handNumber: v.hand_number,
        heroIsAllIn: allIn.includes(SEATS[0].seat),
      },
      notes:
        `seats ${allIn.join(' + ')} all in, action pending on seat ${Number(v.action_on)}, ` +
        `pot ${v.pot} e8s`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.avatar-overlay.allin', { timeout: 30_000 });
    await page.waitForFunction(
      () => document.querySelectorAll('.avatar-overlay.allin').length >= 2,
      undefined,
      { timeout: 30_000 },
    );
    const timer = await stabilizeTimer(page, '.turn-timer', this.timerTarget, 10_000);
    await settle(page);
    return { timer };
  },

  async verify(ctx, page) {
    const allInBadges = await page.locator('.avatar-overlay.allin').count();
    const potText = ((await page.locator('.main-pot').first().textContent()) || '').trim();
    const phaseText = ((await page.locator('.phase-indicator').first().textContent()) || '').trim();
    return {
      verified: allInBadges >= 2,
      checks: { allInBadges, potText, phaseText },
      notes: `${allInBadges} ALL IN badges rendered, pot "${potText.replace(/\s+/g, ' ')}"`,
    };
  },
};
