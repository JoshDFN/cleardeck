#!/usr/bin/env node
// The gates. Every one of them has an answer that is known BEFORE the code runs
// -- a combinatorial identity, a document, a fixture built to fail, or a
// population whose ground truth is set by construction.
//
// A gate nothing runs is worse than no gate, so this file is what
// `./scripts/dev.sh` would call and what tools/archive/README.md points at.
//
//   node tools/archive/selftest/run.mjs
//
// Exit status 0 only if every case passes. No replica, no network.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { shuffle, seedBytes, sha256Hex, checkCommitment, drawBound } from '../lib/spec-shuffle.mjs';
import { deal } from '../lib/spec-deal.mjs';
import { buildDeck, deckSymbols, cardIndex } from '../lib/cards.mjs';
import { score, describe, CATEGORY } from '../lib/evaluator.mjs';
import { equity, countCombinations } from '../lib/equity.mjs';
import { buildPots, awardPots } from '../lib/pots.mjs';
import { parseBundle, BundleError } from '../lib/bundle.mjs';
import { reconstructHand } from '../lib/reconstruct.mjs';
import { replayHand } from '../lib/betting.mjs';
import { analyse } from '../lib/analyse.mjs';
import { coOccurrence, directedTransfer, GATES } from '../lib/collusion.mjs';
import { wilson, benjaminiHochberg, fisherRightTail, rng } from '../lib/statistics.mjs';
import { syntheticPopulation } from './synthetic.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..', '..');

let passed = 0;
const failures = [];

function check(name, fn) {
  try {
    const detail = fn();
    passed += 1;
    console.log(`  ok    ${name}${detail ? `  (${detail})` : ''}`);
  } catch (e) {
    failures.push({ name, message: String(e.message || e) });
    console.log(`  FAIL  ${name}\n          ${String(e.message || e).split('\n').join('\n          ')}`);
  }
}

const assert = (cond, msg) => { if (!cond) throw new Error(msg); };
const eq = (a, b, msg) => assert(a === b, `${msg}: got ${JSON.stringify(a)}, want ${JSON.stringify(b)}`);

// ===========================================================================
console.log('\nTHE SHUFFLE -- this tool against the canister, and against the document');
// ===========================================================================

check('the 2,000 committed golden vectors replay exactly', () => {
  const file = path.join(REPO, 'src', 'poker_core', 'tests', 'golden_vectors.txt');
  const lines = fs.readFileSync(file, 'utf8').split('\n').filter((l) => l.startsWith('S '));
  assert(lines.length >= 2000, `expected at least 2000 vectors, found ${lines.length}`);
  let bad = 0;
  for (const line of lines) {
    const [, seedHex, expected] = line.trim().split(/\s+/);
    const { deck } = shuffle(seedBytes(seedHex));
    if (deckSymbols(deck) !== expected) bad += 1;
  }
  eq(bad, 0, 'vectors that did not reproduce');
  return `${lines.length} vectors, 0 mismatches`;
});

check('SHUFFLE-SPEC section 5 worked example reproduces, card for card', () => {
  const seed = Buffer.from(sha256Hex(Buffer.from('ClearDeck shuffle spec v1 worked example', 'ascii')), 'hex');
  eq(seed.toString('hex'), 'bdfa6f039a2d44dc8a330c5435891ec693bc5a5f4cffd94454d3205c0f4873dc', 'the seed itself');
  eq(sha256Hex(seed), 'c1d4f9e294d69f17f206846cc7b96462901816938b7cd10c36624d3f6af31643', 'SHA256(seed)');
  const { deck, rejections } = shuffle(seed);
  eq(rejections, 0, 'rejected draws');
  eq(deckSymbols(deck), 'WlVSTCfukXxerqbIPzDiQJdaYwKZcNsAEvGhRoMjUOptBFmgHLyn', 'the deck');
  const hand = deal(deck, 2);
  eq(hand.holeCards[0].join(' '), 'Jd Kc', 'seat 0');
  eq(hand.holeCards[1].join(' '), 'Td 7d', 'seat 1');
  eq(hand.flop.join(' '), '4h 7c 9s', 'flop');
  eq(hand.turn, 'Qd', 'turn');
  eq(hand.river, '6c', 'river');
  return 'deck, hole cards, flop, turn and river all match the document';
});

