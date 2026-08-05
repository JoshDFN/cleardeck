// Does the page under the camera actually work?
//
// WHY THIS FILE EXISTS
// --------------------
// `JSON.stringify(lastAction.action)` inside a Svelte `$effect` threw
// `TypeError: Do not know how to serialize a BigInt` roughly twice a second on
// an ordinary hand. The throw killed the effect, and with it the fairness panel
// and the hand-history list. The harness photographed the wreckage, asserted
// `.proof-item >= 2` -- rows that render before the browser computes anything --
// and filed the PNG under the canonical verified filename. Nineteen uncaught
// exceptions in one hand, and the gate said "verified".
//
// `watchPage()` in browser.mjs already RECORDED those errors into the manifest.
// Recording is not gating. A number nobody reads is not a check, and the whole
// point of this harness is that an artifact under a canonical name is evidence.
//
// So: a page that threw does not get photographed. `shoot()` calls
// `assertPageHealthy()` before it writes a single byte, which routes the scene
// into run.mjs's failure path -- `FAILED-*.png`, `verified: false`, and the
// error text in `manifest.scenes[].shots[].error`.
//
// This catches the WHOLE class, not one member of it. Mixing a Candid BigInt
// into Number arithmetic, calling `.toFixed()` on a nat64, indexing an `opt`
// as if it were its payload: every one of them surfaces as an uncaught
// exception, and every one of them now fails the scene that renders it.

/**
 * Playwright pages are created per (scene, viewport) and thrown away, so the
 * watcher arrays are looked up by page object and collected with it.
 * @type {WeakMap<object, {consoleErrors: string[], pageErrors: string[], failedRequests: string[]}>}
 */
const WATCHERS = new WeakMap();

/** Called by `watchPage()` the moment the listeners are attached. */
export function registerWatchers(page, watchers) {
  WATCHERS.set(page, watchers);
  return watchers;
}

/** The watcher arrays for a page, or null if nobody is watching it. */
export function watchersFor(page) {
  return WATCHERS.get(page) || null;
}

/**
 * Console errors that are page NOISE rather than broken application code.
 *
 * Kept deliberately short. Everything here is a RESOURCE-level complaint that
 * `failedRequests` already records separately -- the third-party price feed is
 * blocked on purpose by `installThirdPartyCache`, and the app renders its own
 * no-price state when it is. None of these is an exception escaping app code.
 */
const BENIGN_CONSOLE = [
  /Failed to load resource/i,
  /net::ERR_/i,
  /ERR_BLOCKED_BY_CLIENT/i,
  /favicon/i,
  /Content Security Policy/i,
];

/**
 * Console errors that MEAN an exception escaped, even though the browser did
 * not route them through `pageerror`. Unhandled promise rejections and Svelte's
 * own runtime diagnostics both land here.
 */
const UNCAUGHT_CONSOLE = [
  /\bUncaught\b/,
  /Unhandled\s+(Promise\s+)?[Rr]ejection/i,
  /\b(Type|Reference|Range|Syntax|Internal|Eval|URI)Error\b/,
  /Do not know how to serialize a BigInt/,
  /Cannot (mix BigInt|convert a BigInt)/,
  /svelte\.dev\/e\//,          // Svelte 5 runtime error codes
  /effect_update_depth_exceeded/,
  /is not a function\b/,
  /Cannot read propert(y|ies) of (undefined|null)/,
];

const isBenign = (text) => BENIGN_CONSOLE.some((re) => re.test(text));
const looksUncaught = (text) => UNCAUGHT_CONSOLE.some((re) => re.test(text));

/**
 * Every uncaught error this page logged, as human-readable lines.
 *
 * `pageErrors` are uncaught exceptions and are ALWAYS faults: there is no
 * allowlist and there is not going to be one. Console errors are faults only
 * when they carry the signature of an escaped throw, so an app that
 * deliberately logs a handled failure (`logger.error('price feed unreachable')`)
 * does not fail a scene for behaving correctly.
 *
 * @param {object} page a Playwright page previously passed to `watchPage()`
 * @returns {string[]}
 */
export function pageFaults(page) {
  const w = watchersFor(page);
  if (!w) return [];
  const faults = [];
  for (const e of w.pageErrors) faults.push(`uncaught exception: ${e}`);
  for (const e of w.consoleErrors) {
    if (isBenign(e)) continue;
    if (looksUncaught(e)) faults.push(`console error: ${e}`);
  }
  return faults;
}

/**
 * Throws if the page logged an uncaught error. Call before publishing any
 * artifact derived from it.
 *
 * @param {object} page
 * @param {{scene?: string, viewport?: string}} [where] for the message only
 */
export function assertPageHealthy(page, where = {}) {
  const faults = pageFaults(page);
  if (faults.length === 0) return;
  const label = [where.scene, where.viewport].filter(Boolean).join(' / ') || 'page';
  const unique = [...new Set(faults)];
  throw new Error(
    `${label} threw ${faults.length} uncaught error(s) while it was being staged, `
    + `so nothing it renders can be trusted and no screenshot of it is evidence. `
    + `An uncaught exception in a Svelte $effect tears down that effect and starves `
    + `the ones flushed with it, which is how a dead fairness panel was once certified `
    + `(docs/WAVE-03.md seams 5 and 9). Distinct errors:\n  - `
    + unique.slice(0, 6).join('\n  - ')
    + (unique.length > 6 ? `\n  - ...and ${unique.length - 6} more` : ''),
  );
}
