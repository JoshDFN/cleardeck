// Centralized IC network + identity configuration.
//
// Single source of truth for the Internet Identity provider URL and the agent
// gateway host. Previously these literals (and the isMainnet() helper) were
// duplicated across auth.js, canisters.js, oisy.js and WalletButton.svelte, and
// had already drifted (oisy.js used icp-api.io while the rest used ic0.app).
// Keeping them here makes a future provider change a one-line edit and lets the
// values be overridden via Vite env vars for instant rollback.
//
// Modern DFINITY guidance (icskills):
//   - Internet Identity provider = https://id.ai   (II v2; replaces identity.internetcomputer.org)
//   - Agent gateway host         = https://icp-api.io  (replaces ic0.app)
//
// IMPORTANT: the II URL MUST include the /authorize path. The bare origin
// (https://id.ai) opens id.ai account management and the AuthClient delegation
// handshake never completes — locking users out of their funded session.

const MAINNET_HOSTNAMES = ['icp0.io', 'ic0.app', 'internetcomputer.org'];

// Local replica host used for both the agent and (with a canister-id prefix) II.
export const LOCAL_HOST = 'http://127.0.0.1:4943';

/**
 * True when the page is served from an IC mainnet origin.
 * NOTE: this tests where the PAGE is served, not where the agent connects.
 * @returns {boolean}
 */
export function isMainnet() {
  return typeof window !== 'undefined' &&
    MAINNET_HOSTNAMES.some((h) => window.location.hostname.includes(h));
}

/** @returns {boolean} true in local development. */
export function isLocal() {
  return !isMainnet();
}

// Mainnet agent gateway host (env-overridable for rollback).
export const IC_HOST = import.meta.env.VITE_IC_HOST || 'https://icp-api.io';

// Mainnet Internet Identity provider (env-overridable for rollback). MUST end in /authorize.
export const II_URL = import.meta.env.VITE_II_URL || 'https://id.ai/authorize';

/**
 * Agent host for the current environment.
 * @returns {string} IC_HOST on mainnet, the local replica otherwise.
 */
export function agentHost() {
  return isMainnet() ? IC_HOST : LOCAL_HOST;
}
