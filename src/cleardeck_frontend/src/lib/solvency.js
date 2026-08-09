// CAN THIS TABLE PAY EVERYBODY IT OWES, AND DOES IT KNOW?
//
// ===========================================================================
// THE DEFECT THIS EXISTS FOR (docs/SECURITY-FINDINGS.md FINDING 35, docs/DEFECTS.md E-70)
// ===========================================================================
//
// Measured on mainnet table_1 immediately after the wave-9 upgrade:
//
//     escrow claimed   940,640,001 e8s
//     chips at table             0
//     pot                        0
//     ledger main account holds  740,640,001 e8s
//     ---------------------------------------------
//     SHORTFALL        200,000,000 e8s   (2.00 ICP)
//
// Every published deposit subaccount was audited and held nothing, so the money
// is not hiding there. The table is two ICP short of what its own books say it
// owes, and NO SURFACE OF THE CANISTER CAN SAY SO. The wave-8 insight -- an IC
// query cannot call a ledger, so ask from an UPDATE and write the answer down --
// was implemented for the deposit SUBACCOUNTS and for nothing else. The MAIN
// account, where essentially all of the money is, has no observation record and
// no reader.
//
// A player standing in front of the deposit button is therefore being asked to
// add money to a pool that is already short, by an application that presents no
// way for them to find that out. That is the thing this module refuses to keep
// doing.
//
// ===========================================================================
// WHY THIS READS THE CANDID INSTEAD OF NAMING A METHOD
// ===========================================================================
//
// The canister-side half of FINDING 35 is being built in parallel with this
// screen. Hard-coding the method name that half is expected to export would make
// this file a guess about somebody else's work, and a wrong guess degrades to
// exactly the failure mode being fixed: the UI silently showing nothing.
//
// So the surface is DISCOVERED from the table's own interface definition -- the
// generated `declarations/table_1/table_1.did.js`, which is the Candid, not the
// Rust. Two consequences that matter:
//
//   * When the canister half lands and the declarations are regenerated, this
//     picks it up with no edit here.
//   * Until then, `read()` returns `unsupported`, and `unsupported` is rendered
//     as a WARNING, not as silence. "This table cannot tell you whether it holds
//     the money it owes" is the true statement about today's deployment, and it
//     is the one a depositor needs.
//
// UNKNOWN IS NOT ZERO AND UNKNOWN IS NOT FINE. The three states this module can
// report -- `short`, `unknown`, `unsupported` -- are all reasons not to deposit.
// Only `covered`, backed by a reading with a timestamp on it, is not.

import { IDL } from '@dfinity/candid';
import { idlFactory as tableIdlFactory } from 'declarations/table_1/table_1.did.js';
import logger from './logger.js';

/** Verdicts this module can return. Every one of them is displayable. */
export const SOLVENCY_STATES = Object.freeze({
  COVERED: 'covered',
  SHORT: 'short',
  UNKNOWN: 'unknown',
  UNSUPPORTED: 'unsupported',
  ERROR: 'error',
});

// ---------------------------------------------------------------------------
// Reading the Candid
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FIELD MATCHERS, TUNED AGAINST THE REAL `SolvencyReport`
// ---------------------------------------------------------------------------
//
// The canister half of FINDING 35 landed while this file was being written, and
// `src/table_canister/table_canister.did` now publishes:
//
//   held                  : opt nat64   -- NULL means nobody has ever looked
//   owed                  : nat64
//   shortfall_e8s         : opt nat64   -- "branch on `verdict`, never on this"
//   main_observed_at_ns   : opt nat64   -- when the LEDGER was asked; null = never
//   as_of_ns              : nat64       -- when the REPORT was computed
//   verdict               : variant { CanPayEveryone; CannotPayEveryone; Unknown }
//   summary               : text
//
// THE TWO TRAPS IN THAT LIST, both of which the first draft of this file walked
// into and both of which fail in the reassuring direction:
//
//   1. `as_of_ns` IS NOT AN OBSERVATION. It is when the report was computed, and
//      it is always set. Matching it as "when was the ledger read" turns "nobody
//      has ever looked" into "looked just now". So the observation matchers are
//      an ORDERED list with `as_of` excluded outright, and the main-account one
//      first. @dfinity/candid decodes record fields in hash order, so "whichever
//      key matches first" is not a defined thing to rely on.
//
//   2. `shortfall_e8s` IS NULL IN TWO DIFFERENT SITUATIONS -- no shortfall, and
//      answer unknown -- and the Candid says so in as many words. Deriving the
//      verdict from it reads "unknown" as "fine". So `verdict` wins whenever the
//      canister states one, and the derivation is only the fallback for a
//      surface that does not.

