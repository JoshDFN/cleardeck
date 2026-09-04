<script>
  /**
   * THE CASHIER'S QUICK CHIPS: one tap to a named amount (the table's minimum
   * buy-in, twice it), the row a phone poker cashier opens on. Labels only;
   * the figure a chip fills in is the caller's field, read like any typed
   * amount. "2x" is a factor (token-allowlist `deposit-quick-multiple`).
   *
   * Harness contract: `.quick-amounts .quick-amount`.
   */
  const {
    /** @type {Array<{id: string, label: string, text: string, hint: string}>} */
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
        onclick={() => onPick(chip.text)}
        {disabled}
        title={chip.hint}
      >{chip.label}</button>
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
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-pill);
    border: 1px solid var(--cd-accent-line);
    background: var(--cd-accent-dim);
    color: var(--cd-accent);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  .quick-amount.btc {
    border-color: rgba(247, 147, 26, 0.3);
    background: rgba(247, 147, 26, 0.1);
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
