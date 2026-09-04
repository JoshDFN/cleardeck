/**
 * The action clock, interpolated locally from the last certified figure.
 *
 * The canister reports `time_remaining_secs` on every poll; the browser ticks
 * it down between polls. Two things go wrong without care: the digits jitter
 * when successive polls land at odd sub-second offsets, and a stale replica or
 * a slow poll makes the figure jump. So the local clock keeps its own sample
 * and RESYNCS only when the chain disagrees with the local interpolation by
 * more than a skew tolerance (2 s), or when the chain's figure has gone UP (a
 * time bank engaged, a new turn). Everything else is a smooth local tick.
 *
 * While a send is open (optimistic echo) the displayed figure is frozen at the
 * moment of the click: the player has acted and the clock is no longer theirs.
 *
 * Pure: samples are plain frozen objects, nothing is mutated.
 */

/** @typedef {{ serverSecs: number, receivedAt: number }} ClockSample */

export const CLOCK_SKEW_SECS = 2;

/**
 * Fold a freshly polled `time_remaining_secs` into the local sample.
 * @param {ClockSample|null} prev
 * @param {number|null} serverSecs  null when the chain reports no clock
 * @param {number} now              Date.now()
 * @param {number} [skewSecs]
 * @returns {ClockSample|null}      the sample to keep (prev, or a new one)
 */
export function resyncClock(prev, serverSecs, now, skewSecs = CLOCK_SKEW_SECS) {
  if (serverSecs === null || serverSecs === undefined || !Number.isFinite(Number(serverSecs))) return null;
  const server = Math.max(0, Math.trunc(Number(serverSecs)));
  if (!prev) return Object.freeze({ serverSecs: server, receivedAt: Number(now) });
  const local = displayedSeconds(prev, now);
  const drifted = Math.abs(local - server) > skewSecs;
  const wentUp = server > local;
  if (drifted || wentUp) return Object.freeze({ serverSecs: server, receivedAt: Number(now) });
  return prev;
}

/**
 * The seconds to show at `now` for a sample, never below zero. Pass `frozenAt`
 * to hold the figure at that instant (a send is open).
 */
export function displayedSeconds(sample, now, frozenAt = null) {
  if (!sample) return null;
  const at = frozenAt === null || frozenAt === undefined ? Number(now) : Math.min(Number(now), Number(frozenAt));
  const elapsed = Math.max(0, Math.floor((at - sample.receivedAt) / 1000));
  return Math.max(0, sample.serverSecs - elapsed);
}

/** Fraction of the clock left, 0..1, for the ring and the bar line. */
export function clockFraction(secs, totalSecs) {
  if (secs === null || secs === undefined || !totalSecs) return 0;
  return Math.max(0, Math.min(1, Number(secs) / Number(totalSecs)));
}

/** The last ten seconds are urgent. */
export const URGENT_SECS = 10;

export function clockUrgent(secs) {
  return secs !== null && secs !== undefined && Number(secs) <= URGENT_SECS;
}

/** The time bank offer appears only when the clock is nearly out. */
export const TIME_BANK_OFFER_SECS = 15;

export function offerTimeBank({ secs, bankSecs, usingBank }) {
  if (usingBank || !bankSecs || bankSecs <= 0) return false;
  return secs !== null && secs !== undefined && Number(secs) <= TIME_BANK_OFFER_SECS;
}
