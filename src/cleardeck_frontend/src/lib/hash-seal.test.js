import { describe, it, expect } from 'vitest';
import { hashFingerprint, normalizeHex, sameHash, sealCells, SEAL_SIZE } from './hash-seal.js';

const HASH = 'cfae5b49e5d371be895fa7b06db9e16316ab5c75fd46231b4a470f3595b7a5';

describe('hashFingerprint', () => {
  it('keeps the first eight and the last eight characters', () => {
    expect(hashFingerprint(HASH)).toBe('cfae5b49…3595b7a5');
  });
  it('returns a short value whole and upper case normalised', () => {
    expect(hashFingerprint('ABCDEF')).toBe('abcdef');
  });
  it('returns nothing for a value that is not hex', () => {
    expect(hashFingerprint('not a hash')).toBe('');
    expect(hashFingerprint(null)).toBe('');
  });
});

describe('sealCells', () => {
  it('is a 5 x 5 grid mirrored about the middle column', () => {
    const cells = sealCells(HASH);
    expect(cells).toHaveLength(SEAL_SIZE * SEAL_SIZE);
    for (let row = 0; row < SEAL_SIZE; row += 1) {
      expect(cells[row * SEAL_SIZE + 0]).toBe(cells[row * SEAL_SIZE + 4]);
      expect(cells[row * SEAL_SIZE + 1]).toBe(cells[row * SEAL_SIZE + 3]);
    }
  });
  it('is deterministic and differs between two commitments', () => {
    expect(sealCells(HASH)).toEqual(sealCells(HASH.toUpperCase()));
    const other = `1${HASH.slice(1)}`;
    expect(sealCells(other)).not.toEqual(sealCells(HASH));
  });
  it('reads the leading bits: ffff is all on, 0000 all off', () => {
    expect(sealCells('ffff').every(Boolean)).toBe(true);
    expect(sealCells('0000').some(Boolean)).toBe(false);
  });
  it('draws nothing for a value that is not hex or too short', () => {
    expect(sealCells('zz').some(Boolean)).toBe(false);
    expect(sealCells('abc').some(Boolean)).toBe(false);
  });
});

describe('sameHash / normalizeHex', () => {
  it('ignores case and surrounding whitespace', () => {
    expect(sameHash(` ${HASH.toUpperCase()} `, HASH)).toBe(true);
    expect(normalizeHex(' AB ')).toBe('ab');
  });
  it('never calls two empty values the same', () => {
    expect(sameHash('', '')).toBe(false);
    expect(sameHash('xyz', 'xyz')).toBe(false);
  });
});
