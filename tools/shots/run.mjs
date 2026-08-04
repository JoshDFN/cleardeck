#!/usr/bin/env node
// ClearDeck deterministic screenshot harness.
//
//   node tools/shots/run.mjs                 # everything
//   node tools/shots/run.mjs --scenes lobby,table-preflop
//   node tools/shots/run.mjs --viewports desktop
//   node tools/shots/run.mjs --skip-build    # reuse the existing dist + deploy
//
// It is safe to re-run: every scene resets its own table through the controller
// and re-seats from escrow, and the artifact directory is overwritten in place.
//
// LOCAL REPLICA ONLY. Nothing here can target mainnet: lib/ids.mjs refuses any
// icp invocation carrying `-e ic` or a known ClearDeck mainnet canister id.

import fs from 'node:fs';
import path from 'node:path';
import {
  APP_ORIGIN, GATEWAY_HOST, GATEWAY_ORIGIN, GATEWAY_PORT, HERO_PLAYER,
  PROXY_PORT, REPO_ROOT, VIEWPORTS,
} from './lib/config.mjs';
import { readLocalIds, requireId } from './lib/ids.mjs';
import { buildFrontend, deployFrontend } from './lib/frontend-build.mjs';
import { startGatewayProxy } from './lib/proxy.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { controllerIdentityInUse } from './lib/table-driver.mjs';
import { devPlayerPrincipal } from './lib/identities.mjs';
import {
  burst, clearLatestVariants, gitDirty, gitShortSha, runDirs, shoot, writeIndex, writeManifest,
} from './lib/capture.mjs';
import {
  getAppOrigin, launchBrowser, newContext, resetThirdPartyObservations, setAppOrigin,
  thirdPartyObservations, watchCanisterCalls, watchPage,
} from './lib/browser.mjs';
import { scenesByName } from './scenarios/index.mjs';

const log = (msg) => console.log(msg);

function parseArgs(argv) {
  const out = { scenes: [], viewports: ['desktop', 'mobile'], skipBuild: false, skipDeploy: false };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--scenes') out.scenes = String(argv[++i] || '').split(',').filter(Boolean);
    else if (a === '--viewports') out.viewports = String(argv[++i] || '').split(',').filter(Boolean);
    else if (a === '--skip-build') out.skipBuild = true;
    else if (a === '--skip-deploy') out.skipDeploy = true;
    else if (a === '--help' || a === '-h') out.help = true;
    else throw new Error(`Unknown argument: ${a}`);
  }
  for (const v of out.viewports) {
    if (!VIEWPORTS[v]) throw new Error(`Unknown viewport "${v}". Known: ${Object.keys(VIEWPORTS).join(', ')}`);
  }
  return out;
}

async function requireReplica() {
  const url = `${GATEWAY_ORIGIN}/api/v2/status`;
  try {
    const res = await fetch(url, { signal: AbortSignal.timeout(8000) });
    if (!res.ok && res.status !== 400) throw new Error(`HTTP ${res.status}`);
  } catch (e) {
    throw new Error(
      `The local replica gateway at ${GATEWAY_ORIGIN} is not answering (${e.message}).\n` +
        'This harness needs the shared managed local network for ClearDeck to be up.\n' +
        'It deliberately does NOT start or restart it (other agents share the replica).',
    );
  }
}

/** Maps table canister names to the display names the lobby registered. */
async function resolveTableNames(ids) {
  const lobby = await lobbyActor(ids.lobby);
  const tables = await lobby.get_tables();
  const byCanister = new Map();
  for (const t of tables) {
    const cid = optional(t.canister_id);
    if (cid) byCanister.set(cid.toText(), t.name);
  }
  const names = {};
  for (const key of ['table_1', 'table_2', 'table_3', 'btc_table_1']) {
    if (ids[key] && byCanister.has(ids[key])) names[key] = byCanister.get(ids[key]);
  }
  return { names, lobbyTableCount: tables.length };
}

/**
 * Condenses the volatile third-party log into something a reader can act on.
 *
 * `mode` is the honest label for every fiat figure in the run:
 *   live         a real quote, read at `observedAt`
 *   fixture      SHOTS_PRICE_FIXTURE=1; the number is a placeholder
 *   unavailable  the quote could not be read, so the app shows no price
 *   none         nothing on a volatile host was ever requested
 *
 * @param {Array<object>} observations
 */
function summariseVolatile(observations) {
  const modes = [...new Set(observations.map((o) => o.mode))];
  const mode = modes.length === 0 ? 'none' : modes.length === 1 ? modes[0] : `mixed(${modes.join('+')})`;
  return {
    mode,
    requestCount: observations.length,
    fiatFiguresAreLiveQuotes: mode === 'live',
    note:
      mode === 'live'
        ? 'every fiat figure in this run is a real quote, dated below'
        : mode === 'fixture'
          ? 'SHOTS_PRICE_FIXTURE=1 — every fiat figure in this run is a PLACEHOLDER, not a market price'
          : mode === 'unavailable'
            ? 'the price feed could not be read; the app renders its own no-price state, no stale number was shown'
            : 'no volatile third-party request was made in this run (no fiat figure on screen)',
    observations,
  };
}

