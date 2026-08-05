<script>
  // The fairness panel.
  //
  // This used to call `tableActor.verify_shuffle(seed_hash, revealed_seed)` and
  // print "Cryptographically verified shuffle" when it came back true. That is
  // the house checking its own homework: the query runs on the table canister —
  // the exact party the player is supposed to distrust — and all it does is
  // recompute a SHA-256 (docs/FINDING-02, item 4).
  //
  // Now the verification runs HERE, in the player's browser, from
  // `$lib/shuffle-verify.js` (a port of docs/SHUFFLE-SPEC.md that imports
  // nothing and calls nothing). It re-derives the whole 52-card deck from the
  // revealed seed and shows the player their OWN two cards and the board coming
  // out of it at the positions the dealing rule predicts.
  //
  // The canister's `verify_shuffle` is still called, but only inside "Show the
  // work", explicitly labelled as proving nothing. Its answer is never allowed
  // to influence the verdict.

  import Card from './Card.svelte';
  import logger from '$lib/logger.js';
  import { verifyHandLocally, toCandidCard } from '$lib/shuffle-verify.js';

  const { proof, handNumber, tableActor } = $props();

  let report = $state(null);          // the result of the LOCAL verification
  let observed = $state(null);        // what the table showed us, to be checked
  let phase = $state('idle');         // idle | working | done | failed
  let failure = $state(null);
  let copied = $state(null);
  let showWork = $state(false);
  let showLimits = $state(false);
  let houseEcho = $state(null);       // the canister's own answer; decorative only

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
   * on it succeeding. An unreachable replica makes the agent retry with backoff
   * for tens of seconds, and a fairness panel that sits on a spinner while its
   * own arithmetic is already possible would be telling the player the opposite
   * of the truth about where the check runs.
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
    // Keep whatever the table already told us about this hand if a later read
    // comes back empty. The cards are the CLAIM being checked, not the check, so
    // reusing an earlier reading is honest -- and it means the verification below
    // still runs, and can be seen to run, when the network is unavailable.
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

    askTheHouse(); // deliberately last, deliberately not awaited into the verdict
  }

  /**
   * Asks the table canister its own `verify_shuffle`. This proves NOTHING —
   * it is the accused re-hashing its own evidence — and is shown only so the
   * player can see the difference between that and the check above.
   */
  async function askTheHouse() {
    if (!tableActor || !proof?.seed_hash || !revealedSeed) return;
    houseEcho = { state: 'asking', value: null };
    try {
      const answer = await withDeadline(
        tableActor.verify_shuffle(proof.seed_hash, revealedSeed),
        8000,
        'timeout',
      );
      if (answer === 'timeout') {
        houseEcho = { state: 'unreachable', value: null };
        return;
      }
      houseEcho = { state: 'answered', value: answer === true };
    } catch (e) {
      logger.debug('ShuffleProof: canister echo unavailable', e);
      houseEcho = { state: 'unreachable', value: null };
    }
  }

  // Deliberately a plain variable, NOT $state: the effect must not take a
  // dependency on it, or writing `phase`/`report` from runVerification() would
  // retrigger the effect and re-verify forever.
  let verifiedKey = '';

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

  async function copyToClipboard(text, label) {
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
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

  /** Deck indices worth colouring in the "show the work" deck grid. */
  const marks = $derived.by(() => {
    const map = new Map();
    const layout = report?.layout;
    if (!layout) return map;
    layout.positions.holes.forEach(([a, b], k) => {
      const own = k === layout.dealIndex;
      map.set(a, own ? 'mine' : 'other');
      map.set(b, own ? 'mine' : 'other');
    });
    map.set(layout.positions.burnBeforeFlop, 'burn');
    map.set(layout.positions.burnBeforeTurn, 'burn');
    map.set(layout.positions.burnBeforeRiver, 'burn');
    (layout.boardPositions || []).forEach((p) => map.set(p, 'board'));
    return map;
  });
</script>

<div class="shuffle-proof">
  <div class="proof-header">
    <div class="header-left">
      <div class="shield-icon" class:proved={phase === 'done' && report?.ok}>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          <path d="M9 12l2 2 4-4"/>
        </svg>
      </div>
      <div>
        <h3>Provably Fair</h3>
        <span class="subtitle">
          {#if phase === 'done' && report?.ok}
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
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>
      </svg>
      <p>The fairness proof appears as soon as cards are dealt.</p>
    </div>
  {:else}
    <!-- STEP 1 — the commitment, published before any card existed -->
    <ol class="ladder">
      <li class="rung done">
        <div class="rung-mark">1</div>
        <div class="rung-body">
          <h4>Before the deal, the table locked in a hash</h4>
          <p class="rung-note">
            The table says it published this at {formatTimestamp(proof.timestamp)}. That timestamp is the
            canister's own word and is the one thing on this panel you have to take on trust; everything
            below is checked here. Once the hash is out, the deck behind it cannot be changed.
          </p>
          <div class="proof-item">
            <span class="label">SHA-256 commitment</span>
            <div class="hash-row">
              <code class="hash" title={proof.seed_hash}>{proof.seed_hash}</code>
              <button class="copy-btn" onclick={() => copyToClipboard(proof.seed_hash, 'hash')}>
                {copied === 'hash' ? 'Copied' : 'Copy'}
              </button>
            </div>
          </div>
        </div>
      </li>

      <!-- STEP 2 — the reveal -->
      <li class="rung" class:done={!!revealedSeed}>
        <div class="rung-mark">2</div>
        <div class="rung-body">
          <h4>After the hand, the table revealed the seed</h4>
          {#if revealedSeed}
            <p class="rung-note">The 32 bytes the deck was shuffled from. Yours to keep and re-check anywhere.</p>
            <div class="proof-item">
              <span class="label">Revealed seed</span>
              <div class="hash-row">
                <code class="hash revealed" title={revealedSeed}>{revealedSeed}</code>
                <button class="copy-btn" onclick={() => copyToClipboard(revealedSeed, 'seed')}>
                  {copied === 'seed' ? 'Copied' : 'Copy'}
                </button>
              </div>
            </div>
          {:else}
            <div class="pending-box">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>
              </svg>
              <span>Sealed until this hand ends. Revealing it early would show everyone the deck.</span>
            </div>
          {/if}
        </div>
      </li>

      {#if revealedSeed}
        <!-- STEP 3 — the browser hashes it -->
        <li class="rung" class:done={report?.commitment?.match} class:bad={report && !report.commitment.match}>
          <div class="rung-mark">3</div>
          <div class="rung-body">
            <h4>Your browser hashed that seed</h4>
            <p class="rung-note">
              Computed in this tab with the browser's own WebCrypto SHA-256. If it equals the hash from
              step 1, the seed is the one that was committed to.
            </p>
            {#if phase === 'working' && !report}
              <div class="working"><span class="spinner"></span> hashing…</div>
            {:else if report}
              <div class="compare">
                <div class="compare-row">
                  <span class="compare-label">computed here</span>
                  <code class="hash small">{report.commitment.computed}</code>
                </div>
                <div class="compare-row">
                  <span class="compare-label">committed before</span>
                  <code class="hash small">{report.commitment.committed}</code>
                </div>
                <div class="verdict" class:good={report.commitment.match} class:bad={!report.commitment.match}>
                  {#if report.commitment.match}
                    Identical. The revealed seed is the committed seed.
                  {:else}
                    NOT identical. The table revealed a seed it did not commit to.
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </li>

        <!-- STEP 4 — the cards, re-derived locally -->
        <li class="rung" class:done={report?.ok} class:bad={phase === 'failed'}>
          <div class="rung-mark">4</div>
          <div class="rung-body">
            <h4>Your browser rebuilt the deck and found your cards</h4>
            <p class="rung-note">
              Same seed, same 51 shuffle steps, no network. Cards on the left are what this tab derived;
              cards on the right are what you were dealt.
            </p>

            {#if phase === 'working' && !report}
              <div class="working"><span class="spinner"></span> re-deriving 52 cards…</div>
            {:else if failure && !report?.layout}
              <div class="verdict bad">{failure}</div>
            {:else if report?.layout}
              {#if holeCheck}
                <div class="rederive" class:good={holeCheck.match} class:bad={!holeCheck.match}>
                  <div class="rederive-head">
                    <span class="rederive-title">Your hole cards</span>
                    <span class="rederive-pos">deck positions {holeCheck.positions[0]} &amp; {holeCheck.positions[1]}</span>
                  </div>
                  <div class="rederive-row">
                    <div class="side">
                      <span class="side-label">derived here</span>
                      <div class="cards">
                        <Card card={toCandidCard(holeCheck.derivedCards[0])} small={true} />
                        <Card card={toCandidCard(holeCheck.derivedCards[1])} small={true} />
                      </div>
                    </div>
                    <div class="equals" class:good={holeCheck.match}>{holeCheck.match ? '=' : '≠'}</div>
                    <div class="side">
                      <span class="side-label">dealt to you</span>
                      <div class="cards">
                        <Card card={observed?.myCards?.[0]} small={true} />
                        <Card card={observed?.myCards?.[1]} small={true} />
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
                    <span class="rederive-pos">
                      deck positions {boardChecks.map((c) => c.positions[0]).join(', ')}
                    </span>
                  </div>
                  <div class="board-grid">
                    {#each boardChecks as check}
                      <div class="board-cell" class:good={check.match} class:bad={!check.match}>
                        <span class="cell-label">{check.label}</span>
                        <Card card={toCandidCard(check.derivedCards[0])} small={true} />
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
                  dealing rule puts it. Change any one of them and the hash in step 3 stops matching.
                {:else if report.cardsChecked > 0}
                  <strong>{report.cardsMatched} of {report.cardsChecked} cards</strong> matched.
                  {#if !report.commitment.match}
                    The commitment in step 3 failed, so this deal is not accounted for.
                  {/if}
                {:else}
                  The deck was rebuilt locally, but no dealt card was available to check it against.
                {/if}
              </div>

              <p class="layout-note">
                {#if report.playersDetermined}
                  Laid out for <strong>{report.layout.players} players dealt in</strong>{#if holeCheck}, with you
                  {ordinal(report.layout.dealIndex)} in ascending seat order{/if}. That is the only table size
                  consistent with the cards you saw.
                  {#if report.playersAgree === false}
                    The hand log implies {report.expectedPlayers}; the deal itself says
                    {report.layout.players}, and the deal is what was checked.
                  {/if}
                {:else if holeCheck}
                  Your two cards sit at the front of the deck, so their position does not depend on how many
                  players were dealt in. Without a board, the table size cannot be pinned down from the cards
                  alone, and this panel does not guess it.
                {:else}
                  The number of players dealt in could not be pinned down from the cards available.
                {/if}
                {#if report.rejections > 0}
                  {report.rejections} draw(s) were rejected and re-hashed, per the sampling rule.
                {/if}
              </p>
            {/if}
          </div>
        </li>
      {/if}
    </ol>

    {#if phase === 'done' && report?.ok}
      <div class="headline good">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <polyline points="20,6 9,17 4,12"/>
        </svg>
        <span>Verified on your machine. This page asked no one's permission to say so.</span>
      </div>
    {:else if phase === 'failed' || (report && !report.ok && report.cardsChecked > 0)}
      <div class="headline bad">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
        </svg>
        <span>This hand did not verify. Do not take our word for anything below.</span>
      </div>
    {/if}

    {#if revealedSeed}
      <button class="rerun-btn" onclick={runVerification} disabled={phase === 'working'}>
        {phase === 'working' ? 'Verifying…' : 'Run the check again'}
      </button>
    {/if}

    <!-- ------------------------------------------------------------- -->
    <!-- Show the work                                                  -->
    <!-- ------------------------------------------------------------- -->
    {#if report?.layout}
      <div class="section">
        <button class="section-toggle" onclick={() => showWork = !showWork}>
          <span>Show the work</span>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:rotated={showWork}>
            <polyline points="6,9 12,15 18,9"/>
          </svg>
        </button>

        {#if showWork}
          <div class="work">
            <h5>The whole deck, as this browser built it</h5>
            <p class="work-note">
              52 cards from one seed. Gold is yours, teal is the board, grey struck-through cards were
              burned, and faint cards went to other seats and were never shown to you.
            </p>
            <div class="deck-grid">
              {#each report.deckCodes as code, index}
                <span class="slot {marks.get(index) || 'unused'}">
                  <span class="slot-index">{index}</span>
                  <span class="slot-code">{code}</span>
                </span>
              {/each}
            </div>

            <h5>The first three shuffle steps, in full</h5>
            <p class="work-note">
              Each step hashes the running chain with a counter byte, reads the first 8 bytes as a
              little-endian 64-bit number, reduces it modulo the remaining deck size, and swaps.
              The whole algorithm is 51 repeats of this; the spec is in docs/SHUFFLE-SPEC.md.
            </p>
            <div class="steps">
              {#each report.trace as step}
                <div class="step">
                  <div class="step-head">
                    <span class="step-n">step {step.step}</span>
                    <span>SHA-256(chain ‖ 0x{step.counterByte.toString(16).padStart(2, '0')})</span>
                  </div>
                  <code class="chain">{step.chainHex}</code>
                  <div class="step-math">
                    <span>first 8 bytes <code>{step.drawBytesHex}</code></span>
                    <span>→ {step.draw}</span>
                    <span>mod {step.n} = <strong>{step.j}</strong></span>
                    <span class="swap">swap deck[{step.i}] ↔ deck[{step.j}]</span>
                  </div>
                </div>
              {/each}
            </div>

            <h5>What the table's own verifier says</h5>
            <div class="house-echo">
              <p class="work-note">
                The table canister exposes a <code>verify_shuffle</code> query. It recomputes the same
                SHA-256 and answers true. <strong>That answer is worth nothing</strong>: it is the house
                checking its own homework, on the house's machine, and it never touches a card. It is shown
                here only so the difference is visible. Nothing above depends on it.
              </p>
              <div class="echo-row">
                <span class="echo-label">canister verify_shuffle</span>
                {#if houseEcho?.state === 'answered'}
                  <span class="echo-value">{String(houseEcho.value)} — proves only that the canister can hash</span>
                {:else if houseEcho?.state === 'unreachable'}
                  <span class="echo-value muted">unreachable — and the check above still passed without it</span>
                {:else if houseEcho?.state === 'asking'}
                  <span class="echo-value muted">asking…</span>
                {:else}
                  <span class="echo-value muted">not asked</span>
                {/if}
              </div>
            </div>
          </div>
        {/if}
      </div>
    {/if}

    <!-- ------------------------------------------------------------- -->
    <!-- Do it somewhere else                                           -->
    <!-- ------------------------------------------------------------- -->
    {#if revealedSeed}
      <div class="manual-verify">
        <h4>Don't trust this page either</h4>
        <p class="manual-lead">
          Everything above ran in your browser, but it is still our JavaScript. Take the seed somewhere
          we do not control:
        </p>

        <div class="method">
          <span class="method-name">1. Check the commitment with any SHA-256 tool</span>
          <div class="command-row">
            <code class="command">echo -n "{revealedSeed}" | xxd -r -p | shasum -a 256</code>
            <button class="copy-btn small" onclick={() => copyToClipboard(`echo -n "${revealedSeed}" | xxd -r -p | shasum -a 256`, 'cmd')}>
              {copied === 'cmd' ? '✓' : 'Copy'}
            </button>
          </div>
          <a href="https://emn178.github.io/online-tools/sha256.html" target="_blank" rel="noopener">
            Online SHA-256 calculator
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
              <polyline points="15,3 21,3 21,9"/><line x1="10" y1="14" x2="21" y2="3"/>
            </svg>
          </a>
          <div class="method-hint-box">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="8"/>
            </svg>
            <span>Set the tool's <strong>input encoding to “Hex”</strong> — the seed is bytes, not text.</span>
          </div>
          <div class="expected-result">
            <span class="expected-label">It must print</span>
            <code class="expected-hash">{proof.seed_hash}</code>
          </div>
        </div>

        <div class="method">
          <span class="method-name">2. Re-derive the cards with a verifier that is not ours</span>
          <p class="method-lead">
            Two independent implementations, written from the specification and sharing no code with the
            engine or with this page, ship in the repository. Both print the whole deck and lay out the hand:
          </p>
          <div class="command-row">
            <code class="command">node src/poker_core/tests/verify/verify_shuffle.mjs {revealedSeed} --players {report?.layout?.players ?? 2} --seed-hash {proof.seed_hash}</code>
            <button
              class="copy-btn small"
              onclick={() => copyToClipboard(
                `node src/poker_core/tests/verify/verify_shuffle.mjs ${revealedSeed} --players ${report?.layout?.players ?? 2} --seed-hash ${proof.seed_hash}`,
                'node',
              )}
            >{copied === 'node' ? '✓' : 'Copy'}</button>
          </div>
          <div class="command-row">
            <code class="command">python3 src/poker_core/tests/verify/verify_shuffle.py {revealedSeed} --players {report?.layout?.players ?? 2} --seed-hash {proof.seed_hash}</code>
            <button
              class="copy-btn small"
              onclick={() => copyToClipboard(
                `python3 src/poker_core/tests/verify/verify_shuffle.py ${revealedSeed} --players ${report?.layout?.players ?? 2} --seed-hash ${proof.seed_hash}`,
                'py',
              )}
            >{copied === 'py' ? '✓' : 'Copy'}</button>
          </div>
          <p class="method-lead">
            Or write your own from <code>docs/SHUFFLE-SPEC.md</code>. If it disagrees with this page,
            that is our bug, and we want to hear about it.
          </p>
        </div>
      </div>
    {/if}

    <!-- ------------------------------------------------------------- -->
    <!-- What this does and does not prove                              -->
    <!-- ------------------------------------------------------------- -->
    <div class="section">
      <button class="section-toggle" onclick={() => showLimits = !showLimits}>
        <span>What this proves, and what it does not</span>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:rotated={showLimits}>
          <polyline points="6,9 12,15 18,9"/>
        </svg>
      </button>

      {#if showLimits}
        <div class="limits">
          <div class="limit proven">
            <span class="limit-tag">Proven</span>
            <p>The cards you were dealt follow from the seed the table committed to. That seed fixes all 52
              positions, and every card you saw this hand sits exactly where it puts one. Checked here, on
              your machine, from the cards on your screen.</p>
          </div>
          <div class="limit proven">
            <span class="limit-tag">Proven</span>
            <p>The deck could not be changed after that commitment existed. Any other card at any other
              position would give a different SHA-256, and the one in step 1 is the one that was published.</p>
          </div>
          <div class="limit not-proven">
            <span class="limit-tag">Not proven</span>
            <p><strong>That the commitment came before the cards.</strong> This page reads
              <code>{formatTimestamp(proof.timestamp)}</code> off the table canister; it did not watch the
              order of events. Everything above stays true even if that clock is wrong.
              <strong>You can witness it yourself:</strong> copy the commitment from step 1 while a hand is
              still running — it is on screen from the moment cards are dealt — and check it against the one
              shown here after the seed is revealed. Then the "before" is something you saw, not something
              we told you.</p>
          </div>
          <div class="limit not-proven">
            <span class="limit-tag">Not proven</span>
            <p>That the seed was unpredictable. It comes from the Internet Computer's <code>raw_rand</code>,
              so you are trusting the subnet's randomness, not arithmetic.</p>
          </div>
          <div class="limit not-proven">
            <span class="limit-tag">Not proven</span>
            <p>That the money went to the right player. Settlement is a separate question and this panel says
              nothing about it.</p>
          </div>
          <div class="limit not-proven">
            <span class="limit-tag">Not proven</span>
            <p>That the rest of the engine is correct. Verifying a shuffle is not an audit. This is unaudited
              alpha software with known defects, listed in <code>docs/DEFECTS.md</code>.</p>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .shuffle-proof {
    background: linear-gradient(145deg, rgba(20, 20, 35, 0.95), rgba(10, 10, 20, 0.98));
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 24px;
    max-width: 520px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
  }

  .proof-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 20px;
  }

  .header-left { display: flex; gap: 12px; align-items: flex-start; }

  .shield-icon {
    padding: 10px;
    border-radius: 12px;
    color: #6b7280;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .shield-icon.proved {
    color: #2ecc71;
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.18), rgba(46, 204, 113, 0.05));
    border-color: rgba(46, 204, 113, 0.35);
  }

  .shield-icon svg { display: block; }

  .proof-header h3 { color: #fff; margin: 0; font-size: 18px; font-weight: 700; }
  .subtitle { color: #8b93a7; font-size: 12px; }

  .hand-number {
    color: #8b93a7;
    font-size: 12px;
    background: rgba(255, 255, 255, 0.05);
    padding: 4px 10px;
    border-radius: 6px;
    white-space: nowrap;
  }

  /* The four-rung ladder */
  .ladder { list-style: none; margin: 0; padding: 0; }

  .rung {
    display: grid;
    grid-template-columns: 28px 1fr;
    gap: 12px;
    padding-bottom: 18px;
    position: relative;
  }

  .rung::before {
    content: '';
    position: absolute;
    left: 13px;
    top: 30px;
    bottom: 0;
    width: 2px;
    background: rgba(255, 255, 255, 0.07);
  }

  .rung:last-child::before { display: none; }

  .rung-mark {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 700;
    color: #6b7280;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    z-index: 1;
  }

  .rung.done .rung-mark {
    color: #0b1220;
    background: #2ecc71;
    border-color: #2ecc71;
  }

  .rung.bad .rung-mark {
    color: #fff;
    background: #e74c3c;
    border-color: #e74c3c;
  }

  .rung-body { min-width: 0; }

  .rung-body h4 {
    margin: 4px 0 4px;
    font-size: 14px;
    font-weight: 650;
    color: #fff;
  }

  .rung-note {
    margin: 0 0 10px;
    font-size: 12px;
    line-height: 1.55;
    color: #8b93a7;
  }

  .proof-item { display: flex; flex-direction: column; gap: 6px; }

  .proof-item .label {
    color: #6b7280;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }

  .hash {
    background: rgba(0, 0, 0, 0.4);
    padding: 9px 12px;
    border-radius: 8px;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 10.5px;
    line-height: 1.5;
    color: #4ecdc4;
    border: 1px solid rgba(78, 205, 196, 0.2);
    word-break: break-all;
  }

  .hash.revealed { color: #f1c40f; border-color: rgba(241, 196, 15, 0.22); }
  .hash.small { font-size: 10px; padding: 7px 10px; }

  .hash-row { display: flex; gap: 8px; align-items: stretch; }
  .hash-row .hash { flex: 1; min-width: 0; }

  .copy-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #8b93a7;
    padding: 8px 12px;
    border-radius: 8px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
    align-self: flex-start;
  }

  .copy-btn:hover { background: rgba(255, 255, 255, 0.15); color: #fff; }
  .copy-btn.small { padding: 6px 10px; font-size: 10px; }

  .pending-box {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 14px;
    background: rgba(241, 196, 15, 0.08);
    border: 1px solid rgba(241, 196, 15, 0.2);
    border-radius: 8px;
    color: #f1c40f;
    font-size: 12px;
    line-height: 1.45;
  }

  .working {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #8b93a7;
    font-size: 12px;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #4ecdc4;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .compare { display: flex; flex-direction: column; gap: 6px; }

  .compare-row { display: flex; flex-direction: column; gap: 3px; }

  .compare-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: #6b7280;
  }

  .verdict {
    margin-top: 4px;
    padding: 9px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.45;
  }

  .verdict.good { background: rgba(46, 204, 113, 0.1); color: #2ecc71; border: 1px solid rgba(46, 204, 113, 0.25); }
  .verdict.bad { background: rgba(231, 76, 60, 0.1); color: #e74c3c; border: 1px solid rgba(231, 76, 60, 0.3); }

  /* Re-derived cards, side by side with the dealt ones */
  .rederive {
    margin-bottom: 12px;
    padding: 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.28);
    border: 1px solid rgba(255, 255, 255, 0.07);
  }

  .rederive.good { border-color: rgba(46, 204, 113, 0.3); background: rgba(46, 204, 113, 0.06); }
  .rederive.bad { border-color: rgba(231, 76, 60, 0.35); background: rgba(231, 76, 60, 0.06); }

  .rederive-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 10px;
  }

  .rederive-title { font-size: 12px; font-weight: 650; color: #fff; }
  .rederive-pos { font-size: 10px; color: #6b7280; font-family: 'Monaco', 'Consolas', monospace; }

  .rederive-row { display: flex; align-items: center; gap: 12px; }

  .side { display: flex; flex-direction: column; gap: 6px; }

  .side-label {
    font-size: 9.5px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    color: #6b7280;
  }

  .cards { display: flex; gap: 5px; }

  .equals { font-size: 20px; font-weight: 700; color: #6b7280; }
  .equals.good { color: #2ecc71; }

  .board-grid { display: flex; flex-wrap: wrap; gap: 8px; }

  .board-cell {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 6px;
    border-radius: 8px;
    border: 1px solid transparent;
  }

  .board-cell.good { border-color: rgba(46, 204, 113, 0.25); }
  .board-cell.bad { border-color: rgba(231, 76, 60, 0.4); }

  .cell-label { font-size: 9px; text-transform: uppercase; letter-spacing: 0.5px; color: #6b7280; }
  .cell-mark { font-size: 11px; color: #2ecc71; font-weight: 700; }
  .board-cell.bad .cell-mark { color: #e74c3c; }

  .tally {
    padding: 11px 13px;
    border-radius: 9px;
    font-size: 12px;
    line-height: 1.6;
    color: #c4cbd8;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.07);
  }

  .tally.good { background: rgba(46, 204, 113, 0.08); border-color: rgba(46, 204, 113, 0.25); }
  .tally.bad { background: rgba(231, 76, 60, 0.08); border-color: rgba(231, 76, 60, 0.3); }
  .tally strong { color: #fff; }

  .layout-note {
    margin: 8px 0 0;
    font-size: 11px;
    line-height: 1.6;
    color: #7c8497;
  }

  .layout-note strong { color: #a9b2c4; }

  .soft-note {
    padding: 10px 12px;
    margin-bottom: 12px;
    border-radius: 8px;
    font-size: 11.5px;
    line-height: 1.5;
    color: #8b93a7;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.07);
  }

  .headline {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 10px;
    font-size: 12.5px;
    font-weight: 600;
    line-height: 1.45;
    margin-bottom: 12px;
  }

  .headline.good { background: rgba(46, 204, 113, 0.12); border: 1px solid rgba(46, 204, 113, 0.3); color: #2ecc71; }
  .headline.bad { background: rgba(231, 76, 60, 0.12); border: 1px solid rgba(231, 76, 60, 0.35); color: #e74c3c; }
  .headline svg { flex-shrink: 0; }

  .rerun-btn {
    width: 100%;
    padding: 10px 16px;
    border-radius: 9px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: #a9b2c4;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .rerun-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.09); color: #fff; }
  .rerun-btn:disabled { opacity: 0.6; cursor: wait; }

  /* Collapsible sections */
  .section { margin-top: 18px; border-top: 1px solid rgba(255, 255, 255, 0.08); padding-top: 16px; }

  .section-toggle {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 11px 14px;
    color: #a9b2c4;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .section-toggle:hover { background: rgba(255, 255, 255, 0.06); color: #fff; }
  .section-toggle svg { transition: transform 0.3s; flex-shrink: 0; }
  .section-toggle svg.rotated { transform: rotate(180deg); }

  .work { margin-top: 14px; }

  .work h5 {
    margin: 16px 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.7px;
    color: #8b93a7;
  }

  .work h5:first-child { margin-top: 0; }

  .work-note { margin: 0 0 10px; font-size: 11px; line-height: 1.6; color: #7c8497; }
  .work-note code { color: #4ecdc4; font-size: 10.5px; }
  .work-note strong { color: #c4cbd8; }

  .deck-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(46px, 1fr));
    gap: 3px;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 3px 2px;
    border-radius: 5px;
    font-family: 'Monaco', 'Consolas', monospace;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.03);
  }

  .slot-index { font-size: 8px; color: #5a6172; }
  .slot-code { font-size: 11px; font-weight: 700; color: #7c8497; }

  .slot.unused, .slot.other { opacity: 0.45; }

  .slot.mine {
    background: rgba(241, 196, 15, 0.16);
    border-color: rgba(241, 196, 15, 0.5);
  }
  .slot.mine .slot-code { color: #f1c40f; }

  .slot.board {
    background: rgba(78, 205, 196, 0.14);
    border-color: rgba(78, 205, 196, 0.45);
  }
  .slot.board .slot-code { color: #4ecdc4; }

  .slot.burn { opacity: 0.6; }
  .slot.burn .slot-code { text-decoration: line-through; color: #5a6172; }

  .steps { display: flex; flex-direction: column; gap: 8px; }

  .step {
    padding: 9px 11px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .step-head {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
    font-size: 10.5px;
    color: #8b93a7;
    margin-bottom: 5px;
  }

  .step-n { color: #4ecdc4; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; }

  .chain {
    display: block;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 9.5px;
    line-height: 1.5;
    color: #6b7280;
    word-break: break-all;
    margin-bottom: 5px;
  }

  .step-math {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    font-size: 10px;
    color: #8b93a7;
    align-items: baseline;
  }

  .step-math code { color: #f1c40f; font-size: 9.5px; }
  .step-math strong { color: #fff; }
  .step-math .swap { color: #4ecdc4; }

  .house-echo {
    padding: 11px 13px;
    border-radius: 9px;
    background: rgba(231, 76, 60, 0.05);
    border: 1px dashed rgba(231, 76, 60, 0.3);
  }

  .echo-row { display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: baseline; }

  .echo-label {
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 10px;
    color: #6b7280;
  }

  .echo-value { font-size: 11px; color: #c4cbd8; }
  .echo-value.muted { color: #6b7280; font-style: italic; }

  /* Verify elsewhere */
  .manual-verify { margin-top: 18px; padding-top: 16px; border-top: 1px solid rgba(255, 255, 255, 0.08); }

  .manual-verify h4 {
    color: #fff;
    font-size: 13px;
    font-weight: 650;
    margin: 0 0 6px;
  }

  .manual-lead { color: #8b93a7; font-size: 11.5px; line-height: 1.6; margin: 0 0 14px; }

  .method { display: flex; flex-direction: column; gap: 7px; margin-bottom: 16px; }

  .method-name { color: #a9b2c4; font-size: 11.5px; font-weight: 600; }
  .method-lead { color: #7c8497; font-size: 11px; line-height: 1.6; margin: 0; }
  .method-lead code { color: #4ecdc4; }

  .command-row { display: flex; gap: 8px; align-items: stretch; }

  .command {
    flex: 1;
    min-width: 0;
    background: rgba(0, 0, 0, 0.4);
    padding: 9px 11px;
    border-radius: 8px;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 9.5px;
    line-height: 1.5;
    color: #f39c12;
    word-break: break-all;
    border: 1px solid rgba(243, 156, 18, 0.2);
  }

  .method a {
    color: #3498db;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    align-self: flex-start;
  }

  .method a:hover { text-decoration: underline; }

  .method-hint-box {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 11px;
    background: rgba(241, 196, 15, 0.08);
    border: 1px solid rgba(241, 196, 15, 0.25);
    border-radius: 6px;
    color: #f1c40f;
    font-size: 11px;
    line-height: 1.45;
  }

  .method-hint-box svg { flex-shrink: 0; margin-top: 1px; }
  .method-hint-box strong { color: #f5d547; }

  .expected-result {
    padding: 10px 12px;
    background: rgba(78, 205, 196, 0.08);
    border: 1px solid rgba(78, 205, 196, 0.2);
    border-radius: 8px;
  }

  .expected-label { color: #6b7280; font-size: 10px; display: block; margin-bottom: 5px; text-transform: uppercase; letter-spacing: 0.6px; }

  .expected-hash {
    color: #4ecdc4;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 9.5px;
    word-break: break-all;
    display: block;
    line-height: 1.5;
  }

  /* Proven / not proven */
  .limits { margin-top: 14px; display: flex; flex-direction: column; gap: 8px; }

  .limit {
    display: grid;
    grid-template-columns: 84px 1fr;
    gap: 10px;
    align-items: start;
    padding: 10px 12px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .limit p { margin: 0; font-size: 11.5px; line-height: 1.6; color: #8b93a7; }
  .limit p code { color: #4ecdc4; font-size: 10.5px; }

  .limit-tag {
    font-size: 9.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.6px;
    padding: 3px 7px;
    border-radius: 5px;
    text-align: center;
  }

  .limit.proven .limit-tag { background: rgba(46, 204, 113, 0.16); color: #2ecc71; }
  .limit.not-proven .limit-tag { background: rgba(241, 196, 15, 0.14); color: #f1c40f; }

  .no-proof { text-align: center; padding: 30px; color: #6b7280; }
  .no-proof svg { opacity: 0.4; margin-bottom: 12px; }
  .no-proof p { margin: 0; font-size: 13px; }

  @media (max-width: 500px) {
    .shuffle-proof { padding: 18px; }
    .rederive-row { flex-direction: column; align-items: flex-start; gap: 8px; }
    .equals { align-self: center; transform: rotate(90deg); }
    .limit { grid-template-columns: 1fr; }
    .limit-tag { justify-self: start; }
  }
</style>
