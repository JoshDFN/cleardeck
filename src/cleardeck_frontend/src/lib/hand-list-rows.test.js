import { describe, it, expect } from 'vitest';
import {
  FILTERS, applyFilter, filterCounts, heroCards, heroResult, opponentCount, verdictOf,
} from './hand-list-rows.js';

const ME = 'aaaa-me';
const THEM = 'bbbb-them';
const card = (rank, suit) => ({ rank: { [rank]: null }, suit: { [suit]: null } });

const won = {
  handNumber: 1,
  seats: [0, 1],
  showdown: [
    { seat: 0, principal: ME, cards: [card('Ace', 'Spades'), card('King', 'Hearts')], won: 200 },
    { seat: 1, principal: THEM, cards: [card('Two', 'Clubs'), card('Three', 'Clubs')], won: 0 },
  ],
  winners: [{ seat: 0, principal: ME, amount: 200 }],
  verification: { ok: true, cardsMatched: 7 },
};
const lost = {
  handNumber: 2,
  seats: [0, 1, 2],
  showdown: [{ seat: 0, principal: ME, cards: null, won: 0 }],
  winners: [{ seat: 1, principal: THEM, amount: 50 }],
  verification: null,
};
const notMine = { handNumber: 3, seats: [1, 2], showdown: [], winners: [{ seat: 1, principal: THEM, amount: 9 }], verification: { ok: false, commitment: { match: true }, cardsChecked: 0 } };
const open = { handNumber: 4, seats: [0, 1], showdown: [], winners: [], verification: null };
// The hero folded pre-flop: in no showdown, no winner, but the record's
// actions (the archive names who acted) and its player list say ME was in.
const folded = {
  handNumber: 5,
  seats: [0, 1],
  showdown: [],
  winners: [{ seat: 1, principal: THEM, amount: 30 }],
  actions: [{ kind: 'Fold', seat: 0, principal: ME, amount: null }],
  verification: null,
};
const foldedByPlayers = {
  handNumber: 6,
  seats: [0, 1],
  showdown: [],
  winners: [{ seat: 1, principal: THEM, amount: 30 }],
  players: [{ seat: 0, principal: ME }, { seat: 1, principal: THEM }],
  actions: [{ kind: 'Fold', seat: 0, principal: null, amount: null }],
  verification: null,
};

describe('heroResult', () => {
  it('reads the record, not the pot', () => {
    expect(heroResult(won, ME)).toBe('won');
    expect(heroResult(lost, ME)).toBe('lost');
    expect(heroResult(notMine, ME)).toBe('out');
    expect(heroResult(open, ME)).toBe('open');
    expect(heroResult(won, null)).toBe('out');
  });
  it('a hand the hero folded before showdown reads Lost, never "Not in"', () => {
    expect(heroResult(folded, ME)).toBe('lost');
    expect(heroResult(foldedByPlayers, ME)).toBe('lost');
    expect(heroResult(folded, THEM)).toBe('won');
    expect(heroResult(folded, 'cccc-nobody')).toBe('out');
    expect(heroCards(folded, ME)).toBeNull();
    expect(opponentCount(foldedByPlayers, ME)).toBe(1);
  });
});

describe('heroCards / opponentCount', () => {
  it('shows the two cards the record revealed and nothing for a fold', () => {
    expect(heroCards(won, ME)).toHaveLength(2);
    expect(heroCards(lost, ME)).toBeNull();
    expect(heroCards(notMine, ME)).toBeNull();
  });
  it('counts the other seats the record saw', () => {
    expect(opponentCount(won, ME)).toBe(1);
    expect(opponentCount(lost, ME)).toBe(2);
    expect(opponentCount(notMine, ME)).toBe(2);
  });
});

describe('applyFilter / filterCounts', () => {
  const all = [won, lost, notMine, open];
  it('keeps every hand under "all" and what the record says under the rest', () => {
    expect(applyFilter(all, 'all', ME)).toHaveLength(4);
    expect(applyFilter(all, 'mine', ME).map((h) => h.handNumber)).toEqual([1, 2]);
    expect(applyFilter([...all, folded], 'mine', ME).map((h) => h.handNumber)).toEqual([1, 2, 5]);
    expect(applyFilter(all, 'won', ME).map((h) => h.handNumber)).toEqual([1]);
    expect(applyFilter(all, 'showdown', ME).map((h) => h.handNumber)).toEqual([1, 2]);
  });
  it('counts per chip in the chips\' own order', () => {
    expect(FILTERS.map((f) => f.id)).toEqual(['mine', 'all', 'won', 'showdown']);
    expect(filterCounts(all, ME)).toEqual({ mine: 2, all: 4, won: 1, showdown: 2 });
  });
});

describe('verdictOf', () => {
  it('names the four tones', () => {
    expect(verdictOf(won)).toEqual({ tone: 'good', label: '7 cards re-derived here' });
    expect(verdictOf(lost).tone).toBe('pending');
    expect(verdictOf(notMine).tone).toBe('partial');
    expect(verdictOf({ verification: { ok: false, commitment: { match: false }, cardsChecked: 2 } }).tone).toBe('bad');
  });
});
