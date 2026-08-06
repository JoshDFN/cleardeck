# Wave 2 — what is now proven, what is not, and where the trust story actually stands

Wave 1 established ground truth and deliberately changed no behaviour. **Wave 2 changed
behaviour.** Seven pieces were built in parallel by agents who could not see each other's work,
three of them editing different regions of the same 6,300-line file. This document is the
coherence pass over that: everything below was re-run or re-derived by a reader who built none
of it, against one binary whose sha256 is stated.

Read this file with [DEFECTS.md](DEFECTS.md) (the register) and
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) (the evidence). Where they disagree with a builder's
own summary, they are right and the summary is stale.

> **Still unaudited alpha software.** Nothing in this wave changes that. Nobody has ever played on
> mainnet, and nothing here says anyone should.

---

## The one-paragraph answer

**The shuffle is now genuinely verifiable, and that is the wave's real result.** A stranger can
take a revealed seed and reproduce every card of a real hand; I did it, from the specification,
with code that shares nothing with the engine. **Money conservation is now genuinely tested** —
the harness compiles the canister it tests, **10 of 11 mutations die where 7 of 7 used to
survive**, and an independent settlement oracle agrees with the engine on all 17 hard settlement
classes. **But the question "does the right *player* get paid" is still open**, and it
is open in a sharper way than before: wave 2 closed money being *destroyed* and opened money being
*misdirected to the wrong principal*, on the same code path, in a form every gate it built is
blind to. Two agents also each added a persisted field that makes an upgrade from existing state
fail — and the obvious fix for one of them silently destroys every chip at the table. And the wave's own long gate, `make fuzz`, is **red at its shipped settings** — the wave that
introduced the defect ran a third of it. **The engine is not ready to be left alone yet.** Details,
and a ranked list, below.

---

## 0. READ FIRST — a second session edited `lib.rs` while this pass was measuring it

Partway through this coherence pass, `src/table_canister/src/lib.rs` began changing every ~20
seconds under me. The writer was another Claude Code session working in the same checkout (its
`cargo test --workspace` and its shell wrapper were both visible in `ps`, under a different session
id). It landed the E-38 fix this document recommends: `PersistentState::deposit_watermark` and
`TableState::departed_stakes` are both `Option<…>` in the tree as you read it, with
`departed_stakes()` / `departed_stakes_mut()` / `clear_departed_stakes()` accessors.

Three consequences the lead has to know, because they bound how much of this document is evidence:

1. **Everything in §2 through §9 was measured against a stated sha256, and those measurements are
   sound.** Each one names its binary. Nothing below is a guess.
2. **The tree as committed is NOT the tree those numbers were taken from.** The full gate was last
   green end to end on `0ebf554111ae…`. After that came my `leave_table` side-pot fix (§9a) and the
   other session's E-38 change. **Re-run `./scripts/dev.sh test` and `make fuzz` on whatever is
   finally committed before believing any of it holds.**
3. **My own edits all survived** — verified by content, not by hope: the table-state digest and its
   `post_upgrade` guard, the TDA 47-A rule, the cross-street timer refusal, and the `leave_table`
   side-pot refresh are all present. A copy of the file as this pass left it is at
   `$SCRATCH/coherence-pass-lib.rs` (sha256 `6650d4aff450c789…`) if any of it needs recovering
   after a merge.

**Two agents doing read-modify-write on one 6,300-line file, with no lock and no branch, is how a
fund-holding canister loses a fix silently.** That is the same class of problem as everything else
in this document, one level up: wave 2's defects came from seven agents who could not see each
other's edits to this file, and the coherence pass ran into it live. If wave 3 has more than one
agent touching `lib.rs`, give them separate branches or separate regions with an explicit owner.

### E-38 is FIXED, and M7 says so — measured on a stable tree

Once the writes stopped for two consecutive 30-second windows I re-ran M7 with the source hash
recorded on both sides of the run, so the result is attributable (`lib.rs` `e10e6fa47089a89f`,
unchanged before and after):

