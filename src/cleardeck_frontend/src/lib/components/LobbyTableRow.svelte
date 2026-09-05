<script>
  // ONE TABLE, AS A CARD. Rendered as a <tr> inside the lobby's <tbody> because
  // the screenshot harness reads the list row by row (`tbody tr`, `.table-name`,
  // `.stakes-value`, `.buyin-value`, `.players-text`, `.now`, `.now-detail`) and
  // asserts every money figure on it against the table contract. The element
  // is a table row; the styling is a card at every width.
  //
  // Every figure here came from the canisters through `row` (Lobby.svelte
  // builds it): the stakes and buy-in from the TABLE contract's own config, the
  // seats from its live view, the fiat hint from the one quote the page read.
  import IcpLogo from './IcpLogo.svelte';
  import { fiatPair, formatBlinds, formatBuyIn, unitOf } from '../lobby-format.js';

  const {
    /** @type {import('../lobby-rows.js').LobbyRow} */
    row,
    signedIn = false,
    selected = false,
    /** The page's quote, or null: no quote, no fiat line. */
    prices = null,
    /** The invite link for this table, or null. */
    inviteLink = null,
    /** True while "copied" should show on the invite control. */
    copied = false,
    onOpen = () => {},
    onSelect = () => {},
    onCopyInvite = () => {},
  } = $props();

  const cfg = $derived(row.cfg);
  const currency = $derived(row.currency);
  const unit = $derived(unitOf(row.currency));
  const stakesFiat = $derived(fiatPair(cfg.small_blind, cfg.big_blind, currency, prices, '/'));
  const buyInFiat = $derived(fiatPair(cfg.min_buy_in, cfg.max_buy_in, currency, prices, '–'));
  const empty = $derived(row.filled === 0);
  const action = $derived(row.isFull ? 'Watch' : signedIn ? 'Sit' : 'Watch');

  function onKey(e) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onOpen(row.table);
    }
  }

  function copyInvite(e) {
    e.stopPropagation();
    if (inviteLink) onCopyInvite(inviteLink, row.id);
  }
</script>

<tr
  class:selected
  class:btc={currency === 'BTC'}
  class:full={row.isFull}
  class:empty
  tabindex="0"
  aria-label="Open {row.name}"
  onclick={() => onOpen(row.table)}
  onkeydown={onKey}
  onmouseenter={() => onSelect(row.table)}
  onfocus={() => onSelect(row.table)}
