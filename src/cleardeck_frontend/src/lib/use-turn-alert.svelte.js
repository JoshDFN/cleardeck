/**
 * The your-turn alert, wired: the chime, the vibration and the tab title.
 *
 * `useTurnAlert(facts)` installs two effects in the calling component (call
 * it during init, like any rune). `facts` is a function returning the
 * certified facts of the moment:
 *
 *   {
 *     isMyTurn,        the CERTIFIED view says the hero is on the clock (and a hand is on)
 *     pendingOpen,     an optimistic echo is open
 *     handNumber, phaseKey, actionIdentity, currentBet   the turn's identity
 *   }
 *
 * The decision itself is $lib/turn-alert.js (pure, tested); this file only
 * reads the preference, plays the sound, vibrates where it can and marks the
 * document title while the turn is the hero's and nothing is in flight.
 * The preference is re-read on every edge so a change in the wallet menu
 * takes effect on the next turn without a reload.
 */
import { untrack } from 'svelte';
import { playSound } from '$lib/sounds.js';
import {
  canVibrate, readTurnAlertPref, titleFor, turnAlertDecision, turnKey, TURN_VIBRATION,
} from '$lib/turn-alert.js';

/**
 * @param {() => {isMyTurn:boolean, pendingOpen:boolean, handNumber:number|bigint, phaseKey:string, actionIdentity:string, currentBet:number}} facts
 * @param {{storage?: Storage|null, nav?: Navigator|null, doc?: Document|null}} [env]
 */
export function useTurnAlert(facts, env = {}) {
  const storage = env.storage !== undefined ? env.storage
    : (typeof localStorage !== 'undefined' ? localStorage : null);
  const nav = env.nav !== undefined ? env.nav : (typeof navigator !== 'undefined' ? navigator : null);
  const doc = env.doc !== undefined ? env.doc : (typeof document !== 'undefined' ? document : null);

  let alertedKey = $state(null);

  // The edge: chime and vibrate once per turn identity.
  $effect(() => {
    const f = facts();
    const decision = turnAlertDecision({
      isMyTurn: f.isMyTurn,
      pendingOpen: f.pendingOpen,
      enabled: readTurnAlertPref(storage),
      key: turnKey(f.handNumber, f.phaseKey, f.actionIdentity, f.currentBet),
      lastKey: untrack(() => alertedKey),
    });
    if (decision.lastKey !== untrack(() => alertedKey)) alertedKey = decision.lastKey;
    if (!decision.fire) return;
    playSound('yourTurn');
    if (canVibrate(nav)) {
      try { nav.vibrate(TURN_VIBRATION); } catch { /* a browser that refuses is fine */ }
    }
  });

  // The title: marked while it is the hero's turn and no echo is open,
  // restored when the turn passes, an action is sent, or the table unmounts.
  $effect(() => {
    if (!doc) return undefined;
    const f = facts();
    doc.title = titleFor(doc.title, f.isMyTurn && !f.pendingOpen);
    return () => { doc.title = titleFor(doc.title, false); };
  });
}
