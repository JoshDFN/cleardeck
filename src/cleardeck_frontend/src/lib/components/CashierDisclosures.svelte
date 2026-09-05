<script>
  // BEFORE YOU DEPOSIT (OR WITHDRAW): the disclosures, organised.
  //
  // Summary first, detail one tap away, nothing contradicting anything, and
  // every one of them in flow with no position and no z-index, so none can
  // paint over the five protected phrases above it (HARD RULE 2). The caller
  // renders the solvency block and the cycle-runway panel as children, so the
  // screenshot harness keeps finding `.modal-content .solvency` where it did.
  //
  // THE CUSTODY DISCLOSURE (docs/SECURITY-FINDINGS.md FINDING 23) keeps every
  // word: the headline sentence is always on screen; the mechanism paragraph
  // opens under "How". `scripts/dev.sh hygiene` greps the source for its four
  // phrases and for the two retracted claims, and both still hold here.
  //
  // WAVE 12 CORRECTION, kept with the paragraph because it is the point of it:
  // the first version promised a depositor that the money was unreachable by
  // the operator too. That was FALSE in the operator's favour: a 500-byte
  // module installed with the same call as the wipe moved 39.99990000 ICP of
  // player deposits into a wallet the operator owns
  // (tests/money_safety/tests/controller_custody.rs). A disclosure that is
  // wrong in the operator's favour is worse than none.

  import { IS_MAINNET_BUILD, NETWORK } from '../ic-config.js';

  const {
    /** 'deposit' or 'withdraw': which disclosures apply. */
    context = 'deposit',
    currencySymbol = 'ICP',
    tableCanisterId = null,
    /** docs/SECURITY-FINDINGS.md FINDING 42: is this a table the build was published with? */
    tableIsTrusted = true,
    untrustedReason = null,
    children = null,
  } = $props();

  const isDeposit = $derived(context === 'deposit');
</script>

<section class="disclosures" data-context={context}>
  <h3 class="disclosures-title">{isDeposit ? 'Before you deposit' : 'Before you withdraw'}</h3>

  {#if isDeposit}
    <!-- WHO CAN TAKE THIS MONEY, ON THE SCREEN IT LEAVES FROM (FINDING 23).
         The README sold "fair play without requiring trust" and disclosed only
         that a controller can destroy the HAND HISTORY; an auditor then erased
         her own balance with one install_code --mode reinstall, no code
         change, while the ledger still held it. Reproduced at 40.00000000 ICP
         by tests/money_safety/tests/controller_custody.rs. -->
    <div class="custody-notice">
      <strong>One key can zero this balance, and the same key can take it.</strong>
      <details class="more">
        <summary>How</summary>
        <p>
          Every ClearDeck canister has a single controller principal. That key can erase
          every player's balance with one ordinary management call
          (<code>install_code --mode reinstall</code> or <code>uninstall_code</code>), and
          because the same call installs any code it is handed, it can also pay this
          canister's whole ledger balance into a wallet the operator owns. Both have been
          done on a test replica. No bug in ClearDeck is needed, there is no warning and
          there is no restore path. Depositing means trusting one key with the whole balance.
          The shuffle needs no trust. Custody does.
        </p>
      </details>
    </div>

    <!-- WHERE THE MONEY IS ACTUALLY GOING. Compiled in at build time
         (ic-config.js NETWORK), from the same constant that chooses the
         gateway and the canister ids, so this line and the destination can
         never disagree. -->
    <p class="network-line" class:mainnet={IS_MAINNET_BUILD} data-network={NETWORK}>
      {#if IS_MAINNET_BUILD}
        <strong>Internet Computer mainnet.</strong> This moves REAL {currencySymbol}
        to canister <code>{tableCanisterId ?? 'unknown'}</code>, and it is not reversible.
      {:else}
        <strong>{NETWORK} build.</strong> This moves test {currencySymbol} on your own
        replica, to canister <code>{tableCanisterId ?? 'unknown'}</code>. No real funds
        can be reached from this bundle.
      {/if}
    </p>

    <!-- IS THIS EVEN A CLEARDECK TABLE? FINDING 42. When this shows, the
         address derivation has already refused and the deposit refuses before
         it signs anything, so the paragraph and the behaviour cannot drift. -->
    {#if !tableIsTrusted}
      <p class="untrusted-table" data-untrusted-table={tableCanisterId ?? 'none'}>
        <strong>This is not one of this build's tables. Nothing can be sent here.</strong>
        {untrustedReason}
      </p>
    {/if}
  {/if}

  {@render children?.()}
</section>

<style>
  .disclosures {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
  }

  .disclosures-title {
    margin: 0;
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
  }

  /* Amber, like the trust bar and the solvency block: a second, different
     warning next to the protected strip rather than more of the same
     sentence. In flow, no position, no z-index. */
  .custody-notice {
    margin: 0;
    padding: var(--cd-space-2) var(--cd-space-3);
    border: 1px solid var(--cd-warn-line);
    border-left-width: 3px;
    border-radius: var(--cd-radius-card);
    background: var(--cd-warn-dim);
    color: var(--cd-ink-1);
    font-size: var(--cd-text-sm);
    line-height: 1.5;
  }

  .custody-notice strong { color: var(--cd-warn); }

  .custody-notice code {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--cd-capsule);
    color: var(--cd-ink);
    white-space: nowrap;
  }

  .more { margin-top: var(--cd-space-1); }

  .more summary {
    display: inline-flex;
    align-items: center;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    margin-left: calc(-1 * var(--cd-space-2));
    border-radius: var(--cd-radius-chip);
    color: var(--cd-warn);
    font-weight: var(--cd-weight-strong);
    font-size: var(--cd-text-sm);
    list-style: none;
    cursor: pointer;
  }

  .more summary::-webkit-details-marker { display: none; }
  .more summary::after { content: ' \25BE'; }
  .more[open] summary::after { content: ' \25B4'; }
  .more summary:hover { background: var(--cd-surface-2); }
  .more p { margin: var(--cd-space-1) 0 0; font-size: var(--cd-text-xs); line-height: 1.5; color: var(--cd-ink-1); }

  .network-line {
    margin: 0;
    padding: var(--cd-space-2) var(--cd-space-3);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
  }

  .network-line.mainnet {
    border-color: var(--cd-danger-line);
    background: var(--cd-danger-dim);
    color: var(--cd-ink-1);
  }

  .network-line strong { color: var(--cd-ink); }
  .network-line.mainnet strong { color: var(--cd-danger-hi); }

  .network-line code {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-1);
    word-break: break-all;
  }

  /* FINDING 42. Louder than the custody notice on purpose: this one says the
     destination itself is wrong. Static, no z-index. */
  .untrusted-table {
    margin: 0;
    padding: var(--cd-space-3) var(--cd-space-4);
    border: 2px solid var(--cd-danger);
    border-radius: var(--cd-radius-card);
    background: var(--cd-danger-dim);
    color: var(--cd-ink-1);
    font-size: var(--cd-text-sm);
    line-height: 1.55;
  }

  .untrusted-table strong {
    display: block;
    margin-bottom: var(--cd-space-1);
    color: var(--cd-danger-hi);
  }

  /* The solvency block and the runway panel carry their own margins; inside
     this column the gap does the spacing. */
  .disclosures :global(.solvency),
  .disclosures :global(.runway-notice) { margin: 0; }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .more summary { min-height: var(--cd-touch-min); }
  }
</style>
