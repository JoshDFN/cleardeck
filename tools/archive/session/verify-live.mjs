// DOES THE RECONSTRUCTION MATCH REALITY, INCLUDING THE CARDS NOBODY PUBLISHES?
//
// The reconstruction's central product is the hole cards of players who folded.
// Those are exactly the cards with NO cross-check in the archive: a showdown hand
// can be compared against what the record published, and a folded hand cannot.
// If this tool's mapping from `dealt_in` order to deck positions were permuted,
// every showdown card would still land correctly on any hand where only one
// player showed, and every folded hand would come out wrong, silently, forever.
//
// So this script watches the deal happen.
//
//   1. play real hands on a real local table;
//   2. while each hand is live, read EVERY seat's own view of its own two cards
//      -- the canister shows a player their own cards whether or not they later
//      fold, so this captures the ground truth for cards the record will never
//      contain;
//   3. let the hand settle and be archived;
//   4. fetch the archive, reconstruct from the seed, and compare every card
//      against what that seat was actually holding.
//
// A single disagreement fails the run.
//
//   node tools/archive/session/verify-live.mjs --table table_1 --players 1,2 --hands 12
//
// LOCAL ONLY.

import {
  resetTable, seat, startHand, view, phaseOf, releaseSeats, tableActorFor, doAct, sleep,
} from '../../shots/lib/table-driver.mjs';
import { optional } from '../../shots/lib/agent.mjs';
import { readLocalIds } from '../../shots/lib/ids.mjs';
import { TABLE_CONFIGS } from '../../shots/lib/config.mjs';
import { assertNotMainnet, fetchBundle } from '../fetch/fetch-archive.mjs';
import { parseBundle } from '../lib/bundle.mjs';
import { reconstructHand } from '../lib/reconstruct.mjs';
import { cardFromCandid, prettyList } from '../lib/cards.mjs';
import { rng, honestAction } from './policy.mjs';

const args = (() => {
  const a = { table: 'table_1', players: [1, 2], hands: 12, seed: 5 };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 1) {
    const k = argv[i];
    if (k === '--table') a.table = argv[++i];
    else if (k === '--players') a.players = argv[++i].split(',').map(Number);
    else if (k === '--hands') a.hands = Number(argv[++i]);
    else if (k === '--seed') a.seed = Number(argv[++i]);
    else throw new Error(`unknown argument ${k}`);
  }
  return a;
})();

const ids = readLocalIds();
const tableId = ids[args.table];
if (!tableId) throw new Error(`no local canister id for ${args.table}`);
assertNotMainnet(tableId);
const cfg = TABLE_CONFIGS[args.table];
const BUY_IN = cfg.min_buy_in;
const random = rng(args.seed);

const seatOfPlayer = new Map(args.players.map((p, i) => [p, i]));
const playerOfSeat = new Map(args.players.map((p, i) => [i, p]));

const ownCards = (v) => {
  const me = v.players.map(optional).find((p) => p && p.is_self);
  const hc = me && optional(me.hole_cards);
  return hc ? [cardFromCandid(hc[0]), cardFromCandid(hc[1])] : null;
};

async function actOnce(playerNum, v) {
  const table = await tableActorFor(playerNum, tableId);
  const hole = ownCards(v);
  if (!hole) return;
  const me = v.players.map(optional).find((p) => p && p.is_self);
  const d = honestAction({
    phase: phaseOf(v),
    hole,
    board: v.community_cards.map(cardFromCandid),
    callAmount: Number(v.call_amount),
    canCheck: v.can_check,
    canRaise: v.can_raise,
    minRaise: Number(v.min_raise),
    minBet: Number(v.min_bet),
    pot: Number(v.pot),
    chips: Number(me.chips),
    myCurrentBet: Number(me.current_bet),
    bigBlind: Number(cfg.big_blind),
    random,
  });
  try {
    switch (d.kind) {
      case 'fold': await doAct.fold(table); break;
      case 'check': await doAct.check(table); break;
      case 'call': await doAct.call(table); break;
      case 'allin': await doAct.allIn(table); break;
      case 'bet': await doAct.bet(d.amount)(table); break;
      case 'raise': await doAct.raise(d.amount)(table); break;
      default: break;
    }
  } catch {
    try { if (v.can_check) await doAct.check(table); else await doAct.fold(table); } catch { /* the engine refused both */ }
  }
}

console.log(`table ${args.table} ${tableId}  players ${args.players.join(',')}  hands ${args.hands}`);
await releaseSeats(tableId, args.players);
resetTable(args.table, tableId);
for (const p of args.players) await seat(p, tableId, seatOfPlayer.get(p), BUY_IN);

