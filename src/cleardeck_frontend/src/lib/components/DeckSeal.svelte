<script>
  /**
   * The deck on the table: a small stack of card backs at the near rail's
   * corner (where a shuffler sits on a broadcast table), carrying the seal
   * glyph of this hand's commitment. It reads SEALED while the hand runs,
   * REVEALED when the seed is published, and CHECKED once THIS browser has
   * hashed the revealed seed and found the commitment (lib/deck-seal.js, the
   * same state machine the action log's chip uses). A click opens the proof.
   *
   * Nothing money-shaped and no digits: the fingerprint is the hover title.
   * The layers are plain divs, not <Card>, so no card scraper counts them.
   *
   * Harness: `.deck-seal[data-seal=sealed|revealed|checked|mismatch]`.
   */
  import HashSeal from './HashSeal.svelte';
  import { hashFingerprint } from '$lib/hash-seal.js';
  import { SEAL_HINT, SEAL_WORD, checkSeal, revealedSeedOf, sealKey, sealStateFor } from '$lib/deck-seal.js';

  const { shuffleProof = null, onShowProof = null } = $props();

  let checkedKey = $state(null);
  let checkResult = $state(null);

  const seedHash = $derived(shuffleProof?.seed_hash || null);
  const revealedSeed = $derived(revealedSeedOf(shuffleProof));
  const state = $derived(sealStateFor({ seedHash, revealedSeed, checkedKey, checkResult }));

  $effect(() => {
    const hash = seedHash;
    const seed = revealedSeed;
    if (!hash || !seed) return;
    const key = sealKey(hash, seed);
    if (checkedKey === key) return;
    let cancelled = false;
    checkSeal(hash, seed).then((ok) => {
      if (cancelled) return;
      checkResult = ok;
      checkedKey = key;
    });
    return () => { cancelled = true; };
  });

  const tone = $derived(state === 'checked' ? 'accent' : state === 'mismatch' ? 'muted' : 'accent');
</script>

{#if state}
  <button
    type="button"
    class="deck-seal state-{state}"
    data-seal={state}
    onclick={() => onShowProof?.()}
    title="{SEAL_HINT[state]} Commitment {hashFingerprint(seedHash)}."
    aria-label="Deck seal: {SEAL_WORD[state]}. Open the fairness proof"
  >
    <span class="deck" aria-hidden="true">
      <span class="deck-layer l3"></span>
      <span class="deck-layer l2"></span>
      <span class="deck-layer l1">
        <span class="deck-glyph" class:flip={state !== 'sealed'}>
          <HashSeal hash={seedHash} size={18} {tone} />
        </span>
      </span>
    </span>
    <span class="deck-caption">
      <span class="deck-word">Deck</span>
      <span class="deck-state">{SEAL_WORD[state]}</span>
    </span>
  </button>
{/if}

<style>
  .deck-seal {
    /* the near rail's RIGHT corner: outside every seat ring, under no figure,
       and never behind the LOG drawer, which opens at the left */
    --deck-w: max(calc(var(--fw) * 0.038), 30px);
    position: absolute;
    right: 1.5%;
    bottom: 2%;
    z-index: 3;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: var(--cd-touch-min);
    min-height: var(--cd-touch-min);
    padding: 4px 6px;
    background: transparent;
    border: 0;
    border-radius: var(--cd-radius-chip);
    color: var(--cd-ink-felt);
    cursor: pointer;
    font: inherit;
    transition: transform var(--cd-fast) var(--cd-ease);
  }

  .deck-seal:hover { transform: translateY(-1px); }
  .deck-seal:focus-visible { outline: 2px solid var(--cd-accent-line-strong); outline-offset: 2px; }

  .deck {
    position: relative;
    width: var(--deck-w);
    height: calc(var(--deck-w) * 1.4);
  }

  .deck-layer {
    position: absolute;
    inset: 0;
    border-radius: calc(var(--deck-w) * 0.12);
    background:
      repeating-linear-gradient(45deg, var(--cd-card-back-gold-dim) 0 1px, transparent 1px 4px),
      linear-gradient(150deg, var(--cd-card-back-hi), var(--cd-card-back) 55%, var(--cd-card-back-lo));
    box-shadow: inset 0 0 0 1px var(--cd-card-edge), var(--cd-shadow-chip);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .deck-layer.l3 { transform: translate(3px, 4px); }
  .deck-layer.l2 { transform: translate(1.5px, 2px); }

  .deck-glyph {
    display: inline-flex;
    border-radius: 3px;
    transition: transform var(--cd-flip) var(--cd-ease);
  }

  /* the reveal: the glyph turns over, 150 ms, the house card-flip tempo */
  .deck-glyph.flip { animation: seal-flip var(--cd-flip) var(--cd-ease) both; }

  @keyframes seal-flip {
    from { transform: rotateY(90deg); }
    to { transform: rotateY(0deg); }
  }

  .deck-caption {
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1.15;
    white-space: nowrap;
  }

  .deck-word {
    font-size: var(--cd-felt-label);
    text-transform: uppercase;
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-felt);
  }

  .deck-state {
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink-1);
  }

  .state-checked .deck-state { color: var(--cd-accent-hi); }
  .state-mismatch .deck-state { color: var(--cd-danger-hi); }
  .state-sealed .deck-state { color: var(--cd-ink-1); }

  @media (prefers-reduced-motion: reduce) {
    .deck-glyph.flip { animation: none; }
  }
</style>
