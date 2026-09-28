import { describe, expect, it } from 'vitest';
import {
  BB_DISPLAY_PREF_KEY, BB_UNIT, canShowBB, formatBB, readBbDisplayPref, writeBbDisplayPref,
} from './bb-display.js';

function fakeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => { map.set(k, String(v)); },
    map,
  };
}

const BB = 10_000_000; // 0.10 ICP in e8s

describe('formatBB: a live figure as a multiple of the big blind', () => {
  it('reads one decimal below 100 BB and none from 100 BB up', () => {
    expect(formatBB(125_000_000, BB)).toBe('12.5');
    expect(formatBB(5_000_000, BB)).toBe('0.5');
    expect(formatBB(10_000_000, BB)).toBe('1.0');
    expect(formatBB(999_000_000, BB)).toBe('99.9');
    expect(formatBB(1_000_000_000, BB)).toBe('100');
    expect(formatBB(2_500_000_000, BB)).toBe('250');
    expect(formatBB(12_500_000_000, BB)).toBe('1,250');
  });

  it('rounds a reading rather than truncating it (it is not a figure that is sent)', () => {
    expect(formatBB(123_456_789, BB)).toBe('12.3');
    expect(formatBB(126_000_000, BB)).toBe('12.6');
    expect(formatBB(999_500_000, BB)).toBe('100');
  });

  it('works in sats on a BTC table', () => {
    expect(formatBB(2_500, 100)).toBe('25.0');
    expect(formatBB(150, 100)).toBe('1.5');
  });

  it('refuses to invent a multiple without a big blind', () => {
    expect(formatBB(125_000_000, 0)).toBeNull();
    expect(formatBB(125_000_000, null)).toBeNull();
    expect(formatBB(125_000_000, undefined)).toBeNull();
    expect(formatBB(125_000_000, NaN)).toBeNull();
    expect(formatBB(125_000_000, -1)).toBeNull();
    expect(formatBB(NaN, BB)).toBeNull();
    expect(canShowBB(0)).toBe(false);
    expect(canShowBB(BB)).toBe(true);
  });

  it('a zero reads as zero, not as nothing', () => {
    expect(formatBB(0, BB)).toBe('0.0');
    expect(BB_UNIT).toBe('BB');
  });
});

describe('the big-blind display preference is OFF unless the player turned it on', () => {
  it('is off with no storage, an absent key, or anything but the literal true', () => {
    expect(readBbDisplayPref(null)).toBe(false);
    expect(readBbDisplayPref(undefined)).toBe(false);
    expect(readBbDisplayPref(fakeStorage())).toBe(false);
    expect(readBbDisplayPref(fakeStorage({ [BB_DISPLAY_PREF_KEY]: 'false' }))).toBe(false);
    expect(readBbDisplayPref(fakeStorage({ [BB_DISPLAY_PREF_KEY]: '1' }))).toBe(false);
    expect(readBbDisplayPref(fakeStorage({ [BB_DISPLAY_PREF_KEY]: 'yes' }))).toBe(false);
  });

  it('is on only for the literal true, written by writeBbDisplayPref', () => {
    const s = fakeStorage();
    expect(writeBbDisplayPref(s, true)).toBe(true);
    expect(s.getItem(BB_DISPLAY_PREF_KEY)).toBe('true');
    expect(readBbDisplayPref(s)).toBe(true);
    expect(writeBbDisplayPref(s, false)).toBe(true);
    expect(readBbDisplayPref(s)).toBe(false);
    writeBbDisplayPref(s, 'true');
    expect(readBbDisplayPref(s)).toBe(false);
  });

  it('survives a storage that throws, in both directions, as off', () => {
    const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
    expect(readBbDisplayPref(broken)).toBe(false);
    expect(writeBbDisplayPref(broken, true)).toBe(false);
    expect(writeBbDisplayPref(null, true)).toBe(false);
  });

  it('uses its own key, never the hotkeys one', () => {
    expect(BB_DISPLAY_PREF_KEY).toBe('poker_bb_display');
    expect(readBbDisplayPref(fakeStorage({ poker_hotkeys_enabled: 'true' }))).toBe(false);
  });
});
