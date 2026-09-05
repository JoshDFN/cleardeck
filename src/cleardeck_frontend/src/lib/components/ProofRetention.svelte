<script>
  /**
   * How long this proof survives, and who can take it away. Read live off the
   * canister; the headline sentence is on screen, never behind a toggle (a
   * fairness guarantee you cannot re-check tomorrow is not a guarantee), and the
   * catch folds under it. Wording verbatim from the fairness panel.
   *
   * Harness: `.retention` is the class the token allowlist excuses the hand
   * cap under (occupancy-count); the archive id is a `code`.
   */
  const { retention = null, archiving = false } = $props();
</script>

<div class="retention" class:warn={retention?.state === 'known' && !archiving} class:unknown={retention?.state !== 'known'}>
  <span class="retention-head">How long this proof survives</span>
  {#if retention?.state === 'known' && archiving}
    <p>
      This table keeps only its <strong>last {retention.cap} hands</strong>, and any controller of
      the table can erase all of them with one call. Every settled hand is also written to a separate
      archive canister, <code>{retention.archive}</code>, which has no method that deletes, prunes or
      edits a record. A hand recorded there is permanent for the life of that canister.
    </p>
    <details class="fold">
      <summary>What can still destroy it</summary>
      <p class="retention-catch fold-body">
        <strong>What can still destroy it:</strong> a controller of that archive canister, by
        reinstalling or deleting the canister itself. No code can prevent that. If you want a copy
        nobody else can take away, copy the seed and the hash above and keep them.
      </p>
    </details>
  {:else if retention?.state === 'known'}
    <p class="retention-bad">
      {#if !retention.archive}
        <strong>No archive is configured on this table.</strong> The only copy of this proof is the
        last {retention.cap} hands held in the table canister. Older hands are already gone, and one
        controller call erases the rest.
      {:else}
        <strong>{retention.backlog} settled hand(s) have not reached the archive.</strong> Until they
        do, those proofs exist only in the table's {retention.cap}-hand ring.
      {/if}
      Copy the seed and the hash above now if you may want to re-check this hand later.
    </p>
  {:else}
    <p class="retention-bad">
      <strong>Unknown.</strong> This page could not get a durability answer out of the table, so
      assume the proof survives only inside the table's own capped ring of recent hands, which one
      controller call erases. Copy the seed and the hash above.
    </p>
  {/if}
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.disclosure;

  .retention {
    margin-top: var(--cd-space-3);
    padding: var(--cd-space-3) var(--cd-space-4);
    border-radius: var(--cd-radius-card);
    background: var(--cd-accent-dim);
    border: 1px solid var(--cd-accent-line);
  }

  .retention.warn, .retention.unknown { background: var(--cd-warn-dim); border-color: var(--cd-warn-line); }

  .retention-head {
    display: block;
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-accent);
    margin-bottom: var(--cd-space-1);
  }

  .retention.warn .retention-head, .retention.unknown .retention-head { color: var(--cd-warn); }
  .retention p { margin: 0; font-size: var(--cd-text-xs); line-height: 1.6; color: var(--cd-ink-1); }
  .retention strong { color: var(--cd-ink); }
  .retention code { font-family: var(--cd-font-mono); font-size: var(--cd-text-xs); color: var(--cd-accent); word-break: break-all; }
  .retention-catch { color: var(--cd-ink-2); }
  .retention-bad { color: var(--cd-warn); }
  .retention-bad strong { color: var(--cd-warn); }
  details.fold { margin-top: var(--cd-space-2); }
  details.fold > summary { font-size: var(--cd-text-xs); min-height: var(--cd-control-sm); }
</style>
