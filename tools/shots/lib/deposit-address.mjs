// THE DEPOSIT ADDRESS, DERIVED A SECOND TIME.
//
// src/cleardeck_frontend/src/lib/depositAddress.js derives a player's deposit
// address at a table as `(table canister, sha256("cleardeck-deposit:" ||
// principal bytes))`, and the table canister derives the same subaccount for
// `claim_external_deposit`. This module derives it AGAIN, with node:crypto and
// @dfinity/ledger-icp rather than the app's code, so the address the sheet
// shows is checked against an independent implementation and a transfer the
// harness makes to it lands where the canister will look.

import { createHash } from 'node:crypto';
import { AccountIdentifier, SubAccount } from '@dfinity/ledger-icp';
import { Principal } from '@dfinity/principal';

const DEPOSIT_DOMAIN = 'cleardeck-deposit:';

/**
 * The 32 subaccount bytes of a depositor at any ClearDeck table.
 * @param {string} principalText the DEPOSITOR's principal
 * @returns {Uint8Array}
 */
export function depositSubaccountBytes(principalText) {
  const principal = Principal.fromText(principalText);
  const hash = createHash('sha256');
  hash.update(Buffer.from(DEPOSIT_DOMAIN, 'utf8'));
  hash.update(Buffer.from(principal.toUint8Array()));
  return new Uint8Array(hash.digest());
}

/**
 * The 64-hex account identifier of `(tableCanisterId, depositSubaccount(principal))`.
 * @param {string} tableCanisterId
 * @param {string} principalText
 * @returns {string} lowercase hex
 */
export function depositAddressHex(tableCanisterId, principalText) {
  const sub = SubAccount.fromBytes(depositSubaccountBytes(principalText));
  if (sub instanceof Error) throw sub;
  return AccountIdentifier.fromPrincipal({
    principal: Principal.fromText(tableCanisterId),
    subAccount: sub,
  }).toHex();
}
