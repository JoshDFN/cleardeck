// Plays real hands on the LOCAL replica so the analyser has real data to be
// checked against.
//
// Every hand is dealt by the real table canister, settled by the real settlement
// path and archived by the real archive canister. Nothing here writes game state
// or fabricates a record; the scripted players only choose actions, exactly as a
// browser would.
//
//   node tools/archive/session/play-session.mjs --table table_2 \
//        --players 1,2,3 --hands 250 --mode honest --seed 11
//   node tools/archive/session/play-session.mjs --table table_3 \
//        --players 4,5,6 --hands 250 --mode collude --pair 4,5 --seed 22
//
// LOCAL ONLY: it resolves canister ids from .icp/cache/mappings/local.ids.json
// and refuses anything that names a mainnet canister.

import {
  resetTable, seat, startHand, view, phaseOf, releaseSeats, tableActorFor, doAct, sleep,
} from '../../shots/lib/table-driver.mjs';
import { optional, unwrap } from '../../shots/lib/agent.mjs';
import { devPlayerPrincipal } from '../../shots/lib/identities.mjs';
import { readLocalIds } from '../../shots/lib/ids.mjs';
import { TABLE_CONFIGS } from '../../shots/lib/config.mjs';
import { assertNotMainnet } from '../fetch/fetch-archive.mjs';
import { cardFromCandid } from '../lib/cards.mjs';
import { rng, honestAction, colludingAction } from './policy.mjs';

const args = (() => {
  const a = { table: 'table_2', players: [1, 2, 3], hands: 40, mode: 'honest', pair: null, seed: 7, buyin: null };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 1) {
    const k = argv[i];
    if (k === '--table') a.table = argv[++i];
    else if (k === '--players') a.players = argv[++i].split(',').map(Number);
    else if (k === '--hands') a.hands = Number(argv[++i]);
    else if (k === '--mode') a.mode = argv[++i];
    else if (k === '--pair') a.pair = argv[++i].split(',').map(Number);
    else if (k === '--seed') a.seed = Number(argv[++i]);
    else if (k === '--buyin') a.buyin = BigInt(argv[++i]);
    else throw new Error(`unknown argument ${k}`);
  }
  return a;
})();

const ids = readLocalIds();
const tableId = ids[args.table];
if (!tableId) throw new Error(`no local canister id for ${args.table}`);
assertNotMainnet(tableId);

const cfg = TABLE_CONFIGS[args.table];
if (!cfg) throw new Error(`no known config literal for ${args.table}; cannot reset it`);
const BUY_IN = args.buyin ?? cfg.min_buy_in;
const BIG_BLIND = Number(cfg.big_blind);

const random = rng(args.seed);
const seatOfPlayer = new Map(args.players.map((p, i) => [p, i]));
const playerOfSeat = new Map(args.players.map((p, i) => [i, p]));
const principalOf = new Map(args.players.map((p) => [p, devPlayerPrincipal(p)]));

const dumper = args.mode === 'collude' ? args.pair[0] : null;
const beneficiary = args.mode === 'collude' ? args.pair[1] : null;

const log = (...m) => console.log(...m);

/** Cards a seat can see of its own hand, from the real view. */
function myHole(v) {
  const me = v.players.map(optional).find((p) => p && p.is_self);
  const hc = me && optional(me.hole_cards);
  return hc ? [cardFromCandid(hc[0]), cardFromCandid(hc[1])] : null;
}

const boardOf = (v) => v.community_cards.map(cardFromCandid);

/** Seats still contesting the pot (dealt in, not folded). */
function liveSeats(v) {
  return v.players.map(optional).filter((p) => p && !p.has_folded).map((p) => Number(p.seat));
}

