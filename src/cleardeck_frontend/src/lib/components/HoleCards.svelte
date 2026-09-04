<script>
  /**
   * A seat's two hole cards, on an inner ring off the seat point.
   *
   * A FACE-DOWN pair only has to say "this player has cards", so it tucks
   * behind the plate. A revealed pair has to be READ, so it clears the plate
   * entirely on its own plinth. YOUR pair is bigger and paints above your own
   * plate. Geometry comes from the seat's inherited vectors (--nx/--ny/--cy)
   * and the table's ratios (--card-*-r, --off-*-r).
   *
   * Harness contract: `.player-cards .card` (tools/shots/lib/dom-scrape.mjs).
   */
  import Card from './Card.svelte';

  const {
    isHero = false,
    showCards = false,
    live = false,
    winner = false,
    heroCards = null,
    revealed = null
  } = $props();
</script>

<div class="player-cards" class:hero={isHero} class:shown={!isHero && !!revealed} class:winner>
  {#if isHero && heroCards && showCards}
    <Card card={heroCards[0]} index={0} />
    <Card card={heroCards[1]} index={1} />
  {:else if revealed}
    <Card card={revealed[0]} index={0} />
    <Card card={revealed[1]} index={1} />
  {:else if showCards && live}
    <Card faceDown={true} index={0} />
    <Card faceDown={true} index={1} />
  {/if}
</div>

<style>
  .player-cards {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    gap: calc(var(--fw) * 0.006);
    --card-w: calc(var(--fw) * var(--card-opp-r));
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r)),
        calc(var(--cy, -1) * var(--fw) * var(--off-opp-r))
      );
    z-index: 4;                      /* behind the pod, like GGPoker */
  }

  /* A REVEALED PAIR IS A THIRD OBJECT, on its own plinth, half the nudge. */
  .player-cards.shown {
    z-index: 7;
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r) * 0.5),
        calc(var(--cy, -1) * var(--fw) * var(--off-shown-r))
      );
  }

  .player-cards.shown::before {
    content: '';
    position: absolute;
    inset: calc(var(--fw) * -0.008) calc(var(--fw) * -0.010);
    z-index: -1;
    border-radius: calc(var(--fw) * 0.014);
    background: var(--cd-capsule);
    box-shadow: inset 0 0 0 1px var(--cd-line-strong), var(--cd-shadow-chip);
  }

  .player-cards.shown.winner::before {
    box-shadow: inset 0 0 0 1px var(--cd-money-line), 0 0 calc(var(--fw) * 0.02) var(--cd-money-dim);
  }

  /* A LANDSCAPE FLANK SEAT'S REVEALED PAIR clears the board strip along x:
     --shx is the seat's own shift in felt widths ($lib/table-geometry.js
     revealedCellShift), outward where the plate sits close to the board.
     Measured: the 6-max lower flank cell ran under the first board card. */
  @media (min-aspect-ratio: 1/1) {
    :global(.seat.seat-left) .player-cards.shown,
    :global(.seat.seat-right) .player-cards.shown {
      transform:
        translate(-50%, -50%)
        translate(
          calc(var(--shx, 0) * var(--fw)),
          calc(var(--cy, -1) * var(--fw) * var(--off-shown-r))
        );
    }
  }

  /* THE 6-MAX PHONE'S UPPER FLANK SEAT reveals its pair above the plate over
     the OUTER half: the inner half of that edge is the seat's winner award
     (WinnerAward, portraitAwardSpot). */
  @media (max-aspect-ratio: 1/1) {
    :global(.poker-table-wrapper:not(.ring-crowded) .seat.flank.upper) .player-cards.shown {
      transform:
        translate(-50%, -50%)
        translate(
          calc(-1 * var(--rdx, 0) * var(--pod-w) * 0.24),
          calc(var(--cy, -1) * var(--fw) * var(--off-shown-r))
        );
    }
  }

  /* YOUR cards outrank a pod: the hero's pair paints ABOVE its own plate. */
  .player-cards.hero {
    z-index: 7;
    --card-w: calc(var(--fw) * var(--card-hero-r));
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--off-hero-r)),
        calc(var(--ny, 0) * var(--fw) * var(--off-hero-r))
      );
  }

  /* ---- cards revealing: bar 11, a flip is 150 ms, rotateY only ---- */
  .player-cards.shown > :global(.card) {
    animation: card-reveal var(--cd-flip) var(--cd-ease) both;
    transform-origin: 50% 50%;
  }

  .player-cards.shown > :global(.card:nth-child(2)) { animation-delay: 100ms; }

  @keyframes card-reveal {
    0%   { transform: perspective(600px) rotateY(88deg); }
    100% { transform: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .player-cards.shown > :global(.card) { animation: none; }
  }
</style>