// ---------------------------------------------------------------------------
// ORDERED MATCHERS, AND WHY THE ORDER HAS TO BE HONOURED BY THE PICKER
// ---------------------------------------------------------------------------
//
// These lists are written most-specific-first. That intent was documented from
// the first draft and it was NOT IMPLEMENTED: `pick()` walked the RECORD's keys
// and applied `HELD_FIELDS.some(...)`, so the winner was whichever KEY came first
// in the decoded object, not whichever MATCHER came first in this list.
//
// `@dfinity/candid` emits record fields in CANDID HASH ORDER, and for the shipped
// `SolvencyReport` that order begins:
//
//   pot, main_observed_at_ns, chips_at_table, main_account, held, ...
//
// `main_account` before `held`. So the deposit and withdraw screens would have
// labelled the MAIN ACCOUNT BALANCE as "Held on the ledger" -- a figure that
// excludes every deposit subaccount, on a screen whose entire job is to say
// whether the table holds what it owes. Same shape for `owed`, whose loose
// fallback `/(liabilit|claims|obligation)/` matches `guard_liability`.
//
// [`pickOrdered`] is what makes the documented priority real. A comment shipped
// describing behaviour the code did not implement is the shape of
// docs/SECURITY-FINDINGS.md FINDING 33.

/** Ordered: the first matcher that hits wins, so priority is explicit. */
const HELD_FIELDS = [
  (n) => n === 'held',
  (n) => /(^|_)(main|ledger|reserve|backing)(_|$)/.test(n)
    && /(balance|held|holdings?|amount|reserve|backing)/.test(n),
  (n) => /^(main_account|reserve|backing)$/.test(n),
];

const OBSERVED_AT_FIELDS = [
  (n) => /^main_observed_at_ns$/.test(n),
  (n) => /(^|_)main(_|$)/.test(n) && /(observed_at|read_at|checked_at|measured_at)/.test(n),
  // `as_of` deliberately absent: it dates the REPORT, not the reading.
  (n) => /(observed_at|checked_at|read_at|measured_at)/.test(n),
];

// What the canister's own books say it owes. `^owed$` FIRST: the loose fallback
// also matches `guard_liability`, which is the same number on a healthy build and
// was the LARGER of two disagreeing totals on the build
// docs/SECURITY-FINDINGS.md FINDING 43 was measured on.
const OWED_FIELDS = [
  (n) => n === 'owed',
  (n) => /(liabilit|claims|obligation)/.test(n),
];

const FIELD = Object.freeze({
  held: (n) => HELD_FIELDS.some((f) => f(n)),
  owed: (n) => OWED_FIELDS.some((f) => f(n)),
  // The canister's own arithmetic, when it does it for us.
  shortfall: (n) => /(shortfall|deficit|uncovered|unbacked|missing)/.test(n),
  // When the LEDGER was asked. `null`/absent means NEVER, not "empty".
  observedAt: (n) => OBSERVED_AT_FIELDS.some((f) => f(n)),
  // A sentence the canister wrote, which is always better than one we compose.
  advice: (n) => /^(advice|summary|sentence|explanation|note)$/.test(n),
  // An explicit boolean verdict, for a surface that states one that way.
  solvent: (n) => /^(is_)?(solvent|covered|fully_backed|backed)$/.test(n),
  // An explicit VARIANT verdict. This is what the shipped canister publishes.
  verdict: (n) => /^(verdict|solvency|status)$/.test(n),
});

