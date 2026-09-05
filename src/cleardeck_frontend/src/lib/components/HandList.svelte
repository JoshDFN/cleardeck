<script>
  /**
   * The hand list: one glance per hand. Result, your two cards, the board, the
   * pot, who else was in, when, and the verdict seal; filters as chips with
   * counts, so nothing is hidden silently.
   *
   * HARNESS CONTRACT (handhistory.mjs, handreplay.mjs, chain-agreement.mjs
   * assertHandHistoryAgreement): `.hand-row` per hand, newest first, each with
   * `.hand-number` reading "Hand #N" and `.pot` carrying the hand's pot;
   * `.list-head .chip` with the text "Every hand here"; `.live-note` while a
   * hand is in play; `.gap-note` for missing numbers; `.list-foot`.
   */
  import MiniCard from './MiniCard.svelte';
  import HashSeal from './HashSeal.svelte';
  import { FILTERS, RESULT_WORD, applyFilter, filterCounts, heroCards, heroResult, opponentCount, verdictOf } from '$lib/hand-list-rows.js';
  import { participatedIn } from '$lib/hand-history-records.js';
  import { reachedStreet } from '$lib/hand-record.js';

  let {
    allHands = [],
    myPrincipal = null,
    filter = $bindable('mine'),
    inProgress = null,
    liveSighting = null,
    missing = [],
    money = (v) => String(v),
    formatTimestamp = () => '',
    formatLocalClock = () => '',
    onOpen = () => {},
  } = $props();

  const hands = $derived(applyFilter(allHands, filter, myPrincipal));
  const counts = $derived(filterCounts(allHands, myPrincipal));
  const rederivedCount = $derived(hands.filter((h) => h.verification?.ok).length);

  const BOARD_SLOTS = [0, 1, 2, 3, 4];
</script>

