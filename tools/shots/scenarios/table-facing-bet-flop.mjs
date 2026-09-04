// Scene: the hero is FACING A BET ON THE FLOP. The post-flop decision screen.
//
// WHY A SECOND FACING-BET SCENE. table-facing-bet photographs the pre-flop
// decision, and since the sizer became street-aware its row is Min / 2.5x /
// 3x / 4x / Pot / All in there: the pot FRACTIONS (½ Pot, ⅔ Pot) are only on
// screen after the flop. The two-thirds preset is the one that failed the
// never-round-differently rule (it proposed 46,666,666 e8s and showed 0.47),
// so the harness needs a scene where it exists to click. This is that scene:
// a board is out, the opponent has bet into the hero, and the run asserts
//
//   * "Call X" on the action button and in the turn hint  == call_amount
//   * the pot-odds ratio                                  == get_pot()/call_amount
//   * the ½ Pot, ⅔ Pot and Pot presets                    sized from get_pot(),
//                                                         on the display grid
//   * the board                                           == get_community_cards()
//
// The preset assertion CLICKS the presets, reads the amount the client proposes,
// and puts the sizer back without committing anything.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import {
  assertBetPresetsAgree, assertChainAgreement, named, withAgreement,
} from '../lib/chain-agreement.mjs';
import { doAct, phaseOf, playUntil, startHand, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2'; // 6-max
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const STACK = icp(12);

export default {
  name: 'table-facing-bet-flop',
  title: 'Facing a bet on the flop: the pot-fraction presets on screen',
  auth: true,
  timerTarget: '40s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
      { player: OPP, seat: OPP_SEAT, chips: STACK },
    ]);

    await startHand(OPP, tableId);

    // Pre-flop the hero calls whatever comes and the opponent min-raises once
    // per turn; on the flop the hero checks if allowed and the opponent bets
    // (or raises) the minimum. The hand stops on the FLOP with the action on
    // the hero and a bet in front: current_bet > 0 while the hero has nothing
    // in on this street, so the hero owes. Sizes are read off the live view
    // (min_bet, current_bet + min_raise), never hardcoded.
    const oppAct = (t, sv) => {
      if (Number(sv.current_bet) === 0) return doAct.bet(Number(sv.min_bet))(t);
      return doAct.raise(Number(sv.current_bet) + Number(sv.min_raise))(t);
    };
    const heroAct = (t, sv) => {
      if (phaseOf(sv) === 'PreFlop') return doAct.checkOrCall(t, sv);
      if (sv.can_check) return doAct.check(t);
      return 'hold';
    };
    const v = await playUntil(
      tableId,
      {
        [HERO_SEAT]: { player: HERO_PLAYER, act: heroAct },
        [OPP_SEAT]: { player: OPP, act: oppAct },
      },
      (s) => phaseOf(s) === 'Flop'
        && Number(s.action_on) === HERO_SEAT
        && Number(s.current_bet) > 0,
      'the hero on the clock facing a bet on the flop',
    );

    const heroView = await view(HERO_PLAYER, tableId);
    if (Number(heroView.call_amount) <= 0) {
      throw new Error(
        `Scene precondition failed: the hero is on the clock but owes nothing `
        + `(call_amount=${heroView.call_amount}). This scene exists to photograph a `
        + 'real post-flop decision, so it refuses to pass off a check-or-bet spot as one.',
      );
    }

    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        actionOn: Number(v.action_on),
        pot: v.pot,
        callAmount: heroView.call_amount,
        currentBet: v.current_bet,
        minRaise: v.min_raise,
        handNumber: v.hand_number,
        potOddsFromChain:
          `${(Number(v.pot) / Number(heroView.call_amount)).toFixed(2)}:1`,
      },
      notes:
        `pot ${v.pot} e8s, hero owes ${heroView.call_amount} e8s `
        + `(true pot odds ${(Number(v.pot) / Number(heroView.call_amount)).toFixed(2)}:1)`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.actions:not(.disabled) .action-btn', { timeout: 30_000 });

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
    const callButton = buttons.map((b) => b.trim()).find((b) => /^call/i.test(b)) ?? null;
    const potOddsOnScreen = (await page.locator('.pot-odds-display').count()) > 0;

    // The presets are read FIRST, while the popover can be opened and closed
    // before the still is taken; assertChainAgreement then photographs the
    // resting screen.
    const presets = await assertBetPresetsAgree(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER,
    });
    await settle(page);
    const agreement = await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    });

    const presetLabels = await page.locator('.raise-slider-panel .preset-buttons button').allTextContents();
    const fractionsOnScreen = presetLabels.some((l) => /⅔|2\/3/.test(l)) && presetLabels.some((l) => /½|1\/2/.test(l));
    return withAgreement({
      verified: Boolean(callButton) && fractionsOnScreen,
      checks: {
        actionButtons: buttons.map((b) => b.trim()).filter(Boolean),
        callButton,
        presetLabels: presetLabels.map((l) => l.trim()),
        fractionsOnScreen,
        potOddsStripRendered: potOddsOnScreen,
        potOddsNote: potOddsOnScreen
          ? 'the client renders a pot-odds ratio; it is asserted against get_pot()/call_amount'
          : 'NO pot-odds strip rendered even though the hero owes a call',
      },
      notes: `facing a call on the flop: button "${callButton}"; presets ${presetLabels.map((l) => l.trim()).join(' / ')}`,
    }, agreement, named('betPresets', presets));
  },
};
