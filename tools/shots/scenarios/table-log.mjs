// Scene: the LOG drawer open on a settled hand, the deck seal CHECKED.
//
// WHY THIS SCENE EXISTS
// ---------------------
// The fairness wave gave the live table two things no scene had photographed:
// the deck object on the felt (DeckSeal.svelte) that reads SEALED while a hand
// runs and CHECKED once this browser has hashed the revealed seed, and the
// action log's seal chip with its colour-coded verbs (ActionFeed.svelte). The
// drawer is closed on every other table scene, so until this one the log's
// figures had never been through the token census either.
//
// WHAT IS ASSERTED
// ----------------
//   1. THE SEAL'S PHASES, SEEN. `stage` waits for the deck to read `sealed`
//      while the hand is live, plays the hand out under the browser's eyes, and
//      requires it to reach `checked` (never `mismatch`, never stuck at
//      `revealed`). The chip in the drawer must agree and must carry the
//      fingerprint of the canister's own seed_hash.
//   2. THE LOG'S FIGURES. Every amount in the drawer is matched to the hand
//      record: the blind posts to the table's config, each action line to the
//      canister's ActionRecord in order (the feed is fed by polling, so it is
//      asserted as an in-order subsequence of the record, and every line it
//      does show must be one the record has), each "won" line to the winners.
//      An amount that matches nothing fails the scene.
//   3. THE VERBS ARE COLOUR-CODED CLASSES, not emoji: each action line carries
//      one of the `action-*` classes and a verb.
//   4. The table behind the drawer is a money surface too (assertChainAgreement),
//      and the census, the felt floor, the occlusion gate and the notices run
//      centrally as for every scene.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { optional } from '../lib/agent.mjs';
import {
  doAct, phaseOf, playUntil, sitOutAfterHand, startHand,
} from '../lib/table-driver.mjs';
import { assertChainAgreement, named, withAgreement } from '../lib/chain-agreement.mjs';
import { handRecordActor } from '../lib/hand-record-wire.mjs';
import { checkFigure, foldFigures } from '../lib/money.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';
const HERO_SEAT = 0;
const OPP_PLAYER = 2;
const OPP_SEAT = 1;

/** The bet each post-flop street opens with, in e8s; distinct so a token can only match one check. */
const OPENING_BET = { Flop: 10_000_000, Turn: 20_000_000, River: 40_000_000 };

/** The verb (ActionFeed's ACTION_VERBS) and class each record kind must be logged with. */
const FEED_OF_KIND = {
  Fold: { verb: 'folded', cls: 'action-fold', amount: false },
  Check: { verb: 'checked', cls: 'action-check', amount: false },
  Call: { verb: 'called', cls: 'action-call', amount: true },
  Bet: { verb: 'bet', cls: 'action-bet', amount: true },
  Raise: { verb: 'raised to', cls: 'action-raise', amount: true },
  AllIn: { verb: 'went ALL IN', cls: 'action-allin', amount: true },
};

const VERB_CLASSES = new Set(['action-fold', 'action-check', 'action-call', 'action-bet', 'action-raise', 'action-allin', 'action-blind']);

/** first 8 + ellipsis + last 8, as lib/hash-seal.js hashFingerprint prints it. */
const fingerprint = (hex) => {
  const h = String(hex || '').toLowerCase();
  return h.length <= 16 ? h : `${h.slice(0, 8)}…${h.slice(-8)}`;
};

let observed = null;

/** Bets when first to act on a street, calls when facing one, never folds. */
const streetByStreet = async (t, v) => {
  const phase = phaseOf(v);
  if (phase !== 'PreFlop' && v.can_check && OPENING_BET[phase]) {
    return doAct.bet(OPENING_BET[phase])(t);
  }
  return doAct.checkOrCall(t, v);
};

async function chainRecord(tableId, handNumber) {
  const table = await handRecordActor(HERO_PLAYER, tableId);
  const rec = optional(await table.get_hand_history(BigInt(handNumber)));
  if (!rec) throw new Error(`the table canister has no record of hand ${handNumber}`);
  const v = optional(await table.get_table_view());
  if (!v) throw new Error('the table canister returned no view');
  return {
    handNumber: Number(rec.hand_number),
    seedHash: String(rec.shuffle_proof.seed_hash),
    seedRevealed: optional(rec.shuffle_proof.revealed_seed),
    actions: rec.actions.map((a) => ({
      kind: Object.keys(a.action)[0], seat: Number(a.seat), amount: Number(a.amount),
    })),
    winners: rec.winners.map((w) => ({ seat: Number(w.seat), amount: Number(w.amount) })),
    blinds: { small: Number(v.config.small_blind), big: Number(v.config.big_blind) },
    phase: Object.keys(v.phase)[0],
  };
}

