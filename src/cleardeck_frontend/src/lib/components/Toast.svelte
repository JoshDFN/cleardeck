<script>
  // THE PAGE'S ONE TOAST: an error (with an optional detail line and one
  // action) or a success line.
  //
  // WHERE IT STANDS is a protected-notice rule, not a style choice
  // (docs/DEFECTS.md E-52, docs/DESIGN-BAR.md section 7): the five player
  // notices must be on screen and legible on every view with a toast up. Three
  // independent locks keep that true, each measured separately by
  // tools/shots/lib/toast-notices.mjs on every scene at every viewport:
  //
  //   1. POSITION. The toast starts below the trust bar at
  //      `--notice-safe-top` (the bar's measured height, published by `.app`)
  //      plus a gap, so the two boxes cannot overlap at any scroll offset.
  //   2. PAINT ORDER. z-index 90 puts it BENEATH the bar (100) and the footer
  //      (95), so even a wrong measurement cannot win the hit test the gate
  //      runs on each phrase's own pixels. It is still above the header (50).
  //   3. SIZE. Clamped to the viewport horizontally and to 40vh (320 px)
  //      vertically; a long message scrolls INSIDE the toast.
  //
  // EVERY RULE BELOW IS GLOBAL ON PURPOSE. The harness cannot provoke a real
  // toast on most scenes, so it injects one node (`div.toast.error`) carrying
  // the PAGE component's scope class and requires the stylesheet's `.toast`
  // rules to paint it exactly as they paint the real one; the `toast-notices`
  // scene then compares the injected node with a real toast property by
  // property. A rule scoped to THIS component's class would style the real
  // toast and not the injected one, and the gate would refuse the run. The
  // class names are the toast's own (`.toast`, `.toast-*`), used nowhere else.
  //
  // THE PHONE (routes/app-phone.scss owns the rest of the phone shell): on the
  // table view the toast stands under the header, never over Lobby / History
  // / Verify Fair; on every other view it is a bottom snackbar above the home
  // indicator and ABOVE the footer (z 99: at 90 the footer painted over Retry
  // and Dismiss, measured by tools/shots/probe-lobby.mjs). While a money sheet
  // holds the page scroll-locked (lib/scroll-lock.js) it is a snackbar above
  // the sheet, over its scrolled foot, never over the notices at its head.

  const {
    /** 'error' | 'success' */
    kind = 'error',
    /** The sentence. Nothing renders while it is empty. */
    message = null,
    /** The raw text behind the sentence (error only), or null. */
    detail = null,
    /** One control for THIS message ({ label, run }), or null. */
    action = null,
    onDismiss = () => {},
  } = $props();
</script>

