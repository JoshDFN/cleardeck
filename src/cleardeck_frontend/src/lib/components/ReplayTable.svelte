<script>
  /**
   * The replayer's table: a felt with the board and the pot, seat pods in the
   * live table's lower-third language, bet chips in front of the seats on the
   * current street, the dealer puck, and at the showdown every revealed hand
   * with its equity. One snapshot in, one scene out; the parent owns the stop.
   *
   * HARNESS CONTRACT (tools/shots/scenarios/handreplay.mjs):
   *   `.felt .board .card` + `.felt .deck-pos.good`   the board at each stop
   *   `.pot-panel .pot-total strong`                  the pot (paid, at the end)
   *   `.pot-panel .winner-line[data-award=pod|line] .seat-label|.won-amt`
   *       the winners; the amount rides the line ONLY when no pod paints it
   *   `.board-slot[data-deck-check=good|bad][data-deck-pos]`  the board's
   *       re-derivation, readable with the captions off; `.deck-pos` captions
   *       render only while `showPositions` is on
   *   `.player-row .seat-tag|.player-cards .card|.deck-pos|.player-rank|.player-result .won-amt`
   *   `.replay-money` on every amount; `.replay-equity[data-seat]` on an equity
   * Nothing money-shaped is painted that the scene does not assert: an
   * intermediate stop's pot is shown only when the fold is exact.
   */
  import Card from './Card.svelte';
  import { avatarFor, chipBand, chipStackCount } from '$lib/table-visuals.js';
  import { formatEquity } from '$lib/equity.js';
  import { rankName } from '$lib/hand-history-records.js';

  const {
    hand,
    stop,
    spots = [],
    myPrincipal = null,
    seatName = () => null,
    principalOf = () => null,
    money = (v) => String(v),
    figure = (v) => String(v),
    bigBlind = 0,
    boardChecks = [],
    seatChecks = [],
    equity = null,
    dealerSeat = null,
    /** Paint the "#N ✓" deck-position captions (off by default; the title keeps them). */
    showPositions = false,
  } = $props();

  const board = $derived((hand?.community || []).slice(0, stop?.board ?? 0).map((card, at) => ({
    card, check: boardChecks[at] || null,
  })));

  const showdownBySeat = $derived(new Map((hand?.showdown || []).map((p) => [p.seat, p])));
  const winnersBySeat = $derived(new Map((hand?.winners || []).map((w) => [w.seat, w])));

  const label = (seat) => `Seat ${seat + 1}`;
  const who = (seat) => {
    const principal = principalOf(seat);
    if (myPrincipal && principal === myPrincipal) return 'You';
    return seatName(seat);
  };

  /** Where a seat's chips sit: off the plate toward the centre. */
  const chipSpot = (s) => ({ x: s.x + s.nx * 0.2, y: s.y + s.ny * 0.27 });
  const puckSpot = (s) => ({ x: s.x + s.nx * 0.12 - s.ny * 0.11, y: s.y + s.ny * 0.14 + s.nx * 0.1 });

  const pct = (v) => `${(v * 100).toFixed(2)}%`;
</script>

