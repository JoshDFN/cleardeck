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
import { overrideDistDir, startGatewayProxy } from './lib/proxy.mjs';
import { lobbyActor, optional } from './lib/agent.mjs';
import { controllerIdentityInUse } from './lib/table-driver.mjs';
import { devPlayerPrincipal } from './lib/identities.mjs';
import {
  burst, clearLatestVariants, gitDirty, gitShortSha, runDirs, shoot, writeIndex, writeManifest,
} from './lib/capture.mjs';
import {
  getAppOrigin, launchBrowser, newContext, resetThirdPartyObservations, setAppOrigin, settle,
  thirdPartyObservations, watchCanisterCalls, watchPage,
} from './lib/browser.mjs';
import { scenesByName } from './scenarios/index.mjs';
import { DRIFT_TARGETS, injectDrift, requestedDrift } from './lib/drift.mjs';
import { assertEveryTokenAccountedFor } from './lib/token-census.mjs';
import { assertNothingCoversAFigure } from './lib/occlusion.mjs';
import { foldProtectedNotices, probeProtectedNotices } from './lib/protected-notices.mjs';
import { foldFeltArea, measureFelt } from './lib/felt-area.mjs';

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

/**
 * Every figure any agreement surface compared this scene, flattened.
 *
 * A scene can assert against several surfaces (the table behind a modal, and the
 * modal), and `withAgreement` namespaces them as `chain`, `chain_deposit`, and so
 * on. The census needs all of them in one list, so the checks tree is walked for
 * anything shaped like a figure list rather than hardcoding the key names — a new
 * surface is then covered the day it is added.
 *
 * @param {object} checks the scene's verification.checks
 * @returns {Array<{label:string, chain:number|null, screen:string|null, ok:boolean, detail:string}>}
 */
