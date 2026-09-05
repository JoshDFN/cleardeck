<script>
  /**
   * The action log, street by street, every amount stated, synchronised with
   * the replayer: the line the current stop plays is lit and kept in view, and
   * a click on a line seeks the replay to it.
   *
   * HARNESS CONTRACT (handreplay.mjs scrapeReplayer): `.log-panel .log` whose
   * children are `.log-street > .street-name` and `.log-line.kind-* > .log-index
   * | .log-time | .log-seat | .log-what > .log-verb + .log-amount.replay-money`;
   * `.audit[data-audit]` with exactly one `.replay-money` when balanced;
   * `.provenance`. Unchanged from the previous replayer, on purpose.
   */
  import { STREET_UNKNOWN } from '$lib/hand-record.js';

  const {
    logGroups = [],
    audit = null,
    blinds = null,
    streetsRecorded = true,
    /** The line the current stop plays (lit). */
    activeLine = null,
    /** The last line played at or before the current stop: everything up to it is `.played`. */
    playedThrough = null,
    /** The street the current stop belongs to: its header is lit on a reveal stop. */
    activeStreet = null,
    /** True at the paid stop: the audit line lights, so the sync is visible at the end. */
    auditLit = false,
    actorLabel = (seat) => `Seat ${seat + 1}`,
    money = (v) => String(v),
    clock = () => '·',
    onSeekLine = () => {},
  } = $props();

  const STREET_TOTAL_NOTE = 'the total this street was raised to';

  // Keep the lit line in view inside the dialog's one scroller, without yanking
  // the whole page: `nearest` scrolls only as far as it must.
  function keepInView(node, active) {
    const run = (on) => {
      if (on && typeof node.scrollIntoView === 'function') {
        node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      }
    };
    run(active);
    return { update: run };
  }

  const seekKey = (e, index) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSeekLine(index); }
  };
</script>

