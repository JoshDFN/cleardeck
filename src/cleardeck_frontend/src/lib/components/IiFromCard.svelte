<script>
  // FROM: THE INTERNET IDENTITY WALLET AND WHAT IT HOLDS.
  //
  // The balance row is always rendered on the ICP path: `.balance-crypto` is
  // the figure the screenshot harness compares with the ledger, `.usd-value`
  // with the quote the run served. The card judges nothing; the caller says
  // whether the wallet can pay and what the floor is, both interpolated from
  // the mirrored constants (docs/DEFECTS.md T-26).

  const {
    /** The four-decimal balance with its unit, or the placeholder. */
    balanceText = '…',
    /** "~$0.03", or null with no quote. */
    usdText = null,
    loading = false,
    /** Whether the balance covers the minimum plus both ledger fees. */
    hasEnough = true,
    /** The floor that sentence names, formatted with its unit. */
    minWalletBalanceDisplay = '',
    btc = false,
    /** 'wallet' or 'address': which route the card sits on. */
    route = 'wallet',
    onReread = () => {},
  } = $props();
</script>

<div class="from-card" class:btc>
  <div class="section-label">
    <span>{route === 'address' ? 'Your wallet here' : 'From'}</span>
    <button type="button" class="path-link" onclick={onReread} disabled={loading}>Re-read</button>
  </div>
  <div class="balance-row">
    <span class="balance-label">Internet Identity {btc ? 'ckBTC' : 'ICP'} balance</span>
    <span class="balance-value" class:loading class:btc>
      {#if loading}
        <span class="mini-spinner" class:btc></span>
      {:else}
        <span class="balance-crypto">{balanceText}</span>
        {#if usdText !== null}<span class="usd-value">{usdText}</span>{/if}
      {/if}
    </span>
  </div>
  {#if !loading && !hasEnough}
    <p class="from-note short">
      Below the {minWalletBalanceDisplay} a deposit from this wallet needs (the minimum plus both network fees).
      {#if btc}Switch to "I have BTC" to deposit real Bitcoin, or get ckBTC from an exchange.{:else}Send to your deposit address instead.{/if}
    </p>
  {/if}
</div>

<style lang="scss">
  @use './from-card' as card;
  @include card.card;
</style>
