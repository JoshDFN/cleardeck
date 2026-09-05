// @vitest-environment happy-dom
//
// The action against a real document: the caller's gates array is the
// caller's. `pinAfter` used to alias it and `update` spliced the new list into
// it, so a parent that kept its array (a constant, a derived value) had it
// rewritten under its feet.
import { describe, expect, it } from 'vitest';
import { pinAfter } from './pin-after.js';

function mount() {
  document.body.innerHTML = `
    <div class="modal-body">
      <div class="solvency"></div>
      <div class="runway-notice"></div>
      <div class="actions"></div>
    </div>`;
  return document.querySelector('.actions');
}

describe('pinAfter never mutates the caller\'s gates', () => {
  it('copies the list on entry and replaces it on update', () => {
    const node = mount();
    const mine = ['.solvency', '.runway-notice'];
    const before = [...mine];
    const action = pinAfter(node, { gates: mine, scroller: '.modal-body' });
    expect(typeof action.update).toBe('function');
    const next = ['.solvency'];
    action.update({ gates: next });
    expect(mine).toEqual(before);
    // and the caller's NEW list is not aliased either
    next.push('.something-else');
    action.update({ gates: ['.runway-notice'] });
    expect(next).toEqual(['.solvency', '.something-else']);
    action.destroy();
  });

  it('a mutation of the caller\'s array after the call does not reach the action', () => {
    const node = mount();
    const mine = ['.solvency'];
    const action = pinAfter(node, { gates: mine, scroller: '.modal-body' });
    mine.length = 0; // the caller emptied its own list
    // the action still measures: it has its own copy (no throw, no change of contract)
    expect(() => action.update({})).not.toThrow();
    action.destroy();
  });
});
