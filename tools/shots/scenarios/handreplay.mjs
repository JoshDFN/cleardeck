// Scene: the HAND REPLAYER, opened, walked and asserted.
//
// WHY THIS SCENE EXISTS
// ---------------------
// docs/DEFECTS.md H-31: `handhistory.mjs` asserts the LIST and stops. Nothing in
// this repo had ever clicked a hand open, so the replayer -- the only per-card
// verification surface in the corpus, and the surface where the ordering
// over-claim survived a whole wave undisturbed -- had never been photographed and
// nothing about it was asserted. Every number it renders was outside the token
// census too, because the census only sees what a scene puts on screen.
//
// WHAT IS ASSERTED, AND WHY IT IS THE VERDICT RATHER THAN THE ELEMENTS
// --------------------------------------------------------------------
// The lesson of wave 4 is already paid for: `make hygiene` was green while a
// portrait player saw none of the four protected notices, because the gate greps
// the source. So nothing below is satisfied by an element merely existing.
//
//   1. STREET NAVIGATION. Every stop is clicked and the board is READ at each
//      one: 0 -> 3 -> 4 -> 5 -> 5 cards, and every visible card's face is
//      compared with `get_hand_history(n).community_cards` rank-and-suit, and
//      every board card carries a re-derivation tick.
//   2. THE ACTION LOG, LINE BY LINE. Every line's street, actor seat, verb and
//      AMOUNT is compared with the canister's own `ActionRecord` for that action,
//      in order, plus the two blind lines against `get_table_view().config`.
//      A missing amount fails; a wrong amount fails; a wrong street fails.
//   3. THE LOG'S OWN ARITHMETIC. The replayer claims the blinds plus every amount
//      add up to the pot the table paid. The scene recomputes that sum from the
//      chain and requires the screen to agree with it AND the canister.
//   4. THE FAIRNESS CLAIMS. The headline verdict and every proof label must not
//      claim the ordering; the caveat must be present and must carry it; three
//      historical over-claim strings must be absent from the whole modal.
//   5. THE ORDERING, WITNESSED. The scene opens the panel DURING a live hand so
//      the browser records that hand's commitment while no seed exists, plays the
//      hand out, reopens the replayer and requires the panel to reach the
//      "you saw this commitment while the hand was still running" verdict. Then
//      it pastes the chain's own commitment into the compare box and requires a
//      match, and a corrupted one and requires a mismatch.
//   6. THE FOUR PROTECTED NOTICES, ON SCREEN. Not in the DOM: on screen, and with
//      nothing painted on top of them. This modal is a fixed overlay behind a 72%
//      scrim, so the page's own banner is unreadable underneath it; the four
//      notices are measured where a player would read them, by hit-testing their
//      own pixels.
//
// TABLE CHOICE. `table_1` (heads-up, 0.01/0.02 ICP) is used by no other scene, so
// this one cannot reset a table another scene is mid-way through.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { optional, unwrap } from '../lib/agent.mjs';
import {
  doAct, phaseOf, playUntil, sitOutAfterHand, startHand, tableActorFor, waitForState,
} from '../lib/table-driver.mjs';
import { assertChainAgreement, named, withAgreement } from '../lib/chain-agreement.mjs';
import { handRecordActor } from '../lib/hand-record-wire.mjs';
import {
  RANK_BY_GLYPH, SUIT_BY_SYMBOL, checkFigure, checkPlainNumber, foldFigures, parseDisplayedAmount,
} from '../lib/money.mjs';
import {
  MC_TOLERANCE_POINTS, ORACLE_TRIALS, exactEquity, monteCarloEquity, toCard as toOracleCard,
} from '../lib/equity-oracle.mjs';
import { foldProtectedNotices, probeProtectedNotices } from '../lib/protected-notices.mjs';
import { heroPrincipal, prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_1';
const HERO_SEAT = 0;
const OPP_PLAYER = 2;
const OPP_SEAT = 1;

/**
 * What `stage` observed, for `verify` to assert.
 *
 * run.mjs calls `scene.verify(ctx, page)` and does NOT hand it the value `stage`
 * returned, so a third parameter would silently always be `{}` — the shape of a
 * gate that passes because it checked nothing. `stage` and `verify` run back to
 * back for one (scene, viewport) pair, so a module-level handoff is exact; it is
 * cleared at the top of `stage` so a second viewport cannot inherit the first
 * one's observations.
 */
let observed = null;

/** The bet each post-flop street opens with, in e8s. Distinct on purpose: every
 *  amount in the log is a different number, so a token can only be matched by the
 *  check that really covers it. */
const OPENING_BET = { Flop: 10_000_000, Turn: 20_000_000, River: 40_000_000 };

/** Canister card -> the two-character face the <Card> component renders. */
const RANK_GLYPH = Object.fromEntries(Object.entries(RANK_BY_GLYPH).map(([g, n]) => [n, g]));
const SUIT_SYMBOL = Object.fromEntries(Object.entries(SUIT_BY_SYMBOL).map(([s, n]) => [n, s]));
const faceOf = (card) => {
  const rank = RANK_GLYPH[Object.keys(card.rank)[0]];
  const suit = SUIT_SYMBOL[Object.keys(card.suit)[0]];
  return `${rank}${suit}`;
};

/** `ActionRecord.phase` -> the street header the log must print. */
const STREET_OF_PHASE = {
  preflop: 'Pre-flop', flop: 'Flop', turn: 'Turn', river: 'River',
};

/** The verb each action must be described with (mirrors HandHistory's ACTION_WORD). */
const WORD_OF_KIND = {
  Fold: 'folds', Check: 'checks', Call: 'calls', Bet: 'bets',
  Raise: 'raises to', AllIn: 'is all in for',
};

/** Community cards on the felt by the end of each stop, in stop order. */
const BOARD_BY_STOP = { 'Pre-flop': 0, Flop: 3, Turn: 4, River: 5, Showdown: 5 };

/**
 * Strings that assert the ordering the record cannot establish. Each one really
 * shipped: the first two were the replayer's headline at `3253b67`, the third was
 * the label on its commitment row until this pass.
 */
const OVER_CLAIMS = [
  'were fixed before the hand was played',
  'before any card was dealt',
  'committed before the deal',
];

// ---------------------------------------------------------------------------
// staging the chain
// ---------------------------------------------------------------------------

/** Bets when first to act on a street, calls when facing one, never folds. */
const streetByStreet = async (t, v) => {
  const phase = phaseOf(v);
  if (phase !== 'PreFlop' && v.can_check && OPENING_BET[phase]) {
    return doAct.bet(OPENING_BET[phase])(t);
  }
  return doAct.checkOrCall(t, v);
};

/** Plays the hand that is currently live, street by street, to a real showdown. */
function playToShowdown(tableId, label) {
  return playUntil(
    tableId,
    {
      [HERO_SEAT]: { player: HERO_PLAYER, act: streetByStreet },
      [OPP_SEAT]: { player: OPP_PLAYER, act: streetByStreet },
    },
    (s) => phaseOf(s) === 'HandComplete' && s.last_hand_winners.length > 0,
    label,
    { timeoutMs: 150_000 },
  );
}

/** Sits the opponent back in and deals the next hand, leaving it LIVE. */
async function dealNextHand(tableId) {
  const opp = await tableActorFor(OPP_PLAYER, tableId);
  unwrap(await opp.sit_in(), 'sit_in');
  await startHand(OPP_PLAYER, tableId);
  // Nothing after this hand: with the opponent sitting out again the canister's
  // auto-deal cannot find two active players, so the hand we are about to replay
  // stays the newest one for as long as the capture needs.
  await sitOutAfterHand([OPP_PLAYER], tableId);
  return waitForState(
    HERO_PLAYER, tableId,
    (v) => !['HandComplete', 'WaitingForPlayers'].includes(phaseOf(v)),
    'the next hand to be live',
    { timeoutMs: 60_000 },
  );
}

/**
 * The whole hand record, flattened into the shape the assertions compare against.
 *
 * READ THROUGH THE HARNESS'S OWN CANDID MIRROR, NOT THE APP'S DECLARATION.
 * `lib/hand-record-wire.mjs` explains why at length: the app's generated
 * declaration is missing `ActionRecord.phase` and `ActionRecord.amount`, and when
 * this function read the chain through it, both sides of every street and amount
 * comparison were blind in the same way. The scene still went red, but for the
 * wrong reason -- it reported "not null" instead of naming the amount the canister
 * actually recorded. An assertion has to know the truth, not merely differ from
 * the screen.
 */
async function chainRecord(tableId, handNumber) {
  const table = await handRecordActor(HERO_PLAYER, tableId);
  const rec = optional(await table.get_hand_history(BigInt(handNumber)));
  if (!rec) throw new Error(`the table canister has no record of hand ${handNumber}`);
  const v = optional(await table.get_table_view());
  if (!v) throw new Error('the table canister returned no view');
  const actions = rec.actions.map((a) => {
    const kind = Object.keys(a.action)[0];
    return {
      kind,
      seat: Number(a.seat),
      amount: Number(a.amount),
      phase: String(a.phase),
      street: STREET_OF_PHASE[String(a.phase).toLowerCase()] ?? null,
    };
  });
  const winners = rec.winners.map((w) => ({ seat: Number(w.seat), amount: Number(w.amount) }));
  const blinds = {
    small: Number(v.config.small_blind),
    big: Number(v.config.big_blind),
    ante: Number(v.config.ante),
    sbSeat: Number(v.small_blind_seat),
    bbSeat: Number(v.big_blind_seat),
  };
  const pot = winners.reduce((n, w) => n + w.amount, 0);
  return {
    handNumber: Number(rec.hand_number),
    seedHash: String(rec.shuffle_proof.seed_hash),
    seedRevealed: optional(rec.shuffle_proof.revealed_seed),
    community: rec.community_cards.map(faceOf),
    actions,
    winners,
    showdown: rec.showdown_players.map((p) => ({
      seat: Number(p.seat),
      principal: p.principal.toText(),
      cards: (optional(p.cards) || []).map(faceOf),
      // the Candid cards too, for the independent equity oracle
      raw: (optional(p.cards) || []).map(toOracleCard),
      won: Number(p.amount_won),
    })),
    communityRaw: rec.community_cards.map(toOracleCard),
    blinds,
    pot,
    // The replayer's own arithmetic, recomputed here from the chain: blinds plus
    // every recorded amount. Only meaningful while no action is a Raise or an
    // AllIn, whose amount is a street total rather than an increment.
    sumIsIncremental: actions.every((a) => a.kind !== 'Raise' && a.kind !== 'AllIn')
      && Number(v.config.ante) === 0,
    logSum: blinds.small + blinds.big + actions.reduce((n, a) => n + a.amount, 0),
    liveHandNumber: Number(v.hand_number),
    phase: Object.keys(v.phase)[0],
  };
}

// ---------------------------------------------------------------------------
// reading the rendered page
// ---------------------------------------------------------------------------

/**
 * Every card face inside a container, in DOM order, read the way the table
 * scraper reads a card (dom-scrape.mjs readCard): the `.rank` and the `.pip`
 * of the top-left index. A card's whole textContent is no longer its face:
 * the phase-1 face carries a mirrored index and a centre pip as well.
 */
async function cardFaces(locator) {
  return locator.locator('.card').evaluateAll((els) => els.map((el) => {
    const t = (sel) => (el.querySelector(sel)?.textContent || '').replace(/\s+/g, '');
    return `${t('.rank')}${t('.pip')}`;
  }));
}

/**
 * The replayer, as text. Nothing here is inferred: each field is the rendered
 * content of one element, so a check that passes had to have real content behind
 * it.
 */
function scrapeReplayer(page) {
  return page.evaluate(() => {
    const t = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
    const attr = (sel, name) => document.querySelector(sel)?.getAttribute(name) ?? null;

    const log = document.querySelector('.log-panel .log');
    const lines = [];
    let street = null;
    for (const child of log ? [...log.children] : []) {
      if (child.classList.contains('log-street')) {
        street = t(child.querySelector('.street-name'));
        continue;
      }
      if (!child.classList.contains('log-line')) continue;
      lines.push({
        street,
        index: t(child.querySelector('.log-index')),
        time: t(child.querySelector('.log-time')),
        seat: t(child.querySelector('.log-seat')),
        what: t(child.querySelector('.log-what')),
        amount: t(child.querySelector('.log-amount')),
        kind: [...child.classList].find((c) => c.startsWith('kind-')) || null,
        reconstructed: child.classList.contains('reconstructed'),
      });
    }

    return {
      replayers: document.querySelectorAll('.replayer').length,
      listRows: document.querySelectorAll('.hand-row').length,
      title: t(document.querySelector('.replay-title .hand-number')),
      stops: [...document.querySelectorAll('.scrubber .stop')].map(t),
      transport: t(document.querySelector('.transport-label')),
      lines,
      audit: t(document.querySelector('.log-panel .audit')),
      auditState: attr('.log-panel [data-audit]', 'data-audit'),
      auditMoney: [...document.querySelectorAll('.log-panel .audit .replay-money')].map(t),
      provenance: t(document.querySelector('.log-panel .provenance')),
      potTotal: t(document.querySelector('.pot-panel .pot-total strong')),
      winners: [...document.querySelectorAll('.pot-panel .winner-line')].map((el) => ({
        seat: t(el.querySelector('.seat-label')),
        amount: t(el.querySelector('.won-amt')),
      })),
      // The seat pods on the mini table. A seat that folded is on the table but
      // not a showdown row, so it is not compared with `showdown_players`.
      players: [...document.querySelectorAll('.player-row:not(.folded)')].map((el) => ({
        seat: t(el.querySelector('.seat-tag')),
        cards: [...el.querySelectorAll('.player-cards .card')].map((c) => {
          const f = (sel) => (c.querySelector(sel)?.textContent || '').replace(/\s+/g, '');
          return `${f('.rank')}${f('.pip')}`;
        }),
        deckPos: t(el.querySelector('.deck-pos')),
        rank: t(el.querySelector('.player-rank')),
        won: t(el.querySelector('.player-result .won-amt')),
        equity: t(el.querySelector('.replay-equity')),
        equitySeat: el.querySelector('.replay-equity')?.getAttribute('data-seat') ?? null,
        equityMethod: el.querySelector('.replay-equity')?.getAttribute('data-method') ?? null,
      })),
      tablePot: t(document.querySelector('.hand-history-modal .pot-panel .pot-total strong')),
      tableBets: [...document.querySelectorAll('.hand-history-modal .replay-bet .bet-figure')].map(t),
      equityMethodText: t(document.querySelector('.replay-equity-method')),
      // the fairness claims
      // Svelte appends its own scoped class, so the tone has to be read as a
      // class MEMBERSHIP rather than sliced out of `className`.
      bannerTone: (() => {
        const el = document.querySelector('.proof-banner');
        if (!el) return null;
        return ['good', 'partial', 'bad'].find((c) => el.classList.contains(c)) ?? 'none';
      })(),
      bannerHeadline: t(document.querySelector('.proof-banner strong')),
      caveat: t(document.querySelector('.banner-caveat')),
      proofLabels: [...document.querySelectorAll('.proof-panel .proof-label')].map(t),
      shownSeedHash: t(document.querySelector('.proof-panel .proof-line code')),
      witnessState: attr('[data-witness]', 'data-witness'),
      witnessText: t(document.querySelector('.witness-panel')),
      pasteState: attr('[data-paste]', 'data-paste'),
      pasteText: t(document.querySelector('.paste-verdict')),
      modalText: t(document.querySelector('.hand-history-modal')),
    };
  });
}

/**
 * The figures on the mini table, summed, against one chain number. Each
 * displayed literal stands for a half-ulp window and the windows add.
 */
function sumOnTable(label, chainValue, texts) {
  const parsed = texts.map((t) => parseDisplayedAmount(t, { currency: 'ICP' }));
  const chain = Number(chainValue);
  if (parsed.length === 0 || parsed.some((p) => !p)) {
    return {
      label, chain, domText: texts.join(' + '), agrees: false, discriminates2x: false, ok: false,
      detail: `could not read a number out of ${JSON.stringify(texts)}`,
    };
  }
  const low = parsed.reduce((n, p) => n + p.low, 0);
  const high = parsed.reduce((n, p) => n + p.high, 0);
  const agrees = chain >= low - 1e-6 && chain <= high + 1e-6;
  const resolution = high - low;
  const discriminates2x = chain === 0 ? true : Math.abs(chain) > resolution / 2;
  const ok = agrees && discriminates2x;
  return {
    label, chain, domText: parsed.map((p) => p.text).join(' + '), agrees, discriminates2x, ok,
    detail: agrees
      ? `on the table ${parsed.map((p) => p.text).join(' + ')} == chain ${chain} e8s`
      : `DISAGREES: on the table ${parsed.map((p) => p.text).join(' + ')} (${low}..${high} e8s) but the chain sums to ${chain} e8s`,
  };
}

/** Clicks every street stop and reads the board at each one. */
async function walkStreets(page) {
  const stops = page.locator('.scrubber .stop');
  const count = await stops.count();
  const walk = [];
  for (let i = 0; i < count; i += 1) {
    await stops.nth(i).click();
    await settle(page, { extraFrames: 1 });
    const table = await page.evaluate(() => {
      const t = (el) => (el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : null);
      const root = document.querySelector('.hand-history-modal');
      return {
        pot: t(root?.querySelector('.pot-panel .pot-total strong')),
        bets: [...(root?.querySelectorAll('.replay-bet .bet-figure') || [])].map(t),
        equity: [...(root?.querySelectorAll('.player-row:not(.folded) .replay-equity') || [])].map((el) => ({
          seat: Number(el.getAttribute('data-seat')),
          method: el.getAttribute('data-method'),
          text: t(el),
        })),
      };
    });
    walk.push({
      label: (await stops.nth(i).textContent())?.trim() ?? null,
      faces: await cardFaces(page.locator('.hand-history-modal .felt .board')),
      ticks: await page.locator('.hand-history-modal .felt .deck-pos.good').count(),
      crosses: await page.locator('.hand-history-modal .felt .deck-pos.bad').count(),
      transport: (await page.locator('.transport-label').textContent())?.trim() ?? null,
      table,
    });
  }
  return walk;
}

/**
 * Waits until exactly `n` community cards are FACE UP on the felt.
 *
 * `.community-cards` always holds five <Card> components; the undealt ones carry
 * `.empty`. Counting `.card` therefore counts slots, not cards.
 */
function dealtBoard(page, n) {
  return page.waitForFunction(
    (want) => document.querySelectorAll('.community-cards .card:not(.empty):not(.face-down)').length === want,
    n,
    { timeout: 90_000 },
  );
}

/** Opens the history modal, retrying the mount once (see handhistory.mjs). */
async function openHistory(page) {
  await page.waitForSelector('.history-btn', { timeout: 30_000 });
  await page.click('.history-btn');
  await page.waitForSelector('.hand-history-modal', { timeout: 30_000 });
  try {
    await page.waitForSelector('.hand-row, .live-note', { timeout: 8_000 });
  } catch {
    await page.locator('.hand-history-modal .close-btn').first().click().catch(() => {});
    await page.click('.history-btn');
    await page.waitForSelector('.hand-history-modal', { timeout: 15_000 });
    await page.waitForSelector('.hand-row, .live-note', { timeout: 20_000 });
  }
}

async function closeHistory(page) {
  await page.locator('.hand-history-modal .close-btn').first().click();
  await page.waitForSelector('.hand-history-modal', { state: 'detached', timeout: 15_000 });
}

// ---------------------------------------------------------------------------
// the scene
// ---------------------------------------------------------------------------

export default {
  name: 'handreplay',
  title: 'One hand replayed street by street, every amount and claim asserted',
  auth: true,

  async setup(ctx) {
    // Hand 1 exists only so the list is not a single row and the replayer has to
    // open the RIGHT hand. The hand this scene asserts is hand 2, dealt in
    // `stage` so the browser can witness its commitment while it is still live.
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: HERO_SEAT, chips: icp(6) },
      { player: OPP_PLAYER, seat: OPP_SEAT, chips: icp(8) },
    ]);
    await startHand(OPP_PLAYER, tableId);
    await sitOutAfterHand([OPP_PLAYER], tableId);
    const first = await playToShowdown(tableId, 'hand 1, street by street, to a showdown');
    return {
      table: TABLE,
      tableId,
      onChain: {
        phase: phaseOf(first),
        handNumber: Number(first.hand_number),
        heroPrincipal: heroPrincipal(),
      },
      notes: `hand #${first.hand_number} finished on ${TABLE}; hand #${Number(first.hand_number) + 1} `
        + 'is dealt while the browser is watching so the ordering can be witnessed',
    };
  },

  async stage(ctx, page) {
    observed = null;
    const tableId = ctx.tableIds[TABLE];
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // WAIT ON THE BOARD THE PLAYER CAN SEE, NOT ON FIVE <Card> SLOTS.
    // `.community-cards` always renders `Array(5)` of <Card> (PokerTable.svelte),
    // and an undealt slot is still a `.card` — it just carries `.empty`. So
    // `document.querySelectorAll('.community-cards .card').length >= 5` is true
    // the instant the board frame mounts, dealt cards or not: a wait that cannot
    // fail is not a wait. `:not(.empty)` counts cards that are actually FACE UP.
    await dealtBoard(page, 5);

    // --- 1. the panel BEFORE the next hand: no sighting to be had -----------
    await openHistory(page);
    const beforeAnyHand = await page.evaluate(
      () => document.querySelector('[data-witness]')?.getAttribute('data-witness') ?? null,
    );
    await closeHistory(page);

    // --- 2. deal the hand this scene replays, and read its commitment LIVE ---
    const live = await dealNextHand(tableId);
    const handNumber = Number(live.hand_number);
    // An empty board is how the APP says it has seen the new hand: the previous
    // hand's five cards are gone and nothing is dealt yet. Without this the modal
    // can mount while `handNumber` is still the finished hand, in which case there
    // is no live hand as far as the panel is concerned and no sighting to take.
    await dealtBoard(page, 0);

    // Opening the panel now is the whole point: the table has published this
    // hand's commitment and no seed, so the browser can note it.
    await openHistory(page);
    await page.waitForSelector('.live-note', { timeout: 30_000 });
    const liveNote = (await page.locator('.live-note').first().textContent())
      ?.replace(/\s+/g, ' ').trim() ?? null;
    const witnessed = await page.evaluate(() => {
      try {
        return JSON.parse(localStorage.getItem('cleardeck_commitment_witness') || '{}');
      } catch {
        return null;
      }
    });

    // THE REPLAYER, OPENED ON A HAND THAT IS STILL BEING PLAYED. The table writes
    // a hand's record when the hand STARTS, so this record has actions and no
    // winners. The log's sum-vs-pot audit must REFUSE here rather than announce
    // that the amounts do not match a pot of zero — a false accusation about a
    // hand that is simply not finished.
    //
    // The list's own filter has to be switched first: "My hands" keeps hands whose
    // record names the hero as a showdown player or a winner, and a hand still in
    // play has neither, so the live hand is not in the default view. (That is the
    // filter working: it hides nothing silently, and the count on the other chip
    // says how many.)
    let liveAudit = null;
    await page.locator('.list-head .chip', { hasText: 'Every hand here' }).first().click();
    const liveRow = page.locator('.hand-row', {
      has: page.locator('.hand-number', { hasText: new RegExp(`^Hand #${handNumber}$`) }),
    });
    await liveRow.first().waitFor({ timeout: 15_000 }).catch(() => {});
    if (await liveRow.count()) {
      await liveRow.first().click();
      await page.waitForSelector('.replayer', { timeout: 20_000 });
      liveAudit = await page.evaluate(() => ({
        audit: document.querySelector('.log-panel [data-audit]')?.getAttribute('data-audit') ?? null,
        text: (document.querySelector('.log-panel [data-audit]')?.textContent || '')
          .replace(/\s+/g, ' ').trim().slice(0, 200),
        banner: (document.querySelector('.proof-banner strong')?.textContent || '').trim(),
      }));
    }
    await closeHistory(page);

    // --- 3. play it out, then replay it ------------------------------------
    await playToShowdown(tableId, `hand ${handNumber}, street by street, to a showdown`);
    await dealtBoard(page, 5);

    await openHistory(page);
    const row = page.locator('.hand-row', {
      has: page.locator('.hand-number', { hasText: new RegExp(`^Hand #${handNumber}$`) }),
    });
    await row.first().waitFor({ timeout: 30_000 });
    await row.first().click();
    await page.waitForSelector('.replayer', { timeout: 30_000 });
    await page.waitForSelector('.log-panel .log-line', { timeout: 30_000 });
    await settle(page);

    observed = { handNumber, beforeAnyHand, liveNote, witnessed, liveAudit };
    return observed;
  },

  // One extra frame per viewport, and only one.
  motionFrames: 1,
  motionEveryMs: 1,

  /**
   * A SECOND FRAME, scrolled to the log, because one frame cannot hold the record.
   *
   * Measured on this build: the replayer's content is 1119 px on desktop and
   * 2480 px on a phone, inside a scroller 689 px / 618 px tall. The canonical
   * capture is deliberately the top of the document -- the verdict, its caveat and
   * the ordering panel -- so this writes the other half: the street navigation, the
   * board and the action log with its sum-vs-pot audit. `verify` records exactly
   * which elements fall outside the dialog in `checks.fold`, so neither frame is
   * standing in for something a reader cannot check.
   */
  async advance(ctx, page, { burst }) {
    const stops = page.locator('.scrubber .stop');
    const last = (await stops.count()) - 1;
    if (last >= 0) await stops.nth(last).click();
    await page.evaluate(() => {
      const r = document.querySelector('.replayer');
      const s = document.querySelector('.scrubber');
      if (!r || !s) return;
      r.scrollTop += s.getBoundingClientRect().top - r.getBoundingClientRect().top - 6;
    });
    await settle(page, { extraFrames: 1 });
    const frames = await burst();
    // Put the scroller back so the canonical PNG is the same frame every run.
    await page.evaluate(() => {
      const r = document.querySelector('.replayer');
      if (r) r.scrollTop = 0;
    });
    await settle(page, { extraFrames: 1 });
    return {
      frames,
      onChain: {
        note: 'extra frame: the replayer scrolled to the street navigation, the board and '
          + 'the action log. The canonical PNG is the top of the same document.',
      },
    };
  },

  async verify(ctx, page) {
    const staged = observed || {};
    const tableId = ctx.tableIds[TABLE];
    if (!staged.handNumber) {
      throw new Error('stage() left no observations for verify(): the mid-hand commitment '
        + 'sighting was never taken, so nothing about the ordering could be asserted');
    }
    const handNumber = staged.handNumber;
    const chain = await chainRecord(tableId, handNumber);
    const checks = {};
    const problems = [];
    const figures = [];

    // ---- 1. the replayer is open on the right hand ------------------------
    let dom = await scrapeReplayer(page);
    checks.replayerOpen = dom.replayers;
    checks.listReplaced = dom.listRows;
    checks.title = dom.title;
    checks.handNumberOnChain = chain.handNumber;
    if (dom.replayers !== 1) problems.push('no .replayer on screen');
    if (dom.listRows !== 0) problems.push('the hand list is still rendered behind the replayer');
    if (dom.title !== `Hand #${chain.handNumber}`) {
      problems.push(`replayer title "${dom.title}" is not the hand read from the chain (#${chain.handNumber})`);
    }
    if (chain.seedRevealed === null) problems.push('the chain has no revealed seed for this hand');

    // ---- 2. the streets, and the board growing ----------------------------
    const walk = await walkStreets(page);
    checks.streetWalk = walk;
    const expectedStops = ['Pre-flop', 'Flop', 'Turn', 'River', 'Showdown'];
    checks.stops = walk.map((w) => w.label);
    if (String(checks.stops) !== String(expectedStops)) {
      problems.push(`street stops are ${JSON.stringify(checks.stops)}, expected ${JSON.stringify(expectedStops)}`);
    }
    for (const step of walk) {
      const want = BOARD_BY_STOP[step.label];
      if (want === undefined) continue;
      if (step.faces.length !== want) {
        problems.push(`at "${step.label}" the board shows ${step.faces.length} card(s), expected ${want}`);
      }
      const expectFaces = chain.community.slice(0, want);
      if (String(step.faces) !== String(expectFaces)) {
        problems.push(`at "${step.label}" the board reads ${JSON.stringify(step.faces)} but the canister's `
          + `community cards are ${JSON.stringify(expectFaces)}`);
      }
      if (step.crosses !== 0) problems.push(`at "${step.label}" ${step.crosses} board card(s) failed re-derivation`);
      if (step.ticks !== want) {
        problems.push(`at "${step.label}" ${step.ticks} of ${want} board card(s) carry a re-derivation tick`);
      }
      if (step.transport !== step.label) {
        problems.push(`at "${step.label}" the transport reads "${step.transport}"`);
      }
    }

    // ---- 2b. THE MINI TABLE AT EVERY STREET STOP -----------------------
    // The pot the table module shows plus every bet chip in front of a seat
    // must equal the blinds and every recorded amount up to that street: at a
    // street's reveal the previous streets are in the pot and nothing is in
    // front of anyone; at the showdown the pot is what the table paid. Only
    // meaningful while every amount is an increment (sumIsIncremental), which
    // the staged hand guarantees. And the equity on each pod is recomputed by
    // the independent oracle from the record's own cards and the board at that
    // stop: exact where the client says exact, Monte Carlo inside the stated
    // tolerance where it says Monte Carlo.
    checks.tableWalk = [];
    const streetsBefore = { 'Pre-flop': [], Flop: ['Pre-flop'], Turn: ['Pre-flop', 'Flop'], River: ['Pre-flop', 'Flop', 'Turn'] };
    for (const step of walk) {
      const row = { label: step.label, pot: step.table.pot, bets: step.table.bets, equity: step.table.equity };
      if (chain.sumIsIncremental) {
        const before = streetsBefore[step.label];
        const expectedTotal = step.label === 'Showdown'
          ? chain.pot
          : chain.blinds.small + chain.blinds.big
            + chain.actions.filter((a) => before && before.includes(a.street)).reduce((n, a) => n + a.amount, 0);
        const onTable = [step.table.pot, ...step.table.bets].filter((x) => x !== null);
        // One figure per stop: the sum of what the table shows, compared with
        // the chain's own sum (the windows add, as in chain-agreement's
        // checkSum). The showdown's figure covers the resting frame's pot
        // token; the others are asserted on the walk.
        const f = sumOnTable(`replay table at "${step.label}": pot plus chips in front vs blinds and amounts so far`,
          expectedTotal, onTable);
        figures.push(f);
        row.potOk = f.ok;
        row.potDetail = f.detail;
        if (step.label === 'Showdown' && step.table.bets.length) {
          problems.push(`at "Showdown" ${step.table.bets.length} bet chip(s) are still in front of the seats`);
        }
      }
      // equity, when the client showed one
      const want = BOARD_BY_STOP[step.label] ?? 0;
      const board = chain.communityRaw.slice(0, want);
      const hands = chain.showdown.map((p) => p.raw);
      if (step.table.equity.length && hands.length >= 2 && hands.every((h) => h.length === 2 && h.every(Boolean))) {
        const methods = new Set(step.table.equity.map((e) => e.method));
        if (methods.size !== 1) problems.push(`at "${step.label}" the pods disagree on the equity method`);
        const method = [...methods][0];
        const oracle = method === 'exact'
          ? exactEquity(hands, board)
          : monteCarloEquity(hands, board, ORACLE_TRIALS);
        row.oracle = { method: oracle.method, trials: oracle.trials, share: oracle.share.map((v) => Math.round(v * 10000) / 100) };
        for (const e of step.table.equity) {
          const at = chain.showdown.findIndex((p) => p.seat === e.seat);
          if (at < 0) { problems.push(`at "${step.label}" seat ${e.seat + 1} shows an equity but showed no cards`); continue; }
          const oraclePct = Math.round(oracle.share[at] * 10000) / 100;
          if (method === 'exact') {
            figures.push(checkPlainNumber(
              `replay seat ${e.seat} equity at ${step.label} vs independent exact enumeration (${oracle.trials} runouts)`,
              oraclePct, e.text, { unit: '%' },
            ));
          } else {
            const screenPct = Number(String(e.text).replace(/[^\d.]/g, ''));
            const delta = Math.abs(screenPct - oraclePct);
            const ok = Number.isFinite(screenPct) && delta <= MC_TOLERANCE_POINTS;
            figures.push({
              label: `replay seat ${e.seat} equity at ${step.label} vs independent Monte Carlo (${oracle.trials} trials)`,
              chain: oraclePct, domText: e.text, agrees: ok, discriminates2x: true, ok,
              detail: ok
                ? `screen ${screenPct}% vs independent ${oraclePct.toFixed(2)}% (|d| ${delta.toFixed(3)} <= ${MC_TOLERANCE_POINTS.toFixed(3)} points)`
                : `DISAGREES: screen ${screenPct}% vs independent ${oraclePct.toFixed(2)}% (tolerance ${MC_TOLERANCE_POINTS.toFixed(3)} points)`,
            });
          }
        }
      } else if (step.table.equity.length) {
        problems.push(`at "${step.label}" an equity is shown but the record reveals ${hands.length} hand(s)`);
      }
      checks.tableWalk.push(row);
    }
    const showdownStep = walk.find((w) => w.label === 'Showdown');
    if (showdownStep && chain.showdown.length >= 2 && showdownStep.table.equity.length !== chain.showdown.length) {
      problems.push(`at "Showdown" ${showdownStep.table.equity.length} pod(s) carry an equity for ${chain.showdown.length} revealed hand(s)`);
    }

    // The walk ends on Showdown, which is the frame this scene publishes.
    dom = await scrapeReplayer(page);

    // ---- 3. the action log, line by line ----------------------------------
    // Two blind lines first, then one line per ActionRecord, in order.
    const expectLines = [
      {
        street: 'Blinds', seat: chain.blinds.sbSeat, word: 'posts the small blind',
        amount: chain.blinds.small, kind: 'kind-postblind', label: 'small blind',
      },
      {
        street: 'Blinds', seat: chain.blinds.bbSeat, word: 'posts the big blind',
        amount: chain.blinds.big, kind: 'kind-postblind', label: 'big blind',
      },
      ...chain.actions.map((a, at) => ({
        street: a.street, seat: a.seat, word: WORD_OF_KIND[a.kind] ?? null,
        amount: a.amount > 0 ? a.amount : null, kind: `kind-${a.kind.toLowerCase()}`,
        label: `action ${at + 1} (${a.kind})`,
      })),
    ];
    checks.logLinesOnScreen = dom.lines.length;
    checks.logLinesExpected = expectLines.length;
    if (dom.lines.length !== expectLines.length) {
      problems.push(`the log renders ${dom.lines.length} line(s); the chain record plus the two `
        + `blind posts is ${expectLines.length}`);
    }
    checks.log = [];
    for (let i = 0; i < Math.min(dom.lines.length, expectLines.length); i += 1) {
      const got = dom.lines[i];
      const want = expectLines[i];
      const seatText = `Seat ${want.seat + 1}`;
      const row = {
        label: want.label,
        street: { screen: got.street, chain: want.street, ok: got.street === want.street },
        seat: { screen: got.seat, want: seatText, ok: String(got.seat || '').startsWith(seatText) },
        word: { screen: got.what, want: want.word, ok: want.word ? String(got.what || '').includes(want.word) : null },
        kind: { screen: got.kind, want: want.kind, ok: got.kind === want.kind },
        amountScreen: got.amount,
        amountChain: want.amount,
      };
      if (!row.street.ok) problems.push(`log line ${i + 1} (${want.label}) is under street "${got.street}", not "${want.street}"`);
      if (!row.seat.ok) problems.push(`log line ${i + 1} (${want.label}) names "${got.seat}", not "${seatText}"`);
      if (row.word.ok === false) problems.push(`log line ${i + 1} (${want.label}) reads "${got.what}", which does not contain "${want.word}"`);
      if (!row.kind.ok) problems.push(`log line ${i + 1} (${want.label}) carries class "${got.kind}", not "${want.kind}"`);
      if (want.amount === null) {
        // A Fold or a Check moves no chips, so an amount here would be invented.
        if (got.amount !== null) problems.push(`log line ${i + 1} (${want.label}) shows an amount "${got.amount}" for an action that moved no chips`);
        row.amountOk = got.amount === null;
      } else if (got.amount === null) {
        problems.push(`log line ${i + 1} (${want.label}) shows NO amount, but the canister recorded ${want.amount} e8s`);
        row.amountOk = false;
      } else {
        const f = checkFigure(`replay ${want.label} amount vs the hand record`, want.amount, got.amount, {
          currency: 'ICP',
        });
        figures.push(f);
        row.amountOk = f.ok;
        row.amountDetail = f.detail;
      }
      checks.log.push(row);
    }

    // ---- 4. the log's own arithmetic --------------------------------------
    checks.auditState = dom.auditState;
    checks.auditText = dom.audit;
    checks.chainLogSum = chain.logSum;
    checks.chainPot = chain.pot;
    checks.sumIsIncremental = chain.sumIsIncremental;
    if (!chain.sumIsIncremental) {
      // The scene stages a hand with no raise and no all-in on purpose, so this
      // is a staging failure, not a client one -- said out loud either way.
      problems.push('the staged hand contains a raise or an all-in, so the log cannot be summed '
        + 'and the audit claim was not exercised');
    } else if (chain.logSum !== chain.pot) {
      problems.push(`the CHAIN disagrees with itself: blinds + recorded amounts = ${chain.logSum} e8s `
        + `but the winners were paid ${chain.pot} e8s`);
    } else if (dom.auditState !== 'balanced') {
      problems.push(`the log's audit line reads "${dom.auditState}" for a hand whose amounts do add up`);
    }
    if (dom.auditMoney.length !== 1) {
      problems.push(`the balanced audit line should quote exactly one amount; it quotes ${dom.auditMoney.length}`);
    } else {
      figures.push(checkFigure('replay log sum vs blinds + every recorded amount',
        chain.logSum, dom.auditMoney[0], { currency: 'ICP' }));
    }

    // ---- 5. the pot, the winners and the showdown ------------------------
    figures.push(checkFigure('replay final pot vs the sum the table paid out',
      chain.pot, dom.potTotal, { currency: 'ICP' }));
    checks.winners = dom.winners;
    if (dom.winners.length !== chain.winners.length) {
      problems.push(`${dom.winners.length} winner line(s) for ${chain.winners.length} winner(s) on chain`);
    }
    for (let i = 0; i < Math.min(dom.winners.length, chain.winners.length); i += 1) {
      const w = chain.winners[i];
      if (!String(dom.winners[i].seat || '').startsWith(`Seat ${w.seat + 1}`)) {
        problems.push(`winner line ${i + 1} names "${dom.winners[i].seat}", not Seat ${w.seat + 1}`);
      }
      figures.push(checkFigure(`replay winner ${i + 1} award vs winners[${i}].amount`,
        w.amount, dom.winners[i].amount, { currency: 'ICP' }));
    }

    checks.players = dom.players;
    if (dom.players.length !== chain.showdown.length) {
      problems.push(`${dom.players.length} showdown row(s) for ${chain.showdown.length} on chain`);
    }
    for (let i = 0; i < Math.min(dom.players.length, chain.showdown.length); i += 1) {
      const p = chain.showdown[i];
      const shown = dom.players[i];
      if (!String(shown.seat || '').startsWith(`Seat ${p.seat + 1}`)) {
        problems.push(`showdown row ${i + 1} names "${shown.seat}", not Seat ${p.seat + 1}`);
      }
      if (String(shown.cards) !== String(p.cards)) {
        problems.push(`showdown row ${i + 1} shows ${JSON.stringify(shown.cards)} but the record has ${JSON.stringify(p.cards)}`);
      }
      if (!/✓/.test(shown.deckPos || '')) {
        problems.push(`showdown row ${i + 1} carries no re-derivation tick on its hole cards ("${shown.deckPos}")`);
      }
      if (p.won > 0) {
        figures.push(checkFigure(`replay showdown seat ${p.seat + 1} result vs amount_won`,
          p.won, shown.won, { currency: 'ICP' }));
      } else if (shown.won !== null) {
        problems.push(`showdown row ${i + 1} shows a win of "${shown.won}" for a seat the record paid 0`);
      }
    }

    // ---- 6. the blind level, against the table's own config --------------
    const blindLines = dom.lines.filter((l) => l.street === 'Blinds');
    checks.blindLines = blindLines;
    if (blindLines.length !== 2) {
      problems.push(`${blindLines.length} blind line(s) in the log, expected 2`);
    } else {
      figures.push(checkFigure('replay small blind vs get_table_view().config.small_blind',
        chain.blinds.small, blindLines[0].amount, { currency: 'ICP' }));
      figures.push(checkFigure('replay big blind vs get_table_view().config.big_blind',
        chain.blinds.big, blindLines[1].amount, { currency: 'ICP' }));
    }

    // ---- 7. the fairness claims ------------------------------------------
    checks.bannerTone = dom.bannerTone;
    checks.bannerHeadline = dom.bannerHeadline;
    checks.caveat = dom.caveat;
    checks.proofLabels = dom.proofLabels;
    if (dom.bannerTone !== 'good') {
      problems.push(`the verdict banner is "${dom.bannerTone}", not the green verdict a fully re-derived hand earns`);
    }
    if (/before/i.test(dom.bannerHeadline || '')) {
      problems.push(`the headline verdict claims an ordering: "${dom.bannerHeadline}"`);
    }
    for (const label of dom.proofLabels) {
      if (/before/i.test(label || '')) problems.push(`a proof label claims an ordering: "${label}"`);
    }
    checks.notProvenPresent = /not proven/i.test(dom.modalText || '');
    if (!checks.notProvenPresent) problems.push('the modal never says "Not proven"');
    if (!/not proven/i.test(dom.caveat || '') || !/before/i.test(dom.caveat || '')) {
      problems.push(`the green banner carries no "Not proven … before" caveat (caveat: "${dom.caveat}")`);
    }
    checks.overClaimsFound = OVER_CLAIMS.filter((s) => (dom.modalText || '').includes(s));
    for (const s of checks.overClaimsFound) problems.push(`the modal contains the retired over-claim "${s}"`);
    if (dom.shownSeedHash !== chain.seedHash) {
      problems.push(`the commitment on screen is not the canister's for this hand ("${dom.shownSeedHash}")`);
    }

    // ---- 8. the ordering, witnessed --------------------------------------
    checks.witnessBeforeAnyHand = staged.beforeAnyHand ?? null;
    checks.witnessState = dom.witnessState;
    checks.liveNote = staged.liveNote ?? null;
    checks.witnessStore = staged.witnessed ?? null;
    if (staged.beforeAnyHand && staged.beforeAnyHand !== 'none') {
      problems.push(`the panel claimed a sighting ("${staged.beforeAnyHand}") for a hand it had never `
        + 'seen live, which would be a witness of nothing');
    }
    if (dom.witnessState !== 'matched') {
      problems.push(`the witness panel reads "${dom.witnessState}", not "matched", although this browser `
        + 'read the commitment while the hand was still running');
    }
    if (!/still running/i.test(dom.witnessText || '')) {
      problems.push('the witness panel does not state that the commitment was read mid-hand');
    }
    // The audit refused to sum an unsettled hand rather than accusing it.
    checks.liveAudit = staged.liveAudit ?? null;
    if (!staged.liveAudit) {
      problems.push('the replayer was never opened on the hand while it was still being played, '
        + 'so the audit line\'s behaviour on an unsettled record was not exercised');
    } else if (staged.liveAudit.audit !== 'not-checkable') {
      problems.push(`opened on a hand still in play, the log's audit line read `
        + `"${staged.liveAudit.audit}" instead of refusing: "${staged.liveAudit.text}"`);
    }

    const storedForHand = staged.witnessed?.entries?.[`${tableId}:${handNumber}`] ?? null;
    checks.storedSighting = storedForHand;
    if (!storedForHand) {
      problems.push(`nothing was written to the sighting store for ${tableId}:${handNumber}`);
    } else if (storedForHand.seedHash !== chain.seedHash) {
      problems.push('the sighting this browser stored is not the commitment the record carries');
    }

    // paste-to-compare: the chain's own commitment, then a corrupted one, then
    // the real one again so the published frame shows the honest verdict.
    const input = page.locator('.witness-input');
    const pasteVerdict = async (value) => {
      await input.fill(value);
      await settle(page, { extraFrames: 1 });
      return page.evaluate(() => ({
        state: document.querySelector('[data-paste]')?.getAttribute('data-paste') ?? null,
        text: (document.querySelector('.paste-verdict')?.textContent || '').replace(/\s+/g, ' ').trim(),
      }));
    };
    const corrupted = chain.seedHash.replace(/^./, (c) => (c === 'a' ? 'b' : 'a'));
    checks.pasteMatching = await pasteVerdict(chain.seedHash);
    checks.pasteCorrupted = await pasteVerdict(corrupted);
    checks.pasteRestored = await pasteVerdict(chain.seedHash);
    if (checks.pasteMatching.state !== 'good') {
      problems.push(`pasting the canister's own commitment gives "${checks.pasteMatching.state}", not a match`);
    }
    if (checks.pasteCorrupted.state !== 'bad') {
      problems.push(`pasting a commitment with one character changed gives "${checks.pasteCorrupted.state}", `
        + 'so the box cannot tell a wrong commitment from a right one');
    }
    if (checks.pasteRestored.state !== 'good') {
      problems.push('the compare box did not recover after a mismatch');
    }

    // ---- 9. the four protected notices, ON SCREEN ------------------------
    const notices = foldProtectedNotices(
      await probeProtectedNotices(page), 'with the hand replayer open',
    );
    checks.protectedNotices = notices.notices;
    checks.protectedOnScreen = notices.onScreen;
    checks.protectedTotal = notices.total;
    problems.push(...notices.problems);

    // ---- 10. where the fold falls, measured rather than assumed ----------
    // The published frame is DELIBERATE: the replayer is scrolled back to the top
    // so every run photographs the same thing (typing into the compare box scrolls
    // it into view, which on a phone left the shutter wherever the last locator
    // happened to be). What is below the dialog's bottom edge is then measured and
    // named rather than left for a reader to assume — the H-30 mistake was a
    // verdict below the fold that no check could see.
    await page.evaluate(() => {
      const el = document.querySelector('.replayer');
      if (el) el.scrollTop = 0;
    });
    await settle(page, { extraFrames: 1 });

    checks.fold = await page.evaluate(() => {
      const box = document.querySelector('.modal-content')?.getBoundingClientRect();
      // SCOPED TO THE DIALOG. `.felt` is also a PokerTable class, so an
      // unscoped `document.querySelector('.felt')` measured the table behind the
      // modal and reported a 453px-tall felt inside a 689px dialog.
      const root = document.querySelector('.hand-history-modal');
      const of = (sel) => {
        const el = root?.querySelector(sel);
        if (!el || !box) return null;
        const r = el.getBoundingClientRect();
        return {
          y: Math.round(r.y),
          h: Math.round(r.height),
          insideDialog: r.top >= box.top - 1 && r.bottom <= box.bottom + 1,
        };
      };
      return {
        dialog: box ? { y: Math.round(box.y), h: Math.round(box.height) } : null,
        viewport: { w: window.innerWidth, h: window.innerHeight },
        replayerScrollTop: document.querySelector('.replayer')?.scrollTop ?? null,
        replayerScrollHeight: document.querySelector('.replayer')?.scrollHeight ?? null,
        replayerClientHeight: document.querySelector('.replayer')?.clientHeight ?? null,
        banner: of('.proof-banner'),
        witness: of('.witness-panel'),
        table: of('.scene'),
        scrubber: of('.scrubber'),
        felt: of('.felt'),
        log: of('.log-panel'),
        audit: of('.log-panel .audit'),
        pot: of('.pot-panel'),
        legal: of('.legal'),
      };
    });

    // ---- the table behind the modal is a money surface too ---------------
    const tableAgreement = named('table', await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    }));

    const folded = foldFigures(figures);
    checks.moneyFiguresChecked = folded.checked;
    checks.moneyMismatches = folded.mismatches;
    checks.figures = folded.figures.map((f) => ({
      label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail,
    }));
    checks.problems = problems;

    const verified = problems.length === 0 && folded.ok;
    const notes = verified
      ? `replayer asserted: ${walk.length} street stops walked (board 0/3/4/5/5 against the canister's `
        + `own community cards), ${dom.lines.length} log lines each matched to the hand record `
        + `(${folded.checked} money figures, blinds included), the log's amounts sum to the pot `
        + `(${chain.logSum} e8s = winners paid), the ordering witnessed in-browser from a mid-hand `
        + `sighting, and ${checks.protectedOnScreen} of ${checks.protectedTotal} protected notices `
        + 'measured on screen and unoccluded'
      : `REPLAYER NOT VERIFIED (${problems.length} problem(s)): ${problems.slice(0, 6).join(' | ')}`
        + (folded.ok ? '' : ` || money: ${folded.mismatches.slice(0, 3).join(' | ')}`);

    return withAgreement({ verified, checks, notes }, tableAgreement);
  },
};
