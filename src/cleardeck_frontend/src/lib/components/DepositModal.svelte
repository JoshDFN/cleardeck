<script>
  // THE DEPOSIT SHEET, BUILT FOR A DEPOSITOR. One primary path per state
  // (the amount from the connected wallet when it can pay the minimum plus
  // both ledger fees, otherwise the derived address with a QR that watches
  // itself and sweeps what arrives), the amount and its cost first, the
  // disclosures beside them or between them and the button, chain commits
  // painted as steps, success a receipt with Done.
  //
  // NOTHING IN THE VERIFICATION LOGIC CHANGED. The trust root
  // (lib/trustedTables.js, docs/SECURITY-FINDINGS.md FINDING 42), the derived
  // deposit address and its cross-check against the canister (FINDING 34 / 40,
  // lib/deposit-wallet.svelte.js), the OISY subaccount cross-check, the ledger
  // approval of the amount plus one network charge and the table's pull
  // (lib/deposit-flow.js, pressed by lib/deposit-submit.js), the sweep
  // (lib/deposit-flow.svelte.js) are the same calls in the same order with
  // the same refusals as before. This file keeps the mirrored limits, the
  // refusals that name a limit, the routing and the markup.

  import { auth } from '$lib/auth.js';
  import { oisy } from '$lib/oisy.js';
  import { onMount } from 'svelte';
  import IcpLogo from './IcpLogo.svelte';
  import BtcGlyph from './BtcGlyph.svelte';
  import NoticeStrip from './NoticeStrip.svelte';
  import SolvencyNotice from './SolvencyNotice.svelte';
  import CycleRunwayNotice from './CycleRunwayNotice.svelte';
  import { createRunwayRead, createSolvencyRead } from '$lib/cashier-disclosures.svelte.js';
  import { IS_MAINNET_BUILD } from '$lib/ic-config.js';
  import { isTrustedTableId, untrustedTableMessage } from '$lib/trustedTables.js';
  import { scrollLock } from '$lib/scroll-lock.js';
  import { pinAfter } from '$lib/pin-after.js';
  import { phoneMedia } from '$lib/phone-media.svelte.js';
  import { loadPrices } from '$lib/prices.js';
  import {
    depositCost, formatCashier, formatExact, formatPlain, formatUsd, toSmallest, usdValue,
  } from '$lib/cashier-format.js';
  import { FLOW } from '$lib/cashier-steps.js';
  import { describeCashierFailure } from '$lib/humane-errors.js';
  import { ledgerCanisterFor } from '$lib/deposit-icrc2.js';
  import {
    costRows, equivalentText, inputFloorText, openingAmountText, quickChips, typedToSmallest,
  } from '$lib/deposit-amounts.js';
  import { DETECT, detectionCopy } from '$lib/deposit-detect.js';
  import { createAddressWatch, createCashierFlow } from '$lib/deposit-flow.svelte.js';
  import { createWalletReader } from '$lib/deposit-wallet.svelte.js';
  import { createBtcDeposit } from '$lib/deposit-btc.svelte.js';
  import { submitDeposit } from '$lib/deposit-submit.js';
  import { claimReceipt } from '$lib/deposit-receipts.js';
  import { shortId } from '$lib/lobby-format.js';
  import CashierStepper from './CashierStepper.svelte';
  import CashierAlert from './CashierAlert.svelte';
  import CashierSummary from './CashierSummary.svelte';
  import CashierReceipt from './CashierReceipt.svelte';
  import CashierDisclosures from './CashierDisclosures.svelte';
  import DepositAddressCard from './DepositAddressCard.svelte';
  import DepositAmountField from './DepositAmountField.svelte';
  import DepositRouteTabs from './DepositRouteTabs.svelte';
  import BtcNativeDeposit from './BtcNativeDeposit.svelte';
  import IiFromCard from './IiFromCard.svelte';
  import OisyFromCard from './OisyFromCard.svelte';

  const {
    tableActor, tableCanisterId, onClose, onDepositSuccess, currency = 'ICP',
    // The figure the dialog opens with, in the smallest unit (e8s or sats), or
    // null. A Sit tap on a seat the player cannot yet afford hands the
    // shortfall here (lib/join-gate.js) so the cashier is one tap from the seat.
    initialAmount = null,
    // The TABLE canister's own minimum buy-in (config.min_buy_in, the smallest
    // unit), for the quick chips; null hides them. Never the lobby record's
    // figure (docs/DEFECTS.md T-11).
    minBuyIn = null,
  } = $props();

  // The props are fixed for the life of a sheet (routes/+page.svelte mounts
  // one per opening), so each is read once, on purpose.
  // svelte-ignore state_referenced_locally
  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';
  const ledgerCanisterId = ledgerCanisterFor(currencySymbol);

  // >>> MIRRORED-LIMITS-BEGIN  (tests/money_safety/tests/ui_limits.rs reads this fence)
  // ===========================================================================
  // MIRRORED CANISTER LIMITS: THE ONLY NUMBERS IN THIS FILE (docs/DEFECTS.md T-26)
  // ===========================================================================
  //
  // Every stated figure below is interpolated from these constants, so the copy
  // agrees with the canister by construction rather than by luck.
  //
  // WHAT THE FLOOR NOW PROMISES (docs/SECURITY-FINDINGS.md FINDING 27). Agreeing
  // with the deposit door was never enough: the canister accepted this amount and
  // its WITHDRAWAL floor then refused to return it. The two floors are now one
  // number per currency, asserted at compile time in lib.rs, so the number below
  // is a promise in both directions.
  //
  // src/table_canister/src/lib.rs, MIRRORED, keep in step:
  //   :36 ICP_TRANSFER_FEE       10_000  (0.0001 ICP)
  //   :40 CKBTC_TRANSFER_FEE     10      (10 sats)
  //   :66 ICP_MIN_DEPOSIT_AMOUNT 20_000  (0.0002 ICP)
  //   :67 BTC_MIN_DEPOSIT_AMOUNT 1_000   (1000 sats)
  // `tests/money_safety/tests/ui_limits.rs` fails if these stop matching, and fails
  // if any surface here states the floor or the fee as a literal.
  const TRANSFER_FEE = isBTC ? 10n : 10_000n;
  const MIN_DEPOSIT = isBTC ? 1_000n : 20_000n;
  // THE ADDRESS ROUTE HAS A HIGHER FLOOR THAN THE APPROVE ROUTE, and printing the
  // approve floor beside the address is what cost the fifth auditor 100% of a
  // deposit. `deposit()` charges the ledger fee alongside the amount, so the floor
  // arrives intact. `claim_external_deposit` pays the fee OUT OF the sweep, so a
  // deposit of D is credited D - fee, and at the approve floor that lands below
  // the withdrawal floor and can never leave.
  //   :134 ICP_MIN_EXTERNAL_DEPOSIT = ICP_MIN_WITHDRAWAL_AMOUNT + ICP_TRANSFER_FEE
  //   :135 BTC_MIN_EXTERNAL_DEPOSIT = BTC_MIN_WITHDRAWAL_AMOUNT + CKBTC_TRANSFER_FEE
  const MIN_EXTERNAL_DEPOSIT = isBTC ? 11n + 10n : 20_000n + 10_000n;

  const transferFee = TRANSFER_FEE;
  const minDeposit = MIN_DEPOSIT;

  const minDepositDisplay = formatExact(MIN_DEPOSIT, currencySymbol);
  const minExternalDepositDisplay = formatExact(MIN_EXTERNAL_DEPOSIT, currencySymbol);
  const feeDisplay = formatExact(TRANSFER_FEE, currencySymbol);
  // The floor the LEDGER imposes, which is the floor that actually matters. An
  // ICRC-2 deposit costs the depositor TWO ledger fees, not one: one for
  // `icrc2_approve` and one for the canister's `icrc2_transfer_from`, both charged
  // to the depositor's account. So a wallet holding exactly the minimum cannot make
  // the minimum deposit.
  const DEPOSIT_LEDGER_FEES = TRANSFER_FEE * 2n;
  const minWalletBalance = MIN_DEPOSIT + DEPOSIT_LEDGER_FEES;
  const minWalletBalanceDisplay = formatExact(minWalletBalance, currencySymbol);
  const ledgerFeesDisplay = formatExact(DEPOSIT_LEDGER_FEES, currencySymbol);

  // NOT ClearDeck limits, and NOT enforced by anything in this repository.
  //
  // The native-BTC path hands the player an address owned by the ckBTC MINTER and
  // the thresholds below are the minter's, not the table canister's. They are
  // named once, here, so they cannot drift between the places the native-BTC
  // flow mentions them.
  const BTC_NATIVE_MIN_SATS = 10_000n;
  const BTC_NATIVE_MINTER_FEE_SATS = 2_000n;
  const btcNativeMinDisplay = `${BTC_NATIVE_MIN_SATS.toLocaleString('en-US')} sats`;
  const btcNativeMinBtcDisplay = formatPlain(BTC_NATIVE_MIN_SATS);
  const btcNativeMinterFeeDisplay = `${BTC_NATIVE_MINTER_FEE_SATS.toLocaleString('en-US')} sats`;
  // <<< MIRRORED-LIMITS-END

  // ==========================================================================
  // IS THIS CANISTER ID ONE THIS BUILD HAS EVER HEARD OF?
  // ASKED BEFORE ANY ADDRESS IS DERIVED AND BEFORE ANY TRANSFER IS ADDRESSED.
  // ==========================================================================
  //
  // docs/SECURITY-FINDINGS.md FINDING 42. `tableCanisterId` reaches this
  // component from `routes/+page.svelte`, which took it out of
  // `lobby.get_tables()`, an ordinary QUERY one replica answers out of its own
  // memory. Everything below that touches money is a function of this id (the
  // deposit ADDRESS, the OISY transfer's destination, the ICRC-2 approval's
  // spender), and a substituted id moves all three to a canister the attacker
  // controls. The trust root is `$lib/trustedTables.js`: the ids this build was
  // PUBLISHED with. A wire-supplied id may name a table on screen; it may never
  // be the first argument of a deposit address.
  const tableIsTrusted = $derived(isTrustedTableId(tableCanisterId));
  const untrustedReason = $derived(
    tableIsTrusted ? null : untrustedTableMessage(tableCanisterId)
  );

  // CAN THIS TABLE PAY BACK WHAT IT ALREADY HOLDS, AND FOR HOW LONG CAN IT
  // KEEP HONOURING WITHDRAWALS? Both asked before, not after
  // (lib/cashier-disclosures.svelte.js).
  // svelte-ignore state_referenced_locally
  const solvencyRead = createSolvencyRead(tableActor);
  // svelte-ignore state_referenced_locally
  const runwayRead = createRunwayRead(tableActor);

  let authState = $state({ isAuthenticated: false, principal: null });
  $effect(() => {
    const unsub = auth.subscribe(s => { authState = s; });
    return unsub;
  });

  let oisyState = $state({ isConnected: false, isConnecting: false, icpBalance: null, ckbtcBalance: null, principal: null });
  $effect(() => {
    const unsub = oisy.subscribe(s => { oisyState = s; });
    return unsub;
  });

  // Which sheet this is: the phone's full-height sheet keeps the button row
  // in the scroller after the disclosures; the wide dialog puts it in a footer.
  const phone = phoneMedia();

  // Seeded once, on purpose: the field is the player's from here on.
  // svelte-ignore state_referenced_locally
  let depositAmount = $state(openingAmountText(initialAmount, currency));
  let processing = $state(false);
  // A string is this component's own refusal, shown as written; an object is a
  // canister's or the ledger's answer, already turned into a sentence.
  let error = $state(null);

  // Wallet source: 'ii' (Internet Identity) or 'oisy' (OISY Wallet). OISY
  // signs on mainnet only, so the toggle renders on a mainnet build only.
  let walletSource = $state('ii');

  // The route: 'wallet' (approve + pull from the connected wallet) or
  // 'address' (send to the derived address, which sweeps what arrives). Null
  // follows the balance: the wallet route when it can pay, the address otherwise.
  let route = $state(null);

  let depositMethod = $state('ckbtc'); // 'ckbtc' or 'btc' for BTC tables
  let inputUnit = $state('sats'); // 'sats' or 'btc' for BTC input mode

  // The one quote for the page (lib/prices.js): null renders no fiat hint.
  let prices = $state(null);

  // THE FLOW (which step is running, since when) AND THE RECEIPT (what moved,
  // shown until Done): lib/deposit-flow.svelte.js.
  const cashier = createCashierFlow();

  // THE PAYING WALLET AND YOUR DEPOSIT ADDRESS (lib/deposit-wallet.svelte.js):
  // the ledger balance, and the address derived locally then cross-checked
  // against the canister, never taken from it (FINDING 34 / 40).
  // svelte-ignore state_referenced_locally
  const wallet = createWalletReader({ auth, tableActor, tableCanisterId, ledgerCanisterId, isBTC, currencySymbol });

  /** A canister's or the ledger's refusal, as a sentence with the raw text kept. */
  function fail(e) {
    error = describeCashierFailure(e);
  }

  const errorView = $derived(
    error === null ? null : (typeof error === 'string' ? { message: error, detail: null } : error)
  );

  // The input's own floor, in whichever unit the player is typing.
  const inputMinAttr = $derived(inputFloorText(MIN_DEPOSIT, { isBTC, inputUnit }));

  // What the typed text means in the smallest unit (sats or e8s): the float
  // floor the deposit always used (lib/deposit-amounts.js typedToSmallest).
  const inputToSmallestUnit = (amount) => typedToSmallest(amount, { isBTC, inputUnit });

  const formatWithUnit = (smallestUnit) => formatCashier(smallestUnit, currencySymbol, { placeholder: '...' });

  const perTokenUsd = $derived(prices ? (isBTC ? prices.btcUsd : prices.icpUsd) : null);
  const fiatOf = (smallestUnit) => {
    const v = usdValue(smallestUnit, perTokenUsd);
    return v === null ? null : formatUsd(v);
  };

  // THE NATIVE-BTC PATH (lib/deposit-btc.svelte.js): the minter's address,
  // fetched only for a pinned table (FINDING 45), and the check for what
  // has been minted.
  // svelte-ignore state_referenced_locally
  const btc = createBtcDeposit({
    tableActor, isBTC,
    isTrusted: () => tableIsTrusted,
    untrustedReason: () => untrustedReason,
    isAuthenticated: () => authState.isAuthenticated,
    format: formatWithUnit,
    onFailure: fail,
    onMinted: wallet.load,
  });

  // Connect to OISY wallet
  async function connectOisyWallet() {
    error = null;
    try {
      if (isBTC) {
        await oisy.connectForIcrc();
      } else {
        await oisy.connectForIcp();
      }
    } catch (e) {
      error = e.message || 'Failed to connect OISY wallet';
    }
  }

  async function disconnectOisyWallet() {
    await oisy.disconnect();
    walletSource = 'ii';
  }

  const effectiveWalletBalance = $derived.by(() => {
    if (walletSource === 'oisy') {
      return isBTC ? oisyState.ckbtcBalance : oisyState.icpBalance;
    }
    return wallet.balance;
  });

  const effectiveLoadingBalance = $derived.by(() => {
    if (walletSource === 'oisy') {
      return oisyState.loadingBalances;
    }
    return wallet.loading;
  });

  const hasEnoughBalance = $derived.by(() => {
    const bal = effectiveWalletBalance;
    // `>= minWalletBalance`, not `> minDeposit`: the deposit costs the minimum
    // PLUS both ledger fees, so this is the balance at which a deposit can
    // actually succeed.
    return bal !== null && toSmallest(bal) >= minWalletBalance;
  });

  const walletUsd = $derived(wallet.balance && !isBTC ? fiatOf(wallet.balance) : null);
  const oisyUsd = $derived(
    walletSource === 'oisy' && effectiveWalletBalance ? fiatOf(effectiveWalletBalance) : null
  );

  // THE QUICK CHIPS (QuickAmounts.svelte, lib/deposit-amounts.js): the table's
  // minimum buy-in and twice it, the figure on the face, each disabled with
  // the reason in its hint when this wallet cannot cover it plus both fees.
  const quickAmounts = $derived(quickChips({
    minBuyIn,
    fee: TRANSFER_FEE,
    balance: effectiveLoadingBalance || effectiveWalletBalance === null ? null : toSmallest(effectiveWalletBalance),
    isBTC,
    inputUnit,
    format: formatWithUnit,
  }));

  // The route in force: the player's choice, else what the balance allows.
  const showsWalletRoute = $derived(!isBTC || depositMethod === 'ckbtc');
  const effectiveRoute = $derived.by(() => {
    if (!showsWalletRoute) return 'btc';
    if (route) return route;
    if (isBTC) return 'wallet';
    if (effectiveLoadingBalance) return 'wallet';
    return hasEnoughBalance ? 'wallet' : 'address';
  });

  const walletCanPay = $derived(
    effectiveRoute === 'wallet' && hasEnoughBalance && (walletSource === 'ii' || oisyState.isConnected)
  );

  // THE ADDRESS WATCHES ITSELF AND SWEEPS WHAT ARRIVES (lib/deposit-flow.svelte.js
  // createAddressWatch, decisions in lib/deposit-detect.js): while the card is
  // up the derived subaccount is read every few seconds and a ready reading
  // is claimed with the same `claim_external_deposit` the button makes, once
  // per reading; nothing at an unpinned canister is yours to sweep.
  const watching = $derived(
    !isBTC && effectiveRoute === 'address' && tableIsTrusted && Boolean(wallet.address) && !cashier.receipt
  );
  // svelte-ignore state_referenced_locally
  const watch = createAddressWatch({
    tableActor,
    readBalance: wallet.readAddressBalance,
    isWatching: () => watching,
    isTrusted: () => tableIsTrusted,
    untrustedReason: () => untrustedReason,
    flow: cashier,
    setError: (message) => { error = message; },
    onFailure: fail,
    receiptFor: (arrived, balance) => claimReceipt({
      arrived, fee: TRANSFER_FEE, feeText: feeDisplay, balance, format: formatWithUnit, fiatOf,
    }),
    onCredited: async () => {
      await wallet.load();
      onDepositSuccess?.();
    },
    fee: TRANSFER_FEE,
    minExternal: MIN_EXTERNAL_DEPOSIT,
  });

  // THE AMOUNT TYPED, AND WHAT IT COSTS. Every figure comes from the mirrored
  // fee and the typed amount; the harness recomputes the same rows from the
  // ledger's own fee (chain-agreement.mjs cost-summary).
  const typedSmallest = $derived.by(() => {
    if (!depositAmount || !(Number(depositAmount) > 0)) return 0n;
    return inputToSmallestUnit(depositAmount);
  });

  const typedUsd = $derived(typedSmallest > 0n ? fiatOf(typedSmallest) : null);

  // The preview line under the field and the cost rows (lib/deposit-amounts.js).
  const previewText = $derived(equivalentText(typedSmallest, { isBTC, inputUnit, format: formatWithUnit }));
  const cost = $derived(
    typedSmallest > 0n
      ? depositCost({ amount: typedSmallest, fee: TRANSFER_FEE })
      : null,
  );
  const summaryRows = $derived(costRows(cost, formatWithUnit));

  const primaryLabel = $derived.by(() => {
    if (effectiveRoute === 'btc') return btc.updating ? 'Checking…' : 'Check for deposit';
    if (effectiveRoute === 'address') {
      if (watch.claiming) return 'Claiming…';
      if (watch.claimFailed && watch.status === DETECT.READY) return `Claim ${formatWithUnit(watch.detected)} again`;
      return 'Check for my transfer';
    }
    if (processing) return 'Working…';
    return typedSmallest > 0n ? `Deposit ${formatWithUnit(typedSmallest)}` : `Deposit ${currencySymbol}`;
  });

  const primaryDisabled = $derived.by(() => {
    if (effectiveRoute === 'btc') return btc.updating || !btc.address || !tableIsTrusted;
    if (effectiveRoute === 'address') return watch.claiming || !tableIsTrusted || !wallet.address;
    return processing || !tableIsTrusted || !walletCanPay || typedSmallest <= 0n;
  });

  onMount(() => {
    wallet.load();
    loadPrices().then((p) => { prices = p; }).catch(() => { prices = null; });
    solvencyRead.load();
    runwayRead.load();
    if (isBTC) {
      btc.loadAddress();
    }
  });

  async function handlePrimary() {
    if (effectiveRoute === 'btc') {
      error = null;
      return btc.check();
    }
    if (effectiveRoute === 'address') return watch.checkNow();
    return handleDeposit();
  }

  async function handleDeposit() {
    // FINDING 42, THE OTHER TWO DOORS: the ICRC-2 branch names this id as the
    // SPENDER of an approval over the player's ledger balance, and the OISY
    // branch addresses a transfer to it. Both refuse here, before anything is
    // signed.
    if (!tableIsTrusted) {
      error = untrustedReason;
      return;
    }
    if (!depositAmount || Number(depositAmount) <= 0) {
      error = 'Please enter a valid amount';
      return;
    }

    const amountSmallest = inputToSmallestUnit(depositAmount);

    if (amountSmallest < minDeposit) {
      error = `Minimum deposit is ${minDepositDisplay}`;
      return;
    }

    const currentBalance = effectiveWalletBalance;
    if (currentBalance !== null && amountSmallest > toSmallest(currentBalance)) {
      error = `Insufficient balance. You have ${formatWithUnit(currentBalance)} in your wallet.`;
      return;
    }

    if (!tableCanisterId) {
      error = 'Table canister ID not available';
      return;
    }

    processing = true;
    error = null;
    cashier.receipt = null;

    const approveAmount = amountSmallest + transferFee;

    try {
      // THE TWO ROUTES (lib/deposit-submit.js): OISY derives and cross-checks
      // the subaccount, transfers, claims; Internet Identity approves the
      // amount plus one network charge with the table as the spender, then
      // the table pulls. Each paints its steps on the flow.
      const outcome = await submitDeposit({
        source: walletSource, amountSmallest, approveAmount, amountTyped: depositAmount, currencySymbol, isBTC,
        tableActor, tableCanisterId, ledgerCanisterId, auth, oisy,
        oisyPrincipal: oisyState.principal, sessionPrincipal: authState.principal,
        flow: cashier,
        money: { format: formatWithUnit, fiatOf, feeDisplay, ledgerFees: DEPOSIT_LEDGER_FEES, ledgerFeesDisplay },
        shortId,
      });
      if (outcome.ok) {
        cashier.receipt = outcome.receipt;
        if (outcome.source === 'oisy') await oisy.refreshBalances();
        else wallet.load();
        onDepositSuccess?.();
      } else if (outcome.error) {
        error = outcome.error;
      } else {
        fail(outcome.failure);
      }
    } catch (e) {
      console.error('Deposit error:', e);
      cashier.fail();
      fail(e);
    }
    processing = false;
  }

  function setMaxAmount() {
    const bal = effectiveWalletBalance;
    if (bal === null) return;
    const balance = toSmallest(bal);
    if (balance < minWalletBalance) return;
    // Hold back both ledger fees, derived from the constant, and FLOOR rather
    // than round: toFixed on a balance rounds up half the time, which is how a
    // MAX button produces an amount its own wallet cannot cover.
    const maxSmallest = balance - DEPOSIT_LEDGER_FEES;
    depositAmount = isBTC && inputUnit === 'sats'
      ? maxSmallest.toString()
      : formatPlain(maxSmallest);
  }

  function finishAndClose() {
    cashier.receipt = null;
    cashier.reset();
    onClose();
  }

  // ONE dismissal contract for every dialog in this app (docs/DEFECTS.md T-13).
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<!-- THE BUTTON ROW, ONCE: rendered in the scroller after the disclosures on a
     phone (where lib/pin-after.js pins it only once the solvency block and the
     runway panel have been scrolled past), in the footer on a wide screen. -->
{#snippet actionRow()}
  <div class="actions" use:pinAfter={{ gates: ['.solvency', '.runway-notice'], scroller: '.modal-body' }}>
    <button type="button" class="btn-secondary" onclick={onClose} disabled={processing || watch.claiming}>
      Cancel
    </button>
    <button
      type="button"
      class="btn-primary"
      class:btc={isBTC}
      onclick={handlePrimary}
      disabled={primaryDisabled}
    >
      {#if processing || watch.claiming || btc.updating}<span class="spinner"></span>{/if}
      {primaryLabel}
    </button>
  </div>
{/snippet}

<div class="modal-backdrop" onclick={onClose} role="presentation"></div>

<div
  class="modal-content"
  class:btc-modal={isBTC}
  role="dialog"
  aria-labelledby="deposit-modal-title"
  data-table-trust={tableIsTrusted ? 'pinned' : 'refused'}
  data-route={effectiveRoute}
  data-detect={effectiveRoute === 'address' ? watch.status : null}
  use:scrollLock
>
  <div class="modal-header">
    <h2 id="deposit-modal-title">
      {#if isBTC}<BtcGlyph size={22} />{:else}<IcpLogo size={22} />{/if}
      <span>
        Deposit {currencySymbol}
        <span class="title-sub">Into your balance at this table</span>
      </span>
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close deposit modal">×</button>
  </div>

  <div class="modal-body">
    <!-- THE FIVE PROTECTED NOTICES, INSIDE THE DIALOG (HARD RULE 2, docs/DEFECTS.md T-31). -->
    <NoticeStrip />

    {#if cashier.receipt}
      <CashierReceipt
        title={cashier.receipt.title}
        lead={cashier.receipt.lead}
        rows={cashier.receipt.rows}
        btc={isBTC}
        onDone={finishAndClose}
      />
    {:else}
      <div class="cashier-grid">
        <!-- THE MONEY COLUMN: which wallet, how much, what it costs. -->
        <div class="cashier-col main">
          <DepositRouteTabs
            btc={isBTC}
            {showsWalletRoute}
            route={effectiveRoute}
            {walletSource}
            {depositMethod}
            mainnet={IS_MAINNET_BUILD}
            onRoute={(r) => { route = r; }}
            onSource={(s) => { walletSource = s; }}
            onMethod={(m) => { depositMethod = m; }}
          />

          {#if showsWalletRoute}
            <!-- FROM: the paying wallet and what it holds. -->
            {#if walletSource === 'oisy' && effectiveRoute === 'wallet'}
              <OisyFromCard
                {oisyState}
                balanceText={formatWithUnit(effectiveWalletBalance)}
                usdText={oisyUsd}
                hasEnough={hasEnoughBalance}
                {minWalletBalanceDisplay}
                sessionPrincipal={authState.principal}
                btc={isBTC}
                onConnect={connectOisyWallet}
                onDisconnect={disconnectOisyWallet}
              />
            {:else}
              <IiFromCard
                balanceText={formatWithUnit(wallet.balance)}
                usdText={walletUsd}
                loading={wallet.loading}
                hasEnough={hasEnoughBalance}
                {minWalletBalanceDisplay}
                btc={isBTC}
                route={effectiveRoute}
                onReread={wallet.load}
              />
            {/if}

            {#if effectiveRoute === 'wallet'}
              <!-- THE AMOUNT, then what it costs. -->
              <DepositAmountField
                bind:value={depositAmount}
                bind:unit={inputUnit}
                btc={isBTC}
                minAttr={inputMinAttr}
                disabled={processing || !walletCanPay}
                chips={quickAmounts}
                {typedSmallest}
                equivalentText={previewText}
                {typedUsd}
                onMax={setMaxAmount}
              />

              <CashierSummary caption="What this costs" rows={summaryRows} btc={isBTC} />
            {:else}
              <DepositAddressCard
                address={wallet.address}
                warning={wallet.warning}
                deriving={wallet.loading}
                detectStatus={watch.status}
                detectedText={watch.detected !== null && watch.status !== DETECT.EMPTY ? formatWithUnit(watch.detected) : null}
                detectCopy={detectionCopy(watch.status, { claiming: watch.claiming, failed: watch.claimFailed })}
                sweeping={watch.claiming && watch.status === DETECT.READY}
              />
            {/if}

            <!-- THE LIMITS: the route's own floor first, the other route's
                 after it, the reasons one tap away. Every figure is
                 interpolated (docs/DEFECTS.md T-26) and every labelled claim
                 is what tools/shots/lib/chain-agreement.mjs reads by label,
                 so the sentences keep their words in both orders. -->
            <div class="minimum-notice" class:btc={isBTC}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="16" x2="12" y2="12"/>
                <line x1="12" y1="8" x2="12.01" y2="8"/>
              </svg>
              <div class="min-copy">
                {#if effectiveRoute === 'wallet'}
                  <strong>Minimum from this wallet: {minDepositDisplay}</strong>, and the network fee
                  {feeDisplay} is charged twice by the ledger (the approval and the pull), so you need
                  {minWalletBalanceDisplay} here to deposit the minimum. Sending to your deposit address
                  instead has a minimum of {minExternalDepositDisplay}.
                  <strong>At or below {feeDisplay} nothing can move it.</strong>
                {:else}
                  <strong>Minimum to this address: {minExternalDepositDisplay}</strong>; from a
                  connected wallet the lower minimum of {minDepositDisplay} applies (network fee
                  {feeDisplay}, charged twice by the ledger, so you need {minWalletBalanceDisplay}
                  in your wallet to deposit the minimum).
                  <strong>At or below {feeDisplay} nothing can move it.</strong>
                {/if}
                <details class="min-why-more">
                  <summary>Why</summary>
                  <span class="min-why">Sweeping pays the network fee out of what you send, so
                  anything less would arrive too small to withdraw again. Sending from a
                  connected wallet instead has a lower minimum of {minDepositDisplay}.</span>
                  <span class="min-why">Send less and it is not swept and it is not lost: above
                  the {feeDisplay} network fee you can ask for it back to your own wallet at any
                  time, less that one fee. <strong>At or below {feeDisplay} nothing can move
                  it</strong>: a transfer costs more than the amount, so it cannot be swept,
                  refunded or withdrawn by anyone. Top the same address up to the minimum and
                  the whole balance comes out together.</span>
                </details>
              </div>
            </div>
          {:else}
            <BtcNativeDeposit
              address={tableIsTrusted ? btc.address : ''}
              loading={btc.loading}
              addressError={btc.error}
              minDisplay={btcNativeMinDisplay}
              minBtcDisplay={btcNativeMinBtcDisplay}
              minterFeeDisplay={btcNativeMinterFeeDisplay}
              result={btc.result}
            />
          {/if}
        </div>

        <!-- THE DISCLOSURES: beside the money on a wide screen, between the
             amount and the button on a phone. Each is in flow and above the
             button row (docs/SECURITY-FINDINGS.md FINDING 23 / 35 / 42). -->
        <div class="cashier-col aside">
          <CashierDisclosures
            context="deposit"
            {currencySymbol}
            {tableCanisterId}
            {tableIsTrusted}
            {untrustedReason}
          >
            <SolvencyNotice
              solvency={solvencyRead.solvency}
              {currency}
              context="deposit"
              onRefresh={solvencyRead.refresh}
              refreshing={solvencyRead.refreshing}
            />
            <CycleRunwayNotice runway={runwayRead.runway} context="deposit" canisterId={tableCanisterId} />
          </CashierDisclosures>
        </div>
      </div>

      {#if cashier.flow.phase !== FLOW.IDLE}
        <CashierStepper
          steps={cashier.flow.steps}
          current={cashier.flow.current}
          phase={cashier.flow.phase}
          startedAt={cashier.flow.startedAt}
          failure={cashier.flow.phase === FLOW.FAILED ? 'Stopped here.' : null}
        />
      {/if}

      {#if errorView}
        <CashierAlert message={errorView.message} detail={errorView.detail} />
      {/if}

      <p class="cashier-note">
        {#if effectiveRoute === 'btc'}
          Bitcoin sent to the address above becomes ckBTC in your wallet; deposit it to the table from the ckBTC tab.
        {:else if effectiveRoute === 'address'}
          Whatever arrives at your deposit address is swept into your balance at this table by itself. You can withdraw back to your wallet at any time.
        {:else}
          This moves {isBTC ? 'ckBTC' : 'ICP'} from your {walletSource === 'oisy' ? 'OISY' : 'Internet Identity'} wallet to your balance at this table. You can withdraw back to your wallet at any time.
        {/if}
      </p>

      {#if phone.matches}{@render actionRow()}{/if}
    {/if}
  </div>

  {#if !cashier.receipt && !phone.matches}
    <div class="modal-foot">{@render actionRow()}</div>
  {/if}
</div>

<style lang="scss">
  @use './cashier' as cashier;

  @include cashier.shell;
  @include cashier.columns;
  @include cashier.money;
  @include cashier.feedback;

  .modal-content { --cashier-w: 960px; }

  /* =========================================================================
     THE PHONE: A FULL-HEIGHT SHEET, AND THE FIRST SCREEN IS THE AMOUNT.
     The DOM order is the phone order (the money column first, the
     disclosures after it, the button row last), so no flex reordering is
     needed. The disclosures stand BETWEEN the amount and the Deposit button
     (docs/SECURITY-FINDINGS.md FINDING 23 / 35 / 42: each before every
     control that can move money) and the row pins to the foot only after
     they have been scrolled past.
     ========================================================================= */
  @media #{cashier.$phone} {
    @include cashier.phone($field: false);

    /* THE DEPOSIT ROW PINS TO THE FOOT OF THE SHEET, BUT ONLY AFTER THE
       WARNINGS: lib/pin-after.js adds `pinned` once the solvency block and
       the runway panel have their bottom edges above the line the row's top
       would sit on, and re-measures when the content grows under it. Opaque,
       with a hairline. tools/shots/touch-targets.mjs measures the row at rest
       and at the end of the scroll.

       THE SCROLLER'S BOTTOM PADDING IS ZERO WHILE THE ROW IS IN IT. A sticky
       box is kept inside its containing block's CONTENT edge; with the body
       padded at the foot and the row's in-flow box pushed into that padding
       by a negative margin, the stuck row stood one padding higher than its
       in-flow place at the end of the scroll and covered the note above it
       (the cashier wave's third round measured the 24 px). The row carries
       the safe-area inset itself, so nothing is lost. */
    .modal-body:has(> .actions) { padding-bottom: 0; }

    .actions {
      order: 10;
      z-index: 2;
      margin: 0 calc(-1 * var(--cd-space-4));
      padding: var(--cd-space-2) var(--cd-space-4) calc(var(--cd-space-2) + var(--cd-safe-bottom));
      background: var(--cd-sheet);
      border-top: 1px solid var(--cd-line-soft);
    }

    /* `pinned` is set by the action, so :global keeps Svelte from pruning it. */
    .actions:global(.pinned) {
      position: sticky;
      bottom: 0;
    }
  }
</style>
