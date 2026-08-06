#!/usr/bin/env node
// THE MAINNET BUNDLE, RENDERED, WITH MAINNET UNREACHABLE.
//
// ===========================================================================
// WHAT THIS PROVES THAT `build/verify-bundle.mjs` CANNOT
// ===========================================================================
//
// The static verifier reads the emitted JavaScript. That answers "is the right
// string in the file", which is a real question and not the whole one. It cannot
// answer:
//
//   * does the bundle RUN, or does the mainnet build throw on load;
//   * are the four protected notices ON SCREEN at both viewports, including
//     under an error toast and behind every dialog (HARD RULE 2, and the only
//     kind of evidence this project accepts for it -- source greps were green
//     through an entire wave in which a portrait player saw none of them);
//   * does the app TELL the player it is on mainnet, in rendered pixels;
//   * which host does the agent ACTUALLY dial.
//
// The last one is the interesting one, and it is answered by observing an
// outbound request that is stopped before it leaves the machine.
//
// ===========================================================================
// HOW MAINNET IS KEPT UNREACHABLE. TWO INDEPENDENT LOCKS.
// ===========================================================================
//
// This wave is forbidden to call mainnet, and rendering a mainnet bundle is
// precisely a thing that tries to. So:
//
//   LOCK 1, in the browser process: `--host-resolver-rules=MAP * ~NOTFOUND,
//   EXCLUDE 127.0.0.1`. Chromium's own resolver returns NXDOMAIN for every host
//   except loopback. No socket to icp-api.io can be opened even if every other
//   layer is wrong.
//
//   LOCK 2, in Playwright: a context route that FULFILS only the local static
//   server and ABORTS everything else, recording what was attempted. Route
//   interception happens before the request is issued.
//
// Neither lock depends on the other, and the recorded aborts are the evidence:
// an attempt to reach `https://icp-api.io/api/v2/...` that never left the
// process is exactly the observation "this bundle dials mainnet", taken safely.
//
// Usage:  node tools/shots/verify-mainnet-bundle.mjs [--dist <dir>] [--shots <dir>]

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, URL } from 'node:url';
import { chromium } from 'playwright';
import { PROTECTED_PHRASES, foldProtectedNotices, probeProtectedNotices } from './lib/protected-notices.mjs';

const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url));
const argOf = (flag, fallback) => {
  const i = process.argv.indexOf(flag);
  return i === -1 ? fallback : process.argv[i + 1];
};
const DIST = path.resolve(argOf('--dist', path.join(REPO_ROOT, 'src/cleardeck_frontend/dist')));
const SHOT_DIR = argOf('--shots', null);

const MAINNET = Object.freeze({
  lobby: 'kpfcd-kyaaa-aaaaj-qor3a-cai',
  history: 'kggj7-4qaaa-aaaaj-qor2q-cai',
  frontend: 'kbhpl-riaaa-aaaaj-qor2a-cai',
  table_1: 'kieex-haaaa-aaaaj-qor3q-cai',
  table_2: 'lfkaz-iiaaa-aaaaj-qor4a-cai',
  table_3: 'lclgn-fqaaa-aaaaj-qor4q-cai',
  btc_table_1: 'qrhly-eaaaa-aaaaj-qousa-cai',
});
const LOCAL_ID_SHAPE = /\b[a-z0-9]{5}(?:-[a-z0-9]{5})?-77775-[a-z0-9]{5}-cai\b/;

const VIEWPORTS = [
  { name: 'desktop', width: 1440, height: 900 },
  { name: 'mobile', width: 390, height: 844 },
];

// ---------------------------------------------------------------------------
// A static server for `dist`, on loopback, with SPA fallback.
// ---------------------------------------------------------------------------

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
};

function serve(root) {
  const server = http.createServer((req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    let file = path.join(root, decodeURIComponent(url.pathname));
    if (!file.startsWith(root)) {
      res.writeHead(403).end();
      return;
    }
    if (!fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      file = path.join(root, 'index.html');
    }
    if (!fs.existsSync(file)) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { 'content-type': MIME[path.extname(file)] || 'application/octet-stream' });
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port }));
  });
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

const results = [];
const record = (name, ok, detail) => {
  results.push({ name, ok: Boolean(ok), detail });
  console.log(`  ${ok ? '✓' : '✗'} ${name}\n      ${detail}`);
};

