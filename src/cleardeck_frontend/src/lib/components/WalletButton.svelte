<script>
  import { auth, wallet, formattedBalance } from '$lib/auth.js';
  import { onMount } from 'svelte';
  import { Principal } from '@dfinity/principal';
  import { Actor, HttpAgent } from '@dfinity/agent';
  import logger from '$lib/logger.js';

  // Props
  const { onProfileChange = null } = $props();

  let isLoading = $state(false);
  let authState = $state({ isAuthenticated: false, principal: null, isLoading: true });
  let walletState = $state({ balance: null, isLoading: false });
  let showDropdown = $state(false);
  let accountId = $state('');
  let showAccountId = $state(false);
  let copiedPrincipal = $state(false);
  let copiedAccountId = $state(false);

  // BTC deposit address state
  let btcDepositAddress = $state('');
  let loadingBtcAddress = $state(false);
  let showBtcAddress = $state(false);
  let copiedBtcAddress = $state(false);
  let ckbtcBalance = $state(null);
  let loadingCkbtcBalance = $state(false);

  // ckBTC canister IDs (mainnet)
  const CKBTC_MINTER_CANISTER = 'mqygn-kiaaa-aaaar-qaadq-cai';
  const CKBTC_LEDGER_CANISTER = 'mxzaz-hqaaa-aaaar-qaada-cai';

  // Custom display name
  let customName = $state(typeof localStorage !== 'undefined' ? localStorage.getItem('poker_custom_name') : null);
  let editingName = $state(false);
  let nameInput = $state('');

  // Avatar settings - DiceBear styles (character-focused)
  const AVATAR_STYLES = [
    'bottts', 'bottts-neutral', 'pixel-art', 'pixel-art-neutral',
    'avataaars', 'avataaars-neutral', 'fun-emoji',
    'lorelei', 'lorelei-neutral', 'notionists', 'notionists-neutral',
    'adventurer', 'adventurer-neutral', 'big-ears', 'big-ears-neutral',
    'big-smile', 'croodles', 'croodles-neutral', 'micah', 'miniavs',
    'open-peeps', 'personas', 'thumbs'
  ];
  let avatarStyle = $state(typeof localStorage !== 'undefined' ? (localStorage.getItem('poker_avatar_style') || 'bottts') : 'bottts');
  let showAvatarPicker = $state(false);

  function getAvatarUrl(principal, style = avatarStyle) {
    if (!principal) return null;
    // scale=110 fills the circle better, radius=50 makes it circular
    return `https://api.dicebear.com/7.x/${style}/svg?seed=${encodeURIComponent(principal)}&size=64&scale=110&radius=50`;
  }

  let avatarUrl = $derived(getAvatarUrl(authState.principal));

  function setAvatarStyle(style) {
    avatarStyle = style;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('poker_avatar_style', style);
    }
    showAvatarPicker = false;
    // Notify parent that avatar changed
    onProfileChange?.({ type: 'avatar', style });
  }

  // Generate random name from principal (same logic as PokerTable)
  function generateRandomName(principal) {
    if (!principal) return 'Player';
    const adjectives = ['Lucky', 'Wild', 'Cool', 'Sly', 'Bold', 'Swift', 'Clever', 'Daring', 'Epic', 'Mystic', 'Royal', 'Shadow', 'Golden', 'Silver', 'Cosmic'];
    const nouns = ['Ace', 'King', 'Queen', 'Jack', 'Joker', 'Shark', 'Whale', 'Fox', 'Wolf', 'Tiger', 'Eagle', 'Hawk', 'Viper', 'Dragon', 'Phoenix'];

    let hash = 0;
    for (let i = 0; i < principal.length; i++) {
      hash = ((hash << 5) - hash) + principal.charCodeAt(i);
      hash = hash & hash;
    }
    hash = Math.abs(hash);

    const adj = adjectives[hash % adjectives.length];
    const noun = nouns[(hash >> 8) % nouns.length];
    const num = (hash % 100).toString().padStart(2, '0');

    return `${adj}${noun}${num}`;
  }

  // Display name: custom name if set, otherwise generated random name
  let displayName = $derived(customName || generateRandomName(authState.principal));

  function startEditingName() {
    nameInput = customName || '';
    editingName = true;
  }

  function saveCustomName() {
    const trimmed = nameInput.trim();
    if (trimmed && trimmed.length <= 12) {
      customName = trimmed;
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('poker_custom_name', trimmed);
      }
    } else if (!trimmed) {
      customName = null;
      if (typeof localStorage !== 'undefined') {
        localStorage.removeItem('poker_custom_name');
      }
    }
    editingName = false;
    // Notify parent that name changed
    onProfileChange?.({ type: 'name', name: customName });
  }

  function cancelEditingName() {
    editingName = false;
    nameInput = '';
  }

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

  // SHA-224 implementation (truncated SHA-256)
  async function sha224(data) {
    // SHA-224 uses SHA-256 with different initial values, but for IC account IDs
    // we can use SubtleCrypto SHA-256 and truncate to 28 bytes
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    // SHA-224 is first 28 bytes of SHA-256 with different IV, but for IC we use actual SHA-224
    // Actually IC uses real SHA-224, let's implement it properly
    return sha224Pure(data);
  }

  // Pure JS SHA-224 (subset of SHA-256 with different IV)
  function sha224Pure(message) {
    // SHA-224 initial hash values (different from SHA-256)
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

    // Calculate padding: message + 0x80 + zeros + 8-byte length = multiple of 64
    // We need: (msgLen + 1 + padZeros + 8) % 64 == 0
    // So: padZeros = (64 - ((msgLen + 9) % 64)) % 64
    const totalBeforePad = msgLen + 9; // message + 0x80 byte + 8-byte length
    const padZeros = (64 - (totalBeforePad % 64)) % 64;
    const paddedLen = msgLen + 1 + padZeros + 8;

    const padded = new Uint8Array(paddedLen);
    padded.set(message);
    padded[msgLen] = 0x80;
    // Write bit length as big-endian 64-bit integer at the end
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

    // SHA-224 returns only first 7 words (28 bytes)
    const result = new Uint8Array(28);
    for (let i = 0; i < 7; i++) {
      result[i * 4] = (H[i] >> 24) & 0xff;
      result[i * 4 + 1] = (H[i] >> 16) & 0xff;
      result[i * 4 + 2] = (H[i] >> 8) & 0xff;
      result[i * 4 + 3] = H[i] & 0xff;
    }
    return result;
  }

  // Compute ICP Account Identifier from principal
  function computeAccountId(principalText) {
    if (!principalText) return '';
    try {
      const principal = Principal.fromText(principalText);
      const padding = new Uint8Array(32); // 32 bytes of zeros for default subaccount

      // Domain separator + principal + subaccount
      const domainSeparator = new TextEncoder().encode('\x0Aaccount-id');
      const principalBytes = principal.toUint8Array();

      // Concatenate: domain separator + principal + subaccount
      const data = new Uint8Array(domainSeparator.length + principalBytes.length + padding.length);
      data.set(domainSeparator, 0);
      data.set(principalBytes, domainSeparator.length);
      data.set(padding, domainSeparator.length + principalBytes.length);

      // SHA-224 hash
      const hash = sha224Pure(data);

      // CRC32 checksum
      const crc = crc32(hash);

      // Final account ID: CRC32 (4 bytes) + hash (28 bytes) = 32 bytes
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

  // Check if on mainnet
  function isMainnet() {
    return typeof window !== 'undefined' &&
      (window.location.hostname.includes('icp0.io') ||
       window.location.hostname.includes('ic0.app') ||
       window.location.hostname.includes('internetcomputer.org'));
  }

  // Fetch BTC deposit address from ckBTC minter
  async function loadBtcDepositAddress() {
    if (!authState.isAuthenticated || !authState.identity || !isMainnet()) return;

    loadingBtcAddress = true;
    try {
      const agent = new HttpAgent({
        host: 'https://ic0.app',
        identity: authState.identity,
      });

      const minterIdlFactory = ({ IDL }) => {
        const GetBtcAddressArgs = IDL.Record({
          owner: IDL.Opt(IDL.Principal),
          subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
        });
        return IDL.Service({
          get_btc_address: IDL.Func([GetBtcAddressArgs], [IDL.Text], []),
        });
      };

      const minterActor = Actor.createActor(minterIdlFactory, {
        agent,
        canisterId: CKBTC_MINTER_CANISTER,
      });

      const principal = authState.identity.getPrincipal();
      const address = await minterActor.get_btc_address({
        owner: [principal],
        subaccount: [],
      });

      btcDepositAddress = address;
    } catch (e) {
      logger.error('Failed to get BTC deposit address:', e);
    }
    loadingBtcAddress = false;
  }

  // Update BTC balance - calls the ckBTC minter to check for new deposits and mint ckBTC
  async function updateBtcBalance() {
    if (!authState.isAuthenticated || !authState.identity || !isMainnet()) return;

    try {
      const agent = new HttpAgent({
        host: 'https://ic0.app',
        identity: authState.identity,
      });

      const minterIdlFactory = ({ IDL }) => {
        const UpdateBalanceArgs = IDL.Record({
          owner: IDL.Opt(IDL.Principal),
          subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
        });
        const UtxoOutpoint = IDL.Record({
          txid: IDL.Vec(IDL.Nat8),
          vout: IDL.Nat32,
        });
        const Utxo = IDL.Record({
          outpoint: UtxoOutpoint,
          value: IDL.Nat64,
          height: IDL.Nat32,
        });
        const UtxoStatus = IDL.Variant({
          ValueTooSmall: Utxo,
          Tainted: Utxo,
          Checked: Utxo,
          Minted: IDL.Record({
            block_index: IDL.Nat64,
            minted_amount: IDL.Nat64,
            utxo: Utxo,
          }),
        });
        const PendingUtxo = IDL.Record({
          outpoint: UtxoOutpoint,
          value: IDL.Nat64,
          confirmations: IDL.Nat32,
        });
        const UpdateBalanceError = IDL.Variant({
          GenericError: IDL.Record({
            error_code: IDL.Nat64,
            error_message: IDL.Text,
          }),
          TemporarilyUnavailable: IDL.Text,
          AlreadyProcessing: IDL.Null,
          NoNewUtxos: IDL.Record({
            required_confirmations: IDL.Nat32,
            pending_utxos: IDL.Opt(IDL.Vec(PendingUtxo)),
          }),
        });
        const UpdateBalanceResult = IDL.Variant({
          Ok: IDL.Vec(UtxoStatus),
          Err: UpdateBalanceError,
        });
        return IDL.Service({
          update_balance: IDL.Func([UpdateBalanceArgs], [UpdateBalanceResult], []),
        });
      };

      const minterActor = Actor.createActor(minterIdlFactory, {
        agent,
        canisterId: CKBTC_MINTER_CANISTER,
      });

      const principal = authState.identity.getPrincipal();
      const result = await minterActor.update_balance({
        owner: [principal],
        subaccount: [],
      });

      if ('Ok' in result) {
        logger.info('BTC balance update result:', result.Ok);
        // Check if any were minted
        const minted = result.Ok.filter(s => 'Minted' in s);
        if (minted.length > 0) {
          logger.info('Minted new ckBTC:', minted);
        }
      } else if ('Err' in result) {
        const err = result.Err;
        if ('NoNewUtxos' in err) {
          const pending = err.NoNewUtxos.pending_utxos;
          if (pending && pending.length > 0 && pending[0].length > 0) {
            logger.info('Pending UTXOs waiting for confirmations:', pending[0]);
          }
        } else {
          logger.warn('BTC balance update error:', err);
        }
      }
    } catch (e) {
      logger.error('Failed to update BTC balance:', e);
    }
  }

  // Fetch ckBTC balance
  async function loadCkbtcBalance() {
    if (!authState.isAuthenticated || !authState.identity || !isMainnet()) return;

    loadingCkbtcBalance = true;
    try {
      // First try to update balance (check for new deposits)
      await updateBtcBalance();

      const agent = new HttpAgent({
        host: 'https://ic0.app',
        identity: authState.identity,
      });

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
        canisterId: CKBTC_LEDGER_CANISTER,
      });

      const principal = authState.identity.getPrincipal();
      const balance = await ledgerActor.icrc1_balance_of({
        owner: principal,
        subaccount: [],
      });

      ckbtcBalance = Number(balance);
    } catch (e) {
      logger.error('Failed to get ckBTC balance:', e);
    }
    loadingCkbtcBalance = false;
  }

  // Format satoshis to BTC
  function formatBtcBalance(sats) {
    if (sats === null || sats === undefined) return '-.--';
    const btc = sats / 100_000_000;
    if (btc >= 0.001) return btc.toFixed(6) + ' BTC';
    if (sats >= 1000) return (sats / 1000).toFixed(1) + 'K sats';
    return sats + ' sats';
  }

  // Subscribe to stores
  onMount(() => {
    const unsubAuth = auth.subscribe(s => {
      authState = s;
      if (s.principal) {
        accountId = computeAccountId(s.principal);
      }
      // Auto-load BTC balance when user becomes authenticated on mainnet
      if (s.isAuthenticated && s.identity && isMainnet() && ckbtcBalance === null) {
        loadCkbtcBalance();
      }
    });
    const unsubWallet = wallet.subscribe(s => { walletState = s; });

    // Initialize auth on mount
    auth.init();

    return () => {
      unsubAuth();
      unsubWallet();
    };
  });

  // Check if we're in local dev mode by checking hostname
  function isLocalDev() {
    return typeof window !== 'undefined' &&
      !window.location.hostname.includes('icp0.io') &&
      !window.location.hostname.includes('ic0.app') &&
      !window.location.hostname.includes('internetcomputer.org');
  }
  let showDevMenu = $state(false);

  async function handleLogin() {
    isLoading = true;
    try {
      await auth.login();
      // Refresh balance after login
      await wallet.refreshBalance();
    } catch (e) {
      logger.error('Login failed:', e);
    }
    isLoading = false;
  }

  async function handleDevLogin(playerNum = 1) {
    isLoading = true;
    showDevMenu = false;
    try {
      await auth.devLogin(`dev-player-${playerNum}`);
    } catch (e) {
      logger.error('Dev login failed:', e);
    }
    isLoading = false;
  }

  async function handleLogout() {
    await auth.logout();
    showDropdown = false;
  }

  function formatPrincipal(principal) {
    if (!principal) return '';
    if (principal.length <= 16) return principal;
    return `${principal.slice(0, 8)}...${principal.slice(-4)}`;
  }

  function formatBalance(balance) {
    if (balance === null || balance === undefined) return '-.--';
    const icp = Number(balance) / 100_000_000;
    return icp.toFixed(4);
  }
