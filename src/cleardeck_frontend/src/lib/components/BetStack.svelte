<script>
  /**
   * A seat's committed chips: a stack of discs with edge notches, the amount
   * beside it in the one money colour. Sits on the chip spot (--bx/--by, in
   * felt widths from the seat point) and moves at the 500 ms house tempo.
   *
   * Harness contract: `.bet-amount` inside `.seat` (dom-scrape.mjs reads it as
   * the seat's live bet).
   */
  import { chipBand, chipStackCount } from '$lib/table-visuals.js';

  const {
    amount = 0,
    allIn = false,
    bigBlind = 0,
    fmt = (v) => String(v)
  } = $props();

  const band = $derived(chipBand(amount, bigBlind));
  const discs = $derived(chipStackCount(amount, bigBlind));
</script>

<div class="bet-chip band-{band}" class:all-in={allIn}>
  <span class="chip-stack" aria-hidden="true" style:--discs={discs}>
    {#each Array(discs) as _, k}
      <i class="chip" style:--k={k}></i>
    {/each}
  </span>
  <span class="bet-amount">{fmt(amount)}</span>
</div>

<style>
  .bet-chip {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 8;
    display: flex;
    align-items: center;
    gap: 0.35em;
    white-space: nowrap;
    /* bar 11: 500 ms for anything that moves an object. A NEW bet slides out
       of the seat (the plate's centre) to its chip spot on mount: the hero's
       echo at the click, an opponent's when the poll reports it. */
    transition: transform var(--cd-move) var(--cd-ease-move);
    transform:
      translate(-50%, -50%)
      translate(calc(var(--bx, 0) * var(--fw)), calc(var(--by, 0) * var(--fw)));
    animation: bet-arrive var(--cd-move) var(--cd-ease-move) both;
    --chip-face: var(--cd-chip-white);
    --chip-ink: var(--cd-chip-ink);
  }

  .bet-chip.band-red   { --chip-face: var(--cd-chip-red);   --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-blue  { --chip-face: var(--cd-chip-blue);  --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-green { --chip-face: var(--cd-chip-green); --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-black { --chip-face: var(--cd-chip-black); --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-gold  { --chip-face: var(--cd-chip-gold);  --chip-ink: var(--cd-money-ink); }

  .chip-stack {
    --chip: calc(var(--fw) * 0.026);
    --lift: calc(var(--chip) * 0.2);
    position: relative;
    flex: 0 0 auto;
    width: var(--chip);
    height: calc(var(--chip) + (var(--discs, 1) - 1) * var(--lift));
  }

  /* the stack's shadow on the felt */
  .chip-stack::before {
    content: '';
    position: absolute;
    left: 6%;
    bottom: calc(var(--chip) * -0.1);
    width: 100%;
    height: calc(var(--chip) * 0.4);
    border-radius: 50%;
    background: var(--cd-felt-shade);
    filter: blur(calc(var(--chip) * 0.12));
  }

  .chip {
    position: absolute;
    left: 0;
    bottom: calc(var(--k, 0) * var(--lift));
    width: var(--chip);
    height: var(--chip);
    border-radius: 50%;
    background:
      radial-gradient(circle at 50% 44%, var(--chip-face) 0 56%, transparent 57%),
      repeating-conic-gradient(from 0deg, var(--cd-chip-notch) 0deg 13deg, var(--chip-face) 13deg 45deg);
    box-shadow:
      inset 0 0 0 1px var(--cd-chip-rim),
      0 1px 0 var(--cd-chip-rim);
  }

  .bet-amount {
    font-size: var(--cd-felt-body);
    font-weight: var(--cd-weight-figure);
    padding: 0.12em 0.5em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
    color: var(--cd-money);
    border: 1px solid var(--cd-money-line);
    box-shadow: var(--cd-shadow-chip);
    font-variant-numeric: tabular-nums;
  }

  .bet-chip.all-in .bet-amount { color: var(--cd-ink); border-color: var(--cd-danger-line); }

  @media (max-aspect-ratio: 1/1) {
    .chip-stack { --chip: calc(var(--fw) * 0.05); }
    .bet-amount { font-size: var(--cd-felt-small); }
  }

  @keyframes bet-arrive {
    from {
      opacity: 0.4;
      transform: translate(-50%, -50%) translate(0, 0);
    }
    to {
      opacity: 1;
      transform:
        translate(-50%, -50%)
        translate(calc(var(--bx, 0) * var(--fw)), calc(var(--by, 0) * var(--fw)));
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bet-chip { transition: none; animation: none; }
  }
</style>
