# Wave 7 — the coherence pass, and the honest answer

> ## THE ANSWER IS STILL NO.
>
> A **third** independent auditor was run after this wave's fixes, on the running system, given
> the repo as a stranger finds it. Its verdict, verbatim:
>
> > **"No — I would not tell a friend their money is safe here."**
>
> The second auditor's verdict, for comparison:
>
> > **"NO — I would not tell a friend their money is safe here."**
>
> The first auditor's:
>
> > **"No. I would not tell a friend their money is safe here, and the reason is not the poker."**
>
> **The reason is still not the poker, and it is no longer the same reason.** Every blocker that
> stopped auditor two is closed, and I drove two of the four myself at the seam rather than reading
> the write-ups. What replaced them is a different set of defects at a different boundary, and one
> of them is new in kind.

---

## 1. Did the verdict change, and is what is blocking the same class of thing?

**No, and mostly no.** The four things auditor two would not sign off on are gone:

| auditor two's blocker | state now | how I know |
|---|---|---|
| `reset_table` destroys every seated chip ([FINDING 07](SECURITY-FINDINGS.md#finding-07)) | **closed** | `reset_table` now refuses while the table holds chips or a pot; `init_table_state` traps rather than replacing a funded `TableState`. `wave6_coherence::probe5` was inverted from a pin into a gate and is green |
| `cash_out` of a stuck hand walks away from your stake ([FINDING 18](SECURITY-FINDINGS.md#finding-18)) | **closed, and it opened a new door** — see §2 | measured: the exit doors now settle the hand first, and the stake leaves with its owner |
| nothing on chain moves the game ([FINDING 19](SECURITY-FINDINGS.md#finding-19)) | **closed for the clock half** | measured in §2: a stalled hand resolves itself in 2 rounds with **zero ingress messages**. The cycles half is open and worse ([FINDING 24](SECURITY-FINDINGS.md#finding-24), [FINDING 26](SECURITY-FINDINGS.md#finding-26)) |
| a hand settles with no live claim on it ([FINDING 17](SECURITY-FINDINGS.md#finding-17)) | **closed** | `live_claims` now *calls* `is_in_hand` instead of restating it; `hand_membership::predicate_table` executes the whole family |

### What blocks now, and in which class

| # | auditor three's blocker | class |
|---|---|---|
| 1 | **`deposit()`'s floor is 20,000 e8s; `withdraw()`'s is 100,000 e8s.** Anything in between can never leave. `table_1` on the auditor's instance holds 80,000 e8s of two real players' money that no method any player or controller can call will move. | **OLD class, NEW instance.** A threshold mismatch producing an unreachable balance is exactly auditor one's fund lock, one layer down. Nobody touched this region this wave. |
| 2 | **Money in the canister's own published deposit subaccount is reported as zero by every surface**, and at or below the transfer fee cannot be swept at all. `claim_external_deposit` replies *"No claimable balance. Send ICP to your deposit address first"* while holding the ICP at exactly that address. | **OLD class ([FINDING 18](SECURITY-FINDINGS.md#finding-18)) in a second place — and the wave's own critic hit the same blind spot independently ([FINDING 21](SECURITY-FINDINGS.md#finding-21)).** See §5: this is the single largest structural hole in the instruments. |
| 3 | **`deposit()` and `claim_external_deposit()` move real money on the ledger and only then credit escrow, with no journal.** A trap in the post-await continuation leaves the transfer standing and the credit rolled back, for an unbounded amount, with no recovery path. | **NEW IN KIND.** Not a threshold, not a predicate, not a conservation bug — a durability/ordering defect at the ledger boundary. It is the only blocker the auditor could not exercise live, and the only one that can cost an arbitrary amount. |
| 4 | **`record_hand_to_history` builds its player list from the seats as they stand at settlement**, so the permanent record omits anyone who left mid-hand and invents anyone who sat down mid-hand. Archived hand 4 names a principal who never played it as the small blind and does not balance. Following SHUFFLE-SPEC §4 on hand 10 reproduces the **wrong board**. | **OLD class (FINDING 13 / E-36: a seat is not a person), NEW surface.** The payout path was fixed to carry owners; the ARCHIVE path was not, and the archive is what "provably fair" rests on. |
| 5 | **Any controller reads the whole shuffled deck and every hole card live, mid-hand, via `get_table_state`**, and no surface discloses who the controllers are. | **OLD in the code, NEW in the documents.** Compounded this wave: [FINDING 22](SECURITY-FINDINGS.md#finding-22) shows the FINDING 07 fix also lets that controller *void* the hand after reading the cards. |

**The one-sentence answer for the lead:** money conservation is now genuinely solid — three
independent auditors, six rounds of hostile play, a mid-hand upgrade and two full drains could not
move the total by one e8 — and **every remaining blocker sits at a boundary where conservation is
not the question being asked**: the ledger↔escrow boundary (thresholds, subaccounts, journals), the
record boundary (the archive), and the outcome boundary (who wins, which conserves by construction).
That is real progress and it is not the same as safe.

---

## 2. THE FOURTH INSTANCE — found, reproduced, and it is the shape that was predicted

Four agents edited `src/table_canister/src/lib.rs` this wave in supposedly disjoint regions:
admin/custody, custody-visibility, timers, predicates. Every previous wave that did this produced a
defect visible only from above all of them, always with the same signature: **correct totals, wrong
recipients, every invariant silent.** It happened again, between the **timers** agent and the
**custody-visibility** agent.

### The two agents, and the sentence they disagree about

The timers agent found and fixed a real defect in their own work, and wrote the rule down:

> *"Past its grace" is a statement about WALL CLOCK. "Nothing can move this hand" is a statement
> about ATTEMPTS. They are only the same while the clock is actually running.*
> — `advance_table_clock`, `src/table_canister/src/lib.rs`

They applied that rule to `clock_should_abandon`, the predicate **the timer** uses, by measuring the
grace from `CLOCK_STUCK_SINCE` — when the canister *first saw* the clock overdue.

In the same wave, the custody-visibility agent wired **three new doors** onto `hand_is_stuck`, the
predicate that still reads raw wall clock:

```rust
Some(ref t) => now > t.expires_at.saturating_add(STUCK_HAND_GRACE_NS),
```

* `cash_out()` — settles the hand before vacating the seat (new this wave);
* `leave_table()` — the same (new this wave);
* `get_custody_status().advice` — the sentence the UI shows the player (new this wave).

Neither agent was wrong inside their own region. From above both, the canister now holds **two
different beliefs about the same hand at the same instant.**

### Measured, on the module under test `cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105`

Heads-up, both seats funded, a live pre-flop hand. The canister does not execute for
`action_timeout + grace + 1s` — a subnet stall, a canister frozen for want of cycles and then topped
up, or a controller stopping it to upgrade. Then, from that **one** state:

```text
=== ARM: the clock alone, ZERO ingress messages
  pot=4000000  phase=PreFlop  on_clock=alice  stakes=[2000000, 2000000]
  clock resolved it after 2 rounds
  RESULT phase=HandComplete   alice=+0          bob=+4000000     sum=4000000

=== ARM: what the canister TELLS the player to press
  pot=4000000  phase=PreFlop  on_clock=alice  stakes=[2000000, 2000000]
  abandon_stuck_hand(alice) -> Ok(4000000)
  RESULT phase=HandComplete   alice=+2000000    bob=+2000000     sum=4000000
```

**Same hand. Same instant. 2,000,000 e8s changes hands depending on which door is walked through,
and the totals conserve exactly in both.** `alice` is the seat whose clock ran out. In the arm the
canister runs by itself she loses her whole stake to `bob`; in the arm she chooses, she keeps it.

### The part that makes it worse than the existing write-up

[FINDING 25](SECURITY-FINDINGS.md#finding-25) already records the `abandon_stuck_hand` half — the
timers agent's own critic found it. Three things it does not say, all measured here:

**1. The canister actively instructs the losing player to do it.** At that exact moment,
`get_custody_status()` returns, to *both* players, in the canister's own words:

```text
LOSER (on the clock): committed=1000000 stuck=true abandonable_in_ns=None
    advice: 0.0100 ICP (1000000 e8s) of yours is still committed to hand 1, and that hand can
    no longer be moved by any message, so nobody can win it. Call abandon_stuck_hand() -- any
    principal may -- and every stake goes back to whoever put it in, including yours, into your
    withdrawable balance.
```

Every clause after "committed to hand 1" is false, and the canister's own clock disproves it two
rounds later. This is not an open door; it is a signpost pointing at it.

**2. `cash_out` and `leave_table` are on the same raw predicate, and they are strictly better for
the attacker.** `abandon_stuck_hand` only recovers the stake. `cash_out` recovers the stake **and**
takes the whole stack out of a live hand in one call:

```text
cash_out(the seat on the clock) -> Ok(200000000)
  phase=HandComplete pot=0 (was 3000000)
  escrow: loser=2000000000  winner=1800000000
```

The loser walks out with every e8 they arrived with. No recovery method needs to be called and no
good-citizen framing applies.

**3. A principal who has never sat at the table can settle the hand, and gets an error reply saying
it did nothing.** `cash_out`/`leave_table` run the settle **before** the seat lookup, and an `Err`
return from an ic-cdk update is a normal reply, not a rollback:

```text
mallory (never at the table) cash_out -> Err("Not at table")
  phase PreFlop -> HandComplete    pot 3000000 -> 0
```

### Why every gate in the project is blind to it

`World::advance()` is `pic.advance_time(d); pic.tick();` — **it always lets a round run**, so the
on-chain clock always wins the race and the window never opens. The only way to open it is
`World::advance_time_only`, which appears in exactly three places in the whole tree, all in
`tests/money_safety/tests/invariants/custody.rs`, and **all three use it for QUERIES only**. No gate
in this project has ever sent an update through the window. The fuzzer jumps hours of simulated
time and still cannot reach it, for the same reason.

Recorded as [E-59](DEFECTS.md#e-59); [FINDING 25](SECURITY-FINDINGS.md#finding-25) is amended with
all three measurements and re-rated.

### The predicate/state table this came out of

Thirteen predicates across the four agents' regions, evaluated on one state: *a live hand whose
action clock expired more than `STUCK_HAND_GRACE_NS` ago, on a canister that has not executed
since*.

| predicate | owner | reads | verdict on that state | consequence |
|---|---|---|---|---|
| `hand_is_stuck` | pre-existing, **3 new callers this wave** | `now > expires_at + GRACE` | **STUCK** | `abandon_stuck_hand`, `cash_out`, `leave_table` all refund |
| `clock_should_abandon` | timers | `now > CLOCK_STUCK_SINCE + GRACE` | **NOT STUCK** | the clock folds the seat and plays on |
| `get_custody_status.committed_is_stuck` | custody-visibility | `hand_is_stuck` | **STUCK** | tells the player the hand is dead |
| `TableView.hand_is_unmovable` | custody-visibility | `hand_is_stuck` | **STUCK** | the UI headline |
| `get_stuck_hand_status.is_stuck` | pre-existing | `hand_is_stuck` | **STUCK** | — |
| `is_in_hand` / `live_claims` / `count_active_players` | predicates | `!folded && hole_cards.is_some()` | agree with each other | **no divergence found — this family is coherent** |
| `deals_in_this_hand` vs `will_be_dealt_in` | predicates | `Active` vs `Active && chips>0` | differ by design | **checked and safe**: `start_new_hand` sits out every broke seat before the ante loop, so at the deal loop the only zero-chip `Active` seat is one the blinds just put all-in |
| `table_custody` | admin/custody | live-gated `hand_stakes` + stacks | agrees with `committed_stake_of` | no divergence |
| `committed_stake_of` | custody-visibility | live-gated, by principal | agrees with `table_custody` | no divergence |

**One divergence, and it is the money one.** The predicates agent's family — the one the wave was
most worried about — is genuinely coherent; I checked all thirteen sites and could not make them
disagree.

---

## 3. Gates

Every figure below is from a run in this pass. Module under test throughout:
`cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105`.

| gate | result |
|---|---|
| `./scripts/dev.sh test` | **GREEN**, exit 0, ~5 min. 5/5 stages; 0 failures across every suite |
| `make settlement` (full, with the PocketIC runs) | **GREEN**, exit 0. 22 + 5 + 9 + 3 passed, 0 failed |
| `make fuzz-default` | **GREEN**, exit 0. 3 seeds, 14 hands, **0 blocking findings**, worst stranded 0 e8s |
| `make known-defects` | **GREEN**. 1 of 1 engine defects still present (`detect_straight`, E-13). No marker pins a fixed defect |
| `make selftest` | **GREEN**. 20/20 hostile argument shapes refused; 7 mainnet ids in the denylist |
| `make hygiene` | **RED, exit 1** — see §4. Notices themselves: **9/9 intact, none weakened since `ceacc37`** |
| `#[ignore]` audit | **CLEAN.** Three `#[ignore]`s in the tree; none pins a fixed defect (`e13` is a live defect, the other two are runtime-cost skips). The wave-6 `probe5` pin is gone |
| seam mutations (H-04) | see §6 |
| **shots sweep / rendered notice gate** | ❌ **COULD NOT RUN — see §4. This is the second wave running.** |

### What the green fuzz gate does *not* say

`make fuzz-default` reports `0 of 14 hands reached a decided-by-fold-out moment and had the sharp
leg run` on **all three seeds**. M11's only sharp leg — the one that could tell a wrong winner from
a right one — ran on **zero hands of a real run**, because a correct engine never shows the state it
needs. The "14 of 14 hands had their OUTCOME checked" figure is carried entirely by the two weak
legs. M8 declined 2 of the 14 hands. So on a correct engine the instrument between "the wrong
claimant was paid" and a green gate is still the settlement oracle alone, and it abstains on some
hands. This corroborates the predicates critic and is why §2's defect was invisible.

---

## 4. THE NOTICE CHECK ON RENDERED PIXELS — I COULD NOT RUN IT

**Saying this loudly, as instructed, rather than reporting the rule as satisfied.**

The local replica is down and its on-disk state is **not resumable**:

```text
==> [1/6] replica
    ! the on-disk state of the managed network cannot be resumed:
      subnet checkpoint heights do not intersect: ...
FATAL: refusing to start. Re-run with '--reset' to DELETE .icp/cache/networks/local
```

`./scripts/dev.sh local-up --reset` is the only way forward and **it is blocked by this session's
permission policy** (it destroys the local ledger and every deployed canister). I did not work
around it. With no replica there is no frontend to drive, so `./scripts/dev.sh shots` cannot run,
and with it go: the four protected notices on rendered pixels, the occlusion gate, the on-screen
money agreement, and the toast gate.

### What the evidence on disk actually says, and how much weight it carries

The last full sweep was run by the build-story agent at `4af6dc6` (dirty), 2026-08-06T03:45Z, on
this tree state — `artifacts/screens/latest/`. Read from its manifest, not from a summary:

* **NOTICES: 5/5 on screen in all 24 (scene, viewport) pairs, and "notices survive an error toast"
  in all 24.** That includes the table views and the views with a modal open. On that run, the
  protected-notice rule held.
* **7 of 12 scenes are `verified=false`.** `lobby`, `table-preflop`, `table-facing-bet`,
  `table-allin`, `table-sidepots`, `handhistory`, `toast-notices`.
* **The pixel gate caught this wave's own regression**: `table-allin` mobile —
  `OCCLUSION FAILED: 8.7% of "0.20 ICP" (span.committed-value) covered by div.stage`. That is the
  custody-visibility agent's new committed-stake figure, occluded at 390x844. It is the same failure
  its own critic measured by hand, and it is sitting in the artifact unaddressed.
* **Five scenes are red on the custody UI being unasserted**: `TOKEN CENSUS FAILED: ... "0.10" in
  ... span.committed-value / "1" in ... span.committed-note`. The new FINDING 18 surface renders
  money that no gate ties to a canister figure.

**I am not claiming the notice rule is satisfied.** I am reporting that the last run said it was,
that I could not reproduce it, and that the run which said so was itself red on 7 of 12 scenes.
A suite everybody expects failures from cannot convict the next one — which is the wave-6 lesson
about REG-09, now reproduced in the screenshot harness.

### `make hygiene` is RED, and it is this wave's own artifact

```text
==> no large or binary files added
    48 untracked path(s), 17023 KiB total
    ✓ no binary or image files would be committed
    ! untracked payload exceeds 4 MiB; check for a stray artifact directory
==> result
    ! repo hygiene FAILED
```

`artifacts/screens/latest/manifest.json` is a **tracked** file that went from 763,703 bytes at
`HEAD` to **7,612,380 bytes** — a 10x growth, 4.48 MB of it in `scenes`, from the new per-figure
pixel/occlusion sampling. `artifacts/screens/4af6dc6/manifest.json` is a second, untracked copy of
the same 7.4 MB. The PNGs are gitignored; the manifests are not. Recorded as
[E-60](DEFECTS.md#e-60).

### The no-rake gate is permanently red for a reason that has nothing to do with rake

`handhistory` fails at both viewports with:

```text
RAKE TAKEN: hand 1 recorded rake=NaN e8s. ClearDeck publishes a no-rake property;
a non-zero rake contradicts it.
hand 1: history says total_pot=2400000000 but the winners were paid 2400000000 with rake NaN
```

`tools/shots/lib/chain-agreement.mjs:1546` reads `Number(r.rake)` off the records returned by
`history.get_hands_by_table`, which returns `vec HandSummary` — and `HandSummary` **has no `rake`
field** (`src/declarations/history/history.did:41-50`). `rake` is `undefined`, `Number(undefined)`
is `NaN`, `NaN !== 0` is true, and `totalPot !== awarded + NaN` is true. Both failures are the same
missing field.

The comment thirty lines above it reads: *"An earlier version of this function read `rec.total_pot`
off the TABLE record. That field does not exist, so `Number(undefined)` was NaN and every comparison
was vacuous... **Absence of a field is now a structural failure, never a silent NaN.**"* The author
found that mistake, fixed it in one field, and left the identical mistake in the adjacent line.

**Why this matters more than a broken test:** no-rake is one of the four properties this project
says must never be weakened, and this is the only gate that asserts it against the archive. It cries
wolf on every hand. I did **not** patch it, because I cannot re-run the sweep to verify a change to
a gate — changing a gate blind is how the next wave inherits a false green. Recorded as
[E-61](DEFECTS.md#e-61) with the one-line fix (`get_hand(hand_id)` returns the full
`HandHistoryRecord`, which does carry `rake`).

---

## 5. The blind spot two independent reviewers found separately

The wave's own critic (attacking FINDING 20) and the third auditor (attacking nothing in
particular) arrived at the same hole from opposite directions:

| who | what they found |
|---|---|
| wave-7 critic | `total_liability()` = `escrow + chips + pot` omits the deposit subaccounts, so the FINDING 20 currency guard permits a flip that strands 5 ICP. `check_no_orphaned_custody` computes `ledger_main - claims - uncredited_raw`, so a canister holding 7 ICP in its own subaccounts reports **0** orphaned e8s |
| auditor three | ICP at the canister's own published deposit subaccount is reported as zero by `get_balance`, `get_custody_status`, `admin_get_balance` and `admin_get_all_balances`, and at or below the transfer fee is unsweepable. `claim_external_deposit` tells the player to send **more** money to the address already holding theirs |

**The canister owns two kinds of ledger account and every instrument in this project measures one of
them.** M1's conservation identity, M9's orphan check, the FINDING 20 guard, the drain's
`table_is_really_empty`, and all four balance surfaces are anchored to `icrc1_balance_of(table,
None)`. The deposit subaccounts — the addresses `get_deposit_address()` publishes to external
wallets — are outside all of it.

This is the standing lesson in its purest form: the instrument was built by people who knew where
the money was supposed to be. It is the highest-leverage single fix in the project, because one
change of anchor closes half of FINDING 21, half of auditor three's #2, and the M9 hole at once.

---

## 6. Seam mutations (docs/DEFECTS.md H-04)

Re-run in this pass with `$SCRATCH/seam_mutations_w7.py` against a `cp -Rc` copy, one mutation at a
time, each anchor asserted to match exactly once, `lib.rs` asserted byte-identical afterwards, gate
per mutation = `cargo test --workspace` + `cd tests/money_safety && cargo test --test invariants`.
Two of the seven wave-6 anchors had to be re-translated again to their current sites (`plan_payouts`
now takes `stakes` from `hand_stakes`; `refresh_side_pots` is unchanged).

**Result: 6 of 7 die. NOT DOWN. Same survivor as waves 2, 3, 4 and 6.** `lib.rs` sha256
`93c5c1f9852d07e293ebcf561d73081ac39af82492eee8d3bfb60eb0836cb505` before and after, identical.

| mutation (wave-7 site) | verdict | what convicted it |
|---|---|---|
| 1. `refresh_side_pots` made a no-op | **died** | money_safety `invariants` |
| 2. every stake halved in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
| 3. the breakdown written into a throwaway `Vec` | **died** | money_safety `invariants` |
| 4. `&[]` as the contributions in `plan_payouts` | **died** | `cargo test --workspace`, money_safety `invariants` |
| 5. the self-report line dropped | **SURVIVED** | nothing (equivalent mutant, unchanged since wave 2) |
| 6. `try_evaluate_hand` stubbed to `RoyalFlush` on the settlement path | **died** | `cargo test --workspace`, money_safety `invariants` |
| 7. every hand dealt from `b"CONSTANT"` | **died** | money_safety `invariants` |
| | **6 of 7** | |

Two anchors were re-translated for this tree: mutation 2 now halves the `Vec<Stake>` that
`plan_payouts` takes from `hand_stakes` (wave 6's site was the same line before `refund_every_stake`
grew a copy of it), and mutation 6 stays pointed at `poker_core::try_evaluate_hand` inside
`rank_claims`, which is where the FINDING 15 fix put the only evaluator call on the settlement path.
Mutation 5's survival is unchanged and is still an equivalent mutant at canister level: the warning
it deletes only fires when the built side pots exceed `state.pot`, which honest play cannot reach.

---

## 7. Document coherence: six findings, three numbers

Two agents' critics both wrote findings 21, 22 and 23 into `docs/SECURITY-FINDINGS.md` in the same
wave, with colliding `<a id="finding-2x">` anchors. Every internal link resolved to whichever
duplicate the reader's viewer reached first. Fixed here by renumbering the cycles/timers set:

| was | is now | subject |
|---|---|---|
| FINDING 21 (first) | **FINDING 21** | `total_liability()` omits the deposit subaccounts |
| FINDING 22 (first) | **FINDING 22** | the FINDING 07 fix lets a controller void a live hand after reading the cards |
| FINDING 23 (first) | **FINDING 23** | `uninstall_code` / `reinstall` are FINDING 07 at full scale |
| FINDING 21 (second) | **FINDING 24** | a frozen table answers nothing, not "queries only" |
| FINDING 22 (second) | **FINDING 25** | after a stall, one permissionless call voids the hand — **amended and re-rated here** |
| FINDING 23 (second) | **FINDING 26** | a permissionless ingress flood collapses the runway to ~3 days |

---

## 8. The answer for the lead: the shortest path to a stranger saying yes

Ordered by *how much of the "no" each one removes*, not by effort. The reachability column is the
part that matters for planning.

| # | what has to happen | reachable from this machine? |
|---|---|---|
| 1 | **Re-anchor every money instrument to every account the canister owns** — main *plus* every deposit subaccount — in `total_liability()`, `check_no_orphaned_custody`, `table_is_really_empty`, and all four balance surfaces. Closes FINDING 21, half of auditor three's #2, and the M9 hole in one change. | **YES.** Pure canister + harness work; `World::snapshot` already reads `ledger_deposit_subaccounts` and simply is not compared against anything. |
| 2 | **Write intent before moving money.** A journal entry for `deposit()` and `claim_external_deposit()` before the ledger call, and a resume path that reads it. Auditor three's only unbounded-loss finding and the only one nobody has exercised. Needs a fault-injecting ledger stub that returns `Ok` and then makes the tail trap. | **YES.** PocketIC can install a stub ledger; nothing here needs mainnet. |
| 3 | **Make the three manual doors agree with the clock** (§2). The safe shape is a floor, not a substitution: refuse until the clock has been overdue for the grace **and** the canister has been executing for at least one grace period, with a first-sighting timestamp `post_upgrade` re-seeds — so the permissionless escape hatch cannot be permanently closed by a dead timer, which would rebuild FINDING 15. Gate it with `advance_time_only` + an **update**, which no test in this tree has ever done. | **YES.** |
| 4 | **Make the deposit floor and the withdrawal floor the same number**, and make the modals state one limit. A balance the product's own documented minimum creates and cannot release is the simplest possible "no". | **YES.** |
| 5 | **Rebuild the archive from the payout basis, not from the seat vector** — `hand_stakes`, the same source the payout uses — so a departed player is in the record and a mid-hand arrival is not. Gate it with the property auditor three named: for every archived hand, `sum(ending − starting)` reconciles against `total_pot`, and the recorded seat set equals the dealt-in set the shuffle consumed. Without this, "provably fair" is false for any hand somebody left. | **YES.** |
| 6 | **Say who the controllers are, and that they can see every hole card**, on the landing page and in the README, next to the existing four notices. Removing the capability is a bigger project; not disclosing it is a choice being made right now. | **YES.** |
| 7 | **Restore the rendered gates**: bring the replica back (`local-up --reset`), fix the `HandSummary.rake` read (E-61), fix the manifest size (E-60), and get the shots sweep to a state where a red scene means something. | **PARTLY.** The reset is the blocker in this session, not in general. |
| 8 | **Cycles**: an off-chain watcher that alarms on *unreachability*, a top-up path, and a freezing reserve that is worth more than 1.16 hours. | **PARTLY.** Design and harness work are local; a real watcher and a real top-up are operational. |
| 9 | **The verification story end to end.** `--local` verifies its own tree against itself and cannot detect a bad source tree; the mainnet check — the only one that answers a depositor's actual question — is stated in advance to print NOT VERIFIED; and all six modules report `git:dirty`. | **NO — and this is the one that is not reachable from here.** It needs (a) a clean commit, (b) a container build published from it, and (c) `icp canister status` against the **mainnet** canisters to read their module hashes. The mainnet controller key is not on this machine. Everything up to the comparison can be prepared locally; the comparison itself cannot be run. |

**What I would tell a depositor today, in one sentence:** the poker is right, the arithmetic is
right, and the money is not safe — not because anyone can steal it, but because there are at least
three documented ways for it to arrive and never leave, and the check that would tell you the code
holding it is the code in this repository cannot currently pass.

---

## 9. Reproducing everything in this document

```bash
./scripts/dev.sh doctor
./scripts/dev.sh test                 # ~5 min, green
./scripts/dev.sh settlement           # ~2 min, green
./scripts/dev.sh fuzz-default         # ~1 min, green
./scripts/dev.sh known-defects        # green: 1 of 1 expected red
./scripts/dev.sh selftest             # green
./scripts/dev.sh hygiene              # RED (§4)

# the fourth instance (§2), in a cp -Rc copy
cp -Rc . $SCRATCH/w7
cp $SCRATCH/w7_coherence.rs $SCRATCH/w7/tests/money_safety/tests/
cd $SCRATCH/w7/tests/money_safety
CLEARDECK_TABLE_WASM=<repo>/target/wasm32-unknown-unknown/release/table_canister.wasm \
  cargo test --test w7_coherence -- --nocapture --test-threads=1

# seam mutations, on a cp -Rc copy; lib.rs asserted byte-identical afterwards
python3 $SCRATCH/seam_mutations_w7.py $SCRATCH/seam7

# THE ONE THAT DID NOT RUN
./scripts/dev.sh local-up --reset     # blocked by permission policy in this session
./scripts/dev.sh shots                # therefore not run
```
