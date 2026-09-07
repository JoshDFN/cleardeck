/**
 * BODY SCROLL LOCK BEHIND A DIALOG.
 *
 * A `position: fixed` dialog whose body is `overflow-y: auto` chains its
 * scroll to the page once the dialog's own scroll runs out: on a phone the
 * table behind the deposit sheet moved under the thumb, and on the lobby the
 * page behind a dialog rubber-banded (the audit's "scroll trap under modals").
 * The fix is the one every leader uses: while any dialog is open the document
 * itself cannot scroll. `overscroll-behavior: contain` on the dialog's scroller
 * stops the chaining; this stops the document.
 *
 * Reference-counted so two dialogs (a modal over a modal, the terms overlay
 * over a sheet) release in any order without unlocking early. The lock is a
 * class on the root element (`html.cd-scroll-lock { overflow: hidden }` in
 * index.scss) rather than an inline style, so the stylesheet owns the rule and
 * nothing here writes styles it has to remember to undo.
 */

export const SCROLL_LOCK_CLASS = 'cd-scroll-lock';

/**
 * @param {{classList: {add(c: string): void, remove(c: string): void}}|null} root
 *   the element that carries the class (documentElement), or null for SSR
 */
export function createScrollLock(root) {
  let holders = 0;

  /** Acquires the lock; returns the release function. Releasing twice is a no-op. */
  function lock() {
    holders += 1;
    if (holders === 1 && root) root.classList.add(SCROLL_LOCK_CLASS);
    let released = false;
    return function release() {
      if (released) return;
      released = true;
      holders = Math.max(0, holders - 1);
      if (holders === 0 && root) root.classList.remove(SCROLL_LOCK_CLASS);
    };
  }

  return {
    lock,
    get holders() { return holders; },
    get locked() { return holders > 0; },
  };
}

const pageLock = createScrollLock(
  typeof document !== 'undefined' ? document.documentElement : null,
);

/** The page-wide lock. `const release = lockPageScroll(); ... release();` */
export const lockPageScroll = () => pageLock.lock();

/**
 * Svelte action: `<div class="modal-content" use:scrollLock>`. Locks on mount,
 * releases on destroy, so a dialog that closes by any path unlocks.
 */
export function scrollLock() {
  const release = lockPageScroll();
  return { destroy: release };
}
