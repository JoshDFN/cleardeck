# ClearDeck

**Provably Fair Multi-Chain Poker on the Internet Computer**

> **This entire project was built 100% by AI** (Claude Code) to demonstrate what's possible with AI-assisted development and the Internet Computer blockchain.

ClearDeck is a fully decentralized Texas Hold'em poker application running entirely on the Internet Computer. Play with ICP, Bitcoin, or Ethereum — all powered by ICP's chain fusion technology. Every card shuffle is cryptographically verifiable, ensuring fair play without requiring trust. No middleman, no house edge — just pure poker.

## Screenshots

<p align="center">
  <img src="docs/screenshots/lobby.png" alt="Lobby - Browse Tables" width="800"/>
  <br/><em>Lobby - Browse available tables with different stakes and currencies</em>
</p>

<p align="center">
  <img src="docs/screenshots/table.png" alt="Poker Table" width="800"/>
  <br/><em>Poker Table - Real-time gameplay with multi-currency support</em>
</p>

## Demo

**[View Demo](https://hd26q-uiaaa-aaaad-afzia-cai.icp0.io/)** | **[GitHub](https://github.com/ck-harrison/cleardeck-multichain)**

---

## Features

| Feature | Description |
|---------|-------------|
| **Multi-Currency** | Play with ICP, Bitcoin (via ckBTC), or Ethereum (via ckETH) |
| **Native ETH Deposits** | Send ETH to a unique address — auto-converts to ckETH for gameplay |
| **No Wallet Required** | No MetaMask, no WalletConnect — just send crypto to an address |
| **Provably Fair** | Every shuffle cryptographically verifiable |
| **100% On-Chain** | Frontend, backend, and game logic all on IC |
| **Decentralized Auth** | Internet Identity — no passwords |
| **Instant Deposits** | Direct ledger transfers for ICP/ckBTC/ckETH |
| **Hand History** | Review all past hands with proofs |
| **Time Bank** | Extra time for tough decisions |
| **Multiple Tables** | Heads-up, 6-max, 9-max configurations |
| **Bust Animations** | Fireworks for the winner, rain cloud for the loser |

---

## Chain Fusion: Multi-Currency Support

ClearDeck showcases the power of ICP's chain fusion — holding and transacting with BTC, ETH, and ICP assets inside a single dapp with no bridges, no wrapping, and no external wallet connections.

### ICP Tables
- Direct ICP ledger deposits and withdrawals
- ICRC-1/ICRC-2 standard transfers

### BTC Tables (ckBTC)
- Unique Bitcoin address per user via ckBTC minter
- Send real BTC → ckBTC minted 1:1 after 6 confirmations
- Withdraw back to real BTC anytime

### ETH Tables (ckETH)
- **Unique Ethereum address per user** via ICP threshold ECDSA (secp256k1)
- Send native ETH to your address — the canister auto-sweeps to the ckETH minter
- ckETH minted 1:1 after ~20 minutes (Ethereum finality)
- Presented as "ETH" throughout the UI — the ckETH abstraction is invisible
- No MetaMask, no WalletConnect, no bridge UI — just "send to address" like Bitcoin

**How ETH deposits work under the hood:**
```
User sends ETH to their unique address (derived via threshold ECDSA)
        ↓
Table canister detects balance via EVM RPC canister
        ↓
Builds & signs EIP-1559 tx to ckETH helper contract deposit(bytes32)
        ↓
ckETH minter mints ckETH 1:1 to user's principal (~20 min)
        ↓
User plays poker with their ETH (represented as ckETH on-chain)
```

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| **Frontend** | SvelteKit 5, Vite, TypeScript, SCSS |
| **Backend** | Rust, IC CDK 0.19, Candid |
| **Blockchain** | Internet Computer, ckBTC, ckETH |
| **Auth** | Internet Identity |
| **Crypto** | SHA-256, IC VRF (threshold BLS), threshold ECDSA |
| **Payments** | ICRC-1/ICRC-2 ledger standards |
| **ETH Integration** | EVM RPC canister, alloy-consensus (EIP-1559), k256 |

---

## Disclaimer

> ⚠️ **WARNING**: This is unaudited alpha software with known bugs. This is for educational and testing purposes only. Any deposit of ICP, Bitcoin, or Ethereum is at your own risk — your funds are NOT safe. Expect to lose everything you deposit. Online gambling is illegal in many jurisdictions. Only use where legally permitted. 18+ only.

---

## Architecture

### 100% On-Chain
- **Frontend**: Served from IC canisters (no AWS, no servers)
- **Backend**: All game logic in Rust smart contracts
- **Authentication**: Internet Identity (decentralized)
- **Randomness**: IC's Verifiable Random Function (VRF)
- **Payments**: Real ICP, Bitcoin (via ckBTC), and Ethereum (via ckETH)

### Provably Fair
Every shuffle uses a commit-reveal scheme:
1. Before dealing: Seed hash is committed publicly
2. After hand: Full seed is revealed
3. Anyone can verify: Recalculate the shuffle yourself

### Triple Currency Support
- **ICP Tables**: Play with Internet Computer tokens
- **BTC Tables**: Play with Bitcoin (via ckBTC)
- **ETH Tables**: Play with Ethereum (via ckETH) — native ETH deposits with threshold ECDSA

---

## Mainnet Canister IDs

| Canister | ID | Purpose |
|----------|-----|---------|
| Frontend | [`hd26q-uiaaa-aaaad-afzia-cai`](https://hd26q-uiaaa-aaaad-afzia-cai.icp0.io/) | SvelteKit web app |
| Lobby | `hnyty-pyaaa-aaaad-afzja-cai` | Table discovery |
| History | `he3ye-zqaaa-aaaad-afziq-cai` | Permanent hand records |
| Table 1 | `hkzvm-caaaa-aaaad-afzjq-cai` | Heads-up 0.01/0.02 ICP |
| Table 2 | `h76eb-diaaa-aaaad-afzka-cai` | 6-max 0.05/0.10 ICP |
| Table 3 | `hy7cv-oqaaa-aaaad-afzkq-cai` | 9-max 0.10/0.20 ICP |
| BTC Table | `f6hqy-haaaa-aaaad-afzhq-cai` | Heads-up 100/200 sats |
| ETH Table | `ghxrc-niaaa-aaaad-afzoa-cai` | Heads-up 0.0001/0.0002 ETH |

---

## Native ETH Deposit Flow

The ETH integration uses ICP's **threshold ECDSA** to derive unique Ethereum addresses for each user — no external wallet needed.

### User Experience
1. Open profile sidebar → click **"Show Address"** next to ETH
2. Copy your unique Ethereum address
3. Send ETH from any wallet (Coinbase, MetaMask, exchange, etc.)
4. Click **"Check for Deposit"** — the app detects and auto-sweeps your ETH
5. Wait ~20 minutes for ckETH minting (progress indicator shown)
6. Your ETH balance appears — deposit to any ETH table and play

### Technical Details
- **Threshold ECDSA**: `ecdsa_public_key` + `sign_with_ecdsa` management canister APIs
- **EVM RPC canister** (`7hfb6-caaaa-aaaar-qadga-cai`): Ethereum JSON-RPC calls from ICP
- **ckETH helper contract** (`0x7574eB42cA208A4f6960ECCAfDF186D627dCC175`): Receives ETH deposits
- **EIP-1559 transactions**: Built with `alloy-consensus`, signed with threshold ECDSA
- **Principal encoding**: ICP principal encoded as `bytes32` for the minter's `deposit()` function

---

## Verify the Code (Reproducible Builds)

You can verify that the deployed canisters match this source code:

### Quick Verification

```bash
# Check the deployed WASM hash
dfx canister info hnyty-pyaaa-aaaad-afzja-cai --network ic

# Build locally and compare
docker build -t cleardeck-verify .
docker run --rm cleardeck-verify
```

### Manual Verification Steps

1. **Get the deployed hash:**
   ```bash
   dfx canister info <canister-id> --network ic | grep "Module hash"
   ```

2. **Build from source in Docker:**
   ```bash
   docker build -t cleardeck-verify .
   docker run --rm cleardeck-verify
   ```

3. **Compare the hashes** — they should match exactly.

---

## Local Development Setup

### Prerequisites

**Rust**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown
```

**Node.js 18+**
```bash
# Using nvm
nvm install 18 && nvm use 18
```

**DFX (Internet Computer SDK)**
```bash
sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"
```

### Quick Start

```bash
# Clone the repo
git clone https://github.com/ck-harrison/cleardeck-multichain.git
cd cleardeck-multichain

# Install dependencies
npm install

# Start local replica
dfx start --background

# Deploy everything
dfx deploy

# Open the URL printed by dfx deploy
```

### Development Mode (Hot Reload)

```bash
# Terminal 1: Start replica and deploy backend
dfx start --background
dfx deploy lobby table_1 table_2 table_3 btc_table_1 eth_table_1 history

# Terminal 2: Start frontend dev server
cd src/cleardeck_frontend
npm run dev
```

Access at `http://localhost:5173`

---

## Project Architecture

```
cleardeck-multichain/
├── src/
│   ├── lobby_canister/          # Table discovery & management
│   │   └── src/lib.rs
│   ├── table_canister/          # Core poker logic + ETH integration
│   │   ├── src/lib.rs           # 5000+ lines: poker engine + threshold ECDSA + EVM RPC
│   │   └── table_canister.did   # Candid interface
│   ├── history_canister/        # Permanent hand storage
│   │   └── src/lib.rs
│   └── cleardeck_frontend/      # SvelteKit 5 + Vite
│       └── src/
│           ├── routes/+page.svelte
│           └── lib/components/
│               ├── PokerTable.svelte    # Table UI + bust animations
│               ├── Lobby.svelte         # Multi-currency table list
│               ├── DepositModal.svelte  # ICP/BTC/ETH deposit flows
│               ├── WalletButton.svelte  # Profile sidebar + ETH address
│               ├── Fireworks.svelte     # Winner animation (canvas)
│               ├── RainCloud.svelte     # Loser animation (canvas)
│               └── ...
├── dfx.json                     # Canister configuration
├── Cargo.toml                   # Rust workspace
└── Dockerfile                   # Reproducible builds
```

---

## Technical Deep Dive

### Cryptographic Shuffle (Provably Fair)

The shuffle uses IC's VRF + commit-reveal:

```
BEFORE DEALING:
1. IC VRF generates 32 random bytes (threshold BLS)
2. seed_hash = SHA256(random_bytes)
3. Commit seed_hash publicly
4. Shuffle deck using Fisher-Yates with SHA256 chain
5. Deal cards

AFTER HAND:
6. Reveal original random_bytes
7. Store in history canister
8. Anyone can verify: SHA256(seed) == committed_hash
```

### Threshold ECDSA for ETH Addresses

Each user gets a unique Ethereum address derived from their ICP principal:

```
User's ICP Principal
        ↓
derivation_path = [0x01, principal_bytes]
        ↓
ecdsa_public_key(key_name: "key_1", derivation_path)
        ↓
SEC1 compressed pubkey → k256 uncompressed → keccak256 → last 20 bytes
        ↓
Unique Ethereum address (0x...)
```

### ICP Deposits & Withdrawals

```
DEPOSIT:
Player Wallet → ICP Ledger → notify_deposit(block_index) → Table Canister
                                    ↓
                             Verify on ledger
                                    ↓
                             Credit escrow balance

WITHDRAWAL:
Table Canister → ICRC-1 transfer → Player Wallet
```

### ckBTC Integration

1. **Get BTC address**: Table canister generates a unique BTC address per user
2. **Send BTC**: User sends real Bitcoin to that address
3. **Mint ckBTC**: After 6 confirmations, ckBTC is minted 1:1
4. **Play**: Use ckBTC at the table (10 sats transfer fee)
5. **Withdraw**: Convert back to real BTC via ckBTC minter

### ckETH Integration (Native ETH Deposits)

1. **Get ETH address**: Threshold ECDSA derives a unique Ethereum address per user
2. **Send ETH**: User sends native ETH from any wallet
3. **Auto-sweep**: Canister detects balance via EVM RPC, builds EIP-1559 tx, signs with threshold ECDSA, submits to Ethereum
4. **Mint ckETH**: ckETH minter mints 1:1 after Ethereum finality (~20 min)
5. **Play**: Use ckETH at the table (displayed as "ETH")
6. **Withdraw**: Convert back to native ETH via ckETH minter

---

## Table Configuration

Defined in `dfx.json`:

```json
{
  "table_1": {
    "init_arg": "(record {
      small_blind = 1000000 : nat64;      // 0.01 ICP
      big_blind = 2000000 : nat64;        // 0.02 ICP
      min_buy_in = 200000000 : nat64;     // 2 ICP
      max_buy_in = 1000000000 : nat64;    // 10 ICP
      max_players = 2 : nat8;             // Heads-up
      currency = variant { ICP }
    })"
  }
}
```

**BTC Table:**
```json
{
  "btc_table_1": {
    "init_arg": "(record {
      small_blind = 100 : nat64;          // 100 sats
      big_blind = 200 : nat64;            // 200 sats
      min_buy_in = 10000 : nat64;         // 10,000 sats
      max_buy_in = 100000 : nat64;        // 100,000 sats
      max_players = 2 : nat8;
      currency = variant { BTC }
    })"
  }
}
```

**ETH Table:**
```json
{
  "eth_table_1": {
    "init_arg": "(record {
      small_blind = 100000000000000 : nat64;      // 0.0001 ETH
      big_blind = 200000000000000 : nat64;        // 0.0002 ETH
      min_buy_in = 1000000000000000 : nat64;      // 0.001 ETH
      max_buy_in = 10000000000000000 : nat64;     // 0.01 ETH
      max_players = 2 : nat8;
      currency = variant { ETH }
    })"
  }
}
```

---

## Game Features

- **Multi-Currency**: ICP, Bitcoin, and Ethereum deposits/withdrawals
- **Provably Fair**: Every shuffle verifiable
- **Time Bank**: 30s extra time for tough decisions
- **Auto-Deal**: Hands start automatically
- **Hand History**: Review all past hands with proofs
- **Sit Out**: Take breaks without leaving
- **Side Pots**: Proper all-in handling
- **Display Names**: Custom nicknames (1-12 chars)
- **Bust Animations**: Canvas-based fireworks and rain cloud effects

---

## API Reference

### Table Canister

```candid
// Join table at seat
join_table : (seat: nat8) -> (Result);

// Player actions
player_action : (action: PlayerAction) -> (Result);
  // PlayerAction = Fold | Check | Call | Raise(amount) | AllIn

// Deposit (after ICP transfer)
notify_deposit : (block_index: nat64) -> (Result_1);

// Withdraw to wallet
withdraw : (amount: nat64) -> (Result_1);

// Cash out from table
cash_out : () -> (Result_1);

// Get table state (hides opponent cards)
get_table_view : () -> (opt TableView) query;

// BTC: Get deposit address
get_btc_deposit_address : () -> (variant { Ok : text; Err : text });

// BTC: Check for new deposits
update_btc_balance : () -> (variant { Ok : vec UtxoStatus; Err : text });

// ETH: Get unique Ethereum deposit address
get_eth_deposit_address : () -> (variant { Ok : EthDepositInfo; Err : text });

// ETH: Sweep deposited ETH to ckETH minter
sweep_eth_to_cketh : () -> (variant { Ok : SweepStatus; Err : text });
```

### History Canister

```candid
// Get specific hand
get_hand : (hand_id: nat64) -> (opt HandHistoryRecord) query;

// Get player's hands
get_hands_by_player : (principal, offset: nat64, limit: nat64)
  -> (vec HandSummary) query;

// Verify shuffle
verify_hand_shuffle : (hand_id: nat64) -> (Result<bool, text>);
```

---

## Security Considerations

1. **Unaudited Code**: This is alpha software — expect bugs
2. **Stable Storage**: Uses stable memory for upgrades, but bugs can still cause data loss
3. **Canister Cycles**: Monitor cycles — if depleted, canisters stop
4. **Key Security**: Controller identity must be secured
5. **No Rake**: There's no house edge — this is purely peer-to-peer
6. **Threshold ECDSA**: ETH addresses are derived from ICP subnet keys — the canister never holds private keys directly

---

## Known Issues

- Hand history may be lost during canister upgrades (stable memory limitations)
- BTC deposits require 6 confirmations (~1 hour)
- ETH deposits require ~20 minutes for ckETH minting (Ethereum finality)
- Large pots may have rounding issues (e8s precision)
- UI may lag on slow connections (polling-based updates)
- History canister indexes grow unboundedly (long-term memory concern)
- ETH address derivation takes a few seconds on first load (threshold ECDSA call)

---

## Built With

- **Backend**: Rust, IC CDK 0.19, SHA256, ICRC-1/ICRC-2, threshold ECDSA, EVM RPC
- **Frontend**: SvelteKit 5, Vite, SCSS
- **Blockchain**: Internet Computer, ckBTC, ckETH
- **ETH Libraries**: alloy-consensus, k256, sha3 (keccak256)
- **Auth**: Internet Identity
- **AI**: Claude Code (100% AI-generated code)

---

## Build Your Own with Claude Code

This project is designed to be forked, studied, and extended. **Everything was built with AI** (Claude Code), so you can continue development the same way.

### Prerequisites

| Requirement | Installation |
|-------------|--------------|
| **Rust** | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Wasm target** | `rustup target add wasm32-unknown-unknown` |
| **Node.js 18+** | `nvm install 18` |
| **DFX SDK** | `sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"` |
| **Claude Code** | [Download](https://claude.ai/download) (optional, for AI development) |

### Continue Building with Claude Code

This entire codebase was generated using [Claude Code](https://claude.ai/download). To continue development:

```bash
# 1. Install Claude Code CLI
npm install -g @anthropic/claude-code

# 2. Navigate to the project
cd cleardeck-multichain

# 3. Start Claude Code
claude

# 4. Ask Claude to add features or fix bugs:
#    "Add tournament mode with buy-ins"
#    "Integrate ckSOL for Solana deposits"
#    "Add player statistics tracking"
```

### Ideas for Extensions

- **ckSOL Integration**: Solana deposits via future ckSOL minter
- **Tournaments**: Multi-table tournament support
- **Chat**: In-game messaging
- **Avatars**: NFT avatar integration
- **Statistics**: Detailed player analytics
- **Mobile**: Native mobile apps
- **More Games**: Omaha, Stud, etc.
- **Private Tables**: Password-protected games

---

## Contributing

Contributions are welcome:

1. Fork the repo
2. Create a feature branch
3. Submit a PR

Please note this is experimental software.

---

## License

MIT License

---

## Links

- **Demo**: [hd26q-uiaaa-aaaad-afzia-cai.icp0.io](https://hd26q-uiaaa-aaaad-afzia-cai.icp0.io/)
- **GitHub**: [github.com/ck-harrison/cleardeck-multichain](https://github.com/ck-harrison/cleardeck-multichain)
- **Internet Computer**: [internetcomputer.org](https://internetcomputer.org/)
- **ckBTC**: [internetcomputer.org/ckbtc](https://internetcomputer.org/ckbtc)
- **ckETH**: [internetcomputer.org/cketh](https://internetcomputer.org/cketh)

---

**ClearDeck** — No middleman, no house. Multi-chain poker powered by ICP chain fusion, built by AI.
