import { Actor, HttpAgent } from "@dfinity/agent";
import { idlFactory as lobbyIdlFactory } from 'declarations/lobby/lobby.did.js';
import { idlFactory as tableIdlFactory } from 'declarations/table_1/table_1.did.js';
import { idlFactory as historyIdlFactory } from 'declarations/history/history.did.js';
import { building } from '$app/environment';
import { auth } from './auth.js';
import { agentHost, isLocal, isMainnet, isMainnetCanisterId, NETWORK } from './ic-config.js';

// Network timeout in milliseconds (30 seconds)
const NETWORK_TIMEOUT_MS = 30000;

function dummyActor() {
    return new Proxy({}, { get() { throw new Error("Canister invoked while building"); } });
}

// Wrap a promise with a timeout
function withTimeout(promise, timeoutMs, errorMessage = 'Request timed out') {
    let timeoutId;
    const timeoutPromise = new Promise((_, reject) => {
        timeoutId = setTimeout(() => {
            reject(new Error(errorMessage));
        }, timeoutMs);
    });

    return Promise.race([promise, timeoutPromise]).finally(() => {
        clearTimeout(timeoutId);
    });
}

const buildingOrTesting = building || process.env.NODE_ENV === "test";

// ---------------------------------------------------------------------------
// Canister ID resolution
// ---------------------------------------------------------------------------
//
// docs/DEFECTS.md T-01. This fallback chain used to end at the repo-root `.env`,
// which vite dotenv-loads and which holds the MAINNET ids. A bare
// `npm run build` therefore produced a bundle that pointed a local dev UI at the
// live canisters custodying real ICP and ckBTC, with no warning anywhere.
//
// Two layers now make that structurally impossible:
//
//   1. BUILD TIME (the real guarantee). vite.config.js refuses to build unless
//      the target network is stated explicitly, loads the repo-root `.env` ONLY
//      for an `ic` build, and aborts if a `local` build resolves a mainnet id.
//      A bad bundle cannot be produced.
//
//   2. RUNTIME (belt and braces, for a bundle built by some other toolchain).
//      resolveCanisterId() below refuses a mainnet id on a local build and
//      refuses a missing id on any build. It throws with an actionable message
//      instead of silently talking to the wrong canister — or to nothing.
//
// The mainnet ids are still displayed as text in the "Verify the Code" panel;
// that display is legitimate and comes from ic-config.js MAINNET_CANISTER_IDS.
// What is forbidden is a mainnet id reaching the WIRING of a non-mainnet build.

/**
 * Resolves one canister id from the build environment, with no silent fallback.
 *
 * The deploy scripts export VITE_CANISTER_ID_* from .icp/data/mappings (icp-cli
 * has no dfx-style .env writer); legacy dfx wrote CANISTER_ID_*. Both prefixes
 * are read so the build works under either toolchain.
 *
 * @param {string} name canister name, e.g. "LOBBY"
 * @param {string|undefined} viteValue import.meta.env.VITE_CANISTER_ID_<name>
 * @param {string|undefined} legacyValue import.meta.env.CANISTER_ID_<name>
 * @param {string|undefined} processValue process.env.CANISTER_ID_<name>
 * @returns {string|undefined} the id, or undefined while building/prerendering
 * @throws {Error} if the id is missing, or is a mainnet id on a non-mainnet build
 */
function resolveCanisterId(name, viteValue, legacyValue, processValue) {
    const id = viteValue || legacyValue || processValue;

    if (!id) {
        // Prerender/SSR and unit tests never make a call, so an absent id there
        // is not an error: dummyActor() below stands in for the actor.
        if (buildingOrTesting) return undefined;
        throw new Error(
            `ClearDeck build is broken: no canister id for ${name}. Set ` +
            `VITE_CANISTER_ID_${name} (or CANISTER_ID_${name}) at build time. ` +
            'The build must never guess: guessing is how a local build ends up ' +
            'wired to the live fund-holding canisters (docs/DEFECTS.md T-01).',
        );
    }

    if (!isMainnet() && isMainnetCanisterId(id)) {
        throw new Error(
            `REFUSING to run: this bundle was built for network "${NETWORK}" but ` +
            `${name} is wired to ${id}, a ClearDeck MAINNET canister holding real ` +
            'user funds. Build with the local canister ids ' +
            '(./scripts/dev.sh local-up), or build for mainnet explicitly with ' +
            'DFX_NETWORK=ic. See docs/DEFECTS.md T-01.',
        );
    }

    return id;
}

