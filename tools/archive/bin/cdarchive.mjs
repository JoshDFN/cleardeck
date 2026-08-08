#!/usr/bin/env node
// ClearDeck archive analyser. Offline: it reads a bundle written by
// fetch/fetch-archive.mjs and never opens a socket.
//
//   node tools/archive/bin/cdarchive.mjs verify    <bundle.json>
//   node tools/archive/bin/cdarchive.mjs hand      <bundle.json> --id N [--trace] [--ev]
//   node tools/archive/bin/cdarchive.mjs stats     <bundle.json> [--min-hands N]
//   node tools/archive/bin/cdarchive.mjs ev        <bundle.json> [--hand N] [--exact]
//   node tools/archive/bin/cdarchive.mjs collusion <bundle.json>
//
// Every command takes --table <principal>, --from-hand N, --to-hand N and
// --limit N to narrow the archive to one session or one table.
//
// Exit status is 0 only when everything asked for checked out.

import fs from 'node:fs';
import { parseBundle } from '../lib/bundle.mjs';
import { analyse } from '../lib/analyse.mjs';
import { reconstructHand } from '../lib/reconstruct.mjs';
import { replayHand } from '../lib/betting.mjs';
import { analyseHandEV } from '../lib/ev.mjs';
import { playerStats } from '../lib/stats.mjs';
import { buildLedger, coOccurrence, directedTransfer, foldsToOpponent, handsNeededFor, GATES } from '../lib/collusion.mjs';
import { describe, canisterRankName, score } from '../lib/evaluator.mjs';
import { prettyList } from '../lib/cards.mjs';
import { h1, h2, icp, rate, shortP, signedIcp, table } from '../lib/format.mjs';

const [, , command, bundlePath, ...rest] = process.argv;
const flag = (name) => rest.includes(name);
const value = (name, dflt) => {
  const i = rest.indexOf(name);
  return i >= 0 ? rest[i + 1] : dflt;
};

if (!command || !bundlePath) {
  console.error(fs.readFileSync(new URL(import.meta.url).pathname, 'utf8').split('\n').slice(1, 13).join('\n').replace(/^\/\/ ?/gm, ''));
  process.exit(2);
}

const parsed = parseBundle(JSON.parse(fs.readFileSync(bundlePath, 'utf8')));

/**
 * The window: which hands of the archive this run is about.
 *
 *   --table <principal>   one table's hands
 *   --from-hand / --to-hand   a range of archive ids, which is how you look at one
 *                             session inside an append-only archive
 *   --limit N             the FIRST N hands of that window, so two sessions can be
 *                         compared at the SAME sample size rather than at whatever
 *                         size each happened to reach
 *
 * Order matters: the table filter runs BEFORE the limit, or `--limit 400 --table X`
 * would silently mean "400 hands from all tables, of which some are X".
 */
const tableArg = value('--table', null);
if (tableArg) {
  const known = [...new Set(parsed.hands.map((h) => h.tableId))];
  if (!known.includes(tableArg)) {
    console.error(`no hands from table ${tableArg} in this bundle. Tables present:\n  ${known.join('\n  ')}`);
    process.exit(2);
  }
}
const fromHand = Number(value('--from-hand', 0));
const toHand = Number(value('--to-hand', Number.MAX_SAFE_INTEGER));
const limit = Number(value('--limit', 0));
const windowed = parsed.hands
  .filter((h) => h.handId >= fromHand && h.handId <= toHand)
  .filter((h) => !tableArg || h.tableId === tableArg);
const bundle = { ...parsed, hands: limit > 0 ? windowed.slice(0, limit) : windowed };
const tableFilter = null;   // already applied above; analyse() needs no second pass
let exitCode = 0;

const sourceLine = () => {
  const s = bundle.source ?? {};
  return `bundle ${bundlePath}\n  ${bundle.hands.length} hand record(s) in this window `
    + `(archive holds ${parsed.hands.length}), fetched ${bundle.fetchedAt}\n`
    + `  source: ${s.kind ?? 'unknown'} ${s.canister_id ?? ''} via ${s.gateway ?? 'unknown'}`;
};

// ---------------------------------------------------------------------------

