<script>
  // NATIVE BITCOIN IN: the ckBTC minter's address for this player, and what
  // to expect. The address is FETCHED from the table canister (it cannot be
  // derived), so the caller only hands one over for a table this build was
  // published with (docs/SECURITY-FINDINGS.md FINDING 45); this component
  // paints it and nothing else. The Check-for-deposit button lives in the
  // caller's action row, under the disclosures.

  const {
    address = '',
    loading = false,
    addressError = null,
    /** The minter's floor and its rough cost, as text (not ClearDeck's limits). */
    minDisplay = '',
    minBtcDisplay = '',
    minterFeeDisplay = '',
    result = null,
  } = $props();

  let copied = $state(false);

  function copy() {
    navigator.clipboard?.writeText(address).catch(() => {});
    copied = true;
    setTimeout(() => { copied = false; }, 2000);
  }
</script>

<section class="btc-deposit-section">
  <div class="btc-deposit-header">
    <h3>Deposit Bitcoin</h3>
    <p>Send BTC to this address. After six confirmations (about an hour), press Check for deposit to mint ckBTC.</p>
  </div>

  {#if loading}
    <div class="loading-address"><span class="spinner"></span> Getting your Bitcoin deposit address…</div>
  {:else if addressError}
    <p class="btc-error">{addressError}</p>
  {:else if address}
    <div class="btc-address-box">
      <span class="label">Your Bitcoin deposit address</span>
      <code class="address-text">{address}</code>
      <button type="button" class="copy-btn" onclick={copy}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
        </svg>
        {copied ? 'Copied' : 'Copy address'}
      </button>
    </div>

    <p class="btc-minimum-warning">
      <strong>Minimum: {minDisplay}</strong>
      Smaller amounts may not be processed. This floor and its roughly {minterFeeDisplay}
      cost belong to the ckBTC minter, not to this table, and nothing in ClearDeck enforces them.
    </p>

    <ol class="btc-steps">
      <li>Send BTC to the address above (at least {minDisplay}).</li>
      <li>Wait for six confirmations, about an hour.</li>
      <li>Press Check for deposit below.</li>
    </ol>

    {#if result}
      <p class="btc-result">{result}</p>
    {/if}

    <p class="btc-note">
      <strong>Minimum deposit:</strong> {minBtcDisplay} BTC ({minDisplay}).
      Your BTC becomes ckBTC one to one; ckBTC converts back to BTC at any time.
    </p>
  {/if}
</section>

<style>
  .btc-deposit-section { display: flex; flex-direction: column; gap: var(--cd-space-3); }

  .btc-deposit-header h3 { margin: 0 0 var(--cd-space-1); font-size: var(--cd-text-md); color: var(--cd-btc); }
  .btc-deposit-header p { margin: 0; font-size: var(--cd-text-sm); color: var(--cd-ink-2); line-height: 1.5; }

  .loading-address {
    display: flex;
    align-items: center;
    gap: var(--cd-space-3);
    padding: var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    background: var(--cd-btc-dim);
    color: var(--cd-btc);
    font-size: var(--cd-text-sm);
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--cd-btc-line);
    border-top-color: var(--cd-btc);
    border-radius: 50%;
    animation: btc-spin 0.8s linear infinite;
  }

  .btc-error { margin: 0; color: var(--cd-danger-hi); font-size: var(--cd-text-sm); }

  .btc-address-box {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    padding: var(--cd-space-4);
    border: 1px solid var(--cd-btc-line);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
  }

  .label { color: var(--cd-ink-2); font-size: var(--cd-text-xs); font-weight: var(--cd-weight-strong); letter-spacing: var(--cd-tracking-label); text-transform: uppercase; }

  .address-text {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-bg-deep);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    color: var(--cd-btc);
    word-break: break-all;
    line-height: 1.5;
  }

  .copy-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border: 1px solid var(--cd-btc-line);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-btc-dim);
    color: var(--cd-btc);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  .copy-btn:hover { background: var(--cd-btc-tint); }

  .btc-minimum-warning {
    margin: 0;
    padding: var(--cd-space-3) var(--cd-space-4);
    border: 1px solid var(--cd-warn-line);
    border-left-width: 3px;
    border-radius: var(--cd-radius-card);
    background: var(--cd-warn-dim);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
  }

  .btc-minimum-warning strong { display: block; color: var(--cd-warn); font-size: var(--cd-text-sm); }

  .btc-steps { margin: 0; padding-left: 1.2em; color: var(--cd-ink-1); font-size: var(--cd-text-sm); line-height: 1.6; }

  .btc-result { margin: 0; padding: var(--cd-space-3); border-radius: var(--cd-radius-chip); background: var(--cd-accent-dim); color: var(--cd-accent); font-size: var(--cd-text-sm); }

  .btc-note { margin: 0; color: var(--cd-ink-2); font-size: var(--cd-text-xs); line-height: 1.5; }
  .btc-note strong { color: var(--cd-ink-1); }

  @keyframes btc-spin { to { transform: rotate(360deg); } }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .copy-btn { min-height: var(--cd-touch-min); }
  }
</style>
