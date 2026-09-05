<script>
  // THE RECEIPT: what moved, where it is now, and a Done button. Never an
  // auto-close: a player reads a receipt at their own pace and copies the
  // block index from it.

  const {
    title = 'Done',
    /** One sentence under the title. */
    lead = '',
    /** @type {Array<{id: string, label: string, value: string, mono?: boolean, strong?: boolean}>} */
    rows = [],
    /** An optional explorer link { href, label }. */
    link = null,
    btc = false,
    doneLabel = 'Done',
    onDone = () => {},
  } = $props();

  let copiedId = $state(null);

  function copyRow(row) {
    navigator.clipboard?.writeText(row.value).catch(() => {});
    copiedId = row.id;
    setTimeout(() => { copiedId = null; }, 1500);
  }
</script>

<section class="cashier-receipt" class:btc aria-live="polite">
  <div class="head">
    <span class="mark" aria-hidden="true">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
    </span>
    <div class="head-text">
      <h3>{title}</h3>
      {#if lead}<p class="lead">{lead}</p>{/if}
    </div>
  </div>

  {#if rows.length}
    <dl class="rows">
      {#each rows as row (row.id)}
        <div class="row" class:strong={row.strong} data-receipt-row={row.id}>
          <dt>{row.label}</dt>
          <dd class:mono={row.mono} class:cd-money={!row.mono}>
            <span class="value">{row.value}</span>
            {#if row.mono}
              <button type="button" class="copy" onclick={() => copyRow(row)} aria-label="Copy {row.label}">
                {copiedId === row.id ? 'Copied' : 'Copy'}
              </button>
            {/if}
          </dd>
        </div>
      {/each}
    </dl>
  {/if}

  {#if link}
    <a class="explorer" href={link.href} target="_blank" rel="noopener noreferrer">{link.label}</a>
  {/if}

  <button type="button" class="btn-primary done" class:btc onclick={onDone}>{doneLabel}</button>
</section>

<style>
  .cashier-receipt {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-3);
    padding: var(--cd-space-4);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-card);
    background: var(--cd-accent-dim);
  }

  .cashier-receipt.btc { border-color: var(--cd-btc-line); background: var(--cd-btc-dim); }

  .head { display: flex; gap: var(--cd-space-3); align-items: flex-start; }

  .mark {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
  }

  .btc .mark { background: var(--cd-btc); color: var(--cd-ink-on-light); }

  h3 { margin: 0; font-size: var(--cd-text-lg); font-weight: var(--cd-weight-figure); color: var(--cd-ink); }
  .lead { margin: 2px 0 0; color: var(--cd-ink-1); font-size: var(--cd-text-sm); line-height: 1.5; }

  .rows { margin: 0; display: flex; flex-direction: column; font-size: var(--cd-text-sm); }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--cd-space-3);
    padding: 6px 0;
    border-top: 1px solid var(--cd-line-soft);
  }

  .row.strong dd { font-size: var(--cd-text-figure); font-weight: var(--cd-weight-figure); color: var(--cd-accent); }
  .btc .row.strong dd { color: var(--cd-btc); }

  dt { margin: 0; color: var(--cd-ink-2); }
  dd { margin: 0; text-align: right; color: var(--cd-ink); font-weight: var(--cd-weight-strong); }

  dd.mono {
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-medium);
    overflow-wrap: anywhere;
  }

  .copy {
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    border: 1px solid var(--cd-line-strong);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: var(--cd-font-ui);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  .explorer {
    align-self: flex-start;
    color: var(--cd-accent);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .done {
    min-height: var(--cd-control-md);
    border: 0;
    border-radius: var(--cd-radius-chip);
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
    font-family: inherit;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-figure);
    cursor: pointer;
    box-shadow: var(--cd-gloss);
  }

  .done.btc { background: var(--cd-btc); color: var(--cd-ink-on-light); }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .done { min-height: calc(var(--cd-touch-min) + var(--cd-space-1)); }
    .copy { min-height: var(--cd-touch-min); min-width: var(--cd-touch-min); }
  }
</style>