/** The variant tags the canister's `SolvencyVerdict` can carry, mapped to states. */
const VERDICT_TAGS = Object.freeze({
  CanPayEveryone: 'covered',
  CannotPayEveryone: 'short',
  Unknown: 'unknown',
  // Tolerated spellings, so a rename degrades to the right state rather than to
  // silence. Anything unrecognised falls through to `unknown`, never `covered`.
  Solvent: 'covered',
  Insolvent: 'short',
  Short: 'short',
});

/** Method-name shapes for the two halves of the wave-8 pattern. */
const METHOD = Object.freeze({
  // The QUERY that reads the written-down answer. Free, and still answers when
  // every update path is refusing.
  read: (n) => /^(get_|read_)?(solvency|reserve|backing|main_account|custody_totals|table_solvency)/.test(n)
    || /(solvency|reserve_status|main_account_status|backing_status)$/.test(n),
  // The UPDATE that asks the ledger and writes the answer down. Ordered: a
  // refresh that names solvency re-reads EVERY account, where one that names a
  // single account re-reads only that one.
  refresh: (n) => /^(refresh|audit|observe|check)_/.test(n)
    && /(solvency|main|reserve|backing|account)/.test(n),
  refreshPreferred: (n) => /^(refresh|audit)_/.test(n) && /solvency/.test(n),
});

function isQuery(func) {
  const a = func?.annotations;
  return Array.isArray(a) && (a.includes('query') || a.includes('composite_query'));
}

/** Record field names of a Candid type, or `[]` when it is not a record. */
function recordFieldNames(type) {
  const fields = type?._fields;
  if (!Array.isArray(fields)) return [];
  return fields.map(([name]) => String(name));
}

/**
 * Unwraps the record a method returns, looking through `variant { Ok; Err }` and
 * `opt`, so a canister that wraps its answer in a Result is still readable.
 * @returns {string[]} field names of the payload record
 */
function payloadFieldNames(retTypes) {
  const out = [];
  const visit = (type, depth) => {
    if (!type || depth > 3) return;
    const direct = recordFieldNames(type);
    if (direct.length) out.push(...direct);
    // opt T / vec T keep their element type on `_type`; variants keep `_fields`.
    if (type._type) visit(type._type, depth + 1);
    if (Array.isArray(type._fields)) {
      for (const [, inner] of type._fields) visit(inner, depth + 1);
    }
  };
  for (const t of retTypes || []) visit(t, 0);
  return out;
}

/**
 * Everything the table's Candid says about its solvency surface.
 *
 * Pure, synchronous, and network-free: this reads the interface definition that
 * was compiled into this bundle, so it can be asserted in a test and printed in
 * a build log.
 *
 * @param {(args:{IDL:typeof IDL})=>any} [factory] override for tests
 * @returns {{read:string|null, refresh:string|null, fields:string[], methods:string[]}}
 */
export function describeSolvencySurface(factory = tableIdlFactory) {
  let service;
  try {
    service = factory({ IDL });
  } catch (e) {
    logger.error('Could not read the table interface definition:', e);
    return { read: null, refresh: null, fields: [], methods: [] };
  }
  const entries = Array.isArray(service?._fields) ? service._fields : [];
  const methods = entries.map(([name]) => String(name));

  let read = null;
  let fields = [];
  let refresh = null;
  let refreshIsPreferred = false;

  // The whole walk is guarded. This runs when the deposit dialog opens, and a
  // throw here would blank the dialog rather than warn in it, which is a worse
  // failure than the one this module exists to prevent.
  try {
    for (const [rawName, func] of entries) {
      const name = String(rawName);
      if (!read && isQuery(func) && METHOD.read(name)) {
        const candidate = payloadFieldNames(func?.retTypes);
        // A method is only the solvency reader if its answer actually carries the
        // two numbers. Name-matching alone would wire this to anything.
        const hasHeld = candidate.some(FIELD.held);
        const hasOwed = candidate.some(FIELD.owed);
        const hasShortfall = candidate.some(FIELD.shortfall);
        if ((hasHeld && hasOwed) || hasShortfall) {
          read = name;
          fields = candidate;
        }
      }
      if (!isQuery(func) && (func?.argTypes?.length ?? 0) === 0) {
        if (!refreshIsPreferred && METHOD.refreshPreferred(name)) {
          refresh = name;
          refreshIsPreferred = true;
        } else if (!refresh && METHOD.refresh(name)) {
          refresh = name;
        }
      }
    }
  } catch (e) {
    logger.error('Could not walk the table interface definition:', e);
    return { read: null, refresh: null, fields: [], methods };
  }

  return { read, refresh, fields, methods };
}

