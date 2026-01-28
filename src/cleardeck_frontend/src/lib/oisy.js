/**
 * OISY Wallet Integration for ClearDeck Poker
 *
 * This module provides "Top up with OISY" functionality, allowing users to
 * deposit funds from their OISY wallet without needing to transfer tokens first.
 *
 * Flow:
 * 1. User connects OISY wallet (one popup for permission)
 * 2. User enters deposit amount
 * 3. User approves icrc2_approve via OISY popup
 * 4. Frontend calls table_canister.deposit() which pulls funds via icrc2_transfer_from
 */

import { writable, get } from 'svelte/store';
import { IcpWallet } from '@dfinity/oisy-wallet-signer/icp-wallet';
import { IcrcWallet } from '@dfinity/oisy-wallet-signer/icrc-wallet';
import { Principal } from '@dfinity/principal';
import { Actor, HttpAgent } from '@dfinity/agent';
import logger from './logger.js';

// OISY Wallet URLs
const OISY_MAINNET_URL = 'https://oisy.com/sign';
const OISY_STAGING_URL = 'https://staging.oisy.com/sign';

// Ledger canister IDs
const ICP_LEDGER_CANISTER = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
const CKBTC_LEDGER_CANISTER = 'mxzaz-hqaaa-aaaar-qaada-cai';

// Helper to detect mainnet
function isMainnet() {
    return typeof window !== 'undefined' &&
        (window.location.hostname.includes('icp0.io') ||
         window.location.hostname.includes('ic0.app') ||
         window.location.hostname.includes('internetcomputer.org'));
}

