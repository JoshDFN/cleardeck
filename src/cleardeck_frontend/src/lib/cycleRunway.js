// HOW LONG BEFORE THIS TABLE STOPS LETTING ANYBODY WITHDRAW?
//
// ===========================================================================
// THE DEFECT THIS EXISTS FOR (docs/DEFECTS.md E-55, SECURITY-FINDINGS.md 19/24/26)
// ===========================================================================
//
// A canister below its freezing threshold rejects EVERY update call at once:
// `deposit`, `withdraw`, `cash_out`, `player_action`, `reload`,
// `abandon_stuck_hand`. Every player at the table loses access to their own
// money at the same instant, with no attacker involved and no in-application
// remedy. It is the only total custody failure in this register that arrives on
// a schedule rather than by malice, and NOTHING in this project tops a canister
// up.
//
// Measured this wave on the real module under PocketIC
// (`tests/money_safety/tests/cycles_runway.rs`):
//
//     empty table, on-chain clock only ....... 0.0442 T/day  -> 10 T = 225 days
//     ~200 hands/day ......................... 0.0779 T/day  -> 10 T = 128 days
//     ~1000 hands/day ........................ 0.2126 T/day  -> 10 T =  47 days
//     500 hands/day, six tabs open ........... 0.4994 T/day  -> 10 T =  20 days
//
//   CORRECTION, wave-13 reconciliation -- docs/DEFECTS.md E-92. The line above
//   counts the 10-second HEARTBEAT and stops. The page also drives
//   `check_timeouts` -- an UPDATE, no `query` in the .did -- from a 500 ms
//   setInterval at a measured 6,573,911 cycles a call: 0.28-1.14 T/day PER OPEN
//   TAB, against the heartbeat's 0.0618. Six tabs is 1.7-6.8 T/day before a hand
//   is dealt, so every figure in this header reads HIGH. What this module reads
//   off the canister is NOT affected: get_cycle_status measures real burn over a
//   sliding window and already includes those calls. The table needs
//   re-measuring with the poll in it; until then, treat the numbers above as a
//   floor and not as the bill.
//     dealing continuously ................... 3.68   T/day  -> 10 T =   2 days
//
// So "226 days" is the number for a table NOBODY IS USING. A busy table has
// weeks. A player about to deposit into a canister with three weeks left
// deserves to be told, on the screen the money leaves from, before the amount
// is typed.
//
// ===========================================================================
// THE THREE THINGS THAT MAKE THIS DIFFERENT FROM AN ORDINARY HEALTH CHECK
// ===========================================================================
//
// 1. UNREACHABLE IS THE ALARM, NOT AN ERROR TO SWALLOW.
//    FINDING 24, reproduced in `cycles_runway.rs`: a frozen canister rejects
//    QUERY CALLS TOO, at the boundary, before any canister code runs. So
//    `get_cycle_status` -- the endpoint built to raise this alarm -- goes dark
//    at the exact moment the alarm is true. A reader written the obvious way
//    (poll, read `runway_days`, warn if low) gets a transport error and reports
//    NOTHING. Here, a rejected read is the LOUDEST state, not a silent catch.
//
// 2. NULL IS NOT FINE.
//    `runway_days` is null whenever the canister has not measured a burn rate
//    yet -- which is the state of every instance for the first five minutes
//    after an install or an upgrade. A reader that treats null as "no problem"
//    goes quiet exactly when the fleet was last touched.
//
// 3. THE THRESHOLD IS IN DAYS, NEVER IN CYCLES.
//    A fixed cycles floor is meaningless because the burn rate moves by two
//    orders of magnitude with load: 1 T is 22 days on an idle table and under
//    seven hours on one dealing continuously. Only `runway_days` -- which the
//    canister computes from its own MEASURED burn -- is comparable across
//    tables.
//
// This module is pure apart from the one call. Everything that decides what a
// player is shown is a function of a plain object, so it is gated offline by
// `tools/shots/test-cycle-runway.mjs`.

import { IDL } from '@dfinity/candid';
import { idlFactory as tableIdlFactory } from 'declarations/table_1/table_1.did.js';
import logger from './logger.js';

