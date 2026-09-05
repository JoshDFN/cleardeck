// The deck seal, phase by phase.
//
// One state machine, shared by the seal chip in the action log (ActionFeed)
// and the deck object on the felt (DeckSeal), so the two can never disagree
// about what this browser has and has not checked:
//
//   sealed    the hand runs: the commitment (SHA-256 of the seed) is on screen
//             and the seed is not
//   revealed  the seed is published; this browser has not hashed it yet
//   checked   THIS browser hashed the revealed seed and found the commitment
//   mismatch  it did not; do not trust this table
//
// The full verdict (every card re-derived) belongs to the fairness panel. This
// is the moment made visible, and the check here is the one SHA-256 that the
// commitment rests on, run with lib/shuffle-verify.js and no canister asked.

import { sameHash } from './hash-seal.js';
import { hexToBytes, sha256Hex } from './shuffle-verify.js';

export const SEAL_WORD = {
  sealed: 'Sealed',
  revealed: 'Revealed',
  checked: 'Checked ✓',
  mismatch: 'Mismatch ✗',
};

export const SEAL_HINT = {
  sealed: 'The deck was sealed before the first card: this commitment is the SHA-256 of the seed. Copy it now and compare it after the reveal.',
  revealed: 'The seed is published; this browser is hashing it.',
  checked: 'This browser hashed the revealed seed and it is the commitment that was on screen all hand. Open the proof for the cards.',
  mismatch: 'The revealed seed does NOT hash to the commitment. Do not trust this table.',
};

/** The revealed seed off a proof, whether Candid wrapped it as `opt` or not. */
export function revealedSeedOf(proof) {
  const v = proof?.revealed_seed;
  const seed = Array.isArray(v) ? v[0] : v;
  return typeof seed === 'string' && seed.length > 0 ? seed : null;
}

/** The identity of one (commitment, seed) pair this browser has hashed. */
export const sealKey = (seedHash, seed) => `${seedHash}|${seed}`;

/**
 * @param {object} args
 * @param {string|null|undefined} args.seedHash   the commitment
 * @param {string|null} args.revealedSeed
 * @param {string|null} args.checkedKey    the `sealKey` this browser last hashed
 * @param {boolean|null} args.checkResult  what that hash found
 * @returns {'sealed'|'revealed'|'checked'|'mismatch'|null}
 */
export function sealStateFor({ seedHash, revealedSeed, checkedKey, checkResult }) {
  if (!seedHash) return null;
  if (!revealedSeed) return 'sealed';
  if (checkedKey !== sealKey(seedHash, revealedSeed)) return 'revealed';
  return checkResult ? 'checked' : 'mismatch';
}

/**
 * Hashes the revealed seed and compares it with the commitment. Never throws:
 * a seed that cannot be hashed is a mismatch, said out loud.
 *
 * @param {string} seedHash
 * @param {string} seed
 * @returns {Promise<boolean>}
 */
export async function checkSeal(seedHash, seed) {
  try {
    const computed = await sha256Hex(hexToBytes(seed));
    return sameHash(computed, seedHash);
  } catch {
    return false;
  }
}
