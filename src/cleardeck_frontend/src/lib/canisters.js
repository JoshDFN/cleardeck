import { safeGetCanisterEnv } from "@icp-sdk/core/agent/canister-env";
import { createActor as createLobbyActor } from '../bindings/lobby_canister';
import { createActor as createTableActor_ } from '../bindings/table_canister';
import { createActor as createHistoryActor } from '../bindings/history_canister';
import { building } from '$app/environment';
import { auth } from './auth.js';

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

// Read canister IDs from ic_env cookie (set by asset canister or dev server)
const canisterEnv = typeof window !== 'undefined' ? safeGetCanisterEnv() : null;

export const lobbyCanisterId = canisterEnv?.["PUBLIC_CANISTER_ID:lobby"];
export const historyCanisterId = canisterEnv?.["PUBLIC_CANISTER_ID:history"];

// Get current auth state
function getAuthState() {
    let state = null;
    const unsub = auth.subscribe(s => { state = s; });
    unsub();
    return state;
}

// Build agentOptions for the current auth state
function getAgentOptions() {
    const isMainnet = typeof window !== 'undefined' &&
        (window.location.hostname.includes('icp0.io') ||
         window.location.hostname.includes('ic0.app') ||
         window.location.hostname.includes('internetcomputer.org'));

    const host = isMainnet ? "https://ic0.app" : window.location.origin;
    const authState = getAuthState();

    const agentOptions = {
        host,
        ...(isMainnet ? {} : { verifyQuerySignatures: false }),
    };

    if (authState?.identity) {
        agentOptions.identity = authState.identity;
    }

    // Provide root key for local development
    if (!isMainnet && canisterEnv?.IC_ROOT_KEY) {
        agentOptions.rootKey = canisterEnv.IC_ROOT_KEY;
    }

    return agentOptions;
}

// Create actors that use the current authenticated identity
// Each call creates a fresh actor to pick up identity changes
// Includes network timeout handling to prevent hanging requests
function createAuthenticatedActorProxy(createActorFn, canisterId) {
    return new Proxy({}, {
        get(target, prop) {
            return async (...args) => {
                const agentOptions = getAgentOptions();
                const actor = createActorFn(canisterId, { agentOptions });
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
    : createAuthenticatedActorProxy(createLobbyActor, lobbyCanisterId);

export const history = buildingOrTesting
    ? dummyActor()
    : createAuthenticatedActorProxy(createHistoryActor, historyCanisterId);

// Table actor factory - creates an actor for a specific table canister
// This is used when joining different tables that each have their own canister
export function createTableActor(tableCanisterId) {
    if (buildingOrTesting) {
        return dummyActor();
    }

    const agentOptions = getAgentOptions();
    return createTableActor_(tableCanisterId, { agentOptions });
}

// Create a proxy-style table actor that always uses the latest identity
// but targets a specific canister ID
// Includes network timeout handling to prevent hanging requests
export function createTableActorProxy(tableCanisterId) {
    if (buildingOrTesting) {
        return dummyActor();
    }

    return createAuthenticatedActorProxy(createTableActor_, tableCanisterId);
}
