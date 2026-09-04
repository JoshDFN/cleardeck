<script>
  /**
   * One seat on the ring: everything that hangs off a seat point.
   *
   * The PARENT (PokerTable.svelte) owns the geometry: it places a zero-size
   * `.seat` on the ellipse and publishes the seat's own vectors as custom
   * properties (--nx/--ny the inward normal, --bx/--by the chip spot, --ax/--ay
   * the award spot, --rdx/--rdy the readout spoke, --cy the card direction) and
   * the table's scale (--fw, --pod-w, --pod-h, --avatar, --ui). This component
   * reads those and draws the seat: hole cards, the bet as a chip stack, the
   * plate (a broadcast lower-third: avatar breaking the left edge, name row,
   * stack row, the action clock as a ring on the avatar), the dealer puck, the
   * equity badge, the winner's award, or the empty chair.
   *
   * THE HARNESS CONTRACT. tools/shots/lib/dom-scrape.mjs reads a seat by class:
   * `.player-nameplate(.highlight-me)`, `.player-name`, `.chips`, `.bet-amount`,
   * `.avatar-overlay.allin|.folded`, `.position-badge.dealer`, `.equity-badge
   * (.modelled)`, `.stack-delta`, `.hand-tag`, `.fold-word`, `.player-cards
   * .card`, `.turn-timer`, `.join-seat`. Those names are load-bearing.
   */
  import Card from './Card.svelte';
  import { avatarFor, chipBand, chipStackCount, clockArcDegrees } from '$lib/table-visuals.js';

  const {
    player = null,
    seatIndex = 0,
    seatLabel = '',
    name = '',
    isHero = false,
    acting = false,
    live = false,
    lit = false,           // live hand at the all-in or the showdown
    folded = false,
    winner = false,
    dealer = false,
    puckFrom = null,       // {dx, dy} in ring units the puck travels FROM, or null
    showCards = false,     // gameInProgress || isShowdown
    heroCards = null,      // the hero's own two cards
    revealed = null,       // an opponent's engine-revealed pair, or null
    betAmount = 0,
    betAllIn = false,
    bigBlind = 0,
    timeRemaining = null,
    clockFraction = 0,
    clockUrgent = false,
    equityText = null,
    equityModelled = false,
    equityNote = '',
    awardAmount = null,    // formatted "+X" text, or null
    handTag = '',
    awardOnSpoke = false,
    spokeY = false,
    fmt = (v) => String(v),
    onJoin = () => {}
  } = $props();

  const avatar = $derived(player?.principal ? avatarFor(player.principal.toString(), name) : null);
  const band = $derived(chipBand(betAmount, bigBlind));
  const discs = $derived(chipStackCount(betAmount, bigBlind));
  const arc = $derived(clockArcDegrees(clockFraction));
</script>

