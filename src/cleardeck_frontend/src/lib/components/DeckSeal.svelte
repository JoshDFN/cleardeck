<script>
  /**
   * The deck on the table: a small stack of card backs carrying the seal
   * glyph of this hand's commitment. It reads SEALED while the hand runs,
   * REVEALED when the seed is published, and CHECKED once THIS browser has
   * hashed the revealed seed and found the commitment (lib/deck-seal.js, the
   * same state machine the action log's chip uses). A click opens the proof.
   *
   * Two corners, one object: the near rail's right corner in landscape
   * (where a shuffler sits on a broadcast table); the far rail's left corner
   * in portrait, where the phone's stadium is empty (the near corner is the
   * hero's award chip's spot on a phone).
   *
   * Nothing money-shaped and no digits: the fingerprint is the hover title.
   * The layers are plain divs, not <Card>, so no card scraper counts them.
   * The button RELEASES FOCUS after it opens the proof: a focused button
   * outside the dock mutes the action hotkeys ($lib/hotkeys.js focusKindOf),
   * and this one is pressed mid-hand.
   *
   * Harness: `.deck-seal[data-seal=sealed|revealed|checked|mismatch]` with the
   * state word in `.deck-state`.
   */
  import HashSeal from './HashSeal.svelte';
  import { hashFingerprint } from '$lib/hash-seal.js';
  import { SEAL_HINT, SEAL_WORD, checkSeal, revealedSeedOf, sealKey, sealStateFor } from '$lib/deck-seal.js';

  const { shuffleProof = null, onShowProof = null, portrait = false } = $props();

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

  const tone = $derived(state === 'mismatch' ? 'muted' : 'accent');

  function open(event) {
    onShowProof?.();
    // Let go of the keyboard: the proof panel is the thing to read now, and a
    // held focus here would silently mute F / C / R / A on the dock.
    event.currentTarget?.blur?.();
  }
</script>

{#if state}
  <button
    type="button"
    class="deck-seal state-{state}"
    class:portrait
    data-seal={state}
    onclick={open}
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
  /* the near rail's RIGHT corner: outside every seat ring, under no figure,
     and clear of the log, which is a column at the LEFT of the stage */
  .deck-seal {
    --deck-w: max(calc(var(--fw) * 0.038), 30px);
    position: absolute;
    right: 1.5%;
    bottom: 2%;
    z-index: 3;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--cd-space-1);
    min-width: var(--cd-touch-min);
    min-height: var(--cd-touch-min);
    padding: var(--cd-space-1) calc(var(--cd-space-1) * 1.5);
    background: transparent;
    border: 0;
    border-radius: var(--cd-radius-chip);
    color: var(--cd-ink-felt);
    cursor: pointer;
    font: inherit;
    transition: transform var(--cd-fast) var(--cd-ease);
  }

  /* the phone: the far rail's LEFT corner, above every pod of the stadium */
  .deck-seal.portrait {
    right: auto;
    bottom: auto;
    left: 1.5%;
    top: 1%;
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

  /* the stack's two lower cards peek out by a tenth of the deck's width */
  .deck-layer.l3 { transform: translate(calc(var(--deck-w) * 0.1), calc(var(--deck-w) * 0.13)); }
  .deck-layer.l2 { transform: translate(calc(var(--deck-w) * 0.05), calc(var(--deck-w) * 0.065)); }

  .deck-glyph {
    display: inline-flex;
    border-radius: calc(var(--deck-w) * 0.1);
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
