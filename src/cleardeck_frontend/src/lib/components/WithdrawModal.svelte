<script>
  // THE WITHDRAW SHEET, BUILT FOR A PLAYER LEAVING WITH THEIR MONEY.
  //
  // The balance first, with its fiat hint; money of yours that is NOT in that
  // figure (in a pot, or stuck) right under it; the amount with MAX; what the
  // wallet receives after the fee, row by row, before the button; where it
  // goes (the account id derived from the signed-in principal, copyable); the
  // one chain commit painted as a step; a receipt with the ledger block and
  // the cooldown counting down. Nothing in the arithmetic or the refusals
  // changed: the whole-balance sweep, the two floors, the ceiling and the
  // exact decimal parse are the same lines they were.

  import IcpLogo from './IcpLogo.svelte';
  import BtcGlyph from './BtcGlyph.svelte';
  import NoticeLine from './NoticeLine.svelte';
  import CashierAlert from './CashierAlert.svelte';
  import { phoneMedia } from '$lib/phone-media.svelte.js';
  import SolvencyNotice from './SolvencyNotice.svelte';
  import { readTableSolvency, refreshTableSolvency } from '$lib/solvency.js';
  import CycleRunwayNotice from './CycleRunwayNotice.svelte';
  import { readCycleRunway } from '$lib/cycleRunway.js';
  import { scrollLock } from '$lib/scroll-lock.js';
  import { auth } from '$lib/auth.js';
  import { IS_MAINNET_BUILD } from '$lib/ic-config.js';
  import { accountIdentifierHex } from '$lib/depositAddress.js';
  import { loadPrices } from '$lib/prices.js';
  import { onMount } from 'svelte';
  import {
    decimalToSmallest, formatCashier, formatExact, formatPlain, formatUsd, toSmallest, usdValue,
    withdrawNet,
  } from '$lib/cashier-format.js';
  import { FLOW, cooldownRemainingSecs, formatCountdown, withdrawSteps } from '$lib/cashier-steps.js';
  import { describeCashierFailure } from '$lib/humane-errors.js';
  import { shortId } from '$lib/lobby-format.js';
  import CashierStepper from './CashierStepper.svelte';
  import CashierSummary from './CashierSummary.svelte';
  import CashierReceipt from './CashierReceipt.svelte';
  import CashierDisclosures from './CashierDisclosures.svelte';

  const {
    tableActor,
    // Only so the cycle-runway banner can print a top-up command that names a
    // canister.
    tableCanisterId = null,
    currentBalance,
    onClose,
    onWithdrawSuccess,
    currency = 'ICP',
  } = $props();

  // The phone sheet keeps the button row in the scroller after the
  // disclosures; the wide dialog puts it in a footer under the scroller.
  const phone = phoneMedia();

  // WHETHER THERE IS ENOUGH ON THE LEDGER TO PAY THIS WITHDRAWAL
  // (docs/SECURITY-FINDINGS.md FINDING 35). `Available balance` is a number the
  // canister keeps for you; it is not evidence that the canister holds it.
  let solvency = $state(null);
  let solvencyRefreshing = $state(false);

  async function loadSolvency() {
    solvency = await readTableSolvency(tableActor);
  }

  async function refreshSolvency() {
    solvencyRefreshing = true;
    try {
      solvency = await refreshTableSolvency(tableActor);
    } finally {
      solvencyRefreshing = false;
    }
  }

  // HOW LONG CAN THIS TABLE KEEP HONOURING WITHDRAWALS? (docs/DEFECTS.md E-55).
  let runway = $state(null);

  async function loadRunway() {
    runway = await readCycleRunway(tableActor);
  }

  let withdrawAmount = $state('');
  let processing = $state(false);
  // A string is this component's own refusal, shown as written; an object is
  // the canister's answer turned into a sentence with the raw text kept.
  let error = $state(null);
  let inputUnit = $state('sats'); // 'sats' or 'btc' for BTC input mode

  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';

  // >>> MIRRORED-LIMITS-BEGIN  (tests/money_safety/tests/ui_limits.rs reads this fence)
  // ===========================================================================
  // MIRRORED CANISTER LIMITS: THE ONLY NUMBERS IN THIS FILE (docs/DEFECTS.md T-26)
  // ===========================================================================
  //
  // Every limit this modal states or enforces is derived from this block, so no
  // surface can drift from the constant the canister actually applies. T-26 was
  // that drift: the enforced BTC floor was 11 sats while the copy said 1,000,
  // which made the modal's own error string unreachable for every amount from
  // 12 to 999 and trapped a player's dust. The canister is the enforcement, so
  // the canister's number is the one stated, and the NET the wallet receives is
  // computed and shown so a player is never surprised by what arrives.
  //
  // THE SAME ARGUMENT, ONE CURRENCY OVER (docs/SECURITY-FINDINGS.md FINDING 27):
  // the deposit floor and the withdrawal floor are one number per currency,
  // compile-time asserted in lib.rs ("THE FLOOR INVARIANT"). This file mirrors it.
  //
  // src/table_canister/src/lib.rs, MIRRORED, keep in step:
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

  // The canister sends `amount - fee` (lib.rs transfer_tokens), so TRANSFER_FEE
  // is what the wallet does NOT receive.
  const transferFee = TRANSFER_FEE;

  const minDisplay = formatExact(MIN_WITHDRAWAL, currencySymbol);
  const maxDisplay = formatExact(MAX_WITHDRAWAL, currencySymbol);
  const feeDisplay = formatExact(TRANSFER_FEE, currencySymbol);

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
  const net = $derived(enteredSmallest === null ? null : withdrawNet({ amount: enteredSmallest, fee: TRANSFER_FEE }));
  const netReceived = $derived(net ? net.net : null);
  /** True when the network fee eats at least half of what the player asked for. */
  const feeDominates = $derived(Boolean(net && net.feeDominates));
  // <<< MIRRORED-LIMITS-END

  const formatWithUnit = (smallestUnit) => formatCashier(smallestUnit, currencySymbol, { placeholder: '0' });

  // Convert user input to smallest unit: the exact decimal parse
  // (lib/cashier-format.js decimalToSmallest), never a float, so MAX reads back
  // as the whole balance to the e8.
  function inputToSmallestUnit(amount) {
    if (isBTC && inputUnit === 'sats') {
      const n = Math.floor(Number(amount));
      return Number.isFinite(n) && n > 0 ? BigInt(n) : 0n;
    }
    return decimalToSmallest(amount);
  }

  // THE FLOW and THE RECEIPT.
  let flow = $state({ phase: FLOW.IDLE, steps: [], current: 0, startedAt: null });
  let receipt = $state(null);
  // When the last withdrawal landed, for the cooldown countdown.
  let lastWithdrawalAt = $state(null);
  let now = $state(Date.now());

  const cooldownLeft = $derived(
    cooldownRemainingSecs({ lastAt: lastWithdrawalAt, now, cooldownSecs: WITHDRAWAL_COOLDOWN_SECS })
  );

  function fail(e) {
    error = describeCashierFailure(e);
  }

  const errorView = $derived(
    error === null ? null : (typeof error === 'string' ? { message: error, detail: null } : error)
  );

  // THE DESTINATION: the signed-in principal's default ledger account, derived
  // here from the principal alone (lib/depositAddress.js accountIdentifierHex),
  // which is the account `withdraw` pays. Shown so a player can compare it
  // with the wallet they expect the money in.
  let authState = $state({ isAuthenticated: false, principal: null });
  $effect(() => {
    const unsub = auth.subscribe((s) => { authState = s; });
    return unsub;
  });

  const destination = $derived.by(() => {
    if (!authState.principal) return null;
    try {
      return { principal: authState.principal, accountId: accountIdentifierHex(authState.principal, null) };
    } catch {
      return null;
    }
  });

  let copiedDestination = $state(false);
  function copyDestination() {
    if (!destination) return;
    navigator.clipboard?.writeText(destination.accountId).catch(() => {});
    copiedDestination = true;
    setTimeout(() => { copiedDestination = false; }, 2000);
  }

  // The fiat hint beside the balance and the net (lib/prices.js, one quote).
  let prices = $state(null);
  const perTokenUsd = $derived(prices ? (isBTC ? prices.btcUsd : prices.icpUsd) : null);
  const balanceUsd = $derived(usdValue(toSmallest(currentBalance), perTokenUsd));
  /** A figure's dollars at the sheet's quote, for the receipt; null without a quote. */
  const fiatOf = (smallestUnit) => {
    const v = usdValue(smallestUnit, perTokenUsd);
    return v === null ? null : formatUsd(v);
  };

  const summaryRows = $derived.by(() => {
    if (!net) return [];
    return [
      { id: 'withdraw', label: 'You withdraw', value: formatWithUnit(net.amount) },
      { id: 'fee', label: 'Network fee', note: 'taken out of what you withdraw', value: formatWithUnit(net.fee), tone: 'muted' },
      {
        id: 'net',
        label: 'Your wallet receives',
        note: net.feeDominates ? 'The fee is most of this withdrawal.' : undefined,
        value: formatWithUnit(net.net),
        strong: true,
        tone: net.feeDominates ? 'warn' : 'money',
      },
    ];
  });

  const primaryLabel = $derived.by(() => {
    if (processing) return 'Sending…';
    if (cooldownLeft > 0) return `Next withdrawal in ${formatCountdown(cooldownLeft)}`;
    return enteredSmallest ? `Withdraw ${formatWithUnit(enteredSmallest)}` : `Withdraw ${currencySymbol}`;
  });

  async function handleWithdraw() {
    if (!withdrawAmount || Number(withdrawAmount) <= 0) {
      error = 'Please enter a valid amount';
      return;
    }

    const amountSmallest = inputToSmallestUnit(withdrawAmount);
    const balanceSmallest = toSmallest(currentBalance);

    // THE WHOLE-BALANCE SWEEP, MIRRORED (docs/SECURITY-FINDINGS.md FINDING 27):
    // `withdraw()` waives its floor for "send me everything I have left", at any
    // size the ledger can move, so the modal waives it on the same condition.
    const sweepingWholeBalance =
      amountSmallest === balanceSmallest && amountSmallest > TRANSFER_FEE;

    // TWO REFUSALS, MIRRORED (docs/DEFECTS.md E-81): a policy floor the player
    // clears by pressing MAX, and the ledger's own fee, which no request clears.
    if (amountSmallest < MIN_WITHDRAWAL && !sweepingWholeBalance) {
      if (balanceSmallest > 0n && balanceSmallest <= TRANSFER_FEE) {
        error =
          `No withdrawal of any size can move this, and that is arithmetic rather than a `
          + `policy of this table. Your whole remaining balance is `
          + `${formatExact(balanceSmallest, currencySymbol)}, and the `
          + `ledger charges a ${feeDisplay} network fee on every transfer, so sending it `
          + `would cost at least as much as the amount. It is not lost: it is counted in `
          + `everything this table reports it holds for you, and if you ever put more in, it `
          + `comes out with the rest in one call.`;
        return;
      }
      error =
        `Minimum withdrawal is ${minDisplay}. Your whole remaining balance can always be `
        + `withdrawn in one call whatever its size, as long as it is more than the `
        + `${feeDisplay} network fee: press MAX.`;
      return;
    }
    // The canister enforces a per-transaction ceiling too; stating it cannot trap
    // funds, the balance comes out in successive withdrawals.
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
    receipt = null;
    const amountText = formatWithUnit(amountSmallest);
    flow = { phase: FLOW.RUNNING, steps: withdrawSteps({ amountText }), current: 0, startedAt: Date.now() };

    try {
      const result = await tableActor.withdraw(amountSmallest);
      if ('Ok' in result) {
        // `withdraw` returns the LEDGER BLOCK INDEX, not an amount
        // (src/table_canister/src/lib.rs `-> Result<u64, String>`, `Ok(block)`).
        // docs/DEFECTS.md T-18: it used to be printed as ICP. The wallet
        // receives `amount - fee` (transfer_tokens), and both figures are stated.
        const received = amountSmallest > transferFee ? amountSmallest - transferFee : 0n;
        const block = String(result.Ok);
        flow = { ...flow, phase: FLOW.DONE };
        lastWithdrawalAt = Date.now();
        receipt = {
          rows: [
            { id: 'withdrawn', label: 'Withdrawn from the table', value: amountText, fiat: fiatOf(amountSmallest) },
            { id: 'fee', label: 'Network fee', value: formatWithUnit(transferFee), fiat: fiatOf(transferFee) },
            { id: 'received', label: 'Reached your wallet', value: formatWithUnit(received), fiat: fiatOf(received), strong: true },
            { id: 'block', label: 'Ledger block', value: block, mono: true },
          ],
          link: IS_MAINNET_BUILD
            ? { href: `https://dashboard.internetcomputer.org/transaction/${block}`, label: 'Open this block on the ICP dashboard' }
            : null,
        };
        onWithdrawSuccess?.();
      } else if ('Err' in result) {
        flow = { ...flow, phase: FLOW.FAILED };
        fail(result.Err);
      }
    } catch (e) {
      flow = { ...flow, phase: FLOW.FAILED };
      fail(e);
    }
    processing = false;
  }

  function setMaxAmount() {
    // MAX must be a number the canister will ACCEPT: capped at MAX_WITHDRAWAL,
    // and FLOORED rather than rounded (toFixed on a balance rounds up half the
    // time, which is how MAX produced an amount its own balance did not cover).
    const balance = toSmallest(currentBalance);
    const capped = balance > MAX_WITHDRAWAL ? MAX_WITHDRAWAL : balance;
    if (isBTC && inputUnit === 'sats') {
      withdrawAmount = capped.toString();
      return;
    }
    // Eight decimal places is the full precision of a smallest unit, so this is
    // exact for both currencies and can never exceed `capped`.
    withdrawAmount = formatPlain(capped);
  }

  // =========================================================================
  // MONEY OF YOURS THAT IS NOT IN THIS BALANCE (docs/SECURITY-FINDINGS.md
  // FINDING 18): `get_balance()` answers what can be withdrawn right now, and
  // an auditor read it as zero and left while 2.98 ICP of theirs sat in a pot.
  // `get_custody_status()` is the whole answer, caller-scoped.
  // =========================================================================
  let custody = $state(null);
  let custodyError = $state(null);
  let recovering = $state(false);

  async function loadCustody() {
    if (!tableActor?.get_custody_status) {
      // An older canister build: say so rather than silently show nothing.
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
  let recovered = $state(null);
  async function recoverStuckPot() {
    if (!tableActor?.abandon_stuck_hand) return;
    recovering = true;
    error = null;
    try {
      const result = await tableActor.abandon_stuck_hand();
      if ('Err' in result) {
        fail(result.Err);
      } else {
        recovered = `Recovered ${formatWithUnit(custody?.committed ?? 0n)} from the stuck hand. `
          + `It is in your withdrawable balance now.`;
        // The pot is gone and the balance has moved: re-read both.
        await loadCustody();
        onWithdrawSuccess?.();
      }
    } catch (e) {
      fail(e);
    }
    recovering = false;
  }

  onMount(() => {
    loadCustody();
    loadSolvency();
    loadRunway();
    loadPrices().then((p) => { prices = p; }).catch(() => { prices = null; });
    const tick = setInterval(() => { now = Date.now(); }, 500);
    return () => clearInterval(tick);
  });

  function finishAndClose() {
    receipt = null;
    flow = { phase: FLOW.IDLE, steps: [], current: 0, startedAt: null };
    onClose();
  }

  // ONE dismissal contract for every dialog in this app (docs/DEFECTS.md T-13).
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- THE BUTTON ROW, ONCE: in the scroller after the disclosures on a phone
     (in flow, never sticky: the round that pinned it measured it over the
     solvency advice), in the footer on a wide screen. -->
{#snippet actionRow()}
  <div class="actions">
    <button type="button" class="btn-secondary" onclick={onClose} disabled={processing}>
      Cancel
    </button>
    <button
      type="button"
      class="btn-primary money"
      onclick={handleWithdraw}
      disabled={processing || cooldownLeft > 0 || !withdrawAmount || Number(withdrawAmount) <= 0}
    >
      {#if processing}<span class="spinner"></span>{/if}
      {primaryLabel}
    </button>
  </div>
{/snippet}

<div class="modal-backdrop" onclick={onClose} role="presentation"></div>

<div class="modal-content" class:btc-modal={isBTC} role="dialog" aria-labelledby="withdraw-modal-title" use:scrollLock>
  <div class="modal-header">
    <h2 id="withdraw-modal-title">
      {#if isBTC}<BtcGlyph size={22} />{:else}<IcpLogo size={22} />{/if}
      <span>
        Withdraw {currencySymbol}
        <span class="title-sub">From your balance at this table to your wallet</span>
      </span>
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close withdraw modal">×</button>
  </div>

  <div class="modal-body">
    <!-- THE FIVE PROTECTED NOTICES, INSIDE THE DIALOG (HARD RULE 2, docs/DEFECTS.md T-31):
         the page's trust bar is behind this dialog's scrim, so the words are
         restated here, first. -->
    <p class="player-notice">
      <span class="notice-glyph" aria-hidden="true">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linejoin="round">
          <path d="M12 3.5 2.5 20h19L12 3.5z"/>
          <path d="M12 9.5v4.5" stroke-linecap="round"/>
          <circle cx="12" cy="17" r="0.8" fill="currentColor" stroke="none"/>
        </svg>
      </span>
      <span class="notice-text"><NoticeLine /></span>
    </p>

    {#if receipt}
      <CashierReceipt
        title="Withdrawn"
        lead={cooldownLeft > 0
          ? `On the ledger. The next withdrawal from this table opens in ${formatCountdown(cooldownLeft)}.`
          : 'On the ledger. You can withdraw again now.'}
        rows={receipt.rows}
        link={receipt.link}
        btc={isBTC}
        onDone={finishAndClose}
      />
    {:else}
      <div class="cashier-grid">
        <div class="cashier-col main">
          <!-- THE BALANCE: what can leave right now. -->
          <div class="balance-info" class:btc={isBTC}>
            <span class="label">Available balance</span>
            <span class="amount cd-money" class:btc={isBTC}>{formatWithUnit(currentBalance)}</span>
            {#if balanceUsd !== null}<span class="usd-value">{formatUsd(balanceUsd)}</span>{/if}
          </div>

          <!-- MONEY OF YOURS THAT IS NOT IN THE FIGURE ABOVE (FINDING 18). In
               flow, directly under the balance it corrects. -->
          {#if custody && custody.committed > 0n}
            <div class="committed-stake" class:stuck={custody.stuck}>
              <div class="committed-headline">
                <span class="committed-label">
                  {custody.stuck ? 'In a pot nobody can win' : 'In a pot right now'}
                </span>
                <span class="committed-amount cd-money">{formatWithUnit(custody.committed)}</span>
              </div>
              <p class="committed-advice">{custody.advice}</p>
              {#if custody.stuck}
                <button type="button" class="recover-btn" onclick={recoverStuckPot} disabled={recovering || processing}>
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

          {#if recovered}
            <div class="alert success">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
              <span class="alert-text">{recovered}</span>
            </div>
          {/if}

          <!-- THE AMOUNT. -->
          <div class="form-section">
            <div class="section-label">
              <label for="withdraw-amount">Amount</label>
              {#if isBTC}
                <div class="unit-toggle">
                  <button type="button" class:active={inputUnit === 'sats'} onclick={() => { inputUnit = 'sats'; withdrawAmount = ''; }}>sats</button>
                  <button type="button" class:active={inputUnit === 'btc'} onclick={() => { inputUnit = 'btc'; withdrawAmount = ''; }}>BTC</button>
                </div>
              {/if}
            </div>
            <div class="input-row">
              <div class="amount-field" class:btc={isBTC}>
                <!-- `step` is the full precision of a smallest unit, so MAX is
                     representable in its own input. -->
                <input
                  id="withdraw-amount"
                  type="number"
                  inputmode="decimal"
                  step={isBTC && inputUnit === 'sats' ? "1" : "0.00000001"}
                  min={inputMinAttr}
                  max={inputMaxAttr}
                  placeholder={inputMinAttr}
                  bind:value={withdrawAmount}
                  disabled={processing}
                />
                <span class="input-suffix" class:btc={isBTC}>{isBTC ? inputUnit : 'ICP'}</span>
              </div>
              <button type="button" class="max-btn" class:btc={isBTC} onclick={setMaxAmount} disabled={processing}>MAX</button>
            </div>
            <!-- Every figure here is interpolated from the mirrored constants; a
                 literal is what docs/DEFECTS.md T-26 was. -->
            <p class="hint">
              Minimum {minDisplay}, maximum {maxDisplay} per transaction. The network fee
              is {feeDisplay} and is taken out of what you withdraw. One withdrawal every
              {WITHDRAWAL_COOLDOWN_SECS} seconds.
            </p>
            <p class="hint">
              Whatever your balance is, all of it can leave in one call: press MAX. That
              works below the stated floor too, as long as what is left is more than the
              network fee.
            </p>
          </div>

          <CashierSummary caption="What arrives" rows={summaryRows} btc={isBTC} />

          <!-- WHERE IT GOES: the signed-in principal's default account, derived
               here. There is no destination field: `withdraw` pays the caller. -->
          <div class="from-card destination">
            <div class="section-label"><span>To</span><span class="label-aside">your wallet's default account</span></div>
            {#if destination}
              <p class="from-note">
                Signed in as <code>{shortId(destination.principal)}</code>. {currencySymbol} arrives at
                this account id, the one your wallet shows for that identity:
              </p>
              <div class="dest-row">
                <code class="dest-id">{destination.accountId}</code>
                <button type="button" class="path-link" onclick={copyDestination}>{copiedDestination ? 'Copied' : 'Copy'}</button>
              </div>
            {:else}
              <p class="from-note short">Sign in to see the account this pays.</p>
            {/if}
          </div>
        </div>

        <div class="cashier-col aside">
          <CashierDisclosures context="withdraw" {currencySymbol} {tableCanisterId}>
            <!-- Whether the ledger actually holds the balance above (FINDING 35),
                 and whether the call will be accepted at all (E-55). -->
            <SolvencyNotice
              {solvency}
              {currency}
              context="withdraw"
              onRefresh={refreshSolvency}
              refreshing={solvencyRefreshing}
            />
            <CycleRunwayNotice {runway} context="withdraw" canisterId={tableCanisterId} />
          </CashierDisclosures>
        </div>
      </div>

      {#if flow.phase !== FLOW.IDLE}
        <CashierStepper
          steps={flow.steps}
          current={flow.current}
          phase={flow.phase}
          startedAt={flow.startedAt}
          failure={flow.phase === FLOW.FAILED ? 'Stopped here.' : null}
        />
      {/if}

      {#if errorView}
        <CashierAlert message={errorView.message} detail={errorView.detail} />
      {/if}

      <p class="cashier-note">
        {#if isBTC}
          ckBTC (Bitcoin on ICP) is sent to your wallet. Convert it to BTC through the NNS or use it on ICP apps.
        {:else}
          ICP is sent to the account above, the default account of the identity you are signed in with. Withdrawing to an exchange means sending it on from that wallet.
        {/if}
      </p>

      {#if phone.matches}{@render actionRow()}{/if}
    {/if}
  </div>

  {#if !receipt && !phone.matches}
    <div class="modal-foot">{@render actionRow()}</div>
  {/if}
</div>

<style lang="scss">
  @use './cashier' as cashier;

  @include cashier.shell;
  @include cashier.columns;
  @include cashier.money($preview: false, $destination: true);
  @include cashier.feedback($money-button: true, $success: true);

  .modal-content { --cashier-w: 860px; }

  .balance-info {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--cd-space-4);
    border: 1px solid var(--cd-money-line);
    border-radius: var(--cd-radius-card);
    background: var(--cd-money-dim);
  }

  .balance-info.btc { border-color: var(--cd-btc-line); background: var(--cd-btc-dim); }

  .balance-info .label {
    color: var(--cd-ink-2);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-strong);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
  }

  .balance-info .amount {
    color: var(--cd-money);
    font-size: var(--cd-text-xl);
    font-weight: var(--cd-weight-figure);
    line-height: 1.2;
  }

  .balance-info .amount.btc { color: var(--cd-btc); }
  .balance-info .usd-value { color: var(--cd-ink-2); font-size: var(--cd-text-sm); }

  /* MONEY OF YOURS THAT IS NOT IN THE BALANCE ABOVE (FINDING 18). In flow,
     no position, no z-index. */
  .committed-stake {
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    padding: var(--cd-space-3) var(--cd-space-4);
    border: 1px solid var(--cd-warn-line);
    border-left-width: 3px;
    border-radius: var(--cd-radius-card);
    background: var(--cd-warn-dim);
  }

  .committed-stake.stuck { border-color: var(--cd-danger-line); border-left-color: var(--cd-danger); background: var(--cd-danger-dim); }

  .committed-headline { display: flex; justify-content: space-between; align-items: baseline; gap: var(--cd-space-3); }

  .committed-label {
    color: var(--cd-warn);
    font-size: var(--cd-text-xs);
    font-weight: var(--cd-weight-figure);
    letter-spacing: var(--cd-tracking-label);
    text-transform: uppercase;
  }

  .committed-stake.stuck .committed-label { color: var(--cd-danger-hi); }
  .committed-amount { color: var(--cd-ink); font-size: var(--cd-text-lg); font-weight: var(--cd-weight-figure); }
  .committed-advice { margin: 0; color: var(--cd-ink-1); font-size: var(--cd-text-sm); line-height: 1.45; }
  .committed-countdown { margin: 0; color: var(--cd-ink-2); font-size: var(--cd-text-xs); }

  .recover-btn {
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border: 1px solid var(--cd-danger-line);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-danger-dim);
    color: var(--cd-ink);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    cursor: pointer;
  }

  .recover-btn:disabled { opacity: 0.6; cursor: not-allowed; }
  .committed-unknown { margin: 0; color: var(--cd-warn); font-size: var(--cd-text-xs); }

  .hint { margin: 0; color: var(--cd-ink-2); font-size: var(--cd-text-xs); line-height: 1.5; }

  .destination { border-style: dashed; }
  .dest-row { display: flex; align-items: center; gap: var(--cd-space-2); }

  .dest-id {
    flex: 1 1 auto;
    padding: var(--cd-space-2) var(--cd-space-3);
    border: 1px solid var(--cd-line-soft);
    border-radius: var(--cd-radius-chip);
    background: var(--cd-bg-deep);
    font-family: var(--cd-font-mono);
    font-size: var(--cd-text-xs);
    line-height: 1.5;
    color: var(--cd-ink);
    word-break: break-all;
  }

  /* THE PHONE: a full-height sheet, the rules shared with DepositModal. */
  @media #{cashier.$phone} {
    @include cashier.phone($deposit: false);
    .recover-btn { min-height: var(--cd-touch-min); }
  }
</style>
