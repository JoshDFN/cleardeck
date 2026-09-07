<script>
  // FROM: THE OISY WALLET. Mainnet only (OISY signs there alone); the card
  // renders the connect control, or the balance with who pays and who is
  // credited: OISY pays, the signed-in identity is credited, because the
  // deposit subaccount is derived from the SESSION principal (docs/SECURITY-
  // FINDINGS.md FINDING 40). The audit found that line missing.

  import { shortId } from '$lib/lobby-format.js';

  const {
    /** The oisy store's state. */
    oisyState = { isConnected: false, isConnecting: false, loadingBalances: false, principal: null, error: null },
    /** The four-decimal balance with its unit, or the placeholder. */
    balanceText = '…',
    usdText = null,
    hasEnough = true,
    minWalletBalanceDisplay = '',
    /** The signed-in principal that is credited. */
    sessionPrincipal = null,
    btc = false,
    onConnect = () => {},
    onDisconnect = () => {},
  } = $props();
</script>

{#if !oisyState.isConnected}
  <div class="from-card oisy" class:btc>
    <div class="section-label"><span>From</span><span class="label-aside">OISY Wallet</span></div>
    <p class="from-note">Top up your table balance straight from OISY. Keep the OISY popup open: it asks you to approve the transfer when you press Deposit.</p>
    <button type="button" class="btn-connect-oisy" class:btc onclick={onConnect} disabled={oisyState.isConnecting}>
      {#if oisyState.isConnecting}<span class="mini-spinner"></span> Connecting…{:else}Connect OISY Wallet{/if}
    </button>
    {#if oisyState.error}<p class="from-note short">{oisyState.error}</p>{/if}
  </div>
{:else}
  <div class="from-card oisy" class:btc>
    <div class="section-label">
      <span>From</span>
      <button type="button" class="path-link" onclick={onDisconnect}>Disconnect OISY</button>
    </div>
    <div class="balance-row">
      <span class="balance-label">OISY {btc ? 'ckBTC' : 'ICP'} balance</span>
      <span class="balance-value oisy" class:loading={oisyState.loadingBalances} class:btc>
        {#if oisyState.loadingBalances}
          <span class="mini-spinner" class:btc></span>
        {:else}
          <span class="balance-crypto">{balanceText}</span>
          {#if usdText !== null}<span class="usd-value">{usdText}</span>{/if}
        {/if}
      </span>
    </div>
    <p class="from-note">
      Paid by OISY <code>{shortId(oisyState.principal ?? '')}</code>, credited to your
      signed-in identity <code>{shortId(sessionPrincipal ?? '')}</code>. Keep the OISY
      popup open; it asks you to approve when you press Deposit.
    </p>
    {#if !oisyState.loadingBalances && !hasEnough}
      <p class="from-note short">Not enough {btc ? 'ckBTC' : 'ICP'} in OISY for the minimum plus both network fees ({minWalletBalanceDisplay}). Add funds to OISY, or switch to Internet Identity.</p>
    {/if}
  </div>
{/if}

<style lang="scss">
  @use './from-card' as card;
  @include card.card($oisy: true);

  .btn-connect-oisy {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border: 0;
    border-radius: var(--cd-radius-chip);
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    cursor: pointer;
    box-shadow: var(--cd-gloss);
  }

  .btn-connect-oisy.btc { background: var(--cd-btc); color: var(--cd-ink-on-light); }
  .btn-connect-oisy:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn-connect-oisy .mini-spinner { border-color: var(--cd-accent-ink); border-top-color: transparent; }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .btn-connect-oisy { min-height: var(--cd-touch-min); }
  }
</style>