export const lobbyCanisterId = resolveCanisterId(
    'LOBBY',
    import.meta.env.VITE_CANISTER_ID_LOBBY,
    import.meta.env.CANISTER_ID_LOBBY,
    typeof process !== 'undefined' ? process.env?.CANISTER_ID_LOBBY : undefined,
);
export const historyCanisterId = resolveCanisterId(
    'HISTORY',
    import.meta.env.VITE_CANISTER_ID_HISTORY,
    import.meta.env.CANISTER_ID_HISTORY,
    typeof process !== 'undefined' ? process.env?.CANISTER_ID_HISTORY : undefined,
);

/**
 * Guards a table canister id that came from the lobby at runtime. A compromised
 * or misconfigured local lobby must not be able to point a local build at a
 * mainnet table.
 * @param {unknown} tableCanisterId
 * @returns {string}
 */
function assertTableIdAllowed(tableCanisterId) {
    const id = typeof tableCanisterId === 'string' ? tableCanisterId : String(tableCanisterId);
    if (!isMainnet() && isMainnetCanisterId(id)) {
        throw new Error(
            `REFUSING to open table ${id}: that is a ClearDeck MAINNET canister ` +
            `holding real user funds, and this bundle was built for "${NETWORK}".`,
        );
    }
    return id;
}

// Get current auth state
function getAuthState() {
    let state = null;
    const unsub = auth.subscribe(s => { state = s; });
    unsub();
    return state;
}

// Create an agent - uses authenticated identity if available, anonymous otherwise
async function createAgent() {
    // Network host + mainnet detection are centralized in ./ic-config.js
    // (mainnet agent host = https://icp-api.io, env-overridable).
    const host = agentHost();

    const authState = getAuthState();

    const agentOptions = {
        host,
        // Disable query verification for now - there may be subnet key issues
        verifyQuerySignatures: false,
    };

    // Use authenticated identity if available
    if (authState?.identity) {
        agentOptions.identity = authState.identity;
    }

    const agent = new HttpAgent(agentOptions);

    // Fetch root key for local development (required for certificate verification)
    if (isLocal()) {
        await agent.fetchRootKey();
    }

    return agent;
}

// Create actors that use the current authenticated identity
// Each call creates a fresh actor to pick up identity changes
// Includes network timeout handling to prevent hanging requests
function createAuthenticatedActor(idlFactory, canisterId) {
    return new Proxy({}, {
        get(target, prop) {
            return async (...args) => {
                const agent = await createAgent();
                const actor = Actor.createActor(idlFactory, {
                    agent,
                    canisterId,
                });
                // Wrap the call with a timeout
                return withTimeout(
                    actor[prop](...args),
                    NETWORK_TIMEOUT_MS,
                    `Network request timed out after ${NETWORK_TIMEOUT_MS / 1000}s`
                );
            };
        }
    });
}

// Lobby and history are static canisters
export const lobby = buildingOrTesting
    ? dummyActor()
    : createAuthenticatedActor(lobbyIdlFactory, lobbyCanisterId);

export const history = buildingOrTesting
    ? dummyActor()
    : createAuthenticatedActor(historyIdlFactory, historyCanisterId);

// Table actor factory - creates an actor for a specific table canister
// This is used when joining different tables that each have their own canister
export async function createTableActor(tableCanisterId) {
    if (buildingOrTesting) {
        return dummyActor();
    }

    const agent = await createAgent();
    return Actor.createActor(tableIdlFactory, {
        agent,
        canisterId: assertTableIdAllowed(tableCanisterId),
    });
}

// Create a proxy-style table actor that always uses the latest identity
// but targets a specific canister ID
// Includes network timeout handling to prevent hanging requests
export function createTableActorProxy(tableCanisterId) {
    if (buildingOrTesting) {
        return dummyActor();
    }

    const safeTableId = assertTableIdAllowed(tableCanisterId);

    return new Proxy({}, {
        get(target, prop) {
            return async (...args) => {
                const agent = await createAgent();
                const actor = Actor.createActor(tableIdlFactory, {
                    agent,
                    canisterId: safeTableId,
                });
                // Wrap the call with a timeout
                return withTimeout(
                    actor[prop](...args),
                    NETWORK_TIMEOUT_MS,
                    `Network request timed out after ${NETWORK_TIMEOUT_MS / 1000}s`
                );
            };
        }
    });
}
