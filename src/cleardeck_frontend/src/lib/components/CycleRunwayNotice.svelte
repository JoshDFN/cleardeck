<script>
  // HOW LONG THIS TABLE CAN KEEP HONOURING WITHDRAWALS, SAID OUT LOUD.
  //
  // docs/DEFECTS.md E-55, docs/SECURITY-FINDINGS.md FINDING 19 / 24 / 26.
  //
  // THREE RULES, TAKEN VERBATIM FROM SolvencyNotice.svelte BECAUSE THEY ARE THE
  // RIGHT ONES AND THIS IS THE SAME CLASS OF WARNING.
  //
  //  1. IT IS IN FLOW, NEVER AN OVERLAY. HARD RULE 2: the unaudited-alpha
  //     disclaimer, the 18+ notice, the jurisdiction warning and the no-rake
  //     property must be on screen and legible at every viewport on every view,
  //     and the gate is measured on rendered pixels. A fixed-position banner is
  //     one more thing that can paint over them (docs/DEFECTS.md E-52). This is
  //     an ordinary block element with no `position`, no `z-index` and no
  //     transform, so it cannot occlude anything by construction.
  //
  //  2. SILENCE IS NEVER THE ANSWER. Every state except `ok` renders, including
  //     "this table did not answer" and "this table cannot say". A canister that
  //     has run out of cycles rejects QUERIES too (FINDING 24, reproduced in
  //     tests/money_safety/tests/cycles_runway.rs), so the read failing IS the
  //     alarm and is rendered as the loudest state there is.
  //
  //  3. IT NAMES THE NUMBERS AND THE COMMAND. "Somebody should top this up" is
  //     not actionable. `deposit-cycles` on the IC needs no controller rights and
  //     is open to any principal, so the exact command is shown: anybody reading
  //     this can be the person who fixes it.

  import {
    RUNWAY_STATES, severityOf, headlineFor, adviceFor, formatCycles, topUpHint,
    WARN_DAYS,
  } from '../cycleRunway.js';

  const {
    /** Result of `readCycleRunway()`, or null while it is in flight. */
    runway = null,
    /** 'deposit' tightens the copy for the screen money is committed from. */
    context = 'table',
    /** Shown with the top-up command so it can be copied. */
    canisterId = null,
  } = $props();

  const state = $derived(runway?.state ?? null);
  const severity = $derived(state ? severityOf(state) : 'ok');
  const visible = $derived(Boolean(state) && state !== RUNWAY_STATES.OK);
  const headline = $derived(state ? headlineFor(state, runway?.days ?? null) : '');
  const advice = $derived(state ? adviceFor(state, context) : '');

  // The operator-facing detail line. Rendered only when the canister gave
  // figures, and never as a substitute for the headline.
  const detail = $derived.by(() => {
    if (!runway) return null;
    const bits = [];
    if (runway.liquid !== null && runway.liquid !== undefined) {
      bits.push(`${formatCycles(runway.liquid)} spendable`);
    }
    if (runway.burnPerDay) {
      bits.push(`burning ${formatCycles(runway.burnPerDay)}/day`);
    }
    if (runway.measured === false && runway.days !== null) {
      bits.push('burn rate not yet measured over a full window');
    }
    return bits.length ? bits.join(' · ') : null;
  });
</script>

{#if visible}
  <div
    class="runway-notice {severity}"
    data-testid="cycle-runway-notice"
    data-runway-state={state}
    data-runway-days={runway?.days ?? ''}
    role={severity === 'danger' ? 'alert' : 'status'}
  >
    <p class="headline">{headline}</p>
    <p class="advice">{advice}</p>
    {#if detail}
      <p class="detail">{detail}</p>
    {/if}
    <p class="topup">
      Anyone can top this canister up. It needs no permission and no controller:
      <code>{topUpHint(canisterId)}</code>
    </p>
    {#if state === RUNWAY_STATES.LOW || state === RUNWAY_STATES.CRITICAL}
      <p class="detail">
        Warning threshold is {WARN_DAYS} days of measured runway. There is no automatic
        top-up anywhere in this application.
      </p>
    {/if}
  </div>
{/if}

<style>
  /* NO position, NO z-index, NO transform. See rule 1 in the script block: this
     element must be incapable of covering the protected notices, and the
     cheapest way to guarantee that is to give it nothing to cover them with.
     The same tokens as the solvency block and the trust bar. */
  .runway-notice {
    margin: var(--cd-space-3) 0;
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    border: 1px solid;
    border-left-width: 3px;
    font-size: var(--cd-text-sm);
    line-height: 1.45;
    max-width: 100%;
    overflow-wrap: anywhere;
    color: var(--cd-ink-1);
  }

  .runway-notice.warn {
    background: var(--cd-warn-dim);
    border-color: var(--cd-warn-line);
    border-left-color: var(--cd-warn);
  }

  .runway-notice.danger {
    background: var(--cd-danger-dim);
    border-color: var(--cd-danger-line);
    border-left-color: var(--cd-danger);
  }

  .headline {
    margin: 0 0 var(--cd-space-1);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-warn);
  }

  .runway-notice.danger .headline { color: var(--cd-danger-hi); }

  .advice {
    margin: 0 0 var(--cd-space-1);
  }

  .detail,
  .topup {
    margin: var(--cd-space-1) 0 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
  }

  code {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    padding: 0.1rem 0.3rem;
    border-radius: var(--cd-space-1);
    background: var(--cd-capsule);
    color: var(--cd-ink-1);
  }
</style>
