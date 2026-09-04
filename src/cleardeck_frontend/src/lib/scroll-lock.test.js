import { describe, expect, it } from 'vitest';
import { SCROLL_LOCK_CLASS, createScrollLock } from './scroll-lock.js';

function fakeRoot() {
  const classes = new Set();
  return {
    classes,
    classList: {
      add: (c) => classes.add(c),
      remove: (c) => classes.delete(c),
    },
  };
}

describe('createScrollLock', () => {
  it('adds the class on the first lock and removes it on the last release', () => {
    const root = fakeRoot();
    const lock = createScrollLock(root);
    const release = lock.lock();
    expect(root.classes.has(SCROLL_LOCK_CLASS)).toBe(true);
    expect(lock.locked).toBe(true);
    release();
    expect(root.classes.has(SCROLL_LOCK_CLASS)).toBe(false);
    expect(lock.locked).toBe(false);
  });

  it('keeps the lock while any holder remains, in any release order', () => {
    const root = fakeRoot();
    const lock = createScrollLock(root);
    const a = lock.lock();
    const b = lock.lock();
    expect(lock.holders).toBe(2);
    a();
    expect(root.classes.has(SCROLL_LOCK_CLASS)).toBe(true);
    b();
    expect(root.classes.has(SCROLL_LOCK_CLASS)).toBe(false);
  });

  it('ignores a second release of the same holder', () => {
    const root = fakeRoot();
    const lock = createScrollLock(root);
    const a = lock.lock();
    lock.lock();
    a();
    a();
    expect(lock.holders).toBe(1);
    expect(root.classes.has(SCROLL_LOCK_CLASS)).toBe(true);
  });

  it('tolerates no root (server render)', () => {
    const lock = createScrollLock(null);
    const release = lock.lock();
    expect(lock.locked).toBe(true);
    release();
    expect(lock.locked).toBe(false);
  });
});