function cmdVerify() {
  console.log(h1('RECONSTRUCTION'));
  console.log(sourceLine());
  const rows = [];
  let cardChecks = 0;
  let disagreements = 0;
  let derivedFolded = 0;
  let orderPinned = 0;
  const tally = { VERIFIED: 0, UNVERIFIABLE: 0, MISMATCH: 0 };
  const moneyFail = [];
  const blindRefunds = [];

  for (const hand of bundle.hands) {
    const recon = reconstructHand(hand);
    tally[recon.status] += 1;
    cardChecks += (recon.checks ?? []).length;
    const bad = (recon.checks ?? []).filter((c) => !c.agrees);
    disagreements += bad.length;
    derivedFolded += (recon.derivedHoleCards ?? []).length;
    if (recon.dealOrderPinned) orderPinned += 1;
    if (recon.status !== 'VERIFIED') {
      rows.push({ id: hand.handId, status: recon.status, detail: recon.problems.join('; ') });
    } else {
      const replay = replayHand(hand);
      if (!replay.ok) {
        moneyFail.push({ id: hand.handId, detail: replay.problems.join('; ') });
      }
      for (const r of replay.departureRefunds ?? []) {
        blindRefunds.push({ id: hand.handId, seat: r.seat, amount: r.amount, forced: r.forced });
      }
    }
  }

  console.log(`\ncards, from the seed alone:`);
  console.log(`  hands VERIFIED                 ${tally.VERIFIED}`);
  console.log(`  hands UNVERIFIABLE             ${tally.UNVERIFIABLE}   (no seed, or the record does not state P)`);
  console.log(`  hands the record CONTRADICTS   ${tally.MISMATCH}`);
  console.log(`  individual card checks         ${cardChecks}`);
  console.log(`  card DISAGREEMENTS             ${disagreements}`);
  console.log(`  hole cards reconstructed that the record never published   ${derivedFolded}`);
  console.log(`  hands where two or more seats published their cards, which PINS the deal order`);
  console.log(`  that the reconstructed folded hands depend on            ${orderPinned}`);
  console.log('  (on a hand where at most one seat showed, a permuted deal order would still verify');
  console.log('   and the folded hands would be wrong. tools/archive/session/verify-live.mjs is the');
  console.log('   check that watches the deal happen and compares the cards nobody publishes.)');
  console.log('\nmoney, from the record cross-checked against itself:');
  console.log(`  hands whose money checks out   ${tally.VERIFIED - moneyFail.length}`);
  console.log(`  hands whose money does NOT     ${moneyFail.length}`);
  if (blindRefunds.length > 0) {
    console.log(h2('a posted blind that came back'));
    console.log('  A player who leaves a live hand is handed back anything of theirs nobody had');
    console.log('  covered. Right after the blinds the big blind is always the top contributor, so');
    console.log('  LEAVING BEFORE THE ACTION REFUNDS (big_blind - small_blind) OF A POSTED BLIND.');
    console.log('  The pots still balance to the e8 and nobody else is short a chip; what moves is');
    console.log('  the PRICE, and it moves in favour of whoever walks out.');
    console.log(table(blindRefunds, [
      { header: 'hand', get: (r) => r.id, right: true },
      { header: 'seat', get: (r) => r.seat, right: true },
      { header: 'was forced to post', get: (r) => icp(r.forced), right: true },
      { header: 'handed back on leaving', get: (r) => icp(r.amount), right: true },
    ]));
  }

  if (rows.length > 0) {
    console.log(h2('hands that did not verify'));
    console.log(table(rows, [
      { header: 'hand', get: (r) => r.id, right: true },
      { header: 'status', get: (r) => r.status },
      { header: 'why', get: (r) => r.detail },
    ]));
  }
  if (moneyFail.length > 0) {
    console.log(h2('hands whose money does not add up'));
    console.log(table(moneyFail, [
      { header: 'hand', get: (r) => r.id, right: true },
      { header: 'problem', get: (r) => r.detail },
    ]));
  }
  if (disagreements > 0 || moneyFail.length > 0 || tally.MISMATCH > 0) exitCode = 1;
}

// ---------------------------------------------------------------------------