```
M7: on 5e12d25bf4d2 -> escrow 600000000, seated chips 400000000, pot 0
M7: cross-version upgrade SUCCEEDED with every e8 intact.
```

**That is the number that mattered.** With both fields `opt`, a genuine `install_code --mode
upgrade` from the `801aa79` release preserves escrow and seated chips exactly — so the E-02 deposit
anti-replay work, the fix for the only proven fund theft in this project, **is now deployable by
upgrade.** The same test reported `DESTROYED SEATED CHIPS: 400000000 -> 0` when only
`deposit_watermark` was made `opt`, which is what makes this a measurement rather than an
assumption. E-38 moves to FIXED; my `post_upgrade` digest guard stays as the guard rail for the
NEXT field somebody adds.

### One test is red on the committed tree, and it is not mine

The other session also renamed and strengthened M7 into
`m7_state_written_by_the_previous_release_survives_the_upgrade_exactly`, which additionally drives
a raw transfer plus `notify_deposit` across the upgrade. **On binary
`31fb3b0cd3a452e8…` that test FAILS:**

```
the first notify_deposit for a real transfer must succeed:
  Err("Failed to decode ledger response: CandidDecodeFailed { ...
       type_name: \"(...QueryBlocksResponse,)\", candid_error: \"Fail to decode argument 0\" }")
```

I did not write it and did not try to fix it, but I did establish three things so nobody
mis-diagnoses it:

* **E-04 has NOT regressed.** The decode site in `lib.rs` is still the fixed single-value form
  (`response.candid::<QueryBlocksResponse>()`), and `type AccountIdentifier = Vec<u8>` is still a
  bare blob. The tuple in that `type_name` is how `ic-cdk` 0.19 names its own internal decode, not
  the E-04 bug shape.
* **`notify_deposit` does still work in general.** `reg02_notify_deposit_can_read_the_real_ledger_and_never_fails_to_decode`
  and `dr01_notify_deposit_credits_a_real_transfer_exactly_once` both assert a SUCCESSFUL credit and
  both pass. So this is scenario-specific, not a broken door.
* **The likeliest cause is the block the scenario lands on.** The `Operation` enum in
  `notify_deposit` models `Transfer | Mint | Burn | Approve`. `reg02` and `dr01` use
  `raw_transfer_to_canister`, which writes a plain `Transfer`; a scenario whose target index is an
  ICRC-2 `Approve` block (or any variant not in that enum) would fail to decode exactly like this.
  Start there.

Because the gate's money-safety step short-circuits on the first failing target, that one failure
means **`regressions`, `deposit_replay` and `fuzz` did not run at all** in the final pass. Their last
green was on `0ebf554111ae…`.

### The gate, as of the last run on the committed tree

| step | result |
|---|---|
| `cargo test --workspace` | **green**, 208 tests |
| wasm build | **green**, `31fb3b0cd3a452e8…` |
| `tools/differential` | **green**, 25 + 22 (2 still `#[ignore]`d, both E-13) |
| money-safety `invariants` | **35 pass, 1 FAIL** — the other session's strengthened M7 |
| money-safety `regressions` / `deposit_replay` / `fuzz` | **not reached** (short-circuit) |
| settlement oracle | **green**, 8 + 8 |
| `make hygiene` | **green** |
| `make selftest` | **green** |
| `make known-defects` | **green**, 1 of 1 defect still present (E-13) |

---

## 1. The binary everything below was measured against

```
FINAL (this pass's fixes included)
  sha256 0ebf554111aec413483aebbf504838609200a2ad80e8cebff78ede5b3f64616a  (2,323,420 bytes)

AS THE SEVEN AGENTS LEFT IT (the mutation sweep and the first shuffle run used this)
  sha256 395696359b6d1a6e185fca9e2e051bbb6c8413b62e1545aae55c66d361d8342f  (2,314,067 bytes)
```

Reproducible: three consecutive `rm` + `touch` + `cargo build -p table_canister --target
wasm32-unknown-unknown --release` produced `395696359b…` three times from identical source. The
shuffle verification in §2 was re-run against the FINAL binary and reproduces the same result.

