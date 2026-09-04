<script>
  /**
   * The centre of the table: the pot module, or the winner line once the pot
   * has gone.
   *
   * ONE gold figure with a POT label and the street, as every reference client
   * and every broadcast graphic draws it. The decomposition (collected +
   * betting), the side pots and the equity method are footnotes UNDER it, one
   * type step down, shown only when they say something the headline does not.
   *
   * THE HARNESS CONTRACT. `.pot-display > .main-pot > .pot-amount` is the
   * figure asserted against get_pot(); `.pot-breakdown` must carry two numbers
   * that sum to it; `.side-pot > .side-pot-label + .side-pot-amount` one per
   * layer; `.phase-indicator` the street; `.equity-method` the method line with
   * its `title`; `.winner-display > .winner-text | .winner-hand-rank |
   * .split-info`. tools/shots/lib/dom-scrape.mjs reads exactly these.
   *
   * The parent positions this component's root inside `.board-cluster`; the
   * rules for that anchor live here because they are the module's own shape.
   */
  const {
    totalPot = 0,
    liveBets = 0,
    collectedPot = 0,
    sidePots = [],
    allInMoment = false,
    allInCount = 0,
    streetLabel = '',
    equityMethodLabel = null,
    equityNote = '',
    winners = [],
    myWinInfo = null,
    isHandComplete = false,
    currencySymbol = 'ICP',
    fmt = (v) => String(v),
    seatLabel = (i) => `Seat ${Number(i) + 1}`,
    handRankWords = () => ''
  } = $props();

  const showWinner = $derived(isHandComplete && winners.length > 0);
</script>

