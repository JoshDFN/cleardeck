// DERIVE THE DEPOSIT ADDRESS HERE. DO NOT ASK THE CANISTER FOR IT.
//
// docs/SECURITY-FINDINGS.md FINDING 40.
//
// `get_deposit_address()` is an ordinary query. On the Internet Computer a query
// is answered by ONE replica and carries no certificate this client verifies, so
// a single dishonest replica can reply with somebody else's deposit address and
// the player pays it. That is a working theft with no canister bug in it at all:
// an auditor had Alice send 1 ICP to Bob's subaccount and Bob swept it, and every
// money invariant in the project stayed green, because not one e8 went missing.
//
// A certified query would fix the substitution. Deriving the address locally is
// strictly stronger and much smaller, because it removes the canister from the
// trust path instead of hardening it: the address is a pure function of
//
//     (the table canister id, YOUR principal)
//
// >>> CORRECTION, wave 14 (docs/SECURITY-FINDINGS.md FINDING 42).
// >>> This comment used to finish that sentence with "both of which this client
// >>> already holds -- the canister id from the build's own configuration". THE
// >>> CANISTER ID DID NOT COME FROM THE BUILD'S CONFIGURATION. It came from
// >>> `lobby.get_tables()`, which is an ordinary query with exactly the property
// >>> this file exists to avoid, and substituting it moved every derived address
// >>> into a canister the attacker controls while the cross-check below still
// >>> passed. Deriving locally from a wire-supplied id removed the TABLE
// >>> canister from the trust path and left the LOBBY in it.
// >>>
// >>> The id is now checked against `./trustedTables.js` -- the ids this build
// >>> was published with -- before any address is derived from it. Use
// >>> `deriveTrustedDepositAddress()`. `deriveDepositAddress()` below is
// >>> ARITHMETIC ONLY and decides nothing about trust.
//
// The principal comes from the signed-in identity, so once the id is pinned
// there is no reply to substitute. This module is that function. It makes NO
// network calls, on purpose; a network call is exactly what it exists to avoid.
//
//     subaccount = sha256("cleardeck-deposit:" || principal_bytes)
//     hash       = sha224(0x0A || "account-id" || canister_bytes || subaccount)
//     address    = hex( crc32_be(hash) || hash )
//
// The canister computes the same two lines in `compute_deposit_subaccount` and
// `compute_account_identifier` (src/table_canister/src/lib.rs), and
// `tests/money_safety/tests/deposit_surface.rs` proves a THIRD independent
// implementation lands on the same account on the real ICP ledger.

import { Principal } from '@dfinity/principal';
import { sha256 } from '@noble/hashes/sha2.js';
import { sha224 } from '@noble/hashes/sha2.js';
import { assertTrustedTableId } from './trustedTables.js';

/// The domain separator the canister hashes into every deposit subaccount.
const DEPOSIT_DOMAIN = 'cleardeck-deposit:';

/// The ICP account-identifier domain separator: one length byte, then the label.
const ACCOUNT_ID_DOMAIN = Uint8Array.from([
  0x0a, 0x61, 0x63, 0x63, 0x6f, 0x75, 0x6e, 0x74, 0x2d, 0x69, 0x64, // \x0A "account-id"
]);

/// Every reply from `get_deposit_address()` that is a refusal rather than an
/// address begins with this. Mirrors `NO_DEPOSIT_ADDRESS` in the canister.
export const NO_ADDRESS_PREFIX = 'NO ADDRESS: ';

