# ClearDeck Security & Best Practices Audit

**Date**: 2026-03-20
**Audited against**: DFINITY ICP Skills (ckBTC, EVM RPC, Internet Identity, Canister Security, Stable Memory, ICRC Ledger)
**Scope**: `table_canister/src/lib.rs`, `lobby_canister/src/lib.rs`, `cleardeck_frontend/src/lib/auth.js`

---

## Executive Summary

ClearDeck is a multi-chain poker platform on ICP holding real ICP, ckBTC, ckETH, and DOGE. This audit compared the codebase against official DFINITY ICP Skills best practices and identified **7 critical**, **4 high**, and **7 medium** priority issues. The canister is functionally correct at current scale but has architectural risks that should be addressed before broader production use.

---

## P0 — Critical (fix before production)

### 1. Missing `inspect_message` — Cycle Drain Attack Vector
- **Location**: `table_canister/src/lib.rs` — no `inspect_message` function exists
- **Risk**: Anyone can spam expensive update calls (`start_new_hand`, `notify_deposit`, `withdraw`) to drain canister cycles below freezing threshold
- **Skill reference**: Canister Security — "Anyone on the internet can burn your cycles by sending update calls"
- **Fix**: Add `#[ic_cdk::inspect_message]` to reject anonymous callers and rate-limit at the ingress level
```rust
#[ic_cdk::inspect_message]
fn inspect_message() {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        ic_cdk::trap("Anonymous calls not allowed");
    }
    ic_cdk::api::call::accept_message();
}
```

### 2. Missing `created_at_time` on All ICRC Transfers — Replay/Double-Transfer Risk
- **Location**: Lines ~1063, ~1392, ~1489 in `lib.rs`
- **Risk**: Without `created_at_time`, two identical transfers within 24h both execute. No deduplication protection against retries or network replays
- **Skill reference**: ICRC Ledger — "Without created_at_time, two identical transfers within 24h both execute"
- **Fix**: Add `created_at_time: Some(ic_cdk::api::time())` to every `icrc1_transfer` and `icrc2_transfer_from` call
```rust
// Before (vulnerable)
created_at_time: None,

// After (deduplicated)
created_at_time: Some(ic_cdk::api::time()),
```

### 3. EVM RPC Cycle Budget Too Low — Silent ETH Call Failures
- **Location**: Line ~4922 — `const EVM_RPC_CYCLES: u128 = 3_000_000_000`
- **Risk**: 3B cycles is 30% of the recommended 10B. Calls may silently fail or trap, especially with multi-provider consensus
- **Skill reference**: EVM RPC — "Send 10_000_000_000 cycles (10B) as a starting budget"
- **Fix**: Bump to 10B
```rust
const EVM_RPC_CYCLES: u128 = 10_000_000_000; // 10B as recommended by EVM RPC skill
```

### 4. Missing CallerGuard on Withdrawals — Double-Spend Race Condition
- **Location**: Lines ~1750-1799 (withdraw function)
- **Risk**: Between balance deduction and ledger transfer completion, there's no guard preventing concurrent withdrawal calls. User could call withdraw multiple times before the first ledger call returns
- **Skill reference**: Canister Security — "Implement CallerGuard pattern with Drop trait"
- **Fix**: Implement CallerGuard with Drop trait
```rust
struct CallerGuard {
    principal: Principal,
}

impl CallerGuard {
    fn new(principal: Principal) -> Result<Self, String> {
        PENDING_WITHDRAWALS.with(|pw| {
            let mut pw = pw.borrow_mut();
            if pw.contains(&principal) {
                return Err("Withdrawal already in progress".to_string());
            }
            pw.insert(principal);
            Ok(CallerGuard { principal })
        })
    }
}

impl Drop for CallerGuard {
    fn drop(&mut self) {
        PENDING_WITHDRAWALS.with(|pw| {
            pw.borrow_mut().remove(&self.principal);
        });
    }
}
```