// ---------------------------------------------------------------------------
// Reading the canister
// ---------------------------------------------------------------------------

function toBigInt(value) {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return BigInt(Math.trunc(value));
  if (typeof value === 'string' && /^\d+$/.test(value)) return BigInt(value);
  return null;
}

/** Candid `opt` decodes to `[]` / `[v]`; everything else passes through. */
function unwrapOpt(value) {
  if (Array.isArray(value)) return value.length ? value[0] : null;
  return value ?? null;
}

/** Looks through `variant { Ok; Err }` to the payload, or reports the Err. */
function unwrapResult(value) {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    if ('Err' in value) return { ok: false, error: String(value.Err) };
    if ('Ok' in value) return { ok: true, value: value.Ok };
  }
  return { ok: true, value };
}

function pick(record, predicate) {
  if (!record || typeof record !== 'object') return undefined;
  for (const key of Object.keys(record)) {
    if (predicate(key)) return record[key];
  }
  return undefined;
}

/**
 * `pick`, with the MATCHER list's order deciding, not the record's key order.
 *
 * Every matcher is tried against every key before the next matcher is tried at
 * all, so "most specific first" means what it says. `pick` cannot do this: it
 * walks the record, and a Candid decode hands its keys back in hash order --
 * `main_account` before `held`, for the shipped `SolvencyReport`.
 *
 * @param {unknown} record
 * @param {((name:string)=>boolean)[]} matchers most specific first
 */
function pickOrdered(record, matchers) {
  if (!record || typeof record !== 'object') return undefined;
  const keys = Object.keys(record);
  for (const matcher of matchers) {
    for (const key of keys) {
      if (matcher(key)) return record[key];
    }
  }
  return undefined;
}

/**
 * Turns whatever the canister returned into the four facts a player needs.
 *
 * Deliberately tolerant about SHAPE and intolerant about MEANING: a record that
 * does not let us establish "held" and "owed", or a stated shortfall, produces
 * `unknown` rather than a reassuring default.
 *
 * @param {unknown} raw the decoded Candid value
 * @returns {{state:string, held:bigint|null, owed:bigint|null, shortfall:bigint|null,
 *            observedAtNs:bigint|null, advice:string, raw:unknown}}
 */
