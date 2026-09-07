import { describe, it, expect } from 'vitest';
import { blindsForHand, logGroupsFor, normalizeAction } from './hand-record.js';
import { handToText } from './hand-text-export.js';

const e8 = (icp) => Math.round(icp * 1e8);
const money = (n) => (n / 1e8).toFixed(2);
const card = (rank, suit) => ({ rank: { [rank]: null }, suit: { [suit]: null } });
const act = (seat, action, phase, amount = 0) => normalizeAction({
  seat, action: { [action]: null }, phase, amount, timestamp: 0n,
});

const hand = {
  handNumber: 2,
  community: [card('Seven', 'Clubs'), card('Four', 'Hearts'), card('Jack', 'Hearts'), card('Four', 'Spades'), card('Five', 'Spades')],
  actions: [
    act(0, 'Call', 'preflop', e8(0.01)), act(1, 'Check', 'preflop'),
    act(1, 'Bet', 'flop', e8(0.10)), act(0, 'Call', 'flop', e8(0.10)),
    act(1, 'Bet', 'turn', e8(0.20)), act(0, 'Call', 'turn', e8(0.20)),
    act(1, 'Bet', 'river', e8(0.40)), act(0, 'Call', 'river', e8(0.40)),
  ],
  showdown: [
    { seat: 0, cards: [card('Nine', 'Hearts'), card('Seven', 'Spades')], rank: { TwoPair: null }, won: e8(1.44) },
    { seat: 1, cards: [card('Two', 'Clubs'), card('Three', 'Diamonds')], rank: { Pair: null }, won: 0 },
  ],
  winners: [{ seat: 0, amount: e8(1.44) }],
  seats: [0, 1],
  potTotal: e8(1.44),
  blinds: null,
  proof: { seedHash: 'ab'.repeat(32), revealedSeed: 'cd'.repeat(32) },
};
const tableFacts = {
  config: { smallBlind: e8(0.01), bigBlind: e8(0.02), ante: 0 },
  blindSeats: { sb: 0, bb: 1, dealer: 0 }, liveHandNumber: 2, settled: true,
};

describe('handToText', () => {
  const blinds = blindsForHand({ hand, tableFacts });
  const logGroups = logGroupsFor({ hand, blinds });
  const text = handToText({
    hand, blinds, logGroups, money, tableName: 'Heads Up', currency: 'ICP',
    nameOf: (s) => (s === 0 ? 'You' : 'Nakamoto'), playedAt: new Date(Date.UTC(2026, 7, 9, 19, 42, 24)),
  });
  const lines = text.split('\n');

  it('opens with the PokerStars header, the stakes and the seats', () => {
    expect(lines[0]).toBe("ClearDeck Hand #2: Hold'em No Limit (0.01/0.02 ICP) - 2026-08-09 19:42:24 UTC");
    expect(lines[1]).toBe("Table 'Heads Up' 2-max");
    expect(lines[2]).toBe('Seat 1: You');
    expect(lines[3]).toBe('Seat 2: Nakamoto');
  });
  it('posts the blinds, then heads every street with its board', () => {
    expect(lines).toContain('You: posts small blind 0.01');
    expect(lines).toContain('Nakamoto: posts big blind 0.02');
    expect(lines).toContain('*** HOLE CARDS ***');
    expect(lines).toContain('*** FLOP *** [7c 4h Jh]');
    expect(lines).toContain('*** TURN *** [7c 4h Jh 4s]');
    expect(lines).toContain('*** RIVER *** [7c 4h Jh 4s 5s]');
    expect(lines).toContain('Nakamoto: bets 0.40');
    expect(lines).toContain('You: calls 0.40');
  });
  it('shows the showdown, the award and the summary with a zero rake', () => {
    expect(lines).toContain('*** SHOW DOWN ***');
    expect(lines).toContain('You: shows [9h 7s] (Two Pair)');
    expect(lines).toContain('You collected 1.44 from pot');
    expect(lines).toContain('Total pot 1.44 | Rake 0.00');
    expect(lines).toContain('Board [7c 4h Jh 4s 5s]');
  });
  it('carries the proof as trailing comment lines', () => {
    expect(lines.at(-3)).toBe(`# seed_hash ${'ab'.repeat(32)}`);
    expect(lines.at(-2)).toBe(`# revealed_seed ${'cd'.repeat(32)}`);
  });
  it('never uses an em dash', () => {
    expect(text).not.toContain('\u2014');
  });
  it('handles a hand with no blind level and no showdown', () => {
    const folded = { ...hand, community: [], showdown: [], actions: [act(0, 'Fold', 'preflop')], winners: [{ seat: 1, amount: e8(0.03) }] };
    const b = blindsForHand({ hand: folded, tableFacts: null });
    const t = handToText({ hand: folded, blinds: b, logGroups: logGroupsFor({ hand: folded, blinds: b }), money });
    expect(t).toContain("ClearDeck Hand #2: Hold'em No Limit\n");
    expect(t).toContain('Seat 1: folds');
    expect(t).toContain('Seat 2 collected 0.03 from pot');
    expect(t).not.toContain('SHOW DOWN');
  });
});