**One caveat about that number, which is new.** `tests/money_safety/src/wasms.rs` builds the
canister by shelling out to `cargo` with the **inherited environment**, and `RUSTUP_TOOLCHAIN`
inherited from a parent `cargo` overrides `rust-toolchain.toml` even though the child's
`current_dir` is the repo. Running the harness from a crate that resolves to a different
toolchain silently produced module `7babe62428943bf9…` from byte-identical source. It agrees with
`./scripts/dev.sh wasm` whenever it is invoked the way the Makefile invokes it, and it does verify
that the module the replica reports is the module it built — so H-01 is genuinely closed — but
"the sha256 the harness prints is the sha256 that gets deployed" is a claim about the *invocation*,
not about the code. Pin the toolchain in that `Command` and it becomes a claim about the code.

---

## 2. PROVEN — the shuffle is verifiable by an outsider

This is the item worth the most, because it is the only part of "provably fair" that a player can
check without trusting us, and before this wave **it was not true at all**: `usize` is 32 bits on
`wasm32`, so the on-chain shuffle differed from every native run and nobody could reproduce a deal
(FINDING-02).

What I did, deliberately not reusing either verifier the builder shipped:

1. Wrote a third verifier in Python **from `docs/SHUFFLE-SPEC.md` alone**
   (`$SCRATCH/coherence_verify.py`), including the rejection-sampling rule, the little-endian
   8-byte draw, the deck order and the dealing order.
2. Checked it against the spec's own worked example: **MATCH**, all 52 cards, 0 rejections.
3. Dealt **five real hands** on the real table canister wasm under PocketIC with the real ICP
   ledger, capturing only what a player can see (`get_shuffle_proof`, `get_my_cards` as that
   player, `get_community_cards`). Run twice: once on `395696359b…` and again, after this pass's
   fixes, on the final `0ebf554111ae…`. Identical result both times.
4. Reproduced them from the revealed seed.

| hand | seats | button | cards reproduced | discriminates dealing order? | pre-fix shuffle reproduces |
|---|---|---|---|---|---|
| 1 heads-up | 0,1 | 1 | **9 / 9** | no | 0 / 9 |
| 2 three-handed | 0,1,2 | 2 | **11 / 11** | no | 0 / 11 |
| 3 three-handed | 0,1,2 | **0** | **11 / 11** | **yes** | 1 / 11 |
| 4 three-handed | 0,1,2 | **1** | **11 / 11** | **yes** | 1 / 11 |
| 5 three-handed | 0,1,2 | 2 | **11 / 11** | no | 0 / 11 |
| | | | **53 / 53** | 2 discriminating | 2 / 53 (chance) |

`SHA256(revealed_seed) == seed_hash` held on all five. Hands 3 and 4 matter: with the button on
seat 0 or seat 1, "ascending seat index" and the usual casino "left of the button" convention
predict **different** hole cards, and ascending is what the canister does — so section 4 of the
spec is not folklore, it is checked. The shuffle agent's own two hands had no power over that
question; their reviewer found this and I confirmed it independently on the shipped module.

**What this does not prove:** that the seed was unpredictable. That is the subnet's `raw_rand`,
and it is trust, not arithmetic. The spec says so in its own section 0, correctly.

**The one thing still missing:** all of the above lives outside the repo. There is a wasm32 golden
replay in CI (2,000 vectors, and it convicts the old shuffle 2000/2000, which is real), but
**nothing in-repo plays a hand and reproduces it end to end.** Port it, with a button-on-seat-0
hand, or the deal seam stays as unprotected as the shuffle was. My verifier and the two dealt-hand
JSON files are in `$SCRATCH` (`coherence_verify.py`, `final_real_deals.json`) and are ~200 lines to
land as a test.

---

## 3. PROVEN — the harness now compiles the thing it tests, and the seam has real gates

Wave 1's headline harness defect was that the money-safety suite preferred a stale artifact and
had **never compiled the canister**: seven of seven mutations to `lib.rs` survived with the suite
green. That is fixed, and I re-measured it rather than believing it.

