<script>
  /**
   * THE CASHIER'S QUICK CHIPS: one tap to a named amount (the table's minimum
   * buy-in, twice it), the row a phone poker cashier opens on. Each chip
   * carries its figure on its face, since a thumb never sees a hover hint;
   * the reason a chip is disabled (the wallet cannot cover it plus both
   * ledger fees) stays in the hint. The figure a chip fills in is the
   * caller's field, read like any typed amount. "2x" is a factor
   * (token-allowlist `deposit-quick-multiple`).
   *
   * Harness contract: `.quick-amounts .quick-amount[data-chip]` with the
   * figure in `.chip-figure` (token-census site `deposit-quick-chip`, asserted
   * against the table's min_buy_in by chain-agreement.mjs).
   */
  const {
    /** @type {Array<{id: string, label: string, figure: string, text: string, hint: string, disabled?: boolean}>} */
    chips = [],
    /** The field's current text, so the chip that produced it reads pressed. */
    selected = '',
    btc = false,
    disabled = false,
    onPick = () => {},
  } = $props();
</script>

{#if chips.length}
  <div class="quick-amounts" role="group" aria-label="Quick amounts">
    {#each chips as chip (chip.id)}
      <button
        type="button"
        class="quick-amount"
        class:btc
        class:active={selected === chip.text}
        aria-pressed={selected === chip.text}
        data-chip={chip.id}
        onclick={() => onPick(chip.text)}
        disabled={disabled || Boolean(chip.disabled)}
        title={chip.hint}
      >
        <span class="chip-label">{chip.label}</span>
        <span class="chip-figure">{chip.figure}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .quick-amounts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--cd-space-2);
  }

  .quick-amount {
    display: inline-flex;
    align-items: baseline;
    gap: var(--cd-space-2);
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-pill);
    border: 1px solid var(--cd-accent-line);
    background: var(--cd-accent-dim);
    color: var(--cd-accent);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-medium);
    cursor: pointer;
  }

  .chip-figure {
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
  }

  .quick-amount.btc {
    border-color: var(--cd-btc-line);
    background: var(--cd-btc-dim);
    color: var(--cd-btc);
  }

  .quick-amount.active { background: var(--cd-accent); color: var(--cd-accent-ink); }
  .quick-amount.btc.active { background: var(--cd-btc); color: var(--cd-ink-on-light); }
  .quick-amount:disabled { opacity: 0.5; cursor: not-allowed; }

  /* At the touch floor on a phone. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .quick-amount { min-height: var(--cd-touch-min); }
  }
</style>
