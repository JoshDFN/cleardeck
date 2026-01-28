import { writable, derived } from 'svelte/store';
import { AuthClient } from '@dfinity/auth-client';
import { HttpAgent, Actor } from '@dfinity/agent';
import { Principal } from '@dfinity/principal';
import { Ed25519KeyIdentity } from '@dfinity/identity';
import { idlFactory as ledgerIdlFactory } from './ledger.did.js';
import logger from './logger.js';

// Helper to detect if we're on IC mainnet
function isMainnetHostname() {
    return typeof window !== 'undefined' &&
        (window.location.hostname.includes('icp0.io') ||
         window.location.hostname.includes('ic0.app') ||
         window.location.hostname.includes('internetcomputer.org'));
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

                    // Use local II for local dev, production II for mainnet
                    const isLocal = !isMainnetHostname();
                    const identityProvider = isLocal
                        ? `http://${import.meta.env.CANISTER_ID_INTERNET_IDENTITY}.localhost:4943`
                        : 'https://identity.internetcomputer.org';

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
            const isLocal = !isMainnetHostname();
            if (!isLocal) {
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

                    const isLocal = !isMainnetHostname();
                    const host = isLocal ? 'http://127.0.0.1:4943' : 'https://ic0.app';

                    const agent = new HttpAgent({
                        host,
                        identity: state.identity,
                    });

                    if (isLocal) {
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
                const isLocal = !isMainnetHostname();
                const ledgerId = isLocal ? LOCAL_LEDGER_ID : LEDGER_CANISTER_ID;

                // Skip balance fetch if no local ledger is configured
                if (isLocal && !import.meta.env.CANISTER_ID_LEDGER) {
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
