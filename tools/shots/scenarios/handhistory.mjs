// Scene: hand history with a completed hand.
//
// HandHistory.svelte reads the table canister's own get_hand_history for hands
// 1..hand_number, so the table needs at least one finished hand. Because the
// table is reset at the start of the scene, that hand is always hand #1.

import { devLogin, enterTable, getAppOrigin, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER } from '../lib/config.mjs';
import { tableSlug } from './handreplay.mjs';
import { phaseOf } from '../lib/table-driver.mjs';
import {
  assertChainAgreement, assertHandHistoryAgreement, named, withAgreement,
} from '../lib/chain-agreement.mjs';
import { foldProtectedNotices, probeProtectedNotices } from '../lib/protected-notices.mjs';
import { heroPrincipal, playCompletedHand, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

/**
 * What `stage` observed about the hand's id, its Copy and its deep link, for
 * `verify` to assert (run.mjs does not hand verify the value stage returned;
 * see handreplay.mjs for the same handoff). Cleared at the top of `stage`.
 */
let observed = null;

/** Opens the history dialog and waits for the list (one bounded re-open). */
async function openHistoryList(page) {
  await page.waitForSelector('.history-btn', { timeout: 30_000 });
  await page.click('.history-btn');
  await page.waitForSelector('.hand-history-modal', { timeout: 30_000 });
  // One bounded re-open: HandHistory kicks off its load from an effect on mount,
  // so a panel opened a beat too early can settle on the empty result. Closing
  // and re-opening remounts it against the now-populated table state. Bounded,
  // and any remaining emptiness is reported by verify() rather than retried away.
  try {
    await page.waitForSelector('.hand-row', { timeout: 8_000 });
  } catch {
    await page.locator('.hand-history-modal .close-btn, .hand-history-modal button')
      .first().click().catch(() => {});
    await page.click('.history-btn');
    await page.waitForSelector('.hand-history-modal', { timeout: 15_000 });
    await page.waitForSelector('.hand-row', { timeout: 20_000 });
  }
}

/** The five community cards of the finished hand, face up on the felt. */
function boardOfFive(page) {
  return page.waitForFunction(
    () => document.querySelectorAll('.community-cards .card:not(.empty):not(.face-down)').length >= 5,
    undefined,
    { timeout: 60_000 },
  );
}

/**
 * Captures what a Copy button hands the clipboard: the headless context has no
 * clipboard permission, so `writeText` is replaced with a recorder.
 */
async function armClipboardRecorder(page) {
  await page.evaluate(() => {
    window.__copied = null;
    const rec = (text) => { window.__copied = String(text); return Promise.resolve(); };
    try {
      Object.defineProperty(navigator, 'clipboard', { value: { writeText: rec }, configurable: true });
    } catch {
      navigator.clipboard.writeText = rec;
    }
  });
}

export default {
  name: 'handhistory',
  title: 'Hand history with a completed hand',
  auth: true,

  async setup(ctx) {
    // One real hand to completion is enough. Verified on this replica: the panel
    // lists a hand the hero LOST as well as one it won ("You lost / Showdown"),
    // because the table's record carries every showdown player's principal, not
    // just the winners'. So there is no need to fish for a win, and the hand the
    // shot shows is whichever one the real shuffle produced.
    const { tableId, view: v } = await playCompletedHand(ctx, TABLE);
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        handNumber: v.hand_number,
        winners: v.last_hand_winners.map((w) => ({
          seat: Number(w.seat),
          principal: w.principal?.toText ? w.principal.toText() : String(w.principal),
          amount: String(w.amount),
        })),
        heroPrincipal: heroPrincipal(),
      },
      notes: `hand #${v.hand_number} recorded on the table canister`,
    };
  },

  async stage(ctx, page) {
    observed = null;
    const tableId = ctx.tableIds[TABLE];
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // WAIT ON REAL STATE, NOT A SLEEP, and specifically on the state HandHistory
    // depends on. `+page.svelte` passes `handNumber={tableState?.hand_number || 0}`,
    // and HandHistory only queries the TABLE canister when `handNumber > 0`;
    // otherwise it falls through to the HISTORY canister, which is empty on this
    // network because nothing in any deploy path ever calls the history
    // canister's `authorize_table`, so `record_hand` is rejected as
    // "Unauthorized: table not registered".
    //
    // Opening the panel before the first `get_table_view` has landed therefore
    // renders "No hands recorded yet" for a hand that demonstrably exists. The
    // five community cards of the finished hand are the viewport-independent
    // signal that the app has the hand: they are on the felt at both 1440px and
    // 390px, unlike `.feed-title` ("Hand #N"), which lives in `.feed-container`
    // and is hidden below 900px.
    //
    // `:not(.empty)` IS LOAD-BEARING. `.community-cards` renders `Array(5)` of
    // <Card> unconditionally (PokerTable.svelte), and an undealt slot is still a
    // `.card`; it just carries `.empty`. So the original form of this wait,
    // `querySelectorAll('.community-cards .card').length >= 5`, was satisfied the
    // instant the board frame mounted, with nothing dealt — a wait that could not
    // fail, standing in for the one thing this scene depends on. docs/DEFECTS.md
    // H-33.
    await boardOfFive(page);
    await openHistoryList(page);

    // --- 1. the row's stable id, and what its Copy hands the clipboard ------
    // Done here, not in verify: the button wears "Copied" for two seconds and
    // the published frame must be the resting list.
    const row = page.locator('.hand-row').first();
    const handNumberText = (await row.locator('.hand-number').textContent())?.trim() ?? '';
    const handNumber = Number((handNumberText.match(/#(\d+)/) || [])[1]);
    const handId = (await row.locator('.hand-id').textContent())?.trim() ?? null;
    await armClipboardRecorder(page);
    await row.locator('.copy-id').click();
    await settle(page, { extraFrames: 1 });
    const copied = await page.evaluate(() => window.__copied ?? null);
    const copyIdAfter = (await row.locator('.copy-id').textContent())?.trim() ?? null;
    const listRowsAfterCopy = await page.locator('.hand-row').count();
    const replayerAfterCopy = await page.locator('.replayer').count();

    // --- 2. the deep link: ?table=<canister>&hand=N opens the replay itself --
    const deepLink = `${getAppOrigin()}/?table=${tableId}&hand=${handNumber}`;
    await page.goto(deepLink, { waitUntil: 'domcontentloaded', timeout: 60_000 });
    let deepLinkTitle = null;
    let deepLinkSearch = null;
    let deepLinkError = null;
    try {
      await page.waitForSelector('.replayer .replay-title .hand-number', { timeout: 60_000 });
      deepLinkTitle = (await page.locator('.replay-title .hand-number').textContent())?.trim() ?? null;
      deepLinkSearch = await page.evaluate(() => window.location.search);
    } catch (e) {
      deepLinkError = e?.message || String(e);
    }
    observed = {
      handNumber, handId, copied, copyIdAfter, listRowsAfterCopy, replayerAfterCopy,
      deepLink, deepLinkTitle, deepLinkSearch, deepLinkError,
    };

    // --- 3. the resting list, for the still --------------------------------
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await boardOfFive(page);
    await openHistoryList(page);
    await settle(page);
    return observed;
  },

  async verify(ctx, page) {
    const rows = await page.locator('.hand-row').count();
    const rowText = ((await page.locator('.hand-row').first().textContent()) || '')
      .replace(/\s+/g, ' ').trim();
    const modal = await page.locator('.hand-history-modal').count();
    // Two surfaces state money here: the table behind the modal, and the modal's
    // own per-hand pot. Both are compared with the canister.
    const tableAgreement = named('table', await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    }));
    const historyAgreement = named('history', await assertHandHistoryAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER,
    }));

    // THE FOUR PROTECTED NOTICES, ON THIS SURFACE, MEASURED.
    // This modal is a fixed overlay behind a 72% scrim, so the page's own banner
    // and footer copy are underneath it and unreadable. HARD RULE 2 is about what
    // a player SEES, on any view, so the dialog now carries the notices itself and
    // this asserts they arrived — by hit-testing their own pixels, which is the
    // one thing neither `make hygiene` nor a geometry probe can do. See
    // lib/protected-notices.mjs.
    const notices = foldProtectedNotices(
      await probeProtectedNotices(page), 'with the hand-history modal open',
    );

    // THE ID, THE COPY AND THE DEEP LINK, as `stage` observed them.
    const staged = observed || {};
    const tableId = ctx.tableIds[TABLE];
    const problems = [];
    if (!staged.handNumber) {
      problems.push('stage() left no observations: the id, the Copy and the deep link were never exercised');
    } else {
      const expectedId = `${tableSlug(tableDisplayName(ctx, TABLE))}#${staged.handNumber}`;
      const expectedTail = `?table=${tableId}&hand=${staged.handNumber}`;
      if (staged.handId !== expectedId) problems.push(`the row's id reads "${staged.handId}", expected "${expectedId}"`);
      if (!staged.copied || !staged.copied.startsWith(`${expectedId} `) || !staged.copied.endsWith(expectedTail)) {
        problems.push(`the row's Copy handed the clipboard ${JSON.stringify(staged.copied)}, expected "${expectedId} <origin>/${expectedTail}"`);
      }
      if (staged.copyIdAfter !== 'Copied') problems.push(`after copying, the row's button reads "${staged.copyIdAfter}", not "Copied"`);
      if (staged.replayerAfterCopy !== 0 || staged.listRowsAfterCopy < 1) {
        problems.push('clicking the row\'s Copy opened the replay (the click must not reach the row)');
      }
      if (staged.deepLinkError) problems.push(`the deep link ${staged.deepLink} did not open a replay: ${staged.deepLinkError}`);
      else if (staged.deepLinkTitle !== `Hand #${staged.handNumber}`) {
        problems.push(`the deep link opened "${staged.deepLinkTitle}", not Hand #${staged.handNumber}`);
      }
      if (staged.deepLinkSearch && !staged.deepLinkSearch.includes(`hand=${staged.handNumber}`)) {
        problems.push(`with the replay open the URL reads "${staged.deepLinkSearch}" without the hand`);
      }
    }

    return withAgreement({
      verified: modal === 1 && rows >= 1 && notices.ok && problems.length === 0,
      checks: {
        modal,
        handRows: rows,
        firstRow: rowText.slice(0, 160),
        handId: staged.handId ?? null,
        copied: staged.copied ?? null,
        copyIdAfter: staged.copyIdAfter ?? null,
        deepLink: staged.deepLink ?? null,
        deepLinkTitle: staged.deepLinkTitle ?? null,
        deepLinkSearch: staged.deepLinkSearch ?? null,
        deepLinkError: staged.deepLinkError ?? null,
        idAndLinkProblems: problems,
        protectedOnScreen: notices.onScreen,
        protectedTotal: notices.total,
        protectedProblems: notices.problems,
        protectedNotices: notices.notices,
      },
      notes: notices.ok && problems.length === 0
        ? `${rows} hand row(s); first: ${rowText.slice(0, 80)}; id ${staged.handId} copied with its link and `
          + `${staged.deepLink} opened the replay on ${staged.deepLinkTitle}; `
          + `${notices.onScreen} of ${notices.total} protected notices measured on screen and unoccluded`
        : `NOT VERIFIED: ${[...problems, ...notices.problems].join(' | ')}`,
    }, tableAgreement, historyAgreement);
  },
};
