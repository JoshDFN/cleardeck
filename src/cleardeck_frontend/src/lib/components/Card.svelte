<script>
  const { card, faceDown = false, small = false } = $props();

  const suitSymbols = {
    Hearts: '♥',
    Diamonds: '♦',
    Clubs: '♣',
    Spades: '♠'
  };

  const suitColors = {
    Hearts: '#dc2626',
    Diamonds: '#dc2626',
    Clubs: '#1e293b',
    Spades: '#1e293b'
  };

  const rankDisplay = {
    Two: '2', Three: '3', Four: '4', Five: '5', Six: '6',
    Seven: '7', Eight: '8', Nine: '9', Ten: '10',
    Jack: 'J', Queen: 'Q', King: 'K', Ace: 'A'
  };

  function getSuit(card) {
    if (!card?.suit) return null;
    return Object.keys(card.suit)[0];
  }

  function getRank(card) {
    if (!card?.rank) return null;
    return Object.keys(card.rank)[0];
  }

  const suit = $derived(getSuit(card));
  const rank = $derived(getRank(card));
</script>

<div class="card" class:face-down={faceDown} class:small style:--suit-color={suit ? suitColors[suit] : '#333'}>
  {#if faceDown}
    <div class="card-back">
      <div class="back-border">
        <div class="back-inner">
          <div class="back-pattern">
            <div class="diamond-grid">
              {#each Array(20) as _}
                <div class="diamond"></div>
              {/each}
            </div>
            <div class="center-emblem">
              <div class="emblem-border">
                <span class="emblem-icon">♠</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  {:else if card}
    <div class="card-front">
      <div class="corner top-left">
        <span class="corner-rank">{rankDisplay[rank]}</span>
        <span class="corner-suit">{suitSymbols[suit]}</span>
      </div>
      <div class="center-pip">
        <span class="main-suit">{suitSymbols[suit]}</span>
      </div>
      <div class="corner bottom-right">
        <span class="corner-rank">{rankDisplay[rank]}</span>
        <span class="corner-suit">{suitSymbols[suit]}</span>
      </div>
    </div>
  {:else}
    <div class="card-empty"></div>
  {/if}
</div>

<style>
  .card {
    width: 60px;
    height: 84px;
    border-radius: 6px;
    background: #fefefe;
    box-shadow:
      0 2px 8px rgba(0,0,0,0.25),
      0 1px 3px rgba(0,0,0,0.15),
      inset 0 0 0 1px rgba(0,0,0,0.08);
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'Georgia', serif;
    transition: transform 0.2s, box-shadow 0.2s;
    position: relative;
    overflow: hidden;
    animation: cardDeal 0.4s ease-out;
  }

  .card:hover {
    transform: translateY(-4px);
    box-shadow:
      0 8px 20px rgba(0,0,0,0.3),
      0 4px 8px rgba(0,0,0,0.15),
      inset 0 0 0 1px rgba(0,0,0,0.08);
  }

  /* Card Front - Traditional Design */
  .card-front {
    width: 100%;
    height: 100%;
    position: relative;
    color: var(--suit-color);
    background: linear-gradient(135deg, #ffffff 0%, #f8f9fa 100%);
    border-radius: 6px;
  }

  .corner {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1;
  }

  .top-left {
    top: 4px;
    left: 5px;
  }

  .bottom-right {
    bottom: 4px;
    right: 5px;
    transform: rotate(180deg);
  }

  .corner-rank {
    font-size: 14px;
    font-weight: bold;
    letter-spacing: -0.5px;
  }

  .corner-suit {
    font-size: 12px;
    margin-top: -2px;
  }

  .center-pip {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
  }

  .main-suit {
    font-size: 32px;
    filter: drop-shadow(0 1px 1px rgba(0,0,0,0.1));
  }

  /* Card Back - Premium Design */
  .card-back {
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, #1e3a5f 0%, #0f2744 100%);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 3px;
  }

  .back-border {
    width: 100%;
    height: 100%;
    border: 1.5px solid rgba(212, 175, 55, 0.6);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    background: linear-gradient(145deg, rgba(212, 175, 55, 0.1), transparent);
  }

  .back-inner {
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, #1a3352 0%, #0d1f33 100%);
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    overflow: hidden;
  }

  .back-pattern {
    width: 100%;
    height: 100%;
    position: relative;
  }

  .diamond-grid {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    grid-template-rows: repeat(5, 1fr);
    gap: 1px;
    padding: 4px;
    opacity: 0.4;
  }

  .diamond {
    background: linear-gradient(135deg,
      transparent 30%,
      rgba(212, 175, 55, 0.5) 50%,
      transparent 70%
    );
    transform: rotate(45deg) scale(0.7);
  }

  .center-emblem {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 1;
  }

  .emblem-border {
    width: 28px;
    height: 28px;
    border: 1.5px solid rgba(212, 175, 55, 0.7);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(145deg, #1e3a5f 0%, #152d4a 100%);
    box-shadow:
      0 2px 6px rgba(0,0,0,0.4),
      inset 0 1px 2px rgba(212, 175, 55, 0.2);
  }

  .emblem-icon {
    color: rgba(212, 175, 55, 0.9);
    font-size: 14px;
    filter: drop-shadow(0 1px 2px rgba(0,0,0,0.3));
  }

  .card-empty {
    width: 100%;
    height: 100%;
    background: rgba(255,255,255,0.05);
    border: 2px dashed rgba(255,255,255,0.2);
    border-radius: 6px;
  }

  .face-down {
    background: transparent;
  }

  .face-down:hover {
    transform: translateY(-2px);
  }

  /* Small variant */
  .small {
    width: 44px;
    height: 62px;
  }

  .small .corner-rank {
    font-size: 11px;
  }

  .small .corner-suit {
    font-size: 9px;
  }

  .small .main-suit {
    font-size: 22px;
  }

  .small .top-left {
    top: 3px;
    left: 4px;
  }

  .small .bottom-right {
    bottom: 3px;
    right: 4px;
  }

  .small .emblem-border {
    width: 20px;
    height: 20px;
  }

  .small .emblem-icon {
    font-size: 10px;
  }

  .small .diamond-grid {
    grid-template-columns: repeat(3, 1fr);
    grid-template-rows: repeat(4, 1fr);
  }

  @keyframes cardDeal {
    from {
      opacity: 0;
      transform: translateY(-30px) rotateY(180deg) scale(0.8);
    }
    to {
      opacity: 1;
      transform: translateY(0) rotateY(0deg) scale(1);
    }
  }
</style>
