import { describe, it, expect } from 'vitest';
import { PRESETS } from './bet-sizing.js';
import { dialogIsOpen, focusKindOf, resolveHotkey } from './hotkeys.js';

const base = Object.freeze({
  live: true, modifier: false, focusKind: 'none', dialogOpen: false,
  canCheck: false, canRaise: true, raiseDisabled: false, allInArmed: false,
  compact: false, sizerOpen: false, presets: PRESETS,
});

describe('resolveHotkey: the keys that must never send money', () => {
  it('Enter and Space are never shortcuts, whatever has focus', () => {
    for (const focusKind of ['none', 'field', 'dock-button', 'button']) {
      expect(resolveHotkey('Enter', { ...base, focusKind })).toBeNull();
      expect(resolveHotkey(' ', { ...base, focusKind })).toBeNull();
    }
  });

  it('nothing fires while a dialog is open, even a plain letter', () => {
    const ctx = { ...base, dialogOpen: true };
    expect(resolveHotkey('f', ctx)).toBeNull();
    expect(resolveHotkey('c', ctx)).toBeNull();
    expect(resolveHotkey('a', ctx)).toBeNull();
    expect(resolveHotkey('r', ctx)).toBeNull();
    expect(resolveHotkey('4', ctx)).toBeNull();
  });

  it('nothing fires while a text field or a button outside the dock has focus', () => {
    expect(resolveHotkey('f', { ...base, focusKind: 'field' })).toBeNull();
    expect(resolveHotkey('f', { ...base, focusKind: 'button' })).toBeNull();
    expect(resolveHotkey('c', { ...base, focusKind: 'button' })).toBeNull();
  });

  it('nothing fires off the clock or with a modifier held', () => {
    expect(resolveHotkey('f', { ...base, live: false })).toBeNull();
    expect(resolveHotkey('f', { ...base, modifier: true })).toBeNull();
  });
});

describe('resolveHotkey: the keys that do work', () => {
  it('letters work with no focus and with a focused button inside the dock', () => {
    for (const focusKind of ['none', 'dock-button']) {
      expect(resolveHotkey('f', { ...base, focusKind })).toEqual({ type: 'action', action: 'fold' });
      expect(resolveHotkey('F', { ...base, focusKind })).toEqual({ type: 'action', action: 'fold' });
      expect(resolveHotkey('c', { ...base, focusKind })).toEqual({ type: 'action', action: 'call' });
      expect(resolveHotkey('c', { ...base, focusKind, canCheck: true })).toEqual({ type: 'action', action: 'check' });
    }
  });

  it('R commits the raise only when a raise is allowed', () => {
    expect(resolveHotkey('r', base)).toEqual({ type: 'commit-raise' });
    expect(resolveHotkey('r', { ...base, raiseDisabled: true })).toBeNull();
  });

  it('A arms first and fires on the second press', () => {
    expect(resolveHotkey('a', base)).toEqual({ type: 'arm-allin' });
    expect(resolveHotkey('a', { ...base, allInArmed: true })).toEqual({ type: 'action', action: 'allin' });
  });

  it('presets and steps need canRaise', () => {
    expect(resolveHotkey('4', base)).toEqual({ type: 'preset', id: 'pot' });
    expect(resolveHotkey('+', base)).toEqual({ type: 'step', delta: 1 });
    expect(resolveHotkey('-', base)).toEqual({ type: 'step', delta: -1 });
    expect(resolveHotkey('4', { ...base, canRaise: false })).toBeNull();
    expect(resolveHotkey('+', { ...base, canRaise: false })).toBeNull();
  });

  it('Escape is handled and unknown keys are not', () => {
    expect(resolveHotkey('Escape', base)).toEqual({ type: 'escape' });
    expect(resolveHotkey('x', base)).toBeNull();
    expect(resolveHotkey('Tab', base)).toBeNull();
  });
});

function fakeEl(tagName, { role = null, editable = false, body = null } = {}) {
  return {
    tagName, isContentEditable: editable,
    getAttribute: (name) => (name === 'role' ? role : null),
    ownerDocument: { body },
  };
}

describe('focusKindOf', () => {
  const body = { tagName: 'BODY' };
  const dockChild = fakeEl('BUTTON', { body });
  const dock = { contains: (el) => el === dockChild };

  it('classifies fields, dock buttons, other buttons, and nothing', () => {
    expect(focusKindOf(null, dock)).toBe('none');
    expect(focusKindOf(fakeEl('INPUT', { body }), dock)).toBe('field');
    expect(focusKindOf(fakeEl('DIV', { editable: true, body }), dock)).toBe('field');
    expect(focusKindOf(dockChild, dock)).toBe('dock-button');
    expect(focusKindOf(fakeEl('BUTTON', { body }), dock)).toBe('button');
    expect(focusKindOf(fakeEl('A', { body }), dock)).toBe('button');
    expect(focusKindOf(fakeEl('DIV', { role: 'button', body }), dock)).toBe('button');
    expect(focusKindOf(fakeEl('DIV', { body }), dock)).toBe('none');
  });

  it('treats a missing dock as outside', () => {
    expect(focusKindOf(dockChild, null)).toBe('button');
  });
});

describe('dialogIsOpen', () => {
  it('reads the document for a modal, a dialog, or the expanded terms', () => {
    expect(dialogIsOpen({ querySelector: () => ({}) })).toBe(true);
    expect(dialogIsOpen({ querySelector: () => null })).toBe(false);
    expect(dialogIsOpen(null)).toBe(false);
  });
});
