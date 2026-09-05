<script>
  /**
   * The 4-point equity line under the replay: one row per seat, one cell per
   * street (pre-flop, flop, turn, river), read from the same equity results
   * the pods show at each street's reveal (lib/replay-equity-line.js, fed by
   * HandReplayer's per-stop cache). The column of the current street is lit
   * so the line and the scrubber read as one instrument.
   *
   * HARNESS CONTRACT (handreplay.mjs): `.equity-line .replay-equity[data-seat]
   * [data-method][data-street]` is every percentage on the line (the census
   * site `replay-equity` covers them; the scene recomputes each with the
   * independent oracle at that street's board). `.eq-note` cells carry no
   * digits. The footnote `.eq-method` names the methods, as the pods' caption does.
   */
  import { formatEquity } from '$lib/equity.js';
  import { lineMethodNote } from '$lib/replay-equity-line.js';

  const {
    /** lib/replay-equity-line.js equityLineRows(...) */
    line = { streets: [], rows: [], any: false },
    /** The scrubber's current street label, to light the column. */
    activeStreet = null,
    label = (seat) => `Seat ${seat + 1}`,
    who = () => null,
    isMe = () => false,
    won = () => false,
  } = $props();

  const note = $derived(lineMethodNote(line.streets));
  const NOTE_WORD = { folded: 'folded', unknown: '·', unreached: '·' };
</script>

{#if line.any}
  <div class="equity-line" role="table" aria-label="Equity by street">
    <div class="eq-head" role="row">
      <span class="eq-title" role="columnheader">Equity</span>
      {#each line.streets as s}
        <span class="eq-col" role="columnheader" class:active={activeStreet === s.label} class:unreached={!s.reached}>{s.label}</span>
      {/each}
    </div>
    {#each line.rows as row (row.seat)}
      <div class="eq-row" role="row" class:me={isMe(row.seat)} class:won={won(row.seat)}>
        <span class="eq-seat seat-label" role="rowheader">{label(row.seat)}{#if who(row.seat)}<span class="eq-who">{' · '}{who(row.seat)}</span>{/if}</span>
        {#each row.cells as cell (cell.street)}
          <span class="eq-cell" role="cell" data-state={cell.state} class:active={activeStreet === cell.street}>
            {#if cell.state === 'live'}
              <span class="eq-bar" aria-hidden="true" style:--share={cell.share}></span>
              <span class="replay-equity" data-seat={row.seat} data-method={cell.method} data-street={cell.street}>{formatEquity(cell.share, cell.method)}</span>
            {:else}
              <span class="eq-note">{NOTE_WORD[cell.state] || '·'}</span>
            {/if}
          </span>
        {/each}
      </div>
    {/each}
    {#if note}
      <p class="eq-method">{note}</p>
    {/if}
  </div>
{/if}

<style>
  .equity-line {
    --eq-cols: minmax(0, 1.4fr) repeat(4, minmax(0, 1fr));
    display: flex;
    flex-direction: column;
    gap: calc(var(--cd-space-1) / 2);
    margin: 0 0 var(--cd-space-3);
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate) 55%, var(--cd-plate-lo));
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
    font-size: var(--cd-text-sm);
  }

  .eq-head, .eq-row {
    display: grid;
    grid-template-columns: var(--eq-cols);
    gap: var(--cd-space-2);
    align-items: center;
  }

  .eq-title, .eq-col {
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .eq-title { text-align: left; color: var(--cd-ink-1); font-weight: var(--cd-weight-figure); }
  .eq-col.active { color: var(--cd-accent); }
  .eq-col.unreached { opacity: 0.5; }

  .eq-seat {
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .eq-row.me .eq-seat { color: var(--cd-accent-hi); }
  .eq-who { font-weight: var(--cd-weight-body); color: var(--cd-ink-1); }

  .eq-cell {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: calc(var(--cd-control-sm) * 0.8);
    border-radius: var(--cd-radius-chip);
    overflow: hidden;
    font-variant-numeric: tabular-nums;
    font-weight: var(--cd-weight-figure);
    color: var(--cd-ink-1);
  }

  .eq-cell.active { box-shadow: inset 0 0 0 1px var(--cd-accent-line-strong); color: var(--cd-accent-hi); }
  .eq-row.won .eq-cell:last-child { color: var(--cd-money); }

  /* the share as a bar behind the figure: the eye reads the swing before the digits */
  .eq-bar {
    position: absolute;
    inset: 0 auto 0 0;
    width: calc(var(--share, 0) * 100%);
    background: var(--cd-accent-dim);
    opacity: 0.8;
  }

  .replay-equity { position: relative; }
  .eq-note { color: var(--cd-ink-2); font-weight: var(--cd-weight-body); }
  .eq-cell[data-state="folded"] .eq-note { color: var(--cd-danger-hi); font-size: var(--cd-text-xs); text-transform: uppercase; letter-spacing: 0.04em; }

  .eq-method {
    margin: calc(var(--cd-space-1) / 2) 0 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    text-align: right;
  }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .equity-line { --eq-cols: minmax(0, 1.2fr) repeat(4, minmax(0, 1fr)); padding: var(--cd-space-2); font-size: var(--cd-text-xs); }
    .eq-head, .eq-row { gap: var(--cd-space-1); }
    .eq-cell { min-height: var(--cd-control-sm); }
  }
</style>
