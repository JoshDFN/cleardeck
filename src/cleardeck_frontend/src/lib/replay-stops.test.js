import { describe, it, expect } from 'vitest';
import { blindsForHand, logGroupsFor, normalizeAction } from './hand-record.js';
import {
  ACTION_BEAT_MS, REVEAL_BEAT_MS, buildStops, equityAllowedAt, liveSeatsAt, seatSpots,
  stopBeatMs, streetOfStop, streetStops,
} from './replay-stops.js';

const e8 = (icp) => Math.round(icp * 1e8);
const act = (seat, action, phase, amount = 0) => normalizeAction({
  seat, action: { [action]: null }, phase, amount, timestamp: 0n,
});

/** The hand the audit photographed: heads-up, bet and call on every street, 1.44 paid. */
function streetByStreetHand() {
  const community = new Array(5).fill({ rank: { Two: null }, suit: { Clubs: null } });
  return {
    handNumber: 2,
    community,
    actions: [
      act(0, 'Call', 'preflop', e8(0.01)),
      act(1, 'Check', 'preflop'),
      act(1, 'Bet', 'flop', e8(0.10)),
      act(0, 'Call', 'flop', e8(0.10)),
      act(1, 'Bet', 'turn', e8(0.20)),
      act(0, 'Call', 'turn', e8(0.20)),
      act(1, 'Bet', 'river', e8(0.40)),
      act(0, 'Call', 'river', e8(0.40)),
    ],
    showdown: [{ seat: 0, cards: [{}, {}], won: e8(1.44) }, { seat: 1, cards: [{}, {}], won: 0 }],
    winners: [{ seat: 0, amount: e8(1.44) }],
    seats: [0, 1],
    potTotal: e8(1.44),
    blinds: null,
  };
}

const facts = (handNumber, seatsKnown = true) => ({
  config: { smallBlind: e8(0.01), bigBlind: e8(0.02), ante: 0 },
  blindSeats: { sb: 0, bb: 1, dealer: 0 },
  liveHandNumber: seatsKnown ? handNumber : handNumber + 1,
  settled: true,
});

function replay(hand, tableFacts) {
  const blinds = blindsForHand({ hand, tableFacts });
  const logGroups = logGroupsFor({ hand, blinds });
  return { blinds, logGroups, stops: buildStops({ hand, blinds, logGroups }) };
}

describe('buildStops', () => {
  it('plays the deal, both blinds, every action, every reveal and the showdown in order', () => {
    const { stops } = replay(streetByStreetHand(), facts(2));
    expect(stops.map((s) => s.kind)).toEqual([
      'deal', 'blind', 'blind', 'action', 'action',
      'reveal', 'action', 'action',
      'reveal', 'action', 'action',
      'reveal', 'action', 'action',
      'showdown',
    ]);
    expect(stops.map((s) => s.board)).toEqual([0, 0, 0, 0, 0, 3, 3, 3, 4, 4, 4, 5, 5, 5, 5]);
    expect(stops.at(-1).reveal).toBe(true);
  });

  it('carries the log line each stop plays, in the log\'s own numbering', () => {
    const { stops, logGroups } = replay(streetByStreetHand(), facts(2));
    const lineNumbers = stops.map((s) => s.line).filter((l) => l !== null);
    const logLines = logGroups.flatMap((g) => g.lines.map((l) => l.index));
    expect(lineNumbers).toEqual(logLines);
    expect(lineNumbers).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  });

  it('folds the pot exactly: the blinds sit in front of the seats, sweep at each reveal', () => {
    const { stops } = replay(streetByStreetHand(), facts(2));
    const deal = stops[0];
    expect(deal.pot).toBe(0);
    expect(deal.bets).toEqual({ 0: e8(0.01), 1: e8(0.02) });
    const afterCall = stops[3];
    expect(afterCall.bets).toEqual({ 0: e8(0.02), 1: e8(0.02) });
    const flop = stops[5];
    expect(flop.kind).toBe('reveal');
    expect(flop.pot).toBe(e8(0.04));
    expect(flop.bets).toEqual({});
    const river = stops[11];
    expect(river.pot).toBe(e8(0.64));
    const showdown = stops.at(-1);
    expect(showdown.pot).toBe(e8(1.44));
    expect(showdown.paid).toBe(true);
    expect(stops.every((s) => s.exact)).toBe(true);
  });

  it('puts an unattributed blind level straight into the pot and stays exact for increments', () => {
    const { stops } = replay(streetByStreetHand(), facts(2, false));
    expect(stops[0].pot).toBe(e8(0.03));
    expect(stops[0].bets).toEqual({});
    expect(stops[5].pot).toBe(e8(0.04));
    expect(stops.every((s) => s.exact)).toBe(true);
  });

  it('withholds the pot after a pre-flop raise when the blind seats are unknown, and pays the final stop', () => {
    const hand = streetByStreetHand();
    hand.actions = [act(0, 'Raise', 'preflop', e8(0.06)), act(1, 'Call', 'preflop', e8(0.04))];
    hand.community = [];
    const { stops } = replay(hand, facts(2, false));
    const raise = stops.find((s) => s.verb === 'raises to');
    expect(raise.exact).toBe(false);
    expect(raise.pot).toBeNull();
    expect(stops.at(-1).pot).toBe(e8(1.44));
    expect(stops.at(-1).paid).toBe(true);
  });

  it('treats a raise as the street total when the blind seats are known', () => {
    const hand = streetByStreetHand();
    hand.actions = [act(0, 'Raise', 'preflop', e8(0.06)), act(1, 'Call', 'preflop', e8(0.04))];
    hand.community = [];
    const { stops } = replay(hand, facts(2));
    const raise = stops.find((s) => s.verb === 'raises to');
    expect(raise.exact).toBe(true);
    expect(raise.bets).toEqual({ 0: e8(0.06), 1: e8(0.02) });
    const call = stops.find((s) => s.verb === 'calls');
    expect(call.bets).toEqual({ 0: e8(0.06), 1: e8(0.06) });
  });

  it('records folds and all-ins per seat', () => {
    const hand = streetByStreetHand();
    hand.actions = [act(0, 'AllIn', 'preflop', e8(6)), act(1, 'Fold', 'preflop')];
    hand.community = [];
    hand.showdown = [];
    const { stops } = replay(hand, facts(2));
    expect(stops.find((s) => s.kind === 'action' && s.seat === 0).allIn).toEqual([0]);
    expect(stops.at(-1).folded).toEqual([1]);
    expect(stops.at(-1).kind).toBe('end');
    expect(stops.at(-1).street).toBe('Won without showdown');
  });

  it('reveals the whole board even when nobody acted after the all-in', () => {
    const hand = streetByStreetHand();
    hand.actions = [act(0, 'AllIn', 'preflop', e8(6)), act(1, 'Call', 'preflop', e8(5.98))];
    const { stops } = replay(hand, facts(2));
    expect(stops.filter((s) => s.kind === 'reveal').map((s) => s.board)).toEqual([3, 4, 5]);
  });

  it('is not exact without a blind level', () => {
    const { stops } = replay(streetByStreetHand(), null);
    expect(stops[0].pot).toBeNull();
    expect(stops.at(-1).pot).toBe(e8(1.44));
  });
});

