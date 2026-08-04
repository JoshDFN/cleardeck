// Actor factories for the local replica.
//
// The driver talks to the SAME canisters the browser talks to. Nothing here
// fakes a response: every value that ends up in a screenshot came out of a
// canister on the local replica.

import { Actor, HttpAgent } from '@dfinity/agent';
import { pathToFileURL } from 'node:url';
import path from 'node:path';
import { DECLARATIONS_DIR, FRONTEND_DIR, GATEWAY_ORIGIN } from './config.mjs';
import { assertNotMainnet } from './ids.mjs';

const agentCache = new Map();

/**
 * @param {import('@dfinity/agent').Identity|undefined} identity
 * @returns {Promise<HttpAgent>}
 */
export async function agentFor(identity) {
  const key = identity ? identity.getPrincipal().toText() : 'anonymous';
  if (agentCache.has(key)) return agentCache.get(key);

  const agent = await HttpAgent.create({
    host: GATEWAY_ORIGIN,
    identity,
    // Matches the app's own agent options (canisters.js).
    verifyQuerySignatures: false,
    shouldFetchRootKey: true,
  });
  agentCache.set(key, agent);
  return agent;
}

const idlCache = new Map();

/** Loads an idlFactory straight out of src/declarations (same file the app imports). */
async function loadIdl(declarationName, fileName) {
  const key = `${declarationName}/${fileName}`;
  if (idlCache.has(key)) return idlCache.get(key);
  const file = path.join(DECLARATIONS_DIR, declarationName, fileName);
  const mod = await import(pathToFileURL(file).href);
  if (!mod.idlFactory) throw new Error(`${file} does not export idlFactory`);
  idlCache.set(key, mod.idlFactory);
  return mod.idlFactory;
}

export const tableIdl = () => loadIdl('table_1', 'table_1.did.js');
export const lobbyIdl = () => loadIdl('lobby', 'lobby.did.js');
export const historyIdl = () => loadIdl('history', 'history.did.js');

/** The ledger IDL the app itself uses. */
export async function ledgerIdl() {
  const file = path.join(FRONTEND_DIR, 'src', 'lib', 'ledger.did.js');
  const mod = await import(pathToFileURL(file).href);
  return mod.idlFactory;
}

/**
 * @param {() => Promise<any>} idlLoader
 * @param {string} canisterId
 * @param {import('@dfinity/agent').Identity} [identity]
 */
export async function actorFor(idlLoader, canisterId, identity) {
  assertNotMainnet(canisterId, 'actor canister id');
  const [idlFactory, agent] = await Promise.all([idlLoader(), agentFor(identity)]);
  return Actor.createActor(idlFactory, { agent, canisterId });
}

export const tableActor = (canisterId, identity) => actorFor(tableIdl, canisterId, identity);
export const lobbyActor = (canisterId, identity) => actorFor(lobbyIdl, canisterId, identity);
export const historyActor = (canisterId, identity) => actorFor(historyIdl, canisterId, identity);
export const ledgerActor = (canisterId, identity) => actorFor(ledgerIdl, canisterId, identity);

/** Unwraps a candid `variant { Ok; Err }`. */
export function unwrap(result, what = 'call') {
  if (result && typeof result === 'object') {
    if ('Ok' in result) return result.Ok;
    if ('ok' in result) return result.ok;
    if ('Err' in result) throw new Error(`${what} failed: ${JSON.stringify(result.Err)}`);
    if ('err' in result) throw new Error(`${what} failed: ${JSON.stringify(result.err)}`);
  }
  return result;
}

/** Unwraps a candid `opt T` (which arrives as [] | [T]). */
export function optional(value) {
  if (Array.isArray(value)) return value.length ? value[0] : null;
  return value ?? null;
}

/** The single-key name of a candid variant, e.g. { PreFlop: null } -> 'PreFlop'. */
export function variantKey(v) {
  if (!v || typeof v !== 'object') return null;
  const keys = Object.keys(v);
  return keys.length ? keys[0] : null;
}
