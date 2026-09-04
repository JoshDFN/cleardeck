import { describe, it, expect } from 'vitest';
import {
  TURN_ALERT_PREF_KEY, TURN_TITLE_MARK, canVibrate, readTurnAlertPref, titleFor, turnAlertDecision,
  turnKey, writeTurnAlertPref,
} from './turn-alert.js';

const memStorage = (initial = {}) => {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => { map.set(k, String(v)); },
    dump: () => Object.fromEntries(map),
  };
};

describe('turnAlertDecision', () => {
  const key = turnKey(7, 'Flop');
  it('fires once on the certified my-turn edge and remembers the turn', () => {
    const first = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key, lastKey: null });
    expect(first).toEqual({ fire: true, lastKey: key });
    const again = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key, lastKey: first.lastKey });
    expect(again.fire).toBe(false);
  });
  it('fires again on the next street and the next hand', () => {
    const flop = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key: turnKey(7, 'Flop'), lastKey: turnKey(7, 'PreFlop') });
    expect(flop.fire).toBe(true);
    const nextHand = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key: turnKey(8, 'PreFlop'), lastKey: turnKey(7, 'River') });
    expect(nextHand.fire).toBe(true);
  });
  it('hero calls, villain raises, same street -> fire', () => {
    // The hero's turn facing a bet of 0.20 (the villain's raise was the last action).
    const facingRaise = turnKey(7, 'PreFlop', 'Raise:20000000', 20000000);
    const first = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key: facingRaise, lastKey: null });
    expect(first.fire).toBe(true);
    // The hero calls (the echo holds the turn false, then the chain moves on).
    const echo = turnAlertDecision({ isMyTurn: false, pendingOpen: true, enabled: true, key: facingRaise, lastKey: first.lastKey });
    expect(echo.fire).toBe(false);
    // A third player re-raises on the SAME street: new last action, new bet, new turn.
    const facingReraise = turnKey(7, 'PreFlop', 'Raise:60000000', 60000000);
    const again = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key: facingReraise, lastKey: echo.lastKey });
    expect(again.fire).toBe(true);
    expect(again.lastKey).toBe(facingReraise);
    // The same turn polled again does not chime twice.
    expect(turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: true, key: facingReraise, lastKey: again.lastKey }).fire).toBe(false);
  });
  it('keys the turn on the last action and the bet, not the street alone', () => {
    expect(turnKey(7, 'Flop', 'Bet:10', 10)).not.toBe(turnKey(7, 'Flop', 'Raise:30', 30));
    expect(turnKey(7, 'Flop', 'Bet:10', 10)).toBe(turnKey(7, 'Flop', 'Bet:10', 10));
    expect(turnKey(7, 'Flop')).toBe('7:Flop::0');
  });
  it('never fires while an echo is open, or when it is not my turn', () => {
    expect(turnAlertDecision({ isMyTurn: true, pendingOpen: true, enabled: true, key, lastKey: null }).fire).toBe(false);
    expect(turnAlertDecision({ isMyTurn: false, pendingOpen: false, enabled: true, key, lastKey: null }).fire).toBe(false);
  });
  it('remembers the turn even when switched off, so switching on mid-turn cannot chime late', () => {
    const off = turnAlertDecision({ isMyTurn: true, pendingOpen: false, enabled: false, key, lastKey: null });
    expect(off).toEqual({ fire: false, lastKey: key });
  });
});

describe('title and preference', () => {
  it('marks the title on my turn and restores it without doubling the mark', () => {
    const marked = titleFor('ClearDeck', true);
    expect(marked).toBe(`${TURN_TITLE_MARK} · ClearDeck`);
    expect(titleFor(marked, true)).toBe(marked);
    expect(titleFor(marked, false)).toBe('ClearDeck');
    expect(titleFor('ClearDeck', false)).toBe('ClearDeck');
  });
  it('defaults to ON, reads a stored "false", and writes both ways', () => {
    expect(readTurnAlertPref(null)).toBe(true);
    expect(readTurnAlertPref(memStorage())).toBe(true);
    expect(readTurnAlertPref(memStorage({ [TURN_ALERT_PREF_KEY]: 'false' }))).toBe(false);
    const s = memStorage();
    expect(writeTurnAlertPref(s, false)).toBe(true);
    expect(s.dump()).toEqual({ [TURN_ALERT_PREF_KEY]: 'false' });
    expect(writeTurnAlertPref(null, true)).toBe(false);
  });
  it('survives a storage that throws', () => {
    const broken = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } };
    expect(readTurnAlertPref(broken)).toBe(true);
    expect(writeTurnAlertPref(broken, true)).toBe(false);
  });
  it('vibrates only on a touch device with the API', () => {
    expect(canVibrate({ vibrate: () => true, maxTouchPoints: 5 })).toBe(true);
    expect(canVibrate({ vibrate: () => true, maxTouchPoints: 0 })).toBe(false);
    expect(canVibrate({ maxTouchPoints: 5 })).toBe(false);
    expect(canVibrate(null)).toBe(false);
  });
});
