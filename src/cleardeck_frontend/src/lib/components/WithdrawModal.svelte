<script>
  import IcpLogo from './IcpLogo.svelte';

  const { tableActor, currentBalance, onClose, onWithdrawSuccess, currency = 'ICP' } = $props();

  let withdrawAmount = $state('');
  let processing = $state(false);
  let error = $state(null);
  let success = $state(null);
  let inputUnit = $state('sats'); // 'sats' or 'btc' for BTC input mode

  // Currency-specific settings
  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';
  const minWithdrawal = isBTC ? 11n : 100000n; // 11 sats (fee is 10) vs 0.001 ICP

  // Format balance for display
  function formatBalance(smallestUnit) {
    if (!smallestUnit) return '0';
    const num = Number(smallestUnit);
    if (isBTC) {
      const btc = num / 100_000_000;
      if (btc >= 1) return `${btc.toFixed(4)} BTC`;
      if (num >= 1000) return `${(num / 1000).toFixed(1)}K sats`;
      return `${num} sats`;
    }
    return (num / 100_000_000).toFixed(4);
  }

  // Format with unit
  function formatWithUnit(smallestUnit) {
    const formatted = formatBalance(smallestUnit);
    if (!formatted.includes('BTC') && !formatted.includes('sats') && !formatted.includes('ICP')) {
      return `${formatted} ${currencySymbol}`;
    }
    return formatted;
  }

  // Convert user input to smallest unit
  function inputToSmallestUnit(amount) {
    if (isBTC && inputUnit === 'sats') {
      return BigInt(Math.floor(Number(amount)));
    }
    return BigInt(Math.floor(Number(amount) * 100_000_000));
  }

  async function handleWithdraw() {
    if (!withdrawAmount || Number(withdrawAmount) <= 0) {
      error = 'Please enter a valid amount';
      return;
    }

    const amountSmallest = inputToSmallestUnit(withdrawAmount);

    // Minimum withdrawal check
    if (amountSmallest < minWithdrawal) {
      const minDisplay = isBTC ? '1,000 sats' : '0.001 ICP';
      error = `Minimum withdrawal is ${minDisplay}`;
      return;
    }

    if (amountSmallest > BigInt(currentBalance || 0)) {
      error = 'Insufficient balance';
      return;
    }

    processing = true;
    error = null;
    success = null;

    try {
      const result = await tableActor.withdraw(amountSmallest);
      if ('Ok' in result) {
        success = `Withdrawal successful! ${formatWithUnit(result.Ok)} sent to your wallet.`;
        setTimeout(() => {
          onWithdrawSuccess?.();
          onClose();
        }, 2000);
      } else if ('Err' in result) {
        error = result.Err;
      }
    } catch (e) {
      error = e.message || 'Withdrawal failed';
    }
    processing = false;
  }

  function setMaxAmount() {
    // Withdraw full balance - the transfer fee is deducted from the amount sent, not from balance
    const maxSmallest = currentBalance || 0;
    if (isBTC && inputUnit === 'sats') {
      withdrawAmount = String(maxSmallest);
    } else {
      const maxDisplay = maxSmallest / 100_000_000;
      withdrawAmount = isBTC ? maxDisplay.toFixed(8) : maxDisplay.toFixed(4);
    }
  }
</script>

<div class="modal-backdrop" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="button" tabindex="-1" aria-label="Close modal"></div>