function cmdHand() {
  const id = Number(value('--id', rest[0]));
  const hand = bundle.hands.find((h) => h.handId === id);
  if (!hand) { console.error(`no hand with hand_id ${id} in this bundle`); process.exit(2); }

  const recon = reconstructHand(hand, { trace: flag('--trace') });
  const replay = replayHand(hand);

  console.log(h1(`HAND ${hand.handId} (table hand #${hand.handNumber})`));
  console.log(`table      ${hand.tableId}`);
  console.log(`blinds     ${icp(hand.smallBlind)} / ${icp(hand.bigBlind)} ICP   ante ${icp(hand.ante)}   rake ${icp(hand.rake)}`);
  console.log(`pot        ${icp(hand.totalPot)} ICP`);

  console.log(h2('step 0 -- does the revealed seed hash to the commitment?'));
  console.log(`  seed_hash      ${hand.shuffleProof.seedHash}`);
  console.log(`  revealed_seed  ${hand.shuffleProof.revealedSeed}`);
  console.log(`  SHA256(seed)   ${recon.commitment.computed ?? '(not computed)'}`);
  console.log(`  -> ${recon.commitment.ok ? 'MATCHES' : `FAILED: ${recon.commitment.problems.join('; ')}`}`);
  console.log('  (this proves only that the seed revealed is the seed committed to. It says nothing');
  console.log('   about whether the commitment existed before the cards did -- SHUFFLE-SPEC section 0.)');
  if (!recon.commitment.ok) { exitCode = 1; return; }

  console.log(h2('step 1 -- the deck, from the seed'));
  console.log(`  rejected draws: ${recon.rejections}`);
  for (let row = 0; row < 4; row += 1) {
    const cards = recon.deck.slice(row * 13, row * 13 + 13);
    console.log(`  ${String(row * 13).padStart(2)}..${row * 13 + 12}  ${prettyList(cards)}`);
  }
  console.log(`  symbols ${recon.deckSymbols}`);

  if (flag('--trace') && recon.steps) {
    console.log(h2('the hash chain, step by step (SHUFFLE-SPEC section 3)'));
    console.log(table(recon.steps.slice(0, 6).concat(recon.steps.slice(-3)), [
      { header: 'i', get: (s) => s.i, right: true },
      { header: 'n', get: (s) => s.n, right: true },
      { header: 'chain = SHA256(prev || i)', get: (s) => s.chain.slice(0, 32) + '…' },
      { header: 'draw (LE u64 of chain[0:8])', get: (s) => s.draw, right: true },
      { header: 'rej', get: (s) => s.rejected, right: true },
      { header: 'j = draw mod n', get: (s) => s.j, right: true },
    ]));
    console.log(`  (${recon.steps.length} steps in total; first six and last three shown)`);
  }

  if (recon.status === 'UNVERIFIABLE') {
    console.log(h2('step 2 -- P'));
    console.log(`  ${recon.problems.join('; ')}`);
    for (const n of recon.notes) console.log(`  ${n}`);
    exitCode = 1;
    return;
  }

  console.log(h2('step 2 -- P, READ from the record (never counted)'));
  console.log(`  dealt_in states P = ${recon.P}: ${hand.dealtIn.map((d) => `seat ${d.seat} ${shortP(d.principal)}`).join(', ')}`);
  console.log(`  so hole cards are deck[2k], deck[2k+1]; flop deck[${2 * recon.P + 1}..${2 * recon.P + 3}], `
    + `turn deck[${2 * recon.P + 5}], river deck[${2 * recon.P + 7}]`);

  console.log(h2('step 3 -- the cards, and what the record says about each'));
  console.log(table(recon.checks, [
    { header: 'what', get: (c) => c.what },
    { header: 'deck[i]', get: (c) => c.deckIndex, right: true },
    { header: 'reconstructed', get: (c) => c.derived },
    { header: 'the record says', get: (c) => c.recorded },
    { header: 'agrees', get: (c) => (c.agrees ? 'yes' : 'NO') },
  ]));
  if (recon.checks.length === 0) console.log('  (this hand published no cards at all: it ended before the flop with no showdown)');

  if (recon.derivedHoleCards.length > 0) {
    console.log(h2('cards NO RECORD PUBLISHES, recovered from the seed'));
    for (const d of recon.derivedHoleCards) {
      console.log(`  seat ${d.seat} ${shortP(d.principal)}  ${prettyList(d.cards)}   `
        + `deck[${d.deckIndices.join('], deck[')}]   (${d.reason})`);
    }
    console.log('  No commercial room can show you these. They are not in the record; they follow from the seed.');
  }

  if (recon.nonVacuity) {
    console.log(h2('is the board check vacuous? what the neighbouring P would have given'));
    for (const alt of recon.nonVacuity) {
      console.log(`  P = ${alt.P}: board ${prettyList(alt.board)}  -> ${alt.differs ? 'DIFFERENT, so the check binds' : 'the same, so the board check proves nothing here'}`);
    }
  }

  console.log(h2('the betting, replayed'));
  console.log(`  small blind seat ${replay.sbSeat} posts ${icp(replay.sbPosted)}   big blind seat ${replay.bbSeat} posts ${icp(replay.bbPosted)}`);
  console.log(table(replay.decisions, [
    { header: '#', get: (d) => d.index, right: true },
    { header: 'street', get: (d) => d.street },
    { header: 'seat', get: (d) => d.seat, right: true },
    { header: 'action', get: (d) => d.kind },
    { header: 'record says', get: (d) => (d.recordedAmount ? icp(d.recordedAmount) : ''), right: true },
    { header: 'goes in', get: (d) => (d.increment ? icp(d.increment) : ''), right: true },
    { header: 'to call', get: (d) => icp(d.toCall), right: true },
    { header: 'pot before', get: (d) => icp(d.potBefore), right: true },
  ]));
  for (const r of replay.refunds) {
    console.log(`  uncalled bet of ${icp(r.amount)} returned to seat ${r.seat} (${r.why})`);
  }
  console.log('\n  money checks');
  for (const c of replay.checks) console.log(`    ${c.ok ? 'ok  ' : 'FAIL'} ${c.name}  (${c.detail})`);
  if (!replay.ok) { exitCode = 1; return; }

  console.log(h2('the showdown, scored by this tool rather than by the canister'));
  const board = [...(hand.flop ?? []), ...(hand.turn ? [hand.turn] : []), ...(hand.river ? [hand.river] : [])];
  if (board.length === 5) {
    for (const d of recon.holeCardsByDealIndex) {
      const s = score([...d.cards, ...board]);
      const rec = hand.players.find((p) => p.seat === d.seat && p.principal === d.principal);
      const their = rec?.finalHandRank?.kind ?? null;
      const mine = canisterRankName(s);
      const agree = their === null ? '(record publishes none)' : (their === mine ? 'agrees' : `DISAGREES: record says ${their}`);
      console.log(`  seat ${d.seat} ${prettyList(d.cards)}  ${describe(s).categoryName.padEnd(17)} ${mine.padEnd(14)} ${agree}`);
      if (their !== null && their !== mine) exitCode = 1;
    }
  } else {
    console.log(`  the hand ended with ${board.length} board card(s) dealt, so there is no showdown to score.`);
    console.log(`  the board that WOULD have come, from the deck: ${prettyList([...recon.layout.flop, recon.layout.turn, recon.layout.river])}`);
  }

  if (flag('--ev')) {
    const ev = analyseHandEV(hand, recon, replay, { samples: flag('--exact') ? 10_000_000 : 200_000, hindsight: true });
    printHandEV(ev);
  }
}

