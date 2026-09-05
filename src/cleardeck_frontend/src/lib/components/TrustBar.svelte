<script>
  // THE TRUST BAR: the five protected notices, on every view, at every viewport.
  //
  // The words are the product's hard rule, not this component's: "Unaudited
  // code with known bugs", "your funds are NOT safe", "illegal in many
  // jurisdictions", "18+ only" and "No rake is taken from any pot on any
  // table" are the literal strings that `FRONTEND_NOTICES` in scripts/dev.sh
  // greps for in the source and `PROTECTED_PHRASES` in
  // tools/shots/lib/protected-notices.mjs hit-tests on the rendered pixels.
  // This component only decides how they are typeset; the words themselves
  // are lib/notices.js, the one source every surface renders.
  //
  // One neutral bar, the same on the lobby and the table: the amber glyph, the
  // sentences, and FULL TERMS. It replaced a 160 px (desktop) / 268 px (phone)
  // red block that the audit measured as the first impression of every visit,
  // and the second copy of the same block in the footer. The full text is one
  // tap away, as an opaque overlay, so the felt or the list never resizes under
  // the reader.
  //
  // The wrapper `.alpha-warning-banner` stays in routes/+page.svelte on purpose:
  // it is measured there (`--notice-safe-top`, the toast's anchor) and the
  // screenshot harness reads that element's scope class to style the toast it
  // injects. This component is the bar's content.

  import NoticeLine from './NoticeLine.svelte';
  import { NOTICE_LEAD, NOTICE_NO_RAKE, NOTICE_TERMS } from '../notices.js';

  const {
    /** The full text is open as an overlay. */
    expanded = false,
    onExpand = () => {},
    onClose = () => {},
  } = $props();
</script>

<!-- Collapsed presentation. Every protected phrase is literal, so the
     on-screen test and `make hygiene` ask about the same words. -->
<button
  class="banner-strip"
  type="button"
  aria-expanded={expanded}
  title="Open the full player-protection terms"
  onclick={onExpand}
>
  <span class="warning-icon" aria-hidden="true">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linejoin="round">
      <path d="M12 3.5 2.5 20h19L12 3.5z"/>
      <path d="M12 9.5v4.5" stroke-linecap="round"/>
      <circle cx="12" cy="17" r="0.8" fill="currentColor" stroke="none"/>
    </svg>
  </span>
  <span class="strip-text"><NoticeLine /></span>
  <span class="strip-more">Full terms</span>
</button>

{#if expanded}
  <!-- The complete text, unchanged, every word: a strict superset of the
       strip, which is what a FULL TERMS button has to be. An opaque overlay,
       never an in-flow expansion. -->
  <div class="banner-content" role="dialog" aria-modal="true" aria-label="Player-protection terms">
    <div class="terms">
      <p class="terms-title">Player-protection terms</p>
      <p class="banner-warning">
        <strong>Disclaimer:</strong> {NOTICE_LEAD}. {NOTICE_TERMS.warningBody}
      </p>
      <p class="banner-info">
        {NOTICE_TERMS.infoBefore} <strong>{NOTICE_NO_RAKE}</strong> {NOTICE_TERMS.infoAfter}
      </p>
      <p class="banner-ai">
        {NOTICE_TERMS.ai}
      </p>
    </div>
    <button
      class="banner-close"
      type="button"
      onclick={onClose}
      aria-label="Close the full player-protection terms"
    >Close</button>
  </div>
{/if}

<style>
  .banner-strip {
    display: flex;
    align-items: center;
    gap: var(--cd-space-3);
    width: 100%;
    min-height: var(--cd-control-md);
    margin: 0;
    padding: var(--cd-space-2) var(--cd-space-4) var(--cd-space-2) var(--cd-space-3);
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    font-size: var(--cd-text-sm);
    line-height: 1.35;
    text-align: left;
    cursor: pointer;
  }

  .warning-icon {
    display: inline-flex;
    flex: 0 0 auto;
    color: var(--cd-warn);
  }

  .strip-text {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--cd-ink-1);
    font-weight: var(--cd-weight-medium);
  }

  .strip-text :global(strong) {
    color: var(--cd-ink);
    font-weight: var(--cd-weight-strong);
  }

  /* The affordance: a real target inside a bar the whole width of which is
     the button. */
  .strip-more {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    min-height: 28px;
    padding: 0 var(--cd-space-3);
    border: 1px solid var(--cd-warn-line);
    border-radius: var(--cd-radius-pill);
    background: var(--cd-warn-dim);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--cd-warn);
    white-space: nowrap;
  }

  .banner-strip:hover .strip-more {
    background: var(--cd-warn-line);
    color: var(--cd-ink);
  }

  .banner-strip:focus-visible {
    outline: 2px solid var(--cd-accent);
    outline-offset: -2px;
  }

  /* Expanded: an OVERLAY, opaque, above everything. The wrapper in
     +page.svelte raises its own z-index while expanded, so this reads the
     same layer. */
  .banner-content {
    position: fixed;
    inset: 0;
    z-index: 2000;
    padding: var(--cd-space-6) var(--cd-space-5) 96px;
    overflow-y: auto;
    overscroll-behavior: contain;
    background: var(--cd-bg);
    color: var(--cd-ink-1);
  }

  .terms {
    max-width: 720px;
    margin: 0 auto;
  }

  .terms p {
    margin: 0 0 var(--cd-space-4);
    font-size: var(--cd-text-md);
    line-height: 1.6;
  }

  .terms-title {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
    color: var(--cd-warn);
  }

  .banner-warning { color: var(--cd-ink); }
  .banner-warning strong { color: var(--cd-warn); }
  .banner-info strong { color: var(--cd-ink); }
  .banner-ai { color: var(--cd-ink-2); }

  .banner-close {
    position: fixed;
    bottom: 22px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 2001;
    min-height: var(--cd-touch-min);
    padding: 0 30px;
    border-radius: var(--cd-radius-pill);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-3);
    color: var(--cd-ink);
    font-family: inherit;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  /* The phone: the same words at 11 px, three lines, the whole strip a
     44 px target. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .banner-strip {
      display: block;
      min-height: var(--cd-touch-min);
      padding: 5px 9px;
      font-size: var(--cd-text-xs);
      line-height: 1.25;
    }

    .warning-icon {
      vertical-align: -2px;
      margin-right: 3px;
    }

    .warning-icon svg { width: 11px; height: 11px; }

    .strip-more {
      display: inline-block;
      min-height: 0;
      margin-left: 5px;
      padding: 1px 6px;
      font-size: var(--cd-text-xs);
    }

    .banner-content { padding: var(--cd-space-5) var(--cd-space-4) 84px; }
  }

  /* The phone held sideways: two lines of strip, not three. */
  @media (min-aspect-ratio: 1/1) and (max-height: 560px) {
    .banner-strip {
      padding: var(--cd-space-1) var(--cd-space-2) calc(var(--cd-space-1) * 1.5);
    }
  }
</style>
