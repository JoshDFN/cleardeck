// The felt's showdown predicate (docs/DEFECTS.md E-107): between hands, the
// engine-revealed cards are on the felt only when the hand went to a showdown.
import { describe, expect, it } from 'vitest';
import { handsAreFaceUp, lastHandWentToShowdown } from './showdown.js';

const card = (rank, suit) => ({ rank: { [rank]: null }, suit: { [suit]: null } });
const foldOutWinner = { seat: 2, amount: 40n, cards: [], hand_rank: [] };
const showdownWinner = { seat: 0, amount: 120n, cards: [[card('Ace', 'Spades'), card('Ace', 'Hearts')]], hand_rank: [{ OnePair: null }] };
const refund = { seat: 1, amount: 20n, cards: [], hand_rank: [] };

describe('lastHandWentToShowdown', () => {
  it('a fold-out winner is recorded without cards, so the hand did not go to a showdown', () => {
    expect(lastHandWentToShowdown([foldOutWinner])).toBe(false);
  });
  it('a showdown winner carries the hand it showed', () => {
    expect(lastHandWentToShowdown([showdownWinner])).toBe(true);
    expect(lastHandWentToShowdown([refund, showdownWinner])).toBe(true);
  });
  it('no record, no showdown', () => {
    expect(lastHandWentToShowdown([])).toBe(false);
    expect(lastHandWentToShowdown(undefined)).toBe(false);
    expect(lastHandWentToShowdown(null)).toBe(false);
  });
  it('a malformed record is not a showdown', () => {
    expect(lastHandWentToShowdown([{ seat: 0 }, null, { cards: 'AsAh' }])).toBe(false);
  });
});

describe('handsAreFaceUp', () => {
  it('at the showdown itself, always', () => {
    expect(handsAreFaceUp('Showdown', [])).toBe(true);
  });
  it('between hands, only after a showdown', () => {
    expect(handsAreFaceUp('HandComplete', [showdownWinner])).toBe(true);
    expect(handsAreFaceUp('HandComplete', [foldOutWinner])).toBe(false);
    expect(handsAreFaceUp('HandComplete', [])).toBe(false);
  });
  it('never while the hand is live or the table is waiting', () => {
    for (const phase of ['WaitingForPlayers', 'PreFlop', 'Flop', 'Turn', 'River', null, undefined]) {
      expect(handsAreFaceUp(phase, [showdownWinner])).toBe(false);
    }
  });
});
