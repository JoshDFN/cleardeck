// ===========================================================================
// THE TRUST ROOT FOR ANY CANISTER ID THAT MONEY IS SENT TO.
//
// docs/SECURITY-FINDINGS.md FINDING 42.
// ===========================================================================
//
// WHY THE WIRE IS NOT THE TRUST ROOT
// ---------------------------------------------------------------------------
// A ClearDeck deposit address is a pure function of two things:
//
//     account_identifier( TABLE CANISTER ID , sha256("cleardeck-deposit:" || YOUR PRINCIPAL) )
//
// FINDING 40 removed the table canister from the trust path for the SECOND
// argument -- the client stopped asking `get_deposit_address()` and derived the
// address itself -- and its decision table justified that with the sentence
// "the canister id [comes] from the build's own configuration". That sentence
// was false. The id came from `lobby.get_tables()`:
//
//     routes/+page.svelte:307   let lobbyTables = await lobby.get_tables();
//     lobby_canister.did:61     get_tables : () -> (vec TableInfo) query;
//
// A `query` on the Internet Computer is answered by ONE replica out of its own
// memory and carries no certificate this client verifies. So the FIRST argument
// of the derivation was attacker-controllable, and substituting it moves every
// derived address with it -- into a canister the attacker controls, whose
// controller can pay its whole ledger balance to itself with one
// `install_code` (the capability FINDING 23 demonstrated at 39.9999 ICP).
//
// The modal's own cross-check could not see it: the check asks the canister
// named by the SAME substituted id, and a substituted canister answers
// consistently about itself. Reproduced on rendered pixels, with every check on
// the screen green, by `node tools/shots/repro-finding42.mjs`.
//
// WHAT THIS MODULE IS
// ---------------------------------------------------------------------------
// The set of table canister ids THIS BUILD WAS PRODUCED AGAINST. Nothing here
// is read from the network, ever. A canister id that arrives over the wire may
// be used to DISPLAY a table; it may never be used to derive an address that
// money is sent to, and this module is what makes that difference expressible.
//
//   mainnet build   the frozen list in ic-config.js, which is kept in step with
//                   .icp/data/mappings/ic.ids.json -- the file `icp -e ic`
//                   itself resolves and the one a verifier can read at a commit.
//   local build     the ids compiled in at build time by whatever deployed the
//                   stack (`./scripts/dev.sh local-up` ->
//                   tools/shots/lib/frontend-build.mjs, which exports
//                   CANISTER_ID_TABLE_*). That is "the id mapping the build was
//                   produced against", which is the local equivalent of the
//                   pinned list.
//
// A build with NO table ids compiled in has an EMPTY trusted set, and the
// deposit path then refuses everything and says so in the player's words. That
// is deliberate: the failure mode of a missing trust root must be "you cannot
// deposit", never "we will trust whatever the lobby said".

import { buildValue, isMainnet, MAINNET_TABLE_IDS, NETWORK } from './ic-config.js';

/**
 * Build-time table ids for a NON-mainnet build.
 *
 * Every read is a literal member expression so vite's `define` substitutes it
 * (see `buildValue` in ic-config.js -- a dynamic `env[name]` lookup would
 * compile to `undefined` and empty this set silently).
 *
 * `VITE_TRUSTED_TABLE_IDS` is the explicit, comma-separated form, for a stack
 * with more tables than the four named below. It is a BUILD-TIME variable: it
 * is baked into the bundle by the deploy that produced it, so it is part of the
 * build's configuration and not something a running app can be told.
 *
 * @returns {string[]}
 */