// OISY wallet store
function createOisyStore() {
    const { subscribe, set, update } = writable({
        isConnected: false,
        isConnecting: false,
        principal: null,
        accounts: null,
        wallet: null, // IcpWallet or IcrcWallet instance
        walletType: null, // 'icp' or 'icrc'
        error: null,
        icpBalance: null,
        ckbtcBalance: null,
        loadingBalances: false,
    });

    let currentWallet = null;

    return {
        subscribe,

        /**
         * Connect to OISY wallet for ICP operations
         */
        async connectForIcp() {
            update(s => ({ ...s, isConnecting: true, error: null }));

            try {
                const url = isMainnet() ? OISY_MAINNET_URL : OISY_STAGING_URL;
                const host = isMainnet() ? 'https://icp-api.io' : 'http://localhost:4943';

                logger.info('Connecting to OISY wallet at:', url, 'with host:', host);

                const wallet = await IcpWallet.connect({
                    url,
                    host,
                    windowOptions: {
                        position: 'center',
                        width: 500,
                        height: 700,
                    },
                    connectionOptions: {
                        timeoutInMilliseconds: 120000, // 2 minute timeout for connection
                    },
                    onDisconnect: () => {
                        logger.info('OISY wallet disconnected');
                        set({
                            isConnected: false,
                            isConnecting: false,
                            principal: null,
                            accounts: null,
                            wallet: null,
                            walletType: null,
                            error: null,
                            icpBalance: null,
                            ckbtcBalance: null,
                            loadingBalances: false,
                        });
                        currentWallet = null;
                    },
                });

                currentWallet = wallet;

                // Get accounts - permissions should be granted during connect
                logger.info('OISY wallet connected, fetching accounts...');
                const accounts = await wallet.accounts();
                logger.info('OISY accounts received:', accounts?.length || 0);

                if (!accounts || accounts.length === 0) {
                    throw new Error('No accounts returned from OISY wallet');
                }

                const primaryAccount = accounts[0];
                const principal = primaryAccount.owner;

                logger.info('Connected to OISY wallet, principal:', principal);

                set({
                    isConnected: true,
                    isConnecting: false,
                    principal,
                    accounts,
                    wallet,
                    walletType: 'icp',
                    error: null,
                    icpBalance: null,
                    ckbtcBalance: null,
                    loadingBalances: false,
                });

                // Fetch balances after connection
                await this.refreshBalances();

                return { principal, accounts };
            } catch (error) {
                logger.error('Failed to connect OISY wallet:', error);

                let errorMessage = 'Failed to connect to OISY wallet';
                if (error.message?.includes('timeout')) {
                    errorMessage = 'Connection timed out. Please try again.';
                } else if (error.message?.includes('closed')) {
                    errorMessage = 'Wallet popup was closed. Please try again.';
                } else if (error.message?.includes('rejected')) {
                    errorMessage = 'Connection was rejected. Please approve the connection in OISY.';
                } else if (error.message) {
                    errorMessage = error.message;
                }

                set({
                    isConnected: false,
                    isConnecting: false,
                    principal: null,
                    accounts: null,
                    wallet: null,
                    walletType: null,
                    error: errorMessage,
                    icpBalance: null,
                    ckbtcBalance: null,
                    loadingBalances: false,
                });
                currentWallet = null;
                throw error;
            }
        },

        /**
         * Connect to OISY wallet for ICRC token operations (ckBTC)
         */
        async connectForIcrc() {
            update(s => ({ ...s, isConnecting: true, error: null }));

            try {
                const url = isMainnet() ? OISY_MAINNET_URL : OISY_STAGING_URL;
                const host = isMainnet() ? 'https://icp-api.io' : 'http://localhost:4943';

                logger.info('Connecting to OISY wallet (ICRC) at:', url, 'with host:', host);

                const wallet = await IcrcWallet.connect({
                    url,
                    host,
                    windowOptions: {
                        position: 'center',
                        width: 500,
                        height: 700,
                    },
                    connectionOptions: {
                        timeoutInMilliseconds: 120000,
                    },
                    onDisconnect: () => {
                        logger.info('OISY wallet disconnected');
                        set({
                            isConnected: false,
                            isConnecting: false,
                            principal: null,
                            accounts: null,
                            wallet: null,
                            walletType: null,
                            error: null,
                            icpBalance: null,
                            ckbtcBalance: null,
                            loadingBalances: false,
                        });
                        currentWallet = null;
                    },
                });

                currentWallet = wallet;

                // Get accounts - permissions should be granted during connect
                logger.info('OISY wallet (ICRC) connected, fetching accounts...');
                const accounts = await wallet.accounts();
                logger.info('OISY (ICRC) accounts received:', accounts?.length || 0);

                if (!accounts || accounts.length === 0) {
                    throw new Error('No accounts returned from OISY wallet');
                }

                const primaryAccount = accounts[0];
                const principal = primaryAccount.owner;

                logger.info('Connected to OISY wallet (ICRC), principal:', principal);

                set({
                    isConnected: true,
                    isConnecting: false,
                    principal,
                    accounts,
                    wallet,
                    walletType: 'icrc',
                    error: null,
                    icpBalance: null,
                    ckbtcBalance: null,
                    loadingBalances: false,
                });

                await this.refreshBalances();

                return { principal, accounts };
            } catch (error) {
                logger.error('Failed to connect OISY wallet (ICRC):', error);

                let errorMessage = 'Failed to connect to OISY wallet';
                if (error.message?.includes('timeout')) {
                    errorMessage = 'Connection timed out. Please try again.';
                } else if (error.message?.includes('closed')) {
                    errorMessage = 'Wallet popup was closed. Please try again.';
                } else if (error.message?.includes('rejected')) {
                    errorMessage = 'Connection was rejected. Please approve the connection in OISY.';
                } else if (error.message) {
                    errorMessage = error.message;
                }

                set({
                    isConnected: false,
                    isConnecting: false,
                    principal: null,
                    accounts: null,
                    wallet: null,
                    walletType: null,
                    error: errorMessage,
                    icpBalance: null,
                    ckbtcBalance: null,
                    loadingBalances: false,
                });
                currentWallet = null;
                throw error;
            }
        },

        /**
         * Disconnect from OISY wallet
         */
        async disconnect() {
            if (currentWallet) {
                try {
                    await currentWallet.disconnect();
                } catch (e) {
                    logger.warn('Error during disconnect:', e);
                }
                currentWallet = null;
            }

            set({
                isConnected: false,
                isConnecting: false,
                principal: null,
                accounts: null,
                wallet: null,
                walletType: null,
                error: null,
                icpBalance: null,
                ckbtcBalance: null,
                loadingBalances: false,
            });
        },

        /**
         * Refresh ICP and ckBTC balances from OISY wallet
         */
        async refreshBalances() {
            const state = get(this);
            if (!state.isConnected || !state.principal) {
                return;
            }

            update(s => ({ ...s, loadingBalances: true }));

            try {
                // Create an anonymous agent for balance queries
                const host = isMainnet() ? 'https://ic0.app' : 'http://127.0.0.1:4943';
                const agent = new HttpAgent({ host });

                if (!isMainnet()) {
                    await agent.fetchRootKey();
                }

                const ledgerIdlFactory = ({ IDL }) => {
                    const Account = IDL.Record({
                        owner: IDL.Principal,
                        subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
                    });
                    return IDL.Service({
                        icrc1_balance_of: IDL.Func([Account], [IDL.Nat], ['query']),
                    });
                };

                const ownerPrincipal = typeof state.principal === 'string'
                    ? Principal.fromText(state.principal)
                    : state.principal;

                // Fetch ICP balance
                const icpLedger = Actor.createActor(ledgerIdlFactory, {
                    agent,
                    canisterId: ICP_LEDGER_CANISTER,
                });

                const icpBalance = await icpLedger.icrc1_balance_of({
                    owner: ownerPrincipal,
                    subaccount: [],
                });

                // Fetch ckBTC balance
                const ckbtcLedger = Actor.createActor(ledgerIdlFactory, {
                    agent,
                    canisterId: CKBTC_LEDGER_CANISTER,
                });

                const ckbtcBalance = await ckbtcLedger.icrc1_balance_of({
                    owner: ownerPrincipal,
                    subaccount: [],
                });

                update(s => ({
                    ...s,
                    icpBalance: Number(icpBalance),
                    ckbtcBalance: Number(ckbtcBalance),
                    loadingBalances: false,
                }));

                logger.info('OISY balances:', { icp: Number(icpBalance), ckbtc: Number(ckbtcBalance) });
            } catch (error) {
                logger.error('Failed to fetch OISY balances:', error);
                update(s => ({ ...s, loadingBalances: false }));
            }
        },

        /**
         * Approve ICP spending via OISY wallet (for deposits)
         * @param {bigint} amount - Amount in e8s to approve
         * @param {string} spender - The canister ID that will be allowed to spend
         * @returns {Promise<bigint>} Block height of the approval transaction
         */
        async approveIcp(amount, spender) {
            const state = get(this);

            if (!state.isConnected || !state.wallet) {
                throw new Error('OISY wallet not connected');
            }

            if (state.walletType !== 'icp') {
                // Need to reconnect with IcpWallet
                await this.disconnect();
                await this.connectForIcp();
            }

            const wallet = currentWallet;
            if (!wallet || !wallet.icrc2Approve) {
                throw new Error('Wallet does not support icrc2Approve');
            }

            logger.info('Requesting ICP approval via OISY:', { amount: amount.toString(), spender });

            try {
                const spenderPrincipal = typeof spender === 'string'
                    ? Principal.fromText(spender)
                    : spender;

                logger.info('Calling wallet.icrc2Approve with:', {
                    spenderOwner: spenderPrincipal.toString(),
                    amount: amount.toString(),
                    owner: typeof state.principal === 'string' ? state.principal : state.principal?.toString(),
                    ledgerCanisterId: ICP_LEDGER_CANISTER,
                });

                const blockHeight = await wallet.icrc2Approve({
                    request: {
                        spender: {
                            owner: spenderPrincipal,
                            subaccount: [],
                        },
                        amount,
                    },
                    owner: state.principal,
                    ledgerCanisterId: ICP_LEDGER_CANISTER,
                    options: {
                        timeoutInMilliseconds: 300000, // 5 minute timeout for user approval
                    },
                });

                logger.info('ICP approval successful, block height:', blockHeight);

                if (!blockHeight && blockHeight !== 0n) {
                    logger.warn('icrc2Approve returned without block height:', blockHeight);
                }

                // Refresh balances after approval
                await this.refreshBalances();

                return blockHeight;
            } catch (error) {
                logger.error('ICP approval failed:', error);

                let errorMessage = 'Failed to approve ICP spending';
                if (error.message?.includes('rejected') || error.message?.includes('denied')) {
                    errorMessage = 'Approval was rejected in OISY wallet';
                } else if (error.message?.includes('timeout')) {
                    errorMessage = 'Approval timed out. Please try again.';
                } else if (error.message?.includes('insufficient')) {
                    errorMessage = 'Insufficient balance in OISY wallet';
                } else if (error.message) {
                    errorMessage = error.message;
                }

                throw new Error(errorMessage);
            }
        },

        /**
         * Approve ckBTC spending via OISY wallet (for deposits)
         * @param {bigint} amount - Amount in sats to approve
         * @param {string} spender - The canister ID that will be allowed to spend
         * @returns {Promise<bigint>} Block height of the approval transaction
         */
        async approveCkbtc(amount, spender) {
            const state = get(this);

            if (!state.isConnected || !state.wallet) {
                throw new Error('OISY wallet not connected');
            }

            if (state.walletType !== 'icrc') {
                // Need to reconnect with IcrcWallet
                await this.disconnect();
                await this.connectForIcrc();
            }

            const wallet = currentWallet;
            if (!wallet || !wallet.approve) {
                throw new Error('Wallet does not support approve');
            }

            logger.info('Requesting ckBTC approval via OISY:', { amount: amount.toString(), spender });

            try {
                const spenderPrincipal = typeof spender === 'string'
                    ? Principal.fromText(spender)
                    : spender;

                const blockIndex = await wallet.approve({
                    params: {
                        spender: {
                            owner: spenderPrincipal,
                            subaccount: [],
                        },
                        amount,
                    },
                    owner: state.principal,
                    ledgerCanisterId: CKBTC_LEDGER_CANISTER,
                    options: {
                        timeoutInMilliseconds: 300000,
                    },
                });

                logger.info('ckBTC approval successful, block index:', blockIndex);

                await this.refreshBalances();

                return blockIndex;
            } catch (error) {
                logger.error('ckBTC approval failed:', error);

                let errorMessage = 'Failed to approve ckBTC spending';
                if (error.message?.includes('rejected') || error.message?.includes('denied')) {
                    errorMessage = 'Approval was rejected in OISY wallet';
                } else if (error.message?.includes('timeout')) {
                    errorMessage = 'Approval timed out. Please try again.';
                } else if (error.message?.includes('insufficient')) {
                    errorMessage = 'Insufficient balance in OISY wallet';
                } else if (error.message) {
                    errorMessage = error.message;
                }

                throw new Error(errorMessage);
            }
        },

        /**
         * Get the current wallet instance
         */
        getWallet() {
            return currentWallet;
        },
    };
}

export const oisy = createOisyStore();

// Helper to format balance
export function formatOisyBalance(balance, currency = 'ICP') {
    if (balance === null || balance === undefined) return '...';
    const num = Number(balance);
    if (currency === 'BTC' || currency === 'ckBTC') {
        const btc = num / 100_000_000;
        if (btc >= 1) return `${btc.toFixed(4)} BTC`;
        if (num >= 1000) return `${(num / 1000).toFixed(1)}K sats`;
        return `${num} sats`;
    }
    return (num / 100_000_000).toFixed(4) + ' ICP';
}
