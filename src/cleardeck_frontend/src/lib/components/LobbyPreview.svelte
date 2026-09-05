<script>
  // THE PREVIEW PANE: one table in detail, beside the list.
  //
  // The master/detail shape two of the three reference lobbies use
  // (docs/DESIGN-BAR.md): a stadium seat map, the blinds / buy-in / clock /
  // ante grid, the 0% rake callout, the deck commitment, the table contract's
  // id with COPY, and one big call to action with a "watch for free" note.
  //
  // Every figure is the TABLE contract's (lobby-rows.js effectiveConfig) and
  // the screenshot harness asserts each one: the heading's quoted price, the
  // mini-felt's pot, every seated stack, every fact, the rake line's pot.
  import IcpLogo from './IcpLogo.svelte';
  import {
    fiatPair, formatAmount, formatBlinds, formatBuyIn, shortHash, shortId, stakeTier, tableFormat,
    unitOf, RANGE_WORD,
  } from '../lobby-format.js';
  import {
    PHASE_LABEL, boardOf, canisterIdOf, configDrift, currencyOf, effectiveConfig, isDealing,
    lastPotOf, nameQuote, opt, seatsOf, tagOf,
  } from '../lobby-rows.js';

  const {
    /** The selected lobby record. */
    table,
    /** Its live view, or null. */
    view = null,
    prices = null,
    signedIn = false,
    /** The key of whatever was last copied, for the "copied" state. */
    copiedId = null,
    onOpen = () => {},
    onCopy = () => {},
    onHow = () => {},
  } = $props();

  const currency = $derived(currencyOf(table));
  const cfg = $derived(effectiveConfig(table, view));
  const drift = $derived(configDrift(table, view));
  const seats = $derived(seatsOf(table, view));
  const occupied = $derived(seats.filter(Boolean));
  const board = $derived(boardOf(view));
  const phase = $derived(view ? tagOf(view.phase) : null);
  const dealing = $derived(isDealing(phase));
  const lastPot = $derived(lastPotOf(view));
  const commitment = $derived(opt(view?.shuffle_proof)?.seed_hash ?? null);
  const contractId = $derived(canisterIdOf(table));
  const nameq = $derived(nameQuote(table, view));
  const stakesFiat = $derived(fiatPair(cfg.small_blind, cfg.big_blind, currency, prices, '/'));
  const buyInFiat = $derived(fiatPair(cfg.min_buy_in, cfg.max_buy_in, currency, prices, RANGE_WORD));

  /**
   * Seat coordinates on a ~2:1 stadium, clockwise from the bottom seat
   * (docs/DESIGN-BAR.md 1.2: every leading client's playing surface is a wide
   * ellipse, and an empty seat is a pod with a call to action, never a gap).
   */
  const positions = $derived(Array.from({ length: seats.length }, (_, i) => {
    const angle = Math.PI / 2 - (i * 2 * Math.PI) / seats.length;
    return { left: 50 + 50 * Math.cos(angle), top: 50 + 50 * Math.sin(angle) };
  }));

  const initialOf = (player, index) =>
    (player?.name ? player.name.trim()[0] : String(index + 1)).toUpperCase();

  const stop = (e, fn) => { e.stopPropagation(); fn(); };
</script>

