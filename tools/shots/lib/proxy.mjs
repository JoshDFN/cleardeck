// Transparent reverse proxy: 127.0.0.1:4943 -> the real local gateway.
//
// WHY THIS EXISTS (and why it is not a mock):
//   src/cleardeck_frontend/src/lib/ic-config.js hardcodes
//     export const LOCAL_HOST = 'http://127.0.0.1:4943';
//   and src/lib/auth.js hardcodes `http://<ii-id>.localhost:4943` for local
//   Internet Identity. This project's managed network gateway is pinned to port
//   8077 in icp.yaml, so an unmodified local build points its agent at a port
//   nothing is listening on. This harness is not allowed to edit the frontend
//   source (a later wave redesigns it), so instead it listens on 4943 and
//   forwards verbatim to the gateway, rewriting only the Host header.
//
// Every response body — the asset canister's index.html, the JS bundle, and all
// /api/v2 + /api/v3 agent traffic — comes straight from the replica. Serving the
// page and the agent traffic from the same origin also removes CORS from the
// equation.

//
// A/B MODE (SHOTS_SERVE_DIST), off by default
// -------------------------------------------
// Set `SHOTS_SERVE_DIST=/abs/path/to/dist` and the proxy serves STATIC ASSETS
// from that directory instead of from the asset canister. Every /api/* request —
// every query, every update, every certificate — still goes to the real local
// replica untouched, so the CHAIN SIDE of a comparison is identical between the
// two runs and only the bundle differs.
//
// It exists so a one-line frontend fix can be photographed against the same live
// canisters as the unfixed build WITHOUT deploying anything: other agents share
// this replica's asset canister, and swapping their app out from under them to
// take a before/after pair is not an acceptable cost. The run manifest records
// that the assets did not come from the canister, because a screenshot served
// off local disk is weaker evidence than one served through the gateway and must
// never be filed as though it were the same thing.

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { GATEWAY_HOST, GATEWAY_PORT, PROXY_HOST, PROXY_PORT } from './config.mjs';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.txt': 'text/plain; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
};

/** @returns {string|null} absolute path to an override dist, or null. */
export function overrideDistDir() {
  const raw = (process.env.SHOTS_SERVE_DIST || '').trim();
  if (!raw) return null;
  const dir = path.resolve(raw);
  if (!fs.existsSync(path.join(dir, 'index.html'))) {
    throw new Error(`SHOTS_SERVE_DIST=${dir} has no index.html — that is not a built frontend.`);
  }
  return dir;
}

/**
 * Resolves a URL path inside the override dist, SPA-style.
 * Refuses anything that escapes the directory.
 */
function resolveStatic(dir, urlPath) {
  const clean = decodeURIComponent(urlPath.split('?')[0]);
  const candidate = path.resolve(dir, `.${clean}`);
  if (candidate !== dir && !candidate.startsWith(dir + path.sep)) return null;
  if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
  const indexed = path.join(candidate, 'index.html');
  if (fs.existsSync(indexed)) return indexed;
  // SvelteKit static adapter: unknown routes fall back to the app shell.
  if (!path.extname(clean)) return path.join(dir, 'index.html');
  return null;
}

const HOP_BY_HOP = new Set([
  'connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization',
  'te', 'trailer', 'transfer-encoding', 'upgrade',
]);

function filterHeaders(headers) {
  const out = {};
  for (const [k, v] of Object.entries(headers)) {
    if (HOP_BY_HOP.has(k.toLowerCase())) continue;
    out[k] = v;
  }
  return out;
}

/**
 * @param {object} opts
 * @param {string} opts.frontendCanisterId asset canister that serves the app
 * @param {(msg:string)=>void} [opts.log]
 * @returns {Promise<{close:()=>Promise<void>, stats:()=>object, origin:string}>}
 */
