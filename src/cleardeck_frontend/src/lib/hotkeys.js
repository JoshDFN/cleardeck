/**
 * Keyboard shortcuts for the action dock, decided as a pure function so the
 * guards can be tested without a DOM.
 *
 * THE RULE THAT MATTERS: a key must never send a real-money action the player
 * did not choose. So:
 *  - Enter and Space are never shortcuts. They are the browser's own activation
 *    keys for whatever has focus, and intercepting them turned "Enter on the
 *    Fold button" into a raise (code review, 2026-09-04).
 *  - Nothing fires while a dialog, modal, or the expanded terms overlay is open.
 *  - Nothing fires while a text field has focus.
 *  - Nothing fires while a button OUTSIDE the dock has focus (a modal's Close,
 *    the header's wallet). A focused button INSIDE the dock (the action row,
 *    the Log toggle, the deck seal, Sit out) is fine for letter keys, because
 *    letters do not activate buttons natively. ActionBar passes the whole
 *    `.action-dock` as the dock.
 *  - A REGION beside the table (the action log, role="region") is not a
 *    dialog: the player keeps playing with it open.
 *
 * Nothing here mutates its inputs.
 */

/** @typedef {'none'|'field'|'dock-button'|'button'} FocusKind */

/**
 * Classify the focused element.
 * @param {Element|null} active   document.activeElement
 * @param {Element|null} dock     the dock's root element (the `.actions` row)
 * @returns {FocusKind}
 */
export function focusKindOf(active, dock) {
  if (!active || active === (active.ownerDocument && active.ownerDocument.body)) return 'none';
  const tag = active.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || active.isContentEditable === true) {
    return 'field';
  }
  const role = typeof active.getAttribute === 'function' ? active.getAttribute('role') : null;
  const isButton = tag === 'BUTTON' || tag === 'A' || role === 'button';
  if (!isButton) return 'none';
  return dock && typeof dock.contains === 'function' && dock.contains(active) ? 'dock-button' : 'button';
}

/** Selectors that mean "something modal is on the screen". */
export const DIALOG_SELECTOR = '[role="dialog"], [aria-modal="true"], .modal-backdrop, .alpha-warning-banner.expanded';

/**
 * @param {{ querySelector: (s: string) => Element|null }|null} doc
 * @returns {boolean}
 */
export function dialogIsOpen(doc) {
  if (!doc || typeof doc.querySelector !== 'function') return false;
  return doc.querySelector(DIALOG_SELECTOR) !== null;
}

/**
 * @typedef {object} HotkeyContext
 * @property {boolean} live          the hero is on the clock and nothing is sent
 * @property {boolean} modifier      meta, ctrl or alt is held
 * @property {FocusKind} focusKind
 * @property {boolean} dialogOpen
 * @property {boolean} canCheck
 * @property {boolean} canRaise
 * @property {boolean} raiseDisabled
 * @property {boolean} allInArmed    the first "A" press is standing
 * @property {boolean} compact
 * @property {boolean} sizerOpen
 * @property {ReadonlyArray<{id: string, key: string}>} presets
 */

/**
 * @typedef {{ type: 'action', action: 'fold'|'check'|'call'|'allin' }
 *   | { type: 'arm-allin' }
 *   | { type: 'commit-raise' }
 *   | { type: 'preset', id: string }
 *   | { type: 'step', delta: 1|-1 }
 *   | { type: 'escape' }} HotkeyDecision
 */

/**
 * Decide what a key press means, or null when it means nothing.
 * @param {string} key   KeyboardEvent.key
 * @param {HotkeyContext} ctx
 * @returns {HotkeyDecision|null}
 */
export function resolveHotkey(key, ctx) {
  if (!ctx || !ctx.live || ctx.modifier) return null;
  if (ctx.dialogOpen) return null;
  if (ctx.focusKind === 'field' || ctx.focusKind === 'button') return null;
  if (key === 'Enter' || key === ' ' || key === 'Spacebar') return null;

  const lower = typeof key === 'string' && key.length === 1 ? key.toLowerCase() : key;
  if (lower === 'f') return { type: 'action', action: 'fold' };
  if (lower === 'c') return { type: 'action', action: ctx.canCheck ? 'check' : 'call' };
  if (lower === 'r') return ctx.raiseDisabled ? null : { type: 'commit-raise' };
  if (lower === 'a') return ctx.allInArmed ? { type: 'action', action: 'allin' } : { type: 'arm-allin' };
  if (key === 'Escape') return { type: 'escape' };
  if (!ctx.canRaise) return null;
  const preset = (ctx.presets || []).find((p) => p.key === key);
  if (preset) return { type: 'preset', id: preset.id };
  if (key === '+' || key === '=') return { type: 'step', delta: 1 };
  if (key === '-' || key === '_') return { type: 'step', delta: -1 };
  return null;
}
