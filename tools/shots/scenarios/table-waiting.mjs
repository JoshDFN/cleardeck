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
// says it is the hero's turn, and the opponents HOLD for this scene, so nothing
// moves on chain while the browser is watching.
//
// THREE-HANDED, SO THE HERO OWES. Heads-up, the hero owes money only when it
// is the hero's turn, so the two-seat version of this scene could only ever
// arm the digit-free "Check / Fold" toggle and the money figure on the hero's
// plate ("CALL 0.10") was never photographed or asserted (code review,
// 2026-09-04). With a third seat an opponent raises, the next opponent is on
// the clock, and the hero is behind a real bet: the "Call X" toggle is armed
// and its figure rides the plate tag, where the census now asserts it.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { assertChainAgreement, withAgreement } from '../lib/chain-agreement.mjs';
import { doAct, phaseOf, playUntil, startHand, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2'; // 6-max, 0.05/0.10 ICP, 45s action clock
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const OPP2 = 3;
const OPP2_SEAT = 2;
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
      { player: OPP2, seat: OPP2_SEAT, chips: STACK },
    ]);

    await startHand(OPP, tableId);

    // The hero calls whenever it is their turn; every opponent min-raises on
    // theirs. Whichever seat the button starts on, within three actions an
    // opponent has raised and the OTHER opponent is on the clock with the
    // hero behind the raise. The plan lists the hero first so the view the
    // stop rule reads is the hero's (call_amount is per viewer).
    const minRaiseTo = (t, sv) => doAct.raise(Number(sv.current_bet) + Number(sv.min_raise))(t);
    const v = await playUntil(
      tableId,
      {
        [HERO_SEAT]: { player: HERO_PLAYER, act: (t, sv) => doAct.checkOrCall(t, sv) },
        [OPP_SEAT]: { player: OPP, act: minRaiseTo },
        [OPP2_SEAT]: { player: OPP2, act: minRaiseTo },
      },
      (s) => phaseOf(s) !== 'HandComplete'
        && phaseOf(s) !== 'WaitingForPlayers'
        && Number(s.action_on) !== HERO_SEAT
        && Number(s.call_amount) > 0,
      'an opponent on the clock with the hero behind a raise',
    );

    const heroView = await view(HERO_PLAYER, tableId);
    if (heroView.is_my_turn) throw new Error('Scene precondition failed: it is the hero\'s turn');
    if (Number(heroView.call_amount) <= 0) {
      throw new Error(
        `Scene precondition failed: the hero owes nothing (call_amount=${heroView.call_amount}), `
        + 'so the "Call X" pre-action and the plate tag figure cannot be photographed.',
      );
    }

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

    // Arm the "Call X" toggle (the one that carries a money figure) so the
    // armed state AND the figure on the hero's plate are photographed. Falls
    // back to the first toggle only if no call is owed, which setup forbids.
    // Client state only: nothing is sent.
    const callToggle = page.locator('.pre-actions .pre-btn', { hasText: /^\s*Call\s+\d/i });
    if (await callToggle.count()) await callToggle.first().click();
    else await page.locator('.pre-actions .pre-btn').first().click();
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
    const armedCarriesFigure = armed.some((a) => /^\s*call\s+\d/i.test(a));
    const plateTagCarriesFigure = /\d/.test(plateTag);
    return withAgreement({
      verified: preButtons.length === 3 && armed.length === 1 && !actionRowLive
        && armedCarriesFigure && plateTagCarriesFigure,
      checks: {
        preActionButtons: preButtons.map((b) => b.trim()).filter(Boolean),
        armed: armed.map((b) => b.trim()),
        heroPlateTag: plateTag,
        plateTagCarriesFigure,
        primaryRowLive: actionRowLive,
      },
      notes: `pre-actions: ${preButtons.map((b) => b.trim()).filter(Boolean).join(' / ')}; `
        + `armed: ${armed.map((b) => b.trim()).join(', ') || 'none'}; plate tag "${plateTag}"`,
    }, agreement);
  },
};
