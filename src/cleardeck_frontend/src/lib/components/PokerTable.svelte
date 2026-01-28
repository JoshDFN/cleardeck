<script>
  import Card from './Card.svelte';
  import ActionFeed from './ActionFeed.svelte';
  import { playSound, setSoundEnabled, isSoundEnabled } from '$lib/sounds.js';

  const {
    tableState,
    myCards,
    onAction,
    actionPending = false,
    tableBalance = 0,
    onShowDeposit = null,
    onShowWithdraw = null,
    currency = 'ICP',
    avatarStyle = 'bottts',
    customName = null,
    shuffleProof = null,
    onShowProof = null
  } = $props();

  // Currency-specific formatting
  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';

  // Sound mute state - persisted in localStorage
  let soundMuted = $state(typeof localStorage !== 'undefined' && localStorage.getItem('poker_sound_muted') === 'true');

  function toggleSound() {
    soundMuted = !soundMuted;
    setSoundEnabled(!soundMuted);
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('poker_sound_muted', soundMuted.toString());
    }
  }

  // Initialize sound state on mount
  $effect(() => {
    setSoundEnabled(!soundMuted);
  });

  // Seat positions for different table sizes
  // Positions are arranged symmetrically around the table
  // Seat positions - players sit OUTSIDE the felt (like PokerStars/GGPoker)
  // y values: negative = above table, >100 = below table
  const seatPositions2 = [
    { x: 50, y: 100 },   // 0 - bottom center (hero) - below table
    { x: 50, y: 0 },     // 1 - top center (villain) - at top edge of table
  ];

  const seatPositions6 = [
    { x: 50, y: 100 },   // 0 - bottom center - below table
    { x: -4, y: 68 },    // 1 - left lower - left of table
    { x: -4, y: 32 },    // 2 - left upper - left of table
    { x: 50, y: 0 },     // 3 - top center - at top edge of table
    { x: 104, y: 32 },   // 4 - right upper - right of table
    { x: 104, y: 68 },   // 5 - right lower - right of table
  ];

  // 9-player layout: spread evenly around the oval
  // Bottom row: 3 seats, Top row: 2 seats, Sides: 2 each
  const seatPositions9 = [
    { x: 50, y: 102 },   // 0 - bottom center (hero position)
    { x: 18, y: 92 },    // 1 - bottom left
    { x: -6, y: 60 },    // 2 - left lower
    { x: -6, y: 40 },    // 3 - left upper
    { x: 22, y: 5 },     // 4 - top left
    { x: 78, y: 5 },     // 5 - top right
    { x: 106, y: 40 },   // 6 - right upper
    { x: 106, y: 60 },   // 7 - right lower
    { x: 82, y: 92 },    // 8 - bottom right
  ];

  // Select appropriate seat layout based on max_players
  const maxPlayers = $derived(tableState?.config?.max_players ?? 9);
  const seatPositions = $derived(
    maxPlayers <= 2 ? seatPositions2 :
    maxPlayers <= 6 ? seatPositions6 :
    seatPositions9
  );

  // Raise slider state
  let raiseAmount = $state(0);
  let showRaiseSlider = $state(false);
  let lastInitializedForTurn = $state(false); // Track if we've initialized for this turn

  function getPhaseDisplay(phase) {
    if (!phase) return 'Waiting';
    const keys = Object.keys(phase);
    if (keys.length === 0) return 'Waiting';
    const key = keys[0];
    if (!key) return 'Waiting';
    return key.replace(/([A-Z])/g, ' $1').trim();
  }

  // Safely format hand rank for display
  function formatHandRank(handRank) {
    if (!handRank) return '';
    const keys = Object.keys(handRank);
    if (keys.length === 0) return '';
    const key = keys[0];
    if (!key) return '';
    return key.replace(/([A-Z])/g, ' $1').trim();
  }

  // Format amount based on currency (sats for BTC, e8s for ICP)
  function formatAmount(smallestUnit, includeUnit = false) {
    const num = typeof smallestUnit === 'bigint' ? Number(smallestUnit) : smallestUnit;
    if (isBTC) {
      // BTC: display in sats or BTC depending on size
      if (num >= 100_000_000) return `${(num / 100_000_000).toFixed(2)} BTC`;
      if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M${includeUnit ? ' sats' : ''}`;
      if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K${includeUnit ? ' sats' : ''}`;
      return `${num}${includeUnit ? ' sats' : ''}`;
    } else {
      // ICP: display in ICP
      const icp = num / 100_000_000;
      if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K${includeUnit ? ' ICP' : ''}`;
      if (icp >= 1) return icp.toFixed(2) + (includeUnit ? ' ICP' : '');
      if (icp >= 0.01) return icp.toFixed(2) + (includeUnit ? ' ICP' : '');
      return icp.toFixed(4) + (includeUnit ? ' ICP' : '');
    }
  }

  // Format with unit included (for wallet display)
  function formatWithUnit(smallestUnit) {
    return formatAmount(smallestUnit, true);
  }

  // Legacy function name for templates
  function formatICP(e8s) {
    return formatAmount(e8s);
  }

  // Alias for backward compatibility in templates
  function formatChips(amount) {
    return formatAmount(amount);
  }

  const phase = $derived(getPhaseDisplay(tableState?.phase));
  const phaseKey = $derived(
    tableState?.phase && Object.keys(tableState.phase).length > 0
      ? Object.keys(tableState.phase)[0]
      : 'WaitingForPlayers'
  );
  const pot = $derived(Number(tableState?.pot ?? 0));
  const sidePots = $derived(tableState?.side_pots || []);

  // Calculate total chips in play at the table (all player stacks + pot + side pots)
  const totalTableChips = $derived(() => {
    let total = pot;
    // Add all player stacks
    for (const player of players) {
      if (player) {
        total += Number(player.chips ?? 0);
        total += Number(player.current_bet ?? 0);
      }
    }
    // Add side pots
    for (const sp of sidePots) {
      total += Number(sp.amount ?? 0);
    }
    return total;
  });

  // Calculate total bets in current round (chips committed but not yet in pot)
  const currentRoundBets = $derived(() => {
    let total = 0;
    for (const player of players) {
      if (player) {
        total += Number(player.current_bet ?? 0);
      }
    }
    return total;
  });
  const communityCards = $derived(tableState?.community_cards || []);
  const players = $derived((tableState?.players || []).map(p => (p && p.length > 0) ? p[0] : null));
  const actionOn = $derived(tableState?.action_on ?? 0);
  const currentBet = $derived(Number(tableState?.current_bet ?? 0));
  const minRaise = $derived(Number(tableState?.min_raise ?? tableState?.config?.big_blind ?? 0));
  const mySeat = $derived(tableState?.my_seat?.length > 0 ? tableState.my_seat[0] : null);
  const isMyTurn = $derived(tableState?.is_my_turn === true);
  const gameInProgress = $derived(phaseKey !== 'WaitingForPlayers' && phaseKey !== 'HandComplete');
  const myPlayer = $derived(mySeat !== null && mySeat < players.length ? players[mySeat] : null);

  // Use backend-calculated values when available, fallback to client calculation
  const callAmount = $derived(Number(tableState?.call_amount ?? (myPlayer ? Math.max(0, currentBet - Number(myPlayer.current_bet ?? 0)) : 0)));
  const canCheck = $derived(tableState?.can_check ?? (myPlayer !== null && callAmount === 0));
  const canRaise = $derived(tableState?.can_raise ?? (Number(myPlayer?.chips ?? 0) > callAmount));
  const myChips = $derived(Number(myPlayer?.chips ?? 0));

  // Timer display - client-side countdown for smooth ticking
  // We store the server time and when we received it, then calculate display time locally
  let serverTimeRemaining = $state(null);
  let lastServerUpdate = $state(0);
  let displayedTimeRemaining = $state(null);
  let countdownInterval = null;

  // Sync from server when we get new data
  $effect(() => {
    const serverTime = tableState?.time_remaining_secs?.length > 0
      ? Number(tableState.time_remaining_secs[0])
      : null;

    // Only update if server sent a meaningful change (new hand, action changed, etc.)
    // Small differences (< 2 seconds) are just timing jitter from polling
    if (serverTime !== null) {
      const diff = Math.abs((serverTimeRemaining ?? 0) - serverTime);
      if (serverTimeRemaining === null || diff > 2 || serverTime > serverTimeRemaining) {
        serverTimeRemaining = serverTime;
        lastServerUpdate = Date.now();
        displayedTimeRemaining = serverTime;
      }
    } else {
      serverTimeRemaining = null;
      displayedTimeRemaining = null;
    }
  });

  // Client-side countdown - tick every second
  $effect(() => {
    if (countdownInterval) {
      clearInterval(countdownInterval);
      countdownInterval = null;
    }

    if (displayedTimeRemaining !== null && displayedTimeRemaining > 0) {
      countdownInterval = setInterval(() => {
        const elapsed = Math.floor((Date.now() - lastServerUpdate) / 1000);
        const newTime = Math.max(0, (serverTimeRemaining ?? 0) - elapsed);
        displayedTimeRemaining = newTime;
      }, 1000);
    }

    return () => {
      if (countdownInterval) {
        clearInterval(countdownInterval);
        countdownInterval = null;
      }
    };
  });

  // Alias for compatibility
  const timeRemaining = $derived(displayedTimeRemaining);

  // Play timer warning sound - only when timer is low and it's our turn
  let lastTimerSound = $state(0);
  let timerSoundInterval = null;

  $effect(() => {
    // Clear any existing interval
    if (timerSoundInterval) {
      clearInterval(timerSoundInterval);
      timerSoundInterval = null;
    }

    // Set up timer sound if conditions are met
    if (timeRemaining !== null && timeRemaining !== undefined && isMyTurn && timeRemaining <= 10 && timeRemaining > 0) {
      // Play sound immediately
      playSound('timer', { frequency: 800, duration: 30 });
      lastTimerSound = Date.now();

      // Then play every second
      timerSoundInterval = setInterval(() => {
        if (displayedTimeRemaining > 0 && displayedTimeRemaining <= 10) {
          playSound('timer', { frequency: 800, duration: 30 });
          lastTimerSound = Date.now();
        } else {
          clearInterval(timerSoundInterval);
          timerSoundInterval = null;
        }
      }, 1000);
    }

    // Cleanup
    return () => {
      if (timerSoundInterval) {
        clearInterval(timerSoundInterval);
        timerSoundInterval = null;
      }
    };
  });
  const timeBankRemaining = $derived(
    tableState?.time_bank_remaining_secs?.length > 0
      ? Number(tableState.time_bank_remaining_secs[0])
      : 0
  );
  const usingTimeBank = $derived(tableState?.using_time_bank || false);

  // Get the actual timeout for the current action (action_timeout_secs or time_bank_secs if using time bank)
  const actionTimeout = $derived(
    usingTimeBank
      ? Number(tableState?.config?.time_bank_secs ?? 30)
      : Number(tableState?.config?.action_timeout_secs ?? 60)
  );

  // Position indicators
  const dealerSeat = $derived(tableState?.dealer_seat);
  const smallBlindSeat = $derived(tableState?.small_blind_seat);
  const bigBlindSeat = $derived(tableState?.big_blind_seat);

  // Winners from last hand
  const lastWinners = $derived(tableState?.last_hand_winners || []);
  const isHandComplete = $derived(phaseKey === 'HandComplete');
  const isShowdown = $derived(phaseKey === 'Showdown' || phaseKey === 'HandComplete'); // Showdown or hand complete - cards should be revealed
  const myWinInfo = $derived(
    mySeat !== null
      ? lastWinners.find(w => w.seat === mySeat)
      : null
  );

  // Sitting out status - check player status
  const isSittingOut = $derived(
    myPlayer?.status && Object.keys(myPlayer.status).length > 0 && Object.keys(myPlayer.status)[0] === 'SittingOut'
  );

  // Last action for notification display
  const lastAction = $derived(tableState?.last_action?.[0] || tableState?.last_action);

  // Format the last action for display
  function formatLastAction(actionInfo) {
    if (!actionInfo) return null;
    const action = actionInfo.action;
    if (!action) return null;
    const seat = actionInfo.seat;
    const isMe = seat === mySeat;
    const playerName = isMe ? 'You' : `Seat ${seat + 1}`;

    // Get the action type (key of the variant)
    const actionType = Object.keys(action)[0];
    const actionData = action[actionType];

    switch (actionType) {
      case 'Fold':
        return { player: playerName, action: 'folded', amount: null, type: 'fold' };
      case 'Check':
        return { player: playerName, action: 'checked', amount: null, type: 'check' };
      case 'Call':
        return { player: playerName, action: 'called', amount: actionData?.amount || actionData, type: 'call' };
      case 'Bet':
        return { player: playerName, action: 'bet', amount: actionData?.amount || actionData, type: 'bet' };
      case 'Raise':
        return { player: playerName, action: 'raised to', amount: actionData?.amount || actionData, type: 'raise' };
      case 'AllIn':
        return { player: playerName, action: 'went ALL IN', amount: actionData?.amount || actionData, type: 'allin' };
      case 'PostBlind':
        return { player: playerName, action: 'posted blind', amount: actionData?.amount || actionData, type: 'blind' };
      default:
        return null;
    }
  }

  const formattedLastAction = $derived(formatLastAction(lastAction));

  // Action feed - tracks all actions for the current hand
  let actionFeed = $state([]);
  let previousActionFeed = $state([]);
  let previousHandNumber = $state(0);
  let lastTrackedAction = $state(null);
  let lastTrackedPhase = $state(null);
  let lastTrackedHandNumber = $state(0);

  // Track hand number for resetting the feed
  const handNumber = $derived(tableState?.hand_number ?? 0);

  // Reset action feed when hand number changes, save previous
  $effect(() => {
    if (handNumber !== lastTrackedHandNumber && handNumber > 0) {
      // Save current feed as previous before resetting (if it has meaningful content)
      if (actionFeed.length > 0) {
        previousActionFeed = [...actionFeed];
        previousHandNumber = lastTrackedHandNumber;
      }
      actionFeed = [];
      lastTrackedAction = null;
      lastTrackedPhase = null;
      lastTrackedHandNumber = handNumber;
    }
  });

  // Track phase changes
  $effect(() => {
    if (phaseKey && phaseKey !== lastTrackedPhase && phaseKey !== 'WaitingForPlayers') {
      // Add phase change to feed
      const phaseNames = {
        'PreFlop': 'Pre-Flop',
        'Flop': 'Flop',
        'Turn': 'Turn',
        'River': 'River',
        'Showdown': 'Showdown',
        'HandComplete': 'Hand Complete'
      };

      if (phaseNames[phaseKey] && lastTrackedPhase !== null) {
        actionFeed = [...actionFeed, {
          type: 'phase',
          text: phaseNames[phaseKey],
          timestamp: Date.now()
        }];
      }
      lastTrackedPhase = phaseKey;
    }
  });

  // Track player actions
  $effect(() => {
    if (lastAction) {
      const actionKey = `${lastAction.seat}-${lastAction.timestamp}-${JSON.stringify(lastAction.action)}`;
      if (actionKey !== lastTrackedAction) {
        const formatted = formatLastAction(lastAction);
        if (formatted) {
          actionFeed = [...actionFeed, {
            type: formatted.type,
            seat: lastAction.seat,
            text: formatted.action,
            amount: formatted.amount,
            timestamp: lastAction.timestamp
          }];
        }
        lastTrackedAction = actionKey;
      }
    }
  });

  // Track winners
  $effect(() => {
    if (isHandComplete && lastWinners.length > 0) {
      // Check if we already added winners for this hand
      const hasWinners = actionFeed.some(a => a.type === 'winner');
      if (!hasWinners) {
        for (const winner of lastWinners) {
          actionFeed = [...actionFeed, {
            type: 'winner',
            seat: winner.seat,
            text: 'won',
            amount: winner.amount,
            timestamp: Date.now()
          }];
        }
      }
    }
  });

  // Pot odds calculation - returns ratio like "5:1" (pot to call ratio)
  function getPotOdds() {
    if (callAmount <= 0 || pot <= 0) return null;
    const ratio = pot / callAmount;
    // Format as X:1 ratio
    if (ratio >= 1) {
      return `${ratio.toFixed(1)}:1`;
    } else {
      return `1:${(1/ratio).toFixed(1)}`;
    }
  }

  // Calculate equity needed to call profitably
  function getEquityNeeded() {
    if (callAmount <= 0 || pot <= 0) return null;
    const totalPot = pot + callAmount;
    return (callAmount / totalPot * 100).toFixed(0);
  }

  // Minimum bet amount (for opening bet when no one has bet yet)
  const minBet = $derived(Number(tableState?.min_bet ?? tableState?.config?.big_blind ?? 10));

  // Reset initialization tracking when it's not our turn anymore
  $effect(() => {
    if (!isMyTurn) {
      lastInitializedForTurn = false;
    }
  });

  // Initialize raise amount ONLY once when turn starts (not on every value change)
  $effect(() => {
    if (isMyTurn && gameInProgress && !lastInitializedForTurn) {
      lastInitializedForTurn = true;
      if (canCheck) {
        // No bet yet - minimum bet is the big blind
        raiseAmount = minBet;
      } else {
        // Someone has bet - minimum raise is current bet + min raise increment
        raiseAmount = currentBet + minRaise;
      }
    }
  });

  function handleRaise() {
    if (raiseAmount > 0) {
      onAction('raise', raiseAmount);
      showRaiseSlider = false;
    }
  }

  function handleBet() {
    if (raiseAmount > 0) {
      onAction('bet', raiseAmount);
      showRaiseSlider = false;
    }
  }

  // Calculate max bet/raise amount (total chips available)
  const maxBetAmount = $derived(myChips + Number(myPlayer?.current_bet ?? 0));

  // Preset bet amounts - always capped at what player can afford
  function setBetPreset(multiplier) {
    // Use minBet for opening bet (no current bet), minRaise increment for raises
    const minAmount = currentBet === 0 ? minBet : (currentBet + minRaise);
    let targetAmount;
    if (multiplier === 'half') {
      targetAmount = Math.max(minAmount, Math.floor(pot / 2));
    } else if (multiplier === 'pot') {
      targetAmount = Math.max(minAmount, pot);
    } else if (multiplier === 'allin') {
      targetAmount = maxBetAmount;
    }
    // Cap at player's max
    raiseAmount = Math.min(targetAmount, maxBetAmount);
  }

  // Generate avatar URL using DiceBear API with principal as seed
  // Uses avatarStyle prop for current user's avatar, 'bottts' for others
  function getAvatarUrl(player, seatIndex = null) {
    if (!player?.principal) return null;
    const principalStr = player.principal.toString();
    // Use user's preferred style (from prop) for themselves, default 'bottts' for others
    const style = (seatIndex === mySeat) ? avatarStyle : 'bottts';
    // scale=110 fills the circle better, radius=50 makes it circular
    return `https://api.dicebear.com/7.x/${style}/svg?seed=${encodeURIComponent(principalStr)}&size=68&scale=110&radius=50`;
  }

  // Generate a fun username from principal
  function getPlayerName(player) {
    if (!player?.principal) return 'Unknown';
    const principalStr = player.principal.toString();

    // Adjectives and nouns for generating names
    const adjectives = ['Lucky', 'Wild', 'Cool', 'Sly', 'Bold', 'Swift', 'Clever', 'Daring', 'Epic', 'Mystic', 'Royal', 'Shadow', 'Golden', 'Silver', 'Cosmic'];
    const nouns = ['Ace', 'King', 'Queen', 'Jack', 'Joker', 'Shark', 'Whale', 'Fox', 'Wolf', 'Tiger', 'Eagle', 'Hawk', 'Viper', 'Dragon', 'Phoenix'];

    // Use principal hash to pick consistent names
    let hash = 0;
    for (let i = 0; i < principalStr.length; i++) {
      hash = ((hash << 5) - hash) + principalStr.charCodeAt(i);
      hash = hash & hash; // Convert to 32bit integer
    }
    hash = Math.abs(hash);

    const adj = adjectives[hash % adjectives.length];
    const noun = nouns[(hash >> 8) % nouns.length];
    const num = (hash % 100).toString().padStart(2, '0');

    return `${adj}${noun}${num}`;
  }

  // Wallet panel collapsed state - persisted in localStorage
  let walletCollapsed = $state(typeof localStorage !== 'undefined' && localStorage.getItem('poker_wallet_collapsed') === 'true');

  function toggleWalletPanel() {
    walletCollapsed = !walletCollapsed;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('poker_wallet_collapsed', walletCollapsed.toString());
    }
  }

  // Shorter display name for tight spaces
  function getShortName(player, seatIndex) {
    if (seatIndex === mySeat) {
      // Use custom name if set, otherwise "You"
      return customName || 'You';
    }
    if (!player?.principal) return `Seat ${seatIndex + 1}`;
    // Use display_name from backend if set, otherwise generate one
    if (player.display_name) {
      const name = player.display_name;
      return name.length > 10 ? name.slice(0, 10) : name;
    }
    const fullName = getPlayerName(player);
    // Return first 10 chars if too long
    return fullName.length > 10 ? fullName.slice(0, 10) : fullName;
  }
