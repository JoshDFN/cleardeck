<script>
  /**
   * A hash as a seal: a 5 x 5 mirrored glyph read off the hash's leading bits
   * (lib/hash-seal.js), so two commitments can be compared by eye the way two
   * SSH randomart blocks are. Drawing only; the check is elsewhere.
   *
   * `tone`: accent (a commitment), money (a revealed seed), muted (nothing yet).
   * The glyph carries `aria-label` with the fingerprint so a reader hears it.
   */
  import { hashFingerprint, sealCells, SEAL_SIZE } from '$lib/hash-seal.js';

  const { hash = '', size = 28, tone = 'accent', label = 'commitment' } = $props();

  const cells = $derived(sealCells(hash));
  const fingerprint = $derived(hashFingerprint(hash));
  const cell = 10;
  const gap = 1.5;
  const box = SEAL_SIZE * cell + (SEAL_SIZE - 1) * gap;
</script>

<svg
  class="seal {tone}"
  width={size}
  height={size}
  style:--seal-size="{size}px"
  viewBox="0 0 {box} {box}"
  role="img"
  aria-label="{label} {fingerprint || 'not yet published'}"
>
  {#each cells as on, at}
    <rect
      x={(at % SEAL_SIZE) * (cell + gap)}
      y={Math.floor(at / SEAL_SIZE) * (cell + gap)}
      width={cell}
      height={cell}
      rx="2"
      class:on
    />
  {/each}
</svg>

<style>
  .seal {
    display: block;
    flex: 0 0 auto;
    border-radius: 22%;
    background: var(--cd-bg-deep);
    /* Padding from the seal's own size: a percentage would read the container
       and swallow the glyph inside a wide flex row (measured, phase 6 run 1). */
    padding: calc(var(--seal-size, 28px) * 0.1);
    box-sizing: border-box;
    box-shadow: inset 0 0 0 1px var(--cd-line-soft);
  }

  rect { fill: var(--cd-surface-3); transition: fill var(--cd-flip) var(--cd-ease); }
  .accent rect.on { fill: var(--cd-accent); }
  .money rect.on { fill: var(--cd-money); }
  .muted rect.on { fill: var(--cd-ink-2); }
</style>