### Seam mutation re-run, on the shipped tree

Wave 1's seven mutations targeted `calculate_side_pots`, which the payout fix deleted, so I
translated each to its nearest current site and added three more that target what the wave 2
reviewers said escapes. One mutation at a time, restored between each; every mutation's anchor is
asserted to match exactly once, so a mutation that failed to apply cannot be misread as a
survivor. Gates run per mutation: `cargo test --workspace`, `tools/differential`, money-safety
`invariants`, `regressions`, `deposit_replay`, and the settlement oracle's `settlement` and
`disagreements`.

| # | mutation | compiles? | workspace | differential | invariants | regressions | deposit_replay | settlement | verdict |
|---|---|---|---|---|---|---|---|---|---|
| S1 | side-pot construction made a no-op | yes | green | green | **RED** | **RED** | green | green | **died** |
| S2 | payout basis halved | yes | **RED** | green | **RED** | **RED** | green | **RED** | **died** |
| S3 | side pots written to a throwaway | yes | green | green | **RED** | **RED** | green | green | **died** |
| S4 | `&[]` passed for the contributions | yes | **RED** | green | **RED** | green | green | **RED** | **died** |
| S5 | the pot-disagreement log line dropped | yes | green | green | green | green | green | green | **SURVIVED** |
| S6 | `evaluate_hand` stubbed to `RoyalFlush` | yes | **RED** | green | **RED** | green | green | **RED** | **died** |
| S7 | every hand dealt from `b"CONSTANT"` | yes | green | green | **RED** | green | green | *hung* | **died** |
| N8 | pot share rotated to the next occupied seat | yes | **RED** | green | **RED** | green | green | **RED** | **died** |
| N9 | odd chips to the LAST winner, not the first | yes | **RED** | green | green | green | green | **RED** | **died** |
| N10 | 1% of every pot skimmed to another seated player, in the PLAN | yes | **RED** | green | **RED** | green | green | **RED** | **died** |
| N11 | 1% skimmed where chips physically move, RECORD left correct | yes | **RED** | green | green | green | green | **RED** | **died** |
| | | | | | | | | | **10 of 11 die** |

S1-S7 are wave 1's seven, translated to their current sites (`calculate_side_pots` was deleted by
the payout fix). N8-N11 are new, and target exactly what the wave-2 reviewers said escapes.

**The one survivor is S5, the same equivalent mutant wave 2 already documented**: dropping a log
line that no reachable input can trigger. Wave 1's number was 0 of 7 caught with the suite green,
and the reason was H-01 — the harness had never compiled the canister.

Three results are worth reading closely, because each answers a reviewer's open claim:

* **N9 is the settlement reviewer's escaping plant.** They installed odd chips going to the LAST
  winner clockwise instead of the first — a pure wrong-seat payout that conserves every chip — and
  reported that the ENTIRE harness stayed green: `settlement` (8), `disagreements` (8) and
  `oracle_rules` (21) all exit 0. It now dies, in
  `pinned_odd_chips_now_go_one_each_clockwise` and in the randomised sweep. The payout agent's
  gating fix is real.
* **N11 is the harness reviewer's C4, at its real insertion point.** Chips are skimmed where they
  physically move, while `push_winner` still RECORDS the correct winner and the correct amount.
  Their claim was that money-safety cannot see this, and **they are exactly right**:
  `invariants` reports 30/30 green and `regressions` 6/6 green with it installed. It dies in the
  settlement oracle and in a `payout_tests` host unit test. So the property "the seat that is
  recorded as winning is the seat whose stack grew" is still not asserted by the money-safety
  suite — it is asserted, indirectly, by a different crate.
* **That is why the settlement oracle is now in `./scripts/dev.sh test`.** It is the only harness
  in the repo that measures per-seat chip deltas against an independently derived answer, and for
  the whole of wave 2 it sat behind a `make settlement` that no default target and no CI job
  invoked. A gate outside the gate is not a gate; that is the same lesson as H-17.

