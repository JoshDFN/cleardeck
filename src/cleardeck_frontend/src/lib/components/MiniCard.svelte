<script>
  /**
   * A card as a glyph in a list row: rank and suit on one line in a small white
   * tile, legible at 12 px where a scaled-down Card face is not. A missing card
   * is a back (the record reveals hole cards only at a showdown).
   *
   * Harness: `.mini-card .mc-rank` is the element the token allowlist excuses a
   * rank glyph on (tools/shots/token-allowlist.mjs, card-rank-glyph).
   */
  const { card = null } = $props();

  const SUIT = { Hearts: '♥', Diamonds: '♦', Clubs: '♣', Spades: '♠' };
  const RANK = {
    Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8', Nine: '9',
    Ten: '10', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
  };
  const suit = $derived(card?.suit ? Object.keys(card.suit)[0] : null);
  const rank = $derived(card?.rank ? Object.keys(card.rank)[0] : null);
  const red = $derived(suit === 'Hearts' || suit === 'Diamonds');
</script>

{#if card && rank && suit}
  <span class="mini-card" class:red aria-label="{rank} of {suit}">
    <span class="mc-rank">{RANK[rank]}</span><span class="mc-suit">{SUIT[suit]}</span>
  </span>
{:else}
  <span class="mini-card back" aria-label="face down"></span>
{/if}

<style>
  .mini-card {
    --mc-w: var(--mini-card-w, 22px);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    width: var(--mc-w);
    height: calc(var(--mc-w) * 1.32);
    border-radius: calc(var(--mc-w) * 0.14);
    background: linear-gradient(165deg, var(--cd-card-face), var(--cd-card-face-lo));
    box-shadow: inset 0 0 0 1px var(--cd-card-edge), var(--cd-shadow-chip);
    color: var(--cd-card-black);
    font-size: calc(var(--mc-w) * 0.5);
    line-height: 1;
    flex: 0 0 auto;
  }

  .mini-card.red { color: var(--cd-card-red); }
  .mc-rank { font-family: var(--cd-font-rank); font-weight: 700; letter-spacing: -0.04em; }
  .mc-suit { font-family: var(--cd-font-ui); font-size: 0.82em; }

  .mini-card.back {
    background:
      repeating-linear-gradient(45deg, var(--cd-card-back-gold-dim) 0 1px, transparent 1px 4px),
      linear-gradient(150deg, var(--cd-card-back-hi), var(--cd-card-back) 55%, var(--cd-card-back-lo));
    box-shadow: inset 0 0 0 1px var(--cd-card-edge);
  }
</style>
