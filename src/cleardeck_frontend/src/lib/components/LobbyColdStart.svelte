<script>
  // NOBODY IS SEATED ANYWHERE: the cold-start panel.
  //
  // PokerNow is the honest comparison (no public liquidity either) and its
  // whole product is "sit down, share the link". So when every table is
  // empty the list does not open with four zeros; it opens with the one
  // thing a visitor can do about it: open the cheapest ICP table and send
  // the invite link. The hand deals the moment the second player sits.

  const {
    /** The table to open first (lobby-rows.js starterTable), or null. */
    starter = null,
    /** The invite link for that table, or null when it has no canister. */
    inviteLink = null,
    copied = false,
    onOpen = () => {},
    onCopyInvite = () => {},
  } = $props();
</script>

<div class="cold-start" role="note">
  <div class="cold-copy">
    <p class="cold-title">Nobody is seated yet. Be the first.</p>
    <p class="cold-body">
      Sit down and send the link: the hand deals the moment your opponent sits.
      Sitting down costs nothing until you buy in, and the pot is never raked.
    </p>
  </div>
  <div class="cold-actions">
    {#if starter}
      <button class="btn primary" onclick={() => onOpen(starter)}>
        Open {starter.name}
      </button>
    {/if}
    {#if inviteLink}
      <button class="btn ghost" onclick={() => onCopyInvite(inviteLink, 'cold-start')}>
        {copied ? 'Link copied' : 'Copy invite link'}
      </button>
    {/if}
  </div>
</div>

<style>
  .cold-start {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--cd-space-4);
    margin: 0 0 var(--cd-space-2);
    padding: var(--cd-space-3) var(--cd-space-4);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-card);
    background:
      radial-gradient(90% 140% at 100% 0%, var(--cd-accent-dim), transparent 60%),
      var(--cd-surface-1);
  }

  .cold-title {
    margin: 0 0 2px;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-figure);
    color: var(--cd-accent-hi);
  }

  .cold-body {
    margin: 0;
    max-width: 62ch;
    font-size: var(--cd-text-sm);
    line-height: 1.5;
    color: var(--cd-ink-1);
  }

  .cold-actions { display: flex; gap: var(--cd-space-2); flex: none; flex-wrap: wrap; }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
    font: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    white-space: nowrap;
  }

  .btn.primary { background: var(--cd-accent); color: var(--cd-accent-ink); }
  .btn.primary:hover { background: var(--cd-accent-hi); }
  .btn.ghost { background: var(--cd-surface-2); border-color: var(--cd-line-strong); color: var(--cd-ink); }
  .btn.ghost:hover { background: var(--cd-surface-3); }

  @media (max-width: 760px) {
    .cold-start { flex-direction: column; align-items: stretch; gap: var(--cd-space-2); padding: var(--cd-space-3); }
    .cold-body { font-size: var(--cd-text-xs); }
    .cold-actions { flex-wrap: nowrap; }
    .cold-actions .btn { flex: 1 1 0; min-height: var(--cd-touch-min); padding: 0 var(--cd-space-2); white-space: normal; }
  }
</style>
