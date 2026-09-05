<script>
  // The fairness panel, verdict first.
  //
  // This used to call `tableActor.verify_shuffle(seed_hash, revealed_seed)` and
  // print "Cryptographically verified shuffle" when it came back true. That is
  // the house checking its own homework (docs/FINDING-02, item 4). The
  // verification runs HERE, in the player's browser, from
  // `$lib/shuffle-verify.js` (a port of docs/SHUFFLE-SPEC.md that imports
  // nothing and calls nothing): it re-derives the whole 52-card deck from the
  // revealed seed and shows the player their OWN two cards and the board
  // coming out of it at the positions the dealing rule predicts.
  //
  // WHAT THE FIRST PAINT IS. One verdict card: the seal glyph of the
  // commitment, the sentence the spec proves ("the whole deck was fixed before
  // the board was shown"), your two cards derived beside your two cards dealt,
  // the board cell by cell, the tally, the computed seal snapping onto the
  // committed one, and the limit ("not proven: that the commitment came before
  // the cards") as a tag. The four-rung chain of evidence, the raw hashes, the
  // deck grid, the independent verifiers and the proven / not-proven table are
  // all still here, word for word, behind disclosures. The retention sentence
  // stays on screen: a guarantee you cannot re-check tomorrow is not one.
  //
  // The canister's own commitment check is still called, but only inside
  // "Show the work", explicitly labelled as proving nothing. Its answer is never
  // allowed to influence the verdict. `check_shuffle_commitment` takes a RECORD
  // and answers with a named variant (docs/DEFECTS.md E-44); the panel also
  // states HOW LONG the proof survives and WHO CAN DESTROY IT, read live off
  // the canister (docs/DEFECTS.md T-34).

  import Card from './Card.svelte';
  import HashSeal from './HashSeal.svelte';
  import ProofEvidence from './ProofEvidence.svelte';
  import ProofMath from './ProofMath.svelte';
  import ProofElsewhere from './ProofElsewhere.svelte';
  import ProofLimits from './ProofLimits.svelte';
  import ProofRetention from './ProofRetention.svelte';
  import logger from '$lib/logger.js';
  import { verifyHandLocally, toCandidCard } from '$lib/shuffle-verify.js';
  import { hashFingerprint } from '$lib/hash-seal.js';
  import { readSighting, verdictForSighting } from '$lib/commitment-witness.js';

  const { proof, handNumber, tableActor, tableId = null } = $props();

  let report = $state(null);          // the result of the LOCAL verification
  let observed = $state(null);        // what the table showed us, to be checked
  let phase = $state('idle');         // idle | working | done | failed
  let failure = $state(null);
  let copied = $state(null);
  let showWork = $state(false);
  let houseEcho = $state(null);       // the canister's own answer; decorative only
  let retention = $state(null);       // how long this proof lasts, read off the chain

  // `tableActor` is `createTableActorProxy(...)`: a Proxy that builds a real
  // actor per call, so a method can still be absent on an older canister; the
  // real handling is the rejected call caught below, degrading to "unknown".
  const canAsk = (method) => typeof tableActor?.[method] === 'function';

  /** Candid `opt T` arrives as [] or [value]; some paths hand us the value. */
  function opt(value) {
    if (Array.isArray(value)) return value.length > 0 ? value[0] : null;
    return value === undefined ? null : value;
  }

  const revealedSeed = $derived(
    (() => {
      const seed = opt(proof?.revealed_seed);
      return typeof seed === 'string' && seed.length > 0 ? seed : null;
    })(),
  );

  // ---------------------------------------------------------------------
  // What the table showed the player. None of this is trusted: it is the
  // claim that the local re-derivation is about to check.
  // ---------------------------------------------------------------------
  async function gatherObserved() {
    const seen = {
      myCards: null, board: [], maxPlayers: 9, mySeat: null,
      expectedPlayers: null, cardsFrom: null, boardFrom: null,
    };
    if (!tableActor) return seen;

    try {
      const view = opt(await tableActor.get_table_view());
      if (view) {
        seen.maxPlayers = Number(view.config?.max_players ?? 9) || 9;
        const seat = opt(view.my_seat);
        seen.mySeat = seat === null ? null : Number(seat);
        const me = (view.players || []).map(opt).find((p) => p?.is_self);
        const pair = opt(me?.hole_cards);
        if (pair) {
          seen.myCards = [pair[0], pair[1]];
          seen.cardsFrom = 'your seat at the table';
        }
        if (view.community_cards?.length) {
          seen.board = view.community_cards;
          seen.boardFrom = 'the felt';
        }
      }
    } catch (e) {
      logger.debug('ShuffleProof: get_table_view unavailable', e);
    }

    try {
      const record = opt(await tableActor.get_hand_history(BigInt(handNumber ?? 0)));
      if (record) {
        if (record.community_cards?.length) {
          seen.board = record.community_cards;
          seen.boardFrom = "this hand's own record";
        }
        const seats = new Set();
        (record.actions || []).forEach((a) => seats.add(Number(a.seat)));
        (record.winners || []).forEach((w) => seats.add(Number(w.seat)));
        (record.showdown_players || []).forEach((p) => seats.add(Number(p.seat)));
        if (seats.size >= 2) seen.expectedPlayers = seats.size;
        if (!seen.myCards && seen.mySeat !== null) {
          const mine = [...(record.showdown_players || []), ...(record.winners || [])]
            .find((p) => Number(p.seat) === seen.mySeat);
          const pair = opt(mine?.cards);
          if (pair) {
            seen.myCards = [pair[0], pair[1]];
            seen.cardsFrom = "this hand's own record";
          }
        }
      }
    } catch (e) {
      logger.debug('ShuffleProof: get_hand_history unavailable', e);
    }

    if (!seen.myCards) {
      try {
        const pair = opt(await tableActor.get_my_cards());
        if (pair) {
          seen.myCards = [pair[0], pair[1]];
          seen.cardsFrom = 'the cards you were dealt';
        }
      } catch (e) {
        logger.debug('ShuffleProof: get_my_cards unavailable', e);
      }
    }
    return seen;
  }

  /**
   * Reading what was dealt is bounded, because the verification does not depend
   * on it succeeding: a fairness panel that sits on a spinner while its own
   * arithmetic is already possible would be telling the player the opposite of
   * the truth about where the check runs.
   */
  function withDeadline(work, ms, fallback) {
    return Promise.race([
      work,
      new Promise((resolve) => setTimeout(() => resolve(fallback), ms)),
    ]);
  }

  /** Prefers a fresh reading, falls back to the previous one field by field. */
  function mergeObserved(previous, fresh) {
    if (!previous) return fresh;
    return {
      myCards: fresh.myCards || previous.myCards,
      board: fresh.board?.length ? fresh.board : previous.board,
      maxPlayers: fresh.maxPlayers || previous.maxPlayers,
      mySeat: fresh.mySeat ?? previous.mySeat,
      expectedPlayers: fresh.expectedPlayers ?? previous.expectedPlayers,
      cardsFrom: fresh.cardsFrom || previous.cardsFrom,
      boardFrom: fresh.boardFrom || previous.boardFrom,
    };
  }

  // ---------------------------------------------------------------------
  // The verification itself. Everything below this line is arithmetic in
  // this tab: no canister answer can change the verdict.
  // ---------------------------------------------------------------------
  async function runVerification() {
    if (!proof?.seed_hash || !revealedSeed) return;
    phase = 'working';
    failure = null;
    houseEcho = null;

    const nothingSeen = {
      myCards: null, board: [], maxPlayers: 9, mySeat: null, expectedPlayers: null,
    };
    let seen;
    try {
      seen = await withDeadline(gatherObserved(), 5000, nothingSeen);
    } catch (e) {
      logger.error('ShuffleProof: could not read what was dealt', e);
      seen = nothingSeen;
    }
    // The cards are the CLAIM being checked, not the check, so reusing an
    // earlier reading is honest, and the verification still runs offline.
    seen = mergeObserved(observed, seen);
    observed = seen;

    try {
      report = await verifyHandLocally({
        seedHashHex: proof.seed_hash,
        revealedSeedHex: revealedSeed,
        myCards: seen.myCards,
        board: seen.board,
        maxPlayers: seen.maxPlayers,
        expectedPlayers: seen.expectedPlayers,
      });
      phase = report.error && !report.layout ? 'failed' : 'done';
      failure = report.error;
    } catch (e) {
      logger.error('ShuffleProof: local verification threw', e);
      report = null;
      failure = e?.message || 'The local verifier could not run in this browser.';
      phase = 'failed';
    }

    askTheHouse();    // deliberately last, deliberately not awaited into the verdict
    readRetention();  // same: a durability answer must never gate the arithmetic
  }

  /**
   * Asks the table canister its own commitment check. This proves NOTHING (the
   * accused re-hashing its own evidence) and is shown only inside the work so
   * the player can see the difference. Asked TWICE, straight and transposed,
   * because the endpoint this replaced answered "false" to the second question
   * (docs/DEFECTS.md E-44).
   */
  async function askTheHouse() {
    if (!proof?.seed_hash || !revealedSeed) return;
    if (!canAsk('check_shuffle_commitment')) return;
    houseEcho = { state: 'asking', straight: null, swapped: null };
    try {
      const [straight, swapped] = await withDeadline(
        Promise.all([
          tableActor.check_shuffle_commitment({ seed_hash: proof.seed_hash, revealed_seed: revealedSeed }),
          tableActor.check_shuffle_commitment({ seed_hash: revealedSeed, revealed_seed: proof.seed_hash }),
        ]),
        8000,
        ['timeout', 'timeout'],
      );
      if (straight === 'timeout') {
        houseEcho = { state: 'unreachable', straight: null, swapped: null };
        return;
      }
      houseEcho = {
        state: 'answered',
        straight: Object.keys(straight ?? {})[0] ?? null,
        swapped: Object.keys(swapped ?? {})[0] ?? null,
      };
    } catch (e) {
      logger.debug('ShuffleProof: canister echo unavailable', e);
      houseEcho = { state: 'unreachable', straight: null, swapped: null };
    }
  }

  /** How long this proof survives, read off the table canister. */
  async function readRetention() {
    if (!canAsk('get_fairness_retention') || !canAsk('get_history_status')) {
      retention = { state: 'unknown' };
      return;
    }
    try {
      const [policy, status] = await withDeadline(
        Promise.all([tableActor.get_fairness_retention(), tableActor.get_history_status()]),
        8000,
        ['timeout', 'timeout'],
      );
      if (policy === 'timeout') {
        retention = { state: 'unknown' };
        return;
      }
      const archive = opt(policy.archive_canister);
      retention = {
        state: 'known',
        cap: Number(policy.table_keeps_last_n_hands),
        archive: archive ? archive.toText() : null,
        backlog: Number(status.unrecorded_backlog ?? 0n),
        dropped: Number(status.unrecorded_dropped ?? 0n),
        failed: Number(status.failed_since_start ?? 0n),
        recorded: Number(status.recorded_ok_since_start ?? 0n),
      };
    } catch (e) {
      logger.debug('ShuffleProof: retention unavailable', e);
      retention = { state: 'unknown' };
    }
  }

  /** True only when a durable copy is configured AND nothing is stuck. */
  const archiving = $derived(
    retention?.state === 'known' && !!retention.archive && retention.backlog === 0,
  );

  // Deliberately plain variables, NOT $state: the effect must not take a
  // dependency on them, or writing `phase`/`report` would retrigger it forever.
  let verifiedKey = '';
  let retentionAsked = false;

  $effect(() => {
    if (!tableActor || retentionAsked) return;
    retentionAsked = true;
    readRetention();
  });

  $effect(() => {
    const hash = proof?.seed_hash;
    const seed = revealedSeed;
    const key = hash && seed ? `${handNumber}|${hash}|${seed}` : '';
    if (key === verifiedKey) return;
    verifiedKey = key;
    if (!key) {
      report = null;
      observed = null;
      phase = 'idle';
      failure = null;
      houseEcho = null;
      return;
    }
    runVerification();
  });

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

  function formatTimestamp(ns) {
    if (!ns) return 'N/A';
    return new Date(Number(ns) / 1_000_000).toLocaleString();
  }

  const ordinal = (n) => ['1st', '2nd', '3rd', '4th', '5th', '6th', '7th', '8th', '9th'][n] || `${n + 1}th`;

  const holeCheck = $derived(report?.checks?.find((c) => c.kind === 'hole') || null);
  const boardChecks = $derived(report?.checks?.filter((c) => c.kind === 'board') || []);
  const verified = $derived(phase === 'done' && !!report?.ok);
  const failed = $derived(phase === 'failed' || (!!report && !report.ok && report.cardsChecked > 0));

  /**
   * The ordering, witnessed: the hand-history panel notes a live hand's
   * commitment while its seed is sealed (lib/commitment-witness.js). If this
   * browser holds such a sighting for THIS hand and it matches, the "before"
   * was seen, not told.
   */
  const sighting = $derived(handNumber ? readSighting({ tableId: tableId || 'table', handNumber }) : null);
  const witness = $derived(sighting && proof?.seed_hash ? verdictForSighting(sighting, proof.seed_hash) : null);