check('the pre-shuffle deck order is the one section 2 pins', () => {
  const d = buildDeck();
  eq(d[0], '2h', 'deck[0]'); eq(d[12], 'Ah', 'deck[12]'); eq(d[13], '2d', 'deck[13]');
  eq(d[26], '2c', 'deck[26]'); eq(d[39], '2s', 'deck[39]'); eq(d[51], 'As', 'deck[51]');
  for (let i = 0; i < 52; i += 1) eq(cardIndex(d[i]), i, `index of ${d[i]}`);
  return '52 cards, every index self-consistent';
});

check('the rejection bound is the largest multiple of n below 2^64', () => {
  for (let n = 2; n <= 52; n += 1) {
    const b = drawBound(n);
    assert(b % BigInt(n) === 0n, `bound(${n}) is not a multiple of n`);
    assert((1n << 64n) - b < BigInt(n), `bound(${n}) is not the largest such multiple`);
  }
  eq(drawBound(52).toString(), '18446744073709551600', 'bound(52)');
  eq(((1n << 64n) - drawBound(43)).toString(), '41', 'the worst-case rejection count');
  return 'matches the constants in SHUFFLE-SPEC 3.3';
});

check('the commitment check names the mistake instead of accusing', () => {
  const seed = 'ab'.repeat(32);
  const hash = sha256Hex(Buffer.from(seed, 'hex'));
  assert(checkCommitment(hash, seed).ok, 'a genuine pair must pass');
  const swapped = checkCommitment(seed, hash);
  assert(!swapped.ok, 'a transposed pair must not pass');
  assert(swapped.problems.some((p) => /each other/.test(p)), 'and must say the fields were transposed');
  assert(checkCommitment('', seed).problems.some((p) => /no seed_hash/.test(p)), 'empty hash must be named');
  assert(checkCommitment(hash, '').problems.some((p) => /no seed has been revealed/.test(p)), 'empty seed must be named');
  return 'transposed, empty and malformed inputs each get their own message';
});

// ===========================================================================
console.log('\nTHE EVALUATOR -- against combinatorics, and against a slower second path');
// ===========================================================================

check('every 5-card hand category has exactly its textbook count over all C(52,5)', () => {
  const deck = buildDeck();
  const counts = new Array(9).fill(0);
  for (let a = 0; a < 48; a += 1) {
    for (let b = a + 1; b < 49; b += 1) {
      for (let c = b + 1; c < 50; c += 1) {
        for (let d = c + 1; d < 51; d += 1) {
          for (let e = d + 1; e < 52; e += 1) {
            counts[describe(score([deck[a], deck[b], deck[c], deck[d], deck[e]])).category] += 1;
          }
        }
      }
    }
  }
  const want = [1302540, 1098240, 123552, 54912, 10200, 5108, 3744, 624, 40];
  const names = ['high card', 'pair', 'two pair', 'trips', 'straight', 'flush', 'full house', 'quads', 'straight flush'];
  for (let i = 0; i < 9; i += 1) eq(counts[i], want[i], names[i]);
  eq(counts.reduce((x, y) => x + y, 0), 2598960, 'total five-card hands');
  return 'all 2,598,960 hands, nine categories, every count exact';
});

check('direct 7-card scoring equals best-of-21 five-card scoring', () => {
  // The slow path is a different algorithm, written here and used nowhere else.
  const combos5 = (cards) => {
    const out = [];
    for (let a = 0; a < 3; a += 1) for (let b = a + 1; b < 4; b += 1) for (let c = b + 1; c < 5; c += 1) {
      for (let d = c + 1; d < 6; d += 1) for (let e = d + 1; e < 7; e += 1) out.push([cards[a], cards[b], cards[c], cards[d], cards[e]]);
    }
    return out;
  };
  const rand = rng(20260807);
  const deck = buildDeck();
  for (let trial = 0; trial < 30000; trial += 1) {
    const work = deck.slice();
    for (let i = 0; i < 7; i += 1) {
      const j = i + Math.floor(rand() * (52 - i));
      const t = work[i]; work[i] = work[j]; work[j] = t;
    }
    const seven = work.slice(0, 7);
    const fast = score(seven);
    const slow = Math.max(...combos5(seven).map((five) => score(five)));
    eq(fast, slow, `7-card hand ${seven.join(' ')}`);
  }
  return '30,000 random seven-card hands, two independent paths, identical every time';
});