**One caveat on S7**, recorded because it is a finding in itself: `invariants` convicted it in 22 s
(`seam_c_the_deck_is_different_every_hand`), and then `tests/settlement` and `tests/disagreements`
**hung** and had to be killed. The oracle's rewind-and-re-deal search has no attempt cap, so a deck
that stops varying makes it spin rather than fail (docs/DEFECTS.md H-22). Those two cells are
`hung`, not `convicted`. `./scripts/dev.sh test` now wraps them in a timeout so a hang is a
failure.

### What else is genuinely closed

* **`wasms.rs` has one resolution path.** It always builds, hard-fails if the build fails, prints
  `MONEY-SAFETY: wasm under test sha256=…` on fd 2 before any test line, and re-reads
  `canister_status().module_hash` after every install *and* every upgrade. I saw the sha256 line
  on all three money-safety targets in a passing run.
* **The tolerance register is empty.** `documented::REGISTER` and `TOLERATED_SELF_REPORTS` are
  both `&[]`, so no money-path finding is excused. That is a much stronger position than wave 1's
  sign-based `is_documented_defect`.
* **`integration_test.rs` is no longer comments only.**
* **The evaluator's degenerate-input probes now ask the real engine** instead of carrying
  hard-coded verdicts, and that gating is what detected E-09 being fixed mid-wave.

### What is NOT closed in the harness, and matters

* **No per-seat, per-principal payout check.** Every money assertion is either an aggregate
  (conservation) or a number the canister itself recorded, and the two are never reconciled
  against each other per seat. A canister that records the right winner and physically pays a
  slice to a different seated player passes the whole suite. This is the harness reviewer's C4
  mutation and it is the same hole that hides FINDING 13.
* **`register_entries_are_all_still_needed` does not exist.** `documented.rs` documents it as the
  thing that stops a stale tolerance entry surviving a fix. `grep` finds no such test. The
  register is empty today so nothing is being excused, but the mechanism that keeps it empty is
  not there.
* **The magnitude leg of the classifier is test-only.** Register entries were bounded by
  `WORLD_TOTAL_E8S` — every e8 the harness ever mints — so `OverBound` could only fire above all
  the money in the world. Moot while the register is empty; a trap the moment an entry is added.
* **Never upgrades across a version boundary.** `World::upgrade` reuses `self.table_wasm`, so
  every "survives an upgrade" assertion is new-wasm-to-itself. That is what let FINDING 14
  through. This is the single most valuable missing harness capability.
* **`deposit_replay.rs` was in no make target for the whole wave.** Ten tests including the E-02
  fund-theft reproducer, auto-discovered by cargo, named by nothing. Now wired into
  `./scripts/dev.sh test`.

---

## 4. PROVEN — money is no longer destroyed at settlement, and an independent oracle says so

E-01, E-03, E-05 and the odd-chip rule (E-35) were fixed as one change, which was the right call:
they are four doors into one mistake (the pot was settled from a separate accumulator that could
drift from what players actually put in). The payout basis is now the players' own contributions,
`state.pot` is a cross-checked redundant number that cannot move a chip, and a seat vacated
mid-hand keeps its stake in the basis.

The check that makes this believable is not the fix, it is the oracle: `tests/settlement` derives
what each seat is **owed** from the rules of poker without touching `build_side_pots` or
`determine_winners`, drives the real canister on PocketIC, and diffs per-seat chip deltas.
Measured this pass, against `395696359b…`:

| | |
|---|---|
| deliberate hands | **17 settled, 0 disagreements**, 0 chips destroyed, 0 misdirected |
| classes reached | all 17, including 3-way chop (3), odd chip placed (4), folded money above a short all-in (3), seat vacated mid-hand (2), fold-out no showdown (2) |
| randomised sweep | 24 hands, 0 disagreements |
| exact-deal search | 97 rewind-and-re-deal attempts to buy the named hands |
| pinned reproducers | `pinned_e01_…`, `pinned_e05_…`, `pinned_odd_chips_now_go_one_each_clockwise` — all now pin the per-seat DIFF vector at zero, not merely `!agrees` |

