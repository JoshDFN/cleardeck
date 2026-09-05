<script>
  // NOBODY IS SEATED ANYWHERE: the cold-start row.
  //
  // PokerNow is the honest comparison (no public liquidity either) and its
  // whole product is "sit down, share the link". So when every table is
  // empty the list does not open with four zeros; it opens with the one
  // thing a visitor can do about it: open the cheapest ICP table and send
  // the invite link. The hand deals the moment the second player sits.
  //
  // ONE ROW, on purpose: the panel sits between the pane bar and the first
  // card, and every pixel it takes pushes the first card down (the audit's
  // first metric). Desktop: the sentence and two compact buttons on one
  // line. Phone: the two buttons, the sentence folded into the first.

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
  <p class="cold-title">Nobody is seated yet. <strong>Be the first.</strong></p>
  <div class="cold-actions">
    {#if starter}
      <button class="btn primary" onclick={() => onOpen(starter)}>
        <span class="wide">Open {starter.name}</span>
        <span class="phone">Be the first at {starter.name}</span>
      </button>
    {/if}
    {#if inviteLink}
      <button class="btn ghost" onclick={() => onCopyInvite(inviteLink, 'cold-start')}>
        {#if copied}
          Link copied
        {:else}
          <span class="wide">Copy invite link</span>
          <span class="phone">Copy invite</span>
        {/if}
      </button>
    {/if}
  </div>
</div>

<style>
  .cold-start {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--cd-space-3) var(--cd-space-4);
    margin: 0 0 var(--cd-space-2);
    padding: var(--cd-space-2) var(--cd-space-2) var(--cd-space-2) var(--cd-space-4);
    border: 1px solid var(--cd-accent-line);
    border-radius: var(--cd-radius-card);
    background:
      radial-gradient(90% 140% at 100% 0%, var(--cd-accent-dim), transparent 60%),
      var(--cd-surface-1);
  }

  .cold-title {
    margin: 0;
    min-width: 0;
    font-size: var(--cd-text-sm);
    color: var(--cd-ink-1);
  }

  .cold-title strong { color: var(--cd-accent-hi); font-weight: var(--cd-weight-figure); }

  .cold-actions { display: flex; gap: var(--cd-space-2); flex: none; }

  .phone { display: none; }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
    font: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    white-space: nowrap;
  }

  .btn.primary { background: var(--cd-accent); color: var(--cd-accent-ink); box-shadow: var(--cd-gloss); }
  .btn.primary:hover { background: var(--cd-accent-hi); }
  .btn.ghost { background: var(--cd-surface-2); border-color: var(--cd-line-strong); color: var(--cd-ink); }
  .btn.ghost:hover { background: var(--cd-surface-3); }

  @media (max-width: 760px) {
    .cold-start { padding: var(--cd-space-1) var(--cd-space-2); }
    .cold-title { display: none; }
    .cold-actions { flex: 1 1 auto; }
    .wide { display: none; }
    .phone { display: inline; }
    .btn { min-height: var(--cd-touch-min); padding: 0 var(--cd-space-2); }
    .btn.primary { flex: 1 1 0; white-space: normal; }
  }
</style>
