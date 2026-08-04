// Deterministic identities used by the scenario driver.
//
// These are EXACTLY the identities the app's own local-dev login produces, so
// the principal the browser is authenticated as is the principal the driver can
// fund and seat. See src/cleardeck_frontend/src/lib/auth.js -> createDevIdentity.

import { Ed25519KeyIdentity } from '@dfinity/identity';
import { DEV_PLAYER_SEED } from './config.mjs';

/**
 * Byte-for-byte copy of the app's createDevIdentity(seed).
 * @param {string} seed
 */
export function devIdentityFromSeed(seed) {
  const encoder = new TextEncoder();
  const seedBytes = encoder.encode(seed.padEnd(32, '0').slice(0, 32));
  return Ed25519KeyIdentity.generate(seedBytes);
}

const cache = new Map();

/**
 * The identity behind the app's "Dev Login -> Player N" button.
 * @param {number} n 1-based player number
 */
export function devPlayer(n) {
  if (!cache.has(n)) cache.set(n, devIdentityFromSeed(DEV_PLAYER_SEED(n)));
  return cache.get(n);
}

/** @param {number} n */
export function devPlayerPrincipal(n) {
  return devPlayer(n).getPrincipal().toText();
}
