import { describe, it, expect } from 'vitest';
import {
  POSTFLOP_PRESETS, PREFLOP_PRESETS, PRESETS, clampRaise, displayQuantum, exactDecimals, formatAmountInput,
  formatExact, isAllInRaise, parseAmountInput, presetAt, presetTarget, presetTargets, presetsForPhase,
  primaryRaiseLabel, quantise, raiseCap, raiseFloor, raiseKind, raiseProblem, sliderStep, snapRaise,
  stepByBlind, typedPrecisionDropped, displayUnitLabel,
} from './bet-sizing.js';

// 0.05/0.10 blinds in e8s; the hero (BB, 0.10 in) faces a raise to 0.30 with 11.90 behind.
const ICP = 100_000_000;
const CENT = 1_000_000; // the display unit at two decimals
/** ICP as an exact integer of e8s (0.07 * 1e8 is not an integer in floating point). */
const e8 = (icp) => Math.round(icp * ICP);
const facing = {
  currentBet: e8(0.30), minRaise: e8(0.20), minBet: e8(0.10),
  myChips: e8(11.90), myCurrentBet: e8(0.10), pot: e8(0.45), callAmount: e8(0.20),
  bigBlind: e8(0.10), quantum: CENT,
};
const open = {
  currentBet: 0, minRaise: e8(0.10), minBet: e8(0.10),
  myChips: e8(11.80), myCurrentBet: 0, pot: e8(0.60), callAmount: 0,
  bigBlind: e8(0.10), quantum: CENT,
};
// The harness's own facing-bet scene: pot 0.30, call 0.10, current bet 0.20.
const harnessFacing = {
  currentBet: e8(0.20), minRaise: e8(0.10), minBet: e8(0.10),
  myChips: e8(11.90), myCurrentBet: e8(0.10), pot: e8(0.30), callAmount: e8(0.10),
  bigBlind: e8(0.10), quantum: CENT,
};

describe('displayQuantum / quantise', () => {
  it('is 10^(8 - decimals) e8s for ICP and one sat for BTC', () => {
    expect(displayQuantum({ decimals: 2 })).toBe(CENT);
    expect(displayQuantum({ decimals: 4 })).toBe(10_000);
    expect(displayQuantum({ isBTC: true, decimals: 0 })).toBe(1);
  });
  it('rounds DOWN onto the grid and leaves grid values alone', () => {
    expect(quantise(46_666_666, CENT)).toBe(46_000_000);
    expect(quantise(7_500_000, CENT)).toBe(7_000_000);
    expect(quantise(30_000_000, CENT)).toBe(30_000_000);
    expect(quantise(123, 1)).toBe(123);
  });
});

describe('raiseFloor / raiseCap', () => {
  it('is current_bet + min_raise when facing a bet', () => {
    expect(raiseFloor(facing)).toBe(e8(0.50));
  });
  it('is min_bet when nothing is in front', () => {
    expect(raiseFloor(open)).toBe(e8(0.10));
  });
  it('caps at chips plus what is already in', () => {
    expect(raiseCap(facing)).toBe(e8(12));
  });
});

