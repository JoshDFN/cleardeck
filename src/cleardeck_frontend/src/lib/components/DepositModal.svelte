<script>
  // THE DEPOSIT SHEET, BUILT FOR A DEPOSITOR.
  //
  // One primary path per state: an amount from the connected wallet when the
  // wallet can pay the minimum plus both ledger fees, otherwise the derived
  // deposit address with a QR; the other route is one tap away. The amount,
  // its fiat hint, the quick chips and the cost of the deposit sit first; the
  // disclosures (custody, destination, solvency, runway) stand beside them on
  // a wide screen and between the amount and the button on a phone; the
  // button names the amount; two chain commits are painted as steps; success
  // is a receipt with Done.
  //
  // NOTHING IN THE VERIFICATION LOGIC CHANGED. The trust root
  // (lib/trustedTables.js, docs/SECURITY-FINDINGS.md FINDING 42), the derived
  // deposit address and its cross-check against the canister (FINDING 34 / 40),
  // the OISY subaccount cross-check, the ledger approval of the amount plus
  // one network charge and the table's pull are the same calls in the same order with the same
  // refusals as before; only their presentation moved.

  import { auth } from '$lib/auth.js';
  import NoticeLine from './NoticeLine.svelte';
  import { oisy } from '$lib/oisy.js';
  import { Actor } from '@dfinity/agent';
  import { Principal } from '@dfinity/principal';
  import { onMount } from 'svelte';
  import IcpLogo from './IcpLogo.svelte';
  import SolvencyNotice from './SolvencyNotice.svelte';
  import { readTableSolvency, refreshTableSolvency } from '$lib/solvency.js';
  import CycleRunwayNotice from './CycleRunwayNotice.svelte';
  import { readCycleRunway } from '$lib/cycleRunway.js';
  import { IS_MAINNET_BUILD } from '$lib/ic-config.js';
  import {
    deriveTrustedDepositAddress,
    checkAgainstCanister,
    accountIdentifierHex,
    depositSubaccount,
  } from '$lib/depositAddress.js';
  import { isTrustedTableId, untrustedTableMessage } from '$lib/trustedTables.js';
  import { scrollLock } from '$lib/scroll-lock.js';
  import QuickAmounts from './QuickAmounts.svelte';
  import { pinAfter } from '$lib/pin-after.js';
  import { loadPrices } from '$lib/prices.js';
  import {
    depositCost, floatToSmallest, formatCashier, formatExact, formatPlain, formatUsd, toSmallest,
    usdValue,
  } from '$lib/cashier-format.js';
  import { FLOW, depositSteps } from '$lib/cashier-steps.js';
  import { describeCashierFailure } from '$lib/humane-errors.js';
  import CashierStepper from './CashierStepper.svelte';
  import CashierSummary from './CashierSummary.svelte';
  import CashierReceipt from './CashierReceipt.svelte';
  import CashierDisclosures from './CashierDisclosures.svelte';
  import DepositAddressCard from './DepositAddressCard.svelte';
  import BtcNativeDeposit from './BtcNativeDeposit.svelte';
  import { shortId } from '$lib/lobby-format.js';

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

  // Currency-specific settings
  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';

  // Ledger canister IDs
  const ICP_LEDGER_CANISTER = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
  const CKBTC_LEDGER_CANISTER = 'mxzaz-hqaaa-aaaar-qaada-cai';
  const ledgerCanisterId = isBTC ? CKBTC_LEDGER_CANISTER : ICP_LEDGER_CANISTER;

  // >>> MIRRORED-LIMITS-BEGIN  (tests/money_safety/tests/ui_limits.rs reads this fence)
  // ===========================================================================
  // MIRRORED CANISTER LIMITS -- THE ONLY NUMBERS IN THIS FILE (docs/DEFECTS.md T-26)
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
  // src/table_canister/src/lib.rs -- MIRRORED, keep in step:
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
  // deposit of D is credited D - fee -- and at the approve floor that lands below
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

  // THE QUICK CHIPS (QuickAmounts.svelte): the table's minimum buy-in and
  // twice it, as the field would be typed in its current unit.
  const quickAmounts = $derived.by(() => {
    if (minBuyIn === null || minBuyIn === undefined) return [];
    let buyIn;
    try { buyIn = BigInt(minBuyIn); } catch { return []; }
    if (buyIn <= 0n) return [];
    const text = (v) => (isBTC && inputUnit === 'sats' ? v.toString() : formatPlain(v));
    return [
      { id: 'min', label: 'Min buy-in', text: text(buyIn), hint: 'The table\'s minimum buy-in' },
      { id: 'double', label: '2x', text: text(buyIn * 2n), hint: 'Twice the table\'s minimum buy-in' },
    ];
  });

  // ==========================================================================
  // IS THIS CANISTER ID ONE THIS BUILD HAS EVER HEARD OF?
  // ASKED BEFORE ANY ADDRESS IS DERIVED AND BEFORE ANY TRANSFER IS ADDRESSED.
  // ==========================================================================
  //
  // docs/SECURITY-FINDINGS.md FINDING 42. `tableCanisterId` reaches this
  // component from `routes/+page.svelte`, which took it out of
  // `lobby.get_tables()` -- an ordinary QUERY one replica answers out of its own
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

  // CAN THIS TABLE PAY BACK WHAT IT ALREADY HOLDS? ASKED BEFORE, NOT AFTER
  // (docs/SECURITY-FINDINGS.md FINDING 35 / docs/DEFECTS.md E-70). A query, run
  // on mount: a warning that appears after the button is pressed is a receipt.
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
  // A canister below its freezing threshold rejects EVERY update call at once.
  // The figures live in tools/cycles/burn-table.json; the canister's own
  // sliding window is what this reads.
  let runway = $state(null);

  async function loadRunway() {
    runway = await readCycleRunway(tableActor);
  }

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

  // Opens on the shortfall a Sit tap found, when there is one: e8s to ICP for
  // an ICP table (trailing zeros dropped), sats as they are for a BTC table.
  const openingAmount = (() => {
    const n = initialAmount === null || initialAmount === undefined ? null : Number(initialAmount);
    if (n === null || !Number.isFinite(n) || n <= 0) return '';
    if (currency === 'BTC') return String(Math.ceil(n));
    return (n / 1e8).toFixed(8).replace(/\.?0+$/, '');
  })();
  let depositAmount = $state(openingAmount);
  let processing = $state(false);
  // A string is this component's own refusal, shown as written; an object is a
  // canister's or the ledger's answer, already turned into a sentence.
  let error = $state(null);
  let walletBalance = $state(null);
  let loadingBalance = $state(true);
  // YOUR table deposit address, derived locally. Empty until it is derived, and
  // deliberately left empty when the canister's own answer disagrees with it.
  let tableDepositAddress = $state('');
  let depositAddressWarning = $state(null);
  let claiming = $state(false);

  // Wallet source: 'ii' (Internet Identity) or 'oisy' (OISY Wallet). OISY
  // signs on mainnet only, so the toggle renders on a mainnet build only.
  let walletSource = $state('ii');

  // The route: 'wallet' (approve + pull from the connected wallet) or
  // 'address' (send to the derived address, then claim). Null follows the
  // balance: the wallet route when it can pay, the address otherwise.
  let route = $state(null);

  // BTC-specific state
  let btcDepositAddress = $state('');
  let loadingBtcAddress = $state(false);
  let btcAddressError = $state(null);
  let updatingBtcBalance = $state(false);
  let btcUpdateResult = $state(null);
  let depositMethod = $state('ckbtc'); // 'ckbtc' or 'btc' for BTC tables
  let inputUnit = $state('sats'); // 'sats' or 'btc' for BTC input mode

  // The one quote for the page (lib/prices.js): null renders no fiat hint.
  let prices = $state(null);

  // THE FLOW: which step of the deposit is running, since when.
  let flow = $state({ phase: FLOW.IDLE, steps: [], current: 0, startedAt: null });
  // THE RECEIPT: what moved, shown until Done.
  let receipt = $state(null);

  function beginFlow(steps) {
    flow = { phase: FLOW.RUNNING, steps, current: 0, startedAt: Date.now() };
  }
  function advanceFlow() {
    flow = { ...flow, current: Math.min(flow.current + 1, flow.steps.length - 1), startedAt: Date.now() };
  }
  function finishFlow() {
    flow = { ...flow, phase: FLOW.DONE };
  }
  function failFlow() {
    flow = { ...flow, phase: FLOW.FAILED };
  }
  function resetFlow() {
    flow = { phase: FLOW.IDLE, steps: [], current: 0, startedAt: null };
  }

  /** A canister's or the ledger's refusal, as a sentence with the raw text kept. */
  function fail(e) {
    error = describeCashierFailure(e);
  }

  const errorView = $derived(
    error === null ? null : (typeof error === 'string' ? { message: error, detail: null } : error)
  );

  // The input's own floor, in whichever unit the player is typing.
  const inputMinAttr = $derived(
    isBTC && inputUnit === 'sats' ? MIN_DEPOSIT.toString() : formatPlain(MIN_DEPOSIT)
  );

  // Convert user input to smallest unit (sats or e8s): the float floor the
  // deposit always used (lib/cashier-format.js floatToSmallest).
  function inputToSmallestUnit(amount) {
    if (isBTC && inputUnit === 'sats') {
      const n = Math.floor(Number(amount));
      return Number.isFinite(n) && n > 0 ? BigInt(n) : 0n;
    }
    return floatToSmallest(amount);
  }

  const formatWithUnit = (smallestUnit) => formatCashier(smallestUnit, currencySymbol, { placeholder: '...' });

  const perTokenUsd = $derived(prices ? (isBTC ? prices.btcUsd : prices.icpUsd) : null);

  // Derive YOUR deposit address locally, then ask the canister and compare.
  // The comparison NEVER prefers the canister's answer.
  async function deriveAndVerifyDepositAddress(principal) {
    depositAddressWarning = null;
    tableDepositAddress = '';
    if (!tableCanisterId) {
      depositAddressWarning = 'This table has no canister id in this build, so no deposit address can be derived.';
      return;
    }
    let derived;
    try {
      // THE TRUST ROOT IS CHECKED INSIDE THIS CALL, NOT HERE (FINDING 42).
      derived = deriveTrustedDepositAddress(tableCanisterId, principal).address;
    } catch (e) {
      depositAddressWarning = e.message || 'Could not derive your deposit address.';
      return;
    }
    // Show the derived address first: it is the trustworthy one, and a canister
    // that will not answer must not be able to hide it.
    tableDepositAddress = derived;
    try {
      const reported = await tableActor.get_deposit_address();
      const shared = accountIdentifierHex(tableCanisterId, null);
      const { agrees, safeToShow, reason } = checkAgainstCanister(derived, reported, shared);
      if (!agrees) {
        depositAddressWarning = reason;
        // `safeToShow` is the whole judgement: any disagreement other than the
        // pre-FINDING-34 shared account means the derivations differ, so show nothing.
        if (!safeToShow) tableDepositAddress = '';
      }
    } catch (e) {
      console.error('could not cross-check the deposit address:', e);
    }
  }

  // Sweep whatever is at YOUR deposit address into your table balance.
  async function claimExternalDeposit() {
    // Nothing at an unpinned canister is yours to sweep.
    if (!tableIsTrusted) {
      error = untrustedReason;
      return;
    }
    claiming = true;
    error = null;
    beginFlow([
      { id: 'claim', title: 'Claiming what arrived at your deposit address', hint: 'The table sweeps it into your balance', expectedMs: 1750 },
      { id: 'credited', title: 'Credited to your table balance', hint: '', expectedMs: null },
    ]);
    try {
      const result = await tableActor.claim_external_deposit();
      if ('Ok' in result) {
        finishFlow();
        receipt = {
          title: 'Deposit claimed',
          lead: 'What had arrived at your deposit address is in your table balance now.',
          rows: [{ id: 'balance', label: 'Table balance now', value: formatWithUnit(result.Ok), strong: true }],
        };
        await loadWalletBalance();
        onDepositSuccess?.();
      } else {
        failFlow();
        fail(result.Err);
      }
    } catch (e) {
      console.error('claim_external_deposit failed:', e);
      failFlow();
      fail(e);
    }
    claiming = false;
  }

  // Get user's balance from their wallet (ICP or ckBTC)
  async function loadWalletBalance() {
    loadingBalance = true;
    try {
      const agent = await auth.getAgent();
      const principal = await agent.getPrincipal();

      // YOUR table deposit address. Derived from the canister id and your own
      // principal, then CHECKED against what the canister says -- never taken
      // from it. docs/SECURITY-FINDINGS.md FINDING 34, FINDING 40.
      if (!isBTC) {
        await deriveAndVerifyDepositAddress(principal);
      }

      const ledgerIdlFactory = ({ IDL }) => {
        const Account = IDL.Record({
          owner: IDL.Principal,
          subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
        });
        return IDL.Service({
          icrc1_balance_of: IDL.Func([Account], [IDL.Nat], ['query']),
        });
      };

      const ledgerActor = Actor.createActor(ledgerIdlFactory, {
        agent,
        canisterId: ledgerCanisterId,
      });

      const balance = await ledgerActor.icrc1_balance_of({
        owner: principal,
        subaccount: [],
      });

      walletBalance = Number(balance);
    } catch (e) {
      console.error(`Failed to load ${currencySymbol} wallet balance:`, e);
      walletBalance = 0;
    }
    loadingBalance = false;
  }

  // Get BTC deposit address from the table canister
  async function loadBtcDepositAddress() {
    if (!isBTC || !tableActor) return;

    // THE FOURTH MONEY DOOR (docs/SECURITY-FINDINGS.md FINDING 45): the BTC
    // address is FETCHED, not derived, so a substituted table id would put an
    // attacker's address on screen. Refused for an unpinned table.
    if (!tableIsTrusted) {
      btcAddressError = untrustedReason;
      return;
    }

    if (!authState.isAuthenticated) {
      btcAddressError = 'Please log in with Internet Identity to get a BTC deposit address';
      return;
    }

    loadingBtcAddress = true;
    btcAddressError = null;

    try {
      const result = await tableActor.get_btc_deposit_address();
      if ('Ok' in result) {
        btcDepositAddress = result.Ok;
      } else if ('Err' in result) {
        btcAddressError = result.Err;
      }
    } catch (e) {
      console.error('Failed to get BTC deposit address:', e);
      btcAddressError = e.message || 'Failed to get deposit address';
    }

    loadingBtcAddress = false;
  }

  // Update BTC balance after sending Bitcoin
  async function handleUpdateBtcBalance() {
    if (!tableActor) return;

    updatingBtcBalance = true;
    btcUpdateResult = null;
    error = null;

    try {
      const result = await tableActor.update_btc_balance();
      if ('Ok' in result) {
        const statuses = result.Ok;
        const minted = statuses.filter(s => 'Minted' in s);
        if (minted.length > 0) {
          const totalMinted = minted.reduce((sum, s) => sum + Number(s.Minted.minted_amount), 0);
          btcUpdateResult = `Success! ${formatWithUnit(totalMinted)} minted to your wallet.`;
          await loadWalletBalance();
        } else if (statuses.some(s => 'Checked' in s)) {
          btcUpdateResult = 'UTXOs found and being processed. Please wait and try again.';
        } else {
          btcUpdateResult = 'No new deposits found yet.';
        }
      } else if ('Err' in result) {
        fail(result.Err);
      }
    } catch (e) {
      console.error('Failed to update BTC balance:', e);
      fail(e);
    }

    updatingBtcBalance = false;
  }

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
    return walletBalance;
  });

  const effectiveLoadingBalance = $derived.by(() => {
    if (walletSource === 'oisy') {
      return oisyState.loadingBalances;
    }
    return loadingBalance;
  });

  const effectiveHasEnoughBalance = $derived.by(() => {
    const bal = effectiveWalletBalance;
    // `>= minWalletBalance`, not `> minDeposit`: the deposit costs the minimum
    // PLUS both ledger fees, so this is the balance at which a deposit can
    // actually succeed.
    return bal !== null && toSmallest(bal) >= minWalletBalance;
  });

  const hasEnoughBalance = $derived(effectiveHasEnoughBalance);

  const walletUsd = $derived(
    walletBalance && !isBTC ? usdValue(walletBalance, perTokenUsd) : null
  );
  const oisyUsd = $derived(
    walletSource === 'oisy' && effectiveWalletBalance ? usdValue(effectiveWalletBalance, perTokenUsd) : null
  );

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

  // THE AMOUNT TYPED, AND WHAT IT COSTS. Every figure comes from the mirrored
  // fee and the typed amount; the harness recomputes the same rows from the
  // ledger's own fee (chain-agreement.mjs cost-summary).
  const typedSmallest = $derived.by(() => {
    if (!depositAmount || !(Number(depositAmount) > 0)) return 0n;
    return inputToSmallestUnit(depositAmount);
  });

  const typedUsd = $derived(typedSmallest > 0n ? usdValue(typedSmallest, perTokenUsd) : null);

  const cost = $derived(
    typedSmallest > 0n
      ? depositCost({ amount: typedSmallest, fee: TRANSFER_FEE })
      : null,
  );

  const summaryRows = $derived.by(() => {
    if (!cost) return [];
    return [
      { id: 'send', label: 'You send', value: formatWithUnit(cost.amount) },
      { id: 'fees', label: 'Ledger fees', note: 'charged twice: the approval and the pull', value: formatWithUnit(cost.fees), tone: 'muted' },
      { id: 'total', label: 'Total from your wallet', value: formatWithUnit(cost.total), strong: true },
      { id: 'credited', label: 'The table credits', value: formatWithUnit(cost.credited), tone: 'money' },
    ];
  });

  const primaryLabel = $derived.by(() => {
    if (effectiveRoute === 'btc') return updatingBtcBalance ? 'Checking…' : 'Check for deposit';
    if (effectiveRoute === 'address') return claiming ? 'Claiming…' : 'Claim deposit';
    if (processing) return 'Working…';
    return typedSmallest > 0n ? `Deposit ${formatWithUnit(typedSmallest)}` : `Deposit ${currencySymbol}`;
  });

  const primaryDisabled = $derived.by(() => {
    if (effectiveRoute === 'btc') return updatingBtcBalance || !btcDepositAddress || !tableIsTrusted;
    if (effectiveRoute === 'address') return claiming || !tableIsTrusted || !tableDepositAddress;
    return processing || !tableIsTrusted || !walletCanPay || typedSmallest <= 0n;
  });

  onMount(() => {
    loadWalletBalance();
    loadPrices().then((p) => { prices = p; }).catch(() => { prices = null; });
    loadSolvency();
    loadRunway();
    if (isBTC) {
      loadBtcDepositAddress();
    }
  });

  async function handlePrimary() {
    if (effectiveRoute === 'btc') return handleUpdateBtcBalance();
    if (effectiveRoute === 'address') return claimExternalDeposit();
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
    receipt = null;

    const approveAmount = amountSmallest + transferFee;
    const amountText = formatWithUnit(amountSmallest);

    try {
      if (walletSource === 'oisy') {
        beginFlow(depositSteps({ source: 'oisy', amountText }));
        try {
          // Step 1: DERIVE the destination. Never fetch it (FINDING 40). The
          // subaccount is a pure function of the SESSION principal -- the one
          // that will call `claim_external_deposit()` and be credited, which is
          // NOT the OISY wallet principal paying for it.
          const sessionPrincipal = await (await auth.getAgent()).getPrincipal();
          const depositSubaccountBytes = depositSubaccount(sessionPrincipal);

          // The canister is asked ONLY to catch a drift between the two
          // derivations, and its answer is never preferred. A disagreement
          // aborts: the very next statement moves real money.
          try {
            const reportedSub = await tableActor.get_deposit_subaccount();
            const reportedHex = Array.from(reportedSub ?? [], b => b.toString(16).padStart(2, '0')).join('');
            const derivedHex = Array.from(depositSubaccountBytes, b => b.toString(16).padStart(2, '0')).join('');
            if (reportedHex !== derivedHex) {
              error =
                'Refusing to send: this table reported a different deposit subaccount from the ' +
                'one derived from your principal, so one of the two answers is wrong and paying ' +
                'either would be guessing with your money. Nothing has been sent.';
              failFlow();
              processing = false;
              return;
            }
          } catch (checkError) {
            console.error('could not cross-check the deposit subaccount:', checkError);
          }

          // Step 2: Transfer from OISY wallet directly to the canister's deposit subaccount
          advanceFlow();

          const wallet = oisy.getWallet();

          if (!wallet) {
            error = 'OISY wallet not connected';
            failFlow();
            processing = false;
            return;
          }

          const canisterPrincipal = typeof tableCanisterId === 'string'
            ? Principal.fromText(tableCanisterId)
            : tableCanisterId;

          if (!wallet.transfer) {
            error = `OISY wallet does not support direct transfers. Please transfer ${currencySymbol} to your Internet Identity wallet first, then deposit from there.`;
            failFlow();
            processing = false;
            return;
          }

          const destination = {
            to: { owner: canisterPrincipal, subaccount: [depositSubaccountBytes] },
            amount: amountSmallest,
          };

          await wallet.transfer({
            ...(isBTC ? { params: destination } : { request: destination }),
            owner: oisyState.principal,
            ledgerCanisterId: ledgerCanisterId,
            options: { timeoutInMilliseconds: 300000 },
          });

          // Step 3: Claim the deposit (sweeps from subaccount to main balance)
          advanceFlow();
          const claimResult = await tableActor.claim_external_deposit();

          if ('Ok' in claimResult) {
            finishFlow();
            receipt = {
              title: 'Deposited from OISY',
              lead: `Paid by OISY ${shortId(oisyState.principal ?? '')}, credited to your signed-in identity ${shortId(authState.principal ?? '')}.`,
              rows: [
                { id: 'sent', label: 'Deposited', value: amountText },
                { id: 'balance', label: 'Table balance now', value: formatWithUnit(claimResult.Ok), strong: true },
              ],
            };
            await oisy.refreshBalances();
            onDepositSuccess?.();
          } else if ('Err' in claimResult) {
            failFlow();
            fail(claimResult.Err);
          }
        } catch (oisyError) {
          console.error('OISY deposit failed:', oisyError);
          failFlow();
          fail(oisyError);
        }

        processing = false;
        return;
      } else {
        // Handle Internet Identity wallet deposits (existing flow)
        beginFlow(depositSteps({ source: 'ii', amountText }));

        const agent = await auth.getAgent();

        const ledgerIdlFactory = ({ IDL }) => {
          const Account = IDL.Record({
            owner: IDL.Principal,
            subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
          });
          const ApproveArgs = IDL.Record({
            fee: IDL.Opt(IDL.Nat),
            memo: IDL.Opt(IDL.Vec(IDL.Nat8)),
            from_subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
            created_at_time: IDL.Opt(IDL.Nat64),
            amount: IDL.Nat,
            expected_allowance: IDL.Opt(IDL.Nat),
            expires_at: IDL.Opt(IDL.Nat64),
            spender: Account,
          });
          const ApproveError = IDL.Variant({
            GenericError: IDL.Record({ message: IDL.Text, error_code: IDL.Nat }),
            TemporarilyUnavailable: IDL.Null,
            Duplicate: IDL.Record({ duplicate_of: IDL.Nat }),
            BadFee: IDL.Record({ expected_fee: IDL.Nat }),
            AllowanceChanged: IDL.Record({ current_allowance: IDL.Nat }),
            CreatedInFuture: IDL.Record({ ledger_time: IDL.Nat64 }),
            TooOld: IDL.Null,
            Expired: IDL.Record({ ledger_time: IDL.Nat64 }),
            InsufficientFunds: IDL.Record({ balance: IDL.Nat }),
          });
          const ApproveResult = IDL.Variant({ Ok: IDL.Nat, Err: ApproveError });

          return IDL.Service({
            icrc2_approve: IDL.Func([ApproveArgs], [ApproveResult], []),
          });
        };

        const ledgerActor = Actor.createActor(ledgerIdlFactory, {
          agent,
          canisterId: ledgerCanisterId,
        });

        const tableCanisterPrincipal = typeof tableCanisterId === 'string'
          ? Principal.fromText(tableCanisterId)
          : tableCanisterId;

        const approveResult = await ledgerActor.icrc2_approve({
          fee: [],
          memo: [],
          from_subaccount: [],
          created_at_time: [],
          amount: approveAmount,
          expected_allowance: [],
          expires_at: [],
          spender: {
            owner: tableCanisterPrincipal,
            subaccount: [],
          },
        });

        if ('Err' in approveResult) {
          const errKey = Object.keys(approveResult.Err)[0];
          const errVal = approveResult.Err[errKey];
          if (errKey === 'InsufficientFunds') {
            const balanceDisplay = formatWithUnit(errVal.balance);
            error = `Insufficient funds. You have ${balanceDisplay} but need ${depositAmount} ${currencySymbol} plus the ${feeDisplay} ledger fee.`;
          } else if (errKey === 'GenericError') {
            fail(errVal.message);
          } else {
            fail(`Approval failed: ${errKey}`);
          }
          failFlow();
          processing = false;
          return;
        }

        advanceFlow();

        const depositResult = await tableActor.deposit(amountSmallest);

        if ('Ok' in depositResult) {
          finishFlow();
          receipt = {
            title: 'Deposited',
            lead: `${amountText} moved from your Internet Identity wallet to your balance at this table.`,
            rows: [
              { id: 'sent', label: 'Deposited', value: amountText },
              { id: 'fees', label: 'Ledger fees, from your wallet', value: ledgerFeesDisplay },
              { id: 'balance', label: 'Table balance now', value: formatWithUnit(depositResult.Ok), strong: true },
            ],
          };
          loadWalletBalance();
          onDepositSuccess?.();
        } else if ('Err' in depositResult) {
          failFlow();
          fail(depositResult.Err);
        }
      }
    } catch (e) {
      console.error('Deposit error:', e);
      failFlow();
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
    receipt = null;
    resetFlow();
    onClose();
  }

  // ONE dismissal contract for every dialog in this app (docs/DEFECTS.md T-13).
  function onWindowKeydown(e) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="modal-backdrop" onclick={onClose} role="presentation"></div>

<div
  class="modal-content"
  class:btc-modal={isBTC}
  role="dialog"
  aria-labelledby="deposit-modal-title"
  data-table-trust={tableIsTrusted ? 'pinned' : 'refused'}
  data-route={effectiveRoute}
  use:scrollLock
>
  <div class="modal-header">
    <h2 id="deposit-modal-title">
      {#if isBTC}
        <svg width="22" height="22" viewBox="0 0 64 64" aria-hidden="true">
          <path fill="var(--cd-btc)" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
          <path fill="var(--cd-card-face)" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
        </svg>
      {:else}
        <IcpLogo size={22} />
      {/if}
      <span>
        Deposit {currencySymbol}
        <span class="title-sub">Into your balance at this table</span>
      </span>
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close deposit modal">×</button>
  </div>

  <div class="modal-body">
    <!-- THE FIVE PROTECTED NOTICES, INSIDE THE DIALOG (HARD RULE 2, docs/DEFECTS.md T-31).
         The page's trust bar is behind this dialog's scrim, so the same words
         are restated here, first, as the trust bar's neutral strip. -->
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
        title={receipt.title}
        lead={receipt.lead}
        rows={receipt.rows}
        btc={isBTC}
        onDone={finishAndClose}
      />
    {:else}
      <div class="cashier-grid">
        <!-- THE MONEY COLUMN: which wallet, how much, what it costs. -->
        <div class="cashier-col main">
          {#if isBTC}
            <div class="deposit-method-toggle" role="tablist" aria-label="Deposit method">
              <button type="button" role="tab" aria-selected={depositMethod === 'ckbtc'} class:active={depositMethod === 'ckbtc'} onclick={() => { depositMethod = 'ckbtc'; }}>
                <span class="method-label">I have ckBTC</span>
                <span class="method-hint">Instant</span>
              </button>
              <button type="button" role="tab" aria-selected={depositMethod === 'btc'} class:active={depositMethod === 'btc'} onclick={() => { depositMethod = 'btc'; }}>
                <span class="method-label">I have BTC</span>
                <span class="method-hint">About an hour</span>
              </button>
            </div>
          {/if}

          {#if showsWalletRoute}
            <!-- THE ROUTE: from the connected wallet, or from anywhere via
                 the derived address. The default follows what the wallet can
                 pay; both are always one tap away. -->
            {#if !isBTC}
              <div class="deposit-route" role="tablist" aria-label="Deposit route">
                <button type="button" role="tab" aria-selected={effectiveRoute === 'wallet'} class:active={effectiveRoute === 'wallet'} onclick={() => { route = 'wallet'; }}>
                  From this wallet
                </button>
                <button type="button" role="tab" aria-selected={effectiveRoute === 'address'} class:active={effectiveRoute === 'address'} onclick={() => { route = 'address'; }}>
                  From an exchange or another wallet
                </button>
              </div>
            {/if}

            <!-- Wallet Source Toggle (II vs OISY): a mainnet build only, where
                 OISY can sign. -->
            {#if IS_MAINNET_BUILD && effectiveRoute === 'wallet'}
              <div class="wallet-source-toggle" role="tablist" aria-label="Wallet">
                <button type="button" role="tab" aria-selected={walletSource === 'ii'} class:active={walletSource === 'ii'} onclick={() => { walletSource = 'ii'; }}>
                  <span class="source-label">Internet Identity</span>
                  <span class="source-hint">Your signed-in wallet</span>
                </button>
                <button type="button" role="tab" aria-selected={walletSource === 'oisy'} class:active={walletSource === 'oisy'} onclick={() => { walletSource = 'oisy'; }}>
                  <span class="source-label">OISY Wallet</span>
                  <span class="source-hint">Pay from OISY</span>
                </button>
              </div>
            {/if}

            <!-- FROM: the paying wallet and what it holds. The II balance row
                 is always rendered on the ICP path: it is the figure the
                 screenshot harness compares with the ledger. -->
            {#if walletSource === 'oisy' && effectiveRoute === 'wallet'}
              {#if !oisyState.isConnected}
                <div class="from-card oisy" class:btc={isBTC}>
                  <div class="section-label"><span>From</span><span class="label-aside">OISY Wallet</span></div>
                  <p class="from-note">Top up your table balance straight from OISY. Keep the OISY popup open: it asks you to approve the transfer when you press Deposit.</p>
                  <button type="button" class="btn-connect-oisy" class:btc={isBTC} onclick={connectOisyWallet} disabled={oisyState.isConnecting}>
                    {#if oisyState.isConnecting}<span class="spinner"></span> Connecting…{:else}Connect OISY Wallet{/if}
                  </button>
                  {#if oisyState.error}<p class="from-note short">{oisyState.error}</p>{/if}
                </div>
              {:else}
                <div class="from-card oisy" class:btc={isBTC}>
                  <div class="section-label">
                    <span>From</span>
                    <button type="button" class="path-link" onclick={disconnectOisyWallet}>Disconnect OISY</button>
                  </div>
                  <div class="balance-row">
                    <span class="balance-label">OISY {isBTC ? 'ckBTC' : 'ICP'} balance</span>
                    <span class="balance-value oisy" class:loading={oisyState.loadingBalances} class:btc={isBTC}>
                      {#if oisyState.loadingBalances}
                        <span class="mini-spinner" class:btc={isBTC}></span>
                      {:else}
                        <span class="balance-crypto">{formatWithUnit(effectiveWalletBalance)}</span>
                        {#if oisyUsd !== null}<span class="usd-value">{formatUsd(oisyUsd)}</span>{/if}
                      {/if}
                    </span>
                  </div>
                  <p class="from-note">
                    Paid by OISY <code>{shortId(oisyState.principal ?? '')}</code>, credited to your
                    signed-in identity <code>{shortId(authState.principal ?? '')}</code>. Keep the OISY
                    popup open; it asks you to approve when you press Deposit.
                  </p>
                  {#if !oisyState.loadingBalances && !hasEnoughBalance}
                    <p class="from-note short">Not enough {isBTC ? 'ckBTC' : 'ICP'} in OISY for the minimum plus both network fees ({minWalletBalanceDisplay}). Add funds to OISY, or switch to Internet Identity.</p>
                  {/if}
                </div>
              {/if}
            {:else}
              <div class="from-card" class:btc={isBTC}>
                <div class="section-label">
                  <span>{effectiveRoute === 'address' ? 'Your wallet here' : 'From'}</span>
                  <button type="button" class="path-link" onclick={loadWalletBalance} disabled={loadingBalance}>Re-read</button>
                </div>
                <div class="balance-row">
                  <span class="balance-label">Internet Identity {isBTC ? 'ckBTC' : 'ICP'} balance</span>
                  <span class="balance-value" class:loading={loadingBalance} class:btc={isBTC}>
                    {#if loadingBalance}
                      <span class="mini-spinner" class:btc={isBTC}></span>
                    {:else}
                      <span class="balance-crypto">{formatWithUnit(walletBalance)}</span>
                      {#if walletUsd !== null}<span class="usd-value">{formatUsd(walletUsd)}</span>{/if}
                    {/if}
                  </span>
                </div>
                {#if !loadingBalance && !hasEnoughBalance}
                  <p class="from-note short">
                    Below the {minWalletBalanceDisplay} a deposit from this wallet needs (the minimum plus both network fees).
                    {#if isBTC}Switch to "I have BTC" to deposit real Bitcoin, or get ckBTC from an exchange.{:else}Send to your deposit address instead.{/if}
                  </p>
                {/if}
              </div>
            {/if}

            {#if effectiveRoute === 'wallet'}
              <!-- THE AMOUNT. -->
              <div class="form-section">
                <div class="section-label">
                  <label for="deposit-amount">Amount</label>
                  {#if isBTC}
                    <div class="unit-toggle">
                      <button type="button" class:active={inputUnit === 'sats'} onclick={() => { inputUnit = 'sats'; depositAmount = ''; }}>sats</button>
                      <button type="button" class:active={inputUnit === 'btc'} onclick={() => { inputUnit = 'btc'; depositAmount = ''; }}>BTC</button>
                    </div>
                  {/if}
                </div>
                <div class="input-row">
                  <div class="amount-field" class:btc={isBTC}>
                    <input
                      id="deposit-amount"
                      type="number"
                      inputmode="decimal"
                      step={isBTC && inputUnit === 'sats' ? "1" : "0.00000001"}
                      min={inputMinAttr}
                      placeholder={inputMinAttr}
                      bind:value={depositAmount}
                      disabled={processing || !walletCanPay}
                    />
                    <span class="input-suffix" class:btc={isBTC}>{isBTC ? inputUnit : 'ICP'}</span>
                  </div>
                  <button type="button" class="max-btn" class:btc={isBTC} onclick={setMaxAmount} disabled={processing || !walletCanPay}>MAX</button>
                </div>
                <!-- One tap to the table's buy-in (QuickAmounts.svelte); the
                     figures come from the table canister's config. -->
                <QuickAmounts
                  chips={quickAmounts}
                  selected={depositAmount}
                  btc={isBTC}
                  disabled={processing || !walletCanPay}
                  onPick={(text) => { depositAmount = text; }}
                />
                {#if typedSmallest > 0n}
                  <p class="conversion-preview">
                    {#if isBTC}
                      <span class="crypto-equiv">= {inputUnit === 'sats' ? `${formatPlain(typedSmallest)} BTC` : `${typedSmallest.toLocaleString('en-US')} sats`}</span>
                    {:else}
                      <span class="crypto-equiv">{formatWithUnit(typedSmallest)}</span>
                    {/if}
                    {#if typedUsd !== null}
                      <span class="usd-preview"><span class="usd-amount">{formatUsd(typedUsd)}</span></span>
                    {/if}
                  </p>
                {/if}
              </div>

              <CashierSummary caption="What this costs" rows={summaryRows} btc={isBTC} />
            {:else}
              <DepositAddressCard
                address={tableDepositAddress}
                warning={depositAddressWarning}
                deriving={loadingBalance}
              />
            {/if}

            <!-- THE LIMITS: figures first, the reasons one tap away. Every
                 figure is interpolated (docs/DEFECTS.md T-26) and every
                 labelled claim is what tools/shots/lib/chain-agreement.mjs
                 reads, so the sentences keep their words. -->
            <div class="minimum-notice" class:btc={isBTC}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="16" x2="12" y2="12"/>
                <line x1="12" y1="8" x2="12.01" y2="8"/>
              </svg>
              <div class="min-copy">
                <strong>Minimum to this address: {minExternalDepositDisplay}</strong>; from a
                connected wallet the lower minimum of {minDepositDisplay} applies.
                <strong>At or below {feeDisplay} nothing can move it</strong> (network fee
                {feeDisplay}, charged twice by the ledger, so you need {minWalletBalanceDisplay}
                in your wallet to deposit the minimum).
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
              address={tableIsTrusted ? btcDepositAddress : ''}
              loading={loadingBtcAddress}
              addressError={btcAddressError}
              minDisplay={btcNativeMinDisplay}
              minBtcDisplay={btcNativeMinBtcDisplay}
              minterFeeDisplay={btcNativeMinterFeeDisplay}
              result={btcUpdateResult}
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
              {solvency}
              {currency}
              context="deposit"
              onRefresh={refreshSolvency}
              refreshing={solvencyRefreshing}
            />
            <CycleRunwayNotice {runway} context="deposit" canisterId={tableCanisterId} />
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
        <div class="alert error" role="alert">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          <span class="alert-text">
            {errorView.message}
            {#if errorView.detail}<span class="alert-detail">{errorView.detail}</span>{/if}
          </span>
        </div>
      {/if}

      <p class="cashier-note">
        {#if effectiveRoute === 'btc'}
          Bitcoin sent to the address above becomes ckBTC in your wallet; deposit it to the table from the ckBTC tab.
        {:else if effectiveRoute === 'address'}
          Claim sweeps whatever has arrived at your deposit address into your balance at this table. You can withdraw back to your wallet at any time.
        {:else}
          This moves {isBTC ? 'ckBTC' : 'ICP'} from your {walletSource === 'oisy' ? 'OISY' : 'Internet Identity'} wallet to your balance at this table. You can withdraw back to your wallet at any time.
        {/if}
      </p>

      <!-- THE BUTTON ROW. On a phone it pins to the sheet's foot only once the
           solvency block and the runway panel have scrolled above it
           (lib/pin-after.js); before that it is in flow under them. -->
      <div class="actions" use:pinAfter={{ gates: ['.solvency', '.runway-notice'], scroller: '.modal-body' }}>
        <button type="button" class="btn-secondary" onclick={onClose} disabled={processing || claiming}>
          Cancel
        </button>
        <button
          type="button"
          class="btn-primary"
          class:btc={isBTC}
          onclick={handlePrimary}
          disabled={primaryDisabled}
        >
          {#if processing || claiming || updatingBtcBalance}<span class="spinner"></span>{/if}
          {primaryLabel}
        </button>
      </div>
    {/if}
  </div>
</div>

<style lang="scss">
  @use './cashier' as cashier;

  @include cashier.shell;
  @include cashier.columns;
  @include cashier.money;
  @include cashier.feedback;

  .modal-content { --cashier-w: 960px; }

  /* Segmented controls: the route, the wallet source, the BTC method. */
  .deposit-route,
  .wallet-source-toggle,
  .deposit-method-toggle {
    display: flex;
    gap: var(--cd-space-1);
    padding: var(--cd-space-1);
    border-radius: var(--cd-radius-card);
    background: var(--cd-surface-2);
  }

  .deposit-route button,
  .wallet-source-toggle button,
  .deposit-method-toggle button {
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    min-height: var(--cd-control-md);
    padding: var(--cd-space-2) var(--cd-space-3);
    border: 1px solid transparent;
    border-radius: var(--cd-radius-chip);
    background: transparent;
    color: var(--cd-ink-2);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-strong);
    line-height: 1.25;
    text-align: center;
    cursor: pointer;
    transition: background var(--cd-fast) var(--cd-ease), color var(--cd-fast) var(--cd-ease);
  }

  .deposit-route button.active,
  .wallet-source-toggle button.active {
    background: var(--cd-surface-4);
    border-color: var(--cd-line-strong);
    color: var(--cd-ink);
  }

  .deposit-method-toggle button.active { background: var(--cd-btc-dim); border-color: var(--cd-btc-line); color: var(--cd-btc); }

  .deposit-route button:hover:not(.active),
  .wallet-source-toggle button:hover:not(.active),
  .deposit-method-toggle button:hover:not(.active) { background: var(--cd-surface-3); color: var(--cd-ink-1); }

  .source-hint, .method-hint { font-size: var(--cd-text-xs); font-weight: var(--cd-weight-body); color: var(--cd-ink-2); }

  .from-card.oisy { border-color: var(--cd-accent-line); }

  .btn-connect-oisy {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-2);
    min-height: var(--cd-control-md);
    padding: 0 var(--cd-space-4);
    border: 0;
    border-radius: var(--cd-radius-chip);
    background: var(--cd-accent);
    color: var(--cd-accent-ink);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    cursor: pointer;
    box-shadow: var(--cd-gloss);
  }

  .btn-connect-oisy.btc { background: var(--cd-btc); color: var(--cd-ink-on-light); }
  .btn-connect-oisy:disabled { opacity: 0.6; cursor: not-allowed; }

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
    @include cashier.phone;

    .deposit-route button,
    .wallet-source-toggle button,
    .deposit-method-toggle button { min-height: var(--cd-touch-min); }

    .btn-connect-oisy { min-height: var(--cd-touch-min); }

    /* THE DEPOSIT ROW PINS TO THE FOOT OF THE SHEET, BUT ONLY AFTER THE
       WARNINGS: lib/pin-after.js adds `pinned` once the solvency block and
       the runway panel have their bottom edges above the line the row's top
       would sit on. Opaque, with a hairline; the body's bottom padding is
       cancelled so the pinned row sits flush. tools/shots/touch-targets.mjs
       measures the row at rest and at the end of the scroll. */
    .actions {
      order: 10;
      z-index: 2;
      margin: 0 calc(-1 * var(--cd-space-4)) calc(-1 * (var(--cd-space-5) + var(--cd-safe-bottom)));
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
