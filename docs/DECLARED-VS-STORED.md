# Every place this repository declares something that lives somewhere else

## Why this file exists

Four separate defects in this project share one shape. Each time, a file said
what something *was*, the thing itself was stored or built elsewhere, and nothing
compared the two:

| # | The declaration | The reality | How it was found |
|---|---|---|---|
| 1 | `src/table_canister/table_canister.did` | the interface the wasm exports | an audit, after it had been wrong for months |
| 2 | `icp.yaml` `init_args` | the config a canister is running | **a player**, from a strikethrough in the lobby |
| 3 | the `Dockerfile` `COPY` list | the crates the build needs | nobody — the image had never been built |
| 4 | `.gitignore` | a build input inside an ignored directory | CI, on its first real run |

Defect 2 is the one to remember. `btc_table_1` ran on blinds of 10/200 sats and a
buy-in range of 1,000–20,000 **for its entire life**, while `icp.yaml` declared
100/200 and 10,000–100,000: ten times larger across every field. `icp deploy
--mode upgrade` does not re-run init args — it cannot, they are install-time
input — so from the first upgrade onward the manifest described an intention, not
a canister. Nine upgrades, four blind audits and a full CI suite went past it.

The failure was never detection. It was that **nobody had written down that the
two things were supposed to be equal**, so no instrument was ever pointed at the
gap. This file is that written-down list.

## The rule

> Anything declared in one place and stored or built in another **will** drift,
> and it will drift silently unless something compares them.

A row here without a guard is not a hypothetical. It is a defect waiting for
somebody to notice it, and the historical record above says that somebody is
often a player or an outside auditor rather than us.

## The inventory

Guard column: **automated** = something fails if the two disagree; **by
construction** = they cannot disagree without a build breaking; **none** = nothing
compares them.

### D. The interface

| id | Declared in | The reality it describes | Guard |
|---|---|---|---|
| D1 | `src/{lobby,table,history}_canister/*.did` | the interface the built wasm exports | **automated** — `scripts/check-candid.sh`, CI job *Candid interface drift* |
| D2 | the same `.did` files | the `candid:service` metadata a **live canister publishes** | **automated** — `scripts/check-deployed.sh`, scheduled `deployed-drift.yml` |
| D3 | `src/declarations/<n>/<n>.did` | the canister `.did` it is generated from | **automated, baselined** — `check-candid.sh --declarations` against `scripts/candid-declarations-baseline.txt`. **82 known-drifted items**, see below |
| D4 | `src/declarations/<n>/<n>.did.js` | the canister `.did` — *this is the file the frontend actually builds actors from* | **none** — see "Known gaps" |
| D5 | `src/declarations/<n>/<n>.did.d.ts` | the `.did.js` beside it | **none** |

`.did` drift is not cosmetic: `recipes/rust-reproducible.hbs` embeds the file
verbatim as the module's **public `candid:service` metadata**, so a stale one is
a canister publishing a false description of itself on-chain, and it is what
third-party tooling reads to build a client.

It also does not fail loudly. Candid decodes a bare value into an `opt` without
complaining, and drops record fields the receiver does not declare. A client
built from the stale lobby `.did` read every table's `currency` as null; a client
built from the stale table `.did` never saw `sitting_out_since`. Correct-looking
replies, missing information, no error anywhere.

### C. The configuration and the deployment

| id | Declared in | The reality it describes | Guard |
|---|---|---|---|
| C1 | `icp.yaml` `init_args` | the `TableConfig` a canister is running | **automated** — `scripts/check-deployed-config.sh`, now also scheduled |
| C2 | source at a commit | the module hash actually deployed | **automated** — `scripts/verify-build.sh --mainnet`; `deployed-drift.yml` checks the `git:revision` half daily |
| C3 | `main` | the revision each canister reports (`git:revision` metadata) | **automated** — `check-deployed.sh`, including "is the whole fleet on ONE revision" |
| C4 | — | whether a canister was built from a **clean** tree (`git:dirty`) | **automated** — `check-deployed.sh --require-clean` |
| C5 | `canister_ids.json` | `.icp/data/mappings/ic.ids.json` | **automated** — `scripts/check-declarations.sh` |
| C6 | `deploy-ic.yml` `VITE_CANISTER_ID_*` | the live lobby/history ids | **automated** — `check-declarations.sh` |
| C7 | `README.md` canister ids | the ids this project runs | **automated** — `check-declarations.sh` |

### T. The toolchain (all of these move the module hash)

| id | Declared in | Also declared in | Guard |
|---|---|---|---|
| T1 | `rust-toolchain.toml` `channel` | `RUST_TOOLCHAIN` in 3 workflows, `FROM rust:<v>` in `Dockerfile` | **automated** — `check-declarations.sh` |
| T2 | `Dockerfile` `IC_WASM_VERSION` | `ci.yml`, `deploy-ic.yml`, `recipes/rust-reproducible.hbs` | **automated** — `check-declarations.sh` |
| T3 | `Dockerfile` `ICP_CLI_VERSION` | `ci.yml`, `deploy-ic.yml`, `cycles-monitor.yml`, `deployed-drift.yml` | **automated** — `check-declarations.sh` |
| T4 | `scripts/check-candid.sh` `CANDID_EXTRACTOR_VERSION` | `ci.yml` | **automated** — `check-declarations.sh` |
| T5 | *the absence of a version* — `cargo install <tool> --locked` | — | **automated** — `check-declarations.sh` fails on any unpinned install of a hash-moving tool |

