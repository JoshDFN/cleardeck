/**
 * The one thing that turns "trust the table's clock" into "I saw it myself".
 *
 * THE GAP THIS CLOSES
 * -------------------
 * Re-deriving a hand's cards from its revealed seed proves the deal was not
 * changed after the commitment. It says nothing about WHEN the commitment was
 * made: the only evidence of that in the record is `shuffle_proof.timestamp`,
 * which is a number the table canister wrote about itself. `ShuffleProof.svelte`
 * retires that claim by name ("Not proven: that the commitment came before the
 * cards") and the replayer says the same thing.
 *
 * But a player does not have to take it on trust, because the commitment is on
 * screen from the moment the cards are dealt. Anyone who reads it while the hand
 * is still running, and compares it after the seed is revealed, has witnessed the
 * ordering with their own eyes:
 *
 *   at local time T the table published commitment C for hand N and had NOT
 *   published any seed; the board showed k cards. Later it published seed S with
 *   sha256(S) = C. So the deck was fixed no later than T -- before every card
 *   dealt after T, and before every action taken after T. No canister clock is
 *   involved in that argument.
 *
 * This module does the reading and the remembering, so the player does not have
 * to keep a note in a text file. Every field it stores is an observation this
 * browser made, stamped with this browser's clock.
 *
 * WHAT IT DELIBERATELY DOES NOT DO
 * --------------------------------
 *   * It never records a commitment for a hand whose seed is already revealed.
 *     Reading both at once proves nothing about order.
 *   * It never overwrites an existing sighting. The FIRST sighting is the
 *     strongest evidence, and a later one must not be able to weaken it.
 *   * It is not evidence to anyone else. It is this browser's own note to
 *     itself, and the UI says so rather than dressing it up as a proof a third
 *     party could check.
 */

const STORAGE_KEY = 'cleardeck_commitment_witness';
const STORE_VERSION = 1;
/** Bounded so a long session cannot grow localStorage without limit. */
const MAX_ENTRIES = 200;

const HEX64 = /^[0-9a-f]{64}$/;

/** @param {unknown} v */
function storage() {
  try {
    if (typeof localStorage === 'undefined') return null;
    return localStorage;
  } catch {
    // Safari private mode throws on access, not on use.
    return null;
  }
}

function readStore() {
  const ls = storage();
  if (!ls) return { version: STORE_VERSION, entries: {} };
  try {
    const raw = ls.getItem(STORAGE_KEY);
    if (!raw) return { version: STORE_VERSION, entries: {} };
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || typeof parsed.entries !== 'object') {
      return { version: STORE_VERSION, entries: {} };
    }
    return { version: STORE_VERSION, entries: parsed.entries || {} };
  } catch {
    return { version: STORE_VERSION, entries: {} };
  }
}

function writeStore(store) {
  const ls = storage();
  if (!ls) return false;
  try {
    // Newest kept, oldest dropped, so the bound is never a silent data loss for
    // the hand the player is looking at.
    const keys = Object.keys(store.entries)
      .sort((a, b) => (store.entries[b]?.at ?? 0) - (store.entries[a]?.at ?? 0))
      .slice(0, MAX_ENTRIES);
    const entries = {};
    for (const k of keys) entries[k] = store.entries[k];
    ls.setItem(STORAGE_KEY, JSON.stringify({ version: STORE_VERSION, entries }));
    return true;
  } catch {
    return false;
  }
}

/** Normalises a hash the way both sides of every comparison here are normalised. */
export function normalizeHash(value) {
  return String(value ?? '').trim().toLowerCase().replace(/^0x/, '');
}

export const isHash = (value) => HEX64.test(normalizeHash(value));

const keyFor = (tableId, handNumber) => `${tableId || 'table'}:${Number(handNumber)}`;

/**
 * Remembers that THIS browser saw commitment `seedHash` for a hand that had not
 * revealed its seed yet.
 *
 * @param {{tableId:string|null, handNumber:number, seedHash:string,
 *          boardCount?:number, phase?:string|null, seedAlreadyRevealed?:boolean}} sighting
 * @returns {{stored:boolean, reason?:string, entry?:object}}
 */