The oracle's own reviewer had found it reporting rather than gating; the payout agent closed that
(`Bench::gate` fails the test that ran the hand, allowlist empty). My mutations N8/N9/N10 are the
independent check on whether that actually holds — see the table in §3.

---

## 5. NOT PROVEN — the right *player* gets paid

Wave 1 wrote: "Ranking is settled. What is NOT settled is whether the right player gets PAID."
Wave 2 answered half of that and re-opened the other half.

**Answered:** the right *seat* gets the right *amount*. Seventeen hard classes, an independent
oracle, zero disagreements.

**Re-opened, by this wave's own fix:** `principal_of()` resolves a payout's owner from the
**seat**, not from the stake. A departed player's refunded stake is therefore credited to whoever
has since taken their chair, and the `(Some(occupant), Some(owed)) if occupant != owed` arm written
to prevent exactly that is dead code, because both sides of the comparison are read from the same
seat. It conserves every chip, so no conservation invariant sees it; the oracle diffs per seat, and
the seat is paid correctly, so the oracle does not see it either. Both ingredients — the
refund-to-escrow branch and a chair re-occupied mid-hand (E-36) — already execute against the real
canister in a single fuzz run. Full write-up and reproducer:
[SECURITY-FINDINGS.md FINDING 13](SECURITY-FINDINGS.md).

This is the single most important open item in the engine, and note *why* it is open: the
otherwise-excellent settlement oracle was built to compare seats, so the entire class of
"right amount, wrong owner" defects is outside its field of view by construction.

---

## 6. NOT PROVEN — that the canister can be upgraded at all

Two agents each added a persisted field, neither of which is a Candid-compatible record-field
addition, and neither could see the other:

* `PersistentState::deposit_watermark: u64` — makes `stable_restore` fail outright, so an upgrade
  from pre-wave-2 state is **rejected**. Loud, and therefore safe.
* `TableState::departed_stakes: Vec<DepartedStake>` — nested inside `opt TableState`. Candid
  decodes an `opt` whose inner value cannot be read as **null**, so this one makes the entire table
  arrive as `None`, after which `post_upgrade` re-inits an empty table and **every seated player's
  chips and the live pot are destroyed with nothing in the log.**

The two mistakes hide each other: the first makes the upgrade fail before the second can bite.
**Fixing only the first — which is precisely what the deposit reviewer recommends, having proven
it works — converts a rejected upgrade into silent chip destruction.** Measured with the exact
candid version the canister links; the observed output is in
[SECURITY-FINDINGS.md FINDING 14](SECURITY-FINDINGS.md).

Landed this pass: a flat-scalar `opt` digest of the table in `PersistentState`, and a
`post_upgrade` that panics if the digest says a table was saved and `table_state` came back null.
That converts the silent case into a rejected upgrade for **any** future nested-field mistake, not
just this one. It is a guard rail, not the fix. The fix (both fields to `opt`, plus a real
`upgrade_from(old_wasm)` test) is wave-3 work and is ranked first.

---

## 7. NOT PROVEN — the betting rules

Two of the three fixes in this area are good and one is wrong, and its own test asserts the wrong
rule by name. Verified by reading the code against both rulebooks and by the reviewer's executed
reproducer:

* **E-31, the wedged table, is genuinely fixed** and was a real hazard: nothing in the canister
  called `check_timeouts` (no `ic-cdk-timer`; the frontend polls it), so an expired action timer
  left `action_on` pointing at a seat that could not act and the only remaining tool was
  `admin_reinit_table`, which strands chips (E-07). One shared resolver, called from both doors,
  is the right shape.
  **But** it now runs *before* the whose-turn check, so a message sent out of turn on the flop can
  be accepted and applied on the turn — a wager authorised against one board committed against a
  different one. Fixed this pass by refusing (not applying) any action whose street changed while
  the stale timer was being resolved; the table still unwedges, the stale wager does not land.
