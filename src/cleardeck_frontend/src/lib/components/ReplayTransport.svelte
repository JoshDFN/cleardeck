<script>
  /**
   * The replayer's transport: the street scrubber (one stop per street plus the
   * showdown), step back / play-pause / step forward, the current street's
   * name, and a timeline the thumb can be dragged along one ACTION at a time.
   *
   * HARNESS CONTRACT: `.scrubber .stop` (labels Pre-flop / Flop / Turn / River /
   * Showdown for a hand that reached one; clicking one lands on that street's
   * first stop) and `.transport-label` reading the same label. The timeline is
   * an <input type=range>, which the token census does not read.
   */
  const {
    streets = [],
    currentStreet = null,
    index = 0,
    count = 1,
    playing = false,
    onSeek = () => {},
    onStep = () => {},
    onToggle = () => {},
  } = $props();

  const atStart = $derived(index <= 0);
  const atEnd = $derived(index >= count - 1);
</script>

<div class="transport-shell">
<div class="scrubber" role="tablist" aria-label="Streets">
  {#each streets as s}
    <button
      class="stop"
      class:active={currentStreet?.label === s.label}
      class:passed={currentStreet && s.index < currentStreet.index}
      role="tab"
      aria-selected={currentStreet?.label === s.label}
      onclick={() => onSeek(s.index)}
    >{s.label}</button>
  {/each}
</div>

<div class="transport">
  <button class="transport-btn" onclick={() => onStep(-1)} disabled={atStart} aria-label="Previous action">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M16 5v14L6 12z"/></svg>
  </button>
  <button class="transport-btn play" onclick={onToggle} aria-label={playing ? 'Pause replay' : 'Play replay'}>
    {#if playing}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M7 5h4v14H7zM13 5h4v14h-4z"/></svg>
    {:else}
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M7 5l12 7-12 7z"/></svg>
    {/if}
  </button>
  <button class="transport-btn" onclick={() => onStep(1)} disabled={atEnd} aria-label="Next action">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l10-7z"/></svg>
  </button>
  <span class="transport-label">{currentStreet?.label ?? ''}</span>
  <input
    class="timeline"
    type="range"
    min="0"
    max={Math.max(0, count - 1)}
    step="1"
    value={index}
    aria-label="Action timeline"
    oninput={(e) => onSeek(Number(e.currentTarget.value))}
  />
</div>
</div>

<style>
  .scrubber {
    display: flex;
    gap: var(--cd-space-1);
    margin-bottom: var(--cd-space-2);
  }

  .stop {
    flex: 1;
    min-width: 0;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-1);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-soft);
    background: var(--cd-surface-1);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease), border-color var(--cd-base) var(--cd-ease);
  }

  .stop.passed { color: var(--cd-ink-1); border-color: var(--cd-line); }
  .stop.active { background: var(--cd-accent-dim); border-color: var(--cd-accent-line-strong); color: var(--cd-accent); }
  .stop:hover:not(.active) { background: var(--cd-surface-2); color: var(--cd-ink); }

  .transport {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    margin-bottom: var(--cd-space-3);
  }

  .transport-btn {
    width: var(--cd-control-md);
    height: var(--cd-control-md);
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    cursor: pointer;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease);
  }

  .transport-btn:hover:not(:disabled) { background: var(--cd-surface-3); color: var(--cd-ink); }
  .transport-btn:disabled { opacity: 0.35; cursor: default; }
  .transport-btn.play { background: var(--cd-accent); border-color: transparent; color: var(--cd-accent-ink); }
  .transport-btn.play:hover { background: var(--cd-accent); filter: brightness(1.08); color: var(--cd-accent-ink); }

  .transport-label {
    flex: 0 0 auto;
    min-width: 5.5em;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
  }

  .timeline {
    flex: 1;
    min-width: 0;
    height: var(--cd-control-sm);
    accent-color: var(--cd-accent);
    cursor: pointer;
  }

  /* A wide screen: the transport and the scrubber share one row, so the
     picture, the row and the equity line fit one frame. */
  @media (min-width: 900px) and (min-aspect-ratio: 1/1) {
    .transport-shell { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--cd-space-3); align-items: center; margin-bottom: var(--cd-space-3); }
    .scrubber { order: 2; margin-bottom: 0; }
    .transport { order: 1; margin-bottom: 0; }
    .timeline { min-width: 140px; }
    .transport-label { min-width: 0; }
  }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .stop, .transport-btn { min-height: var(--cd-touch-min); }
    /* five labels across 358 px: no tracking, no caps, no ellipsis */
    .stop { text-transform: none; letter-spacing: 0; padding: 0 2px; }
    .transport-btn { width: var(--cd-touch-min); height: var(--cd-touch-min); }
    .transport { flex-wrap: wrap; }
    .timeline { flex-basis: 100%; height: var(--cd-touch-min); }
    .transport-label { flex: 1; }
  }
</style>
