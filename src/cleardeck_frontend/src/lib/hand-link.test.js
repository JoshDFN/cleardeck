import { describe, expect, it } from 'vitest';
import {
  handIdFor, handLinkFor, handNumberFromSearch, parseHandNumber, searchWithHand, tableSlug,
} from './hand-link.js';

const ID = '4xhad-gd777-77775-aaacq-cai';

describe('tableSlug', () => {
  it('lowercases and collapses punctuation to single underscores', () => {
    expect(tableSlug('Table 1')).toBe('table_1');
    expect(tableSlug('6-Max · 0.05/0.10')).toBe('6_max_0_05_0_10');
    expect(tableSlug('  Heads-Up  ')).toBe('heads_up');
  });

  it('never returns an empty slug', () => {
    expect(tableSlug('')).toBe('table');
    expect(tableSlug(null)).toBe('table');
    expect(tableSlug('···')).toBe('table');
  });
});

describe('handIdFor', () => {
  it('is the slug plus the hand number', () => {
    expect(handIdFor('Table 1', 2)).toBe('table_1#2');
    expect(handIdFor('Table 1', '17')).toBe('table_1#17');
  });

  it('refuses a hand number that is not a positive integer', () => {
    expect(handIdFor('Table 1', 0)).toBeNull();
    expect(handIdFor('Table 1', -1)).toBeNull();
    expect(handIdFor('Table 1', 1.5)).toBeNull();
    expect(handIdFor('Table 1', 'x')).toBeNull();
  });
});

describe('parseHandNumber', () => {
  it('accepts positive integers only', () => {
    expect(parseHandNumber('3')).toBe(3);
    expect(parseHandNumber(3)).toBe(3);
    expect(parseHandNumber('0')).toBeNull();
    expect(parseHandNumber('3.5')).toBeNull();
    expect(parseHandNumber('1e3')).toBeNull();
    expect(parseHandNumber('99999999999')).toBeNull();
    expect(parseHandNumber(null)).toBeNull();
  });
});

describe('handNumberFromSearch', () => {
  it('reads the hand beside a valid table', () => {
    expect(handNumberFromSearch(`?table=${ID}&hand=2`)).toBe(2);
    expect(handNumberFromSearch(`table=${ID}&hand=2`)).toBe(2);
  });

  it('ignores a hand with no table, a bad table, or a bad hand', () => {
    expect(handNumberFromSearch('?hand=2')).toBeNull();
    expect(handNumberFromSearch('?table=nope&hand=2')).toBeNull();
    expect(handNumberFromSearch(`?table=${ID}&hand=abc`)).toBeNull();
    expect(handNumberFromSearch('')).toBeNull();
  });
});

describe('searchWithHand', () => {
  it('sets, replaces and clears the hand while keeping everything else', () => {
    expect(searchWithHand(`?table=${ID}`, 2)).toBe(`?table=${ID}&hand=2`);
    expect(searchWithHand(`?table=${ID}&hand=1`, 2)).toBe(`?table=${ID}&hand=2`);
    expect(searchWithHand(`?x=1&table=${ID}&hand=1`, null)).toBe(`?x=1&table=${ID}`);
  });

  it('never writes a hand without a table to hang it on', () => {
    expect(searchWithHand('', 2)).toBe('');
    expect(searchWithHand('?x=1', 2)).toBe('?x=1');
  });

  it('returns a new string and leaves the input alone', () => {
    const input = `?table=${ID}`;
    searchWithHand(input, 5);
    expect(input).toBe(`?table=${ID}`);
  });
});

describe('handLinkFor', () => {
  it('builds the link on the page\'s own origin and path', () => {
    expect(handLinkFor({ origin: 'https://cleardeck.example', pathname: '/' }, ID, 2))
      .toBe(`https://cleardeck.example/?table=${ID}&hand=2`);
    expect(handLinkFor({ origin: 'http://localhost:8077', pathname: '/app/' }, ID, 7))
      .toBe(`http://localhost:8077/app/?table=${ID}&hand=7`);
  });

  it('refuses a bad table or a bad hand', () => {
    expect(handLinkFor({ origin: 'https://x' }, 'nope', 2)).toBeNull();
    expect(handLinkFor({ origin: 'https://x' }, ID, 0)).toBeNull();
    expect(handLinkFor(null, ID, 2)).toBeNull();
  });
});