<div class="scene" data-stop={stop?.kind}>
  <div class="rail" aria-hidden="true"></div>
  <div class="felt">
    <div class="pot-panel" class:paid={stop?.paid}>
      <div class="pot-total">
        <span class="pot-word">Pot</span>
        {#if stop?.pot !== null && stop?.pot !== undefined}
          <strong class="replay-money cd-money">{money(stop.pot)}</strong>
        {:else}
          <strong class="pot-unknown" title="The pot at this stop cannot be derived exactly from the record">·</strong>
        {/if}
      </div>
      {#if stop?.paid}
        {#each hand.winners as winner}
          <!-- ONE FIGURE IN ONE PLACE. A winner whose pod carries the award chip
               is named here without the amount; the amount rides the line only
               for a seat the pod cannot paint (a hand won without a showdown). -->
          {@const onPod = (showdownBySeat.get(winner.seat)?.won || 0) > 0}
          <div class="winner-line" data-award={onPod ? 'pod' : 'line'}>
            <span class="seat-label">{label(winner.seat)}{#if who(winner.seat)}{' · '}{who(winner.seat)}{/if}</span>
            {#if onPod}
              <span class="won-word">wins</span>
            {:else}
              <span class="won-amt replay-money">+{money(winner.amount)}</span>
            {/if}
            {#if winner.potType}<span class="pot-type">{winner.potType}</span>{/if}
          </div>
        {/each}
      {/if}
    </div>

    <div class="board" class:bare={board.length === 0} class:captioned={showPositions}>
      {#each board as slot, at (at)}
        <!-- The deck position is a hover title at rest and a caption only when
             "Deck positions" is on; the verdict itself (good / bad) rides the
             slot as data so the harness can read it in either state. -->
        <div
          class="board-slot"
          data-deck-check={slot.check ? (slot.check.match ? 'good' : 'bad') : null}
          data-deck-pos={slot.check ? slot.check.positions[0] : null}
          title={slot.check ? `deck position #${slot.check.positions[0]}, re-derived in this browser: ${slot.check.match ? 'matches' : 'does NOT match'}` : null}
        >
          <Card card={slot.card} index={at} />
          {#if showPositions}
            {#if slot.check}
              <span class="deck-pos mono" class:good={slot.check.match} class:bad={!slot.check.match}>
                #{slot.check.positions[0]} {slot.check.match ? '✓' : '✗'}
              </span>
            {:else}
              <span class="deck-pos mono">·</span>
            {/if}
          {/if}
        </div>
      {/each}
      {#if board.length === 0}
        <span class="felt-empty">{stop?.kind === 'deal' || stop?.kind === 'blind' ? 'Cards dealt' : 'No community cards'}</span>
      {/if}
    </div>

    {#if equity}
      <!-- The same caption the live table prints under its pods, so a reader
           knows what the percentages on the pods are. -->
      <span class="replay-equity-method" title={equity.note}>
        Equity · {equity.method === 'exact' ? 'exact' : 'Monte Carlo'} · {equity.trials.toLocaleString('en-US')} {equity.method === 'exact' ? (equity.trials === 1 ? 'runout' : 'runouts') : 'trials'}
      </span>
    {/if}
  </div>

  {#each spots as s (s.seat)}
    {@const shown = showdownBySeat.get(s.seat)}
    {@const won = winnersBySeat.get(s.seat)}
    {@const check = seatChecks.find((c) => c.seat === s.seat)}
    {@const folded = (stop?.folded || []).includes(s.seat)}
    {@const allIn = (stop?.allIn || []).includes(s.seat)}
    {@const acting = stop?.seat === s.seat}
    {@const reveal = !!stop?.reveal && !!shown?.cards}
    {@const isMe = !!myPrincipal && principalOf(s.seat) === myPrincipal}
    {@const name = who(s.seat)}
    {@const bet = stop?.bets?.[s.seat] || 0}
    {@const share = equity?.bySeat?.get(s.seat)}
    <div
      class="player-row side-{s.side}"
      class:me={isMe}
      class:won={stop?.paid && (shown?.won > 0 || won?.amount > 0)}
      class:folded
      class:acting
      class:all-in={allIn}
      style:left="{s.x * 100}%"
      style:top="{s.y * 100}%"
    >
      <div class="player-cards" class:shown={reveal}>
        {#if reveal}
          <Card card={shown.cards[0]} />
          <Card card={shown.cards[1]} />
        {:else if !folded}
          <Card faceDown={true} />
          <Card faceDown={true} />
        {/if}
      </div>
      <div class="plate">
        <img class="pod-avatar" src={avatarFor(principalOf(s.seat) || `seat-${s.seat}`, name || label(s.seat)).src} alt="" />
        <div class="plate-rows">
          <span class="seat-tag seat-label">{label(s.seat)}{#if name}<span class="pod-who">{' · '}{name}</span>{/if}</span>
          <span class="plate-sub">
            {#if reveal && shown?.rank}
              <span class="player-rank">{rankName(shown.rank) || ''}</span>
              {#if check}
                <span class="deck-pos mono" class:good={check.match} class:bad={!check.match} title="deck positions #{check.positions[0]} and #{check.positions[1]}, re-derived in this browser">{#if showPositions}#{check.positions[0]} #{check.positions[1]} {/if}{check.match ? '✓' : '✗'}</span>
              {/if}
            {:else if folded}
              <span class="pod-state fold">Folded</span>
            {:else if allIn}
              <span class="pod-state allin">All in</span>
            {:else if acting}
              <span class="pod-state acting">{stop?.verb || 'acts'}</span>
            {:else}
              <span class="pod-state">In hand</span>
            {/if}
            {#if share !== undefined && share !== null && !folded}
              <span class="replay-equity" data-seat={s.seat} data-method={equity.method}>{formatEquity(share, equity.method)}</span>
            {/if}
          </span>
        </div>
      </div>
      {#if stop?.paid && shown?.won > 0}
        <span class="player-result"><span class="won-amt replay-money">+{money(shown.won)}</span></span>
      {/if}
    </div>

    {#if bet > 0}
      {@const c = chipSpot(s)}
      <div class="replay-bet band-{chipBand(bet, bigBlind)}" class:all-in={allIn} style:left="{c.x * 100}%" style:top="{c.y * 100}%">
        <span class="chip-stack" aria-hidden="true" style:--discs={chipStackCount(bet, bigBlind)}>
          {#each Array(chipStackCount(bet, bigBlind)) as _, k}
            <i class="chip" style:--k={k}></i>
          {/each}
        </span>
        <span class="bet-figure replay-money">{figure(bet)}</span>
      </div>
    {/if}

    {#if dealerSeat === s.seat}
      {@const p = puckSpot(s)}
      <span class="replay-puck" style:left="{p.x * 100}%" style:top="{p.y * 100}%" title="Dealer button">D</span>
    {/if}
  {/each}
</div>

<style>
  .scene {
    --board-card: 7.4cqw;
    --plate-w: max(27cqw, 128px);
    --avatar: max(5.2cqw, 26px);
    --pod-font: max(2.3cqw, 11px);
    position: relative;
    width: 100%;
    aspect-ratio: 1.6;
    container-type: inline-size;
    margin: var(--cd-space-2) 0 var(--cd-space-3);
    /* the pods overhang the felt's ends and the room's edge is the column */
    overflow: visible;
  }

  /* the padded rail, then the felt inside it */
  .rail {
    position: absolute;
    left: 10.5%;
    right: 10.5%;
    top: 18%;
    bottom: 18%;
    border-radius: 50% / 50%;
    background: linear-gradient(180deg, var(--cd-rail-hi), var(--cd-rail) 40%, var(--cd-rail-lo) 85%, var(--cd-rail-face));
    box-shadow: var(--cd-shadow-rail), inset 0 1px 0 var(--cd-rail-crown);
  }

  .felt {
    position: absolute;
    left: 13%;
    right: 13%;
    top: 21.8%;
    bottom: 21.8%;
    border-radius: 50% / 50%;
    background:
      var(--cd-noise),
      radial-gradient(ellipse at 50% 38%, var(--cd-felt-hi) 0%, var(--cd-felt) 48%, var(--cd-felt-lo) 82%, var(--cd-felt-edge) 100%);
    background-blend-mode: overlay, normal;
    box-shadow: inset 0 0 0 2px var(--cd-felt-line), inset 0 0 40px var(--cd-felt-shade), 0 0 0 1px var(--cd-rail-seam);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: calc(var(--board-card) * 0.16);
  }

  /* ---- the pot module ------------------------------------------------- */
  .pot-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 0.2em 0.9em;
    border-radius: var(--cd-radius-chip);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate) 55%, var(--cd-plate-lo));
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
    font-size: var(--pod-font);
  }

  .pot-total { display: flex; align-items: baseline; gap: 0.5em; }
  .pot-word { font-size: 0.8em; text-transform: uppercase; letter-spacing: var(--cd-tracking-label); color: var(--cd-ink-2); }
  .pot-total strong { font-size: 1.3em; color: var(--cd-money); font-weight: var(--cd-weight-display); }
  .pot-total .pot-unknown { color: var(--cd-ink-2); }
  .winner-line { display: flex; gap: 0.5em; align-items: baseline; font-size: 0.85em; color: var(--cd-ink-1); }
  .winner-line .won-amt { color: var(--cd-accent); font-weight: var(--cd-weight-figure); }
  .winner-line .won-word { color: var(--cd-money); font-weight: var(--cd-weight-strong); }
  .pot-type { color: var(--cd-ink-2); font-size: 0.85em; }

  /* ---- the board ------------------------------------------------------ */
  .board {
    --card-w: var(--board-card);
    display: flex;
    gap: calc(var(--board-card) * 0.12);
    min-height: calc(var(--board-card) / 0.81);
    align-items: flex-start;
  }

  .board.captioned { min-height: calc(var(--board-card) / 0.81 + 1.4em); }

  .board.bare { align-items: center; justify-content: center; }
  .board-slot { display: flex; flex-direction: column; align-items: center; gap: 2px; }
  .felt-empty { color: var(--cd-ink-felt); font-size: var(--pod-font); letter-spacing: 0.02em; }

  .deck-pos {
    font-family: var(--cd-font-mono);
    font-size: max(1.9cqw, 10px);
    color: var(--cd-ink-felt);
    white-space: nowrap;
    line-height: 1.2;
  }

  .deck-pos.good { color: var(--cd-accent-hi); }
  .deck-pos.bad { color: var(--cd-danger-hi); }

  .replay-equity-method {
    font-size: max(1.9cqw, 10px);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-felt);
  }

  /* ---- seat pods: the lower third ------------------------------------- */
  .player-row {
    position: absolute;
    z-index: 4;
    width: var(--plate-w);
    transform: translate(-50%, -50%);
    font-size: var(--pod-font);
    transition: opacity var(--cd-base) var(--cd-ease);
  }

  .player-row.folded { opacity: 0.55; }

  .plate {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.4em;
    min-height: calc(var(--avatar) * 1.35);
    padding: 0.25em 0.6em 0.25em calc(var(--avatar) * 0.7);
    border-radius: var(--cd-radius-chip);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate) 55%, var(--cd-plate-lo));
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
    transition: border-color var(--cd-acting) var(--cd-ease), box-shadow var(--cd-acting) var(--cd-ease);
  }

  .player-row.me .plate { border-color: var(--cd-accent-line-strong); }
  .player-row.acting .plate { border-color: var(--cd-line-hot); box-shadow: 0 0 0 3px var(--cd-glow-white), var(--cd-shadow-pod); }
  .player-row.won .plate { border-color: var(--cd-money-line); }

  .pod-avatar {
    position: absolute;
    left: 0;
    top: 50%;
    width: var(--avatar);
    height: var(--avatar);
    border-radius: 50%;
    transform: translate(-50%, -50%);
    box-shadow: 0 0 0 2px var(--cd-plate-lo), var(--cd-shadow-chip);
    background: var(--cd-plate-lo);
  }

  .plate-rows { display: flex; flex-direction: column; min-width: 0; line-height: 1.2; }
  .seat-tag { font-weight: var(--cd-weight-strong); color: var(--cd-ink); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .player-row.me .seat-tag { color: var(--cd-accent-hi); }
  .pod-who { font-weight: var(--cd-weight-body); color: var(--cd-ink-1); }
  .plate-sub { display: flex; align-items: baseline; gap: 0.35em; font-size: 0.86em; color: var(--cd-ink-2); white-space: nowrap; min-width: 0; }
  .plate-sub > :first-child { overflow: hidden; text-overflow: ellipsis; }
  .player-rank { color: var(--cd-money); font-weight: var(--cd-weight-strong); }
  .pod-state.fold { color: var(--cd-danger-hi); }
  .pod-state.allin { color: var(--cd-danger-hi); font-weight: var(--cd-weight-strong); }
  .pod-state.acting { color: var(--cd-ink); }
  .plate .deck-pos { font-size: 1em; flex: 0 0 auto; }

  .player-cards {
    --card-w: calc(var(--board-card) * 0.78);
    position: absolute;
    display: flex;
    gap: calc(var(--card-w) * 0.08);
    z-index: -1;
  }

  .player-cards.shown { z-index: 1; }
  /* The pair stands clear of the plate (no tuck: the occlusion gate counts a
     plate over a card's ink), above a bottom seat, below a top seat, inward
     of a flank seat. */
  .side-bottom .player-cards { left: 50%; bottom: 96%; transform: translateX(-50%); }
  .side-top .player-cards { left: 50%; top: 96%; transform: translateX(-50%); }
  .side-left .player-cards { left: 96%; top: 50%; transform: translateY(-50%); }
  .side-right .player-cards { right: 96%; top: 50%; transform: translateY(-50%); }

  /* the award past the plate's right end, mid-height (the live table's spot) */
  .player-result {
    position: absolute;
    left: 100%;
    top: 50%;
    transform: translate(-18%, -50%);
    padding: 0.1em 0.55em;
    white-space: nowrap;
    z-index: 2;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-money);
    color: var(--cd-money-ink);
    font-weight: var(--cd-weight-figure);
    font-size: 0.9em;
    box-shadow: var(--cd-shadow-chip);
    animation: award-in var(--cd-move) var(--cd-ease-spring) both;
  }

  /* the equity rides the lower third's second row, right-aligned (broadcast) */
  .replay-equity {
    margin-left: auto;
    padding: 0 0.4em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-accent-dim);
    color: var(--cd-accent-hi);
    font-size: 0.95em;
    font-weight: var(--cd-weight-figure);
    font-variant-numeric: tabular-nums;
    flex: 0 0 auto;
  }

  /* ---- chips and the puck --------------------------------------------- */
  .replay-bet {
    position: absolute;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 0.35em;
    transform: translate(-50%, -50%);
    white-space: nowrap;
    font-size: var(--pod-font);
    animation: bet-in var(--cd-move) var(--cd-ease-move) both;
    --chip-face: var(--cd-chip-white);
  }

  .replay-bet.band-red   { --chip-face: var(--cd-chip-red); }
  .replay-bet.band-blue  { --chip-face: var(--cd-chip-blue); }
  .replay-bet.band-green { --chip-face: var(--cd-chip-green); }
  .replay-bet.band-black { --chip-face: var(--cd-chip-black); }
  .replay-bet.band-gold  { --chip-face: var(--cd-chip-gold); }

  .chip-stack {
    --chip: max(2.6cqw, 12px);
    --lift: calc(var(--chip) * 0.2);
    position: relative;
    flex: 0 0 auto;
    width: var(--chip);
    height: calc(var(--chip) + (var(--discs, 1) - 1) * var(--lift));
  }

  .chip-stack::before {
    content: '';
    position: absolute;
    left: 6%;
    bottom: calc(var(--chip) * -0.1);
    width: 100%;
    height: calc(var(--chip) * 0.4);
    border-radius: 50%;
    background: var(--cd-felt-shade);
    filter: blur(calc(var(--chip) * 0.12));
  }

  .chip {
    position: absolute;
    left: 0;
    bottom: calc(var(--k, 0) * var(--lift));
    width: var(--chip);
    height: var(--chip);
    border-radius: 50%;
    background:
      radial-gradient(circle at 50% 44%, var(--chip-face) 0 56%, transparent 57%),
      repeating-conic-gradient(from 0deg, var(--cd-chip-notch) 0deg 13deg, var(--chip-face) 13deg 45deg);
    box-shadow: inset 0 0 0 1px var(--cd-chip-rim), 0 1px 0 var(--cd-chip-rim);
  }

  .bet-figure {
    font-size: 0.86em;
    font-weight: var(--cd-weight-figure);
    padding: 0.1em 0.5em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
    color: var(--cd-money);
    border: 1px solid var(--cd-money-line);
    box-shadow: var(--cd-shadow-chip);
    font-variant-numeric: tabular-nums;
  }

  .replay-bet.all-in .bet-figure { color: var(--cd-ink); border-color: var(--cd-danger-line); }

  .replay-puck {
    position: absolute;
    z-index: 5;
    width: max(3.6cqw, 16px);
    height: max(3.6cqw, 16px);
    transform: translate(-50%, -50%);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(180deg, var(--cd-card-face), var(--cd-card-face-lo));
    color: var(--cd-ink-on-light);
    font-size: max(1.9cqw, 9px);
    font-weight: var(--cd-weight-display);
    box-shadow: var(--cd-shadow-chip);
  }

  @keyframes bet-in {
    from { opacity: 0; transform: translate(-50%, -50%) scale(0.6); }
    to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
  }

  @keyframes award-in {
    from { opacity: 0; transform: scale(0.6); }
    to { opacity: 1; transform: scale(1); }
  }

  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .scene { aspect-ratio: 1.25; --board-card: 9.5cqw; --plate-w: max(36cqw, 128px); }
    .rail { left: 5%; right: 5%; top: 17%; bottom: 17%; }
    .felt { left: 8%; right: 8%; top: 20%; bottom: 20%; }
  }

  @media (prefers-reduced-motion: reduce) {
    .replay-bet, .player-result { animation: none; }
  }
</style>
