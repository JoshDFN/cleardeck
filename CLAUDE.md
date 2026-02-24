# ClearDeck Poker - Security Guidelines for AI Agents

## Critical: This Canister Holds Real User Funds

Every function that touches BALANCES, transfers tokens, or modifies player chips is security-critical.
**Never** use `--mode reinstall` on production canisters (destroys all state including user balances).

## Secure Deposit Patterns on ICP

### 1. ICRC-2 Approve + Transfer_from (Primary - `deposit()`)
The caller approves the canister, then the canister pulls funds. Safe because:
- `from` is always `Account { owner: caller }` - the caller is the source
- The canister is the spender executing the approved transfer
- No third party can redirect funds

**NEVER** allow a function where `caller != from_principal` when crediting the caller's balance.

### 2. Subaccount-based Deposits (External Wallets - `claim_external_deposit()`)
Each user gets a unique deposit address: `(canister_id, sha256("cleardeck-deposit:" || principal))`.
External wallets transfer to the user's unique subaccount, then the user calls `claim_external_deposit()` to sweep funds into their balance.

**Why this is secure**: Only the user whose principal derived the subaccount can claim it. The subaccount is deterministic and unique per user.

### 3. Block Verification (Legacy - `notify_deposit()`)
User transfers to canister, then calls `notify_deposit(block_index)`. The canister queries the ledger and:
- Verifies the transfer destination matches the canister
- **Verifies the transfer sender matches the caller** (via `compute_account_identifier`)
- Marks the block as verified to prevent double-claims

### Patterns to NEVER Use

**DO NOT** create a deposit function that:
- Accepts a `from_principal` parameter and credits the **caller** (not the from_principal)
- Allows any caller to claim any unverified block index
- Trusts the caller's claim of identity without verification
- Credits balances before verifying the transfer on the ledger

**DO NOT** use ICRC-2 transfer_from where the `from` account is a parameter chosen by the caller
and the balance is credited to someone other than the `from` account owner.

## Caller Principal Security

```rust
// ALWAYS capture caller BEFORE any await
let caller = ic_cdk::api::msg_caller(); // FIRST line of function
// ... then await ...

// NEVER call ic_cdk::api::msg_caller() after an await
// After await, msg_caller() returns the CALLEE (canister itself), not the original caller
```

## Anonymous Caller Protection

All functions that modify balances, deposits, withdrawals, or game state must reject anonymous callers:
```rust
if caller == Principal::anonymous() {
    return Err("Anonymous callers cannot perform this action".to_string());
}
```

## Balance Arithmetic

Always use `saturating_add` / `saturating_sub` for balance operations to prevent overflow/underflow:
```rust
// GOOD
balances.insert(caller, current.saturating_add(amount));

// BAD - can overflow
balances.insert(caller, current + amount);
```

## Withdrawal Safety (Reentrancy Prevention)

1. Deduct balance **before** the async transfer call
2. Track pending withdrawals to prevent concurrent attempts
3. Refund on failure using `saturating_add`

```rust
// 1. Atomic deduct + mark pending (before await)
BALANCES.with(|b| { /* deduct */ });
PENDING_WITHDRAWALS.with(|p| { /* mark pending */ });

// 2. Async transfer
let result = transfer_tokens(caller, amount).await;

// 3. Clear pending state
PENDING_WITHDRAWALS.with(|p| { /* clear */ });

// 4. On failure, refund
if result.is_err() {
    BALANCES.with(|b| { /* saturating_add refund */ });
}
```

## State Persistence

- Use `thread_local! { RefCell<T> }` for runtime state
- State does NOT survive upgrades by default - must serialize to stable memory
- `post_upgrade` should PANIC on state restoration failure to reject bad upgrades
- Never use `--mode reinstall` on production (destroys all state)

## Admin Access Control

All admin functions must call `require_controller()`:
```rust
fn admin_function() -> Result<(), String> {
    require_controller()?;
    // ... admin logic ...
}
```

## Rate Limiting

All user-facing update calls should be rate-limited to prevent DoS:
- Player actions: 10/second
- Deposit verifications: 5/minute
- Heartbeats: 2/second
- Withdrawals: 60-second cooldown