<div class="panel log-panel">
  <h4>Action log</h4>
  {#if logGroups.length}
    <div class="log">
      {#each logGroups as group}
        <div class="log-street" class:active={activeStreet === group.street}>
          <span class="street-name">{group.street}</span>
          {#if group.boardAfter !== null && group.boardAfter > 0}
            <span class="street-board">board of <span class="tally">{group.boardAfter}</span></span>
          {/if}
        </div>
        {#each group.lines as line}
          {@const active = activeLine === line.index}
          <div
            class="log-line kind-{String(line.kind).toLowerCase()}"
            class:reconstructed={line.reconstructed}
            class:active
            class:played={!active && playedThrough !== null && line.index <= playedThrough}
            role="button"
            tabindex="0"
            onclick={() => onSeekLine(line.index)}
            onkeydown={(e) => seekKey(e, line.index)}
            use:keepInView={active}
          >
            <span class="log-index mono">#{line.index}</span>
            <span class="log-time timestamp">{line.timestamp ? clock(line.timestamp) : '·'}</span>
            <span class="log-seat seat-label">{actorLabel(line.seat)}</span>
            <span class="log-what">
              <span class="log-verb">{line.word}</span>{#if line.amount}{' '}<span class="log-amount replay-money">{money(line.amount)}</span>{/if}
              {#if line.amountMeaning === 'street-total'}<span class="amount-note">({STREET_TOTAL_NOTE})</span>{/if}
            </span>
          </div>
        {/each}
      {/each}
    </div>

    {#if audit?.checkable}
      <p class="audit {audit.match ? 'good' : 'bad'}" class:lit={auditLit} data-audit={audit.match ? 'balanced' : 'unbalanced'}>
        {#if audit.match}
          ✓ The blinds and every amount above add up to the pot the table paid out:
          <strong class="replay-money">{money(audit.sum)}</strong>. Nothing is missing from this log.
        {:else}
          ✗ The blinds and the amounts above come to
          <strong class="replay-money">{money(audit.sum)}</strong>, but the table paid out
          <strong class="replay-money">{money(audit.pot)}</strong>. One of the two is wrong.
        {/if}
      </p>
    {:else if audit}
      <p class="panel-note" data-audit="not-checkable">
        Not summable here: {audit.reason}. The pot on the table is the canister's own figure.
      </p>
    {/if}

    <p class="panel-note provenance">
      Streets come from each action's own <code class="mono">phase</code> field and amounts from
      its <code class="mono">amount</code> field, both read off the table canister's record.
      {#if !streetsRecorded}
        Some actions in this hand carry no street, and those are grouped under
        “{STREET_UNKNOWN}” rather than guessed at.
      {/if}
      {#if blinds?.level}
        Blind posts are <em>not</em> actions in the record, so the level above is
        {blinds.levelSource}{#if blinds.seats}, and the seats are attributed from
        {blinds.seatSource}{:else}, and no seat is attributed to them because the table's
        dealer button has already moved on{/if}.
      {/if}
    </p>
  {:else}
    <p class="panel-note">No actions recorded for this hand.</p>
  {/if}
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.panel;
  @include f.eyebrow;

  .panel-note code { font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); color: var(--cd-accent); }

  .log { display: flex; flex-direction: column; gap: 1px; }

  .log-street {
    display: flex;
    align-items: baseline;
    gap: var(--cd-space-2);
    margin: var(--cd-space-2) 0 2px;
    padding-bottom: 2px;
    border-bottom: 1px solid var(--cd-line-soft);
  }

  .log-street:first-child { margin-top: 0; }

  .street-name {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-accent);
  }

  .street-board { font-size: var(--cd-text-xs); color: var(--cd-ink-2); }

  .log-line {
    display: grid;
    /* The actor column is WIDE and NOWRAP on purpose: "Seat 2 (Nakamoto) · you"
       wrapping doubled the height of every row and pushed the audit line
       below the dialog. */
    grid-template-columns: 24px 58px 168px 1fr;
    gap: var(--cd-space-2);
    font-size: var(--cd-text-sm);
    padding: 3px var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    align-items: baseline;
    cursor: pointer;
    border-left: 2px solid transparent;
    transition: background var(--cd-fast) var(--cd-ease), border-color var(--cd-fast) var(--cd-ease);
  }

  .log-line:hover { background: var(--cd-surface-2); }
  .log-line.played { opacity: 0.75; }
  .log-line.active { background: var(--cd-accent-dim); border-left-color: var(--cd-accent); opacity: 1; }
  .log-line:focus-visible { outline: 2px solid var(--cd-accent-line-strong); outline-offset: -2px; }

  .log-index { color: var(--cd-ink-2); font-size: var(--cd-text-xs); font-family: var(--cd-font-mono); }
  .log-time { color: var(--cd-ink-2); font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); font-variant-numeric: tabular-nums; }
  .log-seat { color: var(--cd-ink-1); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .log-what { color: var(--cd-ink-1); }
  .log-amount { font-weight: var(--cd-weight-figure); margin-left: 2px; color: var(--cd-money); font-variant-numeric: tabular-nums; }
  .amount-note { color: var(--cd-ink-2); font-size: var(--cd-text-xs); margin-left: var(--cd-space-1); }
  .log-line.reconstructed .log-verb { color: var(--cd-ink-2); }

  /* colour-coded verbs, DESIGN-BAR bar 17 */
  .log-line.kind-fold .log-verb { color: var(--cd-danger-hi); }
  .log-line.kind-check .log-verb { color: var(--cd-ink-2); }
  .log-line.kind-call .log-verb { color: var(--cd-accent-hi); }
  .log-line.kind-bet .log-verb, .log-line.kind-raise .log-verb { color: var(--cd-money); }
  .log-line.kind-allin .log-verb { color: var(--cd-danger-hi); font-weight: var(--cd-weight-strong); }
  .log-line.kind-postblind .log-verb, .log-line.kind-postante .log-verb { color: var(--cd-ink-2); }

  .audit {
    margin: var(--cd-space-3) 0 0;
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    line-height: 1.55;
    color: var(--cd-ink-1);
  }

  .audit.good { background: var(--cd-accent-dim); border: 1px solid var(--cd-accent-line); }
  .audit.good strong { color: var(--cd-accent); }
  /* the paid stop: the sum lights the way a played line does */
  .audit.lit { box-shadow: 0 0 0 2px var(--cd-accent-line-strong); }
  .log-street.active .street-name { color: var(--cd-accent-hi); }
  .log-street.active { border-bottom-color: var(--cd-accent-line); }
  .audit.bad { background: var(--cd-danger-dim); border: 1px solid var(--cd-danger-line); }
  .audit.bad strong { color: var(--cd-danger-hi); }

  .provenance { border-top: 1px solid var(--cd-line-soft); padding-top: var(--cd-space-2); font-size: var(--cd-text-xs); }

  @media (max-width: 560px) {
    /* WHO ACTED IS NOT THE COLUMN TO DROP: the index goes, the time shrinks. */
    .log-line { grid-template-columns: 44px 96px 1fr; gap: var(--cd-space-1); }
    .log-index { display: none; }
  }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .log-line { min-height: var(--cd-control-sm); align-items: center; }
  }
</style>
