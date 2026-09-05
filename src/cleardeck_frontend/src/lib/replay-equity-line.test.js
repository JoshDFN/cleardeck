import { describe, expect, it } from 'vitest';
import { LINE_STREETS, equityLineRows, lineMethodNote, lineStopsFor } from './replay-equity-line.js';

const stop = (board, extra = {}) => ({ board, folded: [], ...extra });

const stops = [stop(0, { kind: 'deal' }), stop(0), stop(0), stop(3, { kind: 'reveal' }), stop(3), stop(4, { kind: 'reveal' }), stop(5, { kind: 'reveal' }), stop(5, { paid: true })];

const engine = (method) => (s) => ({
  method, trials: method === 'exact' ? 1 : 20000,
  bySeat: new Map([[0, s.board === 5 ? 1 : 0.6], [1, s.board === 5 ? 0 : 0.4]]),
});

describe('lineStopsFor', () => {
  it('picks the first stop with each board count, in street order', () => {
    const cols = lineStopsFor(stops);
    expect(cols.map((c) => c.label)).toEqual(LINE_STREETS.map((s) => s.label));
    expect(cols.map((c) => c.stop)).toEqual([stops[0], stops[3], stops[5], stops[6]]);
  });

  it('leaves a street the hand never reached as null', () => {
    const cols = lineStopsFor(stops.slice(0, 5));
    expect(cols.map((c) => c.stop !== null)).toEqual([true, true, false, false]);
  });
});

describe('equityLineRows', () => {
  it('one row per seat, one number per reached street, from the injected engine', () => {
    const line = equityLineRows({ stops, seats: [0, 1], equityAt: engine('exact') });
    expect(line.any).toBe(true);
    expect(line.rows).toHaveLength(2);
    expect(line.rows[0].cells.map((c) => c.share)).toEqual([0.6, 0.6, 0.6, 1]);
    expect(line.rows[1].cells.map((c) => c.share)).toEqual([0.4, 0.4, 0.4, 0]);
    expect(line.rows[0].cells.every((c) => c.state === 'live' && c.method === 'exact')).toBe(true);
    expect(line.streets.map((s) => s.reached)).toEqual([true, true, true, true]);
  });

  it('marks a folded seat from the street it folded on and never invents a figure', () => {
    const withFold = stops.map((s) => (s.board >= 4 ? { ...s, folded: [1] } : s));
    const line = equityLineRows({ stops: withFold, seats: [0, 1], equityAt: (s) => (s.board >= 4 ? null : engine('exact')(s)) });
    expect(line.rows[1].cells.map((c) => c.state)).toEqual(['live', 'live', 'folded', 'folded']);
    expect(line.rows[0].cells.map((c) => c.state)).toEqual(['live', 'live', 'unknown', 'unknown']);
    expect(line.rows[0].cells[2].share).toBeNull();
  });

  it('a street the hand never reached is unreached for every seat', () => {
    const line = equityLineRows({ stops: stops.slice(0, 5), seats: [0, 1], equityAt: engine('exact') });
    expect(line.rows[0].cells.map((c) => c.state)).toEqual(['live', 'live', 'unreached', 'unreached']);
  });

  it('is empty, and says so, when the engine has nothing', () => {
    const line = equityLineRows({ stops, seats: [0, 1], equityAt: () => null });
    expect(line.any).toBe(false);
    expect(line.rows[0].cells.every((c) => c.state === 'unknown')).toBe(true);
  });

  it('does not mutate its inputs', () => {
    const frozen = Object.freeze(stops.map((s) => Object.freeze({ ...s })));
    expect(() => equityLineRows({ stops: frozen, seats: Object.freeze([0, 1]), equityAt: engine('exact') })).not.toThrow();
  });
});

describe('lineMethodNote', () => {
  it('names one method for the whole line', () => {
    const { streets } = equityLineRows({ stops, seats: [0, 1], equityAt: engine('exact') });
    expect(lineMethodNote(streets)).toBe('Pre-flop to River exact.');
  });

  it('splits a Monte Carlo pre-flop from the exact streets after it', () => {
    const { streets } = equityLineRows({ stops, seats: [0, 1], equityAt: (s) => (s.board === 0 ? engine('monte-carlo')(s) : engine('exact')(s)) });
    expect(lineMethodNote(streets)).toBe('Pre-flop Monte Carlo, 20,000 trials; Flop to River exact.');
  });

  it('is empty with nothing shown', () => {
    expect(lineMethodNote([])).toBe('');
    expect(lineMethodNote([{ label: 'Flop', reached: true, method: null, trials: null }])).toBe('');
  });
});
