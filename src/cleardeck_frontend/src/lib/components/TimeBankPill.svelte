<script>
  /**
   * The time bank, offered: one amber pill that adds the bank to the clock.
   *
   * It is NOT in the primary action row (the audit's fix): on desktop it
   * stands under the turn indicator in the dock's left cell, on the phone it
   * is a tap target on the hero's own pod clock, so the phone row is always
   * the same four cells and never has to squeeze a fifth. It appears only in
   * the last fifteen seconds (lib/action-clock.js offerTimeBank) and only
   * while it is the hero's turn with nothing in flight.
   *
   * `compact` is the pod form: the seconds alone at the felt's label size.
   * Both forms grow their hit area to the touch floor with a pseudo-element
   * (it paints nothing), so the visible pill can stay the size of the row
   * it sits in. tools/shots/probe-time-bank.mjs measures that hit area.
   *
   * Harness: the "+30s" figure is seconds, not chips; token-allowlist.mjs
   * rule `time-bank-button` is scoped to `.time-bank-pill`.
   */
  const {
    secs = 0,
    compact = false,
    onUse = () => {},
  } = $props();
</script>

<button
  type="button"
  class="time-bank-pill"
  class:compact
  onclick={onUse}
  title="Add {secs}s from your time bank"
  aria-label="Add {secs} seconds from your time bank"
>
  <span class="tb-word">Time bank</span>
  <span class="tb-secs">+{secs}s</span>
</button>

<style>
  .time-bank-pill {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-1);
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-pill);
    border: 1px solid var(--cd-warn-line);
    background: var(--cd-warn-dim);
    color: var(--cd-warn);
    font-family: inherit;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
    white-space: nowrap;
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
    transition: background-color var(--cd-fast) var(--cd-ease), transform var(--cd-fast) var(--cd-ease);
  }

  .time-bank-pill:hover { background: var(--cd-warn-line); color: var(--cd-ink); }
  .time-bank-pill:active { transform: scale(0.97); }
  .time-bank-pill:focus-visible { outline: 2px solid var(--cd-accent-hi); outline-offset: 2px; }

  .tb-secs { font-weight: var(--cd-weight-display); }

  /* THE POD FORM (phone): the plate row's height, the felt's label size, the
     hit area grown to the touch floor around it (the pseudo-element is the
     button's own box for hit-testing; it paints nothing). */
  .time-bank-pill.compact {
    min-height: 0;
    padding: 0.1em 0.55em;
    font-size: var(--cd-felt-label);
    line-height: 1.3;
    gap: 0.35em;
  }
  .time-bank-pill.compact .tb-word { display: none; }
  /* Both forms: the hit area is never smaller than the touch floor. */
  .time-bank-pill::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    width: max(100%, var(--cd-touch-min));
    height: max(100%, var(--cd-touch-min));
    transform: translate(-50%, -50%);
  }

  @media (prefers-reduced-motion: reduce) {
    .time-bank-pill { transition: none; }
  }
</style>