describe('presets (the harness formulas)', () => {
  it('pot = B + P + c', () => {
    expect(presetTarget('pot', facing)).toBe((0.30 + 0.45 + 0.20) * ICP);
  });
  it('half pot = B + (P + c) / 2, floored at the legal minimum', () => {
    // (0.45 + 0.20) / 2 = 0.325 -> quantised 0.32 -> 0.30 + 0.32 = 0.62
    expect(presetTarget('half', facing)).toBe(e8(0.62));
    // with a tiny pot the half falls below the floor and is lifted to it
    expect(presetTarget('half', { ...facing, pot: e8(0.10), callAmount: e8(0.20) })).toBe(e8(0.50));
  });
  it('half of a 0.15 pot is 0.07, not 0.075 shown as 0.08 (the reviewer case)', () => {
    const ctx = { ...open, pot: e8(0.15), minBet: e8(0.05) };
    expect(presetTarget('half', ctx)).toBe(e8(0.07));
  });
  it('two thirds = B + 2(P + c) / 3, on the display grid', () => {
    // open: 2 * 0.60 / 3 = 0.40 exactly
    expect(presetTarget('twoThirds', open)).toBe(e8(0.40));
    // the harness scene: 2 * (0.30 + 0.10) / 3 = 0.2666.. -> 0.26 -> raise to 0.46
    expect(presetTarget('twoThirds', harnessFacing)).toBe(e8(0.46));
    // unquantised, the same figure would be 46,666,666 e8s
    expect(presetTarget('twoThirds', { ...harnessFacing, quantum: 1 })).toBe(46_666_666);
  });
  it('all in is the cap and min is the floor', () => {
    expect(presetTarget('allin', facing)).toBe(e8(12));
    expect(presetTarget('min', facing)).toBe(e8(0.50));
  });
  it('with no bet in front the same formula is a plain bet of the pot fraction', () => {
    expect(presetTarget('pot', open)).toBe(e8(0.60));
    expect(presetTarget('half', open)).toBe(e8(0.30));
  });
  it('never exceeds the stack', () => {
    const short = { ...facing, myChips: e8(0.25) };
    expect(presetTarget('pot', short)).toBe(e8(0.35));
    expect(presetTargets(short).allin).toBe(e8(0.35));
  });
  it('never rounds the canister\'s own floor or cap, only proposals', () => {
    const odd = { ...facing, currentBet: e8(0.303), minRaise: e8(0.2) };
    expect(presetTarget('min', odd)).toBe(raiseFloor(odd));
    expect(presetTarget('allin', odd)).toBe(raiseCap(odd));
  });
  it('names the preset a value sits on', () => {
    expect(presetAt(presetTarget('pot', facing), facing)).toBe('pot');
    expect(presetAt(e8(0.51), facing)).toBe(null);
  });
  it('lists five post-flop presets in order with number keys', () => {
    expect(PRESETS).toBe(POSTFLOP_PRESETS);
    expect(PRESETS.map((p) => p.key)).toEqual(['1', '2', '3', '4', '5']);
    expect(PRESETS.map((p) => p.label)).toEqual(['Min', '½ Pot', '⅔ Pot', 'Pot', 'All in']);
  });
});

describe('pre-flop presets (multiples)', () => {
  it('is the six-button row only on PreFlop', () => {
    expect(presetsForPhase('PreFlop')).toBe(PREFLOP_PRESETS);
    expect(presetsForPhase('Flop')).toBe(POSTFLOP_PRESETS);
    expect(presetsForPhase(undefined)).toBe(POSTFLOP_PRESETS);
    expect(PREFLOP_PRESETS.map((p) => p.label)).toEqual(['Min', '2.5x', '3x', '4x', 'Pot', 'All in']);
    expect(PREFLOP_PRESETS.map((p) => p.key)).toEqual(['1', '2', '3', '4', '5', '6']);
  });
  it('multiplies the bet in front when facing a raise', () => {
    // the harness scene: current bet 0.20
    expect(presetTarget('x2_5', harnessFacing)).toBe(e8(0.50));
    expect(presetTarget('x3', harnessFacing)).toBe(e8(0.60));
    expect(presetTarget('x4', harnessFacing)).toBe(e8(0.80));
  });
  it('multiplies the big blind when only the blinds are in', () => {
    const blindsOnly = { ...harnessFacing, currentBet: e8(0.10), callAmount: e8(0.05), pot: e8(0.15), myCurrentBet: e8(0.05) };
    expect(presetTarget('x2_5', blindsOnly)).toBe(e8(0.25));
    expect(presetTarget('x3', blindsOnly)).toBe(e8(0.30));
    expect(presetTarget('x4', blindsOnly)).toBe(e8(0.40));
  });
  it('is lifted to the floor when the multiple is below the legal minimum', () => {
    // facing 0.30 with min raise 0.20: 2.5x = 0.75 is legal, but a 0.05 blind table with a 3-bet is not
    const big = { ...facing, currentBet: e8(2), minRaise: e8(5) };
    expect(presetTarget('x2_5', big)).toBe(e8(7));
  });
  it('presetTargets and presetAt take the row', () => {
    const t = presetTargets(harnessFacing, PREFLOP_PRESETS);
    expect(Object.keys(t)).toEqual(['min', 'x2_5', 'x3', 'x4', 'pot', 'allin']);
    expect(presetAt(e8(0.80), harnessFacing, PREFLOP_PRESETS)).toBe('x4');
  });
});

