<script>
  /**
   * One playing card.
   *
   * SIZE IS INHERITED, NOT HARDCODED. Every dimension derives from the
   * `--card-w` custom property (default 60px, which is what HandHistory relies
   * on), so the poker table can scale its board / hero / opponent cards as one
   * rigid object from a single felt-width variable.
   *
   * Proportions follow docs/DESIGN-BAR.md section 6:
   *   bar 6  aspect 0.80-0.82, border-radius ~7% of width  -> 0.81 / 7%
   *   bar 9  rank in a display face at ~2x base UI size, red --cd-card-red,
   *          black --cd-card-black (never pure black)
   *   bar 10 exactly two typefaces on the table: Inter for UI, Playfair
   *          Display (self-hosted, see index.scss) for ranks and nothing else.
   *
   * THE FACE. A corner index (rank over suit) at the top left, mirrored and
   * rotated at the bottom right, and one large centre pip: the layout every
   * real deck uses, readable at 40 px and at 110 px. `.rank` and `.pip` stay the
   * elements the screenshot harness reads the card by (dom-scrape.mjs readCard).
   *
   * `index` is the card's position in its group: community cards deal 1-2-3 with
   * a 120 ms stagger and the turn and river alone, driven by `--i`.
   */
  const { card, faceDown = false, small = false, index = 0 } = $props();

  const suitSymbols = {
    Hearts: '♥',
    Diamonds: '♦',
    Clubs: '♣',
    Spades: '♠'
  };

  const suitTone = {
    Hearts: 'red',
    Diamonds: 'red',
    Clubs: 'black',
    Spades: 'black'
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
  const tone = $derived(suit ? suitTone[suit] : 'black');
</script>

<div
  class="card"
  class:face-down={faceDown}
  class:empty={!faceDown && !card}
  class:small
  class:red={tone === 'red'}
  style:--i={index}
>
  {#if faceDown}
    <div class="card-back">
      <div class="back-frame">
        <span class="back-mark">&#9824;</span>
      </div>
    </div>
  {:else if card}
    <div class="card-front">
      <span class="index top">
        <span class="rank">{rankDisplay[rank]}</span>
        <span class="pip">{suitSymbols[suit]}</span>
      </span>
      <span class="centre-pip" aria-hidden="true">{suitSymbols[suit]}</span>
      <span class="index bottom" aria-hidden="true">
        <span class="rank-mirror">{rankDisplay[rank]}</span>
        <span class="pip-mirror">{suitSymbols[suit]}</span>
      </span>
      <span class="sheen" aria-hidden="true"></span>
    </div>
  {:else}
    <div class="card-empty"></div>
  {/if}
</div>

<style>
  .card {
    /* --card-w is the ONLY size input. Everything else is a ratio of it. */
    --w: var(--card-w, 60px);
    --suit-color: var(--cd-card-black);
    width: var(--w);
    height: calc(var(--w) / 0.81);          /* bar 6: aspect 0.81 */
    border-radius: calc(var(--w) * 0.07);
    /* NOT white: each face paints itself over the full box, so a background
       here could only make an UNDEALT slot render as a white rectangle. */
    background: transparent;
    position: relative;
    overflow: hidden;
    flex: 0 0 auto;
    box-shadow: var(--cd-shadow-card);
    /* bar 11: 500 ms is the house tempo for anything that MOVES an object.
       Cards in a group land in sequence, 120 ms apart, so a flop reads as three
       cards dealt and not one slab. */
    animation: card-deal var(--cd-move) var(--cd-ease-land) both;
    animation-delay: calc(var(--i, 0) * 120ms);
  }

  .card.red { --suit-color: var(--cd-card-red); }

  /* An undealt slot is a RESERVED SPACE, not a card: it holds the board's
     footprint so the flop does not shove the pot sideways when it lands. It is
     invisible (the flop lands on empty felt, as on every reference client) and
     it stays in the DOM, which is what the harness counts "board 5 slots" by. */
  .card.empty {
    box-shadow: none;
    animation: none;
    visibility: hidden;
  }

  /* ---- face up ------------------------------------------------------- */

  .card-front {
    position: absolute;
    inset: 0;
    color: var(--suit-color);
    background: linear-gradient(165deg, var(--cd-card-face) 0%, var(--cd-card-face-lo) 100%);
    box-shadow: inset 0 0 0 1px var(--cd-card-edge);
    border-radius: inherit;
    line-height: 1;
  }

  .index {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 0.9;
  }

  .index.top {
    top: calc(var(--w) * 0.06);
    left: calc(var(--w) * 0.07);
  }

  .index.bottom {
    right: calc(var(--w) * 0.07);
    bottom: calc(var(--w) * 0.06);
    transform: rotate(180deg);
  }

  .rank, .rank-mirror {
    /* bar 10: the display face, used for NOTHING but card ranks. */
    font-family: var(--cd-font-rank);
    font-size: calc(var(--w) * 0.32);
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: lining-nums tabular-nums;
  }

  .pip, .pip-mirror {
    font-size: calc(var(--w) * 0.2);
    margin-top: calc(var(--w) * 0.01);
    /* Suit glyphs come from the UI font: the serif face carries the rank only. */
    font-family: var(--cd-font-ui);
  }

  .centre-pip {
    position: absolute;
    left: 50%;
    top: 58%;
    transform: translate(-50%, -50%);
    font-size: calc(var(--w) * 0.5);
    font-family: var(--cd-font-ui);
    line-height: 1;
  }

  /* A soft specular across the top third: the card is a glossy object under an
     overhead light, not a flat white rectangle. */
  .sheen {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(170deg, var(--cd-card-sheen) 0%, transparent 38%);
    opacity: 0.55;
    pointer-events: none;
  }

  /* ---- face down ----------------------------------------------------- */

  .card-back {
    position: absolute;
    inset: 0;
    padding: 7%;
    border-radius: inherit;
    background:
      repeating-linear-gradient(45deg, var(--cd-card-back-gold-dim) 0 2px, transparent 2px 7px),
      repeating-linear-gradient(-45deg, var(--cd-card-back-gold-dim) 0 2px, transparent 2px 7px),
      linear-gradient(150deg, var(--cd-card-back-hi) 0%, var(--cd-card-back) 55%, var(--cd-card-back-lo) 100%);
    box-shadow: inset 0 0 0 1px var(--cd-card-edge);
  }

  .back-frame {
    width: 100%;
    height: 100%;
    border: 1px solid var(--cd-card-back-gold);
    border-radius: calc(var(--w) * 0.045);
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(ellipse at 50% 40%, var(--cd-card-back-gold-dim), transparent 70%);
  }

  .back-mark {
    color: var(--cd-card-back-gold);
    font-size: calc(var(--w) * 0.34);
    line-height: 1;
  }

  .card-empty {
    position: absolute;
    inset: 0;
    background: var(--cd-card-slot);
    border-radius: inherit;
  }

  .face-down {
    background: var(--cd-card-back-lo);
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
