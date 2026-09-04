/**
 * The your-turn alert: what tells a tabbed-out or pocketed player that the
 * action is on them BEFORE the last ten seconds.
 *
 * The decision is pure so it can be tested: it fires once per certified
 * my-turn edge (keyed by hand and street, so a re-render of the same turn
 * cannot chime twice and the next street can), never while an echo is open
 * (the turn the echo holds false is not a new turn), and never when the
 * player has switched it off. The effects (a two-note chime, a short
 * vibration on touch, the tab title) are the caller's; this file only says
 * when, and what the title should read.
 *
 * Nothing here mutates its inputs.
 */

/** localStorage key of the preference; absent means ON. */
export const TURN_ALERT_PREF_KEY = 'poker_turn_alert';

/** The tab title while it is the hero's turn. */
export const TURN_TITLE_MARK = '● Your turn';

/** The vibration pattern on touch devices (ms on, off, on, off, on). */
export const TURN_VIBRATION = Object.freeze([40, 40, 40]);

/** One key per turn: the hand and the street. */
export function turnKey(handNumber, phaseKey) {
  return `${Number(handNumber ?? 0)}:${String(phaseKey ?? '')}`;
}

/**
 * @param {object} facts
 * @param {boolean} facts.isMyTurn     the CERTIFIED view says it is the hero's turn
 * @param {boolean} facts.pendingOpen  an optimistic echo is open
 * @param {boolean} facts.enabled      the preference
 * @param {string} facts.key           turnKey(handNumber, phaseKey)
 * @param {string|null} facts.lastKey  the key the alert last fired for
 * @returns {{ fire: boolean, lastKey: string|null }} whether to alert, and the key to remember
 */
export function turnAlertDecision({ isMyTurn, pendingOpen, enabled, key, lastKey }) {
  if (!isMyTurn || pendingOpen) return { fire: false, lastKey: lastKey ?? null };
  if (key === lastKey) return { fire: false, lastKey };
  return { fire: enabled === true, lastKey: key };
}

/** The document title for the state: marked while it is the hero's turn. */
export function titleFor(baseTitle, myTurn) {
  const base = String(baseTitle ?? '').replace(new RegExp(`^${escapeRe(TURN_TITLE_MARK)}\\s*\\u00B7\\s*`), '');
  return myTurn ? `${TURN_TITLE_MARK} · ${base}` : base;
}

/** Read the preference (default ON). `storage` is localStorage-like or null. */
export function readTurnAlertPref(storage) {
  try {
    const raw = storage ? storage.getItem(TURN_ALERT_PREF_KEY) : null;
    return raw === null || raw === undefined ? true : raw !== 'false';
  } catch {
    return true;
  }
}

/** Persist the preference. Returns whether the write succeeded. */
export function writeTurnAlertPref(storage, enabled) {
  try {
    if (!storage) return false;
    storage.setItem(TURN_ALERT_PREF_KEY, enabled ? 'true' : 'false');
    return true;
  } catch {
    return false;
  }
}

/** Whether this device should vibrate: a touch device with the API. */
export function canVibrate(nav) {
  return !!nav && typeof nav.vibrate === 'function' && Number(nav.maxTouchPoints ?? 0) > 0;
}

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
