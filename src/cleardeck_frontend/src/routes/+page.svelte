<script>
  import "../index.scss";
  import { lobby, createTableActorProxy } from "$lib/canisters";
  import Lobby from "$lib/components/Lobby.svelte";
  import PokerTable from "$lib/components/PokerTable.svelte";
  import ShuffleProof from "$lib/components/ShuffleProof.svelte";
  import WalletButton from "$lib/components/WalletButton.svelte";
  import HandHistory from "$lib/components/HandHistory.svelte";
  import HowItWorks from "$lib/components/HowItWorks.svelte";
  import DepositModal from "$lib/components/DepositModal.svelte";
  import WithdrawModal from "$lib/components/WithdrawModal.svelte";
  import { playSound, setSoundEnabled, isSoundEnabled } from "$lib/sounds.js";
  import logger from "$lib/logger.js";
  import { auth, isSignatureError, wallet } from "$lib/auth.js";
  import { HttpAgent } from '@dfinity/agent';
  import { Principal } from '@dfinity/principal';

  let view = $state('lobby'); // 'lobby' | 'table'
  let tables = $state([]);
  let tableState = $state(null);
  let myCards = $state(null);
  let shuffleProof = $state(null);
  let loading = $state(false);
  let loadingTableState = false; // Non-reactive flag to prevent concurrent loadTableState calls
  let loadTableStateRequestId = 0; // Counter to discard stale responses
  let error = $state(null);
  let success = $state(null);
  let showProofPanel = $state(false);
  let showHandHistory = $state(false);
  let showHowItWorks = $state(false);
  let showVerify = $state(false);

  // Current avatar style from localStorage - passed to PokerTable
  let currentAvatarStyle = $state(typeof localStorage !== 'undefined' ? (localStorage.getItem('poker_avatar_style') || 'bottts') : 'bottts');

  // Current custom name from localStorage - passed to PokerTable
  let currentCustomName = $state(typeof localStorage !== 'undefined' ? localStorage.getItem('poker_custom_name') : null);

  // JSON stringify that handles BigInt
  function safeStringify(obj) {
    return JSON.stringify(obj, (key, value) =>
      typeof value === 'bigint' ? value.toString() : value
    );
  }

  // Compare table states - returns true if game-relevant data changed
  // Excludes time_remaining_secs since timer ticks client-side
  function gameStateChanged(oldState, newState) {
    if (!oldState && !newState) return false;
    if (!oldState || !newState) return true;
    // Compare key fields that affect the game (not the timer)
    if (String(oldState.hand_number) !== String(newState.hand_number)) return true;
    if (String(oldState.pot) !== String(newState.pot)) return true;
    if (String(oldState.current_bet) !== String(newState.current_bet)) return true;
    if (oldState.is_my_turn !== newState.is_my_turn) return true;
    if (oldState.action_on !== newState.action_on) return true;
    if (safeStringify(oldState.phase) !== safeStringify(newState.phase)) return true;
    if (safeStringify(oldState.community_cards) !== safeStringify(newState.community_cards)) return true;
    if (safeStringify(oldState.players) !== safeStringify(newState.players)) return true;
    if (safeStringify(oldState.last_hand_winners) !== safeStringify(newState.last_hand_winners)) return true;
    return false;
  }

  // Format e8s amount as ICP display
  function formatICP(e8s) {
    const num = typeof e8s === 'bigint' ? Number(e8s) : e8s;
    const icp = num / 100_000_000;
    if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K`;
    if (icp >= 1) return icp.toFixed(2);
    if (icp >= 0.01) return icp.toFixed(2);
    return icp.toFixed(4);
  }

  // Extract currency from candid opt variant
  // Candid opt variants come through as arrays: [] for None, [{ BTC: null }] for Some(BTC)
  function extractCurrency(optCurrency) {
    if (!optCurrency) return null;
    // If it's an array (opt type), unwrap it
    if (Array.isArray(optCurrency)) {
      if (optCurrency.length === 0) return null;
      const inner = optCurrency[0];
      if (inner && typeof inner === 'object') {
        const key = Object.keys(inner)[0];
        return key ? key.toUpperCase() : null;
      }
      return null;
    }
    // If it's already an object (variant), get the key
    if (typeof optCurrency === 'object' && optCurrency !== null) {
      const key = Object.keys(optCurrency)[0];
      return key ? key.toUpperCase() : null;
    }
    // If it's a string directly, return it normalized
    if (typeof optCurrency === 'string') {
      return optCurrency.toUpperCase();
    }
    return null;
  }

  // Get currency from table info, checking both table-level and config-level
  function getTableCurrency(tableInfo) {
    const tableCurrency = extractCurrency(tableInfo?.currency);
    if (tableCurrency) return tableCurrency;
    const configCurrency = extractCurrency(tableInfo?.config?.currency);
    if (configCurrency) return configCurrency;
    return 'ICP'; // Default
  }

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
  let showDepositModal = $state(false);
  let showWithdrawModal = $state(false);

  // Current table info - stores the canister ID of the table we joined
  let currentTableInfo = $state(null);
  // Note: tableActor is NOT $state because Svelte 5's reactive proxy breaks JS Proxy objects
  let tableActor = null;

  // Auto-clear success messages
  $effect(() => {
    if (success) {
      const timer = setTimeout(() => { success = null; }, 3000);
      return () => clearTimeout(timer);
    }
  });

  // Polling interval for table state
  let pollInterval = null;

  async function loadTables() {
    loading = true;
    try {
      let lobbyTables = await lobby.get_tables();

      // For tables that have a canister_id, try to fetch their player counts
      for (let i = 0; i < lobbyTables.length; i++) {
        const t = lobbyTables[i];
        if (t.canister_id && t.canister_id.length > 0) {
          try {
            // Convert Principal to string if needed
            const cid = t.canister_id[0].toText ? t.canister_id[0].toText() : t.canister_id[0].toString();
            const tActor = createTableActorProxy(cid);
            const [playerCount, maxPlayers] = await Promise.all([
              tActor.get_player_count(),
              tActor.get_max_players()
            ]);
            lobbyTables[i] = {
              ...t,
              player_count: playerCount,
              config: { ...t.config, max_players: maxPlayers }
            };
          } catch (e) {
            logger.debug(`Could not fetch player count for table ${t.id}`);
          }
        }
      }

      // Sort tables: ICP first (by ID), then BTC (by ID)
      lobbyTables.sort((a, b) => {
        const currencyA = getTableCurrency(a);
        const currencyB = getTableCurrency(b);
        // ICP comes before BTC
        if (currencyA === 'ICP' && currencyB !== 'ICP') return -1;
        if (currencyA !== 'ICP' && currencyB === 'ICP') return 1;
        // Within same currency, sort by ID
        return Number(a.id) - Number(b.id);
      });
      tables = lobbyTables;
    } catch (e) {
      logger.error('Failed to load tables:', e);
      // Check if this is a signature verification error (expired II delegation)
      if (isSignatureError(e)) {
        error = 'Session expired. Please log in again.';
        await auth.logout();
      } else {
        error = e.message;
      }
    }
    loading = false;
  }

  async function joinTable(tableInfo) {
    // Check if the table has a canister assigned
    if (!tableInfo.canister_id || tableInfo.canister_id.length === 0) {
      error = "This table doesn't have an assigned canister yet";
      return;
    }

    // Store the table info and create an actor for this specific table canister
    currentTableInfo = tableInfo;
    // Convert Principal to string if needed
    const canisterId = tableInfo.canister_id[0].toText ? tableInfo.canister_id[0].toText() : tableInfo.canister_id[0].toString();
    tableActor = createTableActorProxy(canisterId);

    view = 'table';
    startPolling();
  }

  async function loadTableState() {
    if (!tableActor) return;

    // Prevent concurrent loadTableState calls - skip if already loading
    if (loadingTableState) return;
    loadingTableState = true;

    // Track this request to discard stale responses
    const requestId = ++loadTableStateRequestId;

    try {
      // Check for timeouts and auto-deal first
      const timeoutResult = await tableActor.check_timeouts();

      // Discard stale response if a newer request was started
      if (requestId !== loadTableStateRequestId) return;

      // Handle auto-deal if ready
      if (timeoutResult && 'AutoDealReady' in timeoutResult) {
        // Only try to start if we're not already in a hand (client-side guard)
        const currentPhase = tableState?.phase ? Object.keys(tableState.phase)[0] : null;
        const canStartHand = !currentPhase || currentPhase === 'WaitingForPlayers' || currentPhase === 'HandComplete';

        if (canStartHand) {
          try {
            playSound('deal');
            const result = await tableActor.start_new_hand();
            if ('Ok' in result) {
              shuffleProof = result.Ok;
            } else if ('Err' in result) {
              // Silently ignore expected race condition errors
              const errMsg = result.Err.toLowerCase();
              if (!errMsg.includes('active players') && !errMsg.includes('already in progress') && !errMsg.includes('in progress')) {
                logger.error('Auto-deal failed:', result.Err);
              }
            }
          } catch (e) {
            // Silently ignore expected race condition errors
            const errMsg = (e.message || e.toString() || '').toLowerCase();
            if (!errMsg.includes('already in progress') && !errMsg.includes('in progress')) {
              logger.error('Auto-deal failed:', e);
            }
          }
        }
      }

      // Double-check tableActor is still valid (could be cleared if user left table)
      if (!tableActor) return;

      // Use get_table_view which properly hides opponent cards
      const [viewResult, proofResult] = await Promise.all([
        tableActor.get_table_view(),
        tableActor.get_shuffle_proof()
      ]);

      // Discard stale response if a newer request was started
      if (requestId !== loadTableStateRequestId) return;

      // Handle optional return (candid returns arrays for opt types)
      let currentTableView = null;
      if (viewResult && viewResult.length > 0) {
        currentTableView = viewResult[0];
        // Only update state if game data changed (timer ticks client-side)
        if (gameStateChanged(tableState, currentTableView)) {
          tableState = currentTableView;
        } else {
          // Still update time_remaining_secs for the client-side timer sync
          // This is a shallow merge - only update the timer field
          if (tableState && currentTableView.time_remaining_secs) {
            tableState = { ...tableState, time_remaining_secs: currentTableView.time_remaining_secs };
          }
        }

        // Extract my cards from my player view
        if (currentTableView.my_seat && currentTableView.my_seat.length > 0) {
          const mySeatNum = currentTableView.my_seat[0];
          // Bounds check before accessing players array
          if (mySeatNum < currentTableView.players.length) {
            const myPlayerOpt = currentTableView.players[mySeatNum];
            if (myPlayerOpt && myPlayerOpt.length > 0) {
            const myPlayer = myPlayerOpt[0];
            if (myPlayer.hole_cards && myPlayer.hole_cards.length > 0) {
              const newCards = myPlayer.hole_cards[0];
              // Play sound when cards are dealt
              if (!myCards && newCards) {
                playSound('deal');
              }
              myCards = newCards;
            } else {
              myCards = null;
            }
            } else {
              myCards = null;
            }
          } else {
            myCards = null;
          }
        } else {
          myCards = null;
        }
      }

      if (proofResult && proofResult.length > 0) {
        const newProof = proofResult[0];
        // Check if hand completed and we won
        if (currentTableView?.last_hand_winners && currentTableView.last_hand_winners.length > 0) {
          const mySeatNum = currentTableView.my_seat?.[0];
          if (mySeatNum !== undefined) {
            const won = currentTableView.last_hand_winners.some(w => w.seat === mySeatNum);
            if (won && shuffleProof !== newProof) {
              playSound('win');
            }
          }
        }
        shuffleProof = newProof;
      }
    } catch (e) {
      logger.error('Failed to load table state:', e);
      // Check if this is a signature verification error (expired II delegation)
      if (isSignatureError(e)) {
        error = 'Session expired. Please log in again.';
        stopPolling();
        await auth.logout();
      }
    } finally {
      // Always reset loading flag to allow next poll
      loadingTableState = false;
    }
  }

  // Fast polling - 500ms for responsive gameplay
  const POLL_INTERVAL = 500;
  const HEARTBEAT_INTERVAL = 10000; // Send heartbeat every 10 seconds
  const BALANCE_REFRESH_INTERVAL = 5000; // Refresh balance every 5 seconds
  let actionPending = $state(false);
  let heartbeatInterval = null;
  let balanceRefreshInterval = null;

  async function sendHeartbeat() {
    if (!tableActor) return;
    try {
      await tableActor.heartbeat();
    } catch (e) {
      // Silently fail - heartbeat is best effort
      logger.debug('Heartbeat failed:', e);
    }
  }

  // Refresh both table balance and wallet balance
  async function refreshAllBalances() {
    await Promise.all([
      loadBalance(),
      wallet.refreshBalance()
    ]);
  }

  // Sync display name from localStorage to the table canister
  let lastSyncedName = null;
  async function syncDisplayName() {
    if (!tableActor) return;
    const customName = typeof localStorage !== 'undefined' ? localStorage.getItem('poker_custom_name') : null;
    // Only sync if name changed since last sync
    if (customName === lastSyncedName) return;
    try {
      await tableActor.set_display_name(customName ? [customName] : []);
      lastSyncedName = customName;
      logger.debug('Display name synced:', customName || '(cleared)');
    } catch (e) {
      logger.debug('Failed to sync display name:', e);
    }
  }

  // Handle profile changes from WalletButton (name or avatar)
  function handleProfileChange(event) {
    if (event.type === 'name') {
      // Update local state and sync to backend
      currentCustomName = event.name;
      syncDisplayName();
    } else if (event.type === 'avatar') {
      // Update avatar style for PokerTable
      currentAvatarStyle = event.style;
    }
  }

  function startPolling() {
    loadTableState();
    refreshAllBalances(); // Initial balance load
    syncDisplayName(); // Sync display name when joining table
    pollInterval = setInterval(loadTableState, POLL_INTERVAL);
    // Start heartbeat to show we're connected
    sendHeartbeat();
    heartbeatInterval = setInterval(sendHeartbeat, HEARTBEAT_INTERVAL);
    // Refresh balances periodically
    balanceRefreshInterval = setInterval(refreshAllBalances, BALANCE_REFRESH_INTERVAL);
  }

  function stopPolling() {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
    if (heartbeatInterval) {
      clearInterval(heartbeatInterval);
      heartbeatInterval = null;
    }
    if (balanceRefreshInterval) {
      clearInterval(balanceRefreshInterval);
      balanceRefreshInterval = null;
    }
  }

  // Get current balance
  let myBalance = $state(0);
  async function loadBalance() {
    if (!tableActor) return;
    try {
      myBalance = Number(await tableActor.get_balance());
    } catch (e) {
      logger.error('Failed to load balance:', e);
    }
  }

  // Load balance when entering table view
  $effect(() => {
    if (view === 'table' && tableActor) {
      loadBalance();
    }
  });

  async function handleTableAction(action, data) {
    if (actionPending || !tableActor) return;
    actionPending = true;

    try {
      let result;
      switch (action) {
        case 'join':
          result = await tableActor.join_table(data);
          if ('Err' in result) {
            error = result.Err;
          } else {
            success = `Joined seat ${data + 1}!`;
            await refreshAllBalances();
          }
          break;

        case 'fold':
          if (tableState) tableState.is_my_turn = false;
          playSound('fold');
          result = await tableActor.player_action({ Fold: null });
          if ('Err' in result) {
            error = result.Err;
            playSound('error');
          }
          break;

        case 'check':
          if (tableState) tableState.is_my_turn = false;
          playSound('check');
          result = await tableActor.player_action({ Check: null });
          if ('Err' in result) {
            error = result.Err;
            playSound('error');
          }
          break;

        case 'call':
          if (tableState) tableState.is_my_turn = false;
          playSound('call');
          result = await tableActor.player_action({ Call: null });
          if ('Err' in result) {
            error = result.Err;
            playSound('error');
          }
          break;

        case 'raise':
          if (data) {
            if (tableState) tableState.is_my_turn = false;
            playSound('raise');
            result = await tableActor.player_action({ Raise: BigInt(data) });
            if ('Err' in result) {
              error = result.Err;
              playSound('error');
            }
          }
          break;

        case 'allin':
          if (tableState) tableState.is_my_turn = false;
          playSound('allin');
          result = await tableActor.player_action({ AllIn: null });
          if ('Err' in result) {
            error = result.Err;
            playSound('error');
          }
          break;

        case 'start':
          playSound('deal');
          result = await tableActor.start_new_hand();
          if ('Ok' in result) {
            shuffleProof = result.Ok;
            success = 'New hand started!';
          } else if ('Err' in result) {
            // Don't show "need 2 players" as error - it's informational
            if (!result.Err.includes('2 active players')) {
              error = result.Err;
              playSound('error');
            }
            // The UI already shows "Need 2+ players to start" hint
          }
          break;

        case 'bet':
          if (data) {
            if (tableState) tableState.is_my_turn = false;
            playSound('bet');
            result = await tableActor.player_action({ Bet: BigInt(data) });
            if ('Err' in result) {
              error = result.Err;
              playSound('error');
            }
          }
          break;

        case 'useTimeBank':
          result = await tableActor.use_time_bank();
          if ('Err' in result) {
            error = result.Err;
          } else {
            success = 'Using time bank';
          }
          break;

        case 'sitOut':
          result = await tableActor.sit_out();
          if ('Err' in result) {
            error = result.Err;
          } else {
            success = 'Sitting out next hand';
          }
          break;

        case 'sitIn':
          result = await tableActor.sit_in();
          if ('Err' in result) {
            error = result.Err;
          } else {
            success = 'Back in the game';
          }
          break;

        case 'leave':
          result = await tableActor.leave_table();
          if ('Err' in result) {
            error = result.Err;
          } else {
            const returnedSmallest = Number(result.Ok);
            const tableCurrency = getTableCurrency(currentTableInfo);
            let returnedDisplay;
            if (tableCurrency === 'BTC') {
              if (returnedSmallest >= 1000) {
                returnedDisplay = `${(returnedSmallest / 1000).toFixed(1)}K sats`;
              } else {
                returnedDisplay = `${returnedSmallest} sats`;
              }
            } else {
              returnedDisplay = `${(returnedSmallest / 100_000_000).toFixed(4)} ICP`;
            }
            success = `Left table. ${returnedDisplay} returned to balance.`;
            await refreshAllBalances();
          }
          break;
      }

      await loadTableState();
    } catch (e) {
      logger.error(`Action ${action} failed:`, e);
      // Check if this is a signature verification error (expired II delegation)
      if (isSignatureError(e)) {
        error = 'Session expired. Please log in again.';
        stopPolling();
        await auth.logout();
      } else {
        error = e.message || 'Action failed';
      }
    } finally {
      actionPending = false;
    }
  }

  function backToLobby() {
    stopPolling();
    view = 'lobby';
    tableState = null;
    myCards = null;
    shuffleProof = null;
    currentTableInfo = null;
    tableActor = null;
    loadTables(); // Refresh tables list
  }

  // Load tables on mount and ensure cleanup on unmount
  $effect(() => {
    loadTables();
    // Cleanup function ensures all intervals are cleared on component unmount
    return () => {
      stopPolling();
      // Double-check: explicitly clear any lingering intervals
      if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
      }
      if (heartbeatInterval) {
        clearInterval(heartbeatInterval);
        heartbeatInterval = null;
      }
      if (balanceRefreshInterval) {
        clearInterval(balanceRefreshInterval);
        balanceRefreshInterval = null;
      }
    };
  });
</script>

<div class="app">
  <!-- Ambient background effects -->
  <div class="bg-effects">
    <div class="glow glow-1"></div>
    <div class="glow glow-2"></div>
    <div class="glow glow-3"></div>
  </div>

  <!-- Disclaimer Banner -->
  <div class="alpha-warning-banner">
    <div class="banner-content">
      <p class="banner-warning">
        <span class="warning-icon">⚠️</span>
        <strong>DISCLAIMER:</strong> Unaudited code with known bugs. This is for educational and testing purposes only. Any deposit of ICP or Bitcoin is at your own risk—your funds are NOT safe. Expect to lose everything you deposit. Online gambling is illegal in many jurisdictions. Only use where legally permitted. 18+ only.
      </p>
      <p class="banner-info">
        No middleman, no house. Built to demonstrate the power of the Internet Computer: 100% on-chain—frontend, backend, and game logic all running on smart contracts (canisters). Provably fair, fully transparent, and completely decentralized.
      </p>
      <p class="banner-ai">
        This entire project was built 100% by AI. <span class="warning-icon">⚠️</span>
      </p>
    </div>
  </div>

  <header>
    <div class="header-left">
      {#if view === 'table'}
        <button class="back-btn" onclick={backToLobby}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7"/>
          </svg>
          Lobby
        </button>
        {#if currentTableInfo}
          <span class="current-table-name">{currentTableInfo.name}</span>
        {/if}
      {/if}
    </div>

    <div class="logo">
      <div class="logo-mark">
        <span class="suit suit-1">♠</span>
        <span class="suit suit-2">♦</span>
      </div>
      <div class="logo-text">
        <span class="brand">ClearDeck</span>
        <span class="tagline">Provably Fair Poker</span>
      </div>
    </div>

    <div class="header-right">
      <!-- Sound toggle - always visible -->
      <button class="sound-toggle-btn" onclick={toggleSound} title={soundMuted ? 'Unmute sounds' : 'Mute sounds'}>
        {#if soundMuted}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 5L6 9H2v6h4l5 4V5z"/>
            <line x1="23" y1="9" x2="17" y2="15"/>
            <line x1="17" y1="9" x2="23" y2="15"/>
          </svg>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 5L6 9H2v6h4l5 4V5z"/>
            <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
            <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
          </svg>
        {/if}
      </button>
      {#if view === 'table'}
        <button class="history-btn" onclick={() => showHandHistory = true}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12,6 12,12 16,14"/>
          </svg>
          History
        </button>
        <button class="verify-btn" onclick={() => showProofPanel = !showProofPanel}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            <path d="M9 12l2 2 4-4"/>
          </svg>
          Verify Fair
        </button>
      {/if}
      <WalletButton onProfileChange={handleProfileChange} />
    </div>
  </header>

  {#if error}
    <div class="toast error">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <line x1="15" y1="9" x2="9" y2="15"/>
        <line x1="9" y1="9" x2="15" y2="15"/>
      </svg>
      <span>{error}</span>
      <button onclick={() => error = null}>×</button>
    </div>
  {/if}

  {#if success}
    <div class="toast success">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <path d="M9 12l2 2 4-4"/>
      </svg>
      <span>{success}</span>
    </div>
  {/if}

  <main>
    {#if loading && view === 'lobby'}
      <div class="loading-state">
        <div class="spinner"></div>
        <span>Loading tables...</span>
      </div>
    {/if}

    {#if view === 'lobby'}
      <Lobby
        {tables}
        onJoinTable={joinTable}
        onRefresh={loadTables}
      />
    {:else}
      {@const tableCurrency = getTableCurrency(currentTableInfo)}
      <div class="game-layout">
        <div class="table-area">
          <PokerTable
            {tableState}
            {myCards}
            {actionPending}
            onAction={handleTableAction}
            tableBalance={myBalance}
            currency={tableCurrency}
            avatarStyle={currentAvatarStyle}
            customName={currentCustomName}
            onShowDeposit={() => showDepositModal = true}
            onShowWithdraw={() => showWithdrawModal = true}
            {shuffleProof}
            onShowProof={() => showProofPanel = true}
          />
        </div>

        {#if showProofPanel}
          <aside class="proof-sidebar">
            <div class="sidebar-header">
              <h3>Fairness Proof</h3>
              <button class="close-btn" onclick={() => showProofPanel = false}>×</button>
            </div>
            <ShuffleProof
              proof={shuffleProof}
              handNumber={tableState?.hand_number}
              {tableActor}
            />
          </aside>
        {/if}
      </div>
    {/if}
  </main>

  <footer>
    <div class="footer-disclaimer">
      <div class="disclaimer-content">
        <p class="disclaimer-warning">
          <span class="warning-icon">⚠️</span>
          <strong>DISCLAIMER:</strong> Unaudited code with known bugs. This is for educational and testing purposes only. Any deposit of ICP or Bitcoin is at your own risk—your funds are NOT safe. Expect to lose everything you deposit. Online gambling is illegal in many jurisdictions. Only use where legally permitted. 18+ only.
        </p>
        <p class="disclaimer-info">
          No middleman, no house. Built to demonstrate the power of the Internet Computer: 100% on-chain—frontend, backend, and game logic all running on smart contracts (canisters). Provably fair, fully transparent, and completely decentralized.
        </p>
        <p class="disclaimer-ai">
          This entire project was built 100% by AI. <span class="warning-icon">⚠️</span>
        </p>
      </div>
    </div>
    <div class="footer-bottom">
      <div class="footer-left">
        <span class="powered-by">Powered by</span>
        <span class="icp-logo">Internet Computer</span>
      </div>
      <div class="footer-center">
        <button class="footer-link" onclick={() => showHowItWorks = true}>How It Works</button>
        <span class="footer-divider">|</span>
        <button class="footer-link" onclick={() => showVerify = true}>Verify Code</button>
      </div>
      <div class="footer-right">
        <span class="version">v0.1.0-alpha</span>
      </div>
    </div>
  </footer>
</div>

<!-- Hand History Modal -->
{#if showHandHistory}
  <HandHistory
    tableId={currentTableInfo?.canister_id?.[0]}
    {tableActor}
    handNumber={tableState?.hand_number || 0}
    onClose={() => { showHandHistory = false; }}
  />
{/if}

{#if showDepositModal}
  <DepositModal
    {tableActor}
    tableCanisterId={currentTableInfo?.canister_id?.[0]}
    currency={getTableCurrency(currentTableInfo)}
    onClose={() => { showDepositModal = false; }}
    onDepositSuccess={() => { refreshAllBalances(); loadTableState(); }}
  />
{/if}

{#if showWithdrawModal}
  <WithdrawModal
    {tableActor}
    currentBalance={myBalance}
    currency={getTableCurrency(currentTableInfo)}
    onClose={() => { showWithdrawModal = false; }}
    onWithdrawSuccess={() => { refreshAllBalances(); loadTableState(); }}
  />
{/if}

{#if showHowItWorks}
  <HowItWorks onClose={() => { showHowItWorks = false; }} />
{/if}

{#if showVerify}
  <div class="modal-backdrop" onclick={() => showVerify = false} role="button" tabindex="-1" aria-label="Close"></div>
  <div class="verify-modal">
    <button class="close-btn" onclick={() => showVerify = false}>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>

    <h2>Verify the Code</h2>
    <p class="verify-intro">
      Every canister on the Internet Computer has a publicly visible WASM hash.
      You can verify that what's running matches the source code.
    </p>

    <div class="verify-section">
      <h3>1. Check Deployed Hash</h3>
      <p>Query the IC to see the hash of the deployed canister:</p>
      <div class="code-block">
        <code>dfx canister info qrhly-eaaaa-aaaaj-qousa-cai --network ic</code>
        <button class="copy-btn" onclick={() => navigator.clipboard.writeText('dfx canister info qrhly-eaaaa-aaaaj-qousa-cai --network ic')}>Copy</button>
      </div>
    </div>

    <div class="verify-section">
      <h3>2. Build from Source</h3>
      <p>Clone the repo and build in Docker to get the source hash:</p>
      <div class="code-block">
        <code>git clone https://github.com/JoshDFN/cleardeck<br/>cd cleardeck<br/>docker build -t verify .</code>
        <button class="copy-btn" onclick={() => navigator.clipboard.writeText('git clone https://github.com/JoshDFN/cleardeck && cd cleardeck && docker build -t verify .')}>Copy</button>
      </div>
    </div>

    <div class="verify-section">
      <h3>3. Compare</h3>
      <p>If the hashes match, the deployed code is verified to be the source code. No trust required.</p>
    </div>

    <div class="canister-ids">
      <h3>Deployed Canister Hashes</h3>
      <p class="hash-note">Verify with: <code>dfx canister info &lt;ID&gt; --network ic</code></p>
      <table>
        <tbody>
          <tr>
            <td>Lobby</td>
            <td><code class="canister-id">kpfcd-kyaaa-aaaaj-qor3a-cai</code></td>
          </tr>
          <tr>
            <td colspan="2" class="hash-row"><code class="hash">0xff6c893de860c5bd8dae85d67344ee94619fb6faad6d68b3265c9a6fe5a2cef8</code></td>
          </tr>
          <tr>
            <td>Tables (all)</td>
            <td><code class="canister-id">kieex..., lfkaz..., lclgn..., qrhly...</code></td>
          </tr>
          <tr>
            <td colspan="2" class="hash-row"><code class="hash">0x1b84e2fa1c35fd50001cb059ba644784fe5a6b36a093a2ac3e56c39bc3bbdf28</code></td>
          </tr>
          <tr>
            <td>History</td>
            <td><code class="canister-id">kggj7-4qaaa-aaaaj-qor2q-cai</code></td>
          </tr>
          <tr>
            <td colspan="2" class="hash-row"><code class="hash">0xc9b1b78a6490cd2034b967dc9de11bb6377170e0e5ef96144b546da3a93dd8f9</code></td>
          </tr>
        </tbody>
      </table>
      <p class="hash-note">All table canisters use the same WASM (same hash).</p>
    </div>

    <a href="https://github.com/JoshDFN/cleardeck" target="_blank" rel="noopener" class="github-link">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
      </svg>
      View Source on GitHub
    </a>
  </div>
{/if}

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    background: #0a0a0f;
    min-height: 100vh;
    color: #e0e0e0;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    overflow-x: hidden;
  }

  /* Disclaimer Banner */
  .alpha-warning-banner {
    background: linear-gradient(180deg, rgba(185, 28, 28, 0.95), rgba(140, 20, 20, 0.95));
    color: rgba(255, 255, 255, 0.95);
    padding: 16px 24px;
    position: relative;
    z-index: 100;
    border-bottom: 1px solid rgba(0, 0, 0, 0.3);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  }

  .banner-content {
    max-width: 800px;
    margin: 0 auto;
  }

  .banner-content p {
    margin: 0 0 8px 0;
    line-height: 1.5;
    font-size: 12px;
  }

  .banner-content p:last-child {
    margin-bottom: 0;
  }

  p.banner-warning {
    color: rgba(255, 255, 255, 0.95);
    text-align: left;
  }

  p.banner-warning strong {
    color: #fef08a;
    letter-spacing: 0.5px;
  }

  .banner-content .warning-icon {
    font-size: 13px;
  }

  p.banner-info {
    color: rgba(255, 255, 255, 0.75);
    text-align: left;
    padding-left: 20px;
  }

  p.banner-ai {
    color: #c4b5fd;
    text-align: left;
    padding-left: 20px;
    font-weight: 500;
  }

  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
  }

  /* Ambient background */
  .bg-effects {
    position: fixed;
    inset: 0;
    pointer-events: none;
    overflow: hidden;
    z-index: 0;
  }

  .glow {
    position: absolute;
    border-radius: 50%;
    filter: blur(100px);
    opacity: 0.15;
  }

  .glow-1 {
    width: 600px;
    height: 600px;
    background: #00d4aa;
    top: -200px;
    left: -100px;
    animation: float 20s ease-in-out infinite;
  }

  .glow-2 {
    width: 500px;
    height: 500px;
    background: #6366f1;
    bottom: -150px;
    right: -100px;
    animation: float 25s ease-in-out infinite reverse;
  }

  .glow-3 {
    width: 400px;
    height: 400px;
    background: #f59e0b;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    animation: pulse 15s ease-in-out infinite;
  }

  @keyframes float {
    0%, 100% { transform: translate(0, 0); }
    50% { transform: translate(50px, 30px); }
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.1; transform: translate(-50%, -50%) scale(1); }
    50% { opacity: 0.2; transform: translate(-50%, -50%) scale(1.1); }
  }

  /* Header */
  header {
    position: relative;
    z-index: 10;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 32px;
    background: rgba(10, 10, 15, 0.8);
    backdrop-filter: blur(20px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .header-left, .header-right {
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 200px;
  }

  .header-right {
    justify-content: flex-end;
  }

  .current-table-name {
    color: #00d4aa;
    font-weight: 600;
    font-size: 14px;
    padding: 6px 12px;
    background: rgba(0, 212, 170, 0.1);
    border-radius: 8px;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo-mark {
    position: relative;
    width: 44px;
    height: 44px;
    background: linear-gradient(135deg, #00d4aa 0%, #00b894 100%);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 20px rgba(0, 212, 170, 0.3);
  }

  .suit {
    position: absolute;
    font-size: 18px;
    color: white;
    text-shadow: 0 1px 2px rgba(0,0,0,0.3);
  }

  .suit-1 {
    top: 6px;
    left: 8px;
  }

  .suit-2 {
    bottom: 6px;
    right: 8px;
    color: #0a0a0f;
  }

  .logo-text {
    display: flex;
    flex-direction: column;
  }

  .brand {
    font-size: 22px;
    font-weight: 700;
    color: white;
    letter-spacing: -0.5px;
  }

  .tagline {
    font-size: 11px;
    color: #00d4aa;
    text-transform: uppercase;
    letter-spacing: 1.5px;
    font-weight: 500;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #a0a0a0;
    padding: 10px 16px;
    border-radius: 10px;
    cursor: pointer;
    font-size: 14px;
    transition: all 0.2s;
  }

  .back-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .verify-btn, .history-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(0, 212, 170, 0.1);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
    padding: 10px 16px;
    border-radius: 10px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    transition: all 0.2s;
  }

  .history-btn {
    background: rgba(99, 102, 241, 0.1);
    border: 1px solid rgba(99, 102, 241, 0.3);
    color: #6366f1;
  }

  .verify-btn:hover {
    background: rgba(0, 212, 170, 0.2);
    box-shadow: 0 0 20px rgba(0, 212, 170, 0.2);
  }

  .history-btn:hover {
    background: rgba(99, 102, 241, 0.2);
    box-shadow: 0 0 20px rgba(99, 102, 241, 0.2);
  }

  /* Sound toggle button */
  .sound-toggle-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 10px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    transition: all 0.2s;
  }

  .sound-toggle-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
    border-color: rgba(255, 255, 255, 0.2);
  }

  /* Toast notifications */
  .toast {
    position: fixed;
    top: 80px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 100;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 20px;
    border-radius: 12px;
    backdrop-filter: blur(20px);
    animation: slideDown 0.3s ease-out;
  }

  .toast.error {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
  }

  .toast.success {
    background: rgba(0, 212, 170, 0.15);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
  }

  .toast button {
    background: none;
    border: none;
    color: inherit;
    font-size: 20px;
    cursor: pointer;
    padding: 0 0 0 8px;
    opacity: 0.7;
  }

  .toast button:hover {
    opacity: 1;
  }

  @keyframes slideDown {
    from { transform: translateX(-50%) translateY(-20px); opacity: 0; }
    to { transform: translateX(-50%) translateY(0); opacity: 1; }
  }

  /* Main content */
  main {
    flex: 1;
    position: relative;
    z-index: 1;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 80px;
    gap: 16px;
    color: #666;
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid rgba(0, 212, 170, 0.1);
    border-top-color: #00d4aa;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Game layout */
  .game-layout {
    display: flex;
    min-height: calc(100vh - 140px);
    padding-bottom: 20px;
  }

  .table-area {
    flex: 1;
    padding: 20px;
    padding-bottom: 80px;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  /* Balance bar */
  .balance-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 20px;
    padding: 12px 24px;
    background: rgba(15, 15, 25, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    margin-bottom: 16px;
    align-self: center;
  }

  .balance-display {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .balance-label {
    font-size: 13px;
    color: #888;
  }

  .balance-amount {
    font-size: 16px;
    font-weight: 700;
    color: #fbbf24;
  }

  .deposit-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #00d4aa 0%, #00a88a 100%);
    border: none;
    color: white;
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    transition: all 0.2s;
  }

  .deposit-btn:hover {
    background: linear-gradient(135deg, #00e4ba 0%, #00b89a 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(0, 212, 170, 0.3);
  }

  .withdraw-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    border: none;
    color: white;
    padding: 8px 16px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    transition: all 0.2s;
  }

  .withdraw-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, #fbbf24 0%, #f59e0b 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(245, 158, 11, 0.3);
  }

  .withdraw-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Proof sidebar */
  .proof-sidebar {
    width: 360px;
    background: rgba(15, 15, 20, 0.95);
    border-left: 1px solid rgba(255, 255, 255, 0.06);
    padding: 20px;
    overflow-y: auto;
    animation: slideIn 0.3s ease-out;
  }

  @keyframes slideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  .sidebar-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: white;
  }

  .close-btn {
    background: none;
    border: none;
    color: #666;
    font-size: 24px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  .close-btn:hover {
    color: white;
  }

  /* Footer */
  footer {
    position: relative;
    z-index: 10;
    display: flex;
    flex-direction: column;
    background: rgba(10, 10, 15, 0.8);
    backdrop-filter: blur(20px);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 13px;
  }

  .footer-disclaimer {
    padding: 20px 32px;
    background: linear-gradient(180deg, rgba(245, 158, 11, 0.12) 0%, rgba(245, 158, 11, 0.06) 100%);
    border-bottom: 1px solid rgba(245, 158, 11, 0.2);
  }

  .disclaimer-content {
    max-width: 800px;
    margin: 0 auto;
  }

  .disclaimer-content p {
    margin: 0 0 12px 0;
    line-height: 1.6;
  }

  .disclaimer-content p:last-child {
    margin-bottom: 0;
  }

  p.disclaimer-warning {
    color: #f59e0b;
    font-size: 12px;
    text-align: left;
  }

  p.disclaimer-warning strong {
    color: #fbbf24;
    letter-spacing: 0.5px;
  }

  .disclaimer-content .warning-icon {
    font-size: 13px;
  }

  p.disclaimer-info {
    color: #999;
    font-size: 12px;
    text-align: left;
    padding-left: 22px;
  }

  p.disclaimer-ai {
    color: #a855f7;
    font-size: 12px;
    font-weight: 500;
    text-align: left;
    padding-left: 22px;
  }

  .footer-bottom {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 32px;
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .powered-by {
    color: #555;
  }

  .icp-logo {
    color: #a855f7;
    font-weight: 600;
  }

  .footer-center {
    display: flex;
    gap: 24px;
  }

  .footer-link {
    color: #666;
    cursor: pointer;
    transition: color 0.2s;
    background: none;
    border: none;
    font-size: inherit;
    font-family: inherit;
    padding: 0;
  }

  .footer-link:hover {
    color: white;
  }

  .footer-right {
    color: #444;
  }

  /* Responsive */
  @media (max-width: 768px) {
    header {
      padding: 12px 16px;
      flex-wrap: wrap;
      gap: 12px;
    }

    .header-left, .header-right {
      min-width: auto;
      flex: 1 1 auto;
    }

    .logo {
      order: -1;
      width: 100%;
      justify-content: center;
      margin-bottom: 8px;
    }

    .logo-text {
      font-size: 18px;
    }

    .tagline {
      font-size: 10px;
    }

    .back-btn, .verify-btn, .history-btn, .sound-toggle-btn {
      padding: 8px 12px;
      font-size: 12px;
    }

    .sound-toggle-btn {
      padding: 8px;
    }

    .game-layout {
      flex-direction: column;
    }

    .table-area {
      padding: 12px;
    }

    .balance-bar {
      flex-direction: column;
      gap: 12px;
      padding: 10px 16px;
    }

    .balance-display {
      width: 100%;
      justify-content: center;
    }

    .proof-sidebar {
      position: fixed;
      inset: 0;
      width: 100%;
      z-index: 50;
    }

    .footer-disclaimer {
      padding: 16px;
    }

    .disclaimer-content p {
      margin-bottom: 10px;
    }

    p.disclaimer-warning,
    p.disclaimer-info,
    p.disclaimer-ai {
      font-size: 11px;
      text-align: left;
    }

    p.disclaimer-info,
    p.disclaimer-ai {
      padding-left: 18px;
    }

    .footer-bottom {
      flex-direction: column;
      gap: 10px;
      padding: 10px 16px;
    }

    .footer-center {
      flex-direction: column;
      gap: 8px;
    }
  }

  @media (max-width: 480px) {
    header {
      padding: 10px 12px;
    }

    .logo-mark {
      width: 36px;
      height: 36px;
    }

    .brand {
      font-size: 18px;
    }

    .tagline {
      font-size: 9px;
    }

    .back-btn, .verify-btn, .history-btn {
      padding: 6px 10px;
      font-size: 11px;
    }

    .table-area {
      padding: 8px;
    }
  }

  /* Modal Backdrop */
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  /* Verify Modal */
  .verify-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: linear-gradient(180deg, #1a1a24 0%, #12121a 100%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 32px;
    max-width: 600px;
    width: 90%;
    max-height: 85vh;
    overflow-y: auto;
    z-index: 1001;
    box-shadow: 0 25px 50px rgba(0, 0, 0, 0.5);
  }

  .verify-modal h2 {
    color: #00d4aa;
    margin: 0 0 12px 0;
    font-size: 24px;
  }

  .verify-intro {
    color: #999;
    font-size: 14px;
    margin-bottom: 24px;
    line-height: 1.6;
  }

  .verify-section {
    margin-bottom: 24px;
  }

  .verify-section h3 {
    color: #ccc;
    font-size: 14px;
    margin: 0 0 8px 0;
    font-weight: 600;
  }

  .verify-section p {
    color: #888;
    font-size: 13px;
    margin: 0 0 12px 0;
  }

  .code-block {
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 12px 16px;
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }

  .code-block code {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 12px;
    color: #4ade80;
    word-break: break-all;
    line-height: 1.6;
  }

  .copy-btn {
    background: rgba(0, 212, 170, 0.2);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .copy-btn:hover {
    background: rgba(0, 212, 170, 0.3);
  }

  .canister-ids {
    margin-top: 24px;
    padding-top: 24px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
  }

  .canister-ids h3 {
    color: #ccc;
    font-size: 14px;
    margin: 0 0 12px 0;
  }

  .canister-ids table {
    width: 100%;
    border-collapse: collapse;
  }

  .canister-ids td {
    padding: 8px 0;
    font-size: 13px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .canister-ids td:first-child {
    color: #888;
    width: 100px;
  }

  .canister-ids td code {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 11px;
    color: #a78bfa;
    background: rgba(167, 139, 250, 0.1);
    padding: 4px 8px;
    border-radius: 4px;
  }

  .github-link {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 24px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    color: #ccc;
    text-decoration: none;
    font-size: 14px;
    font-weight: 500;
    transition: all 0.2s;
  }

  .github-link:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .footer-divider {
    color: #444;
  }

  .verify-modal .close-btn {
    position: absolute;
    top: 16px;
    right: 16px;
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    padding: 4px;
    transition: color 0.2s;
  }

  .verify-modal .close-btn:hover {
    color: #fff;
  }

  .hash-note {
    font-size: 12px;
    color: #888;
    margin: 4px 0 12px 0;
  }

  .hash-note code {
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 11px;
  }

  .canister-id {
    color: #4dabf7;
    font-size: 12px;
  }

  .hash-row {
    padding-top: 0 !important;
  }

  .hash-row .hash {
    font-size: 10px;
    color: #69db7c;
    word-break: break-all;
    display: block;
    padding: 4px 8px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 4px;
    margin-bottom: 8px;
  }
</style>
