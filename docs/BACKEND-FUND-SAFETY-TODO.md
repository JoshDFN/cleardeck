# Backend fund-safety — deferred upgrade

These backend changes were **intentionally deferred** from the icp-cli / id.ai /
CI modernization pass. They are real fund-safety improvements but each requires a
**gated mainnet canister upgrade**, so they must ship as their own
locally-replica-tested change — never bundled with the tooling/frontend work.

> Ship procedure for anything here: build + test on a **local replica** first,
> then `icp canister snapshot create <c> -e ic` per canister, then
> `icp deploy -e ic <c> --mode upgrade` (NEVER reinstall). See
> `.github/workflows/deploy-ic.yml`.

The Rust backend is otherwise in good shape: it already uses the modern
`ic_cdk::api::msg_caller()` API (captured before `await`), saturating balance
arithmetic in the hot paths, reentrancy-guarded withdrawals, and a `post_upgrade`
that panics on restore failure. The items below are the remaining gaps.

## 1. `#[ic_cdk::inspect_message]` guard (MEDIUM)

There is currently **no `inspect_message`** in any canister, so anonymous and
malformed update calls are fully ingested (and cost cycles) before the inline
auth checks reject them. icskills recommends rejecting cheaply at the inspect
boundary.

- Add `#[ic_cdk::inspect_message]` to `table_canister`, `lobby_canister`,
  `history_canister`.
- **Deny by default** for unknown methods; reject `Principal::anonymous()` for
  fund/state-mutating methods; accept the known public methods — mirroring each
  method's *existing* inline auth exactly.
- ⚠️ An over-strict guard locks out legitimate users. This MUST be tested on a
  local replica against the real call set before any mainnet upgrade.
- `inspect_message` is a cycle-cost optimization, NOT a security boundary — keep
  every inline auth check in place.

## 2. `pre_upgrade` swallows `stable_save` failure (MEDIUM → fund-safety)

All three fund canisters log-and-proceed if serialization to stable memory fails
during `pre_upgrade` (approx `table_canister/src/lib.rs:4543`,
`lobby_canister/src/lib.rs:823` — comment literally says "with potential data
loss", `history_canister/src/lib.rs:509`). If serialized state ever exceeds the
upgrade instruction limit as balances/history grow, the upgrade proceeds and
restores partial/empty state — **silent balance loss**. This is asymmetric with
the `post_upgrade` path, which correctly panics on restore failure.

**Owner decision required:**
- **Trap/abort on save failure** (recommended for a fund canister): aborts the
  upgrade, keeping the old code + intact in-memory state. Before switching,
  confirm the current serialized state fits the instruction limit so a
  legitimate upgrade isn't permanently blocked.
- **Keep current "log and proceed"**: accepts the silent-loss risk.

## 3. Normalize guarded raw subtractions to `saturating_sub` (LOW, consistency)

A handful of balance subtractions use raw `-` behind a prior bounds check
(approx `table_canister/src/lib.rs:1684, 1814, 1882, 2884`). They are currently
safe because of the guard, but `CLAUDE.md` mandates `saturating_sub` everywhere
for defense-in-depth. Pure consistency change; keep the guards.

## 4. (Separate) `verifyQuerySignatures: false` on the mainnet agent (review)

Not a backend change, but related and flagged here so it isn't lost:
`src/cleardeck_frontend/src/lib/canisters.js` ships `verifyQuerySignatures: false`
to mainnet ("there may be subnet key issues"). For a fund app, uncertified query
responses (e.g. balance reads) are an integrity concern. Decide whether to
re-enable certification (and resolve the original subnet-key issue) in a separate,
verified frontend change.

## Explicitly out of scope

- **No crate bumps without a published advisory.** `ic-cdk` (0.19), `candid`
  (0.10), `icrc-ledger-types` are modern; any bump forces a fund-canister upgrade
  for no security benefit. Let `security.yml`'s `cargo audit` flag a real CVE.
- **No balance-storage migration** (bulk `stable_save` → `StableBTreeMap`) as a
  casual change — that is a carefully-staged project of its own.
- **No Rust OQL `schema()`/`execute()` surface** — see `docs/MODERNIZATION.md`
  for why OQL is N/A for ClearDeck.
