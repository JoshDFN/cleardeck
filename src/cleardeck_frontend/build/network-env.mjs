// Build-time network + canister-id resolution for the ClearDeck frontend bundle.
//
// THE DEFECT THIS FILE EXISTS TO KILL (docs/DEFECTS.md T-01)
// --------------------------------------------------------------------------
// vite.config.js used to open with one line:
//
//     dotenv.config({ path: '../../.env' });
//
// The repo-root `.env` holds the MAINNET canister ids, and canisters.js ended its
// id fallback chain at `CANISTER_ID_LOBBY` / `CANISTER_ID_HISTORY`. So a bare
// `npm run build` produced a bundle that pointed a local dev UI at the live
// canisters custodying real ICP and ckBTC, silently, with nothing in the build
// output to say so.
//
// The fix is not a warning. It is: the build cannot express that state.
//
//   * The target network must be stated explicitly (DFX_NETWORK / ICP_NETWORK).
//     No default. A build that does not say where it is going does not happen.
//   * The repo-root `.env` is loaded ONLY for an `ic` build. That file is the
//     mainnet id list; a local build never sees it.
//   * A `local` build must supply local ids, and they are checked against the
//     project's own mainnet mapping (.icp/data/mappings/ic.ids.json). A mainnet
//     id on a local build aborts the build.
//
// Every message here is written for the person whose build just failed.

import fs from 'node:fs';
import path from 'node:path';
import dotenv from 'dotenv';

/** Networks a bundle may be built for. */
export const NETWORKS = Object.freeze(['local', 'ic']);

const HOW_TO_BUILD = `
How to build:

  local dev   ./scripts/dev.sh local-up          (builds + deploys with local ids)
              DFX_NETWORK=local \\
                VITE_CANISTER_ID_LOBBY=<local>  \\
                VITE_CANISTER_ID_HISTORY=<local> npm run build

  mainnet     DFX_NETWORK=ic npm run build       (ids come from the repo-root .env)

Why this is mandatory: the repo-root .env holds the MAINNET canister ids, which
custody real ICP and ckBTC. A build that does not state its target used to fall
back to those ids silently. See docs/DEFECTS.md T-01.`;

/**
 * Reads the project's mainnet canister ids. This file is committed and is what
 * `icp -e ic` itself resolves, so the denylist is never a hand-maintained copy.
 *
 * @param {string} repoRoot
 * @returns {{ids: string[], source: string}}
 */
export function readMainnetIds(repoRoot) {
  const file = path.join(repoRoot, '.icp', 'data', 'mappings', 'ic.ids.json');
  let raw;
  try {
    raw = fs.readFileSync(file, 'utf8');
  } catch (e) {
    throw new Error(
      `Cannot read the mainnet canister mapping ${file} (${e.message}).\n` +
        'The frontend build refuses to run without it: it is the denylist that ' +
        'stops a local build from being wired to the live fund-holding canisters.',
    );
  }
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    throw new Error(`${file} is not valid JSON: ${e.message}`);
  }
  const ids = Object.values(parsed).map(String).filter(Boolean);
  if (ids.length === 0) {
    throw new Error(`${file} lists no canisters; refusing to build with an empty denylist.`);
  }
  return { ids, source: path.relative(repoRoot, file) };
}

/**
 * @param {NodeJS.ProcessEnv} env
 * @returns {'local'|'ic'} the declared target network
 * @throws {Error} when the target is absent or not one of NETWORKS
 */
export function requireNetwork(env) {
  const raw = env.ICP_NETWORK || env.DFX_NETWORK || env.VITE_ICP_NETWORK;
  if (!raw) {
    throw new Error(
      'ABORT: the ClearDeck frontend build does not know which network it is for.\n' +
        'Set DFX_NETWORK (or ICP_NETWORK) to "local" or "ic".\n' +
        HOW_TO_BUILD,
    );
  }
  if (!NETWORKS.includes(raw)) {
    throw new Error(
      `ABORT: DFX_NETWORK="${raw}" is not a ClearDeck network. ` +
        `Use one of: ${NETWORKS.join(', ')}.\n${HOW_TO_BUILD}`,
    );
  }
  return /** @type {'local'|'ic'} */ (raw);
}

