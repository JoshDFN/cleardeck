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

  // ---------------------------------------------------------------------------
  // The row NAME is a money figure
  // ---------------------------------------------------------------------------

  /**
   * A "<sb>/<bb>" pair anywhere in a registered table name.
   *
   * `init_microstakes_tables` in the lobby canister bakes table_1's blinds into
   * ALL THREE names ("6-Max - 0.01/0.02" on a table that charges 0.05/0.10), so
   * the largest string on a row can quote a price the contract does not charge —
   * 222 px from a Stakes cell reading the real one. Two prices, one row.
   */
  const NAME_STAKES_RE = /(\d[\d.,]*)\s*\/\s*(\d[\d.,]*)/;

  /** Whitespace-insensitive compare of two rendered "a/b" quotes. */
  const sameQuote = (a, b) => String(a).replace(/\s+/g, '') === String(b).replace(/\s+/g, '');

  /**
   * Splits a registered name around the price it quotes, and says whether that
   * price is the one the TABLE contract enforces.
   *
   * The name text itself is never rewritten or elided: it is the lobby
   * canister's registered identity for this table and it stays on screen,
   * character for character. What changes is that a figure the contract
   * contradicts is struck through and labelled, so nothing on the row reads as a
   * price except the figures taken from the table contract.
   */
  function nameQuote(table) {
    const name = String(table?.name ?? '');
    const m = NAME_STAKES_RE.exec(name);
    if (!m) return { before: name, quoted: null, after: '', stale: false, chain: null };
    const cfg = effectiveConfig(table);
    const currency = currencyOf(table);
    const chain = formatBlinds(cfg.small_blind, cfg.big_blind, currency);
    return {
      before: name.slice(0, m.index),
      quoted: m[0],
      after: name.slice(m.index + m[0].length),
      stale: !sameQuote(m[0], chain),
      chain,
    };
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
  const staleNames = $derived(tables.filter((t) => nameQuote(t).stale).length);
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

  <!-- THE LIST IS THE FIRST THING. docs/DESIGN-BAR.md §9.4: every reference
       client puts its first table row between 26.6% and 36.1% of the viewport,
       and this lobby used to put it at 88.4%. Nothing is deleted to get there —
       the heading, the counts, the filters and the drift warning all moved
       INSIDE the list pane, and the signed-out pitch moved BELOW the list, where
       a visitor reads it after seeing that the tables are real.

       WAVE 5 measured the rest of the way, on the rendered page at ten widths
       (docs/DESIGN-BAR.md §9.4.2a–c). Above the first row there is now exactly
       ONE row of lobby furniture and the column headers — 70.3 px at 1440×900,
       down from 127.4 — and the first row sits at 34.8% of the viewport against
       PokerStars' 34.4%. What moved: the drift statement went BELOW the rows
       (its count stays here as a chip, at no cost in height), and the density
       and refresh controls went to the list footer. What did not move: 7 facts
       in the row, 18 in the preview, three live tables signed out, five filter
       pills rendered whole, and the four protected notices on screen. -->
  <div class="board">
    <div class="list-pane">
      <header class="pane-bar">
        <div class="pane-title">
          <h2>Cash games</h2>
          <!-- Heading and counts on ONE line, and the drift warning folded into
               it as a chip. Stacked, they cost 43 px of the 900 px viewport that
               BAR 25 measures; inline they cost 19 and the row is unchanged in
               what it says. -->
          <p class="pane-sub">
            <strong>{tables.length}</strong>
            {tables.length === 1 ? 'table' : 'tables'}
            <span class="sep">·</span>
            <strong>{seatsTaken}</strong>/{seatsTotal} seats
            {#if handsRunning > 0}
              <span class="sep">·</span>
              <strong class="hot">{handsRunning}</strong> {handsRunning === 1 ? 'hand' : 'hands'} in play
            {/if}
            <span class="sep">·</span>
            <span class="norake">0% rake</span>
            {#if liveLoaded && driftedTables > 0}
              <!-- The COUNT stays above the list, at zero extra height, and it
                   is stated in full at the foot of the list where there is room
                   for the whole sentence. Each affected row also carries its own
                   "⚠ record differs" flag in the money cell it applies to, which
                   is the placement a player actually reads. -->
              <a
                class="drift-chip"
                href="#lobby-record-drift"
                title="{driftedTables} of {tables.length} lobby records quote figures their table contracts do not charge. Every figure in the list is the contract's. Opens the full statement at the foot of the list."
              >⚠ {driftedTables} {driftedTables === 1 ? 'record' : 'records'} differ</a>
            {/if}
          </p>
        </div>

        <!-- WRAPS, never scrolls. At 390 px the strip measured scrollWidth 434
             against clientWidth 366 with `overflow-x: auto`, which sliced "Micro"
             mid-word and put "Low" off-screen behind a fade. WPT Global renders
             all five of its stake tabs at 1127 px and clips none of them
             (docs/DESIGN-BAR.md BAR 28); a wrapped pill is legible, a sliced one
             is not. -->
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

      </header>

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
              {@const nameq = nameQuote(table)}
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
                  <!-- THE NAME IS NEVER REWRITTEN. This string is the lobby
                       canister's registered identity for the table (and the key
                       the screenshot harness matches a row on), so every
                       character of it stays on screen. What it must not do is
                       READ AS A PRICE when the contract charges something else:
                       `init_microstakes_tables` baked table_1's blinds into all
                       three names, so "9-Max - 0.01/0.02" sat 222 px from a
                       Stakes cell reading 0.10/0.20. The quoted figure is struck
                       through and labelled, leaving exactly one live price on
                       the row and it comes from the table contract. -->
                  <span class="name-line">
                    {#if nameq.stale}
                      <span
                        class="table-name"
                        title="The lobby canister registered this table as “{table.name}”. Its own contract charges {nameq.chain} {unitOf(currency)}, so the figure in the name is struck through: it is the lobby's stale record, not this table's price."
                      >{nameq.before}<s class="stale-quote">{nameq.quoted}</s>{nameq.after}</span>
                      <span class="stale-flag">stale name</span>
                    {:else}
                      <span class="table-name">{table.name}</span>
                    {/if}
                  </span>
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

        {#if liveLoaded && driftedTables > 0}
          <!-- The whole statement, immediately under the rows it is about.
               It used to sit ABOVE them and cost 26 px of desktop viewport and
               57 px of phone viewport — the two lines that pushed the first row
               out of the reference band. Nothing here is softened: the sentence
               is the same one, the per-row "⚠ record differs" flag is still in
               the money cell of every affected row, the struck-through figure is
               still on any stale name, and the count is still above the list as
               a chip beside the table count. -->
          <p class="drift-strip" id="lobby-record-drift">
            <strong>
              {driftedTables === 1
                ? `1 of ${tables.length} lobby records quotes figures its table contract does not charge.`
                : `${driftedTables} of ${tables.length} lobby records quote figures their table contracts do not charge.`}
            </strong>
            Every figure above is what the <em>contract</em> charges — read from the table canister
            itself, not from the lobby's registration{#if staleNames > 0}; the stale
            {staleNames === 1 ? 'name is' : 'names are'} struck through{/if}.
          </p>
        {/if}

        {#if seatsTaken === 0}
          <!-- Under the list, not above it. It is an invitation, not a warning,
               and putting it above the rows cost 63 px of the one thing this
               screen is for. -->
          <div class="nudge">
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

        <!-- The columns a rake-funded client shows that this one does not, and
             why. Stating the gap is better than estimating it: none of these is
             recorded on-chain, so no client could compute them from this engine.

             THE TWO CONTROLS LIVE HERE NOW, not in the bar above the list. They
             cost 121 px of the bar's one row, which is what made the bar wrap to
             two lines and put the first table row 27 px lower; and neither is a
             control anybody reaches for BEFORE reading the list. Refresh belongs
             beside the sentence that states the poll interval it overrides, and
             row density belongs beside the rows it changes. Both are unchanged
             in what they do. -->
        <footer class="list-foot">
          <p class="foot-copy">
            Every column above is read live from a canister: the list from
            <button class="linkish mono" onclick={() => copyText(lobbyCanisterId, 'lobby')}>{shortId(lobbyCanisterId)}</button>,
            each row from its own table contract, re-read every {LIVE_POLL_MS / 1000} seconds.
            Average pot, players-per-flop, hands-per-hour and waiting lists are
            <strong>not recorded on-chain</strong>, so they are absent here rather than estimated.
            <button class="link-btn" onclick={() => showHow = true}>How it works</button>
          </p>

          <div class="pane-actions">
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
            <button class="btn ghost icon" onclick={refreshAll} title="Refresh tables" aria-label="Refresh tables">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M23 4v6h-6M1 20v-6h6"/>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
              </svg>
              <span class="icon-label">Refresh</span>
            </button>
          </div>
        </footer>
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
      {@const nameq = nameQuote(selectedTable)}
      <aside class="preview" aria-label="Table preview">
        <div class="preview-head">
          <div class="preview-heading">
            <!-- Same rule as the row: the registered name is quoted in full, and
                 a figure inside it that the contract contradicts is struck out
                 rather than left to argue with the Blinds fact 90 px below. -->
            <h3>
              {#if nameq.stale}
                {nameq.before}<s class="stale-quote">{nameq.quoted}</s>{nameq.after}
                <span class="stale-flag">stale name</span>
              {:else}
                {selectedTable.name}
              {/if}
            </h3>
            <p class="preview-sub">
              {tableFormat(selectedTable)} · No Limit Hold'em ·
              {stakeTier(cfg.small_blind, currency)} stakes
            </p>
            {#if drift.length > 0}
              <!-- No figure of its own. The blinds it would restate are already
                   on screen in the Blinds fact below, where the screenshot
                   harness's token census asserts them against the contract; a
                   second copy here was 2 money tokens asserted by nothing. -->
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
          <button class="link-btn" onclick={(e) => { e.stopPropagation(); showHow = true; }}>How it works</button>
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

  {#if !signedIn}
    <!-- The signed-out pitch, in full, UNDER the tables it is a claim about.
         Three claims, all of them checkable: no rake is a property of the payout
         code, the commitment is published per hand, and the whole stack is
         canisters. No rake leads, because it is the one thing a rake-funded
         client structurally cannot copy.

         It reads better here than it did above the list. Every claim it makes is
         demonstrated by the rows and the preview a visitor has already scrolled
         past — three live tables, real seat maps, a live board and a deck
         commitment, all of it readable while signed out. Nothing was cut to move
         it: this is the same section, word for word. -->
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
          commitment above is readable while signed out.
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

  <p class="foot">
    Texas Hold'em No Limit. <strong>No rake is taken from any pot on any table.</strong>
    Every deal is verifiable from the hand history.
  </p>
</div>

{#if showHow}
  <HowItWorks onClose={() => { showHow = false; }} />
{/if}

<style>
  /* The top pad is a real budget line, not a taste call. Measured on the
     rendered page at 1440x900: the disclaimer banner is 160 px and the app
     header 82.5, so 242.5 px (27.0%) of the viewport is spent before this
     component paints a pixel; on a 390x844 phone it is 268 + 119.5 = 387.5 px
     (45.9%). Everything above the first table row is measured against what is
     left, and 4 px is all this pad may take. */
  .lobby {
    max-width: 1320px;
    margin: 0 auto;
    padding: 4px 20px 32px;
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

  /* In the footer there is room for the word again: it was dropped from the top
     bar because it cost 52 px of a row the first table row was paying for. */
  .btn.icon { padding: 6px 11px; font-weight: 500; gap: 6px; }
  .icon-label { font-size: 12px; }
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

  /* Below the list now (see the markup note), so the margin is on top. */
  .intro {
    display: grid;
    grid-template-columns: minmax(0, 1.12fr) minmax(0, 1fr);
    gap: 30px;
    align-items: center;
    padding: 20px 24px 21px;
    margin-top: 18px;
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
    margin-top: 16px;
    padding-top: 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
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

  /* ------------------------------------------------- the list pane's own bar

     Heading, counts, filters and the density/refresh controls on ONE row inside
     the pane, and NOTHING else above the first row except the column headers.

     The budget, measured on the rendered page at 1440x900, from the bottom of
     the app header to the top of the first row:
       wave 3      three stacked blocks above the pane                  188 px
       wave 4      56 px bar + 26 px drift strip + 34 px thead + pads   127 px
       now         33 px bar + 30 px thead + 6 px of pad                 69 px
     Nothing was deleted to get from 127 to 69: the heading moved beside the
     counts instead of above them, the drift statement moved to the foot of the
     list with its count kept up here as a chip, and the paddings were cut. */

  .pane-bar {
    display: flex;
    align-items: center;
    gap: 5px 10px;
    flex-wrap: wrap;
    padding: 3px 12px;
    background: rgba(0, 0, 0, 0.16);
    border-bottom: 1px solid rgba(255, 255, 255, 0.055);
  }

  /* Heading BESIDE counts. Stacked they measured 43 px, which is 43 px of the
     one thing this screen is for; on one baseline they measure 19 and say the
     same words. The counts wrap under the heading on a narrow pane rather than
     being cut. */
  .pane-title {
    display: flex;
    flex-direction: row;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 2px 9px;
    min-width: 0;
    margin-right: auto;
    order: 1;
  }

  .pane-title h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: #fff;
    white-space: nowrap;
  }

  .pane-sub {
    margin: 0;
    font-size: 11.5px;
    color: #7d8492;
  }

  /* The drift COUNT, inline in the counts line, so the warning is above the
     list at zero cost in height. The sentence it summarises is at the foot of
     the list and every affected row flags its own money cell. */
  /* A link to the full statement at the foot of the list, which on a phone is
     the only way to read it without hunting: the rows are between them. */
  .drift-chip {
    display: inline-block;
    margin-left: 2px;
    text-decoration: none;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: #f0b429;
    background: rgba(240, 180, 41, 0.12);
    border: 1px solid rgba(240, 180, 41, 0.28);
    border-radius: 5px;
    padding: 0 5px;
    white-space: nowrap;
  }

  .drift-chip:hover {
    background: rgba(240, 180, 41, 0.2);
    border-color: rgba(240, 180, 41, 0.45);
  }

  .pane-sub strong { color: #dfe3ea; font-weight: 600; }
  .pane-sub .hot { color: #00d4aa; }
  .pane-sub .sep { color: #3a3f49; margin: 0 5px; }

  .norake {
    color: #00d4aa;
    font-weight: 600;
  }

  .pane-actions { display: flex; align-items: center; gap: 8px; flex: none; }

  /* PokerStars ships row density as a strip of small icon buttons rather than
     words (docs/DESIGN-BAR.md); at three tables the words were costing more
     width than the feature is worth. */
  /* These two controls set the height of the whole bar, so their padding is a
     viewport measurement, not a taste call: 5 px of button padding put the row
     at 31 px and the first table row 6 px lower down the page. */
  .seg {
    display: flex;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 8px;
    padding: 1px;
  }

  .seg button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: #6f7683;
    font: inherit;
    padding: 4px 7px;
    border-radius: 7px;
    cursor: pointer;
  }

  .seg button:hover { color: #b9bfc9; }
  .seg button.on { background: rgba(255, 255, 255, 0.09); color: #fff; }

  /* ---------------------------------------------------------------- filters */

  .filters {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    order: 2;
  }

  .filter-gap {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.09);
    margin: 0 2px;
  }

  .pill {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.07);
    color: #7d8492;
    font: inherit;
    font-size: 12px;
    padding: 4px 11px;
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

  /* ------------------------------------------- the two in-pane status strips */

  /* BELOW the rows, full-bleed inside the pane. Above them it was 26 px of a
     desktop viewport and 57 px of a phone's — the last thing between the visitor
     and the list, and on a phone the single most expensive line on the screen.
     The statement is unchanged, the count is still above the list as a chip, and
     every affected row still flags its own money cell. */
  .drift-strip {
    margin: 0;
    padding: 7px 12px;
    font-size: 11px;
    line-height: 1.38;
    color: #9aa2ae;
    background: rgba(240, 180, 41, 0.07);
    border-top: 1px solid rgba(240, 180, 41, 0.22);
  }

  .drift-strip strong { color: #f0b429; font-weight: 600; }
  .drift-strip em { color: #dfe3ea; font-style: normal; font-weight: 600; }

  .nudge {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 0;
    padding: 11px 15px;
    font-size: 12.5px;
    line-height: 1.55;
    color: #9aa2ae;
    border-top: 1px solid rgba(0, 212, 170, 0.2);
    background: rgba(0, 212, 170, 0.055);
  }

  .nudge strong { color: #00d4aa; }

  /* ------------------------------------------------------------- the board */

  /* `start`, not `stretch`. Stretching the list pane to the preview's height was
     right when the first row sat at 41% and the rows filled the pane; with the
     rows 57 px higher up the page a three-table lobby stretched to a 352 px
     preview left a 270 px empty BOX between the last row and the footer, which
     reads as a rendering failure. The pane is now exactly as tall as what is in
     it, and the page background carries the difference. */
  .board {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 352px;
    gap: 18px;
    align-items: start;
  }

  /* A flex column so the provenance footer sits at the BOTTOM of the pane.
     The pane stretches to the preview's height (align-items: stretch above), so
     without this a three-row lobby left ~600 px of unexplained void beside a
     tall preview — the single worst thing about the previous layout. */
  /* `container-type: inline-size` is load-bearing, not decoration. Which columns
     fit is a question about the PANE's width, and the pane's width is the
     viewport MINUS the preview (0, 320 or 352 px) minus the gaps — so a
     viewport-width media query answers the wrong question and got it wrong in
     four separate sub-ranges, every one of which clipped the row's action
     control (docs/DEFECTS.md L-01). Measured minimum pane widths for the table's
     own min-content, at 12 px cell padding: 7 columns 879 px, 6 columns (no
     Hands) 807, 5 columns (no Hands, no Buy-in) 687. The two `@container` rules
     below are those numbers. */
  .list-pane {
    display: flex;
    flex-direction: column;
    background: rgba(255, 255, 255, 0.018);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 14px;
    overflow: hidden;
    container-type: inline-size;
  }

  /* Hands goes first — it is the least load-bearing column and the one a
     rake-funded client would not have either. Buy-in goes second, and its range
     is still stated in the preview pane's facts list at every width. */
  @container (max-width: 878px) {
    .c-hands { display: none; }
  }

  @container (max-width: 806px) {
    .c-buyin { display: none; }
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

  /* The only lobby furniture left above the first row besides the bar, so its
     padding is a budget line too: 9 px put the header row at 34.3 px, 7 px puts
     it at 30.3 and no label changed. */
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
    padding: 7px 12px;
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

  /* 12 px, not 16, and that is a correctness fix rather than a taste call.
     Measured at 1440x900 on the rendered page: seven columns of nowrap content
     plus 16 px of padding a side gives the table a min-content width of
     934.5 px inside a 908 px pane, and `.list-pane { overflow: hidden }` then
     CLIPPED the last column — every row's `Sit` / `View` / `Watch` control lost
     its right 10.5 px, arrow included (docs/DEFECTS.md L-01). 12 px takes 56 px
     out of the table's min-content and every column fits with room to spare. */
  .tables-list td { padding: 13px 12px; vertical-align: middle; }
  .tables-list.compact td { padding: 8px 12px; }
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
  .name-line {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
  }

  .table-name {
    display: block;
    min-width: 0;
    color: #fff;
    font-weight: 600;
    font-size: 14.5px;
    letter-spacing: -0.005em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* A struck figure is not a price. The characters stay (this is the lobby
     canister's registered name and it is quoted verbatim), but nothing on the
     row reads as this table's stakes except the Stakes cell, which is the table
     contract's. */
  .stale-quote {
    color: #a98436;
    text-decoration: line-through;
    text-decoration-thickness: 1.5px;
    font-weight: 500;
  }

  .stale-flag {
    flex: none;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: #f0b429;
    background: rgba(240, 180, 41, 0.12);
    border-radius: 4px;
    padding: 1px 5px;
    white-space: nowrap;
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

  /* `.tag.stale` used to live here with no markup to match it. The stale-record
     treatment is `.stale-quote` / `.stale-flag` above, on the name itself. */

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
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin: auto 0 0;
    padding: 11px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.055);
    background: rgba(0, 0, 0, 0.18);
    font-size: 11px;
    line-height: 1.6;
    color: #6b7280;
  }

  .foot-copy { margin: 0; min-width: 0; }

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

  /* Which COLUMNS survive is decided by the two `@container` rules above; these
     rules only decide how much room the preview takes away from the pane. */
  @media (max-width: 1240px) {
    .board { grid-template-columns: minmax(0, 1fr) 320px; }
  }

  /* The preview drops below the list at 1080, not 1000. Between the two the
     pane measured 623–686 px against a 687 px five-column floor, so the table
     overflowed and `overflow: hidden` cut the Sit control off the right edge of
     every row. There is no column left to drop at that point — Table, Stakes,
     Seats, Now and the action are the row — so the preview is what gives way. */
  @media (max-width: 1080px) {
    .board { grid-template-columns: minmax(0, 1fr); }
    .preview { position: static; }
    .intro { grid-template-columns: minmax(0, 1fr); gap: 22px; }
    .intro h1 { font-size: 30px; }
  }

  /* Below this the grid stops being readable, so each row becomes a card.
     The <table> element is kept — one <tr> per table, whatever the layout. */
  @media (max-width: 760px) {
    .lobby { padding: 2px 12px 24px; }

    /* The intro used to sit ABOVE the list, where the disclaimer banner and the
       app header had already spent 388 px of an 844 px phone, so it hid its own
       supporting sentences to buy space it never got back — the first card still
       landed at 119% of the viewport. It sits below the list now, where nothing
       above the fold is competing with it, so every sentence it used to drop on
       a phone is back. */
    .intro { padding: 15px 15px 17px; margin-top: 14px; border-radius: 14px; gap: 16px; }
    .eyebrow { font-size: 9.5px; letter-spacing: 0.09em; margin-bottom: 6px; }
    .intro h1 { font-size: 25px; margin-bottom: 6px; }
    .intro-lead { font-size: 13px; margin-bottom: 12px; }

    .headline-claim { padding: 11px 13px; column-gap: 12px; }
    .headline-figure { font-size: 30px; }
    .headline-title { font-size: 13px; }

    /* One phone-width bar per claim, not two columns of 74 px. */
    .claims li { grid-template-columns: 62px minmax(0, 1fr); column-gap: 11px; }
    .claim-figure { font-size: 12.5px; }

    /* Heading, counts and Refresh on one wrapped row; the filter strip below it.
       No horizontal scroller anywhere: see .filters.

       Measured from the bottom of the app header to the top of the first card at
       390x844: 155 px before this pass (84 px bar + 57 px drift strip + pads),
       67 px after. The chrome above the lobby is 387.5 px of an 844 px screen
       and no lobby layout can move it, so these are the only px the lobby owns
       and they are spent down to the row of pills BAR 28 forbids clipping. */
    .pane-bar {
      padding: 0 0 5px;
      gap: 5px 10px;
      background: none;
      border-bottom: none;
    }

    /* `1 1 0` (not `auto`) so the block shrinks, and Refresh is re-ordered ahead
       of the filter strip so it shares the heading's row instead of taking a
       31 px row of its own. */
    .pane-title { flex: 1 1 0; min-width: 0; gap: 1px 7px; }
    .pane-title h2 { font-size: 14px; }
    .pane-sub { font-size: 11px; }
    .seg { display: none; }
    .drift-chip { font-size: 10px; }

    /* The footer becomes two stacked blocks on a phone rather than one row with
       a 100 px control hanging off the end of a wrapped paragraph. */
    .list-foot { flex-direction: column; align-items: flex-start; gap: 10px; }

    /* WRAPS. At 390 px the pills measured scrollWidth 434 against clientWidth
       366 under `overflow-x: auto`, so "Micro" was sliced mid-word and "Low" was
       off-screen behind a fade. Now scrollWidth == clientWidth == 366: the
       tighter pill fits all five on one line at this width, and anything that
       does not fit wraps to a second line rather than being cut. */
    .filters {
      flex: 1 1 100%;
      flex-wrap: wrap;
      gap: 6px;
      overflow: visible;
      order: 3;
    }

    .filter-gap { display: none; }
    .pill { font-size: 11.5px; padding: 5px 10px; }

    .drift-strip {
      padding: 9px 11px;
      margin-top: 3px;
      font-size: 11px;
      line-height: 1.4;
      border: 1px solid rgba(240, 180, 41, 0.24);
      border-radius: 11px;
    }

    .nudge {
      flex-direction: column;
      align-items: stretch;
      gap: 10px;
      margin-top: 2px;
      padding: 11px 13px;
      font-size: 12px;
      border: 1px solid rgba(0, 212, 170, 0.2);
      border-radius: 11px;
    }

    .list-pane { background: none; border: none; border-radius: 0; overflow: visible; }
    .tables-list, .tables-list tbody { display: block; width: 100%; }
    .tables-list thead { display: none; }

    /* The card's own height decides how many tables a phone can compare without
       scrolling, which is the second half of BAR 31. At 194.6 px only ONE card
       fitted under 387.5 px of chrome plus the bar; every px below ~190 buys the
       second one. 183 px is what these paddings measure, and no field was
       dropped to get there. */
    .tables-list tbody tr {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 2px 12px;
      padding: 9px 11px;
      margin-bottom: 6px;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 12px;
      background: rgba(255, 255, 255, 0.022);
    }

    .tables-list tbody tr .tags { margin-top: 4px; }

    .tables-list tbody tr.selected { box-shadow: none; border-color: rgba(0, 212, 170, 0.32); }
    .tables-list td, .tables-list.compact td { display: block; padding: 0; }

    .c-table, .c-stakes, .c-buyin, .c-seats, .c-hands, .c-now, .c-go { width: auto; }
    .c-table { grid-column: 1; grid-row: 1; }
    .c-go { grid-column: 2; grid-row: 1; text-align: right; align-self: start; }
    .c-stakes { grid-column: 1; grid-row: 2; margin-top: 6px; }
    .c-seats { grid-column: 2; grid-row: 2; margin-top: 6px; text-align: right; }
    /* Kept in the DOM (the harness reads both against the table contract) but
       folded into the buy-in line below, so the card stays four rows tall. */
    .c-buyin { grid-column: 1 / -1; grid-row: 3; margin-top: 5px; }
    /* Specificity has to beat `.tables-list td { display: block }` above. */
    .tables-list .c-hands { display: none; }
    .c-now {
      grid-column: 1 / -1; grid-row: 4;
      margin-top: 6px; padding-top: 6px !important;
      border-top: 1px solid rgba(255, 255, 255, 0.06);
    }
    .tables-list.compact .tags { display: flex; }
    .now-detail { display: inline; margin-left: 8px; }

    /* Inline, not a line of their own. On a phone each of these two flags added
       15–17 px to the tallest cards — the exact cards that decide whether a
       second table is on screen — and both fit beside the figure they qualify:
       "0.05/0.10 ICP ⚠ RECORD DIFFERS" measures 181 px in a 254 px column. */
    .drift-mark { display: inline; margin-top: 0; margin-left: 7px; }
    .seat-note { display: inline; margin-top: 0; margin-left: 6px; }
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

    /* EVERY TAPPABLE CONTROL AT THE 44 PX TOUCH FLOOR (the audit measured the
       pills at 26 px and the copy buttons at ~24 px). The filter pills keep
       their type and radius and grow only in height; five still fit one row
       at 390 px (measured 366 px of content). */
    .pill { min-height: var(--cd-touch-min); padding: 0 10px; display: inline-flex; align-items: center; }
    .drift-chip { min-height: var(--cd-touch-min); display: inline-flex; align-items: center; }
    .btn, .btn.sm, .btn.icon, .btn.ghost { min-height: var(--cd-touch-min); }
    .link-btn, .mono.copy, .linkish { min-height: var(--cd-touch-min); display: inline-flex; align-items: center; }
    .sort { min-height: var(--cd-touch-min); }
  }
</style>