async function runScene(scene, viewportName, ctx, browser, dirs) {
  const vp = VIEWPORTS[viewportName];
  const wantsVideo = Boolean(scene.video) && viewportName === 'desktop';
  const videoDir = wantsVideo ? path.join(dirs.shaDir, 'motion', scene.name) : undefined;
  if (videoDir) fs.mkdirSync(videoDir, { recursive: true });

  log(`\n▸ ${scene.name} [${viewportName}]  ${scene.title}`);

  // Retire every filename this (scene, viewport) could previously have claimed in
  // the stable mirror, BEFORE anything is written. A full run also wipes latest/
  // up front, but a partial run does not, and a verified PNG left over from an
  // earlier commit is exactly the artifact a reader would trust.
  clearLatestVariants(dirs.latestDir, scene.name, viewportName);

  const setup = await scene.setup(ctx);
  log(`  on-chain: ${setup.notes}`);

  const context = await newContext(browser, vp, { recordVideoDir: videoDir, log });
  const page = await context.newPage();
  const watchers = watchPage(page);
  const canisterCalls = watchCanisterCalls(page);

  let staged = {};
  let advanced = {};
  let verification = { verified: false, checks: {}, notes: 'not run' };
  let files = [];
  let videoPath = null;
  let error = null;

  try {
    staged = (await scene.stage(ctx, page)) || {};

    if (scene.advance) {
      const framesDir = path.join(dirs.shaDir, 'motion', scene.name, `frames-${viewportName}`);
      const burstFn = () =>
        burst(page, {
          dir: framesDir,
          frames: scene.motionFrames ?? 20,
          everyMs: scene.motionEveryMs ?? 150,
        });
      advanced = (await scene.advance(ctx, page, { burst: burstFn })) || {};
    }

    verification = await scene.verify(ctx, page);
    // A PNG under the canonical name is EVIDENCE: someone browsing artifacts/
    // will read `table-showdown-desktop.png` as proof of a verified showdown. A
    // soft `verified:false` (the action clock expired mid-staging, say) must
    // therefore never claim that name. Only a verified scene gets it; anything
    // else is written as UNVERIFIED-* so the filename carries the caveat even
    // when nobody opens INDEX.md.
    if (verification.verified) {
      files = await shoot(page, {
        scene: scene.name, viewport: viewportName, shaDir: dirs.shaDir, latestDir: dirs.latestDir,
      });
      log(`  ✓ state verified: ${verification.notes}`);
    } else {
      files = await shoot(page, {
        scene: `UNVERIFIED-${scene.name}`, viewport: viewportName,
        shaDir: dirs.shaDir, latestDir: dirs.latestDir,
      });
      log(`  ✗ STATE NOT VERIFIED: ${verification.notes}`);
      log('    written as UNVERIFIED-* (the canonical filename is reserved for verified state)');
    }
  } catch (e) {
    error = String(e.message || e);
    log(`  ✗ FAILED: ${error.split('\n')[0]}`);
    // Always keep a forensic still of whatever the page was showing.
    try {
      const failFile = path.join(dirs.shaDir, `FAILED-${scene.name}-${viewportName}.png`);
      await page.screenshot({ path: failFile, fullPage: true });
      files = [path.relative(REPO_ROOT, failFile)];
    } catch {}
  } finally {
    await page.close();
    await context.close();
    if (videoDir) {
      const videos = fs.readdirSync(videoDir).filter((f) => f.endsWith('.webm'));
      if (videos.length) {
        const target = path.join(videoDir, `${scene.name}-allin-to-showdown.webm`);
        fs.renameSync(path.join(videoDir, videos[0]), target);
        videoPath = path.relative(REPO_ROOT, target);
      }
    }
  }

  return {
    viewport: viewportName,
    files,
    videoPath,
    timer: staged.timer ?? null,
    frames: advanced.frames?.length ?? 0,
    advancedOnChain: advanced.onChain ?? null,
    setup,
    verification,
    error,
    canisterIdsCalled: [...canisterCalls],
    consoleErrors: watchers.consoleErrors.slice(0, 8),
    pageErrors: watchers.pageErrors.slice(0, 8),
    failedRequests: watchers.failedRequests.slice(0, 8),
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    log('usage: node tools/shots/run.mjs [--scenes a,b] [--viewports desktop,mobile] [--skip-build] [--skip-deploy]');
    return 0;
  }

  const startedAt = new Date().toISOString();
  log('ClearDeck screenshot harness');
  log(`  repo: ${REPO_ROOT}`);
  log(`  gateway: ${GATEWAY_ORIGIN}`);

  log('\n[1/6] replica preflight');
  await requireReplica();
  log('  local replica gateway is answering');

  log('\n[2/6] frontend build (local canister IDs wired in)');
  let buildProof = null;
  if (args.skipBuild) log('  skipped (--skip-build)');
  else buildProof = buildFrontend({ log }).proof;

  log('\n[3/6] deploy asset canister');
  if (args.skipDeploy) log('  skipped (--skip-deploy)');
  else deployFrontend({ log });

  const ids = readLocalIds();
  const frontendId = requireId(ids, 'frontend');

  log('\n[4/6] gateway port shim');
  // The app hardcodes http://127.0.0.1:4943 for the local agent. If the gateway
  // already listens there, no shim is needed and the asset canister is reached
  // directly on its own subdomain.
  let proxy = { origin: null, stats: () => ({ skipped: true }), close: async () => {} };
  if (GATEWAY_PORT === PROXY_PORT) {
    setAppOrigin(`http://${frontendId}.${GATEWAY_HOST}:${GATEWAY_PORT}`);
    log(`  not needed: gateway already on ${PROXY_PORT}; app origin ${getAppOrigin()}`);
  } else {
    proxy = await startGatewayProxy({ frontendCanisterId: frontendId, log });
    setAppOrigin(APP_ORIGIN);
  }

  log('\n[5/6] resolve lobby table names');
  const { names: tableNames, lobbyTableCount } = await resolveTableNames(ids);
  log(`  lobby lists ${lobbyTableCount} tables; mapped: ${JSON.stringify(tableNames)}`);

  const scenes = scenesByName(args.scenes);
  const sha = gitShortSha();
  const dirs = runDirs(sha);

  // A full run owns `latest/`; a partial run only updates the files it produces.
  if (args.scenes.length === 0 && args.viewports.length === Object.keys(VIEWPORTS).length) {
    for (const f of fs.readdirSync(dirs.latestDir)) {
      fs.rmSync(path.join(dirs.latestDir, f), { recursive: true, force: true });
    }
  }

  const ctx = {
    ids,
    tableIds: {
      table_1: ids.table_1, table_2: ids.table_2, table_3: ids.table_3,
      btc_table_1: ids.btc_table_1,
    },
    tableNames,
    frontendId,
    log,
  };

  log('\n[6/6] capture');
  resetThirdPartyObservations();
  const browser = await launchBrowser({ log });
  const sceneResults = [];
  try {
    for (const scene of scenes) {
      const shots = [];
      let sceneError = null;
      let verified = true;
      for (const viewportName of args.viewports) {
        const result = await runScene(scene, viewportName, ctx, browser, dirs);
        if (result.error) { sceneError = result.error; verified = false; }
        if (!result.verification.verified) verified = false;
        shots.push({
          viewport: viewportName,
          files: result.files,
          videoPath: result.videoPath,
          frames: result.frames,
          timer: result.timer,
          checks: result.verification.checks,
          notes: result.verification.notes,
          canisterIdsCalled: result.canisterIdsCalled,
          consoleErrors: result.consoleErrors,
          pageErrors: result.pageErrors,
          failedRequests: result.failedRequests,
          onChainAfterAdvance: result.advancedOnChain,
          error: result.error,
        });
      }
      sceneResults.push({
        scene: scene.name,
        title: scene.title,
        verified,
        error: sceneError,
        notes: shots[0]?.notes ?? '',
        onChain: shots[0]?.checks ?? {},
        shots,
      });
    }
  } finally {
    await browser.close();
    await proxy.close();
  }

  const manifest = {
    startedAt,
    finishedAt: new Date().toISOString(),
    gitSha: sha,
    gitDirty: gitDirty(),
    gateway: GATEWAY_ORIGIN,
    appOrigin: getAppOrigin(),
    frontendCanisterId: frontendId,
    localCanisterIds: ids,
    lobbyTableNames: tableNames,
    heroPrincipal: devPlayerPrincipal(HERO_PLAYER),
    // Which identity signed the controller-only calls (reset_table). Resolved
    // from the canister's real controller list, not assumed.
    controllerIdentity: controllerIdentityInUse(),
    auth:
      "the app's own local-dev login (WalletButton 'Dev Login' -> auth.devLogin), a deterministic " +
      'Ed25519 identity; every canister call is really signed by it. Local Internet Identity is ' +
      'not available on this replica (network descriptor has ii:false).',
    bundleWiring: buildProof,
    proxyStats: proxy.stats(),
    // Provenance of every live-fact third-party response. Any fiat figure in a
    // PNG traces to one of these; a cached quote is never replayed as live.
    volatileThirdParty: summariseVolatile(thirdPartyObservations()),
    viewports: args.viewports.map((v) => VIEWPORTS[v]),
    scenes: sceneResults,
  };
  writeManifest(dirs.shaDir, dirs.latestDir, manifest);
  const index = writeIndex(dirs.shaDir, dirs.latestDir, manifest);

  log(`\nartifacts: ${path.relative(REPO_ROOT, dirs.shaDir)}  (mirrored to artifacts/screens/latest)`);
  log('');
  log(index);

  const failed = sceneResults.filter((s) => !s.verified);
  if (failed.length) {
    log(`\n${failed.length} scene(s) not verified: ${failed.map((s) => s.scene).join(', ')}`);
    return 1;
  }
  log(`\nAll ${sceneResults.length} scenes captured and verified.`);
  return 0;
}

main()
  .then((code) => process.exit(code))
  .catch((e) => {
    console.error(`\nFATAL: ${e.message}`);
    if (process.env.SHOTS_DEBUG) console.error(e.stack);
    process.exit(2);
  });
