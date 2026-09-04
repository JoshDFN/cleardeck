<script>
  import { onMount } from 'svelte';
  import { scrollLock } from '$lib/scroll-lock.js';

  const { onClose } = $props();

  // Escape has to work from anywhere in the dialog, not only while the backdrop
  // happens to hold focus (which it never does, since it is not in the tab
  // order). Bound on <svelte:window> so it fires wherever the caret is.
  function onKeydown(e) {
    if (e.key === 'Escape') onClose();
  }

  /** Focus the close button when the dialog opens, so Escape/Tab land somewhere. */
  function autofocus(node) {
    node.focus();
  }

  // ---------------------------------------------------------------------------
  // THIS DIALOG OPENS BELOW THE APP CHROME, AND THAT IS A CORRECTNESS RULE.
  //
  // The disclaimer banner is `z-index: 100` on `.alpha-warning-banner`, a child
  // of `.app`; this dialog is `z-index: 1000` but it renders inside <main>,
  // which is its own stacking context, so the banner and the header PAINT OVER
  // it however high its z-index goes. Centred on the viewport, the dialog's own
  // title and its close button therefore ended up UNDER the banner: measured at
  // 1440x900 the dialog box started at y=67.5 with 243 px of chrome above it, so
  // 92 px of it — the entire header row and the × — could not be seen or
  // clicked. The keyboard path (Escape, autofocus) still worked, which is why no
  // textContent gate ever noticed.
  //
  // Raising the z-index is the wrong fix twice over: it would put the dialog
  // over the unaudited-alpha disclaimer, the 18+ notice, the jurisdiction
  // warning and the no-house statement, which the project's rules forbid making
  // less visible on any view. So the dialog measures the chrome instead and
  // starts under it. The four notices stay on screen, undimmed, with the dialog
  // open — verified on the rendered page, not in the source.
  let chromeBottom = $state(0);

  function measureChrome() {
    const bottomOf = (sel) => document.querySelector(sel)?.getBoundingClientRect().bottom ?? 0;
    // Whichever of the two ends lower; either may be absent on a given view.
    chromeBottom = Math.max(0, Math.ceil(Math.max(bottomOf('.alpha-warning-banner'), bottomOf('header'))));
  }

  onMount(() => {
    measureChrome();
    // Re-read on SCROLL as well as resize, and both matter:
    //   * the banner reflows with the width (3 lines at 1440, 7 at 390);
    //   * the chrome is in the page flow, not fixed, so its bottom edge in
    //     viewport coordinates moves as the page scrolls. On a phone the only
    //     "How it works" links are below the list, so the dialog opens with the
    //     banner already scrolled off — offset 0, full viewport, correct — and
    //     has to give the space back the moment the page returns to the top.
    //     Latching the value at open time put the × back under the banner.
    const remeasure = () => measureChrome();
    window.addEventListener('resize', remeasure);
    window.addEventListener('scroll', remeasure, { passive: true });
    return () => {
      window.removeEventListener('resize', remeasure);
      window.removeEventListener('scroll', remeasure);
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="modal-backdrop"
  style:--how-chrome="{chromeBottom}px"
  onclick={onClose}
  role="presentation"
></div>

<div
  class="modal-content"
  style:--how-chrome="{chromeBottom}px"
  role="dialog"
  aria-modal="true"
  aria-labelledby="how-it-works-title"
  use:scrollLock
>
  <div class="modal-header">
    <h2 id="how-it-works-title">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
        <path d="M9 12l2 2 4-4"/>
      </svg>
      How ClearDeck Works
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close" use:autofocus>
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M18 6L6 18M6 6l12 12"/>
      </svg>
    </button>
  </div>

  <div class="modal-body">
    <!-- The strongest claim first, because it is the one a rake-funded operator
         structurally cannot make. It is a property of the payout code, not a
         promotion, so it is stated as a fact and not as an offer. -->
    <section class="section">
      <div class="rake-banner">
        <span class="rake-figure">0%</span>
        <div>
          <strong>No rake. Not on any pot, not at any stake.</strong>
          <p>
            There is no house cut anywhere in the settlement code. Every chip that goes
            into a pot is paid back out to the players eligible for it, down to the last
            e8s, and the remainder of an odd split goes to a player rather than the house.
            There is no fee at the table, no time charge, and no tournament juice.
          </p>
        </div>
      </div>
    </section>

    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
        Provably Fair Shuffling
      </h3>
      <p>
        Every hand in ClearDeck uses <strong>commit-reveal cryptography</strong> to ensure the deck is fairly shuffled
        before any cards are dealt, and that the shuffle cannot be changed after.
      </p>
      <div class="steps">
        <div class="step">
          <span class="step-number">1</span>
          <div class="step-content">
            <strong>Commit Phase</strong>
            <p>Before dealing, the canister generates a random seed and publishes its SHA-256 hash. This commits to the shuffle without revealing it.</p>
          </div>
        </div>
        <div class="step">
          <span class="step-number">2</span>
          <div class="step-content">
            <strong>Play Phase</strong>
            <p>Cards are dealt from the shuffled deck. The seed remains hidden, so no one can predict upcoming cards.</p>
          </div>
        </div>
        <div class="step">
          <span class="step-number">3</span>
          <div class="step-content">
            <strong>Reveal Phase</strong>
            <p>When the hand ends, the original seed is revealed. Anyone can verify: hash(seed) = committed hash, proving the shuffle was predetermined.</p>
          </div>
        </div>
      </div>
    </section>

    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <path d="M12 16v-4M12 8h.01"/>
        </svg>
        Why This Matters
      </h3>
      <div class="comparison">
        <div class="comparison-item bad">
          <span class="label">Traditional Online Poker</span>
          <ul>
            <li>Shuffle happens on company servers</li>
            <li>Players must trust the operator</li>
            <li>No way to verify fairness</li>
            <li>History can be altered</li>
          </ul>
        </div>
        <div class="comparison-item good">
          <span class="label">ClearDeck</span>
          <ul>
            <li>Shuffle is cryptographically committed</li>
            <li>Verification is trustless</li>
            <li>Every hand is independently verifiable</li>
            <li>Immutable on-chain history</li>
          </ul>
        </div>
      </div>
    </section>

    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
          <polyline points="3.27,6.96 12,12.01 20.73,6.96"/>
          <line x1="12" y1="22.08" x2="12" y2="12"/>
        </svg>
        Internet Computer Architecture
      </h3>
      <p>
        ClearDeck runs entirely on the <strong>Internet Computer Protocol (ICP)</strong>, a decentralized blockchain that can host full web applications.
      </p>
      <div class="tech-details">
        <div class="tech-item">
          <span class="tech-label">Smart Contracts (Canisters)</span>
          <p>Game logic runs in Rust canisters - deterministic, tamper-proof execution</p>
        </div>
        <div class="tech-item">
          <span class="tech-label">On-Chain Frontend</span>
          <p>The entire UI is served from the blockchain - no centralized servers</p>
        </div>
        <div class="tech-item">
          <span class="tech-label">Internet Identity</span>
          <p>Secure, anonymous authentication using WebAuthn (Face ID, Touch ID, hardware keys)</p>
        </div>
        <div class="tech-item">
          <span class="tech-label">ICP Tokens</span>
          <p>Native cryptocurrency for deposits, withdrawals, and gameplay</p>
        </div>
      </div>
    </section>

    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14,2 14,8 20,8"/>
          <line x1="16" y1="13" x2="8" y2="13"/>
          <line x1="16" y1="17" x2="8" y2="17"/>
          <polyline points="10,9 9,9 8,9"/>
        </svg>
        Verification Steps
      </h3>
      <p>To verify any hand yourself:</p>
      <ol class="verify-steps">
        <li>Click <strong>"Verify Fair"</strong> in the header during or after a hand</li>
        <li>Copy the <strong>Seed Hash</strong> (committed before dealing)</li>
        <li>After the hand, copy the <strong>Revealed Seed</strong></li>
        <li>Use any SHA-256 tool to verify: <code>SHA256(revealed_seed) = seed_hash</code></li>
        <li>The deck order is deterministically derived from the seed</li>
      </ol>
    </section>

    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
          <polyline points="22,4 12,14.01 9,11.01"/>
        </svg>
        Security Guarantees
      </h3>
      <div class="guarantees">
        <div class="guarantee">
          <span class="icon">1</span>
          <div>
            <strong>Pre-committed Shuffle</strong>
            <p>The deck order is fixed before you see your cards</p>
          </div>
        </div>
        <div class="guarantee">
          <span class="icon">2</span>
          <div>
            <strong>Self-checking history</strong>
            <p>Every hand is stored with its own commitment and revealed seed, so a hand that had been rewritten would no longer hash to its published commitment</p>
          </div>
        </div>
        <div class="guarantee">
          <span class="icon">3</span>
          <div>
            <strong>Open Source</strong>
            <p>All canister code can be inspected, and the deployed module hash is reproducible from this source</p>
          </div>
        </div>
        <div class="guarantee">
          <span class="icon">4</span>
          <div>
            <strong>No rake on settlement</strong>
            <p>The payout path has no house cut: the pot is distributed in full to the winning players</p>
          </div>
        </div>
      </div>
    </section>

    <!-- The two claims this modal used to make that were not true, corrected.
         A fairness page that overstates its own guarantees is worse than one
         that has none, because it is the page a player trusts. -->
    <section class="section">
      <h3>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        What This Does NOT Guarantee
      </h3>
      <div class="limits">
        <div class="limit">
          <strong>Your chips are held by the table canister, not by you.</strong>
          <p>
            Deposits are credited to a balance the canister keeps for your principal. That
            is a custodial arrangement. Internet Identity proves who you are; it does not
            hold your chips and it cannot recover them if the canister misbehaves.
          </p>
        </div>
        <div class="limit">
          <strong>A controller can upgrade these canisters.</strong>
          <p>
            Upgradeable code means the rules and the stored history can change. What a
            controller cannot do is retroactively make an already-published commitment
            hash match a different deck, which is why the commit-reveal record above is
            the part worth checking rather than the promise.
          </p>
        </div>
        <div class="limit danger">
          <strong>Unaudited code with known bugs.</strong>
          <p>
            This is published for education and testing. Any deposit is at your own risk
            and your funds are NOT safe: expect to lose everything you deposit. Online
            gambling is illegal in many jurisdictions. Only use it where legally
            permitted. 18+ only.
          </p>
        </div>
      </div>
    </section>
  </div>

  <!-- PINNED OUTSIDE THE SCROLLING REGION, and that is the point.
       The dialog already states all four notices in the "Limits" section above,
       but that section is the LAST thing in a 1,900 px scroller, so on arrival
       none of it is on screen. The banner behind this dialog is what carries
       them today — the dialog opens below it precisely so that stays true — but
       that guarantee depends on a stacking order in another component, and this
       wave's lesson is that a guarantee nobody measures is not one.

       So the four protected phrases are restated here, verbatim and
       `flex-shrink: 0`, the same treatment the hand-history dialog uses
       (docs/DEFECTS.md H-36). Whatever happens to the z-index of anything else,
       a player reading this dialog has the unaudited-alpha disclaimer, the
       funds-at-risk warning, the jurisdiction warning, the 18+ notice and the
       no-rake property on screen and unscrollable. Additional copy only:
       nothing anywhere else is weakened by it. -->
  <p class="modal-notices">
    <span class="notice-icon" aria-hidden="true">⚠️</span>
    <strong>Unaudited code with known bugs</strong> — this is for education and testing, any
    deposit is at your own risk and your funds are NOT safe. Online gambling is illegal in many
    jurisdictions; only use it where legally permitted. 18+ only. No middleman, no house, 0% rake.
    <!-- WAVE 5 COHERENCE PASS: added, nothing changed. "0% rake" above is the
         property in different words; this is the sentence both of the repo's
         notice checks actually look for. -->
    No rake is taken from any pot on any table.
  </p>
</div>

<style>
  /* Starts under the chrome, so the scrim never dims the four protected notices
     and the dialog is never behind them (see the note in the script block). */
  .modal-backdrop {
    position: fixed;
    top: var(--how-chrome, 0px);
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(4px);
    z-index: 999;
  }

  .modal-content {
    position: fixed;
    top: calc(var(--how-chrome, 0px) + 10px);
    left: 50%;
    transform: translateX(-50%);
    width: 90%;
    max-width: 700px;
    max-height: calc(100vh - var(--how-chrome, 0px) - 20px);
    background: linear-gradient(145deg, #1a1a2e, #16162a);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    z-index: 1000;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
    flex-shrink: 0;
  }

  /* Not in the scroller. See the markup note. */
  .modal-notices {
    flex-shrink: 0;
    margin: 0;
    padding: 10px 24px 12px;
    border-top: 1px solid rgba(248, 113, 113, 0.28);
    background: rgba(185, 28, 28, 0.16);
    font-size: 11.5px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.9);
  }

  .modal-notices strong { color: #fef08a; }
  .notice-icon { font-size: 12px; }

  .modal-header h2 {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0;
    font-size: 20px;
    font-weight: 700;
    color: white;
  }

  .modal-header h2 svg {
    color: #00d4aa;
  }

  .close-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 8px;
    cursor: pointer;
    color: #888;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .modal-body {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
  }

  .section {
    margin-bottom: 32px;
  }

  .section:last-child {
    margin-bottom: 0;
  }

  .section h3 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 16px 0;
    font-size: 16px;
    font-weight: 600;
    color: #00d4aa;
  }

  .section h3 svg {
    opacity: 0.8;
  }

  .section p {
    margin: 0 0 16px 0;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.6;
    font-size: 14px;
  }

  .section strong {
    color: white;
  }

  /* Steps */
  .steps {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .step {
    display: flex;
    gap: 14px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 10px;
    border-left: 3px solid #00d4aa;
  }

  .step-number {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #00d4aa, #00a080);
    border-radius: 50%;
    color: #1a1a2e;
    font-weight: 700;
    font-size: 14px;
    flex-shrink: 0;
  }

  .step-content strong {
    display: block;
    margin-bottom: 4px;
    color: white;
    font-size: 14px;
  }

  .step-content p {
    margin: 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
  }

  /* Comparison */
  .comparison {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  .comparison-item {
    padding: 16px;
    border-radius: 10px;
  }

  .comparison-item.bad {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.2);
  }

  .comparison-item.good {
    background: rgba(0, 212, 170, 0.1);
    border: 1px solid rgba(0, 212, 170, 0.2);
  }

  .comparison-item .label {
    display: block;
    font-weight: 600;
    margin-bottom: 10px;
    font-size: 13px;
  }

  .comparison-item.bad .label {
    color: #f87171;
  }

  .comparison-item.good .label {
    color: #00d4aa;
  }

  .comparison-item ul {
    margin: 0;
    padding-left: 20px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
  }

  .comparison-item li {
    margin-bottom: 6px;
  }

  /* Tech details */
  .tech-details {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .tech-item {
    padding: 14px;
    background: rgba(99, 102, 241, 0.08);
    border: 1px solid rgba(99, 102, 241, 0.15);
    border-radius: 10px;
  }

  .tech-label {
    display: block;
    font-weight: 600;
    color: #818cf8;
    font-size: 13px;
    margin-bottom: 6px;
  }

  .tech-item p {
    margin: 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  /* Verify steps */
  .verify-steps {
    margin: 0;
    padding-left: 24px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
  }

  .verify-steps li {
    margin-bottom: 10px;
    line-height: 1.5;
  }

  .verify-steps code {
    background: rgba(0, 0, 0, 0.3);
    padding: 2px 6px;
    border-radius: 4px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    color: #fbbf24;
  }

  /* Guarantees */
  .guarantees {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .guarantee {
    display: flex;
    gap: 12px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 10px;
  }

  .guarantee .icon {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 212, 170, 0.15);
    border-radius: 50%;
    color: #00d4aa;
    font-weight: 700;
    font-size: 12px;
    flex-shrink: 0;
  }

  .guarantee strong {
    display: block;
    color: white;
    font-size: 13px;
    margin-bottom: 4px;
  }

  .guarantee p {
    margin: 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  /* No rake — the lead claim */
  .rake-banner {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 18px;
    align-items: center;
    padding: 16px 18px;
    border-radius: 12px;
    background:
      linear-gradient(100deg, rgba(0, 212, 170, 0.18), rgba(0, 212, 170, 0.05) 70%),
      rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(0, 212, 170, 0.3);
  }

  .rake-figure {
    font-size: 44px;
    line-height: 1;
    font-weight: 800;
    letter-spacing: -0.04em;
    color: #00d4aa;
  }

  .rake-banner strong {
    display: block;
    margin-bottom: 6px;
    font-size: 15px;
    color: #fff;
  }

  .rake-banner p {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: rgba(255, 255, 255, 0.62);
  }

  /* Limits — stated as plainly as the guarantees */
  .limits {
    display: grid;
    gap: 12px;
  }

  .limit {
    padding: 14px;
    border-radius: 10px;
    background: rgba(240, 180, 41, 0.07);
    border: 1px solid rgba(240, 180, 41, 0.22);
  }

  .limit.danger {
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .limit strong {
    display: block;
    margin-bottom: 5px;
    font-size: 13.5px;
    color: #f0b429;
  }

  .limit.danger strong { color: #f87171; }

  .limit p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.55;
    color: rgba(255, 255, 255, 0.62);
  }

  /* Responsive */
  @media (max-width: 600px) {
    .modal-content {
      width: 95%;
      /* The phone's chrome is 388 px of an 844 px screen, so a fraction-of-vh
         cap would put the dialog's foot 340 px below the fold. */
      max-height: calc(100vh - var(--how-chrome, 0px) - 16px);
    }

    .modal-header {
      padding: 16px 20px;
    }

    .modal-header h2 {
      font-size: 18px;
    }

    .modal-body {
      padding: 20px;
    }

    .comparison,
    .tech-details,
    .guarantees {
      grid-template-columns: 1fr;
    }

    .rake-banner { gap: 14px; padding: 14px; }
    .rake-figure { font-size: 34px; }

    .section h3 {
      font-size: 15px;
    }
  }
  /* THE PHONE: the close control at the 44 px touch floor. */
  @media (max-aspect-ratio: 1/1), (max-height: 560px) {
    .close-btn { width: var(--cd-touch-min); height: var(--cd-touch-min); min-width: var(--cd-touch-min); }
  }
</style>
