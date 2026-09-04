/**
 * Per-seat placement rules for the objects that hang off a seat point: which
 * seats count as FLANK seats, where the dealer puck sits, and which side of
 * the plate the readouts (equity badge, and in portrait the winner's award)
 * hang off. Pure functions of the seat's own vectors, so the rules can be
 * unit-tested without a DOM and read without the ring generator around them.
 *
 * Units. A spot is expressed as TWO components per axis so the CSS can resolve
 * it against sizes only it knows: `px/py` in felt widths (`--fw`) and
 * `pkx/pky` in plate widths/heights (`--pod-w` / `--pod-h`). DealerPuck's rule
 * reads `px * fw + pkx * pod-w` for x and `py * fw + pky * pod-h` for y, so a
 * spot phrased as "just past the plate's inner end" stays just past it when
 * the plate changes size (crowded ring, hero plate, portrait).
 */

/** @typedef {{px:number, py:number, pkx:number, pky:number}} Spot */

const sign = (v) => (v >= 0 ? 1 : -1);

/**
 * Is this seat on the rail's left or right run (a FLANK seat), as opposed to
 * its top or bottom run?
 *
 * Landscape keeps the proven test on the seat's angle (|cos| > 0.35).
 * Portrait used to compare the inward normal's components, and for the 6-max
 * lower flank seat that comparison lands at 0.90 against 0.92: a coin flip
 * that decided where the badge, the cards and the chips went. On the tall
 * surface the seat's x alone says which run it sits on.
 *
 * @param {{tall:boolean, cs:number}} seat
 */
export function isFlankSeat({ tall, cs }) {
  return tall ? Math.abs(cs) > 0.6 : Math.abs(cs) > 0.35;
}

/** The one seat at the bottom of a portrait ring: the hero's. */
export function isBottomSeat({ tall, cs, sn }) {
  return tall && sn > 0 && Math.abs(cs) < 0.2;
}

/**
 * Where this seat's dealer puck sits, in felt widths and plate sizes from the
 * seat point. Always on the felt, never on the plate's edge, never on the rail:
 *
 *   landscape flank seat: past the plate's INNER end at mid-height.
 *     Measured: the seat's chips occupy the plate's inner-top corner (the
 *     6-max lower flank seat's chip group ends level with the plate's top
 *     edge), the cards are tucked behind the plate, and the far side of the
 *     plate from the cards is the rail for the lowest seats. The equity
 *     badge, which also used the inner end at mid-height, takes the cards'
 *     side of the inner end instead (EquityBadge.svelte).
 *   portrait lower flank seat: past the inner end, a quarter plate above
 *     mid-height; the hero's pair reaches mid-height on the 6-max ring and
 *     the seat's own chips ride the rail upward.
 *   portrait upper flank seat: below the plate, a little outward: the inner
 *     end meets the pot column on the nine-seat ring and dead below meets the
 *     board's first card on the six-seat ring.
 *   landscape top/bottom seat: past the plate's end OPPOSITE the bet chips
 *     (which took the tangent `away`), above/below the plate's edge.
 *   portrait top/bottom seat: above/below the plate at a third of its width
 *     off centre (toward the side the seat leans to), clear of the hero's
 *     wide pair.
 *
 * @param {{tall:boolean, flank:boolean, cs:number, sn:number, ny:number}} seat
 * @returns {Spot}
 */
export function puckSpot({ tall, flank, cs, sn, ny }) {
  if (flank && tall) {
    if (sn > 0) {
      // lower flank: past the inner end, a quarter plate above mid-height
      // (the hero's pair reaches mid-height on the 6-max ring)
      return { pkx: -sign(cs) * 0.5, px: -sign(cs) * 0.045, pky: -0.25, py: 0 };
    }
    // upper flank: below the plate, a little outward (the inner end meets the
    // pot column on the nine-seat ring; dead below meets the board's first
    // card on the six-seat ring)
    return { pkx: sign(cs) * 0.12, px: 0, pky: 0.5, py: 0.045 };
  }
  if (flank) {
    return { pkx: -sign(cs) * 0.5, px: -sign(cs) * 0.03, pky: 0, py: 0 };
  }
  if (tall) {
    return { pkx: sign(cs) * 0.33, px: 0, pky: sign(ny) * 0.5, py: sign(ny) * 0.045 };
  }
  // landscape top/bottom: the end opposite the chips (which went `away`)
  const away = cs >= 0 ? 1 : -1;
  return { pkx: -away * 0.5, px: -away * 0.025, pky: sign(ny) * 0.5, py: sign(ny) * 0.03 };
}

/**
 * Which side of the plate the readouts hang off.
 *
 *   flank seat: horizontal, INWARD (outward is off a phone's screen).
 *   portrait BOTTOM seat: `spokeEnds`: the badge takes the plate's left end
 *     and the award its right end, both at mid-height. Below the plate is the
 *     action dock; above it are the hero's own cards.
 *   other portrait seat (the top run): vertical, outward.
 *   landscape top/bottom seat: the end opposite the bet chips.
 *
 * @param {{tall:boolean, flank:boolean, bottom:boolean, cs:number, nx:number, ny:number}} seat
 * @returns {{rdx:number, rdy:number, spokeEnds:boolean}}
 */
export function readoutSpoke({ tall, flank, bottom, cs, nx, ny }) {
  if (flank) return { rdx: nx >= 0 ? 1 : -1, rdy: 0, spokeEnds: false };
  if (bottom) return { rdx: -1, rdy: 0, spokeEnds: true };
  if (tall) return { rdx: 0, rdy: ny >= 0 ? -1 : 1, spokeEnds: false };
  return { rdx: cs >= 0 ? -1 : 1, rdy: 0, spokeEnds: false };
}