<div class="list-head">
  <div class="filter" role="tablist" aria-label="Which hands">
    {#each FILTERS as f}
      <button class="chip" class:on={filter === f.id} role="tab" aria-selected={filter === f.id} onclick={() => { filter = f.id; }}>
        {f.label} ({counts[f.id]})
      </button>
    {/each}
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
        None of the {allHands.length} recorded hand(s) here match this filter. Switch to “Every hand here” to see them.
      {:else}
        No hands recorded on this table yet.
      {/if}
    </p>
  </div>
{:else}
  <div class="hands-list">
    <div class="list-cols" aria-hidden="true">
      <span>Result</span><span>Cards</span><span>Board</span><span class="right">Pot</span><span>Hand</span><span></span>
    </div>
    {#each hands as hand (hand.handNumber)}
      {@const verdict = verdictOf(hand)}
      {@const result = heroResult(hand, myPrincipal)}
      {@const mine = heroCards(hand, myPrincipal)}
      {@const others = opponentCount(hand, myPrincipal)}
      <button class="hand-row" class:dim={!participatedIn(hand, myPrincipal)} onclick={() => onOpen(hand)}>
        <span class="row-result {result}">
          <span class="result-word">{RESULT_WORD[result]}</span>
          <span class="result-street">to the {reachedStreet(hand.community.length)}{#if hand.showdown.length}<span class="sep">{'\u00a0·\u00a0'}</span>showdown{/if}</span>
        </span>
        <span class="row-cards" aria-label="your cards">
          {#if mine}
            <MiniCard card={mine[0]} /><MiniCard card={mine[1]} />
          {:else}
            <MiniCard /><MiniCard />
          {/if}
        </span>
        <span class="row-board" aria-label="board">
          {#each BOARD_SLOTS as at}
            {#if hand.community[at]}
              <MiniCard card={hand.community[at]} />
            {:else}
              <span class="board-blank"></span>
            {/if}
          {/each}
        </span>
        <span class="row-pot">
          <span class="pot cd-money">{money(hand.potTotal)}</span>
          <span class="seats">vs {others}</span>
        </span>
        <span class="row-main">
          <span class="hand-number">Hand #{hand.handNumber}</span>
          <span class="hand-time">{formatTimestamp(hand.timestamp)}</span>
        </span>
        <span class="row-verdict {verdict.tone}" title={verdict.label}>
          <HashSeal hash={hand.proof.seedHash} size={22} tone={verdict.tone === 'good' ? 'accent' : 'muted'} />
          <span class="verdict-mark">{verdict.tone === 'good' ? '✓' : verdict.tone === 'bad' ? '✗' : '◌'}</span>
        </span>
      </button>
    {/each}
  </div>
  <div class="list-foot">
    <!-- COUNTED, NOT ASSERTED: a hand still awaiting its seed is listed here too. -->
    {rederivedCount} of {hands.length} hand(s) above were re-derived in this browser from their
    own revealed seed. Nothing here was verified by a canister. Open one to step through it.
  </div>
{/if}

<style lang="scss">
  @use './fairness' as f;

  .list-head {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    padding: var(--cd-space-3) var(--cd-space-4) var(--cd-space-2);
    flex-shrink: 0;
  }

  .filter { display: flex; gap: var(--cd-space-1); flex-wrap: wrap; }

  .chip {
    min-height: var(--cd-control-sm);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line);
    color: var(--cd-ink-2);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-pill);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease);
  }

  .chip:hover { color: var(--cd-ink); }
  .chip.on { background: var(--cd-accent-dim); border-color: var(--cd-accent-line-strong); color: var(--cd-accent); }

  .gap-note, .live-note { font-size: var(--cd-text-xs); color: var(--cd-ink-2); line-height: 1.5; }

  .state-block {
    display: flex;
    justify-content: center;
    padding: var(--cd-space-6) var(--cd-space-5);
    color: var(--cd-ink-2);
    font-size: var(--cd-text-sm);
    text-align: center;
    line-height: 1.6;
  }

  .state-block p { margin: 0; max-width: 46ch; }

  .hands-list { flex: 1; min-height: 0; overflow-y: auto; padding: 0 var(--cd-space-2) var(--cd-space-2); }

  /* the same grid for the column heads and every row */
  .list-cols, .hand-row {
    display: grid;
    grid-template-columns: 8.5em 3.6em minmax(0, 1fr) 6.5em 8em 3em;
    gap: var(--cd-space-3);
    align-items: center;
  }

  .list-cols {
    padding: 0 var(--cd-space-3) var(--cd-space-1);
    font-size: var(--cd-text-xs);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
  }

  .list-cols .right { text-align: right; }

  .hand-row {
    width: 100%;
    padding: var(--cd-space-2) var(--cd-space-3);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-card);
    margin-bottom: var(--cd-space-1);
    cursor: pointer;
    text-align: left;
    color: inherit;
    font: inherit;
    transition: background var(--cd-base) var(--cd-ease), border-color var(--cd-base) var(--cd-ease);
  }

  .hand-row:hover { background: var(--cd-surface-2); border-color: var(--cd-line); }
  .hand-row.dim { opacity: 0.6; }

  .row-result { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .result-word { font-size: var(--cd-text-figure); font-weight: var(--cd-weight-figure); color: var(--cd-ink); line-height: 1.1; }
  .row-result.won .result-word { color: var(--cd-accent); }
  .row-result.lost .result-word { color: var(--cd-danger-hi); }
  .row-result.out .result-word, .row-result.open .result-word { color: var(--cd-ink-2); }
  .result-street { font-size: var(--cd-text-xs); color: var(--cd-ink-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .row-cards { --mini-card-w: 24px; display: flex; gap: 2px; }
  .row-board { --mini-card-w: 21px; display: flex; gap: 3px; }
  .board-blank { width: var(--mini-card-w); height: calc(var(--mini-card-w) * 1.32); border-radius: 3px; border: 1px dashed var(--cd-line-soft); flex: 0 0 auto; }

  .row-pot { display: flex; flex-direction: column; align-items: flex-end; gap: 1px; }
  .pot { color: var(--cd-money); font-size: var(--cd-text-figure); line-height: 1.1; white-space: nowrap; }
  .seats { font-size: var(--cd-text-xs); color: var(--cd-ink-2); }

  .row-main { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .hand-number { font-weight: var(--cd-weight-strong); color: var(--cd-ink-1); font-size: var(--cd-text-sm); }
  .hand-time { font-size: var(--cd-text-xs); color: var(--cd-ink-2); white-space: nowrap; }

  .row-verdict { display: flex; align-items: center; gap: var(--cd-space-1); justify-self: end; }
  .verdict-mark { font-size: var(--cd-text-sm); font-weight: var(--cd-weight-figure); color: var(--cd-ink-2); }
  .row-verdict.good .verdict-mark { color: var(--cd-accent); }
  .row-verdict.bad .verdict-mark { color: var(--cd-danger-hi); }

  .list-foot {
    padding: var(--cd-space-2) var(--cd-space-4) var(--cd-space-3);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink-2);
    border-top: 1px solid var(--cd-line-soft);
    flex-shrink: 0;
  }

  @media (max-width: 640px), #{f.$phone} {
    .list-cols { display: none; }
    .hand-row {
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        'result pot'
        'cards cards'
        'main verdict';
      gap: var(--cd-space-2) var(--cd-space-3);
      padding: var(--cd-space-3);
      margin-bottom: var(--cd-space-2);
    }
    .row-result { grid-area: result; }
    .row-pot { grid-area: pot; }
    .row-cards { grid-area: cards; --mini-card-w: 28px; }
    .row-board { display: none; }
    .row-cards::after { content: ''; }
    .row-main { grid-area: main; flex-direction: row; gap: var(--cd-space-2); align-items: baseline; }
    .row-verdict { grid-area: verdict; }
    .chip { min-height: var(--cd-touch-min); }
  }
</style>
