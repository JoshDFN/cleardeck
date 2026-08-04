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
the harness compiles the canister it tests, six of seven seam mutations die where seven of seven
used to survive, and an independent settlement oracle agrees with the engine on all 17 hard
settlement classes. **But the question "does the right *player* get paid" is still open**, and it
is open in a sharper way than before: wave 2 closed money being *destroyed* and opened money being
*misdirected to the wrong principal*, on the same code path, in a form every gate it built is
blind to. Two agents also each added a persisted field that makes an upgrade from existing state
fail — and the obvious fix for one of them silently destroys every chip at the table. **The engine
is not ready to be left alone yet.** Details, and a ranked list, below.

---

## 1. The binary everything below was measured against

```
src/table_canister/src/lib.rs -> wasm32-unknown-unknown/release/table_canister.wasm
sha256 395696359b6d1a6e185fca9e2e051bbb6c8413b62e1545aae55c66d361d8342f   (2,314,067 bytes)
```

Reproducible: three consecutive `rm` + `touch` + `cargo build -p table_canister --target
wasm32-unknown-unknown --release` produced that hash three times.

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
3. Dealt **five real hands** on the real table canister wasm `395696359b…` under PocketIC with the
   real ICP ledger, capturing only what a player can see (`get_shuffle_proof`, `get_my_cards` as
   that player, `get_community_cards`).
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
hand, or the deal seam stays as unprotected as the shuffle was.

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

<!--MUTATION-TABLE-->

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