<aside class="preview" aria-label="Table preview">
  <div class="preview-head">
    <div class="preview-heading">
      <h3>
        {#if nameq.stale}
          {nameq.before}<s class="stale-quote">{nameq.quoted}</s>{nameq.after}
          <span class="stale-flag">stale name</span>
        {:else}
          {table.name}
        {/if}
      </h3>
      <p class="preview-sub">
        {tableFormat(cfg.max_players)} · No Limit Hold'em · {stakeTier(cfg.small_blind, currency)} stakes
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
          onclick={(e) => stop(e, () => onOpen(table))}
          title={seat
            ? `Seat ${i + 1}: ${seat.name ?? 'seated player'}${seat.chips != null ? `, ${formatAmount(seat.chips, currency)} ${unitOf(currency)}` : ''}${seat.active ? '' : ' (sitting out)'}`
            : `Seat ${i + 1}: open, opens this table`}
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
    <p class="map-note">Every seat is open. Be the first: pick one to open this table.</p>
  {/if}

  <dl class="facts">
    <div>
      <dt>Blinds</dt>
      <dd>{formatBlinds(cfg.small_blind, cfg.big_blind, currency)}</dd>
      {#if stakesFiat}
        <span class="fiat">≈ <span class="fiat-num" data-fiat-of="sb">{stakesFiat.low}</span> / <span class="fiat-num" data-fiat-of="bb">{stakesFiat.high}</span></span>
      {/if}
    </div>
    <div>
      <dt>Buy-in</dt>
      <dd>{formatBuyIn(cfg.min_buy_in, cfg.max_buy_in, currency)}</dd>
      {#if buyInFiat}
        <span class="fiat">≈ <span class="fiat-num" data-fiat-of="min">{buyInFiat.low}</span> {buyInFiat.joiner} <span class="fiat-num" data-fiat-of="max">{buyInFiat.high}</span></span>
      {/if}
    </div>
    <div><dt>Clock</dt><dd>{cfg.action_timeout_secs}s + {cfg.time_bank_secs}s</dd></div>
    <div><dt>Ante</dt><dd>{Number(cfg.ante) === 0 ? 'None' : formatAmount(cfg.ante, currency)}</dd></div>
    <div><dt>Hands dealt</dt><dd>{view ? Number(view.hand_number) : '·'}</dd></div>
    <div><dt>Last pot</dt><dd>{lastPot === null ? 'None yet' : formatAmount(lastPot, currency)}</dd></div>
  </dl>

  <div class="proofs" class:two-up={Boolean(commitment) && Boolean(contractId)}>
    <div class="proof">
      <span class="proof-label">Deck commitment</span>
      {#if commitment}
        <button class="mono copy" onclick={(e) => stop(e, () => onCopy(commitment, commitment))}>
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
        <button class="mono copy" onclick={(e) => stop(e, () => onCopy(contractId, contractId))}>
          {shortId(contractId)}
          <span class="copy-state">{copiedId === contractId ? 'copied' : 'copy'}</span>
        </button>
      </div>
    {/if}
  </div>

  <div class="preview-cta">
    <button class="btn primary wide" onclick={() => onOpen(table)}>
      {signedIn ? 'Sit down here' : occupied.length === 0 ? 'Open this table' : 'Watch this table'}
    </button>
    <p class="cta-note">
      {signedIn
        ? 'Opening a table costs nothing. You pick a seat and a buy-in there.'
        : 'Watching is free and needs no sign-in. Sign in when you want a seat.'}
    </p>
  </div>

  <!-- Under the CTA, not above it: the button is what has to be on screen at
       first paint at 1440x900; this is the receipt for the last pot. -->
  <p class="rake-line">
    <strong>0% rake.</strong> {lastPot === null
      ? 'Whatever this table collects is paid straight back out.'
      : `All ${formatAmount(lastPot, currency)} ${unitOf(currency)} of the last pot went to the winner.`}
    <button class="link-btn" onclick={(e) => stop(e, onHow)}>How it works</button>
  </p>
</aside>

<style>
  .preview {
    align-self: start;
    position: sticky;
    top: calc(var(--notice-safe-top, 0px) + var(--cd-space-4));
    padding: 15px 16px 16px;
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-panel);
    background: var(--cd-surface-1);
  }

  .preview-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--cd-space-3);
  }

  .preview-heading { min-width: 0; }

  .preview-head h3 {
    margin: 0 0 4px;
    font-size: var(--cd-text-figure);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
  }

  .preview-sub {
    margin: 0;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
  }

  .preview-drift {
    margin: 5px 0 0;
    font-size: var(--cd-text-xs);
    line-height: 1.4;
    color: var(--cd-ink-2);
  }

  .stale-quote {
    color: var(--cd-money-lo);
    text-decoration: line-through;
    text-decoration-thickness: 1.5px;
    font-weight: var(--cd-weight-medium);
  }

  .stale-flag {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--cd-warn);
    background: var(--cd-warn-dim);
    border-radius: var(--cd-space-1);
    padding: 1px 5px;
    white-space: nowrap;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.03em;
    text-transform: uppercase;
    padding: 2px 7px;
    border-radius: 5px;
    background: var(--cd-accent-dim);
    color: var(--cd-accent-hi);
    white-space: nowrap;
  }

  .tag.btc { background: var(--cd-btc-dim); color: var(--cd-btc); }
  .btc-mark { font-size: var(--cd-text-sm); line-height: 1; }

  /* ------------------------------------------------------------- the map */
  .map { padding: var(--cd-space-2) var(--cd-space-4) var(--cd-space-2); }

  .mini-felt {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 1;
    border-radius: 50%;
    background: radial-gradient(120% 150% at 50% 0%, var(--cd-felt-hi), var(--cd-felt-lo) 70%);
    border: 1px solid var(--cd-accent-line);
    box-shadow: inset 0 0 34px var(--cd-felt-shade);
  }

  .mini-felt.btc {
    background: radial-gradient(120% 150% at 50% 0%, var(--cd-felt-btc-hi), var(--cd-felt-btc-lo) 70%);
    border-color: var(--cd-btc-line);
  }

  .felt-mark {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--cd-text-xs);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--cd-felt-mark);
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
    font-size: var(--cd-text-figure);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-money);
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }

  .felt-phase {
    font-size: var(--cd-text-xs);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--cd-ink-felt);
  }

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
    background: var(--cd-card-face);
    color: var(--cd-card-black);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    line-height: 1;
    box-shadow: var(--cd-shadow-card);
  }

  .mini-card.red { color: var(--cd-card-red); }
  .mc-suit { font-size: var(--cd-text-xs); }

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
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    background: var(--cd-plate);
    border: 1px dashed var(--cd-line-bright);
    color: var(--cd-ink-2);
    cursor: pointer;
    transition: border-color var(--cd-fast), color var(--cd-fast), background var(--cd-fast);
  }

  .pod:hover { border-color: var(--cd-accent); color: var(--cd-accent-hi); }
  .pod:focus-visible { outline: 2px solid var(--cd-accent); outline-offset: 2px; }

  .pod.taken {
    border: 1px solid var(--cd-accent);
    background: var(--cd-plate-lo);
    color: var(--cd-accent-hi);
  }

  .pod.taken.idle { border-style: dotted; opacity: 0.6; }

  .mini-felt.btc .pod.taken { border-color: var(--cd-btc); color: var(--cd-btc); }

  .seated {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 6px;
    margin: 0 0 11px;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
  }

  .seated-sep { color: var(--cd-line-strong); }
  .seated-one { display: inline-flex; gap: 5px; align-items: baseline; }
  .seated-one.idle { opacity: 0.55; }
  .seated-name { color: var(--cd-ink-2); }
  .seated-stack { color: var(--cd-ink); font-variant-numeric: tabular-nums; }

  .map-note {
    margin: 0 0 11px;
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
    text-align: center;
  }

  /* ------------------------------------------------------------- the facts */
  .facts {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 1px;
    margin: 0 0 9px;
    background: var(--cd-line-soft);
    border: 1px solid var(--cd-line-soft);
    border-radius: 10px;
    overflow: hidden;
  }

  .facts > div {
    background: var(--cd-plate-lo);
    padding: 7px 9px;
    min-width: 0;
  }

  .facts dt {
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
    margin-bottom: 3px;
    white-space: nowrap;
  }

  .facts dd {
    margin: 0;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .fiat {
    display: block;
    margin-top: 2px;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .rake-line {
    margin: var(--cd-space-3) 0 0;
    padding: 8px 11px;
    border-radius: 10px;
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
    font-size: var(--cd-text-xs);
    line-height: 1.45;
    color: var(--cd-ink-1);
  }

  .rake-line strong { color: var(--cd-accent-hi); }

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

  /* ------------------------------------------------------------- proofs */
  .proofs { display: grid; gap: 7px; margin-bottom: 11px; }
  .proofs.two-up { grid-template-columns: 1fr 1fr; }
  .two-up button.mono { flex-direction: column; align-items: flex-start; gap: 2px; }
  .two-up .copy-state { font-size: var(--cd-text-xs); }

  .proof {
    padding: 8px 10px;
    border-radius: 10px;
    background: var(--cd-plate-lo);
    border: 1px solid var(--cd-line-soft);
  }

  .proof-label {
    display: block;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
    margin-bottom: 5px;
  }

  .proof-idle {
    margin: 0;
    font-size: var(--cd-text-xs);
    line-height: 1.45;
    color: var(--cd-ink-2);
  }

  .mono {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    word-break: break-all;
  }

  button.mono {
    display: flex;
    width: 100%;
    text-align: left;
    align-items: center;
    justify-content: space-between;
    gap: var(--cd-space-2);
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    border-radius: 7px;
    padding: 6px 9px;
    cursor: pointer;
  }

  button.mono:hover { border-color: var(--cd-accent-line); color: var(--cd-ink); }

  .copy-state {
    flex: none;
    font-family: inherit;
    font-size: var(--cd-text-xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--cd-accent);
  }

  /* ------------------------------------------------------------- the CTA */
  .preview-cta { display: grid; gap: 7px; }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
    font: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
    box-shadow: var(--cd-gloss);
  }

  .btn:hover { background: var(--cd-accent-hi); }

  .cta-note {
    margin: 0;
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
    text-align: center;
  }

  @media (max-width: 1080px) {
    .preview { position: static; }
  }
</style>