function buildTimeTableIds() {
  const explicit = buildValue(() => import.meta.env.VITE_TRUSTED_TABLE_IDS)
    || buildValue(() => process.env.VITE_TRUSTED_TABLE_IDS)
    || buildValue(() => process.env.CLEARDECK_TRUSTED_TABLE_IDS);
  const fromList = explicit ? explicit.split(',').map((s) => s.trim()).filter(Boolean) : [];

  const named = [
    buildValue(() => import.meta.env.VITE_CANISTER_ID_TABLE_1)
      || buildValue(() => process.env.CANISTER_ID_TABLE_1),
    buildValue(() => import.meta.env.VITE_CANISTER_ID_TABLE_2)
      || buildValue(() => process.env.CANISTER_ID_TABLE_2),
    buildValue(() => import.meta.env.VITE_CANISTER_ID_TABLE_3)
      || buildValue(() => process.env.CANISTER_ID_TABLE_3),
    buildValue(() => import.meta.env.VITE_CANISTER_ID_BTC_TABLE_1)
      || buildValue(() => process.env.CANISTER_ID_BTC_TABLE_1),
  ].filter(Boolean);

  return [...new Set([...fromList, ...named])];
}

/**
 * Every table canister id this build is willing to send money to.
 * @type {readonly string[]}
 */
export const TRUSTED_TABLE_IDS = Object.freeze(
  isMainnet() ? [...MAINNET_TABLE_IDS] : buildTimeTableIds(),
);

/**
 * Where the list above came from, for the message a refused player reads and
 * for the gate. A trust root nobody can name is not one.
 * @type {string}
 */
export const TRUSTED_TABLE_SOURCE = isMainnet()
  ? 'the mainnet canister ids pinned in this build (ic-config.js / .icp/data/mappings/ic.ids.json)'
  : `the ${NETWORK} canister ids this bundle was built against`;

/** @param {unknown} id @returns {string} the textual form of a Principal or string */
export function idText(id) {
  if (typeof id === 'string') return id;
  if (id && typeof (/** @type {any} */ (id).toText) === 'function') {
    return /** @type {any} */ (id).toText();
  }
  return String(id ?? '');
}

/**
 * @param {unknown} id
 * @returns {boolean} true when `id` is a table this build itself names.
 */
export function isTrustedTableId(id) {
  const text = idText(id);
  return text.length > 0 && TRUSTED_TABLE_IDS.includes(text);
}

/**
 * The refusal, in the player's words. Kept as a function rather than a thrown
 * string so both the modal and the gate render the same sentence.
 *
 * @param {unknown} id the canister id that arrived over the wire
 * @returns {string}
 */
export function untrustedTableMessage(id) {
  const text = idText(id) || '(no canister id at all)';
  if (TRUSTED_TABLE_IDS.length === 0) {
    return (
      `This build has no table canister ids compiled into it, so it cannot tell whether ${text} `
      + 'is a ClearDeck table or somebody else\'s canister. No deposit address is shown and no '
      + 'deposit can be made: an address derived from an unverified canister id is an address '
      + 'somebody else can empty. Rebuild with the ids of the tables you deployed '
      + '(./scripts/dev.sh local-up does this).'
    );
  }
  return (
    `Nothing can be deposited to ${text}: this table\'s canister id was sent to your browser by `
    + 'the lobby, over an ordinary query that carries no proof, and it is NOT one of the '
    + `canister ids this build was published with. Your deposit address is computed FROM that id, `
    + 'so if the id is wrong the address belongs to whoever owns that canister and the money is '
    + `theirs the moment you send it. The tables this build knows about are: `
    + `${TRUSTED_TABLE_IDS.join(', ')} (${TRUSTED_TABLE_SOURCE}).`
  );
}

/**
 * Throws unless `id` is a table this build itself names.
 *
 * Call this BEFORE deriving a deposit address, before addressing a transfer,
 * and before approving a spender. It is the only thing standing between a
 * substituted query reply and a completed theft.
 *
 * @param {unknown} id
 * @returns {string} the id, textual, when it is trusted
 * @throws {Error} with a message written for the player
 */
export function assertTrustedTableId(id) {
  if (!isTrustedTableId(id)) throw new Error(untrustedTableMessage(id));
  return idText(id);
}
