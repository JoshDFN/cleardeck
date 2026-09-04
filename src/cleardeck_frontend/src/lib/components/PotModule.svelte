<script>
  /**
   * The centre of the table: the pot module, or the winner line once the pot
   * has gone.
   *
   * ONE gold figure with a POT label and the street, as every reference client
   * and every broadcast graphic draws it. The felt is not a document: the
   * decomposition (collected + betting) is the module's tooltip, the equity
   * method is the badges' tooltip and a LOG line, and the side-pot row is
   * drawn only when there is more than one pot to tell apart.
   *
   * THE HARNESS CONTRACT. `.pot-display > .main-pot > .pot-amount` is the
   * figure asserted against get_pot(); `.pot-breakdown` must carry two numbers
   * that sum to it; `.side-pot > .side-pot-label + .side-pot-amount` one per
   * layer, as many as the canister has; `.phase-indicator` the street;
   * `.equity-method` the method line with its `title`; `.winner-display >
   * .winner-text | .winner-hand-rank | .split-info`. tools/shots/lib/dom-scrape
   * reads these by textContent, so the demoted lines stay in the DOM with
   * `visibility: hidden` (out of flow), which the token census also treats as
   * not on screen.
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
    compact = false,      // the nine-seat phone: a column module, one-word label
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
  const breakdown = $derived(liveBets > 0 ? `${fmt(collectedPot)} collected + ${fmt(liveBets)} betting` : '');
</script>

{#snippet equityMethodLine()}
  {#if equityMethodLabel}
    <!-- THE METHOD TRAVELS WITH THE FIGURE (docs/DEFECTS.md T-27): the same
         element under whichever readout is on screen; demoted to the tooltip. -->
    <div class="equity-method demoted" title={equityNote}>{equityMethodLabel}</div>
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
  <!-- `tall-column`: three or more pots stack three pill rows under the
       module; in portrait that column reached the lower flank seat's dealer
       puck, so the module tightens (see the portrait block). -->
  <div class="pot-display" class:waiting={totalPot <= 0} class:tall-column={sidePots.length >= 3}>
    <div class="main-pot" class:has-chips={totalPot > 0} class:at-risk={allInMoment} title={breakdown}>
      <span class="pot-meta">
        <span class="pot-label" title={allInMoment ? `All in · ${allInCount} at risk` : undefined}>
          {#if allInMoment && compact}
            All in
          {:else if allInMoment}
            All in &middot; {allInCount} at risk
          {:else}
            Pot
          {/if}
        </span>
        <span class="phase-indicator">{streetLabel}</span>
      </span>
      <span class="pot-amount cd-money">{totalPot > 0 ? fmt(totalPot) : '--'}</span>
    </div>
    {#if sidePots.length > 0}
      <!-- index 0 is the MAIN pot (docs/DEFECTS.md T-12). One pot alone is
           the headline figure restated, so the row is drawn only for two or
           more; the elements stay for the count the harness asserts. -->
      <div class="side-pots" class:demoted={sidePots.length < 2}>
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
      <div class="pot-breakdown demoted">{breakdown}</div>
    {/if}
    {@render equityMethodLine()}
  </div>
{/if}

<style>
  /* Both readouts sit ABOVE the board in landscape and under it in portrait. */
  .pot-display,
  .winner-display {
    position: absolute;
    bottom: calc(100% + var(--fw) * 0.016);
    left: 50%;
    transform: translateX(-50%);
    /* Sized by its own content, not by the board tray it is anchored to. */
    width: max-content;
    max-width: var(--fw);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--fw) * 0.006);
    pointer-events: none;
    transition: opacity var(--cd-move) var(--cd-ease);
  }

  /* No money in the middle yet: the module stays in the DOM and painted (the
     harness reads `.pot-amount` on every non-complete hand and the in-frame
     probe wants a painted pot readout on every table view) but says only
     the street, as a quiet capsule: no POT label, no `--` figure, no gold. */
  .pot-display.waiting .main-pot {
    background: var(--cd-capsule);
    border-color: var(--cd-line-soft);
    box-shadow: none;
    padding: 0.2em 0.8em;
    opacity: 0.85;
  }
  .pot-display.waiting .pot-label,
  .pot-display.waiting .pot-amount { display: none; }

  /* DEMOTED LINES: in the DOM for the harness, out of flow, not painted. */
  .demoted {
    position: absolute;
    visibility: hidden;
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

  /* The all-in moment: the figure at risk grows, in gold. Transform only. */
  .main-pot.at-risk { transform: scale(1.07); }
  .main-pot.at-risk .pot-label { color: var(--cd-ink-1); letter-spacing: 0.1em; }

  /* ---- the side-pot row (two or more pots) ---- */

  .side-pots {
    display: flex;
    gap: calc(var(--fw) * 0.006);
    margin-top: calc(var(--fw) * 0.004);
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

  /* The demoted breakdown and method lines keep their type so a tooltip or
     an inspector still reads them as the felt would have. */
  .pot-breakdown,
  .equity-method {
    font-size: var(--cd-felt-label);
    letter-spacing: 0.04em;
    color: var(--cd-ink-felt);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* ---- the winner line: one gold line, the hand named beside it ---- */

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

  /* =========================================================================
     PORTRAIT: the readout sits UNDER the board
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    .pot-display, .winner-display {
      bottom: auto;
      top: calc(100% + var(--fw) * 0.02);
      line-height: 1.15;
      /* The readout sits in the band between the board and the lower flank
         plates (0.17 fw below the centre on the 6-max ring). A flank winner's
         award stands above its plate's outer half in that same band, ending
         0.31 fw from the centre line (portraitAwardSpot), so the readout
         stays inside 0.58 fw and wraps to its two rows instead of reaching
         the award. */
      max-width: calc(var(--fw) * 0.58);
    }

    .main-pot { padding: 0.18em 0.7em 0.18em 0.6em; gap: 0.5em; }
    .pot-amount { font-size: 1.3em; }
    .side-pot-label { font-size: var(--cd-felt-label); }
    .side-pot-amount { font-size: var(--cd-felt-small); }
    .side-pots { flex-wrap: wrap; justify-content: center; }

    /* THREE OR MORE POTS on the phone: the column of pills (main, side 1,
       side 2) stood 103 px tall under the module and its last pill painted
       over the lower flank seat's dealer puck, which sits level with that
       plate's bottom edge past its inner end (table-geometry puckSpot; the
       hero's lifted pair is right under it, so the puck cannot drop). The
       column cannot widen either: the band between the two lower flank
       plates is one pill wide (measured: the meta on one line widened the
       module onto the left flank's bet pill, 10.8% of "0.20" covered). So
       the column gets SHORTER at the same width: the figure a step smaller,
       the module's padding and the pills' pitch tighter, the module closer
       to the board. Measured: the last pill clear of the puck. */
    .pot-display.tall-column { top: calc(100% + var(--fw) * 0.008); gap: calc(var(--fw) * 0.002); }
    .pot-display.tall-column .main-pot { padding: 0.1em 0.7em 0.1em 0.6em; gap: 0.45em; }
    .pot-display.tall-column .pot-meta { gap: 0; line-height: 1; }
    .pot-display.tall-column .pot-amount { font-size: 1.15em; }
    /* One pill per row, always: at the tighter pitch two pills fitted the
       0.58 fw width and the pair reached the left flank's bet pill. */
    .pot-display.tall-column .side-pots { flex-direction: column; align-items: center; gap: calc(var(--fw) * 0.002); margin-top: 0; }
    .pot-display.tall-column .side-pot { padding: 0.02em 0.5em; }

    /* Two short rows by design: the figure, then the hand and the street. A
       long single row wrapped unpredictably and reached a flank plate. */
    .winner-line { padding: 0.22em 0.6em; column-gap: 0.45em; max-width: 100%; }
    .winner-text { font-size: 0.72em; flex: 1 0 100%; text-align: center; }
    .winner-hand-rank, .split-info { font-size: var(--cd-felt-label); }
    .winner-display .phase-indicator { font-size: var(--cd-felt-label); }

    /* Nine seats on a phone: the board sits high, so the readout goes ABOVE
       it, as in landscape; below it ran through the mid-height flank plates. */
    :global(.ring-crowded) .pot-display,
    :global(.ring-crowded) .winner-display {
      top: auto;
      bottom: calc(100% + var(--fw) * 0.02);
      /* The upper flank plates' inner ends are 0.26 fw either side of the
         centre line at the readout's height (measured: the module over
         Turing's plate at 0.58 fw, its shadow touching it at 0.46). */
      max-width: calc(var(--fw) * 0.42);
    }
    /* Nine seats on a phone: a COLUMN module (label, figure, street), each
       row one line at the floor. The label is one word ("All in"; the
       at-risk count is its title) because the row form wrapped the label to
       three lines and the street to two. Only the side-pot row may wrap. */
    :global(.ring-crowded) .main-pot {
      flex-direction: column;
      align-items: center;
      gap: 0.05em;
      padding: 0.2em 0.6em 0.25em;
      white-space: nowrap;
    }
    :global(.ring-crowded) .pot-meta { display: contents; }
    :global(.ring-crowded) .pot-label { order: 1; white-space: nowrap; letter-spacing: 0.14em; }
    :global(.ring-crowded) .pot-amount { order: 2; font-size: 1.15em; }
    :global(.ring-crowded) .phase-indicator { order: 3; white-space: nowrap; letter-spacing: 0.08em; }
    :global(.ring-crowded) .winner-line { padding: 0.2em 0.5em; }
  }

  @media (prefers-reduced-motion: reduce) {
    .main-pot, .pot-display, .winner-display { transition: none; }
    .main-pot.at-risk { transform: none; }
  }
</style>
