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

  // >>> MIRRORED-LIMITS-BEGIN  (tests/money_safety/tests/ui_limits.rs reads this fence)
  // ===========================================================================
  // MIRRORED CANISTER LIMITS -- THE ONLY NUMBERS IN THIS FILE (docs/DEFECTS.md T-26)
  // ===========================================================================
  //
  // Every limit this modal states or enforces is derived from this block, so no
  // surface can drift from the constant the canister actually applies. That drift
  // is what T-26 was: the enforced BTC floor was 11 sats while the copy, the
  // input's `min` attribute and the error string all said 1,000, which made
  // `Minimum withdrawal is 1,000 sats` UNREACHABLE for every amount from 12 to 999
  // and gave a player in that band a rejection they could not explain.
  //
  // WHICH NUMBER IS RIGHT: 11. Three reasons, in order.
  //
  //  1. The canister is the enforcement. `withdraw()` compares against
  //     `Currency::min_withdrawal()` and nothing else does; a player calling the
  //     canister directly gets 11 whatever this file says. A UI that states a
  //     floor the canister does not apply is lying in the only direction that
  //     matters, and it is the direction that produces unreachable error strings.
  //  2. Enforcing 1,000 client-side would TRAP DUST. A BTC player who loses down
  //     to 400 sats has exactly one exit -- `withdraw` -- and a UI-only floor of
  //     1,000 closes it permanently. The canister would pay them 390 sats.
  //     Choosing the higher number costs a player their remaining balance.
  //  3. Raising BTC_MIN_WITHDRAWAL_AMOUNT to 1,000 in the canister would be the
  //     same trap, written into a canister that custodies real funds, and it is
  //     not a change to make from a frontend pass.
  //
  // The real complaint behind "1,000" is sound and is answered honestly instead of
  // by a false floor: 11 sats nets 1 sat, because the 10-sat fee is most of it. So
  // the modal now states the fee, computes the NET the wallet will receive from
  // whatever is typed, and warns when the fee takes more than half. A player can
  // still withdraw 11 sats; they can no longer be surprised by what arrives.
  //
  // THE SAME ARGUMENT, ONE CURRENCY OVER (docs/SECURITY-FINDINGS.md FINDING 27).
  // Reason 2 above was written about BTC and was true about ICP the whole time,
  // in the canister rather than in this file: `deposit()` accepted 20,000 e8s and
  // `withdraw()` refused anything under 100,000, so an ICP player holding
  // anything in between -- including a player who deposited exactly the
  // advertised minimum -- had no exit at all. The canister's two floors are now
  // one number per currency and the invariant is compile-time asserted there
  // ("THE FLOOR INVARIANT" in lib.rs). This file mirrors the result.
  //
  // src/table_canister/src/lib.rs -- MIRRORED, keep in step:
  //   :36 ICP_TRANSFER_FEE          10_000          (0.0001 ICP)
  //   :40 CKBTC_TRANSFER_FEE        10              (10 sats)
  //   :47 ICP_MAX_WITHDRAWAL_PER_TX 10_000_000_000  (100 ICP)
  //   :48 ICP_MIN_WITHDRAWAL_AMOUNT 20_000          (0.0002 ICP == the deposit floor)
  //   :51 BTC_MAX_WITHDRAWAL_PER_TX 10_000_000      (0.1 BTC)
  //   :52 BTC_MIN_WITHDRAWAL_AMOUNT 11              (fee + 1 sat)
  //   :54 WITHDRAWAL_COOLDOWN_NS    60_000_000_000  (60 s)
  // `tests/money_safety/tests/ui_limits.rs` reads both files and fails if any of
  // these five numbers stops matching, and fails if any surface in this file states
  // a limit as a literal instead of interpolating one of the values below.
  const MIN_WITHDRAWAL = isBTC ? 11n : 20_000n;
  const MAX_WITHDRAWAL = isBTC ? 10_000_000n : 10_000_000_000n;
  const TRANSFER_FEE = isBTC ? 10n : 10_000n;
  const WITHDRAWAL_COOLDOWN_SECS = 60;

  // The canister sends `amount - fee` (lib.rs:1038), so TRANSFER_FEE is what the
  // wallet does NOT receive.
  const transferFee = TRANSFER_FEE;

  /** Exact, never rounded: a limit rendered with `toFixed` is a limit that lies. */
  function formatExact(smallestUnit) {
    const v = BigInt(smallestUnit);
    if (isBTC) return `${v.toLocaleString('en-US')} sats`;
    const whole = v / 100_000_000n;
    const frac = (v % 100_000_000n).toString().padStart(8, '0').replace(/0+$/, '');
    return frac ? `${whole}.${frac} ICP` : `${whole} ICP`;
  }

  /**
   * A balance as a BigInt of smallest units, tolerating whatever the caller has.
   * `myBalance` arrives as `Number(await get_balance())` and `BigInt()` throws on a
   * non-integer Number, so the floor is not decoration.
   */
  function toSmallest(value) {
    if (typeof value === 'bigint') return value;
    const n = Number(value);
    return Number.isFinite(n) && n > 0 ? BigInt(Math.floor(n)) : 0n;
  }

  /** The same value as a bare decimal, for an input's `min` / `max` attribute. */
  function formatPlain(smallestUnit) {
    const v = BigInt(smallestUnit);
    const whole = v / 100_000_000n;
    const frac = (v % 100_000_000n).toString().padStart(8, '0').replace(/0+$/, '');
    return frac ? `${whole}.${frac}` : `${whole}`;
  }

  const minDisplay = formatExact(MIN_WITHDRAWAL);
  const maxDisplay = formatExact(MAX_WITHDRAWAL);
  const feeDisplay = formatExact(TRANSFER_FEE);

  // The attributes track the unit toggle, so the browser's own validation agrees
  // with the canister in whichever unit the player is typing.
  const inputMinAttr = $derived(
    isBTC && inputUnit === 'sats' ? MIN_WITHDRAWAL.toString() : formatPlain(MIN_WITHDRAWAL)
  );
  const inputMaxAttr = $derived(
    isBTC && inputUnit === 'sats' ? MAX_WITHDRAWAL.toString() : formatPlain(MAX_WITHDRAWAL)
  );

  // What the wallet will actually receive, from whatever is typed right now.
  const enteredSmallest = $derived.by(() => {
    if (!withdrawAmount || !Number.isFinite(Number(withdrawAmount))) return null;
    const n = Number(withdrawAmount);
    if (n <= 0) return null;
    return inputToSmallestUnit(withdrawAmount);
  });
  const netReceived = $derived(
    enteredSmallest === null
      ? null
      : enteredSmallest > TRANSFER_FEE
        ? enteredSmallest - TRANSFER_FEE
        : 0n
  );
  /** True when the network fee eats at least half of what the player asked for. */
  const feeDominates = $derived(
    enteredSmallest !== null && enteredSmallest > 0n && TRANSFER_FEE * 2n >= enteredSmallest
  );
  // <<< MIRRORED-LIMITS-END

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

  /**
   * Exact decimal text -> smallest units, WITHOUT going through a float.
   *
   * `Number('0.00012345') * 100_000_000` is `12344.999999999998`, which floors to
   * 12,344 -- one unit less than the player has. That is invisible at ordinary
   * sizes and fatal at the bottom: MAX writes the balance out as exact decimal
   * text, and if reading it back lands one unit short then the amount is no
   * longer the WHOLE balance, the sweep below does not recognise it, and a
   * sub-floor balance becomes unwithdrawable again by rounding alone.
   */
  function decimalToSmallest(text) {
    const m = /^\s*(\d*)(?:\.(\d*))?\s*$/.exec(String(text));
    if (!m || (m[1] === '' && (m[2] ?? '') === '')) return 0n;
    const whole = m[1] === '' ? '0' : m[1];
    const frac = (m[2] ?? '').padEnd(8, '0').slice(0, 8);
    return BigInt(whole) * 100_000_000n + BigInt(frac);
  }

  // Convert user input to smallest unit
  function inputToSmallestUnit(amount) {
    if (isBTC && inputUnit === 'sats') {
      return BigInt(Math.floor(Number(amount)));
    }
    return decimalToSmallest(amount);
  }

  async function handleWithdraw() {
    if (!withdrawAmount || Number(withdrawAmount) <= 0) {
      error = 'Please enter a valid amount';
      return;
    }

    const amountSmallest = inputToSmallestUnit(withdrawAmount);
    const balanceSmallest = toSmallest(currentBalance);

    // THE WHOLE-BALANCE SWEEP, MIRRORED (docs/SECURITY-FINDINGS.md FINDING 27).
    //
    // `withdraw()` waives its floor for one request: "send me everything I have
    // left", at any size the ledger can move. That waiver is the thing that stops
    // a floor from becoming a trap for a balance the pot produced rather than the
    // deposit door. A client-side floor that did not mirror the waiver would put
    // the trap straight back, one layer up, where it is invisible to every
    // canister-side gate -- which is the shape of defect this project keeps
    // finding. So the modal waives it on exactly the same condition.
    const sweepingWholeBalance =
      amountSmallest === balanceSmallest && amountSmallest > TRANSFER_FEE;

    // Both bounds, in the same words the canister uses, from the same numbers.
    if (amountSmallest < MIN_WITHDRAWAL && !sweepingWholeBalance) {
      error =
        `Minimum withdrawal is ${minDisplay}. Your whole remaining balance can always be `
        + `withdrawn in one call whatever its size, as long as it is more than the `
        + `${feeDisplay} network fee -- press MAX.`;
      return;
    }
    // The canister enforces a per-transaction ceiling too and the modal never
    // mentioned it, so a player with a large balance pressed MAX and got a
    // rejection out of nowhere. Stating it here cannot trap funds: the balance
    // comes out in successive withdrawals.
    if (amountSmallest > MAX_WITHDRAWAL) {
      error = `Maximum withdrawal per transaction is ${maxDisplay}`;
      return;
    }

    if (amountSmallest > balanceSmallest) {
      error = 'Insufficient balance';
      return;
    }

    processing = true;
    error = null;
    success = null;

    try {
      const result = await tableActor.withdraw(amountSmallest);
      if ('Ok' in result) {
        // `withdraw` returns the LEDGER BLOCK INDEX, not an amount
        // (src/table_canister/src/lib.rs:1894 `-> Result<u64, String>`, `Ok(block)`
        // at :1991). This line used to run it through formatWithUnit and print it
        // as ICP, so the confirmation stated a wrong amount of money — a block
        // index of 4,000 renders as "0.0000 ICP sent to your wallet". docs/DEFECTS.md T-18.
        //
        // What actually happens: `transfer_tokens` sends `amount - fee`
        // (lib.rs:1038), so the wallet receives LESS than the amount withdrawn and
        // nothing on screen said so. Both figures are now stated, and the block
        // index is labelled as what it is.
        const received = amountSmallest > transferFee ? amountSmallest - transferFee : 0n;
        success = `Withdrew ${formatWithUnit(amountSmallest)}. `
          + `${formatWithUnit(received)} reached your wallet after the `
          + `${formatWithUnit(transferFee)} network fee. Ledger block #${result.Ok}.`;
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
    // MAX must be a number the canister will ACCEPT, which means two corrections
    // the old three-liner did not make.
    //
    //  1. It is capped at MAX_WITHDRAWAL. The old version put the whole balance in
    //     the box, so a player holding more than the per-transaction ceiling got
    //     "Maximum withdrawal per transaction is ..." from the canister after
    //     pressing a button labelled MAX.
    //  2. It FLOORS instead of rounding. `toFixed(4)` on an ICP balance rounds to
    //     the nearest 0.0001 ICP, i.e. UPWARDS half the time: a balance of
    //     123,456,789 e8s became "1.2346", which is 123,460,000 e8s -- 3,211 e8s
    //     MORE than the player has -- and the modal then refused its own MAX with
    //     "Insufficient balance".
    const balance = toSmallest(currentBalance);
    const capped = balance > MAX_WITHDRAWAL ? MAX_WITHDRAWAL : balance;
    if (isBTC && inputUnit === 'sats') {
      withdrawAmount = capped.toString();
      return;
    }
    // 8 decimal places is the full precision of a smallest unit, so this is exact
    // for both currencies and can never exceed `capped`.
    withdrawAmount = formatPlain(capped);
  }

  // =========================================================================
  // MONEY OF YOURS THAT IS NOT IN THIS BALANCE (docs/SECURITY-FINDINGS.md
  // FINDING 18)
  // =========================================================================
  //
  // "Available Balance" above is `get_balance()`, and `get_balance()` answers
  // exactly one question: what can I withdraw right now. An auditor read it as
  // zero, withdrew "everything", and left -- while 2.98 ICP of theirs sat in a
  // pot on a table they had already cashed out of. The number was right. It was
  // never the whole answer, and this screen is the last one a leaving player
  // looks at.
  //
  // `get_custody_status()` is the whole answer. It is a query, so it costs
  // nothing and still answers when every update is failing, and it is
  // caller-scoped, so this cannot show the wrong person's money.
  let custody = $state(null);
  let custodyError = $state(null);
  let recovering = $state(false);

  async function loadCustody() {
    if (!tableActor?.get_custody_status) {
      // An older canister build: say so rather than silently show nothing. A
      // missing surface and a zero stake must never look the same.
      custodyError = 'This table cannot report money committed to a pot (older canister build).';
      return;
    }
    try {
      const s = await tableActor.get_custody_status();
      custody = {
        committed: BigInt(s.committed_in_pot ?? 0n),
        stuck: !!s.committed_is_stuck,
        total: BigInt(s.total ?? 0n),
        chips: BigInt(s.chips_at_table ?? 0n),
        advice: s.advice || '',
        abandonableInSecs:
          Array.isArray(s.abandonable_in_ns) && s.abandonable_in_ns.length
            ? Number(s.abandonable_in_ns[0] / 1_000_000_000n)
            : null
      };
      custodyError = null;
    } catch (e) {
      custodyError = e.message || 'Could not read what this table is holding for you.';
    }
  }

  // Follow the canister's own advice, from the withdrawal screen, with one press.
  async function recoverStuckPot() {
    if (!tableActor?.abandon_stuck_hand) return;
    recovering = true;
    error = null;
    try {
      const result = await tableActor.abandon_stuck_hand();
      if ('Err' in result) {
        error = result.Err;
      } else {
        success = `Recovered ${formatWithUnit(custody?.committed ?? 0n)} from the stuck hand. `
          + `It is in your withdrawable balance now.`;
        // The pot is gone and the balance has moved: re-read both.
        await loadCustody();
        onWithdrawSuccess?.();
      }
    } catch (e) {
      error = e.message || 'Recovery failed';
    }
    recovering = false;
  }

  $effect(() => {
    loadCustody();
  });

  // ONE dismissal contract for every dialog in this app (docs/DEFECTS.md T-13).
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="modal-backdrop" onclick={onClose} role="presentation"></div>

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
    <!-- THE FOUR PROTECTED NOTICES, INSIDE THE DIALOG (HARD RULE 2, docs/DEFECTS.md T-31).
         Measured, not assumed: with either money modal open, all four notices in
         the page banner are behind `.modal-backdrop` -- `rgba(0,0,0,0.7)` plus
         `backdrop-filter: blur(4px)` at z-index 200 -- so `elementFromPoint` at the
         centre of each returns the backdrop and not the text. 0 of 4 unobstructed,
         at 1440x900 AND at 390x844, and identical on `fe72d46`, so the scrim is not
         new. It is still a player who cannot read the warning on the one screen
         where they are about to move real money.
         Raising the banner above the backdrop is not this file's to do (the banner
         lives in routes/+page.svelte and src/index.scss). Restating the notices
         INSIDE the dialog is: more prominent is always allowed, it needs nothing
         outside these two components, and it holds whatever any backdrop does. -->
    <!-- WAVE 5 COHERENCE PASS: see the same note in DepositModal.svelte. The last
         clause states the no-rake property in the weaker wording, so the repo's own
         notice gate read 4 of 5 with this dialog open at both viewports. The
         canonical sentence is added verbatim below it; nothing already here moved. -->
    <p class="player-notice">
      <strong>Unaudited code with known bugs: your funds are NOT safe.</strong>
      Online gambling is illegal in many jurisdictions. 18+ only.
      No middleman, no house, 0% rake.
      No rake is taken from any pot on any table.
    </p>
    <div class="balance-info" class:btc={isBTC}>
      <span class="label">Available Balance</span>
      <span class="amount" class:btc={isBTC}>{formatWithUnit(currentBalance)}</span>
    </div>

    <!-- MONEY OF YOURS THAT IS NOT IN THE FIGURE ABOVE.
         docs/SECURITY-FINDINGS.md FINDING 18. Rendered IN FLOW, directly under the
         balance it corrects, and never as an overlay: nothing in this app may
         cover the four notices above (HARD RULE 2). -->
    {#if custody && custody.committed > 0n}
      <div class="committed-stake" class:stuck={custody.stuck}>
        <div class="committed-headline">
          <span class="committed-label">
            {custody.stuck ? 'In a pot nobody can win' : 'In a pot right now'}
          </span>
          <span class="committed-amount">{formatWithUnit(custody.committed)}</span>
        </div>
        <p class="committed-advice">{custody.advice}</p>
        {#if custody.stuck}
          <button class="recover-btn" onclick={recoverStuckPot} disabled={recovering || processing}>
            {recovering ? 'Recovering…' : `Recover ${formatWithUnit(custody.committed)} now`}
          </button>
        {:else if custody.abandonableInSecs !== null}
          <p class="committed-countdown">
            If the table stops moving, this becomes recoverable in about
            {Math.max(0, Math.ceil(custody.abandonableInSecs / 60))} min.
          </p>
        {/if}
      </div>
    {:else if custodyError}
      <p class="committed-unknown">{custodyError}</p>
    {/if}

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
        <!-- `step` is the full precision of a smallest unit, not a coarser grid the
             canister never asked for: at step 0.0001 the box could not express an
             exact ICP balance, so MAX was unrepresentable in its own input. -->
        <input
          id="withdraw-amount"
          type="number"
          step={isBTC && inputUnit === 'sats' ? "1" : "0.00000001"}
          min={inputMinAttr}
          max={inputMaxAttr}
          placeholder={inputMinAttr}
          bind:value={withdrawAmount}
          disabled={processing}
        />
        <span class="input-suffix" class:btc={isBTC}>{isBTC ? inputUnit : 'ICP'}</span>
        <button class="max-btn" class:btc={isBTC} onclick={setMaxAmount} disabled={processing}>
          MAX
        </button>
      </div>
      <!-- Every figure here is interpolated from the mirrored constants. A literal
           in this block is what docs/DEFECTS.md T-26 was, and ui_limits.rs fails
           the build if one comes back. -->
      <p class="hint">
        Minimum {minDisplay}, maximum {maxDisplay} per transaction. The network fee
        is {feeDisplay} and is taken out of what you withdraw, so your wallet
        receives that much less. One withdrawal every {WITHDRAWAL_COOLDOWN_SECS} seconds.
      </p>
      <p class="hint">
        Whatever your balance is, all of it can leave in one call: press MAX. That
        works below the stated floor too, as long as what is left is more than the
        network fee.
      </p>
      {#if netReceived !== null}
        <p class="net-line" class:dust={feeDominates}>
          Your wallet receives <strong>{formatWithUnit(netReceived)}</strong>
          after the {feeDisplay} fee.
          {#if feeDominates}
            The fee is most of this withdrawal.
          {/if}
        </p>
      {/if}
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

  /* MONEY OF YOURS THAT IS NOT IN THE BALANCE ABOVE.
     docs/SECURITY-FINDINGS.md FINDING 18. Deliberately in the document flow, with
     no `position: fixed/absolute` and no z-index, so it can never cover the four
     notices at the top of this dialog (HARD RULE 2). */
  .committed-stake {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    background: rgba(255, 184, 0, 0.08);
    border: 1px solid rgba(255, 184, 0, 0.45);
    border-radius: 12px;
  }

  .committed-stake.stuck {
    background: rgba(255, 92, 92, 0.1);
    border-color: rgba(255, 92, 92, 0.6);
  }

  .committed-headline {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }

  .committed-label {
    color: #ffb800;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .committed-stake.stuck .committed-label {
    color: #ff7676;
  }

  .committed-amount {
    color: #fff;
    font-size: 20px;
    font-weight: 700;
  }

  .committed-advice {
    margin: 0;
    color: #ddd;
    font-size: 13px;
    line-height: 1.45;
  }

  .committed-countdown {
    margin: 0;
    color: #999;
    font-size: 12px;
  }

  .recover-btn {
    padding: 10px 14px;
    border: 1px solid rgba(255, 92, 92, 0.8);
    border-radius: 8px;
    background: rgba(255, 92, 92, 0.18);
    color: #fff;
    font-size: 14px;
    font-weight: 700;
    cursor: pointer;
  }

  .recover-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .committed-unknown {
    margin: 0;
    color: #ffb800;
    font-size: 12px;
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
    color: #8a8a97;
    font-size: 11px;
    margin: 0;
    line-height: 1.5;
  }

  /* The net figure is the one a player checks against their wallet afterwards, so
     it is brighter than the hint above it, not dimmer. */
  .net-line {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: #b9c6c2;
  }

  .net-line strong {
    color: #00d4aa;
  }

  .net-line.dust {
    color: #fbbf24;
  }

  .net-line.dust strong {
    color: #fcd34d;
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

  /* The protected notices, restated inside the dialog. Deliberately NOT dimmed:
     it is the one block in this modal that must not read as fine print. */
  .player-notice {
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: rgba(239, 68, 68, 0.12);
    border: 1px solid rgba(239, 68, 68, 0.35);
    color: #fca5a5;
    font-size: 12px;
    line-height: 1.5;
  }

  .player-notice strong {
    color: #fecaca;
  }
</style>
