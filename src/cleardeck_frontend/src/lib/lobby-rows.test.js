import { describe, expect, it } from 'vitest';
import {
  buildRow, canisterIdOf, configDrift, currencyOf, nameQuote, nowOf, seatsOf, starterTable, tagOf,
} from './lobby-rows.js';

const ICP = (n) => Math.round(n * 100_000_000);

const config = (over = {}) => ({
  small_blind: ICP(0.05), big_blind: ICP(0.10), min_buy_in: ICP(10), max_buy_in: ICP(50),
  max_players: 6, ante: 0, action_timeout_secs: 45, time_bank_secs: 30, currency: { ICP: null },
  ...over,
});

const table = (over = {}) => ({
  id: 2n, name: '6-Max - 0.01/0.02', canister_id: [{ toText: () => '4fbx2-kt777-77775-aaabq-cai' }],
  currency: [{ ICP: null }], player_count: 0n, config: config({ small_blind: ICP(0.01), big_blind: ICP(0.02) }),
  ...over,
});

const player = (name, chips, status = 'Active') =>
  [{ display_name: [name], chips: BigInt(chips), status: { [status]: null } }];

const view = (over = {}) => ({
  config: config(), phase: { WaitingForPlayers: null }, pot: 0n, hand_number: 0n,
  players: [player('You', ICP(40)), [], player('Ada', ICP(50), 'SittingOut'), [], [], []],
  community_cards: [], last_hand_winners: [],
  ...over,
});

describe('Candid helpers', () => {
  it('reads opt, variant tags, canister ids and currencies', () => {
    expect(tagOf([{ Flop: null }])).toBe('Flop');
    expect(tagOf([])).toBeNull();
    expect(canisterIdOf(table())).toBe('4fbx2-kt777-77775-aaabq-cai');
    expect(canisterIdOf({ canister_id: [] })).toBeNull();
    expect(currencyOf(table())).toBe('ICP');
    expect(currencyOf({ currency: [], config: { currency: { BTC: null } } })).toBe('BTC');
  });
});

describe('seats and status', () => {
  it('maps the live seat map with per-seat status', () => {
    const seats = seatsOf(table(), view());
    expect(seats).toHaveLength(6);
    expect(seats[0]).toMatchObject({ seat: 0, name: 'You', chips: ICP(40), active: true });
    expect(seats[2]).toMatchObject({ seat: 2, name: 'Ada', active: false });
    expect(seats[1]).toBeNull();
  });

  it('falls back to the lobby count without a view and never overstates', () => {
    const seats = seatsOf(table({ player_count: 2n }), null);
    expect(seats.filter(Boolean)).toHaveLength(2);
  });

  it('states the table status as an invitation when nobody is seated', () => {
    expect(nowOf(table(), view({ players: [[], [], [], [], [], []] })))
      .toEqual({ kind: 'open', label: 'No one seated yet', detail: null });
    expect(nowOf(table(), view())).toMatchObject({ kind: 'waiting', label: 'Waiting for one more' });
    expect(nowOf(table(), view({ players: [player('A', 1), player('B', 1), [], [], [], []] })))
      .toMatchObject({ kind: 'ready' });
    expect(nowOf(table(), view({ phase: { Flop: null }, pot: BigInt(ICP(0.3)) })))
      .toEqual({ kind: 'live', label: 'Flop', detail: '0.30 ICP' });
  });
});

describe('drift and the name quote', () => {
  it('names the keys where the lobby record and the contract disagree', () => {
    expect(configDrift(table(), view())).toEqual(['small blind', 'big blind']);
    expect(configDrift(table(), null)).toEqual([]);
  });

  it('strikes a stale price in the registered name without rewriting it', () => {
    const q = nameQuote(table(), view());
    expect(q).toMatchObject({ before: '6-Max - ', quoted: '0.01/0.02', after: '', stale: true, chain: '0.05/0.10' });
    expect(nameQuote(table({ name: 'Heads Up' }), view()).stale).toBe(false);
  });
});

describe('buildRow and starterTable', () => {
  it('computes every field a card renders from the contract config', () => {
    const row = buildRow(table(), view());
    expect(row).toMatchObject({
      id: '2', name: '6-Max - 0.01/0.02', currency: 'ICP', filled: 2, idle: 1, isFull: false,
      format: '6-max', tier: 'Low', clockText: '45s + 30s', hands: 0,
    });
    expect(row.cfg.small_blind).toBe(ICP(0.05));
    expect(row.table).toBe(row.table);
  });

  it('picks the cheapest ICP table as the starter, over a cheaper sats table', () => {
    const cheapIcp = table({ id: 1n, name: 'Heads Up', config: config({ min_buy_in: ICP(2) }) });
    const dearIcp = table({ id: 3n, name: '9-Max', config: config({ min_buy_in: ICP(20) }) });
    const sats = { id: 4n, name: 'ckBTC', currency: [{ BTC: null }], player_count: 0n, config: config({ min_buy_in: 1000, currency: { BTC: null } }) };
    expect(starterTable([dearIcp, sats, cheapIcp], () => null)).toBe(cheapIcp);
    expect(starterTable([sats], () => null)).toBe(sats);
    expect(starterTable([], () => null)).toBeNull();
  });
});
