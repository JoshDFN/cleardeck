import { writable, derived } from 'svelte/store';
import { AuthClient } from '@dfinity/auth-client';
import { HttpAgent, Actor } from '@dfinity/agent';
import { Principal } from '@dfinity/principal';
import { Ed25519KeyIdentity } from '@dfinity/identity';
import { idlFactory as ledgerIdlFactory } from './ledger.did.js';
import logger from './logger.js';
import { isLocal, agentHost, II_URL, LOCAL_GATEWAY_PORT } from './ic-config.js';

// Mainnet detection + the II provider / agent-host literals are centralized in
// ./ic-config.js (id.ai/authorize + icp-api.io), env-overridable for rollback.

/**
 * The local Internet Identity origin, or null when this build does not know it.
 *
 * FOUND WHILE VERIFYING THE MAINNET BUNDLE (docs/DEFECTS.md T-39). This line
 * used to be written inline as
 *
 *     `http://${import.meta.env.CANISTER_ID_INTERNET_IDENTITY}.localhost:4943`
 *
 * and `import.meta.env.CANISTER_ID_*` IS NOT DEFINED IN THIS BUILD. Vite exposes
 * only `VITE_`-prefixed variables on `import.meta.env`; vite-plugin-environment
 * puts the `CANISTER_`-prefixed ones on `process.env`. The same file already
 * knows this -- `canisters.js` reads all three spellings for exactly this reason
 * -- but the II URL read only the one that does not exist, so every local build
 * compiled the literal string
 *
 *     "http://undefined.localhost:4943"
 *
 * (confirmed in the emitted chunk), and local sign-in navigated to a host that
 * cannot exist. It never showed up because the screenshot harness authenticates
 * with agent identities and never presses Sign In, and because the MAINNET
 * branch -- the one this wave is shipping -- is unaffected: it takes `II_URL`.
 *
 * The port comes from LOCAL_GATEWAY_PORT for the same reason as the agent host:
 * this project's managed replica is pinned to 8077, not 4943 (docs/DEFECTS.md
 * T-03), so the hard-coded 4943 here was a second, independent way for local
 * sign-in to reach nothing.
 *
 * @returns {string|null}
 */
function localIdentityProvider() {
    const id = import.meta.env.VITE_CANISTER_ID_INTERNET_IDENTITY
        || import.meta.env.CANISTER_ID_INTERNET_IDENTITY
        || (typeof process !== 'undefined' ? process.env?.CANISTER_ID_INTERNET_IDENTITY : undefined);
    if (!id) return null;
    return `http://${id}.localhost:${LOCAL_GATEWAY_PORT}`;
}

// For local dev, we can use a deterministic identity based on a seed
// This avoids the II passkey issues in local development
function createDevIdentity(seed = 'dev-identity-seed-1') {
    const encoder = new TextEncoder();
    const seedBytes = encoder.encode(seed.padEnd(32, '0').slice(0, 32));
    return Ed25519KeyIdentity.generate(seedBytes);
}