check('a duplicated card is refused, not scored', () => {
  let threw = false;
  try { score(['Ah', 'Ah', 'Kd', 'Qc', '9h']); } catch { threw = true; }
  assert(threw, 'a five-card hand with a repeated card must throw');
  return 'the failure that pays a flush assembled from four physical cards';
});

// ===========================================================================
console.log('\nEQUITY -- identities that are true by construction');
// ===========================================================================

check('equity enumerates exactly C(pool, cards to come) boards and sums to 1', () => {
  const r = equity([['Ah', 'Ad'], ['Kh', 'Kd']], ['2c', '7d', '9s']);
  eq(r.method, 'exact', 'method');
  eq(r.boards, countCombinations(45, 2), 'boards enumerated');
  assert(Math.abs(r.equity[0] + r.equity[1] - 1) < 1e-12, 'equities must sum to 1');
  return `${r.boards} boards, equities sum to 1`;
});

check('a complete board gives equity 1 to the better hand and 0.5/0.5 to a chop', () => {
  const board = ['2c', '7d', '9s', 'Jh', '4d'];
  const won = equity([['Ah', 'Ad'], ['Kh', 'Kd']], board);
  eq(won.equity[0], 1, 'aces on a complete board');
  const chop = equity([['Ah', 'Kd'], ['As', 'Kc']], ['Qh', 'Jh', 'Th', '2c', '3d']);
  eq(chop.equity[0], 0.5, 'identical straights must split');
  return 'no run-out left, so equity is the result';
});

check('equity from the fast evaluator equals equity from the slow one', () => {
  // Same enumeration, different scorer: catches a fast-path bug that would move
  // every EV number in the tool by a little and nothing by a lot.
  const hands = [['Ah', 'Kh'], ['7c', '7d']];
  const board = ['2s', '9d', 'Tc'];
  const fast = equity(hands, board);
  const deck = buildDeck().filter((c) => ![...board, ...hands.flat()].includes(c));
  let share = 0;
  let n = 0;
  const slowScore = (cards) => {
    let best = -1;
    for (let a = 0; a < 3; a += 1) for (let b = a + 1; b < 4; b += 1) for (let c = b + 1; c < 5; c += 1) {
      for (let d = c + 1; d < 6; d += 1) for (let e = d + 1; e < 7; e += 1) {
        best = Math.max(best, score([cards[a], cards[b], cards[c], cards[d], cards[e]]));
      }
    }
    return best;
  };
  for (let i = 0; i < deck.length; i += 1) {
    for (let j = i + 1; j < deck.length; j += 1) {
      const full = [...board, deck[i], deck[j]];
      const s0 = slowScore([...hands[0], ...full]);
      const s1 = slowScore([...hands[1], ...full]);
      share += s0 > s1 ? 1 : (s0 === s1 ? 0.5 : 0);
      n += 1;
    }
  }
  eq(n, fast.boards, 'board count');
  assert(Math.abs(share / n - fast.equity[0]) < 1e-12, `equity differs: ${share / n} vs ${fast.equity[0]}`);
  return `${n} boards scored twice, identical`;
});

// ===========================================================================
console.log('\nSIDE POTS');
// ===========================================================================

check('layers are built and awarded so that chips are conserved exactly', () => {
  const contributed = new Map([[0, 10], [1, 50], [2, 100]]);
  const pots = buildPots(contributed, [0, 1, 2]);
  eq(pots.length, 3, 'layer count');
  eq(pots[0].amount, 30, 'main pot 10*3');
  eq(pots[1].amount, 80, 'second layer 40*2');
  eq(pots[2].amount, 50, 'third layer 50*1');
  const won = awardPots(pots, new Map([[0, 900], [1, 500], [2, 100]]));
  eq(won.get(0), 30, 'short stack wins only what it covered');
  eq(won.get(1), 80, 'second best takes the layer it covered');
  eq(won.get(2), 50, 'the uncovered layer comes back to its only contributor');
  eq([...won.values()].reduce((a, b) => a + b, 0), 160, 'total paid out equals total in');
  return 'a 10/50/100 three-way all-in, to the chip';
});

check('a folded contributor funds the layers but wins none of them', () => {
  const contributed = new Map([[0, 100], [1, 100], [2, 40]]);
  const pots = buildPots(contributed, [0, 1]);        // seat 2 folded
  const won = awardPots(pots, new Map([[0, 900], [1, 100]]));
  eq([...won.values()].reduce((a, b) => a + b, 0), 240, 'the folded 40 is still paid out');
  eq(won.get(0), 240, 'to the best remaining hand');
  return 'chips a folder left behind still leave the pot';
});

