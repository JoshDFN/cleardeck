<script>
  /**
   * The dealer puck: a white disc on the felt in front of the plate. Its spot
   * is the seat's `--px/--py` (felt widths) plus `--pkx/--pky` (plate widths
   * and heights), computed per seat by $lib/table-geometry.js puckSpot(), so
   * it sits just past a plate's edge whatever size that plate is. It travels
   * 500 ms from the previous dealer's seat when the button moves (bar 11).
   *
   * Harness contract: `.position-badge.dealer` inside `.seat`.
   */
  const {
    puckFrom = null   // {dx, dy} in ring units the puck travels FROM, or null
  } = $props();
</script>

<span
  class="position-badge dealer"
  class:travel={!!puckFrom}
  style:--pdx={puckFrom?.dx ?? 0}
  style:--pdy={puckFrom?.dy ?? 0}
  title="Dealer"
>D</span>

<style>
  .position-badge.dealer {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 8;
    --puck: calc(var(--fw) * 0.028);
    --puck-dx: calc(var(--px, 0) * var(--fw) + var(--pkx, 0) * var(--pod-w));
    --puck-dy: calc(var(--py, 0) * var(--fw) + var(--pky, 0) * var(--pod-h));
    width: var(--puck);
    height: var(--puck);
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: calc(var(--puck) * 0.55);
    font-weight: var(--cd-weight-display);
    color: var(--cd-ink-on-light);
    background: radial-gradient(circle at 50% 38%, var(--cd-chip-white), var(--cd-ink-1) 75%);
    box-shadow: var(--cd-shadow-chip), inset 0 0 0 1px var(--cd-chip-rim);
    transform: translate(-50%, -50%) translate(var(--puck-dx), var(--puck-dy));
  }

  .position-badge.dealer.travel {
    animation: puck-travel var(--cd-move) var(--cd-ease-move) both;
  }

  @keyframes puck-travel {
    from {
      transform:
        translate(-50%, -50%)
        translate(var(--puck-dx), var(--puck-dy))
        translate(calc(var(--pdx, 0) * var(--rx)), calc(var(--pdy, 0) * var(--ry)));
    }
    to {
      transform: translate(-50%, -50%) translate(var(--puck-dx), var(--puck-dy));
    }
  }

  @media (max-aspect-ratio: 1/1) {
    .position-badge.dealer { --puck: calc(var(--fw) * 0.052); }
  }

  @media (prefers-reduced-motion: reduce) {
    .position-badge.dealer.travel { animation: none; }
  }
</style>
