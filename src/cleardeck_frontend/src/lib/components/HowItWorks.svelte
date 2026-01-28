<script>
  const { onClose } = $props();
</script>

<div class="modal-backdrop" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="button" tabindex="-1" aria-label="Close modal"></div>

<div class="modal-content" role="dialog" aria-labelledby="how-it-works-title">
  <div class="modal-header">
    <h2 id="how-it-works-title">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
        <path d="M9 12l2 2 4-4"/>
      </svg>
      How ClearDeck Works
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M18 6L6 18M6 6l12 12"/>
      </svg>
    </button>
  </div>

  <div class="modal-body">
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
            <strong>Immutable History</strong>
            <p>All hands are recorded on-chain and cannot be altered</p>
          </div>
        </div>
        <div class="guarantee">
          <span class="icon">3</span>
          <div>
            <strong>Open Source</strong>
            <p>All canister code can be inspected and verified</p>
          </div>
        </div>
        <div class="guarantee">
          <span class="icon">4</span>
          <div>
            <strong>Non-Custodial</strong>
            <p>You control your funds via Internet Identity - we can't access them</p>
          </div>
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(4px);
    z-index: 999;
  }

  .modal-content {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 90%;
    max-width: 700px;
    max-height: 85vh;
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
  }

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

  /* Responsive */
  @media (max-width: 600px) {
    .modal-content {
      width: 95%;
      max-height: 90vh;
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

    .section h3 {
      font-size: 15px;
    }
  }
</style>
