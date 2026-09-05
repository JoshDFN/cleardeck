import { describe, expect, it } from 'vitest';
import { SEAL_HINT, SEAL_WORD, checkSeal, revealedSeedOf, sealKey, sealStateFor } from './deck-seal.js';

const HASH = 'a'.repeat(64);
const SEED = 'b'.repeat(64);

describe('revealedSeedOf', () => {
  it('unwraps a Candid opt and refuses an empty one', () => {
    expect(revealedSeedOf({ revealed_seed: [SEED] })).toBe(SEED);
    expect(revealedSeedOf({ revealed_seed: SEED })).toBe(SEED);
    expect(revealedSeedOf({ revealed_seed: [] })).toBeNull();
    expect(revealedSeedOf({ revealed_seed: '' })).toBeNull();
    expect(revealedSeedOf(null)).toBeNull();
  });
});

describe('sealStateFor', () => {
  it('is null with no commitment, sealed until the seed lands', () => {
    expect(sealStateFor({ seedHash: null, revealedSeed: null, checkedKey: null, checkResult: null })).toBeNull();
    expect(sealStateFor({ seedHash: HASH, revealedSeed: null, checkedKey: null, checkResult: null })).toBe('sealed');
  });

  it('is revealed until THIS pair was hashed, then checked or mismatch', () => {
    expect(sealStateFor({ seedHash: HASH, revealedSeed: SEED, checkedKey: null, checkResult: null })).toBe('revealed');
    expect(sealStateFor({ seedHash: HASH, revealedSeed: SEED, checkedKey: sealKey(HASH, 'c'.repeat(64)), checkResult: true })).toBe('revealed');
    expect(sealStateFor({ seedHash: HASH, revealedSeed: SEED, checkedKey: sealKey(HASH, SEED), checkResult: true })).toBe('checked');
    expect(sealStateFor({ seedHash: HASH, revealedSeed: SEED, checkedKey: sealKey(HASH, SEED), checkResult: false })).toBe('mismatch');
  });

  it('has a word and a hint for every state', () => {
    for (const state of ['sealed', 'revealed', 'checked', 'mismatch']) {
      expect(typeof SEAL_WORD[state]).toBe('string');
      expect(typeof SEAL_HINT[state]).toBe('string');
    }
  });
});

describe('checkSeal', () => {
  it('finds the commitment for a seed that hashes to it', async () => {
    // SHA-256 of the 32 zero bytes, a published test vector.
    const seed = '00'.repeat(32);
    const hash = '66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925';
    expect(await checkSeal(hash, seed)).toBe(true);
    expect(await checkSeal(hash.toUpperCase(), seed)).toBe(true);
  });

  it('is a mismatch for a wrong seed and never throws on garbage', async () => {
    const hash = '66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925';
    expect(await checkSeal(hash, '01'.repeat(32))).toBe(false);
    expect(await checkSeal(hash, 'not hex')).toBe(false);
  });
});
