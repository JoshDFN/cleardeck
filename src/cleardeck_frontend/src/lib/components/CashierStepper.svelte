<script>
  // THE CASHIER'S STEPPER: what is happening, how far along it is, and what
  // is still to come, painted live while two chain commits take their
  // 1.35-1.75 s each. The model is lib/cashier-steps.js (pure, tested); this
  // owns the clock and asks it every frame.
  //
  // No numerals on the discs: a glyph (check, spinner, dot) says the state,
  // so nothing here is a numeric token the screenshot census would have to
  // excuse. The one figure it can show is a countdown on a step with a hard
  // deadline (the OISY popup wait), and that step exists on a mainnet build only.

  import { onMount } from 'svelte';
  import { FLOW, formatCountdown, progressAt, secondsLeft, stepStatus } from '../cashier-steps.js';

  const {
    /** @type {import('../cashier-steps.js').CashierStep[]} */
    steps = [],
    /** Index of the step the flow is on. */
    current = 0,
    /** A FLOW value. */
    phase = FLOW.IDLE,
    /** Date.now() when the current step began, or null. */
    startedAt = null,
    /** The failed step's message, shown under it. */
    failure = null,
  } = $props();

  let now = $state(Date.now());

  onMount(() => {
    let frame = 0;
    const tick = () => {
      now = Date.now();
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  });

  const statusOf = (i) => stepStatus(i, current, phase);

  const fillOf = (step, i) => (
    statusOf(i) === 'active' ? progressAt({ startedAt, now, expectedMs: step.expectedMs }) : 0
  );

  const countdownOf = (step, i) => (
    statusOf(i) === 'active' && step.deadlineMs
      ? formatCountdown(secondsLeft({ startedAt, now, deadlineMs: step.deadlineMs }))
      : null
  );
</script>

{#if steps.length}
  <ol class="cashier-steps" data-phase={phase} aria-live="polite">
    {#each steps as step, i (step.id)}
      {@const status = statusOf(i)}
      <li class="step" data-status={status}>
        <span class="disc" aria-hidden="true">
          {#if status === 'done'}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          {:else if status === 'active'}
            <span class="ring"></span>
          {:else if status === 'failed'}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>
          {:else}
            <span class="dot"></span>
          {/if}
        </span>
        <span class="body">
          <span class="title">
            {step.title}
            {#if countdownOf(step, i)}
              <span class="countdown">{countdownOf(step, i)}</span>
            {/if}
          </span>
          {#if status === 'active' && step.expectedMs}
            <span class="bar" role="progressbar" aria-label="{step.title}: in progress">
              <span class="fill" style="--fill: {fillOf(step, i)}"></span>
            </span>
          {/if}
          {#if status === 'failed' && failure}
            <span class="failure">{failure}</span>
          {:else if step.hint && (status === 'active' || status === 'pending')}
            <span class="hint">{step.hint}</span>
          {/if}
        </span>
      </li>
    {/each}
  </ol>
{/if}

<style>
  .cashier-steps {
    list-style: none;
    margin: 0;
    padding: var(--cd-space-3) var(--cd-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
  }

  .step {
    position: relative;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: var(--cd-space-3);
    align-items: start;
    padding-bottom: var(--cd-space-2);
  }

  /* The rail between discs. */
  .step:not(:last-child)::before {
    content: '';
    position: absolute;
    left: 10px;
    top: 24px;
    bottom: -2px;
    width: 2px;
    background: var(--cd-line);
  }

  .step[data-status='done']:not(:last-child)::before { background: var(--cd-accent-line-strong); }

  .disc {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid var(--cd-line-strong);
    background: var(--cd-sheet);
    color: var(--cd-ink-2);
  }

  .step[data-status='done'] .disc { border-color: var(--cd-accent); background: var(--cd-accent); color: var(--cd-accent-ink); }
  .step[data-status='active'] .disc { border-color: var(--cd-accent); color: var(--cd-accent); }
  .step[data-status='failed'] .disc { border-color: var(--cd-danger); background: var(--cd-danger-dim); color: var(--cd-danger-hi); }

  .ring {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--cd-accent-line);
    border-top-color: var(--cd-accent);
    animation: stepper-spin 0.8s linear infinite;
  }

  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--cd-line-bright); }

  .body { display: flex; flex-direction: column; gap: var(--cd-space-1); min-width: 0; padding-top: 2px; }

  .title {
    display: flex;
    justify-content: space-between;
    gap: var(--cd-space-3);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink-1);
  }

  .step[data-status='pending'] .title { color: var(--cd-ink-2); font-weight: var(--cd-weight-medium); }
  .step[data-status='done'] .title { color: var(--cd-ink-1); }
  .step[data-status='failed'] .title { color: var(--cd-danger-hi); }

  .countdown {
    font-variant-numeric: tabular-nums;
    color: var(--cd-accent);
  }

  .bar {
    display: block;
    height: 4px;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-surface-3);
    overflow: hidden;
  }

  .fill {
    display: block;
    height: 100%;
    width: calc(var(--fill, 0) * 100%);
    border-radius: inherit;
    background: linear-gradient(90deg, var(--cd-accent-hi), var(--cd-accent));
    transition: width var(--cd-fast) linear;
  }

  .hint, .failure {
    font-size: var(--cd-text-xs);
    line-height: 1.45;
    color: var(--cd-ink-2);
  }

  .failure { color: var(--cd-danger-hi); }

  @keyframes stepper-spin { to { transform: rotate(360deg); } }

  @media (prefers-reduced-motion: reduce) {
    .ring { animation: none; }
    .fill { transition: none; }
  }
</style>