</script>

<div class="wallet-container">
  {#if authState.isLoading}
    <button class="wallet-btn loading" disabled aria-label="Loading authentication">
      <span class="spinner"></span>
    </button>
  {:else if !authState.isAuthenticated}
    <div class="login-buttons">
      {#if isLocalDev()}
        <div class="dev-login-container">
          <button class="wallet-btn dev" onclick={() => showDevMenu = !showDevMenu} disabled={isLoading} aria-label="Developer login options">
            {#if isLoading}
              <span class="spinner"></span>
            {:else}
              Dev Login
            {/if}
          </button>
          {#if showDevMenu}
            <div class="dev-menu">
              <button onclick={() => handleDevLogin(1)}>Player 1</button>
              <button onclick={() => handleDevLogin(2)}>Player 2</button>
              <button onclick={() => handleDevLogin(3)}>Player 3</button>
              <button onclick={() => handleDevLogin(4)}>Player 4</button>
            </div>
          {/if}
        </div>
      {/if}
      <button class="wallet-btn connect" onclick={handleLogin} disabled={isLoading}>
        {#if isLoading}
          <span class="spinner"></span>
        {:else}
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="4" width="20" height="16" rx="2"/>
            <path d="M2 10h20"/>
            <circle cx="17" cy="14" r="2"/>
          </svg>
          Connect Wallet
        {/if}
      </button>
    </div>
  {:else}
    <div class="wallet-connected">
      <button class="wallet-btn connected" onclick={() => showDropdown = !showDropdown}>
        {#if avatarUrl}
          <img class="avatar-img" src={avatarUrl} alt="Avatar" />
        {:else}
          <svg class="profile-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="8" r="4"/>
            <path d="M4 20c0-4 4-6 8-6s8 2 8 6"/>
          </svg>
        {/if}
        <span class="display-name">{displayName}</span>
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M6 9l6 6 6-6"/>
        </svg>
      </button>

      {#if showDropdown}
        <div class="dropdown">
          <!-- Display Name Section -->
          <div class="dropdown-section">
            <span class="section-title">Display Name</span>
            {#if editingName}
              <div class="name-edit-row">
                <input
                  type="text"
                  class="name-input"
                  placeholder="Enter name (max 12 chars)"
                  maxlength="12"
                  bind:value={nameInput}
                  onkeydown={(e) => e.key === 'Enter' && saveCustomName()}
                />
                <button class="save-name-btn" onclick={saveCustomName}>✓</button>
                <button class="cancel-name-btn" onclick={cancelEditingName}>✕</button>
              </div>
            {:else}
              <div class="id-row">
                <span class="name-display">{displayName}</span>
                <button class="copy-btn small" onclick={startEditingName}>
                  Edit
                </button>
              </div>
            {/if}
            <span class="name-hint">Shown to other players at the table</span>

            <!-- Avatar Selection -->
            <div class="avatar-section">
              <div class="avatar-row">
                <img class="avatar-preview" src={avatarUrl} alt="Your avatar" />
                <button class="copy-btn small" onclick={() => showAvatarPicker = !showAvatarPicker}>
                  Change
                </button>
              </div>
              {#if showAvatarPicker}
                <div class="avatar-picker">
                  {#each AVATAR_STYLES as style}
                    <button
                      class="avatar-option"
                      class:selected={avatarStyle === style}
                      onclick={() => setAvatarStyle(style)}
                    >
                      <img src={getAvatarUrl(authState.principal, style)} alt={style} />
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>

          <!-- Principal ID Section -->
          <div class="dropdown-section">
            <span class="section-title">Principal ID</span>
            <div class="id-row">
              <span class="id-value" title={authState.principal}>{formatPrincipal(authState.principal)}</span>
              <button
                class="copy-btn small"
                onclick={() => {
                  navigator.clipboard.writeText(authState.principal);
                  copiedPrincipal = true;
                  setTimeout(() => copiedPrincipal = false, 2000);
                }}
              >
                {copiedPrincipal ? '✓' : 'Copy'}
              </button>
            </div>
          </div>

          <!-- Balances Section -->
          <div class="dropdown-section balances-section">
            <span class="section-title">Balances</span>
            <div class="balance-row">
              <span class="balance-label">ICP</span>
              <span class="balance-value">{formatBalance(walletState.balance)} ICP</span>
            </div>
            {#if isMainnet()}
              <div class="balance-row btc">
                <span class="balance-label btc">BTC</span>
                <span class="balance-value btc">{ckbtcBalance !== null ? formatBtcBalance(ckbtcBalance) : '-.--'}</span>
              </div>
            {/if}
            <button class="refresh-btn" onclick={() => { wallet.refreshBalance(); if (isMainnet()) loadCkbtcBalance(); }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M23 4v6h-6M1 20v-6h6"/>
                <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>
              </svg>
              Refresh
            </button>
          </div>

          <!-- Deposit Addresses Section -->
          <div class="dropdown-section">
            <span class="section-title">Deposit Addresses</span>
            <span class="section-hint">Send funds to these addresses to deposit</span>

            <!-- ICP Deposit -->
            <div class="deposit-item">
              <span class="deposit-label">ICP</span>
              {#if showAccountId}
                <div class="deposit-address">
                  <span class="address-value">{accountId}</span>
                  <button
                    class="copy-btn small"
                    onclick={() => {
                      navigator.clipboard.writeText(accountId);
                      copiedAccountId = true;
                      setTimeout(() => copiedAccountId = false, 2000);
                    }}
                  >
                    {copiedAccountId ? '✓' : 'Copy'}
                  </button>
                </div>
              {:else}
                <button class="show-address-btn" onclick={() => showAccountId = true}>
                  Show Address
                </button>
              {/if}
            </div>

            <!-- BTC Deposit (mainnet only) -->
            {#if isMainnet()}
              <div class="deposit-item btc">
                <span class="deposit-label btc">BTC</span>
                {#if showBtcAddress}
                  {#if loadingBtcAddress}
                    <span class="loading-text">Loading...</span>
                  {:else if btcDepositAddress}
                    <div class="deposit-address">
                      <span class="address-value btc">{btcDepositAddress}</span>
                      <button
                        class="copy-btn small btc"
                        onclick={() => {
                          navigator.clipboard.writeText(btcDepositAddress);
                          copiedBtcAddress = true;
                          setTimeout(() => copiedBtcAddress = false, 2000);
                        }}
                      >
                        {copiedBtcAddress ? '✓' : 'Copy'}
                      </button>
                    </div>
                  {:else}
                    <span class="error-text">Failed to load</span>
                  {/if}
                {:else}
                  <button class="show-address-btn btc" onclick={() => { showBtcAddress = true; loadBtcDepositAddress(); }}>
                    Show Address
                  </button>
                {/if}
              </div>
              <span class="btc-note">BTC deposits require 6 confirmations (~1 hour)</span>
            {/if}
          </div>

          <hr />
          <button class="dropdown-btn logout" onclick={handleLogout}>
            Disconnect
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wallet-container {
    position: relative;
  }

  .login-buttons {
    display: flex;
    gap: 8px;
  }

  .wallet-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: white;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .wallet-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(0, 212, 170, 0.5);
  }

  .wallet-btn.connect {
    background: linear-gradient(135deg, #00d4aa 0%, #00a88a 100%);
    border: none;
  }

  .wallet-btn.connect:hover:not(:disabled) {
    background: linear-gradient(135deg, #00e4ba 0%, #00b89a 100%);
    transform: translateY(-1px);
  }

  .wallet-btn.dev {
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    border: none;
    font-size: 12px;
  }

  .wallet-btn.dev:hover:not(:disabled) {
    background: linear-gradient(135deg, #fbbf24 0%, #f59e0b 100%);
    transform: translateY(-1px);
  }

  .dev-login-container {
    position: relative;
  }

  .dev-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    background: #1a1a2e;
    border: 1px solid rgba(245, 158, 11, 0.3);
    border-radius: 8px;
    padding: 4px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 100px;
  }

  .dev-menu button {
    background: transparent;
    border: none;
    color: white;
    padding: 8px 12px;
    border-radius: 4px;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
  }

  .dev-menu button:hover {
    background: rgba(245, 158, 11, 0.2);
  }

  .wallet-btn.connected {
    background: rgba(0, 212, 170, 0.1);
    border-color: rgba(0, 212, 170, 0.3);
  }

  .wallet-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .profile-icon {
    color: #00d4aa;
  }

  .avatar-img {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
  }

  .display-name {
    color: white;
    font-weight: 500;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .wallet-connected {
    position: relative;
  }

  .dropdown {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    min-width: 220px;
    background: #1a1a2e;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 12px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .dropdown-section {
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    margin-bottom: 12px;
  }

  .section-title {
    display: block;
    color: #00d4aa;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 8px;
  }

  .section-hint {
    display: block;
    color: #666;
    font-size: 11px;
    margin-bottom: 8px;
    line-height: 1.4;
  }

  .id-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .id-value {
    color: white;
    font-family: monospace;
    font-size: 13px;
  }

  .copy-btn {
    padding: 6px 12px;
    font-size: 11px;
    background: rgba(0, 212, 170, 0.1);
    border: 1px solid rgba(0, 212, 170, 0.3);
    color: #00d4aa;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .copy-btn.small {
    padding: 4px 8px;
  }

  .copy-btn:hover {
    background: rgba(0, 212, 170, 0.2);
  }

  /* Balances Section */
  .balances-section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
    padding: 12px !important;
  }

  .balance-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 0;
  }

  .balance-row.btc {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    margin-top: 4px;
    padding-top: 10px;
  }

  .balance-label {
    color: #888;
    font-size: 13px;
    font-weight: 500;
  }

  .balance-label.btc {
    color: #f7931a;
  }

  .balance-value {
    color: #00d4aa;
    font-family: monospace;
    font-size: 14px;
    font-weight: 600;
  }

  .balance-value.btc {
    color: #f7931a;
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    margin-top: 10px;
    padding: 8px;
    font-size: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #aaa;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  /* Deposit Addresses */
  .deposit-item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 8px 0;
    gap: 10px;
  }

  .deposit-item.btc {
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    margin-top: 4px;
    padding-top: 12px;
  }

  .deposit-label {
    color: #888;
    font-size: 12px;
    font-weight: 500;
    min-width: 30px;
    padding-top: 4px;
  }

  .deposit-label.btc {
    color: #f7931a;
  }

  .deposit-address {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-end;
  }

  .address-value {
    font-family: monospace;
    font-size: 9px;
    color: #aaa;
    word-break: break-all;
    text-align: right;
    line-height: 1.4;
  }

  .address-value.btc {
    color: #f7931a;
  }

  .show-address-btn {
    padding: 4px 10px;
    font-size: 11px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #aaa;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .show-address-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }

  .show-address-btn.btc:hover {
    border-color: rgba(247, 147, 26, 0.4);
    color: #f7931a;
  }

  .loading-text {
    color: #666;
    font-size: 11px;
  }

  .error-text {
    color: #ff6b6b;
    font-size: 11px;
  }

  .btc-note {
    display: block;
    color: #666;
    font-size: 10px;
    margin-top: 4px;
    font-style: italic;
  }

  .dropdown-btn {
    width: 100%;
    padding: 10px;
    margin-top: 8px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.05);
    color: white;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .dropdown-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .dropdown-btn.logout {
    color: #ff6b6b;
    border-color: rgba(255, 107, 107, 0.3);
  }

  .dropdown-btn.logout:hover {
    background: rgba(255, 107, 107, 0.1);
  }

  hr {
    border: none;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    margin: 12px 0;
  }

  /* Display name editing */
  .name-edit-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .name-input {
    flex: 1;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    color: white;
    font-size: 13px;
    outline: none;
  }

  .name-input:focus {
    border-color: #00d4aa;
  }

  .save-name-btn, .cancel-name-btn {
    padding: 6px 8px;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    font-size: 12px;
    transition: all 0.2s;
  }

  .save-name-btn {
    background: rgba(0, 212, 170, 0.2);
    color: #00d4aa;
  }

  .save-name-btn:hover {
    background: rgba(0, 212, 170, 0.3);
  }

  .cancel-name-btn {
    background: rgba(255, 107, 107, 0.2);
    color: #ff6b6b;
  }

  .cancel-name-btn:hover {
    background: rgba(255, 107, 107, 0.3);
  }

  .name-display {
    color: white;
    font-size: 13px;
  }

  .name-hint {
    display: block;
    color: #666;
    font-size: 10px;
    margin-top: 6px;
  }

  /* BTC-specific styles */
  .copy-btn.btc {
    background: rgba(247, 147, 26, 0.1);
    border-color: rgba(247, 147, 26, 0.3);
    color: #f7931a;
  }

  .copy-btn.btc:hover {
    background: rgba(247, 147, 26, 0.2);
  }

  /* Avatar styles */
  .avatar-section {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .avatar-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .avatar-preview {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
  }

  .avatar-picker {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
    padding: 10px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
  }

  .avatar-option {
    width: 44px;
    height: 44px;
    padding: 2px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: rgba(255, 255, 255, 0.05);
    cursor: pointer;
    transition: all 0.2s;
  }

  .avatar-option:hover {
    border-color: rgba(0, 212, 170, 0.5);
    background: rgba(255, 255, 255, 0.1);
  }

  .avatar-option.selected {
    border-color: #00d4aa;
    background: rgba(0, 212, 170, 0.1);
  }

  .avatar-option img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
  }
</style>
