<script>
  // THE FIRST SCREEN'S ONE PARAGRAPH: what this is, how to start.
  //
  // A stranger gets three things in the first five seconds: the name of the
  // game and where it runs, the three claims that make it different (no
  // rake, a committed deck, contracts you can query), and the one way in.
  // The way in is Internet Identity, explained in a sentence, because the
  // audit found "Connect Wallet" set a MetaMask expectation the flow cannot
  // meet and told nobody that a passkey is all it takes.
  //
  // Signed in, the same band collapses to the three claims as chips.

  const {
    signedIn = false,
    signingIn = false,
    /** A sign-in failure to show under the button, or null. */
    error = null,
    onSignIn = () => {},
    onHow = () => {},
  } = $props();
</script>

{#if !signedIn}
  <section class="intro" aria-label="About ClearDeck">
    <div class="intro-copy">
      <p class="eyebrow">No Limit Hold'em on the Internet Computer</p>
      <h1>Poker you can check.</h1>
      <p class="intro-lead">
        Every table is a contract on-chain, the deck is committed before the deal
        and revealed after it, and the pot is never raked.
      </p>
      <ul class="claims" aria-label="What makes this different">
        <li><span class="claim-figure">0%</span> rake</li>
        <li><span class="claim-figure">SHA-256</span> committed deck</li>
        <li><span class="claim-figure">On-chain</span> contracts</li>
        <li><button class="link-btn" onclick={onHow}>How it works</button></li>
      </ul>
    </div>

    <div class="intro-cta">
      <button class="btn primary" onclick={onSignIn} disabled={signingIn}>
        {#if signingIn}
          Opening Internet Identity…
        {:else}
          Sign in with Internet Identity
        {/if}
      </button>
      <p class="cta-hint">
        <span class="hint-wide">A passkey or Google sign-in, no app and no wallet.</span>
        <span class="hint-phone">Sign in (top right) with a passkey or Google, no app and no wallet.</span>
        Nothing to sign in for just to look: pick a table below and watch.
        <button class="link-btn hint-phone" onclick={onHow}>How it works</button>
      </p>
      {#if error}
        <p class="intro-error">{error}</p>
      {/if}
    </div>
  </section>
{:else}
  <div class="intro-slim">
    <span class="slim-chip strong">0% rake</span>
    <span class="slim-chip">Committed deck</span>
    <span class="slim-chip">On-chain settlement</span>
    <button class="link-btn" onclick={onHow}>How it works</button>
  </div>
{/if}

<style>
  .intro {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(280px, 0.8fr);
    gap: var(--cd-space-5) var(--cd-space-6);
    align-items: center;
    padding: var(--cd-space-3) var(--cd-space-5);
    margin: 0 0 var(--cd-space-3);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-panel);
    background:
      radial-gradient(120% 160% at 0% 0%, var(--cd-accent-dim), transparent 55%),
      var(--cd-surface-1);
  }

  .eyebrow {
    margin: 0 0 var(--cd-space-1);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .hint-phone { display: none; }

  h1 {
    margin: 0 0 var(--cd-space-1);
    font-size: 26px;
    line-height: 1.1;
    font-weight: var(--cd-weight-figure);
    letter-spacing: -0.025em;
    color: var(--cd-ink);
  }

  .intro-lead {
    margin: 0 0 var(--cd-space-2);
    max-width: 60ch;
    font-size: var(--cd-text-md);
    line-height: 1.5;
    color: var(--cd-ink-1);
  }

  .claims {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: var(--cd-space-1) var(--cd-space-4);
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
  }

  .claims li { display: inline-flex; align-items: baseline; gap: 5px; }

  .claim-figure {
    font-weight: var(--cd-weight-figure);
    color: var(--cd-accent-hi);
    font-variant-numeric: tabular-nums;
  }

  .intro-cta {
    display: grid;
    gap: var(--cd-space-2);
    justify-items: stretch;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 48px;
    padding: 0 var(--cd-space-5);
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
    font: inherit;
    font-size: 15px;
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    transition: transform var(--cd-fast) var(--cd-ease), background var(--cd-fast) var(--cd-ease);
  }

  .btn.primary {
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.18) inset;
  }

  .btn.primary:hover:not(:disabled) { background: var(--cd-accent-hi); transform: translateY(-1px); }
  .btn:disabled { opacity: 0.7; cursor: progress; }

  .cta-hint {
    margin: 0;
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
  }

  .intro-error {
    margin: 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-danger-hi);
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: inherit;
    color: var(--cd-accent);
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
  }

  /* Signed in: the claims as chips, on one quiet row. */
  .intro-slim {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--cd-space-2) var(--cd-space-3);
    margin: 0 0 var(--cd-space-3);
    padding: 0 var(--cd-space-1);
    font-size: var(--cd-text-xs);
  }

  .slim-chip {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--cd-accent);
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-pill);
    padding: 4px 10px;
  }

  .slim-chip.strong { color: var(--cd-accent-hi); border-color: var(--cd-accent-line-strong); }

  @media (max-width: 1080px) {
    .intro { grid-template-columns: minmax(0, 1fr); gap: var(--cd-space-3); }
  }

  /* The phone: the sentence and the claims; the sign-in button is the one in
     the header, 44 px and in view, so it is not repeated here. */
  @media (max-width: 760px) {
    .intro {
      padding: var(--cd-space-3) var(--cd-space-4);
      margin-bottom: var(--cd-space-2);
      border-radius: var(--cd-radius-card);
    }

    .eyebrow { display: none; }
    h1 { font-size: 22px; }
    .intro-lead { font-size: var(--cd-text-sm); margin-bottom: 0; }
    /* The lead already states the three claims; the row would repeat it. */
    .claims { display: none; }
    .intro-cta { gap: 0; }
    .intro-cta .btn { display: none; }
    .hint-wide { display: none; }
    .hint-phone { display: inline; }
    /* A 44 px target inside the sentence: the negative margins keep the line
       box the height of the text; only the hit area grew. */
    .link-btn.hint-phone {
      display: inline-flex;
      align-items: center;
      min-height: var(--cd-touch-min);
      margin: calc(-1 * var(--cd-space-3)) 0 calc(-1 * var(--cd-space-3)) var(--cd-space-1);
      vertical-align: middle;
    }
  }
</style>
