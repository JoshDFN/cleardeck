import { describe, expect, it } from 'vitest';
import {
  HOTKEYS_OFF_HINT, HOTKEYS_PREF_KEY, HOTKEYS_TOGGLE_PLACE, readHotkeysPref, writeHotkeysPref,
} from './hotkeys-pref.js';

function fakeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => { map.set(k, String(v)); },
    map,
  };
}

describe('the hotkeys preference is OFF unless the player turned it on', () => {
  it('is off with no storage, an absent key, or anything but the literal true', () => {
    expect(readHotkeysPref(null)).toBe(false);
    expect(readHotkeysPref(undefined)).toBe(false);
    expect(readHotkeysPref(fakeStorage())).toBe(false);
    expect(readHotkeysPref(fakeStorage({ [HOTKEYS_PREF_KEY]: 'false' }))).toBe(false);
    expect(readHotkeysPref(fakeStorage({ [HOTKEYS_PREF_KEY]: '1' }))).toBe(false);
    expect(readHotkeysPref(fakeStorage({ [HOTKEYS_PREF_KEY]: 'yes' }))).toBe(false);
  });

  it('is on only for the literal true, written by writeHotkeysPref', () => {
    const s = fakeStorage();
    expect(writeHotkeysPref(s, true)).toBe(true);
    expect(s.getItem(HOTKEYS_PREF_KEY)).toBe('true');
    expect(readHotkeysPref(s)).toBe(true);
    expect(writeHotkeysPref(s, false)).toBe(true);
    expect(readHotkeysPref(s)).toBe(false);
    // anything that is not exactly true writes off
    writeHotkeysPref(s, 'true');
    expect(readHotkeysPref(s)).toBe(false);
  });

  it('survives a storage that throws, in both directions, as off', () => {
    const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
    expect(readHotkeysPref(broken)).toBe(false);
    expect(writeHotkeysPref(broken, true)).toBe(false);
    expect(writeHotkeysPref(null, true)).toBe(false);
  });

  it('uses the app\'s own namespace and names where to turn it on', () => {
    expect(HOTKEYS_PREF_KEY).toMatch(/^poker_/);
    expect(HOTKEYS_OFF_HINT).toBe(`Keyboard shortcuts off. Turn on in ${HOTKEYS_TOGGLE_PLACE}.`);
    // no digit in the hint: the legend is inside the token census's selector
    expect(HOTKEYS_OFF_HINT).not.toMatch(/\d/);
  });
});
