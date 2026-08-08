// The ClearDeck shuffle, reimplemented from docs/SHUFFLE-SPEC.md.
//
// INDEPENDENT ON PURPOSE. This file was written from the normative document,
// section by section, and shares no line with src/poker_core/src/shuffle.rs.
// If it ever disagrees with the canister, one of the two is wrong and the whole
// fairness claim is in question -- which is exactly the disagreement this tool
// is built to surface rather than to hide.
//
// It also does not share a line with src/poker_core/tests/verify/verify_shuffle.mjs.
// That verifier prints a deck for a human; this one is a library that reports
// every intermediate value so a reconstruction can SHOW ITS WORK.

import { createHash } from 'node:crypto';
import { buildDeck } from './cards.mjs';

const TWO_POW_64 = 1n << 64n;

/** SHA-256 of raw bytes, as lowercase hex. Section 1, step 0. */
export function sha256Hex(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

/** Parses a hex seed into bytes, refusing anything a verifier should not guess at. */
export function seedBytes(hex) {
  if (typeof hex !== 'string') throw new Error('seed must be a hex string');
  const clean = hex.trim().toLowerCase();
  if (clean.length === 0) throw new Error('seed is empty: no seed has been revealed for this hand');
  if (clean.length % 2 !== 0) throw new Error(`seed must be even-length hex, got ${clean.length} chars`);
  if (!/^[0-9a-f]*$/.test(clean)) throw new Error('seed must be lowercase hex');
  return Buffer.from(clean, 'hex');
}

/** `floor(2^64 / n) * n` -- the largest multiple of n that is <= 2^64. Section 3.3. */
export function drawBound(n) {
  const big = BigInt(n);
  if (big <= 0n) throw new Error('drawBound: modulus must be positive');
  return (TWO_POW_64 / big) * big;
}

/** One link of the chain: `SHA256(chain || counter)`, and the low 8 bytes LE. Section 3.1/3.2. */
function chainStep(chain, counter) {
  const digest = createHash('sha256').update(chain).update(Buffer.from([counter])).digest();
  return { digest, draw: digest.readBigUInt64LE(0) };
}

/**
 * The shuffle of section 3, with a full trace of every step.
 *
 * @param {Buffer|Uint8Array} seed raw seed BYTES, not the hex text
 * @param {{trace?: boolean}} [opts]
 * @returns {{deck: string[], rejections: number, steps: Array<object>}}
 */
export function shuffle(seed, { trace = false } = {}) {
  const deck = buildDeck();
  let chain = Buffer.from(seed);
  let rejections = 0;
  const steps = [];

  for (let i = deck.length - 1; i >= 1; i -= 1) {
    const n = i + 1;
    const limit = drawBound(n);
    const counter = i % 256;
    let draw;
    let rejectedHere = 0;
    for (;;) {
      const stepped = chainStep(chain, counter);
      chain = stepped.digest;
      draw = stepped.draw;
      if (draw < limit) break;
      rejections += 1;
      rejectedHere += 1;
    }
    const j = Number(draw % BigInt(n));
    if (trace) {
      steps.push({
        i, n, counter, chain: chain.toString('hex'), draw: draw.toString(),
        limit: limit.toString(), rejected: rejectedHere, j,
        swapped: [deck[i], deck[j]],
      });
    }
    const tmp = deck[i];
    deck[i] = deck[j];
    deck[j] = tmp;
  }

  return { deck, rejections, steps };
}

/**
 * Step 0 of any verification: does the revealed seed hash to the commitment?
 *
 * Returns a decision, not a bare boolean, because a bare `false` reads as
 * "I was cheated" when the usual cause is a malformed paste. See SHUFFLE-SPEC
 * section 1 on why the canister's own checker says which field was wrong.
 */
export function checkCommitment(seedHashHex, revealedSeedHex) {
  const problems = [];
  const seedHash = String(seedHashHex ?? '').trim().toLowerCase();
  const revealed = String(revealedSeedHex ?? '').trim().toLowerCase();

  if (revealed === '') problems.push('no seed has been revealed for this hand');
  if (seedHash === '') problems.push('the record carries no seed_hash');
  if (seedHash !== '' && !/^[0-9a-f]{64}$/.test(seedHash)) {
    problems.push(`seed_hash is not 64 hex characters (got ${seedHash.length})`);
  }
  if (revealed !== '' && !/^[0-9a-f]+$/.test(revealed)) {
    problems.push('revealed_seed is not lowercase hex');
  }
  if (problems.length > 0) return { ok: false, computed: null, problems };

  const computed = sha256Hex(Buffer.from(revealed, 'hex'));
  if (computed === seedHash) return { ok: true, computed, problems: [] };

  // The one mistake that manufactures a false accusation: the two fields swapped.
  if (sha256Hex(Buffer.from(seedHash, 'hex')) === revealed) {
    problems.push('seed_hash and revealed_seed appear to be in each other\'s fields');
  } else {
    problems.push('the revealed seed does not hash to the committed seed_hash');
  }
  return { ok: false, computed, problems };
}
