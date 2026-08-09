# Wave 11: the ninth cross-agent defect deletes evidence, and the fuzzer convicted an innocent module

> **A NOTE ON THE NUMBER, carried forward from [WAVE-09.md](WAVE-09.md) and
> [WAVE-10.md](WAVE-10.md).** This is the eleventh wave **document**. The register's `wave` column
> runs ahead of it: the rows this document lands say `13`. The two counters are not the same
> counter. Where a row says wave 13, it means the wave this document covers.

---

## THE ONE SENTENCE FOR THE LEAD

**Safe to merge and safe to redeploy backends from, because the one thing that was NOT safe was
found and closed inside this pass: [E-71](DEFECTS.md#e-71)'s new archive key let any authorised
table silently DELETE another table's hand record, and it shipped as part of the change that
narrowed the key.** The true open count is **68** (58 in the register, 10 in the findings: 3
fund-theft, 1 critical, 19 high, 30 medium, 15 low), and `./scripts/dev.sh fuzz-default` — and
therefore `./scripts/dev.sh test` — is still **RED on [E-89](DEFECTS.md#e-89) alone**, which is the
pre-existing [FINDING 31](SECURITY-FINDINGS.md#finding-31) dead-band defect carried from wave 12 and
is a `src/table_canister` change the lead has to authorise because it moves every deployed module
hash. **The single most valuable next item is not a defect at all: turn `fund-safety-fast` on as a
required status check.** It is proved to convict a planted 1% skim, and
`gh api repos/JoshDFN/cleardeck/branches/main/protection/required_status_checks` answers
`404 Required status checks not enabled`, so today it blocks nothing — and until this pass it would
have arrived on a pipeline that could not go green anyway ([E-95](DEFECTS.md#e-95), fixed here).

> ### THE ONE RED, AND IT IS THE SAME RED WAVE 10 SHIPPED
>
> `fuzz-default` finds exactly one blocking violation on this tree:
>
> ```text
> money-fuzz: seed 0xc1ea2dec0001 finished: 2 hands, 4 upgrades, 0 blocking finding(s)
> money-fuzz: seed 0xc1ea2dec0002 finished: 2 hands, 0 upgrades, 1 blocking finding(s), worst stranded 20002 e8s
> money-fuzz: seed 0xc1ea2dec0003 finished: 4 hands, 3 upgrades, 0 blocking finding(s)
>   M9_FUND_REACHABILITY:money_left_behind_after_drain (seed 0xc1ea2dec0002, 5 ops)
> test result: FAILED. 0 passed; 1 failed   (308.30 s in the authoritative run)
> ```
>
> It arrived in wave 12 with an id, a five-operation reproducer and a written decision to leave it
> red rather than tolerate it. Nothing in this wave changed that, and nothing in this wave should
> have: making it green means fixing FINDING 31 in the canister, and that is a redeploy.
>
> **It came into this pass with a SECOND red beside it, and that one was false.** See §2.

---

## 1. THE NINTH CROSS-AGENT DEFECT: the archive's writer picks half of the hand's name

[E-91](DEFECTS.md#e-91). Found by hunting the seam between two agents' changes, which is where the
last eight have been.

[E-71](DEFECTS.md#e-71) gave a hand a permanent name — `hand_uid = "<table canister id>:<seed_hash>"`
— and narrowed the archive's de-duplication key to exactly that name. It is a good change and the
reasoning behind it is right. But `record_hand` authorises the **caller** and never binds
`record.table_id` to it, so **the writer chooses half of the name**, and the other half — the
shuffle commitment — is public: it is on the player's screen while the hand is running.

| | HEAD (`a65868e`) | after E-71 |
|---|---|---|
| key | `(table_id, hand_number, seed_hash)` | `(table_id, seed_hash)` |
| a forged record naming another table | its own record, own `hand_number` — **visible** | shares the victim's WHOLE name |
| the genuine hand that arrives after it | stored | **discarded, `Ok(forgery_id)` returned** |
| records surviving | 2 | **1, and it is the forgery** |

So the narrowing converted a *visible extra record* into a *silent deletion*. File the forgery
first and a genuine hand disappears from an archive whose entire promise is that it is permanent
and append-only, with an `Ok` on the way out. Reachable by any of the four authorised tables.

**Every instrument built this wave reads green over it, and that is the eight-times-repeated
signature.** `get_archive_integrity` recounts name collisions — there is no collision, there is one
record where there should be two. `index_disagrees_with_records` compares the index against the
records — they agree. `check_recorded_hand` is asked about a record that is present. **The failure
is an ABSENT record, and nothing in this tree counts absences.**

Closed by binding `record.table_id` to the caller, in a `may_record` function split out of the
canister entry point so the rule is host-testable — the same reason `insert_hand` was split out, now
written down twice. Five gates in `cargo test --workspace`; neutering the binding to `if false`
fails exactly one of them and leaves 22 green. The admin stays exempt, and that is **said** rather
than assumed: the archive's admin is its controller and can rewrite the canister by reinstalling
it, so refusing there would be a claim and not a control.

---

## 2. THE INSTRUMENT CONVICTED AN INNOCENT MODULE, AND IT WAS FILED AS A `high` FUND-CREATION DEFECT

[FINDING 44](SECURITY-FINDINGS.md#finding-44). This wave's critic found a deterministic red beside
E-89 — `M3_NO_RAKE … FundCreation … delta=+10000` — reproduced it three times byte for byte,
correctly demolished the "machine noise" explanation it had been given, and filed it as `high` with
the cause not bisected. The cause is the instrument.

The fuzzer's own shrinker produced the answer and nobody read it:

```json
[... {"StartNewHand": {"actor": 0}},
     {"ExternalDepositThenClaim": {"actor": 3, "amount": 10000}},   <-- REAL MONEY, MID-HAND
     {"ActInTurn": {"act": "Fold"}}]
```

`ExternalDepositThenClaim` is a real `icrc1_transfer` from a wallet into the deposit address the
canister published. The +10,000 e8s **is** that deposit.

M3's window guard is supposed to answer *"did anything outside move money"*. It asked
`idle_before.ledger_main == after.ledger_main`. The canister owns **two** classes of ledger account,
and `Snapshot::internal_total()` has counted the second one since wave 8. Measured in the gate that
now holds it:

```text
ledger_main       1600000000 -> 1600000000      the blind basis: unchanged
ledger_holdings   1600000000 -> 1600010000      the sound basis: +10000
unswept                    0 ->      10000
internal_total    1600000000 -> 1600010000      and M3's total moves with it
```

Two sides of one equation, drawn from two different sets of accounts. It reproduces
**byte-identically on the wave-12 canister source** (`git show HEAD:src/table_canister/src/lib.rs`
into a clone, this wave's generator, same seed, same `2 hands, 4 upgrades, +10000`), so no canister
change of this wave is implicated and no money was created.

Fixed by asking the question of `ledger_holdings()`. **The cost is stated rather than glossed:**
windows in which subaccount money moved are now skipped by M3 rather than mis-decided, so a chip
creation that coincided with a deposit is not seen *by M3* there — it is still seen by M1, M8, M9,
M11 and the settlement oracle, none of which are gated on this predicate.

**And the true red was checked before this was written**, because an instrument change that makes a
gate greener is exactly the change nobody should be trusted about: seed `0xc1ea2dec0002` still
produces E-89's 20,002 e8s, unchanged.

> An instrument that convicts the innocent is not a safe instrument. It cost the register a `high`
> fund-creation finding against a clean module, and it made the one TRUE red in the same row harder
> to see rather than easier.

---

## 3. Six things the register said that the world did not

Each of these was checked against the thing itself rather than against the sentence about it.

**1. [H-23](DEFECTS.md#h-23) said the fund-safety job is REQUIRED. Nothing is required.**
`gh api .../branches/main/protection/required_status_checks` → `404 Required status checks not
enabled`. `.../rulesets` → `[]`. `.../actions/workflows` lists CI, Cycles monitor, Deploy to IC
mainnet, Deployed drift and Security — **`Fund safety (deep tier)` is not among them**, so this
wave's CI work has never run in CI. The work itself is good and is provably able to convict a
planted theft; the claim about its status was not. Row corrected.

**2. [H-26](DEFECTS.md#h-26) said the new fuzz generator "costs no runtime". It costs 53% more, in
the configuration that actually runs.** The 55.6 s figure was measured with `MONEY_FUZZ_SHRINK=0`;
`dev.sh test` and `make fuzz-default` both `env -u` that variable. At the real default the new
generator was **597.6 s against HEAD's 389.5 s**, because it produced two shrink passes rather than
one — and one of those two was FINDING 44's false conviction. With that gone the step is **316.4 s**
on this tree. Row corrected with all three numbers.

**3. [E-55](DEFECTS.md#e-55)'s runway table leaves out the biggest per-tab term.**
[E-92](DEFECTS.md#e-92), filed OPEN. `loadTableState` runs on `setInterval(…, 500)` and its first
statement is `await tableActor.check_timeouts()`, which is `#[ic_cdk::update]` and carries no
`query` in the `.did`. That is an update loop per open tab at a measured 6,573,911 cycles a call,
i.e. **0.28–1.14 T/day per tab** (the range is honest: `loadingTableState` blocks re-entry, so the
rate is bounded by finality, ~2 s on mainnet and near-zero on PocketIC) against the **0.0618 T/day**
the 10-second heartbeat contributes and which this wave headlined as the dominant cost. Six tabs add
1.7–6.8 T/day before a hand is dealt; the published "fully occupied table" figure is 0.4994 T/day
total. **Not patched here on purpose:** the honest fix is to re-measure the whole table with the
poll included and publish one set of numbers, and replacing one unverified constant with another is
how the 226-day figure got written in the first place. The canister's own `get_cycle_status` is
unaffected — its sliding window measures real consumption and already includes these calls, and it
says so: `./scripts/dev.sh cycles` immediately after this pass's screenshot sweep reports
**0.045 T/day for `btc_table_1`, which no browser ever opened**, against **0.185, 0.218 and
0.333 T/day** for the three tables a browser did — 4x to 7.4x the idle figure, from a handful of
hands rather than 500. The gauge is right and the documents are wrong.

**4. [FINDING 39](SECURITY-FINDINGS.md#finding-39)'s reachability was understated in the dangerous
direction, and its own critic had already corrected it in this file.** The register still reads
"five ordinary player calls"; the ablation table in FINDING 39 shows the minimum is **two calls and
the passage of time**, with the table's own on-chain clock finishing the job unattended. The
correction is in the findings file where the evidence is; the E-41 row's summary sentence is left as
its owner wrote it.

**5. main's CI cannot go green, for a reason that is a CI wiring fault.** [E-95](DEFECTS.md#e-95),
fixed here. The `Build frontend` job ran `npm run build` with `DFX_NETWORK: ic` and
`VITE_CANISTER_ID_* = aaaaa-aa`, under a comment saying the real ids resolve at deploy time. That
stopped being true when the mainnet-id cross-check landed in `vite.config.js`: an `ic` build whose
ids disagree with the tracked `.icp/data/mappings/ic.ids.json` now ABORTS, and a placeholder never
agrees. Reproduced by running the job's exact command. **This is why it is `high`:** wave 13 merged
the fund-safety jobs — the ones whose entire value is that somebody looks when they turn red — onto
a pipeline that had been red since 2026-08-07, where a new red is indistinguishable from the old
one. Switched to `build:mainnet`, the one named path, which reads the ids from the tracked mapping
itself and verifies the bundle; run locally, **8 of 8 bundle checks passed**.

**6. The guard on the credential-free cycles monitor was a list of yesterday's incidents.**
[E-94](DEFECTS.md#e-94), fixed here. `scripts/assert-read-only.sh` matched `secrets\.IC_` and a
seven-verb list, so a workflow carrying `secrets.DEPLOY_KEY_PEM_BLOB` and running
`icp identity use monitor` printed **`ok … read-only, anonymous`** and exited 0, as did one running
`icp canister create` and `icp canister migrate-id`. All four are refused now, all four are probe
lines in the guard's own `--selftest`, and the two real files it protects are still green. **It is
still not transitive** — it reads the workflow, not the scripts a workflow invokes — and that is
written into the entry rather than left for the next reader to discover.

---

## 4. What this pass did NOT do, stated plainly

* **It did not fix [E-89](DEFECTS.md#e-89) / [FINDING 31](SECURITY-FINDINGS.md#finding-31).** That
  is a canister change, it moves all six deployed module hashes, and it is the lead's call.
* **It did not turn on branch protection.** That is a repository setting and not a tree change.
* **It did not re-measure the cycles table.** [E-92](DEFECTS.md#e-92) says what is missing and by
  how much, so the next owner starts from a number rather than a suspicion.
* **It did not close [E-49](DEFECTS.md#e-49)'s admin half.** The archive's admin can still file a
  record for any table. That is a controller-class power, it is pinned by a test so it cannot be
  removed by accident and mistaken for a fix, and it is named in [E-91](DEFECTS.md#e-91).
* **It did not verify the archive's OLD→NEW upgrade in a gate.** The critic supplied that proof by
  hand (40 records written by the HEAD module, upgraded, 40/40 byte-identical) and the tree still
  has no gate for it: `upgrade_across_versions.rs` is the table canister only.

---

## 5. Byte-reproducibility, and one thing it turned up

**The claim holds where it is made.** The same source built at two paths with the invocation the
deploy recipe uses (`cargo build --package <p> --target wasm32-unknown-unknown --release`):

```text
                     working tree                          cp -Rc clone
table_canister       3890a6d4…46b29a0d                     3890a6d4…46b29a0d
history_canister     ea701f77…d60f862e15                   ea701f77…d60f862e15
```

**And `history_canister.wasm` changes with the `-p` set of the build that produced it.**
[E-93](DEFECTS.md#e-93), `medium`. `cargo build -p table_canister -p history_canister` in the same
directory, clean, yields `9f511048…3db2d` for the archive while `table_canister` stays
`3890a6d4…b29a0d` — cargo unifying dependency features across the packages named in one invocation.

**The repository has both invocations, so this is live.** The deploy recipe and every PocketIC
harness build one package at a time; `scripts/check-candid.sh` line 142 builds the whole workspace.
Measured at the close of this pass: after `dev.sh test` + `check-candid.sh` the file on disk was
`9f511048…`, and a `cargo build -p history_canister` immediately afterwards rewrote it to
`ea701f77…`. No gate is wrong because of it today — the exported Candid interface is identical
either way and every harness rebuilds before it reads — but *"the artifact's identity depends on
which command ran last"* is not a property a product whose pitch is "rebuild it and compare the
hash" is allowed to have.

---

## 6. Every gate, run

Every one of these was run on this tree, in this pass, on this machine. Exit codes are the
script's own, captured with `> /dev/null 2>&1; echo $?`.

| gate | command | result |
|---|---|---|
| the primary gate | `./scripts/dev.sh test` | **RED, on [E-89](DEFECTS.md#e-89) alone.** 8 steps; every step green except the `fuzz-default` invocation inside step 4. The one failing line the runner prints is `FAILED: money-safety fast subset`, and it is the `fuzz-default` row inside it |
| the fuzzer at its own defaults | inside step 4, `env -u MONEY_FUZZ_*` | **RED: 1 blocking finding, `M9_FUND_REACHABILITY`, seed `0xc1ea2dec0002`, 5 ops, 20,002 e8s.** 308.3 s in the authoritative run (316.4 s in the first). Seeds `0001` and `0003` clean. This is [E-89](DEFECTS.md#e-89) |
| the money invariants | step 4, `cargo test --test invariants` | 64 passed, 0 failed (63 before; the new one is FINDING 44's window gate) |
| the archive canister's own rules | step 1, `cargo test --workspace` | `history_canister` 23 passed, 0 failed (18 before; five are [E-91](DEFECTS.md#e-91)'s) |
| the settlement oracle | step 5 | `settlement` 10 passed, 0 failed (88.0 s); `disagreements` 8 passed, 0 failed (11.4 s) |
| the controller seat | step 4, `controller_custody` | 10 passed, 0 failed |
| the cycles runway | step 4, `cycles_runway` | 2 passed, 0 failed (752.8 s) |
| the screenshot sweep | `./scripts/dev.sh shots` | 24 shots, 17 verified, **7 red — the same 7 as the previous sweep**, all acknowledged |
| the recorded verdict, as a gate | `./scripts/dev.sh shots-verdict` | exit 0: every red acknowledged, every acknowledgement still red |
| the screenshot harness's own gates | `./scripts/dev.sh shots-selftest` | exit 0, 9 self-tests |
| the archive analyser | `./scripts/dev.sh archive` | 39 passed, 0 failed |
| repo hygiene / the protected notices | `./scripts/dev.sh hygiene` | exit 0 |
| the mainnet guard | `./scripts/dev.sh selftest` | exit 0 |
| the register | `./scripts/register-stats.sh --check` | exit 0 |
| candid vs the built wasm | `./scripts/check-candid.sh` | exit 0, 0 structural differences on all four |
| declared vs deployed config | `./scripts/check-deployed-config.sh --network local` | exit 0, 4 of 4 tables match `icp.yaml` |
| pinned tool versions | `./scripts/check-declarations.sh` | exit 0, 4 versions agree across every file |
| every test target is run by something | `./scripts/check-suite-wiring.sh` | exit 0 (and `--selftest`: 9 cases, it can go red) |
| the read-only claim on the monitor | `./scripts/assert-read-only.sh` | exit 0, and now refuses the four shapes it used to pass ([E-94](DEFECTS.md#e-94)) |
| the inverted markers | `./scripts/dev.sh known-defects` | exit 0, 1 of 1 engine defect still present |
| **the cycles runway on the local replica** | `./scripts/dev.sh cycles` | **RED (exit 1): `table_2` and `table_3` at 11 days.** A local fixture running low after the sweep, not a product defect — and the measurement that corroborates [E-92](DEFECTS.md#e-92) |

### The notices, on rendered pixels, every view, both viewports

The requirement is that the unaudited-alpha disclaimer, the 18+ notice, the jurisdiction warning and
the no-rake property are on screen and that nothing covers them. Measured on the real replica by the
sweep above, per shot:

* **24 of 24 shots: `5/5 protected notices on screen`.**
* **24 of 24 shots: `0 occluded`**, over 12 to 574 pixel-tested overlap pairs each.
* **Behind modals:** `deposit`, `handhistory`, `handreplay` and `shuffleproof` all report the notices
  measured *and* `N behind an open dialog (not gated)` for the money figures the dialog covers — the
  notices themselves are still asserted, the dialog only exempts what is behind it.
* **Under a toast:** the `toast-notices` scene raises a REAL error toast through the app's own error
  path and reports `5/5 protected notices still on screen under it`, at both viewports.
* `./scripts/dev.sh hygiene` separately proves no notice LINE has been removed or altered since
  `ceacc37`.

### The archive, on the live local canister, after the [E-91](DEFECTS.md#e-91) upgrade

```text
records_held                          3_338
distinct_names                        3_338
name_collisions                           0
records_without_a_usable_commitment       0
index_disagrees_with_records          false
records_with_a_nonzero_rake               0
rake_recorded_total                       0
```

The count was 3,218 when this wave began and 3,331 midway through the sweep. **It is still growing
after the caller binding landed**, which is the end-to-end proof that an honest table can still
record its own hands: `record_hand_to_history` sets `table_id = self_principal()`, so the binding is
invisible to every real writer and closed to every other one.
