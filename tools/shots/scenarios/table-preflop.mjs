// Scene: seated, cards dealt, action on us, bet controls live.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { doAct, phaseOf, playUntil, startHand, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2'; // 6-max, 0.05/0.10 ICP, 45s action clock
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const STACK = icp(12);

export default {
  name: 'table-preflop',
  title: 'Pre-flop, hole cards dealt, action on the hero',
  auth: true,
  timerTarget: '40s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
      { player: OPP, seat: OPP_SEAT, chips: STACK },
    ]);

    await startHand(OPP, tableId);

    // The opponent calls/checks whenever it is their turn; we stop the moment the
    // action is on the hero while still pre-flop.
    const v = await playUntil(
      tableId,
      {
        [OPP_SEAT]: { player: OPP, act: (t, sv) => doAct.checkOrCall(t, sv) },
        [HERO_SEAT]: { player: HERO_PLAYER, hold: true },
      },
      (s) => phaseOf(s) === 'PreFlop' && Number(s.action_on) === HERO_SEAT,
      'pre-flop with action on the hero',
    );

    const heroView = await view(HERO_PLAYER, tableId);
    const heroPlayer = heroView.players[HERO_SEAT][0];
    if (!heroPlayer?.hole_cards?.length) throw new Error('Hero has no hole cards');

    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v), actionOn: Number(v.action_on), pot: v.pot,
        handNumber: v.hand_number, heroIsMyTurn: heroView.is_my_turn,
        heroHoleCards: heroPlayer.hole_cards[0].length,
      },
      notes: `pot ${v.pot} e8s, action on seat ${Number(v.action_on)} (hero) pre-flop`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.turn-indicator.my-turn', { timeout: 30_000 });
    await page.waitForSelector('.actions .action-btn', { timeout: 30_000 });
    // Land the countdown on the same digits every run.
    const timer = await stabilizeTimer(page, '.turn-timer', this.timerTarget, 12_000);
    await settle(page);
    return { timer };
  },

  async verify(ctx, page) {
    const myTurn = await page.locator('.turn-indicator.my-turn').count();
    const buttons = await page.locator('.actions .action-btn').allTextContents();
    const heroCards = await page.locator('.player-nameplate.highlight-me').count();
    const phaseText = ((await page.locator('.phase-indicator').first().textContent()) || '').trim();
    const facedown = await page.locator('.community-cards .card').count();
    return {
      verified: myTurn === 1 && buttons.length >= 3 && phaseText.toLowerCase().includes('pre'),
      checks: {
        myTurnBanner: myTurn,
        actionButtons: buttons.map((b) => b.trim()).filter(Boolean),
        heroNameplate: heroCards,
        phaseText,
        communityCardSlots: facedown,
      },
      notes: `action buttons: ${buttons.map((b) => b.trim()).filter(Boolean).join(' / ')}`,
    };
  },
};
