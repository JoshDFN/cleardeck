/**
 * The big-blind display preference as ONE piece of shared rune state.
 *
 * The toggle lives in the wallet menu (WalletButton.svelte, under Table
 * display) and the felt reads it in PokerTable.svelte; both read this object
 * so a change in the menu re-labels the stacks, the bets and the pot on the
 * next render without a reload. The decision and the storage format are
 * $lib/bb-display.js (pure, tested); this file only holds the live value and
 * keeps it in step with localStorage, including a `storage` event from
 * another tab. Same shape as $lib/hotkeys-pref.svelte.js.
 */
import { BB_DISPLAY_PREF_KEY, readBbDisplayPref, writeBbDisplayPref } from './bb-display.js';

const storage = typeof localStorage !== 'undefined' ? localStorage : null;

let enabled = $state(readBbDisplayPref(storage));

if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  window.addEventListener('storage', (event) => {
    if (event && event.key !== null && event.key !== undefined && event.key !== BB_DISPLAY_PREF_KEY) return;
    enabled = readBbDisplayPref(storage);
  });
}

export const bbDisplay = {
  /** Whether the felt's live figures read in big blinds. */
  get enabled() { return enabled; },
  /** Set and persist. */
  set(next) {
    enabled = next === true;
    writeBbDisplayPref(storage, enabled);
  },
  toggle() { this.set(!enabled); },
  /** Re-read the stored value (a test rig seeds storage directly). */
  refresh() { enabled = readBbDisplayPref(storage); },
};