* **E-30, the incomplete all-in, is fixed in the common case and wrong in the case both rulebooks
  spell out.** TDA 47-A and Robert's Rules both say *cumulative* short all-ins that together add up
  to a full raise **do** reopen the betting; the code implemented "closed if facing anything at
  all". The comment block above the guard states the correct rule and the line below it implements
  a different one. Corrected this pass to `amount_owed < state.min_raise`.
* **E-09, `poker_core` input validation, is fixed** and the trap-rather-than-`Result` choice is
  argued well: inputs come from the canister's own deck, a trap rolls the message back and moves no
  money, and a `HandRank` for a hand that does not exist pays the wrong player irreversibly.

---

## 8. NOT PROVEN — the client tells the truth about money

The frontend work closed the thing that mattered most for safety: **a local build can no longer be
wired at the mainnet canisters** (bare `npm run build` used to fall through to the mainnet ids in
the repo-root `.env`). Four builds were used to demonstrate that, and each mainnet id now appears
in the bundle exactly once, as display text.

What is not closed, and is the reason the screenshot harness cannot be treated as an oracle: every
scene asserts **presence** in the DOM, never **agreement** with the chain. That single gap produced
four substantive misses in one run, two of them quoted verbatim in the notes of a scene that
recorded `verified: true`:

* the table's headline **POT is displayed at 2× during every betting round** (the canister folds
  `current_bet` into `state.pot`, and the same screen's pot-odds strip disagrees with its own
  headline);
* the villain's revealed hand renders as **two blank cards** and the winning hand as **`0`** — both
  the same unwrapped Candid `opt` (`hole_cards[0]` is the whole tuple, `hole_cards[1]` is
  `undefined`);
* the canonical `deposit-desktop.png` is a **loading spinner**.

A client that holds real ICP and displays the pot at twice its size is a serious defect on its own
terms, independent of the harness. Both are in the register.

---

## 8a. NOT PROVEN — `make fuzz` passes

**The wave's own long gate is red, at its shipped settings.** `./scripts/dev.sh fuzz` runs 9 seeds x
600 hostile steps against the real canister and the real ICP ledger. Seeds 1-4 are clean. **Seed 5
reports 2 blocking findings and 2,000,000 e8s stranded.**

```
money-fuzz: seed 0xc1b1576c1105 finished: 11 hands, 4 upgrades, 2 blocking finding(s),
            0 documented finding(s), worst stranded 2000000 e8s
```

The payout work reported "a 3-seed x 600-step hostile fuzz, 0 blocking findings". The default is
**9** seeds. A wave that runs a third of its own gate and reports zero has not run the gate.

