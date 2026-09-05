<script>
  /**
   * The replayer's verdict, as a strip: one line that says what this browser
   * checked, one tag that says what it could not, the ordering witnessed (or
   * not) on one more line, and the raw material behind "Show the math".
   *
   * THE ORDERING IS NOT PART OF THE CLAIM. The headline says what the browser
   * did; that the commitment came BEFORE the deal is carried beside it as the
   * open question, with the panel that settles it directly under. The prose
   * that used to fill two panels is here word for word, behind "Why".
   *
   * HARNESS CONTRACT (handreplay.mjs): `.proof-banner.good|partial|bad` with
   * its first `strong` the headline (never claiming an ordering), `.banner-caveat`
   * saying "Not proven ... before"; `.witness-panel` with `[data-witness]`
   * (matched | differs | none) and the words "still running" when matched;
   * `.witness-input` and `.paste-verdict[data-paste]`; `.proof-panel` whose
   * first `.proof-line code` is the record's commitment and whose
   * `.proof-label`s claim no ordering.
   */
  import HashSeal from './HashSeal.svelte';
  import { hashFingerprint } from '$lib/hash-seal.js';

  let {
    hand,
    sighting = null,
    sightingVerdict = null,
    pasted = $bindable(''),
    pastedVerdict = null,
    copied = null,
    onCopy = () => {},
    formatClock = () => '',
    formatLocalClock = () => '',
  } = $props();

  const verification = $derived(hand?.verification || null);
  const witnessed = $derived(!!sightingVerdict?.witnessed);
</script>

