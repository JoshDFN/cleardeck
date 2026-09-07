<script>
  import Card from './Card.svelte';
  /**
   * The board: five community-card slots, on bare felt.
   *
   * An undealt slot renders EMPTY, never face-down, and invisible: the canister
   * has dealt exactly `cards.length` cards and the felt says so, and the flop
   * lands on bare felt as it does on every reference client. No tray: a tray
   * around five slots was a dark slab when only the flop was out. The five
   * slots stay in the layout so the pot never jumps when the turn lands, and
   * `.community-cards` is the direct parent of exactly five `.card`s, which is
   * the contract tools/shots/lib/dom-scrape.mjs reads the board by. Cards deal
   * 1-2-3 with a 120 ms stagger via their index.
   */
  const {
    cards = []
  } = $props();
</script>

<div class="board-frame" class:bare={cards.length === 0}>
  <div class="community-cards">
    {#each Array(5) as _, i}
      <Card card={cards[i] ?? null} index={i} />
    {/each}
  </div>
</div>

<style>
  .board-frame {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .community-cards {
    display: flex;
    gap: calc(var(--fw) * var(--board-gap-r));
    --card-w: calc(var(--fw) * var(--card-board-r));
  }
</style>
