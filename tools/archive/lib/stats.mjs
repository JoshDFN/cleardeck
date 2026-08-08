// Per-player statistics, the ones a real player actually looks at.
//
// Every rate carries the count it came from and a Wilson interval, and the
// formatter refuses to characterise a player below a stated sample. "VPIP 40%"
// from 10 hands and from 10,000 hands are not the same claim; a bare percentage
// cannot tell them apart, and this tool's whole posture is that a number nobody
// can check is worse than no number.
//
// A hand only reaches this module if its cards reconstructed from the seed AND
// its money added up. Anything else is counted as excluded, by reason, and the
// exclusions are printed.

import { wilson } from './statistics.mjs';

/** Seat labels, derived rather than trusted, then cross-checked against the record. */
export function positionLabel(replay, seat, dealerSeat) {
  const seats = [...replay.dealtSeats].sort((a, b) => a - b);
  const P = seats.length;
  if (P === 2) return seat === replay.sbSeat ? 'BTN/SB' : 'BB';
  if (seat === dealerSeat) return 'BTN';
  if (seat === replay.sbSeat) return 'SB';
  if (seat === replay.bbSeat) return 'BB';
  // Everyone else, in order of action after the big blind.
  const order = [];
  let cur = replay.bbSeat;
  for (let i = 0; i < P; i += 1) {
    const later = seats.find((s) => s > cur);
    cur = later !== undefined ? later : seats[0];
    if (cur === replay.bbSeat) break;
    if (cur !== dealerSeat) order.push(cur);
  }
  const idx = order.indexOf(seat);
  if (idx < 0) return `seat ${seat}`;
  if (idx === order.length - 1) return 'CO';
  const names = ['UTG', 'UTG+1', 'MP', 'MP+1', 'HJ'];
  return names[idx] ?? `seat ${seat}`;
}

const blank = () => ({
  hands: 0, vpip: 0, pfr: 0, threeBet: 0, threeBetOpp: 0,
  sawFlop: 0, wonWhenSawFlop: 0, showdowns: 0, showdownsWon: 0,
  postflopBets: 0, postflopRaises: 0, postflopCalls: 0, postflopChecks: 0, postflopFolds: 0,
  netChips: 0, bbSum: 0, contributed: 0, won: 0,
  byPosition: new Map(),
});

function positionBucket(acc, label) {
  if (!acc.byPosition.has(label)) {
    acc.byPosition.set(label, { hands: 0, vpip: 0, pfr: 0, netChips: 0, bbSum: 0 });
  }
  return acc.byPosition.get(label);
}

/**
 * @param {Array<{hand:object, replay:object, recon:object}>} usable
 */