</script>

<div class="shuffle-proof" data-phase={phase}>
  <div class="proof-header">
    <div class="header-left">
      <HashSeal hash={proof?.seed_hash || ''} size={44} tone={verified ? 'accent' : 'muted'} />
      <div>
        <h3>Provably Fair</h3>
        <span class="subtitle">
          {#if verified}
            Checked in your browser, not by us
          {:else if revealedSeed}
            Re-deriving your cards locally
          {:else}
            Seed committed, not yet revealed
          {/if}
        </span>
      </div>
    </div>
    <span class="hand-number">Hand #{handNumber ?? 'N/A'}</span>
  </div>

  {#if !proof}
    <div class="no-proof">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
      <p>The fairness proof appears as soon as cards are dealt.</p>
    </div>
  {:else}
    <!-- ============================================================ -->
    <!-- THE VERDICT CARD: the first paint.                             -->
    <!-- ============================================================ -->
    <div class="verdict-card" class:good={verified} class:bad={failed} class:sealed={!revealedSeed}>
      {#if verified}
        <div class="headline good">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20,6 9,17 4,12"/></svg>
          <span>The whole deck was fixed before the board was shown, and every card you saw came from it. Verified on your machine, not by us.</span>
        </div>
      {:else if failed}
        <div class="headline bad">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
          <span>This hand did not verify. Do not take our word for anything below.</span>
        </div>
      {:else if !revealedSeed}
        <div class="pending-line sealed">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>
          <span>Deck sealed. The seed is revealed when the hand ends; your browser checks it then.</span>
        </div>
      {:else}
        <div class="pending-line working">
          <span class="spinner"></span>
          <span>Re-deriving the deck in this tab…</span>
        </div>
      {/if}

      {#if report?.layout}
        {#if holeCheck}
          <div class="rederive" class:good={holeCheck.match} class:bad={!holeCheck.match}>
            <div class="rederive-head">
              <span class="rederive-title">Your cards</span>
              <span class="rederive-pos">deck positions {holeCheck.positions[0]} &amp; {holeCheck.positions[1]}</span>
            </div>
            <div class="rederive-row">
              <div class="side">
                <span class="side-label">derived here</span>
                <div class="cards">
                  <Card card={toCandidCard(holeCheck.derivedCards[0])} />
                  <Card card={toCandidCard(holeCheck.derivedCards[1])} />
                </div>
              </div>
              <div class="equals" class:good={holeCheck.match}>{holeCheck.match ? '=' : '≠'}</div>
              <div class="side">
                <span class="side-label">dealt to you</span>
                <div class="cards">
                  <Card card={observed?.myCards?.[0]} />
                  <Card card={observed?.myCards?.[1]} />
                </div>
              </div>
            </div>
          </div>
        {:else}
          <div class="soft-note">
            This browser rebuilt the deck, but the table did not return your hole cards for this
            hand, so there is nothing of yours to compare against. The board below is still checked.
          </div>
        {/if}

        {#if boardChecks.length}
          <div class="rederive" class:good={boardChecks.every((c) => c.match)}>
            <div class="rederive-head">
              <span class="rederive-title">The board</span>
              <span class="rederive-pos">deck positions {boardChecks.map((c) => c.positions[0]).join(', ')}</span>
            </div>
            <div class="board-grid">
              {#each boardChecks as check}
                <div class="board-cell" class:good={check.match} class:bad={!check.match}>
                  <span class="cell-label">{check.label}</span>
                  <Card card={toCandidCard(check.derivedCards[0])} />
                  <span class="cell-mark">{check.match ? '✓' : '✗'}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <div class="tally" class:good={report.ok} class:bad={!report.ok}>
          {#if report.ok}
            <strong>{report.cardsMatched} of {report.cardsChecked} cards</strong> you saw this hand were
            re-derived in this browser from the committed seed, each at exactly the position the
            dealing rule puts it. Change any one of them and the seal stops matching.
          {:else if report.cardsChecked > 0}
            <strong>{report.cardsMatched} of {report.cardsChecked} cards</strong> matched.
            {#if !report.commitment.match}
              The commitment failed, so this deal is not accounted for.
            {/if}
          {:else}
            The deck was rebuilt locally, but no dealt card was available to check it against.
          {/if}
        </div>
      {:else if phase === 'failed' && failure}
        <div class="soft-note bad">{failure}</div>
      {/if}

      {#if report}
        <!-- computed vs committed: two seals that become one -->
        <div class="seal-row" class:good={report.commitment.match} class:bad={!report.commitment.match}>
          <span class="seal-pair" aria-hidden="true">
            <HashSeal hash={report.commitment.computed} size={30} label="computed here" />
            <span class="seal-eq">{report.commitment.match ? '=' : '≠'}</span>
            <HashSeal hash={report.commitment.committed} size={30} label="committed" tone={report.commitment.match ? 'accent' : 'muted'} />
          </span>
          <span class="seal-text">
            {#if report.commitment.match}
              Seal <code class="mono" title={report.commitment.committed}>{hashFingerprint(report.commitment.committed)}</code> matches the revealed seed ✓
            {:else}
              Seal <code class="mono" title={report.commitment.committed}>{hashFingerprint(report.commitment.committed)}</code> does NOT match the revealed seed ✗
            {/if}
          </span>
        </div>
      {:else}
        <div class="seal-row sealed">
          <span class="seal-text">Seal <code class="mono" title={proof.seed_hash}>{hashFingerprint(proof.seed_hash)}</code>, on screen since the first card.</span>
        </div>
      {/if}

      <p class="limit-line">
        {#if witness?.witnessed}
          <span class="witness-tag">Witnessed</span>
          You read this seal while the hand was still running, so the "before" is something you saw.
        {:else}
          <span class="caveat-tag">Not proven</span>
          That the commitment came <em>before</em> the cards. Copy the seal while a hand is running and compare it after the reveal; the hand history notes one for you.
        {/if}
      </p>
    </div>

    {#if revealedSeed}
      <button class="rerun-btn" onclick={runVerification} disabled={phase === 'working'}>
        {phase === 'working' ? 'Verifying…' : 'Run the check again'}
      </button>
    {/if}

    <!-- ============================================================ -->
    <!-- Everything else, word for word, behind disclosures.             -->
    <!-- ============================================================ -->
    <details class="fold">
      <summary>The chain of evidence</summary>
      <div class="fold-body">
        <ProofEvidence {proof} {revealedSeed} {report} {phase} {failure} {holeCheck} {formatTimestamp} {ordinal} />
      </div>
    </details>

    <details class="fold">
      <summary>Show the math</summary>
      <div class="fold-body">
        <ProofMath {proof} {revealedSeed} {report} {houseEcho} {copied} onCopy={copyToClipboard} bind:showWork />
      </div>
    </details>

    {#if revealedSeed}
      <details class="fold">
        <summary>Don't trust this page either</summary>
        <div class="fold-body">
          <ProofElsewhere {proof} {revealedSeed} players={report?.layout?.players ?? 2} {copied} onCopy={copyToClipboard} />
        </div>
      </details>
    {/if}

    <details class="fold">
      <summary>What this proves, and what it does not</summary>
      <div class="fold-body">
        <ProofLimits {proof} {formatTimestamp} />
      </div>
    </details>

    <ProofRetention {retention} {archiving} />
  {/if}
</div>

<style lang="scss">
  @use './fairness' as f;
  @include f.disclosure;

  .shuffle-proof {
    background: var(--cd-sheet);
    border: 1px solid var(--cd-line);
    border-radius: var(--cd-radius-panel);
    padding: var(--cd-space-4);
    max-width: 520px;
    box-shadow: var(--cd-shadow-lift);
  }

  .proof-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--cd-space-2);
    margin-bottom: var(--cd-space-3);
  }

  .header-left { display: flex; gap: var(--cd-space-3); align-items: center; min-width: 0; }
  .proof-header h3 { color: var(--cd-ink); margin: 0; font-size: var(--cd-text-lg); font-weight: var(--cd-weight-figure); }
  .subtitle { color: var(--cd-ink-2); font-size: var(--cd-text-xs); }

  .hand-number {
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    background: var(--cd-surface-2);
    padding: var(--cd-space-1) var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    white-space: nowrap;
  }

  /* ---- the verdict card ------------------------------------------------ */
  .verdict-card {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-3);
    padding: var(--cd-space-3);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line);
  }

  .verdict-card.good { border-color: var(--cd-accent-line); background: var(--cd-accent-dim); }
  .verdict-card.bad { border-color: var(--cd-danger-line); background: var(--cd-danger-dim); }

  .headline, .pending-line {
    display: flex;
    align-items: flex-start;
    gap: var(--cd-space-2);
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-strong);
    line-height: 1.4;
    color: var(--cd-ink);
  }

  .headline svg, .pending-line svg { flex-shrink: 0; margin-top: 2px; }
  .headline.good { color: var(--cd-accent); }
  .headline.bad { color: var(--cd-danger-hi); }
  /* NOT `.headline`: the shuffleproof scene waits on `.headline` as the VERDICT
     and reads the panel the moment it appears; a headline for "still working"
     made it read a panel that had not finished (phase 6 run 1). */
  .pending-line.sealed { color: var(--cd-warn); }
  .pending-line.working { color: var(--cd-ink-2); }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--cd-line);
    border-top-color: var(--cd-accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
    margin-top: 3px;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* Re-derived cards, side by side with the dealt ones */
  .rederive {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-bg-deep);
    border: 1px solid var(--cd-line-soft);
  }

  .rederive.good { border-color: var(--cd-accent-line); }
  .rederive.bad { border-color: var(--cd-danger-line); }

  .rederive-head { display: flex; justify-content: space-between; align-items: baseline; gap: var(--cd-space-2); margin-bottom: var(--cd-space-2); }
  .rederive-title { font-size: var(--cd-text-xs); font-weight: var(--cd-weight-figure); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); color: var(--cd-ink-1); }
  .rederive-pos { font-size: var(--cd-text-xs); color: var(--cd-ink-2); font-family: var(--cd-font-mono); }
  .rederive-row { display: flex; align-items: center; gap: var(--cd-space-3); }
  .side { display: flex; flex-direction: column; gap: var(--cd-space-1); }
  .side-label { font-size: var(--cd-text-xs); text-transform: uppercase; letter-spacing: var(--cd-tracking-label); color: var(--cd-ink-2); }
  .cards { --card-w: 42px; display: flex; gap: var(--cd-space-1); }
  .equals { font-size: var(--cd-text-xl); font-weight: var(--cd-weight-figure); color: var(--cd-ink-2); }
  .equals.good { color: var(--cd-accent); }

  .board-grid { --card-w: 34px; display: flex; flex-wrap: wrap; gap: 3px; }

  .board-cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 3px 2px;
    border-radius: var(--cd-radius-chip);
    border: 1px solid transparent;
  }

  .board-cell.good { border-color: var(--cd-accent-line); }
  .board-cell.bad { border-color: var(--cd-danger-line); }
  .cell-label { font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em; color: var(--cd-ink-2); }
  .cell-mark { font-size: var(--cd-text-xs); color: var(--cd-accent); font-weight: var(--cd-weight-figure); }
  .board-cell.bad .cell-mark { color: var(--cd-danger-hi); }

  .tally {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    line-height: 1.55;
    color: var(--cd-ink-1);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line-soft);
  }

  .tally.good { border-color: var(--cd-accent-line); }
  .tally.bad { border-color: var(--cd-danger-line); }
  .tally strong { color: var(--cd-ink); }

  .soft-note {
    padding: var(--cd-space-2) var(--cd-space-3);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-sm);
    line-height: 1.5;
    color: var(--cd-ink-2);
    background: var(--cd-surface-1);
    border: 1px solid var(--cd-line-soft);
  }

  .soft-note.bad { color: var(--cd-danger-hi); border-color: var(--cd-danger-line); }

  .seal-row { display: flex; align-items: center; gap: var(--cd-space-3); font-size: var(--cd-text-sm); color: var(--cd-ink-1); }
  .seal-pair { display: inline-flex; align-items: center; gap: var(--cd-space-1); flex: 0 0 auto; }
  .seal-eq { font-weight: var(--cd-weight-figure); color: var(--cd-ink-2); }
  .seal-row.good .seal-eq { color: var(--cd-accent); }
  .seal-row.bad .seal-eq, .seal-row.bad .seal-text { color: var(--cd-danger-hi); }
  .seal-text code { font-family: var(--cd-font-mono); color: var(--cd-accent); }
  .seal-row.good .seal-pair :global(.seal + .seal-eq + .seal) { animation: seal-snap var(--cd-flip) var(--cd-ease-land) both; }

  @keyframes seal-snap {
    from { transform: translateX(-6px); opacity: 0.4; }
    to { transform: none; opacity: 1; }
  }

  .limit-line { margin: 0; font-size: var(--cd-text-xs); line-height: 1.55; color: var(--cd-ink-2); }

  .caveat-tag, .witness-tag {
    display: inline-block;
    margin-right: var(--cd-space-1);
    padding: 1px var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    vertical-align: 1px;
  }

  .caveat-tag { background: var(--cd-warn-dim); border: 1px solid var(--cd-warn-line); color: var(--cd-warn); }
  .witness-tag { background: var(--cd-accent-dim); border: 1px solid var(--cd-accent-line); color: var(--cd-accent); }

  .rerun-btn {
    width: 100%;
    margin-top: var(--cd-space-3);
    min-height: var(--cd-control-md);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line);
    background: var(--cd-surface-1);
    color: var(--cd-ink-1);
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    cursor: pointer;
    transition: background var(--cd-base) var(--cd-ease), color var(--cd-base) var(--cd-ease);
  }

  .rerun-btn:hover:not(:disabled) { background: var(--cd-surface-2); color: var(--cd-ink); }
  .rerun-btn:disabled { opacity: 0.6; cursor: wait; }

  details.fold { margin-top: var(--cd-space-2); }

  .no-proof { text-align: center; padding: var(--cd-space-6); color: var(--cd-ink-2); }
  .no-proof svg { opacity: 0.4; margin-bottom: var(--cd-space-3); }
  .no-proof p { margin: 0; font-size: var(--cd-text-sm); }

  @media (max-width: 500px) {
    .shuffle-proof { padding: var(--cd-space-4); }
    .rederive-row { flex-wrap: wrap; }
    .equals { align-self: flex-end; padding-bottom: var(--cd-space-2); }
  }

  @media #{f.$phone} {
    .rerun-btn { min-height: var(--cd-touch-min); }
  }
</style>