{#if message}
  {#if kind === 'error'}
    <div class="toast error" role="alert">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <line x1="15" y1="9" x2="9" y2="15"/>
        <line x1="9" y1="9" x2="15" y2="15"/>
      </svg>
      <span>
        {message}
        {#if detail}<small class="toast-detail">{detail}</small>{/if}
      </span>
      {#if action}
        <button class="toast-action" type="button" onclick={action.run}>{action.label}</button>
      {/if}
      <button class="toast-close" type="button" onclick={onDismiss} aria-label="Dismiss">×</button>
    </div>
  {:else}
    <div class="toast success" role="status">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <path d="M9 12l2 2 4-4"/>
      </svg>
      <span>{message}</span>
    </div>
  {/if}
{/if}

<style>
  :global(.toast) {
    position: fixed;
    top: calc(var(--notice-safe-top, 80px) + var(--cd-space-3));
    left: 50%;
    transform: translateX(-50%);
    z-index: 90;
    display: flex;
    align-items: center;
    gap: var(--cd-space-3);
    padding: var(--cd-space-3) var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    animation: cd-toast-down var(--cd-base) var(--cd-ease);
    box-sizing: border-box;
    width: max-content;
    max-width: min(var(--cd-toast-w), calc(100vw - var(--cd-space-5)));
    max-height: min(40vh, 320px);
    overflow-y: auto;
    overscroll-behavior: contain;
    font-size: var(--cd-text-sm);
    line-height: 1.45;
  }

  /* A long message wraps and, if it still does not fit, scrolls inside the
     toast. Before this it simply made the box wider than the screen. */
  :global(.toast span) {
    flex: 1 1 auto;
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--cd-ink-1);
  }

  /* The glyph sits in a tinted disc; the panel is opaque (4.5:1 over
     anything) and the colour of the disc is the only thing that says which
     kind of message this is. */
  :global(.toast svg) {
    flex: 0 0 auto;
    width: var(--cd-icon-md);
    height: var(--cd-icon-md);
    padding: calc(var(--cd-space-1) * 1.5);
    box-sizing: content-box;
    border-radius: 50%;
  }

  /* The error toast is a panel at its full width budget, whatever its
     message: it carries a Retry and a detail line, and the harness compares
     the box of the toast it injects with the one the app raises
     (toast-notices scene), which a content-sized box would fail on x. */
  :global(.toast.error) {
    width: min(var(--cd-toast-w), calc(100vw - var(--cd-space-5)));
    background: var(--cd-sheet);
    border: 1px solid var(--cd-line-strong);
    color: var(--cd-ink);
    box-shadow: var(--cd-shadow-lift);
  }

  :global(.toast.error svg) {
    background: var(--cd-danger-dim);
    color: var(--cd-danger-hi);
  }

  :global(.toast.success) {
    background: var(--cd-sheet);
    border: 1px solid var(--cd-accent-line);
    color: var(--cd-ink);
    box-shadow: var(--cd-shadow-lift);
  }

  :global(.toast.success svg) {
    background: var(--cd-accent-dim);
    color: var(--cd-accent);
  }

  /* The raw text behind the sentence: three lines at most, then the console. */
  :global(.toast-detail) {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
    margin-top: var(--cd-space-1);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    line-height: 1.4;
    color: var(--cd-ink-2);
    overflow-wrap: anywhere;
  }

  :global(.toast .toast-action) {
    flex: 0 0 auto;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink);
    font: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
  }

  :global(.toast .toast-action:hover) { background: var(--cd-surface-3); }

  /* A 44 px dismiss target (the audit measured ~20x24). The negative margins
     keep the toast's box the size it was; only the hit area grew. */
  :global(.toast .toast-close) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    min-width: var(--cd-touch-min);
    min-height: var(--cd-touch-min);
    margin: calc(-1 * var(--cd-space-2)) calc(-1 * var(--cd-space-3)) calc(-1 * var(--cd-space-2)) 0;
    background: none;
    border: none;
    border-radius: 50%;
    color: var(--cd-ink-2);
    font-size: var(--cd-icon-md);
    line-height: 1;
    cursor: pointer;
    padding: 0;
  }

  :global(.toast .toast-close:hover),
  :global(.toast .toast-close:focus-visible) {
    color: var(--cd-ink);
    background: var(--cd-surface-3);
    outline: none;
  }

  @keyframes cd-toast-down {
    from { transform: translateX(-50%) translateY(-20px); opacity: 0; }
    to { transform: translateX(-50%) translateY(0); opacity: 1; }
  }

  @keyframes cd-toast-up {
    from { transform: translateX(-50%) translateY(20px); opacity: 0; }
    to { transform: translateX(-50%) translateY(0); opacity: 1; }
  }

  /* ------------------------------------------------------------- the phone */
  @media (max-width: 760px) {
    :global(.toast) {
      top: calc(var(--notice-safe-top, 80px) + var(--header-h, 0px) + var(--cd-space-2));
      max-width: calc(100vw - var(--cd-space-4));
      padding: var(--cd-space-2) var(--cd-space-3);
      gap: var(--cd-space-2);
    }

    :global(.app:not(.on-table) .toast) {
      top: auto;
      bottom: calc(var(--cd-safe-bottom) + var(--cd-space-4));
      z-index: 99;
      animation-name: cd-toast-up;
    }

    :global(.toast .toast-action) { min-height: var(--cd-touch-min); }

    :global(html.cd-scroll-lock .toast) {
      top: auto;
      bottom: calc(var(--cd-safe-bottom) + var(--cd-space-4));
      z-index: calc(var(--cd-z-dialog) + 10);
      animation-name: cd-toast-up;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.toast) { animation: none; }
  }
</style>
