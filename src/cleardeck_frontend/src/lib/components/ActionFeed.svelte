<script>
  import HashSeal from './HashSeal.svelte';
  import { hashFingerprint } from '$lib/hash-seal.js';
  import { SEAL_HINT, SEAL_WORD, checkSeal, revealedSeedOf, sealKey, sealStateFor } from '$lib/deck-seal.js';

  const {
    actions = [],
    previousActions = [],
    mySeat = null,
    handNumber = 0,
    previousHandNumber = 0,
    shuffleProof = null,
    onShowProof = null,
    /**
     * Money formatter, injected by the table so the log uses the SAME precision
     * as every figure on the felt. Without this the log re-derived its own
     * precision per value and printed "0.0000" beside "50.00".
     */
    format = null,
    /**
     * The page footer's two links. On a phone the table view is one screen
     * with no footer (routes/app-phone.scss), so "How it works" and "Verify
     * code" ride the bottom of this drawer instead; null hides the row.
     */
    onHowItWorks = null,
    onVerifyCode = null,
    /** Closes the drawer from inside it (the dock's Log toggle also does). */
    onClose = null
  } = $props();

  // Toggle to show previous hand
  let showPreviousHand = $state(false);

  // Which actions to display
  const displayActions = $derived(showPreviousHand ? previousActions : actions);
  const displayHandNumber = $derived(showPreviousHand ? previousHandNumber : handNumber);

  /**
   * THE SEAL, phase by phase (lib/deck-seal.js, shared with the deck object on
   * the felt so the two can never disagree): `sealed` while the hand runs,
   * `revealed` once the seed is published, `checked` once THIS browser has
   * hashed the revealed seed and found the commitment, `mismatch` if it did
   * not. The full verdict (the cards re-derived) is the fairness panel's; this
   * chip is the moment made visible in the log.
   */
  let checkedFor = $state(null);   // the sealKey this browser has hashed
  let checkResult = $state(null);  // true (match) | false (mismatch)
  const revealedSeed = $derived(revealedSeedOf(shuffleProof));
  const sealState = $derived(sealStateFor({
    seedHash: shuffleProof?.seed_hash || null, revealedSeed, checkedKey: checkedFor, checkResult,
  }));

  $effect(() => {
    const hash = shuffleProof?.seed_hash;
    const seed = revealedSeed;
    if (!hash || !seed) return;
    const key = sealKey(hash, seed);
    if (checkedFor === key) return;
    let cancelled = false;
    checkSeal(hash, seed).then((ok) => {
      if (cancelled) return;
      checkResult = ok;
      checkedFor = key;
    });
    return () => { cancelled = true; };
  });

  // Format e8s amount as ICP display. One precision for the whole log.
  function formatChips(e8s) {
    if (format) return format(e8s);
    const num = typeof e8s === 'bigint' ? Number(e8s) : e8s;
    return (Number(num) / 100_000_000).toLocaleString('en-US', {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    });
  }

  /** HH:MM, per docs/DESIGN-BAR.md bar 17 (PokerNow's Session Log floor). */
  function clockOf(ts) {
    if (!ts) return '';
    const d = new Date(Number(ts) > 1e14 ? Number(ts) / 1e6 : Number(ts));
    if (Number.isNaN(d.getTime())) return '';
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
  }

  // Get player name for display
  function getPlayerName(seat) {
    if (seat === mySeat) return 'You';
    return `Seat ${seat + 1}`;
  }

  // Get action class for styling
  function getActionClass(type) {
    switch (type) {
      case 'fold': return 'action-fold';
      case 'check': return 'action-check';
      case 'call': return 'action-call';
      case 'bet': return 'action-bet';
      case 'raise': return 'action-raise';
      case 'allin': return 'action-allin';
      case 'blind': return 'action-blind';
      case 'phase': return 'action-phase';
      case 'showdown': return 'action-showdown';
      case 'winner': return 'action-winner';
      default: return '';
    }
  }
</script>