// Auth state store
function createAuthStore() {
    const { subscribe, set, update } = writable({
        isAuthenticated: false,
        principal: null,
        identity: null,
        authClient: null,
        isLoading: true,
    });

    return {
        subscribe,

        async init() {
            try {
                const authClient = await AuthClient.create();
                const isAuthenticated = await authClient.isAuthenticated();

                if (isAuthenticated) {
                    const identity = authClient.getIdentity();
                    const principal = identity.getPrincipal();

                    // Check if the delegation is still valid
                    // The delegation chain may have expired even if isAuthenticated returns true
                    let delegationValid = true;
                    try {
                        const delegation = identity.getDelegation?.();
                        if (delegation?.delegations?.length > 0) {
                            const expiration = delegation.delegations[0].delegation.expiration;
                            // expiration is in nanoseconds
                            const expirationMs = Number(expiration / BigInt(1_000_000));
                            const now = Date.now();
                            if (expirationMs < now) {
                                logger.warn('Delegation expired, clearing auth state');
                                delegationValid = false;
                            } else {
                                const hoursRemaining = (expirationMs - now) / (1000 * 60 * 60);
                                logger.info(`Delegation valid for ${hoursRemaining.toFixed(1)} more hours`);
                            }
                        }
                    } catch (e) {
                        // Some identity types may not have getDelegation, that's ok
                        logger.debug('Could not check delegation expiry:', e);
                    }

                    if (delegationValid) {
                        set({
                            isAuthenticated: true,
                            principal: principal.toString(),
                            identity,
                            authClient,
                            isLoading: false,
                        });
                    } else {
                        // Delegation expired, log out
                        await authClient.logout();
                        set({
                            isAuthenticated: false,
                            principal: null,
                            identity: null,
                            authClient,
                            isLoading: false,
                        });
                    }
                } else {
                    set({
                        isAuthenticated: false,
                        principal: null,
                        identity: null,
                        authClient,
                        isLoading: false,
                    });
                }
            } catch (error) {
                logger.error('Auth init error:', error);
                set({
                    isAuthenticated: false,
                    principal: null,
                    identity: null,
                    authClient: null,
                    isLoading: false,
                });
            }
        },

        async login() {
            return new Promise((resolve, reject) => {
                update(state => {
                    if (!state.authClient) {
                        reject(new Error('Auth client not initialized'));
                        return state;
                    }

                    // Use local II for local dev, production II (id.ai) for mainnet
                    const identityProvider = isLocal() ? localIdentityProvider() : II_URL;

                    // An unresolvable local II is an ERROR, not a URL to try.
                    // Navigating to `http://undefined.localhost:4943` opens a
                    // window that can never complete the delegation handshake,
                    // and the user is left looking at a browser error with no
                    // way to tell it from a passkey problem.
                    if (!identityProvider) {
                        reject(new Error(
                            'This local build does not know the Internet Identity canister id, so '
                            + 'sign-in cannot be started. Export CANISTER_ID_INTERNET_IDENTITY (or '
                            + 'VITE_CANISTER_ID_INTERNET_IDENTITY) when building, or bring the stack '
                            + 'up with ./scripts/dev.sh local-up. See docs/DEFECTS.md T-39.',
                        ));
                        return state;
                    }

                    state.authClient.login({
                        identityProvider,
                        maxTimeToLive: BigInt(7 * 24 * 60 * 60 * 1000 * 1000 * 1000), // 7 days
                        onSuccess: () => {
                            const identity = state.authClient.getIdentity();
                            const principal = identity.getPrincipal();

                            set({
                                isAuthenticated: true,
                                principal: principal.toString(),
                                identity,
                                authClient: state.authClient,
                                isLoading: false,
                            });
                            resolve(principal.toString());
                        },
                        onError: (error) => {
                            logger.error('Login error:', error);
                            reject(error);
                        },
                    });

                    return state;
                });
            });
        },

        async logout() {
            update(state => {
                if (state.authClient) {
                    state.authClient.logout();
                }
                return {
                    isAuthenticated: false,
                    principal: null,
                    identity: null,
                    authClient: state.authClient,
                    isLoading: false,
                };
            });
        },

        // Dev login - uses a deterministic identity for local testing
        // This bypasses II entirely, useful when local II has issues
        async devLogin(seed = 'dev-player-1') {
            if (!isLocal()) {
                throw new Error('Dev login only available in local development');
            }

            const identity = createDevIdentity(seed);
            const principal = identity.getPrincipal();

            set({
                isAuthenticated: true,
                principal: principal.toString(),
                identity,
                authClient: null,
                isLoading: false,
            });

            return principal.toString();
        },

        // Get an authenticated agent for making canister calls
        async getAgent() {
            return new Promise((resolve, reject) => {
                update(state => {
                    if (!state.identity) {
                        reject(new Error('Not authenticated'));
                        return state;
                    }

                    const host = agentHost();

                    const agent = new HttpAgent({
                        host,
                        identity: state.identity,
                    });

                    if (isLocal()) {
                        agent.fetchRootKey().then(() => resolve(agent)).catch(reject);
                    } else {
                        resolve(agent);
                    }

                    return state;
                });
            });
        },
    };
}