/** Verdicts this module can return. Every one of them except OK renders. */
export const RUNWAY_STATES = Object.freeze({
  /** Comfortable: more than WARN_DAYS of measured runway. */
  OK: 'ok',
  /** Under WARN_DAYS. An operator should be topping this up. */
  LOW: 'low',
  /** Under CRITICAL_DAYS. Do not put more money in. */
  CRITICAL: 'critical',
  /** The canister answered but cannot say how long it has. */
  UNKNOWN: 'unknown',
  /** The canister has no `get_cycle_status`: an older module than this bundle. */
  UNSUPPORTED: 'unsupported',
  /**
   * The canister did not answer at all. On the IC this is what a FROZEN canister
   * looks like from outside — and a frozen canister is holding every player's
   * balance and honouring no withdrawal. Indistinguishable from a subnet fault,
   * which is why it is stated as "one of these two things" rather than guessed.
   */
  UNREACHABLE: 'unreachable',
});

/**
 * Days of measured runway below which a player is warned.
 *
 * 60 days is deliberately generous. The freezing RESERVE is not a safety margin:
 * it is `freezing_threshold_seconds x IDLE resource consumption`, which knows
 * nothing about a timer that sends messages, so on this canister it is worth
 * about **1.2 hours** of real burn and not the 30 days it is nominally sized at
 * (measured: reserve 2,236,696,830 cycles against 44,255,242,762 cycles/day).
 * There is no cushion under this number.
 */
export const WARN_DAYS = 60;

/**
 * Days below which the answer to "should I deposit here" is no.
 *
 * Three weeks. Under FINDING 26 a free, permissionless ingress flood burns a
 * table ~65x faster at zero cost to the attacker, so three weeks of ordinary
 * runway is about eight hours of adversarial runway. Anything tighter than this
 * does not leave a human time to react.
 *
 * Kept equal to `CRITICAL_DAYS` in `scripts/cycles-runway.mjs` and to
 * `CRITICAL_DAYS` in `tests/money_safety/tests/cycles_runway.rs`.
 */
export const CRITICAL_DAYS = 21;

const T = 1_000_000_000_000n;

/**
 * Is `get_cycle_status` on the interface this bundle was built against?
 *
 * Discovered from the Candid rather than assumed, for the reason `solvency.js`
 * gives: a mainnet canister can be running an older module than the declarations
 * in this tree, and the honest report of that is UNSUPPORTED — a warning — not a
 * crash and not silence.
 *
 * @param {object} factory idlFactory to inspect (injectable for tests)
 * @returns {{ read: string|null }}
 */
export function describeRunwaySurface(factory = tableIdlFactory) {
  try {
    const service = factory({ IDL });
    const methods = service?._fields ?? [];
    const found = methods.find(([name]) => name === 'get_cycle_status');
    return { read: found ? 'get_cycle_status' : null };
  } catch (e) {
    logger.error('Could not read the table Candid for get_cycle_status:', e);
    return { read: null };
  }
}

/**
 * Turn a decoded `CycleStatus` record into the thing a person is shown.
 *
 * Pure. `raw` is the Candid record; bigints are expected but numbers are
 * tolerated because a hand-rolled caller is exactly the sort of thing that
 * silently produces `NaN` comparisons.
 *
 * @param {object|null} raw
 * @returns {{state: string, days: number|null, liquid: bigint|null,
 *            burnPerDay: bigint|null, clockAlive: boolean|null,
 *            measured: boolean}}
 */
