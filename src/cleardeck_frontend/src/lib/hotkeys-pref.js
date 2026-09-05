/**
 * The keyboard-shortcuts preference: OFF unless the player turned it on.
 *
 * The dock's hotkeys (F, C, R, A A, 1-6, + and -) send real money, and they
 * shipped always-on: with nothing focused, a stray F folded, a C called and an
 * R committed whatever the sizer held. PokerStars and every serious client
 * ship shortcuts off by default and let the player opt in, so this is an
 * opt-in preference, persisted under the app's own localStorage namespace,
 * read as a single guard in `resolveHotkey` ($lib/hotkeys.js), and surfaced
 * by the key-hints legend under the action row, which names where to turn
 * them on while they are off.
 *
 * Pure: the storage is handed in, so it can be a fake in a test or null on
 * the server. Nothing here mutates its inputs.
 */

/** localStorage key of the preference; absent means OFF. */
export const HOTKEYS_PREF_KEY = 'poker_hotkeys_enabled';

/** Where the toggle lives, for the legend while the shortcuts are off. */
export const HOTKEYS_TOGGLE_PLACE = 'the wallet menu, under Table alerts';

/** The legend's sentence while the shortcuts are off. */
export const HOTKEYS_OFF_HINT = `Keyboard shortcuts off. Turn on in ${HOTKEYS_TOGGLE_PLACE}.`;

/**
 * Read the preference. OFF by default: only the literal 'true' enables it,
 * so an absent key, a stale value or a storage that throws all mean off.
 * @param {{ getItem: (k: string) => string | null } | null | undefined} storage
 * @returns {boolean}
 */
export function readHotkeysPref(storage) {
  try {
    const raw = storage ? storage.getItem(HOTKEYS_PREF_KEY) : null;
    return raw === 'true';
  } catch {
    return false;
  }
}

/**
 * Persist the preference. Returns whether the write succeeded.
 * @param {{ setItem: (k: string, v: string) => void } | null | undefined} storage
 * @param {boolean} enabled
 * @returns {boolean}
 */
export function writeHotkeysPref(storage, enabled) {
  try {
    if (!storage) return false;
    storage.setItem(HOTKEYS_PREF_KEY, enabled === true ? 'true' : 'false');
    return true;
  } catch {
    return false;
  }
}