describe('streetStops / streetOfStop', () => {
  it('names the five stops the scrubber shows for a hand that reached a showdown', () => {
    const { stops } = replay(streetByStreetHand(), facts(2));
    expect(streetStops(stops).map((s) => s.label)).toEqual(['Pre-flop', 'Flop', 'Turn', 'River', 'Showdown']);
    expect(streetStops(stops).map((s) => s.index)).toEqual([0, 5, 8, 11, 14]);
  });
  it('maps an action stop to the street it is on', () => {
    const { stops } = replay(streetByStreetHand(), facts(2));
    expect(streetOfStop(stops, 7).label).toBe('Flop');
    expect(streetOfStop(stops, 0).label).toBe('Pre-flop');
    expect(streetOfStop(stops, 14).label).toBe('Showdown');
  });
});

describe('stopBeatMs', () => {
  it('gives an action one beat and a reveal two', () => {
    expect(stopBeatMs({ kind: 'action' })).toBe(ACTION_BEAT_MS);
    expect(stopBeatMs({ kind: 'blind' })).toBe(ACTION_BEAT_MS);
    expect(stopBeatMs({ kind: 'reveal' })).toBe(REVEAL_BEAT_MS);
    expect(stopBeatMs({ kind: 'showdown' })).toBe(REVEAL_BEAT_MS);
  });
});

describe('equityAllowedAt', () => {
  it('allows equity only when every live seat showed cards', () => {
    const stop = { folded: [] };
    expect(equityAllowedAt(stop, [0, 1], [0, 1])).toBe(true);
    expect(equityAllowedAt(stop, [0, 1, 2], [0, 1])).toBe(false);
    expect(equityAllowedAt({ folded: [2] }, [0, 1, 2], [0, 1])).toBe(true);
    expect(liveSeatsAt({ folded: [2] }, [0, 1, 2])).toEqual([0, 1]);
  });
  it('needs two live hands', () => {
    expect(equityAllowedAt({ folded: [1] }, [0, 1], [0, 1])).toBe(false);
  });
});

describe('seatSpots', () => {
  it('puts the hero at the bottom and a heads-up opponent at the top', () => {
    const spots = seatSpots({ seats: [0, 1], heroSeat: 0, maxPlayers: 2 });
    const hero = spots.find((s) => s.seat === 0);
    const opp = spots.find((s) => s.seat === 1);
    expect(hero.x).toBeCloseTo(0.5, 5);
    expect(hero.y).toBeGreaterThan(0.9);
    expect(hero.side).toBe('bottom');
    expect(opp.x).toBeCloseTo(0.5, 5);
    expect(opp.y).toBeLessThan(0.1);
    expect(opp.side).toBe('top');
  });
  it('anchors on the lowest seat when the viewer was not in the hand', () => {
    const spots = seatSpots({ seats: [3, 5], heroSeat: null, maxPlayers: 6 });
    expect(spots.find((s) => s.seat === 3).side).toBe('bottom');
  });
  it('spreads a six-max ring left and right', () => {
    const spots = seatSpots({ seats: [0, 1, 2, 3, 4, 5], heroSeat: 0, maxPlayers: 6 });
    expect(spots.filter((s) => s.side === 'left').length).toBeGreaterThan(0);
    expect(spots.filter((s) => s.side === 'right').length).toBeGreaterThan(0);
    for (const s of spots) {
      expect(s.x).toBeGreaterThanOrEqual(0);
      expect(s.x).toBeLessThanOrEqual(1);
    }
  });
});
