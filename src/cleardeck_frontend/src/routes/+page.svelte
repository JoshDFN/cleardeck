<script>
  import "../index.scss";
  import { lobby, createTableActorProxy } from "$lib/canisters";
  import Lobby from "$lib/components/Lobby.svelte";
  import PokerTable from "$lib/components/PokerTable.svelte";
  import ShuffleProof from "$lib/components/ShuffleProof.svelte";
  import WalletButton from "$lib/components/WalletButton.svelte";
  import HandHistory from "$lib/components/HandHistory.svelte";
  import HowItWorks from "$lib/components/HowItWorks.svelte";
  import TrustBar from "$lib/components/TrustBar.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { searchWithTable, tableIdFromSearch } from "$lib/invite-link.js";
  import { handNumberFromSearch } from "$lib/hand-link.js";
  import DepositModal from "$lib/components/DepositModal.svelte";
  import WithdrawModal from "$lib/components/WithdrawModal.svelte";
  import { playSound, setSoundEnabled, isSoundEnabled } from "$lib/sounds.js";
  import logger from "$lib/logger.js";
  import { auth, isSignatureError, wallet } from "$lib/auth.js";
  import { describeLobbyFailure, retryDelayMs } from "$lib/humane-errors.js";
  import NoticeLine from "$lib/components/NoticeLine.svelte";
  // docs/DEFECTS.md T-02: the "Verify the Code" panel used to hardcode
  // `icp canister status qrhly-… -e ic` in both the visible <code> block and the
  // Copy button, so a LOCAL dev build handed the user a mainnet command. The
  // command now comes from the same config that wires the actors, so it can never
  // disagree with what this build actually talks to. The mainnet ids further down
  // the panel are still shown as text on purpose: that display is the point of
  // the panel, and it is sourced from MAINNET_CANISTER_IDS rather than retyped.
  import {
    agentHost, II_URL, IS_MAINNET_BUILD, MAINNET_CANISTER_IDS, NETWORK, statusCommandFor,
  } from "$lib/ic-config.js";
  import { lobbyCanisterId, historyCanisterId } from "$lib/canisters";
  // docs/DEFECTS.md T-38: the "Deployed Canister Hashes" table stated three
  // hashes as fact and had been three upgrades stale for an unknown length of
  // time, because nothing in the app, the build or any gate compared them to
  // anything. See the header of $lib/deployed-build.js for what replaced them.
  import {
    EXPECTED_MODULE_HASHES, EXPECTED_PROVENANCE, LIVE_HASH_SOURCE, REBUILD_COMMAND,
    compareHash, displayHash, liveHashCommandForAll, readLiveModuleHash,
  } from "$lib/deployed-build.js";
  import { currencyOf, formatTokenAmount } from "$lib/utils.js";
  // docs/DEFECTS.md E-92: the 500 ms render poll used to open with an UPDATE call
  // (`check_timeouts`), so every open tab drove an update loop that no cycles
  // figure in this tree counted. The decision of when the clock actually needs
  // advancing lives in one pure module, so it can be gated without a replica.
  import { ClockNudgePolicy, CLOCK_NUDGE } from "$lib/clockNudge.js";
  import { HttpAgent } from '@dfinity/agent';
  import { Principal } from '@dfinity/principal';
  import { beginPending, humaneActionError, pendingExpired, pendingStatus, projectPending } from '$lib/optimistic.js';
  // The audit's critical mobile finding: an anonymous "Sit" tap reached the
  // canister and came back as a red balance error over the phone header. The
  // decision (sign in / top up / join) is a pure module; this file only acts.
  import { escrowFromRead, joinGateDecision, joinGateFacts, seatToResume } from '$lib/join-gate.js';
  import RotatePrompt from '$lib/components/RotatePrompt.svelte';
  import { untrack } from 'svelte';

  let view = $state('lobby'); // 'lobby' | 'table'
  let tables = $state([]);
  let tableState = $state(null);
  let myCards = $state(null);
  let shuffleProof = $state(null);
  let loading = $state(false);
  let loadingTableState = false; // Non-reactive flag to prevent concurrent loadTableState calls
  let loadTableStateRequestId = 0; // Counter to discard stale responses
  let error = $state(null);
  /** The raw text behind a humane `error`, shown as a detail line, or null. */
  let errorDetail = $state(null);
  /** A control the toast offers for THIS `error` ({ label, message, run }), or null. */
  let errorAction = $state(null);
  let success = $state(null);
  /** The pending automatic re-read of the lobby after a failed one. */
  let lobbyRetryTimer = null;
  /** How many automatic re-reads have failed in a row (the back-off's input). */
  let lobbyRetryAttempt = 0;

  /**
   * THE ONE WAY A MESSAGE REACHES THE TOAST. The detail line and the action
   * belong to the message that raised them; a bare `error = ...` elsewhere
   * used to leave the previous failure's stack trace under an unrelated
   * sentence. Every caller goes through here, so the other two fields are
   * always the ones written for THIS message.
   */
  function showError(message, { detail = null, action = null } = {}) {
    error = message;
    errorDetail = detail;
    errorAction = action;
  }

  /** Dismiss stops the automatic re-read too: the empty list's Try again remains. */
  function dismissError() {
    error = null;
    errorDetail = null;
    errorAction = null;
    clearTimeout(lobbyRetryTimer);
    lobbyRetryTimer = null;
  }

  /** The lobby read failed: one sentence, the raw text demoted, a Retry, and a re-read on a growing timer. */
  function showLobbyFailure(e) {
    const retryMs = retryDelayMs(lobbyRetryAttempt);
    const failure = describeLobbyFailure(e, { retryMs });
    showError(failure.message, {
      detail: failure.detail,
      action: { label: 'Retry', message: failure.message, run: retryLoadTables },
    });
    clearTimeout(lobbyRetryTimer);
    lobbyRetryTimer = setTimeout(() => {
      lobbyRetryAttempt += 1;
      retryLoadTables();
    }, retryMs);
  }

  /** The spectator dock's one control: the same Internet Identity flow as the header. */
  async function signInFromTable() {
    try {
      await auth.login();
    } catch (e) {
      logger.error('sign-in from the table failed', e);
      showError('Could not open Internet Identity. Try the Sign in button in the header.');
    }
  }

  function retryLoadTables() {
    clearTimeout(lobbyRetryTimer);
    lobbyRetryTimer = null;
    if (view !== 'lobby') return;
    loadTables().then((ok) => { if (ok) openTableFromUrl(); });
  }
  let showProofPanel = $state(false);
  let showHandHistory = $state(false);
  /** The hand a `?table=…&hand=N` link asked for; the history dialog opens on it. */
  let historyOpenHand = $state(null);
  let showHowItWorks = $state(false);
  let showVerify = $state(false);

  // PRESENTATION of the five player-protection notices, never their content.
  // They ride the trust bar (TrustBar.svelte) on every view; this flag opens
  // the full verbatim text as an opaque OVERLAY (never an in-flow expansion:
  // the table sizes itself to the space under the bar and must not resize
  // under a player mid-hand).
  let noticesExpanded = $state(false);

  // HOW TALL THE PROTECTED-NOTICE BANNER IS, RIGHT NOW, IN CSS PIXELS.
  //
  // docs/DEFECTS.md E-52. `.toast` is `position: fixed` and used to be pinned at
  // `top: 80px`, which at 390x844 put a 534 px tall error panel straight over
  // `.alpha-warning-banner`: the no-rake property was covered at 9 of 9 sample
  // points and the other four protected phrases at 3 of 9. HARD RULE 2 says all
  // four must be ON SCREEN AND LEGIBLE at any viewport on any view, so a toast
  // that can paint over them is a hard-rule violation, not a cosmetic one.
  //
  // The toast is now anchored BELOW this banner instead of at a constant offset:
  // `.app` publishes the measured height as `--notice-safe-top` and `.toast`
  // starts there. That is enough on its own, at every scroll position: the banner
  // is the first thing in the flow, so it occupies viewport rows
  // [-scrollY, height - scrollY] and the toast starts at `height + gap`, which is
  // strictly below the banner's bottom edge for every scrollY >= 0. Scrolling can
  // only widen the gap, so no scroll listener is needed and there is no frame in
  // which a stale measurement overlaps.
  //
  // A second, independent lock is in the stylesheet: `.toast` paints BENEATH the
  // banner (z-index 90 vs 100). If this measurement were ever wrong, the notices
  // would still win the hit test that tools/shots/lib/protected-notices.mjs runs.
  let noticeBannerHeight = $state(0);

  // The `icp canister status` command for the network THIS bundle talks to.
  // On a mainnet build that is the live btc_table_1 with `-e ic`; on a local dev
  // build it is the local lobby with `-e local`. docs/DEFECTS.md T-02.
  const verifyStatusCommand = statusCommandFor(
    IS_MAINNET_BUILD ? MAINNET_CANISTER_IDS.btc_table_1 : lobbyCanisterId,
  );

  // ==========================================================================
  // THE HASH CHECK, RUN BY THE READER, AGAINST A SOURCE THAT IS NOT US
  // ==========================================================================
  //
  // The six canisters whose module hash says what code is running. `frontend` is
  // excluded on purpose: its module hash is the ASSET CANISTER's, which says
  // nothing about the bundle it happens to be serving, and listing it under
  // "is the deployed code the source?" would be the panel's fourth false claim.
  const VERIFIABLE_ROLES = ['lobby', 'history', 'table_1', 'table_2', 'table_3', 'btc_table_1'];

  const allHashCommand = liveHashCommandForAll(
    VERIFIABLE_ROLES.map((r) => MAINNET_CANISTER_IDS[r]),
  );

  /** @type {Record<string, {verdict:string, expected:string|null, live:string|null, error:string|null}>} */
  let liveHashes = $state({});
  let checkingHashes = $state(false);
  let hashCheckRan = $state(false);

  // Deliberately NOT on mount. This is a call to a third party, and a page that
  // silently contacts an index every time it loads has made a decision on the
  // reader's behalf. It is one button, and pressing it is the check.
  async function checkLiveHashes() {
    checkingHashes = true;
    hashCheckRan = true;
    try {
      const results = await Promise.all(
        VERIFIABLE_ROLES.map(async (role) => {
          const read = await readLiveModuleHash(MAINNET_CANISTER_IDS[role]);
          const cmp = compareHash(role, read.hash);
          return [role, { ...cmp, error: read.error }];
        }),
      );
      liveHashes = Object.fromEntries(results);
    } finally {
      checkingHashes = false;
    }
  }

  // One summary line, so the answer does not have to be assembled by eye from
  // six rows. Any mismatch dominates; any unknown beats "all match".
  const hashVerdict = $derived.by(() => {
    if (!hashCheckRan) return null;
    const rows = Object.values(liveHashes);
    if (rows.length === 0) return null;
    const mismatched = rows.filter((r) => r.verdict === 'mismatch');
    const unknown = rows.filter((r) => r.verdict === 'unknown' || r.verdict === 'no-expectation');
    if (mismatched.length) {
      return {
        kind: 'mismatch',
        text: `${mismatched.length} of ${rows.length} canisters are running code that is NOT `
          + 'what this page describes. Do not deposit until that is explained.',
      };
    }
    if (unknown.length) {
      return {
        kind: 'unknown',
        text: `${rows.length - unknown.length} of ${rows.length} confirmed; `
          + `${unknown.length} could not be read. Unread is not the same as matching.`,
      };
    }
    return {
      kind: 'match',
      text: `All ${rows.length} canisters are running the module hashes this page states.`,
    };
  });

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

  // There used to be a seventh private copy of "divide by 1e8 and round" here
  // (`formatICP`), called by nothing. Six live copies of that function is already
  // how the client came to display the same on-chain number differently in
  // different panels, and it is the soil docs/DEFECTS.md T-08 (the pot at 2x)
  // grew in. The canonical one now lives in $lib/utils.js as
  // `formatTokenAmount()`; import it rather than writing an eighth.

  /**
   * The stakes pill next to the Lobby button.
   *
   * WHY THIS IS NOT JUST `currentTableInfo.name`. The lobby canister seeds every
   * ICP table with table_1's config and bakes the blinds into the NAME string
   * (`init_microstakes_tables` in src/lobby_canister/src/lib.rs hardcodes
   * 1_000_000/2_000_000 for all three), while icp.yaml initialises table_2 at
   * 0.05/0.10 and table_3 at 0.10/0.20. So the name says "6-Max - 0.01/0.02" on
   * a table that charges 0.05/0.10, and "9-Max - 0.01/0.02" on one that charges
   * 0.10/0.20: wrong by 5x and 10x, in the largest teal string on the screen,
   * seven hundred pixels from the blind discs on the felt that are right.
   *
   * The lobby list already refuses to quote that record (it renders the STAKES
   * column from the table contract and flags the row "record differs"). This
   * makes the table header agree with the lobby list and with the felt: the
   * FORMAT half of the name is kept (that part is true), the stale price half is
   * dropped, and the blinds are read from the contract that will actually charge
   * them. Until the view arrives the pill shows the format alone rather than a
   * number nobody has checked. docs/DEFECTS.md T-11.
   */
  const tableFormatLabel = (name) =>
    String(name ?? '').split(/\s+[-–]\s+/)[0].trim() || String(name ?? '');

  const headerStakes = $derived.by(() => {
    const name = currentTableInfo?.name;
    if (!name) return null;
    const cfg = tableState?.config;
    if (!cfg) return tableFormatLabel(name);
    const currency = currencyOf(cfg) ?? 'ICP';
    const sb = formatTokenAmount(cfg.small_blind, { currency });
    const bb = formatTokenAmount(cfg.big_blind, { currency });
    return `${tableFormatLabel(name)} · ${sb}/${bb}`;
  });

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
  // The shortfall a Sit tap found, handed to the cashier as its opening figure
  // (smallest unit, or null). Cleared when the dialog closes.
  let depositPrefill = $state(null);
  // The seat an anonymous Sit tap wanted, resumed once sign-in lands (a NEW
  // object each time; never mutated). Keyed by table so a sign-in on another
  // table does not seat the player somewhere they did not tap.
  let resumeSeat = $state(null);
  // The header's measured height, published as `--header-h` so the phone's
  // toast can stand UNDER the header instead of over it (the audit measured
  // the error panel covering Lobby / History / Verify Fair at 390 px).
  let headerHeight = $state(0);

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

  /** Reads the lobby. Resolves true on a successful read, false on a failure (the toast says why). */
  async function loadTables() {
    loading = true;
    let ok = false;
    try {
      let lobbyTables = await lobby.get_tables();
      // A read that succeeds after a failed one clears that failure's toast.
      if (errorAction && errorAction.message === error) dismissError();
      lobbyRetryAttempt = 0;

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
      ok = true;
    } catch (e) {
      logger.error('Failed to load tables:', e);
      // Check if this is a signature verification error (expired II delegation)
      if (isSignatureError(e)) {
        showError('Session expired. Please log in again.');
        await auth.logout();
      } else {
        // Not the agent's paragraph: what happened and what happens next
        // (lib/humane-errors.js), the raw text on a detail line, a Retry.
        showLobbyFailure(e);
      }
    }
    loading = false;
    return ok;
  }

  async function joinTable(tableInfo) {
    // Check if the table has a canister assigned
    if (!tableInfo.canister_id || tableInfo.canister_id.length === 0) {
      showError("This table doesn't have an assigned canister yet");
      return;
    }

    // Store the table info and create an actor for this specific table canister
    currentTableInfo = tableInfo;
    // Convert Principal to string if needed
    const canisterId = tableInfo.canister_id[0].toText ? tableInfo.canister_id[0].toText() : tableInfo.canister_id[0].toString();
    tableActor = createTableActorProxy(canisterId);

    view = 'table';
    startPolling();
    rememberTableInUrl(canisterId);
  }

  /**
   * The invite link (lib/invite-link.js): the open table rides the URL so the
   * page can be shared, and a visit with `?table=<id>` opens that table once
   * the lobby has listed it. The URL is replaced, never pushed: Back still
   * leaves the app, as it did before.
   */
  function rememberTableInUrl(canisterId) {
    try {
      const next = searchWithTable(window.location.search, canisterId);
      if (next !== window.location.search) {
        window.history.replaceState(null, '', `${window.location.pathname}${next}`);
      }
    } catch (e) {
      logger.debug('could not update the URL for the open table', e);
    }
  }

  let deepLinkConsumed = false;

  /**
   * Runs after a SUCCESSFUL read only: a failed first read keeps its own
   * toast (the sentence, the detail line, the Retry) and the list's failed
   * state, and the link is honoured by the read that eventually succeeds.
   */
  function openTableFromUrl() {
    if (deepLinkConsumed || view !== 'lobby') return;
    let wanted = null;
    try { wanted = tableIdFromSearch(window.location.search); } catch { wanted = null; }
    if (!wanted) return;
    deepLinkConsumed = true;
    const match = tables.find((t) => {
      const p = t.canister_id?.[0];
      const cid = p ? (p.toText ? p.toText() : String(p)) : null;
      return cid === wanted;
    });
    if (match) {
      // The hand half of the link (lib/hand-link.js): the table opens, and the
      // history dialog opens on that hand once the table's records have landed.
      let hand = null;
      try { hand = handNumberFromSearch(window.location.search); } catch { hand = null; }
      joinTable(match);
      if (hand) {
        historyOpenHand = hand;
        showHandHistory = true;
      }
    } else {
      showError('That invite link points to a table this lobby does not list. Pick one below instead.');
      rememberTableInUrl(null);
    }
  }

  // =========================================================================
  // READING THE TABLE IS A QUERY. ADVANCING ITS CLOCK IS NOT. (docs/DEFECTS.md E-92)
  // =========================================================================
  //
  // This function used to open with `await tableActor.check_timeouts()`, and it
  // is driven by `setInterval(..., POLL_INTERVAL)` at 500 ms. `check_timeouts` is
  // `#[ic_cdk::update]` in `src/table_canister/src/lib.rs` and carries no `query`
  // in `table_canister.did`, so THE RENDER RATE WAS DRIVING AN UPDATE LOOP: one
  // per open browser tab, forever, whether or not anything was due.
  //
  // Measured on the local replica with `tools/cycles/tab-burn.mjs`, ten tabs open
  // on an idle table: the loop cost more than the whole rest of the canister put
  // together, and no cycles figure in this repository counted it. Every runway
  // number in the tree therefore read HIGH, the dangerous direction, and the
  // same failure [E-55](docs/DEFECTS.md#e-55) was reopened for one level down.
  //
  // So the two jobs are now two loops:
  //
  //   this function        QUERIES ONLY, at the render rate. It may never call an
  //                        update method. `tools/shots/test-poll-updates.mjs`
  //                        reads the committed .did and this file and fails if it
  //                        ever does again.
  //   advanceTableClock()  the update, on `$lib/clockNudge.js`'s policy: only when
  //                        a deadline is actually crossable, never faster than a
  //                        floor, backing off when calls change nothing.
  //
  // The `loadingTableState` re-entrancy guard is kept: a query is cheap for the
  // canister but not free for the browser, and overlapping polls still race.
  // A load asked for while one is in flight (the action-return re-read) runs
  // as soon as the in-flight one settles, rather than being dropped.
  let reloadWanted = false;

  async function loadTableState() {
    if (!tableActor) return;

    // Prevent concurrent loadTableState calls - queue one if already loading
    if (loadingTableState) { reloadWanted = true; return; }
    loadingTableState = true;

    // Track this request to discard stale responses
    const requestId = ++loadTableStateRequestId;

    try {
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
        // RECONCILE THE ECHO. A certified view that has absorbed the sent
        // action closes it; one that still shows the pre-click state (a
        // replica that has not seen the update) keeps it open and is
        // rendered with is_my_turn held false, so the dock cannot flip back
        // to "your turn" between the click and the commit (the flicker the
        // audit measured at one poll after every action).
        if (pendingAction) {
          if (pendingStatus(pendingAction, currentTableView) === 'absorbed') {
            pendingAction = null;
          } else if (pendingExpired(pendingAction, Date.now())) {
            pendingAction = null;
            showActionError('The table has not confirmed your action yet. It will show on the next update.', { verbatim: true });
          }
        }
        const nextView = projectPending(currentTableView, pendingAction);
        // Only update state if game data changed (timer ticks client-side)
        if (gameStateChanged(tableState, nextView)) {
          tableState = nextView;
        } else {
          // Still update time_remaining_secs for the client-side timer sync
          // This is a shallow merge - only update the timer field
          if (tableState && nextView.time_remaining_secs) {
            tableState = { ...tableState, time_remaining_secs: nextView.time_remaining_secs };
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
        showError('Session expired. Please log in again.');
        stopPolling();
        await auth.logout();
      }
    } finally {
      // Always reset loading flag to allow next poll
      loadingTableState = false;
      if (reloadWanted) {
        reloadWanted = false;
        loadTableState();
      }
    }
  }

  // Fast polling - 250 ms for responsive gameplay. QUERIES ONLY (E-92): a
  // query costs the canister nothing, and the in-flight guard in
  // loadTableState keeps overlapping polls from racing.
  const POLL_INTERVAL = 250;
  const HEARTBEAT_INTERVAL = 10000; // Send heartbeat every 10 seconds
  const BALANCE_REFRESH_INTERVAL = 5000; // Refresh balance every 5 seconds
  let actionPending = $state(false);
  // THE OPTIMISTIC ECHO ($lib/optimistic.js). Set before the update call is
  // awaited, rendered as fact by PokerTable, cleared when a certified view has
  // absorbed the action (or on Err, or after PENDING_TTL_MS as a lost send).
  let pendingAction = $state(null);
  // A canister refusal of the last action. Shown beside the buttons, not in
  // the page toast, and cleared after a few seconds.
  let actionError = $state(null);
  let actionErrorTimer = null;
  const ACTION_ERROR_MS = 6000;

  function showActionError(err, opts = {}) {
    actionError = humaneActionError(err, opts);
    if (actionErrorTimer) clearTimeout(actionErrorTimer);
    actionErrorTimer = setTimeout(() => { actionError = null; actionErrorTimer = null; }, ACTION_ERROR_MS);
  }

  function dismissActionError() {
    actionError = null;
    if (actionErrorTimer) { clearTimeout(actionErrorTimer); actionErrorTimer = null; }
  }
  let heartbeatInterval = null;
  let balanceRefreshInterval = null;
  let clockNudgeInterval = null;
  let clockPolicy = null;
  let clockNudgeInFlight = false;

  // =========================================================================
  // THE UPDATE HALF OF THE OLD POLL (docs/DEFECTS.md E-92)
  // =========================================================================
  //
  // `check_timeouts` is the only update the old 500 ms poll made, and it is still
  // needed for three things: an action clock the on-chain timer has not resolved,
  // the between-hands `AutoDealReady` signal (which the on-chain clock deliberately
  // never delivers to anybody by itself), and the stall opportunities that make
  // `abandon_stuck_hand` reachable when a timer is lost.
  //
  // What it is NOT needed for is a repaint. `$lib/clockNudge.js` holds the whole
  // decision (is anything due, and may we call yet) and it is pure, so
  // `tools/shots/test-clock-nudge.mjs` simulates a day of table states against it
  // offline and asserts a ceiling on the calls one tab can emit.
  //
  // `clockNudgeInFlight` is the same guard the poll has, for the same reason: an
  // update takes ~2 s to finalise on mainnet and the driver ticks every 2 s, so
  // without it a slow reply would queue calls the policy never authorised.
  //
  // The policy is re-read after the await rather than captured before it: leaving
  // the table clears it, and calling `observe` on the policy of a table this tab
  // is no longer at would teach the NEXT table's clock about this one's.
  async function advanceTableClock() {
    if (!tableActor || !clockPolicy || clockNudgeInFlight) return;
    const viewAtDecision = tableState;
    const decision = clockPolicy.decide(viewAtDecision, Date.now());
    if (!decision.call) return;

    clockNudgeInFlight = true;
    let result = null;
    try {
      result = await tableActor.check_timeouts();
    } catch (e) {
      // A failed nudge is still a nudge: `observe` below has to see it, or the
      // policy cannot back off on a canister that is refusing, which is the one
      // time backing off matters most. Not surfaced to the player: the poll's own
      // error path already reports a table that has stopped answering.
      logger.debug('check_timeouts failed:', e);
    } finally {
      clockNudgeInFlight = false;
      if (clockPolicy) clockPolicy.observe(viewAtDecision, result, Date.now());
    }

    if (!tableActor || !result) return;

    // Handle auto-deal if ready. Unchanged from the old poll except for where it
    // is called from: the canister decides whether a hand may start, this only
    // asks. The client-side phase guard stays because `start_new_hand` is a
    // separate message and the phase can move underneath it.
    if ('AutoDealReady' in result) {
      const currentPhase = tableState?.phase ? Object.keys(tableState.phase)[0] : null;
      const canStartHand = !currentPhase || currentPhase === 'WaitingForPlayers' || currentPhase === 'HandComplete';
      if (canStartHand) {
        try {
          playSound('deal');
          const startResult = await tableActor.start_new_hand();
          if ('Ok' in startResult) {
            shuffleProof = startResult.Ok;
          } else if ('Err' in startResult) {
            // Silently ignore expected race condition errors
            const errMsg = startResult.Err.toLowerCase();
            if (!errMsg.includes('active players') && !errMsg.includes('already in progress') && !errMsg.includes('in progress')) {
              logger.error('Auto-deal failed:', startResult.Err);
            }
          }
        } catch (e) {
          // Silently ignore expected race condition errors
          const errMsg = (e.message || e.toString() || '').toLowerCase();
          if (!errMsg.includes('already in progress') && !errMsg.includes('in progress')) {
            logger.error('Auto-deal failed:', e);
          }
        }
        // Show the new hand immediately rather than up to POLL_INTERVAL later.
        loadTableState();
      }
    }
  }

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
    // The clock, on its own loop and its own rate (docs/DEFECTS.md E-92). A fresh
    // policy per table: the floor and the backoff are state about THIS table, and
    // carrying them across a table change would let one table's stall silence the
    // next table's clock.
    clockPolicy = new ClockNudgePolicy();
    clockNudgeInterval = setInterval(advanceTableClock, CLOCK_NUDGE.TICK_MS);
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
    if (clockNudgeInterval) {
      clearInterval(clockNudgeInterval);
      clockNudgeInterval = null;
    }
    clockPolicy = null;
  }

  // The escrow at this table, as last READ. Null until `get_balance()` has
  // answered and null again after a failed read: the join gate treats null as
  // unknown, not as zero (lib/join-gate.js escrowFromRead), and the dock shows
  // no figure it has not been given.
  let myBalance = $state(null);
  async function loadBalance() {
    if (!tableActor) return;
    try {
      myBalance = escrowFromRead({ ok: true, value: await tableActor.get_balance() });
    } catch (e) {
      logger.error('Failed to load balance:', e);
      myBalance = escrowFromRead({ ok: false });
    }
  }

  // Load balance when entering table view
  $effect(() => {
    if (view === 'table' && tableActor) {
      loadBalance();
    }
  });

  const PLAYER_ACTION_VARIANTS = {
    fold: () => ({ Fold: null }),
    check: () => ({ Check: null }),
    call: () => ({ Call: null }),
    raise: (amount) => ({ Raise: BigInt(amount) }),
    bet: (amount) => ({ Bet: BigInt(amount) }),
    allin: () => ({ AllIn: null }),
  };

  // THE DECISION LOOP'S UPDATE. The echo is set BEFORE the await so the felt
  // shows the action at the click; the certified view is never written to in
  // place (the old code mutated tableState.is_my_turn, which the next poll
  // undid). On Err the echo is dropped, the certified figures are back on the
  // next paint, and the reason lands beside the buttons.
  async function sendPlayerAction(kind, amount) {
    const variant = PLAYER_ACTION_VARIANTS[kind];
    if (!variant) return null;
    if ((kind === 'raise' || kind === 'bet') && !(Number(amount) > 0)) return null;
    dismissActionError();
    pendingAction = beginPending(kind, kind === 'raise' || kind === 'bet' ? amount : null, tableState, Date.now());
    if (pendingAction && tableState) tableState = projectPending(tableState, pendingAction);
    playSound(kind);
    let result;
    try {
      result = await tableActor.player_action(variant(amount));
    } catch (e) {
      pendingAction = null;
      throw e;
    }
    if ('Err' in result) {
      pendingAction = null;
      showActionError(result.Err);
      playSound('error');
    }
    return result;
  }

  /** The key `resumeSeat` is remembered under: this table's canister id. */
  function currentTableKey() {
    const cid = currentTableInfo?.canister_id?.[0];
    if (cid?.toText) return cid.toText();
    return String(cid ?? currentTableInfo?.id ?? '');
  }

  // THE RESUME. When sign-in lands and a seat was remembered for THIS table,
  // refresh the balances (the gate reads escrow) and tap the seat again on
  // the player's behalf. `untrack` keeps the effect keyed to the sign-in edge
  // and the remembered seat only.
  $effect(() => {
    const authed = $auth.isAuthenticated;
    const seat = seatToResume(resumeSeat, untrack(currentTableKey));
    if (!authed || !seat) return;
    resumeSeat = null;
    untrack(() => {
      refreshAllBalances()
        .then(() => handleTableAction('join', seat.seat))
        .catch((e) => { showError(e?.message || 'Could not take the seat after sign-in.'); });
    });
  });

  async function handleTableAction(action, data) {
    if (actionPending || !tableActor) return;
    actionPending = true;

    try {
      let result;
      switch (action) {
        case 'join': {
          // THE GATE. Anonymous: open sign-in and remember the seat. Under the
          // buy-in: open the cashier with the shortfall filled in. Only a
          // signed-in, funded tap reaches the canister (the canister still
          // decides; this only stops the two taps whose answer is known).
          // The facts are CERTIFIED ones: the table canister's own config
          // (never the lobby record, T-11) and the escrow as last read.
          const decision = joinGateDecision(joinGateFacts({
            isAuthenticated: $auth.isAuthenticated,
            escrow: myBalance,
            tableView: tableState,
          }));
          if (decision.kind === 'login') {
            resumeSeat = { seat: data, tableKey: currentTableKey() };
            try {
              await auth.login();
            } catch (e) {
              resumeSeat = null;
              showError(e?.message || 'Sign-in did not complete.');
            }
            break;
          }
          if (decision.kind === 'deposit') {
            depositPrefill = decision.shortfall;
            showDepositModal = true;
            break;
          }
          result = await tableActor.join_table(data);
          if ('Err' in result) {
            showError(result.Err);
          } else {
            success = `Joined seat ${data + 1}!`;
            await refreshAllBalances();
          }
          break;
        }

        case 'fold':
        case 'check':
        case 'call':
        case 'raise':
        case 'bet':
        case 'allin':
          result = await sendPlayerAction(action, data);
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
              showError(result.Err);
              playSound('error');
            }
            // The UI already shows "Need 2+ players to start" hint
          }
          break;

        case 'useTimeBank':
          result = await tableActor.use_time_bank();
          if ('Err' in result) {
            showError(result.Err);
          } else {
            success = 'Using time bank';
          }
          break;

        case 'sitOut':
          result = await tableActor.sit_out();
          if ('Err' in result) {
            showError(result.Err);
          } else {
            success = 'Sitting out next hand';
          }
          break;

        case 'sitIn':
          result = await tableActor.sit_in();
          if ('Err' in result) {
            showError(result.Err);
          } else {
            success = 'Back in the game';
          }
          break;

        case 'leave':
          result = await tableActor.leave_table();
          if ('Err' in result) {
            showError(result.Err);
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

      // The action-return re-read: the felt settles when the call returns,
      // not up to a poll later (docs/RESPONSIVENESS.md section 5).
      await loadTableState();
    } catch (e) {
      logger.error(`Action ${action} failed:`, e);
      pendingAction = null;
      // Check if this is a signature verification error (expired II delegation)
      if (isSignatureError(e)) {
        showError('Session expired. Please log in again.');
        stopPolling();
        await auth.logout();
      } else if (PLAYER_ACTION_VARIANTS[action]) {
        // A throw, not a canister Err: the update may have landed. Say so.
        showActionError(e, { refusal: false });
        playSound('error');
      } else {
        showError(e.message || 'Action failed');
      }
    } finally {
      actionPending = false;
    }
  }

  function backToLobby() {
    stopPolling();
    view = 'lobby';
    pendingAction = null;
    dismissActionError();
    tableState = null;
    myCards = null;
    shuffleProof = null;
    currentTableInfo = null;
    tableActor = null;
    rememberTableInUrl(null);
    loadTables(); // Refresh tables list
  }

  // Load tables on mount and ensure cleanup on unmount
  $effect(() => {
    loadTables().then((ok) => { if (ok) openTableFromUrl(); });
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
      // The clock nudger is the one loop here that sends UPDATE calls, so a leaked
      // one costs cycles rather than bandwidth (docs/DEFECTS.md E-92).
      if (clockNudgeInterval) {
        clearInterval(clockNudgeInterval);
        clockNudgeInterval = null;
      }
    };
  });