// ===========================================================================
console.log('\nRECONSTRUCTION -- fixtures built to fail');
// ===========================================================================

const FIXTURE = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'fixtures', 'one-hand.json'), 'utf8'));
const clone = () => JSON.parse(JSON.stringify(FIXTURE));

check('the honest fixture verifies', () => {
  const b = parseBundle(clone());
  const r = reconstructHand(b.hands[0]);
  eq(r.status, 'VERIFIED', 'status');
  assert(r.checks.length > 0, 'a verification that checks nothing is not a verification');
  assert(r.checks.every((c) => c.agrees), 'every card must agree');
  return `${r.checks.length} card checks`;
});

check('ONE ALTERED CARD is caught, and named', () => {
  const raw = clone();
  raw.hands[0].flop[1] = raw.hands[0].flop[1] === 'Ah' ? 'Kd' : 'Ah';
  const r = reconstructHand(parseBundle(raw).hands[0]);
  eq(r.status, 'MISMATCH', 'status');
  assert(r.problems.some((p) => /flop\[1\]/.test(p)), `the problem must name the card: ${r.problems}`);
  return r.problems[0];
});

check('a seed that does not hash to the commitment stops at step 0', () => {
  const raw = clone();
  const s = raw.hands[0].shuffle_proof.revealed_seed;
  raw.hands[0].shuffle_proof.revealed_seed = (s[0] === '0' ? '1' : '0') + s.slice(1);
  const r = reconstructHand(parseBundle(raw).hands[0]);
  eq(r.status, 'MISMATCH', 'status');
  assert(r.checks.length === 0, 'no card may be checked once the commitment fails');
  return 'refuses to reconstruct anything from an uncommitted seed';
});

check('a record with no dealt_in is UNVERIFIABLE, and no board is guessed', () => {
  const raw = clone();
  raw.hands[0].dealt_in = null;
  const r = reconstructHand(parseBundle(raw).hands[0]);
  eq(r.status, 'UNVERIFIABLE', 'status');
  eq(r.checks.length, 0, 'nothing may be checked without P');
  assert(r.notes.some((n) => /not to guess/.test(n)), 'and it must say why');
  return 'FINDING 30: the hole cards would match at any P while the board came out wrong';
});

check('the WRONG P gives a different board -- so the board check is not vacuous', () => {
  const b = parseBundle(clone());
  const r = reconstructHand(b.hands[0]);
  assert(r.nonVacuity && r.nonVacuity.length > 0, 'the non-vacuity report must exist');
  assert(r.nonVacuity.some((n) => n.differs), 'at least one neighbouring P must give a different board');
  return r.nonVacuity.map((n) => `P=${n.P} ${n.differs ? 'differs' : 'SAME'}`).join(', ');
});

check('a hand says whether its own deal ORDER is pinned, or only assumed', () => {
  // The reconstructed cards of a folded player rest entirely on the mapping
  // dealt_in[k] -> deck[2k], deck[2k+1], and nothing in the record checks it.
  // Two published hands pin it; one does not, and the tool must not pretend
  // otherwise on a hand where only one seat showed.
  const pinned = reconstructHand(parseBundle(clone()).hands[0]);
  eq(pinned.publishedPairs, 2, 'the fixture must publish two hands');
  eq(pinned.dealOrderPinned, true, 'two published hands pin the order');

  const thin = clone();
  const showing = thin.hands[0].players.filter((p) => p.hole_cards);
  showing[0].hole_cards = null;                        // now only one seat shows
  const r = reconstructHand(parseBundle(thin).hands[0]);
  eq(r.publishedPairs, 1, 'one published hand');
  eq(r.dealOrderPinned, false, 'one published hand does NOT pin the order');
  eq(r.status, 'VERIFIED', 'and the hand still verifies, which is exactly the danger');
  return 'a hand where one seat showed still VERIFIES while its folded hands rest on an unchecked order';
});

check('a hand that fails its cards is EXCLUDED from every statistic', () => {
  const raw = clone();
  raw.hands[0].flop[0] = raw.hands[0].flop[0] === '2c' ? '3c' : '2c';
  const { usable, excluded } = analyse(parseBundle(raw));
  eq(usable.length, 0, 'usable hands');
  eq(excluded.length, 1, 'excluded hands');
  return excluded[0].reason;
});

