import { describe, it, expect } from 'vitest';
import {
  beginPending, echoFor, echoedPlayer, humaneActionError, pendingExpired, pendingStatus,
  projectPending, sentLabel,
} from './optimistic.js';

describe('humaneActionError', () => {
  it('rephrases the expired-clock refusal and keeps precise canister reasons', () => {
    expect(humaneActionError('Your action timer expired before this action arrived; the hand has moved on.'))
      .toBe('The clock ran out before your action arrived; the hand has moved on.');
    expect(humaneActionError('Minimum raise is 0.10 (to 0.30)')).toBe('Minimum raise is 0.10 (to 0.30). Nothing was sent.');
    expect(humaneActionError(new Error('Not your turn'))).toBe('Not your turn yet. Your action was not sent.');
    expect(humaneActionError('')).toBe('The table did not accept that action.');
  });
});

const E8 = 100_000_000;
const fmt = (v) => (v / E8).toFixed(2);

function viewWith(hero, extra = {}) {
  return {
    hand_number: 7n,
    is_my_turn: true,
    action_on: 0,
    my_seat: [0],
    call_amount: 0.2 * E8,
    players: [[{ chips: 11.9 * E8, current_bet: 0.1 * E8, has_folded: false, is_all_in: false, ...hero }], []],
    ...extra,
  };
}

describe('beginPending', () => {
  it('snapshots the hero at the click', () => {
    const p = beginPending('call', null, viewWith(), 1000);
    expect(p).toMatchObject({ kind: 'call', seat: 0, handNumber: 7, sentAt: 1000, callAmount: 0.2 * E8 });
    expect(p.snapshot).toEqual({ chips: 11.9 * E8, currentBet: 0.1 * E8, folded: false, allIn: false });
    expect(Object.isFrozen(p)).toBe(true);
  });
  it('refuses an unknown kind or a spectator', () => {
    expect(beginPending('dance', null, viewWith(), 1)).toBe(null);
    expect(beginPending('call', null, { ...viewWith(), my_seat: [] }, 1)).toBe(null);
  });
});

describe('echoFor', () => {
  it('debits a call from the stack into the bet', () => {
    const echo = echoFor(beginPending('call', null, viewWith(), 1));
    expect(echo.chips).toBe(11.7 * E8);
    expect(echo.currentBet).toBe(0.3 * E8);
    expect(echo.tag).toBe('Call');
  });
  it('projects a raise-to as the new bet', () => {
    const echo = echoFor(beginPending('raise', 0.7 * E8, viewWith(), 1));
    expect(echo.currentBet).toBe(0.7 * E8);
    expect(echo.chips).toBe(11.3 * E8);
    expect(echo.amountShown).toBe(0.7 * E8);
  });
  it('an all-in empties the stack', () => {
    const echo = echoFor(beginPending('allin', null, viewWith(), 1));
    expect(echo.chips).toBe(0);
    expect(echo.currentBet).toBe(12 * E8);
    expect(echo.allIn).toBe(true);
  });
  it('a call for more than the stack is an all-in call', () => {
    const echo = echoFor(beginPending('call', null, viewWith({ chips: 0.05 * E8 }), 1));
    expect(echo.chips).toBe(0);
    expect(echo.allIn).toBe(true);
    expect(echo.amountShown).toBe(0.05 * E8);
  });
  it('fold and check move no money', () => {
    expect(echoFor(beginPending('fold', null, viewWith(), 1))).toMatchObject({ chips: 11.9 * E8, folded: true });
    expect(echoFor(beginPending('check', null, viewWith(), 1))).toMatchObject({ chips: 11.9 * E8, folded: false, tag: 'Check' });
  });
});

describe('sentLabel', () => {
  it('reads as the action in progress', () => {
    expect(sentLabel(beginPending('call', null, viewWith(), 1), fmt)).toBe('Calling 0.20');
    expect(sentLabel(beginPending('raise', 0.7 * E8, viewWith(), 1), fmt)).toBe('Raising to 0.70');
    expect(sentLabel(beginPending('fold', null, viewWith(), 1), fmt)).toBe('Folding');
  });
});

