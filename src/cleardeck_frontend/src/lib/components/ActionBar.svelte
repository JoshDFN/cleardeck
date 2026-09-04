<script>
  /**
   * The action bar: the one row a player touches most.
   *
   * Four states of one `.actions` element (the harness reads it as the
   * viewport-independent "my turn" signal: `.actions:not(.disabled)`):
   *
   *   my turn      Fold / Check or Call X / Raise to X (one click commits the
   *                sizer's figure) / All in, with the clock line on top, the
   *                hotkey letters underlined, an inline error strip above.
   *                Never a fifth cell: the time bank is TimeBankPill, under
   *                the turn indicator (desktop) or on the pod clock (phone).
   *   sent         the SAME row, the pressed button marked and reading
   *                "Calling 0.10", the rest dimmed, the clock line held.
   *                No spinner: the felt already shows the echo.
   *   waiting      the pre-action toggles (PreActions), or a quiet pill when
   *                the hero cannot act this hand.
   *   no game      "Waiting for players" / "Hand complete".
   *
   * Keyboard (desktop): F fold, C check or call, R raise at the sizer's
   * figure, A twice for all in, 1-5 (1-6 pre-flop) presets, + and - a big
   * blind, Esc cancels; never Enter or Space, which belong to whatever has
   * focus. Inactive while any field has focus ($lib/hotkeys.js).
   *
   * Every size in the style block is a token: the row's rhythm is the design
   * system's, not this file's.
   */
  import { PRESETS } from '$lib/bet-sizing.js';
  import { dialogIsOpen, focusKindOf, resolveHotkey } from '$lib/hotkeys.js';
  import PreActions from './PreActions.svelte';

  const {
    isMyTurn = false,
    gameInProgress = false,
    actionPending = false,   // the update call is in flight
    sentText = null,         // "Calling 0.10" while a send is open, else null
    sentKind = null,         // the pending kind, for the pressed button
    noGameText = '',
    waitingFor = '',
    canCheck = false,
    canRaise = false,
    callAmount = 0,
    raiseLabel = 'Raise to', // or 'Bet'
    raiseAmount = 0,
    raiseProblem = null,     // an illegal size, in words, or null
    fmt = (v) => String(v),
    clockFraction = 0,
    clockUrgent = false,
    actionError = null,
    preOptions = [],         // [] when the hero cannot pre-act
    preArmedId = null,
    presets = PRESETS,       // the street's preset row (for the number keys and the legend)
    compact = false,         // phone: the sizer caret is shown
    sizerOpen = false,
    keyHints = false,
    onAction = () => {},
    onCommitRaise = () => {},
    onPreset = () => {},
    onStep = () => {},
    onArmPre = () => {},
    onToggleSizer = () => {},
    onDismissError = () => {},
  } = $props();

  const sent = $derived(sentText !== null);
  const live = $derived(isMyTurn && gameInProgress && !actionPending && !sent);
  const showPre = $derived(gameInProgress && !isMyTurn && !sent && preOptions.length > 0);
  const raiseDisabled = $derived(!canRaise || raiseProblem !== null);

  // ALL IN NEEDS TWO PRESSES on the keyboard (the audit's 800 ms rule): the
  // first arms, the button says so, the second within the window fires.
  let allInArmedAt = $state(0);
  const ALL_IN_CONFIRM_MS = 800;
  const allInArmed = $derived(allInArmedAt > 0);
  $effect(() => {
    if (!allInArmedAt) return undefined;
    const id = setTimeout(() => { allInArmedAt = 0; }, ALL_IN_CONFIRM_MS);
    return () => clearTimeout(id);
  });

  // The dock's root, so a focused button INSIDE it can be told apart from a
  // modal's Close or the header's wallet ($lib/hotkeys.js has the rules).
  let dockEl = $state(null);

  function onKey(event) {
    if (typeof document === 'undefined') return;
    const decision = resolveHotkey(event.key, {
      live,
      modifier: event.metaKey || event.ctrlKey || event.altKey,
      focusKind: focusKindOf(document.activeElement, dockEl),
      dialogOpen: dialogIsOpen(document),
      canCheck, canRaise, raiseDisabled, allInArmed, compact, sizerOpen,
      presets,
    });
    if (!decision) return;
    switch (decision.type) {
      case 'action':
        if (decision.action === 'allin') allInArmedAt = 0;
        onAction(decision.action);
        break;
      case 'arm-allin': allInArmedAt = Date.now(); break;
      case 'commit-raise': onCommitRaise(); break;
      case 'preset': onPreset(decision.id); break;
      case 'step': onStep(decision.delta); break;
      case 'escape': allInArmedAt = 0; if (compact && sizerOpen) onToggleSizer(); break;
      default: return;
    }
    event.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="action-stack">
  {#if actionError}
    <div class="action-error" role="alert">
      <span>{actionError}</span>
      <button type="button" class="dismiss" onclick={onDismissError} aria-label="Dismiss">&times;</button>
    </div>
  {/if}

  <div
    class="actions"
    bind:this={dockEl}
    class:disabled={!isMyTurn || !gameInProgress || actionPending || sent}
    class:can-pre={showPre}
    class:sent
    class:urgent={clockUrgent}
  >
    {#if (live || sent) && gameInProgress}
      <span class="actions-clock" class:urgent={clockUrgent} style:--clock={clockFraction} aria-hidden="true"></span>
    {/if}

    {#if sent}
      <button type="button" class="action-btn secondary" class:pressed={sentKind === 'fold'} aria-pressed={sentKind === 'fold'} disabled>
        {sentKind === 'fold' ? sentText : 'Fold'}
      </button>
      <button type="button" class="action-btn primary" class:pressed={sentKind === 'check' || sentKind === 'call'} aria-pressed={sentKind === 'check' || sentKind === 'call'} disabled>
        {#if sentKind === 'check' || sentKind === 'call'}{sentText}{:else if canCheck}Check{:else}Call {fmt(callAmount)}{/if}
      </button>
      {#if canRaise || sentKind === 'raise' || sentKind === 'bet'}
        <button type="button" class="action-btn raise" class:pressed={sentKind === 'raise' || sentKind === 'bet'} aria-pressed={sentKind === 'raise' || sentKind === 'bet'} disabled>
          {#if sentKind === 'raise' || sentKind === 'bet'}{sentText}{:else}{raiseLabel} {fmt(raiseAmount)}{/if}
        </button>
      {/if}
      <button type="button" class="action-btn danger" class:pressed={sentKind === 'allin'} aria-pressed={sentKind === 'allin'} disabled>
        {sentKind === 'allin' ? sentText : 'All in'}
      </button>
    {:else if !gameInProgress}
      <div class="no-game-message">{noGameText}</div>
    {:else if !isMyTurn}
      {#if showPre}
        <PreActions options={preOptions} armedId={preArmedId} {waitingFor} onArm={onArmPre} />
      {:else}
        <div class="not-your-turn">Waiting for {waitingFor}</div>
      {/if}
    {:else}
      <button type="button" class="action-btn secondary" onclick={() => onAction('fold')} title="Fold (F)">
        <u>F</u>old
      </button>
      {#if canCheck}
        <button type="button" class="action-btn primary" onclick={() => onAction('check')} title="Check (C)">
          <u>C</u>heck
        </button>
      {:else}
        <button type="button" class="action-btn primary" onclick={() => onAction('call')} title="Call (C)">
          <u>C</u>all {fmt(callAmount)}
        </button>
      {/if}
      {#if canRaise}
        <span class="raise-group">
          <!-- THE PHONE'S RAISE CELL is two lines (the word, then the money at
               the small size) so a wide figure ("Raise 12.50") never runs
               into the caret: measured at one line the amount ended flush
               against the caret's border, and one run clipped it entirely. -->
          <button
            type="button"
            class="action-btn raise"
            class:has-caret={compact}
            class:stacked={compact}
            disabled={raiseDisabled}
            onclick={onCommitRaise}
            title="{raiseLabel} {fmt(raiseAmount)} (R)"
          >
            <span class="raise-word"><u>{raiseLabel.charAt(0)}</u>{compact ? raiseLabel.slice(1).replace(/ to$/, '') : raiseLabel.slice(1)}</span>
            <span class="raise-amt">{fmt(raiseAmount)}</span>
          </button>
          {#if compact}
            <button
              type="button"
              class="action-btn raise caret"
              class:open={sizerOpen}
              onclick={onToggleSizer}
              aria-expanded={sizerOpen}
              aria-label={sizerOpen ? 'Close bet size' : 'Choose bet size'}
            >
              <svg class="caret-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
                <path d="M6 15l6-6 6 6"/>
              </svg>
            </button>
          {/if}
        </span>
      {/if}
      <button
        type="button"
        class="action-btn danger"
        class:armed={allInArmed}
        onclick={() => { allInArmedAt = 0; onAction('allin'); }}
        title="All in (A, twice)"
      >
        {#if allInArmed}Press A again{:else}<u>A</u>ll in{/if}
      </button>
    {/if}
  </div>

  {#if keyHints && live}
    <div class="key-hints" aria-hidden="true">
      <span><kbd>F</kbd> fold</span>
      <span><kbd>C</kbd> {canCheck ? 'check' : 'call'}</span>
      {#if canRaise}<span><kbd>R</kbd> raise</span><span><kbd>1</kbd>-<kbd>{presets.length}</kbd> sizes</span><span><kbd>+</kbd><kbd>-</kbd> blind</span>{/if}
      <span><kbd>A</kbd><kbd>A</kbd> all in</span>
    </div>
  {/if}
</div>

<style>
  .action-stack {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--cd-space-1);
    min-width: 0;
    width: 100%;
  }

  .action-error {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    max-width: 100%;
    padding: var(--cd-space-1) var(--cd-space-2) var(--cd-space-1) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-danger-line);
    background: var(--cd-danger-dim);
    color: var(--cd-ink);
    font-size: var(--cd-text-xs);
    line-height: 1.3;
  }

  .action-error .dismiss {
    flex: 0 0 auto;
    width: var(--cd-space-5);
    height: var(--cd-space-5);
    border-radius: var(--cd-radius-chip);
    border: 0;
    background: transparent;
    color: var(--cd-ink-1);
    font-size: var(--cd-text-md);
    line-height: 1;
    cursor: pointer;
  }

  .actions {
    position: relative;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: var(--cd-space-2);
    min-width: 0;
    flex-wrap: nowrap;
    padding-top: var(--cd-space-1);
  }

  .actions.disabled { opacity: 0.45; pointer-events: none; }
  /* The pre-action toggles are live while the row is otherwise disabled. */
  .actions.disabled.can-pre { opacity: 1; pointer-events: auto; }
  /* The sent state keeps the row at full strength; the unpressed buttons dim. */
  .actions.sent { opacity: 1; }
  .actions.sent .action-btn:not(.pressed) { opacity: 0.35; }
  .actions.sent .action-btn.pressed { opacity: 1; filter: saturate(0.6) brightness(0.92); }

  /* THE CLOCK LINE: half a space unit (2 px) across the top of the row, the
     same fraction as the pod ring, red in the last ten seconds. Held while a
     send is open. */
  .actions-clock {
    position: absolute;
    left: 0;
    top: 0;
    height: calc(var(--cd-space-1) / 2);
    width: calc(var(--clock, 0) * 100%);
    border-radius: var(--cd-radius-pill);
    background: var(--cd-accent);
    transition: width 1s linear;
  }

  .actions-clock.urgent { background: var(--cd-danger); }
  .actions.urgent .action-btn.secondary,
  .actions.urgent .action-btn.primary { animation: urgent-pulse 1s ease-in-out infinite; }

  @keyframes urgent-pulse {
    0%, 100% { box-shadow: 0 0 0 0 var(--cd-danger-line); }
    50% { box-shadow: 0 0 0 var(--cd-space-1) var(--cd-danger-dim); }
  }

  .no-game-message, .not-your-turn {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-touch-min);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line-soft);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-sm);
    white-space: nowrap;
  }

  /* THE BUTTONS: 44 px everywhere (the touch floor), one radius, tone by
     role, a real pressed state and a focus ring. */
  .action-btn {
    flex: 0 1 auto;
    min-width: calc(var(--cd-touch-min) + var(--cd-space-6));
    min-height: var(--cd-touch-min);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    border: 1px solid transparent;
    font-family: inherit;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    white-space: nowrap;
    -webkit-tap-highlight-color: transparent;
    transition: transform var(--cd-fast) var(--cd-ease), filter var(--cd-fast) var(--cd-ease),
                opacity var(--cd-base) var(--cd-ease);
  }

  .action-btn u { text-decoration: underline; text-decoration-thickness: from-font; text-underline-offset: calc(var(--cd-space-1) * 0.75); }
  .action-btn:hover { transform: translateY(-1px); filter: brightness(1.1); }
  .action-btn:active { transform: scale(0.97); filter: brightness(0.9); }
  .action-btn:focus-visible { outline: 2px solid var(--cd-accent-hi); outline-offset: 2px; }
  .action-btn:disabled { cursor: default; }
  .action-btn:disabled:not(.pressed) { filter: saturate(0.4); }

  .action-btn.secondary {
    background: var(--cd-surface-2);
    border-color: var(--cd-line-strong);
    color: var(--cd-ink-1);
  }

  .action-btn.primary {
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
  }

  .action-btn.raise {
    background: linear-gradient(180deg, var(--cd-money-hi), var(--cd-money-lo));
    color: var(--cd-money-ink);
  }

  .action-btn.danger {
    background: var(--cd-danger);
    color: var(--cd-danger-ink);
  }

  .action-btn.danger.armed { animation: urgent-pulse 0.4s ease-in-out infinite; }

  .raise-word, .raise-amt { white-space: nowrap; }
  /* Two lines on the phone: the word over the money. */
  .action-btn.stacked {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0;
    line-height: 1.15;
    padding: 0 var(--cd-space-2);
  }
  .action-btn.stacked .raise-amt { font-size: var(--cd-text-sm); }

  .raise-group { display: inline-flex; gap: calc(var(--cd-space-1) / 2); min-width: 0; }
  .action-btn.has-caret { border-top-right-radius: calc(var(--cd-radius-chip) / 2); border-bottom-right-radius: calc(var(--cd-radius-chip) / 2); }
  .action-btn.caret {
    flex: 0 0 var(--cd-touch-min);
    min-width: var(--cd-touch-min);
    width: var(--cd-touch-min);
    padding: 0;
    border-top-left-radius: calc(var(--cd-radius-chip) / 2);
    border-bottom-left-radius: calc(var(--cd-radius-chip) / 2);
  }
  .caret-glyph { width: var(--cd-text-md); height: var(--cd-text-md); transition: transform var(--cd-base) var(--cd-ease); }
  .action-btn.caret.open .caret-glyph { transform: rotate(180deg); }

  /* Pointer devices only: the hotkey legend under the row. */
  .key-hints {
    display: flex;
    gap: var(--cd-space-3);
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    white-space: nowrap;
  }

  kbd {
    display: inline-block;
    min-width: 1.2em;
    padding: 0 var(--cd-space-1);
    border-radius: calc(var(--cd-radius-chip) / 2);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: inherit;
    line-height: 1.3;
    text-align: center;
  }

  @media (max-aspect-ratio: 1/1) {
    .actions { flex-wrap: nowrap; gap: calc(var(--cd-space-1) * 1.5); width: 100%; }
    .action-btn {
      flex: 1 1 0;
      min-width: 0;
      min-height: calc(var(--cd-touch-min) + var(--cd-space-1));
      padding: 0 var(--cd-space-1);
      font-size: var(--cd-text-md);
    }
    .action-btn u { text-decoration: none; }
    /* Fold needs the least room; the raise cell (two lines plus the 44 px
       caret) the most. Measured at 390 px: Fold 68, Call 85, Raise 73 + 44,
       All in 85, every cell at or above the touch floor. */
    .action-btn.secondary { flex: 0.8 1 0; }
    .raise-group { flex: 1.4 1 0; }
    /* The row is ALWAYS these four cells: the time bank lives on the pod
       clock (TimeBankPill), never here, so nothing squeezes the raise label. */
    .action-btn.stacked { padding: 0 var(--cd-space-2); min-width: var(--cd-touch-min); }
    .action-btn.caret { flex: 0 0 var(--cd-touch-min); min-width: var(--cd-touch-min); width: var(--cd-touch-min); }
    .no-game-message, .not-your-turn { min-height: calc(var(--cd-touch-min) + var(--cd-space-1)); font-size: var(--cd-text-md); }
    .key-hints { display: none; }
    /* The clock line is read from 240 px away on a phone: a full space unit
       (4 px), not the desktop's hairline (the audit measured 3 px as
       invisible at arm's length). */
    .actions-clock { height: var(--cd-space-1); }
  }

  @media (min-aspect-ratio: 1/1) and (max-height: 560px) {
    .action-btn { padding: 0 var(--cd-space-3); font-size: var(--cd-text-sm); min-width: calc(var(--cd-touch-min) + var(--cd-space-5)); }
    .key-hints { display: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .actions-clock { transition: none; }
    .actions.urgent .action-btn, .action-btn.danger.armed { animation: none; }
  }
</style>