/**
 * Notices on screen, in this exact state, at this exact viewport.
 *
 * On failure this reports EVERY carrier and why each was rejected, not just the
 * first. `foldProtectedNotices` names `best`, which is `measured[0]` when nothing
 * is on screen -- and `measured[0]` was a `display: none` portrait strip with a
 * 0x0 rectangle, so the first failing run of this file said "nearest was null at
 * {0,0,0,0}" for all five phrases and hid the actual finding: a carrier sitting
 * in the viewport underneath the dialog scrim.
 */
async function assertNotices(page, where) {
  const probe = await probeProtectedNotices(page);
  const fold = foldProtectedNotices(probe, where);
  let detail;
  if (fold.ok) {
    detail = `${fold.onScreen}/${fold.total} phrases hit-tested on their own pixels`;
  } else {
    detail = probe
      .filter((n) => !n.onScreen)
      .map((n) => `"${n.phrase}": ${n.rejected.map((r) =>
        `${r.path} rect=${r.rect.x},${r.rect.y} ${r.rect.w}x${r.rect.h} `
        + `vis=${r.visible} inVp=${r.inViewport} own=${r.ownPixels} `
        + `covered-by=${r.occludedBy ?? 'n/a'}`).join(' ; ') || '(no carrier at all)'}`)
      .join('\n      ');
  }
  record(`protected notices on screen — ${where}`, fold.ok, detail);
  return fold.ok;
}

