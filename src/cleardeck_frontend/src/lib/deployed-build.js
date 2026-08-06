// WHAT CODE IS RUNNING IN THE CANISTERS THIS BUNDLE TALKS TO.
//
// ---------------------------------------------------------------------------
// WHY THIS FILE DOES NOT SIMPLY STATE THE HASHES
// ---------------------------------------------------------------------------
//
// The "Verify the Code" panel used to carry three hard-coded hashes under the
// heading "Deployed Canister Hashes". That heading is a claim about the state of
// the world made by a static file, and a static file cannot know the state of the
// world. The three it carried
//
//   lobby   0xff6c893de860c5bd8dae85d67344ee94619fb6faad6d68b3265c9a6fe5a2cef8
//   tables  0x1b84e2fa1c35fd50001cb059ba644784fe5a6b36a093a2ac3e56c39bc3bbdf28
//   history 0xc9b1b78a6490cd2034b967dc9de11bb6377170e0e5ef96144b546da3a93dd8f9
//
// were, by the time this was written, three upgrades stale. Nothing in the app,
// the build, or any gate noticed, because nothing compared them to anything. A
// verification surface whose numbers nobody checks is worse than none: it is the
// page a careful person reads instead of checking.
//
// So this file carries an EXPECTED set, labelled as a claim with a date and an
// author, and the panel puts a LIVE reading next to it. The live reading comes
// from a source outside this project (the public IC dashboard index), so the
// comparison is between two parties rather than one party with itself. Three
// outcomes, all of them displayed:
//
//   match     the code running is the code this page describes
//   MISMATCH  it is not, and the panel says do not deposit
//   unknown   the reading could not be taken, which is not the same as match
//
// ---------------------------------------------------------------------------
// THE CLAIM IN `EXPECTED_MODULE_HASHES` IS NOT SELF-VERIFIED, AND SAYS SO
// ---------------------------------------------------------------------------
//
// These values were supplied by the operator with the wave-9 mainnet upgrade of
// 2026-08-06, as the hashes the reproducible container build produced and as the
// hashes then observed on all six canisters. This bundle was built on a machine
// that is not permitted to call mainnet, so the build could not confirm them.
// `LIVE_HASH_SOURCE` below is how anybody, including the operator, confirms them
// afterwards, and `REBUILD_COMMAND` is how anybody reproduces them from source.
//
// If those two disagree, the honest reading is the live one.

/** @typedef {'lobby'|'history'|'table_1'|'table_2'|'table_3'|'btc_table_1'|'frontend'} CanisterRole */

/**
 * The module hash each backend canister is expected to be running, as declared
 * with the 2026-08-06 mainnet upgrade. `frontend` is deliberately absent: the
 * asset canister's module hash is the hash of the ASSET CANISTER, not of the
 * bundle it serves, so it says nothing about this page.
 * @type {Readonly<Record<string,string>>}
 */
export const EXPECTED_MODULE_HASHES = Object.freeze({
  lobby: '7ee36baacf9df421649663093bc5a1907c107c801e6201677b0618721fd23b6a',
  history: '59d4b80c6b7811a46c331276239c50a6f17291fde217e94c1639cfabab3a2abb',
  table_1: '511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4',
  table_2: '511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4',
  table_3: '511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4',
  btc_table_1: '511c9d0e6d508d627357af22930b02daf252753621a17dd8a9fd0d5ab3796ba4',
});

/** Where the expected values came from, shown next to them, verbatim. */
export const EXPECTED_PROVENANCE = Object.freeze({
  declaredOn: '2026-08-06',
  declaredBy: 'the operator, with the wave-9 mainnet upgrade',
  claim:
    'all six backend canisters were upgraded to this source and the deployed module '
    + 'hashes matched the reproducible container build, 6 of 6',
  confirmedByThisBuild: false,
  whyNot:
    'the machine that built this bundle is not permitted to call mainnet, so it '
    + 'could not read the deployed hashes for itself',
});

/**
 * The public index the live reading comes from. It is an ordinary HTTPS endpoint
 * with `access-control-allow-origin: *`, so the check runs in the reader's own
 * browser and is not mediated by this application or by any ClearDeck canister.
 */
export const LIVE_HASH_SOURCE = Object.freeze({
  name: 'Internet Computer public dashboard index',
  base: 'https://ic-api.internetcomputer.org/api/v3/canisters/',
  field: 'module_hash',
});

