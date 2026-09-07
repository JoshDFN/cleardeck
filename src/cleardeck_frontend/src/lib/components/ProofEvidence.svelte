<script>
  /**
   * The chain of evidence: the four rungs, in the words the fairness panel
   * has always used, behind a disclosure. The verdict card above it carries
   * the cards and the seals; these rungs say WHY each step means what it means.
   *
   * HARNESS CONTRACT (shuffleproof.mjs): `.ladder > .rung` x4 once the seed is
   * revealed, rungs 3 and 4 `.done` only after the browser computed them.
   */
  import HashSeal from './HashSeal.svelte';
  import { hashFingerprint } from '$lib/hash-seal.js';

  const {
    proof,
    revealedSeed = null,
    report = null,
    phase = 'idle',
    failure = null,
    holeCheck = null,
    formatTimestamp = () => '',
    ordinal = (n) => String(n + 1),
  } = $props();
</script>

<ol class="ladder">
  <!-- STEP 1, the commitment. Deliberately NOT "published before the deal":
       start_new_hand commits and deals in one message, so no outsider can
       watch the commitment appear before cards exist. What IS provable is
       that the whole 52-card order was fixed before the board was shown, and
       that is what this rung claims. docs/DEFECTS.md D-06. -->
  <li class="rung done">
    <div class="rung-mark">1</div>
    <div class="rung-body">
      <h4>The whole deck was fixed before the board came out</h4>
      <p class="rung-note">
        This hash has been on screen since the first card was dealt, and it fixes all 52 positions:
        the turn and the river were already decided while you were looking at the flop.
        The table says it published it at {formatTimestamp(proof.timestamp)}; that timestamp is the
        canister's own word and is the one thing on this panel you have to take on trust. Everything
        below is checked here.
      </p>
      <div class="rung-seal">
        <HashSeal hash={proof.seed_hash} size={24} />
        <span class="fingerprint mono" title={proof.seed_hash}>{hashFingerprint(proof.seed_hash)}</span>
        <span class="rung-tag">commitment</span>
      </div>
    </div>
  </li>

  <!-- STEP 2, the reveal -->
  <li class="rung" class:done={!!revealedSeed}>
    <div class="rung-mark">2</div>
    <div class="rung-body">
      <h4>After the hand, the table revealed the seed</h4>
      {#if revealedSeed}
        <p class="rung-note">The 32 bytes the deck was shuffled from. Yours to keep and re-check anywhere.</p>
        <div class="rung-seal">
          <HashSeal hash={revealedSeed} size={24} tone="money" label="revealed seed" />
          <span class="fingerprint mono revealed" title={revealedSeed}>{hashFingerprint(revealedSeed)}</span>
          <span class="rung-tag">revealed seed</span>
        </div>
      {:else}
        <div class="pending-box">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
          <span>Sealed until this hand ends. Revealing it early would show everyone the deck.</span>
        </div>
      {/if}
    </div>
  </li>

  {#if revealedSeed}
    <!-- STEP 3, the browser hashes it -->
    <li class="rung" class:done={report?.commitment?.match} class:bad={report && !report.commitment.match}>
      <div class="rung-mark">3</div>
      <div class="rung-body">
        <h4>Your browser hashed that seed</h4>
        <p class="rung-note">
          Computed in this tab with the browser's own WebCrypto SHA-256. If it equals the hash from
          step 1, the seed is the one that was committed to.
        </p>
        {#if phase === 'working' && !report}
          <div class="working"><span class="spinner"></span> hashing…</div>
        {:else if report}
          <div class="verdict" class:good={report.commitment.match} class:bad={!report.commitment.match}>
            {#if report.commitment.match}
              Identical. The revealed seed is the committed seed. The two hashes are under Show the math.
            {:else}
              NOT identical. The table revealed a seed it did not commit to.
            {/if}
          </div>
        {/if}
      </div>
    </li>

    <!-- STEP 4, the cards, re-derived locally -->
    <li class="rung" class:done={report?.ok} class:bad={phase === 'failed'}>
      <div class="rung-mark">4</div>
      <div class="rung-body">
        <h4>Your browser rebuilt the deck and found your cards</h4>
        <p class="rung-note">
          Same seed, same 51 shuffle steps, no network. The cards on the left of the verdict above are
          what this tab derived; the cards on the right are what you were dealt.
        </p>
        {#if phase === 'working' && !report}
          <div class="working"><span class="spinner"></span> re-deriving 52 cards…</div>
        {:else if failure && !report?.layout}
          <div class="verdict bad">{failure}</div>
        {:else if report?.layout}
          <p class="layout-note">
            {#if report.playersDetermined}
              Laid out for <strong>{report.layout.players} players dealt in</strong>{#if holeCheck}, with you
              {ordinal(report.layout.dealIndex)} in ascending seat order{/if}. That is the only table size
              consistent with the cards you saw.
              {#if report.playersAgree === false}
                The hand log implies {report.expectedPlayers}; the deal itself says
                {report.layout.players}, and the deal is what was checked.
              {/if}
            {:else if holeCheck}
              Your two cards sit at the front of the deck, so their position does not depend on how many
              players were dealt in. Without a board, the table size cannot be pinned down from the cards
              alone, and this panel does not guess it.
            {:else}
              The number of players dealt in could not be pinned down from the cards available.
            {/if}
            {#if report.rejections > 0}
              {report.rejections} draw(s) were rejected and re-hashed, per the sampling rule.
            {/if}
          </p>
        {/if}
      </div>
    </li>
  {/if}
</ol>

<style>
  .ladder { list-style: none; margin: 0; padding: 0; }

  .rung {
    display: grid;
    grid-template-columns: 28px 1fr;
    gap: var(--cd-space-3);
    padding-bottom: var(--cd-space-4);
    position: relative;
  }

  .rung::before {
    content: '';
    position: absolute;
    left: 13px;
    top: 30px;
    bottom: 0;
    width: 2px;
    background: var(--cd-line-soft);
  }

  .rung:last-child::before { display: none; }

  .rung-mark {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-ink-2);
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    z-index: 1;
  }

  .rung.done .rung-mark { color: var(--cd-accent-ink); background: var(--cd-accent); border-color: var(--cd-accent); }
  .rung.bad .rung-mark { color: var(--cd-danger-ink); background: var(--cd-danger); border-color: var(--cd-danger); }

  .rung-body { min-width: 0; }
  .rung-body h4 { margin: 4px 0 var(--cd-space-1); font-size: var(--cd-text-sm); font-weight: var(--cd-weight-strong); color: var(--cd-ink); }
  .rung-note { margin: 0 0 var(--cd-space-2); font-size: var(--cd-text-sm); line-height: 1.55; color: var(--cd-ink-2); }

  .rung-seal { display: flex; align-items: center; gap: var(--cd-space-2); }
  .fingerprint { font-family: var(--cd-font-mono); font-size: var(--cd-text-sm); color: var(--cd-accent); }
  .fingerprint.revealed { color: var(--cd-money); }
  .rung-tag { font-size: var(--cd-text-xs); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); color: var(--cd-ink-2); }

  .pending-box {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    padding: var(--cd-space-2) var(--cd-space-3);
    background: var(--cd-warn-dim);
    border: 1px solid var(--cd-warn-line);
    border-radius: var(--cd-radius-chip);
    color: var(--cd-warn);
    font-size: var(--cd-text-sm);
    line-height: 1.45;
  }

  .working { display: flex; align-items: center; gap: var(--cd-space-2); color: var(--cd-ink-2); font-size: var(--cd-text-sm); }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--cd-line);
    border-top-color: var(--cd-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .verdict {
    margin-top: var(--cd-space-1);
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    line-height: 1.45;
  }

  .verdict.good { background: var(--cd-accent-dim); color: var(--cd-accent); border: 1px solid var(--cd-accent-line); }
  .verdict.bad { background: var(--cd-danger-dim); color: var(--cd-danger-hi); border: 1px solid var(--cd-danger-line); }

  .layout-note { margin: 0; font-size: var(--cd-text-xs); line-height: 1.6; color: var(--cd-ink-2); }
  .layout-note strong { color: var(--cd-ink-1); }
</style>
