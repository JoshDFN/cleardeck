// Centralized IC network + identity configuration.
//
// Single source of truth for:
//   - which NETWORK this bundle was built for (compiled in, not sniffed),
//   - the Internet Identity provider URL,
//   - the agent gateway host,
//   - the local replica gateway port,
//   - the mainnet canister ids, as DISPLAY-ONLY text.
//
// Previously the II URL and the isMainnet() helper were duplicated across
// auth.js, canisters.js, oisy.js and WalletButton.svelte, and had already
// drifted (oisy.js used icp-api.io while the rest used ic0.app).
//
// Modern DFINITY guidance (icskills):
//   - Internet Identity provider = https://id.ai   (II v2; replaces identity.internetcomputer.org)
//   - Agent gateway host         = https://icp-api.io  (replaces ic0.app)
//
// IMPORTANT: the II URL MUST include the /authorize path. The bare origin
// (https://id.ai) opens id.ai account management and the AuthClient delegation
// handshake never completes, locking users out of their funded session.

// ---------------------------------------------------------------------------
// Build target
// ---------------------------------------------------------------------------
//
// docs/DEFECTS.md T-01: the bundle used to decide "am I local?" purely by
// sniffing window.location.hostname, while the canister ids it talked to came
// from a fallback chain that ended at the repo-root .env, which holds the
// MAINNET ids. A local dev build therefore pointed a dev UI at the live
// fund-holding canisters.
//
// The build target is now COMPILED IN by vite.config.js, which refuses to build
// at all unless it is stated explicitly (DFX_NETWORK / ICP_NETWORK = local|ic).
// The hostname sniff survives only as a last-resort fallback for a bundle built
// by some other toolchain; it can no longer decide which canisters are wired.

const MAINNET_HOSTNAMES = ['icp0.io', 'ic0.app', 'internetcomputer.org'];

/**
 * Reads ONE build-time value.
 *
 * Two constraints meet here and both are load-bearing:
 *
 *   1. The expression handed in must be a LITERAL member access
 *      (`() => import.meta.env.VITE_X`, `() => process.env.CANISTER_ID_X`),
 *      because vite's `define` substitutes those exact texts at build time.
 *      `import.meta.env[name]` is never substituted and silently reads
 *      `undefined` in the bundle -- which for the deposit trust root would mean
 *      an EMPTY trusted set that nobody notices until money moves.
 *   2. It must not throw outside a bundler. `import.meta.env` does not exist in
 *      bare node and `process` does not exist in a browser, and this module is
 *      imported by `trustedTables.js`, which is imported by the deposit
 *      derivation, which is run in node by its gates
 *      (`tests/money_safety/tests/deposit_trust_root.rs`). A module that cannot
 *      be loaded outside vite is a module its gate cannot execute.
 *
 * @param {() => unknown} read
 * @returns {string|undefined}
 */
export function buildValue(read) {
  try {
    const v = read();
    return typeof v === 'string' && v.length > 0 ? v : undefined;
  } catch {
    return undefined;
  }
}

/** @typedef {'local'|'ic'} IcNetwork */

/** @returns {IcNetwork|null} the target compiled in at build time, if any. */
function compiledNetwork() {
  const raw = buildValue(() => import.meta.env.VITE_ICP_NETWORK)
    || buildValue(() => import.meta.env.DFX_NETWORK)
    || buildValue(() => process.env.DFX_NETWORK);
  if (raw === 'ic' || raw === 'local') return raw;
  return null;
}

/** @returns {boolean} true when the page is served from an IC mainnet origin. */
function servedFromMainnetOrigin() {
  return typeof window !== 'undefined' &&
    MAINNET_HOSTNAMES.some((h) => window.location.hostname.includes(h));
}

/**
 * The network this bundle talks to. Compiled in when known; otherwise inferred
 * from the serving origin so an unlabelled bundle still behaves sanely.
 * @type {IcNetwork}
 */
export const NETWORK = compiledNetwork() || (servedFromMainnetOrigin() ? 'ic' : 'local');

/** True when this bundle was explicitly built for mainnet. */
export const IS_MAINNET_BUILD = compiledNetwork() === 'ic';

/** @returns {boolean} true when this bundle talks to IC mainnet. */
export function isMainnet() {
  return NETWORK === 'ic';
}

/** @returns {boolean} true in local development. */
export function isLocal() {
  return NETWORK === 'local';
}

// ---------------------------------------------------------------------------
// Hosts
// ---------------------------------------------------------------------------