// ---------------------------------------------------------------------------

function printHandEV(ev) {
  if (!ev.usable) { console.log(h2('EV')); console.log(`  not computed: ${ev.reasons.join('; ')}`); return; }
  console.log(h2('EV, with every card known'));
  console.log(`  the board the deck was always going to produce: ${prettyList(ev.deckBoard)}`);
  console.log(`  the board actually dealt:                        ${ev.boardDealt.length ? prettyList(ev.boardDealt) : '(none: the hand ended first)'}`);

  const withEquity = ev.decisions.filter((d) => d.hindsightEquity !== null);
  if (withEquity.length > 0) {
    console.log('\n  hindsight equity -- what each decision was worth WITH THE CARDS FACE UP.');
    console.log('  This is not a measure of how well the decision was played: nobody could see');
    console.log('  those cards. It is the number a colluder effectively does see.');
    console.log(table(withEquity, [
      { header: 'street', get: (d) => d.street },
      { header: 'seat', get: (d) => d.seat, right: true },
      { header: 'holding', get: (d) => (d.hole ? prettyList(d.hole) : '?') },
      { header: 'action', get: (d) => d.kind },
      { header: 'puts in', get: (d) => icp(d.increment), right: true },
      { header: 'equity then', get: (d) => `${(d.hindsightEquity * 100).toFixed(1)}%`, right: true },
      { header: 'how', get: (d) => d.equityMethod },
    ]));
  }

  const folds = ev.decisions.filter((d) => d.foldPrice);
  if (folds.length > 0) {
    console.log('\n  what each fold cost, EXACTLY -- only folds where calling would have closed');
    console.log('  the action, so nothing has to be assumed about what anyone would do next.');
    for (const d of folds) {
      const f = d.foldPrice;
      const verdict = f.delta > 0 ? `THE FOLD COST ${icp(f.delta)} ICP` : `the fold saved ${icp(-f.delta)} ICP`;
      console.log(`    seat ${d.seat} ${prettyList(f.hero.hole)} folds on the ${d.street} for ${icp(f.callAmount)}`);
      console.log(`      against ${f.opponents.map((o) => `seat ${o.seat} ${prettyList(o.hole)}`).join(', ')}  (${f.why})`);
      console.log(`      board ${prettyList(f.board)}  -> calling wins ${icp(f.wouldWin)}   ${verdict}`);
    }
  } else {
    console.log('\n  no fold in this hand had a closed counterfactual, so no fold is priced.');
  }

  if (ev.allIn) {
    console.log(`\n  all-in EV: betting finished on the ${ev.allIn.street} with `
      + `${ev.allIn.boardAtBettingEnd.length} board card(s) out, and the rest was dealt with no more betting.`);
    console.log(table(ev.allIn.perSeat, [
      { header: 'seat', get: (r) => r.seat, right: true },
      { header: 'was worth', get: (r) => icp(r.evShare), right: true },
      { header: 'actually won', get: (r) => icp(r.actualWon), right: true },
      { header: 'luck', get: (r) => signedIcp(r.luck), right: true },
    ]));
    console.log('  "luck" is the run-out, not the play: a positive number is a pot this player was');
    console.log('  not entitled to on equity, and it says nothing at all about how they played.');
  }
}