check('the boundary refuses a chip amount it cannot represent exactly', () => {
  const raw = clone();
  raw.hands[0].total_pot = '18446744073709551615';
  let msg = null;
  try { parseBundle(raw); } catch (e) { msg = e.message; assert(e instanceof BundleError, 'wrong error type'); }
  assert(msg && /exceeds 2\^53/.test(msg), `expected a refusal, got ${msg}`);
  return 'a nat64 above 2^53 is refused rather than rounded';
});

// ===========================================================================
console.log('\nTHE BETTING REPLAY -- built so a wrong reading fails loudly');
// ===========================================================================

check('the honest fixture\'s money adds up four independent ways', () => {
  const b = parseBundle(clone());
  const r = replayHand(b.hands[0]);
  assert(r.ok, `problems: ${r.problems}`);
  assert(r.checks.length >= 4, 'at least four checks must run');
  return r.checks.map((c) => c.name).join(', ');
});

check('reading Raise as an INCREMENT instead of a raise-TO gives the wrong contribution', () => {
  // The mistake this replay is most likely to make, made on purpose -- in the
  // READER, not in the record. Corrupting the record instead would prove nothing
  // here: the uncalled-bet rule quietly absorbs a single inflated raise, so a
  // doubled `Raise` amount still balances. The claim being pinned is that the
  // canister's `Raise(x)` means "raise TO x", and the only way to pin it is to
  // read it the other way and land somewhere else.
  const b = parseBundle(clone());
  const hand = b.hands[0];
  assert(hand.actions.some((a) => a.kind === 'Raise'), 'the fixture must contain a raise');

  const right = replayHand(hand);
  assert(right.ok, `the correct reading must check out first: ${right.problems}`);

  const wrongReading = (h) => {
    const perSeat = new Map(h.dealtIn.map((d) => [d.seat, 0]));
    const streets = new Map(h.dealtIn.map((d) => [d.seat, 0]));
    const { sbSeat, bbSeat } = right;
    perSeat.set(sbSeat, right.sbPosted); streets.set(sbSeat, right.sbPosted);
    perSeat.set(bbSeat, right.bbPosted); streets.set(bbSeat, right.bbPosted);
    let street = 'preflop';
    for (const a of h.actions) {
      if (a.phase !== street) { street = a.phase; for (const s of streets.keys()) streets.set(s, 0); }
      if (!perSeat.has(a.seat)) continue;
      // THE WRONG RULE: every amount treated as new money going in.
      const inc = ['Call', 'Bet', 'Raise', 'AllIn'].includes(a.kind) ? a.amount : 0;
      perSeat.set(a.seat, perSeat.get(a.seat) + inc);
      streets.set(a.seat, streets.get(a.seat) + inc);
    }
    return perSeat;
  };

  const wrong = wrongReading(hand);
  const differs = [...right.contributed.entries()].filter(([s, v]) => wrong.get(s) !== v);
  assert(differs.length > 0,
    'the two readings agree on this fixture, so the money checks prove nothing about which is right');
  const [seat] = differs[0];
  return `seat ${seat}: raise-TO gives ${right.contributed.get(seat)}, increment gives ${wrong.get(seat)} `
    + `(the record states ${hand.players.find((p) => p.seat === seat).contributed})`;
});

check('A POT PAID TO THE WRONG PERSON, with every total still correct, is caught', () => {
  // The failure signature this repository has produced seven waves running:
  // CORRECT TOTALS, WRONG RECIPIENTS, EVERY INVARIANT SILENT. Move the credit
  // from the player who won it to the player who did not, and leave every sum
  // untouched. sum(winners), sum(contributed) and total_pot all still balance to
  // the e8. If the money checks are only sums, this passes.
  const raw = clone();
  const winners = raw.hands[0].winners;
  const players = raw.hands[0].players;
  assert(winners.length >= 1 && players.length >= 2, 'the fixture needs a winner and a loser');
  const loser = players.find((p) => p.principal !== winners[0].principal);
  assert(loser, 'the fixture needs somebody who did not win');
  winners[0].seat = loser.seat;
  winners[0].principal = loser.principal;

  const b = parseBundle(raw);
  const sums = {
    winners: b.hands[0].winners.reduce((a, w) => a + w.amount, 0),
    contributed: b.hands[0].players.reduce((a, p) => a + (p.contributed ?? 0), 0),
    pot: b.hands[0].totalPot,
  };
  eq(sums.winners, sums.pot, 'the tampered record must still balance, or this gate proves nothing');
  eq(sums.contributed, sums.pot, 'and so must the contributions');

  const r = replayHand(b.hands[0]);
  assert(!r.ok, 'a pot paid to the wrong person MUST NOT check out');
  assert(r.problems.some((p) => /paid the same in both lists/.test(p)),
    `the problem must name the mismatch, got: ${r.problems.join(' | ')}`);
  return `every total still balances (${sums.pot} e8s three ways) and the recipient check still fires`;
});

