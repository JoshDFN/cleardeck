<script>
  import { history } from '$lib/canisters';
  import Card from './Card.svelte';
  import logger from '$lib/logger.js';
  import { auth } from '$lib/auth.js';

  const { tableId = null, playerId = null, onClose, tableActor = null, handNumber = 0 } = $props();

  let hands = $state([]);
  let allHands = $state([]); // Cache of all hands
  let loading = $state(true);
  let selectedHand = $state(null);
  let loadingHand = $state(false);
  let error = $state(null);
  let offset = $state(0);
  let copied = $state(null);
  const limit = 20;

  // Get current user's principal (as string for comparison)
  let myPrincipal = $state(null);
  $effect(() => {
    const unsub = auth.subscribe(s => {
      // Ensure principal is always stored as string for consistent comparison
      myPrincipal = s.principal?.toString ? s.principal.toString() : s.principal;
    });
    return unsub;
  });

  // Load hands on mount
  $effect(() => {
    loadHands();
  });

  async function loadHands() {
    loading = true;
    error = null;
    try {
      // If we have a tableActor, query the table canister's hand history directly
      if (tableActor && handNumber > 0) {
        const loadedHands = [];
        // Load last N hands from the table canister
        const hn = Number(handNumber);
        const startHand = Math.max(1, hn - limit + 1);
        for (let i = hn; i >= startHand; i--) {
          try {
            const result = await tableActor.get_hand_history(BigInt(i));
            if (result && result.length > 0) {
              const hand = result[0];
              // Calculate player count from winners, showdown_players, or actions
              const playerCount = Math.max(
                hand.winners?.length || 0,
                hand.showdown_players?.length || 0,
                new Set(hand.actions?.map(a => a.seat) || []).size
              );

              // Get list of player principals who participated
              // Include both winners AND showdown_players (all who showed down, not just winners)
              const participantPrincipals = [];

              // Add winners
              if (hand.winners) {
                hand.winners.forEach(w => {
                  const pStr = w.principal?.toString ? w.principal.toString() : w.principal;
                  if (pStr && !participantPrincipals.includes(pStr)) {
                    participantPrincipals.push(pStr);
                  }
                });
              }

              // Add showdown players (includes losers who went to showdown)
              if (hand.showdown_players) {
                hand.showdown_players.forEach(p => {
                  const pStr = p.principal?.toString ? p.principal.toString() : p.principal;
                  if (pStr && !participantPrincipals.includes(pStr)) {
                    participantPrincipals.push(pStr);
                  }
                });
              }

              loadedHands.push({
                hand_id: BigInt(i),
                hand_number: Number(hand.hand_number),
                timestamp: hand.shuffle_proof?.timestamp || BigInt(0),
                total_pot: hand.winners?.reduce((sum, w) => sum + Number(w.amount), 0) || 0,
                player_count: playerCount || 2,
                winners: hand.winners || [],
                went_to_showdown: hand.community_cards && hand.community_cards.length >= 5,
                shuffle_proof: hand.shuffle_proof,
                actions: hand.actions || [],
                community_cards: hand.community_cards || [],
                participant_principals: participantPrincipals
              });
            }
          } catch (e) {
            logger.debug(`Hand ${i} not found`);
          }
        }
        allHands = loadedHands;
        filterHands();
      } else if (playerId) {
        hands = await history.get_hands_by_player(playerId, BigInt(offset), BigInt(limit));
      } else if (tableId) {
        hands = await history.get_hands_by_table(tableId, BigInt(offset), BigInt(limit));
      } else {
        hands = await history.get_recent_hands(BigInt(limit));
      }
    } catch (e) {
      logger.error('Failed to load hand history:', e);
      error = e.message || 'Failed to load history';
    }
    loading = false;
  }

  function filterHands() {
    // Filter to only hands where current player participated
    if (myPrincipal) {
      hands = allHands.filter(h =>
        h.participant_principals?.includes(myPrincipal)
      );
    } else {
      hands = allHands;
    }
  }

  async function loadFullHand(handId) {
    loadingHand = true;
    let handLoaded = false;

    try {
      // If we have a tableActor, try it first
      if (tableActor) {
        try {
          const result = await tableActor.get_hand_history(handId);
          if (result && result.length > 0 && result[0]) {
            const hand = result[0];
            // Convert table canister format (community_cards array) to history format (flop/turn/river)
            // Table canister returns HandHistory with community_cards: Vec<Card>
            const communityCards = hand.community_cards || [];

            // Use showdown_players if available (all players who went to showdown), otherwise fall back to winners
            const showdownPlayers = hand.showdown_players || [];
            const hasShowdownData = showdownPlayers.length > 0;

            selectedHand = {
              hand_id: handId,
              hand_number: hand.hand_number,
              timestamp: hand.shuffle_proof?.timestamp || BigInt(0),
              total_pot: hand.winners?.reduce((sum, w) => sum + Number(w.amount || 0), 0) || 0,
              shuffle_proof: hand.shuffle_proof,
              actions: hand.actions || [],
              winners: hand.winners || [],
              went_to_showdown: hasShowdownData || communityCards.length >= 5,
              // Convert community_cards array to flop/turn/river format
              flop: communityCards.length >= 3
                ? [communityCards[0], communityCards[1], communityCards[2]]
                : null,
              turn: communityCards.length >= 4
                ? [communityCards[3]]
                : null,
              river: communityCards.length >= 5
                ? [communityCards[4]]
                : null,
              // Use showdown_players for all players who showed cards (not just winners)
              // Note: Candid optional types come as arrays: [] for None, [value] for Some
              // So p.cards is [[Card, Card]] when present, not [Card, Card]
              players: hasShowdownData
                ? showdownPlayers.map(p => {
                    // Handle Candid optional: cards is [] or [[card1, card2]]
                    const cardsTuple = Array.isArray(p.cards) && p.cards.length > 0 ? p.cards[0] : null;
                    return {
                      seat: p.seat,
                      principal: p.principal,
                      starting_chips: 0,
                      ending_chips: 0,
                      hole_cards: cardsTuple ? [cardsTuple[0], cardsTuple[1]] : null,
                      final_hand_rank: p.hand_rank,
                      amount_won: Number(p.amount_won) || 0,
                      position: `Seat ${p.seat + 1}`
                    };
                  })
                : (hand.winners ? hand.winners.map(w => {
                    // Handle Candid optional: cards is [] or [[card1, card2]]
                    const cardsTuple = Array.isArray(w.cards) && w.cards.length > 0 ? w.cards[0] : null;
                    return {
                      seat: w.seat,
                      principal: w.principal,
                      starting_chips: 0,
                      ending_chips: 0,
                      hole_cards: cardsTuple ? [cardsTuple[0], cardsTuple[1]] : null,
                      final_hand_rank: w.hand_rank,
                      amount_won: Number(w.amount) || 0,
                      position: `Seat ${w.seat + 1}`
                    };
                  }) : [])
            };
            handLoaded = true;
          }
        } catch (tableError) {
          logger.debug('Table canister hand history not available, trying history canister');
        }
      }

      // Fall back to history canister if table didn't have the hand
      if (!handLoaded) {
        try {
          const result = await history.get_hand(handId);
          if (result && result.length > 0) {
            selectedHand = result[0];
            handLoaded = true;
          }
        } catch (historyError) {
          logger.error('History canister failed:', historyError);
        }
      }

      if (!handLoaded) {
        logger.warn('Hand details not available for hand', handId);
        error = 'Hand details not available. The hand history may have been cleared during a canister upgrade.';
      }
    } catch (e) {
      logger.error('Failed to load hand:', e);
      error = 'Failed to load hand details';
    }
    loadingHand = false;
  }

  async function verifyHand(handId) {
    try {
      const result = await history.verify_hand_shuffle(handId);
      if ('Ok' in result) {
        return result.Ok;
      }
      return false;
    } catch (e) {
      logger.error('Verification failed:', e);
      return false;
    }
  }

  function formatTimestamp(ns) {
    if (!ns) return 'N/A';
    const ms = Number(ns) / 1_000_000;
    return new Date(ms).toLocaleString();
  }

  function formatChips(amount) {
    const num = typeof amount === 'bigint' ? Number(amount) : amount;
    // Convert e8s to ICP (1 ICP = 100,000,000 e8s)
    const icp = num / 100_000_000;
    if (icp >= 1000) return `${(icp / 1000).toFixed(2)}K ICP`;
    if (icp >= 1) return `${icp.toFixed(2)} ICP`;
    if (icp >= 0.01) return `${icp.toFixed(2)} ICP`;
    return `${icp.toFixed(4)} ICP`;
  }

  function getHandRankName(handRank) {
    if (!handRank) return null;
    // Handle Candid optional - could be [] or [value]
    const rank = Array.isArray(handRank) ? handRank[0] : handRank;
    if (!rank) return null;
    const keys = Object.keys(rank);
    if (keys.length === 0) return null;
    const key = keys[0];
    if (!key) return null;
    return key.replace(/([A-Z])/g, ' $1').trim();
  }

  function truncateHash(hash) {
    if (!hash || hash.length < 16) return hash;
    return `${hash.slice(0, 8)}...${hash.slice(-8)}`;
  }

  async function copyToClipboard(text, label) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      copied = label;
      setTimeout(() => { copied = null; }, 2000);
    } catch (e) {
      logger.error('Failed to copy:', e);
    }
  }

  // Client-side verification of a hand's shuffle proof
  async function verifyHandProof(hand) {
    if (!hand?.shuffle_proof?.seed_hash || !hand?.shuffle_proof?.revealed_seed) return;

    hand.verifying = true;
    selectedHand = { ...hand }; // Trigger reactivity

    try {
      const revealedSeed = hand.shuffle_proof.revealed_seed;
      const seedHash = hand.shuffle_proof.seed_hash;

      // Convert hex string to bytes
      const hexMatch = revealedSeed.match(/.{1,2}/g);
      if (!hexMatch) {
        hand.verificationResult = 'invalid';
        selectedHand = { ...hand };
        return;
      }
      const seedBytes = new Uint8Array(hexMatch.map(byte => parseInt(byte, 16)));

      // Hash with SHA-256
      const hashBuffer = await crypto.subtle.digest('SHA-256', seedBytes);
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

      hand.verificationResult = hashHex === seedHash ? 'valid' : 'invalid';
    } catch (e) {
      logger.error('Hand verification error:', e);
      hand.verificationResult = 'invalid';
    }

    hand.verifying = false;
    selectedHand = { ...hand }; // Trigger reactivity
  }
