// THE LEDGER CALLS THE DEPOSIT SHEET MAKES, IN ONE PLACE.
//
// Two reads and one approval. `readLedgerBalance` is `icrc1_balance_of` for
// any account (the signed-in wallet, or the derived deposit subaccount the
// address route watches); `approveSpender` is the `icrc2_approve` the
// Internet Identity route makes before the table pulls the amount. The
// arguments of the approval are exactly what DepositModal.svelte sent before
// this module existed: the amount, the table canister as the spender, and
// nothing else set (docs/SECURITY-FINDINGS.md FINDING 42 is checked by the
// caller before this is reached). Only the IDL declarations moved.

import { Actor } from '@dfinity/agent';

/** The ICP ledger and the ckBTC ledger, by currency. */
export const ICP_LEDGER_CANISTER = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
export const CKBTC_LEDGER_CANISTER = 'mxzaz-hqaaa-aaaar-qaada-cai';

/** @param {'ICP'|'BTC'} currency */
export function ledgerCanisterFor(currency) {
  return currency === 'BTC' ? CKBTC_LEDGER_CANISTER : ICP_LEDGER_CANISTER;
}

const balanceIdlFactory = ({ IDL }) => {
  const Account = IDL.Record({
    owner: IDL.Principal,
    subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  return IDL.Service({
    icrc1_balance_of: IDL.Func([Account], [IDL.Nat], ['query']),
  });
};

const approveIdlFactory = ({ IDL }) => {
  const Account = IDL.Record({
    owner: IDL.Principal,
    subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const ApproveArgs = IDL.Record({
    fee: IDL.Opt(IDL.Nat),
    memo: IDL.Opt(IDL.Vec(IDL.Nat8)),
    from_subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
    created_at_time: IDL.Opt(IDL.Nat64),
    amount: IDL.Nat,
    expected_allowance: IDL.Opt(IDL.Nat),
    expires_at: IDL.Opt(IDL.Nat64),
    spender: Account,
  });
  const ApproveError = IDL.Variant({
    GenericError: IDL.Record({ message: IDL.Text, error_code: IDL.Nat }),
    TemporarilyUnavailable: IDL.Null,
    Duplicate: IDL.Record({ duplicate_of: IDL.Nat }),
    BadFee: IDL.Record({ expected_fee: IDL.Nat }),
    AllowanceChanged: IDL.Record({ current_allowance: IDL.Nat }),
    CreatedInFuture: IDL.Record({ ledger_time: IDL.Nat64 }),
    TooOld: IDL.Null,
    Expired: IDL.Record({ ledger_time: IDL.Nat64 }),
    InsufficientFunds: IDL.Record({ balance: IDL.Nat }),
  });
  const ApproveResult = IDL.Variant({ Ok: IDL.Nat, Err: ApproveError });

  return IDL.Service({
    icrc2_approve: IDL.Func([ApproveArgs], [ApproveResult], []),
  });
};

/**
 * `icrc1_balance_of` for one account, as a bigint.
 *
 * @param {import('@dfinity/agent').HttpAgent} agent
 * @param {string} ledgerCanisterId
 * @param {{owner: import('@dfinity/principal').Principal, subaccount?: Uint8Array|number[]|null}} account
 * @returns {Promise<bigint>}
 */
export async function readLedgerBalance(agent, ledgerCanisterId, { owner, subaccount = null }) {
  const ledger = Actor.createActor(balanceIdlFactory, { agent, canisterId: ledgerCanisterId });
  const balance = await ledger.icrc1_balance_of({
    owner,
    subaccount: subaccount ? [subaccount] : [],
  });
  return BigInt(balance);
}

/**
 * `icrc2_approve`: the signed-in account lets `spender` pull `amount` (the
 * deposit plus one ledger fee, the caller's arithmetic). Returns the ledger's
 * own `{ Ok } | { Err }` untouched, so the caller keeps reading the error
 * variants it always did.
 *
 * @param {import('@dfinity/agent').HttpAgent} agent
 * @param {string} ledgerCanisterId
 * @param {{amount: bigint, spender: import('@dfinity/principal').Principal}} p
 */
export async function approveSpender(agent, ledgerCanisterId, { amount, spender }) {
  const ledger = Actor.createActor(approveIdlFactory, { agent, canisterId: ledgerCanisterId });
  return ledger.icrc2_approve({
    fee: [],
    memo: [],
    from_subaccount: [],
    created_at_time: [],
    amount,
    expected_allowance: [],
    expires_at: [],
    spender: {
      owner: spender,
      subaccount: [],
    },
  });
}
