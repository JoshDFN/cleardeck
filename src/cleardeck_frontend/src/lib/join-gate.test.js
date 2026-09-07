import { describe, expect, it } from 'vitest';
import { escrowFromRead, joinGateDecision, joinGateFacts, seatToResume } from './join-gate.js';

describe('joinGateDecision', () => {
  it('sends an anonymous tap to sign-in, whatever the balance', () => {
    expect(joinGateDecision({ isAuthenticated: false, escrow: 10_000_000_000, minBuyIn: 1_000_000_000 }))
      .toEqual({ kind: 'login' });
  });

  it('sends a funded player straight to the seat', () => {
    expect(joinGateDecision({ isAuthenticated: true, escrow: 1_000_000_000, minBuyIn: 1_000_000_000 }))
      .toEqual({ kind: 'join' });
  });

  it('sends an underfunded player to the cashier with the exact shortfall', () => {
    expect(joinGateDecision({ isAuthenticated: true, escrow: 250_000_000, minBuyIn: 1_000_000_000 }))
      .toEqual({ kind: 'deposit', shortfall: 750_000_000 });
  });

  it('accepts bigint figures (the canister speaks nat64)', () => {
    expect(joinGateDecision({ isAuthenticated: true, escrow: 0n, minBuyIn: 200_000_000n }))
      .toEqual({ kind: 'deposit', shortfall: 200_000_000 });
  });

  it('does not treat an unknown balance or buy-in as zero', () => {
    expect(joinGateDecision({ isAuthenticated: true, escrow: null, minBuyIn: 1_000_000_000 }))
      .toEqual({ kind: 'join' });
    expect(joinGateDecision({ isAuthenticated: true, escrow: 0, minBuyIn: undefined }))
      .toEqual({ kind: 'join' });
  });
});

describe('joinGateFacts', () => {
  // The table canister's view, as get_table_view() returns it (nat64 -> bigint).
  const tableView = { config: { min_buy_in: 1_000_000_000n, big_blind: 10_000_000n } };

  it('reads the minimum buy-in from the TABLE canister\'s config, as a number', () => {
    expect(joinGateFacts({ isAuthenticated: true, escrow: 250_000_000, tableView }))
      .toEqual({ isAuthenticated: true, escrow: 250_000_000, minBuyIn: 1_000_000_000 });
  });

  it('has no way to be handed the lobby record (T-11: the record is 5x-10x wrong on this stack)', () => {
    // A caller that tried to pass the lobby record's figure finds no parameter
    // for it: only the table view is read.
    const facts = joinGateFacts({
      isAuthenticated: true, escrow: 250_000_000, tableView, lobbyRecord: { config: { min_buy_in: 200_000_000 } },
    });
    expect(facts.minBuyIn).toBe(1_000_000_000);
    expect(joinGateDecision(facts)).toEqual({ kind: 'deposit', shortfall: 750_000_000 });
  });

  it('treats a missing table view as an unknown buy-in, so the canister decides', () => {
    const facts = joinGateFacts({ isAuthenticated: true, escrow: 0, tableView: null });
    expect(facts.minBuyIn).toBeNull();
    expect(joinGateDecision(facts)).toEqual({ kind: 'join' });
  });

  it('carries an unread balance as null, never as zero', () => {
    const facts = joinGateFacts({ isAuthenticated: true, escrow: null, tableView });
    expect(facts.escrow).toBeNull();
    expect(joinGateDecision(facts)).toEqual({ kind: 'join' });
  });
});

describe('escrowFromRead', () => {
  it('turns a successful get_balance() into a number', () => {
    expect(escrowFromRead({ ok: true, value: 1_200_000_000n })).toBe(1_200_000_000);
    expect(escrowFromRead({ ok: true, value: 0n })).toBe(0);
  });

  it('turns a failed read into null, and the gate then lets the canister decide', () => {
    expect(escrowFromRead({ ok: false })).toBeNull();
    expect(escrowFromRead(undefined)).toBeNull();
    const facts = joinGateFacts({
      isAuthenticated: true, escrow: escrowFromRead({ ok: false }), tableView: { config: { min_buy_in: 1_000_000_000n } },
    });
    expect(joinGateDecision(facts)).toEqual({ kind: 'join' });
  });

  it('a read of zero IS zero: the funded-or-not question has an answer', () => {
    const facts = joinGateFacts({
      isAuthenticated: true, escrow: escrowFromRead({ ok: true, value: 0n }), tableView: { config: { min_buy_in: 1_000_000_000n } },
    });
    expect(joinGateDecision(facts)).toEqual({ kind: 'deposit', shortfall: 1_000_000_000 });
  });
});

describe('seatToResume', () => {
  it('returns the remembered seat for the same table', () => {
    expect(seatToResume({ seat: 3, tableKey: 't2' }, 't2')).toEqual({ seat: 3 });
  });

  it('forgets a seat remembered for another table', () => {
    expect(seatToResume({ seat: 3, tableKey: 't1' }, 't2')).toBeNull();
  });

  it('refuses nonsense seats and nothing remembered', () => {
    expect(seatToResume(null, 't2')).toBeNull();
    expect(seatToResume({ seat: -1, tableKey: 't2' }, 't2')).toBeNull();
    expect(seatToResume({ seat: 1.5, tableKey: 't2' }, 't2')).toBeNull();
  });
});
