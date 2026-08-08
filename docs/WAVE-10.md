# Wave 10: the disclosure was false in the operator's favour, and the eighth cross-agent defect

> **A NOTE ON THE NUMBER, carried forward from [WAVE-09.md](WAVE-09.md).** This is the tenth wave
> **document**. The register's `wave` column runs ahead of it: the rows this document lands say
> `12`. The two counters are not the same counter. Where a row says wave 12, it means the wave
> this document covers.

---

## THE ONE SENTENCE FOR THE LEAD

**Safe to merge, and the frontend must be redeployed with it, because the deposit screen was
telling depositors something false in the operator's favour and now tells them the truth.** No
backend source changed, so the six mainnet module hashes are untouched by this wave and no
backend redeploy is needed or implied. **FINDING 23 is not closed and is not mitigated on
mainnet: it is DISCLOSED, and this wave found that the disclosure itself understated the danger.**

> ### ONE GATE IS RED, THE RED IS TRUE, AND WAVE 12 DID NOT CAUSE IT
>
> `./scripts/dev.sh fuzz-default` — and therefore `./scripts/dev.sh test`, which contains it —
> **fails**, deterministically, on two independent runs. It is not flaky and it is not a stale
> assertion: it is [FINDING 31](SECURITY-FINDINGS.md#finding-31), the register's top open
> `critical`, **reached by an automated gate for the first time**, in a five-operation minimal
> reproducer. Two players each send 10,001 e8s to the deposit address the canister publishes for
> them, and 20,002 e8s ends up unreachable with the transcript printing `sweepable=true` beside
> each. Filed as [E-89](DEFECTS.md#e-89).
>
> **Pre-existing on `main`, stated rather than assumed.** Wave 12 changed no canister source and
> no fuzzer source; the only change under `tests/money_safety/src/` is additive functions in
> `wasms.rs` that the fuzz binary never calls, and the run's report records the same
> `table_wasm_sha256` the gate builds.
>
> **It was left red on purpose.** The two ways to make it green are to fix FINDING 31 (a
> `src/table_canister` change that moves all six deployed module hashes and requires a mainnet
> redeploy — the lead's decision) or to add a tolerance entry to
> `tests/money_safety/src/documented.rs`, whose own header says *"Adding an entry is admitting a
> defect is shipping"* and whose register is deliberately empty. **Tolerating a live fund-loss
> defect so that a gate goes green is [D-14](DEFECTS.md#d-14) in a different costume**, and this
> wave is the wave that found D-14.

### The five sentences

1. **Safe to merge**, and the frontend has to be redeployed with it: the deposit screen was telling
   depositors the operator could destroy their money but not take it, and that was false.
2. The single most important thing still open is **[FINDING 31](SECURITY-FINDINGS.md#finding-31)**,
   which is now red in an automated gate for the first time
   ([E-89](DEFECTS.md#e-89), `dev.sh fuzz-default`, five operations, 20,002 e8s unreachable) and is
   the reason `./scripts/dev.sh test` does not pass on `main` today.
3. **FINDING 23 is DISCLOSED, not closed and not mitigated** — the guardian is built, gated and not
   deployed, mainnet still has one key on every controller seat, and this wave established that
   that key can pay the whole ledger balance to itself rather than merely destroy it.
4. No backend source changed, the six module hashes reproduce byte for byte before and after this
   pass, and `icp.yaml`'s only edit is a comment, so **nothing here obliges a backend redeploy**.
5. Five gates that nothing ran now have targets, including one whose manifest named its five test
   binaries explicitly *so that nothing would be auto-discovered* and which no human had ever
   typed.

---

## 1. THE THING THAT MATTERS MOST: the worst case is theft, not destruction

Wave 12's custody work put a disclosure on the deposit screen and in the README for the first
time in nine waves. That was the right thing to do and it is the largest single improvement in
this tree. **One clause of it was false**, and false in the direction that flatters the operator:

> "Your ICP would stay at the canister's ledger address, unreachable by anybody — including the
> operator, who [could not pay] it to themselves. [Destruction, not] theft, and permanent either
> way."

and, in the README's decision table, to *"Can the operator take the ICP out of the canister to
their own wallet?"*: the single word **No**.

The argument behind it was *"there is no method that pays a controller"*. **That is a true
statement about ClearDeck's code and an irrelevant one about a controller, because a controller
replaces the code.** `install_code` installs whatever module it is handed, and the canister's
ledger account is spendable by whatever is then running in it. The money never had to pass
through a ClearDeck method.

Executed on the local replica, with the same verb and the same single command as the wipe this
finding is already about:

```text
--- FINDING 23c: the operator does not have to destroy it. They can TAKE it ---
BEFORE   ledger_at_canister=4000000000  escrow=2600000000  chips=1400000000
         operator_wallet=1000000000000
thief module sha256=4c3be308… (508924 bytes, NOT ClearDeck)
reinstall_canister(controller, thief_module) -> Ok
steal(3999990000) -> icrc1_transfer accepted
AFTER    ledger_at_canister=0  operator_wallet=1003999990000
MOVED    3999990000 e8s = 39.99990000 ICP of player deposits into a wallet the operator owns
```

`tests/thief_canister` is that module: init with a ledger and a payee, one `steal`, one
`icrc1_transfer`. It is a detached crate under `tests/`, absent from `icp.yaml`, so it can reach
no deploy path and cannot move a root lockfile.

### Why the existing gate was green over it, and what holds the correction

`cmd_hygiene`'s custody check is a **presence** check: seven phrases that must appear. Every one
of them was present. The false sentence was *additional* reassurance sitting beside true ones,
which no presence check can see.

So the retraction has an **inverted** gate of its own — `RETRACTED_CUSTODY_CLAIMS` in
`scripts/dev.sh`, the same shape as the `fully decentralized` check — asserting that the two
retracted sentences are **absent** from `README.md` and the frontend. And the claim that replaced
them is not an argument either: `finding23c_the_operator_can_pay_the_ledger_balance_to_themselves`
executes the theft in `./scripts/dev.sh custody` and `./scripts/dev.sh test`.

**A disclosure that is wrong in the operator's favour is worse than no disclosure**, because a
player who reads it deposits with a false floor under them. Filed [D-14](DEFECTS.md#d-14), graded
`fund-theft`.

### It changes what the guardian is worth, too

The guardian removes seven destructive verbs and keeps a state-preserving **upgrade** path behind
a public queue and 72 hours. New code can `icrc1_transfer` a ledger balance as easily as it can
zero one in `post_upgrade`. **What the guardian buys against theft is 72 hours of public notice,
not impossibility.** That is a real and large difference, and it is not the same claim. Both
README and [FINDING 23](SECURITY-FINDINGS.md#finding-23) now say so.

---

## 2. THE EIGHTH CROSS-AGENT DEFECT, hunted rather than stumbled on

**[E-86](DEFECTS.md#e-86). Correct totals, wrong recipients, every invariant silent — for the
eighth wave running, and this one is the purest instrument-side instance yet.**

The screenshot sweep's deposit check read the four money figures out of `.minimum-notice` **in
DOM order** and assigned them positionally to (minimum, fee, minimum + 2 × fee). At `134550e` the
copy changed: the modal now names the **address** minimum, the **connected-wallet** minimum, the
ledger fee, and the wallet balance required. Four figures where the check expected three.

| screen figure | what it is | what it was compared with | verdict |
|---|---|---|---|
| 0.0003 | the address minimum (`MIN_WITHDRAWAL + FEE`) | `min_deposit` = 20 000 e8s | **DISAGREES, 1.500x** |
| 0.0002 | the connected-wallet minimum | `icrc1_fee()` = 10 000 e8s | **DISAGREES, 2.000x** |
| 0.0001 | the ledger fee | minimum + 2 × fee = 40 000 e8s | **DISAGREES** |
| 0.0004 | the wallet requirement | *nothing* | census: unaccounted for |

Both deposit shots were red with *"DEPOSIT CHAIN DISAGREEMENT … screen is 1.500x the chain"*.
**The screen was right. Every figure on it was correct and correctly derived.** The comparison had
been silently re-pointed at the wrong quantity by a copy edit, and the one genuinely new figure —
the largest of the four, and the one a player with a nearly-empty wallet acts on — was asserted by
nothing at all.

The shape is why it is worth naming: the instrument reported a **product** defect with real
numbers attached, and acting on the report would have meant changing a correct screen. The four
claims are now anchored on their **labels**, and a label that stops matching is a structural
failure that says *"the figure it names is now checked by nothing"*.

---

## 3. Four gates that did not exist, and one that had no gate

Every one of these is [H-45](DEFECTS.md#h-45)'s shape — a gate nothing runs — recurring in the
wave that closed H-45.

| | what was unheld | now |
|---|---|---|
| [H-50](DEFECTS.md#h-50) | `tools/archive/**`, 39 offline cases, **no target at all** — under a heading whose first sentence is *"A gate nothing runs is worse than no gate"* | `dev.sh archive`, `make archive`, and step 7 of 8 in `dev.sh test` |
| [H-51](DEFECTS.md#h-51) | `lib/table-in-frame.mjs`, the wave's best new instrument: **one runner, `run.mjs`, which needs a live replica** | `tools/shots/test-table-in-frame.mjs`, 17 cases, in `dev.sh shots-selftest` |
| [H-52](DEFECTS.md#h-52) | the guardian's committed `.did`, which **three** custody gates parse and **nothing compared to the module** | `check-candid.sh --guardian-only`, run by `cmd_custody` **before** the tests |
| [H-53](DEFECTS.md#h-53) | `tests/no_peeking/**`, 5 targets, 36 tests, named explicitly in its own manifest *so that nothing would be auto-discovered* — and then typed by no target | `dev.sh no-peeking`, `make no-peeking`, step 8 of 8 in `dev.sh test` |
| [H-47](DEFECTS.md#h-47) | **two** lists of the screenshot self-tests, already drifted, about to become three | one runner, `tools/shots/selftest.mjs`, that discovers AND requires; both callers use it |

### H-51 is the interesting one, because the instrument could lose its own conviction

`.seat` is `width: 0; height: 0` — a point on the ring — so a box measured on it is inside the
frame by construction and **cannot fail**. `table-in-frame.mjs` measures the union of each seat's
painted descendants for exactly that reason, and its author found the naive version reporting
*"0 seat pods, all in frame"* while pods were visibly on the edge.

Restore the naive measurement today and the verdict comes back **"9/9 seat pods, all in frame"**
over zero-size boxes, because every check below it iterates a set that is now empty. Three blind
spots are now asserted and all three were **verified to convict** by reverting the hardening
(3 of 17 cases go red):

1. the seats block must carry `seatElements`/`renderingNothing`, i.e. must be the pod union;
2. seats on the ring with **no** measurable pod is a failure, not an empty loop;
3. the board's `if (m.board.rendered)` skip is cross-checked against painted hole cards, so
   "no board" is a **checked** skip on `table-empty` and behind the deposit dialog, and a
   **failure** on a dealt hand. The module's header claimed the latter in two places while doing
   the former on 4 of 24 shots.

---

## 4. The red ledger was excusing failures it had never seen

**[E-87](DEFECTS.md#e-87).** `acknowledged-reds.json` matched an entry to a red shot by
`(scene, viewport)`. Once a shot was red for **any** reason, every **later** failure on it joined
the same array and was excused by an entry that had never been about it.

`table-in-frame.mjs`'s first real conviction — [E-85](DEFECTS.md#e-85), a Leave-table control
6–7 px outside a 1440 px frame that a player cannot reach or scroll to — landed on two shots
already acknowledged under [E-76](DEFECTS.md#e-76) for a felt shortfall of 0.2 points.
`dev.sh shots-verdict` stayed green.

**Rule 3:** an entry must now list `covers`, the substrings of the recorded problems it accounts
for. Every recorded problem must match one, and every `covers` string must still match a problem,
so a cover that stops applying fails exactly the way a stale acknowledgement does.

**[E-88](DEFECTS.md#e-88)** is the same hole one layer down: `verdicts.mjs` keeps clauses matching
a six-word failure vocabulary and **discards the rest whenever one matches**. Neither geometry
check uses any of those six words; both reached the tracked file only because they happened to be
the only clause on their shot. The first shot to fail an occlusion check *and* the felt floor
would have recorded the occlusion and deleted the felt red before any gate could see it.

---

## 5. Two documents were claiming more than their code does

Both corrections are in the direction of *less* capability. Neither touches a measured number.

**[DESIGN-BAR §11.2](DESIGN-BAR.md).** The phone pass's felt percentages are measured twice with
two independent instruments and are right. The published *explanation* of where they came from is
wrong by about a factor of four: the budget claims 19 px of empty chrome was recovered, and the
`.table-area { padding-top: 0 }` it credits with 8 px of that is **dead CSS** — a pre-existing,
more-specific `main[data-view='table'] .table-area { padding-top: 4px }` sits later in the same
media block and wins. The real recovery is 5 px, and **the builder's own after-figures prove it**:
135.6 px before, 130.1 px after. Also corrected: "chrome above the felt is identical (130 px) on
all of them" (the tracked manifest says 130.0 / 130.1 / 131.9 / 144.6 / 145.3 / 160.0), and
"desktop is untouched" (true at 1440×900; the `+page.svelte` changes sit in a media query with a
non-portrait `(max-height: 560px)` arm).

**[ARCHIVE.md §6](ARCHIVE.md) — [T-42](DEFECTS.md#t-42), and this one is a product claim about
accusing people.** Signal 2 asks *"does A lose to B faster than A loses to everybody else"*. That
is **also** the definition of *"B is better than A's other opponents"*, so the null is false under
ordinary skill variation and the error rate rises toward 1 with sample size instead of holding at
alpha. On **800 real, uncoordinated `table_3` hands** the shipped detector returned two `REVIEW`
rows at q = 1.50e-4, both naming the best player — the same verdict, count and q as a scripted
chip dump. The shipped false-positive gate cannot see it because `synthetic.mjs` has no
per-player skill parameter at all, and its assertion tolerates `0.12` while claiming alpha `0.05`.
The power table's `sd = 3.70` came from a slice no documented command produces; the real figure on
the documented bundle is `12.25`, which makes every "hands needed" number **about eleven times
larger**.

The limitation is now at the top of §6, in §9, in the gates table, and **in the tool's own output
above every Signal 2 table**, because the reader who never opens the document is the one who needs
it. Signals 1 and 3 are unaffected and remain the thing no other poker room can do. **No Signal 2
row should be shown to a player or used to restrict an account** until the null is conditioned on
opponent strength.

---

## 6. What the lead has to do differently, because a guardian canister landed

`src/guardian_canister/` is **built, gated and not deployed**, and the deploy story changes only
if the lead chooses to deploy it. Until then **nothing about mainnet changes and no step is
different**.

### The safe part, verified this wave

* **No backend source changed.** `git status` over `src/table_canister`, `src/lobby_canister`,
  `src/history_canister`, `src/poker_core`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`,
  `recipes/` and `Dockerfile` is **empty**. The six deployed module hashes cannot have moved.
* **`icp.yaml`'s only change is a comment block** explaining why the guardian is deliberately
  absent from it.
* **The build is still byte-reproducible.** Docker is unavailable on this machine, so
  `verify-build.sh --two-paths` could not run; the host-toolchain equivalent was run instead —
  the same source built at two different absolute path lengths — and all six modules are
  identical:

```text
  ok       btc_table_1    4511ab187cff8b90202c4ba2c4e22aab4745636235aa4f3b2e9c69e79c42c608
  ok       history        2c90ac0988d0744d65148440bba156683f8d9722a4338b03e8486243a5e57582
  ok       lobby          e68452ac9206c4e44555ef0bca12165e9a59e2fd8d1aa782de45dd31fcf134b6
  ok       table_1        4511ab187cff8b90202c4ba2c4e22aab4745636235aa4f3b2e9c69e79c42c608
  ok       table_2        4511ab187cff8b90202c4ba2c4e22aab4745636235aa4f3b2e9c69e79c42c608
  ok       table_3        4511ab187cff8b90202c4ba2c4e22aab4745636235aa4f3b2e9c69e79c42c608
```

> **Run twice: once before any of this wave's reconciliation edits and once after, and all six
> hashes are the same six values both times.** That is the strongest available statement that no
> edit in this pass touched a build input.
>
> **A verifier with Docker must still run `./scripts/verify-build.sh --two-paths`.** The host
> check above proves path-independence on this toolchain; the container check is the one a
> stranger reproduces against mainnet, and it is the one that has ever been believed.

### If the guardian is deployed, four things are different, and one is irreversible

1. **It is a two-step deploy, outside `icp build`.** The guardian is not a root workspace member
   (so it cannot rewrite the root lockfile and therefore cannot move a deployed module hash), so
   `icp build` cannot see it: build with `./scripts/build-guardian.sh`, then `icp canister create`
   and `icp canister install --mode install` the artifact.
2. **Adding it to `icp.yaml` is the LAST step, not the first.** A seventh backend canister in the
   manifest makes `check-deployed-config.sh` and `check-deployed.sh` report a fleet that does not
   match, on a live system, before anything is deployed — a false red on the two gates whose whole
   job is to be believed.
3. **The handover is one-way and unrepeatable.** `update_settings(controllers = [guardian])` on a
   table, then `update_settings(controllers = [guardian])` on the guardian **itself**. A guardian
   the operator controls is worth nothing — that is measured, not assumed — and a self-controlled
   guardian cannot be rescued by any key. **Set the freezing threshold and top up cycles FIRST.**
4. **The operator loses the audit surface with the destructive one.** After handover the
   operator's key stops reaching every `require_controller()` method on the table, including
   `admin_get_all_balances`, `admin_get_deposit_custody` and `admin_audit_deposit_custody`. That
   is deliberate — a passthrough that can carry an audit call can carry any call — and it is a
   real operational cost that has to be accepted before the handover, not discovered after it.

**Can the existing mainnet canisters adopt it without risk?** *Technically yes and operationally
not without a rehearsal.* `update_settings` on a live canister does not touch state, code or
balances, and controllership on the IC is not transitive — 9 of 9 management verbs are rejected
for the operator once the guardian holds the seat, measured. The risk is not in the mechanism, it
is that the arrangement is **unrecoverable by design**: a guardian with a bug, or a guardian on a
canister that later freezes, cannot be reached by anybody. Rehearse the whole sequence on the
local replica, on all six canisters, with the freezing thresholds set the way mainnet's are,
before touching a canister that holds ~12.3 ICP.

---

## 7. Every gate, run

| gate | result |
|---|---|
| `cargo test --workspace` | green |
| `tools/differential` fast subset | green |
| money-safety, all **17** suites, run individually in the gate's own configuration | **17/17 green**, 190 tests. `fund_reachability` is **6/6** ([H-48](DEFECTS.md#h-48) says it is red; it is not, and the entry now says so) |
| `dev.sh custody` (10 tests, incl. the new `finding23c_`) | green |
| `dev.sh settlement` (oracle + pinned reproducers) | green |
| **`dev.sh fuzz-default`** | **RED — [E-89](DEFECTS.md#e-89) / [FINDING 31](SECURITY-FINDINGS.md#finding-31). Reproduced twice, identically. See the box at the top of this document.** |
| `dev.sh known-defects` | green (1 of 1 engine defect still present, as designed) |
| `dev.sh hygiene` (incl. `shots-verdict`) | green, and the new retraction check negative-tested |
| `dev.sh selftest` (the mainnet guard) | green, 7 mainnet ids in the denylist |
| `dev.sh shots-selftest` | 7 files discovered, 7 run, all green |
| `dev.sh archive` | 39/39 green |
| `dev.sh no-peeking` | 36/36 green |
| `register-stats.sh --check` | consistent: vocabulary closed, every FIXED names a gate, every anchor resolves |
| `check-candid.sh --declarations` (+ the new guardian block) | 4 interfaces, 0 structural differences, 82 known declaration items |
| `check-deployed-config.sh --network local` | 4 of 4 table configs match `icp.yaml` |
| `dev.sh shots` | 24 shots, 15 verified, **9 red and all 9 acknowledged with `covers`** |
| **protected notices, on rendered pixels** | **5 of 5 on all 24 shots, both viewports, behind every modal, and under a real error toast** |
| two-path build reproducibility (host toolchain) | **6 of 6 identical** |

**`./scripts/dev.sh test` as a whole is therefore RED, on step 4, for [E-89](DEFECTS.md#e-89)
alone.** Every other step of it passes; the 17 money-safety suites were re-run one at a time to
establish that, because `tail`-ing the gate's output showed only the failing group's name.

### A note on how that was established, because it is [H-54](DEFECTS.md#h-54)

The first full `dev.sh test` appeared to hang for twenty minutes *after* it had finished. It had
not: `with_timeout`'s watchdog forks a `sleep`, `kill "$wd"` kills only the subshell, and the
orphaned `sleep` kept the gate's **stdout and stderr** open at PPID 1. Measured: the gate's process
was gone and `sleep 900` held the pipe for another fourteen minutes.

**That is [H-42](DEFECTS.md#h-42)'s mechanism, and H-42 blames the opposite step** — it diagnoses
the money-safety block for having no time bound, when the cause is the two settlement steps that DO
have one. `900 + 300` is H-42's twenty minutes and all of it is post-hoc. Fixed in
[H-54](DEFECTS.md#h-54); reproduced both ways in four lines.

### No test suite is auto-discovered and named by nothing

Checked by enumerating every `tests/*.rs` in every crate in the tree and every `--test <name>` in
`scripts/`, `Makefile` and `.github/`:

* `tests/money_safety` — 17 targets, all 17 named.
* `tests/settlement` — 3 targets, all 3 named.
* `tools/differential` — 1 target, named.
* **`tests/no_peeking` — 5 targets, 36 tests, named by NOTHING.** Fixed this wave
  ([H-53](DEFECTS.md#h-53)). It is the only one, and it was found by doing the enumeration rather
  than by trusting the last wave's answer.
* `tools/shots` — 7 self-tests, previously two divergent lists, now one discovering runner
  ([H-47](DEFECTS.md#h-47)).
* `tools/archive` — 39 cases, previously no runner at all ([H-50](DEFECTS.md#h-50)).
