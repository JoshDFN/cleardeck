<script>
  // THREE STEPS TO A SEAT, under the list.
  //
  // At 1440x900 the list column (three cards and its footer) ended ~200 px
  // above the foot of the sticky preview beside it, and the one line that
  // explained the game was an orphan centred under both. This strip gives
  // that space the thing a first-time visitor still lacks after the hero:
  // what happens between Sign in and a dealt hand, in three short steps,
  // with the game named. It renders on the two-column desktop board only;
  // the phone's hero hint already says how to start and every pixel under
  // the fold there is scroll.
  //
  // STATIC COPY, no live state: the numerals ("1", "2", "3", "256") are
  // excused by the census's static-explainer rule (tools/shots/
  // token-allowlist.mjs, selector `.lobby-steps`), which is why the wrapper
  // class is load-bearing.

  const { onHow = () => {} } = $props();

  const STEPS = [
    {
      title: 'Sign in',
      body: 'Internet Identity: a passkey or Google, no app and no wallet. Watching needs no sign-in.',
    },
    {
      title: 'Deposit ICP',
      body: 'From any wallet, into the table contract. The same cashier pays it back out.',
    },
    {
      title: 'Sit',
      body: 'The deck is committed with SHA-256 before the first card. Check any hand from the history.',
    },
  ];
</script>

<section class="lobby-steps" aria-label="Three steps to a seat">
  <header class="steps-head">
    <h3>Texas Hold'em No Limit, three steps to a seat</h3>
    <button class="link-btn" type="button" onclick={onHow}>How it works</button>
  </header>
  <ol class="steps">
    {#each STEPS as step, i (step.title)}
      <li class="step">
        <span class="step-number" aria-hidden="true">{i + 1}</span>
        <div class="step-body">
          <strong>{step.title}</strong>
          <p>{step.body}</p>
        </div>
      </li>
    {/each}
  </ol>
</section>

<style>
  .lobby-steps {
    display: none;
    margin: var(--cd-space-4) 0 0;
    padding: var(--cd-space-4);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
  }

  /* The two-column board only (Lobby.svelte collapses it under 1081 px). */
  @media (min-width: 1081px) {
    .lobby-steps { display: block; }
  }

  .steps-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--cd-space-3);
    margin: 0 0 var(--cd-space-3);
  }

  .steps-head h3 {
    margin: 0;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink-1);
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--cd-accent);
    font: inherit;
    font-size: var(--cd-text-sm);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 3px;
    white-space: nowrap;
  }

  .link-btn:hover { color: var(--cd-accent-hi); }

  .steps {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--cd-space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .step {
    display: flex;
    gap: var(--cd-space-3);
    min-width: 0;
  }

  .step-number {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--cd-icon-xl);
    height: var(--cd-icon-xl);
    border-radius: 50%;
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
    color: var(--cd-accent);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
  }

  .step-body { min-width: 0; }

  .step-body strong {
    display: block;
    margin: calc(var(--cd-space-1) / 2) 0 var(--cd-space-1);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
  }

  .step-body p {
    margin: 0;
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
  }
</style>
