# ClearDeck Modernization

Brings ClearDeck onto the current ICP toolchain and conventions used across the
rest of the fleet (ForumIC / MotokoHR / CompanyIC) and DFINITY's `icskills`
guidance. Scope of this pass: **local working-tree changes only** — nothing here
deploys to mainnet or changes the live fund canisters by itself.

## 1. Build/deploy: `dfx` → `icp-cli`

- **`icp.yaml`** is now the project manifest; `dfx.json` was removed (single source of truth).
  - All six Rust canisters on `@dfinity/rust@v3.2.0`; the four table instances
    share the `table_canister` package and carry their distinct init args via
    `init_args.value`.
  - Frontend on `@dfinity/asset-canister@v2.1.0`.
  - Environments: `local` (`ii: true`) and `ic`.
- **`.icp/data/mappings/ic.ids.json`** maps canister names → the **live mainnet
  IDs** (from `canister_ids.json`). This is committed and is what makes
  `icp deploy` **upgrade** the existing fund canisters instead of creating new
  ones. ⚠️ Without it, a deploy would orphan the live canisters.
- **Declarations are committed** under `src/declarations/` (icp-cli has no
  `dfx generate`); the frontend's `prebuild` no longer shells out to dfx.
- **Scripts:** `scripts/deploy-mainnet.sh` rewritten for `icp` and **forces
  `--mode upgrade`** on the six fund canisters (`icp deploy` defaults to
  `--mode auto`, which can pick `install`/reinstall on a canister it deems
  fresh — a fund-wipe risk). `deploy-production.sh` (stale, referenced
  nonexistent canisters) was deleted. `verify-build.sh` + `Dockerfile` use
  `icp`.
- Target CLI: **`@icp-sdk/icp-cli@1.0.0` (GA)**. The old "pin 0.2.7" note
  predates GA.

> First icp deploy resets the verified module-hash baseline (the build toolchain
> changed from dfx to the `@dfinity/rust` recipe). Re-run `verify-build.sh` after
> it to capture the new baseline.

## 2. Identity: Internet Identity → `id.ai`, agent host → `icp-api.io`

- Frontend II provider is now **`https://id.ai/authorize`** (II v2; the
  `/authorize` path is required — the bare origin hangs the handshake).
- Agent host is now **`https://icp-api.io`** (replaces `ic0.app`) everywhere.
- Both are centralized in `src/cleardeck_frontend/src/lib/ic-config.js` and are
  **env-overridable** (`VITE_II_URL`, `VITE_IC_HOST`) for instant rollback.
- **Existing balances are safe:** II principals derive from (passkey/anchor,
  frontend origin); the origin and the II backend canister (`rdmx6`) are
  unchanged, so returning players get the same principal.
- Takes effect only after a **frontend rebuild + asset-canister redeploy** (no
  backend upgrade needed).

## 3. CI/CD hardening (was: none)

`.github/workflows/`:
- `ci.yml` — cargo build (wasm32) + cargo test + candid-drift + frontend build.
- `security.yml` — `cargo audit` + `npm audit` + a static gate that fails any PR
  adding `--mode reinstall` on a backend canister.
- `cycles-monitor.yml` — daily low-balance → opens an `incident` issue.
- `deploy-ic.yml` — **manual + `IC mainnet` environment-gated, upgrade-only**
  mainnet deploy. No push trigger; no auto-merge/auto-fix/auto-deploy fleet.
- `rust-toolchain.toml` pins the compiler; third-party actions are SHA-pinned.

See `.github/workflows/README.md` for the gate model and required secrets.

## 4. OQL / World-Model — N/A for ClearDeck

`mo:oql` is a **pure-Motoko** library; ClearDeck's canisters are **Rust**, so it
cannot be imported or "dropped in." Matching ForumIC/MotokoHR's queryable graph
nodes would mean reimplementing a JSON query engine in Rust — large, and **no
consumer is asking for it**. The only language-agnostic path (exposing
`tip()`/`range()` so `oql-browser` could read hand history) would leak player
principals + showdown hole-cards to anonymous scraping for no benefit.

**Verdict: OQL is closed as N/A.** Revisit only if a real `oql-browser` consumer
for ClearDeck hand history appears.

## 5. Deferred (separate gated upgrade)

Backend fund-safety items (`inspect_message` guard, `pre_upgrade` trap decision,
`saturating_sub` normalization) were intentionally deferred — they require a
gated mainnet upgrade. See `docs/BACKEND-FUND-SAFETY-TODO.md`.
