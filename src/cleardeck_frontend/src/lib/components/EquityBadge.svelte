<script>
  /**
   * THE EQUITY BADGE IS A SEAT-LEVEL OBJECT (docs/DEFECTS.md T-22): a sibling
   * of the plate, on the readout spoke (--rdx/--rdy). Solid means computed
   * over hands the ENGINE revealed; dashed means a model against random
   * opponents, and the title says which and how (the method line the felt no
   * longer paints).
   *
   * Where the spoke points is the seat's business (PokerTable's `.spoke-y` and
   * `.spoke-ends` classes); how far past the plate the badge sits is decided
   * here from the plate's own size.
   *
   * Harness contract: `.equity-badge(.modelled)` inside `.seat`.
   */
  const {
    text = '',
    modelled = false,
    note = ''
  } = $props();
</script>

<span class="equity-badge" class:modelled title={note}>{text}</span>

<style>
  .equity-badge {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 9;
    /* Past the plate's end; and past the avatar too when the badge takes the
       left end, which is the edge the avatar breaks. */
    --badge-dx: calc(var(--rdx, 0) * (var(--pod-w) * 0.5 + 2.1em) - max(0px, -1 * var(--rdx, 0) * var(--avatar) * 0.45));
    --badge-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + 0.8em));
    transform: translate(-50%, -50%) translate(var(--badge-dx), var(--badge-dy));
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.1em 0.4em;
    border-radius: 0.35em;
    font-size: var(--cd-felt-small);
    font-weight: var(--cd-weight-display);
    line-height: 1.25;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
    background: var(--cd-capsule);
    color: var(--cd-ink);
    border: 1px solid var(--cd-line-bright);
    white-space: nowrap;
    cursor: help;
    box-shadow: var(--cd-shadow-chip);
  }

  :global(.seat.spoke-y) .equity-badge { --badge-dx: calc(var(--pod-w) * -0.24); }

  /* A LANDSCAPE FLANK SEAT'S BADGE TAKES THE CARDS' SIDE of the inner end (the
     cards ride --cy, perpendicular to the rail). Mid-height past the inner end
     is the dealer puck's spot; the badge only appears at the showdown, when
     the chips that live in that corner during the hand have been swept. */
  @media (min-aspect-ratio: 1/1) {
    :global(.seat.seat-left) .equity-badge,
    :global(.seat.seat-right) .equity-badge {
      --badge-dy: calc(var(--cy, -1) * (var(--pod-h) * 0.5 + 0.9em));
    }
  }

  .equity-badge.modelled {
    border-style: dashed;
    border-color: var(--cd-line-strong);
    color: var(--cd-ink-1);
  }

  @media (max-aspect-ratio: 1/1) {
    .equity-badge { font-size: var(--cd-felt-label); padding: 0.08em 0.32em; }

    /* A vertical spoke carries both readouts on the same edge: the badge takes
       the inner end. */
    :global(.seat.spoke-y) .equity-badge { --badge-dx: calc(var(--pod-w) * -0.3); }

    /* THE PORTRAIT FLANK SEAT'S BADGE hangs off the plate's inner end on the
       BOARD side (above a lower seat's plate, below an upper one's). Measured,
       the other spots are taken: mid-height at the inner end is the dealer
       puck; below the inner end was covered by the hero's lifted pair (36.8%
       of a badge in the showdown scene); beyond the seat's own revealed pair
       touched the widened hero plate's corner. Two facing flank seats at the
       same height would meet at the centre line, so a left seat's badge sits
       0.45em lower than a right seat's (the --rdx term). */
    :global(.seat:not(.spoke-y):not(.spoke-ends)) .equity-badge {
      /* no avatar clearance here: in portrait the avatar sits INSIDE the plate */
      --badge-dx: calc(var(--rdx, 0) * (var(--pod-w) * 0.5 + 2.1em));
      --badge-dy: calc(-1 * sign(var(--sn, 1)) * (var(--pod-h) * 0.5 + 0.7em) + var(--rdx, 0) * 0.45em);
    }

    /* THE 6-MAX PHONE'S LOWER FLANK SEAT: the inner end at MID-HEIGHT. Above
       the plate is the winner line (the badge sat between it and the dealer
       puck, touching both); the puck now stands at the plate's bottom edge
       ($lib/table-geometry.js puckSpot) and the hero's lifted pair starts a
       card below that. */
    :global(.poker-table-wrapper:not(.ring-crowded) .seat.flank.lower) .equity-badge {
      --badge-dy: 0px;
    }

    /* The bottom seat: the plate's LEFT end at mid-height. Below the plate is
       the action dock; above it are the hero's own cards. The avatar is inside
       the plate here, so no extra clearance for it. */
    :global(.seat.spoke-ends) .equity-badge {
      --badge-dx: calc(-1 * (var(--pod-w) * 0.5 + 1.6em));
      --badge-dy: 0px;
    }
  }
</style>
