// Scene: the provably-fair verification view for a real hand.
//
// The seed is only revealed once the hand ends, so this scene plays a hand to
// completion first and then opens the "Verify Fair" sidebar. The seed hash and
// revealed seed on screen are the canister's own, for the hand that just played.
//
// WHAT THIS SCENE ASSERTS, AND WHY IT CHANGED
// -------------------------------------------
// It used to assert `.proof-item >= 2` and `.hash.revealed >= 1`. Both are
// SCAFFOLDING: rung 1 (the commitment) and rung 2 (the revealed seed) are
// printed straight from the canister's `shuffle_proof` the moment the sidebar
// opens, before the browser has hashed or derived anything. A panel whose
// verification never ran satisfies that assertion perfectly, and that is
// exactly what shipped: two PNGs filed under the canonical verified filename
// showing rungs 3 and 4 grey, the subtitle frozen on "Re-deriving your cards
// locally", no derived-vs-dealt pair, no tally. Filed as H-24.
//
// A fairness panel's only product is a VERDICT, so the verdict is what is
// asserted now:
//
//   1. `.headline.good` exists           the panel committed to an answer
//   2. rungs 3 and 4 are `.done`         both computed steps finished
//   3. computed hash == committed hash == the canister's own `seed_hash`
//   4. derived hole cards == dealt hole cards == the cards on the felt,
//      compared as text, both sides non-empty
//   5. every board cell carries a tick
//   6. the tally reads "N of N cards" with N >= 2
//   7. the deck grid holds 52 slots (expanded, counted, collapsed again so the
//      screenshot is unchanged)
//
// Each of those is false in the dead state, and 4 is false even if the panel
// renders a `=` between two blank cards.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER } from '../lib/config.mjs';
import { optional } from '../lib/agent.mjs';
import { phaseOf, view } from '../lib/table-driver.mjs';
import { assertChainAgreement, withAgreement } from '../lib/chain-agreement.mjs';
import { playCompletedHand, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

/** Card faces as text ("2♠"), from any container of <Card> components. */
async function cardFaces(locator) {
  const faces = await locator.locator('.card').allTextContents();
  return faces.map((t) => t.replace(/\s+/g, ''));
}

const sameFaces = (a, b) =>
  a.length > 0 && a.length === b.length && a.every((f, i) => f.length > 0 && f === b[i]);

export default {
  name: 'shuffleproof',
  title: 'Provably-fair verification for a real hand',
  auth: true,

  async setup(ctx) {
    const { tableId, view: v } = await playCompletedHand(ctx, TABLE);
    const proof = optional(v.shuffle_proof);
    if (!proof) throw new Error('Table returned no shuffle proof after a completed hand');
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(v),
        handNumber: v.hand_number,
        seedHash: proof.seed_hash,
        seedRevealed: optional(proof.revealed_seed) !== null,
      },
      notes:
        `hand #${v.hand_number}; seed hash ${String(proof.seed_hash).slice(0, 16)}…; ` +
        `seed ${optional(proof.revealed_seed) ? 'revealed' : 'NOT revealed'}`,
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));
    await page.waitForSelector('.verify-btn', { timeout: 30_000 });
    await page.click('.verify-btn');
    await page.waitForSelector('.shuffle-proof', { timeout: 30_000 });
    // Wait for the VERDICT, not for the scaffolding. `.headline` appears in both
    // its good and its bad form, so a panel that decides "this hand did not
    // verify" is still staged and still photographed -- it is `verify()` below
    // that separates the two. Only a panel that never finishes at all times out,
    // and a swallowed timeout here would put us straight back in the dead state
    // this scene exists to catch, so it is reported rather than caught.
    await page.waitForSelector('.shuffle-proof .headline', { timeout: 45_000 });
    await settle(page);
  },

  async verify(ctx, page) {
    const panel = page.locator('.shuffle-proof');
    const checks = {};

    // 1 — the verdict itself
    checks.verdictGood = await panel.locator('.headline.good').count();
    checks.verdictBad = await panel.locator('.headline.bad').count();
    checks.verdictText = (await panel.locator('.headline').first().textContent().catch(() => ''))
      ?.trim() ?? '';

    // 2 — every rung finished. Four rungs render once the seed is revealed;
    // rungs 3 and 4 only carry `.done` after the browser has computed them.
    checks.rungsTotal = await panel.locator('.ladder > .rung').count();
    checks.rungsDone = await panel.locator('.ladder > .rung.done').count();

    // 3 — the hash this browser computed, against the commitment the canister
    // published for this hand. The commitment is re-read over Candid in THIS
    // process, so the comparison does not go through the page at all: it is the
    // browser's own arithmetic against the chain's own record. (It says nothing
    // about WHEN the commitment was written — see docs/WAVE-03.md seam 10.)
    const compared = (await panel.locator('.compare-row .hash.small').allTextContents())
      .map((h) => h.replace(/\s+/g, ''));
    const chainProof = optional(
      (await view(HERO_PLAYER, ctx.tableIds[TABLE]))?.shuffle_proof,
    );
    const committedOnChain = chainProof?.seed_hash ? String(chainProof.seed_hash) : null;
    checks.hashComputedHere = compared[0] ?? null;
    checks.hashCommittedShown = compared[1] ?? null;
    checks.hashesIdentical = compared.length === 2 && compared[0] === compared[1];
    checks.hashMatchesCanister = committedOnChain === null
      ? null
      : compared[0] === committedOnChain.replace(/\s+/g, '');

    // 4 — the player's own two cards, derived locally vs dealt vs on the felt
    const hole = panel.locator('.rederive').first();
    const derived = await cardFaces(hole.locator('.side').nth(0));
    const dealt = await cardFaces(hole.locator('.side').nth(1));
    const onFelt = await cardFaces(page.locator('.player-cards.hero').first());
    checks.holeDerived = derived;
    checks.holeDealt = dealt;
    checks.holeOnFelt = onFelt;
    checks.holeDerivedMatchesDealt = sameFaces(derived, dealt);
    checks.holeDerivedMatchesFelt = onFelt.length === 2 ? sameFaces(derived, onFelt) : null;
    checks.equalsSignGood = await hole.locator('.equals.good').count();

    // 5 — the board, cell by cell
    checks.boardCells = await panel.locator('.board-cell').count();
    checks.boardCellsGood = await panel.locator('.board-cell.good').count();

    // 6 — the tally line
    const tallyText = (await panel.locator('.tally').first().textContent().catch(() => ''))?.trim() ?? '';
    const tally = tallyText.match(/(\d+)\s+of\s+(\d+)\s+cards/);
    checks.tallyText = tallyText.slice(0, 120);
    checks.cardsMatched = tally ? Number(tally[1]) : 0;
    checks.cardsChecked = tally ? Number(tally[2]) : 0;
    checks.tallyGood = await panel.locator('.tally.good').count();

    // 7 — the deck grid, counted behind "Show the work" and put back exactly as
    // it was, so the PNG this run publishes is still the collapsed panel.
    //
    // The toggle is driven with `el.click()` inside the page rather than
    // Playwright's `click()`, because Playwright scrolls a target into view
    // first: expanding a 52-slot grid and then collapsing it left the sidebar
    // scrolled hundreds of pixels down, and the published viewport PNG showed
    // an empty felt. `el.click()` fires the same handler and moves nothing. The
    // scroll offsets are snapshotted and restored anyway, so a future change to
    // this block cannot silently reframe the artifact.
    let deckSlots = 0;
    const workToggle = panel.locator('.section-toggle', { hasText: 'Show the work' }).first();
    if (await workToggle.count()) {
      const scrollBefore = await page.evaluate(() => {
        const chain = [];
        for (let el = document.querySelector('.shuffle-proof'); el; el = el.parentElement) {
          chain.push(el.scrollTop);
        }
        return { win: [window.scrollX, window.scrollY], chain };
      });

      await workToggle.evaluate((el) => el.click());
      await panel.locator('.deck-grid').waitFor({ state: 'attached', timeout: 10_000 }).catch(() => {});
      deckSlots = await panel.locator('.deck-grid .slot').count();
      await workToggle.evaluate((el) => el.click());
      await panel.locator('.deck-grid').waitFor({ state: 'detached', timeout: 10_000 }).catch(() => {});

      await page.evaluate((snap) => {
        let i = 0;
        for (let el = document.querySelector('.shuffle-proof'); el; el = el.parentElement, i += 1) {
          if (typeof snap.chain[i] === 'number') el.scrollTop = snap.chain[i];
        }
        window.scrollTo(snap.win[0], snap.win[1]);
      }, scrollBefore);
      await settle(page);
    }
    checks.deckSlots = deckSlots;

    const verified =
      checks.verdictGood === 1
      && checks.verdictBad === 0
      && checks.rungsTotal === 4
      && checks.rungsDone === 4
      && checks.hashesIdentical
      && checks.hashMatchesCanister !== false
      && checks.holeDerivedMatchesDealt
      && checks.holeDerivedMatchesFelt !== false
      && checks.equalsSignGood === 1
      && checks.boardCells >= 3
      && checks.boardCells === checks.boardCellsGood
      && checks.tallyGood === 1
      && checks.cardsChecked >= 2
      && checks.cardsMatched === checks.cardsChecked
      && checks.deckSlots === 52;

    // The fairness panel is not a money surface, but the table underneath it is,
    // and it is on screen in the same PNG.
    const agreement = await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true, requireWinnerBanner: true,
    });

    const notes = verified
      ? `verdict reached: ${checks.cardsMatched}/${checks.cardsChecked} cards re-derived in the browser, `
        + `hole cards ${derived.join(' ')} derived = dealt = on the felt, 52-slot deck, `
        + `computed hash equals the canister's commitment`
      : `NO VERDICT: headline good=${checks.verdictGood} bad=${checks.verdictBad}; `
        + `rungs ${checks.rungsDone}/${checks.rungsTotal} done; `
        + `hole derived [${derived.join(' ') || '-'}] vs dealt [${dealt.join(' ') || '-'}] `
        + `vs felt [${onFelt.join(' ') || '-'}]; board ${checks.boardCellsGood}/${checks.boardCells}; `
        + `tally ${checks.cardsMatched}/${checks.cardsChecked}; deck slots ${checks.deckSlots}`;

    return withAgreement({ verified, checks, notes }, agreement);
  },
};