{#snippet equityMethodLine()}
  {#if equityMethodLabel}
    <!-- THE METHOD TRAVELS WITH THE FIGURE (docs/DEFECTS.md T-27): the same
         element under whichever readout is on screen. -->
    <div class="equity-method" title={equityNote}>{equityMethodLabel}</div>
  {/if}
{/snippet}

{#if showWinner}
  <div class="winner-display" class:you-won={!!myWinInfo}>
    <span class="winner-line">
      {#if myWinInfo}
        <span class="winner-text">You won {fmt(Number(myWinInfo.amount))} {currencySymbol}</span>
        {#if handRankWords(myWinInfo.hand_rank)}
          <span class="winner-hand-rank">{handRankWords(myWinInfo.hand_rank)}</span>
        {/if}
      {:else}
        <span class="winner-text">
          {seatLabel(winners[0].seat)} wins {fmt(Number(winners[0].amount))} {currencySymbol}
        </span>
        {#if handRankWords(winners[0].hand_rank)}
          <span class="winner-hand-rank">{handRankWords(winners[0].hand_rank)}</span>
        {/if}
      {/if}
      {#if winners.length > 1}
        <span class="split-info">Split pot &middot; {winners.length} winners</span>
      {/if}
      <span class="phase-indicator">{streetLabel}</span>
    </span>
    {@render equityMethodLine()}
  </div>
{:else}
  <div class="pot-display" class:waiting={totalPot <= 0}>
    <div class="main-pot" class:has-chips={totalPot > 0} class:at-risk={allInMoment}>
      <span class="pot-meta">
        <span class="pot-label">
          {#if allInMoment}
            All in &middot; {allInCount} at risk
          {:else}
            Pot
          {/if}
        </span>
        <span class="phase-indicator">{streetLabel}</span>
      </span>
      <span class="pot-amount cd-money">{totalPot > 0 ? fmt(totalPot) : '--'}</span>
    </div>
    {#if sidePots.length > 0 || liveBets > 0}
      <div class="pot-foot">
        {#if sidePots.length > 0}
          <!-- index 0 is the MAIN pot (docs/DEFECTS.md T-12) -->
          <div class="side-pots">
            {#each sidePots as sidePot, i}
              <div class="side-pot">
                <span class="side-pot-label">{i === 0 ? 'Main' : `Side ${i}`}</span>
                <span class="side-pot-amount cd-money">{fmt(sidePot.amount)}</span>
              </div>
            {/each}
          </div>
        {/if}
        {#if liveBets > 0}
          <!-- Both legs are chain figures and sum back to get_pot(). -->
          <div class="pot-breakdown">
            {fmt(collectedPot)} collected + {fmt(liveBets)} betting
          </div>
        {/if}
      </div>
    {/if}
    {@render equityMethodLine()}
  </div>
{/if}

<style>
  /* Both readouts sit ABOVE the board in landscape and under it in portrait. */
  .pot-display,
  .winner-display {
    position: absolute;
    bottom: calc(100% + var(--fw) * 0.014);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--fw) * 0.006);
    pointer-events: none;
  }

  /* ---- the pot module: a rectangle, not a pill (shape encodes class) ---- */

  .main-pot {
    display: flex;
    align-items: center;
    gap: 0.7em;
    padding: 0.28em 0.95em 0.28em 0.8em;
    border-radius: calc(var(--fw) * 0.01);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate-lo));
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
    white-space: nowrap;
    transition: transform var(--cd-move) var(--cd-ease-spring),
                border-color var(--cd-move) var(--cd-ease),
                box-shadow var(--cd-move) var(--cd-ease);
  }

  .pot-meta {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.05em;
    line-height: 1.05;
  }

  .pot-label {
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-figure);
    letter-spacing: var(--cd-tracking-wide);
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .phase-indicator {
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.1em;
    color: var(--cd-accent-hi);
    text-transform: uppercase;
  }

  .pot-amount {
    font-size: 1.4em;
    font-weight: var(--cd-weight-display);
    letter-spacing: -0.01em;
    color: var(--cd-money);
    line-height: 1;
  }

  .main-pot.has-chips {
    border-color: var(--cd-money-line);
    box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-money-dim), var(--cd-shadow-pod);
  }

  /* No money in the middle yet: the module stays in the DOM (the harness reads
     `.pot-amount` on every non-complete hand) but steps back. */
  .pot-display.waiting .main-pot { opacity: 0.55; }

  /* The all-in moment: the figure at risk grows, in gold. Transform only. */
  .main-pot.at-risk { transform: scale(1.07); }
  .main-pot.at-risk .pot-label { color: var(--cd-ink-1); letter-spacing: 0.1em; }

  /* ---- footnotes ---- */

  .pot-foot {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--fw) * 0.004);
  }

  .pot-breakdown {
    font-size: var(--cd-felt-label);
    letter-spacing: 0.04em;
    color: var(--cd-ink-felt);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    padding: 0.1em 0.6em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
  }

  .equity-method {
    pointer-events: auto;
    cursor: help;
    font-size: var(--cd-felt-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--cd-ink-felt);
    white-space: nowrap;
    padding: 0.1em 0.6em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
  }

  .side-pots {
    display: flex;
    gap: calc(var(--fw) * 0.006);
  }

  .side-pot {
    display: flex;
    align-items: baseline;
    gap: 0.4em;
    padding: 0.1em 0.6em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
    border: 1px solid var(--cd-line);
    white-space: nowrap;
  }

  .side-pot-label {
    font-size: var(--cd-felt-label);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .side-pot-amount {
    font-size: var(--cd-felt-body);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-money);
  }

  /* ---- the winner line: one gold line, the hand named beside it ---- */

  .winner-display {
    gap: 0.2em;
  }

  .winner-line {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: baseline;
    column-gap: 0.7em;
    row-gap: 0.1em;
    padding: 0.3em 1.1em;
    border-radius: calc(var(--fw) * 0.01);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate-lo));
    border: 1px solid var(--cd-money-line);
    box-shadow: 0 0 calc(var(--fw) * 0.05) var(--cd-money-dim), var(--cd-shadow-pod);
    white-space: nowrap;
    max-width: var(--fw);
  }

  .winner-text {
    font-size: 1.1em;
    font-weight: var(--cd-weight-display);
    color: var(--cd-money);
    font-variant-numeric: tabular-nums;
  }

  .winner-hand-rank {
    font-size: var(--cd-felt-small);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--cd-ink-1);
  }

  .split-info {
    font-size: var(--cd-felt-small);
    color: var(--cd-ink-2);
  }

  /* The method line hangs off the winner line on the side facing away from the
     board (docs/DEFECTS.md T-27): above it in landscape, below it in portrait. */
  .winner-display .equity-method {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 0.25em);
    transform: translateX(-50%);
  }

  /* =========================================================================
     PORTRAIT: the readout sits UNDER the board and inside a 166 px slot
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    .pot-display, .winner-display {
      bottom: auto;
      top: calc(100% + var(--fw) * 0.016);
      line-height: 1.15;
      max-width: var(--fw);
    }

    .main-pot { padding: 0.18em 0.7em 0.18em 0.6em; gap: 0.5em; }
    .pot-amount { font-size: 1.3em; }
    .pot-breakdown, .equity-method, .side-pot-label { font-size: var(--cd-felt-label); }
    .side-pot-amount { font-size: var(--cd-felt-small); }
    .side-pots { flex-wrap: wrap; justify-content: center; }

    /* One row, so the block stays inside the 0.175 fw above the flank plates:
       at 0.86em "You won 24.00 ICP · STRAIGHT · COMPLETE" wrapped and the
       method footnote landed on the flank plate's name row. */
    .winner-line { padding: 0.2em 0.6em; column-gap: 0.45em; }
    .winner-text { font-size: 0.78em; }
    .winner-hand-rank, .split-info { font-size: var(--cd-felt-label); }
    .winner-display .phase-indicator { font-size: var(--cd-felt-label); }
    /* In flow under the winner line rather than hung off it: the flank seats'
       badges sit right below, and a hung line landed on one. */
    .winner-display .equity-method { position: static; transform: none; }

    /* Nine seats on a phone: the two mid-height flank plates sit 0.26 fw from
       the centre line, so everything in the middle column wraps inside 0.5 fw
       rather than running under them. */
    :global(.ring-crowded) .pot-display,
    :global(.ring-crowded) .winner-display {
      max-width: calc(var(--fw) * 0.5);
      /* ...and ABOVE the board, as in landscape: the crowded ring's board sits
         high (cluster-dy -0.18 fw) and a column under it ran through the two
         mid-height flank plates and over the lower seats' chips. Above it
         there is 0.47 fw of clear felt below the two top chairs. */
      top: auto;
      bottom: calc(100% + var(--fw) * 0.016);
    }
    :global(.ring-crowded) .pot-breakdown,
    :global(.ring-crowded) .equity-method,
    :global(.ring-crowded) .pot-label { white-space: normal; text-align: center; }
    :global(.ring-crowded) .main-pot { white-space: normal; }
  }

  @media (prefers-reduced-motion: reduce) {
    .main-pot { transition: none; }
    .main-pot.at-risk { transform: none; }
  }
</style>
