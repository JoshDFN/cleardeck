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
  //     the canister gives figures, they are shown in full e8s as well as
  //     rounded tokens, because a player reconciling a balance needs the exact
  //     integer and `formatTokenAmount` rounds to four decimals.

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

  // Both spellings of every figure: the rounded one a person reads, and the exact
  // integer someone reconciling a balance needs (`formatTokenAmount` rounds to
  // four decimals).
  //
  // NOTE FOR THE HARNESS. These are MONEY FIGURES. The moment the canister half
  // of FINDING 35 lands and these render with real numbers, they need a site in
  // `tools/shots/lib/chain-agreement.mjs` asserting them against the canister --
  // NOT a rule in `token-allowlist.mjs`, which that file's own rule 5 forbids for
  // money-shaped tokens. Today they never render (the state is `unsupported`,
  // which carries no figures), so there is nothing yet to assert.
  const amount = (v) =>
    v === null || v === undefined
      ? null
      : `${formatTokenAmount(v, { currency, includeUnit: true })} (${v.toString()} e8s)`;

  const shortfallText = $derived(amount(solvency?.shortfall ?? null));
  const heldText = $derived(amount(solvency?.held ?? null));
  const owedText = $derived(amount(solvency?.owed ?? null));

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

    {#if solvency?.advice}
      <p class="advice">{solvency.advice}</p>
    {/if}

    <p class="action">{action}</p>

    {#if onRefresh && solvency?.canRefresh}
      <button class="refresh" onclick={onRefresh} disabled={refreshing}>
        {refreshing ? 'Asking the ledger…' : 'Ask this table to re-read the ledger now'}
      </button>
    {/if}
  </section>
{/if}

<style>
  /* In flow. No `position: fixed`, no z-index: this block must never be able to
     paint over the four protected notices (HARD RULE 2). */
  .solvency {
    margin: 0 0 14px 0;
    padding: 12px 14px;
    border-radius: 10px;
    border: 1px solid rgba(240, 180, 41, 0.35);
    background: rgba(240, 180, 41, 0.1);
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.82);
  }

  .solvency.critical {
    border-color: rgba(248, 113, 113, 0.55);
    background: rgba(185, 28, 28, 0.18);
  }

  .headline {
    display: flex;
    gap: 8px;
    align-items: baseline;
    margin: 0 0 8px 0;
  }

  .headline strong {
    font-size: 13.5px;
    color: #f0b429;
  }

  .solvency.critical .headline strong { color: #fca5a5; }

  .mark { flex-shrink: 0; }

  .figures {
    display: grid;
    gap: 4px;
    margin: 0 0 6px 0;
    font-variant-numeric: tabular-nums;
  }

  .figures div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .figures dt {
    margin: 0;
    color: rgba(255, 255, 255, 0.6);
  }

  .figures dd {
    margin: 0;
    text-align: right;
    color: #fff;
  }

  .figures .gap dd { color: #fca5a5; font-weight: 700; }

  .asof,
  .advice,
  .action {
    margin: 0 0 6px 0;
  }

  .asof { color: rgba(255, 255, 255, 0.55); font-size: 11.5px; }

  .action {
    margin-bottom: 0;
    color: #fff;
    font-weight: 600;
  }

  .refresh {
    margin-top: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .refresh:disabled { opacity: 0.6; cursor: default; }
  .refresh:hover:not(:disabled) { background: rgba(255, 255, 255, 0.12); }
</style>