function cmdEv() {
  const only = value('--hand', null);
  const samples = flag('--exact') ? 10_000_000 : Number(value('--samples', 200_000));
  if (only) {
    const hand = bundle.hands.find((h) => h.handId === Number(only));
    if (!hand) { console.error(`no hand ${only}`); process.exit(2); }
    const recon = reconstructHand(hand);
    const replay = replayHand(hand);
    printHandEV(analyseHandEV(hand, recon, replay, { samples, hindsight: true }));
    return;
  }

  const { usable, excluded } = analyse(bundle, { ev: true, samples, hindsight: false, tables: tableFilter });
  console.log(h1('EV OVER THE SESSION'));
  console.log(sourceLine());
  console.log(`\n  ${usable.length} hand(s) usable, ${excluded.length} excluded`);

  const perPlayer = new Map();
  const get = (p) => {
    if (!perPlayer.has(p)) {
      perPlayer.set(p, { principal: p, allInEv: 0, allInActual: 0, allInHands: 0, foldsPriced: 0, foldCost: 0, foldSaved: 0, foldedWinner: 0 });
    }
    return perPlayer.get(p);
  };
  for (const u of usable) {
    if (!u.ev?.usable) continue;
    if (u.ev.allIn) {
      for (const r of u.ev.allIn.perSeat) {
        const a = get(r.principal);
        a.allInEv += r.evShare; a.allInActual += r.actualWon; a.allInHands += 1;
      }
    }
    for (const d of u.ev.decisions) {
      if (!d.foldPrice) continue;
      const a = get(u.replay.principalOfSeat.get(d.seat));
      a.foldsPriced += 1;
      if (d.foldPrice.delta > 0) { a.foldCost += d.foldPrice.delta; a.foldedWinner += 1; } else a.foldSaved += -d.foldPrice.delta;
    }
  }

  console.log(h2('all-in EV: what the pot was worth when the betting stopped'));
  const allInRows = [...perPlayer.values()].filter((r) => r.allInHands > 0);
  if (allInRows.length === 0) console.log('  no hand in this sample finished its betting before the river.');
  else {
    console.log(table(allInRows, [
      { header: 'player', get: (r) => shortP(r.principal) },
      { header: 'all-in hands', get: (r) => r.allInHands, right: true },
      { header: 'was worth', get: (r) => icp(r.allInEv), right: true },
      { header: 'actually won', get: (r) => icp(r.allInActual), right: true },
      { header: 'luck', get: (r) => signedIcp(r.allInActual - r.allInEv), right: true },
    ]));
  }

  console.log(h2('folds priced exactly (calling would have closed the action)'));
  const foldRows = [...perPlayer.values()].filter((r) => r.foldsPriced > 0);
  if (foldRows.length === 0) console.log('  no fold in this sample had a closed counterfactual.');
  else {
    console.log(table(foldRows, [
      { header: 'player', get: (r) => shortP(r.principal) },
      { header: 'folds priced', get: (r) => r.foldsPriced, right: true },
      { header: 'folded the winner', get: (r) => r.foldedWinner, right: true },
      { header: 'chips forgone', get: (r) => icp(r.foldCost), right: true },
      { header: 'chips saved', get: (r) => icp(r.foldSaved), right: true },
    ]));
    console.log('\n  "folded the winner" is only countable because the folded cards are recoverable');
    console.log('  from the seed. It is the strongest available evidence of a chip dump, and it is');
    console.log('  also perfectly normal in small numbers: everybody folds the best hand sometimes.');
  }
}

