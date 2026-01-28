import { Actor, HttpAgent } from "@dfinity/agent";
import { idlFactory as lobbyIdlFactory } from 'declarations/lobby/lobby.did.js';
import { idlFactory as tableIdlFactory } from 'declarations/table_1/table_1.did.js';
import { idlFactory as historyIdlFactory } from 'declarations/history/history.did.js';
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

// Canister IDs from environment
export const lobbyCanisterId = import.meta.env.CANISTER_ID_LOBBY || process.env.CANISTER_ID_LOBBY;
export const historyCanisterId = import.meta.env.CANISTER_ID_HISTORY || process.env.CANISTER_ID_HISTORY;

// Get current auth state
function getAuthState() {
    let state = null;
    const unsub = auth.subscribe(s => { state = s; });
    unsub();
    return state;
}

// Create an agent - uses authenticated identity if available, anonymous otherwise
async function createAgent() {
    // Detect if we're on mainnet by checking the hostname
    // If the page is served from icp0.io, ic0.app, or internetcomputer.org, we're on mainnet
    const isMainnet = typeof window !== 'undefined' &&
        (window.location.hostname.includes('icp0.io') ||
         window.location.hostname.includes('ic0.app') ||
         window.location.hostname.includes('internetcomputer.org'));

    const isLocal = !isMainnet;
    const host = isLocal ? "http://127.0.0.1:4943" : "https://ic0.app";

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
    if (isLocal) {
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
        canisterId: tableCanisterId,
    });
}

// Create a proxy-style table actor that always uses the latest identity
// but targets a specific canister ID
// Includes network timeout handling to prevent hanging requests
export function createTableActorProxy(tableCanisterId) {
    if (buildingOrTesting) {
        return dummyActor();
    }

    return new Proxy({}, {
        get(target, prop) {
            return async (...args) => {
                const agent = await createAgent();
                const actor = Actor.createActor(tableIdlFactory, {
                    agent,
                    canisterId: tableCanisterId,
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