check('an uncalled bet is returned, and without that the pot would be overstated', () => {
  // A real hand where somebody bet 0.18 into a 0.30 pot and everyone folded. The
  // 0.18 was handed straight back, so it is NOT in the pot -- and a replay that
  // counted it would report that the canister had lost money it had correctly
  // returned. This gate is the reason `returnUncalled` exists.
  const raw = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'fixtures', 'uncalled-bet.json'), 'utf8'));
  const b = parseBundle(raw);
  const r = replayHand(b.hands[0]);
  assert(r.ok, `problems: ${r.problems}`);
  assert(r.refunds.length > 0, 'this fixture must contain an uncalled bet for the gate to bind');
  const returned = r.refunds.reduce((a, x) => a + x.amount, 0);
  const gross = [...r.contributed.values()].reduce((a, c) => a + c, 0) + returned;
  assert(gross > b.hands[0].totalPot, 'the gross wagered must exceed the pot when a bet went uncalled');
  return `${r.refunds.length} refund(s), ${returned} e8s returned, gross ${gross} vs pot ${b.hands[0].totalPot}`;
});

check('a blind refunded to a player who walked out is CLASSIFIED, not called a money failure', () => {
  // A REAL hand this tool found: the big blind left before anybody acted and got
  // (big_blind - small_blind) of its own posted blind back, because leaving hands
  // back anything nobody has covered and right after the blinds the big blind is
  // always the top contributor. Every sum in the record still balances; only a
  // replay from the action list disagrees, because the action list has no record
  // of a departure.
  const raw = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'fixtures', 'departed-blind.json'), 'utf8'));
  const r = replayHand(parseBundle(raw).hands[0]);
  assert(r.ok, `the hand must not be reported as a money failure: ${r.problems}`);
  eq(r.departureRefunds.length, 1, 'exactly one departure refund');
  const [refund] = r.departureRefunds;
  eq(refund.amount, raw.hands[0].big_blind - raw.hands[0].small_blind, 'refund == big blind - small blind');
  assert(r.notes.some((n) => /handed back/.test(n)), 'and it must be said out loud');
  return `seat ${refund.seat} posted ${refund.forced} and kept ${refund.amount} of it by leaving`;
});

check('the departure allowance applies ONLY to a seat that actually left', () => {
  // The classification above is the only place this tool forgives a replay
  // disagreement, so it has to be narrow. The same shortfall on a seat that
  // stayed must still be a failure -- otherwise the allowance is a hole big
  // enough to walk a mis-split pot through, with every total still balancing.
  const raw = clone();
  const hand = raw.hands[0];
  assert(hand.players.every((p) => p.left_mid_hand !== true), 'this fixture must have nobody leaving');
  const a = hand.players[0];
  const b2 = hand.players.find((p) => p !== a && Number(p.contributed) > 0);
  const move = Number(hand.small_blind);
  a.contributed = String(Number(a.contributed) + move);
  a.ending_chips = String(Number(a.ending_chips) - move);
  b2.contributed = String(Number(b2.contributed) - move);
  b2.ending_chips = String(Number(b2.ending_chips) + move);

  const parsedB = parseBundle(raw);
  const sumContrib = parsedB.hands[0].players.reduce((x, p) => x + (p.contributed ?? 0), 0);
  eq(sumContrib, parsedB.hands[0].totalPot, 'the tampered record must still balance, or this gate proves nothing');
  const r = replayHand(parsedB.hands[0]);
  assert(!r.ok, 'a replay disagreement on a seat that did NOT leave must still be a failure');
  eq(r.departureRefunds.length, 0, 'and nothing may be classified as a departure refund');
  return `${move} e8s moved between two seated players is refused even though every sum balances`;
});

