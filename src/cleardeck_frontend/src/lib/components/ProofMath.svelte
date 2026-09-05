<script>
  /**
   * The raw material: every hash in full, monospace and copyable, the hash
   * this browser computed beside the one the table committed to, and the whole
   * deck as this browser built it (ProofWork) behind "Show the work".
   *
   * HARNESS CONTRACT (shuffleproof.mjs): `.compare-row .hash.small` x2 (computed
   * here, committed) read as text and compared with each other and with the
   * canister's own seed_hash; the `.section-toggle` reading "Show the work".
   */
  import ProofWork from './ProofWork.svelte';

  let {
    proof,
    revealedSeed = null,
    report = null,
    houseEcho = null,
    copied = null,
    onCopy = () => {},
    showWork = $bindable(false),
  } = $props();
</script>

<div class="math">
  <div class="proof-item">
    <span class="label">SHA-256 commitment</span>
    <div class="hash-row">
      <code class="hash" title={proof.seed_hash}>{proof.seed_hash}</code>
      <button class="copy-btn" onclick={() => onCopy(proof.seed_hash, 'hash')}>
        {copied === 'hash' ? 'Copied' : 'Copy'}
      </button>
    </div>
  </div>

  {#if revealedSeed}
    <div class="proof-item">
      <span class="label">Revealed seed</span>
      <div class="hash-row">
        <code class="hash revealed" title={revealedSeed}>{revealedSeed}</code>
        <button class="copy-btn" onclick={() => onCopy(revealedSeed, 'seed')}>
          {copied === 'seed' ? 'Copied' : 'Copy'}
        </button>
      </div>
    </div>
  {/if}

  {#if report}
    <div class="compare">
      <div class="compare-row">
        <span class="compare-label">computed here</span>
        <code class="hash small">{report.commitment.computed}</code>
      </div>
      <div class="compare-row">
        <span class="compare-label">committed in the record</span>
        <code class="hash small">{report.commitment.committed}</code>
      </div>
      <div class="verdict" class:good={report.commitment.match} class:bad={!report.commitment.match}>
        {#if report.commitment.match}
          Identical. The revealed seed is the committed seed.
        {:else}
          NOT identical. The table revealed a seed it did not commit to.
        {/if}
      </div>
    </div>
  {/if}

  {#if report?.layout}
    <div class="section">
      <button class="section-toggle" onclick={() => { showWork = !showWork; }}>
        <span>Show the work</span>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:rotated={showWork}>
          <polyline points="6,9 12,15 18,9"/>
        </svg>
      </button>
      {#if showWork}
        <ProofWork {report} {houseEcho} />
      {/if}
    </div>
  {/if}
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.hashcode;
  @include f.copybtn;

  .math { display: flex; flex-direction: column; gap: var(--cd-space-3); }
  .proof-item { display: flex; flex-direction: column; gap: var(--cd-space-1); }

  .proof-item .label, .compare-label {
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
  }

  .hash-row { display: flex; gap: var(--cd-space-2); align-items: stretch; }
  .hash-row .hash { flex: 1; min-width: 0; }
  code.hash.small { font-size: var(--cd-text-xs); }

  .compare { display: flex; flex-direction: column; gap: var(--cd-space-2); }
  .compare-row { display: flex; flex-direction: column; gap: 3px; }

  .verdict {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    line-height: 1.45;
  }

  .verdict.good { background: var(--cd-accent-dim); color: var(--cd-accent); border: 1px solid var(--cd-accent-line); }
  .verdict.bad { background: var(--cd-danger-dim); color: var(--cd-danger-hi); border: 1px solid var(--cd-danger-line); }

  .section { margin-top: var(--cd-space-1); }

  .section-toggle {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    min-height: var(--cd-control-md);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    padding: 0 var(--cd-space-3);
    color: var(--cd-ink-1);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease);
  }

  .section-toggle:hover { background: var(--cd-surface-2); color: var(--cd-ink); }
  .section-toggle svg { transition: transform var(--cd-base) var(--cd-ease); flex-shrink: 0; }
  .section-toggle svg.rotated { transform: rotate(180deg); }

  @media #{f.$phone} {
    .section-toggle { min-height: var(--cd-touch-min); }
  }
</style>
