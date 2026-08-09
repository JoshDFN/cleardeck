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
import {
  ENV, FRONTEND_DIR, FRONTEND_DIST, GATEWAY_HOST, GATEWAY_PORT, ICP_LEDGER_CANISTER_ID, REPO_ROOT,
} from './config.mjs';
import { assertNotMainnet, icp, readLocalIds, resolveControllerIdentity } from './ids.mjs';
import { CONTROLLER_IDENTITY, FUNDER_IDENTITIES } from './config.mjs';

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
    // THE GATEWAY PORT THE BUNDLE WILL TALK TO (docs/DEFECTS.md T-14).
    //
    // `ic-config.js` has read this since T-03 and defaults to 4943 when it is
    // absent; `auth.js` derives the local Internet Identity origin from the same
    // value. This function is THE ONLY WIRED BUILD PATH for the local app, and it
    // never set it -- so every bundle ever deployed to the local asset canister
    // pointed its agent at 127.0.0.1:4943 while this project's gateway is pinned
    // to 8077 in icp.yaml. The screenshot harness papered over that with a
    // reverse proxy listening on 4943, which is a thing NO USER HAS: a human
    // opening http://<frontend-id>.localhost:8077/ got a raw fetch stack trace
    // and "The lobby canister is reporting no tables" while the lobby was
    // reporting three. T-03 was recorded as fixed by "it now comes from the build
    // environment"; the build environment is this object, and it did not supply it.
    VITE_LOCAL_GATEWAY_PORT: String(GATEWAY_PORT),
    // Same origin as the page itself, so the agent call is not cross-origin.
    // Without this the default is `http://127.0.0.1:<port>` while the page is
    // served from `http://<canister-id>.localhost:<port>`, which is a different
    // host and puts every ingress call behind CORS for no reason.
    VITE_LOCAL_HOST: `http://${GATEWAY_HOST}:${GATEWAY_PORT}`,
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

/**
 * THE BUNDLE MUST TALK TO THE GATEWAY THIS PROJECT ACTUALLY RUNS (docs/DEFECTS.md T-14).
 *
 * `assertBundleWiredTo` above proves the bundle carries the right CANISTER IDS.
 * It was green for every build that shipped T-14: the ids were right and the PORT
 * was 4943 while the gateway is on 8077, so the app was wired to the correct
 * canisters at an address nothing was listening on. The screenshot harness hid it
 * behind a reverse proxy, so the only person who ever saw the failure was a human
 * opening the URL, and there was no human in the loop.
 *
 * So the port is asserted the same way the ids are: read out of the built files.
 * `ic-config.js` compiles `LOCAL_GATEWAY_PORT` and `LOCAL_HOST` in from the build
 * environment, so both appear literally in the bundle.
 *
 * @param {number} port
 * @returns {{files:string[]}}
 */
export function assertBundleLocalGateway(port) {
  const files = walk(FRONTEND_DIST).filter((f) => f.endsWith('.js'));
  const wanted = `:${port}`;
  const hits = files.filter((f) => fs.readFileSync(f, 'utf8').includes(wanted));
  if (hits.length === 0) {
    throw new Error(
      `ABORT: the built bundle in ${path.relative(REPO_ROOT, FRONTEND_DIST)} contains no reference ` +
        `to the local gateway port ${port}. VITE_LOCAL_GATEWAY_PORT did not reach the build, so the ` +
        'app will point its agent at the ic-config.js default (4943) and a person opening it gets a ' +
        'fetch stack trace and an empty lobby. See docs/DEFECTS.md T-14.',
    );
  }
  return { files: hits.map((f) => path.relative(REPO_ROOT, f)) };
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
  const gateway = assertBundleLocalGateway(GATEWAY_PORT);
  log(`  bundle gateway: port ${GATEWAY_PORT} found in ${gateway.files.length} built file(s) (T-14)`);
  return { ids, proof, gateway };
}

/**
 * Deploys the asset canister to the local network only.
 *
 * THE IDENTITY IS RESOLVED, NOT ASSUMED -- docs/DEFECTS.md E-53, third instance.
 *
 * `icp deploy` with no `--identity` runs as whatever identity happens to be
 * selected on the machine, and its FIRST action is `update_settings` on the
 * canister, which is controller-only. On this machine the frontend asset
 * canister is controlled by `oms-port-trial` while the backend canisters are
 * controlled by `cyclepay-hotwallet` and the docs name `cd-local-deployer` --
 * three different identities for one stack, because each was created by
 * whatever was selected at the time. `./scripts/dev.sh local-up` therefore died
 * in step [6/6] with `IC0512 Only controllers of canister ... can call ic00
 * method update_settings`, AFTER a successful backend deploy, so the stack was
 * left with new canisters and a stale frontend.
 *
 * Same fix as the table driver: read the controller list off the canister and
 * use an identity that is actually in it. A fresh machine, where the canister
 * does not exist yet, has no controller list to read -- there the deploy runs
 * with the default identity and creates it, which is correct.
 */
export function deployFrontend({ log = console.log } = {}) {
  const existing = readLocalIds().frontend;
  const idFlag = [];
  if (existing) {
    const found = resolveControllerIdentity(existing, {
      preferred: CONTROLLER_IDENTITY,
      candidates: FUNDER_IDENTITIES,
    });
    if (found.identity === undefined) {
      throw new Error(
        `No local icp identity controls the frontend asset canister ${existing}.\n`
        + `  controllers on the canister: ${found.controllers.join(', ') || '(none reported)'}\n`
        + `  identities tried: ${found.checked.length} (every name in \`icp identity list\`)\n`
        + '`icp deploy` starts with a controller-only update_settings call, so it cannot '
        + 'redeploy this canister. Add one of your identities as a controller, or delete the '
        + 'canister and let the deploy recreate it.',
      );
    }
    if (found.identity) idFlag.push('--identity', found.identity);
    log(`  frontend controller: ${found.identity ?? '(default identity)'}`);
  }
  log(`  icp deploy -e local frontend${idFlag.length ? ` ${idFlag.join(' ')}` : ''}`);
  const out = icp(['deploy', '-e', ENV, 'frontend', ...idFlag], { timeoutMs: 15 * 60_000 });
  const ids = readLocalIds();
  if (!ids.frontend) {
    throw new Error(
      'Deploy finished but no local id for "frontend" appeared in the id mapping.\n' + out,
    );
  }
  log(`  frontend asset canister: ${ids.frontend}`);
  return { output: out, frontendId: ids.frontend };
}
