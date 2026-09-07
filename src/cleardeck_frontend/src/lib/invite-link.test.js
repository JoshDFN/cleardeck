import { describe, expect, it } from 'vitest';
import { inviteLinkFor, isCanisterId, searchWithTable, tableIdFromSearch } from './invite-link.js';

const ID = '4fbx2-kt777-77775-aaabq-cai';
const MAINNET_ID = 'kieex-piaaa-aaaaj-qor3q-cai';

describe('isCanisterId', () => {
  it('accepts principal-shaped canister ids and nothing else', () => {
    expect(isCanisterId(ID)).toBe(true);
    expect(isCanisterId(MAINNET_ID)).toBe(true);
    expect(isCanisterId('javascript:alert(1)')).toBe(false);
    expect(isCanisterId('')).toBe(false);
    expect(isCanisterId(null)).toBe(false);
    expect(isCanisterId('4fbx2-kt777-77775-aaabq-caix')).toBe(false);
  });
});

describe('inviteLinkFor', () => {
  it('builds the link on the page\'s own origin and path', () => {
    expect(inviteLinkFor({ origin: 'https://cleardeck.example', pathname: '/' }, ID))
      .toBe(`https://cleardeck.example/?table=${ID}`);
    expect(inviteLinkFor({ origin: 'http://localhost:8077', pathname: '/app/' }, ID))
      .toBe(`http://localhost:8077/app/?table=${ID}`);
  });

  it('refuses anything that is not a canister id', () => {
    expect(inviteLinkFor({ origin: 'https://x' }, 'nope')).toBeNull();
    expect(inviteLinkFor(null, ID)).toBeNull();
  });
});

describe('tableIdFromSearch', () => {
  it('reads the table off the query string', () => {
    expect(tableIdFromSearch(`?table=${ID}`)).toBe(ID);
    expect(tableIdFromSearch(`?x=1&table=${ID}`)).toBe(ID);
    expect(tableIdFromSearch(`table=${ID}`)).toBe(ID);
  });

  it('ignores a missing or hostile value', () => {
    expect(tableIdFromSearch('')).toBeNull();
    expect(tableIdFromSearch('?table=<script>')).toBeNull();
    expect(tableIdFromSearch(undefined)).toBeNull();
  });
});

describe('searchWithTable', () => {
  it('writes the open table and clears it for the lobby without touching other params', () => {
    expect(searchWithTable('', ID)).toBe(`?table=${ID}`);
    expect(searchWithTable(`?table=${ID}`, null)).toBe('');
    expect(searchWithTable('?debug=1', ID)).toBe(`?debug=1&table=${ID}`);
    expect(searchWithTable(`?debug=1&table=${ID}`, null)).toBe('?debug=1');
  });
});
