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
   *   sent         the SAME row, the pressed button marked and reading
   *                "Calling 0.10", the rest dimmed, the clock line held.
   *                No spinner: the felt already shows the echo.
   *   waiting      the pre-action toggles (PreActions), or a quiet pill when
   *                the hero cannot act this hand.
   *   no game      "Waiting for players" / "Hand complete".
   *
   * Keyboard (desktop): F fold, C check or call, R raise at the sizer's
   * figure, A twice for all in, 1-5 presets, + and - a big blind; never Enter or Space, which belong to whatever has focus,
   * confirms, Esc cancels. Inactive while any field has focus.
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
    offerBank = false,
    bankSecs = 0,
    actionError = null,
    preOptions = [],         // [] when the hero cannot pre-act
    preArmedId = null,
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
      presets: PRESETS,
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
          <button
            type="button"
            class="action-btn raise"
            class:has-caret={compact}
            disabled={raiseDisabled}
            onclick={onCommitRaise}
            title="{raiseLabel} {fmt(raiseAmount)} (R)"
          >
            <u>{raiseLabel.charAt(0)}</u>{compact ? raiseLabel.slice(1).replace(/ to$/, '') : raiseLabel.slice(1)} {fmt(raiseAmount)}
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
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
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
      {#if offerBank}
        <button type="button" class="action-btn ghost time-bank" onclick={() => onAction('useTimeBank')} title="Add time from your time bank">
          +{bankSecs}s
        </button>
      {/if}
    {/if}
  </div>

  {#if keyHints && live}
    <div class="key-hints" aria-hidden="true">
      <span><kbd>F</kbd> fold</span>
      <span><kbd>C</kbd> {canCheck ? 'check' : 'call'}</span>
      {#if canRaise}<span><kbd>R</kbd> raise</span><span><kbd>1</kbd>-<kbd>5</kbd> sizes</span><span><kbd>+</kbd><kbd>-</kbd> blind</span>{/if}
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
    padding: 4px var(--cd-space-2) 4px var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-danger-line);
    background: var(--cd-danger-dim);
    color: var(--cd-ink);
    font-size: var(--cd-text-xs);
    line-height: 1.3;
  }

  .action-error .dismiss {
    flex: 0 0 auto;
    width: 20px;
    height: 20px;
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
    padding-top: 3px;
  }

  .actions.disabled { opacity: 0.45; pointer-events: none; }
  /* The pre-action toggles are live while the row is otherwise disabled. */
  .actions.disabled.can-pre { opacity: 1; pointer-events: auto; }
  /* The sent state keeps the row at full strength; the unpressed buttons dim. */
  .actions.sent { opacity: 1; }
  .actions.sent .action-btn:not(.pressed) { opacity: 0.35; }
  .actions.sent .action-btn.pressed { opacity: 1; filter: saturate(0.6) brightness(0.92); }

  /* THE CLOCK LINE: 2 px across the top of the row, the same fraction as the
     pod ring, red in the last ten seconds. Held while a send is open. */
  .actions-clock {
    position: absolute;
    left: 0;
    top: 0;
    height: 2px;
    width: calc(var(--clock, 0) * 100%);
    border-radius: 1px;
    background: var(--cd-accent);
    transition: width 1s linear;
  }

  .actions-clock.urgent { background: var(--cd-danger); }
  .actions.urgent .action-btn.secondary,
  .actions.urgent .action-btn.primary { animation: urgent-pulse 1s ease-in-out infinite; }

  @keyframes urgent-pulse {
    0%, 100% { box-shadow: 0 0 0 0 var(--cd-danger-line); }
    50% { box-shadow: 0 0 0 3px var(--cd-danger-dim); }
  }

  .no-game-message, .not-your-turn {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-touch-min);
    padding: 0 18px;
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
    min-width: 78px;
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

  .action-btn u { text-decoration: underline; text-decoration-thickness: 1.5px; text-underline-offset: 3px; }
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

  .action-btn.ghost {
    background: transparent;
    border-color: var(--cd-line-strong);
    color: var(--cd-ink-2);
    min-width: 0;
    padding: 0 var(--cd-space-3);
  }

  .action-btn.ghost.time-bank { border-color: var(--cd-warn-line); color: var(--cd-warn); }

  .raise-group { display: inline-flex; gap: 2px; min-width: 0; }
  .action-btn.has-caret { border-top-right-radius: 4px; border-bottom-right-radius: 4px; }
  .action-btn.caret {
    min-width: 0;
    width: var(--cd-touch-min);
    padding: 0;
    border-top-left-radius: 4px;
    border-bottom-left-radius: 4px;
  }
  .action-btn.caret svg { transition: transform var(--cd-base) var(--cd-ease); }
  .action-btn.caret.open svg { transform: rotate(180deg); }

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
    padding: 0 3px;
    border-radius: 3px;
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: inherit;
    line-height: 1.3;
    text-align: center;
  }

  @media (max-aspect-ratio: 1/1) {
    .actions { flex-wrap: nowrap; gap: 6px; width: 100%; }
    .action-btn {
      flex: 1 1 0;
      min-width: 0;
      min-height: 48px;
      padding: 0 4px;
      font-size: var(--cd-text-md);
    }
    .action-btn u { text-decoration: none; }
    /* "Raise 0.30" plus its caret: the widest cell in the row. */
    .raise-group { flex: 1.7 1 0; }
    .action-btn.caret { flex: 0 0 40px; width: 40px; }
    .action-btn.ghost { flex: 0 0 auto; padding: 0 9px; }
    .no-game-message, .not-your-turn { min-height: 48px; font-size: var(--cd-text-md); }
    .key-hints { display: none; }
  }

  @media (min-aspect-ratio: 1/1) and (max-height: 560px) {
    .action-btn { padding: 0 12px; font-size: var(--cd-text-sm); min-width: 66px; }
    .key-hints { display: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .actions-clock { transition: none; }
    .actions.urgent .action-btn, .action-btn.danger.armed { animation: none; }
  }
</style>