The finding is real and I fixed it: `leave_table` reduced `state.pot` through
`return_uncalled_bet` without the paired `refresh_side_pots` that both other call sites have, so the
side pots a player is SHOWN stopped summing to the pot they are shown. No money moved wrongly —
`plan_payouts` rebuilds the layering from the contributions and never reads `state.side_pots` — but
the engine's two accounts of the pot disagreed, and that is correctly blocking. Full write-up:
[DEFECTS.md E-39](DEFECTS.md#e-39).

Depth matters too, and here the payout reviewer was right: at `MONEY_FUZZ_STEPS=1200`, **seed 1
alone** reported **6** blocking findings against the pre-fix build. Their claim was about a different
seed family; it reproduces on the default seeds.

**Not verified: that the fuzz is green after the fix.** The concurrent edits described in §0 landed
before I could re-run 9 seeds against a stable tree. `make fuzz` on the committed tree is the second
number to collect after M7.

---

## 9. The honest state of the trust story

| claim a player might make | can they check it today? |
|---|---|
| "the deck was fixed before any card was seen" | **yes.** `SHA256(seed)` is published before the deal and checked. |
| "the cards I got follow from the revealed seed" | **yes, by a stranger, in any language.** Proven on the shipped module, 53/53 cards, five hands, two of them discriminating. |
| "the seed was unpredictable" | **no.** That is trust in the subnet's `raw_rand`. Correctly disclaimed. |
| "the best hand won" | **yes for ranking** (wave 1: all 2,598,960 five-card hands vs three evaluators, zero disagreements) and **yes for the amount at each seat** (wave 2: the oracle, 17 classes). |
| "the money reached the right *person*" | **no.** FINDING 13. |
| "my chips survive a canister upgrade" | **no.** FINDING 14. |
| "the number on my screen is the number in the canister" | **no.** The pot is displayed at 2×. |
| "nobody takes a rake" | **yes**, and it is now checked by `awarded_equals_payout_basis` rather than assumed. |
| "the side pots on my screen add up to the pot on my screen" | **not until E-39's fix is verified.** `make fuzz` found them drifting after a mid-hand departure. |

## 9a. What this pass changed, and what proves each change

Nine edits, all of them reconciliation or a gate. Every one has a test that goes red without it.

| change | file | proven by |
|---|---|---|
| table-state integrity digest + `post_upgrade` refusal | `lib.rs` `PersistentState`, `post_upgrade` | M7; and the guard's limit is stated in FINDING 14 (it cannot fire on `801aa79` state, which has no digest) |
| cross-version upgrade capability | `wasms.rs`, `world.rs`, `invariants/upgrade_across_versions.rs` | M7 green on the shipped tree (upgrade refused, nothing lost) and RED on the half-fix (`DESTROYED SEATED CHIPS: 400000000 -> 0`) |
| TDA 47-A: closed to raising iff `amount_owed < min_raise` | `lib.rs` `apply_player_action` | `tda_47a_cumulative_short_all_ins_must_reopen_the_betting_to_seat_0`; reverting the one line makes it and only it go red |
| an action whose street changed under a resolved timer is refused, not applied | `lib.rs` `apply_player_action` | `an_out_of_turn_message_is_not_applied_to_a_street_the_player_has_not_seen`; reverting makes it and only it go red |
| `WARNING:` lines are findings; E-36's is a NAMED tolerance | `invariants/relational.rs`, `documented.rs` | `a_warning_line_is_a_finding_and_e36s_is_a_tolerated_one`, `an_unregistered_warning_line_blocks` |
| `register_entries_are_all_still_needed` now exists | `invariants/classifier.rs` | the test itself; `documented.rs` had documented it since it was written |
| `dr02` retries a rate-limited block index | `deposit_replay.rs` | the assertion that a block cannot stay rate-limited past three advances |
| `deposit_replay` and the settlement oracle are in the default gate | `scripts/dev.sh` | N11 dies only in the settlement oracle and a host test; `invariants` and `regressions` stay green with it installed |
| four E-09 markers promoted from red-on-purpose to ordinary gates | `fast_subset.rs`, `scripts/dev.sh` | `make known-defects` now reports 1 of 1 still present instead of shouting about four fixed ones |

Not changed, deliberately: **E-37** (FINDING 13). It is a fund misdirection and the project's rule
is to write those up prominently rather than quietly patch them. It also needs the owner to travel
with the contribution rather than be re-derived from the seat, which is a design change in another
agent's region, not a reconciling edit. It is ranked first-equal for wave 3.

---

## 10. Ranked wave-3 queue, and whether look-and-feel can start

See the ranked list at the end of [DEFECTS.md](DEFECTS.md#wave-3-queue). The short version:
**do not start the look-and-feel loops yet.** Two items have to land first, and both are small
relative to what this wave already built:

1. **FINDING 14** (both persisted fields to `opt`, plus `upgrade_from(old_wasm)`). Until this
   lands, the deposit fix — the fix for the only proven fund theft in the project — cannot be
   deployed by upgrade, and the obvious workaround destroys chips.
2. **FINDING 13** (carry the owner with the stake; one gate that asserts on principals). Until this
   lands, "the right player gets paid" is false, which is the one sentence the whole engine exists
   to make true.

Item 3 on the list — the 2× pot display — is genuinely a look-and-feel-adjacent fix and can be
done in the same breath as the UI work, provided the screenshot harness starts asserting
agreement with `get_pot()` rather than presence of an element. That change is what turns wave 3's
screenshots into evidence instead of decoration.
