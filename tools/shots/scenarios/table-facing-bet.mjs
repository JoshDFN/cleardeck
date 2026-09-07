// Scene: the hero is FACING A BET. The decision screen.
//
// WHY THIS SCENE EXISTS, AND WHY IT IS NOT COSMETIC
//
// Every other table scene photographs a state in which the hero owes nothing:
// `call_amount` is 0, so the client renders no "Call X" button, no pot-odds
// ratio and no required-equity hint, and the bet-sizing presets are never
// opened. Those four surfaces are exactly the ones a player reads while deciding
// whether to put money in, and until this scene existed the harness had never
// looked at any of them. "No scene reaches it" is the same failure mode as "no
// assertion covers it".
//
// The hero is put on the clock facing a real raise from a real opponent, and
// then the run asserts:
//
//   * "Call X" on the action button and in the turn hint  == call_amount
//   * the pot-odds ratio                                  == get_pot()/call_amount
//   * the required-equity %                               == call/(pot+call)
//   * the ½ Pot and Pot presets                           sized from get_pot()
//
// The preset assertion CLICKS the presets, reads the amount the client proposes,
// and closes the popover again without committing anything. That is deliberate:
// a preset is not a label, it is the number the next click sends to the
// canister, so a wrong pot here is money wagered at the wrong size rather than a
// wrong number read.

import { devLogin, enterTable, openApp, settle, stabilizeTimer } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import {
  assertBetPresetsAgree, assertChainAgreement, named, withAgreement,
} from '../lib/chain-agreement.mjs';
import { doAct, phaseOf, playUntil, startHand, view } from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

/** Flip the opt-in shortcuts preference the way the wallet menu does. */
async function setHotkeysPref(page, enabled) {
  await page.evaluate((on) => {
    localStorage.setItem('poker_hotkeys_enabled', on ? 'true' : 'false');
    window.dispatchEvent(new StorageEvent('storage', { key: 'poker_hotkeys_enabled', newValue: on ? 'true' : 'false' }));
  }, enabled);
}

/**
 * THE HOTKEYS WITH THE ACTION LOG OPEN. Round 2 of the fairness wave made the
 * LOG drawer an ARIA dialog and $lib/hotkeys.js muted every key while it was
 * in the document; no scene played the hero's turn with the log open, so
 * nothing saw it. This opens the log ON THE HERO'S TURN, presses A once
 * (which ARMS the all-in and sends nothing; a second press would), reads the
 * armed state off the button, presses Escape to disarm, and closes the log
 * again before the still. Real money is on the clock, so exactly one press.
 */
async function hotkeysWithLogOpen(page) {
  // THE SHORTCUTS ARE OPT-IN ($lib/hotkeys-pref.js, off by default): the
  // probe turns them on through the same localStorage key the wallet menu
  // writes, and tells the page (a `storage` event is what the shared rune
  // listens for), then turns them off again after the still is safe from a
  // stray key. The legend under the row is photographed in its OFF state.
  await setHotkeysPref(page, true);
  const toggle = page.locator('.action-dock .log-toggle').first();
  await toggle.click();
  await page.waitForSelector('[data-surface="log"] .action-feed', { timeout: 8_000 });
  await settle(page);
  const surface = await page.evaluate(() => {
    const el = document.querySelector('.feed-container');
    return { role: el?.getAttribute('role') ?? null, modal: el?.getAttribute('aria-modal') ?? null };
  });
  const armedBefore = await page.locator('.action-btn.danger.armed').count();
  await page.keyboard.press('a');
  await settle(page, { extraFrames: 1 });
  const armed = await page.locator('.action-btn.danger.armed').count();
  await page.keyboard.press('Escape');
  await settle(page, { extraFrames: 1 });
  const armedAfterEscape = await page.locator('.action-btn.danger.armed').count();
  await toggle.click();
  await page.waitForSelector('[data-surface="log"]', { state: 'detached', timeout: 8_000 });
  await setHotkeysPref(page, false);
  await settle(page);
  const ok = surface.role !== 'dialog' && surface.modal !== 'true' && armedBefore === 0 && armed === 1 && armedAfterEscape === 0;
  return {
    ok,
    surface,
    armedBefore, armedOnA: armed, armedAfterEscape,
    note: ok
      ? 'with the log open, A armed the all-in and Escape disarmed it; the log is not a dialog'
      : `HOTKEYS WITH THE LOG OPEN: role=${surface.role} aria-modal=${surface.modal}; armed ${armedBefore} -> ${armed} on A -> ${armedAfterEscape} on Escape`,
  };
}

const TABLE = 'table_2'; // 6-max
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;
const STACK = icp(12);

export default {
  name: 'table-facing-bet',
  title: 'Facing a bet: call amount, pot odds and bet sizing on screen',
  auth: true,
  timerTarget: '40s',

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: STACK },
      { player: OPP, seat: OPP_SEAT, chips: STACK },
    ]);

    await startHand(OPP, tableId);

    // The opponent RAISES whenever it is their turn, so when the action comes
    // back to the hero there is a genuine amount owed. The raise target is read
    // off the live view (current_bet + min_raise) rather than hardcoded, so this
    // scene does not silently stop raising if the blinds change.
    const v = await playUntil(
      tableId,
      {
        [OPP_SEAT]: {
          player: OPP,
          act: (t, sv) => {
            const to = Number(sv.current_bet) + Number(sv.min_raise);
            return doAct.raise(to)(t);
          },
        },
        [HERO_SEAT]: { player: HERO_PLAYER, hold: true },
      },
      (s) => phaseOf(s) !== 'HandComplete'
        && phaseOf(s) !== 'WaitingForPlayers'
        && Number(s.action_on) === HERO_SEAT,
      'the hero on the clock',
    );

    const heroView = await view(HERO_PLAYER, tableId);
    if (Number(heroView.call_amount) <= 0) {
      throw new Error(
        `Scene precondition failed: the hero is on the clock but owes nothing `
        + `(call_amount=${heroView.call_amount}). This scene exists to photograph a `
        + 'real decision, so it refuses to pass off a check-or-bet spot as one.',
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

    // The hotkeys are tried with the log open FIRST (one arming press, no
    // send), then the presets, while the popover can be opened and closed
    // before the still is taken; assertChainAgreement then photographs the
    // resting screen.
    const hotkeys = await hotkeysWithLogOpen(page);
    const presets = await assertBetPresetsAgree(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER,
    });
    await settle(page);
    const agreement = await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    });

    return withAgreement({
      verified: Boolean(callButton) && hotkeys.ok,
      checks: {
        hotkeysWithLogOpen: hotkeys,
        actionButtons: buttons.map((b) => b.trim()).filter(Boolean),
        callButton,
        potOddsStripRendered: potOddsOnScreen,
        potOddsNote: potOddsOnScreen
          ? 'the client renders a pot-odds ratio; it is asserted against get_pot()/call_amount'
          : 'NO pot-odds strip rendered even though the hero owes a call',
      },
      notes: `facing a call: button "${callButton}"; ${hotkeys.note}`,
    }, agreement, named('betPresets', presets));
  },
};