describe('clampRaise / stepByBlind / sliderStep', () => {
  it('clamps into [floor, cap] and collapses to the cap when the cap is under the floor', () => {
    expect(clampRaise(1, 50, 1200)).toBe(50);
    expect(clampRaise(5000, 50, 1200)).toBe(1200);
    expect(clampRaise(30, 50, 40)).toBe(40);
  });
  it('steps one big blind and walks the blind grid', () => {
    const bb = e8(0.10);
    expect(stepByBlind(e8(0.50), bb, +1, e8(0.50), e8(12))).toBe(e8(0.60));
    expect(stepByBlind(e8(0.55), bb, +1, e8(0.50), e8(12))).toBe(e8(0.60));
    expect(stepByBlind(e8(0.55), bb, -1, e8(0.50), e8(12))).toBe(e8(0.50));
    expect(stepByBlind(e8(0.50), bb, -1, e8(0.50), e8(12))).toBe(e8(0.50));
  });
  it('steps land on the display grid even from an off-grid blind', () => {
    const bb = e8(0.003); // a blind finer than the display
    expect(stepByBlind(0, bb, +1, 0, e8(12), CENT) % CENT).toBe(0);
  });
  it('slider step is one display unit, so the range can hold every quantised proposal', () => {
    expect(sliderStep(e8(0.20), CENT)).toBe(CENT);
    expect(sliderStep(e8(0.10), CENT)).toBe(CENT);
    expect(sliderStep(0)).toBe(1);
    expect(sliderStep(e8(0.10), 1)).toBe(1);
    // every preset is min + k * step from a grid-aligned floor
    const floor = raiseFloor(harnessFacing);
    for (const id of ['x2_5', 'x3', 'x4', 'pot']) {
      expect((presetTarget(id, harnessFacing) - floor) % sliderStep(harnessFacing.minRaise, CENT)).toBe(0);
    }
  });
});

