import { describe, it, expect } from 'vitest';
import {
  PRESETS, clampRaise, formatAmountInput, parseAmountInput, presetAt, presetTarget,
  presetTargets, raiseCap, raiseFloor, raiseKind, raiseProblem, sliderStep, stepByBlind,
} from './bet-sizing.js';

// 0.05/0.10 blinds in e8s; the hero (BB, 0.10 in) faces a raise to 0.30 with 11.90 behind.
const ICP = 100_000_000;
const facing = {
  currentBet: 0.30 * ICP, minRaise: 0.20 * ICP, minBet: 0.10 * ICP,
  myChips: 11.90 * ICP, myCurrentBet: 0.10 * ICP, pot: 0.45 * ICP, callAmount: 0.20 * ICP,
};
const open = {
  currentBet: 0, minRaise: 0.10 * ICP, minBet: 0.10 * ICP,
  myChips: 11.80 * ICP, myCurrentBet: 0, pot: 0.60 * ICP, callAmount: 0,
};

describe('raiseFloor / raiseCap', () => {
  it('is current_bet + min_raise when facing a bet', () => {
    expect(raiseFloor(facing)).toBe(0.50 * ICP);
  });
  it('is min_bet when nothing is in front', () => {
    expect(raiseFloor(open)).toBe(0.10 * ICP);
  });
  it('caps at chips plus what is already in', () => {
    expect(raiseCap(facing)).toBe(12 * ICP);
  });
});

describe('presets (the harness formulas)', () => {
  it('pot = B + P + c', () => {
    expect(presetTarget('pot', facing)).toBe((0.30 + 0.45 + 0.20) * ICP);
  });
  it('half pot = B + floor((P + c) / 2), floored at the legal minimum', () => {
    // (0.45 + 0.20) / 2 = 0.325 -> 0.30 + 0.325 = 0.625
    expect(presetTarget('half', facing)).toBe(Math.round(0.625 * ICP));
    // with a tiny pot the half falls below the floor and is lifted to it
    expect(presetTarget('half', { ...facing, pot: 0.10 * ICP, callAmount: 0.20 * ICP })).toBe(0.50 * ICP);
  });
  it('two thirds = B + floor(2(P + c) / 3)', () => {
    expect(presetTarget('twoThirds', open)).toBe(Math.floor((2 * 0.60 * ICP) / 3));
  });
  it('all in is the cap and min is the floor', () => {
    expect(presetTarget('allin', facing)).toBe(12 * ICP);
    expect(presetTarget('min', facing)).toBe(0.50 * ICP);
  });
  it('with no bet in front the same formula is a plain bet of the pot fraction', () => {
    expect(presetTarget('pot', open)).toBe(0.60 * ICP);
    expect(presetTarget('half', open)).toBe(0.30 * ICP);
  });
  it('never exceeds the stack', () => {
    const short = { ...facing, myChips: 0.25 * ICP };
    expect(presetTarget('pot', short)).toBe(0.35 * ICP);
    expect(presetTargets(short).allin).toBe(0.35 * ICP);
  });
  it('names the preset a value sits on', () => {
    expect(presetAt(presetTarget('pot', facing), facing)).toBe('pot');
    expect(presetAt(0.51 * ICP, facing)).toBe(null);
  });
  it('lists five presets in order with number keys', () => {
    expect(PRESETS.map((p) => p.key)).toEqual(['1', '2', '3', '4', '5']);
  });
});

describe('clampRaise / stepByBlind / sliderStep', () => {
  it('clamps into [floor, cap] and collapses to the cap when the cap is under the floor', () => {
    expect(clampRaise(1, 50, 1200)).toBe(50);
    expect(clampRaise(5000, 50, 1200)).toBe(1200);
    expect(clampRaise(30, 50, 40)).toBe(40);
  });
  it('steps one big blind and walks the blind grid', () => {
    const bb = 0.10 * ICP;
    expect(stepByBlind(0.50 * ICP, bb, +1, 0.50 * ICP, 12 * ICP)).toBe(0.60 * ICP);
    expect(stepByBlind(0.55 * ICP, bb, +1, 0.50 * ICP, 12 * ICP)).toBe(0.60 * ICP);
    expect(stepByBlind(0.55 * ICP, bb, -1, 0.50 * ICP, 12 * ICP)).toBe(0.50 * ICP);
    expect(stepByBlind(0.50 * ICP, bb, -1, 0.50 * ICP, 12 * ICP)).toBe(0.50 * ICP);
  });
  it('slider step is a quarter of the min raise, at least one unit', () => {
    expect(sliderStep(0.20 * ICP)).toBe(0.05 * ICP);
    expect(sliderStep(0)).toBe(1);
  });
});

describe('raiseKind / raiseProblem', () => {
  const fmt = (v) => (v / ICP).toFixed(2);
  it('is a bet with nothing in front and a raise otherwise', () => {
    expect(raiseKind(0)).toBe('bet');
    expect(raiseKind(1)).toBe('raise');
  });
  it('explains an illegal size in the words the canister would use', () => {
    expect(raiseProblem(0.40 * ICP, facing, fmt)).toBe('Minimum raise is 0.50');
    expect(raiseProblem(13 * ICP, facing, fmt)).toBe('You have 12.00 behind');
    expect(raiseProblem(0.50 * ICP, facing, fmt)).toBe(null);
  });
});

describe('amount field parse and format', () => {
  it('parses ICP text into e8s and rejects junk', () => {
    expect(parseAmountInput('0.30')).toBe(0.30 * ICP);
    expect(parseAmountInput('1,000.5')).toBe(1000.5 * ICP);
    expect(parseAmountInput('abc')).toBe(null);
    expect(parseAmountInput('')).toBe(null);
    expect(parseAmountInput('-1')).toBe(null);
  });
  it('parses sats as whole numbers', () => {
    expect(parseAmountInput('1500', { isBTC: true })).toBe(1500);
  });
  it('formats with the table precision', () => {
    expect(formatAmountInput(0.30 * ICP, { decimals: 2 })).toBe('0.30');
    expect(formatAmountInput(12_345, { decimals: 4 })).toBe('0.0001');
    expect(formatAmountInput(1500, { isBTC: true })).toBe('1500');
  });
});