/** @param {string} canisterId @returns {string} the URL that answers for one canister. */
export function liveHashUrl(canisterId) {
  return `${LIVE_HASH_SOURCE.base}${canisterId}`;
}

/**
 * The one-liner a reader runs in a terminal to get the same answer this panel
 * gets. Deliberately `curl` and not `icp canister status`: on mainnet
 * `canister_status` is a CONTROLLER-ONLY management call, so the command the
 * panel used to print could not be run by any of the strangers it was printed
 * for. This one can be run by anybody.
 * @param {string} canisterId
 * @returns {string}
 */
export function liveHashCommand(canisterId) {
  return `curl -s ${liveHashUrl(canisterId)} | jq -r .module_hash`;
}

/** Every canister in one command, so a reader gets all six with one paste. */
export function liveHashCommandForAll(ids) {
  const list = ids.join(' ');
  return (
    `for c in ${list}; do\n`
    + `  printf '%s %s\\n' "$c" "$(curl -s ${LIVE_HASH_SOURCE.base}$c | jq -r .module_hash)"\n`
    + 'done'
  );
}

/**
 * How a reader turns the source into those hashes themselves. This rebuilds every
 * canister in the digest-pinned container the mainnet fleet is deployed from and
 * compares each one against the live hash, so it answers the whole question
 * ("is the deployed code this source?") rather than half of it.
 */
export const REBUILD_COMMAND =
  'git clone https://github.com/JoshDFN/cleardeck && cd cleardeck && ./scripts/verify-build.sh --mainnet';

/** Normalises `0x…`/uppercase/whitespace so two spellings of one hash compare equal. */
export function normaliseHash(value) {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim().toLowerCase().replace(/^0x/, '');
  return /^[0-9a-f]{64}$/.test(trimmed) ? trimmed : null;
}

/** Display form: always `0x…`, so the panel and the terminal are visibly the same value. */
export function displayHash(value) {
  const h = normaliseHash(value);
  return h ? `0x${h}` : null;
}

/**
 * Compares one live reading against the expectation.
 * @param {string} role
 * @param {string|null|undefined} live
 * @returns {{role:string, expected:string|null, live:string|null,
 *            verdict:'match'|'mismatch'|'unknown'|'no-expectation'}}
 */
export function compareHash(role, live) {
  const expected = normaliseHash(EXPECTED_MODULE_HASHES[role]);
  const actual = normaliseHash(live);
  let verdict;
  if (!expected) verdict = 'no-expectation';
  else if (!actual) verdict = 'unknown';
  else verdict = actual === expected ? 'match' : 'mismatch';
  return { role, expected, live: actual, verdict };
}

/**
 * Reads the live module hash for one canister from the public index.
 *
 * Never throws: a verification surface that breaks the page when the network is
 * down has replaced "unknown" with "nothing", and those are the two answers this
 * whole file exists to keep apart.
 *
 * @param {string} canisterId
 * @param {{fetchImpl?: typeof fetch, timeoutMs?: number}} [opts]
 * @returns {Promise<{ok:boolean, hash:string|null, error:string|null}>}
 */
export async function readLiveModuleHash(canisterId, opts = {}) {
  const fetchImpl = opts.fetchImpl
    || (typeof fetch === 'function' ? fetch.bind(globalThis) : null);
  if (!fetchImpl) {
    return { ok: false, hash: null, error: 'no fetch available in this environment' };
  }
  const timeoutMs = opts.timeoutMs ?? 12_000;
  const controller = typeof AbortController === 'function' ? new AbortController() : null;
  const timer = controller ? setTimeout(() => controller.abort(), timeoutMs) : null;
  try {
    const res = await fetchImpl(liveHashUrl(canisterId), {
      signal: controller?.signal,
      headers: { accept: 'application/json' },
    });
    if (!res.ok) {
      return { ok: false, hash: null, error: `the index answered HTTP ${res.status}` };
    }
    const body = await res.json();
    const hash = normaliseHash(body?.[LIVE_HASH_SOURCE.field]);
    if (!hash) {
      return { ok: false, hash: null, error: 'the index returned no module hash for this canister' };
    }
    return { ok: true, hash, error: null };
  } catch (e) {
    const why = e?.name === 'AbortError'
      ? `no answer within ${Math.round(timeoutMs / 1000)}s`
      : (e?.message || String(e));
    return { ok: false, hash: null, error: why };
  } finally {
    if (timer) clearTimeout(timer);
  }
}
