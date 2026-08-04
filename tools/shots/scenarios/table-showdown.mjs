// Scene: showdown — winner shown, winning hand named, pot awarded.
//
// The hand is frozen on screen without any stubbing: the opponent asks the
// canister to sit out after this hand (sit_out_next_hand), so once the hand ends
// there are fewer than two active players and the canister's 3-second auto-deal
// cannot start the next hand. The completed hand therefore stays on screen.
//
// This scene also carries the all-in -> showdown motion capture.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import {
  allInSeats, doAct, phaseOf, playUntil, sitOutAfterHand, startHand, tableActorFor, view,
  waitForState,
} from '../lib/table-driver.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';
const HERO_SEAT = 0;
const OPP = 2;
const OPP_SEAT = 1;

export default {
  name: 'table-showdown',
  title: 'Showdown — winner, winning hand and pot award',
  auth: true,
  video: true,
  motionFrames: 26,
  motionEveryMs: 140,

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: icp(12) },
      { player: OPP, seat: OPP_SEAT, chips: icp(18) },
    ]);
    await startHand(OPP, tableId);

    // Keep the finished hand on screen: with the opponent sitting out after this
    // hand, auto-deal cannot find two active players.
    await sitOutAfterHand([OPP], tableId);

    // Drive to "hero all in, opponent still to act" so the browser can watch the
    // final call turn into a showdown. The opponent must be given a real action:
    // heads-up, the small blind acts first pre-flop and that may be the opponent,
    // so a plan that only ever holds their seat would deadlock. stopWhen is
    // evaluated before any action, so the opponent never gets to end the hand.
    const v = await playUntil(
      tableId,
      {
        [HERO_SEAT]: { player: HERO_PLAYER, act: (t) => doAct.allIn(t) },
        [OPP_SEAT]: { player: OPP, act: (t, sv) => doAct.checkOrCall(t, sv) },
      },
      (s) => allInSeats(s).includes(HERO_SEAT) && Number(s.action_on) === OPP_SEAT,
      'hero all in with the opponent still to act',
    );

    return {
      table: TABLE,
      tableId,
      onChain: { phase: phaseOf(v), allInSeats: allInSeats(v), pot: v.pot, handNumber: v.hand_number },
      notes: 'staged at hero all-in; the showdown itself happens while the browser is watching',
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.avatar-overlay.allin', { timeout: 30_000 });
    return {};
  },

  /**
   * Completes the hand on-chain while the browser watches, optionally capturing
   * a frame burst of the all-in -> showdown transition.
   */
  async advance(ctx, page, { burst } = {}) {
    const tableId = ctx.tableIds[TABLE];
    const opponent = await tableActorFor(OPP, tableId);

    const burstPromise = burst ? burst() : Promise.resolve([]);
    // Small head start so the burst captures the pre-showdown frame first.
    await page.waitForTimeout(250);
    await doAct.call(opponent);

    const v = await waitForState(
      HERO_PLAYER,
      tableId,
      (s) => phaseOf(s) === 'HandComplete' && s.last_hand_winners.length > 0,
      'hand complete with winners',
    );

    await page.waitForSelector('.winner-display', { timeout: 30_000 });
    const frames = await burstPromise;
    await settle(page);

    return {
      frames,
      onChain: {
        phase: phaseOf(v),
        winners: v.last_hand_winners.map((w) => ({
          seat: Number(w.seat), amount: w.amount, handRank: Object.keys(w.hand_rank?.[0] ?? {})[0] ?? null,
        })),
        handNumber: v.hand_number,
      },
    };
  },

  async verify(ctx, page) {
    const winnerVisible = await page.locator('.winner-display').count();
    const winnerText = ((await page.locator('.winner-display').first().textContent()) || '')
      .replace(/\s+/g, ' ').trim();
    const revealedCards = await page.locator('.player-cards .card').count();
    const v = await view(HERO_PLAYER, ctx.tableIds[TABLE]);
    return {
      verified: winnerVisible === 1 && phaseOf(v) === 'HandComplete' && v.last_hand_winners.length > 0,
      checks: {
        winnerBanner: winnerVisible,
        winnerText,
        revealedCardElements: revealedCards,
        onChainPhase: phaseOf(v),
        onChainWinners: v.last_hand_winners.length,
      },
      notes: winnerText.slice(0, 120),
    };
  },
};
