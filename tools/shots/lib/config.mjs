// Static configuration for the ClearDeck screenshot harness.
//
// Everything that could otherwise be a magic number lives here so the harness
// has no hardcoded values scattered through the driver.

import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));

/** Repository root (…/cleardeck). */
export const REPO_ROOT = path.resolve(here, '..', '..', '..');
export const FRONTEND_DIR = path.join(REPO_ROOT, 'src', 'cleardeck_frontend');
export const FRONTEND_DIST = path.join(FRONTEND_DIR, 'dist');
export const DECLARATIONS_DIR = path.join(REPO_ROOT, 'src', 'declarations');
export const ARTIFACTS_DIR = path.join(REPO_ROOT, 'artifacts', 'screens');
export const ASSET_CACHE_DIR = path.join(REPO_ROOT, 'artifacts', 'screens', '.thirdparty-cache');

/**
 * The local icp-cli environment name. NEVER change this to `ic`: the mainnet
 * canisters hold real user funds and are strictly off limits to this harness.
 */
export const ENV = 'local';

/**
 * Managed local network HTTP gateway (port pinned to 8077 in icp.yaml).
 * Override with SHOTS_GATEWAY_PORT / SHOTS_GATEWAY_HOST if the local network is
 * ever brought up on a different port.
 */
export const GATEWAY_HOST = process.env.SHOTS_GATEWAY_HOST || 'localhost';
export const GATEWAY_PORT = Number(process.env.SHOTS_GATEWAY_PORT || 8077);
export const GATEWAY_ORIGIN = `http://${GATEWAY_HOST}:${GATEWAY_PORT}`;

/**
 * src/lib/ic-config.js hardcodes `LOCAL_HOST = 'http://127.0.0.1:4943'` for the
 * agent in local development, and src/lib/auth.js hardcodes the same port for
 * the local Internet Identity origin. This project's gateway is on 8077, so the
 * harness runs a transparent reverse proxy on 4943 that forwards to the real
 * gateway. This is a port shim only: every byte served still comes from the
 * replica (asset canister HTML/JS included). Nothing is mocked.
 *
 * Serving the app through the same origin the agent talks to also removes CORS
 * from the equation.
 */
export const PROXY_HOST = '127.0.0.1';
export const PROXY_PORT = 4943;
export const APP_ORIGIN = `http://${PROXY_HOST}:${PROXY_PORT}`;

/** The real ICP ledger, live on the local replica at the ID the table hardcodes. */
export const ICP_LEDGER_CANISTER_ID = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
export const ICP_TRANSFER_FEE_E8S = 10_000n;

/**
 * icp-cli identities used only to move local play money and reset local tables.
 * Each throwaway local identity holds ~1000 local ICP; the driver walks the list
 * so a long screenshot session cannot run one of them dry.
 */
export const FUNDER_IDENTITIES = ['cd-alice', 'cd-bob', 'cd-carol', 'cd-local-deployer'];
export const CONTROLLER_IDENTITY = 'cd-local-deployer';

/** e8s helpers. */
export const E8S = 100_000_000n;
export const icp = (n) => BigInt(Math.round(n * 1e8));

/**
 * Deterministic in-app identities.
 *
 * The app ships its own local-dev login (`auth.devLogin`, wired to the "Dev
 * Login" button that WalletButton.svelte renders whenever the page is not on an
 * IC mainnet hostname). It derives an Ed25519 identity from the seed
 * `dev-player-<n>` padded to 32 bytes, which makes the principal deterministic
 * across runs. The harness clicks that real button, so the browser runs the
 * app's real authenticated code path against the real canisters.
 */
export const DEV_PLAYER_SEED = (n) => `dev-player-${n}`;
export const HERO_PLAYER = 1; // the player the browser is logged in as
export const OPPONENT_PLAYERS = [2, 3, 4];

/** Viewports. PNGs are written at exactly these pixel dimensions. */
export const VIEWPORTS = {
  desktop: { name: 'desktop', width: 1440, height: 900, deviceScaleFactor: 1, isMobile: false },
  mobile: { name: 'mobile', width: 390, height: 844, deviceScaleFactor: 1, isMobile: true },
};

/** Third-party hosts the app pulls from. Cached to disk for determinism. */
export const THIRD_PARTY_HOSTS = [
  'fonts.googleapis.com',
  'fonts.gstatic.com',
  'api.dicebear.com',
  'api.coingecko.com',
];

/** How long to wait for on-chain state to satisfy a scene predicate. */
export const STATE_TIMEOUT_MS = 60_000;
export const STATE_POLL_MS = 250;

/** Table config literals, mirrored from icp.yaml so reset_table can restore them. */
export const TABLE_CONFIGS = {
  table_1: {
    small_blind: 1_000_000n, big_blind: 2_000_000n,
    min_buy_in: 200_000_000n, max_buy_in: 1_000_000_000n,
    max_players: 2, action_timeout_secs: 30n, ante: 0n, time_bank_secs: 30n, currency: 'ICP',
  },
  table_2: {
    small_blind: 5_000_000n, big_blind: 10_000_000n,
    min_buy_in: 1_000_000_000n, max_buy_in: 5_000_000_000n,
    max_players: 6, action_timeout_secs: 45n, ante: 0n, time_bank_secs: 30n, currency: 'ICP',
  },
  table_3: {
    small_blind: 10_000_000n, big_blind: 20_000_000n,
    min_buy_in: 2_000_000_000n, max_buy_in: 10_000_000_000n,
    max_players: 9, action_timeout_secs: 60n, ante: 0n, time_bank_secs: 30n, currency: 'ICP',
  },
};

/** Animation/transition kill switch injected before each still. */
export const FREEZE_CSS = `
  *, *::before, *::after {
    animation-delay: -1ms !important;
    animation-duration: 1ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0ms !important;
    transition-delay: 0ms !important;
    scroll-behavior: auto !important;
  }
  .glow, .glow-1, .glow-2, .glow-3 { animation: none !important; }
  .spinner { animation: none !important; }
`;
