<script>
  /**
   * THE PHONE HELD SIDEWAYS. A 390 px-tall window cannot hold a trust bar, a
   * header, a felt and a dock; the audit measured an 18% felt, seats hanging
   * outside the rail and a two-screen scroll trap. Until a real landscape
   * layout exists (docs/UI-WAVE.md), the table view asks for portrait.
   *
   * It stands over the TABLE AREA ONLY. The trust bar and the header above it
   * are untouched (the five protected phrases stay on screen, unoccluded), the
   * table keeps polling underneath, and the panel is gated in CSS to a coarse
   * pointer under 560 px tall, so a short desktop window never sees it.
   */
</script>

<div class="rotate-prompt" role="status" aria-live="polite">
  <svg class="glyph" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
    <rect x="5" y="2" width="14" height="20" rx="2.5" />
    <path d="M12 18.5h.01" />
    <path d="M21 8.5a9 9 0 0 0-3.5-4.2M3 15.5a9 9 0 0 0 3.5 4.2" />
  </svg>
  <p class="line">Turn your phone upright to play.</p>
  <p class="sub">The hand keeps running; nothing is paused.</p>
</div>

<style>
  .rotate-prompt {
    display: none;
    position: absolute;
    inset: 0;
    z-index: 31;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--cd-space-2);
    padding: var(--cd-space-4);
    background: var(--cd-sheet);
    color: var(--cd-ink);
    text-align: center;
  }

  .glyph { color: var(--cd-accent); }

  .line {
    margin: 0;
    font-size: var(--cd-text-lg);
    font-weight: var(--cd-weight-strong);
  }

  .sub {
    margin: 0;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
  }

  /* A phone (coarse pointer) in landscape, under 560 px tall: the same
     threshold the header and dock already use for "held sideways". */
  @media (min-aspect-ratio: 1/1) and (max-height: 560px) and (pointer: coarse) {
    .rotate-prompt { display: flex; }
  }
</style>
