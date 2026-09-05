// Copying text, with a humane failure.
//
// `navigator.clipboard.writeText` is refused in plenty of real situations (no
// user gesture left, an insecure context, a permissions policy, a browser
// without the API). The old behaviour was a `logger.error` and a button that
// silently stayed "Copy", which reads as "nothing happened". Now the button
// says so, and the older `execCommand('copy')` route is tried first as a
// fallback, because a selected textarea is copyable in browsers where the
// async API is not.

export const COPY_FAILED_TEXT = 'Could not copy, select the text instead';

/** How long a button wears "Copied" and the failure sentence, in ms. */
export const COPIED_FOR_MS = 2000;
export const FAILED_FOR_MS = 4000;

/**
 * The legacy copy: a hidden textarea, selected, then `execCommand('copy')`.
 * Returns false rather than throwing when the document cannot do it.
 *
 * @param {string} value
 * @param {Document|undefined} doc
 */
function copyViaSelection(value, doc) {
  if (!doc?.body || typeof doc.execCommand !== 'function') return false;
  const area = doc.createElement('textarea');
  area.value = value;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.opacity = '0';
  area.style.pointerEvents = 'none';
  doc.body.appendChild(area);
  let ok = false;
  try {
    area.select();
    area.setSelectionRange(0, value.length);
    ok = doc.execCommand('copy') === true;
  } catch {
    ok = false;
  } finally {
    area.remove();
  }
  return ok;
}

/**
 * Copies `value`. Never throws: the result says what happened.
 *
 * @param {string} value
 * @param {{clipboard?: {writeText?: (s: string) => Promise<void>}, doc?: Document}} [env]
 * @returns {Promise<{ok: true} | {ok: false, reason: string}>}
 */
export async function copyText(value, env = {}) {
  const text = typeof value === 'string' ? value : '';
  if (!text) return { ok: false, reason: 'nothing to copy' };
  const clipboard = 'clipboard' in env ? env.clipboard : globalThis.navigator?.clipboard;
  const doc = 'doc' in env ? env.doc : globalThis.document;
  if (clipboard && typeof clipboard.writeText === 'function') {
    try {
      await clipboard.writeText(text);
      return { ok: true };
    } catch (e) {
      if (copyViaSelection(text, doc)) return { ok: true };
      return { ok: false, reason: e?.message || 'clipboard refused' };
    }
  }
  if (copyViaSelection(text, doc)) return { ok: true };
  return { ok: false, reason: 'no clipboard available' };
}

/**
 * A copy button's state, as one string a component can hold in `$state`:
 * null (idle), `<label>` (copied), or `failed:<label>`.
 */
export const copiedState = (label, ok) => (ok ? label : `failed:${label}`);

/**
 * The word a copy button wears for `label`, given the component's state.
 *
 * @param {string|null} state    what `copiedState` returned, or null
 * @param {string} label         the button's own label
 * @param {string} idle          the button's resting text ("Copy")
 * @param {string} [done]        the text on success ("Copied")
 */
export function copyWord(state, label, idle, done = 'Copied') {
  if (state === label) return done;
  if (state === `failed:${label}`) return COPY_FAILED_TEXT;
  return idle;
}

/** How long the state should stay on screen. */
export const copyStateTtlMs = (state) => (
  typeof state === 'string' && state.startsWith('failed:') ? FAILED_FOR_MS : COPIED_FOR_MS
);
