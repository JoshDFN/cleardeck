<script>
  import Card from './Card.svelte';
  /**
   * The board: five community-card slots on a tray.
   *
   * An undealt slot renders EMPTY, never face-down, and invisible: the canister
   * has dealt exactly `cards.length` cards and the felt says so, and the flop
   * lands on bare felt as it does on every reference client. The five slots
   * stay in the layout so the pot never jumps when the flop lands, and
   * `.community-cards` is the direct parent of exactly five `.card`s, which is
   * the contract tools/shots/lib/dom-scrape.mjs reads the board by.
   *
   * THE TRAY is drawn only at the all-in and the showdown, the two moments a
   * revealed pair sits next to the board, and its caption row physically
   * occupies the gap between the board and the hero's pair, naming both sides
   * of it ("Board · 2 to come", "Your hand · King high"). Cards deal 1-2-3 with
   * a 120 ms stagger via their index.
   */
  const {
    cards = [],
    framed = false,
    showdown = false,
    heroHandName = null
  } = $props();

  const toCome = $derived(Math.max(0, 5 - cards.length));
</script>

<div class="board-frame" class:framed class:showdown class:bare={cards.length === 0}>
  <div class="community-cards">
    {#each Array(5) as _, i}
      <Card card={cards[i] ?? null} index={i} />
    {/each}
  </div>
  {#if framed}
    <div class="board-caption">
      <span class="caption-tag">
        Board
        {#if toCome > 0}
          &middot; {toCome} to come
        {/if}
      </span>
      {#if heroHandName}
        <!-- GGPoker's named hand-strength readout. Yours only: a function of
             your own two cards and a board everyone can see. -->
        <span class="caption-hand">Your hand &middot; {heroHandName}</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .board-frame {
    display: flex;
    flex-direction: column;
    align-items: center;
    border-radius: calc(var(--fw) * 0.018);
  }

  .community-cards {
    display: flex;
    gap: calc(var(--fw) * var(--board-gap-r));
    --card-w: calc(var(--fw) * var(--card-board-r));
  }

  .board-frame.framed {
    gap: calc(var(--fw) * 0.010);
    padding: calc(var(--fw) * 0.013) calc(var(--fw) * 0.016) calc(var(--fw) * 0.010);
    background: var(--cd-capsule);
    box-shadow:
      inset 0 0 0 1px var(--cd-line),
      inset 0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.02) var(--cd-felt-shade);
  }

  /* No card dealt yet (an all-in before the flop): the caption stands alone
     on the felt; a tray around five invisible slots is a dark rectangle. */
  .board-frame.framed.bare { background: transparent; box-shadow: none; }

  .board-frame.framed.showdown {
    box-shadow:
      inset 0 0 0 1px var(--cd-money-line),
      inset 0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.02) var(--cd-felt-shade);
  }

  .board-caption {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 0.9em;
    white-space: nowrap;
  }

  .caption-tag {
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-display);
    letter-spacing: var(--cd-tracking-wide);
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .caption-hand {
    font-size: var(--cd-felt-body);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.04em;
    color: var(--cd-accent-hi);
  }

  @media (max-aspect-ratio: 1/1) {
    .caption-hand { font-size: var(--cd-felt-small); }
    .caption-tag { font-size: var(--cd-felt-label); letter-spacing: 0.12em; }
    .board-frame.framed {
      gap: calc(var(--fw) * 0.006);
      padding: calc(var(--fw) * 0.008) calc(var(--fw) * 0.012) calc(var(--fw) * 0.006);
    }
    .board-caption { gap: 0.6em; }
  }
</style>