<div class="verdict">
  {#if verification?.ok}
    <div class="proof-banner good">
      <HashSeal hash={hand.proof.seedHash} size={44} label="commitment" />
      <div class="banner-body">
        <strong>Verified in your browser: every card shown follows from the sealed deck.</strong>
        <p class="banner-line">
          All <span class="tally">{verification.cardsMatched}</span> cards were re-derived here from the
          revealed seed, and its <span class="method-name">SHA-256</span>, recomputed here, is the commitment
          the table published for this hand. No canister was asked to confirm it.
        </p>
        <p class="banner-caveat">
          <span class="caveat-tag">Not proven</span>
          That the commitment came <em>before</em> the deal. The
          <span class="timestamp">{formatClock(hand.timestamp)}</span> on this record is a clock the table
          canister read off itself; this page did not watch the order of events, and everything above stays
          true even if that clock is wrong.
          {#if witnessed}You settled it yourself, below, without that clock.{:else}The line below is how you settle it without trusting that clock.{/if}
        </p>
      </div>
    </div>
  {:else if verification?.commitment?.match}
    <div class="proof-banner partial">
      <HashSeal hash={hand.proof.seedHash} size={44} tone="muted" />
      <div class="banner-body">
        <strong>Commitment checked here; cards not checkable for this hand.</strong>
        <p class="banner-line">
          The revealed seed hashes to the pre-published commitment (recomputed in this browser), but
          this hand exposes no card that could be matched against the deck it produces.
        </p>
      </div>
    </div>
  {:else if verification}
    <div class="proof-banner bad">
      <HashSeal hash={hand.proof.seedHash} size={44} tone="muted" />
      <div class="banner-body">
        <strong>This hand did not verify in your browser.</strong>
        <p class="banner-line">{verification.error || 'The cards do not follow from the committed seed.'}</p>
      </div>
    </div>
  {:else}
    <div class="proof-banner partial">
      <HashSeal hash={hand.proof.seedHash} size={44} tone="muted" />
      <div class="banner-body">
        <strong>No revealed seed for this hand.</strong>
        <p class="banner-line">The seed is only published once a hand ends, so there is nothing to re-derive yet.</p>
      </div>
    </div>
  {/if}

  <!-- THE ORDERING, WITNESSED BY THE READER RATHER THAN ASSERTED BY US. -->
  <div class="panel witness-panel">
    {#if witnessed}
      <div class="witness-line good" data-witness="matched">
        <span class="banner-mark">✓</span>
        <span class="witness-text">
          <strong>Ordering witnessed by you</strong> at <span class="tally">{sighting.boardCount}</span> board card(s),
          <span class="timestamp">{formatLocalClock(sighting.at)}</span> by your own clock.
        </span>
      </div>
    {:else if sightingVerdict?.tone === 'bad'}
      <div class="witness-line bad" data-witness="differs">
        <span class="banner-mark">✗</span>
        <span class="witness-text"><strong>The commitment changed under you.</strong> Do not trust this table.</span>
      </div>
    {:else}
      <div class="witness-line none" data-witness="none">
        <span class="banner-mark">◌</span>
        <span class="witness-text">Ordering not witnessed by this browser. Open this panel once mid-hand and it will be.</span>
      </div>
    {/if}

    <!-- The compare field stays in the first paint: it is the mechanism, not
         a footnote, and the harness types into it. The prose folds. -->
    <label class="witness-field">
      <span class="witness-label">Compare a commitment you saved</span>
      <input
        class="witness-input mono"
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder="paste the commitment you copied while the hand was running"
        bind:value={pasted}
      />
    </label>
    {#if pastedVerdict && pastedVerdict.tone !== 'none'}
      <p class="paste-verdict {pastedVerdict.tone}" data-paste={pastedVerdict.tone}>
        {pastedVerdict.detail}
      </p>
    {/if}

    <details class="fold">
      <summary>Why this settles the order</summary>
      <div class="fold-body">
        {#if witnessed}
          <p class="why">
            <strong>You saw this commitment while the hand was still running.</strong>
            This browser read the commitment for
            <span class="hand-number">hand #{sighting.handNumber}</span> at
            <span class="timestamp">{formatLocalClock(sighting.at)}</span> by your own clock, when
            the table had published no seed for it and the board showed
            <span class="tally">{sighting.boardCount}</span> card(s). The seed the table revealed
            afterwards hashes to that same commitment. So the deck was already fixed at the moment
            you looked: before every card dealt after it, and before every action taken after it.
            Nothing in that sentence relies on the table's clock.
          </p>
        {:else if sightingVerdict?.tone === 'bad'}
          <p class="why">
            This browser read <code class="mono">{hashFingerprint(sighting.seedHash)}</code> for
            <span class="hand-number">hand #{sighting.handNumber}</span> while it was still
            running, and the finished record carries a different commitment. Do not trust this
            table.
          </p>
        {:else}
          <p class="why">
            This browser has no sighting of this hand's commitment from while it was running, so it
            cannot vouch for the order. It notes one automatically whenever this panel is open during
            a live hand, open it once mid-hand and the check above appears for that hand by itself.
          </p>
        {/if}
        <p class="why">
          The commitment is on screen from the moment cards are dealt and the seed is not published
          until the hand ends, so anyone who reads the commitment mid-hand and compares it afterwards
          has established the ordering themselves, with no canister clock in the argument.
        </p>
      </div>
    </details>
  </div>

  <details class="fold proof-panel">
    <summary>Show the math</summary>
    <div class="fold-body">
      <div class="proof-line">
        <!-- NOT "committed before the deal": that label asserted the exact
             ordering the banner above declines to claim, in the same modal. -->
        <span class="proof-label">commitment in this hand's record</span>
        <code class="hash">{hand.proof.seedHash || 'n/a'}</code>
        <button class="copy-btn" onclick={() => onCopy(hand.proof.seedHash, 'hash')}>
          {copied === 'hash' ? 'Copied' : 'Copy'}
        </button>
      </div>
      <div class="proof-line">
        <span class="proof-label">seed, published with the finished hand</span>
        {#if hand.proof.revealedSeed}
          <code class="hash revealed">{hand.proof.revealedSeed}</code>
          <button class="copy-btn" onclick={() => onCopy(hand.proof.revealedSeed, 'seed')}>
            {copied === 'seed' ? 'Copied' : 'Copy'}
          </button>
        {:else}
          <code class="hash muted">not revealed</code>
        {/if}
      </div>
      {#if verification}
        <div class="proof-line">
          <span class="proof-label"><span class="method-name">SHA-256</span> recomputed here</span>
          <code class="hash" class:revealed={!verification.commitment.match}>{verification.commitment.computed}</code>
        </div>
        <p class="panel-note layout-note">
          {#if verification.playersDetermined}
            Laid out for {verification.layout.players} players dealt in.
          {/if}
          {verification.rejections} draw(s) rejected by the sampling rule.
          Verifying a shuffle proves the deal was not tampered with after the commitment. It proves
          nothing about settlement or about the rest of this unaudited engine.
        </p>
      {/if}
    </div>
  </details>
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.panel;
  @include f.tones;
  @include f.disclosure;
  @include f.hashcode;
  @include f.copybtn;

  code.hash.muted { color: var(--cd-ink-2); font-style: italic; }

  .verdict { display: flex; flex-direction: column; gap: var(--cd-space-2); }

  .proof-banner {
    display: flex;
    gap: var(--cd-space-3);
    align-items: flex-start;
    padding: var(--cd-space-3) var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    background: var(--tone-dim);
    border: 1px solid var(--tone-line);
  }

  .banner-body { min-width: 0; }
  .proof-banner strong { display: block; font-size: var(--cd-text-md); color: var(--tone); line-height: 1.35; }
  .banner-line { margin: var(--cd-space-1) 0 0; font-size: var(--cd-text-sm); line-height: 1.5; color: var(--cd-ink-1); }

  /* The limit rides INSIDE the banner so the reader cannot take the verdict
     without the caveat: a tag, not a footnote. */
  .banner-caveat {
    margin: var(--cd-space-2) 0 0;
    padding-top: var(--cd-space-2);
    border-top: 1px solid var(--cd-line-soft);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
  }

  .caveat-tag {
    display: inline-block;
    margin-right: var(--cd-space-1);
    padding: 1px var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-warn-dim);
    border: 1px solid var(--cd-warn-line);
    color: var(--cd-warn);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    vertical-align: 1px;
  }

  .witness-panel { margin-bottom: 0; padding-top: var(--cd-space-2); padding-bottom: 0; }

  .witness-line {
    display: flex;
    gap: var(--cd-space-2);
    align-items: baseline;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-1);
    padding-bottom: var(--cd-space-2);
  }

  .witness-line strong { color: var(--tone); }
  .witness-line.none { color: var(--cd-ink-2); }
  .banner-mark { font-weight: var(--cd-weight-figure); color: var(--tone, var(--cd-ink-2)); flex: 0 0 auto; }
  .why { margin: 0 0 var(--cd-space-2); font-size: var(--cd-text-sm); line-height: 1.55; color: var(--cd-ink-2); }
  .why strong { color: var(--cd-ink-1); }
  .why code { font-family: var(--cd-font-mono); color: var(--cd-accent); }

  .witness-field { display: block; margin-bottom: var(--cd-space-2); }
  .witness-label {
    display: block;
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
    margin-bottom: var(--cd-space-1);
  }

  .witness-input {
    width: 100%;
    box-sizing: border-box;
    min-height: var(--cd-control-md);
    background: var(--cd-bg-deep);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    color: var(--cd-accent);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-sm);
    padding: var(--cd-space-2) var(--cd-space-3);
  }

  .witness-input::placeholder { color: var(--cd-ink-2); font-family: var(--cd-font-ui); }
  .witness-input:focus { outline: none; border-color: var(--cd-accent-line-strong); }

  .paste-verdict { margin: var(--cd-space-2) 0 0; font-size: var(--cd-text-sm); line-height: 1.5; color: var(--tone); }

  .proof-panel { padding: 0 var(--cd-space-1); }
  .proof-line { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--cd-space-2); align-items: start; margin-bottom: var(--cd-space-2); }
  .proof-label {
    grid-column: 1 / -1;
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
  }

  /* Not decoration: `.method-name` is the class the token allowlist attributes
     "SHA-256" to (static-claim-copy). */
  .method-name { color: inherit; font-weight: inherit; }
  .layout-note { font-size: var(--cd-text-xs); }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .witness-input { font-size: 16px; min-height: var(--cd-touch-min); }
  }
</style>
