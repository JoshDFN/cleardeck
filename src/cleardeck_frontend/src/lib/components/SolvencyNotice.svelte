<script>
  // WHAT THIS TABLE ACTUALLY HOLDS, NEXT TO WHAT IT SAYS IT OWES.
  //
  // docs/SECURITY-FINDINGS.md FINDING 35 / docs/DEFECTS.md E-70. On mainnet
  // table_1 the two numbers differ by 2.00 ICP and no surface of the canister
  // can say so. This block is the surface.
  //
  // THREE RULES IT FOLLOWS, ALL OF THEM LOAD-BEARING.
  //
  //  1. IT IS IN FLOW, NEVER AN OVERLAY. HARD RULE 2 says the four protected
  //     notices must be on screen and legible at every viewport on every view.
  //     A fixed-position solvency banner would be one more thing that can paint
  //     over them, which is docs/DEFECTS.md E-52 again with a different colour.
  //     So this renders as an ordinary block, in the document flow, and cannot
  //     occlude anything by construction.
  //
  //  2. SILENCE IS NEVER THE ANSWER. Every state except `covered` renders. A
  //     canister with no solvency surface renders the loudest of them, because
  //     "cannot tell you" is the true state of today's deployment and the whole
  //     finding is that it currently looks identical to "fine".
  //
  //  3. IT NAMES THE NUMBERS. "Something may be wrong" is not actionable. When
  //     the canister gives figures, the rounded token figure stands on the row
  //     and the exact e8s integer stands once under "What the table said",
  //     because a player reconciling a balance needs the exact integer and
  //     `formatTokenAmount` rounds to four decimals, while a depositor reading
  //     the row needs one figure, not two spellings of it.

  import {
    SOLVENCY_STATES, headlineFor, observationAge, severityOf,
  } from '../solvency.js';
  import { formatTokenAmount } from '../utils.js';

  const {
    /** Result of `readTableSolvency()`, or null while it is in flight. */
    solvency = null,
    currency = 'ICP',
    /** 'deposit' tightens the copy for the screen money is committed from. */
    context = 'table',
    /** Optional callback wired to the canister's refresh update, when it has one. */
    onRefresh = null,
    refreshing = false,
  } = $props();

  const state = $derived(solvency?.state ?? null);
  const severity = $derived(state ? severityOf(state) : 'ok');
  const visible = $derived(Boolean(state) && state !== SOLVENCY_STATES.COVERED);

  // Both spellings of every figure: the rounded one a person reads on the row,
  // and the exact integer someone reconciling a balance needs, under the
  // disclosure (`formatTokenAmount` rounds to four decimals).
  //
  // NOTE FOR THE HARNESS. These are MONEY FIGURES, asserted against
  // get_solvency() by `tools/shots/lib/chain-agreement.mjs` (the row by its
  // label, the exact figure by its `data-exact` id), never excused by a rule in
  // `token-allowlist.mjs`, which that file's own rule 5 forbids for money-shaped
  // tokens.
  const amount = (v) =>
    v === null || v === undefined ? null : formatTokenAmount(v, { currency, includeUnit: true });
  const exact = (v) => (v === null || v === undefined ? null : `${v.toString()} e8s`);

  const shortfallText = $derived(amount(solvency?.shortfall ?? null));
  const heldText = $derived(amount(solvency?.held ?? null));
  const owedText = $derived(amount(solvency?.owed ?? null));

  /** The exact integers, once, under the disclosure. */
  const exactRows = $derived([
    { id: 'owed', label: 'Owed to players', text: exact(solvency?.owed ?? null) },
    { id: 'held', label: 'Held on the ledger', text: exact(solvency?.held ?? null) },
    { id: 'shortfall', label: 'Short by', text: exact(solvency?.shortfall ?? null) },
  ].filter((r) => r.text !== null));

  const age = $derived(
    solvency && solvency.observedAtNs !== undefined
      ? observationAge(solvency.observedAtNs)
      : 'never taken',
  );

  /** The one sentence that tells the player what to DO, per state. */
  const action = $derived.by(() => {
    switch (state) {
      case SOLVENCY_STATES.SHORT:
        return context === 'deposit'
          ? 'Do not deposit. Money added now joins a pool that is already short, and a '
            + 'withdrawal is paid out of that same pool, first come first served.'
          : 'Withdraw what you can and do not add more. A shortfall is paid out first '
            + 'come first served, so being early is the only protection there is.';
      case SOLVENCY_STATES.UNSUPPORTED:
        return context === 'deposit'
          ? 'Treat this as a reason not to deposit. An unverifiable custodian is not the '
            + 'same as a verified one, and this page will not pretend otherwise.'
          : 'Nothing here can confirm your balance is backed by anything on the ledger.';
      case SOLVENCY_STATES.UNKNOWN:
        return 'Ask it to check before you commit money. An unread account is not an empty '
          + 'one and it is not a full one either.';
      case SOLVENCY_STATES.ERROR:
        return 'The reading failed, so this is unknown rather than fine. Try again before '
          + 'you commit money.';
      default:
        return '';
    }
  });
</script>