<div class="modal-content" class:btc-modal={isBTC} role="dialog" aria-labelledby="withdraw-modal-title">
  <div class="modal-header">
    <h2 id="withdraw-modal-title">
      {#if isBTC}
        <svg width="20" height="20" viewBox="0 0 64 64">
          <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
        </svg>
      {:else}
        <IcpLogo size={20} />
      {/if}
      Withdraw {currencySymbol}
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close withdraw modal">×</button>
  </div>

  <div class="modal-body">
    <div class="balance-info" class:btc={isBTC}>
      <span class="label">Available Balance</span>
      <span class="amount" class:btc={isBTC}>{formatWithUnit(currentBalance)}</span>
    </div>

    <div class="form-section">
      <div class="label-row">
        <label for="withdraw-amount">Withdraw Amount</label>
        {#if isBTC}
          <div class="unit-toggle">
            <button
              class:active={inputUnit === 'sats'}
              onclick={() => { inputUnit = 'sats'; withdrawAmount = ''; }}
            >sats</button>
            <button
              class:active={inputUnit === 'btc'}
              onclick={() => { inputUnit = 'btc'; withdrawAmount = ''; }}
            >BTC</button>
          </div>
        {/if}
      </div>
      <div class="input-with-max">
        <input
          id="withdraw-amount"
          type="number"
          step={isBTC ? (inputUnit === 'sats' ? "1" : "0.00000001") : "0.0001"}
          min={isBTC ? (inputUnit === 'sats' ? "1000" : "0.00001") : "0.001"}
          placeholder={isBTC ? (inputUnit === 'sats' ? "1000" : "0.00000000") : "0.0000"}
          bind:value={withdrawAmount}
          disabled={processing}
        />
        <span class="input-suffix" class:btc={isBTC}>{isBTC ? inputUnit : 'ICP'}</span>
        <button class="max-btn" class:btc={isBTC} onclick={setMaxAmount} disabled={processing}>
          MAX
        </button>
      </div>
      <p class="hint">
        {#if isBTC}
          Minimum: 1,000 sats (Fee: 10 sats)
        {:else}
          Minimum withdrawal: 0.001 ICP. A small network fee applies.
        {/if}
      </p>
    </div>

    {#if error}
      <div class="alert error">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="8" x2="12" y2="12"/>
          <line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        {error}
      </div>
    {/if}

    {#if success}
      <div class="alert success">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
          <polyline points="22 4 12 14.01 9 11.01"/>
        </svg>
        {success}
      </div>
    {/if}

    <div class="actions">
      <button class="btn-secondary" onclick={onClose} disabled={processing}>
        Cancel
      </button>
      <button
        class="btn-primary"
        onclick={handleWithdraw}
        disabled={processing || !withdrawAmount || Number(withdrawAmount) <= 0}
      >
        {#if processing}
          <span class="spinner"></span>
          Processing...
        {:else}
          Withdraw
        {/if}
      </button>
    </div>

    <div class="info-box" class:btc={isBTC}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10"/>
        <line x1="12" y1="16" x2="12" y2="12"/>
        <line x1="12" y1="8" x2="12.01" y2="8"/>
      </svg>
      {#if isBTC}
        <p>
          ckBTC (Bitcoin on ICP) will be sent to your wallet. You can convert it to real BTC
          through the NNS or use it directly on ICP apps.
        </p>
      {:else}
        <p>
          ICP will be sent to your authenticated principal's default account.
          Make sure you're logged in with the correct wallet.
        </p>
      {/if}
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
    z-index: 200;
  }

  .modal-content {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 90%;
    max-width: 440px;
    background: linear-gradient(145deg, rgba(25, 25, 40, 0.98), rgba(15, 15, 25, 0.98));
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 25px 80px rgba(0, 0, 0, 0.5);
    z-index: 201;
    max-height: 90vh;
    overflow-y: auto;
  }

  .modal-content.btc-modal {
    border-color: rgba(247, 147, 26, 0.3);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .modal-header h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 18px;
    color: #fff;
  }

  .close-btn {
    background: none;
    border: none;
    color: #888;
    font-size: 28px;
    cursor: pointer;
    padding: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .modal-body {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .balance-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 20px;
    background: rgba(0, 212, 170, 0.05);
    border: 1px solid rgba(0, 212, 170, 0.2);
    border-radius: 12px;
  }

  .balance-info.btc {
    background: rgba(247, 147, 26, 0.05);
    border-color: rgba(247, 147, 26, 0.2);
  }

  .balance-info .label {
    color: #888;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .balance-info .amount {
    color: #00d4aa;
    font-size: 28px;
    font-weight: 700;
  }

  .balance-info .amount.btc {
    color: #f7931a;
  }

  .form-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .label-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  label {
    color: #888;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-weight: 600;
  }

  .unit-toggle {
    display: flex;
    gap: 2px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 6px;
    padding: 2px;
  }

  .unit-toggle button {
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    background: transparent;
    border: none;
    color: #666;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .unit-toggle button.active {
    background: rgba(247, 147, 26, 0.3);
    color: #f7931a;
  }

  .unit-toggle button:hover:not(.active) {
    color: #999;
  }

  .input-with-max {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .input-suffix {
    font-size: 14px;
    font-weight: 600;
    color: #666;
    min-width: 40px;
  }

  .input-suffix.btc {
    color: #f7931a;
  }

  input {
    flex: 1;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 12px;
    color: white;
    font-size: 16px;
    transition: all 0.2s;
  }

  input:focus {
    outline: none;
    border-color: rgba(0, 212, 170, 0.5);
    background: rgba(0, 0, 0, 0.4);
  }

  input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .max-btn {
    background: rgba(0, 212, 170, 0.1);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
    padding: 12px 16px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
  }

  .max-btn.btc {
    background: rgba(247, 147, 26, 0.1);
    border-color: rgba(247, 147, 26, 0.3);
    color: #f7931a;
  }

  .max-btn:hover:not(:disabled) {
    background: rgba(0, 212, 170, 0.2);
  }

  .max-btn.btc:hover:not(:disabled) {
    background: rgba(247, 147, 26, 0.2);
  }

  .max-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hint {
    color: #666;
    font-size: 11px;
    margin: 0;
    line-height: 1.4;
  }

  .alert {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px;
    border-radius: 8px;
    font-size: 13px;
  }

  .alert.error {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #ef4444;
  }

  .alert.success {
    background: rgba(0, 212, 170, 0.15);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
  }

  .actions {
    display: flex;
    gap: 12px;
  }

  .btn-primary, .btn-secondary {
    flex: 1;
    padding: 12px 20px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border: none;
  }

  .btn-primary {
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    color: white;
  }

  .btn-primary:hover:not(:disabled) {
    background: linear-gradient(135deg, #fbbf24 0%, #f59e0b 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(245, 158, 11, 0.3);
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #888;
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .btn-primary:disabled, .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .info-box {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    background: rgba(100, 100, 120, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    color: #888;
  }

  .info-box svg {
    flex-shrink: 0;
    margin-top: 2px;
  }

  .info-box p {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