describe('the all-in figure is the EXACT stack, never the display grid', () => {
  // An off-grid stack: 1.23456789 ICP behind plus 0.10 in front.
  const oddStack = { ...facing, myChips: 123_456_789, myCurrentBet: e8(0.10) };
  const oddCap = 123_456_789 + e8(0.10);
  // The same shape on a BTC table: sats are integral, the quantum is 1.
  const satsStack = {
    currentBet: 300, minRaise: 200, minBet: 100, myChips: 12_345, myCurrentBet: 100,
    pot: 450, callAmount: 200, bigBlind: 100, quantum: 1,
  };

  it('the All in preset on an off-grid stack equals the stack in e8s', () => {
    expect(raiseCap(oddStack)).toBe(oddCap);
    expect(oddCap % CENT).not.toBe(0);
    expect(presetTarget('allin', oddStack)).toBe(oddCap);
    expect(presetTargets(oddStack).allin).toBe(oddCap);
    // quantised down it would have been 456,789 e8s short of all in
    expect(quantise(oddCap, CENT)).toBe(oddCap - 456_789);
  });

  it('the All in preset on a BTC table equals the stack in sats', () => {
    expect(presetTarget('allin', satsStack)).toBe(12_445);
    expect(presetTargets(satsStack, PRESETS).allin).toBe(12_445);
  });

  it('a pot preset that reaches the cap is the exact cap, not the grid under it', () => {
    const tiny = { ...oddStack, pot: e8(50) };
    expect(presetTarget('pot', tiny)).toBe(oddCap);
    expect(presetTarget('x4', { ...tiny, currentBet: e8(40), minRaise: e8(1) })).toBe(oddCap);
  });

  it('snapRaise: at or past the cap the cap, under it the grid, never below the floor', () => {
    const floor = raiseFloor(oddStack);
    expect(snapRaise(oddCap, floor, oddCap, CENT)).toBe(oddCap);
    expect(snapRaise(oddCap + 5, floor, oddCap, CENT)).toBe(oddCap);
    expect(snapRaise(oddCap - 1, floor, oddCap, CENT)).toBe(quantise(oddCap - 1, CENT));
    expect(snapRaise(1, floor, oddCap, CENT)).toBe(floor);
    expect(isAllInRaise(oddCap, oddStack)).toBe(true);
    expect(isAllInRaise(oddCap - 1, oddStack)).toBe(false);
  });

  it('stepping a blind past the cap lands on the exact cap', () => {
    const floor = raiseFloor(oddStack);
    // 1.3046 + a blind is 1.40, past the cap: the cap, not 1.30 under it
    expect(stepByBlind(oddCap - e8(0.03), e8(0.10), +1, floor, oddCap, CENT)).toBe(oddCap);
    expect(stepByBlind(oddCap, e8(0.10), +1, floor, oddCap, CENT)).toBe(oddCap);
    // and stepping down from the cap is back on the grid
    expect(stepByBlind(oddCap, e8(0.10), -1, floor, oddCap, CENT) % CENT).toBe(0);
  });

  it('the dock label reads "All in <exact figure>" and the figure equals the figure sent', () => {
    const label = primaryRaiseLabel(oddCap, oddStack, { isBTC: false, decimals: 2 });
    expect(label).toEqual({ word: 'All in', figure: '1.33456789', text: 'All in 1.33456789', allIn: true });
    // what the button shows, parsed back, is what commitRaise sends
    expect(Math.round(Number(label.figure.replace(/,/g, '')) * ICP)).toBe(clampRaise(oddCap, raiseFloor(oddStack), oddCap));
    const sats = primaryRaiseLabel(12_445, satsStack, { isBTC: true, decimals: 0 });
    expect(sats.text).toBe('All in 12,445');
    expect(Number(sats.figure.replace(/,/g, ''))).toBe(raiseCap(satsStack));
  });

  it('a floor above the cap is not labelled All in', () => {
    // A short stack: 0.25 behind with 0.10 in front, facing 0.30 with a
    // 0.20 minimum raise. The legal floor (0.50) exceeds the stack (0.35);
    // the canister still shows the raise button (chips > to_call), the dock
    // initialises the sizer at the floor and the button is disabled by the
    // problem line. The words must agree with that line, so not "All in".
    const short = { ...facing, currentBet: e8(0.30), minRaise: e8(0.20), myChips: e8(0.25), myCurrentBet: e8(0.10) };
    const floor = raiseFloor(short);
    const cap = raiseCap(short);
    expect(floor).toBe(e8(0.50));
    expect(cap).toBe(e8(0.35));
    expect(floor).toBeGreaterThan(cap);
    expect(isAllInRaise(floor, short)).toBe(false);
    expect(isAllInRaise(cap, short)).toBe(true);
    const label = primaryRaiseLabel(floor, short, { decimals: 2 });
    expect(label).toEqual({ word: 'Raise to', figure: '0.50', text: 'Raise to 0.50', allIn: false });
    expect(raiseProblem(floor, short, (v) => (v / ICP).toFixed(2))).toBe('You have 0.35 behind');
    // and any figure past the cap, not only the floor, keeps the raise word
    expect(primaryRaiseLabel(cap + 1, short, { decimals: 2 }).allIn).toBe(false);
  });

  it('below the cap the label is Raise to / Bet on the display grid', () => {
    expect(primaryRaiseLabel(e8(0.62), facing, { decimals: 2 })).toEqual({ word: 'Raise to', figure: '0.62', text: 'Raise to 0.62', allIn: false });
    expect(primaryRaiseLabel(e8(0.30), open, { decimals: 2 }).text).toBe('Bet 0.30');
    expect(primaryRaiseLabel(e8(12), facing, { decimals: 2 }).text).toBe('All in 12.00');
  });

  it('formatExact / formatAmountInput keep every digit an off-grid figure has and no more', () => {
    expect(exactDecimals(e8(1.23), 2)).toBe(2);
    expect(exactDecimals(123_450_000, 2)).toBe(4);
    expect(exactDecimals(123_456_789, 2)).toBe(8);
    expect(formatExact(e8(1234.5), { decimals: 2 })).toBe('1,234.50');
    expect(formatExact(123_456_789, { decimals: 2 })).toBe('1.23456789');
    expect(formatExact(12_445, { isBTC: true })).toBe('12,445');
    expect(formatAmountInput(123_456_789)).toBe('1.23456789');
    expect(formatAmountInput(e8(1.23))).toBe('1.23');
    expect(formatAmountInput(e8(0.5), { decimals: 4 })).toBe('0.5000');
  });
});

