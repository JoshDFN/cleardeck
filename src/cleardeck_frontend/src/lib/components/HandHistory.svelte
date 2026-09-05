<script>
  // Hand history as a REPLAYER, not a table.
  //
  // This component LOADS and VERIFIES; the list (HandList.svelte) and the
  // replayer (HandReplayer.svelte) render. Three things it does:
  //
  // 1. It reads the table canister's own records for the last WINDOW hands
  //    (or the history canister's, when the table has none), normalised by
  //    $lib/hand-history-records.js into one model.
  //
  // 2. Every card it shows was re-derived in this browser from the seed the
  //    table committed to, using $lib/shuffle-verify.js. No canister is asked
  //    to vouch for anything; the arithmetic runs here.
  //
  //    IT DOES NOT CLAIM THE ORDERING. That the commitment came BEFORE the deal
  //    is not something this page can see, and the wording never says otherwise.
  //    What the page does instead is let the player witness the ordering for
  //    themselves: while a hand is running its commitment is published and its
  //    seed is not, so the panel notes the commitment automatically
  //    ($lib/commitment-witness.js) and compares it after the reveal.
  //
  // 3. It hides no hand. Hands you were not in are listed greyed, hands the
  //    canister has no record of are listed as gaps, and the filter is a
  //    visible chip row with counts.

  import { get } from 'svelte/store';
  import { NOTICE_LEAD, NOTICE_NO_RAKE, NOTICE_TERMS } from '../notices.js';
  import { history } from '$lib/canisters';
  import HandList from './HandList.svelte';
  import HandReplayer from './HandReplayer.svelte';
  import { scrollLock } from '$lib/scroll-lock.js';
  import logger from '$lib/logger.js';
  import { auth } from '$lib/auth.js';
  import { verifyHandLocally } from '$lib/shuffle-verify.js';
  // Importing utils installs the app-wide BigInt/JSON guard (see its header).
  // `formatTokenAmount` + `currencyOf` are the ONE formatter.
  import { currencyOf, formatTokenAmount } from '$lib/utils.js';
  import {
    factsFrom, fromHistoryRecord, fromTableRecord, num, opt, seatCardChecks, text,
  } from '$lib/hand-history-records.js';
  import { readSighting, recordSighting, verdictForSighting } from '$lib/commitment-witness.js';

  const { tableId = null, onClose, tableActor = null, handNumber = 0, tableName = 'ClearDeck table' } = $props();

  const WINDOW = 20; // how many hand numbers back to ask the table for

  let allHands = $state([]);
  let missing = $state([]);          // hand numbers the canister returned nothing for
  let inProgress = $state(null);     // the hand still being played, if any
  let loading = $state(true);
  let error = $state(null);
  let filter = $state('mine');
  let selected = $state(null);       // the hand being replayed
  let myPrincipal = $state(null);
  let maxPlayers = $state(9);
  let currency = $state('ICP');
  /** What the LIVE table view says, and how far it can be trusted. */
  let tableFacts = $state(null);
  /** Bumped whenever a commitment sighting is written, so derived reads re-run. */
  let sightingEpoch = $state(0);

  $effect(() => auth.subscribe((s) => {
    myPrincipal = s.principal?.toString ? s.principal.toString() : s.principal;
  }));

  /**
   * The principal, read straight from the store rather than from the reactive
   * mirror above: verification runs after an await, and if the store had not
   * emitted yet the component would quietly fail to identify the player's own
   * cards, which is exactly how hands used to look like they had vanished.
   */
  function principalNow() {
    const p = get(auth)?.principal;
    return p?.toString ? p.toString() : p ?? null;
  }

  /** Key the commitment-sighting store is scoped by. */
  const tableKey = () => text(tableId) || 'table';

  // -------------------------------------------------------------------------
  // Loading
  // -------------------------------------------------------------------------

  /**
   * @param {any} actor the table canister actor, or null
   * @param {number} top the newest hand number the table knows about, 0 if none
   *
   * Both are passed in rather than read from the props inside, so the `$effect`
   * below provably depends on them: the modal can be opened before the table
   * view has arrived, when `handNumber` is still 0.
   */
  async function loadHands(actor = tableActor, top = Number(handNumber) || 0) {
    loading = true;
    error = null;
    missing = [];
    inProgress = null;
    try {
      if (actor && top > 0) {
        let view = null;
        try {
          view = opt(await actor.get_table_view());
          if (view?.config?.max_players) maxPlayers = Number(view.config.max_players) || 9;
          currency = currencyOf(view?.config) ?? 'ICP';
          tableFacts = factsFrom(view);
        } catch (e) {
          logger.debug('HandHistory: table view unavailable', e);
        }

        // `handNumber` is the hand being PLAYED. The table writes that hand's
        // record when the hand STARTS with `revealed_seed` empty: sealed on
        // purpose until the hand ends, not missing.
        const topIsLive = tableFacts !== null && !tableFacts.settled;

        // THE ONE MOMENT WORTH REMEMBERING: the commitment is published and the
        // seed is not, so reading it now and comparing after the reveal is a
        // witness of the ordering that needs no canister clock.
        if (topIsLive) noteSighting(view);

        const wanted = [];
        for (let n = top; n >= Math.max(1, top - WINDOW + 1); n -= 1) wanted.push(n);

        const fetched = await Promise.all(wanted.map(async (n) => {
          try {
            const record = opt(await actor.get_hand_history(BigInt(n)));
            return record ? fromTableRecord(record, n) : { gap: n };
          } catch (e) {
            logger.debug(`HandHistory: hand ${n} query failed`, e);
            return { gap: n };
          }
        }));

        allHands = fetched.filter((h) => !h.gap);
        const gaps = fetched.filter((h) => h.gap).map((h) => h.gap);
        const topRecord = allHands.find((h) => h.requestedNumber === top);
        if (topIsLive && (gaps.includes(top) || (topRecord && !topRecord.proof.revealedSeed))) {
          inProgress = top;
        }
        // A hand still in play is not a hand the table lost the record for.
        missing = gaps.filter((n) => n !== inProgress);
      } else if (tableId) {
        // Fallback: the separate history canister. It only holds hands for
        // tables authorised with `authorize_table`, which the local dev wiring
        // never does, so this path is normally empty on a local replica.
        const summaries = await history.get_hands_by_table(tableId, 0n, BigInt(WINDOW));
        const full = await Promise.all((summaries || []).map(async (s) => {
          try {
            const record = opt(await history.get_hand(s.hand_id));
            return record ? fromHistoryRecord(record) : null;
          } catch {
            return null;
          }
        }));
        allHands = full.filter(Boolean);
      } else {
        allHands = [];
      }
      await verifyAll();
    } catch (e) {
      logger.error('Failed to load hand history:', e);
      error = e?.message || 'Failed to load history';
    }
    loading = false;
  }

  /** Writes down the commitment of a hand that has not revealed its seed yet. */
  function noteSighting(view) {
    const proof = view?.shuffle_proof ? opt(view.shuffle_proof) : null;
    if (!proof?.seed_hash) return;
    const result = recordSighting({
      tableId: tableKey(),
      handNumber: num(view.hand_number),
      seedHash: proof.seed_hash,
      boardCount: (view.community_cards || []).length,
      phase: tableFacts?.phase ?? null,
      seedAlreadyRevealed: opt(proof.revealed_seed) !== null,
    });
    if (result.stored) sightingEpoch += 1;
  }

  /**
   * Re-derives every loaded hand locally. 51 SHA-256 per hand, so a full
   * window costs single-digit milliseconds.
   */
  async function verifyAll() {
    myPrincipal = principalNow() ?? myPrincipal;
    const checked = [];
    for (const hand of allHands) {
      // eslint-disable-next-line no-await-in-loop -- tiny, and keeps ordering simple
      checked.push({ ...hand, verification: await verifyOne(hand) });
    }
    allHands = checked;
    if (selected) {
      const fresh = checked.find((h) => h.handNumber === selected.handNumber);
      if (fresh) selected = fresh;
    }
  }

  async function verifyOne(hand) {
    if (!hand.proof.seedHash || !hand.proof.revealedSeed) return null;
    const mine = hand.showdown.find((p) => p.principal && p.principal === myPrincipal)
      || hand.winners.find((w) => w.principal && w.principal === myPrincipal && w.cards);
    try {
      const report = await verifyHandLocally({
        seedHashHex: hand.proof.seedHash,
        revealedSeedHex: hand.proof.revealedSeed,
        myCards: mine?.cards ? [mine.cards[0], mine.cards[1]] : null,
        board: hand.community,
        maxPlayers,
        expectedPlayers: hand.seats.length >= 2 ? hand.seats.length : null,
      });
      return { ...report, seatCards: seatCardChecks(report, hand) };
    } catch (e) {
      logger.error('HandHistory: local verification failed', e);
      return null;
    }
  }

  // Reads both props HERE, synchronously, so the effect re-runs when the table
  // view finally lands and `handNumber` stops being 0.
  $effect(() => {
    loadHands(tableActor, Number(handNumber) || 0);
  });

  // -------------------------------------------------------------------------
  // Witnessing the ordering
  // -------------------------------------------------------------------------

  const sighting = $derived.by(() => {
    void sightingEpoch;
    if (!selected) return null;
    return readSighting({ tableId: tableKey(), handNumber: selected.handNumber });
  });
  const sightingVerdict = $derived(
    selected ? verdictForSighting(sighting, selected.proof.seedHash) : null,
  );
  const liveSighting = $derived.by(() => {
    void sightingEpoch;
    if (inProgress === null) return null;
    return readSighting({ tableId: tableKey(), handNumber: inProgress });
  });

  // -------------------------------------------------------------------------
  // Presentation helpers
  // -------------------------------------------------------------------------

  function formatTimestamp(ns) {
    if (!ns) return 'N/A';
    return new Date(Number(ns) / 1_000_000).toLocaleString();
  }

  /** A browser-clock millisecond stamp (the sighting store's own clock). */
  function formatLocalClock(ms) {
    if (!ms) return '--:--';
    return new Date(Number(ms)).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  /** One formatter, currency-aware, for every amount this modal renders. */
  const money = (amount) => formatTokenAmount(amount, { currency, includeUnit: true });
  const figure = (amount) => formatTokenAmount(amount, { currency, includeUnit: false });

  // ONE dismissal contract for every dialog in this app: Escape closes it from
  // anywhere, a backdrop click closes it, the close button closes it (T-13).
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="hand-history-modal">
  <div class="modal-backdrop" onclick={onClose} role="presentation"></div>

  <!-- The replayer is a two-column document on a wide screen; the LIST keeps
       the narrow dialog. On a phone both are a full-height sheet. -->
  <div class="modal-content" class:wide={!!selected} use:scrollLock role="dialog" aria-modal="true" aria-label="Hand history">
    <div class="modal-header">
      <h2>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5,3 19,12 5,21"/></svg>
        {selected ? 'Hand Replay' : 'Hand History'}
      </h2>
      <button class="close-btn" onclick={onClose} aria-label="Close hand history">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 6L6 18M6 6l12 12"/></svg>
      </button>
    </div>

    {#if loading}
      <div class="state-block">
        <div class="spinner"></div>
        <span>Loading hands and re-deriving their decks locally…</span>
      </div>
    {:else if error}
      <div class="state-block bad">
        <p>{error}</p>
        <button class="ghost-btn" onclick={() => loadHands()}>Retry</button>
      </div>
    {:else if selected}
      {#key selected.handNumber}
        <HandReplayer
          hand={selected} {tableFacts} {myPrincipal} {currency} {money} {figure}
          {sighting} {sightingVerdict} {tableName} {maxPlayers}
          onBack={() => { selected = null; }}
        />
      {/key}
    {:else}
      <HandList
        {allHands} {myPrincipal} bind:filter {inProgress} {liveSighting} {missing}
        {money} {formatTimestamp} {formatLocalClock}
        onOpen={(hand) => { selected = hand; }}
      />
    {/if}

    <!-- THE FIVE PLAYER-PROTECTION NOTICES, ON THIS SURFACE. This modal is a
         fixed overlay over a 72% scrim, so the trust bar behind it is
         unreadable; the same words ride INSIDE the dialog, verbatim, pinned
         outside the scrolling region so they cannot be scrolled away. Both
         history scenes measure them on the rendered page. -->
    <div class="legal">
      <p class="legal-warning">
        <strong>Disclaimer:</strong> {NOTICE_LEAD}. {NOTICE_TERMS.warningBody}
      </p>
      <p class="legal-norake">
        <strong class="norake">{NOTICE_NO_RAKE}</strong> Every amount in
        this record is money that went to a player.
      </p>
    </div>
  </div>
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.ghost;

  .hand-history-modal {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--cd-space-5);
  }

  .modal-backdrop {
    position: absolute;
    inset: 0;
    background: var(--cd-scrim);
    backdrop-filter: blur(4px);
  }

  .modal-content {
    position: relative;
    width: 100%;
    max-width: 760px;
    max-height: 88vh;
    background: var(--cd-sheet);
    border-radius: var(--cd-radius-panel);
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-lift);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--cd-space-3) var(--cd-space-4);
    border-bottom: 1px solid var(--cd-line-soft);
    flex-shrink: 0;
  }

  .modal-header h2 {
    display: flex;
    align-items: center;
    gap: var(--cd-space-2);
    margin: 0;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
  }

  .close-btn {
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    color: var(--cd-ink-2);
    width: var(--cd-control-sm);
    height: var(--cd-control-sm);
    border-radius: var(--cd-radius-chip);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease);
  }

  .close-btn:hover { background: var(--cd-danger-dim); border-color: var(--cd-danger-line); color: var(--cd-danger-hi); }

  @media (min-width: 900px) {
    .modal-content.wide { max-width: 1160px; max-height: 92vh; }
  }

  .state-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--cd-space-3);
    padding: var(--cd-space-6) var(--cd-space-5);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-sm);
    text-align: center;
    line-height: 1.6;
  }

  .state-block.bad { color: var(--cd-danger-hi); }
  .state-block p { margin: 0; max-width: 46ch; }

  .spinner {
    width: 26px;
    height: 26px;
    border: 3px solid var(--cd-line);
    border-top-color: var(--cd-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ---- the protected notices, pinned inside the dialog ---- */
  .legal {
    flex-shrink: 0;
    padding: var(--cd-space-2) var(--cd-space-4) var(--cd-space-3);
    border-top: 1px solid var(--cd-line-soft);
    background: var(--cd-surface-1);
  }

  .legal p { margin: 0; font-size: var(--cd-text-xs); line-height: 1.5; color: var(--cd-ink-1); }
  .legal p + p { margin-top: 2px; }
  .legal strong { color: var(--cd-notice-strong); }
  .legal .norake { color: var(--cd-accent); }

  /* THE PHONE: a full-height sheet, one scroller, the notices at its foot. */
  @media #{f.$phone} {
    .hand-history-modal { padding: 0; align-items: stretch; }
    .modal-content {
      max-width: none;
      max-height: none;
      height: 100dvh;
      border-radius: 0;
      border: 0;
      padding-top: var(--cd-safe-top);
      padding-bottom: var(--cd-safe-bottom);
    }
    .close-btn { width: var(--cd-touch-min); height: var(--cd-touch-min); min-width: var(--cd-touch-min); }
    /* THE NOTICES UNDER THE HEADER, NOT AT THE FOOT. The phone's toast is a
       bottom snackbar raised over a scroll-locked sheet (Toast.svelte), so a
       block pinned to the sheet's foot is exactly what it paints over: the
       first run of this phase measured all five phrases occluded by the toast
       at 390x844. Under the header nothing ever stands on them. */
    .modal-header { order: -2; }
    .legal { order: -1; padding: var(--cd-space-2) var(--cd-space-3); border-top: 0; border-bottom: 1px solid var(--cd-line-soft); }
  }
</style>