**T2 and T3 were both wrong when this list was written.** `deploy-ic.yml` ran
`cargo install ic-wasm --locked` with **no version**, inside a step titled
*"Install pinned toolchain"* — the name asserted a property the code did not
have. CI proves the build reproducible with ic-wasm 0.9.9; the mainnet deploy
used whatever crates.io served that morning. The `Dockerfile` states the
consequence in its own words: *"a different ic-wasm shrinks differently and the
module hash moves."* `deploy-ic.yml` and `cycles-monitor.yml` also pinned icp-cli
1.0.0 while `ci.yml` and the `Dockerfile` pinned 1.0.2. All fixed and now
compared.

### B. The build inputs

| id | Declared in | The reality it describes | Guard |
|---|---|---|---|
| B1 | `Dockerfile` `COPY` list | the crates in `Cargo.toml` `[workspace] members` | **by construction** — CI job *The verification image a stranger actually runs* builds the image, and a missing crate fails it |
| B2 | `ci.yml` reproducible-build `cp -R` list | the same crates | **by construction** — the build fails without them |
| B3 | `.gitignore` | files that are build **inputs** | **by construction** — `build-frontend` builds from a fresh checkout |
| B4 | `Cargo.lock` | `Cargo.toml` | **by construction** — every build passes `--locked` |
| B5 | `icp.yaml` asset recipe version | the `icp-cli` version that can render it | **none** — see "Known gaps" |

### R. The registers

| id | Declared in | The reality it describes | Guard |
|---|---|---|---|
| R1 | `docs/DEFECTS.md` status markers | whether the defect is actually fixed | **none** |
| R2 | `docs/SECURITY-FINDINGS.md` headers | whether the finding is actually closed | **none** |

R1 and R2 are the same shape as every row above, applied to our own bookkeeping,
and they have already cost a wave: FINDING 15 and FINDING 17 were closed in later
waves and their headers were never updated, so six findings sat unscheduled
because the register could not be trusted at a glance.

## Known gaps, stated plainly

**D4 — `src/declarations/*/*.did.js` is unguarded, and it is the file that
matters most.** `src/cleardeck_frontend/src/lib/canisters.js` builds every actor
from `.did.js`, not from `.did`. Measured on 2026-08-06, `table_1.did.js` is
missing `get_solvency`, `get_all_ledger_intents`, `get_cycle_status`,
`refresh_solvency` and `get_deposit_replay_state` — the fund-safety instruments
built in earlier waves are, as of today, **not reachable from the UI at all**,
because the bindings were never regenerated. `lobby.did.js` still declares
`currency : IDL.Opt(Currency)` against a canister that exports a bare `Currency`.

Both are currently *masked* by defensive frontend code — `utils.js:currencyOf`
unwraps either shape, and `WithdrawModal.svelte:331` guards with
`if (!tableActor?.get_custody_status)`. That masking is why the drift was
invisible, and it is not a fix.

It is unguarded here because comparing a `.did.js` to a `.did` means evaluating
the IDL factory and rendering it back to Candid, for which there is no stable
public API — and a half-tested comparator over files this task does not own is a
liability, not a guard. The cheap correct fix is to **regenerate the declarations
from the corrected `.did` files**, which is mechanical and belongs to whoever
owns `src/declarations` and `src/cleardeck_frontend`.

**D3 baseline.** `scripts/candid-declarations-baseline.txt` pins 82 currently
drifted items so the check can go red on anything *new*. It is a ratchet, not an
amnesty: the check also fails if a baselined line stops being true, so the list
can only shrink. It is not a statement that the drift is acceptable.

**B5.** `icp.yaml` pins the asset recipe at `v2.2.1` and its header explains that
icp-cli 1.0.2 removed the step type `v2.1.0` emitted, so the wrong pairing makes
*every* `icp` command fail. Nothing compares the recipe version against the CLI
version; the coupling is only described in a comment. Cheap to guard once the
compatible ranges are written down somewhere machine-readable.

**R1 / R2.** Not guarded here. Making the register true is a separate task this
wave; a mechanical guard would need a status vocabulary the documents do not yet
have.

## How to run the guards

```sh
./scripts/check-declarations.sh                 # file vs file. No build, no network, seconds.
./scripts/check-candid.sh --declarations        # committed .did vs built wasm, + the baseline
./scripts/check-deployed.sh --network local     # a running deployment vs this tree
./scripts/check-deployed-config.sh --network local
```

`check-deployed.sh` against `--network ic` is **read-only**: its only network
operations are `icp canister metadata` on public sections and `icp canister call
--query`. `.github/workflows/deployed-drift.yml` runs it daily and refuses to
start if a mutating `icp` command ever appears in it or in the scripts it calls.

## If you add a row

Add it here first, then decide whether it can be guarded cheaply. A row with
**none** in the guard column is a promise that somebody will eventually find it
the hard way.