</script>

<div class="hand-history-modal">
  <div class="modal-backdrop" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="button" tabindex="-1" aria-label="Close modal"></div>

  <div class="modal-content">
    <div class="modal-header">
      <h2>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <polyline points="12,6 12,12 16,14"/>
        </svg>
        My Hand History
      </h2>
      <button class="close-btn" onclick={onClose} aria-label="Close hand history">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M18 6L6 18M6 6l12 12"/>
        </svg>
      </button>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <span>Loading history...</span>
      </div>
    {:else if error}
      <div class="error">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="8" x2="12" y2="12"/>
          <line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        <p>{error}</p>
        <button onclick={loadHands}>Retry</button>
      </div>
    {:else if hands.length === 0}
      <div class="empty">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
          <line x1="16" y1="2" x2="16" y2="6"/>
          <line x1="8" y1="2" x2="8" y2="6"/>
          <line x1="3" y1="10" x2="21" y2="10"/>
        </svg>
        <p>No hands recorded yet</p>
        <span class="hint">Play some hands and they'll appear here</span>
      </div>
    {:else if loadingHand}
      <div class="loading">
        <div class="spinner"></div>
        <span>Loading hand details...</span>
      </div>
    {:else if selectedHand}
      <!-- Detailed hand view -->
      <div class="hand-detail">
        <button class="back-btn" onclick={() => selectedHand = null}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15,18 9,12 15,6"/>
          </svg>
          Back to list
        </button>

        <div class="detail-header">
          <h3>Hand #{Number(selectedHand.hand_number)}</h3>
          <span class="timestamp">{formatTimestamp(selectedHand.timestamp)}</span>
        </div>

        <!-- Shuffle Proof -->
        <div class="proof-section">
          <h4>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
            Shuffle Proof
            {#if selectedHand.verificationResult === 'valid'}
              <span class="verified-badge">Verified</span>
            {/if}
          </h4>
          <div class="proof-data">
            <div class="proof-row">
              <span class="label">Seed Hash (Pre-committed):</span>
              <div class="hash-with-copy">
                <code title={selectedHand.shuffle_proof?.seed_hash}>{truncateHash(selectedHand.shuffle_proof?.seed_hash || '')}</code>
                <button class="copy-btn-small" onclick={() => copyToClipboard(selectedHand.shuffle_proof?.seed_hash, 'hash')}>
                  {copied === 'hash' ? '✓' : 'Copy'}
                </button>
              </div>
            </div>
            <div class="proof-row">
              <span class="label">Revealed Seed:</span>
              {#if selectedHand.shuffle_proof?.revealed_seed}
                <div class="hash-with-copy">
                  <code class="revealed" title={selectedHand.shuffle_proof.revealed_seed}>{truncateHash(selectedHand.shuffle_proof.revealed_seed)}</code>
                  <button class="copy-btn-small" onclick={() => copyToClipboard(selectedHand.shuffle_proof.revealed_seed, 'seed')}>
                    {copied === 'seed' ? '✓' : 'Copy'}
                  </button>
                </div>
              {:else}
                <code class="not-revealed">Not yet revealed</code>
              {/if}
            </div>
          </div>
          {#if selectedHand.shuffle_proof?.revealed_seed}
            <button
              class="verify-hand-btn"
              class:verified={selectedHand.verificationResult === 'valid'}
              class:invalid={selectedHand.verificationResult === 'invalid'}
              onclick={() => verifyHandProof(selectedHand)}
              disabled={selectedHand.verifying}
            >
              {#if selectedHand.verifying}
                <span class="mini-spinner"></span>
                Verifying...
              {:else if selectedHand.verificationResult === 'valid'}
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <polyline points="20,6 9,17 4,12"/>
                </svg>
                Verified Fair
              {:else if selectedHand.verificationResult === 'invalid'}
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/>
                  <line x1="15" y1="9" x2="9" y2="15"/>
                  <line x1="9" y1="9" x2="15" y2="15"/>
                </svg>
                Verification Failed
              {:else}
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                </svg>
                Verify This Hand
              {/if}
            </button>
          {/if}
        </div>

        <!-- Community Cards -->
        <div class="community-section">
          <h4>Community Cards</h4>
          <div class="community-cards">
            {#if selectedHand.flop && selectedHand.flop.length > 0}
              <Card card={selectedHand.flop[0]} />
              <Card card={selectedHand.flop[1]} />
              <Card card={selectedHand.flop[2]} />
            {/if}
            {#if selectedHand.turn && selectedHand.turn.length > 0}
              <div class="card-divider"></div>
              <Card card={selectedHand.turn[0]} />
            {/if}
            {#if selectedHand.river && selectedHand.river.length > 0}
              <div class="card-divider"></div>
              <Card card={selectedHand.river[0]} />
            {/if}
          </div>
        </div>

        <!-- Players -->
        <div class="players-section">
          <h4>Players</h4>
          <div class="players-list">
            {#if selectedHand.players && selectedHand.players.length > 0}
              {#each selectedHand.players as player}
                <div class="player-row" class:winner={player.amount_won > 0}>
                <div class="player-info">
                  <span class="position">{player.position}</span>
                  <span class="seat">Seat {player.seat + 1}</span>
                </div>
                <div class="player-cards">
                  {#if player.hole_cards && Array.isArray(player.hole_cards) && player.hole_cards.length >= 2}
                    <Card card={player.hole_cards[0]} small={true} />
                    <Card card={player.hole_cards[1]} small={true} />
                    {@const rankName = getHandRankName(player.final_hand_rank)}
                    {#if rankName}
                      <span class="hand-rank">{rankName}</span>
                    {/if}
                  {:else}
                    <span class="mucked">Mucked</span>
                  {/if}
                </div>
                <div class="player-result">
                  {#if player.amount_won > 0}
                    <span class="won">+{formatChips(player.amount_won)}</span>
                  {:else}
                    {@const loss = Number(player.starting_chips) - Number(player.ending_chips)}
                    {#if loss > 0}
                      <span class="lost">-{formatChips(loss)}</span>
                    {:else}
                      <span class="even">-</span>
                    {/if}
                  {/if}
                </div>
                </div>
              {/each}
            {:else}
              <div class="no-players">Player data not available for this hand</div>
            {/if}
          </div>
        </div>

        <!-- Pot Info -->
        <div class="pot-section">
          <div class="pot-total">
            <span class="label">Total Pot</span>
            <span class="amount">{formatChips(selectedHand.total_pot)}</span>
          </div>
          <div class="winners-list">
            {#each selectedHand.winners as winner}
              <div class="winner-row">
                <span>Seat {winner.seat + 1}</span>
                <span class="win-amount">+{formatChips(winner.amount)}</span>
                <span class="pot-type">({winner.pot_type})</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    {:else}
      <!-- Hands list -->
      <div class="hands-list">
        {#each hands as hand}
          <button class="hand-row" onclick={() => loadFullHand(hand.hand_id)}>
            <div class="hand-info">
              <span class="hand-number">Hand #{Number(hand.hand_number)}</span>
              <span class="hand-time">{formatTimestamp(hand.timestamp)}</span>
            </div>
            <div class="hand-stats">
              <span class="players">{hand.player_count} players</span>
              <span class="pot">Pot: {formatChips(hand.total_pot)}</span>
            </div>
            <div class="hand-result">
              {#if hand.winners && hand.winners.length > 0 && myPrincipal}
                {@const userWon = hand.winners.some(w => {
                  const winnerPrincipal = w.principal?.toString ? w.principal.toString() : w.principal;
                  return winnerPrincipal === myPrincipal;
                })}
                {@const userParticipated = hand.participant_principals?.some(p => {
                  const participantStr = p?.toString ? p.toString() : p;
                  return participantStr === myPrincipal;
                })}
                {@const rankName = getHandRankName(hand.winners[0].hand_rank)}
                {#if userWon}
                  <span class="winner-badge you-won">
                    You won!
                    {#if rankName}
                      ({rankName})
                    {/if}
                  </span>
                {:else if userParticipated}
                  <span class="winner-badge you-lost">
                    You lost
                  </span>
                {:else}
                  <!-- User wasn't in this hand, don't show win/loss badge -->
                {/if}
              {/if}
              {#if hand.went_to_showdown}
                <span class="showdown-badge">Showdown</span>
              {/if}
            </div>
            <svg class="chevron" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="9,6 15,12 9,18"/>
            </svg>
          </button>
        {/each}
      </div>

      {#if hands.length >= limit}
        <div class="pagination">
          <button onclick={() => { offset = Math.max(0, offset - limit); loadHands(); }} disabled={offset === 0}>
            Previous
          </button>
          <span>Page {Math.floor(offset / limit) + 1}</span>
          <button onclick={() => { offset += limit; loadHands(); }}>
            Next
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .hand-history-modal {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .modal-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
  }

  .modal-content {
    position: relative;
    width: 100%;
    max-width: 600px;
    max-height: 80vh;
    background: linear-gradient(145deg, rgba(25, 25, 40, 0.98), rgba(15, 15, 25, 0.98));
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 25px 80px rgba(0, 0, 0, 0.5);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .modal-header h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 18px;
    color: #fff;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.3);
    color: #ef4444;
  }

  .loading, .error, .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 20px;
    gap: 16px;
    color: #888;
  }

  .spinner {
    width: 32px;
    height: 32px;
    border: 3px solid rgba(255, 255, 255, 0.1);
    border-top-color: #00d4aa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error {
    color: #ef4444;
  }

  .error button {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
  }

  .empty {
    color: #666;
  }

  .empty svg {
    opacity: 0.3;
  }

  .empty p {
    margin: 0;
    font-size: 16px;
    color: #888;
  }

  .hint {
    font-size: 13px;
    color: #666;
  }

  /* Hands List */
  .hands-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .hand-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 16px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    margin-bottom: 8px;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
  }

  .hand-row:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .hand-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 100px;
  }

  .hand-number {
    font-weight: 600;
    color: #fff;
    font-size: 14px;
  }

  .hand-time {
    font-size: 11px;
    color: #666;
  }

  .hand-stats {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    color: #888;
  }

  .pot {
    color: #fbbf24;
    font-weight: 500;
  }

  .hand-result {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: flex-end;
  }

  .winner-badge {
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 500;
  }

  .winner-badge.you-won {
    background: rgba(46, 204, 113, 0.15);
    color: #2ecc71;
  }

  .winner-badge.you-lost {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  .showdown-badge {
    background: rgba(52, 152, 219, 0.15);
    color: #3498db;
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 10px;
  }

  .chevron {
    color: #444;
  }

  /* Hand Detail */
  .hand-detail {
    padding: 20px;
    overflow-y: auto;
    flex: 1;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    color: #888;
    font-size: 13px;
    cursor: pointer;
    padding: 8px 12px;
    margin: -8px -12px 16px;
    border-radius: 8px;
    transition: all 0.2s;
  }

  .back-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
  }

  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  .detail-header h3 {
    margin: 0;
    font-size: 20px;
    color: #fff;
  }

  .timestamp {
    color: #666;
    font-size: 12px;
  }

  .proof-section, .community-section, .players-section, .pot-section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 16px;
  }

  .proof-section h4, .community-section h4, .players-section h4 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 12px;
    font-size: 13px;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .proof-data {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .proof-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .proof-row .label {
    color: #666;
    min-width: 100px;
  }

  .proof-row code {
    background: rgba(0, 0, 0, 0.3);
    padding: 4px 8px;
    border-radius: 4px;
    color: #4ecdc4;
    font-family: monospace;
    font-size: 11px;
  }

  .proof-row code.revealed {
    color: #f1c40f;
  }

  .hash-with-copy {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
  }

  .copy-btn-small {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 10px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .copy-btn-small:hover {
    background: rgba(255, 255, 255, 0.15);
    color: white;
  }

  .verified-badge {
    background: rgba(46, 204, 113, 0.2);
    color: #2ecc71;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    margin-left: auto;
    text-transform: none;
  }

  .verify-hand-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    margin-top: 12px;
    padding: 10px 16px;
    background: linear-gradient(135deg, #3498db, #2980b9);
    border: none;
    border-radius: 8px;
    color: white;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .verify-hand-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(52, 152, 219, 0.4);
  }

  .verify-hand-btn:disabled {
    opacity: 0.7;
    cursor: wait;
  }

  .verify-hand-btn.verified {
    background: linear-gradient(135deg, #2ecc71, #27ae60);
  }

  .verify-hand-btn.invalid {
    background: linear-gradient(135deg, #e74c3c, #c0392b);
  }

  .mini-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .community-cards {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .card-divider {
    width: 1px;
    height: 40px;
    background: rgba(255, 255, 255, 0.1);
    margin: 0 4px;
  }

  .players-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .player-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .player-row.winner {
    border-color: rgba(46, 204, 113, 0.3);
    background: rgba(46, 204, 113, 0.05);
  }

  .player-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 60px;
  }

  .position {
    font-size: 11px;
    color: #fbbf24;
    font-weight: 600;
    text-transform: uppercase;
  }

  .seat {
    font-size: 10px;
    color: #666;
  }

  .player-cards {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
  }

  .hand-rank {
    font-size: 11px;
    color: #888;
    margin-left: 8px;
  }

  .mucked {
    font-size: 11px;
    color: #666;
    font-style: italic;
  }

  .player-result {
    font-weight: 600;
    font-size: 13px;
  }

  .won {
    color: #2ecc71;
  }

  .lost {
    color: #ef4444;
  }

  .even {
    color: #666;
  }

  .no-players {
    padding: 20px;
    text-align: center;
    color: #666;
    font-size: 14px;
    font-style: italic;
  }

  .not-revealed {
    color: #888;
    font-style: italic;
  }

  .pot-section {
    background: linear-gradient(135deg, rgba(251, 191, 36, 0.1), rgba(251, 191, 36, 0.05));
    border: 1px solid rgba(251, 191, 36, 0.2);
  }

  .pot-total {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .pot-total .label {
    color: #888;
    font-size: 12px;
    text-transform: uppercase;
  }

  .pot-total .amount {
    color: #fbbf24;
    font-size: 24px;
    font-weight: 700;
  }

  .winners-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .winner-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: #888;
  }

  .win-amount {
    color: #2ecc71;
    font-weight: 600;
  }

  .pot-type {
    color: #666;
    font-size: 11px;
  }

  .pagination {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 16px;
    padding: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .pagination button {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .pagination button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .pagination button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .pagination span {
    color: #666;
    font-size: 13px;
  }
</style>