/**
 * Canister ids the bundle genuinely needs. Table ids are fetched from the lobby
 * at runtime, so only these two are wired at build time.
 */
const REQUIRED_CANISTERS = Object.freeze(['LOBBY', 'HISTORY']);

/**
 * @param {NodeJS.ProcessEnv} env
 * @param {string} name canister name, e.g. "LOBBY"
 * @returns {string|undefined}
 */
function idFromEnv(env, name) {
  return env[`VITE_CANISTER_ID_${name}`] || env[`CANISTER_ID_${name}`] || undefined;
}

/**
 * Prepares `process.env` for the vite build and validates the result.
 *
 * Mutation note: this deliberately writes into the live `process.env` object,
 * because that is the only channel vite-plugin-environment reads. Everything
 * else here is pure; the returned proof object is a fresh value.
 *
 * @param {object} opts
 * @param {string} opts.repoRoot repository root (…/cleardeck)
 * @param {NodeJS.ProcessEnv} [opts.env] environment to read and populate
 * @returns {{network:'local'|'ic', ids:Record<string,string>, envFileLoaded:string|null,
 *            denylistSource:string, denylistSize:number}}
 * @throws {Error} with an actionable message on any unsafe or ambiguous build
 */
export function prepareBuildEnv({ repoRoot, env = process.env }) {
  const network = requireNetwork(env);
  const { ids: mainnetIds, source: denylistSource } = readMainnetIds(repoRoot);

  // The repo-root .env IS the mainnet id list. Load it only for a mainnet build.
  // dotenv never overrides a variable already present, so an explicit id wins.
  let envFileLoaded = null;
  if (network === 'ic') {
    const envFile = path.join(repoRoot, '.env');
    if (fs.existsSync(envFile)) {
      dotenv.config({ path: envFile, processEnv: env });
      envFileLoaded = path.relative(repoRoot, envFile);
    }
  }

  /** @type {Record<string,string>} */
  const resolved = {};
  const missing = [];
  const wrongNetwork = [];

  for (const name of REQUIRED_CANISTERS) {
    const id = idFromEnv(env, name);
    if (!id) {
      missing.push(name);
      continue;
    }
    if (network !== 'ic' && mainnetIds.includes(id)) {
      wrongNetwork.push(`${name}=${id}`);
      continue;
    }
    resolved[name] = id;
  }

  if (missing.length) {
    throw new Error(
      `ABORT: building for network "${network}" but no canister id for ` +
        `${missing.join(', ')}.\n` +
        'The bundle would render fine and talk to nothing.\n' +
        HOW_TO_BUILD,
    );
  }

  if (wrongNetwork.length) {
    throw new Error(
      `ABORT: building for network "${network}" but these are MAINNET ClearDeck ` +
        `canisters, which custody real ICP and ckBTC:\n  ${wrongNetwork.join('\n  ')}\n` +
        `(denylist read from ${denylistSource})\n` +
        'A local dev build must never be wired to them. This is exactly the ' +
        'silent failure docs/DEFECTS.md T-01 describes.\n' +
        HOW_TO_BUILD,
    );
  }

  // Publish the resolved values under both prefixes so canisters.js resolves the
  // same ids no matter which one it reads first, and stamp the target network
  // into the bundle so ic-config.js never has to guess from window.location.
  for (const [name, id] of Object.entries(resolved)) {
    env[`VITE_CANISTER_ID_${name}`] = id;
    env[`CANISTER_ID_${name}`] = id;
  }
  env.VITE_ICP_NETWORK = network;
  env.DFX_NETWORK = network;

  // `CANISTER_ID` alone is the asset-canister id dfx used to export; on this
  // checkout it is the MAINNET frontend id. Nothing in the app reads it, and
  // leaving it in the environment only invites a future "sensible default".
  if (network !== 'ic') {
    delete env.CANISTER_ID;
    delete env.CANISTER_CANDID_PATH;
  }

  return {
    network,
    ids: resolved,
    envFileLoaded,
    denylistSource,
    denylistSize: mainnetIds.length,
  };
}
