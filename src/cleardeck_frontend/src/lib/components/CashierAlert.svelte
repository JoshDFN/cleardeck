<script>
  // A refusal, as the cashier shows it: the humane sentence, and the
  // canister's or the ledger's own words under it as the detail line
  // (lib/humane-errors.js describeCashierFailure).
  const {
    /** The sentence. */
    message = '',
    /** The raw text, or null. */
    detail = null,
  } = $props();
</script>

<div class="alert error" role="alert">
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
    <circle cx="12" cy="12" r="10"/>
    <line x1="12" y1="8" x2="12" y2="12"/>
    <line x1="12" y1="16" x2="12.01" y2="16"/>
  </svg>
  <span class="alert-text">
    {message}
    {#if detail}<span class="alert-detail">{detail}</span>{/if}
  </span>
</div>

<style>
  .alert {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--cd-space-2) var(--cd-space-3);
    align-items: start;
    padding: var(--cd-space-3) var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    border: 1px solid var(--cd-danger-line);
    background: var(--cd-danger-dim);
    color: var(--cd-danger-hi);
    font-size: var(--cd-text-sm);
    line-height: 1.5;
  }

  .alert svg { margin-top: 2px; }
  .alert-text { min-width: 0; }

  .alert-detail {
    display: block;
    margin-top: var(--cd-space-1);
    color: var(--cd-ink-2);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    overflow-wrap: anywhere;
  }
</style>