function figuresFrom(checks) {
  const out = [];
  const walk = (node, depth) => {
    if (!node || typeof node !== 'object' || depth > 4) return;
    if (Array.isArray(node)) {
      for (const item of node) walk(item, depth + 1);
      return;
    }
    for (const [key, value] of Object.entries(node)) {
      if (key === 'figures' && Array.isArray(value)) {
        for (const f of value) {
          if (f && typeof f === 'object' && typeof f.label === 'string') out.push(f);
        }
        continue;
      }
      walk(value, depth + 1);
    }
  };
  walk(checks, 0);
  return out;
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
  let drift = [];

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

    // Fault injection, off unless SHOTS_INJECT_DRIFT names a target. The chain is
    // untouched; only the rendered number is changed, so this is exactly the
    // situation the agreement assertion exists to catch.
    const driftTargets = requestedDrift();
    if (driftTargets.length) {
      drift = await injectDrift(page, driftTargets);
      for (const d of drift) {
        log(`  ⚠ DRIFT INJECTED [${d.target}] ${d.selector}: "${d.before}" -> "${d.after}"${d.note ? ` (${d.note})` : ''}`);
      }
      // A FAULT THAT WAS NOT INJECTED IS A FALSE NEGATIVE IN THE PROOF. If the
      // selector no longer matches, the scene may still go UNVERIFIED for some
      // other reason and be read as "the assertion caught my injected fault"
      // when it caught nothing of the sort. Fail the scene instead, naming the
      // dead selector.
      const dead = drift.filter((d) => d.before === null);
      // Doubling "0.00" is still "0.00". A no-op edit is indistinguishable from a
      // correct screen, so it proves nothing either and is refused the same way.
      const inert = drift.filter((d) => d.before !== null && d.before === d.after);
      if (dead.length || inert.length) {
        throw new Error(
          'fault injection did not change what is on screen: '
          + [
            ...dead.map((d) => `${d.target} (${d.selector}) matched nothing`),
            ...inert.map((d) => `${d.target} rewrote "${d.before}" to the same text`),
          ].join('; ')
          + '. Nothing was proven. Either pick a scene that renders a non-zero figure there, '
          + 'or update DRIFT_TARGETS in lib/drift.mjs.',
        );
      }
    }

    verification = await scene.verify(ctx, page);

    // THE INVERTED GATE, RUN CENTRALLY SO NO SCENE CAN FORGET IT.
    // `scene.verify` produces a LIST of figures it chose to compare. That list
    // has no denominator: the wave-3 critic rewrote the header stakes pill to
    // "9.99/19.98" and the run still reported "14 money figures on screen all
    // equal the canister's". So after every scene has had its say, every numeric
    // token the page renders is enumerated and each one must be either matched to
    // one of those figures or declared non-monetary in the reviewed allowlist.
    const census = await assertEveryTokenAccountedFor(
      page, figuresFrom(verification.checks), { scene: scene.name, viewport: viewportName },
    );
    verification = {
      verified: verification.verified && census.ok,
      checks: { ...verification.checks, tokenCensus: census.checks },
      notes: census.ok
        ? `${verification.notes}; ${census.notes}`
        : `${census.notes} || ${verification.notes}`,
    };

    // THE PIXEL GATE, run centrally for the same reason the census is: a scene
    // cannot forget it. Every other gate in this repo reads `textContent`, so a
    // correct number with a card painted over it was photographed and filed as
    // verified twice (docs/DEFECTS.md T-22, T-23). This one asks whether the
    // figure is ON SCREEN — geometry, then effective paint order, then hit
    // testing, then a four-shot pixel differential that is the authority.
    const occlusion = await assertNothingCoversAFigure(page, {
      scene: scene.name,
      viewport: viewportName,
      outDir: path.join(dirs.shaDir, 'occlusion'),
    });
    verification = {
      verified: verification.verified && occlusion.ok,
      checks: { ...verification.checks, occlusion: occlusion.checks },
      notes: occlusion.ok
        ? `${verification.notes}; ${occlusion.notes}`
        : `${occlusion.notes} || ${verification.notes}`,
    };
    for (const f of occlusion.checks.findings) {
      log(`  ⚠ COVERED: ${(f.pixels.coveredInkFraction * 100).toFixed(1)}% of `
        + `${f.occluded.kind} "${f.occluded.text}" (${f.occluded.path}) is painted over by `
        + `${f.occluder.path}`);
    }

    // HARD RULE 2, RUN CENTRALLY, ON THE RENDERED PAGE.
    //
    // WHY IT IS HERE AND NOT IN A SCENE. `lib/protected-notices.mjs` was written
    // in wave 5 and imported by exactly two scenarios — handhistory and
    // handreplay — and by nothing else: not this file, not a table scene, not the
    // lobby, not deposit. The cost of that was demonstrated with a build rather
    // than an argument: flipping ONE declaration, `.banner-strip { display:
    // block }` to `display: none`, shows a phone player ZERO of the five
    // protected phrases on the table view, raises the felt, and leaves the
    // occlusion gate, the token census and `./scripts/dev.sh hygiene` all green.
    // That is wave 4's crime re-committed in one line with every gate passing,
    // which is the precise structural failure wave 5 was chartered to end. Wave 4
    // was green because hygiene greps the SOURCE; wave 5 was green because the
    // probe that reads the SCREEN lived in a scratchpad.
    //
    // So it runs where the census and the pixel gate run: once, centrally, for
    // every scene at every viewport, with no per-scene opt-in and no opt-out.
    //
    // WHAT SCROLL POSITION THE RULE IS ABOUT. Wave 4's crime was that the notices
    // were 248 px of scroll BELOW the fold on the view as it lands, so the rule
    // has to be evaluated at the top of the view or it is not the rule. Some
    // scenes then scroll deliberately to frame something (`shuffleproof` scrolls
    // the proof panel into shot on a phone, which puts the banner at y=-90), and
    // failing those would be a false red that gets the gate switched off. So: probe
    // where the scene left the page; if anything is off screen, probe again at
    // scroll 0 and let THAT be the verdict, recording both. A notice that is off
    // screen at the top of the view fails either way.
    let notices = foldProtectedNotices(
      await probeProtectedNotices(page),
      `on the rendered page (scene ${scene.name}, ${viewportName})`,
    );
    if (!notices.ok) {
      const wasAt = await page.evaluate(() => window.scrollY);
      if (wasAt !== 0) {
        await page.evaluate(() => window.scrollTo(0, 0));
        await settle(page);
        const atTop = foldProtectedNotices(
          await probeProtectedNotices(page),
          `at the top of the view (scene ${scene.name}, ${viewportName}; `
          + `the scene had scrolled to y=${wasAt})`,
        );
        await page.evaluate((y) => window.scrollTo(0, y), wasAt);
        await settle(page);
        notices = {
          ...atTop,
          measuredAtScrollTop: true,
          sceneScrollY: wasAt,
          atSceneScroll: { ok: false, onScreen: notices.onScreen, problems: notices.problems },
        };
      }
    }
    verification = {
      verified: verification.verified && notices.ok,
      checks: { ...verification.checks, protectedNotices: notices },
      notes: notices.ok
        ? `${verification.notes}; ${notices.onScreen}/${notices.total} protected notices on screen`
        : `${notices.problems.join(' | ')} || ${verification.notes}`,
    };
    for (const p of notices.problems) log(`  ⚠ NOTICE OFF SCREEN: ${p}`);

    // THE OTHER HALF OF THE SAME RULE. Hiding a notice makes the felt bigger, so
    // a notice gate on its own is an incentive to shrink the felt instead. This
    // asserts a floor under the playing surface and RECORDS the exact geometry,
    // which nothing in the harness did before: wave 5's headline 60.6% existed
    // only in prose and in a scratchpad script.
    const feltArea = foldFeltArea(await measureFelt(page), {
      viewport: viewportName, scene: scene.name,
    });
    verification = {
      verified: verification.verified && feltArea.ok,
      checks: { ...verification.checks, feltArea },
      notes: feltArea.ok
        ? `${verification.notes}; ${feltArea.notes}`
        : `${feltArea.problems.join(' | ')} || ${verification.notes}`,
    };
    if (feltArea.felt) log(`  ▪ ${feltArea.notes}`);
    for (const p of feltArea.problems) log(`  ⚠ FELT: ${p}`);

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
    driftInjected: drift,
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
          driftInjected: result.driftInjected,
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
    // Where the HTML/JS in these PNGs came from. Normally the asset canister on
    // the local replica; on an A/B run (SHOTS_SERVE_DIST) an alternate build off
    // local disk, with every canister call still going to the real replica. A
    // reader must never have to infer which.
    assetProvenance: overrideDistDir()
      ? {
        source: 'LOCAL DISK',
        dir: overrideDistDir(),
        warning:
            'SHOTS_SERVE_DIST was set: the page in these PNGs was served from a build on '
            + 'local disk, NOT from the deployed asset canister. Every canister query and '
            + 'update still went to the real local replica, so the CHAIN side of every '
            + 'agreement check is unaffected — but this is an A/B artifact, not a picture '
            + 'of what the deployed app serves.',
      }
      : { source: 'the deployed local asset canister', canisterId: frontendId },
    // Non-empty ONLY on a deliberate fault-injection run. Every scene in such a
    // run is expected to be UNVERIFIED; the artifacts are proof that the
    // agreement assertion fires, not evidence about the product.
    faultInjection: requestedDrift().length
      ? {
        targets: requestedDrift(),
        how: Object.fromEntries(requestedDrift().map((t) => [t, DRIFT_TARGETS[t].how])),
        warning:
            'SHOTS_INJECT_DRIFT was set: the RENDERED page was deliberately made to '
            + 'disagree with the canister. No canister response was mocked. Every PNG in '
            + 'this run is a fault-injection artifact.',
      }
      : null,
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