check('the departure allowance also requires the record to balance with itself', () => {
  // A departure explains a replay disagreement. It does not explain a record whose
  // own sums are wrong, and it must not be allowed to launder one.
  const raw = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'fixtures', 'departed-blind.json'), 'utf8'));
  const hand = raw.hands[0];
  const winner = hand.winners[0];
  const paid = hand.players.find((p) => p.principal === winner.principal && p.seat === winner.seat);
  paid.amount_won = String(Number(paid.amount_won) + 1);      // winners list and player list now differ by 1 e8
  const r = replayHand(parseBundle(raw).hands[0]);
  assert(!r.ok, 'a record that disagrees with itself must fail even when somebody left');
  return r.problems[0];
});

// ===========================================================================
console.log('\nCOLLUSION SIGNALS -- populations whose truth is set by construction');
// ===========================================================================

check('co-occurrence reports NO POWER on a population where everyone always plays together', () => {
  const pop = syntheticPopulation({ players: 3, tables: 1, hands: 400, seed: 5 });
  const co = coOccurrence(pop);
  assert(co.structural.length > 0, 'the structural refusal must fire');
  assert(co.rows.every((r) => r.verdict === 'NO POWER'), 'no pair may be given a verdict');
  return co.structural[0];
});

check('co-occurrence stays silent below its own sample floor', () => {
  const pop = syntheticPopulation({ players: 12, tables: 4, hands: 60, seed: 6 });
  const co = coOccurrence(pop);
  assert(co.rows.every((r) => r.p === null), 'no p-value may be computed below the floor');
  assert(co.rows.every((r) => r.verdict !== 'REVIEW'), 'nothing may reach REVIEW');
  return `${co.rows.length} pairs, all withheld`;
});

check('co-occurrence FIRES on a pair that always sits together in a large pool', () => {
  const pop = syntheticPopulation({ players: 14, tables: 6, hands: 4000, seed: 7, gluedPair: true });
  const co = coOccurrence(pop);
  const glued = co.rows.find((r) => [r.a, r.b].sort().join('|') === ['P00', 'P01'].join('|'));
  assert(glued, 'the glued pair must be in the output');
  eq(glued.verdict, 'REVIEW', `glued pair verdict (q=${glued.q})`);
  return `glued pair together ${glued.both} times, expected ${glued.expected.toFixed(1)}, q=${glued.q.toExponential(2)}`;
});

check('co-occurrence does NOT fire on the same pool with nobody glued', () => {
  let falsePositives = 0;
  const trials = 12;
  for (let t = 0; t < trials; t += 1) {
    const pop = syntheticPopulation({ players: 14, tables: 6, hands: 4000, seed: 100 + t, gluedPair: false });
    const co = coOccurrence(pop);
    if (co.rows.some((r) => r.verdict === 'REVIEW')) falsePositives += 1;
  }
  assert(falsePositives <= 1, `${falsePositives} of ${trials} honest populations produced a REVIEW`);
  return `${falsePositives} of ${trials} honest populations flagged (BH keeps this near zero)`;
});

check('directed chip flow FIRES on a scripted dumper and NOT on the honest pair beside it', () => {
  const pop = syntheticPopulation({ players: 8, tables: 2, hands: 6000, seed: 21, dumpBBPerHand: 1.2 });
  const dt = directedTransfer(pop, { iters: 4000, seed: 3 });
  const dumper = dt.find((r) => r.from === 'P00' && r.to === 'P01');
  assert(dumper, 'the dumping pair must be tested');
  eq(dumper.verdict, 'REVIEW', `dumper verdict (q=${dumper.q}, excess=${dumper.excessBBPer100})`);
  const others = dt.filter((r) => !(r.from === 'P00' && r.to === 'P01') && r.verdict === 'REVIEW');
  assert(others.length <= 1, `honest pairs also flagged: ${others.map((r) => `${r.from}->${r.to}`).join(', ')}`);
  return `dumper +${dumper.excessBBPer100.toFixed(1)} bb/100 over own baseline, q=${dumper.q.toExponential(2)}; `
    + `${others.length} other pair(s) flagged`;
});

