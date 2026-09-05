<script>
  /**
   * "Don't trust this page either": the seed taken to tools we do not control.
   * Verbatim from the fairness panel; only the file moved.
   *
   * Harness: `.method .method-name` carries the numbered method names, which
   * the token allowlist excuses as static claim copy.
   */
  import { copyWord } from '$lib/copy-text.js';

  const { proof, revealedSeed, players = 2, copied = null, onCopy = () => {} } = $props();

  const nodeCmd = $derived(`node src/poker_core/tests/verify/verify_shuffle.mjs ${revealedSeed} --players ${players} --seed-hash ${proof.seed_hash}`);
  const pyCmd = $derived(`python3 src/poker_core/tests/verify/verify_shuffle.py ${revealedSeed} --players ${players} --seed-hash ${proof.seed_hash}`);
  const shaCmd = $derived(`echo -n "${revealedSeed}" | xxd -r -p | shasum -a 256`);
</script>

<div class="manual-verify">
  <p class="manual-lead">
    Everything above ran in your browser, but it is still our JavaScript. Take the seed somewhere
    we do not control:
  </p>

  <div class="method">
    <span class="method-name">1. Check the commitment with any SHA-256 tool</span>
    <div class="command-row">
      <code class="command">{shaCmd}</code>
      <button class="copy-btn" class:failed={copied === 'failed:cmd'} onclick={() => onCopy(shaCmd, 'cmd')}>{copyWord(copied, 'cmd', 'Copy', '✓')}</button>
    </div>
    <a href="https://emn178.github.io/online-tools/sha256.html" target="_blank" rel="noopener">
      Online SHA-256 calculator
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
        <polyline points="15,3 21,3 21,9"/><line x1="10" y1="14" x2="21" y2="3"/>
      </svg>
    </a>
    <div class="method-hint-box">
      <span>Set the tool's <strong>input encoding to “Hex”</strong>: the seed is bytes, not text.</span>
    </div>
    <div class="expected-result">
      <span class="expected-label">It must print</span>
      <code class="expected-hash">{proof.seed_hash}</code>
    </div>
  </div>

  <div class="method">
    <span class="method-name">2. Re-derive the cards with a verifier that is not ours</span>
    <p class="method-lead">
      Two independent implementations, written from the specification and sharing no code with the
      engine or with this page, ship in the repository. Both print the whole deck and lay out the hand:
    </p>
    <div class="command-row">
      <code class="command">{nodeCmd}</code>
      <button class="copy-btn" class:failed={copied === 'failed:node'} onclick={() => onCopy(nodeCmd, 'node')}>{copyWord(copied, 'node', 'Copy', '✓')}</button>
    </div>
    <div class="command-row">
      <code class="command">{pyCmd}</code>
      <button class="copy-btn" class:failed={copied === 'failed:py'} onclick={() => onCopy(pyCmd, 'py')}>{copyWord(copied, 'py', 'Copy', '✓')}</button>
    </div>
    <p class="method-lead">
      Or write your own from <code>docs/SHUFFLE-SPEC.md</code>. If it disagrees with this page,
      that is our bug, and we want to hear about it.
    </p>
  </div>
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.copybtn;

  .manual-lead { color: var(--cd-ink-2); font-size: var(--cd-text-sm); line-height: 1.6; margin: 0 0 var(--cd-space-3); }
  .method { display: flex; flex-direction: column; gap: var(--cd-space-2); margin-bottom: var(--cd-space-4); }
  .method-name { color: var(--cd-ink-1); font-size: var(--cd-text-sm); font-weight: var(--cd-weight-strong); }
  .method-lead { color: var(--cd-ink-2); font-size: var(--cd-text-xs); line-height: 1.6; margin: 0; }
  .method-lead code { color: var(--cd-accent); font-family: var(--cd-font-mono); }
  .command-row { display: flex; gap: var(--cd-space-2); align-items: stretch; }

  .command {
    flex: 1;
    min-width: 0;
    background: var(--cd-bg-deep);
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-money);
    word-break: break-all;
    border: 1px solid var(--cd-money-line);
  }

  .method a {
    color: var(--cd-accent);
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-1);
    font-size: var(--cd-text-sm);
    align-self: flex-start;
    min-height: var(--cd-control-sm);
  }

  .method a:hover { text-decoration: underline; }

  .method-hint-box {
    padding: var(--cd-space-2) var(--cd-space-3);
    background: var(--cd-warn-dim);
    border: 1px solid var(--cd-warn-line);
    border-radius: var(--cd-radius-chip);
    color: var(--cd-warn);
    font-size: var(--cd-text-xs);
    line-height: 1.45;
  }

  .expected-result {
    padding: var(--cd-space-2) var(--cd-space-3);
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-chip);
  }

  .expected-label { color: var(--cd-ink-2); font-size: var(--cd-text-xs); display: block; margin-bottom: var(--cd-space-1); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); }
  .expected-hash { color: var(--cd-accent); font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); word-break: break-all; display: block; line-height: 1.5; }

  @media #{f.$phone} {
    .method a { min-height: var(--cd-touch-min); }
  }
</style>