export const auth = createAuthStore();

// Helper to detect if an error is a signature verification failure
// This typically means the II delegation has expired or is invalid
export function isSignatureError(error) {
    const msg = error?.message || error?.toString() || '';
    return msg.includes('signature could not be verified') ||
           msg.includes('Invalid signature') ||
           msg.includes('EcdsaP256') ||
           msg.includes('delegation') ||
           (error?.status === 400 && msg.includes('signature'));
}

// Wrapper for canister calls that handles signature errors by forcing re-login
export async function withAuthRecovery(fn, onAuthError = null) {
    try {
        return await fn();
    } catch (error) {
        if (isSignatureError(error)) {
            logger.error('Signature verification failed - forcing logout:', error);
            // Clear the invalid auth state
            await auth.logout();
            // Notify caller if they want to handle this (e.g., show a message)
            if (onAuthError) {
                onAuthError('Your session has expired. Please log in again.');
            }
            throw new Error('Session expired. Please log in again.');
        }
        throw error;
    }
}

// Wallet state store
function createWalletStore() {
    const { subscribe, set, update } = writable({
        balance: null,  // ICP balance in e8s (1 ICP = 100_000_000 e8s)
        isLoading: false,
        error: null,
    });

    // ICP Ledger canister ID
    const LEDGER_CANISTER_ID = 'ryjl3-tyaaa-aaaaa-aaaba-cai';  // Mainnet ICP ledger
    const LOCAL_LEDGER_ID = import.meta.env.CANISTER_ID_LEDGER || LEDGER_CANISTER_ID;

    return {
        subscribe,

        async refreshBalance() {
            update(s => ({ ...s, isLoading: true, error: null }));

            try {
                const agent = await auth.getAgent();
                const local = isLocal();
                const ledgerId = local ? LOCAL_LEDGER_ID : LEDGER_CANISTER_ID;

                // Skip balance fetch if no local ledger is configured
                if (local && !import.meta.env.CANISTER_ID_LEDGER) {
                    set({
                        balance: BigInt(0),
                        isLoading: false,
                        error: null,
                    });
                    return BigInt(0);
                }

                const ledger = Actor.createActor(ledgerIdlFactory, {
                    agent,
                    canisterId: ledgerId,
                });

                // Get the user's principal from identity
                let identity;
                auth.subscribe(s => { identity = s.identity; })();

                if (!identity) {
                    // Not authenticated - silently return null balance instead of throwing
                    set({
                        balance: null,
                        isLoading: false,
                        error: null,
                    });
                    return null;
                }

                // Query balance using the actual Principal object
                const principalObj = identity.getPrincipal();
                const balance = await ledger.icrc1_balance_of({
                    owner: principalObj,
                    subaccount: [],
                });

                set({
                    balance: balance,
                    isLoading: false,
                    error: null,
                });

                return balance;
            } catch (error) {
                logger.error('Balance fetch error:', error);

                // Check if this is a signature error (expired delegation)
                if (isSignatureError(error)) {
                    logger.warn('Signature verification failed during balance fetch - session may be expired');
                    await auth.logout();
                    set({
                        balance: null,
                        isLoading: false,
                        error: 'Session expired. Please log in again.',
                    });
                    return null;
                }

                set({
                    balance: null,
                    isLoading: false,
                    error: error.message,
                });
                return null;
            }
        },

        // Format balance for display (e8s to ICP)
        formatICP(e8s) {
            if (e8s === null || e8s === undefined) return '-.--';
            const icp = Number(e8s) / 100_000_000;
            return icp.toFixed(4);
        },

        // Convert ICP to e8s
        toE8s(icp) {
            return BigInt(Math.floor(icp * 100_000_000));
        },
    };
}

export const wallet = createWalletStore();

// Derived store for formatted balance
export const formattedBalance = derived(wallet, $wallet => {
    if ($wallet.balance === null) return '-.--';
    const icp = Number($wallet.balance) / 100_000_000;
    return icp.toFixed(4);
});
