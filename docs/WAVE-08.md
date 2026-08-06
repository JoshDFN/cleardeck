# Wave 8: the ledger/escrow boundary, and the honest answer

> ## THE ANSWER IS STILL NO.
>
> A **fourth** independent auditor was run after this wave's fixes, on the running system, given
> the repo as a stranger finds it. Its verdict, verbatim:
>
> > **"NO — I would not tell a friend their money is safe here."**
>
> The third auditor's, for comparison:
>
> > **"No — I would not tell a friend their money is safe here."**
>
> The second auditor's:
>
> > **"NO — I would not tell a friend their money is safe here."**
>
> The first auditor's:
>
> > **"No. I would not tell a friend their money is safe here, and the reason is not the poker."**

**The set changed, and this time it changed less than it looks.** Four of auditor three's five
blockers are closed. The fifth ([FINDING 22](SECURITY-FINDINGS.md#finding-22), a controller reads
every hole card mid-hand) was not worked by anybody. And **six of auditor four's ten findings were
already in this repository's own register before the wave started**, including the only one ever
graded `fund-theft`. That is a different failure from the last three waves and it is the one the
lead needs to hear: **we are no longer mostly failing to _find_ things.**

I did not take the closures on trust. I re-drove [FINDING 33](SECURITY-FINDINGS.md#finding-33)
(reported FIXED by one agent, filed CRITICAL/OPEN by another in the same wave; the code agreed
with the second), and I drove [FINDING 35](SECURITY-FINDINGS.md#finding-35) fresh, which shows
FINDING 28 is closed at one of the two addresses the canister publishes and open at the other.

---

## 1. Did the verdict change, and is it the same set?

### Auditor three's blockers: four of five closed

| auditor three's blocker | state now | how I know |
|---|---|---|
| deposit floor 20,000 e8s / withdrawal floor 100,000 e8s, so money at the advertised minimum can never leave ([FINDING 27](SECURITY-FINDINGS.md#finding-27)) | **closed** | one number per currency, relation asserted at compile time, whole-balance sweep always available. `deposit_floor` is in `dev.sh test` |
| money at the canister's own published deposit subaccount reported as zero by every surface ([FINDING 28](SECURITY-FINDINGS.md#finding-28)) | **closed at that address** | `DEPOSIT_CUSTODY`, `get_deposit_custody()`, `refresh_deposit_custody()`, `unswept_deposit` in `get_custody_status`, folded into `total_liability()`. **Not closed at the OTHER published address: [FINDING 35](SECURITY-FINDINGS.md#finding-35)** |
| all three money doors move real money before writing any record of intent ([FINDING 29](SECURITY-FINDINGS.md#finding-29)) | **closed** | the ledger-intent journal, made replayable by the ledger's own ICRC-1/ICRC-2 dedup; driven by discarding a real continuation on a real ledger; M14 LEDGER/BOOKS COHERENCE gates it |
| the permanent record is built from the seats at settlement ([FINDING 30](SECURITY-FINDINGS.md#finding-30)) | **closed for the CONTENT, open for the IDENTITY** | `hand_participants` unions `hand_stakes` with `DEALT_IN`; `dealt_in` is on the record so `P` is read, not guessed. But `(table_id, hand_number)` still names eleven different hands. See [E-71](DEFECTS.md#e-71) |
| a controller reads every hole card mid-hand ([FINDING 22](SECURITY-FINDINGS.md#finding-22)) | **OPEN, untouched** | nobody worked it this wave |

### Auditor four's blockers, classified honestly

| # | severity | blocker | was it already filed? |
|---|---|---|---|
| 1 | **fund-theft** | one controller key holds unilateral, unrestricted custody; `--mode reinstall` wiped every balance on a funded table | **YES. [FINDING 23](SECURITY-FINDINGS.md#finding-23), filed wave 7, OPEN, never worked** |
| 2 | critical | `get_deposit_address()` returns the canister's MAIN account; money sent there is unreachable and invisible on every surface | **YES. [FINDING 34](SECURITY-FINDINGS.md#finding-34), filed by this wave's own critic.** Deepened here into [FINDING 35](SECURITY-FINDINGS.md#finding-35) |
| 3 | high | the deposit address is served over an **uncertified query**; a dishonest replica substitutes it and the canister hands the money to the attacker | **NO. New in kind: the trust model of the transport, not of the code** |
| 4 | high | no `derivationOrigin`, so the same human at `icp0.io` and `ic0.app` is two players with two balances and two deposit addresses | **NO. New in kind: identity, in the frontend** |
| 5 | high | the archive's admin can write arbitrary fiction into "permanent hand records", and nothing deletes | **NO. New in kind: the archive's writer authority** |
| 6 | medium | `HowItWorks.svelte` still makes the fairness claim `docs/SHUFFLE-SPEC.md` §0 retracted | **HALF. The doc was corrected in wave 7; the product was not. Documentation drift, known class, new instance** |
| 7 | medium | a ckBTC table's `get_deposit_address()` returns an ICP account identifier | **YES, in substance: the same census hole as #2, on the other currency** |
| 8 | low | an escrow balance of exactly the transfer fee can never be withdrawn | **YES. [FINDING 31](SECURITY-FINDINGS.md#finding-31), filed by this wave's own critic, OPEN** |
| 9 | low | `local-up` prints a green tick on a lobby it failed to populate | **YES, and it is worse than filed: [E-74](DEFECTS.md#e-74)** |
| 10 | low | `max_players` can be set below the number of seated players | **NO. Cosmetic** |

**Six of ten were already written down. Three are genuinely new in kind, and all three are
outside the canister's arithmetic: the transport (#3), the frontend's identity model (#4), and
who is allowed to write history (#5).**

---

## 2. THE FIFTH CROSS-AGENT DEFECT: hunted, not stumbled on

Five agents edited `src/table_canister/src/lib.rs`. Four consecutive waves have produced a defect
visible only from above all of them. I looked for the fifth first, and I looked for it by asking
the one question that is above every agent rather than inside any of them:

> **Which of the six accounts in THE ACCOUNT CENSUS does the CANISTER have an instrument for?**

The answer is four of six, and the two it does not have are the two that hold the money.

### The state table

| account | money arrives without a message? | canister-side observation record | in `total_liability()` | in "UNKNOWN IS NOT ZERO" | operator can audit it |
|---|---|---|---|---|---|
| (1) ICP main | **yes** (any raw transfer) | **NONE** | **no** | **no** | **no** |
| (2) ICP deposit subaccounts | yes | `DEPOSIT_CUSTODY` | yes | yes | yes |
| (3) ckBTC main | **yes** | **NONE** | **no** | **no** | **no** |
| (4) ckBTC deposit subaccounts | yes | `DEPOSIT_CUSTODY` | yes | yes | yes |
| (5) native BTC via minter | n/a, the player owns it | out of scope, stated | n/a | n/a | n/a |
| (6) cycles | n/a | `get_cycle_status()` | n/a | n/a | n/a |

The wave's organising insight was: *money arrives with no message to the canister, and an IC query
cannot call the ledger, so the canister must ASK from an update and WRITE THE ANSWER DOWN.* That
insight was implemented for rows (2) and (4) and for nothing else. `refuse_currency_change_while_funded`
is a **synchronous** function. It can never read a ledger, only what somebody wrote down, and
nobody writes down the main account.

The census's own rule is *"an instrument that measures fewer than all of these accounts is not
measuring this canister's custody."* Its own **MEASURED BY** column for row (1) names
`total_liability()`, which measures the *claims* on that account and never its *balance*. **The
only thing in this repository that reads the main account is the money-safety harness, and the
harness does not ship.** That is [FINDING 35](SECURITY-FINDINGS.md#finding-35) / [E-70](DEFECTS.md#e-70).

### Driven, on the fixed tree, after FINDING 33 was closed

`tests/money_safety/tests/coherence_w8.rs::finding_35_reproduction_money_at_get_deposit_address`

```
get_deposit_address() -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
withdraw(all 300000000) -> Ok(4)
admin_audit_deposit_custody([alice]) -> Ok((1, 0, 0))   (read, observed_total, still_unaudited)
LEDGER main=500000000 subaccounts=0
  get_balance()             -> 0
  get_custody_status total  -> 0   advice=""
  admin_get_deposit_custody -> 0
  admin_update_config(BTC)  -> ACCEPTED currency now BTC
  notify_deposit(3) while BTC -> Err("Failed to query ckBTC ledger: ... mxzaz-hqaaa-aaaar-qaada-cai")
  admin_update_config(ICP) back -> ACCEPTED currency now ICP
  notify_deposit(3) after flip back -> Ok(500000000)
```

`admin_audit_deposit_custody` reports **"1 account audited, 0 held, 0 unaudited" on a canister
holding 5 ICP of alice's.** The operator's all-clear is structurally incapable of seeing this
money. The auditor measured the identical reply and so did I, on the build that closed FINDING 28.

And the currency guard is blind at the same address: an audited, drained table holding 5 ICP in
its main account reads a liability of **zero**, `admin_update_config(BTC)` is accepted, and
`notify_deposit`, the door [FINDING 34](SECURITY-FINDINGS.md#finding-34) correctly identifies as
the *only* remaining recovery, then routes to the ckBTC ledger and fails. **No attacker, no trap,
no fault injection, no unfinished call: one exchange withdrawal and one routine config change.**

**The honest mitigation, measured.** Unlike FINDING 33's, this flip is REVERSIBLE. Flipping back
to ICP is accepted and `notify_deposit` then returns `Ok(500000000)`. It is still high, because
nothing in the system would ever tell an operator to flip back: every surface, player-facing and
admin-facing, affirmatively reports zero.

### And the signature this time is not "wrong recipients"

It is one level up: **a correct instrument pointed at a subset of the accounts, and a census whose
own MEASURED BY column names the test harness rather than the canister.** Two more instances of
the same shape turned up while I was looking, both in the instruments:

* [FINDING 36](SECURITY-FINDINGS.md#finding-36): the harness's `CustodyStatus` mirror has nine of
  the canister's ten fields. The missing one is `unfinished_ledger_ops`, the field carrying
  FINDING 29's money. Candid record subtyping drops it silently. The mirror's own comment, written
  by a different agent in the same wave, says *"Mirrored in FULL on purpose. A partial mirror here
  would let the field be deleted from the canister without a single test noticing."* Found by
  writing the field name in a probe and having `rustc` refuse it.
* [FINDING 37](SECURITY-FINDINGS.md#finding-37): `total_liability()` has **one caller, no query,
  no surface and, until this pass, no test**. The only way anybody has ever sampled the project's
  last custody guard is to attempt the destructive operation it guards. That is the structural
  reason FINDING 21, FINDING 33 and FINDING 35 are three instances of one defect.

---

## 3. FINDING 33: reproduced a third time, then FIXED and GATED

Two of this wave's own critics filed it independently and neither left a gate. I re-drove it on
the shipping tree before touching anything:

```
BEFORE  admin_update_config(BTC) -> ACCEPTED currency now BTC
        admin_update_config(ICP) back -> REFUSED "... still owes players 9990 sats"
```

The flip through was accepted on a canister holding 1.9999 ICP of bob's, and **the flip back was
refused**, so the money ended permanently locked in a currency the canister holds none of. At the
same instant `get_custody_status(bob).total` correctly read `199990000`.

The cause is one word. `journalled_incoming_total()`'s comment says *"`Pull` AND `Sweep`"*; its
filter said `== LedgerIntentKind::Pull`. An open sweep was subtracted by `observed_deposit_total()`
and added by neither term.

**Fixed** (`src/table_canister/src/lib.rs`): the filter is now `i.kind.credits_on_success()`, the
same predicate `unfinished_ledger_ops_for()` uses for the player-facing surface, so the guard and
the surface read one definition of arriving money.

```
AFTER   admin_update_config(BTC) -> REFUSED "Refusing to change this table's currency from ICP to
                                    BTC while it still owes players 1.9999 ICP. ..."
```

**Gated**: `tests/money_safety/tests/coherence_w8.rs`, named explicitly in `./scripts/dev.sh test`
next to `ledger_boundary`. Reverting the filter turns it red. This is the first gate in the
project that reads what the currency guard reads.

> **Scope note, stated because it matters.** I own the documents this wave. This is a one-word
> change making code match the comment directly above it, on a critical defect three reviewers
> drove and nobody closed, and I verified it against the full `./scripts/dev.sh test` (§5). I also
> added one line to `scripts/dev.sh` so the gate is inside the gate: the `deposit_replay` /
> H-28 lesson. Those are the only two edits outside `docs/` and `tests/money_safety/tests/`.

---

## 4. THE RENDERED NOTICE GATE RAN, for the first time in three waves

`./scripts/dev.sh shots` had been dark since wave 6. It is not dark any more, and the reason it
was dark is [E-74](DEFECTS.md#e-74): `local-up` calls `lobby init_microstakes_tables` with an
identity that is not the lobby's admin, discards the `Err` with `|| warn`, and prints
`✓ lobby lists 0 table record(s)`. An empty lobby kills every scene:

```
✗ FAILED: Lobby has no registered name for table_2      (x20)
12 scene(s) not verified   EXIT=1
```

Running the same call as the identity `lobby get_admin` actually names returns `(variant { Ok })`.
With the lobby populated the sweep completes: **24 shots, 14 verified, 10 red.**

### The notice check, on rendered pixels: GREEN, everywhere

**5/5 protected notices on screen, and `notices survive an error toast`, on all 24 shots**: the
lobby, the empty table, pre-flop, facing a bet, the all-in, the showdown, side pots, **behind the
deposit modal, behind the hand-history modal, behind the hand-replay modal, behind the shuffle-proof
panel**, and on the dedicated `toast-notices` scene where the app's own error path raises a real
toast (every canister call aborted, then a reload, so `loadTables()` rejects and assigns
`error = e.message`). Nothing covers them and nothing was weakened.

`make hygiene` separately confirms 9/9 notice strings intact and none altered since `ceacc37`.

### What the sweep found now that it can see

| red | scene(s) | entry |
|---|---|---|
| a player's stack figure 9.8% painted over by the pod clock | `table-sidepots` mobile | [E-75](DEFECTS.md#e-75) |
| felt below its floor (27.8% vs 28%, 42.8% vs 45%) | `table-preflop` + `table-facing-bet` desktop, `table-allin` mobile | [E-76](DEFECTS.md#e-76) |
| lobby rows quote blinds 5x and 10x below the table's real config | `lobby`, `toast-notices` both viewports | [E-65](DEFECTS.md#e-65), now measured on pixels |
| the archive and the table disagree about "hand 1" | `handhistory` both viewports | [E-71](DEFECTS.md#e-71) |

[E-65](DEFECTS.md#e-65) deserves the emphasis it did not get when it was filed from a manifest:
`init_microstakes_tables` hardcodes `small_blind: 1_000_000` and the name `"… - 0.01/0.02"` for
**all three** tables, while `icp.yaml` gives table_2 `0.05/0.10` and table_3 `0.10/0.20`. **A
player who picks "9-Max - 0.01/0.02" sits down at ten times the advertised stake.** It was
invisible for three waves because the lobby was empty in every sweep.

[E-71](DEFECTS.md#e-71) is the one that matters for "provably fair". `hand_number` restarts at 1
on every table reset and the archive has no uniqueness on `(table_id, hand_number)`:

```
hand_id 37  table_2  hand_number 1  total_pot 2_400_000_000
hand_id 36  table_2  hand_number 1  total_pot 2_400_000_000
hand_id 24  table_2  hand_number 1  total_pot    20_000_000
...  eleven records, three different pots, one name
```

FINDING 30 made the *content* of each record true. It did not give the records *identity*, and
`hand_number` is the name SHUFFLE-SPEC and the UI use.

---

## 5. Gates

Everything below was run on this tree, in this pass, after the FINDING 33 fix.

| gate | result |
|---|---|
| `./scripts/dev.sh test` (pre-fix baseline) | **GREEN**, exit 0, ~11 min. 6/6 stages, 0 failures |
| `./scripts/dev.sh test` (post-fix, with the new `coherence_w8` target inside it) | **GREEN**, exit 0. The one-word change moved no other gate |
| `make settlement` (inside `test`: oracle + pinned reproducers) | **GREEN**. 9 + 8 passed, 0 failed |
| `make fuzz-default` | **GREEN**, exit 0. 3 seeds, 14 hands, 11 upgrades, **0 blocking findings**, worst stranded 0 e8s |
| `make known-defects` | **GREEN**, exit 0. 1 of 1 engine defect (`detect_straight`, E-13) still present; no marker pins a fixed defect |
| `make hygiene` | **GREEN**, exit 0. Notices 9/9 intact, none weakened since `ceacc37`; 10 recorded screenshot reds, **10 acknowledged, 0 stale**. It went RED first: see below |
| `make selftest` | **GREEN**. Guard selftest: every hostile argument shape refused, 7 mainnet ids in the denylist |
| `make shots-selftest` | **GREEN**. 22 no-rake cases, 16 dock-containment cases, occlusion, census, money parser |
| **shots sweep** | **RED, exit 1, and it RAN.** 24 shots, **14 verified, 10 red** across 7 scenes. Every red is a real defect, filed above. **Notices 5/5 on all 24, toast raised.** |
| money suite ledger pin | **VERIFIED.** `a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec`, the real mainnet ICP ledger module, hash-checked before install by `harness_runs_the_real_ledger_at_the_hardcoded_canister_id` |
| seam mutations (H-04) | **see §6** |

### `make hygiene` went RED first, and it was right to

The moment a real sweep replaced the wave-7 one, the verdict gate did the thing it was built to do:

```
! STALE ACKNOWLEDGEMENT: table-allin / desktop (E-64) is GREEN in the last recorded sweep, so
  this entry excuses nothing. Delete it.
```

`artifacts/screens/acknowledged-reds.json` was **rebased from the real sweep rather than carried
forward**, and three entries were deleted because the pixels say they are fixed:

* **[E-61](DEFECTS.md#e-61)**: no `RAKE TAKEN` line on any of the 24 shots.
* **[E-63](DEFECTS.md#e-63)**: the shot that carried it, `table-allin/mobile`, now reports
  *"81 pairs pixel-tested, 0 occluded"*. **Fixed on pixels.**
* **[E-64](DEFECTS.md#e-64)**: *"0 unaccounted for"* in the token census on every table scene,
  both viewports. **Fixed on pixels.**

Three "fixed but not re-measured" entries became measured, in the same sweep that found three new
reds. That is the acknowledgement ledger working in both directions, which is the only way it
stays worth reading.

### What the green fuzz gate still does not say

Unchanged from wave 7 and worth repeating because it is the standing blind spot: on a correct
engine the fuzzer never reaches the state M11's sharp leg needs, so the instrument between "the
wrong claimant was paid" and a green gate is the settlement oracle alone. And this wave added a
second one: **`abandon_stuck_hand` returned `Err` on 42 of 42 rows of its own gate**
([FINDING 25](SECURITY-FINDINGS.md#finding-25)'s critic amendment). The hatch between a wedged
hand and the pot is gated at unit level and by a code read.

## 6. Seam mutations (docs/DEFECTS.md H-04)

Re-run in this pass with `$SCRATCH/seam_mutations_w8.py` against a `cp -Rc` copy, one mutation at
a time, each anchor asserted to match exactly once, `lib.rs` asserted byte-identical afterwards,
gate per mutation = `cargo test --workspace` + `cd tests/money_safety && cargo test --test invariants`.

**A note on the harness itself, because it is the wave's own lesson in miniature.** The first run
of this script reported mutations 1, 2 and 3 SURVIVING. They had not: every gate command was
piped to `tail`, and **a shell pipeline's exit status is the last command's**, so every gate
returned 0 regardless. Three mutations were reported as holes in the instruments by an instrument
that could not fail. The pipes are gone; the results below are from the fixed script. **A
mutation harness whose gate cannot go red reports the code as unprotected and the code as fine
with the same output.** It is the FINDING 37 shape in a python script, found the same day.

**Result: 6 of 7 die. NOT DOWN. Same survivor as waves 2, 3, 4, 6 and 7.**
`lib.rs` sha256 `4b788a228e1d659a752b792cc7acc16c77ae3c472b8528914299ec4d12f7b90c` before and
after, identical.

| mutation (wave-8 site) | verdict | what convicted it |
|---|---|---|
| 1. `refresh_side_pots` made a no-op | **died** | money_safety `invariants` |
| 2. every stake halved in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
| 3. the breakdown written into a throwaway `Vec` | **died** | money_safety `invariants` |
| 4. `&[]` as the contributions in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
| 5. the self-report line dropped | **SURVIVED** | nothing (equivalent mutant, unchanged since wave 2) |
| 6. `try_evaluate_hand` stubbed to `RoyalFlush` in `rank_claims` | **died** | `cargo test --workspace`, money_safety `invariants` |
| 7. every hand dealt from `b"CONSTANT"` | **died** | money_safety `invariants` |
| | **6 of 7** | |

Mutation 5's survival is unchanged and is still an equivalent mutant at canister level: the
warning it deletes only fires when the built side pots exceed `state.pot`, which honest play
cannot reach.

**Scope, stated exactly.** These seven anchors are all in the settlement and shuffle seam
(`refresh_side_pots`, `plan_payouts`, `rank_claims`, `shuffle_deck`). None of them is touched by
the FINDING 33 one-word change, which is in the custody-guard arithmetic, and both gates the
mutation suite uses (`cargo test --workspace` and money_safety `invariants`) ran green on the
fixed tree inside `./scripts/dev.sh test`. The table above was executed against the **pre-fix**
`lib.rs`. A second full run against the **fixed** `lib.rs` was started; at the time of writing it
had reproduced mutations **1 through 5 with identical verdicts and identical killers** (four dead,
5 the same equivalent-mutant survivor) and had not finished 6 and 7. Stated that way rather than
claimed as a completed second run.
**The seam suite says nothing about the custody guard, which is exactly
[FINDING 37](SECURITY-FINDINGS.md#finding-37): there is no mutation anchor anywhere in this
project pointed at `total_liability()`, and `coherence_w8.rs` is the first test of any kind that
is.**

---

## 7. Answers for the lead

### First: what is the shortest remaining path to a stranger saying yes?

Not more auditing, and not more arithmetic. Three things must become true, in this order, and
**the first one makes the other two meaningful**:

**1. The code a stranger can read must be the code that is running.** This is auditor four's own
"what I would fix first", and they are right that it decides the answer. Today the tree has **72
uncommitted paths** (68 when the auditor counted, and the number only goes up while waves keep
landing without a commit) and corresponds to no commit anyone can fetch; the mainnet fleet predates the
reproducible-build pipeline; `verify-build.sh --mainnet` does not print VERIFIED. **Until that is
true, nothing in any of the four audit reports, good or bad, describes the software people's
money is actually in.** Commit, deploy the container build with `--mode upgrade`, make
`--mainnet` print VERIFIED. This is process, not engineering, and it is the cheapest item on the
list.

**2. Nobody may unilaterally take the money.** [FINDING 23](SECURITY-FINDINGS.md#finding-23),
filed in wave 7, untouched, and the only thing an auditor has ever graded `fund-theft`. One
private key can `--mode reinstall` a funded table and wipe every balance while the ICP stays in
the canister's account. The fourth auditor did it and then watched bob be told
`"Insufficient balance. Have: 0.0000 ICP"` for 10.98 ICP that was his. No amount of conservation
work reaches this, and `README.md:7` promises the opposite of it in the first paragraph. The fix
is governance, not code: NNS or SNS control, or a blackhole with a timelocked upgrade path.

**3. Every address the product hands out must be one the canister can see, account for and
prove.** Three findings collapse into this single sentence:
[FINDING 35](SECURITY-FINDINGS.md#finding-35) (the main account has no observation record and no
guard term), auditor four's #3 (the address is served over an **uncertified query**, so one
dishonest node is a working theft), and auditor four's #4 (no `derivationOrigin`, so the address
depends on which hostname you arrived at). The concrete work is small and specific: delete
`get_deposit_address()` or make it return the caller's own subaccount; add a main-account
observation and a fifth term to `total_liability()`; serve the deposit address from an update call
or certified data; pin `derivationOrigin` and ship `.well-known/ii-alternative-origins`.

Everything else on the register (the stuck-hand hatch, the archive's identity, the pod-clock
occlusion, the lobby's blinds) is real and should be fixed, and none of it changes a stranger's
answer while any of those three is false.

### Second, and bluntly: is the answer converging, or is each wave finding a new class at a new boundary?

**Both, and the mix has flipped. That is the finding.**

**On the money itself, it has converged.** Four independent auditors, 25+ randomised hands, dozens
of hostile sequences, two full drains, mid-hand upgrades, side-pot layering on a 4-way all-in with
a player leaving mid-hand: **nobody has moved a total by one e8**. Auditor four drained table_2
to exactly zero on both sides using player-callable methods only: 200 ICP in, 200 ICP out. The
shuffle is genuinely verifiable: three independent implementations written from
`docs/SHUFFLE-SPEC.md` alone have reproduced live hands, including predicting the turn and river
from a commitment copied off the wire mid-hand. That is not nothing and it is not in dispute.

**On the boundary, the classes are still moving outward, but they are moving outward
predictably.** Look at where the blockers have sat:

| wave | auditor's blockers | boundary |
|---|---|---|
| 1 | fund lock, unidentifiable binary | inside the canister; the build |
| 2 | controller destroys chips, silent stake, no clock | inside the canister; liveness |
| 3 | floors, subaccounts, no journal, the archive | **the ledger/escrow boundary; the record** |
| 4 | controller custody, the main account, uncertified queries, per-origin identity, archive authority | **the trust model: transport, identity, governance** |

Each wave's set is further from the arithmetic and closer to *who is trusted*. That is what
convergence looks like when it is happening from the inside out. A project that was still finding
conservation bugs in wave 8 would be in far worse shape than one finding that its deposit address
is uncertified.

**But here is the blunt part, and it is a change of failure mode.** Six of auditor four's ten
findings, including the only `fund-theft`, were **already in this repository's register before the
wave started**. FINDING 23 was filed in wave 7 and nobody worked it. FINDING 31 and FINDING 34
were filed by this wave's *own critics*, mid-wave, and shipped open. E-65 and the lobby bring-up
bug were both known. **We have stopped mostly failing to find things and started failing to
schedule them.**

**Count it.** At the start of wave 8 the register's highest entry was FINDING 30. Wave 8 **closed
six** findings that predated it (21, 25, 27, 28, 29, 30) and **opened seven** (31–37), of which
only 33 was closed in the same wave, by this pass rather than by the agent that opened it. Net movement
on the open queue: **minus six, plus six.** The register is not shrinking. It is churning, at the
same rate, one boundary further out each time.

That has a specific, measurable cause and it is visible in this wave's own numbers. Five agents
produced five real fixes and, between them, **three findings that were filed as OPEN by one agent
and reported as FIXED by another in the same wave.** FINDING 33 is the sharpest: the journal
agent's own problem list says the seam is closed, another wave-8 reviewer had already filed it as
critical and open, and the code says `== Pull`. Nothing reconciled the reports against the
register. A wave that finds more than it closes and does not reconcile will keep producing a new
auditor set forever, not because the boundary keeps moving but because the queue keeps growing.

**The concrete recommendation:** wave 9 should fix **nothing it discovers** until it has closed
FINDING 23, FINDING 31, FINDING 34/35 and the build-provenance gap: the four items already in the
register with the highest severity, and it should start by reconciling every agent's problem list
against `SECURITY-FINDINGS.md` before any of them writes code. On the current evidence that alone
would move a stranger further than another wave of discovery.

---

## 8. What is in this wave, file by file

**Canister** (`src/table_canister/src/lib.rs`), one change by this pass:
`journalled_incoming_total()`'s filter, `== LedgerIntentKind::Pull` → `i.kind.credits_on_success()`
([FINDING 33](SECURITY-FINDINGS.md#finding-33)), with the reason written above the line so it
cannot be "simplified" back.

**Gate** (`tests/money_safety/tests/coherence_w8.rs`, new; `scripts/dev.sh`, one line):
`finding_33_*` asserts the currency guard stays shut with an open sweep and that the round trip is
available on a drained table. `finding_35_*` is a **reproduction with no assertion on the
defective numbers.** It prints them and asserts only the ledger fact, because a test that pins an
open defect's wrong answer goes red the day somebody fixes it, and this project has already lost a
wave to that (HARD RULE 7).

**Documents:** [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 33 → FIXED with the
before/after measured; FINDING 35, 36, 37 new. [DEFECTS.md](DEFECTS.md) E-70 … E-76 new, plus one
**live anchor collision fixed**: `<a id="e-68">` was sitting above the **E-69** heading, so every
`#e-69` link in the register resolved nowhere and every `#e-68` link landed on the wrong entry.
`#h-44` had no anchor at all. Both repaired; all cross-document anchors in these three files now
resolve.

**Evidence ledger:** `artifacts/screens/acknowledged-reds.json` was **rebased from the real
sweep**, not carried forward. Three entries deleted as measured-green (E-61, E-63, E-64), five
new ones filed under E-71/E-75/E-76, and E-65's two re-stated with the numbers the pixels
actually produced. `make hygiene` is green on it with 10 red, 10 acknowledged, 0 stale.

**Local fixture:** the lobby was registered by hand so the sweep could run
(`lobby init_microstakes_tables` as the identity `get_admin` names). That is an operational step
on the local replica, not a code change, and [E-74](DEFECTS.md#e-74) is the entry for making
`dev.sh` do it correctly.