</script>

<!-- `--notice-safe-top` is the measured height of the protected-notice banner.
     Everything that floats over the page reads it so that nothing can be
     positioned on top of the notices (docs/DEFECTS.md E-52). -->
<div class="app" class:on-table={view === 'table'} style="--notice-safe-top: {noticeBannerHeight}px; --header-h: {headerHeight}px">
  <!-- Ambient background: ONE static gradient (the audit retired the three
       animated blur orbs), and none at all behind the table, where the stage
       paints its own single pool of light. -->
  <div class="bg-effects"></div>

  <!--
    THE FIVE NOTICES, ONCE PER PAGE, ON EVERY VIEW, AT EVERY VIEWPORT.

    `.alpha-warning-banner` is the sticky carrier of the trust bar
    (TrustBar.svelte): every protected phrase verbatim, FULL TERMS opening the
    complete text. It is measured here (`--notice-safe-top`, which anchors the
    toast below it) and it is the element whose scope class the screenshot
    harness reads for the toast it injects (Toast.svelte's rules are global,
    so that node and the real toast are painted alike), so it stays here.

    The words are protected by `make hygiene` (source) and by
    tools/shots/lib/protected-notices.mjs (rendered pixels, hit-tested on every
    scene at both viewports, with an error toast up). Making them MORE
    prominent is always allowed; shortening, hiding, or gating them behind a
    click is not.
  -->
  <div
    class="alpha-warning-banner"
    class:expanded={noticesExpanded}
    bind:clientHeight={noticeBannerHeight}
  >
    <TrustBar
      expanded={noticesExpanded}
      onExpand={() => { noticesExpanded = true; }}
      onClose={() => { noticesExpanded = false; }}
    />
  </div>

  <header class:compact={view === 'table'} bind:clientHeight={headerHeight}>
    <div class="header-left">
      {#if view === 'table'}
        <!-- The labels are spans so the phone's one-row header can keep the
             glyph and drop the word without a second markup path. -->
        <button class="back-btn" onclick={backToLobby} aria-label="Back to the lobby" title="Lobby">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7"/>
          </svg>
          <span class="btn-label">Lobby</span>
        </button>
        {#if currentTableInfo}
          <span class="current-table-name">{headerStakes}</span>
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
        <!-- WHICH CHAIN THIS PAGE IS TALKING TO, ON THE SAME LINE AS THE
             TAGLINE. On the same line on purpose: it costs no vertical space, and
             every pixel above the table is a pixel of felt (docs/DEFECTS.md T-19,
             docs/WAVE-04.md). The value is COMPILED IN by the build
             (ic-config.js NETWORK), not sniffed from the hostname, so it agrees
             with the canisters this bundle is actually wired to by construction:
             the same constant decides both. -->
        <span class="tagline">
          <span class="tagline-text">Provably Fair Poker</span>
          <span class="net-chip" class:mainnet={IS_MAINNET_BUILD} data-network={NETWORK}>
            {IS_MAINNET_BUILD ? 'IC mainnet · real funds' : `${NETWORK} build · test funds`}
          </span>
        </span>
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
        <button class="history-btn" onclick={() => { historyOpenHand = null; showHandHistory = true; }} aria-label="Hand history" title="Hand history">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <polyline points="12,6 12,12 16,14"/>
          </svg>
          <span class="btn-label">History</span>
        </button>
        <button class="verify-btn" onclick={() => showProofPanel = !showProofPanel} aria-label="Verify fair" title="Verify fair">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            <path d="M9 12l2 2 4-4"/>
          </svg>
          <span class="btn-label">Verify Fair</span>
        </button>
      {/if}
      <WalletButton onProfileChange={handleProfileChange} />
    </div>
  </header>

  <Toast kind="error" message={error} detail={errorDetail} action={errorAction && errorAction.message === error ? errorAction : null} onDismiss={dismissError} />
  <Toast kind="success" message={success} />

  <!--
    docs/DEFECTS.md H-09. The "Loading tables..." block used to be a SIBLING of
    <Lobby>, not an either/or branch, so while `loading` was true the spinner and
    the fully rendered lobby were both on screen: ~230 px of layout that was in a
    screenshot or not depending on when the shutter fired.

    It is now a real either/or, and the spinner only stands in when there is
    genuinely nothing to show yet (`tables.length === 0`), a background refresh
    of an already-populated lobby must not blank the list.

    `data-lobby-state` exposes the settled/unsettled distinction to the
    screenshot harness so it can wait on real state instead of a sleep.
  -->
  <main
    data-view={view}
    data-lobby-state={view !== 'lobby' ? 'n-a' : (loading && tables.length === 0 ? 'loading' : 'ready')}
    data-lobby-tables={view === 'lobby' ? tables.length : ''}
  >
    {#if view === 'lobby'}
      {#if loading && tables.length === 0}
        <div class="loading-state">
          <div class="spinner"></div>
          <span>Loading tables...</span>
        </div>
      {:else}
        <Lobby
          {tables}
          onJoinTable={joinTable}
          onRefresh={loadTables}
          loadFailed={Boolean(errorAction) && errorAction.message === error}
        />
      {/if}
    {:else}
      {@const tableCurrency = getTableCurrency(currentTableInfo)}
      <div class="game-layout">
        <div class="table-area">
          <PokerTable
            {tableState}
            {myCards}
            {actionPending}
            {pendingAction}
            {actionError}
            onDismissError={dismissActionError}
            maxPlayers={Number(currentTableInfo?.config?.max_players ?? 0) || null}
            onAction={handleTableAction}
            tableBalance={myBalance}
            currency={tableCurrency}
            avatarStyle={currentAvatarStyle}
            customName={currentCustomName}
            onShowDeposit={() => showDepositModal = true}
            onShowWithdraw={() => showWithdrawModal = true}
            signedIn={$auth.isAuthenticated}
            onSignIn={signInFromTable}
            {soundMuted}
            onToggleSound={toggleSound}
            {shuffleProof}
            onShowProof={() => showProofPanel = true}
            onHowItWorks={() => { showHowItWorks = true; }}
            onVerifyCode={() => { showVerify = true; }}
          />
          <!-- The phone held sideways: over the table area only, so the trust
               bar and the header stay on screen (RotatePrompt.svelte). -->
          <RotatePrompt />
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
              tableId={currentTableInfo?.canister_id?.[0]}
              {tableActor}
            />
          </aside>
        {/if}
      </div>
    {/if}
  </main>

  <footer>
    <!-- Provenance, not a second copy of the notices. The five protected
         phrases ride the sticky trust bar at the top of every view (TrustBar
         .svelte), which is on screen at every scroll position, so the footer
         no longer repeats them; it says who built this and links the terms. -->
    <div class="footer-provenance">
      <p>
        Built with AI, in the open. Every table is a canister you can query while signed out.
        <button class="footer-link inline" onclick={() => noticesExpanded = true}>Full terms</button>
      </p>
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
        <span class="footer-divider">|</span>
        <!-- The second statement of the target network, at the other end of the
             page from the first, and this one names the canister the app is
             actually wired to rather than only the chain. -->
        <!-- The id is inside `<code class="canister-id">` deliberately. The
             screenshot harness's token census requires every numeric token on
             screen to be matched to a canister figure or excused by a REVIEWED
             rule, and a bare principal in a <span> is four unexplained numbers
             ("4", "5", "777", "77775") on every scene: it failed the census on
             all 24 shots the first time this shipped. `token-allowlist.mjs`
             already has the right rule (`identifier-digits`, scoped to
             `.canister-id` among others), so this reuses it rather than widening
             the one escape hatch the inverted gate has. -->
        <span class="net-footer" class:mainnet={IS_MAINNET_BUILD} data-network={NETWORK}>
          {IS_MAINNET_BUILD ? 'IC mainnet' : NETWORK} · lobby
          <code class="canister-id">{lobbyCanisterId ?? 'unwired'}</code>
        </span>
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
    tableName={currentTableInfo?.name || 'ClearDeck table'}
    openHand={historyOpenHand}
    onClose={() => { showHandHistory = false; historyOpenHand = null; }}
  />
{/if}

{#if showDepositModal}
  <DepositModal
    {tableActor}
    tableCanisterId={currentTableInfo?.canister_id?.[0]}
    currency={getTableCurrency(currentTableInfo)}
    initialAmount={depositPrefill}
    minBuyIn={tableState?.config?.min_buy_in ?? null}
    onClose={() => { showDepositModal = false; depositPrefill = null; }}
    onDepositSuccess={() => { refreshAllBalances(); loadTableState(); }}
  />
{/if}

{#if showWithdrawModal}
  <WithdrawModal
    {tableActor}
    tableCanisterId={currentTableInfo?.canister_id?.[0]}
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
  <div class="verify-modal" role="dialog" aria-modal="true" aria-labelledby="verify-modal-title">
    <button class="close-btn" onclick={() => showVerify = false}>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>

  <div class="verify-scroll">
    <h2 id="verify-modal-title">Verify the Code</h2>
    <p class="verify-intro">
      Every canister on the Internet Computer has a publicly visible module hash.
      You can check that what is running is what this source builds to, without
      trusting this page for any of it.
    </p>

    <!-- WHAT THIS PAGE IS TALKING TO. Sourced from the same constants that wire
         the actors (ic-config.js / canisters.js), so it cannot disagree with the
         connection it describes. docs/DEFECTS.md T-01, T-02. -->
    <div class="verify-section wiring" data-network={NETWORK}>
      <h3>0. What this page is connected to</h3>
      <dl class="wiring-list">
        <div><dt>Network</dt><dd class:live={IS_MAINNET_BUILD}>
          {IS_MAINNET_BUILD ? 'Internet Computer mainnet: REAL funds' : `${NETWORK}: test funds only`}
        </dd></div>
        <div><dt>Gateway</dt><dd><code>{agentHost()}</code></dd></div>
        <div><dt>Sign-in</dt><dd><code>{IS_MAINNET_BUILD ? II_URL : 'local Internet Identity'}</code></dd></div>
        <div><dt>Lobby canister</dt><dd><code>{lobbyCanisterId ?? 'unwired'}</code></dd></div>
        <div><dt>History canister</dt><dd><code>{historyCanisterId ?? 'unwired'}</code></dd></div>
      </dl>
      {#if !IS_MAINNET_BUILD}
        <p class="hash-note">
          This is a <strong>{NETWORK}</strong> development build. It cannot reach the live
          canisters (the build refuses to wire them, docs/DEFECTS.md T-01), so nothing
          you do here moves real money. The mainnet ids below are shown for reference.
        </p>
      {/if}
    </div>

    <div class="verify-section">
      <h3>1. Read the hash that is running, right now</h3>
      <p>
        Anyone can read a canister's module hash from the public dashboard index. No
        identity, no controller rights, no wallet:
      </p>
      <div class="code-block">
        <code>{allHashCommand}</code>
        <button class="copy-btn" onclick={() => navigator.clipboard.writeText(allHashCommand)}>Copy</button>
      </div>
      <p class="hash-note">
        <code>icp canister status &lt;ID&gt; -e ic</code> reads the same value, but
        <code>canister_status</code> is a controller-only management call on mainnet,
        so it will refuse for anyone who is not an operator of this project. That is
        why the command above is the one printed here.
        {#if !IS_MAINNET_BUILD}
          On this build, <code>{verifyStatusCommand}</code> reads your own replica.
        {/if}
      </p>
    </div>

    <div class="verify-section">
      <h3>2. Build the source and compare</h3>
      <p>
        This rebuilds every canister in the digest-pinned container the mainnet fleet
        is deployed from, and compares each module against the live hash:
      </p>
      <div class="code-block">
        <code>{REBUILD_COMMAND}</code>
        <button class="copy-btn" onclick={() => navigator.clipboard.writeText(REBUILD_COMMAND)}>Copy</button>
      </div>
      <p class="hash-note">
        If they match, the code running is the code in the repository. If they do not,
        nothing else on this page means anything.
      </p>
    </div>

    <!-- THE HASH TABLE, AS A CLAIM WITH A DATE AND A CHECK BESIDE IT.
         This block used to print three hashes under the heading "Deployed
         Canister Hashes" with no date, no source and no comparison, and they were
         three upgrades stale. A number nobody checks is not verification; it is
         the thing a careful reader checks INSTEAD of verifying. -->
    <div class="canister-ids">
      <h3>3. Expected module hashes, and the live reading</h3>
      <p class="hash-note provenance">
        <strong>These are a claim, not a measurement.</strong> Declared
        {EXPECTED_PROVENANCE.declaredOn} by {EXPECTED_PROVENANCE.declaredBy}:
        {EXPECTED_PROVENANCE.claim}. This page was built on a machine that
        {EXPECTED_PROVENANCE.whyNot}, so press the button and compare for yourself:
        the reading comes from {LIVE_HASH_SOURCE.name}, which is not us.
      </p>

      <button class="hash-check-btn" onclick={checkLiveHashes} disabled={checkingHashes}>
        {checkingHashes ? 'Reading the live hashes…' : 'Check the live hashes now'}
      </button>

      {#if hashVerdict}
        <p class="hash-verdict {hashVerdict.kind}" data-hash-verdict={hashVerdict.kind}>
          {hashVerdict.text}
        </p>
      {/if}

      <table>
        <tbody>
          {#each VERIFIABLE_ROLES as role}
            {@const live = liveHashes[role]}
            <tr>
              <td>{role}</td>
              <td><code class="canister-id">{MAINNET_CANISTER_IDS[role]}</code></td>
            </tr>
            <tr>
              <td colspan="2" class="hash-row">
                <code class="hash">expected {displayHash(EXPECTED_MODULE_HASHES[role])}</code>
              </td>
            </tr>
            {#if live}
              <tr>
                <td colspan="2" class="hash-row">
                  <code class="hash live-{live.verdict}">
                    {#if live.live}
                      live&nbsp;&nbsp;&nbsp;&nbsp; {displayHash(live.live)}
                      {live.verdict === 'match' ? '  ✓ match' : '  ✗ MISMATCH'}
                    {:else}
                      live&nbsp;&nbsp;&nbsp;&nbsp; could not be read: {live.error ?? 'unknown'}
                    {/if}
                  </code>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
      <p class="hash-note">
        All four tables run the same module, so they are expected to share one hash.
      </p>
    </div>

    <a href="https://github.com/JoshDFN/cleardeck" target="_blank" rel="noopener" class="github-link">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
      </svg>
      View Source on GitHub
    </a>
  </div>

    <!-- THE FOUR PROTECTED NOTICES, INSIDE THIS DIALOG (HARD RULE 2).
         docs/DEFECTS.md T-40, found by rendering the mainnet bundle and
         hit-testing each phrase on its own pixels. Measured at 1440x900 AND
         390x844, with this dialog open:

           .banner-warning     y -325  (the page auto-scrolls to the footer link
                                        that opens this dialog, so the top banner
                                        is above the fold)
           .disclaimer-warning y  674  in the viewport, and under
                                       `.modal-backdrop`: rgba(0,0,0,0.8) plus a
                                       4 px blur at z-index 1000
           .strip-text         display:none (portrait table strip, not this view)

         0 of 5 phrases legible, at both viewports. Every other dialog in this
         app was fixed for exactly this (DepositModal, WithdrawModal, HowItWorks
         docs/DEFECTS.md T-31; HandHistory H-36) and the one dialog whose entire
         subject is "can you trust this deployment" was missed.

         Same remedy, same reasoning: restated verbatim, pinned OUTSIDE the
         scrolling region so arriving at the dialog is enough to have read them,
         additional copy only, nothing anywhere else weakened. -->
    <p class="modal-notices">
      <span class="notice-icon" aria-hidden="true">⚠️</span>
      <NoticeLine />
    </p>
  </div>
{/if}

<style lang="scss">
  /* The table view's phone header lives in a partial next to this file,
     included below where its rules used to sit (the cascade is
     order-sensitive) and scoped by Svelte like everything else. */
  @use './table-header-phone' as phone;
  @use './app-phone' as app-phone;

  :global(*) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    background: var(--cd-bg);
    min-height: 100vh;
    color: var(--cd-ink-1);
    font-family: var(--cd-font-ui);
    overflow-x: hidden;
  }

  /* THE TRUST BAR'S CARRIER. The words are protected (docs/DESIGN-BAR.md
     section 7, tools/shots/lib/protected-notices.mjs); the carrier is not. One
     neutral bar with a single amber rule, on every view, STICKY so it is on
     screen at every scroll position of the lobby (which is why the footer no
     longer repeats it). The content is TrustBar.svelte; this element stays
     here because it is measured (`--notice-safe-top`) and the screenshot
     harness reads its scope class to style the toast it injects. */
  .alpha-warning-banner {
    position: sticky;
    top: 0;
    z-index: 100;
    padding: 0;
    background: var(--cd-plate);
    color: var(--cd-ink-1);
    border-top: 3px solid var(--cd-warn);
    border-bottom: 1px solid var(--cd-line);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.25);
  }

  /* Expanded: the overlay is the bar's child, and the bar is a stacking
     context, so the bar itself rises above every dialog while the terms are
     open. */
  .alpha-warning-banner.expanded {
    z-index: 2000;
  }

  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
  }

  /* Ambient background: one static gradient. The three 400-600 px blur(100px)
     orbs that animated for 20-25 s behind every page are gone (they were two
     contradictory light sources behind a felt that has its own), and on the
     table view the stage paints the room, so this is hidden outright. */
  .bg-effects {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 0;
    background:
      radial-gradient(ellipse 60% 50% at 20% 0%, var(--cd-accent-dim), transparent 70%),
      radial-gradient(ellipse 50% 40% at 100% 100%, var(--cd-surface-2), transparent 70%);
  }

  .app.on-table .bg-effects { display: none; }

  /* Header */
  header {
    position: relative;
    z-index: 50;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 24px;
    background: var(--cd-bg);
    border-bottom: 1px solid var(--cd-line-soft);
  }

  /* THE TABLE VIEW'S HEADER IS ONE SLIM BAR. Every pixel above the felt is a
     pixel of felt (docs/DEFECTS.md T-19): 52 px, brand mark and name only, the
     network chip beside the name, controls at one height and one radius. */
  header.compact {
    min-height: 52px;
    padding: 6px 16px;
  }

  header.compact .logo { gap: 10px; }
  header.compact .logo-mark { width: 30px; height: 30px; border-radius: var(--cd-radius-chip); }
  header.compact .suit { font-size: 13px; }
  header.compact .suit-1 { top: 4px; left: 6px; }
  header.compact .suit-2 { bottom: 4px; right: 6px; }
  header.compact .brand { font-size: 17px; }
  header.compact .logo-text { flex-direction: row; align-items: center; gap: 8px; }
  header.compact .tagline-text { display: none; }

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
    color: var(--cd-accent);
    font-weight: var(--cd-weight-strong);
    font-size: var(--cd-text-sm);
    font-variant-numeric: tabular-nums;
    padding: 6px 12px;
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-chip);
    white-space: nowrap;
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
    background: var(--cd-accent);
    border-radius: var(--cd-radius-card);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 20px var(--cd-accent-glow);
  }

  .suit {
    position: absolute;
    font-size: 18px;
    color: var(--cd-ink);
  }

  .suit-1 {
    top: 6px;
    left: 8px;
  }

  .suit-2 {
    bottom: 6px;
    right: 8px;
    color: var(--cd-accent-ink);
  }

  .logo-text {
    display: flex;
    flex-direction: column;
  }

  .brand {
    font-size: 22px;
    font-weight: var(--cd-weight-figure);
    color: var(--cd-ink);
    letter-spacing: -0.5px;
  }

  .tagline {
    font-size: var(--cd-text-xs);
    color: var(--cd-accent);
    text-transform: uppercase;
    letter-spacing: 1.5px;
    font-weight: var(--cd-weight-medium);
  }

  /* ONE control primitive for the header (the audit counted three heights,
     two radii and two accent families on this row): 40 px, one radius, quiet
     for navigation and the sound toggle, outline teal for the two tools.
     History and Verify Fair are the same button; indigo is gone. */
  .back-btn,
  .verify-btn,
  .history-btn,
  .sound-toggle-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: var(--cd-control-md);
    padding: 0 14px;
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    line-height: 1;
    cursor: pointer;
    transition: background-color var(--cd-fast) var(--cd-ease),
                border-color var(--cd-fast) var(--cd-ease),
                color var(--cd-fast) var(--cd-ease);
  }

  .back-btn:hover,
  .sound-toggle-btn:hover {
    background: var(--cd-surface-3);
    color: var(--cd-ink);
  }

  .verify-btn, .history-btn {
    background: var(--cd-accent-dim);
    border-color: var(--cd-accent-line);
    color: var(--cd-accent);
  }

  .verify-btn:hover, .history-btn:hover {
    border-color: var(--cd-accent-line-strong);
  }

  .sound-toggle-btn {
    width: var(--cd-control-md);
    padding: 0;
    color: var(--cd-ink-2);
  }

  /* The toast (Toast.svelte) is not styled here: its rules are global by
     design so the harness's injected node and the real toast are painted by
     the same declarations. Its place under the trust bar reads
     `--notice-safe-top`, published by `.app` above. */

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

  /* The stage fills the frame: 8 px above, nothing below that the wrapper
     does not already cancel (measureViewport reads the parent's bottom padding
     as --cd-slack). */
  main[data-view='table'] .game-layout { min-height: 0; padding-bottom: 0; }
  main[data-view='table'] .table-area { padding: 8px 16px 8px; }

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
    /* Above `.toast` (90), as it was when it carried a copy of the notices
       (docs/DEFECTS.md E-52); the trust bar is sticky now and the footer only
       carries provenance, but the layer is kept so nothing floats over the
       footer's links. Both money dialogs still cover it from z-index 200. */
    z-index: 95;
    display: flex;
    flex-direction: column;
    background: rgba(10, 10, 15, 0.8);
    backdrop-filter: blur(20px);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 13px;
  }

  .footer-provenance {
    padding: var(--cd-space-4) var(--cd-space-6) 0;
  }

  .footer-provenance p {
    max-width: 800px;
    margin: 0 auto;
    font-size: var(--cd-text-sm);
    line-height: 1.5;
    color: var(--cd-ink-2);
  }

  .footer-link.inline {
    margin-left: var(--cd-space-2);
    color: var(--cd-accent);
    text-decoration: underline;
    text-underline-offset: 3px;
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
    /* THE PHONE LOBBY HEADER IS ONE ROW: mark and brand at the left, the
       sound toggle and Sign in at the right (the table view's own one-row
       header is table-header-phone.scss). It was a centred logo over a second
       row of controls, ~120 px; one row is ~64, and every px above the first
       table card is a px the five-second test pays for. */
    header {
      padding: var(--cd-space-2) var(--cd-space-3);
      flex-wrap: nowrap;
      gap: var(--cd-space-2);
    }

    .header-left, .header-right {
      min-width: auto;
      flex: 0 1 auto;
    }

    header:not(.compact) .header-left { display: none; }

    .logo {
      flex: 1 1 auto;
      min-width: 0;
      gap: var(--cd-space-2);
    }

    header:not(.compact) .logo-mark { width: 36px; height: 36px; }
    header:not(.compact) .suit { font-size: 15px; }
    header:not(.compact) .brand { font-size: 18px; }
    header:not(.compact) .tagline-text { display: none; }

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
      /* UNDER the trust bar and the nav, not beneath them: with `inset: 0`
         the panel's own header (Fairness Proof, the close control) and the
         top of the verdict card sat behind the sticky chrome and the first
         screen of the phone showed no way to close it. Both heights are
         measured and published by `.app`. */
      inset: calc(var(--notice-safe-top, 0px) + var(--header-h, 0px)) 0 0 0;
      width: 100%;
      /* Above `footer`, which E-52 raised from 10 to 95 so that the copy of the
         notices a desktop player reads after scrolling wins its own hit test.
         This panel is `inset: 0` in portrait, so without this it would be the
         one full-screen overlay in the app that the footer bar paints through.
         Still below `.alpha-warning-banner` (100), exactly as it was at 50. */
      z-index: 96;
    }

    .footer-provenance { padding: var(--cd-space-3) var(--cd-space-4) 0; }

    .footer-bottom {
      flex-direction: column;
      gap: 10px;
      padding: 10px 16px;
    }

    /* How It Works and Verify Code on ONE row; the divider between them
       was a line of its own on the phone. */
    .footer-center {
      flex-direction: row;
      justify-content: center;
      gap: var(--cd-space-5);
    }

    .footer-center .footer-divider { display: none; }
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

  /* =========================================================================
     THE TABLE VIEW ON A PHONE (portrait, or any window under 560 px tall):
     WIN BACK THE PLAYING SURFACE FROM CHROME, NOT FROM THE NOTICES.
     =========================================================================

     `PokerTable.svelte` sizes itself to "the viewport, less everything above me
     in the flow", so on a 390x844 phone every pixel this block does not spend is
     a pixel of felt. Measured before this block, on the real table view:

       .alpha-warning-banner   268.0 px   four notices, full text, in the flow
       header                  173.5 px   THREE rows: brand / left / right,
                                          because .header-right needed 411.8 px
                                          and could not share a row with anything
       .table-area padding       8.0 px
       ------------------------------------------------------------------------
       above the table         449.5 px of a 918 px layout viewport (49%)

     and the felt came out 199.1 x 358.7 = 18.3% of the screen, against
     PokerNow's 52.1% on the same device.

     Three things happen here, none of which takes a word away from a player:

     1. THE BRAND ROW GOES, on the table view only. A 44 px row telling the
        player which app they are already playing in.
     2. THE HEADER BECOMES TWO SHORT ROWS instead of three, and every control in
        it comes down one type step so `.header-right` fits the device width.
        That last part is load-bearing for `initial-scale=1` in `app.html`: the
        moment anything in the document is wider than 390 px, Chrome either
        clips it or (with no pinned scale) shrinks the WHOLE PAGE to fit, which
        is the 0.918 zoom-out of docs/DEFECTS.md T-19.
     3. THE FOUR NOTICES BECOME A STRIP THAT STILL SAYS ALL FOUR THINGS, one tap
        from the full text. Nothing is shortened; `.strip-text` quotes the
        protected phrases verbatim.

     Everything here is scoped to `header.compact` / `.alpha-warning-banner
     .on-table`, i.e. THE TABLE VIEW ONLY. The lobby still gets the full banner
     in the flow at every viewport, because on the lobby nothing is competing for
     the space.

     THE SECOND CONDITION, `max-height: 560px`, IS THE PHONE HELD SIDEWAYS.
     Measured at 844x390 before it: 160 px of banner plus a 99 px header left
     123 px for a poker table, so `MIN_AVAIL_LANDSCAPE` took over and put the
     felt at `y 302..553` of a 390 px viewport, with the action dock, the pot
     readout and the bottom-left pod below the fold. It is the same disease and it
     takes the same medicine, so the query matches BOTH. 560 px is not a new
     number: `PokerTable.svelte` already trims the action dock at
     `(min-aspect-ratio: 1/1) and (max-height: 560px)`, and the follow-up block
     after this one puts the compact header on ONE row there, because in a 390 px
     -tall window a header row costs more than a header column.
     ========================================================================= */
  /* The trust bar's own rules live in TrustBar.svelte; on every view the
     carrier above is the same sticky bar. */

  @include phone.header;

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

  /* Verify Modal.
     A FLEX COLUMN with the scrolling body in the middle, so `.modal-notices`
     below can be `flex-shrink: 0` and stay on screen however long the panel
     grows. Before docs/DEFECTS.md T-40 the whole dialog was one `overflow-y:
     auto` box; a pinned footer inside that would have scrolled away with
     everything else. */
  .verify-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: linear-gradient(180deg, #1a1a24 0%, #12121a 100%);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    max-width: 600px;
    width: 90%;
    max-height: 85vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    z-index: 1001;
    box-shadow: 0 25px 50px rgba(0, 0, 0, 0.5);
  }

  .verify-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 32px;
  }

  /* Not in the scroller. See the markup note: the four protected notices must be
     on screen the moment this dialog opens, not after a scroll. */
  .verify-modal .modal-notices {
    flex-shrink: 0;
    margin: 0;
    padding: 10px 24px 12px;
    border-top: 1px solid rgba(248, 113, 113, 0.28);
    background: rgba(185, 28, 28, 0.16);
    font-size: 11.5px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.9);
  }

  .verify-modal .modal-notices :global(strong) { color: var(--cd-notice-strong); }
  .verify-modal .notice-icon { font-size: 12px; }

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

  /* ==========================================================================
     WHICH NETWORK, AND WHETHER THE DEPLOYED CODE IS THIS CODE
     ==========================================================================
     Nothing in this block is `position: fixed` and nothing carries a z-index.
     Every one of these elements is in the document flow, so none of them can
     paint over the four protected notices (HARD RULE 2, docs/DEFECTS.md E-52).
     ========================================================================== */

  /* Inline with the tagline, and `display: inline` RATHER THAN `inline-block`,
     which is the whole point of the rule.
     Above the table every vertical pixel is felt (docs/DEFECTS.md T-19,
     docs/WAVE-05.md). An `inline-block` contributes its full margin box to the
     line box, so this chip at 9px with 1px padding and a 1px border made the
     11px tagline line 14.8px tall, grew the header, and cost measurable felt:
     the sweep read table-preflop desktop at 27.7% of frame against a 27.8%
     baseline. A plain `inline` box's padding and border do NOT affect line
     height, so the chip is now free. Keep it `inline`; no border. */
  .net-chip {
    display: inline;
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-surface-3);
    color: var(--cd-ink-2);
    font-size: 9px;
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.08em;
    white-space: nowrap;
  }

  /* Mainnet: the same neutral chip with an amber dot. The audit measured the
     red pill as the loudest thing beside the wordmark on every view; red is
     for the two things on a poker table that are red, and the trust bar
     already carries the money warning in words. */
  .net-chip.mainnet {
    background: var(--cd-surface-4);
    color: var(--cd-ink-1);
  }

  .net-chip.mainnet::before {
    content: '';
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 5px;
    border-radius: 50%;
    background: var(--cd-warn);
    vertical-align: 0.5px;
  }

  .net-footer {
    color: #555;
    font-variant-numeric: tabular-nums;
  }

  .net-footer.mainnet { color: #f87171; }

  .net-footer .canister-id {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 11px;
    color: inherit;
  }

  .verify-section.wiring {
    padding: 12px 14px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.03);
  }

  .verify-section.wiring[data-network="ic"] {
    border-color: rgba(248, 113, 113, 0.4);
    background: rgba(185, 28, 28, 0.12);
  }

  .wiring-list {
    display: grid;
    gap: 5px;
    margin: 0;
    font-size: 12.5px;
  }

  .wiring-list div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: baseline;
  }

  .wiring-list dt { margin: 0; color: #888; }
  .wiring-list dd { margin: 0; color: #ddd; text-align: right; word-break: break-all; }
  .wiring-list dd.live { color: #fca5a5; font-weight: 700; }

  .wiring-list code {
    font-family: 'Monaco', 'Menlo', monospace;
    font-size: 11px;
    color: #a78bfa;
  }

  .hash-note.provenance {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(240, 180, 41, 0.3);
    background: rgba(240, 180, 41, 0.08);
    color: rgba(255, 255, 255, 0.72);
    line-height: 1.55;
  }

  .hash-note.provenance strong { color: #f0b429; }

  .hash-check-btn {
    width: 100%;
    margin: 4px 0 12px 0;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(0, 212, 170, 0.35);
    background: rgba(0, 212, 170, 0.16);
    color: #00d4aa;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }

  .hash-check-btn:disabled { opacity: 0.6; cursor: default; }
  .hash-check-btn:hover:not(:disabled) { background: rgba(0, 212, 170, 0.26); }

  .hash-verdict {
    margin: 0 0 12px 0;
    padding: 10px 12px;
    border-radius: 8px;
    font-size: 12.5px;
    line-height: 1.5;
    font-weight: 600;
  }

  .hash-verdict.match {
    border: 1px solid rgba(74, 222, 128, 0.4);
    background: rgba(22, 101, 52, 0.25);
    color: #86efac;
  }

  .hash-verdict.unknown {
    border: 1px solid rgba(240, 180, 41, 0.4);
    background: rgba(240, 180, 41, 0.12);
    color: #fcd34d;
  }

  .hash-verdict.mismatch {
    border: 1px solid rgba(248, 113, 113, 0.6);
    background: rgba(185, 28, 28, 0.25);
    color: #fca5a5;
  }

  .hash-row .hash.live-match { color: #86efac; }
  .hash-row .hash.live-mismatch {
    color: #fca5a5;
    background: rgba(185, 28, 28, 0.25);
    font-weight: 700;
  }
  .hash-row .hash.live-unknown { color: #fcd34d; }

  /* THE PHONE (portrait, or a window under 560 px tall): the one-screen table
     view, the toast's place and the shell's touch floors. A mixin, included
     LAST on purpose (see app-phone.scss). */
  @include app-phone.rules;
</style>
