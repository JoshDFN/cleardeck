/**
 * A hash as something the eye can compare.
 *
 * Sixty-four hex characters at 10 px are not comparable by a human: the
 * fairness panel used to print "computed here" and "committed before" as two
 * three-line blocks and assert their equality in a sentence. SSH's randomart,
 * Signal's safety numbers and the blockies on a block explorer all solve the
 * same problem the same way: turn the bytes into a small shape, and let two
 * shapes be compared in one glance.
 *
 * Two derivations, both pure and both deterministic:
 *
 *   hashFingerprint  the first eight and the last eight hex characters, joined
 *                    by an ellipsis. Enough to tell two commitments apart by eye;
 *                    the full hex stays behind Copy and a title.
 *   sealCells        a 5 x 5 grid of on/off cells, mirrored left to right the
 *                    way an identicon is, read off the leading bits of the hash.
 *                    Fifteen bits decide the glyph, so two hashes that differ in
 *                    their first two bytes get different seals and a change
 *                    deeper in the hash is caught by the fingerprint's tail.
 *
 * Neither function is part of the verification: they are how a verified value
 * is DRAWN, never how it is CHECKED. The check is shuffle-verify.js.
 */

const HEX = /^[0-9a-f]+$/;

export const SEAL_SIZE = 5;

/** The middle column plus the two mirrored ones: the bits a seal reads. */
const SEAL_BITS = SEAL_SIZE * Math.ceil(SEAL_SIZE / 2);

/** Lower-cased hex with nothing else in it, or an empty string. */
export function normalizeHex(value) {
  const hex = String(value ?? '').trim().toLowerCase();
  return HEX.test(hex) ? hex : '';
}

/**
 * "cfae5b49…3595b7a5" for a 64-character commitment; a short value is returned
 * whole (there is nothing to elide); anything that is not hex is an empty string.
 *
 * @param {string} value
 * @param {{head?:number, tail?:number}} [opts]
 */
export function hashFingerprint(value, { head = 8, tail = 8 } = {}) {
  const hex = normalizeHex(value);
  if (!hex) return '';
  if (hex.length <= head + tail) return hex;
  return `${hex.slice(0, head)}…${hex.slice(-tail)}`;
}

/**
 * The seal as 25 booleans, row-major, mirrored about the middle column.
 *
 * @param {string} value
 * @returns {boolean[]} exactly SEAL_SIZE * SEAL_SIZE entries; all false when the
 *   value is not hex or too short to carry fifteen bits
 */
export function sealCells(value) {
  const hex = normalizeHex(value);
  const half = Math.ceil(SEAL_SIZE / 2);
  const cells = new Array(SEAL_SIZE * SEAL_SIZE).fill(false);
  if (hex.length * 4 < SEAL_BITS) return cells;
  const bits = [];
  for (const ch of hex) {
    const n = parseInt(ch, 16);
    for (let b = 3; b >= 0; b -= 1) bits.push(((n >> b) & 1) === 1);
    if (bits.length >= SEAL_BITS) break;
  }
  return cells.map((_, at) => {
    const row = Math.floor(at / SEAL_SIZE);
    const col = at % SEAL_SIZE;
    const mirrored = col < half ? col : SEAL_SIZE - 1 - col;
    return bits[row * half + mirrored];
  });
}

/** Two values that are the same hash, ignoring case and whitespace. */
export function sameHash(a, b) {
  const x = normalizeHex(a);
  const y = normalizeHex(b);
  return x.length > 0 && x === y;
}
