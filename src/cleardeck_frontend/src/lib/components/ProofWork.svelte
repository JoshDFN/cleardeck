<script>
  /**
   * The work: the whole deck as this browser built it, the first three shuffle
   * steps in full, and what the table's own verifier says (labelled as proving
   * nothing). Verbatim from the fairness panel; only the file moved.
   *
   * HARNESS CONTRACT (shuffleproof.mjs): `.deck-grid .slot` x52.
   */
  const { report, houseEcho = null } = $props();

  /** Deck indices worth colouring in the deck grid. */
  const marks = $derived.by(() => {
    const map = new Map();
    const layout = report?.layout;
    if (!layout) return map;
    layout.positions.holes.forEach(([a, b], k) => {
      const own = k === layout.dealIndex;
      map.set(a, own ? 'mine' : 'other');
      map.set(b, own ? 'mine' : 'other');
    });
    map.set(layout.positions.burnBeforeFlop, 'burn');
    map.set(layout.positions.burnBeforeTurn, 'burn');
    map.set(layout.positions.burnBeforeRiver, 'burn');
    (layout.boardPositions || []).forEach((p) => map.set(p, 'board'));
    return map;
  });
</script>

<div class="work">
  <h5>The whole deck, as this browser built it</h5>
  <p class="work-note">
    52 cards from one seed. Gold is yours, teal is the board, grey struck-through cards were
    burned, and faint cards went to other seats and were never shown to you.
  </p>
  <div class="deck-grid">
    {#each report.deckCodes as code, index}
      <span class="slot {marks.get(index) || 'unused'}">
        <span class="slot-index">{index}</span>
        <span class="slot-code">{code}</span>
      </span>
    {/each}
  </div>

  <h5>The first three shuffle steps, in full</h5>
  <p class="work-note">
    Each step hashes the running chain with a counter byte, reads the first 8 bytes as a
    little-endian 64-bit number, reduces it modulo the remaining deck size, and swaps.
    The whole algorithm is 51 repeats of this; the spec is in docs/SHUFFLE-SPEC.md.
  </p>
  <div class="steps">
    {#each report.trace as step}
      <div class="step">
        <div class="step-head">
          <span class="step-n">step {step.step}</span>
          <span>SHA-256(chain ‖ 0x{step.counterByte.toString(16).padStart(2, '0')})</span>
        </div>
        <code class="chain">{step.chainHex}</code>
        <div class="step-math">
          <span>first 8 bytes <code>{step.drawBytesHex}</code></span>
          <span>→ {step.draw}</span>
          <span>mod {step.n} = <strong>{step.j}</strong></span>
          <span class="swap">swap deck[{step.i}] ↔ deck[{step.j}]</span>
        </div>
      </div>
    {/each}
  </div>

  <h5>What the table's own verifier says</h5>
  <div class="house-echo">
    <p class="work-note">
      The table canister exposes a <code>check_shuffle_commitment</code> query. It recomputes the
      same SHA-256 and answers <code>Match</code>. <strong>That answer is worth nothing</strong>:
      it is the house checking its own homework, on the house's machine, and it never touches a
      card. It is shown here only so the difference is visible. Nothing above depends on it.
    </p>
    <div class="echo-row">
      <span class="echo-label">seed_hash = hash, revealed_seed = seed</span>
      {#if houseEcho?.state === 'answered'}
        <span class="echo-value">{houseEcho.straight}: proves only that the canister can hash</span>
      {:else if houseEcho?.state === 'unreachable'}
        <span class="echo-value muted">unreachable, and the check above still passed without it</span>
      {:else if houseEcho?.state === 'asking'}
        <span class="echo-value muted">asking…</span>
      {:else}
        <span class="echo-value muted">not asked</span>
      {/if}
    </div>
    <div class="echo-row">
      <span class="echo-label">the two values transposed</span>
      {#if houseEcho?.state === 'answered'}
        <span class="echo-value">{houseEcho.swapped}: the same proof, and it says so</span>
      {:else}
        <span class="echo-value muted">·</span>
      {/if}
    </div>
    <p class="work-note echo-history">
      Both rows are here on purpose. The endpoint this replaced took two unnamed hex strings and
      returned a bare <code>true</code>/<code>false</code>, so passing them the way a reader
      naturally would got you <code>false</code> for a perfectly good proof, and
      <em>"you called it backwards"</em> and <em>"you were cheated"</em> were the same answer.
    </p>
  </div>
</div>

<style>
  .work { margin-top: var(--cd-space-3); }

  .work h5 {
    margin: var(--cd-space-4) 0 var(--cd-space-1);
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
  }

  .work h5:first-child { margin-top: 0; }

  .work-note { margin: 0 0 var(--cd-space-2); font-size: var(--cd-text-xs); line-height: 1.6; color: var(--cd-ink-2); }
  .work-note code { color: var(--cd-accent); font-family: var(--cd-font-mono); }
  .work-note strong { color: var(--cd-ink-1); }

  .deck-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(46px, 1fr));
    gap: 3px;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 3px 2px;
    border-radius: 5px;
    font-family: var(--cd-font-mono);
    border: 1px solid transparent;
    background: var(--cd-surface-1);
  }

  .slot-index { font-size: 8px; color: var(--cd-ink-2); }
  .slot-code { font-size: var(--cd-text-xs); font-weight: var(--cd-weight-figure); color: var(--cd-ink-2); }
  .slot.unused, .slot.other { opacity: 0.45; }
  .slot.mine { background: var(--cd-money-dim); border-color: var(--cd-money-line); }
  .slot.mine .slot-code { color: var(--cd-money); }
  .slot.board { background: var(--cd-accent-dim); border-color: var(--cd-accent-line); }
  .slot.board .slot-code { color: var(--cd-accent); }
  .slot.burn { opacity: 0.6; }
  .slot.burn .slot-code { text-decoration: line-through; }

  .steps { display: flex; flex-direction: column; gap: var(--cd-space-2); }

  .step {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-bg-deep);
    border: 1px solid var(--cd-line-soft);
  }

  .step-head { display: flex; gap: var(--cd-space-2); align-items: baseline; flex-wrap: wrap; font-size: var(--cd-text-xs); color: var(--cd-ink-2); margin-bottom: var(--cd-space-1); }
  .step-n { color: var(--cd-accent); font-weight: var(--cd-weight-figure); text-transform: uppercase; letter-spacing: 0.05em; }

  .chain {
    display: block;
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
    word-break: break-all;
    margin-bottom: var(--cd-space-1);
  }

  .step-math { display: flex; flex-wrap: wrap; gap: var(--cd-space-1) var(--cd-space-3); font-size: var(--cd-text-xs); color: var(--cd-ink-2); align-items: baseline; }
  .step-math code { color: var(--cd-money); font-family: var(--cd-font-mono); }
  .step-math strong { color: var(--cd-ink); }
  .step-math .swap { color: var(--cd-accent); }

  .house-echo {
    padding: var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-danger-dim);
    border: 1px dashed var(--cd-danger-line);
  }

  .echo-row { display: flex; flex-wrap: wrap; gap: var(--cd-space-1) var(--cd-space-2); align-items: baseline; }
  .echo-label { font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); color: var(--cd-ink-2); }
  .echo-value { font-size: var(--cd-text-xs); color: var(--cd-ink-1); }
  .echo-value.muted { color: var(--cd-ink-2); font-style: italic; }
  .echo-history { margin: var(--cd-space-2) 0 0; }
</style>