async function run() {
  if (!fs.existsSync(path.join(DIST, 'index.html'))) {
    console.error(`No bundle at ${DIST}. Build it with: npm run build:mainnet`);
    process.exit(2);
  }
  if (SHOT_DIR) fs.mkdirSync(SHOT_DIR, { recursive: true });

  const { server, port } = await serve(DIST);
  const origin = `http://127.0.0.1:${port}`;
  console.log(`\n  serving the mainnet bundle at ${origin}`);
  console.log('  every non-loopback host is NXDOMAIN in this browser, and every');
  console.log('  non-loopback request is aborted before it is issued.\n');

  const browser = await chromium.launch({
    headless: true,
    args: ['--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1'],
  });

  /** Every request the page tried to make that was not to the local server. */
  const blocked = [];

  try {
    for (const vp of VIEWPORTS) {
      const context = await browser.newContext({
        viewport: { width: vp.width, height: vp.height },
        deviceScaleFactor: 1,
        isMobile: vp.name === 'mobile',
        hasTouch: vp.name === 'mobile',
      });

      await context.route('**/*', async (route) => {
        const url = route.request().url();
        if (url.startsWith(origin) || url.startsWith('data:') || url.startsWith('blob:')) {
          await route.continue();
          return;
        }
        blocked.push(url);
        await route.abort('blockedbyclient');
      });

      const page = await context.newPage();
      await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
      // The lobby load fails (mainnet is unreachable), which is the point: the
      // error toast is one of the states HARD RULE 2 has to hold in. WAIT for it
      // rather than sleeping — the first version of this slept 3.5s, the toast
      // arrived at ~5s, and the run reported "0 .toast.error elements" while
      // claiming to have measured the notices underneath one.
      await page.locator('.toast.error').first()
        .waitFor({ state: 'visible', timeout: 40_000 })
        .catch(() => {});

      // --- what the page says it is ---------------------------------------
      const chip = await page.locator('.net-chip').first();
      const chipText = (await chip.count()) ? (await chip.innerText()).trim() : '';
      const chipVisible = (await chip.count()) ? await chip.isVisible() : false;
      if (vp.name === 'desktop') {
        record(
          'the page tells the reader it is on mainnet',
          chipVisible && /IC mainnet/i.test(chipText),
          `.net-chip = ${JSON.stringify(chipText)} (visible: ${chipVisible})`,
        );
      }

      const footer = await page.locator('.net-footer').first();
      const footerText = (await footer.count()) ? (await footer.innerText()).trim() : '';
      if (vp.name === 'desktop') {
        record(
          'the footer names the wired lobby canister',
          footerText.includes(MAINNET.lobby),
          `.net-footer = ${JSON.stringify(footerText)}`,
        );
      }

      // --- notices, in every state that can cover them --------------------
      await assertNotices(page, `${vp.name}, lobby, with the connection error toast`);

      const toast = await page.locator('.toast.error').count();
      if (vp.name === 'desktop') {
        record(
          'an error toast is actually present for that measurement',
          toast > 0,
          `${toast} .toast.error element(s) — the notices were measured with one on screen`,
        );
      }

      // The Verify panel: a full-screen dialog over a blurred scrim.
      await page.locator('button.footer-link', { hasText: 'Verify Code' }).first().click();
      await page.waitForTimeout(600);
      await assertNotices(page, `${vp.name}, Verify Code dialog open`);

      // --- the Verify panel's own content ---------------------------------
      if (vp.name === 'desktop') {
        const panel = await page.locator('.verify-modal').first().innerText();
        // ROLE AND ID IN THE SAME ROW. Reading them from the same <tr> is the
        // point: "the id is somewhere in the panel" would pass on a panel that
        // labels every canister wrongly, which is the exact defect this check
        // exists for. (The first version of this split innerText on newlines and
        // looked for a line equal to the role; a table row is one line with a
        // tab in it, so it reported "role row missing" on a correct panel.)
        const rows = await page.locator('.canister-ids tbody tr').evaluateAll(
          (trs) => trs.map((tr) => [...tr.children].map((td) => td.textContent.trim())),
        );
        for (const [role, id] of Object.entries(MAINNET)) {
          if (role === 'frontend') continue; // deliberately not listed; see +page.svelte
          const row = rows.find((cells) => cells[0] === role);
          record(
            `Verify panel lists ${role} = ${id}`,
            Boolean(row) && row[1] === id,
            row ? `row: ${JSON.stringify(row)}` : `no row labelled "${role}" (${rows.length} rows read)`,
          );
        }
        record(
          'Verify panel states the mainnet gateway',
          panel.includes('https://icp-api.io'),
          'agent host as rendered',
        );
        record(
          'Verify panel states the mainnet sign-in provider',
          panel.includes('https://id.ai/authorize'),
          'II provider as rendered',
        );
        record(
          'Verify panel states the wired lobby and history',
          panel.includes(MAINNET.lobby) && panel.includes(MAINNET.history),
          'wiring section as rendered',
        );
        record(
          'Verify panel calls the hashes a claim, not a measurement',
          /claim, not a measurement/i.test(panel),
          'the provenance sentence is on screen',
        );
      }

      if (SHOT_DIR) {
        await page.screenshot({ path: path.join(SHOT_DIR, `mainnet-verify-${vp.name}.png`) });
      }
      await page.locator('.verify-modal .close-btn').first().click().catch(() => {});
      await page.waitForTimeout(300);

      // The other dialog reachable without a table. Both are scrims over the
      // banner, so both have to carry the notices themselves.
      await page.locator('button.footer-link', { hasText: 'How It Works' }).first().click();
      await page.waitForTimeout(600);
      await assertNotices(page, `${vp.name}, How It Works dialog open`);
      await page.keyboard.press('Escape');
      await page.waitForTimeout(300);

      // --- no local id anywhere in the rendered page ----------------------
      const bodyText = await page.locator('body').innerText();
      const leak = bodyText.match(LOCAL_ID_SHAPE);
      record(
        `no local replica canister id rendered — ${vp.name}`,
        !leak,
        leak ? `found ${leak[0]}` : 'none in the rendered text',
      );

      if (SHOT_DIR) {
        await page.screenshot({ path: path.join(SHOT_DIR, `mainnet-lobby-${vp.name}.png`) });
      }
      await context.close();
    }

    // --- which host did the agent actually dial? --------------------------
    const hosts = [...new Set(blocked.map((u) => {
      try { return new URL(u).host; } catch { return u; }
    }))];
    record(
      'the agent dialled the MAINNET gateway (observed, then blocked)',
      hosts.some((h) => h === 'icp-api.io'),
      `hosts attempted and stopped: ${hosts.join(', ') || '(none)'}`,
    );
    record(
      'the agent never dialled a local replica',
      !hosts.some((h) => /^(127\.0\.0\.1|localhost)(:\d+)?$/.test(h)),
      `no loopback host outside the static server; attempted: ${hosts.join(', ') || '(none)'}`,
    );
  } finally {
    await browser.close();
    server.close();
  }

  const failed = results.filter((r) => !r.ok);
  console.log('');
  if (failed.length) {
    console.error(`  ${failed.length} of ${results.length} checks FAILED.\n`);
    process.exit(1);
  }
  console.log(`  ${results.length} of ${results.length} checks passed `
    + `(${PROTECTED_PHRASES.length} protected phrases, ${VIEWPORTS.length} viewports).\n`);
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
