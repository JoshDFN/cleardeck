// Scene: the hero is seated and it is NOT their turn. The pre-action row.
//
// WHY THIS SCENE EXISTS. Every other table scene photographs the hero on the
// clock (or the hand over). The state a player spends most of a hand in, waiting
// for somebody else, was never captured, and the decision-loop wave put a
// control surface there: the pre-action toggles (Check / Fold, Check, Call X,
// Call any, Fold to any bet) that fire the real action on the certified my-turn
// edge. One of them is armed before the still is taken, so the armed look and
// the hero plate's tag are on the record too.
//
// Arming a pre-action sends nothing: it is client state until a polled view
// says it is the hero's turn, and the opponent HOLDS for this scene, so nothing
// moves on chain while the browser is watching.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { assertChainAgreement, withAgreement } from '../lib/chain-agreement.mjs';
import { doAct, phaseOf, playUntil, startHand, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2'; // 6-max, 0.05/0.10 ICP, 45s action clock
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const STACK = icp(12);

export default {
  name: 'table-waiting',
  title: 'Waiting for the opponent: the pre-action row, one choice armed',
  auth: true,
  timerTarget: '40s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
      { player: OPP, seat: OPP_SEAT, chips: STACK },
    ]);

    await startHand(OPP, tableId);

    // The hero checks or calls whenever it is their turn; the opponent holds.
    // We stop the moment the action is on the opponent mid-hand.
    const v = await playUntil(
      tableId,
      {
        [HERO_SEAT]: { player: HERO_PLAYER, act: (t, sv) => doAct.checkOrCall(t, sv) },
        [OPP_SEAT]: { player: OPP, hold: true },
      },
      (s) => phaseOf(s) !== 'HandComplete'
        && phaseOf(s) !== 'WaitingForPlayers'
        && Number(s.action_on) === OPP_SEAT,
      'the opponent on the clock',
    );

    const heroView = await view(HERO_PLAYER, tableId);
    if (heroView.is_my_turn) throw new Error('Scene precondition failed: it is the hero\'s turn');

    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v), actionOn: Number(v.action_on), pot: v.pot,
        callAmount: heroView.call_amount, handNumber: v.hand_number,
        heroIsMyTurn: heroView.is_my_turn,
      },
      notes: `pot ${v.pot} e8s, action on seat ${Number(v.action_on)} (the opponent), `
        + `hero owes ${heroView.call_amount} e8s`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.pre-actions .pre-btn', { timeout: 30_000 });

    // Arm the first choice (Check / Fold, or Fold to any bet when a call is
    // owed) so the armed state is photographed. Client state only.
    await page.locator('.pre-actions .pre-btn').first().click();
    await page.waitForSelector('.pre-actions .pre-btn.armed', { timeout: 5_000 });

    const timerPresent = (await page.locator('.turn-timer').count()) > 0
      && (await page.locator('.turn-timer').first().isVisible().catch(() => false));
    const timer = timerPresent
      ? await stabilizeTimer(page, '.turn-timer', this.timerTarget, 12_000)
      : { matched: false, value: null, absent: true };
    await settle(page);
    return { timer };
  },

  async verify(ctx, page) {
    const preButtons = await page.locator('.pre-actions .pre-btn').allTextContents();
    const armed = await page.locator('.pre-actions .pre-btn.armed').allTextContents();
    const plateTag = ((await page.locator('.player-nameplate.highlight-me .plate-tag').first()
      .textContent().catch(() => '')) || '').trim();
    const actionRowLive = (await page.locator('.actions:not(.disabled) .action-btn').count()) > 0;
    const agreement = await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    });
    return withAgreement({
      verified: preButtons.length === 3 && armed.length === 1 && !actionRowLive,
      checks: {
        preActionButtons: preButtons.map((b) => b.trim()).filter(Boolean),
        armed: armed.map((b) => b.trim()),
        heroPlateTag: plateTag,
        primaryRowLive: actionRowLive,
      },
      notes: `pre-actions: ${preButtons.map((b) => b.trim()).filter(Boolean).join(' / ')}; `
        + `armed: ${armed.map((b) => b.trim()).join(', ') || 'none'}; plate tag "${plateTag}"`,
    }, agreement);
  },
};