export function interpretSolvency(raw) {
  const unwrapped = unwrapResult(raw);
  if (!unwrapped.ok) {
    return {
      state: SOLVENCY_STATES.UNKNOWN,
      held: null, owed: null, shortfall: null, observedAtNs: null,
      advice: unwrapped.error,
      raw,
    };
  }
  const record = unwrapped.value;

  // ORDERED. `pick` here would return whichever KEY the Candid decoder happened
  // to emit first -- `main_account` for `held`, which is the main account alone
  // and not what this canister holds. See the matcher lists above.
  const held = toBigInt(unwrapOpt(pickOrdered(record, HELD_FIELDS)));
  const owed = toBigInt(unwrapOpt(pickOrdered(record, OWED_FIELDS)));
  const statedShort = toBigInt(unwrapOpt(pick(record, FIELD.shortfall)));
  const observedAtNs = toBigInt(unwrapOpt(pickOrdered(record, OBSERVED_AT_FIELDS)));
  const adviceRaw = pick(record, FIELD.advice);
  const advice = typeof adviceRaw === 'string' ? adviceRaw : '';
  const solventFlag = pick(record, FIELD.solvent);

  let shortfall = statedShort;
  if (shortfall === null && held !== null && owed !== null) {
    shortfall = owed > held ? owed - held : 0n;
  }

  // ------------------------------------------------------------------
  // THE CANISTER'S OWN VERDICT WINS, WHEN IT STATES ONE.
  // ------------------------------------------------------------------
  // `SolvencyVerdict` is the field the shipped Candid tells callers to branch
  // on, in as many words: "null when there is no shortfall OR when the answer is
  // not known -- branch on `verdict`, never on this". It also accounts for
  // things this client cannot see, such as deposit subaccounts that have never
  // been read. Re-deriving over the top of it would be a second opinion formed
  // from less information, and the direction it would get wrong is `covered`.
  const verdictRaw = pick(record, FIELD.verdict);
  const verdictTag = verdictRaw && typeof verdictRaw === 'object' && !Array.isArray(verdictRaw)
    ? Object.keys(verdictRaw)[0]
    : (typeof verdictRaw === 'string' ? verdictRaw : null);
  if (verdictTag) {
    // An UNRECOGNISED tag is `unknown`, never `covered`: a canister that grew a
    // verdict this client has not been taught must not read as an all-clear.
    const state = VERDICT_TAGS[verdictTag] ?? SOLVENCY_STATES.UNKNOWN;
    return { state, held, owed, shortfall, observedAtNs, advice, raw };
  }

  // NEVER LOOKED is its own answer. A canister that has not asked the ledger
  // cannot report a balance of zero; it can only report that it does not know,
  // and a surface that renders those two the same way is the whole defect.
  const hasObservationField = Object.keys(record ?? {}).some(FIELD.observedAt);
  if (hasObservationField && observedAtNs === null) {
    return {
      state: SOLVENCY_STATES.UNKNOWN,
      held, owed, shortfall: statedShort, observedAtNs: null,
      advice: advice
        || 'This table has never read the ledger account it pays out of, so it cannot say '
          + 'whether it holds the money it owes.',
      raw,
    };
  }

  let state;
  if (shortfall !== null) {
    state = shortfall > 0n ? SOLVENCY_STATES.SHORT : SOLVENCY_STATES.COVERED;
  } else if (typeof solventFlag === 'boolean') {
    state = solventFlag ? SOLVENCY_STATES.COVERED : SOLVENCY_STATES.SHORT;
  } else {
    state = SOLVENCY_STATES.UNKNOWN;
  }

  return { state, held, owed, shortfall, observedAtNs, advice, raw };
}

/**
 * The sentence shown when the canister has no solvency surface at all.
 *
 * NO DEFECT REFERENCE IN PLAYER-FACING COPY, and not only for tone: the
 * screenshot harness's token census requires every numeric token on screen to be
 * matched to a canister figure or excused by a reviewed rule, and a trailing
 * "FINDING 35" puts an unassertable "35" on the deposit screen. The citation
 * belongs in the header of this file, where it is.
 */
export const UNSUPPORTED_ADVICE =
  'This table cannot report whether it actually holds the money it says it owes. '
  + 'Its balances are numbers it keeps for itself; nothing here compares them to the '
  + 'ledger account it pays out of, so a shortfall would look exactly like this screen.';

/**
 * Asks the table whether it is solvent.
 *
 * Never throws and never returns "fine" by default. The caller gets one of five
 * states, four of which are reasons to stop.
 *
 * @param {object|null} tableActor actor proxy for the table canister
 * @param {{surface?:ReturnType<typeof describeSolvencySurface>}} [opts]
 * @returns {Promise<{state:string, held:bigint|null, owed:bigint|null,
 *                    shortfall:bigint|null, observedAtNs:bigint|null,
 *                    advice:string, method:string|null, canRefresh:boolean}>}
 */
