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

    // `.actions:not(.disabled)` is the VIEWPORT-INDEPENDENT "it is my turn"
    // signal: PokerTable.svelte writes `class:disabled={!isMyTurn ||
    // !gameInProgress || actionPending}` on the action bar, and the action bar is
    // never hidden by a media query.
    //
    // The old wait was on `.turn-indicator.my-turn`, which lives inside
    // `.feed-container.left`. At <=900px PokerTable sets `.feed-container {
    // display: none }`, so on a 390px viewport that element is invisible and the
    // wait could only ever time out. That is not just a harness problem: the
    // action COUNTDOWN (`.turn-timer`) is inside the same hidden container, so a
    // mobile player has no visible action clock at all.
    await page.waitForSelector('.actions:not(.disabled) .action-btn', { timeout: 30_000 });

    // Land the countdown on the same digits every run — but only where the app
    // actually renders one. Absence is recorded, never silently tolerated.
    const timerPresent = (await page.locator('.turn-timer').count()) > 0
      && (await page.locator('.turn-timer').first().isVisible().catch(() => false));
    const timer = timerPresent
      ? await stabilizeTimer(page, '.turn-timer', this.timerTarget, 12_000)
      : { matched: false, value: null, absent: true };
    await settle(page);
    return { timer };
  },

  async verify(ctx, page) {
    const buttons = await page.locator('.actions .action-btn').allTextContents();
    const actionBarLive = (await page.locator('.actions:not(.disabled)').count()) === 1;
    const heroCards = await page.locator('.player-nameplate.highlight-me').count();
    const phaseText = ((await page.locator('.phase-indicator').first().textContent()) || '').trim();
    const facedown = await page.locator('.community-cards .card').count();
    // Recorded, not asserted: at <=900px these are hidden by a media query.
    const turnIndicatorVisible = await page.locator('.turn-indicator.my-turn').first()
      .isVisible().catch(() => false);
    const actionClockVisible = await page.locator('.turn-timer').first()
      .isVisible().catch(() => false);
    return {
      verified: actionBarLive && buttons.length >= 3 && phaseText.toLowerCase().includes('pre'),
      checks: {
        actionBarLive,
        actionButtons: buttons.map((b) => b.trim()).filter(Boolean),
        heroNameplate: heroCards,
        phaseText,
        communityCardSlots: facedown,
        turnIndicatorVisible,
        actionClockVisible,
        actionClockNote: actionClockVisible
          ? 'action countdown is on screen'
          : 'NO visible action countdown at this viewport: .turn-timer lives in '
            + '.feed-container, which PokerTable hides below 900px',
      },
      notes:
        `action buttons: ${buttons.map((b) => b.trim()).filter(Boolean).join(' / ')}`
        + `; action clock ${actionClockVisible ? 'visible' : 'HIDDEN at this viewport'}`,
    };
  },
};
