// Builds the SvelteKit frontend with the LOCAL canister IDs wired in, proves the
// IDs actually landed in the bundle, then deploys the asset canister.
//
// THE TRAP THIS GUARDS AGAINST: `npm run build` with no canister IDs in the
// environment produces a bundle that renders fine and talks to nothing —
// `lobbyCanisterId` ends up undefined and every call fails at runtime. The
// assertion below refuses to continue unless the local lobby id is in the built
// JavaScript.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { ENV, FRONTEND_DIR, FRONTEND_DIST, ICP_LEDGER_CANISTER_ID, REPO_ROOT } from './config.mjs';
import { assertNotMainnet, icp, readLocalIds } from './ids.mjs';

/**
 * Environment the vite build needs.
 *
 * vite.config.js exposes process.env keys with the CANISTER_ / DFX_ / VITE_
 * prefixes via vite-plugin-environment, and dotenv-loads ../../.env (which on
 * this checkout holds MAINNET ids). dotenv does not override variables that are
 * already in process.env, so the values below win.
 *
 * canisters.js reads VITE_CANISTER_ID_LOBBY first, then CANISTER_ID_LOBBY, so we
 * set both to the local id and leave nothing to precedence luck.
 */
export function buildEnvFor(ids) {
  const env = {
    ...process.env,
    DFX_NETWORK: 'local',
    NODE_ENV: 'production',
    // The two IDs the bundle genuinely needs (table ids come from the lobby).
    VITE_CANISTER_ID_LOBBY: ids.lobby,
    VITE_CANISTER_ID_HISTORY: ids.history,
    CANISTER_ID_LOBBY: ids.lobby,
    CANISTER_ID_HISTORY: ids.history,
    // The local ICP ledger really is at the mainnet ledger id on this replica,
    // so the wallet balance in the header shows a real on-chain balance.
    CANISTER_ID_LEDGER: ICP_LEDGER_CANISTER_ID,
    // Table ids are informational in the bundle but keep them local-consistent.
    CANISTER_ID_TABLE_1: ids.table_1,
    CANISTER_ID_TABLE_2: ids.table_2,
    CANISTER_ID_TABLE_3: ids.table_3,
    CANISTER_ID_BTC_TABLE_1: ids.btc_table_1,
  };
  if (ids.frontend) env.CANISTER_ID_FRONTEND = ids.frontend;
  // Never leak the mainnet asset-canister id as "the" canister id.
  delete env.CANISTER_ID;
  delete env.CANISTER_CANDID_PATH;
  return env;
}

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

/**
 * Fails loudly unless the local lobby id is present in the built bundle.
 * @returns {{files:string[], bytes:number}}
 */
export function assertBundleWiredTo(lobbyId) {
  assertNotMainnet(lobbyId, 'lobby id checked in bundle');
  if (!fs.existsSync(FRONTEND_DIST)) {
    throw new Error(`Frontend dist does not exist: ${FRONTEND_DIST}`);
  }
  const files = walk(FRONTEND_DIST).filter((f) => /\.(js|html|css)$/.test(f));
  const hits = [];
  let bytes = 0;
  for (const f of files) {
    const text = fs.readFileSync(f, 'utf8');
    bytes += text.length;
    if (text.includes(lobbyId)) hits.push(path.relative(REPO_ROOT, f));
  }
  if (hits.length === 0) {
    throw new Error(
      `ABORT: the built bundle in ${path.relative(REPO_ROOT, FRONTEND_DIST)} does not contain the ` +
        `local lobby canister id ${lobbyId}. The build did not pick up the canister IDs, so the ` +
        'app would render fine and talk to nothing. Fix the build environment before continuing.',
    );
  }
  return { files: hits, bytes };
}

/** `npm run build` in the frontend workspace with local IDs in the environment. */
export function buildFrontend({ log = console.log } = {}) {
  const ids = readLocalIds();
  for (const required of ['lobby', 'history']) {
    if (!ids[required]) {
      throw new Error(`Local canister "${required}" is not deployed; cannot build the frontend.`);
    }
  }
  log(`  building frontend with lobby=${ids.lobby} history=${ids.history}`);
  execFileSync('npm', ['run', 'build'], {
    cwd: FRONTEND_DIR,
    env: buildEnvFor(ids),
    stdio: 'inherit',
    timeout: 15 * 60_000,
  });
  const proof = assertBundleWiredTo(ids.lobby);
  log(`  bundle wired: local lobby id found in ${proof.files.length} built file(s)`);
  return { ids, proof };
}

/** Deploys the asset canister to the local network only. */
export function deployFrontend({ log = console.log } = {}) {
  log('  icp deploy -e local frontend');
  const out = icp(['deploy', '-e', ENV, 'frontend'], { timeoutMs: 15 * 60_000 });
  const ids = readLocalIds();
  if (!ids.frontend) {
    throw new Error(
      'Deploy finished but no local id for "frontend" appeared in the id mapping.\n' + out,
    );
  }
  log(`  frontend asset canister: ${ids.frontend}`);
  return { output: out, frontendId: ids.frontend };
}