/// Standard IEEE CRC-32, big-endian, the same polynomial `crc32fast` uses.
/// Twenty lines rather than a dependency, because a dependency that computes an
/// ADDRESS is a supply chain that can move money.
const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes) {
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i += 1) {
    crc = CRC32_TABLE[(crc ^ bytes[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function toBytes(value, what) {
  if (value instanceof Uint8Array) return value;
  throw new TypeError(`${what} must be a Uint8Array`);
}

function asPrincipal(value, what) {
  if (value == null) throw new TypeError(`${what} is required`);
  if (typeof value === 'string') return Principal.fromText(value);
  if (typeof value.toUint8Array === 'function') return value;
  throw new TypeError(`${what} must be a Principal or its textual form`);
}

function concat(...parts) {
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

function toHex(bytes) {
  let out = '';
  for (let i = 0; i < bytes.length; i += 1) {
    out += bytes[i].toString(16).padStart(2, '0');
  }
  return out;
}

/**
 * `sha256("cleardeck-deposit:" || principal)` -- the 32 subaccount bytes an
 * ICRC-1 wallet needs alongside the canister id.
 *
 * @param {Principal|string} principal the DEPOSITOR's principal, not the canister's
 * @returns {Uint8Array} 32 bytes
 */
export function depositSubaccount(principal) {
  const p = asPrincipal(principal, 'principal');
  const domain = new TextEncoder().encode(DEPOSIT_DOMAIN);
  return sha256(concat(domain, toBytes(p.toUint8Array(), 'principal bytes')));
}

/**
 * The ICP account identifier of `(owner, subaccount)`, as 64 lowercase hex
 * characters. This is the form a legacy/NNS-style wallet and every exchange
 * withdrawal form accepts.
 *
 * @param {Principal|string} owner
 * @param {Uint8Array|null} subaccount 32 bytes, or null for the main account
 * @returns {string}
 */
export function accountIdentifierHex(owner, subaccount) {
  const p = asPrincipal(owner, 'owner');
  const sub = subaccount ?? new Uint8Array(32);
  if (sub.length !== 32) {
    throw new RangeError(`a subaccount is 32 bytes; this one is ${sub.length}`);
  }
  const hash = sha224(concat(ACCOUNT_ID_DOMAIN, p.toUint8Array(), sub));
  const crc = crc32(hash);
  const prefix = Uint8Array.from([
    (crc >>> 24) & 0xff,
    (crc >>> 16) & 0xff,
    (crc >>> 8) & 0xff,
    crc & 0xff,
  ]);
  return toHex(concat(prefix, hash));
}

/**
 * ARITHMETIC ONLY. The account identifier that `(tableCanisterId, principal)`
 * hashes to, with no network call -- and NO judgement about whether
 * `tableCanisterId` is a canister this build has ever heard of.
 *
 * **The app must not call this.** It exists so the derivation can be compared,
 * principal by principal, against the canister's own two lines
 * (`tests/money_safety/tests/deposit_surface.rs`), which needs an id the build
 * was never published with -- a fresh PocketIC canister. Anything that leads to
 * a player sending money must go through {@link deriveTrustedDepositAddress}:
 * an address derived from an unpinned id is FINDING 42, and it is a completed
 * theft with every money invariant in this project silent.
 *
 * @param {Principal|string} tableCanisterId
 * @param {Principal|string} principal the signed-in player
 * @returns {{ address: string, subaccount: Uint8Array, canisterId: string }}
 */
export function deriveDepositAddress(tableCanisterId, principal) {
  const table = asPrincipal(tableCanisterId, 'tableCanisterId');
  const player = asPrincipal(principal, 'principal');
  if (player.isAnonymous?.()) {
    throw new Error(
      'An anonymous session has no deposit address: the address is derived from your ' +
        'principal, and money sent to the anonymous principal could never be claimed.'
    );
  }
  const subaccount = depositSubaccount(player);
  return {
    address: accountIdentifierHex(table, subaccount),
    subaccount,
    canisterId: table.toText(),
  };
}

/**
 * YOUR deposit address at a table THIS BUILD NAMES, derived with no network call.
 *
 * This is the only derivation the app is allowed to use. The trust root is
 * `./trustedTables.js`: the mainnet ids pinned in this bundle, or for a local
 * build the ids the bundle was compiled against. The canister id that arrives
 * in `lobby.get_tables()` is a QUERY REPLY -- one replica, no certificate this
 * client checks -- so it may name a table on screen and may never be the first
 * argument of this hash. docs/SECURITY-FINDINGS.md FINDING 42.
 *
 * @param {Principal|string} tableCanisterId the id the lobby handed the client
 * @param {Principal|string} principal the signed-in player
 * @returns {{ address: string, subaccount: Uint8Array, canisterId: string }}
 * @throws {Error} written for the player, when the id is not one this build names
 */
export function deriveTrustedDepositAddress(tableCanisterId, principal) {
  assertTrustedTableId(tableCanisterId);
  return deriveDepositAddress(tableCanisterId, principal);
}

/**
 * Compare the locally derived address against what the canister says, WITHOUT
 * ever preferring the canister's answer.
 *
 * The canister's reply is used for one thing only: detecting that the two
 * derivations have drifted. A mismatch is never resolved in the canister's
 * favour.
 *
 * There are two kinds of mismatch and they are NOT the same risk:
 *
 * 1. **The reply is the canister's own MAIN account.** That is the pre-FINDING-34
 *    build answering: it published one shared address to every player. The local
 *    derivation is still correct against such a canister, because
 *    `claim_external_deposit()` has always swept
 *    `sha256("cleardeck-deposit:" || caller)` and that has not changed. So the
 *    address is safe to show, with the reason stated.
 * 2. **Anything else.** Now the client and the canister disagree about the
 *    derivation itself, and the derived address might be one the sweep cannot
 *    reach. Show nothing: the client cannot tell which of the two answers is the
 *    substituted one, and showing either is guessing with the player's money.
 *
 * @param {string} derived the address from {@link deriveDepositAddress}
 * @param {unknown} reported whatever `get_deposit_address()` returned
 * @param {string} [sharedMainAddress] `accountIdentifierHex(tableCanisterId, null)`
 * @returns {{ agrees: boolean, safeToShow: boolean, reason: string|null }}
 */
export function checkAgainstCanister(derived, reported, sharedMainAddress) {
  if (typeof reported !== 'string' || reported.length === 0) {
    return {
      agrees: false,
      safeToShow: false,
      reason: 'this table did not answer when asked to confirm your deposit address.',
    };
  }
  if (reported.toLowerCase() === derived.toLowerCase()) {
    return { agrees: true, safeToShow: true, reason: null };
  }
  if (reported.startsWith(NO_ADDRESS_PREFIX)) {
    return {
      agrees: false,
      safeToShow: false,
      reason: reported.slice(NO_ADDRESS_PREFIX.length),
    };
  }
  if (sharedMainAddress && reported.toLowerCase() === sharedMainAddress.toLowerCase()) {
    return {
      agrees: false,
      safeToShow: true,
      reason:
        'this table is running a build from before the deposit address was made per-player, ' +
        'so it still reports one shared address for everybody. The address shown is yours and ' +
        'the claim still sweeps it; the shared one is not yours and money sent there is only ' +
        'recoverable with notify_deposit(block_index).',
    };
  }
  return {
    agrees: false,
    safeToShow: false,
    reason:
      'the address this table reported is not the one your own principal derives, and it is ' +
      'not this table\'s shared account either. Do not send anything. Either the reply was ' +
      'tampered with in transit, or this build and this canister disagree about how the ' +
      'address is computed.',
  };
}
