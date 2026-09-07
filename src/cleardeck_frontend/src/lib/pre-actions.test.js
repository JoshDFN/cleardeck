import { describe, it, expect } from 'vitest';
import {
  armPreAction, armedTag, availablePreActions, keepPreAction, resolvePreAction,
} from './pre-actions.js';

const fmt = (v) => (v / 100_000_000).toFixed(2);
const E8 = 100_000_000;

describe('availablePreActions', () => {
  it('offers Check/Fold, Check, Call any when nothing is owed', () => {
    expect(availablePreActions({ callAmount: 0, fmt }).map((p) => p.id))
      .toEqual(['checkFold', 'check', 'callAny']);
  });
  it('offers Fold to any bet, Call X, Call any when facing a bet', () => {
    const list = availablePreActions({ callAmount: 0.2 * E8, fmt });
    expect(list.map((p) => p.id)).toEqual(['foldAny', 'call', 'callAny']);
    expect(list[1].label).toBe('Call 0.20');
  });
});

describe('armPreAction', () => {
  it('records the amount and hand it was armed under', () => {
    expect(armPreAction('call', { callAmount: 20, handNumber: 7 })).toEqual({ id: 'call', callAmount: 20, handNumber: 7 });
    expect(armPreAction('nonsense', { callAmount: 0, handNumber: 1 })).toBe(null);
  });
});

describe('keepPreAction (while still waiting)', () => {
  const hand = { handNumber: 3 };
  it('drops Call X when the amount to call changes (a raise behind)', () => {
    const armed = armPreAction('call', { callAmount: 20, ...hand });
    expect(keepPreAction(armed, { callAmount: 20, canCheck: false, ...hand })).toBe(armed);
    expect(keepPreAction(armed, { callAmount: 60, canCheck: false, ...hand })).toBe(null);
  });
  it('drops Check when a bet arrives', () => {
    const armed = armPreAction('check', { callAmount: 0, ...hand });
    expect(keepPreAction(armed, { callAmount: 0, canCheck: true, ...hand })).toBe(armed);
    expect(keepPreAction(armed, { callAmount: 10, canCheck: false, ...hand })).toBe(null);
  });
  it('keeps the context-free promises through a raise', () => {
    for (const id of ['checkFold', 'foldAny', 'callAny']) {
      const armed = armPreAction(id, { callAmount: 0, ...hand });
      expect(keepPreAction(armed, { callAmount: 90, canCheck: false, ...hand })).toBe(armed);
    }
  });
  it('drops everything when the hand changes', () => {
    const armed = armPreAction('callAny', { callAmount: 0, ...hand });
    expect(keepPreAction(armed, { callAmount: 0, canCheck: true, handNumber: 4 })).toBe(null);
  });
});

describe('resolvePreAction (the certified my-turn edge)', () => {
  const hand = { handNumber: 3 };
  it('does nothing until the state says it is my turn', () => {
    const armed = armPreAction('callAny', { callAmount: 0, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: false, callAmount: 0, canCheck: true, ...hand }))
      .toEqual({ fire: null, armed });
  });
  it('Check/Fold checks when free and folds when not', () => {
    const armed = armPreAction('checkFold', { callAmount: 0, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 0, canCheck: true, ...hand }).fire).toBe('check');
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 30, canCheck: false, ...hand }).fire).toBe('fold');
  });
  it('Call X calls only the amount agreed and cancels otherwise', () => {
    const armed = armPreAction('call', { callAmount: 20, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 20, canCheck: false, ...hand })).toEqual({ fire: 'call', armed: null });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 50, canCheck: false, ...hand })).toEqual({ fire: null, armed: null });
  });
  it('Call any calls a bet and checks a free spot', () => {
    const armed = armPreAction('callAny', { callAmount: 0, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 80, canCheck: false, ...hand }).fire).toBe('call');
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 0, canCheck: true, ...hand }).fire).toBe('check');
  });
  it('Fold to any bet folds a bet and checks a free spot', () => {
    const armed = armPreAction('foldAny', { callAmount: 20, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 20, canCheck: false, ...hand }).fire).toBe('fold');
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 0, canCheck: true, ...hand }).fire).toBe('check');
  });
  it('is one-shot: nothing stays armed after resolution', () => {
    const armed = armPreAction('check', { callAmount: 0, ...hand });
    expect(resolvePreAction(armed, { isMyTurn: true, callAmount: 0, canCheck: true, ...hand }).armed).toBe(null);
  });
});

describe('armedTag', () => {
  it('names the armed choice for the plate', () => {
    expect(armedTag(armPreAction('call', { callAmount: 0.2 * E8, handNumber: 1 }), fmt)).toBe('Call 0.20');
    expect(armedTag(armPreAction('checkFold', { callAmount: 0, handNumber: 1 }), fmt)).toBe('Check / Fold');
    expect(armedTag(null, fmt)).toBe(null);
  });
});
