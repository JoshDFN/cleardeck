// EV, with the cards face up.
//
// This is the thing a commercial room cannot offer, and the reason is structural
// rather than commercial: a room can only show you a hand you were in, and it can
// never show you the cards of somebody who folded. ClearDeck's seed reveals the
// whole deck, so three otherwise impossible things are computable here:
//
//   1. THE RUN-OUT IS KNOWN EVEN FOR HANDS THAT ENDED EARLY. The board comes off
//      the shuffled deck at fixed offsets (SHUFFLE-SPEC section 4) and the
//      offsets depend only on P, which is fixed when the cards are dealt. Betting
//      cannot move them. So the turn and river of a hand that ended pre-flop are
//      not a guess: they are deck[2P+5] and deck[2P+7].
//   2. THE OPPONENT'S HAND IS KNOWN even when they never showed it.
//   3. Therefore a FOLD has an exact price whenever the alternative closes the
//      action -- no model, no assumption, just the cards.
//
// What is NOT computed, and is deliberately left blank rather than modelled:
// the price of a fold when calling would have left more decisions to make. That
// number depends on what everyone would have done next, which is not in the
// record and is not knowable. A tool that prints one is printing an opinion.

import { equity } from './equity.mjs';
import { score } from './evaluator.mjs';
import { buildPots, awardPots } from './pots.mjs';
import { boardForStreet } from './betting.mjs';

const STREETS = ['preflop', 'flop', 'turn', 'river'];

/** The five board cards the deck was always going to produce for this hand. */
export function deckBoard(recon) {
  const l = recon.layout;
  return [...l.flop, l.turn, l.river];
}

/**
 * Was there any voluntary betting left after `seat` folds?
 *
 * A counterfactual is only priced when the answer is a certain NO. Three shapes
 * qualify, and each is checked against the record rather than assumed:
 *
 *   - the board was complete, this fold ended the hand, and everyone still in had
 *     already matched the bet: calling goes straight to a showdown;
 *   - every remaining player was already all-in and this fold ended the hand: the
 *     board simply runs out;
 *   - one opponent, and calling would have put the folder all-in: nobody can bet
 *     into a heads-up pot where both players are committed.
 *
 * `endsTheHand` is read off the record (is this the last action?), not inferred.
 * If somebody still had a decision to make, this returns null and the fold goes
 * unpriced -- the price would depend on what they would have done, which is not
 * in the record and is not knowable.
 */
function closedShape(decision, lastActionIndex) {
  const liveAfter = decision.liveSeatsBefore.filter((s) => s !== decision.seat);
  if (liveAfter.length === 0) return null;

  const streetTotals = decision.streetTotalsBefore ?? {};
  const currentBet = decision.streetTotalBefore + decision.toCall;
  const everyoneMatched = liveAfter.every(
    (s) => (streetTotals[s] ?? 0) >= currentBet || decision.allInBefore.includes(s),
  );
  const endsTheHand = decision.index === lastActionIndex;

  if (liveAfter.length === 1 && decision.toCall >= decision.stackBefore && endsTheHand) {
    return { opponents: liveAfter, why: 'calling would have put this player all-in, heads-up' };
  }
  if (!endsTheHand || !everyoneMatched) return null;
  if (decision.street === 'river') {
    return {
      opponents: liveAfter,
      why: liveAfter.length === 1
        ? 'the board was complete and the fold ended the hand'
        : `the board was complete, the fold ended the hand and all ${liveAfter.length} remaining players had matched`,
    };
  }
  if (liveAfter.every((s) => decision.allInBefore.includes(s))) {
    return { opponents: liveAfter, why: 'every remaining player was already all-in, so the board just runs out' };
  }
  return null;
}

/**
 * What a fold cost or saved, exactly, when the counterfactual is closed.
 *
 * The alternative branch is: call `min(toCall, stack)`, run the board out from
 * the deck the seed already fixed, and settle. Chips are conserved: the returned
 * `delta` is (what calling would have netted) minus (what folding netted), and
 * folding nets zero more by definition.
 */
function priceOfFold(hand, recon, replay, decision, holeBySeat, lastActionIndex) {
  const shape = closedShape(decision, lastActionIndex);
  if (!shape) return null;
  if (decision.toCall <= 0) return null;

  const callAmount = Math.min(decision.toCall, decision.stackBefore);
  const contributed = new Map(Object.entries(decision.contributedBefore).map(([k, v]) => [Number(k), v]));
  contributed.set(decision.seat, (contributed.get(decision.seat) ?? 0) + callAmount);

  // The bettor gets back anything the call could not cover.
  const entries = [...contributed.entries()].filter(([, v]) => v > 0).sort((a, b) => b[1] - a[1]);
  if (entries.length > 1 && entries[0][1] > entries[1][1]) {
    contributed.set(entries[0][0], entries[1][1]);
  }

  const board = deckBoard(recon);
  const live = [decision.seat, ...shape.opponents];
  const scores = new Map();
  for (const s of live) {
    const hole = holeBySeat.get(s);
    if (!hole) return null;
    scores.set(s, score([...hole, ...board]));
  }
  const pots = buildPots(contributed, live);
  const won = awardPots(pots, scores);
  const wouldWin = won.get(decision.seat) ?? 0;

  return {
    callAmount,
    wouldWin,
    delta: wouldWin - callAmount,
    board,
    boardWasDealt: boardForStreet(hand, 'river').length === 5,
    opponents: shape.opponents.map((s) => ({ seat: s, hole: holeBySeat.get(s) })),
    why: shape.why,
    hero: { seat: decision.seat, hole: holeBySeat.get(decision.seat) },
    heroScore: scores.get(decision.seat),
    bestScore: Math.max(...live.map((s) => scores.get(s))),
  };
}