{#if visible}
  <section
    class="solvency"
    class:critical={severity === 'critical'}
    class:warning={severity === 'warning'}
    data-solvency-state={state}
    aria-live="polite"
  >
    <p class="headline">
      <span class="mark" aria-hidden="true">{severity === 'critical' ? '⛔' : '⚠️'}</span>
      <strong>{headlineFor(state)}</strong>
    </p>

    {#if shortfallText || heldText || owedText}
      <dl class="figures">
        {#if owedText}
          <div><dt>Owed to players</dt><dd>{owedText}</dd></div>
        {/if}
        {#if heldText}
          <div><dt>Held on the ledger</dt><dd>{heldText}</dd></div>
        {/if}
        {#if shortfallText}
          <div class="gap"><dt>Short by</dt><dd>{shortfallText}</dd></div>
        {/if}
      </dl>
      <p class="asof">Ledger reading {age}.</p>
    {/if}

    <p class="action">{action}</p>

    <!-- The canister's own sentence, verbatim, under a disclosure: the
         headline, the figures and the action above are the summary a player
         reads; this is the detail. Its figures are still read by
         tools/shots/lib/chain-agreement.mjs (textContent, open or closed) and
         asserted against get_solvency(). -->
    {#if solvency?.advice || exactRows.length}
      <details class="more">
        <summary>What the table said</summary>
        {#if solvency?.advice}<p class="advice">{solvency.advice}</p>{/if}
        {#if exactRows.length}
          <dl class="exact">
            {#each exactRows as row (row.id)}
              <div data-exact={row.id}><dt>{row.label}, exactly</dt><dd>{row.text}</dd></div>
            {/each}
          </dl>
        {/if}
      </details>
    {/if}

    {#if onRefresh && solvency?.canRefresh}
      <button class="refresh" onclick={onRefresh} disabled={refreshing}>
        {refreshing ? 'Asking the ledger…' : 'Ask this table to re-read the ledger now'}
      </button>
    {/if}
  </section>
{/if}

<style>
  /* In flow. No `position: fixed`, no z-index: this block must never be able to
     paint over the five protected notices (HARD RULE 2). The same tokens as
     the trust bar: amber for a warning, the danger red only for a shortfall. */
  .solvency {
    margin: 0 0 var(--cd-space-4);
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    border: 1px solid var(--cd-warn-line);
    border-left-width: 3px;
    background: var(--cd-warn-dim);
    font-size: var(--cd-text-sm);
    line-height: 1.5;
    color: var(--cd-ink-1);
  }

  .solvency.critical {
    border-color: var(--cd-danger-line);
    border-left-color: var(--cd-danger);
    background: var(--cd-danger-dim);
  }

  .headline {
    display: flex;
    gap: var(--cd-space-2);
    align-items: baseline;
    margin: 0 0 var(--cd-space-2);
  }

  .headline strong {
    font-size: var(--cd-text-sm);
    color: var(--cd-warn);
  }

  .solvency.critical .headline strong { color: var(--cd-danger-hi); }

  .mark { flex-shrink: 0; }

  .figures {
    display: grid;
    gap: 2px;
    margin: 0 0 var(--cd-space-1);
    font-variant-numeric: tabular-nums;
  }

  .figures div {
    display: flex;
    justify-content: space-between;
    gap: var(--cd-space-3);
  }

  .figures dt {
    margin: 0;
    color: var(--cd-ink-2);
  }

  .figures dd {
    margin: 0;
    text-align: right;
    color: var(--cd-ink);
  }

  .figures .gap dd { color: var(--cd-danger-hi); font-weight: var(--cd-weight-figure); }

  .asof,
  .advice,
  .action {
    margin: 0 0 var(--cd-space-1);
  }

  .asof { color: var(--cd-ink-2); font-size: var(--cd-text-xs); }

  .action {
    margin-bottom: 0;
    color: var(--cd-ink);
    font-weight: var(--cd-weight-strong);
  }

  .more { margin-top: 0; }

  .more summary {
    display: inline-flex;
    align-items: center;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    margin-left: calc(-1 * var(--cd-space-2));
    border-radius: var(--cd-radius-chip);
    color: var(--cd-warn);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    list-style: none;
    cursor: pointer;
  }

  .solvency.critical .more summary { color: var(--cd-danger-hi); }
  .more summary::-webkit-details-marker { display: none; }
  .more summary::after { content: ' \25BE'; }
  .more[open] summary::after { content: ' \25B4'; }
  .more summary:hover { background: var(--cd-surface-2); }
  .more .advice { margin: var(--cd-space-1) 0 0; color: var(--cd-ink-1); }

  .exact {
    display: grid;
    gap: 2px;
    margin: var(--cd-space-2) 0 0;
    font-size: var(--cd-text-xs);
    font-variant-numeric: tabular-nums;
  }

  .exact div { display: flex; justify-content: space-between; gap: var(--cd-space-3); }
  .exact dt { margin: 0; color: var(--cd-ink-2); }
  .exact dd { margin: 0; font-family: var(--cd-font-mono); color: var(--cd-ink-1); }

  .refresh {
    margin-top: var(--cd-space-2);
    width: 100%;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink);
    font: inherit;
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  .refresh:disabled { opacity: 0.6; cursor: default; }
  .refresh:hover:not(:disabled) { background: var(--cd-surface-3); }

  /* THE PHONE: the refresh control at the 44 px touch floor. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .refresh { min-height: var(--cd-touch-min); }
    .more summary { min-height: var(--cd-touch-min); }
  }
</style>