export function interpretCycleStatus(raw) {
  const base = {
    state: RUNWAY_STATES.UNKNOWN,
    days: null,
    liquid: null,
    burnPerDay: null,
    clockAlive: null,
    measured: false,
  };
  if (!raw || typeof raw !== 'object') return base;

  const big = (v) => {
    if (v === null || v === undefined) return null;
    try { return BigInt(v); } catch { return null; }
  };

  // Candid `opt t` decodes as [] or [value]. A bare value is tolerated too:
  // Candid's constituent subtyping lets a client declaring `opt t` decode a wire
  // record whose field is a bare `t`, so both shapes reach here.
  const optBig = (v) => {
    const arr = Array.isArray(v) ? v : (v === null || v === undefined ? [] : [v]);
    if (arr.length === 0) return null;
    return big(arr[0]);
  };
  const optNum = (v) => {
    const n = optBig(v);
    return n === null ? null : Number(n);
  };

  const liquid = big(raw.liquid_balance);
  const lifetime = big(raw.observed_burn_per_day) ?? 0n;
  // `opt nat` on the wire, and ABSENT ENTIRELY on the module running on mainnet
  // today, which is an older build than this tree. Read through the optional so a
  // version-skewed reply degrades to "the lifetime figure is all we have"
  // instead of failing to decode -- a failed decode is reported as UNREACHABLE,
  // and UNREACHABLE means "this canister has run out of cycles".
  const recentOpt = optBig(raw.recent_burn_per_day);
  const recent = recentOpt ?? 0n;
  // The pessimistic of the two, matching what the canister itself divides by. A
  // reader that recomputed this from the lifetime average alone would reproduce
  // the very defect the canister was changed to remove.
  const burnPerDay = lifetime > recent ? lifetime : recent;
  let days = optNum(raw.runway_days);
  const measured = Boolean(raw.measurement_is_meaningful);
  const clockAlive = raw.clock_ticks === undefined
    ? null
    : (big(raw.clock_ticks) ?? 0n) > 0n;

  // RESOLVE A DISAGREEMENT DOWNWARDS.
  //
  // `runway_days` is the canister's own arithmetic, and on a module older than
  // this bundle it was computed from a LIFETIME burn average -- the defect this
  // whole surface exists because of. Recomputing it here from the pessimistic
  // rate and taking the SMALLER of the two means a stale or optimistic field
  // cannot talk this banner round. On a current module the two agree exactly and
  // this is a no-op. `scripts/cycles-runway.sh` does the same thing for the same
  // reason; if they ever disagree, one of them is reading a canister the other is
  // not.
  //
  // ONLY WHEN THE CANISTER SAYS THE MEASUREMENT MEANS SOMETHING. A burn rate
  // taken over a window too short to be trusted is exactly what a canister
  // reports in the minutes after a top-up, and a quiet 30 seconds in that window
  // derives a runway of centuries. Deriving from it would turn "I cannot say yet"
  // into "you have plenty" -- the reading-high failure this file exists to stop,
  // reintroduced by the arithmetic meant to prevent it.
  const derived = (measured && liquid !== null && burnPerDay > 0n)
    ? Number(liquid / burnPerDay)
    : null;
  if (days !== null && derived !== null) days = Math.min(days, derived);
  else if (days === null) days = derived;

  const out = {
    ...base, days, liquid, burnPerDay: burnPerDay || null, clockAlive, measured,
  };

  // NULL IS NOT FINE. A canister that cannot say how long it has is a canister
  // nobody should be told is fine.
  if (days === null) return { ...out, state: RUNWAY_STATES.UNKNOWN };
  if (days < CRITICAL_DAYS) return { ...out, state: RUNWAY_STATES.CRITICAL };
  if (days < WARN_DAYS) return { ...out, state: RUNWAY_STATES.LOW };
  return { ...out, state: RUNWAY_STATES.OK };
}

/**
 * Ask a table how long it has. Never throws.
 *
 * @param {object|null} tableActor
 * @param {{surface?: {read: string|null}}} [opts]
 */
export async function readCycleRunway(tableActor, opts = {}) {
  const surface = opts.surface || describeRunwaySurface();
  const base = {
    state: RUNWAY_STATES.UNKNOWN,
    days: null,
    liquid: null,
    burnPerDay: null,
    clockAlive: null,
    measured: false,
  };

  if (!surface.read) return { ...base, state: RUNWAY_STATES.UNSUPPORTED };
  if (!tableActor) return { ...base, state: RUNWAY_STATES.UNKNOWN };

  try {
    const raw = await tableActor[surface.read]();
    return interpretCycleStatus(raw);
  } catch (e) {
    const message = e?.message || String(e);
    // An older module than these declarations. A warning, not a crash.
    if (/no (query |update )?method|method .* not found|IC0302/i.test(message)) {
      return { ...base, state: RUNWAY_STATES.UNSUPPORTED };
    }
    // ===================================================================
    // THE STATE THIS WHOLE MODULE EXISTS FOR.
    // ===================================================================
    // A frozen canister rejects queries at the boundary with "is unable to
    // process query calls because it's frozen" / "is out of cycles". If the
    // read failed for ANY reason, this table is not answering, and a table
    // that is not answering is not paying anybody out either. It is reported
    // as UNREACHABLE, which renders louder than every other state, because
    // the alternative -- swallowing it and showing nothing -- is precisely the
    // failure FINDING 24 describes.
    logger.error('Cycle-runway read failed (treated as UNREACHABLE):', e);
    return { ...base, state: RUNWAY_STATES.UNREACHABLE, error: message };
  }
}

