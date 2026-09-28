import { describe, expect, it } from 'vitest';
import { ACTION_WORDS, streetActionsOf } from './street-actions.js';

const street = (text) => ({ type: 'phase', text, street: true });
const act = (type, seat, amount = null) => ({ type, seat, text: type, amount, timestamp: 1 });

describe('streetActionsOf: the word each plate shows for this street', () => {
  it('names check, call, bet and raise, and nothing else', () => {
    const words = streetActionsOf([
      act('blind', 1, 5), act('blind', 2, 10),
      act('call', 3, 10), act('raise', 4, 30), act('check', 5),
      act('bet', 0, 20),
    ]);
    expect(words.get(3)).toBe('Call');
    expect(words.get(4)).toBe('Raise');
    expect(words.get(5)).toBe('Check');
    expect(words.get(0)).toBe('Bet');
    // posting a blind is not a decision
    expect(words.has(1)).toBe(false);
    expect(words.has(2)).toBe(false);
    expect(ACTION_WORDS.check).toBe('Check');
  });

  it('keeps only the LAST action of a seat', () => {
    const words = streetActionsOf([act('call', 1, 10), act('raise', 2, 30), act('call', 1, 30)]);
    expect(words.get(1)).toBe('Call');
    expect(words.get(2)).toBe('Raise');
    // a seat that called and then raised reads Raise
    expect(streetActionsOf([act('call', 1, 10), act('raise', 1, 30)]).get(1)).toBe('Raise');
  });

  it('a new street clears every plate', () => {
    const words = streetActionsOf([
      street('Pre-Flop'), act('call', 1, 10), act('check', 2),
      street('Flop'), act('check', 2),
    ]);
    expect(words.has(1)).toBe(false);
    expect(words.get(2)).toBe('Check');
    expect(streetActionsOf([act('call', 1, 10), street('Flop')]).size).toBe(0);
  });

  it('the equity method line is a phase-style entry that is NOT a street', () => {
    const words = streetActionsOf([
      street('Flop'), act('check', 1),
      { type: 'phase', text: 'Equity vs 1 random · Monte Carlo · 200,000 trials' },
    ]);
    expect(words.get(1)).toBe('Check');
    // an unmarked phase line (an older shape) does not reset either
    expect(streetActionsOf([act('check', 1), { type: 'phase', text: 'Flop' }]).get(1)).toBe('Check');
  });

  it('a fold or an all-in removes the word: the plate paints those from chain state', () => {
    expect(streetActionsOf([act('call', 1, 10), act('fold', 1)]).has(1)).toBe(false);
    expect(streetActionsOf([act('raise', 2, 30), act('allin', 2, 500)]).has(2)).toBe(false);
    // and they never add one
    expect(streetActionsOf([act('fold', 1), act('allin', 2, 500)]).size).toBe(0);
  });

  it('ignores showdown, winner and malformed entries', () => {
    const words = streetActionsOf([
      { type: 'showdown', seat: 1, text: 'shows a pair' },
      { type: 'winner', seat: 2, text: 'won', amount: 40 },
      { type: 'check', seat: 'x' },
      { type: 'check', seat: -1 },
      null,
      undefined,
      { type: 'check' },
    ]);
    expect(words.size).toBe(0);
    expect(streetActionsOf(null).size).toBe(0);
    expect(streetActionsOf(undefined).size).toBe(0);
  });

  it('does not mutate the feed', () => {
    const feed = Object.freeze([Object.freeze(act('check', 1)), Object.freeze(street('Flop'))]);
    expect(() => streetActionsOf(feed)).not.toThrow();
  });
});
