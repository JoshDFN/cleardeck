# ClearDeck CI/CD

ClearDeck is a **public** repository whose Rust canisters hold **real ICP + ckBTC**
on mainnet. The workflows here are built around one rule: **no automated path may
merge to `main` or deploy a fund-holding canister.** All merges and all deploys
stay human-gated.

## Workflows

| Workflow | Trigger | Purpose | Touches mainnet? |
|---|---|---|---|
| `ci.yml` | push / PR to `main` | `cargo build` (wasm32), `cargo test`, candid-drift check, frontend build | No (read-only) |
| `security.yml` | push / PR / weekly | `cargo audit` + `npm audit`; fails any PR that adds `--mode reinstall` on a backend canister | No |
| `cycles-monitor.yml` | daily / manual | Opens an `incident` issue if any canister is below 1T cycles | Read-only status |
| `deploy-ic.yml` | **manual only** | Gated, upgrade-only mainnet deploy | **Yes — gated** |

## The deploy gate

`deploy-ic.yml` is intentionally:

- **`workflow_dispatch` only** — no `push` trigger. A fund app must never deploy on merge.
- **`environment: IC mainnet`** — add a **required reviewer** in *Settings → Environments → IC mainnet* so every run waits for a human approval.
- **upgrade-only** — backend canisters (`lobby`, `table_*`, `btc_table_1`, `history`) are deployed with `--mode upgrade`. **Reinstall is never used on a backend canister** — it wipes stable state, including balances. The frontend asset canister can be reinstalled only via the explicit `allow_frontend_reinstall` input.
- **needs `ci`** — the deploy job runs the full CI suite first.
- **identity-shredding** — the deploy PEM is imported, used, and deleted on `if: always()`.

Rollback: there is no `--rollback`. `deploy-ic.yml` takes a `icp canister snapshot create` of each backend canister before upgrading; restore with `icp canister snapshot restore`.

## Required repository secrets

| Secret | Used by | Notes |
|---|---|---|
| `IC_DEPLOY_IDENTITY` | `deploy-ic.yml`, `cycles-monitor.yml` | **base64-encoded PEM** of the funded deploy identity. Store as an **encrypted Actions secret only** — NEVER commit it. This is a public repo: fork PRs do **not** receive secrets, and these workflows never run on `pull_request`. |
| `GITHUB_TOKEN` | `cycles-monitor.yml` | Provided automatically. |

Create the deploy secret with:

```bash
icp identity export <funded-identity> | base64 | gh secret set IC_DEPLOY_IDENTITY -R JoshDFN/cleardeck
```

Also set an **Anthropic-independent spend safeguard**: a cycles top-up alert (this monitor) and a funded backup controller on each canister.

## Recommended branch protection

In *Settings → Branches*, protect `main`:
- Require `ci.yml` (and `security.yml`) status checks to pass.
- Require a pull request review before merge.
- Disallow force-pushes.

## One-time follow-ups

- **Candid hard-fail:** `candid-check` in `ci.yml` is **warn-only**. After reconciling the committed `src/*/*.did` files to `candid-extractor` output once, change its final `exit 0` to `exit $fail` to hard-fail on real interface drift.
- **`icp canister status` field names:** confirm the exact `Balance` / `Module hash` line format on first run and adjust the greps in `cycles-monitor.yml` and `scripts/verify-build.sh` if needed.

## Deliberately NOT included

No auto-merge / auto-fix / auto-deploy agent fleet. For a fund-holding app the
cost/benefit is wrong: an agent that auto-fixes CI and auto-merges to `main` is
one approval-click from a fund-canister upgrade. If you want an **opt-in,
review-only** assistant (comments only, never approve/merge), add a separate
`claude.yml` triggered on `@claude` mentions with a `CLAUDE_CODE_OAUTH_TOKEN`
secret and `pull-requests: write` (no `contents: write`, no deploy dispatch).