/** 'ok' | 'warn' | 'danger', for styling. */
export function severityOf(state) {
  switch (state) {
    case RUNWAY_STATES.CRITICAL:
    case RUNWAY_STATES.UNREACHABLE:
      return 'danger';
    case RUNWAY_STATES.LOW:
    case RUNWAY_STATES.UNKNOWN:
    case RUNWAY_STATES.UNSUPPORTED:
      return 'warn';
    default:
      return 'ok';
  }
}

/** Should this state stop a player before they deposit? */
export function shouldWarnBeforeDeposit(state) {
  return state !== RUNWAY_STATES.OK;
}

/** The one-line headline, per state. */
export function headlineFor(state, days = null) {
  switch (state) {
    case RUNWAY_STATES.CRITICAL:
      return days === null
        ? 'This table is nearly out of cycles'
        : `This table has about ${days} day${days === 1 ? '' : 's'} of cycles left`;
    case RUNWAY_STATES.LOW:
      return `This table has about ${days} days of cycles left`;
    case RUNWAY_STATES.UNKNOWN:
      return 'This table cannot say how long it can keep running';
    case RUNWAY_STATES.UNSUPPORTED:
      return 'This table cannot report how long it can keep running';
    case RUNWAY_STATES.UNREACHABLE:
      return 'This table is not answering';
    default:
      return '';
  }
}

/** The sentence that tells a player what to DO. `context` tightens it. */
export function adviceFor(state, context = 'table') {
  switch (state) {
    case RUNWAY_STATES.CRITICAL:
      return context === 'deposit'
        ? 'Do not deposit. When a canister runs out of cycles it stops accepting every '
          + 'update call at once, including withdrawals, and money already inside it '
          + 'cannot be taken out until somebody tops it up.'
        : 'Withdraw what you are not using. When a canister runs out of cycles it stops '
          + 'accepting every update call at once, including withdrawals.';
    case RUNWAY_STATES.LOW:
      return context === 'deposit'
        ? 'Think before you deposit. This table needs topping up well before that runs '
          + 'out, and nothing in this application does it automatically.'
        : 'This table needs topping up well before that runs out, and nothing in this '
          + 'application does it automatically.';
    case RUNWAY_STATES.UNKNOWN:
      return 'It answered, but it has not measured its own burn rate yet — which is normal '
        + 'for the first few minutes after an upgrade. Unknown is not the same as fine.';
    case RUNWAY_STATES.UNSUPPORTED:
      return 'The running module is older than this page and has no cycle-status endpoint, so '
        + 'nothing here can tell you how close it is to freezing.';
    case RUNWAY_STATES.UNREACHABLE:
      return 'It is either out of cycles or its subnet is unwell, and from outside those two '
        + 'look identical. A canister that has run out of cycles rejects queries AND '
        + 'withdrawals until somebody tops it up; your balance is not lost, but it is not '
        + 'reachable either. Do not send it more money.';
    default:
      return '';
  }
}

/** `123456789012n` -> "0.123 T". For the operator-facing line. */
export function formatCycles(v) {
  if (v === null || v === undefined) return 'unknown';
  let n;
  try { n = BigInt(v); } catch { return 'unknown'; }
  const whole = n / T;
  const frac = ((n % T) * 1000n) / T;
  return `${whole}.${frac.toString().padStart(3, '0')} T`;
}

/**
 * Anybody can top a canister up on the IC and it needs no controller rights.
 * Stated in the UI because "somebody should top this up" without the command is
 * not actionable.
 * @param {string|null} canisterId
 */
export function topUpHint(canisterId) {
  const id = canisterId || '<table canister id>';
  // VERIFIED AGAINST THE PINNED CLI (icp-cli 1.0.2), not remembered. `dfx` spells
  // this `deposit-cycles`; `icp` does not have that subcommand at all, and the
  // first draft of this file printed it. Telling a player "anybody can run this"
  // and then handing them a command that does not exist is worse than saying
  // nothing, because it looks like a remedy.
  return `icp canister top-up ${id} --amount 20t -e ic`;
}
