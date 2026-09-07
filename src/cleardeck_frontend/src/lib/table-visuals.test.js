import { describe, it, expect } from 'vitest';
import {
  hashString,
  generatedName,
  monogram,
  avatarFor,
  chipBand,
  chipStackCount,
  clockArcDegrees,
  shortName,
  AVATAR_HUES,
} from './table-visuals.js';

describe('hashString', () => {
  it('is deterministic and non-negative', () => {
    expect(hashString('abc')).toBe(hashString('abc'));
    expect(hashString('abc')).toBeGreaterThanOrEqual(0);
    expect(hashString('')).toBe(0);
    expect(hashString(null)).toBe(0);
  });
  it('separates nearby inputs', () => {
    expect(hashString('aaaaa-aa')).not.toBe(hashString('aaaaa-ab'));
  });
});

describe('generatedName', () => {
  it('returns Unknown for a missing principal', () => {
    expect(generatedName('')).toBe('Unknown');
    expect(generatedName(undefined)).toBe('Unknown');
  });
  it('is Adjective + Noun + two digits and stable', () => {
    const n = generatedName('2vxsx-fae');
    expect(n).toMatch(/^[A-Z][a-z]+[A-Z][a-z]+\d{2}$/);
    expect(generatedName('2vxsx-fae')).toBe(n);
  });
});

describe('monogram', () => {
  it('takes the first letter of up to two capitalised words', () => {
    expect(monogram('ShadowDragon71')).toBe('SD');
    expect(monogram('Ada')).toBe('A');
    expect(monogram('You')).toBe('Y');
    expect(monogram('Seat 4')).toBe('S');
    expect(monogram('lucky fox')).toBe('LF');
  });
  it('never returns an empty string', () => {
    expect(monogram('')).toBe('?');
    expect(monogram('1234')).toBe('?');
    expect(monogram(null)).toBe('?');
  });
});

describe('avatarFor', () => {
  it('produces a data URI SVG with the monogram and a hue from the palette', () => {
    const a = avatarFor('2vxsx-fae', 'ShadowDragon71');
    expect(a.src.startsWith('data:image/svg+xml;utf8,')).toBe(true);
    expect(decodeURIComponent(a.src)).toContain('>SD<');
    expect(AVATAR_HUES).toContain(a.hue);
  });
  it('is deterministic per principal', () => {
    expect(avatarFor('p-1', 'A').src).toBe(avatarFor('p-1', 'A').src);
    expect(avatarFor('p-1', 'A').hue).toBe(avatarFor('p-1', 'B').hue);
  });
});

describe('chipBand', () => {
  const BB = 10_000_000; // 0.10 ICP in e8s
  it('is white when there is nothing to draw or no blind to scale by', () => {
    expect(chipBand(0, BB)).toBe('white');
    expect(chipBand(50, 0)).toBe('white');
    expect(chipBand(NaN, BB)).toBe('white');
  });
  it('steps through the casino denominations by big-blind multiple', () => {
    expect(chipBand(BB, BB)).toBe('white');
    expect(chipBand(BB * 1.9, BB)).toBe('white');
    expect(chipBand(BB * 2, BB)).toBe('red');
    expect(chipBand(BB * 4.9, BB)).toBe('red');
    expect(chipBand(BB * 5, BB)).toBe('blue');
    expect(chipBand(BB * 12, BB)).toBe('green');
    expect(chipBand(BB * 30, BB)).toBe('black');
    expect(chipBand(BB * 100, BB)).toBe('gold');
  });
});

describe('chipStackCount', () => {
  const BB = 10_000_000;
  it('draws nothing for nothing and one disc without a blind', () => {
    expect(chipStackCount(0, BB)).toBe(0);
    expect(chipStackCount(BB, 0)).toBe(1);
  });
  it('grows on a log scale and caps at five', () => {
    expect(chipStackCount(BB, BB)).toBe(1);
    expect(chipStackCount(BB * 2, BB)).toBe(2);
    expect(chipStackCount(BB * 5, BB)).toBe(3);
    expect(chipStackCount(BB * 20, BB)).toBe(4);
    expect(chipStackCount(BB * 400, BB)).toBe(5);
    expect(chipStackCount(BB * 1e9, BB)).toBe(5);
  });
});

describe('clockArcDegrees', () => {
  it('maps the fraction to a 0-360 arc and clamps', () => {
    expect(clockArcDegrees(1)).toBe(360);
    expect(clockArcDegrees(0.5)).toBe(180);
    expect(clockArcDegrees(0)).toBe(0);
    expect(clockArcDegrees(1.7)).toBe(360);
    expect(clockArcDegrees(-2)).toBe(0);
    expect(clockArcDegrees(NaN)).toBe(0);
    expect(clockArcDegrees('x')).toBe(0);
  });
});

describe('shortName', () => {
  it('truncates at 11 characters by default and never throws', () => {
    expect(shortName('ShadowDragon71')).toBe('ShadowDrago');
    expect(shortName('Ada')).toBe('Ada');
    expect(shortName(null)).toBe('');
    expect(shortName('abcdef', 3)).toBe('abc');
  });
});
