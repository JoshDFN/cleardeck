<script>
  // Hand history as a REPLAYER, not a table.
  //
  // Two things are different from what this component used to do.
  //
  // 1. It replays. You step through a hand street by street and watch the board
  //    arrive, instead of reading a static summary.
  //
  // 2. Every card it shows was re-derived in this browser from the seed the
  //    table committed to BEFORE the hand, using $lib/shuffle-verify.js. So the
  //    replay does not just say what happened, it shows that the cards could not
  //    have been chosen after the fact. No canister is asked to vouch for
  //    anything; the arithmetic runs here.
  //
  // It also no longer hides hands. The previous version filtered the list down to
  // hands whose `winners` or `showdown_players` contained your principal, which
  // silently dropped every hand you folded and lost -- indistinguishable from the
  // table losing your history. Hands you were not in are now shown greyed rather
  // than removed, hands the canister has no record of are listed as gaps, and the
  // filter is a visible toggle with counts.

  import { get } from 'svelte/store';
  import { history } from '$lib/canisters';
  import Card from './Card.svelte';
  import logger from '$lib/logger.js';
  import { auth } from '$lib/auth.js';
  import { verifyHandLocally } from '$lib/shuffle-verify.js';
  // Importing this module installs the app-wide BigInt/JSON guard (see the
  // header of $lib/utils.js). `safeStringify` is the same rule, stated locally.
  import { safeStringify } from '$lib/utils.js';

  const { tableId = null, onClose, tableActor = null, handNumber = 0 } = $props();

  const WINDOW = 20; // how many hand numbers back to ask the table for

  let allHands = $state([]);
  let missing = $state([]);          // hand numbers the canister returned nothing for
  let inProgress = $state(null);     // the hand still being played, if any
  let loading = $state(true);
  let error = $state(null);
  let onlyMine = $state(true);
  let selected = $state(null);       // the hand being replayed
  let stopIndex = $state(0);
  let playing = $state(false);
  let copied = $state(null);
  let myPrincipal = $state(null);
  let maxPlayers = $state(9);

  /** Phases in which the current hand number has no record yet, and should not. */
  const SETTLED_PHASES = ['HandComplete', 'WaitingForPlayers'];

  $effect(() => auth.subscribe((s) => {
    myPrincipal = s.principal?.toString ? s.principal.toString() : s.principal;
  }));

  /**
   * The principal, read straight from the store rather than from the reactive
   * mirror above. Verification runs after an await, and if the auth store had not
   * emitted yet, `myPrincipal` would still be null and this component would
   * quietly fail to identify the player's own cards -- exactly the class of
   * silent omission that made hands look like they had vanished.
   */
  function principalNow() {
    const p = get(auth)?.principal;
    return p?.toString ? p.toString() : p ?? null;
  }

  /** Candid `opt T` arrives as [] or [value]. */
  const opt = (v) => (Array.isArray(v) ? (v.length ? v[0] : null) : v ?? null);
  const text = (p) => (p?.toString ? p.toString() : p ?? null);

  // -------------------------------------------------------------------------
  // Normalising the two record shapes into one model
  // -------------------------------------------------------------------------

  /** `{Bet: 500n}` / `{Fold: null}` -> `{kind:'Bet', amount:500}`. */
  function normalizeAction(record) {
    const variant = record?.action;
    const kind = variant && typeof variant === 'object' ? Object.keys(variant)[0] : null;
    const payload = kind ? variant[kind] : null;
    const amount = typeof payload === 'bigint' || typeof payload === 'number'
      ? Number(payload)
      : null;
    return {
      kind: kind || 'Unknown',
      amount,
      seat: Number(record?.seat ?? 0),
      timestamp: record?.timestamp ?? 0n,
      phase: typeof record?.phase === 'string' ? record.phase : null,
    };
  }

  /** The table canister's own `HandHistory`. */
  function fromTableRecord(record, requestedNumber) {
    const proof = record.shuffle_proof || {};
    const revealed = opt(proof.revealed_seed);
    const showdown = (record.showdown_players || []).map((p) => ({
      seat: Number(p.seat),
      principal: text(p.principal),
      cards: opt(p.cards),
      rank: opt(p.hand_rank),
      won: Number(p.amount_won || 0),
    }));
    const winners = (record.winners || []).map((w) => ({
      seat: Number(w.seat),
      principal: text(w.principal),
      amount: Number(w.amount || 0),
      potType: null,
      rank: opt(w.hand_rank),
      cards: opt(w.cards),
    }));
    const actions = (record.actions || []).map(normalizeAction);
    const seats = new Set([
      ...actions.map((a) => a.seat),
      ...showdown.map((p) => p.seat),
      ...winners.map((w) => w.seat),
    ]);
    return {
      handNumber: Number(record.hand_number ?? requestedNumber),
      requestedNumber,
      timestamp: proof.timestamp ?? 0n,
      proof: { seedHash: proof.seed_hash || null, revealedSeed: revealed },
      community: record.community_cards || [],
      actions,
      showdown,
      winners,
      seats: [...seats].sort((a, b) => a - b),
      potTotal: winners.reduce((sum, w) => sum + w.amount, 0),
      source: 'table canister',
      verification: null,
    };
  }

  /** The history canister's richer `HandHistoryRecord` (tags actions by street). */
  function fromHistoryRecord(record) {
    const proof = record.shuffle_proof || {};
    const flop = opt(record.flop);
    const community = [
      ...(flop ? [flop[0], flop[1], flop[2]] : []),
      ...(opt(record.turn) ? [opt(record.turn)] : []),
      ...(opt(record.river) ? [opt(record.river)] : []),
    ];
    const players = (record.players || []).map((p) => ({
      seat: Number(p.seat),
      principal: text(p.principal),
      cards: opt(p.hole_cards),
      rank: opt(p.final_hand_rank),
      won: Number(p.amount_won || 0),
    }));
    const winners = (record.winners || []).map((w) => ({
      seat: Number(w.seat),
      principal: text(w.principal),
      amount: Number(w.amount || 0),
      potType: w.pot_type || null,
      rank: opt(w.hand_rank),
      cards: null,
    }));
    const actions = (record.actions || []).map(normalizeAction);
    return {
      handNumber: Number(record.hand_number),
      requestedNumber: Number(record.hand_id ?? record.hand_number),
      timestamp: record.timestamp ?? proof.timestamp ?? 0n,
      proof: { seedHash: proof.seed_hash || null, revealedSeed: proof.revealed_seed || null },
      community,
      actions,
      showdown: players.filter((p) => p.cards),
      winners,
      seats: players.map((p) => p.seat).sort((a, b) => a - b),
      potTotal: Number(record.total_pot || 0),
      source: 'history canister',
      verification: null,
    };
  }

  // -------------------------------------------------------------------------
  // Loading
  // -------------------------------------------------------------------------

  /**
   * @param {any} actor the table canister actor, or null
   * @param {number} top the newest hand number the table knows about, 0 if none
   *
   * Both are passed in rather than read from the props inside, so that the
   * `$effect` below provably depends on them. The modal can be opened before the
   * table view has arrived, when `handNumber` is still 0; without that dependency
   * the list would take the history-canister fallback once and stay empty for the
   * rest of the session even after the real hand number appeared.
   */
  async function loadHands(actor = tableActor, top = Number(handNumber) || 0) {
    loading = true;
    error = null;
    missing = [];
    inProgress = null;
    try {
      if (actor && top > 0) {
        let livePhase = null;
        try {
          const view = opt(await actor.get_table_view());
          if (view?.config?.max_players) maxPlayers = Number(view.config.max_players) || 9;
          livePhase = view?.phase && typeof view.phase === 'object'
            ? Object.keys(view.phase)[0]
            : null;
        } catch (e) {
          logger.debug('HandHistory: table view unavailable', e);
        }

        // `handNumber` is the hand being PLAYED. The table canister writes that
        // hand's record when the hand STARTS, with `revealed_seed` empty, so it
        // comes back here looking like an ordinary finished hand whose seed has
        // gone missing. It has not: the seed is sealed on purpose until the hand
        // ends. Saying so is the difference between "we are not showing you this
        // yet, and here is why" and an unexplained blank.
        const topIsLive = livePhase !== null && !SETTLED_PHASES.includes(livePhase);
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
        // Fallback: the separate history canister. It only holds hands for tables
        // that were authorised with `authorize_table`, which the local dev wiring
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

  /**
   * Re-derives every loaded hand locally. 51 SHA-256 per hand, so a full window
   * costs single-digit milliseconds and there is no reason not to check all of
   * them up front.
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

  /**
   * Every card any player showed, checked against the locally derived deck.
   *
   * The hand log does not record which seats were dealt in, so this assumes the
   * seats it saw, in ascending order, are that list. If the assumption is wrong
   * the codes simply will not match and no tick is shown: a wrong guess can
   * never produce a false confirmation.
   */
  function seatCardChecks(report, hand) {
    if (!report?.layout || report.layout.players !== hand.seats.length) return [];
    return hand.showdown
      .filter((p) => p.cards)
      .map((p) => {
        const k = hand.seats.indexOf(p.seat);
        if (k < 0) return null;
        const [a, b] = report.layout.positions.holes[k];
        const derived = [report.deckCodes[a], report.deckCodes[b]];
        const dealt = [p.cards[0], p.cards[1]].map(codeOf);
        return {
          seat: p.seat,
          positions: [a, b],
          derived,
          match: derived[0] === dealt[0] && derived[1] === dealt[1],
        };
      })
      .filter(Boolean);
  }

  const RANK_LETTER = {
    Two: '2', Three: '3', Four: '4', Five: '5', Six: '6', Seven: '7', Eight: '8',
    Nine: '9', Ten: 'T', Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A',
  };
  const SUIT_LETTER = { Hearts: 'h', Diamonds: 'd', Clubs: 'c', Spades: 's' };

  function codeOf(card) {
    if (!card) return null;
    const rank = Object.keys(card.rank || {})[0];
    const suit = Object.keys(card.suit || {})[0];
    return rank && suit ? RANK_LETTER[rank] + SUIT_LETTER[suit] : null;
  }

  // Reads both props HERE, synchronously, so the effect re-runs when the table
  // view finally lands and `handNumber` stops being 0.
  $effect(() => {
    loadHands(tableActor, Number(handNumber) || 0);
  });

  // -------------------------------------------------------------------------
  // Replay
  // -------------------------------------------------------------------------

  const participated = (hand) =>
    !!myPrincipal && (
      hand.showdown.some((p) => p.principal === myPrincipal)
      || hand.winners.some((w) => w.principal === myPrincipal)
    );

  const hands = $derived(onlyMine ? allHands.filter(participated) : allHands);
  const mineCount = $derived(allHands.filter(participated).length);

  function stopsFor(hand) {
    if (!hand) return [];
    const n = hand.community.length;
    const stops = [{ key: 'preflop', label: 'Pre-flop', board: 0, reveal: false }];
    if (n >= 3) stops.push({ key: 'flop', label: 'Flop', board: 3, reveal: false });
    if (n >= 4) stops.push({ key: 'turn', label: 'Turn', board: 4, reveal: false });
    if (n >= 5) stops.push({ key: 'river', label: 'River', board: 5, reveal: false });
    stops.push(hand.showdown.length
      ? { key: 'showdown', label: 'Showdown', board: n, reveal: true }
      : { key: 'end', label: 'Won without showdown', board: n, reveal: false });
    return stops;
  }

  const stops = $derived(stopsFor(selected));
  const stop = $derived(stops[Math.min(stopIndex, Math.max(0, stops.length - 1))] || null);

  /** Board cards visible at the current stop, each with its proof annotation. */
  const visibleBoard = $derived.by(() => {
    if (!selected || !stop) return [];
    const boardChecks = (selected.verification?.checks || []).filter((c) => c.kind === 'board');
    return selected.community.slice(0, stop.board).map((card, at) => ({
      card,
      check: boardChecks[at] || null,
    }));
  });

  function openHand(hand) {
    selected = hand;
    stopIndex = 0;
    playing = false;
  }

  function step(delta) {
    playing = false;
    stopIndex = Math.max(0, Math.min(stops.length - 1, stopIndex + delta));
  }

  $effect(() => {
    if (!playing) return;
    const id = setInterval(() => {
      if (stopIndex >= stops.length - 1) { playing = false; return; }
      stopIndex += 1;
    }, 1100);
    return () => clearInterval(id);
  });

  // -------------------------------------------------------------------------
  // Presentation helpers
  // -------------------------------------------------------------------------

  function formatTimestamp(ns) {
    if (!ns) return 'N/A';
    return new Date(Number(ns) / 1_000_000).toLocaleString();
  }

  function formatClock(ns) {
    if (!ns) return '--:--';
    return new Date(Number(ns) / 1_000_000).toLocaleTimeString([], {
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
  }

  function formatChips(amount) {
    const icp = Number(amount ?? 0) / 100_000_000;
    if (icp >= 1000) return `${(icp / 1000).toFixed(2)}K ICP`;
    if (icp >= 0.01) return `${icp.toFixed(2)} ICP`;
    return `${icp.toFixed(4)} ICP`;
  }

  function rankName(rank) {
    const value = Array.isArray(rank) ? rank[0] : rank;
    if (!value || typeof value !== 'object') return null;
    const key = Object.keys(value)[0];
    return key ? key.replace(/([A-Z])/g, ' $1').trim() : null;
  }

  const ACTION_WORD = {
    Fold: 'folds', Check: 'checks', Call: 'calls', Bet: 'bets',
    Raise: 'raises to', AllIn: 'goes all in', PostBlind: 'posts a blind of',
  };

  function actionLine(action) {
    const word = ACTION_WORD[action.kind] || action.kind.toLowerCase();
    return action.amount ? `${word} ${formatChips(action.amount)}` : word;
  }

  function seatLabel(seat) {
    return `Seat ${seat + 1}`;
  }

  const verdictOf = (hand) => {
    const v = hand.verification;
    if (!v) return { tone: 'pending', label: 'seed not revealed' };
    if (v.ok) return { tone: 'good', label: `${v.cardsMatched} cards re-derived here` };
    if (v.commitment?.match && v.cardsChecked === 0) {
      return { tone: 'partial', label: 'commitment checked here' };
    }
    return { tone: 'bad', label: 'DID NOT verify' };
  };

  async function copyToClipboard(value, label) {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      copied = label;
      setTimeout(() => { copied = null; }, 2000);
    } catch (e) {
      logger.error('Failed to copy:', e);
    }
  }

  /** The whole audit trail for one hand, as a file the player keeps. */
  function downloadHand(hand) {
    const v = hand.verification;
    const payload = {
      note: 'ClearDeck hand record. The deck below was re-derived in the player\'s '
        + 'browser from revealed_seed using the algorithm in docs/SHUFFLE-SPEC.md. '
        + 'Verifying a shuffle proves the deal was not tampered with after the '
        + 'commitment; it proves nothing else about the engine.',
      hand_number: hand.handNumber,
      recorded_at: formatTimestamp(hand.timestamp),
      source: hand.source,
      shuffle_proof: {
        seed_hash: hand.proof.seedHash,
        revealed_seed: hand.proof.revealedSeed,
      },
      seats_seen: hand.seats,
      community_cards: hand.community.map(codeOf),
      actions: hand.actions.map((a) => ({
        seat: a.seat, action: a.kind, amount: a.amount, at: formatClock(a.timestamp),
      })),
      showdown: hand.showdown.map((p) => ({
        seat: p.seat, cards: p.cards ? [p.cards[0], p.cards[1]].map(codeOf) : null,
        hand_rank: rankName(p.rank), won_e8s: p.won,
      })),
      winners: hand.winners.map((w) => ({ seat: w.seat, amount_e8s: w.amount })),
      local_verification: v ? {
        commitment_recomputed_in_browser: v.commitment.computed,
        commitment_published_before_deal: v.commitment.committed,
        commitment_matches: v.commitment.match,
        players_dealt_in: v.playersDetermined ? v.layout?.players : 'not determined',
        cards_checked: v.cardsChecked,
        cards_matched: v.cardsMatched,
        rejected_draws: v.rejections,
        derived_deck: v.deckCodes,
      } : 'seed not revealed for this hand',
    };
    // `amount`, `won_e8s` and `amount_e8s` are Candid nat64s, i.e. BigInt. A bare
    // JSON.stringify THROWS on those, so the download button did nothing at all.
    // Same class of defect as docs/DEFECTS.md T-10; e8s are written as decimal
    // strings, which is what a nat64 is on the wire anyway. `safeStringify` is
    // the one copy of that rule ($lib/utils.js).
    const blob = new Blob([safeStringify(payload, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `cleardeck-hand-${hand.handNumber}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }
  // ONE dismissal contract for every dialog in this app: Escape closes it from
  // anywhere, a backdrop click closes it, the close button closes it. The
  // `onkeydown` that used to sit on the backdrop could never fire — the backdrop
  // is `tabindex="-1"` and nothing ever focuses it — so Escape worked in
  // HowItWorks and silently did nothing here, in DepositModal and in
  // WithdrawModal. docs/DEFECTS.md T-13.
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="hand-history-modal">
  <div
    class="modal-backdrop"
    onclick={onClose}
    role="presentation"
  ></div>

  <div class="modal-content">
    <div class="modal-header">
      <h2>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5,3 19,12 5,21"/>
        </svg>
        Hand Replay
      </h2>
      <button class="close-btn" onclick={onClose} aria-label="Close hand history">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M18 6L6 18M6 6l12 12"/>
        </svg>
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
        <button class="ghost-btn" onclick={loadHands}>Retry</button>
      </div>
    {:else if selected}
      <!-- ---------------- REPLAYER ---------------- -->
      <div class="replayer">
        <div class="replay-top">
          <button class="ghost-btn" onclick={() => { selected = null; playing = false; }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15,18 9,12 15,6"/>
            </svg>
            All hands
          </button>
          <div class="replay-title">
            <strong>Hand #{selected.handNumber}</strong>
            <span>{formatTimestamp(selected.timestamp)}</span>
          </div>
          <button class="ghost-btn" onclick={() => downloadHand(selected)}>Download</button>
        </div>

        <!-- proof banner: the claim, stated only as strongly as it was checked.
             THE ORDERING IS NOT PART OF THE CLAIM. This banner used to lead with
             "These cards were fixed before the hand was played … before any card
             was dealt", which is the ordering claim ShuffleProof.svelte retires
             by name ("Not proven: that the commitment came before the cards").
             Two surfaces of the same product cannot say opposite things about
             the same fact, and the one a player opens to review a hand they lost
             is the worse one to over-claim on. What the browser actually did is
             stated instead, and the ordering is carried beside it as the open
             question, with the one thing that would settle it. -->
        {#if selected.verification?.ok}
          <div class="proof-banner good">
            <span class="banner-mark">✓</span>
            <div>
              <strong>These cards follow from the seed the table committed to.</strong>
              <p>
                All {selected.verification.cardsMatched} cards below were re-derived in this browser from
                the revealed seed, and its SHA-256 recomputed here matches the commitment the table
                published for this hand. No canister was asked to confirm it.
              </p>
              <p class="banner-caveat">
                <span class="caveat-tag">Not proven</span>
                That the commitment came <em>before</em> the deal. The
                {formatClock(selected.timestamp)} above is a clock read off the table canister; this page
                did not watch the order of events, and everything above stays true even if that clock is
                wrong. To witness it yourself, copy the commitment off the table while a hand is still
                running and compare it here after the seed is revealed.
              </p>
            </div>
          </div>
        {:else if selected.verification?.commitment?.match}
          <div class="proof-banner partial">
            <span class="banner-mark">◐</span>
            <div>
              <strong>Commitment checked here; cards not checkable for this hand.</strong>
              <p>
                The revealed seed hashes to the pre-published commitment (recomputed in this browser), but
                this hand exposes no card that could be matched against the deck it produces.
              </p>
            </div>
          </div>
        {:else if selected.verification}
          <div class="proof-banner bad">
            <span class="banner-mark">✗</span>
            <div>
              <strong>This hand did not verify in your browser.</strong>
              <p>{selected.verification.error || 'The cards do not follow from the committed seed.'}</p>
            </div>
          </div>
        {:else}
          <div class="proof-banner partial">
            <span class="banner-mark">◌</span>
            <div>
              <strong>No revealed seed for this hand.</strong>
              <p>The seed is only published once a hand ends, so there is nothing to re-derive yet.</p>
            </div>
          </div>
        {/if}

        <!-- street scrubber -->
        <div class="scrubber">
          {#each stops as s, at}
            <button
              class="stop"
              class:active={at === stopIndex}
              class:passed={at < stopIndex}
              onclick={() => { playing = false; stopIndex = at; }}
            >{s.label}</button>
          {/each}
        </div>

        <div class="transport">
          <button class="transport-btn" onclick={() => step(-1)} disabled={stopIndex === 0} aria-label="Previous street">◀</button>
          <button
            class="transport-btn play"
            onclick={() => { playing = !playing; if (playing && stopIndex >= stops.length - 1) stopIndex = 0; }}
            aria-label={playing ? 'Pause replay' : 'Play replay'}
          >{playing ? '❙❙' : '▶'}</button>
          <button class="transport-btn" onclick={() => step(1)} disabled={stopIndex >= stops.length - 1} aria-label="Next street">▶</button>
          <span class="transport-label">{stop?.label} · {stopIndex + 1} of {stops.length}</span>
        </div>

        <!-- the board, with each card's deck position and local proof -->
        <div class="felt">
          {#if visibleBoard.length === 0}
            <span class="felt-empty">No community cards yet</span>
          {:else}
            <div class="board">
              {#each visibleBoard as slot}
                <div class="board-slot">
                  <Card card={slot.card} small={true} />
                  {#if slot.check}
                    <span class="deck-pos" class:good={slot.check.match} class:bad={!slot.check.match}>
                      #{slot.check.positions[0]} {slot.check.match ? '✓' : '✗'}
                    </span>
                  {:else}
                    <span class="deck-pos">—</span>
                  {/if}
                </div>
              {/each}
            </div>
            <span class="felt-note">deck position, re-derived in this browser</span>
          {/if}
        </div>

        <!-- players -->
        <div class="panel">
          <h4>Players</h4>
          {#if selected.showdown.length}
            {#each selected.showdown as player}
              {@const seatCheck = (selected.verification?.seatCards || []).find((c) => c.seat === player.seat)}
              <div class="player-row" class:me={player.principal === myPrincipal} class:won={player.won > 0}>
                <span class="seat-tag">{seatLabel(player.seat)}{#if player.principal === myPrincipal} · you{/if}</span>
                <div class="player-cards">
                  {#if stop?.reveal && player.cards}
                    <Card card={player.cards[0]} small={true} />
                    <Card card={player.cards[1]} small={true} />
                    {#if seatCheck}
                      <span class="deck-pos" class:good={seatCheck.match} class:bad={!seatCheck.match}>
                        #{seatCheck.positions[0]},{seatCheck.positions[1]} {seatCheck.match ? '✓' : '✗'}
                      </span>
                    {/if}
                  {:else}
                    <span class="hidden-cards">face down until showdown</span>
                  {/if}
                </div>
                <span class="player-rank">{stop?.reveal ? (rankName(player.rank) || '') : ''}</span>
                <span class="player-result">
                  {#if player.won > 0}<span class="won-amt">+{formatChips(player.won)}</span>{:else}<span class="lost-amt">—</span>{/if}
                </span>
              </div>
            {/each}
          {:else}
            <p class="panel-note">
              Nobody showed cards in this hand: it ended before a showdown, so the table never revealed
              anyone's hole cards. Seats seen in the log: {selected.seats.map(seatLabel).join(', ') || 'none'}.
            </p>
          {/if}
        </div>

        <!-- chronological action log, PokerNow style -->
        <div class="panel">
          <h4>Action log</h4>
          {#if selected.actions.length}
            <div class="log">
              {#each selected.actions as action}
                <div class="log-line kind-{action.kind.toLowerCase()}">
                  <span class="log-time">{formatClock(action.timestamp)}</span>
                  <span class="log-seat">{seatLabel(action.seat)}</span>
                  <span class="log-what">{actionLine(action)}</span>
                </div>
              {/each}
            </div>
            {#if !selected.actions.some((a) => a.phase)}
              <p class="panel-note">
                The table records each action with a timestamp but does not tag it with the street it
                happened on, so this log is chronological and is deliberately not split by betting round.
                Blind posts are not recorded as actions at all.
              </p>
            {/if}
          {:else}
            <p class="panel-note">No actions recorded for this hand.</p>
          {/if}
        </div>

        <!-- the proof, in full -->
        <div class="panel proof-panel">
          <h4>Shuffle proof</h4>
          <div class="proof-line">
            <span class="proof-label">committed before the deal</span>
            <code>{selected.proof.seedHash || 'n/a'}</code>
            <button class="mini-btn" onclick={() => copyToClipboard(selected.proof.seedHash, 'hash')}>
              {copied === 'hash' ? '✓' : 'Copy'}
            </button>
          </div>
          <div class="proof-line">
            <span class="proof-label">revealed after the hand</span>
            {#if selected.proof.revealedSeed}
              <code class="revealed">{selected.proof.revealedSeed}</code>
              <button class="mini-btn" onclick={() => copyToClipboard(selected.proof.revealedSeed, 'seed')}>
                {copied === 'seed' ? '✓' : 'Copy'}
              </button>
            {:else}
              <code class="muted">not revealed</code>
            {/if}
          </div>
          {#if selected.verification}
            <div class="proof-line">
              <span class="proof-label">SHA-256 recomputed here</span>
              <code class:revealed={!selected.verification.commitment.match}>{selected.verification.commitment.computed}</code>
            </div>
            <p class="panel-note">
              {#if selected.verification.playersDetermined}
                Laid out for {selected.verification.layout.players} players dealt in.
              {/if}
              {selected.verification.rejections} draw(s) rejected by the sampling rule.
              Verifying a shuffle proves the deal was not tampered with after the commitment. It proves
              nothing about settlement or about the rest of this unaudited engine.
            </p>
          {/if}
        </div>

        <div class="panel pot-panel">
          <div class="pot-total">
            <span>Final pot</span>
            <strong>{formatChips(selected.potTotal)}</strong>
          </div>
          {#each selected.winners as winner}
            <div class="winner-line">
              <span>{seatLabel(winner.seat)}{#if winner.principal === myPrincipal} · you{/if}</span>
              <span class="won-amt">+{formatChips(winner.amount)}</span>
              {#if winner.potType}<span class="pot-type">{winner.potType}</span>{/if}
            </div>
          {/each}
        </div>
      </div>
    {:else}
      <!-- ---------------- LIST ---------------- -->
      <div class="list-head">
        <div class="filter">
          <button class="chip" class:on={onlyMine} onclick={() => onlyMine = true}>
            My hands ({mineCount})
          </button>
          <button class="chip" class:on={!onlyMine} onclick={() => onlyMine = false}>
            Every hand here ({allHands.length})
          </button>
        </div>
        {#if inProgress !== null}
          <span class="live-note">
            Hand #{inProgress} is still being played, so its seed stays sealed and its cards cannot be
            re-derived yet. Revealing it early would show everyone the deck.
          </span>
        {/if}
        {#if missing.length}
          <span class="gap-note">
            {missing.length} hand number(s) in this window have no record on the table:
            {missing.slice(0, 6).join(', ')}{missing.length > 6 ? '…' : ''}
          </span>
        {/if}
      </div>

      {#if hands.length === 0}
        <div class="state-block">
          <p>
            {#if allHands.length > 0}
              None of the {allHands.length} recorded hand(s) here list you as a showdown player or a
              winner. Switch to “Every hand here” to see them.
            {:else}
              No hands recorded on this table yet.
            {/if}
          </p>
        </div>
      {:else}
        <div class="hands-list">
          {#each hands as hand}
            {@const verdict = verdictOf(hand)}
            {@const iWon = hand.winners.some((w) => w.principal === myPrincipal && w.amount > 0)}
            <button class="hand-row" class:dim={!participated(hand)} onclick={() => openHand(hand)}>
              <div class="row-main">
                <span class="hand-number">Hand #{hand.handNumber}</span>
                <span class="hand-time">{formatTimestamp(hand.timestamp)}</span>
              </div>
              <div class="row-mid">
                <span class="pot">{formatChips(hand.potTotal)}</span>
                <span class="seats">{hand.seats.length || '?'} seats</span>
              </div>
              <div class="row-badges">
                {#if participated(hand)}
                  <span class="badge" class:you-won={iWon} class:you-lost={!iWon}>
                    {iWon ? 'You won' : 'You lost'}
                  </span>
                {/if}
                {#if hand.showdown.length}<span class="badge showdown">Showdown</span>{/if}
                <span class="badge verdict {verdict.tone}">{verdict.label}</span>
              </div>
              <svg class="chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="9,6 15,12 9,18"/>
              </svg>
            </button>
          {/each}
        </div>
        <div class="list-foot">
          Every hand above was re-derived in this browser from its own revealed seed. Nothing here was
          verified by a canister.
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
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(4px);
  }

  .modal-content {
    position: relative;
    width: 100%;
    max-width: 680px;
    max-height: 86vh;
    background: linear-gradient(145deg, rgba(25, 25, 40, 0.98), rgba(15, 15, 25, 0.98));
    border-radius: 18px;
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
    padding: 16px 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }

  .modal-header h2 {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 0;
    font-size: 16px;
    font-weight: 650;
    color: #fff;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #8b93a7;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .close-btn:hover { background: rgba(239, 68, 68, 0.15); border-color: rgba(239, 68, 68, 0.3); color: #ef4444; }

  .state-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 48px 24px;
    color: #8b93a7;
    font-size: 13px;
    text-align: center;
    line-height: 1.6;
  }

  .state-block.bad { color: #ef4444; }
  .state-block p { margin: 0; max-width: 46ch; }

  .spinner {
    width: 26px;
    height: 26px;
    border: 3px solid rgba(255, 255, 255, 0.1);
    border-top-color: #00d4aa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .ghost-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #a9b2c4;
    padding: 6px 11px;
    border-radius: 8px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .ghost-btn:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }

  /* ---- list ---- */
  .list-head {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px 6px;
    flex-shrink: 0;
  }

  .filter { display: flex; gap: 6px; }

  .chip {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.09);
    color: #8b93a7;
    padding: 6px 12px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .chip.on { background: rgba(0, 212, 170, 0.14); border-color: rgba(0, 212, 170, 0.4); color: #00d4aa; }

  .gap-note { font-size: 10.5px; color: #7c8497; line-height: 1.5; }
  .live-note { font-size: 10.5px; color: #8b93a7; line-height: 1.5; }

  .hands-list { flex: 1; overflow-y: auto; padding: 6px 8px 8px; }

  .hand-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 11px;
    margin-bottom: 6px;
    cursor: pointer;
    transition: all 0.18s;
    text-align: left;
  }

  .hand-row:hover { background: rgba(255, 255, 255, 0.055); border-color: rgba(255, 255, 255, 0.12); }
  .hand-row.dim { opacity: 0.55; }

  .row-main { display: flex; flex-direction: column; gap: 2px; min-width: 96px; }
  .hand-number { font-weight: 650; color: #fff; font-size: 13px; }
  .hand-time { font-size: 10px; color: #6b7280; }

  .row-mid { display: flex; flex-direction: column; gap: 2px; font-size: 11px; min-width: 76px; }
  .pot { color: #fbbf24; font-weight: 600; }
  .seats { color: #7c8497; }

  .row-badges { flex: 1; display: flex; flex-wrap: wrap; gap: 5px; justify-content: flex-end; }

  .badge {
    padding: 3px 8px;
    border-radius: 6px;
    font-size: 10px;
    font-weight: 600;
    white-space: nowrap;
  }

  .badge.you-won { background: rgba(46, 204, 113, 0.15); color: #2ecc71; }
  .badge.you-lost { background: rgba(239, 68, 68, 0.14); color: #ef4444; }
  .badge.showdown { background: rgba(52, 152, 219, 0.14); color: #3498db; }
  .badge.verdict.good { background: rgba(0, 212, 170, 0.16); color: #00d4aa; }
  .badge.verdict.partial { background: rgba(241, 196, 15, 0.14); color: #f1c40f; }
  .badge.verdict.pending { background: rgba(255, 255, 255, 0.05); color: #7c8497; }
  .badge.verdict.bad { background: rgba(231, 76, 60, 0.2); color: #ff6b5b; }

  .chevron { color: #4a5160; flex-shrink: 0; }

  .list-foot {
    padding: 10px 18px 14px;
    font-size: 10.5px;
    line-height: 1.5;
    color: #6b7280;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  /* ---- replayer ---- */
  .replayer { padding: 14px 18px 20px; overflow-y: auto; flex: 1; }

  .replay-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 14px;
  }

  .replay-title { display: flex; flex-direction: column; align-items: center; gap: 1px; }
  .replay-title strong { color: #fff; font-size: 15px; }
  .replay-title span { color: #6b7280; font-size: 10.5px; }

  .proof-banner {
    display: flex;
    gap: 11px;
    padding: 12px 14px;
    border-radius: 11px;
    margin-bottom: 14px;
    align-items: flex-start;
  }

  .proof-banner strong { display: block; font-size: 12.5px; margin-bottom: 3px; }
  .proof-banner p { margin: 0; font-size: 11px; line-height: 1.55; color: #a9b2c4; }
  .banner-mark { font-size: 15px; font-weight: 700; line-height: 1.2; flex-shrink: 0; }

  .proof-banner.good { background: rgba(0, 212, 170, 0.09); border: 1px solid rgba(0, 212, 170, 0.32); }
  .proof-banner.good strong, .proof-banner.good .banner-mark { color: #00d4aa; }
  .proof-banner.partial { background: rgba(241, 196, 15, 0.08); border: 1px solid rgba(241, 196, 15, 0.28); }
  .proof-banner.partial strong, .proof-banner.partial .banner-mark { color: #f1c40f; }
  .proof-banner.bad { background: rgba(231, 76, 60, 0.1); border: 1px solid rgba(231, 76, 60, 0.35); }
  .proof-banner.bad strong, .proof-banner.bad .banner-mark { color: #ff6b5b; }

  /* The limit rides INSIDE the green banner rather than under it, so the reader
     cannot take the verdict without the caveat. Same treatment ShuffleProof's
     `.limit.not-proven` gets: a tag, not a footnote. */
  .banner-caveat { margin-top: 7px !important; padding-top: 7px; border-top: 1px solid rgba(255, 255, 255, 0.09); }
  .caveat-tag {
    display: inline-block;
    margin-right: 6px;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(241, 196, 15, 0.14);
    border: 1px solid rgba(241, 196, 15, 0.34);
    color: #f1c40f;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    vertical-align: 1px;
  }

  .scrubber { display: flex; gap: 4px; margin-bottom: 8px; flex-wrap: wrap; }

  .stop {
    flex: 1;
    min-width: 74px;
    padding: 7px 6px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: #7c8497;
    font-size: 10.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.18s;
  }

  .stop.passed { color: #a9b2c4; border-color: rgba(255, 255, 255, 0.14); }
  .stop.active { background: rgba(0, 212, 170, 0.16); border-color: rgba(0, 212, 170, 0.45); color: #00d4aa; }

  .transport { display: flex; align-items: center; gap: 6px; margin-bottom: 14px; }

  .transport-btn {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: #a9b2c4;
    font-size: 10px;
    cursor: pointer;
    transition: all 0.18s;
  }

  .transport-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.1); color: #fff; }
  .transport-btn:disabled { opacity: 0.35; cursor: default; }
  .transport-btn.play { background: rgba(0, 212, 170, 0.14); border-color: rgba(0, 212, 170, 0.35); color: #00d4aa; }
  .transport-label { font-size: 10.5px; color: #6b7280; margin-left: 4px; }

  .felt {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 7px;
    padding: 16px 12px;
    border-radius: 14px;
    background: radial-gradient(ellipse at center, rgba(16, 84, 62, 0.55), rgba(8, 40, 30, 0.6));
    border: 1px solid rgba(0, 212, 170, 0.14);
    margin-bottom: 14px;
    min-height: 84px;
    justify-content: center;
  }

  .felt-empty { color: rgba(255, 255, 255, 0.4); font-size: 11.5px; }
  .felt-note { font-size: 9.5px; color: rgba(255, 255, 255, 0.42); letter-spacing: 0.3px; }

  .board { display: flex; gap: 7px; }

  .board-slot { display: flex; flex-direction: column; align-items: center; gap: 4px; }

  .deck-pos {
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 9px;
    color: #7c8497;
    white-space: nowrap;
  }

  .deck-pos.good { color: #00d4aa; }
  .deck-pos.bad { color: #ff6b5b; }

  .panel {
    background: rgba(0, 0, 0, 0.22);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 11px;
    padding: 13px 14px;
    margin-bottom: 12px;
  }

  .panel h4 {
    margin: 0 0 10px;
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.7px;
    color: #7c8497;
  }

  .panel-note { margin: 8px 0 0; font-size: 10.5px; line-height: 1.6; color: #6b7280; }

  .player-row {
    display: grid;
    grid-template-columns: 92px 1fr auto auto;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 6px;
  }

  .player-row.won { border-color: rgba(46, 204, 113, 0.28); background: rgba(46, 204, 113, 0.05); }
  .player-row.me { border-left: 3px solid #f1c40f; }

  .seat-tag { font-size: 11px; color: #a9b2c4; font-weight: 600; }
  .player-cards { display: flex; align-items: center; gap: 5px; flex-wrap: wrap; }
  .hidden-cards { font-size: 10.5px; color: #5a6172; font-style: italic; }
  .player-rank { font-size: 10.5px; color: #7c8497; }
  .won-amt { color: #2ecc71; font-weight: 650; font-size: 11.5px; }
  .lost-amt { color: #5a6172; font-size: 11.5px; }

  .log { display: flex; flex-direction: column; gap: 2px; max-height: 190px; overflow-y: auto; }

  .log-line {
    display: grid;
    grid-template-columns: 62px 62px 1fr;
    gap: 8px;
    font-size: 11.5px;
    padding: 3px 4px;
    border-radius: 5px;
    align-items: baseline;
  }

  .log-time { color: #5a6172; font-family: 'Monaco', 'Consolas', monospace; font-size: 10px; }
  .log-seat { color: #8b93a7; }
  .log-what { color: #c4cbd8; }

  .log-line.kind-fold .log-what { color: #ef4444; }
  .log-line.kind-check .log-what { color: #8b93a7; }
  .log-line.kind-call .log-what { color: #5aa9e6; }
  .log-line.kind-bet .log-what, .log-line.kind-raise .log-what { color: #fbbf24; }
  .log-line.kind-allin .log-what { color: #c084fc; font-weight: 650; }
  .log-line.kind-postblind .log-what { color: #2ecc71; }

  .proof-panel .proof-line {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 8px;
    align-items: center;
    margin-bottom: 7px;
  }

  .proof-label {
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6b7280;
    min-width: 150px;
  }

  .proof-panel code {
    flex: 1;
    min-width: 180px;
    background: rgba(0, 0, 0, 0.35);
    padding: 5px 8px;
    border-radius: 5px;
    color: #4ecdc4;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 9.5px;
    word-break: break-all;
    line-height: 1.5;
  }

  .proof-panel code.revealed { color: #f1c40f; }
  .proof-panel code.muted { color: #6b7280; font-style: italic; }

  .mini-btn {
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #8b93a7;
    padding: 4px 8px;
    border-radius: 5px;
    font-size: 10px;
    cursor: pointer;
  }

  .mini-btn:hover { background: rgba(255, 255, 255, 0.14); color: #fff; }

  .pot-panel {
    background: linear-gradient(135deg, rgba(251, 191, 36, 0.09), rgba(251, 191, 36, 0.04));
    border-color: rgba(251, 191, 36, 0.2);
  }

  .pot-total { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 8px; }
  .pot-total span { color: #8b93a7; font-size: 10.5px; text-transform: uppercase; letter-spacing: 0.6px; }
  .pot-total strong { color: #fbbf24; font-size: 20px; }

  .winner-line { display: flex; gap: 8px; align-items: baseline; font-size: 11.5px; color: #8b93a7; }
  .pot-type { color: #6b7280; font-size: 10px; }

  @media (max-width: 560px) {
    .hand-history-modal { padding: 8px; }
    .modal-content { max-height: 94vh; }
    .replayer { padding: 12px 12px 18px; }
    .hand-row { flex-wrap: wrap; gap: 8px; }
    .row-badges { justify-content: flex-start; }
    .player-row { grid-template-columns: 1fr; gap: 6px; }
    /* WHO ACTED IS NOT THE COLUMN TO DROP. This used to be
       `grid-template-columns: 54px 1fr` plus `.log-seat { display: none }`,
       which on a phone rendered the whole log as "05:46:19 AM calls" — every
       line anonymous, so the log could not answer the one question it exists
       to answer. The TIME column is the one with slack: it is monospace, fixed
       width and the least load-bearing thing on the line. */
    .log-line { grid-template-columns: 52px 46px 1fr; gap: 6px; font-size: 11px; }
    .log-seat { font-size: 10.5px; }
    .proof-label { min-width: 0; }
  }
</style>
