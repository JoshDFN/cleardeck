<script>
  /**
   * One playing card.
   *
   * SIZE IS INHERITED, NOT HARDCODED. Every dimension derives from the
   * `--card-w` custom property (default 60px, which is what HandHistory relies
   * on), so the poker table can scale its board / hero / opponent cards as one
   * rigid object from a single felt-width variable.
   *
   * Proportions follow docs/DESIGN-BAR.md §6:
   *   bar 6  aspect 0.80-0.82, border-radius ~7% of width  -> 0.81 / 7%
   *   bar 9  rank in a display face at ~2x base UI size, red #DB3131,
   *          black #2C2C2C (never pure black)
   *   bar 10 exactly two typefaces on the table: Inter for UI, one display
   *          serif for ranks. A SYSTEM serif stack is used on purpose so the
   *          table never waits on (or fails to fetch) a webfont.
   */
  const { card, faceDown = false, small = false } = $props();

  const suitSymbols = {
    Hearts: '♥',
    Diamonds: '♦',
    Clubs: '♣',
    Spades: '♠'
  };

  // docs/DESIGN-BAR.md bar 9: measured PokerNow rank colours.
  const suitColors = {
    Hearts: '#DB3131',
    Diamonds: '#DB3131',
    Clubs: '#2C2C2C',
    Spades: '#2C2C2C'
  };

  const rankDisplay = {
    Two: '2', Three: '3', Four: '4', Five: '5', Six: '6',
    Seven: '7', Eight: '8', Nine: '9', Ten: '10',
    Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A'
  };

  function getSuit(c) {
    if (!c?.suit) return null;
    return Object.keys(c.suit)[0];
  }

  function getRank(c) {
    if (!c?.rank) return null;
    return Object.keys(c.rank)[0];
  }

  const suit = $derived(getSuit(card));
  const rank = $derived(getRank(card));
</script>

<div
  class="card"
  class:face-down={faceDown}
  class:empty={!faceDown && !card}
  class:small
  style:--suit-color={suit ? suitColors[suit] : '#2C2C2C'}
>
  {#if faceDown}
    <div class="card-back">
      <div class="back-frame">
        <span class="back-mark">&#9824;</span>
      </div>
    </div>
  {:else if card}
    <div class="card-front">
      <span class="rank">{rankDisplay[rank]}</span>
      <span class="pip">{suitSymbols[suit]}</span>
    </div>
  {:else}
    <div class="card-empty"></div>
  {/if}
</div>

<style>
  .card {
    /* --card-w is the ONLY size input. Everything else is a ratio of it. */
    width: var(--card-w, 60px);
    height: calc(var(--card-w, 60px) / 0.81);   /* bar 6: aspect 0.81 */
    border-radius: calc(var(--card-w, 60px) * 0.07);
    /* NOT white. `.card-front` and `.card-back` each paint their own face over
       the full box, so the only thing a background here can do is make an
       UNDEALT slot render as a solid white rectangle -- which is exactly what
       it used to do: five blank white cards sat in the middle of the felt on
       every waiting table and every pre-flop board. */
    background: transparent;
    position: relative;
    overflow: hidden;
    flex: 0 0 auto;
    box-shadow:
      0 calc(var(--card-w, 60px) * 0.02) calc(var(--card-w, 60px) * 0.11) rgba(0, 0, 0, 0.35),
      inset 0 0 0 1px rgba(0, 0, 0, 0.10);
    /* bar 11: 500 ms is the house tempo for anything that MOVES an object. */
    animation: card-deal 0.5s cubic-bezier(0.22, 0.61, 0.36, 1) both;
  }

  /* An undealt slot is a RESERVED SPACE, not a card: it holds the board's
     footprint so the flop does not shove the pot sideways when it lands, and
     it carries none of a card's weight -- no white, no lift, no deal. */
  .card.empty {
    box-shadow: none;
    animation: none;
  }

  /* ---- face up ------------------------------------------------------- */

  .card-front {
    position: absolute;
    inset: 0;
    color: var(--suit-color);
    background: linear-gradient(160deg, #ffffff 0%, #f4f5f7 100%);
    /* bar 10: the display face, used for NOTHING but card ranks. */
    font-family: 'Playfair Display', Georgia, 'Times New Roman', serif;
    line-height: 0.9;
  }

  .rank {
    position: absolute;
    top: 4%;
    left: 7%;
    font-size: calc(var(--card-w, 60px) * 0.44);
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: lining-nums tabular-nums;
  }

  .pip {
    position: absolute;
    right: 6%;
    bottom: 2%;
    font-size: calc(var(--card-w, 60px) * 0.46);
    line-height: 1;
    /* Suit glyphs come from the UI font: the serif face carries the rank only. */
    font-family: 'Inter', -apple-system, 'Segoe UI', Roboto, sans-serif;
  }

  /* ---- face down ----------------------------------------------------- */

  .card-back {
    position: absolute;
    inset: 0;
    padding: 6%;
    background:
      repeating-linear-gradient(45deg,
        rgba(212, 175, 55, 0.16) 0 2px, transparent 2px 6px),
      linear-gradient(150deg, #1e3a5f 0%, #0d1f33 100%);
  }

  .back-frame {
    width: 100%;
    height: 100%;
    border: 1px solid rgba(212, 175, 55, 0.55);
    border-radius: calc(var(--card-w, 60px) * 0.045);
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(ellipse at 50% 40%, rgba(212, 175, 55, 0.13), transparent 70%);
  }

  .back-mark {
    color: rgba(212, 175, 55, 0.85);
    font-size: calc(var(--card-w, 60px) * 0.34);
    line-height: 1;
  }

  .card-empty {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.16);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.045);
    border-radius: inherit;
  }

  .face-down {
    background: #0d1f33;
  }

  /* Legacy fixed-size variant kept for HandHistory's compact rows. */
  .small {
    --card-w: 44px;
  }

  @keyframes card-deal {
    from { opacity: 0; transform: translateY(-14%) scale(0.94); }
    to   { opacity: 1; transform: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .card { animation: none; }
  }
</style>
