import { describe, it, expect } from 'vitest';
import { isFlankSeat, isBottomSeat, puckSpot, readoutSpoke } from './table-geometry.js';

// Seats as ringSeats() describes them: cs/sn the seat's angle, nx/ny the
// inward normal, flank/bottom from the two classifiers.
const withClass = (s) => ({ ...s, flank: isFlankSeat(s), bottom: isBottomSeat(s) });

const landscapeLowerLeft = withClass({ tall: false, cs: -0.8, sn: 0.63, nx: 0.94, ny: -0.35 });
const landscapeUpperRight = withClass({ tall: false, cs: 0.8, sn: -0.63, nx: -0.94, ny: 0.35 });
const landscapeBottom = withClass({ tall: false, cs: 0, sn: 1, nx: 0, ny: -1 });
const landscapeTop = withClass({ tall: false, cs: 0, sn: -1, nx: 0, ny: 1 });
// The 6-max portrait lower flank seat: its normal is a 0.70/0.71 coin flip.
const portraitLowerLeft = withClass({ tall: true, cs: -0.9, sn: 0.51, nx: 0.7, ny: -0.71 });
const portraitUpperRight = withClass({ tall: true, cs: 0.9, sn: -0.51, nx: -0.7, ny: 0.71 });
// The 9-max portrait seat next to the hero: steep, but a flank seat.
const portraitSteepLeft = withClass({ tall: true, cs: -0.77, sn: 0.7, nx: 0.52, ny: -0.85 });
const portraitBottom = withClass({ tall: true, cs: 0, sn: 1, nx: 0, ny: -1 });
const portraitTop = withClass({ tall: true, cs: 0, sn: -1, nx: 0, ny: 1 });
// The 9-max portrait top chairs straddle the top at |cs| 0.46.
const portraitTopLeft = withClass({ tall: true, cs: -0.46, sn: -0.9, nx: 0.27, ny: 0.96 });

describe('isFlankSeat / isBottomSeat', () => {
  it('portrait classifies by the seat x, not by the normal', () => {
    expect(portraitLowerLeft.flank).toBe(true);
    expect(portraitSteepLeft.flank).toBe(true);
    expect(portraitTopLeft.flank).toBe(false);
    expect(portraitBottom.flank).toBe(false);
  });
  it('landscape keeps the angle test', () => {
    expect(landscapeLowerLeft.flank).toBe(true);
    expect(landscapeBottom.flank).toBe(false);
  });
  it('only the portrait seat at the bottom centre is the bottom seat', () => {
    expect(portraitBottom.bottom).toBe(true);
    expect(portraitSteepLeft.bottom).toBe(false);
    expect(portraitLowerLeft.bottom).toBe(false);
    expect(landscapeBottom.bottom).toBe(false);
  });
});

describe('puckSpot', () => {
  it('flank seats: past the inner end at mid-height, clear of the plate by more than a puck radius', () => {
    const s = puckSpot(landscapeLowerLeft);
    expect(s.pkx).toBeCloseTo(0.5);      // toward the centre from a left seat
    expect(s.px).toBeGreaterThan(0.014); // the landscape puck radius is 0.014 fw
    expect(s.pky).toBe(0);
    expect(s.py).toBe(0);
    expect(puckSpot(landscapeUpperRight).pkx).toBeCloseTo(-0.5);
  });

  it('portrait lower flank: past the inner end, above mid-height', () => {
    const p = puckSpot(portraitLowerLeft);
    expect(p.pkx).toBeCloseTo(0.5);
    expect(p.px).toBeGreaterThan(0.026); // the portrait puck radius is 0.026 fw
    expect(p.pky).toBeLessThan(0);
    expect(puckSpot(portraitSteepLeft).pkx).toBeCloseTo(0.5);
  });

  it('portrait upper flank: below the plate, a little outward', () => {
    const p = puckSpot(portraitUpperRight);
    expect(p.pky).toBeCloseTo(0.5);
    expect(p.py).toBeGreaterThan(0.026);
    expect(p.pkx).toBeGreaterThan(0);   // outward for a right seat
    expect(Math.abs(p.pkx)).toBeLessThan(0.3);
  });

  it('landscape top/bottom: the end opposite the chips, above or below the plate', () => {
    const b = puckSpot(landscapeBottom);
    expect(b.pkx).toBeCloseTo(-0.5);     // chips break to the right at dead centre
    expect(b.pky).toBeCloseTo(-0.5);     // above the bottom plate
    expect(puckSpot(landscapeTop).pky).toBeCloseTo(0.5);
  });

  it('portrait top/bottom: above or below the plate, off centre', () => {
    const b = puckSpot(portraitBottom);
    expect(b.pky).toBeCloseTo(-0.5);
    expect(b.py).toBeLessThan(0);
    expect(Math.abs(b.pkx)).toBeGreaterThan(0.25);
    const t = puckSpot(portraitTop);
    expect(t.pky).toBeCloseTo(0.5);
    expect(t.py).toBeGreaterThan(0);
  });
});

describe('readoutSpoke', () => {
  it('flank seats read inward, horizontally', () => {
    expect(readoutSpoke(landscapeLowerLeft)).toEqual({ rdx: 1, rdy: 0, spokeEnds: false });
    expect(readoutSpoke(landscapeUpperRight)).toEqual({ rdx: -1, rdy: 0, spokeEnds: false });
    expect(readoutSpoke(portraitLowerLeft)).toEqual({ rdx: 1, rdy: 0, spokeEnds: false });
    expect(readoutSpoke(portraitSteepLeft)).toEqual({ rdx: 1, rdy: 0, spokeEnds: false });
  });

  it('only the portrait bottom seat uses the plate ends (below it is the dock)', () => {
    expect(readoutSpoke(portraitBottom)).toEqual({ rdx: -1, rdy: 0, spokeEnds: true });
    expect(readoutSpoke(portraitSteepLeft).spokeEnds).toBe(false);
  });

  it('the portrait top run reads upward, outward', () => {
    expect(readoutSpoke(portraitTop)).toEqual({ rdx: 0, rdy: -1, spokeEnds: false });
    expect(readoutSpoke(portraitTopLeft)).toEqual({ rdx: 0, rdy: -1, spokeEnds: false });
  });

  it('landscape top/bottom seats take the end opposite the chips', () => {
    expect(readoutSpoke(landscapeBottom)).toEqual({ rdx: -1, rdy: 0, spokeEnds: false });
    expect(readoutSpoke({ ...landscapeBottom, cs: -0.2 })).toEqual({ rdx: 1, rdy: 0, spokeEnds: false });
  });
});
