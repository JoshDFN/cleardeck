<script>
  // THE COST OF WHAT IS ABOUT TO HAPPEN, ROW BY ROW, BEFORE THE BUTTON.
  //
  // Every value here is money the caller computed from the typed amount and
  // the mirrored ledger fee (lib/cashier-format.js depositCost / withdrawNet).
  // Each row carries `data-row` so the screenshot harness can compare it, by
  // name, with the same arithmetic done from the chain's own fee
  // (tools/shots/lib/chain-agreement.mjs, the cost-summary check).

  const {
    /** @type {Array<{id: string, label: string, value: string, note?: string, strong?: boolean, tone?: 'money'|'muted'|'warn'}>} */
    rows = [],
    /** A small caption over the rows ("What this costs"). */
    caption = '',
    btc = false,
  } = $props();
</script>

{#if rows.length}
  <dl class="cost-summary" class:btc data-summary>
    {#if caption}
      <div class="caption">{caption}</div>
    {/if}
    {#each rows as row (row.id)}
      <div class="row" class:strong={row.strong} data-row={row.id} data-tone={row.tone ?? ''}>
        <dt>
          {row.label}
          {#if row.note}<span class="note">{row.note}</span>{/if}
        </dt>
        <dd class="cd-money">{row.value}</dd>
      </div>
    {/each}
  </dl>
{/if}

<style>
  .cost-summary {
    margin: 0;
    padding: var(--cd-space-2) var(--cd-space-4);
    display: flex;
    flex-direction: column;
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
    font-size: var(--cd-text-sm);
  }

  .caption {
    padding: var(--cd-space-1) 0;
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--cd-space-3);
    padding: 6px 0;
  }

  .row + .row { border-top: 1px solid var(--cd-line-soft); }
  .row.strong { border-top: 1px solid var(--cd-line-strong); padding-top: var(--cd-space-2); }

  dt {
    margin: 0;
    color: var(--cd-ink-2);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .row.strong dt { color: var(--cd-ink-1); font-weight: var(--cd-weight-strong); }

  .note { color: var(--cd-ink-2); font-size: var(--cd-text-xs); font-weight: var(--cd-weight-body); }

  dd {
    margin: 0;
    text-align: right;
    color: var(--cd-ink);
    font-weight: var(--cd-weight-strong);
    white-space: nowrap;
  }

  .row.strong dd { font-size: var(--cd-text-figure); font-weight: var(--cd-weight-figure); }
  .row[data-tone='money'] dd { color: var(--cd-accent); }
  .cost-summary.btc .row[data-tone='money'] dd { color: var(--cd-btc); }
  .row[data-tone='muted'] dd { color: var(--cd-ink-2); font-weight: var(--cd-weight-medium); }
  .row[data-tone='warn'] dd { color: var(--cd-warn); }
</style>