describe('raiseKind / raiseProblem', () => {
  const fmt = (v) => (v / ICP).toFixed(2);
  it('is a bet with nothing in front and a raise otherwise', () => {
    expect(raiseKind(0)).toBe('bet');
    expect(raiseKind(1)).toBe('raise');
  });
  it('explains an illegal size in the words the canister would use', () => {
    expect(raiseProblem(e8(0.40), facing, fmt)).toBe('Minimum raise is 0.50');
    expect(raiseProblem(e8(13), facing, fmt)).toBe('You have 12.00 behind');
    expect(raiseProblem(e8(0.50), facing, fmt)).toBe(null);
  });
});

describe('amount field parse and format', () => {
  it('parses ICP text into e8s and rejects junk', () => {
    expect(parseAmountInput('0.30')).toBe(e8(0.30));
    expect(parseAmountInput('1,000.5')).toBe(e8(1000.5));
    expect(parseAmountInput('abc')).toBe(null);
    expect(parseAmountInput('')).toBe(null);
    expect(parseAmountInput('-1')).toBe(null);
  });
  it('drops a decimal the table does not display, so what is shown is what is sent', () => {
    expect(parseAmountInput('0.123')).toBe(e8(0.12));
    expect(parseAmountInput('0.1234', { decimals: 4 })).toBe(12_340_000);
    expect(parseAmountInput('0.12345', { decimals: 4 })).toBe(12_340_000);
  });
  it('parses sats as whole numbers', () => {
    expect(parseAmountInput('1500', { isBTC: true })).toBe(1500);
  });
  it('formats with the table precision', () => {
    expect(formatAmountInput(e8(0.30), { decimals: 2 })).toBe('0.30');
    // an off-grid figure keeps its digits (the field never rounds what is sent)
    expect(formatAmountInput(12_345, { decimals: 4 })).toBe('0.00012345');
    expect(formatAmountInput(10_000, { decimals: 4 })).toBe('0.0001');
    expect(formatAmountInput(1500, { isBTC: true })).toBe('1500');
  });
});

describe('typedPrecisionDropped / displayUnitLabel', () => {
  it('reports a third decimal dropped on a two-decimal table, and nothing else', () => {
    expect(typedPrecisionDropped('0.075')).toBe(true);
    expect(typedPrecisionDropped('0.07')).toBe(false);
    expect(typedPrecisionDropped('0.070')).toBe(false);
    expect(typedPrecisionDropped('1,000.5')).toBe(false);
    expect(typedPrecisionDropped('abc')).toBe(false);
    expect(typedPrecisionDropped('')).toBe(false);
  });
  it('follows the table decimals and treats a fraction of a sat the same way', () => {
    expect(typedPrecisionDropped('0.0755', { decimals: 3 })).toBe(true);
    expect(typedPrecisionDropped('0.075', { decimals: 3 })).toBe(false);
    expect(typedPrecisionDropped('12.5', { isBTC: true, decimals: 0 })).toBe(true);
    expect(typedPrecisionDropped('12', { isBTC: true, decimals: 0 })).toBe(false);
  });
  it('names the display unit the way the field writes it', () => {
    expect(displayUnitLabel()).toBe('0.01');
    expect(displayUnitLabel({ decimals: 4 })).toBe('0.0001');
    expect(displayUnitLabel({ isBTC: true, decimals: 0 })).toBe('1');
  });
});
