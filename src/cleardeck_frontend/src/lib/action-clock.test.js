import { describe, it, expect } from 'vitest';
import {
  clockFraction, clockUrgent, displayedSeconds, offerTimeBank, resyncClock,
} from './action-clock.js';

describe('resyncClock', () => {
  it('takes the first certified figure as the sample', () => {
    expect(resyncClock(null, 40, 10_000)).toEqual({ serverSecs: 40, receivedAt: 10_000 });
  });
  it('keeps the local sample while the chain agrees within 2 s', () => {
    const s = resyncClock(null, 40, 10_000);
    expect(resyncClock(s, 39, 11_300)).toBe(s);
    expect(resyncClock(s, 37, 12_400)).toBe(s);
  });
  it('resyncs when the chain drifts by more than 2 s', () => {
    const s = resyncClock(null, 40, 10_000);
    const next = resyncClock(s, 33, 12_000);
    expect(next).not.toBe(s);
    expect(next.serverSecs).toBe(33);
  });
  it('resyncs immediately when the figure goes up (time bank, new turn)', () => {
    const s = resyncClock(null, 5, 10_000);
    expect(resyncClock(s, 30, 11_000).serverSecs).toBe(30);
  });
  it('clears when the chain reports no clock', () => {
    const s = resyncClock(null, 40, 10_000);
    expect(resyncClock(s, null, 11_000)).toBe(null);
  });
});

describe('displayedSeconds', () => {
  const s = { serverSecs: 40, receivedAt: 10_000 };
  it('ticks down locally in whole seconds and never below zero', () => {
    expect(displayedSeconds(s, 10_900)).toBe(40);
    expect(displayedSeconds(s, 11_000)).toBe(39);
    expect(displayedSeconds(s, 60_000)).toBe(0);
    expect(displayedSeconds(null, 60_000)).toBe(null);
  });
  it('freezes at the instant of a send', () => {
    expect(displayedSeconds(s, 20_000, 12_500)).toBe(38);
  });
});

describe('fraction, urgency and the time bank offer', () => {
  it('fraction is clamped', () => {
    expect(clockFraction(30, 60)).toBe(0.5);
    expect(clockFraction(90, 60)).toBe(1);
    expect(clockFraction(null, 60)).toBe(0);
  });
  it('the last ten seconds are urgent', () => {
    expect(clockUrgent(10)).toBe(true);
    expect(clockUrgent(11)).toBe(false);
    expect(clockUrgent(null)).toBe(false);
  });
  it('offers the bank only under 15 s and only when one is left', () => {
    expect(offerTimeBank({ secs: 14, bankSecs: 30, usingBank: false })).toBe(true);
    expect(offerTimeBank({ secs: 40, bankSecs: 30, usingBank: false })).toBe(false);
    expect(offerTimeBank({ secs: 5, bankSecs: 0, usingBank: false })).toBe(false);
    expect(offerTimeBank({ secs: 5, bankSecs: 30, usingBank: true })).toBe(false);
  });
});
