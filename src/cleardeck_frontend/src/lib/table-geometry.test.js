import { describe, it, expect } from 'vitest';
import { isFlankSeat, isBottomSeat, puckSpot, readoutSpoke, revealedCellShift, freeSpokeEnd, portraitAwardSpot } from './table-geometry.js';

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

  it('portrait lower flank on the nine-seat ring: past the inner end, above mid-height', () => {
    const p = puckSpot({ ...portraitLowerLeft, crowded: true });
    expect(p.pkx).toBeCloseTo(0.5);
    expect(p.px).toBeGreaterThan(0.026); // the portrait puck radius is 0.026 fw
    expect(p.pky).toBeLessThan(0);
    expect(puckSpot({ ...portraitSteepLeft, crowded: true }).pkx).toBeCloseTo(0.5);
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

describe('puckSpot on the two portrait rings', () => {
  it('6-max lower flank: past the inner end, level with the plate bottom edge', () => {
    const p = puckSpot({ ...portraitLowerLeft, crowded: false });
    expect(p.pkx).toBeCloseTo(0.5);
    expect(p.pky).toBeCloseTo(0.5);
    expect(p.py).toBe(0);                 // the hero's pair starts a puck's width below
    expect(p.px).toBeGreaterThan(0.026);  // clear of the plate's end by a puck radius
  });
  it('9-max lower flank keeps the spot above mid-height', () => {
    const p = puckSpot({ ...portraitSteepLeft, crowded: true });
    expect(p.pky).toBeLessThan(0);
    expect(p.pkx).toBeCloseTo(0.5);
  });
});

describe('revealedCellShift (landscape flank seats against the board strip)', () => {
  // 6-max desktop: cards 0.076 fw, plinth 0.010, gap 0.006; board 5 x 0.112 + 4 x 0.009
  const sixMax = { ringKx: 1.02, cellHalfW: 0.076 + 0.003 + 0.010, boardHalfW: 0.298, nudge: 0.0175 };
  const nineMax = { ringKx: 1.02, cellHalfW: 0.065 + 0.003 + 0.010, boardHalfW: 0.298, nudge: 0.0175 };

  it('the 6-max lower flank seat moves its revealed pair OUTWARD so the cell clears the board', () => {
    const shift = revealedCellShift({ ...landscapeLowerLeft, cs: -0.77, ...sixMax });
    expect(shift).toBeLessThan(0);
    // distances from the centre line: inward shift brings the cell closer
    const seatX = 0.77 * 1.02 * 0.5;
    // On this ring the felt edge binds first (the plate is 0.39 fw out on a
    // 0.5 fw half-felt), so the cell stands ON the felt's margin and clears
    // the board's edge by what is left, which is still clear of the cards.
    const innerEdge = seatX - shift - sixMax.cellHalfW;
    expect(innerEdge).toBeGreaterThanOrEqual(sixMax.boardHalfW + 0.015);
    const outerEdge = seatX - shift + sixMax.cellHalfW;
    expect(outerEdge).toBeCloseTo(0.5 - 0.006, 5);
  });

  it('the 9-max flank seat next to the hero is clamped by the felt edge, never past it', () => {
    const shift = revealedCellShift({ tall: false, flank: true, cs: -0.52, ...nineMax });
    const seatX = 0.52 * 1.02 * 0.5;
    expect(shift).toBeLessThan(0);
    expect(seatX - shift + nineMax.cellHalfW).toBeLessThanOrEqual(0.5 - 0.006 + 1e-9);
  });

  it('a heads-up flank seat far from the board keeps the default inward nudge', () => {
    const shift = revealedCellShift({ tall: false, flank: true, cs: -1, ...sixMax, cellHalfW: 0.097 });
    expect(shift).toBeCloseTo(0.0175);
  });

  it('does not apply to top/bottom seats or to portrait', () => {
    expect(revealedCellShift({ ...landscapeBottom, ...sixMax })).toBeNull();
    expect(revealedCellShift({ ...portraitLowerLeft, ...sixMax })).toBeNull();
  });
});

describe('freeSpokeEnd (a landscape top/bottom plate against its neighbours)', () => {
  // 9-max desktop ratios: plate 0.202 x 0.074, avatar 0.056, badge ~2.8 ui wide at ui 0.018
  const dims = { podW: 0.202, podH: 0.074, avatarW: 0.056, badgeW: 0.05, badgeH: 0.02, badgeGap: 0.038 };
  const hero = { x: 0, y: 0.238 };
  // the seat next to the hero on the nine-seat ring (measured cs -0.52, sn 0.85)
  const leftNeighbour = { x: -0.52 * 0.51, y: 0.85 * 0.238 };
  const rightNeighbour = { x: 0.52 * 0.51, y: 0.85 * 0.238 };

  it('takes the right end when both neighbours are seated (the left end meets the avatar clearance)', () => {
    expect(freeSpokeEnd(hero, dims, [leftNeighbour, rightNeighbour])).toBe(1);
  });
  it('takes the right end when nobody is next to it', () => {
    expect(freeSpokeEnd(hero, dims, [])).toBe(1);
  });
  it('falls back to the left end when only the right neighbour blocks it', () => {
    const tight = { ...dims, badgeGap: 0.06, badgeW: 0.08 };
    expect(freeSpokeEnd(hero, tight, [rightNeighbour])).toBe(-1);
    expect(freeSpokeEnd(hero, tight, [leftNeighbour, rightNeighbour])).toBe(1);
  });
});

describe('portraitAwardSpot', () => {
  it('6-max lower flank: above the plate, over its outer half, no further out than 0.20 widths', () => {
    const a = portraitAwardSpot({ ...portraitLowerLeft, crowded: false });
    expect(a.aky).toBe(-1);
    expect(a.akx).toBeCloseTo(-0.20);   // outward for a left seat
    expect(portraitAwardSpot({ tall: true, flank: true, cs: 0.9, sn: 0.51, crowded: false }).akx).toBeCloseTo(0.20);
  });
  it('6-max upper flank: above the plate too, over its inner half', () => {
    const a = portraitAwardSpot({ ...portraitUpperRight, crowded: false });
    expect(a.aky).toBe(-1);
    expect(a.akx).toBeCloseTo(-0.22);   // inward for a right seat
  });
  it('9-max: toward the board, a tenth of a plate in', () => {
    expect(portraitAwardSpot({ ...portraitSteepLeft, crowded: true })).toEqual({ akx: 0.1, aky: -1 });
    expect(portraitAwardSpot({ tall: true, flank: true, cs: 0.9, sn: -0.3, crowded: true })).toEqual({ akx: -0.1, aky: 1 });
  });
  it('does not apply off the flanks or in landscape', () => {
    expect(portraitAwardSpot({ ...portraitBottom, crowded: false })).toBeNull();
    expect(portraitAwardSpot({ ...landscapeLowerLeft, crowded: false })).toBeNull();
  });
});
