import { describe, expect, it } from 'vitest';
import { joinGateDecision, seatToResume } from './join-gate.js';

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
