// Scene: multiple side pots from all-ins at different stack sizes.
//
// WHY FOUR PLAYERS AND NOT THREE
// The task asked for 3 players with 2 all-ins. The engine makes that state
// impossible to photograph: once two of three players are all in, only one
// player can still act, so advance_to_next_street runs the board out and calls
// determine_winners, which sets phase = HandComplete and then
// `state.side_pots.clear()`. PokerTable.svelte only renders `.side-pots` while a
// hand is in progress (at HandComplete the winner banner replaces the pot
// display), so the side-pot breakdown is never on screen in a resting state.
//
// With four players, the two short stacks go all in, the two deep stacks call,
// side pots are calculated at the end of the pre-flop round, and the hand rests
// on the flop with two players still able to act — side pots visible on screen.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { doAct, phaseOf, playUntil, startHand } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_3'; // 9-max, min buy-in 20 ICP, max 100 ICP
const SHORT = [
  { player: HERO_PLAYER, seat: 0, chips: icp(20) }, // shortest stack: hero
  { player: 2, seat: 1, chips: icp(45) },           // middle stack
];
const DEEP = [
  { player: 3, seat: 2, chips: icp(95) },
  { player: 4, seat: 3, chips: icp(95) },
];

export default {
  name: 'table-sidepots',
  title: 'Multiple side pots (all-ins at different stack sizes)',
  auth: true,
  timerTarget: '50s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [...SHORT, ...DEEP]);
    await startHand(3, tableId);

    const plan = {};
    for (const s of SHORT) plan[s.seat] = { player: s.player, act: (t) => doAct.allIn(t) };
    for (const s of DEEP) plan[s.seat] = { player: s.player, act: (t, v) => doAct.checkOrCall(t, v) };

    const v = await playUntil(
      tableId,
      plan,
      (s) => s.side_pots.length >= 2 && phaseOf(s) !== 'PreFlop' && phaseOf(s) !== 'HandComplete',
      'two or more side pots with the hand still live',
      { timeoutMs: 90_000 },
    );

    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        sidePots: v.side_pots.map((p) => p.amount),
        pot: v.pot,
        actionOn: Number(v.action_on),
        handNumber: v.hand_number,
      },
      notes:
        `${v.side_pots.length} side pots (${v.side_pots.map((p) => p.amount).join(', ')} e8s) ` +
        `at ${phaseOf(v)}, action on seat ${Number(v.action_on)}`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForFunction(
      () => document.querySelectorAll('.side-pot').length >= 2,
      undefined,
      { timeout: 30_000 },
    );
    const timer = await stabilizeTimer(page, '.turn-timer', this.timerTarget, 8_000);
    await settle(page);
    return { timer };
  },

  async verify(ctx, page) {
    const sidePots = await page.locator('.side-pot').allTextContents();
    const allInBadges = await page.locator('.avatar-overlay.allin').count();
    const phaseText = ((await page.locator('.phase-indicator').first().textContent()) || '').trim();
    return {
      verified: sidePots.length >= 2,
      checks: {
        sidePotsRendered: sidePots.map((t) => t.replace(/\s+/g, ' ').trim()),
        allInBadges,
        phaseText,
      },
      notes: `${sidePots.length} side pots rendered, ${allInBadges} ALL IN badges, phase ${phaseText}`,
    };
  },
};
