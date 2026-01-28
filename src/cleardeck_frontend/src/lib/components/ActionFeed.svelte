<script>
  const { actions = [], previousActions = [], mySeat = null, handNumber = 0, previousHandNumber = 0, shuffleProof = null, onShowProof = null } = $props();

  // Toggle to show previous hand
  let showPreviousHand = $state(false);

  // Which actions to display
  const displayActions = $derived(showPreviousHand ? previousActions : actions);
  const displayHandNumber = $derived(showPreviousHand ? previousHandNumber : handNumber);

  // Truncate hash for display
  function truncateHash(hash) {
    if (!hash || hash.length < 12) return hash || '';
    return `${hash.slice(0, 6)}...${hash.slice(-4)}`;
  }

  // Format e8s amount as ICP display
  function formatChips(e8s) {
    const num = typeof e8s === 'bigint' ? Number(e8s) : e8s;
    const icp = num / 100_000_000;
    if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K`;
    if (icp >= 1) return icp.toFixed(2);
    if (icp >= 0.01) return icp.toFixed(2);
    return icp.toFixed(4);
  }

  // Get player name for display
  function getPlayerName(seat) {
    if (seat === mySeat) return 'You';
    return `Seat ${seat + 1}`;
  }

  // Get action icon based on type
  function getActionIcon(type) {
    switch (type) {
      case 'fold': return '✕';
      case 'check': return '✓';
      case 'call': return '☎';
      case 'bet': return '●';
      case 'raise': return '▲';
      case 'allin': return '★';
      case 'blind': return '◐';
      case 'phase': return '→';
      case 'winner': return '♛';
      default: return '•';
    }
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
    </div>
    <span class="feed-subtitle">{showPreviousHand ? 'Previous Hand' : 'Action Log'}</span>
  </div>

  <div class="feed-list">
    {#if shuffleProof?.seed_hash && !showPreviousHand}
      <button class="fairness-indicator" onclick={onShowProof} title="Click to view full verification">
        <div class="fairness-header">
          <div class="fairness-icon">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
              <path d="M9 12l2 2 4-4"/>
            </svg>
          </div>
          <span class="fairness-title">Provably Fair</span>
          {#if shuffleProof.revealed_seed}
            <span class="verified-badge">✓</span>
          {/if}
        </div>
        <div class="fairness-steps">
          <span class="step">VRF</span>
          <span class="arrow">→</span>
          <span class="step">SHA256</span>
          <span class="arrow">→</span>
          <span class="step">Shuffle</span>
          <span class="arrow">→</span>
          <span class="step">Deal</span>
        </div>
        <div class="fairness-hash-row">
          <span class="hash-label">Commit:</span>
          <span class="hash-value">{truncateHash(shuffleProof.seed_hash)}</span>
        </div>
      </button>
    {/if}
    {#if displayActions.length === 0}
      <div class="feed-empty">
        <span class="empty-icon">🃏</span>
        <span>Waiting for action...</span>
      </div>
    {:else}
      {#each displayActions as action, i (i)}
        <div class="feed-item {getActionClass(action.type)}" class:is-me={action.seat === mySeat}>
          <span class="action-icon">{getActionIcon(action.type)}</span>
          <div class="action-content">
            {#if action.type === 'phase'}
              <span class="phase-text">{action.text}</span>
            {:else if action.type === 'winner'}
              <span class="winner-name">{getPlayerName(action.seat)}</span>
              <span class="action-text">won</span>
              <span class="action-amount">{formatChips(action.amount)}</span>
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
</div>

<style>
  .action-feed {
    display: flex;
    flex-direction: column;
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.95));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    overflow: hidden;
    width: 220px;
    max-height: 400px;
    box-shadow:
      0 10px 40px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .feed-header {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.02));
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .feed-header-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }

  .feed-title {
    font-size: 14px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: 0.5px;
  }

  .toggle-btn {
    padding: 3px 8px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    transition: all 0.15s;
  }

  .toggle-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.8);
  }

  .toggle-btn.active {
    background: rgba(99, 102, 241, 0.2);
    border-color: rgba(99, 102, 241, 0.4);
    color: #818cf8;
  }

  .feed-subtitle {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.4);
    text-transform: uppercase;
    letter-spacing: 1.5px;
  }

  .feed-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .feed-list::-webkit-scrollbar {
    width: 4px;
  }

  .feed-list::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.02);
  }

  .feed-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
  }

  .feed-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 16px;
    color: rgba(255, 255, 255, 0.3);
    font-size: 12px;
  }

  .empty-icon {
    font-size: 24px;
    opacity: 0.5;
  }

  .feed-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    animation: slideIn 0.2s ease-out;
    border-left: 2px solid transparent;
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateX(-10px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .feed-item.is-me {
    background: rgba(0, 212, 170, 0.08);
    border-left-color: rgba(0, 212, 170, 0.5);
  }

  .action-icon {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.6);
  }

  .action-content {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    font-size: 12px;
    line-height: 1.3;
  }

  .player-name {
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
  }

  .action-text {
    color: rgba(255, 255, 255, 0.5);
  }

  .action-amount {
    font-weight: 700;
    color: #fbbf24;
  }

  /* Phase transitions */
  .action-phase {
    background: linear-gradient(135deg, rgba(99, 102, 241, 0.1), rgba(79, 70, 229, 0.05));
    border-left-color: rgba(99, 102, 241, 0.5);
  }

  .action-phase .action-icon {
    background: rgba(99, 102, 241, 0.2);
    color: #818cf8;
  }

  .phase-text {
    color: #818cf8;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  /* Fold */
  .action-fold .action-icon {
    background: rgba(107, 114, 128, 0.2);
    color: #9ca3af;
  }

  /* Check */
  .action-check .action-icon {
    background: rgba(34, 197, 94, 0.2);
    color: #4ade80;
  }

  /* Call */
  .action-call .action-icon {
    background: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
  }

  /* Bet */
  .action-bet .action-icon {
    background: rgba(245, 158, 11, 0.2);
    color: #fbbf24;
  }

  .action-bet .action-amount {
    color: #fbbf24;
  }

  /* Raise */
  .action-raise {
    background: rgba(245, 158, 11, 0.06);
  }

  .action-raise .action-icon {
    background: rgba(245, 158, 11, 0.25);
    color: #f59e0b;
  }

  .action-raise .action-amount {
    color: #f59e0b;
  }

  /* All In */
  .action-allin {
    background: linear-gradient(135deg, rgba(239, 68, 68, 0.1), rgba(220, 38, 38, 0.05));
    border-left-color: rgba(239, 68, 68, 0.5);
  }

  .action-allin .action-icon {
    background: rgba(239, 68, 68, 0.25);
    color: #f87171;
    animation: pulse-icon 1s infinite;
  }

  @keyframes pulse-icon {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }

  .action-allin .action-amount {
    color: #f87171;
  }

  /* Blinds */
  .action-blind .action-icon {
    background: rgba(168, 85, 247, 0.2);
    color: #c084fc;
  }

  /* Winner */
  .action-winner {
    background: linear-gradient(135deg, rgba(234, 179, 8, 0.15), rgba(202, 138, 4, 0.08));
    border-left-color: #fbbf24;
  }

  .action-winner .action-icon {
    background: linear-gradient(135deg, #fbbf24, #f59e0b);
    color: #1a1a2e;
  }

  .winner-name {
    font-weight: 700;
    color: #fbbf24;
  }

  .action-winner .action-amount {
    color: #22c55e;
    font-weight: 800;
  }

  /* Fairness indicator */
  .fairness-indicator {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    margin-bottom: 8px;
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.1), rgba(39, 174, 96, 0.05));
    border: 1px solid rgba(46, 204, 113, 0.2);
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.2s;
    width: 100%;
    text-align: left;
  }

  .fairness-indicator:hover {
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.15), rgba(39, 174, 96, 0.08));
    border-color: rgba(46, 204, 113, 0.35);
  }

  .fairness-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .fairness-icon {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(46, 204, 113, 0.2);
    border-radius: 5px;
    color: #2ecc71;
  }

  .fairness-title {
    font-size: 11px;
    font-weight: 700;
    color: #2ecc71;
    flex: 1;
  }

  .verified-badge {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #2ecc71;
    color: #0a0a0f;
    border-radius: 50%;
    font-size: 9px;
    font-weight: 700;
  }

  .fairness-steps {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 4px 0;
  }

  .fairness-steps .step {
    font-size: 9px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.08);
    padding: 2px 5px;
    border-radius: 3px;
  }

  .fairness-steps .arrow {
    font-size: 9px;
    color: rgba(46, 204, 113, 0.6);
  }

  .fairness-hash-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .hash-label {
    font-size: 9px;
    color: rgba(255, 255, 255, 0.4);
    text-transform: uppercase;
  }

  .hash-value {
    font-size: 10px;
    font-family: 'Monaco', 'Consolas', monospace;
    color: rgba(78, 205, 196, 0.9);
  }

  /* Responsive */
  @media (max-width: 1200px) {
    .action-feed {
      width: 180px;
      max-height: 350px;
    }

    .feed-item {
      padding: 6px 8px;
    }

    .action-content {
      font-size: 11px;
    }
  }

  @media (max-width: 900px) {
    .action-feed {
      display: none;
    }
  }
</style>