async function actOnce(playerNum, v) {
  const table = await tableActorFor(playerNum, tableId);
  const hole = myHole(v);
  if (!hole) return false;                       // not dealt in this hand
  const me = v.players.map(optional).find((p) => p && p.is_self);
  const ctx = {
    phase: phaseOf(v),
    hole,
    board: boardOf(v),
    callAmount: Number(v.call_amount),
    canCheck: v.can_check,
    canRaise: v.can_raise,
    minRaise: Number(v.min_raise),
    minBet: Number(v.min_bet),
    pot: Number(v.pot),
    chips: Number(me.chips),
    myCurrentBet: Number(me.current_bet),
    bigBlind: BIG_BLIND,
    random,
  };

  let decision;
  if (args.mode === 'collude' && (playerNum === dumper || playerNum === beneficiary)) {
    const partner = playerNum === dumper ? beneficiary : dumper;
    const partnerSeat = seatOfPlayer.get(partner);
    const live = liveSeats(v);
    const partnerInHand = live.includes(partnerSeat);
    const headsUp = live.length === 2 && live.includes(seatOfPlayer.get(playerNum)) && partnerInHand;
    decision = colludingAction(ctx, playerNum === dumper ? 'dumper' : 'beneficiary', partnerInHand, headsUp);
  } else {
    decision = honestAction(ctx);
  }

  try {
    switch (decision.kind) {
      case 'fold': await doAct.fold(table); break;
      case 'check': await doAct.check(table); break;
      case 'call': await doAct.call(table); break;
      case 'allin': await doAct.allIn(table); break;
      case 'bet': await doAct.bet(decision.amount)(table); break;
      case 'raise': await doAct.raise(decision.amount)(table); break;
      default: throw new Error(`bad decision ${JSON.stringify(decision)}`);
    }
  } catch (e) {
    // A rejected action is the engine enforcing its own rules on a crude bot.
    // Fall back to the safest legal move rather than stalling the session.
    const msg = String(e.message || e);
    try {
      if (ctx.canCheck) await doAct.check(table);
      else if (ctx.callAmount > 0 && ctx.callAmount < ctx.chips) await doAct.call(table);
      else await doAct.fold(table);
    } catch (e2) {
      log(`    ! seat ${seatOfPlayer.get(playerNum)} could not act: ${msg} / ${String(e2.message || e2)}`);
      return false;
    }
  }
  return true;
}

/**
 * Keeps every scripted player's stack near the buy-in between hands.
 *
 * Short stacks rebuy, the way a real player does. Deep stacks are ALSO reset,
 * which is not realism: it is so the chips a dumper hands over come back out of
 * escrow instead of piling up on one seat and draining the local funders. Both
 * directions go through the real `leave_table` / `buy_in` path.
 */
async function ensureStacked(playerNum) {
  const seatIndex = seatOfPlayer.get(playerNum);
  const v = await view(playerNum, tableId);
  const me = v ? v.players.map(optional).find((p) => p && p.is_self) : null;
  const low = BigInt(BIG_BLIND) * 25n;
  const high = BUY_IN * 3n;
  if (me && BigInt(me.chips) >= low && BigInt(me.chips) <= high) return;
  if (me) {
    const table = await tableActorFor(playerNum, tableId);
    try { unwrap(await table.leave_table(), 'leave_table'); } catch { /* not seated */ }
  }
  await seat(playerNum, tableId, seatIndex, BUY_IN);
}

async function playOneHand(handIndex) {
  for (const p of args.players) await ensureStacked(p);

  let v = await view(args.players[0], tableId);
  const before = Number(v.hand_number);
  if (phaseOf(v) === 'WaitingForPlayers' || phaseOf(v) === 'HandComplete') {
    try { await startHand(args.players[0], tableId); } catch (e) {
      const msg = String(e.message || e);
      if (!/in progress|already/i.test(msg)) throw e;
    }
  }

  const deadline = Date.now() + 90_000;
  let guard = 0;
  while (Date.now() < deadline) {
    v = await view(args.players[0], tableId);
    const phase = phaseOf(v);
    if (phase === 'HandComplete' || phase === 'WaitingForPlayers') {
      if (Number(v.hand_number) > before) return { handNumber: Number(v.hand_number), pot: Number(v.pot) };
      if (guard++ > 8) return null;
      await sleep(150);
      continue;
    }
    const onSeat = Number(v.action_on);
    const actor = playerOfSeat.get(onSeat);
    if (actor === undefined) { await sleep(150); continue; }
    const seatView = await view(actor, tableId);
    if (!seatView?.is_my_turn) { await sleep(120); continue; }
    await actOnce(actor, seatView);
  }
  throw new Error(`hand ${handIndex} did not finish within 90s (phase ${phaseOf(v)})`);
}

// --- run --------------------------------------------------------------------

log(`table ${args.table} ${tableId}  mode=${args.mode}  players=${args.players.join(',')}  hands=${args.hands}  seed=${args.seed}`);
for (const p of args.players) log(`  player ${p} seat ${seatOfPlayer.get(p)} ${principalOf.get(p)}`);
if (args.mode === 'collude') log(`  COLLUDING PAIR: player ${dumper} dumps to player ${beneficiary}`);

await releaseSeats(tableId, args.players);
resetTable(args.table, tableId);
for (const p of args.players) await seat(p, tableId, seatOfPlayer.get(p), BUY_IN);

let played = 0;
const startedAt = Date.now();
for (let h = 1; h <= args.hands; h += 1) {
  try {
    const r = await playOneHand(h);
    if (r) played += 1;
  } catch (e) {
    log(`  hand ${h} failed: ${String(e.message || e).split('\n')[0]}`);
  }
  if (h % 25 === 0) {
    const rate = ((Date.now() - startedAt) / 1000 / h).toFixed(1);
    log(`  ${h}/${args.hands} hands (${played} settled, ${rate}s/hand)`);
  }
}
log(`done: ${played} hands settled on ${args.table}`);