function scrapeDrawer(page) {
  return page.evaluate(() => {
    const t = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
    const chip = document.querySelector('.feed-container .fairness-indicator');
    const deck = document.querySelector('.poker-table .deck-seal');
    return {
      drawerOpen: document.querySelectorAll('.feed-container .action-feed').length,
      chipState: chip ? [...chip.classList].find((c) => c.startsWith('state-'))?.slice(6) ?? null : null,
      chipWord: t(chip?.querySelector('.seal-state')),
      chipFingerprint: t(chip?.querySelector('.hash-value')),
      deckState: deck?.getAttribute('data-seal') ?? null,
      deckWord: t(deck?.querySelector('.deck-state')),
      deckTitle: deck?.getAttribute('title') ?? null,
      feedTitle: t(document.querySelector('.feed-container .feed-title')),
      lines: [...document.querySelectorAll('.feed-container .feed-item')].map((el) => ({
        cls: [...el.classList].find((c) => c.startsWith('action-')) || null,
        seat: t(el.querySelector('.player-name, .winner-name')),
        verb: t(el.querySelector('.action-text, .phase-text, .showdown-text')),
        amount: t(el.querySelector('.action-amount')),
        icons: el.querySelectorAll('.action-icon').length,
      })),
    };
  });
}

export default {
  name: 'table-log',
  title: 'The LOG drawer on a settled hand: the deck seal checked, every line a colour-coded verb',
  auth: true,

  async setup(ctx) {
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: icp(12) },
      { player: OPP_PLAYER, seat: OPP_SEAT, chips: icp(18) },
    ]);
    await startHand(OPP_PLAYER, tableId);
    // With the opponent sitting out after this hand, auto-deal cannot find two
    // active players and the settled hand stays on screen.
    await sitOutAfterHand([OPP_PLAYER], tableId);
    return {
      table: TABLE,
      tableId,
      onChain: { note: 'a hand dealt and left LIVE; it is played out while the browser watches' },
      notes: `a live hand on ${TABLE}; played to a showdown under the browser's eyes so the log fills`,
    };
  },

  async stage(ctx, page) {
    observed = null;
    const tableId = ctx.tableIds[TABLE];
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // 1. the deck reads SEALED while the hand is live
    await page.waitForSelector('.deck-seal[data-seal="sealed"]', { timeout: 60_000 });
    const sealedWord = (await page.locator('.deck-seal .deck-state').textContent())?.trim() ?? null;

    // 2. the hand plays out street by street, the browser watching every action
    const finished = await playUntil(
      tableId,
      {
        [HERO_SEAT]: { player: HERO_PLAYER, act: streetByStreet },
        [OPP_SEAT]: { player: OPP_PLAYER, act: streetByStreet },
      },
      (s) => phaseOf(s) === 'HandComplete' && s.last_hand_winners.length > 0,
      'the hand, street by street, to a showdown',
      { timeoutMs: 150_000 },
    );
    await page.waitForSelector('.winner-display', { timeout: 30_000 });

    // 3. the reveal lands and this browser checks it
    await page.waitForSelector('.deck-seal[data-seal="checked"]', { timeout: 30_000 });

    // 4. the drawer
    await page.locator('.action-dock .log-toggle').first().click();
    await page.waitForSelector('.feed-container .fairness-indicator.state-checked', { timeout: 15_000 });
    await settle(page);
    observed = { handNumber: Number(finished.hand_number), sealedWord };
    return observed;
  },

  async verify(ctx, page) {
    const staged = observed || {};
    const tableId = ctx.tableIds[TABLE];
    if (!staged.handNumber) throw new Error('stage() left no observations for verify()');
    const chain = await chainRecord(tableId, staged.handNumber);
    const dom = await scrapeDrawer(page);
    const problems = [];
    const figures = [];
    const checks = { handNumber: chain.handNumber, sealedWordMidHand: staged.sealedWord };

    // ---- 1. the seal's phases -------------------------------------------
    if (staged.sealedWord !== 'Sealed') problems.push(`mid-hand the deck read "${staged.sealedWord}", not "Sealed"`);
    if (chain.seedRevealed === null) problems.push('the chain has no revealed seed for this hand');
    checks.deckState = dom.deckState;
    checks.deckWord = dom.deckWord;
    checks.chipState = dom.chipState;
    checks.chipWord = dom.chipWord;
    checks.chipFingerprint = dom.chipFingerprint;
    checks.fingerprintExpected = fingerprint(chain.seedHash);
    if (dom.deckState !== 'checked') problems.push(`the deck on the felt reads "${dom.deckState}", not "checked"`);
    if (!/Checked/.test(dom.deckWord || '')) problems.push(`the deck's word is "${dom.deckWord}"`);
    if (dom.chipState !== 'checked') problems.push(`the drawer's seal chip reads "${dom.chipState}", not "checked"`);
    if (dom.chipFingerprint !== fingerprint(chain.seedHash)) {
      problems.push(`the chip's fingerprint "${dom.chipFingerprint}" is not the canister's seed_hash (${fingerprint(chain.seedHash)})`);
    }
    if (!(dom.deckTitle || '').includes(fingerprint(chain.seedHash))) {
      problems.push('the deck\'s hover title does not carry the commitment\'s fingerprint');
    }
    if (dom.drawerOpen !== 1) problems.push(`${dom.drawerOpen} drawer(s) open, expected 1`);
    if (dom.feedTitle !== `Hand #${chain.handNumber}`) problems.push(`the drawer is titled "${dom.feedTitle}", not Hand #${chain.handNumber}`);

    // ---- 2. every figure in the log, matched to the record ---------------
    checks.lines = dom.lines;
    const blindPool = [chain.blinds.small, chain.blinds.big];
    let nextAction = 0;
    let winnerAt = 0;
    dom.lines.forEach((line, i) => {
      const n = i + 1;
      if (line.icons) problems.push(`feed line ${n} still carries ${line.icons} emoji icon(s)`);
      if (line.cls === 'action-phase' || line.cls === 'action-showdown') {
        if (line.amount !== null) problems.push(`feed line ${n} (${line.cls}) shows an amount "${line.amount}"`);
        return;
      }
      if (line.cls === 'action-blind') {
        const at = blindPool.findIndex((b) => checkFigure('probe', b, line.amount, { currency: 'ICP' }).ok);
        if (at < 0) { problems.push(`feed line ${n} posts a blind of "${line.amount}" that is neither blind in the table's config`); return; }
        figures.push(checkFigure(`feed line ${n} amount vs get_table_view().config blind`, blindPool[at], line.amount, { currency: 'ICP' }));
        blindPool.splice(at, 1);
        return;
      }
      if (line.cls === 'action-winner') {
        const w = chain.winners[winnerAt];
        winnerAt += 1;
        if (!w) { problems.push(`feed line ${n} names a winner the record does not have`); return; }
        if (!String(line.seat || '').startsWith(`Seat ${w.seat + 1}`) && line.seat !== 'You') {
          problems.push(`feed line ${n} names "${line.seat}" as a winner, not Seat ${w.seat + 1}`);
        }
        figures.push(checkFigure(`feed line ${n} amount vs winners[${winnerAt - 1}].amount`, w.amount, line.amount, { currency: 'ICP' }));
        return;
      }
      if (!VERB_CLASSES.has(line.cls)) { problems.push(`feed line ${n} carries class "${line.cls}", not a colour-coded verb class`); return; }
      // an action line: the next record action whose kind and amount it states
      let matched = null;
      for (let k = nextAction; k < chain.actions.length; k += 1) {
        const a = chain.actions[k];
        const spec = FEED_OF_KIND[a.kind];
        if (!spec || spec.cls !== line.cls) continue;
        if (spec.amount && !checkFigure('probe', a.amount, line.amount, { currency: 'ICP' }).ok) continue;
        matched = { a, spec, k };
        break;
      }
      if (!matched) { problems.push(`feed line ${n} ("${line.seat} ${line.verb} ${line.amount ?? ''}") matches no action in the record after #${nextAction}`); return; }
      nextAction = matched.k + 1;
      if (!String(line.verb || '').includes(matched.spec.verb)) problems.push(`feed line ${n} reads "${line.verb}", expected "${matched.spec.verb}"`);
      const seatText = `Seat ${matched.a.seat + 1}`;
      if (!String(line.seat || '').startsWith(seatText) && !(line.seat === 'You' && matched.a.seat === HERO_SEAT)) {
        problems.push(`feed line ${n} names "${line.seat}", not ${seatText}`);
      }
      if (matched.spec.amount) {
        figures.push(checkFigure(`feed line ${n} amount vs the hand record`, matched.a.amount, line.amount, { currency: 'ICP' }));
      } else if (line.amount !== null) {
        problems.push(`feed line ${n} shows an amount "${line.amount}" for an action that moved no chips`);
      }
    });
    checks.actionLinesMatched = nextAction;
    checks.actionsOnChain = chain.actions.length;
    if (dom.lines.filter((l) => VERB_CLASSES.has(l.cls) && l.cls !== 'action-blind').length === 0) {
      problems.push('the drawer shows no action line at all');
    }
    if (winnerAt !== chain.winners.length) problems.push(`${winnerAt} "won" line(s) for ${chain.winners.length} winner(s) on chain`);

    // ---- 3. the table behind the drawer ------------------------------------
    const tableAgreement = named('table', await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true, requireWinnerBanner: true,
    }));

    const folded = foldFigures(figures);
    checks.moneyFiguresChecked = folded.checked;
    checks.moneyMismatches = folded.mismatches;
    checks.figures = folded.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail }));
    checks.problems = problems;
    const verified = problems.length === 0 && folded.ok;
    return withAgreement({
      verified,
      checks,
      notes: verified
        ? `deck seal sealed mid-hand then checked; chip fingerprint = the canister's seed_hash; `
          + `${dom.lines.length} log lines, ${folded.checked} amounts matched to the record`
        : `LOG NOT VERIFIED (${problems.length} problem(s)): ${problems.slice(0, 6).join(' | ')}`
          + (folded.ok ? '' : ` || money: ${folded.mismatches.slice(0, 3).join(' | ')}`),
    }, tableAgreement);
  },
};
