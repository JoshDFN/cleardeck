<script>
  // Hand history as a REPLAYER, not a table.
  //
  // Three things are different from what this component used to do.
  //
  // 1. It replays. You step through a hand street by street and watch the board
  //    arrive, instead of reading a static summary.
  //
  // 2. Every card it shows was re-derived in this browser from the seed the
  //    table committed to, using $lib/shuffle-verify.js. So the replay does not
  //    just say what happened; it shows that the cards follow from a commitment
  //    the table published and cannot have been swapped for others afterwards.
  //    No canister is asked to vouch for anything; the arithmetic runs here.
  //
  //    IT DOES NOT CLAIM THE ORDERING. That the commitment came BEFORE the deal
  //    is not something this page can see, and the wording never says otherwise:
  //    the only evidence of it in the record is a timestamp the table canister
  //    wrote about itself. What the page does instead is let the player witness
  //    the ordering for themselves -- see $lib/commitment-witness.js.
  //
  // 3. The action log is a real record. Every action carries its actor, its
  //    amount and the street it happened on, and the blinds are stated. Those
  //    fields were on the wire the whole time and the Candid declaration dropped
  //    two of them; see the header of $lib/hand-record.js and docs/DEFECTS.md
  //    H-32.
  //
  // It also no longer hides hands. The previous version filtered the list down to
  // hands whose `winners` or `showdown_players` contained your principal, which
  // silently dropped every hand you folded and lost -- indistinguishable from the
  // table losing your history. Hands you were not in are now shown greyed rather
  // than removed, hands the canister has no record of are listed as gaps, and the
  // filter is a visible toggle with counts.

  import { get } from 'svelte/store';
  import { NOTICE_LEAD, NOTICE_NO_RAKE, NOTICE_TERMS } from '../notices.js';
  import { history } from '$lib/canisters';
  import Card from './Card.svelte';
  import { scrollLock } from '$lib/scroll-lock.js';
  import logger from '$lib/logger.js';
  import { auth } from '$lib/auth.js';
  import { verifyHandLocally } from '$lib/shuffle-verify.js';
  // Importing this module installs the app-wide BigInt/JSON guard (see the
  // header of $lib/utils.js). `safeStringify` is the same rule, stated locally.
  // `formatTokenAmount` + `currencyOf` are the ONE formatter: this component used
  // to divide by 1e8 and append " ICP" unconditionally, which mislabels every
  // amount on a ckBTC table as ICP.
  import { currencyOf, formatTokenAmount, safeStringify } from '$lib/utils.js';
  import {
    STREET_UNKNOWN, blindsForHand, logGroupsFor, normalizeAction, potAudit,
    reachedStreet,
  } from '$lib/hand-record.js';
  import {
    readSighting, recordSighting, verdictForPasted, verdictForSighting,
  } from '$lib/commitment-witness.js';

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
  let currency = $state('ICP');
  /** What the LIVE table view says, and how far it can be trusted. */
  let tableFacts = $state(null);
  /** Bumped whenever a commitment sighting is written, so derived reads re-run. */
  let sightingEpoch = $state(0);
  /** A commitment the player pasted in to compare with this hand's. */
  let pasted = $state('');

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
  const num = (v) => (typeof v === 'bigint' ? Number(v) : Number(v ?? 0));

  /** Key the commitment-sighting store is scoped by. */
  const tableKey = () => text(tableId) || 'table';

  // -------------------------------------------------------------------------
  // Normalising the two record shapes into one model
  // -------------------------------------------------------------------------

  /** The table canister's own `HandHistory`. */
  function fromTableRecord(record, requestedNumber) {
    const proof = record.shuffle_proof || {};
    const revealed = opt(proof.revealed_seed);
    const showdown = (record.showdown_players || []).map((p) => ({
      seat: Number(p.seat),
      principal: text(p.principal),
      cards: opt(p.cards),
      rank: opt(p.hand_rank),
      won: num(p.amount_won),
    }));
    const winners = (record.winners || []).map((w) => ({
      seat: Number(w.seat),
      principal: text(w.principal),
      amount: num(w.amount),
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
      // The TABLE record carries no blind level of its own. Saying so is the
      // difference between a level read off this hand and a level read off the
      // table as it is configured today.
      blinds: null,
      source: 'table canister',
      verification: null,
    };
  }

  /** The history canister's richer `HandHistoryRecord` (blinds and streets included). */
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
      won: num(p.amount_won),
    }));
    const winners = (record.winners || []).map((w) => ({
      seat: Number(w.seat),
      principal: text(w.principal),
      amount: num(w.amount),
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
      potTotal: num(record.total_pot),
      // This record DOES carry the level that was in force for this hand.
      blinds: {
        small: num(record.small_blind), big: num(record.big_blind), ante: num(record.ante),
      },
      source: 'history canister',
      verification: null,
    };
  }

  // -------------------------------------------------------------------------
  // Loading
  // -------------------------------------------------------------------------

  /**
   * Reads the live table view for the facts the hand record does not carry: the
   * blind level, which seats the button had on the blinds, and the display names
   * of the principals currently seated.
   *
   * Everything read here is about the table NOW. `blindsForHand` decides how far
   * that is evidence about the hand being replayed, and refuses to attribute a
   * blind post to a seat once the button has moved on.
   */
  function factsFrom(view) {
    if (!view) return null;
    const cfg = view.config || {};
    const phase = view.phase && typeof view.phase === 'object' ? Object.keys(view.phase)[0] : null;
    return {
      config: {
        smallBlind: num(cfg.small_blind), bigBlind: num(cfg.big_blind), ante: num(cfg.ante),
      },
      blindSeats: {
        sb: Number(view.small_blind_seat), bb: Number(view.big_blind_seat),
        dealer: Number(view.dealer_seat),
      },
      liveHandNumber: num(view.hand_number),
      phase,
      settled: phase !== null && SETTLED_PHASES.includes(phase),
      // Seat -> {principal, name}. Only ever used when the principal in this map
      // is the same principal the hand record has for that seat.
      seated: (view.players || []).map((p) => opt(p)).filter(Boolean).map((p) => ({
        seat: Number(p.seat),
        principal: text(p.principal),
        name: opt(p.display_name),
      })),
    };
  }

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
        let view = null;
        try {
          view = opt(await actor.get_table_view());
          if (view?.config?.max_players) maxPlayers = Number(view.config.max_players) || 9;
          currency = currencyOf(view?.config) ?? 'ICP';
          tableFacts = factsFrom(view);
        } catch (e) {
          logger.debug('HandHistory: table view unavailable', e);
        }

        // `handNumber` is the hand being PLAYED. The table canister writes that
        // hand's record when the hand STARTS, with `revealed_seed` empty, so it
        // comes back here looking like an ordinary finished hand whose seed has
        // gone missing. It has not: the seed is sealed on purpose until the hand
        // ends. Saying so is the difference between "we are not showing you this
        // yet, and here is why" and an unexplained blank.
        const topIsLive = tableFacts !== null && !tableFacts.settled;

        // THE ONE MOMENT WORTH REMEMBERING. While a hand is still running its
        // commitment is published and its seed is not, so reading the commitment
        // now and comparing it after the reveal is a witness of the ordering that
        // needs no canister clock. Noted automatically, because a feature that
        // requires the player to keep their own text file is not a feature.
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

  // ---- the log, the blinds, and whether the log adds up --------------------

  const blinds = $derived(selected ? blindsForHand({ hand: selected, tableFacts }) : null);
  const logGroups = $derived(selected ? logGroupsFor({ hand: selected, blinds }) : []);
  const audit = $derived(selected ? potAudit({ hand: selected, blinds }) : null);
  const streetsRecorded = $derived(
    selected ? selected.actions.every((a) => a.street !== null) : true,
  );

  /** The principal the hand record itself attributes to a seat, if any. */
  function principalForSeat(hand, seat) {
    const shown = hand.showdown.find((p) => p.seat === seat);
    if (shown?.principal) return shown.principal;
    const won = hand.winners.find((w) => w.seat === seat);
    return won?.principal ?? null;
  }

  /**
   * A name for a seat, used only when it can be justified.
   *
   * The hand record carries no display names, so the only source is the LIVE
   * table. A live name is used only when the principal sitting in that seat now
   * is the same principal the hand record attributes to it -- otherwise the chair
   * changed hands and the name would be somebody else's.
   */
  function seatName(hand, seat) {
    const recorded = principalForSeat(hand, seat);
    if (!recorded) return null;
    const live = (tableFacts?.seated || []).find((p) => p.seat === seat);
    if (!live || live.principal !== recorded) return null;
    return live.name || null;
  }

  function openHand(hand) {
    selected = hand;
    stopIndex = 0;
    playing = false;
    pasted = '';
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

  // ---- witnessing the ordering ---------------------------------------------

  const sighting = $derived.by(() => {
    void sightingEpoch;
    if (!selected) return null;
    return readSighting({ tableId: tableKey(), handNumber: selected.handNumber });
  });
  const sightingVerdict = $derived(
    selected ? verdictForSighting(sighting, selected.proof.seedHash) : null,
  );
  const pastedVerdict = $derived(
    selected ? verdictForPasted(pasted, selected.proof.seedHash) : null,
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

  function formatClock(ns) {
    if (!ns) return '--:--';
    return new Date(Number(ns) / 1_000_000).toLocaleTimeString([], {
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
  }

  /**
   * The log's own clock: 24-hour, so a row is one line.
   *
   * Every row carried "03:35:57 PM", which wrapped in the time column and made
   * the whole log twice as tall as it needed to be.
   */
  function formatLogClock(ns) {
    if (!ns) return '--:--';
    return new Date(Number(ns) / 1_000_000).toLocaleTimeString('en-GB', {
      hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false,
    });
  }

  /** A browser-clock millisecond stamp (the sighting store's own clock). */
  function formatLocalClock(ms) {
    if (!ms) return '--:--';
    return new Date(Number(ms)).toLocaleTimeString([], {
      hour: '2-digit', minute: '2-digit', second: '2-digit',
    });
  }

  /** One formatter, currency-aware, for every amount this modal renders. */
  const money = (amount) => formatTokenAmount(amount, { currency, includeUnit: true });

  function rankName(rank) {
    const value = Array.isArray(rank) ? rank[0] : rank;
    if (!value || typeof value !== 'object') return null;
    const key = Object.keys(value)[0];
    return key ? key.replace(/([A-Z])/g, ' $1').trim() : null;
  }

  function seatLabel(seat) {
    return seat === null || seat === undefined ? 'Seat not recorded' : `Seat ${seat + 1}`;
  }

  /** "Seat 2 (Nakamoto) · you", every part of it justified above. */
  function actorLabel(hand, seat) {
    if (seat === null || seat === undefined) return 'Seat not recorded';
    const name = seatName(hand, seat);
    const mine = principalForSeat(hand, seat) === myPrincipal && !!myPrincipal;
    return `${seatLabel(seat)}${name ? ` (${name})` : ''}${mine ? ' · you' : ''}`;
  }

  /**
   * What an amount on a log line MEANS, spelled out rather than assumed.
   *
   * Only the surprising one is printed on the line. A call's or a bet's amount is
   * the chips that action put in, which is what a reader assumes; a raise's or an
   * all-in's is the total the street was raised TO, which is not, and which is
   * also why the pot audit refuses to sum a log containing one.
   */
  const STREET_TOTAL_NOTE = 'the total this street was raised to';

  const verdictOf = (hand) => {
    const v = hand.verification;
    if (!v) return { tone: 'pending', label: 'seed not revealed' };
    if (v.ok) return { tone: 'good', label: `${v.cardsMatched} cards re-derived here` };
    if (v.commitment?.match && v.cardsChecked === 0) {
      return { tone: 'partial', label: 'commitment checked here' };
    }
    return { tone: 'bad', label: 'DID NOT verify' };
  };

  /** How many of the hands on screen this browser actually re-derived. */
  const rederivedCount = $derived(hands.filter((h) => h.verification?.ok).length);

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
    const b = blindsForHand({ hand, tableFacts });
    const a = potAudit({ hand, blinds: b });
    const payload = {
      note: 'ClearDeck hand record. The deck below was re-derived in the player\'s '
        + 'browser from revealed_seed using the algorithm in docs/SHUFFLE-SPEC.md. '
        + 'Verifying a shuffle proves the deal was not tampered with after the '
        + 'commitment; it proves nothing else about the engine.',
      hand_number: hand.handNumber,
      recorded_at: formatTimestamp(hand.timestamp),
      source: hand.source,
      currency,
      shuffle_proof: {
        seed_hash: hand.proof.seedHash,
        revealed_seed: hand.proof.revealedSeed,
      },
      seats_seen: hand.seats,
      community_cards: hand.community.map(codeOf),
      blinds: b.level
        ? { ...b.level, level_source: b.levelSource, seats: b.seats, seat_source: b.seatSource }
        : 'not known for this hand',
      actions: hand.actions.map((a2) => ({
        seat: a2.seat,
        action: a2.kind,
        amount: a2.amount,
        amount_means: a2.amountMeaning,
        street: a2.street ?? 'not recorded',
        at: formatClock(a2.timestamp),
      })),
      pot_audit: a,
      showdown: hand.showdown.map((p) => ({
        seat: p.seat, cards: p.cards ? [p.cards[0], p.cards[1]].map(codeOf) : null,
        hand_rank: rankName(p.rank), won_e8s: p.won,
      })),
      winners: hand.winners.map((w) => ({ seat: w.seat, amount_e8s: w.amount })),
      local_verification: v ? {
        commitment_recomputed_in_browser: v.commitment.computed,
        // NOT `commitment_published_before_deal`, which is what this field used
        // to be called. The record does not establish that, and a field name in
        // a file the player keeps is a claim like any other.
        commitment_in_the_hand_record: v.commitment.committed,
        commitment_matches: v.commitment.match,
        players_dealt_in: v.playersDetermined ? v.layout?.players : 'not determined',
        cards_checked: v.cardsChecked,
        cards_matched: v.cardsMatched,
        rejected_draws: v.rejections,
        derived_deck: v.deckCodes,
      } : 'seed not revealed for this hand',
      not_proven: 'That the commitment was made BEFORE the cards were dealt. The only '
        + 'evidence of that in the record is shuffle_proof.timestamp, which the table '
        + 'canister wrote about itself. Every other statement in this file holds even if '
        + 'that timestamp is wrong.',
      ordering_witnessed_by_this_browser: sighting && sighting.handNumber === hand.handNumber
        ? {
          commitment_seen: sighting.seedHash,
          seen_at_browser_local_time: new Date(sighting.at).toISOString(),
          board_cards_on_the_table_then: sighting.boardCount,
          table_phase_then: sighting.phase,
          matches_the_recorded_commitment:
            sighting.seedHash === String(hand.proof.seedHash || '').toLowerCase(),
        }
        : 'this browser has no sighting of this hand\'s commitment from while it was running',
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
  // `onkeydown` that used to sit on the backdrop could never fire, the backdrop
  // is `tabindex="-1"` and nothing ever focuses it, so Escape worked in
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

  <!-- The replayer is a two-column document on a wide screen (see the
       `.replay-cols` rule): everything it claims then fits in one frame instead
       of hiding the action log a scroll below the verdict, which is the shape of
       docs/DEFECTS.md H-30. The LIST keeps the narrow dialog it always had. -->
  <div class="modal-content" class:wide={!!selected} use:scrollLock>
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
          <button class="ghost-btn back-to-list" onclick={() => { selected = null; playing = false; }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15,18 9,12 15,6"/>
            </svg>
            All hands
          </button>
          <div class="replay-title">
            <strong class="hand-number">Hand #{selected.handNumber}</strong>
            <span class="timestamp">{formatTimestamp(selected.timestamp)}</span>
          </div>
          <button class="ghost-btn" onclick={() => downloadHand(selected)}>Download</button>
        </div>

        <div class="replay-cols">
        <div class="replay-col col-a">

        <!-- proof banner: the claim, stated only as strongly as it was checked.
             THE ORDERING IS NOT PART OF THE CLAIM. This banner used to lead with
             "These cards were fixed before the hand was played … before any card
             was dealt", which is the ordering claim ShuffleProof.svelte retires
             by name ("Not proven: that the commitment came before the cards").
             Two surfaces of the same product cannot say opposite things about
             the same fact, and the one a player opens to review a hand they lost
             is the worse one to over-claim on. What the browser actually did is
             stated instead, and the ordering is carried beside it as the open
             question, with the panel that settles it directly underneath. -->
        {#if selected.verification?.ok}
          <div class="proof-banner good">
            <span class="banner-mark">✓</span>
            <div>
              <strong>These cards follow from the seed the table committed to.</strong>
              <p>
                All <span class="tally">{selected.verification.cardsMatched}</span> cards below were
                re-derived in this browser from the revealed seed, and its
                <span class="method-name">SHA-256</span> recomputed here matches the commitment the
                table published for this hand. No canister was asked to confirm it.
              </p>
              <p class="banner-caveat">
                <span class="caveat-tag">Not proven</span>
                That the commitment came <em>before</em> the deal. The
                <span class="timestamp">{formatClock(selected.timestamp)}</span> above is a clock read
                off the table canister; this page did not watch the order of events, and everything
                above stays true even if that clock is wrong. The panel below is how you settle it
                without trusting that clock.
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

        <!-- THE ORDERING, WITNESSED BY THE READER RATHER THAN ASSERTED BY US.
             The commitment is on screen from the moment cards are dealt and the
             seed is not published until the hand ends, so anyone who reads the
             commitment mid-hand and compares it afterwards has established the
             ordering themselves, with no canister clock in the argument. This
             browser does the reading automatically (see
             $lib/commitment-witness.js); the box is for a commitment the player
             kept somewhere else. -->
        <div class="panel witness-panel">
          <h4>Did the commitment come first? Check it yourself</h4>
          {#if sightingVerdict?.witnessed}
            <div class="witness-verdict good" data-witness="matched">
              <span class="banner-mark">✓</span>
              <div>
                <strong>You saw this commitment while the hand was still running.</strong>
                <p>
                  This browser read the commitment for
                  <span class="hand-number">hand #{sighting.handNumber}</span> at
                  <span class="timestamp">{formatLocalClock(sighting.at)}</span> by your own clock, when
                  the table had published no seed for it and the board showed
                  <span class="tally">{sighting.boardCount}</span> card(s). The seed the table revealed
                  afterwards hashes to that same commitment. So the deck was already fixed at the moment
                  you looked: before every card dealt after it, and before every action taken after it.
                  Nothing in that sentence relies on the table's clock.
                </p>
              </div>
            </div>
          {:else if sightingVerdict?.tone === 'bad'}
            <div class="witness-verdict bad" data-witness="differs">
              <span class="banner-mark">✗</span>
              <div>
                <strong>The commitment changed under you.</strong>
                <p>
                  This browser read <code class="mono">{sighting.seedHash.slice(0, 16)}…</code> for
                  <span class="hand-number">hand #{sighting.handNumber}</span> while it was still
                  running, and the finished record carries a different commitment. Do not trust this
                  table.
                </p>
              </div>
            </div>
          {:else}
            <p class="panel-note" data-witness="none">
              This browser has no sighting of this hand's commitment from while it was running, so it
              cannot vouch for the order. It notes one automatically whenever this panel is open during
              a live hand, open it once mid-hand and the check above appears for that hand by itself.
            </p>
          {/if}

          <label class="witness-field">
            <span class="witness-label">Compare a commitment you saved</span>
            <input
              class="witness-input mono"
              type="text"
              spellcheck="false"
              autocomplete="off"
              placeholder="paste the commitment you copied while the hand was running"
              bind:value={pasted}
            />
          </label>
          {#if pastedVerdict && pastedVerdict.tone !== 'none'}
            <p class="paste-verdict {pastedVerdict.tone}" data-paste={pastedVerdict.tone}>
              {pastedVerdict.detail}
            </p>
          {/if}
        </div>

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
          <span class="transport-label">{stop?.label}</span>
          <!-- Position as dots rather than "4 of 5": the scrubber above already
               names every stop, and a digit here would be a number on a money
               surface that nothing asserts. -->
          <span class="transport-dots" aria-hidden="true">
            {#each stops as _, at}<span class="dot" class:on={at <= stopIndex}></span>{/each}
          </span>
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
                    <span class="deck-pos mono" class:good={slot.check.match} class:bad={!slot.check.match}>
                      #{slot.check.positions[0]} {slot.check.match ? '✓' : '✗'}
                    </span>
                  {:else}
                    <span class="deck-pos mono">·</span>
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
                <span class="seat-tag seat-label">{actorLabel(selected, player.seat)}</span>
                <div class="player-cards">
                  {#if stop?.reveal && player.cards}
                    <Card card={player.cards[0]} small={true} />
                    <Card card={player.cards[1]} small={true} />
                    {#if seatCheck}
                      <span class="deck-pos mono" class:good={seatCheck.match} class:bad={!seatCheck.match}>
                        #{seatCheck.positions[0]} #{seatCheck.positions[1]} {seatCheck.match ? '✓' : '✗'}
                      </span>
                    {/if}
                  {:else}
                    <span class="hidden-cards">face down until showdown</span>
                  {/if}
                </div>
                <span class="player-rank">{stop?.reveal ? (rankName(player.rank) || '') : ''}</span>
                <span class="player-result">
                  {#if player.won > 0}<span class="won-amt replay-money">+{money(player.won)}</span>{:else}<span class="lost-amt">·</span>{/if}
                </span>
              </div>
            {/each}
          {:else}
            <p class="panel-note">
              Nobody showed cards in this hand: it ended before a showdown, so the table never revealed
              anyone's hole cards. Seats seen in the log:
              <span class="seat-label">{selected.seats.map(seatLabel).join(', ') || 'none'}</span>.
            </p>
          {/if}
        </div>

        </div>
        <div class="replay-col col-b">

        <!-- THE ACTION LOG, street by street, every amount stated.
             The A/B this lost badly: PokerNow's session log names the street and
             carries an explicit amount on 6 of its 11 lines; ours carried an
             amount on NONE of five and never mentioned the blinds. Both fields
             were on the wire the whole time -- see $lib/hand-record.js. -->
        <div class="panel log-panel">
          <h4>Action log</h4>
          {#if logGroups.length}
            <div class="log">
              {#each logGroups as group}
                <div class="log-street">
                  <span class="street-name">{group.street}</span>
                  {#if group.boardAfter !== null && group.boardAfter > 0}
                    <span class="street-board">board of <span class="tally">{group.boardAfter}</span></span>
                  {/if}
                </div>
                {#each group.lines as line}
                  <div class="log-line kind-{String(line.kind).toLowerCase()}" class:reconstructed={line.reconstructed}>
                    <span class="log-index mono">#{line.index}</span>
                    <span class="log-time timestamp">{line.timestamp ? formatLogClock(line.timestamp) : '·'}</span>
                    <span class="log-seat seat-label">{actorLabel(selected, line.seat)}</span>
                    <span class="log-what">
                      <span class="log-verb">{line.word}</span>{#if line.amount}{' '}<span class="log-amount replay-money">{money(line.amount)}</span>{/if}
                      {#if line.amountMeaning === 'street-total'}<span class="amount-note">({STREET_TOTAL_NOTE})</span>{/if}
                    </span>
                  </div>
                {/each}
              {/each}
            </div>

            <!-- Does the log account for the pot? When every amount is an
                 increment it must, to the last e8, which makes the log check
                 itself. When it cannot, the reason is named. -->
            {#if audit?.checkable}
              <p class="audit {audit.match ? 'good' : 'bad'}" data-audit={audit.match ? 'balanced' : 'unbalanced'}>
                {#if audit.match}
                  ✓ The blinds and every amount above add up to the pot the table paid out:
                  <strong class="replay-money">{money(audit.sum)}</strong>. Nothing is missing from this log.
                {:else}
                  ✗ The blinds and the amounts above come to
                  <strong class="replay-money">{money(audit.sum)}</strong>, but the table paid out
                  <strong class="replay-money">{money(audit.pot)}</strong>. One of the two is wrong.
                {/if}
              </p>
            {:else if audit}
              <p class="panel-note" data-audit="not-checkable">
                Not summable here: {audit.reason}. The pot below is the canister's own figure.
              </p>
            {/if}

            <p class="panel-note provenance">
              Streets come from each action's own <code class="mono">phase</code> field and amounts from
              its <code class="mono">amount</code> field, both read off the table canister's record.
              {#if !streetsRecorded}
                Some actions in this hand carry no street, and those are grouped under
                “{STREET_UNKNOWN}” rather than guessed at.
              {/if}
              {#if blinds?.level}
                Blind posts are <em>not</em> actions in the record, so the level above is
                {blinds.levelSource}{#if blinds.seats}, and the seats are attributed from
                {blinds.seatSource}{:else}, and no seat is attributed to them because the table's
                dealer button has already moved on{/if}.
              {/if}
            </p>
          {:else}
            <p class="panel-note">No actions recorded for this hand.</p>
          {/if}
        </div>

        <!-- the proof, in full -->
        <div class="panel proof-panel">
          <h4>Shuffle proof</h4>
          <div class="proof-line">
            <!-- NOT "committed before the deal". This label asserted the exact
                 ordering the banner above declines to claim, in the same modal. -->
            <span class="proof-label">commitment in this hand's record</span>
            <code>{selected.proof.seedHash || 'n/a'}</code>
            <button class="mini-btn" onclick={() => copyToClipboard(selected.proof.seedHash, 'hash')}>
              {copied === 'hash' ? '✓' : 'Copy'}
            </button>
          </div>
          <div class="proof-line">
            <span class="proof-label">seed, published with the finished hand</span>
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
              <span class="proof-label"><span class="method-name">SHA-256</span> recomputed here</span>
              <code class:revealed={!selected.verification.commitment.match}>{selected.verification.commitment.computed}</code>
            </div>
            <p class="panel-note layout-note">
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
            <strong class="replay-money">{money(selected.potTotal)}</strong>
          </div>
          {#each selected.winners as winner}
            <div class="winner-line">
              <span class="seat-label">{actorLabel(selected, winner.seat)}</span>
              <span class="won-amt replay-money">+{money(winner.amount)}</span>
              {#if winner.potType}<span class="pot-type">{winner.potType}</span>{/if}
            </div>
          {/each}
        </div>

        </div>
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
            {#if liveSighting}
              This browser has noted its commitment, at
              <span class="timestamp">{formatLocalClock(liveSighting.at)}</span> by your own clock, so
              you can check for yourself afterwards that the deck was already fixed now.
            {/if}
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
                <span class="pot">{money(hand.potTotal)}</span>
                <span class="seats">{hand.seats.length || '?'} seats</span>
              </div>
              <div class="row-badges">
                {#if participated(hand)}
                  <span class="badge" class:you-won={iWon} class:you-lost={!iWon}>
                    {iWon ? 'You won' : 'You lost'}
                  </span>
                {/if}
                <span class="badge street">to the {reachedStreet(hand.community.length)}</span>
                <span class="badge actions">{hand.actions.length} actions</span>
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
          <!-- COUNTED, NOT ASSERTED. This line used to read "Every hand above was
               re-derived in this browser from its own revealed seed", which is
               false for any hand still awaiting its seed, and one of those is
               listed here whenever a hand is in play. -->
          {rederivedCount} of {hands.length} hand(s) above were re-derived in this browser from their
          own revealed seed. Nothing here was verified by a canister. Open one to step through it.
        </div>
      {/if}
    {/if}

    <!-- THE FOUR PLAYER-PROTECTION NOTICES, ON THIS SURFACE.
         This modal is `position: fixed; inset: 0` over a scrim at 72% black with
         a 4px blur, so the copy in `.alpha-warning-banner` and
         `.footer-disclaimer` behind it is on the page and unreadable to the
         person looking at the screen. docs/WAVE-04.md §2 is the same finding one
         surface over: a notice in the bundle and not on the screen is invisible
         to every check this repo has, and `make hygiene` greps the source.

         So the notices ride INSIDE the dialog, verbatim, pinned outside the
         scrolling region (`flex-shrink: 0`) so they cannot be scrolled away, on
         both the list and the replayer, at every viewport. Nothing anywhere else
         is weakened; this is an additional copy. `handreplay.mjs` and
         `handhistory.mjs` both measure it on the RENDERED page, including
         whether anything is painted on top of it. -->
    <div class="legal">
      <p class="legal-warning">
        <strong>DISCLAIMER:</strong> {NOTICE_LEAD}. {NOTICE_TERMS.warningBody}
      </p>
      <p class="legal-norake">
        <strong class="norake">{NOTICE_NO_RAKE}</strong> Every amount in
        this record is money that went to a player.
      </p>
    </div>
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
    /* `.wide` is only ever set while a hand is open, so the LIST keeps the
       narrow dialog it always had and its captures are unchanged. */
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

  @media (min-width: 900px) {
    .modal-content.wide { max-width: 1040px; max-height: 92vh; }
  }

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

  /* ---- the protected notices, pinned inside the dialog ---- */
  .legal {
    flex-shrink: 0;
    padding: 10px 16px 12px;
    border-top: 1px solid rgba(239, 68, 68, 0.22);
    background: rgba(239, 68, 68, 0.06);
  }

  .legal p { margin: 0; font-size: 10px; line-height: 1.5; color: #cfd4de; }
  .legal p + p { margin-top: 4px; }
  .legal strong { color: #ff8f85; }
  .legal .norake { color: #00d4aa; }

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
  .badge.street, .badge.actions { background: rgba(255, 255, 255, 0.05); color: #8b93a7; }
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

  /* A phone gets one column, in reading order. A wide screen gets two, so the
     verdict, the street navigation, the board, the log and the pot are all in
     one frame -- the thing H-30 says the fairness sidebar fails to do. */
  .replay-cols { display: flex; flex-direction: column; }

  @media (min-width: 900px) {
    .replay-cols {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      gap: 0 16px;
      align-items: start;
    }
  }

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

  /* ---- witnessing the ordering ---- */
  .witness-panel { border-color: rgba(0, 212, 170, 0.18); }

  .witness-verdict { display: flex; gap: 10px; align-items: flex-start; }
  .witness-verdict strong { display: block; font-size: 12px; margin-bottom: 3px; }
  .witness-verdict p { margin: 0; font-size: 11px; line-height: 1.55; color: #a9b2c4; }
  .witness-verdict.good strong, .witness-verdict.good .banner-mark { color: #00d4aa; }
  .witness-verdict.bad strong, .witness-verdict.bad .banner-mark { color: #ff6b5b; }

  .witness-field { display: block; margin-top: 10px; }
  .witness-label {
    display: block;
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6b7280;
    margin-bottom: 4px;
  }

  .witness-input {
    width: 100%;
    box-sizing: border-box;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 7px;
    color: #4ecdc4;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 10px;
    padding: 7px 9px;
  }

  .witness-input::placeholder { color: #5a6172; font-family: inherit; }
  .witness-input:focus { outline: none; border-color: rgba(0, 212, 170, 0.45); }

  .paste-verdict { margin: 7px 0 0; font-size: 10.5px; line-height: 1.55; }
  .paste-verdict.good { color: #00d4aa; }
  .paste-verdict.bad { color: #ff6b5b; }
  .paste-verdict.warn { color: #f1c40f; }

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
  .transport-label { font-size: 10.5px; color: #a9b2c4; margin-left: 4px; font-weight: 600; }

  .transport-dots { display: inline-flex; gap: 4px; margin-left: auto; }
  .transport-dots .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.14);
  }
  .transport-dots .dot.on { background: #00d4aa; }

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
    padding: 11px 13px;
    margin-bottom: 10px;
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
  .panel-note code { font-size: 9.5px; color: #4ecdc4; }

  .player-row {
    display: grid;
    grid-template-columns: 132px 1fr auto auto;
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

  /* ONE SCROLLER, NOT TWO. This used to be `max-height: 190px; overflow-y: auto`,
     which put the log inside a nested scroller: content a player can only reach
     by scrolling a container they have no reason to know exists. That is exactly
     the failure docs/DEFECTS.md H-30 records for the fairness sidebar. The
     replayer's own scroller is the only one now, and on a wide screen the
     two-column layout means there is usually nothing to scroll at all. */
  .log { display: flex; flex-direction: column; gap: 2px; }

  /* A street header is the field PokerNow's log has and ours did not. */
  .log-street {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 6px 0 2px;
    padding-bottom: 2px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }

  .log-street:first-child { margin-top: 0; }

  .street-name {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.7px;
    color: #00d4aa;
  }

  .street-board { font-size: 9.5px; color: #6b7280; }

  .log-line {
    display: grid;
    /* The actor column is WIDE and NOWRAP on purpose. At 148px "Seat 2 (Nakamoto)
       · you" wrapped, which doubled the height of every row in the log and put
       the sum-vs-pot audit line below the dialog. */
    grid-template-columns: 24px 60px 172px 1fr;
    gap: 8px;
    font-size: 11.5px;
    padding: 2px 4px;
    border-radius: 5px;
    align-items: baseline;
  }

  .log-index { color: #4a5160; font-size: 9.5px; }
  .log-time { color: #5a6172; font-family: 'Monaco', 'Consolas', monospace; font-size: 10px; }
  .log-seat { color: #8b93a7; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .log-what { color: #c4cbd8; }
  .log-amount { font-weight: 650; margin-left: 2px; }
  .amount-note { color: #6b7280; font-size: 9.5px; margin-left: 4px; }
  .log-line.reconstructed { background: rgba(46, 204, 113, 0.05); }

  .log-line.kind-fold .log-what { color: #ef4444; }
  .log-line.kind-check .log-what { color: #8b93a7; }
  .log-line.kind-call .log-what { color: #5aa9e6; }
  .log-line.kind-bet .log-what, .log-line.kind-raise .log-what { color: #fbbf24; }
  .log-line.kind-allin .log-what { color: #c084fc; font-weight: 650; }
  .log-line.kind-postblind .log-what, .log-line.kind-postante .log-what { color: #2ecc71; }

  .audit {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: 10.5px;
    line-height: 1.6;
  }

  .audit.good { background: rgba(0, 212, 170, 0.08); border: 1px solid rgba(0, 212, 170, 0.28); color: #a9b2c4; }
  .audit.good strong { color: #00d4aa; }
  .audit.bad { background: rgba(231, 76, 60, 0.1); border: 1px solid rgba(231, 76, 60, 0.35); color: #ffb3ab; }
  .audit.bad strong { color: #ff6b5b; }

  .provenance { border-top: 1px solid rgba(255, 255, 255, 0.06); padding-top: 8px; }

  /* Not decoration: `.method-name` is the class the screenshot harness's reviewed
     non-monetary allowlist attributes the string "SHA-256" to, so the hash
     algorithm's name is a named method rather than an unaccounted-for number on a
     surface full of money (tools/shots/token-allowlist.mjs, static-claim-copy). */
  .method-name { color: inherit; font-weight: inherit; }

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
       which on a phone rendered the whole log as "05:46:19 AM calls", every
       line anonymous, so the log could not answer the one question it exists
       to answer. The TIME column is the one with slack: it is monospace, fixed
       width and the least load-bearing thing on the line. The INDEX column goes
       first, because a line's ordinal is recoverable by counting. */
    .log-line { grid-template-columns: 46px 104px 1fr; gap: 6px; font-size: 11px; }
    .log-index { display: none; }
    .log-seat { font-size: 10.5px; }
    .proof-label { min-width: 0; }
    .legal { padding: 8px 12px 10px; }
    .legal p { font-size: 9.5px; }
  }
  /* THE PHONE: the close control at the 44 px touch floor. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .close-btn { width: var(--cd-touch-min); height: var(--cd-touch-min); min-width: var(--cd-touch-min); }
  }
</style>