export async function readTableSolvency(tableActor, opts = {}) {
  const surface = opts.surface || describeSolvencySurface();
  const base = {
    held: null, owed: null, shortfall: null, observedAtNs: null,
    method: surface.read, canRefresh: Boolean(surface.refresh),
  };

  if (!surface.read) {
    return { ...base, state: SOLVENCY_STATES.UNSUPPORTED, advice: UNSUPPORTED_ADVICE };
  }
  if (!tableActor) {
    return {
      ...base,
      state: SOLVENCY_STATES.UNKNOWN,
      advice: 'No connection to this table yet, so its solvency has not been read.',
    };
  }

  try {
    const raw = await tableActor[surface.read]();
    const read = interpretSolvency(raw);
    return { ...base, ...read, method: surface.read, canRefresh: Boolean(surface.refresh) };
  } catch (e) {
    const message = e?.message || String(e);
    // A canister older than its declarations answers "has no query method". That
    // is `unsupported`, not a transient failure, and it must read as a warning.
    if (/no (query |update )?method|method .* not found|IC0302/i.test(message)) {
      return { ...base, state: SOLVENCY_STATES.UNSUPPORTED, advice: UNSUPPORTED_ADVICE };
    }
    logger.error('Solvency read failed:', e);
    return {
      ...base,
      state: SOLVENCY_STATES.ERROR,
      advice: `Could not read this table's solvency: ${message}`,
    };
  }
}

/**
 * Asks the table to re-read its own ledger account, then reads the answer.
 * This is the UPDATE half of the wave-8 pattern; the caller should offer it as an
 * explicit action, never run it on page load.
 */
export async function refreshTableSolvency(tableActor, opts = {}) {
  const surface = opts.surface || describeSolvencySurface();
  if (tableActor && surface.refresh) {
    try {
      await tableActor[surface.refresh]();
    } catch (e) {
      logger.error('Solvency refresh failed:', e);
    }
  }
  return readTableSolvency(tableActor, { surface });
}

// ---------------------------------------------------------------------------
// Presentation helpers (pure, so they can be unit-tested without a canister)
// ---------------------------------------------------------------------------

/** Severity a caller should render this state at. `covered` is the only calm one. */
export function severityOf(state) {
  switch (state) {
    case SOLVENCY_STATES.SHORT: return 'critical';
    case SOLVENCY_STATES.UNSUPPORTED:
    case SOLVENCY_STATES.UNKNOWN:
    case SOLVENCY_STATES.ERROR: return 'warning';
    default: return 'ok';
  }
}

/** Should a deposit be discouraged in this state? Everything but `covered`. */
export function shouldWarnBeforeDeposit(state) {
  return state !== SOLVENCY_STATES.COVERED;
}

/** Short headline, in the words a player thinks in. */
export function headlineFor(state) {
  switch (state) {
    case SOLVENCY_STATES.SHORT:
      return 'This table does not hold all the money it owes';
    case SOLVENCY_STATES.COVERED:
      return 'This table holds at least what it owes';
    case SOLVENCY_STATES.UNSUPPORTED:
      return 'This table cannot say whether it holds your money';
    case SOLVENCY_STATES.ERROR:
      return 'This table could not be asked whether it holds your money';
    default:
      return 'This table has not checked whether it holds the money it owes';
  }
}

/** Age of an observation, in words, or the honest absence of one. */
export function observationAge(observedAtNs, nowMs = Date.now()) {
  if (observedAtNs === null || observedAtNs === undefined) return 'never taken';
  const thenMs = Number(observedAtNs / 1_000_000n);
  const secs = Math.max(0, Math.round((nowMs - thenMs) / 1000));
  if (secs < 90) return `${secs}s ago`;
  const mins = Math.round(secs / 60);
  if (mins < 90) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `${hours} h ago`;
  return `${Math.round(hours / 24)} days ago`;
}