/**
 * Local replica gateway port. docs/DEFECTS.md T-03: this was hardcoded to 4943,
 * but this project's managed network is pinned to 8077 in icp.yaml (8000 belongs
 * to another project on the same machine), so an unmodified local build pointed
 * its agent at a port nothing was listening on. It now comes from the build
 * environment, the same place the canister ids come from.
 */
export const LOCAL_GATEWAY_PORT =
  Number(buildValue(() => import.meta.env.VITE_LOCAL_GATEWAY_PORT)) || 4943;

/** Local replica host used for both the agent and (with a canister-id prefix) II. */
export const LOCAL_HOST =
  buildValue(() => import.meta.env.VITE_LOCAL_HOST) || `http://127.0.0.1:${LOCAL_GATEWAY_PORT}`;

// Mainnet agent gateway host (env-overridable for rollback).
export const IC_HOST = buildValue(() => import.meta.env.VITE_IC_HOST) || 'https://icp-api.io';

// Mainnet Internet Identity provider (env-overridable for rollback). MUST end in /authorize.
export const II_URL = buildValue(() => import.meta.env.VITE_II_URL) || 'https://id.ai/authorize';

/**
 * Agent host for the current environment.
 * @returns {string} IC_HOST on mainnet, the local replica otherwise.
 */
export function agentHost() {
  return isMainnet() ? IC_HOST : LOCAL_HOST;
}

// ---------------------------------------------------------------------------
// Mainnet canister ids, DISPLAY ONLY
// ---------------------------------------------------------------------------
//
// These are the live, fund-holding ClearDeck canisters. They are listed here so
// the "Verify the Code" panel can show a user which canisters to audit on
// mainnet, that display is legitimate and deliberately kept.
//
// They are NEVER used to wire an actor. canisters.js resolves its ids from the
// build environment and treats any of these values as a build error on a local
// build. Keep this list in sync with .icp/data/mappings/ic.ids.json.
export const MAINNET_CANISTER_IDS = Object.freeze({
  lobby: 'kpfcd-kyaaa-aaaaj-qor3a-cai',
  history: 'kggj7-4qaaa-aaaaj-qor2q-cai',
  frontend: 'kbhpl-riaaa-aaaaj-qor2a-cai',
  table_1: 'kieex-haaaa-aaaaj-qor3q-cai',
  table_2: 'lfkaz-iiaaa-aaaaj-qor4a-cai',
  table_3: 'lclgn-fqaaa-aaaaj-qor4q-cai',
  btc_table_1: 'qrhly-eaaaa-aaaaj-qousa-cai',
});

/** Every mainnet id, for the "did a local build wire a mainnet canister?" check. */
export const MAINNET_ID_LIST = Object.freeze(Object.values(MAINNET_CANISTER_IDS));

/**
 * The mainnet canisters that can legitimately hold a player's deposit: the
 * tables, and nothing else. This is the TRUST ROOT for a mainnet deposit
 * address (see `trustedTables.js`), which is why it is a frozen literal here
 * rather than anything read at runtime.
 *
 * docs/SECURITY-FINDINGS.md FINDING 42. The list above was already written down
 * "so a build can check what it is wired to", and no deposit path consulted it,
 * so the deposit address was rooted in whatever `lobby.get_tables()` said.
 */
export const MAINNET_TABLE_IDS = Object.freeze([
  MAINNET_CANISTER_IDS.table_1,
  MAINNET_CANISTER_IDS.table_2,
  MAINNET_CANISTER_IDS.table_3,
  MAINNET_CANISTER_IDS.btc_table_1,
]);

/**
 * @param {unknown} id
 * @returns {boolean} true if `id` is one of the live fund-holding canisters.
 */
export function isMainnetCanisterId(id) {
  return typeof id === 'string' && MAINNET_ID_LIST.includes(id);
}

/**
 * The `icp canister status` command for the network this bundle actually talks
 * to. docs/DEFECTS.md T-02: a locally-wired build used to offer a Copy button
 * that handed the user `icp canister status qrhly-… -e ic`, a mainnet command
 * from a local dev build. The command now comes from the same runtime config as
 * the wiring, so it can never disagree with it.
 *
 * @param {string|undefined} canisterId id to inspect; falls back to a sensible default
 * @returns {string} a copy-pasteable icp command for THIS build's network
 */
export function statusCommandFor(canisterId) {
  const env = isMainnet() ? 'ic' : 'local';
  const id = canisterId || (isMainnet() ? MAINNET_CANISTER_IDS.lobby : '<LOCAL_CANISTER_ID>');
  return `icp canister status ${id} -e ${env}`;
}
