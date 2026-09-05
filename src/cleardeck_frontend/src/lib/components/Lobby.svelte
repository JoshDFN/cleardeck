<script>
  // ClearDeck lobby.
  //
  // The first screen a stranger sees, and it has five seconds to say three
  // things: what this is (on-chain No Limit Hold'em, no rake, a deck you can
  // verify), how to start (one sign-in, Internet Identity, explained in a
  // sentence), and where the tables are (designed cards, not a data grid).
  //
  // Everything on this screen is read from a canister. Two sources:
  //   1. the lobby canister's TableInfo (via the `tables` prop): name, stakes,
  //      buy-in range, seat count, currency, per-table canister id, the clock;
  //   2. each table canister's `get_table_view()`, an anonymous query, so a
  //      signed-out visitor sees the same live truth a seated player does.
  // The table contract's config is what a row renders (lib/lobby-rows.js);
  // the lobby record is the fallback and a disagreement is named quietly.
  //
  // What is deliberately NOT here, because the engine does not record it and
  // a lobby statistic that is invented is worse than a missing column: average
  // pot, players-per-flop, hands/hour, waiting lists. The list footer says so.
  import { onMount } from 'svelte';
  import { auth } from '$lib/auth.js';
  import { createTableActorProxy, lobbyCanisterId } from '$lib/canisters.js';
  import logger from '$lib/logger.js';
  import HowItWorks from './HowItWorks.svelte';
  import LobbyHero from './LobbyHero.svelte';
  import LobbyTableRow from './LobbyTableRow.svelte';
  import LobbyPreview from './LobbyPreview.svelte';
  import LobbyColdStart from './LobbyColdStart.svelte';
  import LobbyEmpty from './LobbyEmpty.svelte';
  import { shortId, stakeTier, tierDefinition } from '$lib/lobby-format.js';
  import {
    buildRow, canisterIdOf, configDrift, currencyOf, effectiveConfig, isDealing, nameQuote, opt,
    starterTable, tagOf,
  } from '$lib/lobby-rows.js';
  import { loadPrices } from '$lib/prices.js';
  import { inviteLinkFor } from '$lib/invite-link.js';

  const {
    tables,
    onJoinTable,
    onRefresh,
    /** True while the page's last read of the lobby failed (the toast says so too). */
    loadFailed = false,
  } = $props();

  /** How often the live per-table view is re-read while the lobby is open. */
  const LIVE_POLL_MS = 12_000;

  // ---------------------------------------------------------------------------
  // Local UI state
  // ---------------------------------------------------------------------------

  let openSeatsOnly = $state(false);
  let currencyFilter = $state('all');
  let stakeFilter = $state('all');
  let sortColumn = $state('seats');
  let selectedId = $state(null);
  let showHow = $state(false);
  let signInError = $state(null);
  let signingIn = $state(false);
  let copiedId = $state(null);
  /** The page's fiat quote (lib/prices.js), or null: no quote, no fiat hints. */
  let prices = $state(null);

  let authState = $state({ isAuthenticated: false, principal: null, isLoading: true });
  const signedIn = $derived(authState.isAuthenticated);

  // ---------------------------------------------------------------------------
  // Live table views
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

  const cfgOf = (table) => effectiveConfig(table, liveOf(table));

  // ---------------------------------------------------------------------------
  // Filtering and sorting
  // ---------------------------------------------------------------------------

  const currenciesPresent = $derived([...new Set(tables.map(currencyOf))]);
  const tiersPresent = $derived(
    [...new Set(tables.map((t) => stakeTier(cfgOf(t).small_blind, currencyOf(t))))],
  );

  const SORTS = [
    ['seats', 'Most players'],
    ['stakes', 'Lowest stakes'],
    ['buyin', 'Lowest buy-in'],
    ['name', 'Name'],
  ];

  const visibleTables = $derived.by(() => {
    const filtered = tables.filter((t) => {
      if (openSeatsOnly && Number(t.player_count) >= Number(cfgOf(t).max_players)) return false;
      if (currencyFilter !== 'all' && currencyOf(t) !== currencyFilter) return false;
      if (stakeFilter !== 'all' && stakeTier(cfgOf(t).small_blind, currencyOf(t)) !== stakeFilter) return false;
      return true;
    });
    // Sorted into a NEW array: `tables` is owned by the page and must not move.
    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (sortColumn) {
        case 'name': cmp = a.name.localeCompare(b.name); break;
        case 'stakes': cmp = Number(cfgOf(a).small_blind) - Number(cfgOf(b).small_blind); break;
        case 'buyin': cmp = Number(cfgOf(a).min_buy_in) - Number(cfgOf(b).min_buy_in); break;
        case 'seats':
        default: cmp = Number(b.player_count) - Number(a.player_count); break;
      }
      // Table id is the tie-break so the order is total and stable across
      // reloads: a lobby that reshuffles itself on refresh is unusable.
      return cmp !== 0 ? cmp : Number(a.id) - Number(b.id);
    });
  });

  const rows = $derived(visibleTables.map((t) => buildRow(t, liveOf(t))));

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
  const seatsTotal = $derived(tables.reduce((n, t) => n + Number(cfgOf(t).max_players), 0));
  const driftedTables = $derived(tables.filter((t) => configDrift(t, liveOf(t)).length > 0).length);
  const staleNames = $derived(tables.filter((t) => nameQuote(t, liveOf(t)).stale).length);
  const handsRunning = $derived(
    tables.filter((t) => isDealing(liveOf(t) ? tagOf(liveOf(t).phase) : null)).length,
  );
  /** Where a first-time visitor should start (lobby-rows.js starterTable). */
  const starter = $derived(starterTable(tables, liveOf));

  // ---------------------------------------------------------------------------
  // Selection / preview
  // ---------------------------------------------------------------------------

  const selectedTable = $derived(
    visibleTables.find((t) => String(t.id) === String(selectedId)) ?? visibleTables[0] ?? null,
  );

  function select(table) {
    selectedId = String(table.id);
  }

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

  /** The share link for a table (lib/invite-link.js), or null off the browser. */
  function inviteFor(table) {
    if (typeof window === 'undefined' || !table) return null;
    return inviteLinkFor(window.location, canisterIdOf(table));
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

  /**
   * The same call the header's Sign in makes (`auth.login()`, AuthClient against
   * the Internet Identity provider in ic-config.js). One auth code path, two
   * entry points, so this button cannot drift from it.
   */
  async function signIn() {
    if (signingIn) return;
    signingIn = true;
    signInError = null;
    try {
      await auth.login();
    } catch (e) {
      logger.error('lobby sign-in failed', e);
      signInError = 'Could not reach Internet Identity. Try the Sign in button in the header.';
    } finally {
      signingIn = false;
    }
  }

  // ---------------------------------------------------------------------------
  // Wiring
  // ---------------------------------------------------------------------------

  onMount(() => {
    loadPrices().then((p) => { prices = p; }).catch((e) => logger.debug('no fiat quote', e));
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
  <LobbyHero
    {signedIn}
    {signingIn}
    error={signInError}
    onSignIn={signIn}
    onHow={() => { showHow = true; }}
  />

  <div class="board">
    <div class="list-pane">
      <header class="pane-bar">
        <div class="pane-title">
          <h2>Cash games</h2>
          <p class="pane-sub">
            <strong>{tables.length}</strong>
            {tables.length === 1 ? 'table' : 'tables'}
            <span class="sep">·</span>
            <strong>{seatsTaken}</strong>/{seatsTotal} seats
            {#if handsRunning > 0}
              <span class="sep">·</span>
              <strong class="hot">{handsRunning}</strong> {handsRunning === 1 ? 'hand' : 'hands'} in play
            {/if}
          </p>
        </div>

        <!-- WRAPS, never scrolls (docs/DESIGN-BAR.md BAR 28): a wrapped pill is
             legible, a sliced one is not. -->
        <div class="filters">
          <button class="pill" class:on={!openSeatsOnly} onclick={() => openSeatsOnly = false}>
            <span class="wide">All tables</span><span class="phone">All</span>
          </button>
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
              <button
                class="pill"
                class:on={stakeFilter === tier}
                title={tierDefinition(tier, prices?.icpUsd ?? null)}
                onclick={() => stakeFilter = tier}
              >{tier}</button>
            {/each}
          {/if}

          {#if filtersActive}
            <button class="pill clear" onclick={clearFilters}>Clear</button>
          {/if}
        </div>
      </header>

      {#if tables.length === 0 && loadFailed}
        <!-- The read failed (the toast carries the raw text): the empty list
             is not evidence of an empty lobby, and must not say it is. -->
        <LobbyEmpty kind="failed" onRetry={refreshAll} />
      {:else if tables.length === 0}
        <LobbyEmpty
          kind="none"
          lobbyId={lobbyCanisterId}
          copied={copiedId === 'lobby'}
          onCopy={() => copyText(lobbyCanisterId, 'lobby')}
          onRetry={refreshAll}
        />
      {:else if !liveLoaded}
        <!-- Rows are held back for exactly one round of `get_table_view()`: the
             lobby canister's cached config can be wrong, so rendering it even
             for a frame would put stakes on screen the table will not charge. -->
        <LobbyEmpty kind="loading" count={tables.length} />
      {:else if visibleTables.length === 0}
        <LobbyEmpty kind="filtered" count={tables.length} onClearFilters={clearFilters} />
      {:else}
        {#if seatsTaken === 0}
          <LobbyColdStart
            {starter}
            inviteLink={inviteFor(starter)}
            copied={copiedId === 'cold-start'}
            onOpen={openTable}
            onCopyInvite={copyText}
          />
        {/if}

        <!-- One <tr> per table, whatever the layout: the harness reads the
             list row by row. The header row is for assistive tech and the
             scraper; each card labels its own cells. -->
        <table class="tables-list">
          <thead>
            <tr>
              <th>Table</th>
              <th>Stakes</th>
              <th>Buy-in</th>
              <th>Seats</th>
              <th>Clock</th>
              <th>Now</th>
              <th>Open</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as row (row.id)}
              <LobbyTableRow
                {row}
                {signedIn}
                {prices}
                selected={Boolean(selectedTable) && String(selectedTable.id) === row.id}
                inviteLink={inviteFor(row.table)}
                copied={copiedId === row.id}
                onOpen={openTable}
                onSelect={select}
                onCopyInvite={copyText}
              />
            {/each}
          </tbody>
        </table>

        {#if driftedTables > 0}
          <!-- A footnote, not a warning: every figure above is the contract's.
               The affected rows carry their own quiet "record differs" mark. -->
          <p class="drift-strip" id="lobby-record-drift">
            {driftedTables === 1
              ? `1 of ${tables.length} lobby records quotes figures its table contract does not charge.`
              : `${driftedTables} of ${tables.length} lobby records quote figures their table contracts do not charge.`}
            Every figure above is what the contract charges, read from the table canister
            itself{#if staleNames > 0}; the stale {staleNames === 1 ? 'name is' : 'names are'} struck through{/if}.
          </p>
        {/if}

        <footer class="list-foot">
          <p class="foot-copy">
            Every figure is read live from its table contract, re-read every {LIVE_POLL_MS / 1000} seconds;
            the list comes from
            <button class="linkish mono" onclick={() => copyText(lobbyCanisterId, 'lobby')}>{shortId(lobbyCanisterId)}</button>.
            Average pot, players-per-flop and hands-per-hour are
            <strong>not recorded on-chain</strong>, so they are absent rather than estimated.
            <button class="link-btn" onclick={() => showHow = true}>How it works</button>
          </p>

          <div class="pane-actions">
            <label class="sort-ctl">
              <span>Sort</span>
              <select bind:value={sortColumn} aria-label="Sort tables">
                {#each SORTS as [key, label]}
                  <option value={key}>{label}</option>
                {/each}
              </select>
            </label>
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
      <LobbyPreview
        table={selectedTable}
        view={liveOf(selectedTable)}
        {prices}
        {signedIn}
        {copiedId}
        onOpen={openTable}
        onCopy={copyText}
        onHow={() => { showHow = true; }}
      />
    {/if}
  </div>

  <!-- The no-rake sentence is the trust bar's (lib/notices.js), stated once
       per screen; the hero's claims line carries the 0% figure. -->
  <p class="foot">
    Texas Hold'em No Limit. Every deal is verifiable from the hand history.
  </p>
</div>

{#if showHow}
  <HowItWorks onClose={() => { showHow = false; }} />
{/if}

<style>
  .lobby {
    max-width: 1320px;
    margin: 0 auto;
    padding: var(--cd-space-2) var(--cd-space-5) var(--cd-space-6);
    color: var(--cd-ink-1);
  }

  /* ---------------------------------------------------------------- buttons */

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: var(--cd-control-sm);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    font-family: inherit;
    padding: 0 var(--cd-space-4);
    cursor: pointer;
    border: 1px solid transparent;
    transition: background var(--cd-fast), border-color var(--cd-fast), color var(--cd-fast);
  }

  .btn.ghost {
    background: var(--cd-surface-2);
    border-color: var(--cd-line-strong);
    color: var(--cd-ink-1);
  }

  .btn.ghost:hover:not(:disabled) { background: var(--cd-surface-3); color: var(--cd-ink); }
  .btn.icon { padding: 0 11px; font-weight: var(--cd-weight-medium); gap: 6px; }
  .icon-label { font-size: var(--cd-text-sm); }

  .link-btn {
    background: none;
    border: none;
    color: var(--cd-accent);
    font: inherit;
    font-size: var(--cd-text-sm);
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
    color: var(--cd-ink-2);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .linkish:hover { color: var(--cd-accent); }

  /* ----------------------------------------------------------- the board */

  /* `start`, not `stretch`: the pane is exactly as tall as what is in it, and
     the page background carries the difference beside a tall preview. */
  .board {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 352px;
    gap: var(--cd-space-4);
    align-items: start;
  }

  /* `container-type: inline-size` is load-bearing: which cells a card shows
     is a question about the PANE's width (the viewport minus the preview),
     and a viewport media query answers the wrong question (docs/DEFECTS.md
     L-01). The narrow rule below hides the clock and the buy-in cells; both
     are stated in the preview pane at every width. */
  .list-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    container-type: inline-size;
  }

  @media (min-width: 761px) {
    @container (max-width: 820px) {
      .tables-list :global(tbody tr) {
        grid-template-columns:
          minmax(0, 2fr) minmax(0, 1.3fr) minmax(0, 1.3fr) minmax(0, 1.5fr) auto;
      }

      .tables-list :global(.c-clock),
      .tables-list :global(.c-buyin) { display: none; }
    }
  }

  /* ------------------------------------------------- the list pane's bar */

  .pane-bar {
    display: flex;
    align-items: center;
    gap: 5px var(--cd-space-3);
    flex-wrap: wrap;
    padding: 0 var(--cd-space-1) var(--cd-space-2);
  }

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
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-figure);
    letter-spacing: -0.01em;
    color: var(--cd-ink);
    white-space: nowrap;
  }

  .pane-sub {
    margin: 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
  }

  .pane-sub strong { color: var(--cd-ink); font-weight: var(--cd-weight-strong); }
  .pane-sub .hot { color: var(--cd-accent); }
  .pane-sub .sep { color: var(--cd-line-strong); margin: 0 5px; }

  .pane-actions { display: flex; align-items: center; gap: var(--cd-space-2); flex: none; }

  .sort-ctl {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
  }

  .sort-ctl select {
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font: inherit;
    font-size: var(--cd-text-sm);
  }

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
    background: var(--cd-line);
    margin: 0 2px;
  }

  .pill {
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line);
    color: var(--cd-ink-2);
    font: inherit;
    font-size: var(--cd-text-sm);
    padding: 4px 11px;
    border-radius: var(--cd-radius-pill);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--cd-fast), color var(--cd-fast), border-color var(--cd-fast);
  }

  .pill:hover { color: var(--cd-ink); }
  .pill .phone { display: none; }

  .pill.on {
    background: var(--cd-accent-dim);
    border-color: var(--cd-accent-line);
    color: var(--cd-accent);
  }

  .pill.clear { color: var(--cd-ink-2); text-decoration: underline; text-underline-offset: 3px; border-color: transparent; }

  /* ---------------------------------------------------------------- the list */

  /* The <table> is a block; the body is a column of cards (LobbyTableRow). */
  .tables-list { display: block; width: 100%; border-collapse: collapse; }
  .tables-list thead { display: none; }
  .tables-list :global(tbody) { display: grid; gap: var(--cd-space-2); }

  /* The lobby-record footnote: quiet, under the rows it is about. */
  .drift-strip {
    margin: var(--cd-space-2) 0 0;
    padding: 0 var(--cd-space-1);
    font-size: var(--cd-text-xs);
    line-height: 1.45;
    color: var(--cd-ink-2);
  }

  .list-foot {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--cd-space-4);
    margin: var(--cd-space-2) 0 0;
    padding: var(--cd-space-3) var(--cd-space-1) 0;
    border-top: 1px solid var(--cd-line-soft);
    font-size: var(--cd-text-xs);
    line-height: 1.6;
    color: var(--cd-ink-2);
  }

  .foot-copy { margin: 0; min-width: 0; }
  .list-foot strong { color: var(--cd-ink-1); font-weight: var(--cd-weight-strong); }

  /* The empty, failed, loading and filtered states are LobbyEmpty.svelte. */

  .mono {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    word-break: break-all;
  }

  button.linkish.mono { display: inline; width: auto; padding: 0; border: none; background: none; }

  /* ----------------------------------------------------------------- footer */

  .foot {
    margin: var(--cd-space-4) 0 0;
    text-align: center;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
  }

  /* ------------------------------------------------------------ responsive */

  @media (max-width: 1240px) {
    .board { grid-template-columns: minmax(0, 1fr) 320px; }
  }

  @media (max-width: 1080px) {
    .board { grid-template-columns: minmax(0, 1fr); }
  }

  @media (max-width: 760px) {
    .lobby { padding: var(--cd-space-2) var(--cd-space-3) var(--cd-space-5); }

    .pane-bar { padding: 0 0 5px; gap: 5px 10px; }
    .pane-title { flex: 1 1 0; min-width: 0; gap: 1px 7px; }
    .list-foot { flex-direction: column; align-items: flex-start; gap: var(--cd-space-3); }

    .filters { flex: 1 1 100%; flex-wrap: wrap; gap: 6px; overflow: visible; order: 3; }
    .filter-gap { display: none; }

    /* A phone has no room for a persistent master/detail pane: tapping a card
       opens the real table, which is a better preview than a picture of one. */
    .board :global(aside.preview) { display: none; }

    /* Every tappable control at the 44 px touch floor, and the five pills
       on ONE row at 390 px (the first reads "All" here). */
    .pill { min-height: var(--cd-touch-min); min-width: var(--cd-touch-min); padding: 0 9px; display: inline-flex; align-items: center; justify-content: center; }
    .pill .wide { display: none; }
    .pill .phone { display: inline; }
    .btn, .btn.icon, .btn.ghost { min-height: var(--cd-touch-min); }
    .sort-ctl select { min-height: var(--cd-touch-min); }
    .link-btn, .linkish { min-height: var(--cd-touch-min); display: inline-flex; align-items: center; }
  }
</style>