export function playerStats(usable) {
  const acc = new Map();
  const get = (principal) => {
    if (!acc.has(principal)) acc.set(principal, blank());
    return acc.get(principal);
  };

  for (const { hand, replay } of usable) {
    const bb = hand.bigBlind || 1;
    const perSeat = new Map();
    for (const seat of replay.dealtSeats) {
      perSeat.set(seat, {
        principal: replay.principalOfSeat.get(seat),
        vpip: false, pfr: false, threeBet: false, threeBetOpp: false,
        foldedPreflop: false, folded: false,
      });
    }

    let preflopRaises = 0;
    for (const d of replay.decisions) {
      const s = perSeat.get(d.seat);
      if (!s) continue;
      const a = get(s.principal);
      if (d.street === 'preflop') {
        // The blinds are not actions and are not voluntary, so they are not VPIP.
        if (['Call', 'Bet', 'Raise', 'AllIn'].includes(d.kind)) s.vpip = true;
        const currentBet = d.streetTotalBefore + d.toCall;
        const isRaise = d.kind === 'Raise' || d.kind === 'Bet'
          || (d.kind === 'AllIn' && d.recordedAmount > currentBet);
        if (isRaise) {
          if (preflopRaises >= 1) s.threeBet = true;
          s.pfr = true;
          preflopRaises += 1;
        } else if (preflopRaises >= 1 && ['Call', 'Fold'].includes(d.kind)) {
          s.threeBetOpp = true;
        }
      } else {
        switch (d.kind) {
          case 'Bet': a.postflopBets += 1; break;
          case 'Raise': a.postflopRaises += 1; break;
          case 'AllIn': {
            const currentBet = d.streetTotalBefore + d.toCall;
            if (d.recordedAmount > currentBet) a.postflopRaises += 1; else a.postflopCalls += 1;
            break;
          }
          case 'Call': a.postflopCalls += 1; break;
          case 'Check': a.postflopChecks += 1; break;
          case 'Fold': a.postflopFolds += 1; break;
          default: break;
        }
      }
      if (d.kind === 'Fold') {
        s.folded = true;
        if (d.street === 'preflop') s.foldedPreflop = true;
      }
    }

    for (const [seat, s] of perSeat) {
      const a = get(s.principal);
      const rec = hand.players.find((p) => p.seat === seat && p.principal === s.principal);
      const contributed = rec?.contributed ?? replay.contributed.get(seat) ?? 0;
      const won = rec?.amountWon ?? 0;
      const net = won - contributed;

      a.hands += 1;
      if (s.vpip) a.vpip += 1;
      if (s.pfr) a.pfr += 1;
      if (s.threeBet) a.threeBet += 1;
      if (s.threeBet || s.threeBetOpp) a.threeBetOpp += 1;
      // SAW A FLOP means the flop was dealt and this player was still in it --
      // NOT "this player acted after the flop". Somebody all-in pre-flop takes no
      // post-flop action and still sees every card, and counting the denominator
      // from actions would quietly drop exactly the hands where the most money
      // was at stake.
      const sawFlop = hand.flop !== null && !s.foldedPreflop;
      if (sawFlop) { a.sawFlop += 1; if (net > 0) a.wonWhenSawFlop += 1; }
      // A showdown for THIS player means they were still in when cards were shown.
      const reachedShowdown = hand.wentToShowdown && !s.folded && !(rec?.leftMidHand === true);
      if (reachedShowdown) { a.showdowns += 1; if (won > 0) a.showdownsWon += 1; }
      a.netChips += net;
      a.bbSum += net / bb;
      a.contributed += contributed;
      a.won += won;

      const label = positionLabel(replay, seat, hand.dealerSeat);
      const bucket = positionBucket(a, label);
      bucket.hands += 1;
      if (s.vpip) bucket.vpip += 1;
      if (s.pfr) bucket.pfr += 1;
      bucket.netChips += net;
      bucket.bbSum += net / bb;
    }
  }

  const out = [];
  for (const [principal, a] of acc) {
    const aggressiveActions = a.postflopBets + a.postflopRaises;
    out.push({
      principal,
      hands: a.hands,
      vpip: wilson(a.vpip, a.hands),
      pfr: wilson(a.pfr, a.hands),
      threeBet: wilson(a.threeBet, a.threeBetOpp),
      wtsd: wilson(a.showdowns, a.sawFlop),
      wsd: wilson(a.showdownsWon, a.showdowns),
      wwsf: wilson(a.wonWhenSawFlop, a.sawFlop),
      aggressionFactor: a.postflopCalls > 0 ? aggressiveActions / a.postflopCalls : null,
      // The industry definition: checks are NOT in the denominator, because a
      // check is not a decision to play passively when there was nothing to call.
      aggressionFrequency: wilson(
        aggressiveActions,
        aggressiveActions + a.postflopCalls + a.postflopFolds,
      ),
      netChips: a.netChips,
      bbPer100: a.hands > 0 ? (a.bbSum / a.hands) * 100 : null,
      contributed: a.contributed,
      won: a.won,
      byPosition: [...a.byPosition.entries()]
        .map(([label, b]) => ({
          label, hands: b.hands, vpip: wilson(b.vpip, b.hands), pfr: wilson(b.pfr, b.hands),
          bbPer100: b.hands > 0 ? (b.bbSum / b.hands) * 100 : null,
        }))
        .sort((x, y) => y.hands - x.hands),
    });
  }
  return out.sort((x, y) => y.hands - x.hands);
}
