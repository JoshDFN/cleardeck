# ClearDeck CI/CD

ClearDeck is a **public** repository whose Rust canisters hold **real ICP + ckBTC**
on mainnet. The workflows here are built around one rule: **no automated path may
merge to `main` or deploy a fund-holding canister.** All merges and all deploys
stay human-gated.

## Workflows

| Workflow | Trigger | Purpose | Touches mainnet? |
|---|---|---|---|
| `ci.yml` | push / PR to `main` | `cargo build` (wasm32), `cargo test`, **the fast fund-safety tier**, **the suite-wiring gate**, candid-drift check, reproducible build, frontend build | No (read-only) |
| `fund-safety-deep.yml` | nightly / push to `main` / manual | The fund-safety runs whose value is depth: the 9×600 fuzz, the fuzzer at its own defaults, the 138-state stall sweep, the on-chain clock, the cycles runway, the oracle's full sweep | No (PocketIC, in-process) |
| `security.yml` | push / PR / weekly | `cargo audit` + `npm audit`; fails any PR that adds `--mode reinstall` on a backend canister | No |
| `cycles-monitor.yml` | twice daily / manual | Reads every mainnet canister's own `get_cycle_status` and **fails below 60 days of measured runway**, treating a canister that will not answer as the alarm. Opens a `cycles` incident. docs/DEFECTS.md E-55 | **Read-only, anonymous — holds no secret at all** |
| `deploy-ic.yml` | **manual only** | Gated, upgrade-only mainnet deploy | **Yes — gated** |

## The fund-safety tiers (docs/DEFECTS.md H-23)

Until wave 13, **every fund-safety result in this project's documents came from a harness no CI job
invoked.** `cargo test --locked --workspace` could not reach one of them: `tests/money_safety`,
`tests/settlement` and `tests/no_peeking` each carry a bare `[workspace]` on purpose, so that the
six deployed canisters keep building byte-reproducibly — and that put every money invariant outside
the only test command CI ran.

- **`fund-safety-fast`** in `ci.yml` runs on every pull request, and because `deploy-ic.yml` does
  `uses: ./.github/workflows/ci.yml`, **a mainnet deploy cannot proceed while it is red.**
  > ⚠️ **IT IS NOT A REQUIRED STATUS CHECK, AND UNTIL SOMEBODY MAKES IT ONE IT BLOCKS NO MERGE.**
  > Verified against the API on 2026-08-09:
  > `gh api repos/JoshDFN/cleardeck/branches/main/protection/required_status_checks` answers
  > `404 Required status checks not enabled`, and `.../rulesets` answers `[]`. See
  > "Recommended branch protection" below — that section is the one-click follow-up this whole
  > directory depends on.
- **`fund-safety-deep.yml`** carries the slow runs. `stall_agreement` alone is 802 s measured; a
  required check that takes an hour gets marked non-required, and then the fast half stops being
  required either.
- **What is in which tier, and why, is one line per suite in `scripts/test-suites.list`.** Both
  workflows call `scripts/ci-fund-safety.sh <tier>`, which executes that file — the rows are not a
  description of what CI runs, they *are* what CI runs.
- **`Candid interface drift`** now has a second step, `./scripts/check-declarations-js.sh --selftest`,
  which regenerates `src/declarations/<n>/<n>.did.js` and `.did.d.ts` from the committed `.did` and
  diffs. That file is what the app builds every actor from and it was a hand-maintained third copy:
  table_1's binding was missing sixteen methods, including `claim_external_deposit`, `get_solvency`
  and `refresh_solvency`, and declared two the canister does not have (docs/DEFECTS.md D-11).

- **`suite-wiring`** runs `scripts/check-suite-wiring.sh`, which fails the build when any cargo
  test target in the tree is run by no tier. It `--selftest`s its own ability to go red before it
  judges. Adding a test suite and not wiring it is now a build failure rather than a suite that
  sits unrun for a wave.

Neither tier can reach mainnet: PocketIC starts an in-process replica, and the only network access
is two pinned, checksummed downloads (the PocketIC server and the real ICP ledger module).

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
| `IC_DEPLOY_IDENTITY` | `deploy-ic.yml` | **base64-encoded PEM** of the funded deploy identity. Store as an **encrypted Actions secret only** — NEVER commit it. This is a public repo: fork PRs do **not** receive secrets, and these workflows never run on `pull_request`. |
| `GITHUB_TOKEN` | `cycles-monitor.yml`, `deployed-drift.yml` | Provided automatically. Used only to open an incident issue. |

> **`cycles-monitor.yml` used to be on the first row and is not any more, and that is the
> point.** `icp canister status` is controller-only, so reading a cycles figure with it meant
> base64-decoding the DEPLOY key into `/tmp` on a schedule, every day, for a read — a
> credential that can `install_code` on a canister custodying real ICP
> ([FINDING 23](../../docs/SECURITY-FINDINGS.md#finding-23)), sitting in a cron job for the
> sake of a status line. The canister publishes its own runway now (`get_cycle_status`, a
> public query), so the job authenticates as **nobody** and cannot deploy even if the runner
> were compromised. `scripts/assert-read-only.sh` fails the run if a mutating command, a
> non-query call or any reference to an identity or a secret ever appears in either that
> workflow or the script it runs — and it self-tests against synthetic positives first, so a
> guard that has stopped matching anything fails instead of passing quietly.

Create the deploy secret with:

```bash
icp identity export <funded-identity> | base64 | gh secret set IC_DEPLOY_IDENTITY -R JoshDFN/cleardeck
```

Also set an **Anthropic-independent spend safeguard**: a cycles top-up alert (this monitor) and a funded backup controller on each canister.

## Recommended branch protection — NOT YET DONE, and it is what makes everything above real

**Status, checked 2026-08-09:** `main` carries `required_approving_review_count = 1` and
`enforce_admins = false`, **no required status checks and no rulesets**. Every job in this
directory is therefore advisory. The fast tier is proved able to convict a planted 1% skim in
`apply_payouts`; today that conviction arrives as a red X beside a merge button that already works.

In *Settings → Branches*, protect `main`:
- Require `ci.yml` (and `security.yml`) status checks to pass. **`fund-safety-fast` and
  `suite-wiring` are the two that must not be dropped from the required list** — the first is the
  only thing standing between a merge and an unexercised money invariant, and the second is what
  keeps a new suite from being added and never run.
- Require a pull request review before merge.
- Disallow force-pushes.

## One-time follow-ups

- **Candid hard-fail:** `candid-check` in `ci.yml` is **warn-only**. After reconciling the committed `src/*/*.did` files to `candid-extractor` output once, change its final `exit 0` to `exit $fail` to hard-fail on real interface drift.
- **`icp canister status` field names:** confirm the exact `Balance` / `Module hash` line format on first run and adjust the greps in `scripts/verify-build.sh` if needed. `cycles-monitor.yml` no longer parses `canister status` at all: it reads the canister's own `get_cycle_status` query, so it needs no controller rights and no field-name guess.

## Deliberately NOT included

No auto-merge / auto-fix / auto-deploy agent fleet. For a fund-holding app the
cost/benefit is wrong: an agent that auto-fixes CI and auto-merges to `main` is
one approval-click from a fund-canister upgrade. If you want an **opt-in,
review-only** assistant (comments only, never approve/merge), add a separate
`claude.yml` triggered on `@claude` mentions with a `CLAUDE_CODE_OAUTH_TOKEN`
secret and `pull-requests: write` (no `contents: write`, no deploy dispatch).
