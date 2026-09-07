<script>
  /**
   * The pre-action row: what the hero will do when the turn arrives, chosen
   * while it is still somebody else's. Three toggles in the action row's own
   * slot (PokerStars, GGPoker), one armed at a time; pressing the armed one
   * disarms it. The resolution rules are $lib/pre-actions.js; the parent fires
   * the real action on the certified my-turn edge.
   *
   * Harness contract: `.pre-actions .pre-btn`; a "Call X" label is asserted
   * against call_amount like the primary Call button (chain-agreement.mjs).
   */
  const {
    options = [],          // availablePreActions()
    armedId = null,
    waitingFor = '',
    onArm = () => {},
  } = $props();
</script>

<div class="pre-actions" role="group" aria-label="Pre-select your next action">
  {#if waitingFor}
    <span class="pre-label"><span class="pre-wait">Waiting for {waitingFor}</span><span class="pre-sub">Pre-select</span></span>
  {/if}
  {#each options as opt (opt.id)}
    <button
      type="button"
      class="pre-btn"
      class:armed={armedId === opt.id}
      aria-pressed={armedId === opt.id}
      title={opt.hint}
      onclick={() => onArm(armedId === opt.id ? null : opt.id)}
    >
      <span class="pre-box" aria-hidden="true"></span>
      <span class="pre-text">{opt.label}</span>
    </button>
  {/each}
</div>

<style>
  .pre-actions {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--cd-space-2);
    min-width: 0;
    width: 100%;
  }

  .pre-label {
    display: flex;
    flex-direction: column;
    flex: 0 1 auto;
    min-width: 0;
    padding: 0 var(--cd-space-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pre-wait {
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pre-sub {
    font-size: var(--cd-text-xs);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .pre-btn {
    flex: 0 1 auto;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-touch-min);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
    transition: background-color var(--cd-fast) var(--cd-ease), border-color var(--cd-fast) var(--cd-ease),
                transform var(--cd-fast) var(--cd-ease);
  }

  .pre-btn:hover { background: var(--cd-surface-3); color: var(--cd-ink); }
  .pre-btn:active { transform: scale(0.97); }
  .pre-btn:focus-visible { outline: 2px solid var(--cd-accent-hi); outline-offset: 2px; }

  .pre-box {
    width: 14px;
    height: 14px;
    border-radius: 4px;
    border: 1.5px solid var(--cd-line-bright);
    background: transparent;
    flex: 0 0 auto;
  }

  .pre-btn.armed {
    background: var(--cd-accent-dim);
    border-color: var(--cd-accent-line-strong);
    color: var(--cd-accent-hi);
  }

  .pre-btn.armed .pre-box {
    border-color: var(--cd-accent);
    background: var(--cd-accent);
    box-shadow: inset 0 0 0 2.5px var(--cd-accent-ink);
  }

  /* THE PHONE: three toggles fill the row at the touch floor; the label is
     the hero pod's armed tag and the acting seat's ring. */
  @media (max-aspect-ratio: 1/1) {
    .pre-label { display: none; }
    /* Three toggles in 390 px: the checkbox glyph goes (measured: "Check /
       Fold" ellipsised with it); the armed fill and aria-pressed carry the
       state. */
    .pre-box { display: none; }
    .pre-btn { flex: 1 1 0; min-width: 0; min-height: 48px; padding: 0 var(--cd-space-1); justify-content: center; }
    .pre-text { overflow: hidden; text-overflow: ellipsis; }
  }
</style>
