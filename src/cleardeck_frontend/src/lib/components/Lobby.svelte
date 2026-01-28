<script>
  import IcpLogo from './IcpLogo.svelte';

  const { tables, onJoinTable, onRefresh } = $props();

  let activeFilter = $state('all'); // 'all' | 'available'
  let currencyFilter = $state('all'); // 'all' | 'ICP' | 'BTC'
  let sortColumn = $state('stakes'); // 'name' | 'stakes' | 'players' | 'buyin'
  let sortDirection = $state('asc'); // 'asc' | 'desc'
  let viewMode = $state(typeof localStorage !== 'undefined' ? (localStorage.getItem('lobby_view') || 'list') : 'list'); // 'list' | 'grid'

  function setViewMode(mode) {
    viewMode = mode;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('lobby_view', mode);
    }
  }

  function getStatusColor(status) {
    if (!status) return '#666';
    const key = Object.keys(status)[0];
    switch(key) {
      case 'WaitingForPlayers': return '#f59e0b';
      case 'InProgress': return '#00d4aa';
      case 'Paused': return '#6366f1';
      default: return '#666';
    }
  }

  function getStatusText(status) {
    if (!status) return 'Unknown';
    const key = Object.keys(status)[0];
    if (key === 'WaitingForPlayers') return 'Waiting';
    if (key === 'InProgress') return 'Playing';
    return key;
  }

  // Get table currency from currency field or config
  function getTableCurrency(tableInfo) {
    function extractCurrency(optCurrency) {
      if (!optCurrency) return null;
      if (Array.isArray(optCurrency)) {
        if (optCurrency.length === 0) return null;
        const inner = optCurrency[0];
        if (inner && typeof inner === 'object') {
          const key = Object.keys(inner)[0];
          return key ? key.toUpperCase() : null;
        }
        return null;
      }
      if (typeof optCurrency === 'object' && optCurrency !== null) {
        const key = Object.keys(optCurrency)[0];
        return key ? key.toUpperCase() : null;
      }
      if (typeof optCurrency === 'string') {
        return optCurrency.toUpperCase();
      }
      return null;
    }

    const tableCurrency = extractCurrency(tableInfo.currency);
    if (tableCurrency) return tableCurrency;
    const configCurrency = extractCurrency(tableInfo.config?.currency);
    if (configCurrency) return configCurrency;
    return 'ICP';
  }

  // Format amount based on currency
  function formatAmount(e8s, currency) {
    const num = Number(e8s);
    if (currency === 'BTC') {
      if (num >= 100_000_000) return `${(num / 100_000_000).toFixed(2)} BTC`;
      if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
      if (num >= 1_000) return `${(num / 1_000).toFixed(0)}K`;
      return `${num}`;
    } else {
      const icp = num / 100_000_000;
      if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K`;
      if (icp >= 1) return icp.toFixed(2);
      if (icp >= 0.01) return icp.toFixed(2);
      return icp.toFixed(4);
    }
  }

  // Format blinds display
  function formatBlinds(smallBlind, bigBlind, currency) {
    const sb = formatAmount(smallBlind, currency);
    const bb = formatAmount(bigBlind, currency);
    return `${sb}/${bb}`;
  }

  // Format buy-in range
  function formatBuyIn(min, max, currency) {
    return `${formatAmount(min, currency)} - ${formatAmount(max, currency)}`;
  }

  function getStakeLabel(smallBlind, currency) {
    const sb = Number(smallBlind);
    if (currency === 'BTC') {
      if (sb <= 200) return 'Micro';
      if (sb <= 1000) return 'Low';
      if (sb <= 5000) return 'Medium';
      if (sb <= 20000) return 'High';
      return 'VIP';
    } else {
      const sbIcp = sb / 100_000_000;
      if (sbIcp <= 0.02) return 'Micro';
      if (sbIcp <= 0.10) return 'Low';
      if (sbIcp <= 0.50) return 'Medium';
      if (sbIcp <= 2) return 'High';
      return 'VIP';
    }
  }

  // Sort handler
  function handleSort(column) {
    if (sortColumn === column) {
      sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      sortColumn = column;
      sortDirection = 'asc';
    }
  }

  // Filter and sort tables
  const filteredTables = $derived.by(() => {
    let result = tables;

    // Filter by availability
    if (activeFilter === 'available') {
      result = result.filter(t => t.player_count < t.config.max_players);
    }

    // Filter by currency
    if (currencyFilter !== 'all') {
      result = result.filter(t => getTableCurrency(t) === currencyFilter);
    }

    // Sort
    result = [...result].sort((a, b) => {
      let comparison = 0;
      switch (sortColumn) {
        case 'name':
          comparison = a.name.localeCompare(b.name);
          break;
        case 'stakes':
          comparison = Number(a.config.small_blind) - Number(b.config.small_blind);
          break;
        case 'players':
          comparison = a.player_count - b.player_count;
          break;
        case 'buyin':
          comparison = Number(a.config.min_buy_in) - Number(b.config.min_buy_in);
          break;
      }
      return sortDirection === 'asc' ? comparison : -comparison;
    });

    return result;
  });

  // Counts
  const totalPlayers = $derived(tables.reduce((sum, t) => sum + t.player_count, 0));
  const availableTables = $derived(tables.filter(t => t.player_count < t.config.max_players).length);
  const icpTableCount = $derived(tables.filter(t => getTableCurrency(t) === 'ICP').length);
  const btcTableCount = $derived(tables.filter(t => getTableCurrency(t) === 'BTC').length);
</script>

<div class="lobby">
  <!-- Compact Header -->
  <header class="lobby-header">
    <div class="header-left">
      <h1>Tables</h1>
      <div class="quick-stats">
        <span class="stat"><strong>{tables.length}</strong> Tables</span>
        <span class="divider">|</span>
        <span class="stat"><strong>{totalPlayers}</strong> Players</span>
        <span class="divider">|</span>
        <span class="stat available"><strong>{availableTables}</strong> Available</span>
      </div>
    </div>
    <div class="header-right">
      <div class="view-toggle">
        <button class:active={viewMode === 'list'} onclick={() => setViewMode('list')} title="List View">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/>
          </svg>
        </button>
        <button class:active={viewMode === 'grid'} onclick={() => setViewMode('grid')} title="Grid View">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
            <rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>
          </svg>
        </button>
      </div>
      <button class="refresh-btn" onclick={onRefresh}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M23 4v6h-6M1 20v-6h6"/>
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
        </svg>
        Refresh
      </button>
    </div>
  </header>

  <!-- Filters Bar -->
  <div class="filters-bar">
    <div class="filter-left">
      <div class="filter-tabs">
        <button class:active={activeFilter === 'all'} onclick={() => activeFilter = 'all'}>
          All Tables
        </button>
        <button class:active={activeFilter === 'available'} onclick={() => activeFilter = 'available'}>
          Available Seats
        </button>
      </div>

      <div class="currency-pills">
        <button class:active={currencyFilter === 'all'} onclick={() => currencyFilter = 'all'}>
          All
        </button>
        <button class="icp" class:active={currencyFilter === 'ICP'} onclick={() => currencyFilter = 'ICP'}>
          <IcpLogo size={14} />
          ICP
          {#if icpTableCount > 0}<span class="badge">{icpTableCount}</span>{/if}
        </button>
        <button class="btc" class:active={currencyFilter === 'BTC'} onclick={() => currencyFilter = 'BTC'}>
          <svg width="14" height="14" viewBox="0 0 64 64">
            <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
            <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
          </svg>
          BTC
          {#if btcTableCount > 0}<span class="badge">{btcTableCount}</span>{/if}
        </button>
      </div>
    </div>
  </div>

  <!-- Tables Container -->
  <div class="tables-container" class:grid-mode={viewMode === 'grid'}>
    {#if filteredTables.length === 0}
      <div class="empty-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"/>
          <path d="M12 6v6l4 2"/>
        </svg>
        <p>No tables found</p>
        <button class="refresh-btn small" onclick={onRefresh}>Refresh</button>
      </div>
    {:else if viewMode === 'list'}
      <!-- LIST VIEW -->
      <table class="tables-list">
        <thead>
          <tr>
            <th class="col-game sortable" class:sorted={sortColumn === 'name'} onclick={() => handleSort('name')}>
              Game
              <span class="sort-icon">{sortColumn === 'name' ? (sortDirection === 'asc' ? '▲' : '▼') : ''}</span>
            </th>
            <th class="col-stakes sortable" class:sorted={sortColumn === 'stakes'} onclick={() => handleSort('stakes')}>
              Stakes
              <span class="sort-icon">{sortColumn === 'stakes' ? (sortDirection === 'asc' ? '▲' : '▼') : ''}</span>
            </th>
            <th class="col-players sortable" class:sorted={sortColumn === 'players'} onclick={() => handleSort('players')}>
              Players
              <span class="sort-icon">{sortColumn === 'players' ? (sortDirection === 'asc' ? '▲' : '▼') : ''}</span>
            </th>
            <th class="col-buyin sortable" class:sorted={sortColumn === 'buyin'} onclick={() => handleSort('buyin')}>
              Buy-in
              <span class="sort-icon">{sortColumn === 'buyin' ? (sortDirection === 'asc' ? '▲' : '▼') : ''}</span>
            </th>
            <th class="col-status">Status</th>
            <th class="col-action"></th>
          </tr>
        </thead>
        <tbody>
          {#each filteredTables as table}
            {@const currency = getTableCurrency(table)}
            {@const isFull = table.player_count >= table.config.max_players}
            {@const fillPercent = (table.player_count / table.config.max_players) * 100}
            <tr class:btc={currency === 'BTC'} class:full={isFull} onclick={() => onJoinTable(table)}>
              <td class="col-game">
                <div class="game-info">
                  <span class="table-name">{table.name}</span>
                  <span class="table-meta">
                    <span class="currency-tag" class:btc={currency === 'BTC'} class:icp={currency === 'ICP'}>
                      {#if currency === 'BTC'}
                        <svg width="12" height="12" viewBox="0 0 64 64">
                          <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                          <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
                        </svg>
                      {:else}
                        <IcpLogo size={12} />
                      {/if}
                      {currency}
                    </span>
                    <span class="stake-level">{getStakeLabel(table.config.small_blind, currency)}</span>
                  </span>
                </div>
              </td>
              <td class="col-stakes">
                <span class="stakes-value" class:btc={currency === 'BTC'}>
                  {formatBlinds(table.config.small_blind, table.config.big_blind, currency)}
                </span>
                <span class="stakes-unit" class:btc={currency === 'BTC'}>
                  {#if currency === 'BTC'}
                    <svg width="10" height="10" viewBox="0 0 64 64">
                      <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                      <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
                    </svg>
                  {:else}
                    <IcpLogo size={10} />
                  {/if}
                  {currency === 'BTC' ? 'sats' : 'ICP'}
                </span>
              </td>
              <td class="col-players">
                <div class="players-cell">
                  <div class="players-bar">
                    <div class="players-fill" class:btc={currency === 'BTC'} style:width="{fillPercent}%"></div>
                  </div>
                  <span class="players-text">
                    <strong>{table.player_count}</strong>/{table.config.max_players}
                  </span>
                </div>
              </td>
              <td class="col-buyin">
                <span class="buyin-value">{formatBuyIn(table.config.min_buy_in, table.config.max_buy_in, currency)}</span>
                <span class="buyin-unit" class:btc={currency === 'BTC'}>
                  {#if currency === 'BTC'}
                    <svg width="10" height="10" viewBox="0 0 64 64">
                      <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                      <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
                    </svg>
                    sats
                  {:else}
                    <IcpLogo size={10} />
                    ICP
                  {/if}
                </span>
              </td>
              <td class="col-status">
                <span class="status-badge" style:--status-color={getStatusColor(table.status)}>
                  <span class="status-dot"></span>
                  {getStatusText(table.status)}
                </span>
              </td>
              <td class="col-action">
                <button class="join-btn" class:btc={currency === 'BTC'} disabled={isFull}>
                  {#if isFull}
                    Full
                  {:else}
                    Seat
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                      <path d="M5 12h14M12 5l7 7-7 7"/>
                    </svg>
                  {/if}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <!-- GRID VIEW -->
      <div class="tables-grid">
        {#each filteredTables as tableInfo}
          {@const currency = getTableCurrency(tableInfo)}
          {@const isFull = tableInfo.player_count >= tableInfo.config.max_players}
          <div class="table-card" class:btc-table={currency === 'BTC'} onclick={() => onJoinTable(tableInfo)} onkeydown={(e) => e.key === 'Enter' && onJoinTable(tableInfo)} role="button" tabindex="0">
            <div class="card-header">
              <div class="table-name-section">
                <div class="name-row">
                  <h3>{tableInfo.name}</h3>
                  <span class="currency-badge" class:btc={currency === 'BTC'} class:icp={currency === 'ICP'}>
                    {#if currency === 'BTC'}
                      <svg width="12" height="12" viewBox="0 0 64 64">
                        <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                        <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
                      </svg>
                    {:else}
                      <IcpLogo size={12} />
                    {/if}
                    {currency}
                  </span>
                </div>
                <span class="stake-badge">{getStakeLabel(tableInfo.config.small_blind, currency)}</span>
              </div>
              <div class="status-indicator" style:--status-color={getStatusColor(tableInfo.status)}>
                <span class="status-dot"></span>
                {getStatusText(tableInfo.status)}
              </div>
            </div>

            <div class="card-body">
              <div class="blinds-display" class:btc={currency === 'BTC'}>
                <span class="blinds-value">
                  {formatBlinds(tableInfo.config.small_blind, tableInfo.config.big_blind, currency)}
                </span>
                <span class="blinds-label">{currency === 'BTC' ? 'sats' : 'ICP'} Blinds</span>
              </div>

              <div class="info-grid">
                <div class="info-item">
                  <span class="info-label">Buy-in</span>
                  <span class="info-value">{formatBuyIn(tableInfo.config.min_buy_in, tableInfo.config.max_buy_in, currency)}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">Players</span>
                  <span class="info-value players">
                    <span class="current">{tableInfo.player_count}</span>
                    <span class="separator">/</span>
                    <span class="max">{tableInfo.config.max_players}</span>
                  </span>
                </div>
              </div>

              <!-- Player slots visualization -->
              <div class="seats-visual">
                {#each Array(Number(tableInfo.config.max_players)) as _, i}
                  <div class="seat" class:occupied={i < tableInfo.player_count} class:btc={currency === 'BTC'}></div>
                {/each}
              </div>
            </div>

            <div class="card-footer">
              <button
                class="join-btn-card"
                class:btc={currency === 'BTC'}
                disabled={isFull}
              >
                {#if isFull}
                  Table Full
                {:else}
                  Join Table
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M5 12h14M12 5l7 7-7 7"/>
                  </svg>
                {/if}
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Footer info -->
  <div class="lobby-footer">
    <span class="footer-note">{viewMode === 'list' ? 'Click any row' : 'Click any card'} to join • All games are Texas Hold'em No Limit • 100% Verifiable</span>
  </div>
</div>

<style>
  .lobby {
    max-width: 1100px;
    margin: 0 auto;
    padding: 20px;
  }

  /* Header */
  .lobby-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding-bottom: 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 20px;
  }

  .lobby-header h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    color: white;
  }

  .quick-stats {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: #666;
  }

  .quick-stats .stat strong {
    color: #aaa;
  }

  .quick-stats .stat.available strong {
    color: #00d4aa;
  }

  .quick-stats .divider {
    color: #333;
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
    padding: 8px 14px;
    border-radius: 8px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
    border-color: rgba(255, 255, 255, 0.2);
  }

  .refresh-btn.small {
    padding: 6px 12px;
    font-size: 12px;
  }

  /* Filters Bar */
  .filters-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
    flex-wrap: wrap;
    gap: 12px;
  }

  .filter-left {
    display: flex;
    gap: 16px;
    align-items: center;
    flex-wrap: wrap;
  }

  .filter-tabs {
    display: flex;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    padding: 3px;
  }

  .filter-tabs button {
    background: none;
    border: none;
    color: #555;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .filter-tabs button.active {
    background: rgba(0, 212, 170, 0.15);
    color: #00d4aa;
  }

  .filter-tabs button:hover:not(.active) {
    color: #888;
  }

  .currency-pills {
    display: flex;
    gap: 6px;
  }

  .currency-pills button {
    display: flex;
    align-items: center;
    gap: 5px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    color: #555;
    padding: 6px 12px;
    border-radius: 20px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .currency-pills button:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #888;
  }

  .currency-pills button.active {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.15);
    color: white;
  }

  .currency-pills button.icp.active {
    background: rgba(99, 102, 241, 0.2);
    border-color: rgba(99, 102, 241, 0.3);
    color: #a5b4fc;
  }

  .currency-pills button.btc.active {
    background: rgba(247, 147, 26, 0.2);
    border-color: rgba(247, 147, 26, 0.3);
    color: #f7931a;
  }

  .currency-pills .badge {
    font-size: 10px;
    background: rgba(255, 255, 255, 0.15);
    padding: 1px 5px;
    border-radius: 8px;
    margin-left: 2px;
  }

  /* Tables Container */
  .tables-container {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    overflow: hidden;
  }

  /* Table List */
  .tables-list {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .tables-list thead {
    background: rgba(0, 0, 0, 0.3);
  }

  .tables-list th {
    text-align: left;
    padding: 12px 16px;
    color: #666;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    user-select: none;
  }

  .tables-list th.sortable {
    cursor: pointer;
    transition: color 0.2s;
  }

  .tables-list th.sortable:hover {
    color: #aaa;
  }

  .tables-list th.sorted {
    color: #00d4aa;
  }

  .sort-icon {
    margin-left: 4px;
    font-size: 9px;
  }

  .tables-list tbody tr {
    cursor: pointer;
    transition: background 0.15s;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }

  .tables-list tbody tr:hover {
    background: rgba(0, 212, 170, 0.05);
  }

  .tables-list tbody tr.btc:hover {
    background: rgba(247, 147, 26, 0.05);
  }

  .tables-list tbody tr.full {
    opacity: 0.6;
  }

  .tables-list td {
    padding: 14px 16px;
    vertical-align: middle;
  }

  /* Columns */
  .col-game {
    width: 25%;
  }

  .col-stakes {
    width: 15%;
  }

  .col-players {
    width: 20%;
  }

  .col-buyin {
    width: 20%;
  }

  .col-status {
    width: 12%;
  }

  .col-action {
    width: 8%;
    text-align: right;
  }

  /* Game Info */
  .game-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .table-name {
    color: white;
    font-weight: 600;
    font-size: 14px;
  }

  .table-meta {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .currency-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .currency-tag.icp {
    background: rgba(99, 102, 241, 0.2);
    color: #a5b4fc;
  }

  .currency-tag.btc {
    background: rgba(247, 147, 26, 0.2);
    color: #f7931a;
  }

  .stake-level {
    font-size: 10px;
    color: #00d4aa;
    background: rgba(0, 212, 170, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
  }

  /* Stakes */
  .stakes-value {
    color: #f59e0b;
    font-weight: 700;
    font-size: 14px;
  }

  .stakes-value.btc {
    color: #f7931a;
  }

  .stakes-unit {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 4px;
    color: #555;
    font-size: 11px;
  }

  .stakes-unit.btc {
    color: #f7931a;
  }

  /* Buy-in unit */
  .buyin-unit {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    margin-left: 4px;
    color: #555;
    font-size: 11px;
  }

  .buyin-unit.btc {
    color: #f7931a;
  }

  /* Players */
  .players-cell {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .players-bar {
    flex: 1;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    overflow: hidden;
    max-width: 80px;
  }

  .players-fill {
    height: 100%;
    background: linear-gradient(90deg, #00d4aa, #00b894);
    border-radius: 3px;
    transition: width 0.3s;
  }

  .players-fill.btc {
    background: linear-gradient(90deg, #f7931a, #c77700);
  }

  .players-text {
    color: #888;
    font-size: 12px;
    white-space: nowrap;
  }

  .players-text strong {
    color: white;
    font-size: 14px;
  }

  /* Buy-in */
  .buyin-value {
    color: #888;
    font-size: 12px;
  }

  /* Status */
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--status-color);
    font-size: 12px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    background: var(--status-color);
    border-radius: 50%;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  /* Join Button */
  .join-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: linear-gradient(135deg, #00d4aa 0%, #00b894 100%);
    color: #0a0a0f;
    border: none;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .join-btn.btc {
    background: linear-gradient(135deg, #f7931a 0%, #c77700 100%);
  }

  .join-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 212, 170, 0.3);
  }

  .join-btn.btc:hover:not(:disabled) {
    box-shadow: 0 4px 12px rgba(247, 147, 26, 0.3);
  }

  .join-btn:disabled {
    background: rgba(255, 255, 255, 0.1);
    color: #555;
    cursor: not-allowed;
  }

  /* Empty State */
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 20px;
    color: #555;
    gap: 12px;
  }

  .empty-state p {
    margin: 0;
    color: #666;
  }

  /* Footer */
  .lobby-footer {
    margin-top: 12px;
    text-align: center;
  }

  .footer-note {
    font-size: 11px;
    color: #444;
  }

  /* Responsive */
  @media (max-width: 900px) {
    .col-buyin {
      display: none;
    }

    .col-game { width: 35%; }
    .col-stakes { width: 20%; }
    .col-players { width: 25%; }
    .col-status { width: 15%; }
    .col-action { width: 5%; }
  }

  @media (max-width: 768px) {
    .lobby {
      padding: 12px;
    }

    .lobby-header {
      flex-direction: column;
      align-items: flex-start;
      gap: 12px;
    }

    .header-left {
      flex-direction: column;
      align-items: flex-start;
      gap: 8px;
    }

    .filter-left {
      flex-direction: column;
      align-items: flex-start;
      width: 100%;
    }

    .filter-tabs {
      width: 100%;
    }

    .filter-tabs button {
      flex: 1;
      text-align: center;
    }

    .currency-pills {
      width: 100%;
      justify-content: flex-start;
    }

    .tables-list th,
    .tables-list td {
      padding: 10px 12px;
    }

    .col-status {
      display: none;
    }

    .col-game { width: 40%; }
    .col-stakes { width: 25%; }
    .col-players { width: 25%; }
    .col-action { width: 10%; }

    .players-bar {
      display: none;
    }
  }

  @media (max-width: 480px) {
    .col-stakes {
      display: none;
    }

    .col-game { width: 50%; }
    .col-players { width: 35%; }
    .col-action { width: 15%; }

    .table-name {
      font-size: 13px;
    }

    .join-btn {
      padding: 5px 8px;
      font-size: 11px;
    }

    .join-btn svg {
      display: none;
    }

    .view-toggle {
      display: none;
    }
  }

  /* Header Right */
  .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* View Toggle */
  .view-toggle {
    display: flex;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    padding: 3px;
  }

  .view-toggle button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    background: none;
    border: none;
    color: #555;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .view-toggle button:hover {
    color: #888;
  }

  .view-toggle button.active {
    background: rgba(0, 212, 170, 0.15);
    color: #00d4aa;
  }

  /* Grid View Styles */
  .tables-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
    padding: 16px;
  }

  .table-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 16px;
    padding: 20px;
    cursor: pointer;
    transition: all 0.3s;
  }

  .table-card:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(0, 212, 170, 0.3);
    transform: translateY(-4px);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
  }

  .table-card.btc-table {
    border-color: rgba(247, 147, 26, 0.2);
  }

  .table-card.btc-table:hover {
    border-color: rgba(247, 147, 26, 0.4);
    box-shadow: 0 12px 40px rgba(247, 147, 26, 0.15);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 16px;
  }

  .table-name-section h3 {
    margin: 0 0 6px 0;
    font-size: 16px;
    font-weight: 600;
    color: white;
  }

  .name-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .name-row h3 {
    margin: 0;
  }

  .currency-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .currency-badge.icp {
    color: #a5b4fc;
    background: rgba(99, 102, 241, 0.15);
  }

  .currency-badge.btc {
    color: #f7931a;
    background: rgba(247, 147, 26, 0.15);
  }

  .stake-badge {
    font-size: 10px;
    color: #00d4aa;
    background: rgba(0, 212, 170, 0.1);
    padding: 3px 8px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 600;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--status-color);
  }

  .card-body {
    margin-bottom: 16px;
  }

  .blinds-display {
    text-align: center;
    padding: 16px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 10px;
    margin-bottom: 16px;
  }

  .blinds-display.btc {
    background: linear-gradient(135deg, rgba(247, 147, 26, 0.1), rgba(180, 100, 20, 0.05));
  }

  .blinds-value {
    display: block;
    font-size: 24px;
    font-weight: 700;
    color: #f59e0b;
  }

  .blinds-display.btc .blinds-value {
    color: #f7931a;
  }

  .blinds-label {
    font-size: 11px;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 1px;
  }

  .info-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-bottom: 16px;
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .info-label {
    font-size: 11px;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .info-value {
    font-size: 14px;
    color: #ccc;
    font-weight: 500;
  }

  .info-value.players {
    display: flex;
    align-items: baseline;
    gap: 2px;
  }

  .info-value.players .current {
    font-size: 18px;
    color: #00d4aa;
    font-weight: 700;
  }

  .info-value.players .separator {
    color: #444;
  }

  .info-value.players .max {
    color: #666;
  }

  .seats-visual {
    display: flex;
    justify-content: center;
    gap: 6px;
  }

  .seat {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    transition: all 0.2s;
  }

  .seat.occupied {
    background: rgba(0, 212, 170, 0.2);
    border-color: #00d4aa;
  }

  .seat.btc.occupied {
    background: rgba(247, 147, 26, 0.2);
    border-color: #f7931a;
  }

  .card-footer {
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    padding-top: 16px;
  }

  .join-btn-card {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: linear-gradient(135deg, #00d4aa 0%, #00b894 100%);
    color: #0a0a0f;
    border: none;
    padding: 12px;
    border-radius: 10px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .join-btn-card.btc {
    background: linear-gradient(135deg, #f7931a 0%, #c77700 100%);
  }

  .join-btn-card:hover:not(:disabled) {
    box-shadow: 0 4px 20px rgba(0, 212, 170, 0.3);
  }

  .join-btn-card.btc:hover:not(:disabled) {
    box-shadow: 0 4px 20px rgba(247, 147, 26, 0.3);
  }

  .join-btn-card:disabled {
    background: rgba(255, 255, 255, 0.05);
    color: #555;
    cursor: not-allowed;
  }

  /* Grid responsive */
  @media (max-width: 768px) {
    .tables-grid {
      grid-template-columns: 1fr;
      padding: 12px;
    }

    .header-right {
      width: 100%;
      justify-content: flex-end;
    }
  }
</style>
