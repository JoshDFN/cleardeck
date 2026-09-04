<script>
  /**
   * THE DELTA CHIP on the award spot (docs/DEFECTS.md T-23): what changed, at
   * the stack it changed, with the winning hand named beneath it. Landscape
   * uses the award spot (--ax/--ay); portrait rides the readout spoke with the
   * badge, or the plate's right END for the bottom seat (`spokeEnds`), where
   * below the plate is the action dock.
   *
   * Harness contract: `.stack-delta` and `.hand-tag` inside `.seat`.
   */
  const {
    awardAmount = '',     // formatted "+X" text
    handTag = '',
    onSpoke = false,
    spokeY = false,
    spokeEnds = false
  } = $props();
</script>

<div class="winner-award" class:on-spoke={onSpoke} class:spoke-y={spokeY} class:spoke-ends={spokeEnds}>
  <span class="stack-delta cd-money">{awardAmount}</span>
  {#if handTag}<span class="hand-tag">{handTag}</span>{/if}
</div>

<style>
  .winner-award {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.18em;
    white-space: nowrap;
    --award-dx: calc(var(--ax, 0) * var(--fw));
    --award-dy: calc(var(--ay, 0) * var(--fw));
    transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy));
    /* The award LANDS, and it lands LAST: 760 ms in, where the pot ghost
       finishes its flight to this pod. */
    animation: award-land 0.42s var(--cd-ease-spring) 0.76s both;
  }

  .stack-delta {
    padding: 0.12em 0.55em;
    border-radius: var(--cd-radius-pill);
    background: linear-gradient(180deg, var(--cd-money-hi), var(--cd-money));
    color: var(--cd-money-ink);
    font-size: 0.86em;
    font-weight: var(--cd-weight-display);
    box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-money-glow), var(--cd-shadow-chip);
  }

  .hand-tag {
    padding: 0.1em 0.5em;
    border-radius: 0.35em;
    background: var(--cd-capsule);
    border: 1px solid var(--cd-money-line);
    color: var(--cd-money);
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-display);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  @keyframes award-land {
    from { opacity: 0; transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(0.6); }
    to   { opacity: 1; transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(1); }
  }

  /* =========================================================================
     PORTRAIT: no room on the chip vector (T-23), so the award rides the
     readout spoke with the badge.
     ========================================================================= */
  @media (max-aspect-ratio: 1/1) {
    .winner-award { gap: 0.1em; }

    /* Flank seat (horizontal spoke): the seat's own award spot, in plate
       widths (--akx, signed) past the plate edge --aky names (-1 above, +1
       below), from $lib/table-geometry.js portraitAwardSpot: over the outer
       half above a 6-max lower plate (no further out than the felt's edge),
       over the inner half above a 6-max upper plate (its revealed pair takes
       the outer half), toward the board a tenth of a plate in on the
       nine-seat ring. */
    .winner-award.on-spoke {
      --award-dx: calc(var(--akx, 0) * var(--pod-w));
      --award-dy: calc(var(--aky, -1) * (var(--pod-h) * 0.5 + 1.2em));
    }

    /* Top seat (vertical spoke): the badge at one end of the far edge, the
       award at the other; both were measured touching at the old 0.24. */
    .winner-award.on-spoke.spoke-y {
      --award-dx: calc(var(--pod-w) * 0.34);
      --award-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + var(--fw) * 0.05));
    }

    /* Bottom seat: the plate's RIGHT end at mid-height. Below the plate is the
       dock; above it are the hero's own cards; the badge has the left end. */
    .winner-award.on-spoke.spoke-ends {
      --award-dx: calc(var(--pod-w) * 0.5 + 1.2em);
      --award-dy: 0px;
    }

    .stack-delta { font-size: 0.66em; padding: 0.08em 0.4em; }
    .hand-tag { font-size: var(--cd-felt-label); padding: 0.06em 0.35em; }

    /* A flank or bottom seat's award on a phone is the delta chip alone: the
       winner line names the hand a few pixels away, and a long name here
       ("THREE OF A KIND", 110 px at the floor) ran under the winner line and
       off the felt's edge (measured on the 6-max showdown). The top run's
       vertical spoke keeps its tag; there is room there. */
    .winner-award.on-spoke:not(.spoke-y) .hand-tag { display: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .winner-award { animation: none; }
  }
</style>
