// Resolve the LOCAL canister IDs. Never read .icp/data/mappings/ic.ids.json —
// that file maps names to live mainnet canisters holding real funds.

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { REPO_ROOT } from './config.mjs';

const CANDIDATE_MAPPINGS = [
  path.join(REPO_ROOT, '.icp', 'cache', 'mappings', 'local.ids.json'),
  path.join(REPO_ROOT, '.icp', 'data', 'mappings', 'local.ids.json'),
];

// The denylist is NOT a hardcoded copy: it is derived from the mapping the CLI
// itself resolves for `-e ic`, so adding a mainnet canister to the project
// automatically protects it here. scripts/dev.sh reads the same file, so there is
// exactly one list in the repo.
const IC_IDS_JSON = path.join(REPO_ROOT, '.icp', 'data', 'mappings', 'ic.ids.json');

/** @returns {string[]} full mainnet canister ids, from ic.ids.json. */
function mainnetIds() {
  let raw;
  try {
    raw = fs.readFileSync(IC_IDS_JSON, 'utf8');
  } catch (e) {
    throw new Error(
      `Cannot read the mainnet canister mapping ${IC_IDS_JSON} (${e.message}). ` +
        'The harness refuses to run without the denylist it uses to keep away from mainnet.',
    );
  }
  const ids = Object.values(JSON.parse(raw)).map(String).filter(Boolean);
  if (ids.length === 0) {
    throw new Error(`${IC_IDS_JSON} lists no canisters; refusing to run with an empty denylist.`);
  }
  return ids;
}

const MAINNET_IDS = mainnetIds();
// Kept for messages and for the substring scan of arbitrary command arguments.
const MAINNET_PREFIXES = MAINNET_IDS.map((id) => `${id.split('-')[0]}-`);

/** Throws if an id is (or begins like) one of the mainnet ClearDeck canisters. */
export function assertNotMainnet(id, label = 'canister id') {
  if (typeof id !== 'string') throw new Error(`${label} is not a string: ${id}`);
  for (const p of MAINNET_PREFIXES) {
    if (id.startsWith(p)) {
      throw new Error(
        `REFUSING to use ${label}=${id}: that is a ClearDeck MAINNET canister holding real funds.`,
      );
    }
  }
  return id;
}

/**
 * @returns {Record<string,string>} name -> local canister id
 */
export function readLocalIds() {
  for (const file of CANDIDATE_MAPPINGS) {
    if (!fs.existsSync(file)) continue;
    const raw = fs.readFileSync(file, 'utf8');
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      throw new Error(`Local canister id mapping ${file} is not valid JSON: ${e.message}`);
    }
    const out = {};
    for (const [name, id] of Object.entries(parsed)) {
      out[name] = assertNotMainnet(String(id), `local id for "${name}"`);
    }
    if (Object.keys(out).length === 0) continue;
    return out;
  }
  throw new Error(
    `No local canister id mapping found. Looked in:\n  ${CANDIDATE_MAPPINGS.join('\n  ')}\n` +
      'Deploy the backend to the local network first.',
  );
}

/**
 * Resolves which local icp identity ACTUALLY controls a local canister.
 *
 * The harness used to hardcode `cd-local-deployer` as the controller. That is an
 * assumption, not a fact: `./scripts/dev.sh local-up` runs `icp deploy` with no
 * `--identity`, so the canisters end up controlled by whatever identity happens
 * to be the machine's current default — which on this machine is a completely
 * unrelated project's identity. Every controller-only call (`reset_table`, the
 * thing that makes a screenshot run idempotent) then fails, and the lobby ends up
 * with zero registered tables.
 *
 * So: read the real controller list off the canister and pick a local identity
 * that is in it. Preference order is `preferred` first, then the rest of the
 * candidates, then the current default identity.
 *
 * @param {string} canisterId a local canister to read the controller list from
 * @param {object} [opts]
 * @param {string} [opts.preferred] identity name to use if it qualifies
 * @param {string[]} [opts.candidates] other identity names to consider
 * @returns {{identity:string|null, controllers:string[], checked:Array<{identity:string,principal:string,isController:boolean}>}}
 */
export function resolveControllerIdentity(canisterId, { preferred, candidates = [] } = {}) {
  assertNotMainnet(canisterId, 'canister id for controller lookup');
  const status = icp(['canister', 'status', canisterId, '-e', 'local']);
  const line = status.split('\n').find((l) => /^\s*Controllers:/.test(l)) || '';
  const controllers = line
    .replace(/^\s*Controllers:\s*/, '')
    .split(/[\s,]+/)
    .map((s) => s.trim())
    .filter(Boolean);

  const names = [];
  for (const n of [preferred, ...candidates]) {
    if (n && !names.includes(n)) names.push(n);
  }

  const checked = [];
  for (const name of names) {
    let principal;
    try {
      principal = icp(['identity', 'principal', '--identity', name]).trim();
    } catch {
      continue; // identity does not exist on this machine
    }
    const isController = controllers.includes(principal);
    checked.push({ identity: name, principal, isController });
    if (isController) return { identity: name, controllers, checked };
  }

  // Last resort: the current default identity, whatever it is called.
  try {
    const principal = icp(['identity', 'principal']).trim();
    const isController = controllers.includes(principal);
    checked.push({ identity: '(default)', principal, isController });
    if (isController) return { identity: null, controllers, checked };
  } catch { /* nothing else to try */ }

  return { identity: undefined, controllers, checked };
}

/**
 * The frontend asset canister is created by `icp deploy -e local frontend`, so
 * it may not be in the mapping yet. `icp canister list` prints names only, so we
 * re-read the mapping after the deploy instead of parsing CLI output.
 */
export function requireId(ids, name) {
  const id = ids[name];
  if (!id) {
    throw new Error(
      `Local canister "${name}" is not deployed (missing from the local id mapping). ` +
        'Run the deploy step first.',
    );
  }
  return assertNotMainnet(id, `local id for "${name}"`);
}

/**
 * Runs `icp` with a hard guarantee that the target is the local environment.
 *
 * Refuses:
 *   - `-e ic` / `--environment ic` in any position
 *   - any argument containing a known ClearDeck mainnet canister prefix
 *   - a leaked `ICP_ENVIRONMENT` / `ICP_NETWORK` from the parent environment
 *     (both are stripped from the child, so the explicit `-e local` decides)
 */
export function icp(args, { cwd = REPO_ROOT, input, timeoutMs = 180_000 } = {}) {
  const flat = args.map(String);
  for (let i = 0; i < flat.length; i += 1) {
    if ((flat[i] === '-e' || flat[i] === '--environment') && flat[i + 1] === 'ic') {
      throw new Error(`REFUSING to run an icp command against -e ic: icp ${flat.join(' ')}`);
    }
  }
  for (const p of MAINNET_PREFIXES) {
    if (flat.some((a) => a.includes(p))) {
      throw new Error(`REFUSING to run an icp command referencing mainnet canister prefix ${p}`);
    }
  }
  const env = { ...process.env };
  delete env.ICP_ENVIRONMENT;
  delete env.ICP_NETWORK;
  return execFileSync('icp', flat, {
    cwd,
    input,
    encoding: 'utf8',
    timeout: timeoutMs,
    maxBuffer: 64 * 1024 * 1024,
    env,
  });
}
