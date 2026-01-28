<script>
  import { auth } from '$lib/auth.js';
  import { oisy, formatOisyBalance } from '$lib/oisy.js';
  import { Actor } from '@dfinity/agent';
  import { Principal } from '@dfinity/principal';
  import { onMount } from 'svelte';
  import IcpLogo from './IcpLogo.svelte';

  const { tableActor, tableCanisterId, onClose, onDepositSuccess, currency = 'ICP' } = $props();

  // Subscribe to auth state to check if user is authenticated
  let authState = $state({ isAuthenticated: false });
  $effect(() => {
    const unsub = auth.subscribe(s => { authState = s; });
    return unsub;
  });

  // Subscribe to OISY wallet state
  let oisyState = $state({ isConnected: false, isConnecting: false, icpBalance: null, ckbtcBalance: null });
  $effect(() => {
    const unsub = oisy.subscribe(s => { oisyState = s; });
    return unsub;
  });

  let depositAmount = $state('');
  let processing = $state(false);
  let error = $state(null);
  let success = $state(null);
  let statusMessage = $state('');
  let walletBalance = $state(null);
  let loadingBalance = $state(true);
  let copied = $state(false);
  let accountId = $state('');
  let copiedAddress = $state(false);

  // Wallet source: 'ii' (Internet Identity) or 'oisy' (OISY Wallet)
  let walletSource = $state('ii');

  // BTC-specific state
  let btcDepositAddress = $state('');
  let loadingBtcAddress = $state(false);
  let btcAddressError = $state(null);
  let updatingBtcBalance = $state(false);
  let btcUpdateResult = $state(null);
  let depositMethod = $state('ckbtc'); // 'ckbtc' or 'btc' for BTC tables
  let inputUnit = $state('sats'); // 'sats' or 'btc' for BTC input mode

  // Ledger canister IDs
  const ICP_LEDGER_CANISTER = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
  const CKBTC_LEDGER_CANISTER = 'mxzaz-hqaaa-aaaar-qaada-cai';

  // Price state (fetched from CoinGecko)
  let icpPriceUsd = $state(null);
  let btcPriceUsd = $state(null);
  let priceLoading = $state(false);
  let priceError = $state(null);

  // Get the correct ledger based on currency
  const ledgerCanisterId = currency === 'BTC' ? CKBTC_LEDGER_CANISTER : ICP_LEDGER_CANISTER;

  // Currency-specific settings
  const isBTC = currency === 'BTC';
  const currencySymbol = isBTC ? 'BTC' : 'ICP';
  const unitName = isBTC ? 'sats' : 'e8s';
  const transferFee = isBTC ? 10n : 10000n; // 10 sats vs 0.0001 ICP
  const minDeposit = isBTC ? 1000n : 20000n; // 1000 sats vs 0.0002 ICP

  // CRC32 implementation
  function crc32(data) {
    let crc = 0xffffffff;
    const table = new Uint32Array(256);
    for (let i = 0; i < 256; i++) {
      let c = i;
      for (let j = 0; j < 8; j++) {
        c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1);
      }
      table[i] = c;
    }
    for (let i = 0; i < data.length; i++) {
      crc = table[(crc ^ data[i]) & 0xff] ^ (crc >>> 8);
    }
    return (crc ^ 0xffffffff) >>> 0;
  }

  // SHA-224 implementation
  function sha224Pure(message) {
    const H = new Uint32Array([
      0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939,
      0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4
    ]);
    const K = new Uint32Array([
      0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
      0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
      0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
      0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
      0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
      0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
      0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
      0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
    ]);
    const rotr = (x, n) => (x >>> n) | (x << (32 - n));
    const ch = (x, y, z) => (x & y) ^ (~x & z);
    const maj = (x, y, z) => (x & y) ^ (x & z) ^ (y & z);
    const sigma0 = x => rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22);
    const sigma1 = x => rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25);
    const gamma0 = x => rotr(x, 7) ^ rotr(x, 18) ^ (x >>> 3);
    const gamma1 = x => rotr(x, 17) ^ rotr(x, 19) ^ (x >>> 10);

    const msgLen = message.length;
    const bitLen = BigInt(msgLen) * 8n;
    const totalBeforePad = msgLen + 9;
    const padZeros = (64 - (totalBeforePad % 64)) % 64;
    const paddedLen = msgLen + 1 + padZeros + 8;

    const padded = new Uint8Array(paddedLen);
    padded.set(message);
    padded[msgLen] = 0x80;
    const view = new DataView(padded.buffer);
    view.setBigUint64(paddedLen - 8, bitLen, false);

    for (let i = 0; i < padded.length; i += 64) {
      const W = new Uint32Array(64);
      for (let j = 0; j < 16; j++) {
        W[j] = view.getUint32(i + j * 4, false);
      }
      for (let j = 16; j < 64; j++) {
        W[j] = (gamma1(W[j - 2]) + W[j - 7] + gamma0(W[j - 15]) + W[j - 16]) >>> 0;
      }
      let [a, b, c, d, e, f, g, h] = H;
      for (let j = 0; j < 64; j++) {
        const T1 = (h + sigma1(e) + ch(e, f, g) + K[j] + W[j]) >>> 0;
        const T2 = (sigma0(a) + maj(a, b, c)) >>> 0;
        h = g; g = f; f = e; e = (d + T1) >>> 0;
        d = c; c = b; b = a; a = (T1 + T2) >>> 0;
      }
      H[0] = (H[0] + a) >>> 0; H[1] = (H[1] + b) >>> 0;
      H[2] = (H[2] + c) >>> 0; H[3] = (H[3] + d) >>> 0;
      H[4] = (H[4] + e) >>> 0; H[5] = (H[5] + f) >>> 0;
      H[6] = (H[6] + g) >>> 0; H[7] = (H[7] + h) >>> 0;
    }
    const result = new Uint8Array(28);
    for (let i = 0; i < 7; i++) {
      result[i * 4] = (H[i] >> 24) & 0xff;
      result[i * 4 + 1] = (H[i] >> 16) & 0xff;
      result[i * 4 + 2] = (H[i] >> 8) & 0xff;
      result[i * 4 + 3] = H[i] & 0xff;
    }
    return result;
  }

  // Compute ICP Account ID from principal
  function computeAccountId(principal) {
    if (!principal) return '';
    try {
      const padding = new Uint8Array(32);
      const domainSeparator = new TextEncoder().encode('\x0Aaccount-id');
      const principalBytes = principal.toUint8Array();
      const data = new Uint8Array(domainSeparator.length + principalBytes.length + padding.length);
      data.set(domainSeparator, 0);
      data.set(principalBytes, domainSeparator.length);
      data.set(padding, domainSeparator.length + principalBytes.length);
      const hash = sha224Pure(data);
      const crc = crc32(hash);
      const accountIdBytes = new Uint8Array(32);
      accountIdBytes[0] = (crc >> 24) & 0xff;
      accountIdBytes[1] = (crc >> 16) & 0xff;
      accountIdBytes[2] = (crc >> 8) & 0xff;
      accountIdBytes[3] = crc & 0xff;
      accountIdBytes.set(hash, 4);
      return Array.from(accountIdBytes).map(b => b.toString(16).padStart(2, '0')).join('');
    } catch (e) {
      console.error('Failed to compute account ID:', e);
      return '';
    }
  }

  // Convert user input to smallest unit (sats or e8s)
  function inputToSmallestUnit(amount) {
    if (isBTC) {
      // For BTC, check the input unit mode
      if (inputUnit === 'sats') {
        return BigInt(Math.floor(Number(amount)));
      }
      return BigInt(Math.floor(Number(amount) * 100_000_000));
    }
    return BigInt(Math.floor(Number(amount) * 100_000_000));
  }

  // Format balance for display
  function formatBalance(smallestUnit) {
    if (smallestUnit === null || smallestUnit === undefined) return '...';
    const num = Number(smallestUnit);
    if (isBTC) {
      const btc = num / 100_000_000;
      if (btc >= 1) return `${btc.toFixed(4)} BTC`;
      if (num >= 1000) return `${(num / 1000).toFixed(1)}K sats`;
      return `${num} sats`;
    }
    return (num / 100_000_000).toFixed(4);
  }

  // Format for display with unit
  function formatWithUnit(smallestUnit) {
    if (smallestUnit === null || smallestUnit === undefined) return '...';
    const formatted = formatBalance(smallestUnit);
    if (!formatted.includes('BTC') && !formatted.includes('sats') && !formatted.includes('ICP')) {
      return `${formatted} ${currencySymbol}`;
    }
    return formatted;
  }

  // Get user's balance from their wallet (ICP or ckBTC)
  async function loadWalletBalance() {
    loadingBalance = true;
    try {
      const agent = await auth.getAgent();
      const principal = await agent.getPrincipal();

      // Compute account ID for display (for ICP deposits)
      if (!isBTC) {
        accountId = computeAccountId(principal);
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

    // Check if user is authenticated - ckBTC minter requires non-anonymous principal
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
        // Check if any were minted
        const minted = statuses.filter(s => 'Minted' in s);
        if (minted.length > 0) {
          const totalMinted = minted.reduce((sum, s) => sum + Number(s.Minted.minted_amount), 0);
          btcUpdateResult = `Success! ${formatBalance(totalMinted)} minted to your wallet.`;
          // Refresh wallet balance
          await loadWalletBalance();
        } else if (statuses.some(s => 'Checked' in s)) {
          btcUpdateResult = 'UTXOs found and being processed. Please wait and try again.';
        } else {
          btcUpdateResult = 'No new deposits found yet.';
        }
      } else if ('Err' in result) {
        error = result.Err;
      }
    } catch (e) {
      console.error('Failed to update BTC balance:', e);
      error = e.message || 'Failed to update balance';
    }

    updatingBtcBalance = false;
  }

  // Fetch exchange rates from CoinGecko (free, no auth required)
  async function loadPrices() {
    priceLoading = true;
    priceError = null;

    try {
      // CoinGecko simple price API - free tier, no API key needed
      const response = await fetch(
        'https://api.coingecko.com/api/v3/simple/price?ids=internet-computer,bitcoin&vs_currencies=usd'
      );

      if (response.ok) {
        const data = await response.json();
        if (data['internet-computer']?.usd) {
          icpPriceUsd = data['internet-computer'].usd;
        }
        if (data['bitcoin']?.usd) {
          btcPriceUsd = data['bitcoin'].usd;
        }
      } else {
        throw new Error('CoinGecko API error');
      }
    } catch (e) {
      console.error('Failed to fetch prices:', e);
      priceError = 'Failed to fetch prices';
    }

    priceLoading = false;
  }

  // Calculate USD value
  function getUsdValue(sats) {
    if (isBTC && btcPriceUsd) {
      return (sats / 100_000_000) * btcPriceUsd;
    } else if (!isBTC && icpPriceUsd) {
      return (sats / 100_000_000) * icpPriceUsd;
    }
    return null;
  }

  // Format USD
  function formatUsd(value) {
    if (value === null || value === undefined) return '';
    if (value < 0.01) return `~$${value.toFixed(4)}`;
    return `~$${value.toFixed(2)}`;
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

  // Disconnect OISY wallet
  async function disconnectOisyWallet() {
    await oisy.disconnect();
    walletSource = 'ii';
  }

  // Get the effective wallet balance based on source
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
    return bal !== null && bal > Number(minDeposit);
  });

  onMount(() => {
    loadWalletBalance();
    loadPrices();
    if (isBTC) {
      loadBtcDepositAddress();
    }
  });

  function copyDepositAddress() {
    navigator.clipboard.writeText(tableCanisterId);
    copied = true;
    setTimeout(() => copied = false, 2000);
  }

  async function handleDeposit() {
    if (!depositAmount || Number(depositAmount) <= 0) {
      error = 'Please enter a valid amount';
      return;
    }

    const amountSmallest = inputToSmallestUnit(depositAmount);

    if (amountSmallest < minDeposit) {
      const minDisplay = isBTC ? '0.00001 BTC (1000 sats)' : '0.0002 ICP';
      error = `Minimum deposit is ${minDisplay}`;
      return;
    }

    const currentBalance = effectiveWalletBalance;
    if (currentBalance !== null && amountSmallest > BigInt(currentBalance)) {
      error = `Insufficient balance. You have ${formatWithUnit(currentBalance)} in your wallet.`;
      return;
    }

    if (!tableCanisterId) {
      error = 'Table canister ID not available';
      return;
    }

    processing = true;
    error = null;

    const approveAmount = amountSmallest + transferFee;

    try {
      // Handle OISY wallet deposits
      if (walletSource === 'oisy') {
        statusMessage = 'Please approve the transaction in OISY wallet...';

        // Use OISY wallet for approval
        if (isBTC) {
          await oisy.approveCkbtc(approveAmount, tableCanisterId);
        } else {
          await oisy.approveIcp(approveAmount, tableCanisterId);
        }

        statusMessage = `Transferring ${currencySymbol} to poker table...`;

        // Now call deposit on the table canister
        const depositResult = await tableActor.deposit(amountSmallest);

        if ('Ok' in depositResult) {
          const newBalance = formatWithUnit(depositResult.Ok);
          success = `Deposited ${depositAmount} ${currencySymbol} from OISY! Table balance: ${newBalance}`;
          // Refresh OISY balances
          await oisy.refreshBalances();
          setTimeout(() => {
            onDepositSuccess?.();
            onClose();
          }, 2000);
        } else if ('Err' in depositResult) {
          error = depositResult.Err;
        }
      } else {
        // Handle Internet Identity wallet deposits (existing flow)
        statusMessage = 'Requesting approval from your wallet...';

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
            const feeDisplay = isBTC ? '10 sats' : '0.0001 ICP';
            error = `Insufficient funds. You have ${balanceDisplay} but need ${depositAmount} ${currencySymbol} plus ${feeDisplay} fee.`;
          } else if (errKey === 'GenericError') {
            error = errVal.message;
          } else {
            error = `Approval failed: ${errKey}`;
          }
          processing = false;
          return;
        }

        statusMessage = `Transferring ${currencySymbol} to poker table...`;

        const depositResult = await tableActor.deposit(amountSmallest);

        if ('Ok' in depositResult) {
          const newBalance = formatWithUnit(depositResult.Ok);
          success = `Deposited ${depositAmount} ${currencySymbol}! Table balance: ${newBalance}`;
          loadWalletBalance();
          setTimeout(() => {
            onDepositSuccess?.();
            onClose();
          }, 2000);
        } else if ('Err' in depositResult) {
          error = depositResult.Err;
        }
      }
    } catch (e) {
      console.error('Deposit error:', e);
      error = e.message || 'Deposit failed';
    }
    processing = false;
    statusMessage = '';
  }

  function setMaxAmount() {
    const minRequired = Number(minDeposit);
    if (walletBalance !== null && walletBalance > minRequired) {
      const feeBuffer = isBTC ? 20 : 20000;
      const maxSmallest = Math.max(0, walletBalance - feeBuffer);
      if (isBTC && inputUnit === 'sats') {
        depositAmount = String(maxSmallest);
      } else {
        const maxDisplay = maxSmallest / 100_000_000;
        depositAmount = isBTC ? maxDisplay.toFixed(8) : maxDisplay.toFixed(4);
      }
    }
  }

  // Use effective balance (from II or OISY depending on walletSource)
  const hasEnoughBalance = $derived(effectiveWalletBalance !== null && effectiveWalletBalance > Number(minDeposit));