export async function startGatewayProxy({ frontendCanisterId, log = () => {} }) {
  if (!frontendCanisterId) throw new Error('startGatewayProxy needs frontendCanisterId');

  const distDir = overrideDistDir();
  const stats = {
    requests: 0, apiRequests: 0, assetRequests: 0, errors: 0,
    assetsFromDisk: 0,
    assetSource: distDir ? `LOCAL DISK ${distDir} (SHOTS_SERVE_DIST)` : 'the asset canister via the gateway',
  };
  if (distDir) {
    log(`  ⚠ SHOTS_SERVE_DIST: static assets come from ${distDir}, NOT from the asset canister.`);
    log('    All /api/* traffic still goes to the real local replica.');
  }

  const server = http.createServer((req, res) => {
    stats.requests += 1;
    const isApi = req.url.startsWith('/api/') || req.url.startsWith('/_/');
    if (isApi) stats.apiRequests += 1;
    else stats.assetRequests += 1;

    if (!isApi && distDir) {
      const file = resolveStatic(distDir, req.url);
      if (file) {
        stats.assetsFromDisk += 1;
        const body = fs.readFileSync(file);
        res.writeHead(200, {
          'content-type': MIME[path.extname(file).toLowerCase()] || 'application/octet-stream',
          'content-length': body.length,
          'cache-control': 'no-store',
        });
        res.end(body);
        return;
      }
    }

    // /api/* is resolved by the canister id in the path, so a plain gateway Host
    // works. Everything else is a canister HTTP request the gateway resolves
    // from the Host header, so we point it at the frontend asset canister.
    const hostHeader = isApi
      ? `${GATEWAY_HOST}:${GATEWAY_PORT}`
      : `${frontendCanisterId}.${GATEWAY_HOST}:${GATEWAY_PORT}`;

    const upstream = http.request(
      {
        host: GATEWAY_HOST === 'localhost' ? '127.0.0.1' : GATEWAY_HOST,
        port: GATEWAY_PORT,
        method: req.method,
        path: req.url,
        headers: { ...filterHeaders(req.headers), host: hostHeader },
      },
      (up) => {
        res.writeHead(up.statusCode || 502, filterHeaders(up.headers));
        up.pipe(res);
      },
    );

    upstream.on('error', (err) => {
      stats.errors += 1;
      log(`proxy upstream error for ${req.method} ${req.url}: ${err.message}`);
      if (!res.headersSent) res.writeHead(502, { 'content-type': 'text/plain' });
      res.end(`gateway proxy error: ${err.message}`);
    });

    req.pipe(upstream);
  });

  // ONE RUN AT A TIME, BY CONSTRUCTION. The app hardcodes 4943, so the shim
  // cannot move to another port, so two screenshot runs cannot coexist. Several
  // agents share this tree and this replica, and the old behaviour was an
  // immediate `FATAL: listen EADDRINUSE`, which reads like a broken harness
  // rather than "somebody else is mid-run". Wait for the other run to finish,
  // saying so, and only then give up.
  const deadline = Date.now() + Number(process.env.SHOTS_PORT_WAIT_MS || 300_000);
  let announced = false;
  for (;;) {
    try {
      await new Promise((resolve, reject) => {
        server.once('error', reject);
        server.listen(PROXY_PORT, PROXY_HOST, resolve);
      });
      break;
    } catch (e) {
      if (e.code !== 'EADDRINUSE') throw e;
      if (Date.now() > deadline) {
        throw new Error(
          `${PROXY_HOST}:${PROXY_PORT} is still in use after waiting. The app hardcodes that\n`
          + 'port (src/lib/ic-config.js LOCAL_HOST), so only one screenshot run can exist at a\n'
          + 'time. Another agent is probably running tools/shots/run.mjs against this tree.\n'
          + 'Raise SHOTS_PORT_WAIT_MS to wait longer.',
        );
      }
      if (!announced) {
        log(`  ${PROXY_HOST}:${PROXY_PORT} busy — another screenshot run is in progress; waiting for it`);
        announced = true;
      }
      server.removeAllListeners('error');
      await new Promise((r) => setTimeout(r, 3_000));
    }
  }

  log(
    `gateway proxy listening on http://${PROXY_HOST}:${PROXY_PORT} -> ` +
      `http://${GATEWAY_HOST}:${GATEWAY_PORT}`,
  );

  return {
    origin: `http://${PROXY_HOST}:${PROXY_PORT}`,
    stats: () => ({ ...stats }),
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections?.();
        server.close(() => resolve());
      }),
  };
}