/**
 * Analyses one hand's EV. Returns `{usable:false}` and a reason rather than a
 * number when the hand's cards or money did not check out.
 *
 * @param {object} hand validated record
 * @param {object} recon reconstruction from lib/reconstruct.mjs
 * @param {object} replay from lib/betting.mjs
 * @param {{samples?:number, hindsight?:boolean}} [opts]
 */
export function analyseHandEV(hand, recon, replay, { samples = 100_000, hindsight = true } = {}) {
  const reasons = [];
  if (recon.status !== 'VERIFIED') reasons.push(`reconstruction status ${recon.status}`);
  if (!replay.ok) reasons.push('the money in this record does not check out');
  if (replay.departed.size > 0) {
    // Leaving is a fold that leaves no trace in the action list, so WHEN it
    // happened is not recoverable. Every counterfactual downstream would be built
    // on a hand containing a player who was no longer in it.
    reasons.push(
      `seat(s) ${[...replay.departed].join(', ')} left this hand while it was live, and the record `
      + 'does not say when, so nothing here can be priced exactly',
    );
  }
  if (reasons.length > 0) return { handId: hand.handId, usable: false, reasons };

  const holeBySeat = new Map(recon.holeCardsByDealIndex.map((d) => [d.seat, d.cards]));

  // --- what the last betting left behind -----------------------------------
  const lastStreet = replay.decisions.length > 0
    ? replay.decisions[replay.decisions.length - 1].street : 'preflop';
  const boardAtBettingEnd = boardForStreet(hand, lastStreet);
  const liveAtEnd = replay.dealtSeats.filter((s) => !replay.folded.has(s));

  // --- all-in EV: what the pot was worth before the cards decided it -------
  let allIn = null;
  if (liveAtEnd.length >= 2 && boardAtBettingEnd.length < 5 && hand.wentToShowdown) {
    const pots = buildPots(replay.contributed, liveAtEnd);
    const evBySeat = new Map(liveAtEnd.map((s) => [s, 0]));
    let ok = true;
    const layers = [];
    for (const pot of pots) {
      const hands = pot.eligible.map((s) => holeBySeat.get(s));
      if (pot.eligible.length < 2 || hands.some((h) => !h)) {
        // A layer only one player can win is theirs outright.
        if (pot.eligible.length === 1) {
          evBySeat.set(pot.eligible[0], evBySeat.get(pot.eligible[0]) + pot.amount);
          layers.push({ amount: pot.amount, eligible: pot.eligible, equity: [1], method: 'uncontested' });
          continue;
        }
        ok = false; break;
      }
      const e = equity(hands, boardAtBettingEnd, { samples });
      pot.eligible.forEach((s, i) => evBySeat.set(s, evBySeat.get(s) + pot.amount * e.equity[i]));
      layers.push({ amount: pot.amount, eligible: pot.eligible, equity: e.equity, method: e.method, stderr: e.stderr });
    }
    if (ok) {
      allIn = {
        boardAtBettingEnd,
        street: lastStreet,
        layers,
        perSeat: liveAtEnd.map((s) => {
          const rec = hand.players.find((p) => p.seat === s && p.principal === replay.principalOfSeat.get(s));
          const actual = rec ? rec.amountWon : 0;
          const ev = evBySeat.get(s);
          return { seat: s, principal: replay.principalOfSeat.get(s), evShare: ev, actualWon: actual, luck: actual - ev };
        }),
      };
    }
  }

  // --- per decision ---------------------------------------------------------
  const decisions = [];
  for (const d of replay.decisions) {
    const live = d.liveSeatsBefore;
    const entry = {
      index: d.index, seat: d.seat, principal: d.principal, street: d.street, kind: d.kind,
      increment: d.increment, toCall: d.toCall, potBefore: d.potBefore,
      hole: holeBySeat.get(d.seat) ?? null,
      board: d.board,
      hindsightEquity: null,
      equityMethod: null,
      foldPrice: null,
    };
    if (hindsight && live.length >= 2 && live.every((s) => holeBySeat.get(s))) {
      const hands = live.map((s) => holeBySeat.get(s));
      const e = equity(hands, d.board, { samples });
      entry.hindsightEquity = e.equity[live.indexOf(d.seat)];
      entry.equityMethod = e.method;
      entry.equityStderr = e.stderr;
    }
    if (d.kind === 'Fold') entry.foldPrice = priceOfFold(hand, recon, replay, d, holeBySeat, hand.actions.length - 1);
    decisions.push(entry);
  }

  return {
    handId: hand.handId,
    handNumber: hand.handNumber,
    tableId: hand.tableId,
    usable: true,
    reasons: [],
    bigBlind: hand.bigBlind,
    allIn,
    decisions,
    deckBoard: deckBoard(recon),
    boardDealt: [...(hand.flop ?? []), ...(hand.turn ? [hand.turn] : []), ...(hand.river ? [hand.river] : [])],
  };
}

export { STREETS };
