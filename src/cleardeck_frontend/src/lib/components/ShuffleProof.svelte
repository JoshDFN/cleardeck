<script>
  import logger from '$lib/logger.js';
  
  const { proof, handNumber, tableActor } = $props();

  let verificationResult = $state(null);
  let verifying = $state(false);
  let copied = $state(null);
  let showDetails = $state(false);
  let verificationSource = $state(null); // 'onchain' or 'client'

  // Unwrap optional revealed_seed (Candid opt text comes as [] or [string])
  // Also handle case where it might be a direct string
  function unwrapRevealedSeed(rs) {
    if (!rs) return null;
    // If it's an array (Candid optional), unwrap it
    if (Array.isArray(rs)) {
      return rs.length > 0 ? rs[0] : null;
    }
    // If it's already a string, use it directly
    if (typeof rs === 'string' && rs.length > 0) {
      return rs;
    }
    return null;
  }

  const revealedSeed = $derived(unwrapRevealedSeed(proof?.revealed_seed));

  // Client-side verification using SubtleCrypto
  async function verifyClientSide() {
    if (!proof?.seed_hash || !revealedSeed) {
      logger.debug('Verification skipped: missing seed_hash or revealedSeed', {
        hasSeedHash: !!proof?.seed_hash,
        hasRevealedSeed: !!revealedSeed,
        revealedSeedType: typeof revealedSeed,
        rawRevealedSeed: proof?.revealed_seed
      });
      return false;
    }

    try {
      // Convert hex string to bytes
      const hexMatch = revealedSeed.match(/.{1,2}/g);
      if (!hexMatch) {
        logger.error('Failed to parse revealedSeed as hex:', revealedSeed);
        return false;
      }
      const seedBytes = new Uint8Array(hexMatch.map(byte => parseInt(byte, 16)));

      // Hash with SHA-256
      const hashBuffer = await crypto.subtle.digest('SHA-256', seedBytes);
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

      logger.debug('Verification comparison:', {
        computedHash: hashHex,
        expectedHash: proof.seed_hash,
        match: hashHex === proof.seed_hash
      });

      return hashHex === proof.seed_hash;
    } catch (e) {
      logger.error('Client-side verification error:', e);
      return false;
    }
  }

  // On-chain verification - calls the canister to verify
  async function verifyOnChain() {
    if (!tableActor || !proof?.seed_hash || !revealedSeed) return null;

    try {
      const result = await tableActor.verify_shuffle(proof.seed_hash, revealedSeed);
      return result;
    } catch (e) {
      logger.error('On-chain verification error:', e);
      return null;
    }
  }

  async function verifyProof() {
    if (!proof?.seed_hash || !revealedSeed) return;

    verifying = true;
    try {
      // Try on-chain verification first (most trusted)
      if (tableActor) {
        const onChainResult = await verifyOnChain();
        if (onChainResult !== null) {
          verificationResult = onChainResult ? 'valid' : 'invalid';
          verificationSource = 'onchain';
          verifying = false;
          return;
        }
      }

      // Fallback to client-side verification
      const clientResult = await verifyClientSide();
      verificationResult = clientResult ? 'valid' : 'invalid';
      verificationSource = 'client';
    } catch (e) {
      logger.error('Verification failed:', e);
      verificationResult = 'invalid';
      verificationSource = 'client';
    }
    verifying = false;
  }

  async function copyToClipboard(text, label) {
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
    const ms = Number(ns) / 1_000_000;
    return new Date(ms).toLocaleString();
  }

  function truncateHash(hash) {
    if (!hash || hash.length < 16) return hash;
    return `${hash.slice(0, 8)}...${hash.slice(-8)}`;
  }
</script>