{#if player}
  <!-- Hole cards. A FACE-DOWN pair only has to say "this player has cards", so
       it tucks behind the plate. A revealed pair has to be READ, so it clears
       the plate entirely. -->
  <div class="player-cards" class:hero={isHero} class:shown={!isHero && !!revealed} class:winner>
    {#if isHero && heroCards && showCards}
      <Card card={heroCards[0]} index={0} />
      <Card card={heroCards[1]} index={1} />
    {:else if revealed}
      <Card card={revealed[0]} index={0} />
      <Card card={revealed[1]} index={1} />
    {:else if showCards && live}
      <Card faceDown={true} index={0} />
      <Card faceDown={true} index={1} />
    {/if}
  </div>

  <!-- committed chips, on the chip spot between the pod and the pot -->
  {#if betAmount > 0}
    <div class="bet-chip band-{band}" class:all-in={betAllIn}>
      <span class="chip-stack" aria-hidden="true" style:--discs={discs}>
        {#each Array(discs) as _, k}
          <i class="chip" style:--k={k}></i>
        {/each}
      </span>
      <span class="bet-amount">{fmt(betAmount)}</span>
    </div>
  {/if}

  <!-- the plate: a broadcast lower-third -->
  <div
    class="player-nameplate"
    class:highlight-me={isHero}
    class:action-on={acting}
    class:is-winner={winner}
    class:folded
    class:lit
  >
    <div class="avatar-container" class:is-me={isHero}>
      {#if acting}
        <span class="pod-clock" class:urgent={clockUrgent} style:--arc="{arc}deg" aria-hidden="true"></span>
      {/if}
      {#if avatar}
        <img src={avatar.src} alt="" class="player-avatar" />
      {/if}
      {#if folded}
        <div class="avatar-overlay folded"><span class="fold-x" aria-hidden="true">&#10005;</span></div>
      {:else if player.is_all_in}
        <div class="avatar-overlay allin">All In</div>
      {/if}
    </div>
    <div class="pod-text">
      <span class="player-name" class:is-me={isHero}>{name}</span>
      <span class="stack-row">
        <span class="chips cd-money">{fmt(player.chips)}</span>
        {#if folded}<span class="fold-word">Fold</span>{/if}
      </span>
    </div>
    {#if acting && timeRemaining !== null}
      <div class="pod-slot">
        <span class="turn-timer" class:urgent={clockUrgent}>{timeRemaining}s</span>
      </div>
    {/if}
  </div>

  <!-- the dealer puck: a white disc on the felt at the chip spot's inner side,
       travelling 500 ms from the previous dealer's seat when the button moves -->
  {#if dealer}
    <span
      class="position-badge dealer"
      class:travel={!!puckFrom}
      style:--pdx={puckFrom?.dx ?? 0}
      style:--pdy={puckFrom?.dy ?? 0}
      title="Dealer"
    >D</span>
  {/if}

  {#if equityText}
    <!-- THE EQUITY BADGE IS A SEAT-LEVEL OBJECT (docs/DEFECTS.md T-22): a
         sibling of the plate, above the cards, on the readout spoke. Solid means
         computed over hands the ENGINE revealed; dashed means a model against
         random opponents, and the method line beside the pot says so. -->
    <span class="equity-badge" class:modelled={equityModelled} title={equityNote}>{equityText}</span>
  {/if}

  {#if awardAmount}
    <!-- THE DELTA CHIP on the award spot: what changed, at the stack it changed. -->
    <div class="winner-award" class:on-spoke={awardOnSpoke} class:spoke-y={spokeY}>
      <span class="stack-delta cd-money">{awardAmount}</span>
      {#if handTag}<span class="hand-tag">{handTag}</span>{/if}
    </div>
  {/if}
{:else}
  <!-- An empty chair: a solid avatar-sized disc ON the rail with a plus, and
       the call to action beneath it. Never a dashed outline. -->
  <button class="join-seat" onclick={() => onJoin(seatIndex)}>
    <span class="sit-disc" aria-hidden="true">+</span>
    <span class="sit-text">
      <span class="sit-word">Sit</span>
      <span class="sit-seat">{seatLabel}</span>
    </span>
  </button>
{/if}

<style>
  /* =========================================================================
     THE PLATE
     ========================================================================= */

  .player-nameplate {
    position: absolute;
    left: 0;
    top: 0;
    /* ABOVE the hole cards: `.player-cards` opens a stacking context at z 4. */
    z-index: 6;
    transform: translate(-50%, -50%);
    width: var(--pod-w);
    height: var(--pod-h);
    display: flex;
    align-items: center;
    gap: 0.4em;
    /* The avatar breaks the left edge: its centre sits ON the plate's edge, so
       the text starts half an avatar in. */
    padding: 0 0.55em 0 calc(var(--avatar) * 0.62);
    border-radius: calc(var(--fw) * 0.012);
    background: linear-gradient(180deg, var(--cd-plate-hi), var(--cd-plate) 55%, var(--cd-plate-lo));
    border: 1px solid var(--cd-line);
    box-shadow: var(--cd-shadow-pod), var(--cd-shadow-inset-soft);
    /* bar 12: the acting indicator lands in <= 400 ms */
    transition: border-color var(--cd-acting) var(--cd-ease),
                box-shadow var(--cd-acting) var(--cd-ease);
  }

  /* YOUR plate: same charcoal, the "this is you" signal on the edge and name. */
  .player-nameplate.highlight-me {
    border-color: var(--cd-accent-line-strong);
  }

  .player-nameplate.action-on {
    border-color: var(--cd-line-hot);
    box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-glow-white), var(--cd-shadow-pod);
  }

  .player-nameplate.lit {
    border-color: var(--cd-line-bright);
  }
  .player-nameplate.lit.highlight-me { border-color: var(--cd-accent-line-strong); }

  .player-nameplate.folded {
    filter: grayscale(1) contrast(0.85);
    border-color: var(--cd-line-soft);
    box-shadow: none;
  }

  .player-nameplate.is-winner {
    border-color: var(--cd-money);
    animation: winner-glow var(--cd-glow) cubic-bezier(0.4, 0, 0.2, 1) 1 both;
  }

  /* bar 13: one long beat, three phases, the single money accent */
  @keyframes winner-glow {
    0%   { box-shadow: 0 0 0 var(--cd-money-glow), var(--cd-shadow-pod); }
    25%  { box-shadow: 0 0 calc(var(--fw) * 0.09) var(--cd-money-glow), var(--cd-shadow-pod); }
    50%  { box-shadow: 0 0 calc(var(--fw) * 0.035) var(--cd-money-line), var(--cd-shadow-pod); }
    75%  { box-shadow: 0 0 calc(var(--fw) * 0.075) var(--cd-money-glow), var(--cd-shadow-pod); }
    100% { box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-money-line), var(--cd-shadow-pod); }
  }

  /* ---- avatar, breaking the plate's left edge ---- */

  .avatar-container {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translate(-50%, -50%);
    width: var(--avatar);
    height: var(--avatar);
    border-radius: 50%;
    background: var(--cd-plate);
    box-shadow: 0 0 0 2px var(--cd-plate-lo), var(--cd-shadow-chip);
  }

  .avatar-container.is-me { box-shadow: 0 0 0 2px var(--cd-accent), var(--cd-shadow-chip); }

  .player-avatar {
    width: 100%;
    height: 100%;
    display: block;
    border-radius: 50%;
  }

  /* THE ACTION CLOCK RING. A conic sweep around the avatar, driven by --arc
     (degrees, from the clock fraction). It shrinks clockwise as time runs out;
     the last ten seconds turn it red. */
  .pod-clock {
    position: absolute;
    inset: calc(var(--avatar) * -0.11);
    border-radius: 50%;
    background: conic-gradient(var(--cd-accent) var(--arc), transparent 0);
    -webkit-mask: radial-gradient(circle, transparent calc(50% - var(--avatar) * 0.11), currentColor calc(50% - var(--avatar) * 0.11 + 1px));
    mask: radial-gradient(circle, transparent calc(50% - var(--avatar) * 0.11), currentColor calc(50% - var(--avatar) * 0.11 + 1px));
    transition: background var(--cd-base) linear;
    z-index: 1;
  }

  .pod-clock.urgent { background: conic-gradient(var(--cd-danger) var(--arc), transparent 0); }

  .avatar-overlay {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.5em;
    font-weight: var(--cd-weight-display);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-align: center;
    line-height: 1;
    z-index: 2;
  }

  .avatar-overlay.folded { background: var(--cd-capsule); color: var(--cd-ink-2); }

  .fold-x {
    font-size: 2em;
    font-weight: 400;
    line-height: 1;
    color: var(--cd-ink-2);
  }

  .avatar-overlay.allin {
    background: var(--cd-danger);
    color: var(--cd-ink);
    box-shadow: inset 0 0 0 2px var(--cd-glow-white);
  }

  /* ---- text rows ---- */

  .pod-text {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    flex: 1 1 auto;
    line-height: 1.12;
  }

  .player-name {
    font-size: var(--cd-felt-name);
    font-weight: var(--cd-weight-strong);
    color: var(--cd-ink-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .player-name.is-me { color: var(--cd-accent-hi); }

  .stack-row {
    display: flex;
    align-items: baseline;
    gap: 0.45em;
    min-width: 0;
  }

  /* The stack is the largest text on the table after the pot. */
  .chips {
    font-size: var(--cd-felt-stack);
    font-weight: var(--cd-weight-display);
    color: var(--cd-ink);
    white-space: nowrap;
  }

  .fold-word {
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-display);
    letter-spacing: var(--cd-tracking-wide);
    text-transform: uppercase;
    color: var(--cd-ink-2);
    line-height: 1;
  }

  .pod-slot {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
  }

  .turn-timer {
    font-size: var(--cd-felt-body);
    font-weight: var(--cd-weight-display);
    color: var(--cd-ink);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .turn-timer.urgent { color: var(--cd-danger-hi); }

  /* =========================================================================
     THE DEALER PUCK: on the felt, in front of the plate, on the inner side.
     It travels 500 ms from the previous dealer's seat (bar 11).
     ========================================================================= */

  .position-badge.dealer {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 8;
    --puck: calc(var(--fw) * 0.028);
    /* --px/--py: the puck spot computed per seat and per orientation in
       ringSeats(), on the felt, clear of the plate, the cards and the chips. */
    --puck-dx: calc(var(--px, 0) * var(--fw));
    --puck-dy: calc(var(--py, 0) * var(--fw));
    width: var(--puck);
    height: var(--puck);
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: calc(var(--puck) * 0.55);
    font-weight: var(--cd-weight-display);
    color: var(--cd-ink-on-light);
    background: radial-gradient(circle at 50% 38%, var(--cd-chip-white), var(--cd-ink-1) 75%);
    box-shadow: var(--cd-shadow-chip), inset 0 0 0 1px var(--cd-chip-rim);
    transform: translate(-50%, -50%) translate(var(--puck-dx), var(--puck-dy));
  }

  .position-badge.dealer.travel {
    animation: puck-travel var(--cd-move) var(--cd-ease-move) both;
  }

  @keyframes puck-travel {
    from {
      transform:
        translate(-50%, -50%)
        translate(var(--puck-dx), var(--puck-dy))
        translate(calc(var(--pdx, 0) * var(--rx)), calc(var(--pdy, 0) * var(--ry)));
    }
    to {
      transform: translate(-50%, -50%) translate(var(--puck-dx), var(--puck-dy));
    }
  }

  /* =========================================================================
     THE EQUITY BADGE, on the readout spoke (docs/DEFECTS.md T-22)
     ========================================================================= */

  .equity-badge {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 9;
    /* Past the plate's end; and past the avatar too when the badge takes the
       left end, which is the edge the avatar breaks. */
    --badge-dx: calc(var(--rdx, 0) * (var(--pod-w) * 0.5 + 2.1em) - max(0px, -1 * var(--rdx, 0) * var(--avatar) * 0.45));
    --badge-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + 0.8em));
    transform: translate(-50%, -50%) translate(var(--badge-dx), var(--badge-dy));
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.1em 0.4em;
    border-radius: 0.35em;
    font-size: var(--cd-felt-small);
    font-weight: var(--cd-weight-display);
    line-height: 1.25;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
    background: var(--cd-capsule);
    color: var(--cd-ink);
    border: 1px solid var(--cd-line-bright);
    white-space: nowrap;
    cursor: help;
    box-shadow: var(--cd-shadow-chip);
  }

  :global(.seat.spoke-y) .equity-badge { --badge-dx: calc(var(--pod-w) * -0.24); }

  .equity-badge.modelled {
    border-style: dashed;
    border-color: var(--cd-line-strong);
    color: var(--cd-ink-1);
  }

  /* =========================================================================
     HOLE CARDS, on an inner ring
     ========================================================================= */

  .player-cards {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    gap: calc(var(--fw) * 0.006);
    --card-w: calc(var(--fw) * var(--card-opp-r));
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r)),
        calc(var(--cy, -1) * var(--fw) * var(--off-opp-r))
      );
    z-index: 4;                      /* behind the pod, like GGPoker */
  }

  /* A REVEALED PAIR IS A THIRD OBJECT, on its own plinth, half the nudge. */
  .player-cards.shown {
    z-index: 7;
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r) * 0.5),
        calc(var(--cy, -1) * var(--fw) * var(--off-shown-r))
      );
  }

  .player-cards.shown::before {
    content: '';
    position: absolute;
    inset: calc(var(--fw) * -0.008) calc(var(--fw) * -0.010);
    z-index: -1;
    border-radius: calc(var(--fw) * 0.014);
    background: var(--cd-capsule);
    box-shadow: inset 0 0 0 1px var(--cd-line-strong), var(--cd-shadow-chip);
  }

  .player-cards.shown.winner::before {
    box-shadow: inset 0 0 0 1px var(--cd-money-line), 0 0 calc(var(--fw) * 0.02) var(--cd-money-dim);
  }

  /* YOUR cards outrank a pod: the hero's pair paints ABOVE its own plate. */
  .player-cards.hero {
    z-index: 7;
    --card-w: calc(var(--fw) * var(--card-hero-r));
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--off-hero-r)),
        calc(var(--ny, 0) * var(--fw) * var(--off-hero-r))
      );
  }

  /* ---- cards revealing: bar 11, a flip is 150 ms, rotateY only ---- */
  .player-cards.shown > :global(.card) {
    animation: card-reveal var(--cd-flip) var(--cd-ease) both;
    transform-origin: 50% 50%;
  }

  .player-cards.shown > :global(.card:nth-child(2)) { animation-delay: 100ms; }

  @keyframes card-reveal {
    0%   { transform: perspective(600px) rotateY(88deg); }
    100% { transform: none; }
  }

  /* =========================================================================
     BET CHIPS: a stack of discs with edge notches and the amount beside it
     ========================================================================= */

  .bet-chip {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 8;
    display: flex;
    align-items: center;
    gap: 0.35em;
    white-space: nowrap;
    /* bar 11: 500 ms for anything that moves an object */
    transition: transform var(--cd-move) var(--cd-ease-move);
    transform:
      translate(-50%, -50%)
      translate(calc(var(--bx, 0) * var(--fw)), calc(var(--by, 0) * var(--fw)));
    --chip-face: var(--cd-chip-white);
    --chip-ink: var(--cd-chip-ink);
  }

  .bet-chip.band-red   { --chip-face: var(--cd-chip-red);   --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-blue  { --chip-face: var(--cd-chip-blue);  --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-green { --chip-face: var(--cd-chip-green); --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-black { --chip-face: var(--cd-chip-black); --chip-ink: var(--cd-chip-notch); }
  .bet-chip.band-gold  { --chip-face: var(--cd-chip-gold);  --chip-ink: var(--cd-money-ink); }

  .chip-stack {
    --chip: calc(var(--fw) * 0.026);
    --lift: calc(var(--chip) * 0.2);
    position: relative;
    flex: 0 0 auto;
    width: var(--chip);
    height: calc(var(--chip) + (var(--discs, 1) - 1) * var(--lift));
  }

  /* the stack's shadow on the felt */
  .chip-stack::before {
    content: '';
    position: absolute;
    left: 6%;
    bottom: calc(var(--chip) * -0.1);
    width: 100%;
    height: calc(var(--chip) * 0.4);
    border-radius: 50%;
    background: var(--cd-felt-shade);
    filter: blur(calc(var(--chip) * 0.12));
  }

  .chip {
    position: absolute;
    left: 0;
    bottom: calc(var(--k, 0) * var(--lift));
    width: var(--chip);
    height: var(--chip);
    border-radius: 50%;
    background:
      radial-gradient(circle at 50% 44%, var(--chip-face) 0 56%, transparent 57%),
      repeating-conic-gradient(from 0deg, var(--cd-chip-notch) 0deg 13deg, var(--chip-face) 13deg 45deg);
    box-shadow:
      inset 0 0 0 1px var(--cd-chip-rim),
      0 1px 0 var(--cd-chip-rim);
  }

  .bet-amount {
    font-size: var(--cd-felt-body);
    font-weight: var(--cd-weight-figure);
    padding: 0.12em 0.5em;
    border-radius: var(--cd-radius-pill);
    background: var(--cd-capsule);
    color: var(--cd-money);
    border: 1px solid var(--cd-money-line);
    box-shadow: var(--cd-shadow-chip);
    font-variant-numeric: tabular-nums;
  }

  .bet-chip.all-in .bet-amount { color: var(--cd-ink); border-color: var(--cd-danger-line); }

  /* =========================================================================
     THE WINNER'S AWARD, on the award spot (docs/DEFECTS.md T-23)
     ========================================================================= */

  .winner-award {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.18em;
    white-space: nowrap;
    --award-dx: calc(var(--ax, 0) * var(--fw));
    --award-dy: calc(var(--ay, 0) * var(--fw));
    transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy));
    /* The award LANDS, and it lands LAST: 760 ms in, where the pot ghost
       finishes its flight to this pod. */
    animation: award-land 0.42s var(--cd-ease-spring) 0.76s both;
  }

  .stack-delta {
    padding: 0.12em 0.55em;
    border-radius: var(--cd-radius-pill);
    background: linear-gradient(180deg, var(--cd-money-hi), var(--cd-money));
    color: var(--cd-money-ink);
    font-size: 0.86em;
    font-weight: var(--cd-weight-display);
    box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-money-glow), var(--cd-shadow-chip);
  }

  .hand-tag {
    padding: 0.1em 0.5em;
    border-radius: 0.35em;
    background: var(--cd-capsule);
    border: 1px solid var(--cd-money-line);
    color: var(--cd-money);
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-display);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  @keyframes award-land {
    from { opacity: 0; transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(0.6); }
    to   { opacity: 1; transform: translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(1); }
  }

  /* =========================================================================
     THE EMPTY CHAIR: a solid disc on the rail, a plus, and "Sit" beneath
     ========================================================================= */

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

  /* The whole footprint of a plate is the hit area, as it always was. */
  .join-seat::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    width: var(--pod-w);
    height: calc(var(--pod-h) * 1.2);
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

  /* =========================================================================
     CROWDED RING (8-9 seats): the plate's inner spacing comes down with it
     ========================================================================= */

  :global(.ring-crowded) .player-nameplate { gap: 0.32em; padding-right: 0.45em; }
  :global(.ring-crowded) .chips { letter-spacing: -0.015em; }

  /* =========================================================================
     PORTRAIT: the same objects, one type step tighter, the award on the spoke
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    .player-nameplate { overflow: visible; padding-left: calc(var(--avatar) * 1.12); }

    /* In portrait the pods sit ON the felt, and a flank plate already reaches
       the phone's edge; the avatar therefore sits INSIDE the plate rather than
       breaking its outer edge, so nothing hangs off a 390 px screen. */
    .avatar-container { left: calc(var(--avatar) * 0.56); }

    .equity-badge { font-size: var(--cd-felt-label); padding: 0.08em 0.32em; }
    /* A vertical spoke carries both readouts on the same edge: the badge takes
       the inner end (see .spoke-y in PokerTable for the seat class). */
    :global(.seat.spoke-y) .equity-badge { --badge-dx: calc(var(--pod-w) * -0.3); }

    .winner-award { gap: 0.1em; }

    /* PORTRAIT HAS NO ROOM ON THE CHIP VECTOR (T-23), so the award rides the
       readout spoke with the badge: one step further out on a horizontal spoke,
       the other END of the same edge on a vertical one. */
    /* Flank seat (horizontal spoke): the award sits on the plate's OUTER half,
       on the side facing the board. Measured on the 6-max portrait ring every
       other spot is taken: past the inner end is the hero's pair (0.406 fw
       from the centre) or the winner line and its method footnote (to 0.22 fw,
       within 0.3 fw of the centre line, which the outer half never reaches);
       past the outer end is the screen edge; along the rail away from the
       board is the seat's own revealed pair; on the plate is the name. The
       badge keeps the spoke past the inner end. */
    .winner-award.on-spoke {
      flex-direction: column;
      gap: 0.1em;
      --award-dx: calc(-1 * var(--rdx, 0) * var(--pod-w) * 0.26);
      --award-dy: calc(-1 * sign(var(--sn, 1)) * (var(--pod-h) * 0.5 + 1.4em));
    }
    /* ...and the flank seat's badge hangs BELOW the plate's inner end: at
       mid-plate height it sat under the winner line's method footnote (a three
       row winner line reaches 0.25 fw from the centre, the flank plate's band
       starts at 0.22). Below the plate is clear: the revealed pair is at the
       plate's centre, the hero's pair starts 0.406 fw from the centre. */
    :global(.seat:not(.spoke-y)) .equity-badge { --badge-dy: calc(var(--pod-h) * 0.5 + 0.7em); }
    /* Top/bottom seat (vertical spoke): the badge at one end of the far edge,
       the award at the other; both were measured touching at the old 0.24. */
    .winner-award.on-spoke.spoke-y {
      flex-direction: column;
      gap: 0.1em;
      --award-dx: calc(var(--pod-w) * 0.34);
      --award-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + var(--fw) * 0.05));
    }

    .stack-delta { font-size: 0.66em; padding: 0.08em 0.4em; }
    .hand-tag { font-size: var(--cd-felt-label); padding: 0.06em 0.35em; }

    .chip-stack { --chip: calc(var(--fw) * 0.05); }
    .bet-amount { font-size: var(--cd-felt-small); }

    .position-badge.dealer { --puck: calc(var(--fw) * 0.052); }

    .sit-text { padding: 0.05em 0.4em; }
  }

  @media (prefers-reduced-motion: reduce) {
    .player-nameplate.is-winner { animation: none; box-shadow: 0 0 0 2px var(--cd-money); }
    .bet-chip, .player-nameplate, .pod-clock { transition: none; }
    .winner-award { animation: none; }
    .player-cards.shown > :global(.card) { animation: none; }
    .position-badge.dealer.travel { animation: none; }
  }
</style>