</script>

<div class="modal-backdrop" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="button" tabindex="-1" aria-label="Close modal"></div>

<div class="modal-content" class:btc-modal={isBTC} role="dialog" aria-labelledby="deposit-modal-title">
  <div class="modal-header">
    <h2 id="deposit-modal-title">
      {#if isBTC}
        <svg width="20" height="20" viewBox="0 0 64 64">
          <path fill="#f7931a" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
          <path fill="#fff" d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.51-.875-1.4 5.616c-.924-.23-1.872-.447-2.814-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.019 4.57 1.139c.85.213 1.682.436 2.502.646l-1.453 5.834 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726l-1.434 5.745 3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.384-4.739 1.527-4.36-.076-6.875-3.226-8.515 2.294-.529 4.022-2.038 4.483-5.155zM38.086 38.69c-1.085 4.36-8.426 2.003-10.806 1.412l1.928-7.729c2.38.594 10.012 1.77 8.878 6.317zm1.086-11.312c-.99 3.966-7.1 1.951-9.082 1.457l1.748-7.01c1.982.494 8.365 1.416 7.334 5.553z"/>
        </svg>
      {:else}
        <IcpLogo size={20} />
      {/if}
      Deposit {currencySymbol} to Table
    </h2>
    <button class="close-btn" onclick={onClose} aria-label="Close deposit modal">×</button>
  </div>

  <div class="modal-body">
    <!-- BTC Deposit Method Toggle -->
    {#if isBTC}
      <div class="deposit-method-toggle">
        <button
          class:active={depositMethod === 'ckbtc'}
          onclick={() => depositMethod = 'ckbtc'}
        >
          <span class="method-icon">⚡</span>
          I have ckBTC
          <span class="method-hint">Instant</span>
        </button>
        <button
          class:active={depositMethod === 'btc'}
          onclick={() => depositMethod = 'btc'}
        >
          <svg width="16" height="16" viewBox="0 0 64 64">
            <path fill="currentColor" d="M63.04 39.741c-4.275 17.143-21.638 27.576-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
          </svg>
          I have BTC
          <span class="method-hint">~1 hour</span>
        </button>
      </div>
    {/if}

    <!-- Wallet Source Toggle (II vs OISY) -->
    {#if (!isBTC || depositMethod === 'ckbtc')}
      <div class="wallet-source-toggle">
        <button
          class:active={walletSource === 'ii'}
          onclick={() => walletSource = 'ii'}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
            <circle cx="12" cy="7" r="4"/>
          </svg>
          <span class="source-label">Internet Identity</span>
          <span class="source-hint">Your II wallet</span>
        </button>
        <button
          class:active={walletSource === 'oisy'}
          onclick={() => walletSource = 'oisy'}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="4" width="20" height="16" rx="2"/>
            <path d="M6 8h.01M6 12h.01M6 16h.01M10 8h8M10 12h8M10 16h8"/>
          </svg>
          <span class="source-label">OISY Wallet</span>
          <span class="source-hint">Top up directly</span>
        </button>
      </div>
    {/if}

    <!-- OISY Wallet Connection Section -->
    {#if walletSource === 'oisy' && (!isBTC || depositMethod === 'ckbtc')}
      {#if !oisyState.isConnected}
        <div class="oisy-connect-section" class:btc={isBTC}>
          <div class="oisy-icon">
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <rect x="2" y="4" width="20" height="16" rx="2"/>
              <circle cx="12" cy="12" r="3"/>
              <path d="M2 10h4M18 10h4M2 14h4M18 14h4"/>
            </svg>
          </div>
          <h3>Connect OISY Wallet</h3>
          <p>Top up your poker balance directly from OISY without transferring tokens first.</p>
          <button
            class="btn-connect-oisy"
            class:btc={isBTC}
            onclick={connectOisyWallet}
            disabled={oisyState.isConnecting}
          >
            {#if oisyState.isConnecting}
              <span class="spinner"></span>
              Connecting...
            {:else}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M13.8 12H3"/>
              </svg>
              Connect OISY Wallet
            {/if}
          </button>
          {#if oisyState.error}
            <div class="oisy-error">{oisyState.error}</div>
          {/if}
        </div>
      {:else}
        <!-- OISY Connected - Show Balance -->
        <div class="balance-section oisy" class:btc={isBTC}>
          <div class="oisy-connected-header">
            <span class="connected-badge">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                <polyline points="20 6 9 17 4 12"/>
              </svg>
              OISY Connected
            </span>
            <button class="disconnect-btn" onclick={disconnectOisyWallet}>Disconnect</button>
          </div>
          <div class="balance-row">
            <span class="balance-label">Your OISY {isBTC ? 'ckBTC' : 'ICP'} Balance</span>
            <span class="balance-value oisy" class:loading={oisyState.loadingBalances} class:btc={isBTC}>
              {#if oisyState.loadingBalances}
                <span class="mini-spinner" class:btc={isBTC}></span>
              {:else}
                <span class="balance-crypto">{formatWithUnit(effectiveWalletBalance)}</span>
                {#if effectiveWalletBalance && !priceLoading}
                  <span class="usd-value">({formatUsd(getUsdValue(effectiveWalletBalance))})</span>
                {/if}
              {/if}
            </span>
          </div>
          {#if !oisyState.loadingBalances && !hasEnoughBalance}
            <div class="no-balance-warning oisy" class:btc={isBTC}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="8" x2="12" y2="12"/>
                <line x1="12" y1="16" x2="12.01" y2="16"/>
              </svg>
              <div class="warning-content">
                <strong>No {isBTC ? 'ckBTC' : 'ICP'} in OISY wallet</strong>
                <p>Add funds to your OISY wallet first, or switch to Internet Identity.</p>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    {/if}

    <!-- Wallet Balance Display (for ckBTC/ICP flow) - Only show for II -->
    {#if walletSource === 'ii' && (!isBTC || depositMethod === 'ckbtc')}
      <div class="balance-section" class:btc={isBTC}>
        <div class="balance-row">
          <span class="balance-label">Your {isBTC ? 'ckBTC' : 'ICP'} Wallet Balance</span>
          <span class="balance-value" class:loading={loadingBalance} class:btc={isBTC}>
            {#if loadingBalance}
              <span class="mini-spinner" class:btc={isBTC}></span>
            {:else}
              <span class="balance-crypto">{formatWithUnit(walletBalance)}</span>
              {#if walletBalance && !priceLoading}
                <span class="usd-value">({formatUsd(getUsdValue(walletBalance))})</span>
              {/if}
            {/if}
          </span>
        </div>
        {#if !loadingBalance && !hasEnoughBalance}
          <div class="no-balance-warning" class:btc={isBTC}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="8" x2="12" y2="12"/>
              <line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>
            <div class="warning-content">
              <strong>No {isBTC ? 'ckBTC' : 'ICP'} in wallet</strong>
              {#if isBTC}
                <p>Switch to "I have BTC" tab to deposit real Bitcoin, or get ckBTC from an exchange.</p>
              {:else}
                <p>Transfer ICP from an exchange or another wallet to your II account first.</p>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      {#if hasEnoughBalance && (walletSource === 'ii' || oisyState.isConnected)}
        <div class="form-section">
          <div class="label-row">
            <label for="deposit-amount">Deposit Amount</label>
            {#if isBTC}
              <div class="unit-toggle">
                <button
                  class:active={inputUnit === 'sats'}
                  onclick={() => { inputUnit = 'sats'; depositAmount = ''; }}
                >sats</button>
                <button
                  class:active={inputUnit === 'btc'}
                  onclick={() => { inputUnit = 'btc'; depositAmount = ''; }}
                >BTC</button>
              </div>
            {/if}
          </div>
          <div class="input-row">
            <input
              id="deposit-amount"
              type="number"
              step={isBTC ? (inputUnit === 'sats' ? "1" : "0.00000001") : "0.0001"}
              min={isBTC ? (inputUnit === 'sats' ? "1000" : "0.00001") : "0.0002"}
              placeholder={isBTC ? (inputUnit === 'sats' ? "5000" : "0.00000000") : "0.0000"}
              bind:value={depositAmount}
              disabled={processing}
            />
            <span class="input-suffix" class:btc={isBTC}>{isBTC ? inputUnit : 'ICP'}</span>
            <button class="max-btn" class:btc={isBTC} onclick={setMaxAmount} disabled={processing}>
              MAX
            </button>
          </div>
          {#if depositAmount && Number(depositAmount) > 0}
            <div class="conversion-preview">
              <div class="conversion-row">
                {#if isBTC}
                  {#if inputUnit === 'sats'}
                    <span class="crypto-equiv">= {(Number(depositAmount) / 100_000_000).toFixed(8)} BTC</span>
                  {:else}
                    <span class="crypto-equiv">= {Math.floor(Number(depositAmount) * 100_000_000).toLocaleString()} sats</span>
                  {/if}
                {/if}
                {#if !priceLoading}
                  {@const amountSats = isBTC
                    ? (inputUnit === 'sats' ? Number(depositAmount) : Number(depositAmount) * 100_000_000)
                    : Number(depositAmount) * 100_000_000}
                  {@const usdVal = getUsdValue(amountSats)}
                  {#if usdVal !== null}
                    <span class="usd-preview">
                      <span class="usd-amount">{formatUsd(usdVal)}</span>
                    </span>
                  {/if}
                {/if}
              </div>
            </div>
          {/if}
          <div class="minimum-notice" class:btc={isBTC}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="16" x2="12" y2="12"/>
              <line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            {#if isBTC}
              <span><strong>Minimum: 1,000 sats</strong> (Fee: 10 sats)</span>
            {:else}
              <span><strong>Minimum deposit: 0.0002 ICP</strong> (Network fee: 0.0001 ICP)</span>
            {/if}
          </div>
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

        {#if statusMessage}
          <div class="status-message">
            <span class="spinner"></span>
            {statusMessage}
          </div>
        {/if}

        <div class="actions">
          <button class="btn-secondary" onclick={onClose} disabled={processing}>
            Cancel
          </button>
          <button
            class="btn-primary"
            class:btc={isBTC}
            onclick={handleDeposit}
            disabled={processing || !depositAmount || Number(depositAmount) <= 0}
          >
            {#if processing}
              <span class="spinner"></span>
              Processing...
            {:else}
              Deposit to Table
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
              This transfers ckBTC from your wallet to your poker table balance.
              You can withdraw back to your wallet at any time.
            </p>
          {:else}
            <p>
              This transfers ICP from your Internet Identity wallet to your poker table balance.
              You can withdraw back to your wallet at any time.
            </p>
          {/if}
        </div>
      {:else if !loadingBalance}
        <!-- No ckBTC balance - show help -->
        <div class="deposit-info-section btc">
          <h3>How to Get ckBTC</h3>
          <p class="info-text">ckBTC is Bitcoin on ICP. You can get it by converting real BTC or buying on exchanges.</p>
          <div class="funding-options">
            <div class="option">
              <strong>Switch to BTC tab above</strong>
              <p>Deposit real Bitcoin directly - it will be converted to ckBTC automatically.</p>
            </div>
            <div class="option">
              <strong>Or buy ckBTC</strong>
              <p>Purchase ckBTC on ICP DEXs like ICPSwap or Sonic.</p>
            </div>
          </div>
        </div>
        <div class="actions">
          <button class="btn-secondary" onclick={onClose}>
            Close
          </button>
          <button class="btn-primary btc" onclick={loadWalletBalance}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
            </svg>
            Refresh Balance
          </button>
        </div>
      {/if}
    {/if}

    <!-- Native BTC Deposit Flow -->
    {#if isBTC && depositMethod === 'btc'}
      <div class="btc-deposit-section">
        <div class="btc-deposit-header">
          <h3>Deposit Bitcoin</h3>
          <p>Send BTC to this address. After 6 confirmations (~1 hour), click "Check for Deposit" to mint ckBTC.</p>
        </div>

        {#if loadingBtcAddress}
          <div class="loading-address">
            <span class="spinner btc"></span>
            Getting your Bitcoin deposit address...
          </div>
        {:else if btcAddressError}
          <div class="alert error">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="8" x2="12" y2="12"/>
              <line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>
            {btcAddressError}
          </div>
        {:else if btcDepositAddress}
          <div class="btc-address-box">
            <label>Your Bitcoin Deposit Address</label>
            <div class="address-display">
              <span class="address-text">{btcDepositAddress}</span>
            </div>
            <button
              class="copy-btn"
              onclick={() => {
                navigator.clipboard.writeText(btcDepositAddress);
                copiedAddress = true;
                setTimeout(() => copiedAddress = false, 2000);
              }}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
              {copiedAddress ? 'Copied!' : 'Copy Address'}
            </button>
          </div>

          <div class="btc-minimum-warning">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
              <line x1="12" y1="9" x2="12" y2="13"/>
              <line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
            <div>
              <strong>Minimum: 10,000 sats</strong>
              <span>Smaller amounts may not be processed (ckBTC has a ~2,000 sat fee)</span>
            </div>
          </div>

          <div class="btc-steps">
            <div class="step">
              <span class="step-num">1</span>
              <span>Send BTC to the address above (min 10,000 sats)</span>
            </div>
            <div class="step">
              <span class="step-num">2</span>
              <span>Wait for 6 confirmations (~1 hour)</span>
            </div>
            <div class="step">
              <span class="step-num">3</span>
              <span>Click "Check for Deposit" below</span>
            </div>
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

          {#if btcUpdateResult}
            <div class="alert success">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
                <polyline points="22 4 12 14.01 9 11.01"/>
              </svg>
              {btcUpdateResult}
            </div>
          {/if}

          <div class="actions">
            <button class="btn-secondary" onclick={onClose}>
              Close
            </button>
            <button
              class="btn-primary btc"
              onclick={handleUpdateBtcBalance}
              disabled={updatingBtcBalance}
            >
              {#if updatingBtcBalance}
                <span class="spinner"></span>
                Checking...
              {:else}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
                </svg>
                Check for Deposit
              {/if}
            </button>
          </div>

          <div class="info-box btc">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="16" x2="12" y2="12"/>
              <line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            <p>
              <strong>Minimum deposit:</strong> 0.0001 BTC (10,000 sats)<br/>
              Your BTC will be converted to ckBTC at a 1:1 rate. ckBTC can be converted back to BTC anytime.
            </p>
          </div>
        {/if}
      </div>
    {/if}

    <!-- ICP funding instructions when no balance -->
    {#if !isBTC && !loadingBalance && !hasEnoughBalance}
      <div class="deposit-address-section">
        <h3>Your Deposit Address</h3>
        <p class="address-hint">Send ICP to this address to fund your poker account:</p>
        <div class="address-box">
          <span class="address-value">{accountId}</span>
        </div>
        <button
          class="copy-address-btn"
          onclick={() => {
            navigator.clipboard.writeText(accountId);
            copiedAddress = true;
            setTimeout(() => copiedAddress = false, 2000);
          }}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
          {copiedAddress ? 'Copied!' : 'Copy Address'}
        </button>
      </div>

      <div class="how-to-fund">
        <h3>How to fund:</h3>
        <ol>
          <li>Copy the deposit address above</li>
          <li>Send ICP from an exchange or another wallet</li>
          <li>Wait for confirmation, then click Refresh</li>
        </ol>
      </div>
      <div class="actions">
        <button class="btn-secondary" onclick={onClose}>
          Close
        </button>
        <button class="btn-primary" onclick={loadWalletBalance}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
          </svg>
          Refresh Balance
        </button>
      </div>
    {/if}
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
    max-width: 520px;
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

  /* Deposit Method Toggle */
  .deposit-method-toggle {
    display: flex;
    gap: 8px;
    background: rgba(0, 0, 0, 0.3);
    padding: 6px;
    border-radius: 12px;
  }

  .deposit-method-toggle button {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 16px;
    background: transparent;
    border: none;
    border-radius: 8px;
    color: #888;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .deposit-method-toggle button .method-icon {
    font-size: 18px;
  }

  .deposit-method-toggle button .method-hint {
    font-size: 10px;
    font-weight: 400;
    opacity: 0.7;
  }

  .deposit-method-toggle button.active {
    background: rgba(247, 147, 26, 0.2);
    color: #f7931a;
  }

  .deposit-method-toggle button:hover:not(.active) {
    background: rgba(255, 255, 255, 0.05);
    color: #ccc;
  }

  .balance-section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    padding: 16px;
  }

  .balance-section.btc {
    background: linear-gradient(135deg, rgba(247, 147, 26, 0.1), rgba(180, 100, 20, 0.05));
    border: 1px solid rgba(247, 147, 26, 0.2);
  }

  .balance-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .balance-label {
    color: #888;
    font-size: 13px;
  }

  .balance-value {
    color: #00d4aa;
    font-size: 20px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .balance-value.btc {
    color: #f7931a;
  }

  .balance-value.loading {
    color: #666;
  }

  .mini-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(0, 212, 170, 0.3);
    border-top-color: #00d4aa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .mini-spinner.btc {
    border-color: rgba(247, 147, 26, 0.3);
    border-top-color: #f7931a;
  }

  .no-balance-warning {
    display: flex;
    gap: 12px;
    margin-top: 16px;
    padding: 14px;
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.3);
    border-radius: 8px;
    color: #f59e0b;
  }

  .no-balance-warning svg {
    flex-shrink: 0;
    margin-top: 2px;
  }

  .warning-content strong {
    display: block;
    margin-bottom: 4px;
  }

  .warning-content p {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: #c9a227;
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

  .input-row {
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

  .conversion-preview {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
    color: #888;
    padding: 10px 14px;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .conversion-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .crypto-equiv {
    color: #aaa;
  }

  .usd-preview {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .usd-amount {
    color: #4ade80;
    font-weight: 600;
    font-size: 14px;
  }

  .usd-value {
    font-size: 12px;
    color: #4ade80;
    margin-left: 6px;
    font-weight: 500;
    opacity: 0.9;
  }

  .balance-crypto {
    font-weight: 600;
  }

  input {
    flex: 1;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 14px;
    color: white;
    font-size: 18px;
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
    padding: 14px 16px;
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

  .minimum-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: rgba(99, 102, 241, 0.1);
    border: 1px solid rgba(99, 102, 241, 0.3);
    border-radius: 8px;
    color: #a5b4fc;
    font-size: 12px;
  }

  .minimum-notice.btc {
    background: rgba(247, 147, 26, 0.1);
    border-color: rgba(247, 147, 26, 0.3);
    color: #fbbf24;
  }

  .minimum-notice svg {
    flex-shrink: 0;
    color: #6366f1;
  }

  .minimum-notice.btc svg {
    color: #f7931a;
  }

  .minimum-notice strong {
    color: #c7d2fe;
  }

  .minimum-notice.btc strong {
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

  .status-message {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px;
    background: rgba(100, 100, 200, 0.1);
    border: 1px solid rgba(100, 100, 200, 0.2);
    border-radius: 8px;
    color: #aab;
    font-size: 13px;
  }

  .actions {
    display: flex;
    gap: 12px;
  }

  .btn-primary, .btn-secondary {
    flex: 1;
    padding: 14px 20px;
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
    background: linear-gradient(135deg, #00d4aa 0%, #00a88a 100%);
    color: white;
  }

  .btn-primary.btc {
    background: linear-gradient(135deg, #f7931a 0%, #c77700 100%);
  }

  .btn-primary:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(0, 212, 170, 0.3);
  }

  .btn-primary.btc:hover:not(:disabled) {
    box-shadow: 0 4px 15px rgba(247, 147, 26, 0.3);
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

  .info-box.btc {
    background: rgba(247, 147, 26, 0.05);
    border-color: rgba(247, 147, 26, 0.1);
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

  /* BTC Deposit Section */
  .btc-deposit-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .btc-deposit-header h3 {
    margin: 0 0 8px 0;
    font-size: 16px;
    color: #f7931a;
  }

  .btc-deposit-header p {
    margin: 0;
    font-size: 13px;
    color: #888;
    line-height: 1.5;
  }

  .loading-address {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 20px;
    background: rgba(247, 147, 26, 0.1);
    border-radius: 12px;
    color: #f7931a;
    font-size: 14px;
  }

  .btc-address-box {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(247, 147, 26, 0.3);
    border-radius: 12px;
    padding: 16px;
  }

  .btc-address-box label {
    display: block;
    margin-bottom: 8px;
    color: #888;
  }

  .address-display {
    background: rgba(0, 0, 0, 0.4);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 12px;
  }

  .address-text {
    font-family: monospace;
    font-size: 12px;
    color: #f7931a;
    word-break: break-all;
    line-height: 1.5;
  }

  .copy-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;
    background: rgba(247, 147, 26, 0.15);
    border: 1px solid rgba(247, 147, 26, 0.3);
    color: #f7931a;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .copy-btn:hover {
    background: rgba(247, 147, 26, 0.25);
  }

  .btc-minimum-warning {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    padding: 12px 14px;
    background: rgba(234, 179, 8, 0.1);
    border: 1px solid rgba(234, 179, 8, 0.3);
    border-radius: 8px;
    margin-bottom: 12px;
  }

  .btc-minimum-warning svg {
    flex-shrink: 0;
    color: #eab308;
    margin-top: 2px;
  }

  .btc-minimum-warning div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .btc-minimum-warning strong {
    color: #eab308;
    font-size: 13px;
  }

  .btc-minimum-warning span {
    color: #a3a3a3;
    font-size: 12px;
  }

  .btc-steps {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .step {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
    font-size: 13px;
    color: #ccc;
  }

  .step-num {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(247, 147, 26, 0.2);
    color: #f7931a;
    border-radius: 50%;
    font-size: 12px;
    font-weight: 700;
  }

  /* ICP deposit address section */
  .deposit-address-section {
    background: linear-gradient(135deg, rgba(0, 212, 170, 0.1) 0%, rgba(0, 100, 80, 0.1) 100%);
    border: 1px solid rgba(0, 212, 170, 0.3);
    border-radius: 12px;
    padding: 20px;
    text-align: center;
  }

  .deposit-address-section h3 {
    margin: 0 0 8px 0;
    font-size: 14px;
    color: #00d4aa;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .address-hint {
    margin: 0 0 12px 0;
    font-size: 12px;
    color: #888;
  }

  .address-box {
    background: rgba(0, 0, 0, 0.4);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 12px;
  }

  .address-value {
    font-family: monospace;
    font-size: 11px;
    color: #fff;
    word-break: break-all;
    line-height: 1.5;
  }

  .copy-address-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 20px;
    background: rgba(0, 212, 170, 0.2);
    border: 1px solid rgba(0, 212, 170, 0.4);
    color: #00d4aa;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .copy-address-btn:hover {
    background: rgba(0, 212, 170, 0.3);
    transform: translateY(-1px);
  }

  .how-to-fund {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    padding: 16px;
  }

  .how-to-fund h3 {
    margin: 0 0 12px 0;
    font-size: 13px;
    color: #888;
  }

  .how-to-fund ol {
    margin: 0;
    padding-left: 20px;
    color: #666;
    font-size: 12px;
    line-height: 1.6;
  }

  .how-to-fund li {
    margin-bottom: 6px;
  }

  .how-to-fund li:last-child {
    margin-bottom: 0;
  }

  .deposit-info-section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    padding: 20px;
  }

  .deposit-info-section.btc {
    background: linear-gradient(135deg, rgba(247, 147, 26, 0.1) 0%, rgba(180, 100, 20, 0.05) 100%);
    border: 1px solid rgba(247, 147, 26, 0.3);
  }

  .deposit-info-section h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    color: #f7931a;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .deposit-info-section .info-text {
    margin: 0 0 16px 0;
    font-size: 13px;
    color: #999;
    line-height: 1.5;
  }

  .funding-options {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .funding-options .option {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
    padding: 14px;
  }

  .funding-options .option strong {
    display: block;
    margin-bottom: 6px;
    font-size: 13px;
    color: #fbbf24;
  }

  .funding-options .option p {
    margin: 0;
    font-size: 12px;
    color: #888;
    line-height: 1.4;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .spinner.btc {
    border-color: rgba(247, 147, 26, 0.3);
    border-top-color: #f7931a;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Wallet Source Toggle */
  .wallet-source-toggle {
    display: flex;
    gap: 8px;
    background: rgba(0, 0, 0, 0.3);
    padding: 6px;
    border-radius: 12px;
  }

  .wallet-source-toggle button {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 16px;
    background: transparent;
    border: none;
    border-radius: 8px;
    color: #888;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .wallet-source-toggle button svg {
    opacity: 0.7;
  }

  .wallet-source-toggle .source-label {
    font-weight: 600;
  }

  .wallet-source-toggle .source-hint {
    font-size: 10px;
    font-weight: 400;
    opacity: 0.7;
  }

  .wallet-source-toggle button.active {
    background: linear-gradient(135deg, rgba(99, 102, 241, 0.2), rgba(139, 92, 246, 0.2));
    color: #a5b4fc;
    border: 1px solid rgba(99, 102, 241, 0.3);
  }

  .wallet-source-toggle button.active svg {
    opacity: 1;
    color: #a5b4fc;
  }

  .wallet-source-toggle button:hover:not(.active) {
    background: rgba(255, 255, 255, 0.05);
    color: #ccc;
  }

  /* OISY Connect Section */
  .oisy-connect-section {
    background: linear-gradient(135deg, rgba(99, 102, 241, 0.1), rgba(139, 92, 246, 0.05));
    border: 1px solid rgba(99, 102, 241, 0.2);
    border-radius: 12px;
    padding: 24px;
    text-align: center;
  }

  .oisy-connect-section.btc {
    background: linear-gradient(135deg, rgba(247, 147, 26, 0.1), rgba(180, 100, 20, 0.05));
    border-color: rgba(247, 147, 26, 0.2);
  }

  .oisy-icon {
    margin-bottom: 12px;
    color: #a5b4fc;
  }

  .oisy-connect-section.btc .oisy-icon {
    color: #f7931a;
  }

  .oisy-connect-section h3 {
    margin: 0 0 8px 0;
    font-size: 16px;
    color: #fff;
  }

  .oisy-connect-section p {
    margin: 0 0 16px 0;
    font-size: 13px;
    color: #888;
    line-height: 1.5;
  }

  .btn-connect-oisy {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 24px;
    background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
    border: none;
    border-radius: 8px;
    color: white;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-connect-oisy.btc {
    background: linear-gradient(135deg, #f7931a 0%, #c77700 100%);
  }

  .btn-connect-oisy:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(99, 102, 241, 0.3);
  }

  .btn-connect-oisy.btc:hover:not(:disabled) {
    box-shadow: 0 4px 15px rgba(247, 147, 26, 0.3);
  }

  .btn-connect-oisy:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  .oisy-error {
    margin-top: 12px;
    padding: 10px;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 8px;
    color: #ef4444;
    font-size: 13px;
  }

  /* OISY Connected State */
  .balance-section.oisy {
    background: linear-gradient(135deg, rgba(99, 102, 241, 0.1), rgba(139, 92, 246, 0.05));
    border: 1px solid rgba(99, 102, 241, 0.2);
  }

  .balance-section.oisy.btc {
    background: linear-gradient(135deg, rgba(247, 147, 26, 0.1), rgba(180, 100, 20, 0.05));
    border-color: rgba(247, 147, 26, 0.2);
  }

  .oisy-connected-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .connected-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.3);
    border-radius: 20px;
    color: #22c55e;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .disconnect-btn {
    padding: 4px 10px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: #888;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .disconnect-btn:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: rgba(239, 68, 68, 0.3);
    color: #ef4444;
  }

  .balance-value.oisy {
    color: #a5b4fc;
  }

  .balance-value.oisy.btc {
    color: #f7931a;
  }

  .no-balance-warning.oisy {
    background: rgba(99, 102, 241, 0.1);
    border-color: rgba(99, 102, 241, 0.3);
    color: #a5b4fc;
  }

  .no-balance-warning.oisy.btc {
    background: rgba(247, 147, 26, 0.1);
    border-color: rgba(247, 147, 26, 0.3);
    color: #f7931a;
  }

  .no-balance-warning.oisy .warning-content p {
    color: #818cf8;
  }

  .no-balance-warning.oisy.btc .warning-content p {
    color: #d97706;
  }
</style>