</script>

<div class="poker-table-wrapper">
  <!-- Action Feed on the left with turn indicator above it -->
  <div class="feed-container left">
    <!-- Turn indicator - matches ActionFeed styling -->
    {#if gameInProgress && mySeat !== null}
      <div class="turn-indicator" class:my-turn={isMyTurn} class:waiting={!isMyTurn} class:time-bank={usingTimeBank}>
        {#if isMyTurn}
          <div class="turn-header">
            <span class="turn-title">Your Turn</span>
            {#if timeRemaining !== undefined && timeRemaining !== null}
              <span class="turn-timer" class:urgent={timeRemaining <= 10}>{timeRemaining}s</span>
            {/if}
          </div>
          <span class="turn-hint">
            {#if canCheck}
              Check or Bet
            {:else}
              Call {formatChips(callAmount)} or Raise
            {/if}
          </span>
          {#if usingTimeBank}
            <span class="time-bank-badge">TIME BANK</span>
          {/if}
        {:else}
          <div class="turn-header">
            <span class="turn-title">Waiting</span>
            {#if timeRemaining !== undefined && timeRemaining !== null}
              <span class="turn-timer">{timeRemaining}s</span>
            {/if}
          </div>
          <span class="turn-hint">{getPlayerName(players[actionOn])} to act</span>
        {/if}
      </div>
    {/if}
    <ActionFeed actions={actionFeed} previousActions={previousActionFeed} mySeat={mySeat} handNumber={handNumber} previousHandNumber={previousHandNumber} {shuffleProof} {onShowProof} />
  </div>

  <div class="poker-table">
    <!-- Top right controls -->
    <div class="top-controls">
      <!-- Sound mute button -->
      <button class="sound-toggle" onclick={toggleSound} title={soundMuted ? 'Unmute sounds' : 'Mute sounds'}>
        {#if soundMuted}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 5L6 9H2v6h4l5 4V5z"/>
            <line x1="23" y1="9" x2="17" y2="15"/>
            <line x1="17" y1="9" x2="23" y2="15"/>
          </svg>
        {:else}
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 5L6 9H2v6h4l5 4V5z"/>
            <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
            <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
          </svg>
      {/if}
    </button>
    </div>

  <!-- Overlay banners container - for sitting-out notification -->
  <div class="overlay-banners">
    <!-- Sitting out notification -->
    {#if isSittingOut}
      <div class="sitting-out-banner">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
        </svg>
        <span>You are sitting out</span>
        <button class="sit-in-btn" onclick={() => onAction('sitIn')}>
          Sit Back In
        </button>
      </div>
    {/if}

  </div>

  <!-- The felt table -->
  <div class="felt">
    <!-- Community cards area -->
    <div class="community-area">
      {#if isHandComplete && lastWinners.length > 0}
        <!-- Winner display on the table -->
        <div class="winner-display" class:you-won={myWinInfo}>
          {#if myWinInfo}
            <span class="winner-text">You won {formatChips(Number(myWinInfo.amount))} {currencySymbol}!</span>
            {#if myWinInfo.hand_rank && formatHandRank(myWinInfo.hand_rank)}
              <span class="winner-hand-rank">{formatHandRank(myWinInfo.hand_rank)}</span>
            {/if}
          {:else}
            <span class="winner-text">Seat {lastWinners[0].seat + 1} wins {formatChips(Number(lastWinners[0].amount))} {currencySymbol}</span>
            {#if lastWinners[0].hand_rank && formatHandRank(lastWinners[0].hand_rank)}
              <span class="winner-hand-rank">{formatHandRank(lastWinners[0].hand_rank)}</span>
            {/if}
          {/if}
          {#if lastWinners.length > 1}
            <span class="split-info">Split pot ({lastWinners.length} winners)</span>
          {/if}
        </div>
      {:else}
        <div class="pot-display">
          <div class="main-pot" class:has-chips={pot > 0 || currentRoundBets() > 0}>
            <span class="pot-label">POT</span>
            <span class="pot-amount">
              {#if pot > 0 || currentRoundBets() > 0}
                {formatChips(pot + currentRoundBets())}
              {:else}
                --
              {/if}
            </span>
            {#if currentRoundBets() > 0 && pot > 0}
              <span class="pot-breakdown">({formatChips(pot)} + {formatChips(currentRoundBets())} betting)</span>
            {/if}
          </div>
          {#if sidePots.length > 0}
            <div class="side-pots">
              {#each sidePots as sidePot, i}
                <div class="side-pot">
                  <span class="side-pot-label">Side {i + 1}</span>
                  <span class="side-pot-amount">{formatChips(sidePot.amount)}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <div class="community-cards">
        {#each Array(5) as _, i}
          {#if communityCards[i]}
            <Card card={communityCards[i]} />
          {:else if gameInProgress}
            <!-- Show face-down cards for undealt community cards during a hand -->
            <Card faceDown={true} />
          {:else}
            <!-- Show empty placeholder when waiting for players -->
            <Card card={null} />
          {/if}
        {/each}
      </div>
      <div class="phase-indicator">{phase}</div>
    </div>

    <!-- Player seats -->
    {#each players as player, i}
      {#if i < seatPositions.length}
        <div
          class="seat"
          class:occupied={player}
          class:action-on={player && i === actionOn}
          class:is-me={i === mySeat}
          class:folded={player?.has_folded}
          style:left="{seatPositions[i].x}%"
          style:top="{seatPositions[i].y}%"
        >
          {#if player}
            <div class="player-info">
              {#if i === actionOn && gameInProgress}
                <div class="action-indicator">
                  {i === mySeat ? 'YOUR TURN' : 'THINKING...'}
                </div>
              {/if}
              <!-- Avatar with position badges -->
              <div class="avatar-wrapper">
                <!-- Position badges - positioned next to avatar toward table center -->
                {#if i === dealerSeat || i === smallBlindSeat || i === bigBlindSeat}
                  <div class="position-badges"
                       class:badges-top={seatPositions[i].y > 50}
                       class:badges-bottom={seatPositions[i].y <= 50}
                       class:badges-left={seatPositions[i].x > 80}
                       class:badges-right={seatPositions[i].x < 20}>
                    {#if i === dealerSeat}
                      <span class="position-badge dealer">D</span>
                    {/if}
                    {#if i === smallBlindSeat}
                      <span class="position-badge sb">SB</span>
                    {/if}
                    {#if i === bigBlindSeat}
                      <span class="position-badge bb">BB</span>
                    {/if}
                  </div>
                {/if}
              </div>
              <!-- Avatar + Nameplate together -->
              <div class="player-nameplate" class:highlight-me={i === mySeat} class:action-on={i === actionOn && gameInProgress}>
                <div class="avatar-container" class:is-me={i === mySeat}>
                  <img
                    src={getAvatarUrl(player, i)}
                    alt="Avatar"
                    class="player-avatar"
                  />
                  {#if player.has_folded}
                    <div class="avatar-overlay folded">FOLD</div>
                  {:else if player.is_all_in}
                    <div class="avatar-overlay allin">ALL IN</div>
                  {/if}
                </div>
                <div class="player-info">
                  <span class="player-name" class:is-me={i === mySeat}>{getShortName(player, i)}</span>
                  <span class="chips">{formatChips(player.chips)}</span>
                </div>
                {#if player.current_bet > 0}
                  <span class="bet-amount">{formatChips(player.current_bet)}</span>
                {/if}
              </div>
              <!-- Cards below -->
              <div class="player-cards">
                {#if i === mySeat && myCards && (gameInProgress || isShowdown)}
                  <!-- Show our cards face up during active hand or showdown/hand complete -->
                  <Card card={myCards[0]} />
                  <Card card={myCards[1]} />
                {:else if player.hole_cards && player.hole_cards.length > 0}
                  <!-- Backend returns hole_cards when they should be visible (showdown, voluntarily shown) -->
                  <Card card={player.hole_cards[0]} />
                  <Card card={player.hole_cards[1]} />
                {:else if gameInProgress && !player.has_folded}
                  <!-- Face down during play -->
                  <Card faceDown={true} />
                  <Card faceDown={true} />
                {:else if isShowdown && !player.has_folded}
                  <!-- At showdown, show face down if cards not yet revealed by backend -->
                  <Card faceDown={true} />
                  <Card faceDown={true} />
                {/if}
              </div>
            </div>
          {:else}
            <button class="join-seat" onclick={() => onAction('join', i)}>
              Seat {i + 1}
            </button>
          {/if}
        </div>
      {/if}
    {/each}
  </div>

  <!-- Pot odds display -->
  {#if isMyTurn && callAmount > 0}
    {@const potOdds = getPotOdds()}
    {@const equityNeeded = getEquityNeeded()}
    {#if potOdds}
      <div class="pot-odds-display">
        <div class="pot-odds-main">
          <span class="pot-odds-label">Pot Odds</span>
          <span class="pot-odds-value">{potOdds}</span>
        </div>
        <div class="pot-odds-explanation">
          <span>Call <strong>{formatChips(callAmount)}</strong> to win <strong>{formatChips(pot)}</strong></span>
          <span class="equity-hint">Need {equityNeeded}%+ equity to profit</span>
        </div>
      </div>
    {/if}
  {/if}

  <!-- Raise slider -->
  {#if showRaiseSlider && isMyTurn && gameInProgress}
    <div class="raise-slider-panel">
      <div class="slider-header">
        <span>{canCheck ? 'Bet Amount' : 'Raise To'}</span>
        <button class="close-slider" onclick={() => showRaiseSlider = false}>x</button>
      </div>
      <div class="slider-amount">{formatChips(raiseAmount)}</div>
      <input
        type="range"
        class="raise-slider"
        min={currentBet === 0 ? minBet : currentBet + minRaise}
        max={myChips + Number(myPlayer?.current_bet ?? 0)}
        bind:value={raiseAmount}
      />
      <div class="preset-buttons">
        <button onclick={() => setBetPreset('half')}>1/2 Pot</button>
        <button onclick={() => setBetPreset('pot')}>Pot</button>
        <button onclick={() => setBetPreset('allin')}>All-In</button>
      </div>
      <button class="confirm-raise" onclick={currentBet === 0 ? handleBet : handleRaise}>
        {currentBet === 0 ? `Bet ${formatChips(raiseAmount)}` : `Raise to ${formatChips(raiseAmount)}`}
      </button>
    </div>
  {/if}

  <!-- Action buttons -->
  <div class="actions" class:disabled={!isMyTurn || !gameInProgress || actionPending}>
    {#if actionPending}
      <div class="action-pending">
        <div class="spinner"></div>
        <span>Processing...</span>
      </div>
    {:else if !gameInProgress}
      <div class="no-game-message">
        {#if phaseKey === 'HandComplete'}
          Hand complete - Start a new hand
        {:else}
          Waiting for players...
        {/if}
      </div>
    {:else if !isMyTurn}
      <div class="not-your-turn">Waiting for other player...</div>
    {:else}
      <!-- Secondary action - muted -->
      <button
        class="action-btn secondary"
        onclick={() => onAction('fold')}
      >
        Fold
      </button>
      {#if canCheck}
        <!-- Primary action - filled -->
        <button
          class="action-btn primary"
          onclick={() => onAction('check')}
        >
          Check
        </button>
      {:else}
        <!-- Primary action - filled green -->
        <button
          class="action-btn primary"
          onclick={() => onAction('call')}
        >
          Call {formatChips(callAmount)}
        </button>
      {/if}
      {#if canRaise}
        <button
          class="action-btn raise"
          onclick={() => showRaiseSlider = true}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <path d="M12 19V5M5 12l7-7 7 7"/>
          </svg>
          {currentBet === 0 ? 'Bet' : 'Raise'}
        </button>
      {/if}
      <button
        class="action-btn danger"
        onclick={() => onAction('allin')}
      >
        All In
      </button>
      {#if timeBankRemaining > 0 && !usingTimeBank}
        <button
          class="action-btn ghost"
          onclick={() => onAction('useTimeBank')}
        >
          +{timeBankRemaining}s
        </button>
      {/if}
    {/if}
  </div>

  <!-- Sit Out / Sit In controls - show when player is seated -->
  {#if mySeat !== null}
    <div class="sit-controls">
      {#if isSittingOut}
        <button class="control-btn" onclick={() => onAction('sitIn')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/>
            <polyline points="10,17 15,12 10,7"/>
            <line x1="15" y1="12" x2="3" y2="12"/>
          </svg>
          Sit In
        </button>
      {:else}
        <button class="control-btn" onclick={() => onAction('sitOut')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
            <polyline points="16,17 21,12 16,7"/>
            <line x1="21" y1="12" x2="9" y2="12"/>
          </svg>
          Sit Out
        </button>
      {/if}
      <button class="control-btn destructive" onclick={() => onAction('leave')}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M18 6L6 18M6 6l12 12"/>
        </svg>
        Leave
      </button>
    </div>
  {/if}
  </div>

  <!-- Right side container - Wallet panel and optionally Take a Seat -->
  <div class="feed-container right" class:collapsed={walletCollapsed}>
    <!-- Collapse/Expand toggle button -->
    <button class="panel-toggle" onclick={toggleWalletPanel} title={walletCollapsed ? 'Expand panel' : 'Collapse panel'}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        {#if walletCollapsed}
          <path d="M15 18l-6-6 6-6"/>
        {:else}
          <path d="M9 18l6-6-6-6"/>
        {/if}
      </svg>
    </button>

    {#if !walletCollapsed}
      <!-- Wallet Panel -->
      <div class="wallet-panel">
        <div class="wallet-panel-header">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="4" width="20" height="16" rx="2"/>
            <path d="M2 10h20"/>
            <circle cx="17" cy="14" r="2"/>
          </svg>
          <span>Wallet</span>
        </div>
        <div class="wallet-balance">
          <span class="balance-label">Table Balance</span>
          <span class="balance-value">{formatWithUnit(tableBalance)}</span>
        </div>
        <div class="wallet-actions">
          {#if onShowDeposit}
            <button class="wallet-action-btn deposit" onclick={onShowDeposit}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 19V5M5 12l7-7 7 7"/>
              </svg>
              Deposit
            </button>
          {/if}
          {#if onShowWithdraw}
            <button class="wallet-action-btn withdraw" onclick={onShowWithdraw} disabled={tableBalance <= 0}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14M5 12l7 7 7-7"/>
              </svg>
              Withdraw
            </button>
          {/if}
        </div>
      </div>

      <!-- Take a Seat panel - only when not seated -->
      {#if mySeat === null}
        <div class="take-seat-panel">
          <div class="take-seat-header">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
              <circle cx="12" cy="7" r="4"/>
            </svg>
            <span>Take a Seat</span>
          </div>
          <div class="seat-buttons-grid">
            {#each players as player, i}
              {#if !player && i < seatPositions.length}
                <button class="seat-btn" onclick={() => onAction('join', i)}>
                  Seat {i + 1}
                </button>
              {/if}
            {/each}
          </div>
          <p class="take-seat-hint">Or click an empty seat on the table</p>
        </div>
      {/if}
    {:else}
      <!-- Collapsed state - just show minimal info -->
      <div class="wallet-collapsed-info">
        <span class="collapsed-balance">{formatWithUnit(tableBalance)}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .poker-table-wrapper {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    gap: 20px;
    width: 100%;
    max-width: 1500px;
    margin: 0 auto;
    padding: 0 10px;
  }

  .feed-container {
    flex-shrink: 0;
    position: sticky;
    top: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .feed-container.left {
    order: -1;
  }

  .feed-container.right {
    order: 1;
  }

  .feed-container.right.collapsed {
    width: auto;
  }

  /* Panel toggle button - collapse/expand */
  .panel-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    transition: all 0.2s;
    align-self: flex-start;
    margin-bottom: 4px;
  }

  .panel-toggle:hover {
    background: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.9);
    border-color: rgba(255, 255, 255, 0.2);
  }

  /* Collapsed wallet info */
  .wallet-collapsed-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px;
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.95));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
  }

  .collapsed-balance {
    font-size: 12px;
    font-weight: 700;
    color: #00d4aa;
    font-family: 'JetBrains Mono', monospace;
  }

  /* Wallet panel - right side - matches ActionFeed width (220px) */
  .wallet-panel {
    width: 220px;
    padding: 14px;
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.95));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    box-shadow:
      0 10px 40px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .wallet-panel-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    color: rgba(255, 255, 255, 0.9);
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .wallet-panel-header svg {
    color: #00d4aa;
  }

  .wallet-balance {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 10px;
    margin-bottom: 12px;
  }

  .wallet-balance .balance-label {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.5);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .wallet-balance .balance-value {
    font-size: 18px;
    font-weight: 700;
    color: #00d4aa;
    font-family: 'JetBrains Mono', monospace;
  }

  .wallet-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .wallet-action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    border: none;
  }

  .wallet-action-btn.deposit {
    background: linear-gradient(135deg, #00d4aa 0%, #00a88a 100%);
    color: white;
  }

  .wallet-action-btn.deposit:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(0, 212, 170, 0.4);
  }

  .wallet-action-btn.withdraw {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: rgba(255, 255, 255, 0.8);
  }

  .wallet-action-btn.withdraw:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .wallet-action-btn.withdraw:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* Take a Seat panel - right side - matches ActionFeed width */
  .take-seat-panel {
    width: 220px;
    padding: 16px;
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.95));
    border: 1px solid rgba(0, 212, 170, 0.3);
    border-radius: 16px;
    box-shadow:
      0 10px 40px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
    animation: take-seat-glow 2s ease-in-out infinite;
  }

  @keyframes take-seat-glow {
    0%, 100% { box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4), 0 0 10px rgba(0, 212, 170, 0.15); }
    50% { box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4), 0 0 20px rgba(0, 212, 170, 0.3); }
  }

  .take-seat-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    color: #00d4aa;
    font-size: 14px;
    font-weight: 700;
  }

  .take-seat-header svg {
    color: #00d4aa;
  }

  .seat-buttons-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
    margin-bottom: 12px;
  }

  .seat-btn {
    padding: 10px 8px;
    background: linear-gradient(135deg, #00d4aa 0%, #00a88a 100%);
    border: none;
    border-radius: 8px;
    color: white;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .seat-btn:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 212, 170, 0.4);
  }

  .seat-btn:active {
    transform: translateY(0);
  }

  .take-seat-hint {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.4);
    text-align: center;
    margin: 0;
  }

  /* Turn indicator - matches ActionFeed styling */
  .turn-indicator {
    width: 220px;
    padding: 12px 14px;
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.95));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    box-shadow:
      0 10px 40px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .turn-indicator.my-turn {
    background: linear-gradient(145deg, rgba(0, 212, 170, 0.15), rgba(0, 180, 150, 0.1));
    border-color: rgba(0, 212, 170, 0.4);
    animation: turn-glow 1.5s ease-in-out infinite;
  }

  .turn-indicator.time-bank {
    background: linear-gradient(145deg, rgba(245, 158, 11, 0.15), rgba(217, 119, 6, 0.1));
    border-color: rgba(245, 158, 11, 0.4);
  }

  @keyframes turn-glow {
    0%, 100% { box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4), 0 0 15px rgba(0, 212, 170, 0.2); }
    50% { box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4), 0 0 25px rgba(0, 212, 170, 0.4); }
  }

  .turn-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .turn-title {
    font-size: 14px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.9);
    letter-spacing: 0.5px;
  }

  .turn-indicator.my-turn .turn-title {
    color: #00d4aa;
  }

  .turn-timer {
    font-size: 14px;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 8px;
    border-radius: 4px;
  }

  .turn-timer.urgent {
    color: #f87171;
    background: rgba(239, 68, 68, 0.2);
    animation: timer-pulse 0.5s ease-in-out infinite;
  }

  @keyframes timer-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }

  .turn-indicator .turn-hint {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
    text-transform: uppercase;
    letter-spacing: 1px;
  }

  .turn-indicator.my-turn .turn-hint {
    color: rgba(0, 212, 170, 0.8);
  }

  .time-bank-badge {
    font-size: 9px;
    font-weight: 700;
    color: #f59e0b;
    background: rgba(245, 158, 11, 0.2);
    padding: 2px 6px;
    border-radius: 3px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    align-self: flex-start;
  }

  /* Hide side panels on smaller screens */
  /* Hide right panel on smaller screens - users can still access wallet from header */
  @media (max-width: 1050px) {
    .feed-container.right {
      display: none;
    }
  }

  @media (max-width: 900px) {
    .poker-table-wrapper {
      flex-direction: column;
      align-items: center;
    }

    .feed-container {
      display: none;
    }
  }

  .poker-table {
    flex: 1 1 auto;
    max-width: 900px;
    min-width: 0;
    position: relative;
    overflow: visible;
  }

  /* Overlay banners - positioned in corner to not block cards */
  .overlay-banners {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 50;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
    max-width: 280px;
  }

  .overlay-banners > * {
    pointer-events: auto;
  }

  /* Top controls container */
  .top-controls {
    position: absolute;
    top: 0;
    right: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* Sound toggle button */
  .sound-toggle {
    padding: 8px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .sound-toggle:hover {
    background: rgba(0, 0, 0, 0.6);
    color: white;
    border-color: rgba(255, 255, 255, 0.2);
  }

  /* Sitting out banner */
  .sitting-out-banner {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 10px 16px;
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.9), rgba(217, 119, 6, 0.85));
    border: 1px solid rgba(245, 158, 11, 0.6);
    border-radius: 10px;
    color: #fff;
    font-weight: 500;
    backdrop-filter: blur(8px);
  }

  .sitting-out-banner svg {
    color: #fff;
  }

  .sit-in-btn {
    padding: 6px 14px;
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    border: none;
    border-radius: 6px;
    color: white;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .sit-in-btn:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(245, 158, 11, 0.3);
  }

  /* Avatar wrapper - contains avatar and position badges */
  .avatar-wrapper {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* Position badges - positioned absolutely relative to avatar-wrapper */
  .position-badges {
    position: absolute;
    display: flex;
    gap: 3px;
    z-index: 15;
  }

  /* Badges for bottom seats (y > 50) - show above the avatar toward table */
  .position-badges.badges-top {
    bottom: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
  }

  /* Badges for top seats (y <= 50) - show below the avatar toward table */
  .position-badges.badges-bottom {
    top: calc(100% + 4px);
    left: 50%;
    transform: translateX(-50%);
  }

  /* Badges for right side seats - show to the left of avatar */
  .position-badges.badges-left {
    right: calc(100% + 4px);
    left: auto;
    top: 50%;
    bottom: auto;
    transform: translateY(-50%);
    flex-direction: column;
  }

  /* Badges for left side seats - show to the right of avatar */
  .position-badges.badges-right {
    left: calc(100% + 4px);
    right: auto;
    top: 50%;
    bottom: auto;
    transform: translateY(-50%);
    flex-direction: column;
  }

  .position-badge {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: bold;
    box-shadow: 0 2px 6px rgba(0,0,0,0.4);
  }

  .position-badge.dealer {
    background: linear-gradient(135deg, #fff 0%, #e0e0e0 100%);
    color: #333;
    border: 2px solid #888;
  }

  .position-badge.sb {
    background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
    color: white;
    border: 2px solid #1d4ed8;
  }

  .position-badge.bb {
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    color: white;
    border: 2px solid #b45309;
  }

  /* Pot odds */
  .pot-odds-display {
    padding: 12px 16px;
    background: rgba(99, 102, 241, 0.1);
    border: 1px solid rgba(99, 102, 241, 0.3);
    border-radius: 10px;
    margin: 12px 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .pot-odds-main {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .pot-odds-label {
    color: #888;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .pot-odds-value {
    color: #6366f1;
    font-weight: bold;
    font-size: 20px;
  }

  .pot-odds-explanation {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
  }

  .pot-odds-explanation strong {
    color: #fbbf24;
  }

  .equity-hint {
    font-size: 11px;
    color: #6366f1;
    font-style: italic;
  }

  /* ========================================
     PREMIUM RAISE SLIDER PANEL
     ======================================== */
  .raise-slider-panel {
    background: linear-gradient(145deg, rgba(25, 25, 40, 0.98), rgba(15, 15, 25, 0.98));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 20px;
    padding: 24px;
    margin: 16px 0;
    box-shadow:
      0 20px 60px rgba(0, 0, 0, 0.5),
      0 0 0 1px rgba(255, 255, 255, 0.05),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(20px);
    animation: slideUp 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(20px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .slider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1.5px;
  }

  .close-slider {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.5);
    width: 32px;
    height: 32px;
    border-radius: 10px;
    font-size: 18px;
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-slider:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.3);
    color: #ef4444;
    transform: rotate(90deg);
  }

  .slider-amount {
    text-align: center;
    font-size: 48px;
    font-weight: 800;
    background: linear-gradient(135deg, #fbbf24 0%, #f59e0b 50%, #d97706 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    margin-bottom: 20px;
    text-shadow: 0 0 40px rgba(245, 158, 11, 0.3);
    letter-spacing: -1px;
  }

  .raise-slider {
    width: 100%;
    height: 12px;
    -webkit-appearance: none;
    appearance: none;
    background: linear-gradient(90deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.12));
    border-radius: 20px;
    outline: none;
    cursor: pointer;
    position: relative;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .raise-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 32px;
    height: 32px;
    background: linear-gradient(145deg, #fbbf24, #f59e0b);
    border-radius: 50%;
    cursor: grab;
    box-shadow:
      0 4px 15px rgba(245, 158, 11, 0.5),
      0 0 0 4px rgba(245, 158, 11, 0.15),
      inset 0 2px 0 rgba(255, 255, 255, 0.3);
    transition: all 0.15s;
    border: 3px solid rgba(255, 255, 255, 0.9);
  }

  .raise-slider::-webkit-slider-thumb:hover {
    transform: scale(1.15);
    box-shadow:
      0 6px 25px rgba(245, 158, 11, 0.6),
      0 0 0 6px rgba(245, 158, 11, 0.2),
      inset 0 2px 0 rgba(255, 255, 255, 0.3);
  }

  .raise-slider::-webkit-slider-thumb:active {
    cursor: grabbing;
    transform: scale(1.1);
  }

  .raise-slider::-moz-range-thumb {
    width: 32px;
    height: 32px;
    background: linear-gradient(145deg, #fbbf24, #f59e0b);
    border-radius: 50%;
    cursor: grab;
    box-shadow: 0 4px 15px rgba(245, 158, 11, 0.5);
    border: 3px solid rgba(255, 255, 255, 0.9);
  }

  .preset-buttons {
    display: flex;
    gap: 10px;
    margin: 20px 0;
  }

  .preset-buttons button {
    flex: 1;
    padding: 14px 12px;
    background: linear-gradient(145deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.02));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.3px;
    position: relative;
    overflow: hidden;
  }

  .preset-buttons button::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.2), rgba(217, 119, 6, 0.1));
    opacity: 0;
    transition: opacity 0.2s;
  }

  .preset-buttons button:hover {
    background: linear-gradient(145deg, rgba(245, 158, 11, 0.15), rgba(245, 158, 11, 0.08));
    border-color: rgba(245, 158, 11, 0.4);
    color: #fbbf24;
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(245, 158, 11, 0.15);
  }

  .preset-buttons button:hover::before {
    opacity: 1;
  }

  .preset-buttons button:active {
    transform: translateY(0);
  }

  .confirm-raise {
    width: 100%;
    padding: 18px 24px;
    background: linear-gradient(135deg, #fbbf24 0%, #f59e0b 50%, #d97706 100%);
    border: none;
    border-radius: 14px;
    color: #1a1a2e;
    font-weight: 800;
    font-size: 16px;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    text-transform: uppercase;
    letter-spacing: 1px;
    position: relative;
    overflow: hidden;
    box-shadow:
      0 8px 25px rgba(245, 158, 11, 0.35),
      inset 0 1px 0 rgba(255, 255, 255, 0.25);
  }

  .confirm-raise::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 50%;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.2), transparent);
    border-radius: 14px 14px 0 0;
  }

  .confirm-raise:hover {
    transform: translateY(-3px);
    box-shadow:
      0 15px 40px rgba(245, 158, 11, 0.45),
      inset 0 1px 0 rgba(255, 255, 255, 0.3);
  }

  .confirm-raise:active {
    transform: translateY(-1px);
  }

  .felt {
    position: relative;
    width: 100%;
    /* Use aspect-ratio for cleaner responsive behavior */
    aspect-ratio: 1.8 / 1;
    min-height: 280px;
    /* Classic poker table green - vibrant and inviting */
    background: radial-gradient(ellipse at center, #1a472a 0%, #0d2818 100%);
    border-radius: 200px / 120px;
    border: 12px solid #4a2810;
    box-shadow:
      inset 0 0 50px rgba(0,0,0,0.5),
      0 10px 30px rgba(0,0,0,0.5);
    /* Margin for seats outside - top needs more for avatars+badges, bottom for hero seat */
    margin: clamp(100px, 15vh, 140px) clamp(40px, 5vw, 80px) clamp(80px, 10vh, 110px);
    overflow: visible;
  }

  .community-area {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .community-cards {
    display: flex;
    gap: 8px;
  }

  .pot-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  /* Winner display on the table - replaces pot during hand complete */
  .winner-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 20px;
    background: linear-gradient(135deg, rgba(243, 156, 18, 0.4), rgba(230, 126, 34, 0.3));
    border-radius: 12px;
    border: 2px solid rgba(243, 156, 18, 0.6);
    animation: winner-pop 0.4s ease-out;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .winner-display.you-won {
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.4), rgba(39, 174, 96, 0.3));
    border-color: rgba(46, 204, 113, 0.6);
  }

  @keyframes winner-pop {
    0% { transform: scale(0.8); opacity: 0; }
    50% { transform: scale(1.05); }
    100% { transform: scale(1); opacity: 1; }
  }

  .winner-text {
    font-size: 18px;
    font-weight: 700;
    color: #ffd700;
  }

  .winner-display.you-won .winner-text {
    color: #2ecc71;
  }

  .winner-hand-rank {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.9);
    font-weight: 500;
  }

  .split-info {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.6);
  }

  .main-pot {
    display: flex;
    flex-direction: column;
    align-items: center;
    color: #888;
    text-shadow: 0 2px 4px rgba(0,0,0,0.5);
    padding: 8px 16px;
    background: rgba(0, 0, 0, 0.4);
    border-radius: 12px;
    border: 1px solid rgba(255, 215, 0, 0.2);
    min-width: 100px;
  }

  .main-pot.has-chips {
    color: #ffd700;
    border-color: rgba(255, 215, 0, 0.5);
    animation: pot-glow 2s ease-in-out infinite;
  }

  @keyframes pot-glow {
    0%, 100% { box-shadow: 0 0 10px rgba(255, 215, 0, 0.2); }
    50% { box-shadow: 0 0 20px rgba(255, 215, 0, 0.4); }
  }

  .pot-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 2px;
    font-weight: 600;
  }

  .pot-amount {
    font-size: 26px;
    font-weight: bold;
  }

  .pot-breakdown {
    font-size: 10px;
    color: rgba(255, 215, 0, 0.6);
    margin-top: 2px;
  }

  .side-pots {
    display: flex;
    gap: 12px;
  }

  .side-pot {
    display: flex;
    flex-direction: column;
    align-items: center;
    color: #00d4aa;
    font-size: 12px;
  }

  .side-pot-label {
    font-size: 10px;
    opacity: 0.8;
  }

  .side-pot-amount {
    font-weight: bold;
  }

  .phase-indicator {
    background: rgba(0,0,0,0.6);
    color: #fff;
    padding: 4px 12px;
    border-radius: 12px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
  }

  .seat {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    z-index: 10;
    /* Prevent text selection during fast clicks */
    user-select: none;
    /* Smooth transitions for any position changes */
    transition: opacity 0.3s ease, filter 0.3s ease;
  }

  .seat.action-on {
    z-index: 20;
    animation: none; /* Remove jarring pulse, use avatar pulse instead */
  }

  .seat.folded {
    opacity: 0.5;
    filter: grayscale(0.3);
  }

  .player-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    /* Add subtle backdrop for readability */
    padding: 4px;
    border-radius: 12px;
  }

  .action-indicator {
    background: linear-gradient(135deg, #ffd700, #f59e0b);
    color: #000;
    padding: 3px 10px;
    border-radius: 6px;
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    animation: blink 0.8s infinite;
    box-shadow: 0 2px 8px rgba(255, 215, 0, 0.4);
  }

  /* ========================================
     POKERSTARS-STYLE AVATAR CONTAINER
     Clean, minimal design with clear hierarchy
     ======================================== */
  .avatar-container {
    position: relative;
    width: 68px;
    height: 68px;
    border-radius: 50%;
    background: linear-gradient(145deg, #2a2a3e, #1a1a28);
    border: 3px solid rgba(255, 255, 255, 0.15);
    box-shadow:
      0 4px 16px rgba(0, 0, 0, 0.5),
      0 2px 4px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
    flex-shrink: 0;
  }

  .avatar-container.is-me {
    border-color: #00d4aa;
    border-width: 2px;
    box-shadow:
      0 0 0 3px rgba(0, 212, 170, 0.2),
      0 4px 20px rgba(0, 212, 170, 0.25);
  }

  .avatar-container.action-on {
    border-color: #ffd700;
    border-width: 2px;
    animation: avatar-pulse 2s ease-in-out infinite;
  }

  @keyframes avatar-pulse {
    0%, 100% {
      box-shadow:
        0 0 0 3px rgba(255, 215, 0, 0.25),
        0 4px 24px rgba(255, 215, 0, 0.3);
    }
    50% {
      box-shadow:
        0 0 0 6px rgba(255, 215, 0, 0.15),
        0 4px 32px rgba(255, 215, 0, 0.45);
    }
  }

  .player-avatar {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
    /* Subtle image enhancement */
    filter: saturate(0.9) contrast(1.05);
  }

  .avatar-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-radius: 50%;
  }

  .avatar-overlay.folded {
    background: rgba(0, 0, 0, 0.75);
    color: rgba(255, 255, 255, 0.5);
  }

  .avatar-overlay.allin {
    background: rgba(239, 68, 68, 0.85);
    color: #fff;
    animation: allin-pulse 0.8s ease-in-out infinite;
  }

  @keyframes allin-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
  }

  .player-cards {
    display: flex;
    gap: 3px;
    margin: 4px 0;
  }

  /* ========================================
     PLAYER NAMEPLATE - Avatar + Name together
     Clean, compact design
     ======================================== */
  .player-nameplate {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    background: linear-gradient(145deg, rgba(20, 20, 32, 0.95), rgba(12, 12, 20, 0.98));
    padding: 6px 10px;
    border-radius: 10px;
    border: 2px solid rgba(255, 255, 255, 0.08);
    min-width: 90px;
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.4),
      0 1px 2px rgba(0, 0, 0, 0.2);
    backdrop-filter: blur(8px);
    transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .player-nameplate.highlight-me {
    background: linear-gradient(145deg, rgba(0, 180, 140, 0.12), rgba(0, 140, 110, 0.08));
    border-color: rgba(0, 212, 170, 0.4);
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.4),
      0 0 12px rgba(0, 212, 170, 0.2);
  }

  .player-nameplate.action-on {
    border-color: #ffd700;
    animation: nameplate-pulse 2s ease-in-out infinite;
  }

  @keyframes nameplate-pulse {
    0%, 100% {
      box-shadow:
        0 0 0 3px rgba(255, 215, 0, 0.25),
        0 4px 16px rgba(255, 215, 0, 0.3);
    }
    50% {
      box-shadow:
        0 0 0 5px rgba(255, 215, 0, 0.15),
        0 4px 24px rgba(255, 215, 0, 0.4);
    }
  }

  /* Avatar inside nameplate - smaller size */
  .player-nameplate .avatar-container {
    width: 40px;
    height: 40px;
    border-width: 2px;
  }

  .player-nameplate .player-avatar {
    width: 100%;
    height: 100%;
  }

  .player-info {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    min-width: 0;
  }

  .player-name {
    color: rgba(255, 255, 255, 0.85);
    font-size: 10px;
    font-weight: 600;
    max-width: 65px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    letter-spacing: 0.2px;
  }

  .player-name.is-me {
    color: #00d4aa;
  }

  .bet-amount {
    font-size: 9px;
    color: #00d4aa;
    font-weight: 600;
    background: rgba(0, 212, 170, 0.1);
    padding: 1px 6px;
    border-radius: 4px;
    margin-top: 2px;
  }

  .chips {
    color: #fbbf24;
    font-weight: 700;
    font-size: 12px;
    font-family: 'JetBrains Mono', 'SF Mono', monospace;
    letter-spacing: -0.3px;
  }

  .bet {
    color: #4ecdc4;
    font-size: 10px;
  }

  .all-in-label {
    color: #e74c3c;
    font-weight: bold;
    font-size: 10px;
    animation: blink 0.5s infinite;
  }

  .folded-label {
    color: #888;
    font-size: 10px;
  }

  @keyframes blink {
    50% { opacity: 0.5; }
  }

  .join-seat {
    background: rgba(255,255,255,0.1);
    border: 2px dashed rgba(255,255,255,0.3);
    color: rgba(255,255,255,0.5);
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
    font-size: 12px;
  }

  .join-seat:hover {
    background: rgba(0, 212, 170, 0.2);
    border-color: #00d4aa;
    color: #00d4aa;
  }

  /* ========================================
     PREMIUM ACTION BUTTONS PANEL
     ======================================== */
  .actions {
    display: flex;
    justify-content: center;
    gap: 12px;
    margin-top: 20px;
    padding: 20px 28px;
    background: linear-gradient(145deg, rgba(25, 25, 40, 0.98), rgba(15, 15, 25, 0.98));
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow:
      0 20px 60px rgba(0, 0, 0, 0.5),
      0 0 0 1px rgba(255, 255, 255, 0.05),
      inset 0 1px 0 rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(20px);
    flex-wrap: wrap;
    animation: slideUp 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .actions.disabled {
    opacity: 0.4;
    pointer-events: none;
    filter: grayscale(0.3);
  }

  .no-game-message, .not-your-turn, .action-pending {
    color: rgba(255, 255, 255, 0.5);
    font-size: 15px;
    padding: 16px 32px;
    background: linear-gradient(145deg, rgba(255, 255, 255, 0.03), rgba(255, 255, 255, 0.01));
    border-radius: 14px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    display: flex;
    align-items: center;
    gap: 14px;
    font-weight: 500;
    letter-spacing: 0.3px;
  }

  .action-pending {
    color: #00d4aa;
    background: linear-gradient(145deg, rgba(0, 212, 170, 0.08), rgba(0, 212, 170, 0.03));
    border-color: rgba(0, 212, 170, 0.15);
  }

  .spinner {
    width: 22px;
    height: 22px;
    border: 2.5px solid rgba(0, 212, 170, 0.15);
    border-top-color: #00d4aa;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Action Buttons */
  .action-btn {
    padding: 10px 16px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.9);
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .action-btn:active {
    background: rgba(255, 255, 255, 0.08);
    transform: scale(0.98);
  }

  /* Primary - the main action (Call/Check) */
  .action-btn.primary {
    background: #fff;
    color: #000;
    border: 1px solid #fff;
  }

  .action-btn.primary:hover {
    background: rgba(255, 255, 255, 0.9);
    border-color: rgba(255, 255, 255, 0.9);
  }

  .action-btn.primary:active {
    background: rgba(255, 255, 255, 0.85);
  }

  /* Secondary - muted action (Fold) */
  .action-btn.secondary {
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .action-btn.secondary:hover {
    color: rgba(255, 255, 255, 0.7);
    border-color: rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.05);
  }

  /* Raise - prominent gold/amber action */
  .action-btn.raise {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.2) 0%, rgba(217, 119, 6, 0.15) 100%);
    color: #fbbf24;
    border: 2px solid rgba(245, 158, 11, 0.5);
    font-weight: 600;
    min-width: 100px;
    animation: raise-pulse 2s ease-in-out infinite;
  }

  @keyframes raise-pulse {
    0%, 100% { box-shadow: 0 0 8px rgba(245, 158, 11, 0.2); }
    50% { box-shadow: 0 0 16px rgba(245, 158, 11, 0.4); }
  }

  .action-btn.raise:hover {
    background: linear-gradient(135deg, rgba(245, 158, 11, 0.35) 0%, rgba(217, 119, 6, 0.25) 100%);
    border-color: rgba(245, 158, 11, 0.7);
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(245, 158, 11, 0.3);
  }

  .action-btn.raise:active {
    transform: translateY(0);
  }

  .action-btn.raise svg {
    flex-shrink: 0;
  }

  /* Danger - destructive action (All In) */
  .action-btn.danger {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .action-btn.danger:hover {
    background: rgba(239, 68, 68, 0.25);
    border-color: rgba(239, 68, 68, 0.5);
  }

  /* Ghost - minimal style (Time bank) */
  .action-btn.ghost {
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    border: 1px dashed rgba(255, 255, 255, 0.2);
    font-size: 13px;
    padding: 8px 12px;
  }

  .action-btn.ghost:hover {
    color: rgba(255, 255, 255, 0.8);
    border-color: rgba(255, 255, 255, 0.3);
    background: rgba(255, 255, 255, 0.05);
  }

  /* Sit Controls */
  .sit-controls {
    display: flex;
    justify-content: center;
    gap: 8px;
    margin-top: 16px;
  }

  .control-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    background: transparent;
    color: rgba(255, 255, 255, 0.5);
    border: none;
  }

  .control-btn:hover {
    color: rgba(255, 255, 255, 0.8);
    background: rgba(255, 255, 255, 0.05);
  }

  .control-btn.destructive {
    color: rgba(239, 68, 68, 0.7);
  }

  .control-btn.destructive:hover {
    color: #f87171;
    background: rgba(239, 68, 68, 0.1);
  }

  .control-btn svg {
    opacity: 0.7;
  }

  /* ========================================
     RESPONSIVE DESIGN
     Fluid scaling with clean breakpoints
     ======================================== */

  /* Large tablets and small desktops */
  @media (max-width: 1100px) {
    .felt {
      margin: clamp(70px, 10vh, 100px) clamp(30px, 4vw, 60px) clamp(70px, 8vh, 90px);
    }

    .avatar-container {
      width: 60px;
      height: 60px;
    }

    .player-nameplate {
      min-width: 70px;
      padding: 5px 10px;
    }
  }

  /* Tablets */
  @media (max-width: 768px) {
    .poker-table {
      max-width: 100%;
      padding: 0;
    }

    .felt {
      aspect-ratio: 1.6 / 1;
      border-width: 8px;
      margin: clamp(60px, 8vh, 80px) clamp(25px, 3vw, 45px) clamp(60px, 7vh, 75px);
      border-radius: 45% / 32%;
    }

    .seat {
      transform: translate(-50%, -50%) scale(0.9);
    }

    .avatar-container {
      width: 52px;
      height: 52px;
    }

    .player-nameplate {
      min-width: 65px;
      padding: 4px 8px;
    }

    .player-name {
      font-size: 10px;
    }

    .chips {
      font-size: 11px;
    }

    .community-cards {
      gap: 4px;
    }

    .pot-amount {
      font-size: 22px;
    }

    .actions {
      padding: 12px 16px;
      gap: 8px;
      flex-wrap: wrap;
    }

    .action-btn {
      padding: 12px 16px;
      font-size: 14px;
      min-height: 44px;
      min-width: 80px;
      flex: 1 1 auto;
    }

    .action-btn.primary {
      flex: 2 1 auto;
    }

    .sit-controls {
      flex-wrap: wrap;
      gap: 8px;
      padding: 8px;
    }

    .control-btn {
      padding: 10px 14px;
      font-size: 13px;
      min-height: 44px;
    }

    .raise-slider-panel {
      padding: 16px;
      margin: 12px 0;
    }

    .slider-amount {
      font-size: 36px;
    }

    .preset-buttons {
      gap: 8px;
    }

    .preset-buttons button {
      padding: 12px 10px;
      font-size: 12px;
    }
  }

  /* Mobile phones */
  @media (max-width: 480px) {
    .felt {
      aspect-ratio: 1.4 / 1;
      margin: 50px 15px 50px;
      border-width: 6px;
      border-radius: 40% / 30%;
    }

    .seat {
      transform: translate(-50%, -50%) scale(0.75);
    }

    .avatar-container {
      width: 44px;
      height: 44px;
    }

    .player-nameplate {
      min-width: 55px;
      padding: 3px 6px;
    }

    .player-name {
      font-size: 9px;
      max-width: 60px;
    }

    .chips {
      font-size: 10px;
    }

    .actions {
      padding: 10px 12px;
      gap: 6px;
    }

    .action-btn {
      padding: 10px 12px;
      font-size: 13px;
      min-width: 70px;
    }

    .pot-amount {
      font-size: 18px;
    }

    .slider-amount {
      font-size: 28px;
    }
  }

  /* Landscape phones - optimize for width */
  @media (orientation: landscape) and (max-height: 500px) {
    .felt {
      aspect-ratio: 2.2 / 1;
      margin: 45px 40px 25px;
    }

    .seat {
      transform: translate(-50%, -50%) scale(0.7);
    }

    .avatar-container {
      width: 40px;
      height: 40px;
    }
  }
</style>