// ---------------------------------------------------------------------------

function cmdStats() {
  const minHands = Number(value('--min-hands', 100));
  const { usable, excluded } = analyse(bundle, { tables: tableFilter });
  console.log(h1('PLAYER STATISTICS'));
  console.log(sourceLine());
  console.log(`\n  ${usable.length} of ${bundle.hands.length} hand(s) usable${tableArg ? ` on table ${tableArg}` : ''}`);
  if (excluded.length > 0) {
    console.log(`  ${excluded.length} excluded:`);
    const byReason = new Map();
    for (const e of excluded) byReason.set(e.reason, (byReason.get(e.reason) ?? 0) + 1);
    for (const [r, n] of byReason) console.log(`    ${n}  ${r}`);
  }

  const stats = playerStats(usable);
  console.log(h2('core'));
  console.log(table(stats, [
    { header: 'player', get: (s) => shortP(s.principal) },
    { header: 'hands', get: (s) => s.hands, right: true },
    { header: 'VPIP', get: (s) => rate(s.vpip, { minN: minHands }) },
    { header: 'PFR', get: (s) => rate(s.pfr, { minN: minHands }) },
    { header: 'AF', get: (s) => (s.aggressionFactor === null ? '   -' : s.aggressionFactor.toFixed(2)), right: true },
    { header: 'bb/100', get: (s) => (s.bbPer100 === null ? '-' : s.bbPer100.toFixed(1)), right: true },
  ]));
  console.log('\n  VPIP  hands where the player voluntarily put money in pre-flop (blinds are not voluntary)');
  console.log('  PFR   hands where the player raised pre-flop');
  console.log('  AF    (post-flop bets + raises) / calls');
  console.log('  Every rate carries a 95% Wilson interval and the count it came from. A rate below');
  console.log(`  ${minHands} hands is labelled noise, because it is: at n=50 a "VPIP 40%" is anywhere from 27% to 55%.`);

  console.log(h2('showdown'));
  console.log(table(stats, [
    { header: 'player', get: (s) => shortP(s.principal) },
    { header: 'WTSD (of hands that saw a flop)', get: (s) => rate(s.wtsd, { minN: minHands }) },
    { header: 'W$SD (won at showdown)', get: (s) => rate(s.wsd, { minN: minHands }) },
    { header: 'WWSF (won when saw flop)', get: (s) => rate(s.wwsf, { minN: minHands }) },
  ]));

  console.log(h2('by position'));
  for (const s of stats) {
    console.log(`\n  ${shortP(s.principal)}  (${s.hands} hands)`);
    console.log(table(s.byPosition, [
      { header: 'position', get: (p) => p.label },
      { header: 'hands', get: (p) => p.hands, right: true },
      { header: 'VPIP', get: (p) => rate(p.vpip, { minN: minHands }) },
      { header: 'PFR', get: (p) => rate(p.pfr, { minN: minHands }) },
      { header: 'bb/100', get: (p) => (p.bbPer100 === null ? '-' : p.bbPer100.toFixed(1)), right: true },
    ]).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

// ---------------------------------------------------------------------------

function cmdCollusion() {
  const samples = Number(value('--samples', 60_000));
  const { usable, excluded } = analyse(bundle, { ev: true, samples, hindsight: false, tables: tableFilter });
  const ledger = buildLedger(usable);

  console.log(h1('COLLUSION SIGNALS'));
  console.log(sourceLine());
  console.log(`\n  ${usable.length} usable hand(s), ${excluded.length} excluded`);
  console.log('\n  READ THIS FIRST. Nothing below is a finding of collusion. These are screening');
  console.log('  signals with stated sample sizes and stated error rates. The strongest verdict any');
  console.log('  of them can return is REVIEW, which means "a human should look at this hand set".');
  console.log('  A false accusation is worse than a missed one, so every test refuses below its own');
  console.log('  sample floor and every test is corrected for the number of pairs examined.');

  console.log(h2('signal 1 -- do these two sit down together more than chance explains?'));
  const co = coOccurrence(ledger);
  if (co.structural.length > 0) {
    console.log('  THIS TEST HAS NO POWER ON THIS DATA:');
    for (const s of co.structural) console.log(`    - ${s}`);
    console.log('  Reporting "no signal" here would be misleading, so nothing is reported.');
  } else {
    console.log(table(co.rows.slice(0, 20), [
      { header: 'player A', get: (r) => shortP(r.a) },
      { header: 'player B', get: (r) => shortP(r.b) },
      { header: 'together', get: (r) => r.both, right: true },
      { header: 'expected', get: (r) => r.expected.toFixed(1), right: true },
      { header: 'x', get: (r) => (r.ratio === null ? '-' : r.ratio.toFixed(2)), right: true },
      { header: 'q (BH)', get: (r) => (r.q === null ? '-' : r.q.toExponential(2)), right: true },
      { header: 'verdict', get: (r) => r.verdict },
      { header: 'why', get: (r) => r.why.join('; ') },
    ]));
  }

  console.log(h2('signal 2 -- does A lose to B faster than A loses to everybody else?'));
  console.log('  Compared against A\'s OWN baseline. "This player loses money" is not collusion.');
  // THE LIMITATION HAS TO BE WHERE THE NUMBERS ARE, NOT ONLY IN docs/ARCHIVE.md.
  // Measured on 800 real, uncoordinated canister hands: this signal returned two
  // REVIEW rows at q=1.5e-4, both naming the strongest player, which is the same
  // verdict, count and q it returns on a scripted chip dump. "Directed" removes the
  // LOSER's skill from the comparison and leaves the OPPONENTS' skill in it, so the
  // null is false whenever opponents differ in skill and the error rate climbs with
  // sample size instead of holding at alpha. docs/ARCHIVE.md §6, DEFECTS.md T-42.
  console.log('  LIMITATION, MEASURED: this signal CANNOT distinguish a chip dump from an honest');
  console.log('  table with one strong player. On 800 uncoordinated hands it returned the same');
  console.log('  two REVIEW rows, at the same q, as a scripted dumper -- accusing the best player.');
  console.log('  Read every row below as "money moved this way", NOT as evidence of anything.');
  console.log('  Do not restrict an account on it. docs/ARCHIVE.md section 6.');
  const dt = directedTransfer(ledger);
  console.log(table(dt.slice(0, 24), [
    { header: 'from', get: (r) => shortP(r.from) },
    { header: 'to', get: (r) => shortP(r.to) },
    { header: 'hands', get: (r) => r.hands, right: true },
    { header: 'bb/100 to them', get: (r) => (r.meanBBPerHand === null ? '-' : (r.meanBBPerHand * 100).toFixed(1)), right: true },
    { header: 'vs own baseline', get: (r) => (r.excessBBPer100 === null ? '-' : r.excessBBPer100.toFixed(1)), right: true },
    { header: 'p', get: (r) => (r.p === null ? '-' : r.p.toExponential(2)), right: true },
    { header: 'q (BH)', get: (r) => (r.q === null ? '-' : r.q.toExponential(2)), right: true },
    { header: 'verdict', get: (r) => r.verdict },
    { header: 'opps in baseline', get: (r) => r.baselineOpponents ?? '-', right: true },
    { header: 'note', get: (r) => r.why.concat(r.needed ? [r.needed] : []).concat(r.caveat ? [r.caveat] : []).join('; ') },
  ]));
  const need = handsNeededFor(ledger, 5);
  if (need) {
    console.log(`\n  At the variance this population actually shows, detecting a directed leak of`);
    console.log(`  5 bb/100 at 80% power needs about ${need} shared hands per pair. Below that, this`);
    console.log('  test cannot tell a chip dump from a bad session, and it says so rather than guessing.');
  }

  console.log(h2('signal 3 -- did A fold the winner to B more often than to anyone else?'));
  console.log('  Only possible because the folded cards are recoverable from the seed, and only');
  console.log('  counted on folds whose price is exact. A strong player wins by betting when ahead,');
  console.log('  not by their opponent folding the best hand at an anomalous rate.');
  const fo = foldsToOpponent(ledger);
  if (fo.length === 0) console.log('  no exactly-priced folds in this sample.');
  else {
    console.log(table(fo.slice(0, 24), [
      { header: 'folder', get: (r) => shortP(r.folder) },
      { header: 'to', get: (r) => shortP(r.beneficiary) },
      { header: 'priced folds', get: (r) => r.folds, right: true },
      { header: 'folded the winner', get: (r) => rate(r.rate, { minN: GATES.MIN_CLOSED_FOLDS }) },
      { header: 'baseline', get: (r) => rate(r.baselineRate, { minN: GATES.MIN_CLOSED_FOLDS }) },
      { header: 'baseline is', get: (r) => r.baselineSource ?? '-' },
      { header: 'of this folder\'s priced folds', get: (r) => (r.shareOfFolds === null ? '-' : `${(r.shareOfFolds * 100).toFixed(0)}% of ${r.allFolds}`), right: true },
      { header: 'bb given up', get: (r) => r.bbForgone.toFixed(1), right: true },
      { header: 'of all they gave up', get: (r) => (r.shareOfForgone === null ? '-' : `${(r.shareOfForgone * 100).toFixed(0)}%`), right: true },
      { header: 'q (BH)', get: (r) => (r.q === null ? '-' : r.q.toExponential(2)), right: true },
      { header: 'verdict', get: (r) => r.verdict },
      { header: 'why', get: (r) => r.why.concat(r.caveat ? [r.caveat] : []).join('; ') },
    ]));
  }

  const reviews = [...dt, ...fo, ...co.rows].filter((r) => r.verdict === 'REVIEW');
  console.log(h2('summary'));
  if (reviews.length === 0) {
    console.log('  No signal in this sample cleared its threshold. That is NOT a clean bill of health:');
    console.log('  read the "note" column for what each test could and could not have detected.');
  } else {
    console.log(`  ${reviews.length} pair-signal(s) reached REVIEW. A human should look at these hand sets.`);
    console.log('  REVIEW is not a finding. It means the numbers are unlikely enough to be worth a look.');
  }
}

// ---------------------------------------------------------------------------

switch (command) {
  case 'verify': cmdVerify(); break;
  case 'hand': cmdHand(); break;
  case 'stats': cmdStats(); break;
  case 'ev': cmdEv(); break;
  case 'collusion': cmdCollusion(); break;
  default:
    console.error(`unknown command "${command}"`);
    process.exit(2);
}
process.exit(exitCode);
