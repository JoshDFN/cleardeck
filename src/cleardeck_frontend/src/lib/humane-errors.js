// A FAILED READ OF THE LOBBY, IN PLAIN ENGLISH.
//
// The agent's own message for a dead transport is a paragraph of URLs and OS
// error numbers ("Failed to fetch HTTP request: error sending request for url
// (...): client error (Connect): tcp connect error: Connection refused (os
// error 61)"), and that paragraph was the toast a visitor saw when the tables
// could not be read. The visitor needs two things: what happened, in a
// sentence, and what happens next. The raw text is kept, demoted to a detail
// line, so nothing is hidden from someone debugging.

/** How long the page waits before it re-reads the lobby on its own (the first time). */
export const LOBBY_RETRY_MS = 12_000;

/** The longest the page waits between automatic re-reads during an outage. */
export const LOBBY_RETRY_MAX_MS = 60_000;

/**
 * The wait before automatic re-read number `attempt` (0 for the first): the
 * base interval doubled each time and capped, so a long outage does not poll
 * every 12 s for an hour, and the sentence on screen stays true.
 *
 * @param {number} attempt how many automatic re-reads have already failed
 * @param {{ baseMs?: number, maxMs?: number }} [opts]
 * @returns {number} milliseconds
 */
export function retryDelayMs(attempt, { baseMs = LOBBY_RETRY_MS, maxMs = LOBBY_RETRY_MAX_MS } = {}) {
  const n = Number.isFinite(attempt) && attempt > 0 ? Math.floor(attempt) : 0;
  return Math.min(baseMs * 2 ** n, maxMs);
}

/** The longest raw message a detail line carries; the console has the rest. */
export const DETAIL_MAX_CHARS = 240;

const TRANSPORT_PATTERNS = [
  /failed to fetch/i,
  /networkerror/i,
  /network request failed/i,
  /load failed/i,
  /connect(ion)? (refused|reset|error)/i,
  /tcp connect/i,
  /timed? ?out/i,
  /ERR_(CONNECTION|NETWORK|INTERNET|NAME_NOT_RESOLVED)/,
  /\b50[234]\b/,
];

/** The raw text of whatever was thrown, as a trimmed string ('' for nothing). */
export function rawMessageOf(e) {
  if (e == null) return '';
  if (typeof e === 'string') return e.trim();
  const text = typeof e.message === 'string' && e.message.trim() ? e.message : String(e);
  return text.trim();
}

/** True when the failure looks like the network, not the canister. */
export function isTransportFailure(e) {
  const raw = rawMessageOf(e);
  return TRANSPORT_PATTERNS.some((re) => re.test(raw));
}

/** The raw text cut to one detail line. */
export function detailOf(e) {
  const raw = rawMessageOf(e).replace(/\s+/g, ' ');
  if (!raw) return null;
  return raw.length > DETAIL_MAX_CHARS ? `${raw.slice(0, DETAIL_MAX_CHARS - 1)}…` : raw;
}

/**
 * The message for a failed lobby read.
 *
 * @param {unknown} e what `get_tables()` threw
 * @param {{ retryMs?: number }} [opts] the automatic retry interval (null: no automatic retry)
 * @returns {{ message: string, detail: string | null, transport: boolean }}
 */
export function describeLobbyFailure(e, { retryMs = LOBBY_RETRY_MS } = {}) {
  const transport = isTransportFailure(e);
  const what = transport ? 'Could not reach the tables.' : 'The table list could not be read.';
  const next = retryMs == null ? 'Try again in a moment.' : `Retrying in ${Math.round(retryMs / 1000)} s.`;
  return { message: `${what} ${next}`, detail: detailOf(e), transport };
}