### 5. Unbounded Wait Calls — Canister Brick Risk
- **Location**: Lines ~963, ~1267, ~1618
- **Risk**: `Call::unbounded_wait` to history/ledger canisters means if the target hangs, the canister cannot be stopped or upgraded
- **Skill reference**: Canister Security — "If a called canister hangs, this canister cannot be stopped/upgraded while awaiting"
- **Fix**: Replace with `Call::bounded_wait` with appropriate timeouts
```rust
// Before (dangerous)
Call::unbounded_wait(history_id, "record_hand").await

// After (safe)
Call::bounded_wait(history_id, "record_hand").await
```

### 6. DOGE Withdrawal Balance Deduction Not Atomic
- **Location**: Lines ~5583-5750+
- **Risk**: Balance check happens before ECDSA signing (multiple inter-canister calls), but balance isn't deducted until after. User could call `withdraw_doge` multiple times, signing multiple transactions that all succeed
- **Fix**: Deduct balance BEFORE signing, refund on failure. Add CallerGuard (same pattern as #4)

### 7. Deposit Amount Overflow Silenced
- **Location**: Line ~1651
- **Current code**: `let amount: u64 = transfer.amount.0.clone().try_into().unwrap_or(0);`
- **Risk**: If amount > u64::MAX, silently becomes 0. Attacker could craft a transaction that credits 0 tokens
- **Fix**: Return error instead of defaulting to 0
```rust
let amount: u64 = transfer.amount.0.clone()
    .try_into()
    .map_err(|_| "Deposit amount exceeds maximum")?;
```

---

## P1 — High Priority

### 8. ckBTC: Wrong `owner` Parameter in Deposit Calls
- **Location**: Lines ~4819, ~4853
- **Current**: `owner: Some(caller)` — makes the minter think the USER owns the address
- **Correct**: `owner: Some(ic_cdk::api::canister_self())` — the CANISTER owns the deposit addresses
- **Skill reference**: ckBTC — "Setting owner to caller makes the minter index UTXOs under the wrong principal"
- **Impact**: BTC deposits may not be matched correctly during `update_balance`

### 9. ckBTC: Missing Subaccount Derivation
- **Location**: Lines ~4818-4821, ~4852-4855
- **Current**: `subaccount: None` — all users share the same deposit address
- **Correct**: Derive per-user subaccounts from principal
- **Skill reference**: ckBTC — "To give each user a distinct deposit address, derive subaccounts from a user-specific identifier"
```rust
fn principal_to_subaccount(principal: &Principal) -> [u8; 32] {
    let mut subaccount = [0u8; 32];
    let principal_bytes = principal.as_slice();
    subaccount[0] = principal_bytes.len() as u8;
    subaccount[1..1 + principal_bytes.len()].copy_from_slice(principal_bytes);
    subaccount
}
```

### 10. No Schema Versioning on PersistentState
- **Location**: Lines ~4583-4601
- **Risk**: Adding a new field without `#[serde(default)]` causes deserialization failure on upgrade, bricking the canister
- **Fix**: Add explicit versioning or ensure all fields have `#[serde(default)]`

### 11. `post_upgrade` Panic Too Aggressive
- **Location**: Lines ~4639-4642
- **Risk**: If deserialization fails, the upgrade is rejected and old buggy code keeps running
- **Fix**: Gracefully degrade to empty state and alert admins, rather than panicking

---

## P2 — Medium Priority

### 12. Migrate to `ic-stable-structures`
- **Location**: All state in `thread_local! { RefCell<HashMap> }` (lines ~481-518)
- **Risk**: Full heap serialization in `pre_upgrade` will hit the 400M instruction limit if state grows large (10K+ verified deposits, many players)
- **Fix**: Use `StableBTreeMap` with `MemoryManager` to bypass heap serialization entirely
- **Effort**: High — requires rearchitecting state storage

### 13. BTC Withdrawal Missing
- **Location**: No `withdraw_btc()` / `retrieve_btc_with_approval` implementation
- **Risk**: Users can deposit ckBTC but cannot withdraw back to Bitcoin addresses
- **Fix**: Implement full withdrawal flow: `icrc2_approve` → `retrieve_btc_with_approval` on minter

### 14. EVM RPC Inconsistent Provider Handling
- **Location**: Lines ~5113-5125
- **Current**: Accepts first successful provider response in `Inconsistent` case
- **Risk**: If providers return different values, canister uses whichever succeeded first
- **Skill reference**: EVM RPC — "Always handle both arms or your canister traps on provider disagreement"
- **Fix**: Require consensus or trap on disagreement

### 15. Stale PENDING_DEPOSITS Cleanup
- **Location**: PENDING_DEPOSITS map
- **Risk**: If an async call times out, the pending flag stays forever, blocking the user from retrying
- **Fix**: Add time-based cleanup for entries older than 5 minutes

### 16. Inconsistent Fee Handling
- **Location**: Mix of `fee: None` and `fee: Some(...)` across transfer calls
- **Fix**: Always explicitly set `fee: Some(Nat::from(currency.transfer_fee()))` for consistency

### 17. Rate Limiting Not Applied to All Endpoints
- **Location**: Only `start_new_hand` has rate limiting
- **Risk**: `player_action`, `withdraw`, `notify_deposit` can be spammed
- **Fix**: Extend rate limiting to all update functions

### 18. ICP Deposit Uses Legacy `query_blocks` Instead of ICRC-3
- **Location**: Lines ~1168-1342
- **Risk**: Maintenance burden; doesn't check archive canister (returns "may be archived")
- **Fix**: Unify all currencies on ICRC-3 `get_transactions`

---

## Positive Findings

The audit also identified several correctly implemented patterns:

- Anonymous principal checks on all async entry points
- Controller authorization using `ic_cdk::api::is_controller`
- Proper cycle attachment with `call_with_payment128`
- Correct threshold ECDSA usage with proper derivation paths
- EIP-1559 transaction building with `alloy-consensus`
- Atomic `Entry::Vacant` pattern for deposit deduplication (partially mitigates TOCTOU)
- `saturating_add`/`saturating_sub` for overflow-safe chip calculations
- Bounded collections: HAND_HISTORY (100), DISPLAY_NAMES (200), VERIFIED_DEPOSITS (10K)
- Fire-and-forget pattern for non-critical history recording

---

## Frontend Findings (Internet Identity)

### Principal Consistency
- `icp0.io` and `ic0.app` are automatically rewritten to the same origin by II — no `derivationOrigin` needed
- If user gets a different principal, they are likely using a different II anchor or passkey
- The `identityProvider` URL `identity.internetcomputer.org` redirects to `id.ai` (same service)

### Recommendations
- Update `identityProvider` to `https://id.ai` to avoid redirect latency
- Consider reducing `maxTimeToLive` from 7 days to 8 hours for better security posture

---

## Implementation Roadmap

**Phase 1 — Security Hardening (P0)**
1. Add `inspect_message` guard
2. Add `created_at_time` to all ICRC transfers
3. Bump EVM RPC cycles to 10B
4. Implement CallerGuard on withdrawals (ICP, DOGE)
5. Replace `unbounded_wait` with `bounded_wait`
6. Fix deposit amount overflow handling

**Phase 2 — ckBTC Fixes (P1)**
7. Fix ckBTC owner parameter
8. Add per-user subaccount derivation
9. Add schema versioning to PersistentState
10. Soften post_upgrade panic

**Phase 3 — Architecture & Features (P2)**
11. Implement BTC withdrawals
12. Migrate to ic-stable-structures
13. Fix EVM RPC provider consensus handling
14. Add stale deposit cleanup
15. Unify deposit verification on ICRC-3

---

## Skills Installed

The following DFINITY ICP Skills were installed and used for this audit:
- `ckbtc` — ckBTC deposit/withdrawal patterns
- `evm-rpc` — EVM RPC canister usage and cycle management
- `internet-identity` — Authentication and principal derivation
- `canister-security` — Access control, reentrancy, async safety
- `stable-memory` — State persistence and upgrade safety
- `icrc-ledger` — ICRC-1/ICRC-2 token transfer patterns
- `asset-canister` — Frontend deployment and certified assets

Installed via: `npx skills add dfinity/icskills --skill <name> -y`
Location: `.agents/skills/`
