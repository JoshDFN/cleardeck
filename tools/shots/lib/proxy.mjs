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

import http from 'node:http';
import { GATEWAY_HOST, GATEWAY_PORT, PROXY_HOST, PROXY_PORT } from './config.mjs';

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

  const stats = { requests: 0, apiRequests: 0, assetRequests: 0, errors: 0 };

  const server = http.createServer((req, res) => {
    stats.requests += 1;
    const isApi = req.url.startsWith('/api/') || req.url.startsWith('/_/');
    if (isApi) stats.apiRequests += 1;
    else stats.assetRequests += 1;

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

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(PROXY_PORT, PROXY_HOST, resolve);
  });

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
