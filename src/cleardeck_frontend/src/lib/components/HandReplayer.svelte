<script>
  /**
   * One hand, replayed: the table first, then the transport, then the verdict;
   * the action log beside it on a wide screen, under it on a phone.
   *
   * Stops are per ACTION (lib/replay-stops.js): the street scrubber jumps to a
   * street's reveal, the arrows and the timeline walk one action, play runs at
   * the house tempo (500 ms per action, a second on a reveal). The log's line
   * for the current stop is lit. Equity is shown at any stop where every live
   * hand is on its face (a showdown hand), from lib/equity.js, and never
   * modelled.
   *
   * HARNESS CONTRACT (handreplay.mjs): `.replayer` is the scroller; `.replay-title
   * .hand-number` reads "Hand #N"; the pieces carry their own contracts.
   */
  import ReplayTable from './ReplayTable.svelte';
  import ReplayTransport from './ReplayTransport.svelte';
  import ReplayLog from './ReplayLog.svelte';
  import ReplayVerdict from './ReplayVerdict.svelte';
  import logger from '$lib/logger.js';
  import { safeStringify } from '$lib/utils.js';
  import { blindsForHand, logGroupsFor, potAudit } from '$lib/hand-record.js';
  import {
    buildStops, equityAllowedAt, liveSeatsAt, seatSpots, stopBeatMs, streetOfStop, streetStops,
  } from '$lib/replay-stops.js';
  import { codeOf, principalForSeat, rankName, seatCardChecks, seatNameFor, seatOfPrincipal } from '$lib/hand-history-records.js';
  import { computeEquity } from '$lib/equity.js';
  import { verdictForPasted } from '$lib/commitment-witness.js';
  import { handToText } from '$lib/hand-text-export.js';

  const {
    hand,
    tableFacts = null,
    myPrincipal = null,
    currency = 'ICP',
    money = (v) => String(v),
    figure = (v) => String(v),
    sighting = null,
    sightingVerdict = null,
    tableName = 'ClearDeck table',
    maxPlayers = 9,
    onBack = () => {},
  } = $props();

  let stopIndex = $state(0);
  let playing = $state(false);
  let pasted = $state('');
  let copied = $state(null);

  // ---- the record, folded ---------------------------------------------------
  const blinds = $derived(blindsForHand({ hand, tableFacts }));
  const logGroups = $derived(logGroupsFor({ hand, blinds }));
  const audit = $derived(potAudit({ hand, blinds }));
  const streetsRecorded = $derived(hand.actions.every((a) => a.street !== null));
  const stops = $derived(buildStops({ hand, blinds, logGroups }));
  const streets = $derived(streetStops(stops));
  const stop = $derived(stops[Math.min(stopIndex, stops.length - 1)] || null);
  const currentStreet = $derived(streetOfStop(stops, stop?.index ?? 0));
  const heroSeat = $derived(seatOfPrincipal(hand, myPrincipal));
  const spots = $derived(seatSpots({ seats: hand.seats, heroSeat, maxPlayers, rx: 0.37, ry: 0.42 }));
  const boardChecks = $derived((hand.verification?.checks || []).filter((c) => c.kind === 'board'));
  const seatChecks = $derived(hand.verification?.seatCards || seatCardChecks(hand.verification, hand));
  const showdownSeats = $derived(hand.showdown.filter((p) => p.cards).map((p) => p.seat));
  const dealerSeat = $derived(blinds?.seats && tableFacts?.blindSeats ? tableFacts.blindSeats.dealer : null);
  const pastedVerdict = $derived(verdictForPasted(pasted, hand.proof.seedHash));

  // ---- equity at the current stop, cached per (board, live seats) -----------
  const equityCache = new Map();
  const equity = $derived.by(() => {
    if (!stop || !equityAllowedAt(stop, hand.seats, showdownSeats)) return null;
    const live = liveSeatsAt(stop, hand.seats);
    const key = `${stop.board}|${live.join(',')}`;
    if (equityCache.has(key)) return equityCache.get(key);
    const revealed = hand.showdown.filter((p) => p.cards && live.includes(p.seat))
      .map((p) => ({ seat: p.seat, cards: [p.cards[0], p.cards[1]] }));
    const hero = revealed.find((r) => r.seat === heroSeat) || null;
    let result = null;
    try {
      result = computeEquity({ revealed, liveCount: live.length, hero, board: hand.community.slice(0, stop.board) });
    } catch (e) {
      logger.debug('replay equity unavailable', e);
    }
    equityCache.set(key, result);
    return result;
  });

  // ---- transport --------------------------------------------------------------
  function seek(index) {
    playing = false;
    stopIndex = Math.max(0, Math.min(stops.length - 1, index));
  }

  function step(delta) { seek(stopIndex + delta); }

  function seekLine(lineIndex) {
    const at = stops.findIndex((s) => s.line === lineIndex);
    if (at >= 0) seek(at);
  }

  function togglePlay() {
    if (!playing && stopIndex >= stops.length - 1) stopIndex = 0;
    playing = !playing;
  }

  $effect(() => {
    if (!playing) return;
    if (stopIndex >= stops.length - 1) { playing = false; return; }
    const id = setTimeout(() => { stopIndex += 1; }, stopBeatMs(stops[stopIndex]));
    return () => clearTimeout(id);
  });

  function onKeydown(e) {
    const tag = e.target?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'BUTTON' || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1); }
    else if (e.key === 'ArrowRight') { e.preventDefault(); step(1); }
    else if (e.key === ' ') { e.preventDefault(); togglePlay(); }
    else if (e.key === 'Home') { e.preventDefault(); seek(0); }
    else if (e.key === 'End') { e.preventDefault(); seek(stops.length - 1); }
  }

  // ---- labels ---------------------------------------------------------------
  const seatName = (seat) => seatNameFor(hand, seat, tableFacts);
  const principalOf = (seat) => principalForSeat(hand, seat);
  const seatLabel = (seat) => (seat === null || seat === undefined ? 'Seat not recorded' : `Seat ${seat + 1}`);

  /** "Seat 2 (Nakamoto) · you", every part of it justified. */
  function actorLabel(seat) {
    if (seat === null || seat === undefined) return 'Seat not recorded';
    const name = seatName(seat);
    const mine = !!myPrincipal && principalOf(seat) === myPrincipal;
    return `${seatLabel(seat)}${name ? ` (${name})` : ''}${mine ? ' · you' : ''}`;
  }

  function formatTimestamp(ns) {
    if (!ns) return 'N/A';
    return new Date(Number(ns) / 1_000_000).toLocaleString();
  }

  function formatClock(ns) {
    if (!ns) return '--:--';
    return new Date(Number(ns) / 1_000_000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  /** The log's own clock: 24-hour, so a row is one line. */
  function formatLogClock(ns) {
    if (!ns) return '--:--';
    return new Date(Number(ns) / 1_000_000).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false });
  }

  function formatLocalClock(ms) {
    if (!ms) return '--:--';
    return new Date(Number(ms)).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  // ---- copies and downloads -------------------------------------------------
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

  function copyAsText() {
    const text = handToText({
      hand, blinds, logGroups, money: (v) => figure(v), currency, tableName,
      nameOf: (seat) => actorLabel(seat),
      playedAt: hand.timestamp ? new Date(Number(hand.timestamp) / 1_000_000) : null,
    });
    copyToClipboard(text, 'text');
  }

  /** The whole audit trail for one hand, as a file the player keeps. */
  function downloadHand() {
    const v = hand.verification;
    const payload = {
      note: 'ClearDeck hand record. The deck below was re-derived in the player\'s '
        + 'browser from revealed_seed using the algorithm in docs/SHUFFLE-SPEC.md. '
        + 'Verifying a shuffle proves the deal was not tampered with after the '
        + 'commitment; it proves nothing else about the engine.',
      hand_number: hand.handNumber,
      recorded_at: formatTimestamp(hand.timestamp),
      source: hand.source,
      currency,
      shuffle_proof: { seed_hash: hand.proof.seedHash, revealed_seed: hand.proof.revealedSeed },
      seats_seen: hand.seats,
      community_cards: hand.community.map(codeOf),
      blinds: blinds.level
        ? { ...blinds.level, level_source: blinds.levelSource, seats: blinds.seats, seat_source: blinds.seatSource }
        : 'not known for this hand',
      actions: hand.actions.map((a) => ({
        seat: a.seat, action: a.kind, amount: a.amount, amount_means: a.amountMeaning,
        street: a.street ?? 'not recorded', at: formatClock(a.timestamp),
      })),
      pot_audit: audit,
      showdown: hand.showdown.map((p) => ({
        seat: p.seat, cards: p.cards ? [p.cards[0], p.cards[1]].map(codeOf) : null,
        hand_rank: rankName(p.rank), won_e8s: p.won,
      })),
      winners: hand.winners.map((w) => ({ seat: w.seat, amount_e8s: w.amount })),
      local_verification: v ? {
        commitment_recomputed_in_browser: v.commitment.computed,
        // NOT `commitment_published_before_deal`: the record does not establish
        // that, and a field name in a file the player keeps is a claim like any other.
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
          matches_the_recorded_commitment: sighting.seedHash === String(hand.proof.seedHash || '').toLowerCase(),
        }
        : 'this browser has no sighting of this hand\'s commitment from while it was running',
    };
    // Candid nat64s are BigInt; a bare JSON.stringify throws on those (T-10).
    const blob = new Blob([safeStringify(payload, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `cleardeck-hand-${hand.handNumber}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="replayer">
  <div class="replay-top">
    <button class="ghost-btn back-to-list" onclick={onBack}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15,18 9,12 15,6"/></svg>
      All hands
    </button>
    <div class="replay-title">
      <strong class="hand-number">Hand #{hand.handNumber}</strong>
      <span class="timestamp">{formatTimestamp(hand.timestamp)}</span>
    </div>
    <div class="replay-actions">
      <button class="ghost-btn" onclick={copyAsText} title="Copy this hand as PokerStars-format text, with the seed hash and the revealed seed as trailing comment lines">
        {copied === 'text' ? 'Copied' : 'Copy as text'}
      </button>
      <button class="ghost-btn" onclick={downloadHand}>Download</button>
    </div>
  </div>

  <div class="replay-cols">
    <div class="replay-col col-a">
      <ReplayTable
        {hand} {stop} {spots} {myPrincipal} {seatName} {principalOf} {money} {figure}
        bigBlind={blinds?.level?.big || 0} {boardChecks} {seatChecks} {equity} {dealerSeat}
      />
      <ReplayTransport
        {streets} {currentStreet} index={stop?.index ?? 0} count={stops.length} {playing}
        onSeek={seek} onStep={step} onToggle={togglePlay}
      />
      <ReplayVerdict
        {hand} {sighting} {sightingVerdict} bind:pasted {pastedVerdict} {copied}
        onCopy={copyToClipboard} {formatClock} {formatLocalClock}
      />
    </div>
    <div class="replay-col col-b">
      <ReplayLog
        {logGroups} {audit} {blinds} {streetsRecorded} activeLine={stop?.line ?? null}
        {actorLabel} {money} clock={formatLogClock} onSeekLine={seekLine}
      />
    </div>
  </div>
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.ghost;

  .replayer { padding: var(--cd-space-3) var(--cd-space-4) var(--cd-space-5); overflow-y: auto; flex: 1; min-height: 0; }

  .replay-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--cd-space-2);
    margin-bottom: var(--cd-space-2);
  }

  .replay-title { display: flex; flex-direction: column; align-items: center; gap: 1px; min-width: 0; }
  .replay-title strong { color: var(--cd-ink); font-size: var(--cd-text-figure); }
  .replay-title .timestamp { color: var(--cd-ink-2); font-size: var(--cd-text-xs); white-space: nowrap; }
  .replay-actions { display: flex; gap: var(--cd-space-1); }

  /* A phone gets one column, in reading order. A wide screen gets two, so the
     table, the transport, the verdict and the log are all in one frame. */
  .replay-cols { display: flex; flex-direction: column; }

  @media (min-width: 900px) {
    .replay-cols {
      display: grid;
      grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
      gap: 0 var(--cd-space-4);
      align-items: start;
    }
  }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .replayer { padding: var(--cd-space-2) var(--cd-space-3) var(--cd-space-4); }
    .replay-top { flex-wrap: wrap; }
    .replay-title { order: -1; flex-basis: 100%; align-items: flex-start; }
  }
</style>
