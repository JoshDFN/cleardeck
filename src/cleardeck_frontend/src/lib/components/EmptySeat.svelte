<script>
  /**
   * An empty chair: a solid avatar-sized disc ON the rail with a plus, and the
   * call to action beneath it. Never a dashed outline (the audit: no real
   * table draws wireframe seats).
   *
   * Harness contract: `.join-seat` and `.sit-seat` (dom-scrape.mjs counts the
   * open seats and the token allowlist excuses the seat number).
   */
  const {
    seatIndex = 0,
    seatLabel = '',
    onJoin = () => {}
  } = $props();
</script>

<button class="join-seat" onclick={() => onJoin(seatIndex)}>
  <span class="sit-disc" aria-hidden="true">+</span>
  <span class="sit-text">
    <span class="sit-word">Sit</span>
    <span class="sit-seat">{seatLabel}</span>
  </span>
</button>

<style>
  .join-seat {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 5;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2em;
    padding: 0;
    margin: 0;
    border: 0;
    background: none;
    color: var(--cd-ink-2);
    font-family: inherit;
    cursor: pointer;
    transition: color var(--cd-fast) var(--cd-ease);
  }

  /* The whole footprint of a plate is the hit area, as it always was, and
     never under the 44 px touch floor: the nine-seat phone's plates are 34 px
     tall (0.112 fw), so without the floor a Sit tap there was a 41 px target.
     The floor grows the invisible hit area only; the painted disc and label
     keep the felt budget the harness protects. */
  .join-seat::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    width: max(var(--pod-w), var(--cd-touch-min));
    height: max(calc(var(--pod-h) * 1.2), var(--cd-touch-min));
    transform: translate(-50%, -50%);
  }

  .sit-disc {
    width: var(--avatar);
    height: var(--avatar);
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: calc(var(--avatar) * 0.5);
    font-weight: 300;
    line-height: 1;
    color: var(--cd-ink-2);
    background: var(--cd-plate);
    border: 1px solid var(--cd-line-strong);
    box-shadow: var(--cd-shadow-chip);
    transition: box-shadow var(--cd-fast) var(--cd-ease), border-color var(--cd-fast) var(--cd-ease),
                color var(--cd-fast) var(--cd-ease), transform var(--cd-fast) var(--cd-ease);
  }

  .sit-text {
    display: flex;
    align-items: baseline;
    gap: 0.35em;
    margin-top: calc(var(--avatar) * -0.22);
    position: relative;
    z-index: 1;
    padding: 0.08em 0.5em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
    white-space: nowrap;
  }

  .sit-word {
    font-size: var(--cd-felt-small);
    font-weight: var(--cd-weight-display);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--cd-ink-1);
  }

  .sit-seat {
    font-size: var(--cd-felt-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .join-seat:hover .sit-disc {
    border-color: var(--cd-money);
    color: var(--cd-money);
    transform: translateY(-2px);
    box-shadow: var(--cd-shadow-pod);
  }

  .join-seat:hover .sit-word { color: var(--cd-money); }

  @media (max-aspect-ratio: 1/1) {
    .sit-text { padding: 0.05em 0.4em; }
  }
</style>
