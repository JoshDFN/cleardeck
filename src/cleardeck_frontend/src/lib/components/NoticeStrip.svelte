<script>
  // THE FIVE PROTECTED NOTICES, INSIDE A DIALOG (HARD RULE 2, docs/DEFECTS.md
  // T-31). The page's trust bar is behind a dialog's scrim, so the same words
  // are restated first inside the sheet, as the trust bar's neutral strip:
  // the amber glyph, the lead in bold, the sentences in body ink. Not dimmed
  // and not red: it is the one block that must read as a statement, not as
  // fine print and not as an alarm bell.
  //
  // Harness contract: `.player-notice` with the words in `.notice-text`
  // (tools/shots/lib/protected-notices.mjs hit-tests the deepest carrier;
  // the token allowlist keys on `.player-notice`).

  import NoticeLine from './NoticeLine.svelte';
</script>

<p class="player-notice">
  <span class="notice-glyph" aria-hidden="true">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linejoin="round">
      <path d="M12 3.5 2.5 20h19L12 3.5z"/>
      <path d="M12 9.5v4.5" stroke-linecap="round"/>
      <circle cx="12" cy="17" r="0.8" fill="currentColor" stroke="none"/>
    </svg>
  </span>
  <span class="notice-text"><NoticeLine /></span>
</p>

<style>
  .player-notice {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--cd-space-3);
    align-items: start;
    margin: 0;
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    border: 1px solid var(--cd-warn-line);
    border-left-width: 3px;
    background: var(--cd-surface-1);
    color: var(--cd-ink-1);
    font-size: var(--cd-text-sm);
    line-height: 1.5;
  }

  .notice-glyph {
    display: inline-flex;
    margin-top: 2px;
    color: var(--cd-warn);
  }

  .player-notice :global(strong) { color: var(--cd-notice-strong); }

  /* The phone sheet: a compact strip, the size the table view's trust bar
     uses; every word stays. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .player-notice {
      font-size: var(--cd-text-xs);
      line-height: 1.45;
      padding: var(--cd-space-2) var(--cd-space-3);
    }
  }
</style>
