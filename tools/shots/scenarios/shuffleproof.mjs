// Scene: the provably-fair verification view for a real hand.
//
// The seed is only revealed once the hand ends, so this scene plays a hand to
// completion first and then opens the "Verify Fair" sidebar. The seed hash and
// revealed seed on screen are the canister's own, for the hand that just played.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER } from '../lib/config.mjs';
import { optional } from '../lib/agent.mjs';
import { phaseOf } from '../lib/table-driver.mjs';
import { playCompletedHand, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

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
    await page.waitForSelector('.proof-item', { timeout: 30_000 });
    await settle(page);
  },

  async verify(ctx, page) {
    const panel = await page.locator('.shuffle-proof').count();
    const items = await page.locator('.proof-item').count();
    const revealed = await page.locator('.hash.revealed').count();
    const hashes = await page.locator('.proof-item .hash').allTextContents();
    return {
      verified: panel >= 1 && items >= 2 && revealed >= 1,
      checks: {
        proofPanels: panel,
        proofItems: items,
        revealedSeedShown: revealed,
        hashes: hashes.map((h) => h.trim()),
      },
      notes: `${items} proof rows, revealed seed ${revealed ? 'shown' : 'MISSING'}`,
    };
  },
};
