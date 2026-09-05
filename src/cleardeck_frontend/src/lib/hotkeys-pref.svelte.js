/**
 * The keyboard-shortcuts preference as ONE piece of shared rune state.
 *
 * The toggle lives in the wallet menu (WalletButton.svelte) and the guard
 * lives in the action bar (ActionBar.svelte); both read this object so a
 * change in the menu takes effect on the very next key press and the legend
 * under the action row flips without a reload. The decision and the storage
 * format are $lib/hotkeys-pref.js (pure, tested); this file only holds the
 * live value and keeps it in step with localStorage, including a `storage`
 * event from another tab (or from a test rig that seeds the key and
 * dispatches one).
 */
import { HOTKEYS_PREF_KEY, readHotkeysPref, writeHotkeysPref } from './hotkeys-pref.js';

const storage = typeof localStorage !== 'undefined' ? localStorage : null;

let enabled = $state(readHotkeysPref(storage));

if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  window.addEventListener('storage', (event) => {
    if (event && event.key !== null && event.key !== undefined && event.key !== HOTKEYS_PREF_KEY) return;
    enabled = readHotkeysPref(storage);
  });
}

export const hotkeysPref = {
  /** Whether the dock's letter and digit keys may act. */
  get enabled() { return enabled; },
  /** Set and persist. */
  set(next) {
    enabled = next === true;
    writeHotkeysPref(storage, enabled);
  },
  toggle() { this.set(!enabled); },
  /** Re-read the stored value (a test rig seeds storage directly). */
  refresh() { enabled = readHotkeysPref(storage); },
};
