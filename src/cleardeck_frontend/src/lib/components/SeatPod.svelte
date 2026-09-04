<script>
  /**
   * One seat on the ring: everything that hangs off a seat point.
   *
   * The PARENT (PokerTable.svelte) owns the geometry: it places a zero-size
   * `.seat` on the ellipse and publishes the seat's own vectors as custom
   * properties (--nx/--ny the inward normal, --bx/--by the chip spot, --ax/--ay
   * the award spot, --rdx/--rdy the readout spoke, --px/--py/--pkx/--pky the
   * puck spot, --cy the card direction) and the table's scale (--fw, --pod-w,
   * --pod-h, --avatar, --ui). This component draws the plate (a broadcast
   * lower-third: avatar breaking the left edge, name row, stack row, your
   * hand named on a third row, the action clock as a ring on the avatar)
   * and composes the other seat objects: HoleCards, BetStack, DealerPuck,
   * EquityBadge, WinnerAward, EmptySeat.
   *
   * THE HARNESS CONTRACT. tools/shots/lib/dom-scrape.mjs reads a seat by class:
   * `.player-nameplate(.highlight-me)`, `.player-name`, `.chips`, `.bet-amount`,
   * `.avatar-overlay.allin|.folded`, `.position-badge.dealer`, `.equity-badge
   * (.modelled)`, `.stack-delta`, `.hand-tag`, `.fold-word`, `.player-cards
   * .card`, `.turn-timer`, `.join-seat`, and `.caption-hand` for your named
   * hand. Those names are load-bearing.
   */
  import HoleCards from './HoleCards.svelte';
  import BetStack from './BetStack.svelte';
  import DealerPuck from './DealerPuck.svelte';
  import WinnerAward from './WinnerAward.svelte';
  import EquityBadge from './EquityBadge.svelte';
  import EmptySeat from './EmptySeat.svelte';
  import { avatarFor, clockArcDegrees } from '$lib/table-visuals.js';

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
    heroHandName = null,   // your hand, named in words (hero only)
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
    spokeEnds = false,
    fmt = (v) => String(v),
    onJoin = () => {}
  } = $props();

  const avatar = $derived(player?.principal ? avatarFor(player.principal.toString(), name) : null);
  const arc = $derived(clockArcDegrees(clockFraction));
</script>

{#if player}
  <HoleCards {isHero} {showCards} {live} {winner} {heroCards} {revealed} />

  {#if betAmount > 0}
    <BetStack amount={betAmount} allIn={betAllIn} {bigBlind} {fmt} />
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
      {#if isHero && heroHandName && !folded}
        <!-- GGPoker's named hand-strength readout, IN the plate: a function of
             your own two cards and a board everyone can see. -->
        <span class="hero-hand caption-hand">{heroHandName}</span>
      {/if}
    </div>
    {#if acting && timeRemaining !== null}
      <div class="pod-slot">
        <span class="turn-timer" class:urgent={clockUrgent}>{timeRemaining}s</span>
      </div>
    {/if}
  </div>

  {#if dealer}
    <DealerPuck {puckFrom} />
  {/if}

  {#if equityText}
    <EquityBadge text={equityText} modelled={equityModelled} note={equityNote} />
  {/if}

  {#if awardAmount}
    <WinnerAward {awardAmount} {handTag} onSpoke={awardOnSpoke} {spokeY} {spokeEnds} />
  {/if}
{:else}
  <EmptySeat {seatIndex} {seatLabel} {onJoin} />
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

  /* YOUR plate: same charcoal, the "this is you" signal on the edge and name.
     It carries a third row (your hand), so it may grow past the pod height. */
  .player-nameplate.highlight-me {
    border-color: var(--cd-accent-line-strong);
    height: auto;
    min-height: var(--pod-h);
    padding-block: 0.25em;
  }

  .player-nameplate.action-on {
    border-color: var(--cd-line-hot);
    box-shadow: 0 0 calc(var(--fw) * 0.03) var(--cd-glow-white), var(--cd-shadow-pod);
  }

  .player-nameplate.lit { border-color: var(--cd-line-bright); }
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

  /* The status disc over the avatar. Type at the on-felt floor, never below. */
  .avatar-overlay {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--cd-felt-label);
    font-weight: var(--cd-weight-display);
    letter-spacing: 0.02em;
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

  /* Your hand, named: the third row of your own plate. */
  .hero-hand {
    font-size: var(--cd-felt-small);
    font-weight: var(--cd-weight-figure);
    letter-spacing: 0.02em;
    color: var(--cd-accent-hi);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
     CROWDED RING (8-9 seats): the plate's inner spacing comes down with it
     ========================================================================= */

  :global(.ring-crowded) .player-nameplate { gap: 0.32em; padding-right: 0.45em; }
  :global(.ring-crowded) .chips { letter-spacing: -0.015em; }

  /* =========================================================================
     PORTRAIT: the same objects, one type step tighter, the avatar INSIDE
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    .player-nameplate { overflow: visible; padding-left: calc(var(--avatar) * 1.12); }

    /* In portrait the pods sit ON the felt, and a flank plate already reaches
       the phone's edge; the avatar therefore sits INSIDE the plate rather than
       breaking its outer edge, so nothing hangs off a 390 px screen. */
    .avatar-container { left: calc(var(--avatar) * 0.56); }

  }

  @media (prefers-reduced-motion: reduce) {
    .player-nameplate.is-winner { animation: none; box-shadow: 0 0 0 2px var(--cd-money); }
    .player-nameplate, .pod-clock { transition: none; }
  }
</style>