>
  <td class="c-table">
    <!-- THE NAME IS NEVER REWRITTEN: it is the lobby canister's registered
         identity (and the key the harness matches a row on). A figure inside
         it that the contract contradicts is struck through, so exactly one
         live price is on the row and it comes from the table contract. -->
    <span class="name-line">
      {#if row.nameq.stale}
        <span
          class="table-name"
          title="The lobby canister registered this table as “{row.name}”. Its own contract charges {row.nameq.chain} {unit}, so the figure in the name is struck through: it is the lobby's stale record, not this table's price."
        >{row.nameq.before}<s class="stale-quote">{row.nameq.quoted}</s>{row.nameq.after}</span>
        <span class="stale-flag">stale name</span>
      {:else}
        <span class="table-name">{row.name}</span>
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
      <span class="tag">{row.format}</span>
      <span class="tag">{row.tier}</span>
    </span>
  </td>

  <td class="c-stakes">
    <span class="cell-label">Blinds</span>
    <span class="num stakes-value">{formatBlinds(cfg.small_blind, cfg.big_blind, currency)}</span>
    <span class="unit">{unit}</span>
    {#if stakesFiat}
      <span class="fiat" title="About, at the quote this page read when it opened">
        ≈ <span class="fiat-num" data-fiat-of="sb">{stakesFiat.low}</span> / <span class="fiat-num" data-fiat-of="bb">{stakesFiat.high}</span>
      </span>
    {/if}
    {#if row.drift.length > 0}
      <span
        class="drift-mark"
        title="This table's own contract enforces the figures shown. The lobby canister's registration disagrees on: {row.drift.join(', ')}."
      >record differs</span>
    {/if}
  </td>

  <td class="c-buyin">
    <span class="cell-label">Buy-in</span>
    <span class="num muted buyin-value">{formatBuyIn(cfg.min_buy_in, cfg.max_buy_in, currency)}</span>
    {#if buyInFiat}
      <span class="fiat" title="About, at the quote this page read when it opened">
        ≈ <span class="fiat-num" data-fiat-of="min">{buyInFiat.low}</span> – <span class="fiat-num" data-fiat-of="max">{buyInFiat.high}</span>
      </span>
    {/if}
  </td>

  <td class="c-seats">
    <span class="cell-label">Seats</span>
    <span class="seats-line">
      <span class="meter" class:btc={currency === 'BTC'} aria-hidden="true">
        {#each row.seats as seat}
          <span class="pip" class:taken={Boolean(seat)} class:idle={Boolean(seat) && !seat.active}></span>
        {/each}
      </span>
      <span class="seat-count players-text"><strong>{row.filled}</strong>/{cfg.max_players}</span>
    </span>
    {#if row.idle > 0}
      <span class="seat-note">{row.idle} sitting out</span>
    {/if}
  </td>

  <td class="c-clock">
    <span class="cell-label">Clock</span>
    <span class="clock-value" title="Seconds to act, plus the time bank a player can add once per hand">{row.clockText}</span>
  </td>

  <td class="c-now">
    <span class="cell-label">Now</span>
    <span class="now {row.now.kind}">
      <span class="dot"></span>
      {row.now.label}
    </span>
    {#if row.now.detail}<span class="now-detail">pot {row.now.detail}</span>{/if}
    {#if row.hands !== null && row.hands > 0}
      <span class="hands-value">{row.hands} {row.hands === 1 ? 'hand' : 'hands'} dealt</span>
    {/if}
  </td>

  <td class="c-go">
    <!-- A full table is not a dead end: its state is public, so the honest
         label is the one PokerStars uses next to Play Now, you can watch it. -->
    <span class="go" class:btc={currency === 'BTC'} class:sit={action === 'Sit'}>
      {action}
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
        <path d="M5 12h14M12 5l7 7-7 7"/>
      </svg>
    </span>
    {#if empty && inviteLink}
      <!-- A pill at the touch floor beside Watch (the phone) or under it
           (desktop): the round-1 text link measured 29 px wide. -->
      <button class="invite" type="button" onclick={copyInvite} title="Copy a link that opens this table. The hand deals the moment your opponent sits.">
        {copied ? 'Link copied' : 'Invite'}
      </button>
    {/if}
  </td>
</tr>

<style>
  /* ---------------------------------------------------------------- the row
     A card at every width. `tbody` is a grid in Lobby.svelte; each row is a
     grid of seven cells sharing one column template, so the figures line up
     down the list as they would in a table. */
  tr {
    display: grid;
    grid-template-columns:
      minmax(0, 2.1fr) minmax(0, 1.25fr) minmax(0, 1.35fr) minmax(0, 1.2fr)
      minmax(0, 0.85fr) minmax(0, 1.35fr) auto;
    align-items: center;
    gap: var(--cd-space-3);
    padding: var(--cd-space-3) var(--cd-space-4);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
    cursor: pointer;
    transition: background var(--cd-fast) var(--cd-ease), border-color var(--cd-fast) var(--cd-ease);
  }

  tr:hover,
  tr.selected {
    background: var(--cd-surface-2);
    border-color: var(--cd-accent-line);
  }

  tr.btc:hover,
  tr.btc.selected { border-color: var(--cd-btc-line); }

  tr:focus-visible { outline: 2px solid var(--cd-accent); outline-offset: -2px; }

  td {
    display: block;
    min-width: 0;
    padding: 0;
  }

  .cell-label {
    display: block;
    margin-bottom: 2px;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
    white-space: nowrap;
  }

  /* ---------------------------------------------------------------- name */
  .name-line {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
  }

  .table-name {
    display: block;
    min-width: 0;
    color: var(--cd-ink);
    font-weight: var(--cd-weight-strong);
    font-size: var(--cd-text-figure);
    letter-spacing: -0.005em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .stale-quote {
    color: var(--cd-money-lo);
    text-decoration: line-through;
    text-decoration-thickness: 1.5px;
    font-weight: var(--cd-weight-medium);
  }

  .stale-flag {
    flex: none;
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

  .tags { display: flex; gap: var(--cd-space-1); margin-top: 6px; flex-wrap: wrap; }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.03em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 5px;
    background: var(--cd-surface-2);
    color: var(--cd-ink-2);
    white-space: nowrap;
  }

  .tag.currency { background: var(--cd-accent-dim); color: var(--cd-accent-hi); }
  .tag.currency.btc { background: var(--cd-btc-dim); color: var(--cd-btc); }
  .btc-mark { font-size: var(--cd-text-sm); line-height: 1; }

  /* ---------------------------------------------------------------- money */
  .num {
    font-variant-numeric: tabular-nums;
    font-weight: var(--cd-weight-figure);
    font-size: var(--cd-text-figure);
    color: var(--cd-money);
  }

  .num.muted { color: var(--cd-ink-1); font-weight: var(--cd-weight-medium); font-size: var(--cd-text-sm); }
  .unit { font-size: var(--cd-text-xs); color: var(--cd-ink-2); margin-left: 4px; }

  /* The dollar line under a figure. Always "about"; never the figure the
     contract charges, which is the one above it. */
  .fiat {
    display: block;
    margin-top: 2px;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* Quiet, in the money column: the contract's figure is shown and the
     lobby record is the one that is wrong. */
  .drift-mark {
    display: block;
    margin-top: 3px;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
    cursor: help;
    white-space: nowrap;
  }

  /* ---------------------------------------------------------------- seats */
  .seats-line { display: inline-flex; align-items: center; gap: var(--cd-space-2); }
  .meter { display: inline-flex; gap: 2.5px; }

  .pip {
    width: 6px;
    height: 6px;
    border-radius: 2px;
    background: var(--cd-surface-4);
  }

  .pip.taken { background: var(--cd-accent); }
  .meter.btc .pip.taken { background: var(--cd-btc); }
  /* Seated but not dealt in: the seat is gone, the player is not. */
  .pip.taken.idle { background: transparent; box-shadow: inset 0 0 0 1.5px var(--cd-accent); }
  .meter.btc .pip.taken.idle { box-shadow: inset 0 0 0 1.5px var(--cd-btc); }

  .seat-count {
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
    font-variant-numeric: tabular-nums;
  }

  .seat-count strong { color: var(--cd-ink); font-size: var(--cd-text-figure); }

  .seat-note {
    display: block;
    margin-top: 3px;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
  }

  .clock-value {
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-1);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* ---------------------------------------------------------------- status */
  .now {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
    white-space: nowrap;
  }

  .now .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .now.live { color: var(--cd-accent); }
  .now.ready { color: var(--cd-money); }
  .now.waiting { color: var(--cd-ink-1); }
  .now.open { color: var(--cd-ink-2); }

  .now-detail,
  .hands-value {
    display: block;
    margin-top: 3px;
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* ---------------------------------------------------------------- action */
  .c-go {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--cd-space-1);
  }

  .go {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-ink);
    white-space: nowrap;
  }

  .go.sit {
    background: var(--cd-accent);
    border-color: transparent;
    color: var(--cd-accent-ink);
  }

  .go.sit.btc { background: var(--cd-btc); color: var(--cd-ink-on-light); }
  tr:hover .go:not(.sit) { border-color: var(--cd-accent-line-strong); }

  /* Never narrower than the touch floor: a 44 px box even when the word is
     shorter (touch-targets.mjs measures the painted box). */
  .invite {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: var(--cd-touch-min);
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--cd-radius-chip);
    font: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-accent);
    cursor: pointer;
    white-space: nowrap;
  }

  .invite:hover { color: var(--cd-accent-hi); border-color: var(--cd-accent-line); }

  /* ------------------------------------------------------------ the phone */
  @media (max-width: 760px) {
    tr {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        'name go'
        'stakes seats'
        'buyin buyin'
        'now now';
      gap: 2px var(--cd-space-3);
      padding: var(--cd-space-3);
    }

    .cell-label { display: none; }
    .c-table { grid-area: name; }
    .c-go { grid-area: go; flex-direction: row; align-items: center; }
    .c-stakes { grid-area: stakes; margin-top: 6px; }
    .c-seats { grid-area: seats; margin-top: 6px; text-align: right; }
    .c-buyin { grid-area: buyin; margin-top: 4px; }
    .c-clock { display: none; }
    .c-now {
      grid-area: now;
      margin-top: 6px;
      padding-top: 6px;
      border-top: 1px solid var(--cd-line-soft);
    }

    .c-buyin .num::before {
      content: 'Buy-in ';
      font-size: var(--cd-text-xs);
      letter-spacing: 0.07em;
      text-transform: uppercase;
      color: var(--cd-ink-2);
      font-weight: var(--cd-weight-strong);
    }

    .fiat { display: inline; margin-left: 6px; }
    .drift-mark, .seat-note { display: inline; margin-top: 0; margin-left: 6px; }
    .now-detail, .hands-value { display: inline; margin-left: 8px; }

    .go { min-height: var(--cd-touch-min); padding: 0 var(--cd-space-4); font-size: var(--cd-text-sm); }
    .invite { min-height: var(--cd-touch-min); padding: 0 var(--cd-space-2); }
  }
</style>
