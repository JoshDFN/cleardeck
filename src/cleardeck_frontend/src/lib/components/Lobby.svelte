<script>
  // ClearDeck lobby.
  //
  // Table selection is a density problem: a player wants stakes, seat
  // occupancy, whether a hand is actually running, and a way to look at a table
  // before committing money. The reference clients (docs/DESIGN-BAR.md) solve it
  // with a sortable grid plus a master/detail preview pane. PokerStars ships
  // nine columns and puts "Observe" next to "Play Now"; WPT Global ships five
  // and previews the table as a stadium with a "+" on every empty seat. Both
  // treatments are here.
  //
  // Everything on this screen is read from a canister. Two sources:
  //   1. the lobby canister's TableInfo (via the `tables` prop): name, stakes,
  //      buy-in range, seat count, currency, per-table canister id, and the
  //      action clock / time bank the table is configured with;
  //   2. each table canister's `get_table_view()` query, called here — an
  //      anonymous query, so a signed-out visitor sees the same live truth a
  //      seated player does: pot, hand number, phase, the community board, the
  //      seat map with real stacks and per-seat status, the last hand's payout,
  //      and the SHA-256 deck commitment for the hand in progress.
  //
  // What is deliberately NOT here, because the engine does not record it and a
  // lobby statistic that is invented is worse than a missing column: average
  // pot, players-per-flop, hands/hour, and waiting lists. The list footer says
  // so on screen rather than leaving a reader to assume we simply forgot.
  import { onMount } from 'svelte';
  import { auth } from '$lib/auth.js';
  import { createTableActorProxy, lobbyCanisterId } from '$lib/canisters.js';
  import logger from '$lib/logger.js';
  import IcpLogo from './IcpLogo.svelte';
  import HowItWorks from './HowItWorks.svelte';

  const { tables, onJoinTable, onRefresh } = $props();

  /** How often the live per-table view is re-read while the lobby is open. */
  const LIVE_POLL_MS = 12_000;
  const E8S = 100_000_000;

  // ---------------------------------------------------------------------------
  // Local UI state
  // ---------------------------------------------------------------------------

  let openSeatsOnly = $state(false);
  let currencyFilter = $state('all');
  let stakeFilter = $state('all');
  let sortColumn = $state('seats');
  let sortDirection = $state('desc');
  let selectedId = $state(null);
  let showHow = $state(false);
  let signInError = $state(null);
  let signingIn = $state(false);
  let copiedId = $state(null);

  let authState = $state({ isAuthenticated: false, principal: null, isLoading: true });
  const signedIn = $derived(authState.isAuthenticated);

  let density = $state('comfortable');

  function setDensity(next) {
    density = next;
    try { localStorage.setItem('lobby_density', next); } catch { /* private mode */ }
  }

  // ---------------------------------------------------------------------------
  // Candid helpers
  // ---------------------------------------------------------------------------

  /** `opt T` decodes to [] or [value]; everything here must tolerate both. */
  const opt = (v) => (Array.isArray(v) ? (v.length ? v[0] : null) : (v ?? null));

  /** The variant tag of a Candid variant, or null. */
  function tagOf(variant) {
    const v = opt(variant);
    if (!v || typeof v !== 'object') return typeof v === 'string' ? v : null;
    const keys = Object.keys(v);
    return keys.length ? keys[0] : null;
  }

  function canisterIdOf(table) {
    const p = opt(table?.canister_id);
    if (!p) return null;
    return typeof p === 'string' ? p : (p.toText ? p.toText() : String(p));
  }

  function currencyOf(table) {
    const direct = tagOf(table?.currency);
    if (direct) return direct.toUpperCase();
    const fromConfig = tagOf(table?.config?.currency);
    return fromConfig ? fromConfig.toUpperCase() : 'ICP';
  }

  // ---------------------------------------------------------------------------
  // Live table views — the part no rake-taking client can show a signed-out
  // visitor, because their table state is not public and not verifiable.
  // ---------------------------------------------------------------------------

  let liveViews = $state({});
  let liveLoaded = $state(false);
  let liveRequest = 0;

  async function readLiveViews(list) {
    const ids = list.map(canisterIdOf).filter(Boolean);
    if (ids.length === 0) {
      liveViews = {};
      liveLoaded = true;
      return;
    }
    const token = ++liveRequest;
    const pairs = await Promise.all(ids.map(async (id) => {
      try {
        const view = opt(await createTableActorProxy(id).get_table_view());
        return [id, view];
      } catch (e) {
        // A table canister that cannot be reached must not blank the lobby: the
        // row still renders from the lobby record, it just carries no live line.
        logger.debug(`lobby: get_table_view failed for ${id}`, e);
        return [id, null];
      }
    }));
    if (token !== liveRequest) return; // a newer refresh already landed
    liveViews = Object.fromEntries(pairs);
    liveLoaded = true;
  }

  function liveOf(table) {
    const id = canisterIdOf(table);
    return id ? (liveViews[id] ?? null) : null;
  }

  /**
   * The config a player will actually be charged by.
   *
   * The lobby canister keeps its OWN copy of every table's config, written once
   * by `init_microstakes_tables`, and that copy can disagree with the config the
   * table canister was initialised with — on this deployment it does, for two of
   * the three tables (see the wave report). The money a player posts is set by
   * the TABLE canister, so that is what this lobby renders; the lobby record is
   * only a fallback for a table we have not reached yet.
   */
  function effectiveConfig(table) {
    return liveOf(table)?.config ?? table.config;
  }

  const DRIFT_KEYS = [
    ['small_blind', 'small blind'],
    ['big_blind', 'big blind'],
    ['min_buy_in', 'minimum buy-in'],
    ['max_buy_in', 'maximum buy-in'],
    ['max_players', 'seat count'],
    ['ante', 'ante'],
  ];

  /**
   * Keys where the lobby canister's registration disagrees with the table
   * canister's own config. An empty list means the two on-chain sources agree.
   */
  function configDrift(table) {
    const live = liveOf(table)?.config;
    if (!live) return [];
    return DRIFT_KEYS
      .filter(([key]) => Number(live[key]) !== Number(table.config[key]))
      .map(([, label]) => label);
  }

  /**
   * Seat map for a table: one entry per seat, `null` when the seat is empty.
   *
   * `status` is the seat's own PlayerStatus off the table canister, so a seat
   * held by somebody who is sitting out or disconnected is not counted as a
   * player who can be dealt in. That distinction is the closest honest thing
   * this engine records to PokerStars' "players per flop" column.
   */
  function seatsOf(table) {
    const view = liveOf(table);
    const max = Number(effectiveConfig(table).max_players);
    if (view?.players?.length) {
      return Array.from({ length: max }, (_, i) => {
        const p = opt(view.players[i]);
        if (!p) return null;
        const status = tagOf(p.status) ?? 'Active';
        return {
          seat: i,
          name: opt(p.display_name) || null,
          chips: Number(p.chips),
          status,
          active: status === 'Active',
        };
      });
    }
    // No live view yet: fall back to the count the lobby already resolved, so
    // the meter is never blank and never overstates occupancy.
    const filled = Number(table.player_count);
    return Array.from({ length: max }, (_, i) => (
      i < filled ? { seat: i, name: null, chips: null, status: null, active: true } : null
    ));
  }

  const PHASE_LABEL = {
    PreFlop: 'Pre-flop',
    Flop: 'Flop',
    Turn: 'Turn',
    River: 'River',
    Showdown: 'Showdown',
    HandComplete: 'Hand complete',
    WaitingForPlayers: 'Waiting',
  };

  const isDealing = (phase) => Boolean(phase) && phase !== 'WaitingForPlayers' && phase !== 'HandComplete';

  /**
   * The "Now" cell. A hand in progress is stated with its street and pot; with
   * no hand running the honest thing to say is how close the table is to one.
   */
  function nowOf(table) {
    const view = liveOf(table);
    const currency = currencyOf(table);
    const seats = seatsOf(table);
    const ready = seats.filter((s) => s && s.active).length;
    const seated = seats.filter(Boolean).length;
    const phase = view ? tagOf(view.phase) : null;

    if (isDealing(phase)) {
      const pot = Number(view.pot);
      return {
        kind: 'live',
        label: PHASE_LABEL[phase] ?? phase,
        detail: pot > 0 ? `${formatAmount(pot, currency)} ${unitOf(currency)}` : null,
      };
    }
    if (ready >= 2) return { kind: 'ready', label: 'Ready to deal', detail: null };
    if (ready === 1) return { kind: 'waiting', label: 'Waiting for one more', detail: null };
    if (seated > 0) return { kind: 'waiting', label: 'All sitting out', detail: null };
    return { kind: 'open', label: 'Open — no one seated', detail: null };
  }

  /** Hands dealt at this table, straight off the table canister. */
  function handsOf(table) {
    const view = liveOf(table);
    return view ? Number(view.hand_number) : null;
  }

  /** What the last completed hand paid out, summed across its winners. */
  function lastPotOf(table) {
    const winners = liveOf(table)?.last_hand_winners;
    if (!winners?.length) return null;
    return winners.reduce((n, w) => n + Number(w.amount), 0);
  }

  const RANK_TEXT = {
    Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8',
    Nine: '9', Ten: '10', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
  };
  const SUIT_TEXT = { Spades: '♠', Hearts: '♥', Diamonds: '♦', Clubs: '♣' };

  /** The community board as it stands right now. Public state; no hole cards. */
  function boardOf(table) {
    const cards = liveOf(table)?.community_cards;
    if (!cards?.length) return [];
    return cards.map((c) => {
      const suit = tagOf(c.suit);
      return {
        rank: RANK_TEXT[tagOf(c.rank)] ?? '?',
        suit: SUIT_TEXT[suit] ?? '?',
        red: suit === 'Hearts' || suit === 'Diamonds',
      };
    });
  }

  // ---------------------------------------------------------------------------
  // Formatting
  // ---------------------------------------------------------------------------

  const unitOf = (currency) => (currency === 'BTC' ? 'sats' : 'ICP');

  function formatAmount(raw, currency) {
    const num = Number(raw);
    if (currency === 'BTC') {
      if (num >= 100_000_000) return `${(num / 100_000_000).toFixed(2)} BTC`;
      if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`;
      if (num >= 1_000) return `${(num / 1_000).toFixed(0)}K`;
      return `${num}`;
    }
    const icp = num / E8S;
    if (icp >= 1000) return `${(icp / 1000).toFixed(1)}K`;
    if (icp >= 0.01) return icp.toFixed(2);
    if (icp === 0) return '0.00';
    return icp.toFixed(4);
  }

  const formatBlinds = (sb, bb, currency) =>
    `${formatAmount(sb, currency)}/${formatAmount(bb, currency)}`;

  // Spaces around the dash matter: the screenshot harness reads two numbers out
  // of this cell (tools/shots/lib/dom-scrape.mjs) and a bare "2.00-10.00" parses
  // the second one as negative.
  const formatBuyIn = (min, max, currency) =>
    `${formatAmount(min, currency)} – ${formatAmount(max, currency)}`;

  function stakeTier(smallBlind, currency) {
    const sb = Number(smallBlind);
    if (currency === 'BTC') {
      if (sb <= 200) return 'Micro';
      if (sb <= 1000) return 'Low';
      if (sb <= 5000) return 'Mid';
      if (sb <= 20000) return 'High';
      return 'Nosebleed';
    }
    const icp = sb / E8S;
    if (icp <= 0.02) return 'Micro';
    if (icp <= 0.10) return 'Low';
    if (icp <= 0.50) return 'Mid';
    if (icp <= 2) return 'High';
    return 'Nosebleed';
  }

  const shortId = (id) => (id && id.length > 16 ? `${id.slice(0, 5)}…${id.slice(-9)}` : id || '');
  // Short enough to sit on one line in a half-width proof box: a hash that
  // wraps mid-digit reads as corrupted rather than as an elision.
  const shortHash = (h) => (h && h.length > 16 ? `${h.slice(0, 8)}…${h.slice(-6)}` : h || '');

  function tableFormat(table) {
    const max = Number(effectiveConfig(table).max_players);
    return max === 2 ? 'Heads-up' : `${max}-max`;
  }

  async function copyText(text, key) {
    try {
      await navigator.clipboard.writeText(text);
      copiedId = key;
      setTimeout(() => { if (copiedId === key) copiedId = null; }, 1600);
    } catch (e) {
      logger.debug('clipboard unavailable', e);
    }
  }

  // ---------------------------------------------------------------------------
  // Filtering and sorting
  // ---------------------------------------------------------------------------

  const currenciesPresent = $derived([...new Set(tables.map(currencyOf))]);
  const tiersPresent = $derived(
    [...new Set(tables.map((t) => stakeTier(effectiveConfig(t).small_blind, currencyOf(t))))],
  );

  const visibleTables = $derived.by(() => {
    const filtered = tables.filter((t) => {
      if (openSeatsOnly && Number(t.player_count) >= Number(effectiveConfig(t).max_players)) return false;
      if (currencyFilter !== 'all' && currencyOf(t) !== currencyFilter) return false;
      if (stakeFilter !== 'all' && stakeTier(effectiveConfig(t).small_blind, currencyOf(t)) !== stakeFilter) return false;
      return true;
    });

    const dir = sortDirection === 'asc' ? 1 : -1;
    // Sorted into a NEW array: `tables` is owned by the page and must not move.
    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (sortColumn) {
        case 'name': cmp = a.name.localeCompare(b.name); break;
        case 'stakes': cmp = Number(effectiveConfig(a).small_blind) - Number(effectiveConfig(b).small_blind); break;
        case 'buyin': cmp = Number(effectiveConfig(a).min_buy_in) - Number(effectiveConfig(b).min_buy_in); break;
        case 'hands': cmp = (handsOf(a) ?? -1) - (handsOf(b) ?? -1); break;
        case 'seats':
        default: cmp = Number(a.player_count) - Number(b.player_count); break;
      }
      // Table id is the tie-break so the order is total and stable across
      // reloads — a lobby that reshuffles itself on refresh is unusable.
      return (cmp !== 0 ? cmp * dir : Number(a.id) - Number(b.id));
    });
  });

  function sortBy(column) {
    if (sortColumn === column) {
      sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      sortColumn = column;
      sortDirection = (column === 'seats' || column === 'hands') ? 'desc' : 'asc';
    }
  }

  const sortGlyph = (column) =>
    sortColumn !== column ? '' : (sortDirection === 'asc' ? '↑' : '↓');

  const filtersActive = $derived(openSeatsOnly || currencyFilter !== 'all' || stakeFilter !== 'all');

  function clearFilters() {
    openSeatsOnly = false;
    currencyFilter = 'all';
    stakeFilter = 'all';
  }

  // ---------------------------------------------------------------------------
  // Counts
  // ---------------------------------------------------------------------------

  const seatsTaken = $derived(tables.reduce((n, t) => n + Number(t.player_count), 0));
  const seatsTotal = $derived(tables.reduce((n, t) => n + Number(effectiveConfig(t).max_players), 0));
  const driftedTables = $derived(tables.filter((t) => configDrift(t).length > 0).length);
  const handsRunning = $derived(
    tables.filter((t) => isDealing(liveOf(t) ? tagOf(liveOf(t).phase) : null)).length,
  );
  /** Cheapest table by minimum buy-in: where a first-time visitor should start. */
  const cheapestTable = $derived(
    [...tables].sort(
      (a, b) => Number(effectiveConfig(a).min_buy_in) - Number(effectiveConfig(b).min_buy_in),
    )[0] ?? null,
  );

  // ---------------------------------------------------------------------------
  // Selection / preview
  // ---------------------------------------------------------------------------

  const selectedTable = $derived(
    visibleTables.find((t) => String(t.id) === String(selectedId)) ?? visibleTables[0] ?? null,
  );

  function select(table) {
    selectedId = String(table.id);
  }

  /**
   * Seat coordinates on a ~2:1 stadium, clockwise from the bottom seat.
   * docs/DESIGN-BAR.md §1.2: every leading client's playing surface is a wide
   * ellipse between 1.72:1 and 2.63:1, median 2.13, so the lobby's preview of a
   * table should be one too — and an empty seat is a pod with a call to action,
   * never a gap (WPT Global: "Click on an empty seat to sit at the table").
   */
  function seatPositions(count) {
    return Array.from({ length: count }, (_, i) => {
      const angle = Math.PI / 2 - (i * 2 * Math.PI) / count;
      return { left: 50 + 50 * Math.cos(angle), top: 50 + 50 * Math.sin(angle) };
    });
  }

  const initialOf = (player, index) =>
    (player?.name ? player.name.trim()[0] : String(index + 1)).toUpperCase();

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  function openTable(table) {
    if (!table || !canisterIdOf(table)) return;
    onJoinTable(table);
  }

  function refreshAll() {
    onRefresh?.();
    readLiveViews(tables).catch((e) => logger.debug('lobby live refresh failed', e));
  }

  /**
   * The same call the header's Connect Wallet makes (`auth.login()` →
   * AuthClient against the Internet Identity provider in ic-config.js). One
   * auth code path, two entry points, so this button cannot drift from it.
   */
  async function signIn() {
    if (signingIn) return;
    signingIn = true;
    signInError = null;
    try {
      await auth.login();
    } catch (e) {
      logger.error('lobby sign-in failed', e);
      signInError = 'Could not reach the identity provider. Try the wallet button in the header.';
    } finally {
      signingIn = false;
    }
  }

  // ---------------------------------------------------------------------------
  // Wiring
  // ---------------------------------------------------------------------------

  onMount(() => {
    try {
      const saved = localStorage.getItem('lobby_density');
      if (saved === 'compact' || saved === 'comfortable') density = saved;
    } catch { /* private mode */ }
    // WalletButton owns auth.init(); this only listens.
    return auth.subscribe((s) => { authState = s; });
  });

  $effect(() => {
    const list = tables;
    readLiveViews(list).catch((e) => logger.debug('lobby live read failed', e));
    const timer = setInterval(() => {
      readLiveViews(list).catch((e) => logger.debug('lobby live poll failed', e));
    }, LIVE_POLL_MS);
    return () => clearInterval(timer);
  });
</script>

<div class="lobby" data-lobby-live={liveLoaded ? 'ready' : 'pending'}>

  {#if !signedIn}
    <!-- The signed-out first impression. Three claims, all of them checkable:
         no rake is a property of the payout code, the commitment is published
         per hand, and the whole stack is canisters. No rake leads, because it
         is the one thing a rake-funded client structurally cannot copy. -->
    <section class="intro">
      <div class="intro-copy">
        <p class="eyebrow">No Limit Hold'em · on the Internet Computer</p>
        <h1>Poker you can check.</h1>
        <p class="intro-lead">
          The deck is committed before the deal and revealed after it, so anyone can
          re-derive the shuffle from the hand history. Every table is a smart contract
          with an address you can query while signed out.
        </p>
        <div class="intro-actions">
          <button class="btn primary" onclick={signIn} disabled={signingIn}>
            {signingIn ? 'Connecting…' : 'Connect wallet'}
          </button>
          <button class="btn ghost" onclick={() => showHow = true}>How it works</button>
        </div>
        {#if signInError}
          <p class="intro-error">{signInError}</p>
        {/if}
        <p class="intro-note">
          No wallet needed to look around: every table, seat map, live board and deck
          commitment below is readable while signed out.
        </p>
      </div>

      <div class="intro-proof">
        <div class="headline-claim">
          <span class="headline-figure">0%</span>
          <span class="headline-title">Rake. Every pot, every stake.</span>
          <span class="headline-body">
            There is no house cut anywhere in the payout code. Whatever goes into a pot
            is paid back out to players, down to the last e8.
          </span>
        </div>
        <ul class="claims">
          <li>
            <span class="claim-figure">SHA-256</span>
            <span class="claim-title">Committed deck</span>
            <span class="claim-body">The shuffle-seed hash is published before a card moves, the seed itself after the hand.</span>
          </li>
          <li>
            <span class="claim-figure">On-chain</span>
            <span class="claim-title">No server</span>
            <span class="claim-body">This page, the lobby and every table are canisters. Each has an address you can query.</span>
          </li>
        </ul>
      </div>
    </section>
  {:else}
    <div class="intro-slim">
      <span class="slim-chip strong">0% rake</span>
      <span class="slim-chip">Committed deck</span>
      <span class="slim-chip">On-chain settlement</span>
      <button class="link-btn" onclick={() => showHow = true}>How it works</button>
    </div>
  {/if}

  <header class="bar">
    <div class="bar-title">
      <h2>Cash games</h2>
      <p class="bar-sub">
        <strong>{tables.length}</strong>
        {tables.length === 1 ? 'table' : 'tables'}
        <span class="sep">·</span>
        <strong>{seatsTaken}</strong> of {seatsTotal} seats taken
        {#if handsRunning > 0}
          <span class="sep">·</span>
          <strong class="hot">{handsRunning}</strong> {handsRunning === 1 ? 'hand' : 'hands'} in play
        {/if}
        <span class="sep">·</span>
        <span class="norake">no rake on any of them</span>
      </p>
    </div>

    <div class="bar-actions">
      <div class="seg" role="group" aria-label="Row density">
        <button
          class:on={density === 'comfortable'}
          onclick={() => setDensity('comfortable')}
          title="Comfortable rows"
          aria-label="Comfortable rows"
          aria-pressed={density === 'comfortable'}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="14" width="18" height="7" rx="1.5"/>
          </svg>
        </button>
        <button
          class:on={density === 'compact'}
          onclick={() => setDensity('compact')}
          title="Compact rows"
          aria-label="Compact rows"
          aria-pressed={density === 'compact'}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/>
          </svg>
        </button>
      </div>
      <button class="btn ghost icon" onclick={refreshAll} aria-label="Refresh tables">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M23 4v6h-6M1 20v-6h6"/>
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
        </svg>
        Refresh
      </button>
    </div>
  </header>

  <div class="filters">
    <button class="pill" class:on={!openSeatsOnly} onclick={() => openSeatsOnly = false}>All tables</button>
    <button class="pill" class:on={openSeatsOnly} onclick={() => openSeatsOnly = true}>Open seats</button>

    {#if currenciesPresent.length > 1}
      <span class="filter-gap"></span>
      <button class="pill" class:on={currencyFilter === 'all'} onclick={() => currencyFilter = 'all'}>Any currency</button>
      {#each currenciesPresent as code}
        <button class="pill" class:on={currencyFilter === code} onclick={() => currencyFilter = code}>{code}</button>
      {/each}
    {/if}

    {#if tiersPresent.length > 1}
      <span class="filter-gap"></span>
      <button class="pill" class:on={stakeFilter === 'all'} onclick={() => stakeFilter = 'all'}>Any stake</button>
      {#each tiersPresent as tier}
        <button class="pill" class:on={stakeFilter === tier} onclick={() => stakeFilter = tier}>{tier}</button>
      {/each}
    {/if}

    {#if filtersActive}
      <button class="pill clear" onclick={clearFilters}>Clear</button>
    {/if}
  </div>

  {#if liveLoaded && driftedTables > 0}
    <!-- One statement of the problem, at the top, instead of the same warning
         shouted in every money cell. The figures below are always the table
         contract's; it is the NAME the lobby canister registered that is stale. -->
    <p class="notice warn">
      <strong>
        {driftedTables} of {tables.length}
        {driftedTables === 1 ? 'table is' : 'tables are'} registered in the lobby canister
        with figures its own contract does not enforce.
      </strong>
      Every number in this list is read from the <em>table</em> contract, because that is
      what it will charge you. Only the name comes from the lobby canister, and for those
      tables the name still quotes the old stakes.
    </p>
  {/if}

  {#if tables.length > 0 && liveLoaded && seatsTaken === 0}
    <div class="notice go">
      <div>
        <strong>Nobody is seated yet.</strong>
        Take a seat and the hand starts the moment a second player joins you. Sitting down
        costs nothing until you buy in, and the pot is never raked.
      </div>
      {#if cheapestTable}
        <button class="btn primary sm" onclick={() => openTable(cheapestTable)}>
          Open {cheapestTable.name}
        </button>
      {/if}
    </div>
  {/if}

  <div class="board">
    <div class="list-pane">
      {#if tables.length === 0}
        <div class="empty">
          <h3>The lobby canister is reporting no tables.</h3>
          <p>
            Nothing is wrong with your wallet or your connection — the lobby simply has no
            table registered against it right now. You can confirm that yourself: query
            <code class="mono inline">get_tables()</code> on the lobby canister.
          </p>
          <button class="mono copy narrow" onclick={() => copyText(lobbyCanisterId, 'lobby')}>
            {shortId(lobbyCanisterId)}
            <span class="copy-state">{copiedId === 'lobby' ? 'copied' : 'copy'}</span>
          </button>
          <button class="btn ghost" onclick={refreshAll}>Try again</button>
        </div>
      {:else if !liveLoaded}
        <!-- Rows are held back for exactly one round of `get_table_view()`.
             The lobby canister's cached copy of a table's config can be wrong
             (it is, on this deployment, for two tables), so rendering it even
             for a frame would put stakes on screen that the table will not
             charge. A skeleton is the honest thing to show instead. -->
        <div class="skeleton" aria-live="polite">
          <p>Reading each table's own contract…</p>
          {#each tables as t (t.id)}
            <span class="skeleton-row"></span>
          {/each}
        </div>
      {:else if visibleTables.length === 0}
        <div class="empty">
          <h3>No table matches this filter.</h3>
          <p>{tables.length} {tables.length === 1 ? 'table is' : 'tables are'} open; none of them fits the filters above.</p>
          <button class="btn ghost" onclick={clearFilters}>Clear filters</button>
        </div>
      {:else}
        <table class="tables-list" class:compact={density === 'compact'}>
          <thead>
            <tr>
              <th class="c-table">
                <button class="sort" class:on={sortColumn === 'name'} onclick={() => sortBy('name')}>
                  Table <span class="glyph">{sortGlyph('name')}</span>
                </button>
              </th>
              <th class="c-stakes">
                <button class="sort" class:on={sortColumn === 'stakes'} onclick={() => sortBy('stakes')}>
                  Stakes <span class="glyph">{sortGlyph('stakes')}</span>
                </button>
              </th>
              <th class="c-buyin">
                <button class="sort" class:on={sortColumn === 'buyin'} onclick={() => sortBy('buyin')}>
                  Buy-in <span class="glyph">{sortGlyph('buyin')}</span>
                </button>
              </th>
              <th class="c-seats">
                <button class="sort" class:on={sortColumn === 'seats'} onclick={() => sortBy('seats')}>
                  Seats <span class="glyph">{sortGlyph('seats')}</span>
                </button>
              </th>
              <th class="c-hands">
                <button class="sort" class:on={sortColumn === 'hands'} onclick={() => sortBy('hands')}>
                  Hands <span class="glyph">{sortGlyph('hands')}</span>
                </button>
              </th>
              <th class="c-now"><span class="head">Now</span></th>
              <th class="c-go"><span class="sr-only">Sit down</span></th>
            </tr>
          </thead>
          <tbody>
            {#each visibleTables as table (table.id)}
              {@const currency = currencyOf(table)}
              {@const cfg = effectiveConfig(table)}
              {@const drift = configDrift(table)}
              {@const seats = seatsOf(table)}
              {@const filled = seats.filter(Boolean).length}
              {@const idle = seats.filter((s) => s && !s.active).length}
              {@const isFull = filled >= Number(cfg.max_players)}
              {@const now = nowOf(table)}
              {@const hands = handsOf(table)}
              {@const isSelected = selectedTable && String(selectedTable.id) === String(table.id)}
              <tr
                class:selected={isSelected}
                class:btc={currency === 'BTC'}
                class:full={isFull}
                tabindex="0"
                aria-label="Open {table.name}"
                onclick={() => openTable(table)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openTable(table); }
                }}
                onmouseenter={() => select(table)}
                onfocus={() => select(table)}
              >
                <td class="c-table">
                  <!-- Rendered verbatim: this string is the lobby canister's
                       registered name and it is the key the screenshot harness
                       matches a row on. When it is stale, say so next to the
                       money it misquotes rather than editing it. -->
                  <span class="table-name">{table.name}</span>
                  <span class="tags">
                    <span class="tag currency currency-tag" class:btc={currency === 'BTC'}>
                      {#if currency === 'BTC'}
                        <span class="btc-mark" aria-hidden="true">₿</span>
                      {:else}
                        <IcpLogo size={11} />
                      {/if}
                      {currency}
                    </span>
                    <span class="tag">NLHE</span>
                    <span class="tag">{tableFormat(table)}</span>
                  </span>
                </td>

                <td class="c-stakes">
                  <span class="num stakes-value">{formatBlinds(cfg.small_blind, cfg.big_blind, currency)}</span>
                  <span class="unit">{unitOf(currency)}</span>
                  {#if drift.length > 0}
                    <span
                      class="drift-mark"
                      title="This table's own contract enforces the figures shown. The lobby canister's registration disagrees on: {drift.join(', ')}. Its name still carries the lobby's numbers."
                    >⚠ record differs</span>
                  {/if}
                </td>

                <td class="c-buyin">
                  <span class="num muted buyin-value">{formatBuyIn(cfg.min_buy_in, cfg.max_buy_in, currency)}</span>
                </td>

                <td class="c-seats">
                  <span class="meter" class:btc={currency === 'BTC'} aria-hidden="true">
                    {#each seats as seat}
                      <span class="pip" class:taken={Boolean(seat)} class:idle={Boolean(seat) && !seat.active}></span>
                    {/each}
                  </span>
                  <span class="seat-count players-text"><strong>{filled}</strong>/{cfg.max_players}</span>
                  {#if idle > 0}
                    <span class="seat-note">{idle} sitting out</span>
                  {/if}
                </td>

                <td class="c-hands">
                  <span class="hands-value">{hands === null ? '—' : hands}</span>
                </td>

                <td class="c-now">
                  <span class="now {now.kind}">
                    <span class="dot"></span>
                    {now.label}
                  </span>
                  {#if now.detail}<span class="now-detail">pot {now.detail}</span>{/if}
                </td>

                <td class="c-go">
                  <!-- A full table is not a dead end: its state is public, so
                       the honest label is the one PokerStars uses next to Play
                       Now — you can watch it. -->
                  <span class="go" class:btc={currency === 'BTC'}>
                    {isFull ? 'Watch' : signedIn ? 'Sit' : 'View'}
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                      <path d="M5 12h14M12 5l7 7-7 7"/>
                    </svg>
                  </span>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        <!-- The columns a rake-funded client shows that this one does not, and
             why. Stating the gap is better than estimating it: none of these is
             recorded on-chain, so no client could compute them from this engine. -->
        <p class="list-foot">
          Every column above is read live from a canister: the list from
          <button class="linkish mono" onclick={() => copyText(lobbyCanisterId, 'lobby')}>{shortId(lobbyCanisterId)}</button>,
          each row from its own table contract, re-read every {LIVE_POLL_MS / 1000} seconds.
          Average pot, players-per-flop, hands-per-hour and waiting lists are
          <strong>not recorded on-chain</strong>, so they are absent here rather than estimated.
        </p>
      {/if}
    </div>

    {#if selectedTable && liveLoaded}
      {@const currency = currencyOf(selectedTable)}
      {@const cfg = effectiveConfig(selectedTable)}
      {@const drift = configDrift(selectedTable)}
      {@const view = liveOf(selectedTable)}
      {@const seats = seatsOf(selectedTable)}
      {@const positions = seatPositions(seats.length)}
      {@const occupied = seats.filter(Boolean)}
      {@const board = boardOf(selectedTable)}
      {@const phase = view ? tagOf(view.phase) : null}
      {@const dealing = isDealing(phase)}
      {@const lastPot = lastPotOf(selectedTable)}
      {@const commitment = opt(view?.shuffle_proof)?.seed_hash ?? null}
      {@const contractId = canisterIdOf(selectedTable)}
      <aside class="preview" aria-label="Table preview">
        <div class="preview-head">
          <div class="preview-heading">
            <h3>{selectedTable.name}</h3>
            <p class="preview-sub">
              {tableFormat(selectedTable)} · No Limit Hold'em ·
              {stakeTier(cfg.small_blind, currency)} stakes
            </p>
            {#if drift.length > 0}
              <p class="preview-drift">
                Lobby record disagrees on {drift.join(', ')}. Shown: the table's own.
              </p>
            {/if}
          </div>
          <span class="tag currency" class:btc={currency === 'BTC'}>
            {#if currency === 'BTC'}
              <span class="btc-mark" aria-hidden="true">₿</span>
            {:else}
              <IcpLogo size={11} />
            {/if}
            {currency}
          </span>
        </div>

        <!-- A ~2:1 ellipse, because that is the shape of the table you are about
             to sit at (docs/DESIGN-BAR.md §1.2), and every empty seat is an
             explicit invitation rather than a gap. -->
        <div class="map">
          <div class="mini-felt" class:btc={currency === 'BTC'}>
            {#if dealing}
              <span class="felt-live">
                {#if Number(view.pot) > 0}
                  <span class="felt-pot">{formatAmount(view.pot, currency)}</span>
                {/if}
                <span class="felt-phase">{PHASE_LABEL[phase] ?? phase}</span>
              </span>
              {#if board.length > 0}
                <span class="felt-board" aria-label="Community cards">
                  {#each board as card}
                    <span class="mini-card" class:red={card.red}>
                      <span class="mc-rank">{card.rank}</span><span class="mc-suit">{card.suit}</span>
                    </span>
                  {/each}
                </span>
              {/if}
            {:else}
              <span class="felt-mark">No Limit Hold'em</span>
            {/if}
            {#each seats as seat, i}
              <button
                class="pod"
                class:taken={Boolean(seat)}
                class:idle={Boolean(seat) && !seat.active}
                style:left="{positions[i].left}%"
                style:top="{positions[i].top}%"
                onclick={(e) => { e.stopPropagation(); openTable(selectedTable); }}
                title={seat
                  ? `Seat ${i + 1}: ${seat.name ?? 'seated player'}${seat.chips != null ? ` — ${formatAmount(seat.chips, currency)} ${unitOf(currency)}` : ''}${seat.active ? '' : ' (sitting out)'}`
                  : `Seat ${i + 1}: open — opens this table`}
              >{seat ? initialOf(seat, i) : '+'}</button>
            {/each}
          </div>
        </div>

        {#if occupied.length > 0}
          <p class="seated">
            {#each occupied as p, i}
              {#if i > 0}<span class="seated-sep">·</span>{/if}
              <span class="seated-one" class:idle={!p.active}>
                <span class="seated-name">{p.name ?? `Seat ${p.seat + 1}`}</span>
                {#if p.chips != null}<span class="seated-stack">{formatAmount(p.chips, currency)}</span>{/if}
              </span>
            {/each}
          </p>
        {:else}
          <p class="map-note">Every seat is open. Pick one to open this table.</p>
        {/if}

        <dl class="facts">
          <div><dt>Blinds</dt><dd>{formatBlinds(cfg.small_blind, cfg.big_blind, currency)}</dd></div>
          <div><dt>Buy-in</dt><dd>{formatBuyIn(cfg.min_buy_in, cfg.max_buy_in, currency)}</dd></div>
          <div><dt>Clock</dt><dd>{cfg.action_timeout_secs}s + {cfg.time_bank_secs}s</dd></div>
          <div><dt>Ante</dt><dd>{Number(cfg.ante) === 0 ? 'None' : formatAmount(cfg.ante, currency)}</dd></div>
          <div><dt>Hands dealt</dt><dd>{view ? Number(view.hand_number) : '—'}</dd></div>
          <div><dt>Last pot</dt><dd>{lastPot === null ? '—' : formatAmount(lastPot, currency)}</dd></div>
        </dl>

        <p class="rake-line">
          <strong>0% rake.</strong> {lastPot === null
            ? 'Whatever this table collects is paid straight back out.'
            : `All ${formatAmount(lastPot, currency)} ${unitOf(currency)} of the last pot went to the winner.`}
        </p>

        <div class="proofs" class:two-up={Boolean(commitment) && Boolean(contractId)}>
          <div class="proof">
            <span class="proof-label">Deck commitment</span>
            {#if commitment}
              <button class="mono copy" onclick={(e) => { e.stopPropagation(); copyText(commitment, commitment); }}>
                {shortHash(commitment)}
                <span class="copy-state">{copiedId === commitment ? 'copied' : 'copy'}</span>
              </button>
            {:else}
              <p class="proof-idle">No hand in progress. The commitment appears the moment the deck is shuffled.</p>
            {/if}
          </div>

          {#if contractId}
            <div class="proof">
              <span class="proof-label">Table contract</span>
              <button class="mono copy" onclick={(e) => { e.stopPropagation(); copyText(contractId, contractId); }}>
                {shortId(contractId)}
                <span class="copy-state">{copiedId === contractId ? 'copied' : 'copy'}</span>
              </button>
            </div>
          {/if}
        </div>

        <div class="preview-cta">
          <button class="btn primary wide" onclick={() => openTable(selectedTable)}>
            {signedIn ? 'Sit down here' : 'Open this table'}
          </button>
          <p class="cta-note">
            {signedIn
              ? 'Opening a table costs nothing. You pick a seat and a buy-in there.'
              : 'Open it and watch for free. Connect a wallet when you want a seat.'}
          </p>
        </div>
      </aside>
    {/if}
  </div>

  <p class="foot">
    Texas Hold'em No Limit. <strong>No rake is taken from any pot on any table.</strong>
    Every deal is verifiable from the hand history.
  </p>
</div>

{#if showHow}
  <HowItWorks onClose={() => { showHow = false; }} />
{/if}

<style>
  .lobby {
    max-width: 1320px;
    margin: 0 auto;
    padding: 22px 20px 32px;
    color: #e6e8ec;
  }

  .sr-only {
    position: absolute;
    width: 1px; height: 1px;
    padding: 0; margin: -1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  /* ---------------------------------------------------------------- buttons */

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: 9px;
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    padding: 10px 18px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s, border-color 0.15s, color 0.15s, transform 0.15s;
  }

  .btn.primary {
    background: linear-gradient(135deg, #00d4aa, #00b894);
    color: #04120f;
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.18) inset;
  }

  .btn.primary:hover:not(:disabled) { transform: translateY(-1px); }

  .btn.ghost {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.12);
    color: #b9bfc9;
  }

  .btn.ghost:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .btn.icon { padding: 9px 14px; font-weight: 500; }
  .btn.sm { padding: 8px 14px; font-size: 12.5px; white-space: nowrap; }
  .btn.wide { width: 100%; }
  .btn:disabled { opacity: 0.55; cursor: progress; }

  .link-btn {
    background: none;
    border: none;
    color: #00d4aa;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .linkish {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: #8b93a0;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .linkish:hover { color: #00d4aa; }

  /* ------------------------------------------------------ signed-out intro */

  .intro {
    display: grid;
    grid-template-columns: minmax(0, 1.12fr) minmax(0, 1fr);
    gap: 30px;
    align-items: center;
    padding: 20px 24px 21px;
    margin-bottom: 18px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    background:
      radial-gradient(110% 150% at 0% 0%, rgba(0, 212, 170, 0.11), transparent 56%),
      radial-gradient(90% 130% at 100% 100%, rgba(99, 102, 241, 0.07), transparent 60%),
      rgba(255, 255, 255, 0.022);
  }

  .eyebrow {
    margin: 0 0 8px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: #5f6773;
  }

  .intro h1 {
    margin: 0 0 8px;
    font-size: 32px;
    line-height: 1.08;
    font-weight: 700;
    letter-spacing: -0.025em;
    color: #fff;
  }

  .intro-lead {
    margin: 0 0 15px;
    font-size: 13.5px;
    line-height: 1.55;
    color: #a8aebb;
    max-width: 52ch;
  }

  .intro-actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .intro-actions .btn { padding: 9px 17px; }

  .intro-error {
    margin: 10px 0 0;
    font-size: 12px;
    color: #f8b4b4;
  }

  .intro-note {
    margin: 11px 0 0;
    font-size: 11.5px;
    line-height: 1.5;
    color: #6f7683;
    max-width: 52ch;
  }

  .intro-proof { display: grid; gap: 8px; }

  /* No rake is the one claim a rake-funded operator cannot copy, so it is the
     one that gets the display figure. */
  .headline-claim {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: 15px;
    align-items: center;
    padding: 12px 15px;
    border-radius: 12px;
    background:
      linear-gradient(100deg, rgba(0, 212, 170, 0.16), rgba(0, 212, 170, 0.05) 70%),
      rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(0, 212, 170, 0.3);
  }

  .headline-figure {
    grid-row: 1 / span 2;
    font-size: 38px;
    line-height: 1;
    font-weight: 800;
    letter-spacing: -0.04em;
    color: #00d4aa;
  }

  .headline-title {
    font-size: 14px;
    font-weight: 700;
    color: #fff;
    letter-spacing: -0.01em;
  }

  .headline-body {
    margin-top: 3px;
    font-size: 11.5px;
    line-height: 1.45;
    color: #9199a6;
  }

  .claims {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }

  .claims li {
    display: grid;
    grid-template-columns: 74px minmax(0, 1fr);
    column-gap: 13px;
    align-items: center;
    padding: 8px 13px;
    border-radius: 11px;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.055);
  }

  .claim-figure {
    grid-row: 1 / span 2;
    font-size: 14px;
    font-weight: 700;
    color: #8be8d3;
    letter-spacing: -0.01em;
  }

  .claim-title {
    font-size: 12.5px;
    font-weight: 600;
    color: #fff;
  }

  .claim-body {
    font-size: 11.5px;
    line-height: 1.45;
    color: #838a97;
  }

  .intro-slim {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 18px;
    padding-bottom: 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .slim-chip {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #00d4aa;
    background: rgba(0, 212, 170, 0.10);
    border: 1px solid rgba(0, 212, 170, 0.22);
    border-radius: 999px;
    padding: 4px 10px;
  }

  .slim-chip.strong {
    background: rgba(0, 212, 170, 0.2);
    border-color: rgba(0, 212, 170, 0.45);
    color: #7bf0d8;
  }

  /* ----------------------------------------------------------------- header */

  .bar {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    margin-bottom: 11px;
  }

  .bar h2 {
    margin: 0 0 5px;
    font-size: 21px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: #fff;
  }

  .bar-sub {
    margin: 0;
    font-size: 13px;
    color: #7d8492;
  }

  .bar-sub strong { color: #dfe3ea; font-weight: 600; }
  .bar-sub .hot { color: #00d4aa; }
  .bar-sub .sep { color: #3a3f49; margin: 0 5px; }

  .norake {
    color: #00d4aa;
    font-weight: 600;
  }

  .bar-actions { display: flex; align-items: center; gap: 8px; }

  /* PokerStars ships row density as a strip of small icon buttons rather than
     words (docs/DESIGN-BAR.md); at three tables the words were costing more
     width than the feature is worth. */
  .seg {
    display: flex;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 9px;
    padding: 2px;
  }

  .seg button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: #6f7683;
    font: inherit;
    padding: 6px 9px;
    border-radius: 7px;
    cursor: pointer;
  }

  .seg button:hover { color: #b9bfc9; }
  .seg button.on { background: rgba(255, 255, 255, 0.09); color: #fff; }

  /* ---------------------------------------------------------------- filters */

  .filters {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
    margin-bottom: 11px;
  }

  .filter-gap {
    width: 1px;
    height: 18px;
    background: rgba(255, 255, 255, 0.09);
    margin: 0 6px;
  }

  .pill {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.07);
    color: #7d8492;
    font: inherit;
    font-size: 12.5px;
    padding: 7px 14px;
    border-radius: 999px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .pill:hover { color: #cfd4dd; }

  .pill.on {
    background: rgba(0, 212, 170, 0.13);
    border-color: rgba(0, 212, 170, 0.32);
    color: #00d4aa;
  }

  .pill.clear { color: #9aa2ae; text-decoration: underline; text-underline-offset: 3px; border-color: transparent; }

  /* --------------------------------------------------------------- notices */

  .notice {
    margin: 0 0 12px;
    padding: 11px 15px;
    border-radius: 11px;
    font-size: 12.5px;
    line-height: 1.55;
    color: #9aa2ae;
  }

  .notice.warn {
    border: 1px solid rgba(240, 180, 41, 0.24);
    background: rgba(240, 180, 41, 0.07);
  }

  .notice.warn strong { color: #f0b429; }
  .notice.warn em { color: #dfe3ea; font-style: normal; font-weight: 600; }

  .notice.go {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border: 1px solid rgba(0, 212, 170, 0.2);
    background: rgba(0, 212, 170, 0.055);
  }

  .notice.go strong { color: #00d4aa; }

  /* ------------------------------------------------------------- the board */

  .board {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 352px;
    gap: 18px;
    align-items: stretch;
  }

  /* A flex column so the provenance footer sits at the BOTTOM of the pane.
     The pane stretches to the preview's height (align-items: stretch above), so
     without this a three-row lobby left ~600 px of unexplained void beside a
     tall preview — the single worst thing about the previous layout. */
  .list-pane {
    display: flex;
    flex-direction: column;
    background: rgba(255, 255, 255, 0.018);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 14px;
    overflow: hidden;
  }

  .tables-list {
    width: 100%;
    border-collapse: collapse;
  }

  .tables-list thead th {
    text-align: left;
    padding: 0;
    background: rgba(0, 0, 0, 0.3);
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }

  .sort, .head {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    background: none;
    border: none;
    font: inherit;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: #666d79;
    padding: 13px 16px;
  }

  .sort { cursor: pointer; }
  .sort:hover { color: #9aa2ae; }
  .sort.on { color: #00d4aa; }
  .glyph { font-size: 9px; }

  .tables-list tbody tr {
    cursor: pointer;
    border-bottom: 1px solid rgba(255, 255, 255, 0.045);
    transition: background 0.12s;
  }

  .tables-list tbody tr:last-child { border-bottom: none; }

  .tables-list tbody tr:hover,
  .tables-list tbody tr.selected {
    background: rgba(0, 212, 170, 0.055);
  }

  .tables-list tbody tr.btc:hover,
  .tables-list tbody tr.btc.selected {
    background: rgba(247, 147, 26, 0.06);
  }

  .tables-list tbody tr.selected { box-shadow: inset 3px 0 0 #00d4aa; }
  .tables-list tbody tr.btc.selected { box-shadow: inset 3px 0 0 #f7931a; }
  .tables-list tbody tr:focus-visible { outline: 2px solid #00d4aa; outline-offset: -2px; }

  .tables-list td { padding: 15px 16px; vertical-align: middle; }
  .tables-list.compact td { padding: 9px 16px; }
  .tables-list.compact .tags { display: none; }

  .c-table { width: 30%; }
  .c-stakes { width: 14%; }
  .c-buyin { width: 17%; white-space: nowrap; }
  .c-seats { width: 17%; }
  .c-hands { width: 7%; }
  .c-now { width: 15%; white-space: nowrap; }
  .c-go { width: 6%; text-align: right; }

  /* One line, always. A wrapped table name makes rows different heights and
     turns a scannable list into a ragged one; `table-layout: auto` widens the
     column to honour the nowrap instead. The text itself is never altered —
     it is the lobby canister's registered name. */
  .table-name {
    display: block;
    color: #fff;
    font-weight: 600;
    font-size: 14.5px;
    letter-spacing: -0.005em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Quiet, and in the money column, because that is the cell the stale record
     misquotes. The banner above the list carries the full explanation. */
  .drift-mark {
    display: block;
    margin-top: 3px;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: #f0b429;
    cursor: help;
    white-space: nowrap;
  }

  .tags { display: flex; gap: 4px; margin-top: 6px; flex-wrap: wrap; }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.055);
    color: #868d99;
    white-space: nowrap;
  }

  /* The preview's currency chip has room to breathe; the row's does not. */
  .preview .tag { font-size: 10px; padding: 2px 7px; }

  .tag.currency { background: rgba(99, 102, 241, 0.16); color: #a5b4fc; }
  .tag.currency.btc { background: rgba(247, 147, 26, 0.16); color: #f7931a; }
  .btc-mark { font-size: 12px; line-height: 1; }

  .tag.stale {
    background: rgba(240, 180, 41, 0.14);
    color: #f0b429;
    cursor: help;
  }

  .num {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    font-size: 14px;
    color: #f0b429;
  }

  .num.muted { color: #aeb4be; font-weight: 500; font-size: 13px; }
  .unit { font-size: 10.5px; color: #5d646f; margin-left: 4px; }

  .meter { display: inline-flex; gap: 3px; vertical-align: middle; }

  .meter { gap: 2.5px; }

  .pip {
    width: 6px;
    height: 6px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.11);
  }

  .pip.taken { background: #00d4aa; }
  .meter.btc .pip.taken { background: #f7931a; }
  /* Seated but not dealt in: the seat is gone, the player is not. */
  .pip.taken.idle { background: transparent; box-shadow: inset 0 0 0 1.5px #00d4aa; }
  .meter.btc .pip.taken.idle { box-shadow: inset 0 0 0 1.5px #f7931a; }

  .seat-count {
    margin-left: 9px;
    font-size: 12px;
    color: #767d89;
    font-variant-numeric: tabular-nums;
  }

  .seat-count strong { color: #fff; font-size: 14px; }

  .seat-note {
    display: block;
    margin-top: 3px;
    font-size: 10.5px;
    color: #5d646f;
  }

  .hands-value {
    font-variant-numeric: tabular-nums;
    font-size: 13px;
    color: #aeb4be;
  }

  .now {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: #767d89;
  }

  .now .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .now.live { color: #00d4aa; }
  .now.ready { color: #f0b429; }
  .now.waiting { color: #9aa2ae; }
  .now.open { color: #5d646f; }

  .now-detail {
    display: block;
    margin-top: 3px;
    font-size: 11px;
    color: #5d646f;
    font-variant-numeric: tabular-nums;
  }

  .go {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    font-weight: 700;
    color: #00d4aa;
    white-space: nowrap;
  }

  .go.btc { color: #f7931a; }
  /* Full: still reachable, just not the primary act. */
  tr.full .go { color: #8b93a0; font-weight: 600; }

  .list-foot {
    margin: auto 0 0;
    padding: 13px 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.055);
    background: rgba(0, 0, 0, 0.18);
    font-size: 11px;
    line-height: 1.6;
    color: #6b7280;
  }

  .list-foot strong { color: #9aa2ae; font-weight: 600; }

  /* ------------------------------------------------------------ empty state */

  .empty {
    padding: 56px 32px;
    text-align: center;
  }

  .empty h3 {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 600;
    color: #dfe3ea;
  }

  .empty p {
    margin: 0 auto 14px;
    max-width: 46ch;
    font-size: 13px;
    line-height: 1.6;
    color: #767d89;
  }

  .skeleton { padding: 22px 18px 20px; }

  .skeleton p {
    margin: 0 0 14px;
    font-size: 12px;
    color: #666d79;
  }

  .skeleton-row {
    display: block;
    height: 46px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.032);
    margin-bottom: 8px;
  }

  /* ---------------------------------------------------------------- preview */

  .preview {
    align-self: start;
    position: sticky;
    top: 16px;
    padding: 15px 16px 16px;
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.018);
  }

  .preview-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .preview-heading { min-width: 0; }

  .preview-head h3 {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 600;
    color: #fff;
  }

  .preview-sub {
    margin: 0;
    font-size: 11.5px;
    color: #767d89;
  }

  .map { padding: 14px 18px 10px; }

  .mini-felt {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 1;
    border-radius: 50%;
    background: radial-gradient(120% 150% at 50% 0%, #12564a, #0b3b34 70%);
    border: 1px solid rgba(0, 212, 170, 0.18);
    box-shadow: inset 0 0 34px rgba(0, 0, 0, 0.45);
  }

  .mini-felt.btc {
    background: radial-gradient(120% 150% at 50% 0%, #4a3212, #2c1e0b 70%);
    border-color: rgba(247, 147, 26, 0.2);
  }

  .felt-mark {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 8.5px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.13);
    pointer-events: none;
  }

  .felt-live {
    position: absolute;
    left: 0; right: 0; top: 22%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    pointer-events: none;
  }

  .felt-pot {
    font-size: 15px;
    font-weight: 700;
    color: #fff;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .felt-phase {
    font-size: 8px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.42);
  }

  /* The live community board. Public canister state, so a signed-out visitor
     sees exactly what a seated player sees. */
  .felt-board {
    position: absolute;
    left: 0; right: 0; top: 56%;
    display: flex;
    justify-content: center;
    gap: 3px;
    pointer-events: none;
  }

  .mini-card {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 1px;
    min-width: 20px;
    height: 22px;
    padding: 0 3px;
    border-radius: 3px;
    background: #f3f4f6;
    color: #14171d;
    font-size: 9.5px;
    font-weight: 700;
    line-height: 1;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }

  .mini-card.red { color: #c81e3c; }
  .mc-suit { font-size: 9px; }

  .pod {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font: inherit;
    font-size: 11px;
    font-weight: 700;
    background: #14171d;
    border: 1px dashed rgba(255, 255, 255, 0.28);
    color: #79808c;
    cursor: pointer;
    transition: border-color 0.15s, color 0.15s, background 0.15s;
  }

  .pod:hover { border-color: #00d4aa; color: #7bf0d8; }
  .pod:focus-visible { outline: 2px solid #00d4aa; outline-offset: 2px; }

  .pod.taken {
    border: 1px solid #00d4aa;
    background: #0c2f2a;
    color: #7bf0d8;
  }

  .pod.taken.idle { border-style: dotted; opacity: 0.6; }

  .mini-felt.btc .pod.taken {
    border-color: #f7931a;
    background: #33230c;
    color: #ffc477;
  }

  .seated {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 6px;
    margin: 0 0 11px;
    font-size: 11.5px;
    color: #9aa2ae;
  }

  .seated-sep { color: #3a3f49; }
  .seated-one { display: inline-flex; gap: 5px; align-items: baseline; }
  .seated-one.idle { opacity: 0.55; }
  .seated-name { color: #9aa2ae; }
  .seated-stack { color: #dfe3ea; font-variant-numeric: tabular-nums; }

  .map-note {
    margin: 0 0 11px;
    font-size: 11.5px;
    line-height: 1.5;
    color: #767d89;
    text-align: center;
  }

  .facts {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 1px;
    margin: 0 0 9px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    overflow: hidden;
  }

  .facts > div {
    background: #0d1014;
    padding: 7px 9px;
    min-width: 0;
  }

  .facts dt {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: #666d79;
    margin-bottom: 3px;
    white-space: nowrap;
  }

  .facts dd {
    margin: 0;
    font-size: 12px;
    color: #dfe3ea;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .rake-line {
    margin: 0 0 10px;
    padding: 8px 11px;
    border-radius: 10px;
    background: rgba(0, 212, 170, 0.1);
    border: 1px solid rgba(0, 212, 170, 0.24);
    font-size: 11.5px;
    line-height: 1.45;
    color: #9fd8cb;
  }

  .rake-line strong { color: #7bf0d8; }

  .preview-drift {
    margin: 5px 0 0;
    font-size: 10.5px;
    line-height: 1.4;
    color: #f0b429;
  }

  .proofs { display: grid; gap: 7px; margin-bottom: 11px; }
  .proofs.two-up { grid-template-columns: 1fr 1fr; }

  /* Side by side there is no room for value and hint on one line, so the
     button stacks instead of truncating the hash. */
  .two-up button.mono { flex-direction: column; align-items: flex-start; gap: 2px; }
  .two-up .copy-state { font-size: 8.5px; }

  .proof {
    padding: 8px 10px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.26);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .proof-label {
    display: block;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: #666d79;
    margin-bottom: 5px;
  }

  .proof-idle {
    margin: 0;
    font-size: 10.5px;
    line-height: 1.45;
    color: #6f7683;
  }

  .mono {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 11px;
    color: #9aa2ae;
    word-break: break-all;
  }

  .mono.inline { font-size: 11px; color: #aeb4be; }

  button.mono {
    display: flex;
    width: 100%;
    text-align: left;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 7px;
    padding: 6px 9px;
    cursor: pointer;
  }

  button.mono.narrow { width: auto; margin: 0 auto 14px; }
  button.mono:hover { border-color: rgba(0, 212, 170, 0.35); color: #dfe3ea; }
  button.linkish.mono { display: inline; width: auto; padding: 0; border: none; background: none; }

  .copy-state {
    flex: none;
    font-family: inherit;
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #00d4aa;
  }

  .preview-cta { display: grid; gap: 7px; }

  .cta-note {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
    color: #666d79;
    text-align: center;
  }

  /* ----------------------------------------------------------------- footer */

  .foot {
    margin: 18px 0 0;
    text-align: center;
    font-size: 12px;
    color: #5d646f;
  }

  .foot strong { color: #00d4aa; font-weight: 600; }

  /* ------------------------------------------------------------ responsive */

  @media (max-width: 1240px) {
    .board { grid-template-columns: minmax(0, 1fr) 320px; }
    .c-hands { display: none; }
  }

  @media (max-width: 1080px) {
    .c-buyin { display: none; }
  }

  @media (max-width: 1000px) {
    .board { grid-template-columns: minmax(0, 1fr); }
    .preview { position: static; }
    .intro { grid-template-columns: minmax(0, 1fr); gap: 22px; }
    .intro h1 { font-size: 30px; }
    .c-buyin, .c-hands { display: table-cell; }
  }

  /* Below this the grid stops being readable, so each row becomes a card.
     The <table> element is kept — one <tr> per table, whatever the layout. */
  @media (max-width: 760px) {
    .lobby { padding: 10px 12px 24px; }

    /* The disclaimer banner and the app header already own ~370 px of a
       390x844 phone, so the intro has to earn every pixel: the supporting
       claims collapse from cards to chips and keep only their headline, while
       0% rake keeps its display figure because it is the reason to be here. */
    .intro { padding: 15px 15px 17px; margin-bottom: 12px; border-radius: 14px; gap: 14px; }
    .eyebrow { font-size: 9.5px; letter-spacing: 0.09em; margin-bottom: 6px; }
    .intro h1 { font-size: 25px; margin-bottom: 6px; }
    .intro-lead { font-size: 13px; margin-bottom: 12px; }
    .intro-note { display: none; }

    .headline-claim { padding: 10px 13px; column-gap: 12px; }
    .headline-figure { font-size: 30px; }
    .headline-title { font-size: 13px; }
    .headline-body { display: none; }

    .claims { display: flex; flex-wrap: wrap; gap: 6px; }

    .claims li {
      display: inline-flex;
      align-items: baseline;
      gap: 6px;
      padding: 5px 11px;
      border-radius: 999px;
    }

    .claim-body { display: none; }
    .claim-figure { font-size: 11.5px; }
    .claim-title { font-size: 11.5px; color: #9aa2ae; }

    /* Keep Refresh on the title's row: a phone cannot spare 46 px for a
       toolbar of its own before the first table card. */
    .bar { align-items: flex-start; flex-wrap: nowrap; gap: 10px; margin-bottom: 9px; }
    .bar-title { min-width: 0; }
    .bar-actions { flex: none; }
    .seg { display: none; }

    .notice { font-size: 12px; padding: 10px 13px; margin-bottom: 10px; }
    .notice.go { flex-direction: column; align-items: stretch; gap: 10px; }

    .filters {
      flex-wrap: nowrap;
      overflow-x: auto;
      padding-bottom: 4px;
      margin-bottom: 9px;
      scrollbar-width: none;
      /* The strip scrolls, so the last pill is faded rather than guillotined. */
      -webkit-mask-image: linear-gradient(to right, #000 88%, transparent 100%);
      mask-image: linear-gradient(to right, #000 88%, transparent 100%);
    }

    .filters::-webkit-scrollbar { display: none; }

    .list-pane { background: none; border: none; border-radius: 0; overflow: visible; }
    .tables-list, .tables-list tbody { display: block; width: 100%; }
    .tables-list thead { display: none; }

    .tables-list tbody tr {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 2px 12px;
      padding: 13px 14px;
      margin-bottom: 9px;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 12px;
      background: rgba(255, 255, 255, 0.022);
    }

    .tables-list tbody tr.selected { box-shadow: none; border-color: rgba(0, 212, 170, 0.32); }
    .tables-list td, .tables-list.compact td { display: block; padding: 0; }

    .c-table, .c-stakes, .c-buyin, .c-seats, .c-hands, .c-now, .c-go { width: auto; }
    .c-table { grid-column: 1; grid-row: 1; }
    .c-go { grid-column: 2; grid-row: 1; text-align: right; align-self: start; }
    .c-stakes { grid-column: 1; grid-row: 2; margin-top: 8px; }
    .c-seats { grid-column: 2; grid-row: 2; margin-top: 8px; text-align: right; }
    /* Kept in the DOM (the harness reads both against the table contract) but
       folded into the buy-in line below, so the card stays four rows tall. */
    .c-buyin { grid-column: 1 / -1; grid-row: 3; margin-top: 7px; }
    /* Specificity has to beat `.tables-list td { display: block }` above. */
    .tables-list .c-hands { display: none; }
    .c-now {
      grid-column: 1 / -1; grid-row: 4;
      margin-top: 9px; padding-top: 9px !important;
      border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .tables-list.compact .tags { display: flex; }
    .now-detail { display: inline; margin-left: 8px; }
    .buyin-value::before {
      content: 'Buy-in ';
      font-size: 10px;
      letter-spacing: 0.07em;
      text-transform: uppercase;
      color: #5d646f;
      font-weight: 600;
    }

    .go {
      background: linear-gradient(135deg, #00d4aa, #00b894);
      color: #04120f;
      padding: 7px 13px;
      border-radius: 8px;
    }

    .go.btc { background: linear-gradient(135deg, #f7931a, #c77700); color: #1a1205; }
    tr.full .go { background: rgba(255, 255, 255, 0.09); color: #b9bfc9; }

    .list-foot {
      margin: 2px 0 0;
      padding: 12px 2px 0;
      background: none;
      border-top-color: rgba(255, 255, 255, 0.07);
    }

    /* A phone has no room for a persistent master/detail pane: it would repeat
       the row the visitor is already looking at and push the list off screen.
       WPT Global's phone lobby drops it for the same reason; tapping a row
       opens the real table, which is a better preview than a picture of one. */
    .preview { display: none; }
  }
</style>