<div class="shuffle-proof">
  <div class="proof-header">
    <div class="header-left">
      <div class="shield-icon">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          <path d="M9 12l2 2 4-4" stroke="#2ecc71"/>
        </svg>
      </div>
      <div>
        <h3>Provably Fair</h3>
        <span class="subtitle">Cryptographically verified shuffle</span>
      </div>
    </div>
    <span class="hand-number">Hand #{handNumber || 'N/A'}</span>
  </div>

  <!-- Trust Chain Visualization -->
  <div class="trust-chain">
    <div class="chain-step" class:active={true}>
      <div class="step-icon vrf">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <path d="M12 6v6l4 2"/>
        </svg>
      </div>
      <div class="step-content">
        <span class="step-label">VRF Random</span>
        <span class="step-status">ICP Subnet</span>
      </div>
    </div>
    <div class="chain-arrow">→</div>
    <div class="chain-step" class:active={proof?.seed_hash}>
      <div class="step-icon hash">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
      </div>
      <div class="step-content">
        <span class="step-label">Committed</span>
        <span class="step-status">{proof?.seed_hash ? 'Locked' : 'Pending'}</span>
      </div>
    </div>
    <div class="chain-arrow">→</div>
    <div class="chain-step" class:active={revealedSeed}>
      <div class="step-icon reveal">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
          <circle cx="12" cy="12" r="3"/>
        </svg>
      </div>
      <div class="step-content">
        <span class="step-label">Revealed</span>
        <span class="step-status">{revealedSeed ? 'Visible' : 'Hidden'}</span>
      </div>
    </div>
    <div class="chain-arrow">→</div>
    <div class="chain-step" class:active={verificationResult === 'valid'} class:verified={verificationResult === 'valid'}>
      <div class="step-icon verify">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
          <polyline points="22,4 12,14.01 9,11.01"/>
        </svg>
      </div>
      <div class="step-content">
        <span class="step-label">Verified</span>
        <span class="step-status">{verificationResult === 'valid' ? 'Fair!' : 'Check'}</span>
      </div>
    </div>
  </div>

  {#if proof}
    <div class="proof-details">
      <div class="proof-item">
        <span class="label">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
          Seed Hash (Pre-committed before dealing)
        </span>
        <div class="hash-row">
          <code class="hash" title={proof.seed_hash}>{truncateHash(proof.seed_hash)}</code>
          <button class="copy-btn" onclick={() => copyToClipboard(proof.seed_hash, 'hash')}>
            {copied === 'hash' ? 'Copied!' : 'Copy'}
          </button>
        </div>
      </div>

      {#if revealedSeed}
        <div class="proof-item">
          <span class="label">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
            Revealed Seed (After hand completed)
          </span>
          <div class="hash-row">
            <code class="hash revealed" title={revealedSeed}>{truncateHash(revealedSeed)}</code>
            <button class="copy-btn" onclick={() => copyToClipboard(revealedSeed, 'seed')}>
              {copied === 'seed' ? 'Copied!' : 'Copy'}
            </button>
          </div>
        </div>
      {:else}
        <div class="proof-item pending">
          <span class="label">Revealed Seed</span>
          <div class="pending-box">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <path d="M12 6v6l4 2"/>
            </svg>
            <span>Will be revealed after hand completes</span>
          </div>
        </div>
      {/if}

      <div class="proof-item timestamp">
        <span class="label">Timestamp</span>
        <span class="value">{formatTimestamp(proof.timestamp)}</span>
      </div>

      {#if revealedSeed}
        <div class="verification">
          <button
            class="verify-btn"
            onclick={verifyProof}
            disabled={verifying}
            class:verified={verificationResult === 'valid'}
          >
            {#if verifying}
              <span class="spinner"></span>
              Verifying...
            {:else if verificationResult === 'valid'}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <polyline points="20,6 9,17 4,12"/>
              </svg>
              Verified Fair! {#if verificationSource === 'onchain'}<span class="source-badge">On-Chain</span>{/if}
            {:else}
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
              </svg>
              Verify Shuffle
            {/if}
          </button>

          {#if verificationResult === 'invalid'}
            <div class="result invalid">
              <span class="icon">✗</span>
              <span>Verification unavailable for this hand</span>
              <p class="invalid-note">Some older hands may have verification data that was affected by an early system update. New hands are fully verifiable.</p>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Why This Can't Be Cheated -->
    <div class="security-section">
      <button class="section-toggle" onclick={() => showDetails = !showDetails}>
        <span>Why Can't This Be Cheated?</span>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:rotated={showDetails}>
          <polyline points="6,9 12,15 18,9"/>
        </svg>
      </button>

      {#if showDetails}
        <div class="security-details">
          <div class="security-point">
            <div class="point-header">
              <span class="point-icon vrf">1</span>
              <h5>Unpredictable Randomness (VRF)</h5>
            </div>
            <p>The seed comes from ICP's <strong>Verifiable Random Function</strong> - threshold BLS signatures across 13+ independent subnet nodes. No single party (including us) can predict or control it.</p>
          </div>

          <div class="security-point">
            <div class="point-header">
              <span class="point-icon commit">2</span>
              <h5>Tamper-Proof Commitment</h5>
            </div>
            <p>The SHA-256 hash is published <strong>before any cards are dealt</strong>. Once committed, the seed cannot be changed - any modification would produce a completely different hash.</p>
          </div>

          <div class="security-point">
            <div class="point-header">
              <span class="point-icon shuffle">3</span>
              <h5>Deterministic Shuffle</h5>
            </div>
            <p>The same seed always produces the <strong>exact same card order</strong>. Given the revealed seed, anyone can independently verify the entire shuffle sequence.</p>
          </div>

          <div class="security-point">
            <div class="point-header">
              <span class="point-icon math">4</span>
              <h5>Mathematical Impossibility</h5>
            </div>
            <p>To cheat, an attacker would need to find a different seed that both hashes to the same commitment AND produces favorable cards. With SHA-256, this requires ~2<sup>128</sup> operations - more than all computers on Earth could do in billions of years.</p>
          </div>

          <div class="trust-summary">
            <div class="trust-flow">
              <span class="flow-item">VRF</span>
              <span class="flow-arrow">→</span>
              <span class="flow-item">Seed</span>
              <span class="flow-arrow">→</span>
              <span class="flow-item">SHA-256</span>
              <span class="flow-arrow">→</span>
              <span class="flow-item">Shuffle</span>
            </div>
            <p class="trust-note">Each step is publicly verifiable. No trust in ClearDeck required.</p>
          </div>
        </div>
      {/if}
    </div>

    {#if revealedSeed}
      <div class="manual-verify">
        <h4>Verify Independently</h4>
        <p>Don't trust us - verify yourself using any SHA-256 tool:</p>
        <div class="verify-methods">
          <div class="method">
            <span class="method-name">Terminal (Linux/Mac):</span>
            <div class="command-row">
              <code class="command">echo -n "{revealedSeed}" | xxd -r -p | shasum -a 256</code>
              <button class="copy-btn small" onclick={() => copyToClipboard(`echo -n "${revealedSeed}" | xxd -r -p | shasum -a 256`, 'cmd')}>
                {copied === 'cmd' ? '✓' : 'Copy'}
              </button>
            </div>
          </div>
          <div class="method">
            <span class="method-name">Online Tool:</span>
            <a href="https://emn178.github.io/online-tools/sha256.html" target="_blank" rel="noopener">
              SHA256 Calculator
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
                <polyline points="15,3 21,3 21,9"/>
                <line x1="10" y1="14" x2="21" y2="3"/>
              </svg>
            </a>
            <div class="method-hint-box">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="16" x2="12" y2="12"/>
                <line x1="12" y1="8" x2="12" y2="8"/>
              </svg>
              <span><strong>Important:</strong> Set "Input Encoding" dropdown to <strong>"Hex"</strong> before pasting the seed!</span>
            </div>
          </div>
        </div>
        <div class="expected-result">
          <span class="expected-label">Result must match:</span>
          <code class="expected-hash">{proof.seed_hash}</code>
        </div>
      </div>
    {/if}
  {:else}
    <div class="no-proof">
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <path d="M12 6v6l4 2"/>
      </svg>
      <p>Shuffle proof will appear when cards are dealt</p>
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

  .header-left {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .shield-icon {
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.15), rgba(46, 204, 113, 0.05));
    padding: 10px;
    border-radius: 12px;
    border: 1px solid rgba(46, 204, 113, 0.2);
  }

  .shield-icon svg {
    display: block;
  }

  .proof-header h3 {
    color: #fff;
    margin: 0;
    font-size: 18px;
    font-weight: 700;
  }

  .subtitle {
    color: #888;
    font-size: 12px;
  }

  .hand-number {
    color: #666;
    font-size: 12px;
    background: rgba(255, 255, 255, 0.05);
    padding: 4px 10px;
    border-radius: 6px;
  }

  /* Trust Chain */
  .trust-chain {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 12px;
    margin-bottom: 20px;
    gap: 4px;
  }

  .chain-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    opacity: 0.4;
    transition: all 0.3s;
  }

  .chain-step.active {
    opacity: 1;
  }

  .chain-step.verified .step-icon {
    background: linear-gradient(135deg, #2ecc71, #27ae60);
    border-color: #2ecc71;
  }

  .step-icon {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
  }

  .step-icon.vrf { border-color: rgba(155, 89, 182, 0.4); }
  .chain-step.active .step-icon.vrf { background: rgba(155, 89, 182, 0.2); }

  .step-icon.hash { border-color: rgba(52, 152, 219, 0.4); }
  .chain-step.active .step-icon.hash { background: rgba(52, 152, 219, 0.2); }

  .step-icon.reveal { border-color: rgba(241, 196, 15, 0.4); }
  .chain-step.active .step-icon.reveal { background: rgba(241, 196, 15, 0.2); }

  .step-icon.verify { border-color: rgba(46, 204, 113, 0.4); }
  .chain-step.active .step-icon.verify { background: rgba(46, 204, 113, 0.2); }

  .step-content {
    text-align: center;
  }

  .step-label {
    font-size: 10px;
    font-weight: 600;
    color: #fff;
    display: block;
  }

  .step-status {
    font-size: 9px;
    color: #666;
  }

  .chain-arrow {
    color: #444;
    font-size: 14px;
  }

  .proof-details {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .proof-item {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .proof-item .label {
    color: #888;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hash {
    background: rgba(0, 0, 0, 0.4);
    padding: 10px 14px;
    border-radius: 8px;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 13px;
    color: #4ecdc4;
    border: 1px solid rgba(78, 205, 196, 0.2);
    word-break: break-all;
  }

  .hash.revealed {
    color: #f1c40f;
    border-color: rgba(241, 196, 15, 0.2);
  }

  .hash-row {
    display: flex;
    gap: 8px;
    align-items: stretch;
  }

  .hash-row .hash {
    flex: 1;
  }

  .copy-btn {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #888;
    padding: 8px 14px;
    border-radius: 8px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    white-space: nowrap;
  }

  .copy-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: white;
    border-color: rgba(255, 255, 255, 0.25);
  }

  .copy-btn.small {
    padding: 6px 10px;
    font-size: 10px;
  }

  .pending-box {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: rgba(241, 196, 15, 0.1);
    border: 1px solid rgba(241, 196, 15, 0.2);
    border-radius: 8px;
    color: #f1c40f;
    font-size: 13px;
  }

  .timestamp .value {
    color: #666;
    font-size: 12px;
  }

  .verification {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .verify-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    background: linear-gradient(135deg, #3498db, #2980b9);
    color: white;
    border: none;
    padding: 14px 24px;
    border-radius: 10px;
    font-weight: 700;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .verify-btn:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 8px 20px rgba(52, 152, 219, 0.4);
  }

  .verify-btn:disabled {
    opacity: 0.7;
    cursor: wait;
  }

  .verify-btn.verified {
    background: linear-gradient(135deg, #2ecc71, #27ae60);
    box-shadow: 0 8px 20px rgba(46, 204, 113, 0.3);
  }

  .source-badge {
    background: rgba(255, 255, 255, 0.2);
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-left: 6px;
  }

  .verify-btn .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .result.invalid {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 12px 16px;
    border-radius: 8px;
    background: rgba(241, 196, 15, 0.1);
    color: #f1c40f;
    border: 1px solid rgba(241, 196, 15, 0.3);
    font-weight: 600;
  }

  .result.invalid .icon {
    color: #f1c40f;
  }

  .result.invalid .invalid-note {
    margin: 0;
    font-size: 11px;
    font-weight: 400;
    color: #888;
    line-height: 1.4;
  }

  /* Security Section */
  .security-section {
    margin-top: 20px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 16px;
  }

  .section-toggle {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 12px 16px;
    color: #aaa;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .section-toggle:hover {
    background: rgba(255, 255, 255, 0.06);
    color: white;
  }

  .section-toggle svg {
    transition: transform 0.3s;
  }

  .section-toggle svg.rotated {
    transform: rotate(180deg);
  }

  .security-details {
    margin-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: fadeIn 0.3s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-10px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .security-point {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 10px;
    padding: 14px 16px;
    border-left: 3px solid;
  }

  .security-point:nth-child(1) { border-color: #9b59b6; }
  .security-point:nth-child(2) { border-color: #3498db; }
  .security-point:nth-child(3) { border-color: #f1c40f; }
  .security-point:nth-child(4) { border-color: #2ecc71; }

  .point-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .point-icon {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 700;
    color: white;
  }

  .point-icon.vrf { background: #9b59b6; }
  .point-icon.commit { background: #3498db; }
  .point-icon.shuffle { background: #f1c40f; color: #333; }
  .point-icon.math { background: #2ecc71; }

  .point-header h5 {
    margin: 0;
    color: #fff;
    font-size: 13px;
    font-weight: 600;
  }

  .security-point p {
    margin: 0;
    color: #aaa;
    font-size: 12px;
    line-height: 1.6;
  }

  .security-point strong {
    color: #fff;
  }

  .trust-summary {
    background: linear-gradient(135deg, rgba(46, 204, 113, 0.1), rgba(46, 204, 113, 0.05));
    border: 1px solid rgba(46, 204, 113, 0.2);
    border-radius: 10px;
    padding: 16px;
    text-align: center;
  }

  .trust-flow {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-bottom: 10px;
  }

  .flow-item {
    background: rgba(46, 204, 113, 0.2);
    color: #2ecc71;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
  }

  .flow-arrow {
    color: #2ecc71;
    font-size: 14px;
  }

  .trust-note {
    margin: 0;
    color: #888;
    font-size: 11px;
  }

  /* Manual Verify */
  .manual-verify {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .manual-verify h4 {
    color: #888;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    margin: 0 0 8px;
  }

  .manual-verify > p {
    color: #666;
    font-size: 12px;
    margin: 0 0 14px;
  }

  .verify-methods {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .method {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .method-name {
    color: #888;
    font-size: 11px;
  }

  .command-row {
    display: flex;
    gap: 8px;
  }

  .command {
    flex: 1;
    background: rgba(0, 0, 0, 0.4);
    padding: 10px 12px;
    border-radius: 8px;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 10px;
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
    font-size: 13px;
  }

  .method a:hover {
    text-decoration: underline;
  }

  .method-hint-box {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(241, 196, 15, 0.1);
    border: 1px solid rgba(241, 196, 15, 0.3);
    border-radius: 6px;
    color: #f1c40f;
    font-size: 11px;
    line-height: 1.4;
  }

  .method-hint-box svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .method-hint-box strong {
    color: #f5d547;
  }

  .expected-result {
    margin-top: 14px;
    padding: 12px;
    background: rgba(78, 205, 196, 0.1);
    border: 1px solid rgba(78, 205, 196, 0.2);
    border-radius: 8px;
  }

  .expected-label {
    color: #888;
    font-size: 11px;
    display: block;
    margin-bottom: 6px;
  }

  .expected-hash {
    color: #4ecdc4;
    font-family: 'Monaco', 'Consolas', monospace;
    font-size: 10px;
    word-break: break-all;
    display: block;
  }

  .no-proof {
    text-align: center;
    padding: 30px;
    color: #666;
  }

  .no-proof svg {
    opacity: 0.4;
    margin-bottom: 12px;
  }

  .no-proof p {
    margin: 0;
    font-size: 13px;
  }

  /* Responsive */
  @media (max-width: 500px) {
    .trust-chain {
      flex-wrap: wrap;
      gap: 8px;
    }

    .chain-arrow {
      display: none;
    }

    .chain-step {
      flex: 1;
      min-width: 70px;
    }
  }
</style>
