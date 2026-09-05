<script>
  // THE DEPOSIT SHEET'S TABS: which method (a BTC table only), which route
  // (the connected wallet, or the derived address from anywhere), which
  // wallet (Internet Identity, or OISY on a mainnet build only, where OISY
  // can sign). Presentation only: the parent owns every choice.
  //
  // Harness contract: `.wallet-source-toggle button` (the deposit scene reads
  // the source buttons), `.deposit-route`, `.deposit-method-toggle`.

  const {
    btc = false,
    /** False on a BTC table whose method is native BTC: no wallet route then. */
    showsWalletRoute = true,
    /** 'wallet' | 'address' | 'btc': the route in force. */
    route = 'wallet',
    /** 'ii' | 'oisy' */
    walletSource = 'ii',
    /** 'ckbtc' | 'btc' */
    depositMethod = 'ckbtc',
    /** Only a mainnet build renders the wallet toggle. */
    mainnet = false,
    onRoute = () => {},
    onSource = () => {},
    onMethod = () => {},
  } = $props();
</script>

{#if btc}
  <div class="segmented btc deposit-method-toggle" role="tablist" aria-label="Deposit method">
    <button type="button" role="tab" aria-selected={depositMethod === 'ckbtc'} class:active={depositMethod === 'ckbtc'} onclick={() => onMethod('ckbtc')}>
      <span>I have ckBTC</span>
      <span class="segment-hint">Instant</span>
    </button>
    <button type="button" role="tab" aria-selected={depositMethod === 'btc'} class:active={depositMethod === 'btc'} onclick={() => onMethod('btc')}>
      <span>I have BTC</span>
      <span class="segment-hint">About an hour</span>
    </button>
  </div>
{/if}

{#if showsWalletRoute}
  <!-- THE ROUTE: from the connected wallet, or from anywhere via the derived
       address. The default follows what the wallet can pay; both are always
       one tap away. -->
  {#if !btc}
    <div class="segmented deposit-route" role="tablist" aria-label="Deposit route">
      <button type="button" role="tab" aria-selected={route === 'wallet'} class:active={route === 'wallet'} onclick={() => onRoute('wallet')}>
        From this wallet
      </button>
      <button type="button" role="tab" aria-selected={route === 'address'} class:active={route === 'address'} onclick={() => onRoute('address')}>
        From an exchange or another wallet
      </button>
    </div>
  {/if}

  {#if mainnet && route === 'wallet'}
    <div class="segmented wallet-source-toggle" role="tablist" aria-label="Wallet">
      <button type="button" role="tab" aria-selected={walletSource === 'ii'} class:active={walletSource === 'ii'} onclick={() => onSource('ii')}>
        <span>Internet Identity</span>
        <span class="segment-hint">Your signed-in wallet</span>
      </button>
      <button type="button" role="tab" aria-selected={walletSource === 'oisy'} class:active={walletSource === 'oisy'} onclick={() => onSource('oisy')}>
        <span>OISY Wallet</span>
        <span class="segment-hint">Pay from OISY</span>
      </button>
    </div>
  {/if}
{/if}

<style lang="scss">
  @use './cashier' as cashier;

  @include cashier.segmented;
</style>