/** seed_hash -> { seat -> the two cards that seat was actually holding } */
const observed = new Map();

for (let h = 1; h <= args.hands; h += 1) {
  // Re-stack anyone short, between hands only.
  for (const p of args.players) {
    const v = await view(p, tableId);
    const me = v?.players.map(optional).find((x) => x && x.is_self);
    if (!me || BigInt(me.chips) < BigInt(cfg.big_blind) * 20n) {
      const t = await tableActorFor(p, tableId);
      try { await t.leave_table(); } catch { /* not seated */ }
      await seat(p, tableId, seatOfPlayer.get(p), BUY_IN);
    }
  }

  let v = await view(args.players[0], tableId);
  if (phaseOf(v) === 'WaitingForPlayers' || phaseOf(v) === 'HandComplete') {
    try { await startHand(args.players[0], tableId); } catch { /* already running */ }
  }

  // THE OBSERVATION. Read every seat's own cards while the hand is live.
  v = await view(args.players[0], tableId);
  const proof = optional(v.shuffle_proof);
  if (!proof) { console.log(`  hand ${h}: no shuffle proof on the live view, skipping`); continue; }
  const seedHash = proof.seed_hash;
  const seen = new Map();
  for (const p of args.players) {
    const pv = await view(p, tableId);
    const cards = ownCards(pv);
    if (cards) seen.set(seatOfPlayer.get(p), { cards, principal: pv.players.map(optional).find((x) => x && x.is_self).principal.toText() });
  }
  observed.set(seedHash, seen);

  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    v = await view(args.players[0], tableId);
    const phase = phaseOf(v);
    if (phase === 'HandComplete' || phase === 'WaitingForPlayers') break;
    const actor = playerOfSeat.get(Number(v.action_on));
    if (actor === undefined) { await sleep(150); continue; }
    const sv = await view(actor, tableId);
    if (!sv?.is_my_turn) { await sleep(120); continue; }
    await actOnce(actor, sv);
  }
}

// --- now check the reconstruction against what was actually dealt ----------

console.log(`\nobserved ${observed.size} live deal(s); fetching the archive`);
const raw = await fetchBundle({ canisterId: ids.history, gateway: process.env.CLEARDECK_GATEWAY || 'http://localhost:8077' });
const bundle = parseBundle(raw);

let handsChecked = 0;
let cardsChecked = 0;
let cardsNeverPublished = 0;
let mismatches = 0;
const rows = [];

for (const hand of bundle.hands) {
  const seen = observed.get(hand.shuffleProof.seedHash);
  if (!seen) continue;
  const recon = reconstructHand(hand);
  if (recon.status !== 'VERIFIED') {
    console.log(`  hand ${hand.handId}: reconstruction ${recon.status}: ${recon.problems.join('; ')}`);
    mismatches += 1;
    continue;
  }
  handsChecked += 1;
  const published = new Set(
    hand.players.filter((p) => p.holeCards).map((p) => `${p.seat}|${p.principal}`),
  );
  for (const d of recon.holeCardsByDealIndex) {
    const truth = seen.get(d.seat);
    if (!truth) continue;
    const wasPublished = published.has(`${d.seat}|${d.principal}`);
    const agrees = truth.cards[0] === d.cards[0] && truth.cards[1] === d.cards[1];
    cardsChecked += 2;
    if (!wasPublished) cardsNeverPublished += 2;
    if (!agrees) {
      mismatches += 1;
      rows.push(`  hand ${hand.handId} seat ${d.seat}: canister dealt ${prettyList(truth.cards)}, `
        + `the seed reconstructs ${prettyList(d.cards)}  ${wasPublished ? '(published)' : '(NEVER PUBLISHED)'}`);
    }
  }
}

console.log('\nRECONSTRUCTION vs WHAT THE CANISTER ACTUALLY DEALT');
console.log('==================================================');
console.log(`  hands matched to a live observation      ${handsChecked}`);
console.log(`  hole cards compared                      ${cardsChecked}`);
console.log(`  of those, cards NO RECORD PUBLISHES      ${cardsNeverPublished}`);
console.log(`  DISAGREEMENTS                            ${mismatches}`);
for (const r of rows) console.log(r);

if (mismatches > 0 || handsChecked === 0) {
  console.log('\nFAILED. The tool is wrong, or the deal is: either way nothing downstream may be trusted.');
  process.exit(1);
}
console.log('\nEvery card matched, including the ones only the seed can recover.');
