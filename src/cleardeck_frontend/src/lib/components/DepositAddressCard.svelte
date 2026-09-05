<script>
  // YOUR DEPOSIT ADDRESS AT THIS TABLE, AS A CARD: the QR, the hex, Copy.
  //
  // The address arrives derived (lib/depositAddress.js, from the pinned
  // canister id and the signed-in principal, no network call) and already
  // cross-checked by the caller; this component paints it and nothing else.
  // When the caller has no address to show it says so, on purpose: an empty
  // card is safer than a guessed one (docs/SECURITY-FINDINGS.md FINDING 40).
  //
  // The minimum for this route is NOT repeated here: it is stated once, in
  // the limits line the caller renders next to this card, where the
  // screenshot harness asserts it against the canister.

  import { qrSvgPath } from '../qr-svg.js';

  const {
    /** The 64-hex account identifier, or '' while deriving / when refused. */
    address = '',
    /** A reason the address is not shown, or a heads-up shown beside it. */
    warning = null,
    /** True while the address is being derived. */
    deriving = false,
  } = $props();

  let copied = $state(false);

  const qr = $derived.by(() => {
    if (!address) return null;
    try {
      return qrSvgPath(address);
    } catch {
      return null;
    }
  });

  /** The hex in groups of eight, so a person can compare it with a wallet's. */
  const grouped = $derived(address ? address.match(/.{1,8}/g) ?? [address] : []);

  function copy() {
    navigator.clipboard?.writeText(address).catch(() => {});
    copied = true;
    setTimeout(() => { copied = false; }, 2000);
  }
</script>

<section class="deposit-address-section" data-address-shown={address ? 'yes' : 'no'}>
  <h3>Your deposit address at this table</h3>

  {#if warning && !address}
    <p class="address-mismatch">No address is shown, on purpose: {warning}</p>
  {:else if address}
    {#if warning}
      <p class="address-mismatch">Heads up: {warning}</p>
    {/if}
    <p class="address-hint">
      Yours alone, at this table. Send ICP here from an exchange or any wallet, then come
      back and press Claim.
    </p>
    <div class="address-body">
      {#if qr}
        <div class="qr" aria-label="QR code of your deposit address">
          <svg viewBox="0 0 {qr.size} {qr.size}" shape-rendering="crispEdges" role="img">
            <rect width={qr.size} height={qr.size} fill="currentColor" class="qr-bg" />
            <path d={qr.path} class="qr-ink" />
          </svg>
        </div>
      {/if}
      <div class="address-box">
        <code class="address-value">{#each grouped as part, i (i)}<span class="group">{part}</span>{/each}</code>
        <button type="button" class="copy-address-btn" onclick={copy}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
          {copied ? 'Copied' : 'Copy address'}
        </button>
      </div>
    </div>
    <ol class="how-to-fund">
      <li>Copy the address, or scan the code with your wallet.</li>
      <li>Send ICP to it from an exchange or another wallet (the minimum is in Limits below).</li>
      <li>When the transfer has landed, press Claim deposit. The table sweeps it into your balance.</li>
    </ol>
  {:else if deriving}
    <p class="address-hint">Deriving your address from your principal…</p>
  {:else}
    <p class="address-hint">Sign in to derive your address.</p>
  {/if}
</section>

<style>
  .deposit-address-section {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-3);
    padding: var(--cd-space-4);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-card);
    background: var(--cd-accent-dim);
  }

  h3 {
    margin: 0;
    color: var(--cd-accent);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
  }

  .address-hint, .address-mismatch {
    margin: 0;
    font-size: var(--cd-text-sm);
    line-height: 1.5;
    color: var(--cd-ink-1);
  }

  .address-mismatch { color: var(--cd-warn); }

  .address-body {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--cd-space-4);
    align-items: start;
  }

  .qr {
    width: 112px;
    height: 112px;
    padding: 6px;
    border-radius: var(--cd-radius-chip);
    background: var(--cd-card-face);
    color: var(--cd-card-face);
    box-shadow: var(--cd-shadow-card);
  }

  .qr svg { display: block; width: 100%; height: 100%; }
  .qr-ink { fill: var(--cd-ink-on-light); }

  .address-box {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    min-width: 0;
  }

  .address-value {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-bg-deep);
    border: 1px solid var(--cd-line-soft);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    line-height: 1.6;
    color: var(--cd-ink);
    word-break: break-all;
  }

  .group:nth-child(even) { color: var(--cd-ink-1); }

  .copy-address-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-accent-dim);
    color: var(--cd-accent);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  .copy-address-btn:hover { background: var(--cd-accent-line); }

  .how-to-fund {
    margin: 0;
    padding-left: 1.2em;
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    line-height: 1.55;
  }

  .how-to-fund li + li { margin-top: 2px; }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .address-body { grid-template-columns: minmax(0, 1fr); }
    .qr { width: 132px; height: 132px; }
    .copy-address-btn { min-height: var(--cd-touch-min); }
  }
</style>