<div class="action-feed">
  <div class="feed-header">
    <div class="feed-header-top">
      <span class="feed-title">Hand #{displayHandNumber}</span>
      {#if previousActions.length > 0}
        <button
          class="toggle-btn"
          class:active={showPreviousHand}
          onclick={() => showPreviousHand = !showPreviousHand}
          title={showPreviousHand ? 'Show current hand' : 'Show previous hand'}
        >
          {showPreviousHand ? 'Current' : 'Prev'}
        </button>
      {/if}
      {#if onClose}
        <button type="button" class="feed-close" onclick={onClose} aria-label="Close the action log" title="Close">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>
        </button>
      {/if}
    </div>
    <span class="feed-subtitle">{showPreviousHand ? 'Previous Hand' : 'Action Log'}</span>
  </div>

  <div class="feed-list">
    {#if sealState && !showPreviousHand}
      <button class="fairness-indicator state-{sealState}" onclick={onShowProof} title={SEAL_HINT[sealState]}>
        <HashSeal hash={shuffleProof.seed_hash} size={28} tone={sealState === 'checked' ? 'accent' : sealState === 'mismatch' ? 'muted' : 'accent'} />
        <span class="seal-body">
          <span class="seal-row">
            <span class="fairness-title">Deck seal</span>
            <span class="seal-state">{SEAL_WORD[sealState]}</span>
          </span>
          <span class="hash-value hash" title={shuffleProof.seed_hash}>{hashFingerprint(shuffleProof.seed_hash)}</span>
        </span>
      </button>
    {/if}
    {#if displayActions.length === 0}
      <div class="feed-empty">
        <span>Waiting for action...</span>
      </div>
    {:else}
      {#each displayActions as action, i (i)}
        <div class="feed-item {getActionClass(action.type)}" class:is-me={action.seat === mySeat}>
          <span class="action-time">{clockOf(action.timestamp)}</span>
          <div class="action-content">
            {#if action.type === 'phase'}
              <span class="phase-text">{action.text}</span>
            {:else if action.type === 'showdown'}
              <!-- The hand that was turned up, in words. No amount: the pot that
                   moved is the NEXT line, and one number belongs in one place. -->
              <span class="player-name">{getPlayerName(action.seat)}</span>
              <span class="showdown-text">{action.text}</span>
            {:else if action.type === 'winner'}
              <span class="winner-name">{getPlayerName(action.seat)}</span>
              <span class="action-text">won</span>
              <!-- SIGNED, like the delta chip on the pod. The log is where a
                   player reconstructs a session, and "24.00" does not say
                   whether it arrived or left. Same token, same value, same
                   assertion; it just states its direction. -->
              <span class="action-amount">+{formatChips(action.amount)}</span>
            {:else}
              <span class="player-name">{getPlayerName(action.seat)}</span>
              <span class="action-text">{action.text}</span>
              {#if action.amount}
                <span class="action-amount">{formatChips(action.amount)}</span>
              {/if}
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  {#if onHowItWorks || onVerifyCode}
    <div class="feed-links">
      {#if onHowItWorks}
        <button type="button" class="feed-link" onclick={onHowItWorks}>How it works</button>
      {/if}
      {#if onVerifyCode}
        <button type="button" class="feed-link" onclick={onVerifyCode}>Verify code</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .action-feed {
    display: flex;
    flex-direction: column;
    background: var(--cd-panel);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-card);
    overflow: hidden;
    /* Sized by the container the table gives it, not by a fixed width. */
    width: 100%;
    max-height: 100%;
    min-height: 0;
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
  }

  .feed-header {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--cd-space-3) var(--cd-space-3);
    background: var(--cd-surface-1);
    border-bottom: 1px solid var(--cd-line-soft);
  }

  .feed-header-top { display: flex; justify-content: space-between; align-items: center; gap: var(--cd-space-2); }

  .feed-title { font-size: var(--cd-text-md); font-weight: var(--cd-weight-figure); color: var(--cd-ink); letter-spacing: 0.02em; }

  .toggle-btn {
    padding: 3px var(--cd-space-2);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    color: var(--cd-ink-2);
    cursor: pointer;
    transition: background var(--cd-fast) var(--cd-ease), color var(--cd-fast) var(--cd-ease);
  }

  .toggle-btn:hover { background: var(--cd-surface-3); color: var(--cd-ink-1); }

  .feed-close {
    margin-left: auto;
    width: var(--cd-control-sm);
    height: var(--cd-control-sm);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    color: var(--cd-ink-2);
    cursor: pointer;
    transition: background var(--cd-fast) var(--cd-ease), color var(--cd-fast) var(--cd-ease);
  }

  .feed-close:hover { background: var(--cd-surface-3); color: var(--cd-ink); }

  /* THE PHONE SHEET'S HEADER IS ONE ROW: "ACTION LOG  Hand #1 ...... [x]".
     The sheet between the stage and the dock is ~200 px tall; a two-row
     header and the links row left it a sliver of log (measured: the seal chip
     cut in half, no action line in view). */
  @media (max-aspect-ratio: 1/1) {
    .feed-close { width: var(--cd-touch-min); height: var(--cd-touch-min); }
    .feed-header { flex-direction: row; align-items: center; gap: var(--cd-space-2); padding: var(--cd-space-1) var(--cd-space-2) var(--cd-space-1) var(--cd-space-3); }
    .feed-header-top { flex: 1 1 auto; min-width: 0; }
    .feed-subtitle { order: -1; flex: 0 0 auto; }
  }
  .toggle-btn.active { background: var(--cd-accent-dim); border-color: var(--cd-accent-line); color: var(--cd-accent); }

  .feed-subtitle { font-size: var(--cd-text-xs); color: var(--cd-ink-2); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); }

  .feed-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--cd-space-2);
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-1);
  }

  .feed-list::-webkit-scrollbar { width: 4px; }
  .feed-list::-webkit-scrollbar-track { background: var(--cd-surface-1); }
  .feed-list::-webkit-scrollbar-thumb { background: var(--cd-line); border-radius: 2px; }

  .feed-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--cd-space-5) var(--cd-space-4);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-sm);
  }

  .feed-item {
    display: flex;
    align-items: baseline;
    gap: var(--cd-space-2);
    padding: 6px var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-surface-1);
    animation: slideIn var(--cd-base) var(--cd-ease);
    border-left: 2px solid transparent;
  }

  @keyframes slideIn {
    from { opacity: 0; transform: translateX(-10px); }
    to { opacity: 1; transform: translateX(0); }
  }

  .feed-item.is-me { background: var(--cd-accent-dim); border-left-color: var(--cd-accent-line-strong); }

  .action-content {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--cd-space-1);
    font-size: var(--cd-text-sm);
    line-height: 1.3;
  }

  .player-name { font-weight: var(--cd-weight-strong); color: var(--cd-ink-1); }
  .action-text { color: var(--cd-ink-2); }
  .action-amount { font-weight: var(--cd-weight-figure); color: var(--cd-money); font-variant-numeric: tabular-nums; }

  .action-time {
    flex-shrink: 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    font-family: var(--cd-font-mono);
    font-variant-numeric: tabular-nums;
  }

  /* Phase transitions */
  .action-phase { background: var(--cd-surface-2); border-left-color: var(--cd-accent-line); }
  .phase-text { color: var(--cd-accent); font-weight: var(--cd-weight-strong); font-size: var(--cd-text-xs); text-transform: uppercase; letter-spacing: 0.05em; }

  /* docs/DESIGN-BAR.md bar 17: the VERB carries the colour. Folds red, calls
     teal, bets and raises gold, all-ins red and bold, blinds and checks quiet. */
  .action-fold .action-text { color: var(--cd-danger-hi); }
  .action-check .action-text { color: var(--cd-ink-2); }
  .action-call .action-text { color: var(--cd-accent-hi); }
  .action-bet .action-text, .action-raise .action-text { color: var(--cd-money); }
  .action-raise { background: var(--cd-money-dim); }
  .action-allin { background: var(--cd-danger-dim); border-left-color: var(--cd-danger-line); }
  .action-allin .action-text { color: var(--cd-danger-hi); font-weight: var(--cd-weight-strong); }
  .action-allin .action-amount { color: var(--cd-danger-hi); }
  .action-blind .action-text, .action-blind .action-amount { color: var(--cd-ink-2); }

  /* Showdown reveal, the named hand, one step below a win in weight */
  .action-showdown { background: var(--cd-accent-dim); border-left-color: var(--cd-accent-line); }
  .showdown-text { color: var(--cd-accent-hi); font-weight: var(--cd-weight-strong); }

  /* Winner */
  .action-winner { background: var(--cd-money-dim); border-left-color: var(--cd-money); }
  .winner-name { font-weight: var(--cd-weight-figure); color: var(--cd-money); }
  .action-winner .action-amount { color: var(--cd-accent); font-weight: var(--cd-weight-display); }

  /* THE SEAL CHIP: the hand's commitment as an object with a state. */
  .fairness-indicator {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    padding: var(--cd-space-2) var(--cd-space-3);
    margin-bottom: var(--cd-space-1);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    cursor: pointer;
    transition: background var(--cd-base) var(--cd-ease), border-color var(--cd-base) var(--cd-ease);
    width: 100%;
    text-align: left;
    color: inherit;
    font: inherit;
  }

  .fairness-indicator:hover { background: var(--cd-surface-2); border-color: var(--cd-line-strong); }
  .fairness-indicator.state-checked { border-color: var(--cd-accent-line); background: var(--cd-accent-dim); }
  .fairness-indicator.state-mismatch { border-color: var(--cd-danger-line); background: var(--cd-danger-dim); }

  .seal-body { display: flex; flex-direction: column; gap: 1px; min-width: 0; flex: 1; }
  .seal-row { display: flex; align-items: baseline; justify-content: space-between; gap: var(--cd-space-2); }
  .fairness-title { font-size: var(--cd-text-xs); font-weight: var(--cd-weight-figure); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); color: var(--cd-ink-2); }
  .seal-state { font-size: var(--cd-text-xs); font-weight: var(--cd-weight-figure); text-transform: uppercase; letter-spacing: 0.06em; color: var(--cd-warn); }
  .state-revealed .seal-state { color: var(--cd-ink-1); }
  .state-checked .seal-state { color: var(--cd-accent); }
  .state-mismatch .seal-state { color: var(--cd-danger-hi); }
  .hash-value { font-size: var(--cd-text-xs); font-family: var(--cd-font-mono); color: var(--cd-accent); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* Responsive */
  /* The feed is sized by its container (a column beside the stage, a sheet
     between the stage and the dock); a fixed width here made the phone's
     sheet a 180 px strip. */
  @media (max-width: 1200px) {
    .feed-item { padding: var(--cd-space-1) var(--cd-space-2); }
    .action-content { font-size: var(--cd-text-xs); }
  }

  /* The two page links at the drawer's foot, at the touch floor everywhere
     (a drawer is a thumb surface on the phone and costs nothing on desktop). */
  .feed-links {
    flex: 0 0 auto;
    display: flex;
    gap: var(--cd-space-1);
    padding: var(--cd-space-1) var(--cd-space-2) calc(var(--cd-space-1) + var(--cd-safe-bottom));
    border-top: 1px solid var(--cd-line-soft);
  }

  .feed-link {
    flex: 1 1 0;
    min-height: var(--cd-touch-min);
    padding: 0 var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
    background: transparent;
    color: var(--cd-ink-2);
    font-family: inherit;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .feed-link:hover { color: var(--cd-ink); border-color: var(--cd-line); }

  /* The drawer used to be `display: none` under 900 px, which left the
     phone's Log button opening nothing. It is an overlay sized by
     `.feed-container` (PokerTable.svelte) at every viewport now. */
</style>