describe('pendingStatus / projectPending', () => {
  const pending = beginPending('call', null, viewWith(), 1);
  it('is open while the certified view still shows the pre-click state', () => {
    expect(pendingStatus(pending, viewWith())).toBe('open');
  });
  it('is absorbed when the clock moves, the hand moves or the hero record changes', () => {
    expect(pendingStatus(pending, viewWith({}, { is_my_turn: false }))).toBe('absorbed');
    expect(pendingStatus(pending, viewWith({}, { action_on: 1 }))).toBe('absorbed');
    expect(pendingStatus(pending, viewWith({}, { hand_number: 8n }))).toBe('absorbed');
    expect(pendingStatus(pending, viewWith({ current_bet: 0.3 * E8 }))).toBe('absorbed');
    expect(pendingStatus(pending, viewWith({ has_folded: true }))).toBe('absorbed');
  });
  it('holds is_my_turn false on a new object while open, and leaves an absorbed view alone', () => {
    const raw = viewWith();
    const projected = projectPending(raw, pending);
    expect(projected).not.toBe(raw);
    expect(projected.is_my_turn).toBe(false);
    expect(raw.is_my_turn).toBe(true);
    const done = viewWith({}, { is_my_turn: false, action_on: 1 });
    expect(projectPending(done, pending)).toBe(done);
    expect(projectPending(raw, null)).toBe(raw);
  });
});

describe('pendingExpired / echoedPlayer', () => {
  it('expires after the TTL', () => {
    const p = beginPending('check', null, viewWith(), 1000);
    expect(pendingExpired(p, 5000)).toBe(false);
    expect(pendingExpired(p, 20_000)).toBe(true);
    expect(pendingExpired(null, 20_000)).toBe(false);
  });
  it('returns a new player record with the echo applied', () => {
    const player = viewWith().players[0][0];
    const out = echoedPlayer(player, beginPending('call', null, viewWith(), 1));
    expect(out).not.toBe(player);
    expect(out.chips).toBe(11.7 * E8);
    expect(player.chips).toBe(11.9 * E8);
  });
});

describe('pendingStatus: a check that changes no hero figure', () => {
  const E8n = 100_000_000;
  const hero = { chips: 11.9 * E8n, current_bet: 0, has_folded: false, is_all_in: false };
  const preflop = viewWith(hero, { call_amount: 0, phase: { PreFlop: null }, community_cards: [], last_action: [] });
  const pending = beginPending('check', null, preflop, 1000);

  it('records the street, the board and the last action at the click', () => {
    expect(pending.street).toEqual({ phase: 'PreFlop', board: 0, lastAction: 'none' });
  });

  it('stays open while the view is byte-for-byte the pre-click state', () => {
    expect(pendingStatus(pending, preflop)).toBe('open');
  });

  it('is absorbed when the street advances and the turn comes straight back with identical figures', () => {
    const flop = viewWith(hero, {
      call_amount: 0, phase: { Flop: null },
      community_cards: [{ rank: 2, suit: 0 }, { rank: 9, suit: 1 }, { rank: 12, suit: 2 }], last_action: [],
    });
    expect(pendingStatus(pending, flop)).toBe('absorbed');
  });

  it('is absorbed when only the board length changes', () => {
    const board = viewWith(hero, { call_amount: 0, phase: { PreFlop: null }, community_cards: [{ rank: 2, suit: 0 }], last_action: [] });
    expect(pendingStatus(pending, board)).toBe('absorbed');
  });

  it('is absorbed when the last action identity changes, BigInt amounts included', () => {
    const acted = viewWith(hero, {
      call_amount: 0, phase: { PreFlop: null }, community_cards: [],
      last_action: [{ seat: 0, action: { Check: null }, timestamp: 1_700_000_000_000n }],
    });
    expect(pendingStatus(pending, acted)).toBe('absorbed');
    const other = viewWith(hero, {
      call_amount: 0, phase: { PreFlop: null }, community_cards: [],
      last_action: [{ seat: 0, action: { Raise: { amount: 30_000_000n } }, timestamp: 1_700_000_000_001n }],
    });
    expect(pendingStatus(pending, other)).toBe('absorbed');
  });

  it('a pending record without a street snapshot still reconciles on the old rules', () => {
    const legacy = { ...pending, street: undefined };
    expect(pendingStatus(legacy, preflop)).toBe('open');
    expect(pendingStatus(legacy, viewWith(hero, { is_my_turn: false }))).toBe('absorbed');
  });
});

describe('humaneActionError: refusal, transport failure, verbatim', () => {
  it('a canister refusal says nothing was sent', () => {
    expect(humaneActionError('Insufficient chips')).toBe('Insufficient chips. Nothing was sent.');
  });

  it('a throw never claims nothing was sent, because the update may have landed', () => {
    const text = humaneActionError(new Error('Failed to fetch'), { refusal: false });
    expect(text).not.toMatch(/nothing was sent/i);
    expect(text).toMatch(/may not have reached the table/i);
    expect(text).toMatch(/do not act again/i);
  });

  it('verbatim shows the text as given, with no suffix', () => {
    const msg = 'The table has not confirmed your action yet. It will show on the next update.';
    expect(humaneActionError(msg, { verbatim: true })).toBe(msg);
  });
});