export function recordSighting({
  tableId, handNumber, seedHash, boardCount = 0, phase = null, seedAlreadyRevealed = false,
}) {
  if (seedAlreadyRevealed) {
    return { stored: false, reason: 'the seed was already revealed, so the order proves nothing' };
  }
  if (!isHash(seedHash)) return { stored: false, reason: 'not a 64-character hex commitment' };
  if (!Number.isFinite(Number(handNumber)) || Number(handNumber) <= 0) {
    return { stored: false, reason: 'no hand number' };
  }
  const store = readStore();
  const key = keyFor(tableId, handNumber);
  if (store.entries[key]) {
    return { stored: false, reason: 'already witnessed', entry: store.entries[key] };
  }
  const entry = {
    tableId: tableId || null,
    handNumber: Number(handNumber),
    seedHash: normalizeHash(seedHash),
    at: Date.now(),
    boardCount: Number(boardCount) || 0,
    phase: phase || null,
  };
  store.entries[key] = entry;
  const stored = writeStore(store);
  return stored ? { stored: true, entry } : { stored: false, reason: 'this browser refused to store it' };
}

/**
 * @param {{tableId:string|null, handNumber:number}} which
 * @returns {object|null}
 */
export function readSighting({ tableId, handNumber }) {
  const store = readStore();
  return store.entries[keyFor(tableId, handNumber)] ?? null;
}

/**
 * The verdict on a sighting, against the commitment the finished record carries.
 *
 * @param {object|null} sighting
 * @param {string|null} recordedSeedHash
 * @returns {{tone:'good'|'bad'|'none', witnessed:boolean, detail:string, sighting:object|null}}
 */
export function verdictForSighting(sighting, recordedSeedHash) {
  if (!sighting) {
    return {
      tone: 'none',
      witnessed: false,
      detail: 'this browser has no sighting of a commitment for this hand',
      sighting: null,
    };
  }
  const same = normalizeHash(sighting.seedHash) === normalizeHash(recordedSeedHash);
  return {
    tone: same ? 'good' : 'bad',
    witnessed: same,
    detail: same
      ? 'the commitment this browser read while the hand was still running is the '
        + 'commitment the finished record carries'
      : 'the commitment this browser read while the hand was running is NOT the one '
        + 'the finished record carries',
    sighting,
  };
}

/**
 * The verdict on a commitment the player pasted in by hand.
 *
 * @param {string} pasted
 * @param {string|null} recordedSeedHash
 * @returns {{tone:'good'|'bad'|'warn'|'none', detail:string}}
 */
export function verdictForPasted(pasted, recordedSeedHash) {
  const value = normalizeHash(pasted);
  if (value.length === 0) return { tone: 'none', detail: '' };
  if (!isHash(value)) {
    // Deliberately spelled out in words rather than digits: this string renders
    // inside a dialog full of money, and the screenshot harness must account for
    // every numeric token on screen (tools/shots/lib/token-census.mjs). A number
    // in static copy is a number something has to explain.
    return {
      tone: 'warn',
      detail: 'that does not look like a commitment. A ClearDeck commitment is the '
        + 'hash of the hand\'s seed, written as sixty-four hex characters.',
    };
  }
  if (!recordedSeedHash) {
    return { tone: 'warn', detail: 'this hand carries no commitment to compare against' };
  }
  if (value === normalizeHash(recordedSeedHash)) {
    return {
      tone: 'good',
      detail: 'identical to the commitment recorded for this hand. If you copied it '
        + 'while the hand was still running, you have witnessed the ordering yourself: '
        + 'the deck was already fixed at that moment.',
    };
  }
  return {
    tone: 'bad',
    detail: 'this does NOT match the commitment recorded for this hand. Check you '
      + 'pasted the commitment for this hand number and this table before drawing '
      + 'any conclusion.',
  };
}

/** Test seam: forget everything this browser witnessed. */
export function forgetSightings() {
  const ls = storage();
  if (!ls) return false;
  try {
    ls.removeItem(STORAGE_KEY);
    return true;
  } catch {
    return false;
  }
}
