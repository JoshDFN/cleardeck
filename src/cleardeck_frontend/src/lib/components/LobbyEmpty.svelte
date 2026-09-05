<script>
  // THE LIST WITH NOTHING IN IT, four ways.
  //
  //   failed:   the page's last read of the lobby threw (the toast carries the
  //             raw text); an empty list is NOT evidence of an empty lobby.
  //   none:     the lobby canister answered with no table registered.
  //   loading:  the rows are held back for one round of get_table_view()
  //             (the lobby record's config can be wrong, docs/DEFECTS.md T-11).
  //   filtered: tables exist, none fits the filters.
  import { shortId } from '../lobby-format.js';

  const {
    /** @type {'failed' | 'none' | 'loading' | 'filtered'} */
    kind,
    /** How many tables the lobby lists (loading, filtered). */
    count = 0,
    /** The lobby canister id (none). */
    lobbyId = null,
    copied = false,
    onCopy = () => {},
    onRetry = () => {},
    onClearFilters = () => {},
  } = $props();
</script>

{#if kind === 'failed'}
  <div class="empty">
    <h3>The tables could not be read.</h3>
    <p>The lobby canister did not answer. The page tries again on its own; Try again asks now.</p>
    <button class="cd-btn outline" onclick={onRetry}>Try again</button>
  </div>
{:else if kind === 'none'}
  <div class="empty">
    <h3>The lobby canister is reporting no tables.</h3>
    <p>
      Nothing is wrong with your connection: the lobby simply has no table
      registered right now. You can confirm that yourself by querying
      <code class="mono inline">get_tables()</code> on the lobby canister.
    </p>
    {#if lobbyId}
      <button class="mono copy" onclick={onCopy}>
        {shortId(lobbyId)}
        <span class="copy-state">{copied ? 'copied' : 'copy'}</span>
      </button>
    {/if}
    <button class="cd-btn outline" onclick={onRetry}>Try again</button>
  </div>
{:else if kind === 'loading'}
  <div class="skeleton" aria-live="polite">
    <p>Reading each table's own contract…</p>
    {#each Array.from({ length: count }) as _, i (i)}
      <span class="skeleton-row"></span>
    {/each}
  </div>
{:else}
  <div class="empty">
    <h3>No table matches this filter.</h3>
    <p>{count} {count === 1 ? 'table is' : 'tables are'} open; none of them fits the filters above.</p>
    <button class="cd-btn outline" onclick={onClearFilters}>Clear filters</button>
  </div>
{/if}

<style>
  .empty {
    padding: var(--cd-space-6) var(--cd-space-6);
    text-align: center;
  }

  .empty h3 {
    margin: 0 0 var(--cd-space-2);
    font-size: var(--cd-text-lg);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink);
  }

  .empty p {
    margin: 0 auto var(--cd-space-3);
    max-width: 46ch;
    font-size: var(--cd-text-sm);
    line-height: 1.6;
    color: var(--cd-ink-2);
  }

  .skeleton { padding: var(--cd-space-5) var(--cd-space-4) var(--cd-space-4); }

  .skeleton p {
    margin: 0 0 var(--cd-space-3);
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-2);
  }

  .skeleton-row {
    display: block;
    height: 64px;
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
    margin-bottom: var(--cd-space-2);
  }

  .mono {
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    color: var(--cd-ink-2);
    word-break: break-all;
  }

  .mono.inline { color: var(--cd-ink-1); }

  button.mono {
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    margin: 0 auto var(--cd-space-3);
    background: var(--cd-surface-2);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-chip);
    padding: 6px 9px;
    cursor: pointer;
  }

  .copy-state {
    flex: none;
    font-family: inherit;
    font-size: var(--cd-text-xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--cd-accent);
  }

  @media (max-width: 760px) {
    button.mono { min-height: var(--cd-touch-min); }
  }
</style>
