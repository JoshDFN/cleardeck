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
 *   portrait lower flank seat: past the inner end; at the plate's bottom
 *     edge on the 6-max ring (the badge takes the inner end at mid-height,
 *     the winner line is above), a quarter plate above mid-height on the
 *     nine-seat ring (the hero's pair reaches the plate's bottom edge there).
 *   portrait upper flank seat: below the plate, a little outward: the inner
 *     end meets the pot column on the nine-seat ring and dead below meets the
 *     board's first card on the six-seat ring.
 *   landscape top/bottom seat: past the plate's end OPPOSITE the bet chips
 *     (which took the tangent `away`), above/below the plate's edge.
 *   portrait top/bottom seat: above/below the plate at a third of its width
 *     off centre (toward the side the seat leans to), clear of the hero's
 *     wide pair.
 *
 * @param {{tall:boolean, flank:boolean, cs:number, sn:number, ny:number, crowded?:boolean}} seat
 * @returns {Spot}
 */
export function puckSpot({ tall, flank, cs, sn, ny, crowded = false }) {
  if (flank && tall) {
    if (sn > 0 && !crowded) {
      // 6-max lower flank: past the inner end, level with the plate's BOTTOM
      // edge. Measured at the showdown: above mid-height the puck was
      // squeezed between the equity badge (which now takes the inner end at
      // mid-height) and the winner line; below the bottom edge it touched
      // the hero's lifted pair on the pre-flop scene (the whole table is
      // shorter under the two-row dock), so level with the edge it is.
      return { pkx: -sign(cs) * 0.5, px: -sign(cs) * 0.045, pky: 0.5, py: 0 };
    }
    if (sn > 0) {
      // 9-max lower flank: past the inner end, a quarter plate above
      // mid-height (the hero's pair reaches the plate's bottom edge there)
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

/**
 * Where a landscape FLANK seat's revealed pair sits along x, in felt widths
 * from the seat point, positive INWARD. Measured on the 6-max desktop
 * showdown: the cell nudged half a card-nudge inward ran under the first
 * board card (cell x 231-417 against the 7 of diamonds at 405), and the band
 * between the board's bottom edge and the plate's top edge is thinner than a
 * card, so the clearance has to come along x. The rule: the cell's inner
 * edge stays `gap` clear of the board strip, and its outer edge stays on the
 * felt; when both cannot hold, the felt wins (a revealed pair on the rail
 * lip reads as a mistake, a pair kissing the board's edge does not).
 *
 * Returns null where the rule does not apply (portrait, top/bottom seats):
 * the CSS default (half a card-nudge inward) holds there.
 *
 * @param {{tall:boolean, flank:boolean, cs:number, ringKx:number,
 *          cellHalfW:number, boardHalfW:number, nudge:number,
 *          gap?:number, feltMargin?:number}} seat  lengths in felt widths
 * @returns {number|null}
 */
export function revealedCellShift({
  tall, flank, cs, ringKx, cellHalfW, boardHalfW, nudge, gap = 0.02, feltMargin = 0.006
}) {
  if (tall || !flank) return null;
  const seatX = Math.abs(cs) * ringKx * 0.5;
  const maxInward = seatX - boardHalfW - gap - cellHalfW;
  const maxOutward = Math.max(0, 0.5 - feltMargin - seatX - cellHalfW);
  const shift = Math.min(nudge, maxInward);
  return Number(Math.max(shift, -maxOutward).toFixed(5));
}

/**
 * Which END of a landscape top/bottom plate the equity badge takes, +1 for
 * the right end and -1 for the left: the end whose badge rectangle is clear
 * of every neighbouring plate. Measured on the nine-seat desktop ring at the
 * all-in: the hero's badge at the left end sat on the corner of the next
 * plate (the left end is the crowded one on every plate, because the avatar
 * breaks that edge and the badge has to clear it too). The right end is
 * therefore preferred, the left is the fallback, and when both are blocked
 * the right end it is. Every length is in felt widths; the seat points are
 * screen offsets from the table's centre.
 *
 * @param {{x:number, y:number}} seat
 * @param {{podW:number, podH:number, avatarW:number, badgeW:number, badgeH:number, badgeGap:number}} dims
 * @param {{x:number, y:number}[]} neighbours  the OCCUPIED plates' seat points
 * @returns {1|-1}
 */
export function freeSpokeEnd(seat, dims, neighbours) {
  const badgeRect = (end) => {
    const past = dims.podW / 2 + dims.badgeGap + (end < 0 ? dims.avatarW * 0.45 : 0);
    const cx = seat.x + end * (past + dims.badgeW / 2);
    return {
      x0: cx - dims.badgeW / 2, x1: cx + dims.badgeW / 2,
      y0: seat.y - dims.badgeH / 2, y1: seat.y + dims.badgeH / 2
    };
  };
  const plateRect = (n) => ({
    x0: n.x - dims.podW / 2 - dims.avatarW / 2, x1: n.x + dims.podW / 2,
    y0: n.y - dims.podH / 2, y1: n.y + dims.podH / 2
  });
  const overlaps = (a, b) => a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
  const blocked = (end) => neighbours.some((n) => overlaps(badgeRect(end), plateRect(n)));
  if (!blocked(1)) return 1;
  if (!blocked(-1)) return -1;
  return 1;
}

/**
 * Where a PORTRAIT flank seat's winner award sits: `akx` in plate widths
 * along x (signed, screen space) and `aky` naming the plate edge it stands
 * past (-1 above, +1 below). Measured on the two phone rings:
 *
 *   6-max, lower seat: above the plate, over its OUTER half but no further
 *     out than 0.20 plate widths, so the award's edge stays a puck's width
 *     inside the felt (at 0.26 it stood on the rail lip, x 20 against a felt
 *     edge at 37). The winner line under the board is capped at 0.58 fw for
 *     the same reason: the two must not meet.
 *   6-max, upper seat: above the plate too (below it is the board, 28 px
 *     away), over its INNER half at 0.22; the seat's revealed pair takes the
 *     outer half of the same edge (HoleCards, 0.24 out).
 *   9-max: the plates hang off the felt, so the award stands toward the
 *     board (above a lower plate, below an upper one) a tenth of a plate in
 *     from the seat point: further in meets the board's first card, further
 *     out is the rail.
 *
 * @param {{tall:boolean, flank:boolean, cs:number, sn:number, crowded:boolean}} seat
 * @returns {{akx:number, aky:number}|null}  null where the rule does not apply
 */
export function portraitAwardSpot({ tall, flank, cs, sn, crowded }) {
  if (!tall || !flank) return null;
  const inward = cs < 0 ? 1 : -1;
  if (crowded) return { akx: inward * 0.10, aky: sn > 0 ? -1 : 1 };
  if (sn > 0) return { akx: -inward * 0.20, aky: -1 };
  return { akx: inward * 0.22, aky: -1 };
}
