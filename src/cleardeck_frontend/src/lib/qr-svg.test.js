import { describe, expect, it } from 'vitest';
import { qrModules, qrSvgPath } from './qr-svg.js';

const ADDRESS = 'd3e13d4777e22367532053190b6c6ccf57444a61337e996242b1abfb52cf92c8';

describe('qrModules', () => {
  it('encodes a 64-hex account identifier into a square grid', () => {
    const { size, isDark } = qrModules(ADDRESS);
    expect(size).toBeGreaterThanOrEqual(21);
    expect(size % 4).toBe(1);
    // The top-left finder pattern is always dark at its corner.
    expect(isDark(0, 0)).toBe(true);
    expect(isDark(3, 3)).toBe(true);
  });

  it('is deterministic', () => {
    const a = qrSvgPath(ADDRESS);
    const b = qrSvgPath(ADDRESS);
    expect(a).toEqual(b);
  });

  it('refuses empty text rather than drawing a blank', () => {
    expect(() => qrModules('')).toThrow();
  });
});

describe('qrSvgPath', () => {
  it('paints runs of dark modules as unit-high rectangles', () => {
    const { size, path } = qrSvgPath(ADDRESS);
    expect(path.startsWith('M0 0h7v1h-7z')).toBe(true);
    expect(path.length).toBeGreaterThan(size * 4);
  });
});
