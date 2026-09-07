<script>
  /**
   * What this proves, and what it does not. Every sentence verbatim from the
   * fairness panel (docs/SHUFFLE-SPEC.md section 0); only the file moved.
   */
  const { proof, formatTimestamp = () => '' } = $props();
</script>

<div class="limits">
  <div class="limit proven">
    <span class="limit-tag">Proven</span>
    <p><strong>The whole 52-card order was fixed before the board was shown.</strong> The cards you
      were dealt follow from the seed the table committed to; that seed fixes all 52 positions, and
      every card you saw this hand sits exactly where it puts one. Checked here, on your machine,
      from the cards on your screen. Copy the hash while a hand is still running and you can
      predict the turn and the river from the seed revealed at the end.</p>
  </div>
  <div class="limit proven">
    <span class="limit-tag">Proven</span>
    <p>The deck could not be changed after that commitment existed. Any other card at any other
      position would give a different SHA-256, and the one in step 1 is the one that was published.</p>
  </div>
  <div class="limit not-proven">
    <span class="limit-tag">Not proven</span>
    <p><strong>That the commitment came before the cards.</strong> This page reads
      <code>{formatTimestamp(proof.timestamp)}</code> off the table canister; it did not watch the
      order of events. Everything above stays true even if that clock is wrong.
      <strong>You can witness it yourself:</strong> copy the commitment from step 1 while a hand is
      still running, it is on screen from the moment cards are dealt, and check it against the one
      shown here after the seed is revealed. Then the "before" is something you saw, not something
      we told you.</p>
  </div>
  <div class="limit not-proven">
    <span class="limit-tag">Not proven</span>
    <p>That the seed was unpredictable. It comes from the Internet Computer's <code>raw_rand</code>,
      so you are trusting the subnet's randomness, not arithmetic.</p>
  </div>
  <div class="limit not-proven">
    <span class="limit-tag">Not proven</span>
    <p>That the money went to the right player. Settlement is a separate question and this panel says
      nothing about it.</p>
  </div>
  <div class="limit not-proven">
    <span class="limit-tag">Not proven</span>
    <p>That the rest of the engine is correct. Verifying a shuffle is not an audit. This is unaudited
      alpha software with known defects, listed in <code>docs/DEFECTS.md</code>.</p>
  </div>
</div>

<style>
  .limits { display: flex; flex-direction: column; gap: var(--cd-space-2); }

  .limit {
    display: grid;
    grid-template-columns: 84px 1fr;
    gap: var(--cd-space-2);
    align-items: start;
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line-soft);
  }

  .limit p { margin: 0; font-size: var(--cd-text-sm); line-height: 1.6; color: var(--cd-ink-2); }
  .limit p strong { color: var(--cd-ink-1); }
  .limit p code { color: var(--cd-accent); font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); }

  .limit-tag {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 3px 7px;
    border-radius: 5px;
    text-align: center;
  }

  .limit.proven .limit-tag { background: var(--cd-accent-dim); color: var(--cd-accent); }
  .limit.not-proven .limit-tag { background: var(--cd-warn-dim); color: var(--cd-warn); }

  @media (max-width: 500px) {
    .limit { grid-template-columns: 1fr; }
    .limit-tag { justify-self: start; }
  }
</style>