check('a single-opponent baseline is labelled as contaminable, not reported as clean', () => {
  // Found on real ClearDeck hands: at a three-handed table the beneficiary of a
  // dump ALSO reaches REVIEW against the innocent third player, because their own
  // baseline is the dump. Three players is the smallest pool where that happens
  // and it is the pool a lot of real tables run at.
  const pop = syntheticPopulation({ players: 3, tables: 1, hands: 3000, seed: 41, tableSize: 3, dumpBBPerHand: 1.5 });
  const dt = directedTransfer(pop, { iters: 2000, seed: 3 });
  const reviews = dt.filter((r) => r.verdict === 'REVIEW');
  assert(reviews.length > 0, 'the dump must be visible at all for this gate to mean anything');
  assert(dt.every((r) => r.baselineOpponents !== 1 || r.caveat),
    'every single-opponent baseline must carry the contamination caveat');
  const spurious = reviews.find((r) => !(r.from === 'P00' && r.to === 'P01'));
  return spurious
    ? `${reviews.length} REVIEW rows including the spurious ${spurious.from}->${spurious.to}; all carry the caveat`
    : `${reviews.length} REVIEW row(s), all carrying the caveat`;
});

check('directed chip flow refuses below its sample floor instead of guessing', () => {
  const pop = syntheticPopulation({ players: 8, tables: 2, hands: 120, seed: 31, dumpBBPerHand: 3 });
  const dt = directedTransfer(pop, { iters: 2000, seed: 3 });
  assert(dt.every((r) => r.p === null), 'no p-value may be produced below the floor');
  assert(dt.every((r) => r.verdict === 'INSUFFICIENT DATA'), 'every verdict must be INSUFFICIENT DATA');
  assert(dt.every((r) => r.why.length > 0), 'and each must say what was missing');
  return `${dt.length} ordered pairs, all withheld: "${dt[0].why[0]}"`;
});

check('the false-positive rate on honest play is at or below the stated alpha', () => {
  let flagged = 0;
  const trials = 25;
  for (let t = 0; t < trials; t += 1) {
    const pop = syntheticPopulation({ players: 8, tables: 2, hands: 3000, seed: 500 + t, dumpBBPerHand: 0 });
    const dt = directedTransfer(pop, { iters: 2000, seed: 9 });
    if (dt.some((r) => r.verdict === 'REVIEW')) flagged += 1;
  }
  assert(flagged / trials <= 0.12, `${flagged}/${trials} honest populations flagged, above the tolerance`);
  return `${flagged} of ${trials} honest populations produced any REVIEW (alpha ${GATES.ALPHA}, BH-corrected)`;
});

// ===========================================================================
console.log('\nTHE STATISTICS THEMSELVES');
// ===========================================================================

check('Wilson intervals bracket the point estimate and narrow with n', () => {
  const small = wilson(4, 10);
  const large = wilson(400, 1000);
  assert(small.lo < small.p && small.p < small.hi, 'the interval must contain the estimate');
  assert((large.hi - large.lo) < (small.hi - small.lo) / 5, 'more data must mean a tighter interval');
  eq(wilson(0, 0).p, null, 'no data means no estimate');
  return `n=10 gives [${(small.lo * 100).toFixed(0)}, ${(small.hi * 100).toFixed(0)}]%, n=1000 gives [${(large.lo * 100).toFixed(1)}, ${(large.hi * 100).toFixed(1)}]%`;
});

check('Benjamini-Hochberg is monotone and never reports below the raw p-value', () => {
  const ps = [0.001, 0.008, 0.02, 0.04, 0.2, 0.9];
  const q = benjaminiHochberg(ps);
  for (let i = 0; i < ps.length; i += 1) assert(q[i] >= ps[i] - 1e-12, `q[${i}] < p[${i}]`);
  for (let i = 1; i < q.length; i += 1) assert(q[i] >= q[i - 1] - 1e-12, 'q must be monotone in p');
  return q.map((x) => x.toFixed(3)).join(', ');
});

check('Fisher\'s exact right tail matches a hand-computable case', () => {
  // 2x2 with margins small enough to enumerate by hand:
  // 10 hands, A in 5, B in 5, together in all 5 -> p = C(5,5)C(5,0)/C(10,5) = 1/252.
  const p = fisherRightTail(5, 5, 5, 10);
  assert(Math.abs(p - 1 / 252) < 1e-12, `got ${p}, want ${1 / 252}`);
  return `p = 1/252 = ${p.toExponential(3)}`;
});

// ===========================================================================
console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length > 0) {
  console.log('\nFAILURES');
  for (const f of failures) console.log(`  ${f.name}\n    ${f.message}`);
  process.exit(1);
}
