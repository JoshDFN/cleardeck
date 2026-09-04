import { describe, it, expect } from 'vitest';
import { compactHandName } from './hand-names.js';

describe('compactHandName', () => {
  it('drops the kicker clause from a pair', () => {
    expect(compactHandName('Pair of Eights, Ace kicker')).toBe('Pair of Eights');
  });
  it('lets the ranks name two pair and a full house', () => {
    expect(compactHandName('Two Pair, Aces and Eights')).toBe('Aces and Eights');
    expect(compactHandName('Full House, Aces full of Eights')).toBe('Aces full of Eights');
  });
  it('counts trips and quads in one word', () => {
    expect(compactHandName('Three of a Kind, Sevens')).toBe('Three Sevens');
    expect(compactHandName('Four of a Kind, Sevens')).toBe('Four Sevens');
  });
  it('drops the high card of a straight flush', () => {
    expect(compactHandName('Straight Flush, Nine high')).toBe('Straight Flush');
  });
  it('leaves the short names alone', () => {
    for (const name of ['Ace-Four offsuit', 'King-Queen suited', 'Pocket Tens', 'Straight, Ten high',
      'Flush, Ace high', 'Ace high', 'Royal Flush']) {
      expect(compactHandName(name)).toBe(name);
    }
  });
  it('never introduces a digit', () => {
    for (const name of ['Pair of Eights, Ace kicker', 'Two Pair, Tens and Twos', 'Four of a Kind, Nines']) {
      expect(compactHandName(name)).not.toMatch(/\d/);
    }
  });
  it('returns null for no name', () => {
    expect(compactHandName(null)).toBeNull();
    expect(compactHandName(undefined)).toBeNull();
    expect(compactHandName('   ')).toBeNull();
  });
});
