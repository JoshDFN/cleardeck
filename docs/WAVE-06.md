# Wave 6 — the coherence pass, and the honest answer

> ## THE ANSWER IS STILL NO.
>
> A second independent auditor was run **after** this wave's fixes, on the running system, given
> the repo as a stranger finds it and told to treat every document in this project as a marketing
> claim until verified. Its verdict, verbatim:
>
> > **"NO — I would not tell a friend their money is safe here."**
>
> The first auditor's verdict, for comparison:
>
> > **"No. I would not tell a friend their money is safe here, and the reason is not the poker."**
>
> **The reason is still not the poker.** Everything the first auditor found is closed. I drove the
> two that touch money myself, on the module built from this source, rather than reading the
> write-ups: the fund lock (§2) and the free-showdown cheat (§3, REG-09). The other two — the
> fairness endpoint and the identifiable artifact — I checked only for presence on the Candid
> surface and via the suites in `./scripts/dev.sh test`, which are their authors' own gates. What
> is blocking, precisely:
>
> | # | blocker | why it blocks |
> |---|---|---|
> | 1 | **One controller call destroys every seated player's chips.** `reset_table` and `admin_reinit_table` are byte-identical bodies that build a fresh `TableState`. Re-measured this wave: **4.00000000 ICP destroyed**, canister left holding 40 ICP against 36 ICP of claims, `admin_restore_balance` deliberately removed so nobody can undo it. **Every invariant in the project stayed silent, including M9.** ([FINDING 07](SECURITY-FINDINGS.md#finding-07)) | a labelled recovery button deletes 100% of a table's deposits, reachable by one honest mistake |
> | 2 | **`cash_out` of a stuck hand walks away from your stake and says nothing.** Both players cashed out, both withdrew everything the canister admits to owing, and the canister was left holding a live pre-flop hand with a 3,000,000 e8 pot and zero players. Recoverable only via `abandon_stuck_hand`, which nothing on the withdrawal path names. ([FINDING 18](SECURITY-FINDINGS.md#finding-18)) | composes with #1 into permanent loss with no malice at any step |
> | 3 | **Nothing on chain moves the game.** `ic-cdk-timers` is declared and `set_timer` appears nowhere in `src/`. Every clock waits for an external caller. ([FINDING 19](SECURITY-FINDINGS.md#finding-19)) | liveness of a fund-holding canister depends on somebody having a browser tab open; and cycles exhaustion would stop every withdrawal at once, with no monitoring anywhere |
> | 4 | **A hand can settle with no live claim on it and refund every stake to the players who FOLDED.** Found by this pass, at the seam between two of this wave's own fixes. ([FINDING 17](SECURITY-FINDINGS.md#finding-17)) | the fold-out winner loses the pot to an ordinary disconnection, silently |
> | 5 | **The build-verification story does not close.** `verify-build.sh --local` gives 6/6 MISMATCH against the canisters `local-up` itself produces; the reproducibility work is real and is uncommitted. | "the code running is the code in this repo" is the load-bearing trust claim, and today a stranger who follows the instructions gets NOT VERIFIED on every path available to them |
>
> **Nothing in that list is fund theft.** #1 and #2 are fund loss. #1 is the largest unfixed
> fund-loss path in the project and it has now survived four waves.

---

## 1. What this pass was, and what it found first

Three agents edited `src/table_canister/src/lib.rs` this wave in supposedly disjoint regions: the
settlement/evaluator path, the status/eligibility logic, and `verify_shuffle` plus the history
wiring. Reconciling that file was the job. **All three fixes are present, and none of them undoes
another** — but two of them meet at one predicate, and where they meet there is a defect that
neither owner could see from inside their own region.

That defect is [FINDING 17](SECURITY-FINDINGS.md#finding-17), and it is the whole argument for
doing coherence passes:

* the **status/eligibility** agent made participation ask `is_in_hand`, so a dropped client is no
  longer skipped by the betting round — it is offered the action and **folded by its own clock**;
* the **settlement** agent gave `plan_payouts` a safe fallback for corrupt states — *"hand every
  seat back exactly what it put in"* — replacing an early return that destroyed the money;
* `is_in_hand` accepts a seat that is `Active` and holds **no cards** (a mid-hand arrival,
  docs/DEFECTS.md E-36), and `live_claims` does not.

Put those together and `count_active_players(state) == 1` becomes true with only a *cardless* seat
left, `end_hand_single_winner` runs a settlement whose `live_claims` is EMPTY, and the hand is
un-played. Measured on this tree:

```
alice v bob heads-up; a real 52,000,000 e8 pot
carol calls join_table(2) mid-hand   -> Ok      (an ordinary call)
carol calls sit_in()                 -> Ok      (an ordinary call)
alice folds                          -> Ok      (ordinary poker; BOB HAS WON THE HAND)
bob's client drops; his own clock expires
check_timeouts()                     -> PlayerTimedOut(1)

FINAL phase=HandComplete pot=0
  seat0 chips=200000000 folded=true    <- got their blind back
  seat1 chips=200000000 folded=true    <- WON the hand, paid nothing
  seat2 chips=200000000 cards=false
```

No trap. No `CRITICAL:` line. Exact conservation. **M1 through M9 silent and the M9 drain clean.**

Neither owner was wrong. The status agent's fix increased the traffic through the one door E-36
leaves open; the settlement agent's fix turned what used to be destruction into a silent refund.
It is only visible from above both.

It is **not a regression** — before this wave the identical state destroyed the money instead — but
the property this wave claims, that the table can no longer be wedged into a state the rules cannot
describe, is established **only for the states that TRAP.** The states that settle quietly are as
invisible as they were before.

### The one written claim I deleted

`is_in_hand`'s doc comment said of the cardless seat: *"including it changes nothing; it holds no
cards, so `live_claims` still refuses to pay it."* True of the payout, false of everything upstream
of it, and it is the sentence that stopped anyone looking — the same shape as `apply_payouts`'s
"recoverable" comment that the unbrick agent corrected for the same reason. It is now replaced by
the measurement above. **The predicate itself is unchanged**: see §6 for why.

---

## 2. The first auditor's fund lock, reproduced by me on the fixed build

This was the single most important regression to check, so I drove it myself rather than trusting
`fund_reachability.rs`, and I drove it **by real silence** rather than by the `sit_out()` shortcut
the builders' own test uses — because `sit_out()` is a call whose mid-hand behaviour this very wave
changed, and the auditor's table got where it got because a client stopped sending heartbeats.

`tests/money_safety/tests/wave6_coherence.rs::probe1`, three funded seats, carol's client goes
quiet and nobody ever calls anything on her behalf:

```
after silence:
    seat0 chips=201000000 folded=false cards=true status=Active
    seat1 chips=200000000 folded=true  cards=true status=Active
    seat2 chips=199000000 folded=true  cards=true status=Disconnected

check_timeouts  x3 principals   no trap
leave_table     x3 principals   no trap
player_action   x8              no trap
drain: every player's money back on the ledger, owed_after = 0
```

**PASS.** Every door the auditor found shut is open, on the module built from this source. The
mechanism is right too: the Disconnected seat was not skipped, it was offered the action and folded
by its own clock, and seat 0 won the pot properly.

`abandon_stuck_hand` also works and is not decoration — the fuzzer exercised it unprompted:

```
CRITICAL: hand 6 was ABANDONED as unmovable at phase turn: no message could advance it.
6000000 e8s across 3 stakes returned to the players who put them in; nobody won the hand.
```

---

## 3. Gates

Module under test: `5e07edf6fdfdb60f96471dcfc6adab226d1c14746d12b291d952c4eeec1a969c`. The final
artifact is `306caef414578a066da448c1f23f501183f983077aee28e82d7847d9496a2b35` — the difference is
**44 comment lines and nothing else** (`diff` against a pre-edit `cp -Rc` copy has zero non-comment
hunks; the hash moves because panic-location line numbers live in the data section, docs/DEFECTS.md
H-29). Both were gated.

| gate | verdict |
|---|---|
| `./scripts/dev.sh test` | **GREEN**, exit 0, "all fast gates green" — **after** the REG-09 repair below. It was RED before it, and had been for a whole wave |
| `make settlement` | **GREEN**. 22 + 5 + 9 + 3 tests, **0 hands where the engine and the oracle disagreed** |
| `make fuzz-default` | **GREEN**. 3 seeds at the fuzzer's own defaults, 14 hands, 11 upgrades, **0 blocking findings, 1 documented (E-36), worst stranded 0 e8s** |
| `make known-defects` | **1 of 1 still red on purpose** (E-13 `detect_straight`). No marker has silently gone green |
| `make hygiene` | **GREEN**. All four notices present in README and frontend source, no-rake property stated, nothing weakened since `ceacc37` |
| `make selftest` | **GREEN**. 7 mainnet ids in the denylist, every hostile shape refused, no false positives |
| money suite, sha256 announced | on every sub-run, both times. `sha256=5e07edf6… bytes=2390603 toolchain=1.90.0`, then `sha256=306caef4… bytes=2390536` after the comment edits. 45 + 6 + 9 + 6 + 1 + 1 green each time |
| **seam mutations** | **6 of 7 die. NOT DOWN.** Same survivor (mutation 5, the dropped self-report). Run on a `cp -Rc` copy of the pre-comment tree (`5e07edf6…`); `lib.rs` asserted byte-identical after the run |
| **shots sweep + pixel gate** | ❌ **COULD NOT RUN** — see §4 |

### The seam count, and the one mutation that had to be re-translated

Two of the seven wave-4 anchors no longer matched this tree, and both mismatches are themselves
information:

* **mutation 4** (`&[]` passed as the contributions) now matched **twice**, because wave 6 gave
  `refund_every_stake` the same line as `plan_payouts`. Disambiguated to the `plan_payouts` site,
  which is what wave 1 meant.
* **mutation 6** (`evaluate_hand` stubbed to `RoyalFlush`) matched **zero** times, because
  `evaluate_hand` is no longer imported into `lib.rs` at all — that IS the FINDING 15 fix. Its
  MEANING is "the evaluator says everybody has the nuts", so it was re-pointed at
  `poker_core::try_evaluate_hand` on the settlement path. **It still dies** (8 of 39 workspace
  tests).

| mutation (wave-6 site) | verdict | what convicted it |
|---|---|---|
| 1. `refresh_side_pots` made a no-op | **died** | `seam_a`, `seam_b` |
| 2. every stake halved in `plan_payouts` | **died** | `cargo test --workspace` (20 of 39) |
| 3. the breakdown written into a throwaway `Vec` | **died** | `seam_a`, `seam_b` |
| 4. `&[]` as the contributions in `plan_payouts` | **died** | `cargo test --workspace` (14 of 39) |
| 5. the self-report line dropped | **SURVIVED** | — (equivalent mutant, unchanged since wave 2) |
| 6. `try_evaluate_hand` stubbed to `RoyalFlush` | **died** | `cargo test --workspace` (8 of 39) |
| 7. every hand dealt from `b"CONSTANT"` | **died** | `seam_c`, `seam_honest_play…` |
| | **6 of 7** | same survivor as waves 2, 3 and 4 |

### REG-09: a gate that was red for a whole wave

`./scripts/dev.sh test` was RED at the start of this pass, on exactly one test:

```
reg09_one_timeout_with_no_heartbeats_runs_the_whole_board_out_and_settles ... FAILED
  assertion `left == right` failed: ONE timeout ended the hand for everybody.
  left: Turn / right: HandComplete
```

That test **pinned** E-06 as still present. E-06 was fixed in wave 5, so the pin failed — and its
own message said what to do: *"If this now leaves the hand in progress the interaction has been
fixed — update docs/SECURITY-FINDINGS.md and this test together."* Nobody did, for a wave.

**That is the worst possible state for a gate.** A suite everybody has learned to expect one
failure from cannot convict the second. So it was inverted, renamed
`reg09_one_timeout_folds_only_the_seat_on_the_clock`, and now asserts on the identical sequence:
exactly one seat folded and it is the one on the clock; the hand still in progress with two live
seats; the board **4 cards, not 5**; nobody marked `Disconnected` by a 31 s lull at all; nothing
destroyed; and the hand eventually ending by fold-out on the short board, paying the last player
standing the whole 206,000,000 e8s. [FINDING 12](SECURITY-FINDINGS.md#finding-12) was updated in
the same change, as instructed.

**It goes RED again with the defect restored.** Verified in a `cp -Rc` copy by reverting both
halves (`DISCONNECT_TIMEOUT_SECS` 90 → 30, `is_in_hand` back to `status == Active`):
`PINNED FIX (E-06): one timeout must not end the hand for everybody … phase after: HandComplete`.

---

## 4. The notice check, measured on the rendered page — and the gate that could not run

HARD RULE 2: the unaudited-alpha disclaimer, the 18+ notice, the jurisdiction warning and the
no-rake property must be **on screen**, not merely in the DOM, at any viewport and on any view.
Grep does not discharge it. So I built a probe that measures something the repo's own harness does
not: after locating each phrase's real line boxes with a DOM `Range`, it hit-tests every line box
and then **samples the actual screenshot pixels** in those boxes and requires real luminance
contrast. A notice painted in the background colour passes every existing check and fails this one.

**What passed** (`$SCRATCH/notice_probe.mjs`, 1440×900 and 390×844, against the real deployed
asset canister on the local replica):

```
desktop  / signed-out landing, scrolled top AND bottom, and the signed-in lobby
portrait / signed-out landing, scrolled top AND bottom, and the signed-in lobby
  all five phrases: on screen, unoccluded, contrast 5.25 – 10.2, font 12 px
```

**That result is conditional, and the condition is how I found the next thing.** The probe was
flaky across runs on exactly one view — the portrait lobby — reporting three phrases occluded on
some runs and clean on others. Rather than average it away I chased the nondeterminism, and it was
not noise: an error toast that auto-dismisses. So read the block above as *"…and the signed-in
lobby, whenever the app is not showing an error"*.

**What failed — [E-52](DEFECTS.md#e-52), and it is a real HARD RULE 2 violation.** The app's own
error toast is `position: fixed; z-index: 100` and 534 px tall. Forced deterministically through
the app's own error path (abort the agent fetches it already makes, then sign in):

```
### portrait 390x844  toasts=1
   toast toast.error rect=[-117, 80, 624, 534] z=100 pos=fixed
   COVERED  "Unaudited code with known bugs"             toastHits=3/9
   COVERED  "your funds are NOT safe"                    toastHits=3/9
   COVERED  "illegal in many jurisdictions"              toastHits=3/9
   COVERED  "18+ only"                                   toastHits=3/9
   COVERED  "No rake is taken from any pot on any table" toastHits=9/9   <- totally covered,
                                                         and its only other carrier is offscreen
### desktop 1440x900  readable — the banner is wide enough that the 720 px toast leaves it clear
```

At 390 px the toast is **624 px wide starting at x = −117**: it overflows the viewport on both
sides, so there is no clear column. Its top edge is y = 80 and the no-rake line sits at y = 135, so
**any toast taller than 55 px covers it** — 534 px is merely what a long message produces.

Why no gate saw it: `protected-notices.mjs` is a good probe and it hit-tests the right pixels, but
no scene in `tools/shots/scenarios` raises a toast, and `occlusion.mjs` is written against
`.modal-backdrop`. The phrases are visible in every state the harness puts the app into. **The app
has a state the harness never puts it into.** That is the wave-4 lesson recurring one layer out:
the notices were measured on the rendered page, and the *set of rendered pages* was the blind spot.

### What I could NOT measure, and I am not going to pretend otherwise

**The table view and the deposit/withdraw modals were not measured at either viewport.**
`./scripts/dev.sh shots` aborts before capture:

```
FATAL: No local icp identity controls 46el7-ql777-77775-aaada-cai.
  controllers on the canister: nsp5q-aelxk-…      (= cyclepay-hotwallet)
  identities tried: cd-local-deployer, cd-alice, cd-bob, cd-carol, (default)
```

That is [E-53](DEFECTS.md#e-53): wave 6 taught `scripts/dev.sh` to resolve the controller from
`canister status`, and `tools/shots/lib/config.mjs` still hardcodes `cd-local-deployer`. **The
pixel gate, the occlusion gate and the protected-notice gate all live behind `shots`, so one stale
identity list disables three fund-adjacent gates at once** — and reports it as a fatal setup error
rather than a red gate, which reads as an environment problem rather than a hole.

The lobby was also empty after the documented bring-up (`get_tables -> (vec {})`), the second
auditor's finding; I restored it by hand as `cyclepay-hotwallet`, which is how I got the lobby views
above. I did **not** run `local-up --reset` to repair the rest: it destroys every other agent's
fixture, and three agents were live in this tree.

**One thing my own run broke, and how I repaired it.** `run.mjs` wipes `artifacts/screens/latest/`
at the start of a full run — correct in itself — but it wipes it *before* the first scene, and the
identity check that killed the run fires *inside* the scenes. So a run that captured nothing still
deleted the index, the manifest and every PNG. I rebuilt `latest/` from `artifacts/screens/fe72d46/`
(the last full run, 76 entries) with `artifacts/screens/fcfa4e8/` (the later partial) layered on
top, which is exactly what `latest/` is defined to mirror; `git status` shows the two tracked files
back to `M` rather than `D`. It is written up as part of [E-53](DEFECTS.md#e-53), because a harness
that destroys the previous evidence when it fails to produce new evidence is a hazard in its own
right.

**So the notice check is discharged for the landing page and the lobby at both viewports, and it is
NOT discharged for the table view or behind any modal.** The table view at portrait is precisely
where wave 4's failure was.

---

## 5. What is proven, and what is not

### Proven, by execution, on the module built from this source

* The first auditor's fund lock is **gone**. Reproduced by real silence, not by the builders'
  `sit_out()` shortcut: no update traps for any principal at any point, and every player's money
  reaches the ledger. (The wave's own write-up claimed every door was shut for every player; its
  critic showed the disconnected seat's own doors were open. I do not restate that claim — what I
  measured is that no door is shut for anybody now.)
* A `Disconnected` seat is **offered the action and folded by its own clock**, not skipped. The
  free showdown (FINDING 16 / E-32) is closed, and E-06 with it, structurally: the heartbeat
  threshold (90 s) is now longer than every deployed action clock (30/45/60 s).
* `abandon_stuck_hand` works, refunds every stake to its own funder, and is reached by the fuzzer
  in ordinary hostile play.
* The settlement path cannot trap: `evaluate_hand` is not imported into `lib.rs` at all, and the
  greppable rule is asserted on every run.
* The independent settlement oracle finds **0 disagreements** across every hand it drives.
* The payout seam is still **6 of 7** mutation-covered, with mutation 6 re-pointed at the new site.
* The four protection notices and the no-rake property are on screen, unoccluded and legible —
  measured on the rendered pixels, not grepped — on the landing page at scroll 0 and at the bottom,
  and on the signed-in lobby, at both viewports, **whenever the app is not showing an error toast**
  (E-52).

### NOT proven

* **That a stranger's money is safe.** Two independent auditors, both no.
* That the notices survive the table view or any modal — not measured this pass (§4).
* That the toast occlusion is the only one of its kind; I found it by asking a question no gate
  asks, and I only asked it about toasts.
* That the reproducible-build chain works from a fresh clone. It works, and it is uncommitted; the
  branch `feat/world-class` has it and `main` does not, and the README names no branch.
* That the archive survives a failed `pre_upgrade`. Its own `stable_save` failure is logged and the
  upgrade proceeds, rolling records back while the canister goes on answering
  `records_are_append_only = true`.
* That the published fairness record is always sufficient to reproduce the board. The second
  auditor followed it on a real hand and got a wrong flop.

---

## 6. Why FINDING 17 was not fixed in this pass

The fix is one line — drop the `|| p.status == PlayerStatus::Active` disjunct, so participation
really is the same predicate as eligibility. I did not take it, and the reasoning should be on the
record so the next owner can overrule it cheaply:

1. it is a **behaviour** change to the seating path, in a file three other agents were editing
   concurrently, during a pass whose remit was minimal reconciling edits;
2. docs/DEFECTS.md E-36 documents that landing it requires deleting the `E-36` register entry in
   `tests/money_safety/src/documented.rs` **and** the tolerated `"carries both a live stake and a
   departed stake"` substring in the same change, or `register_entries_are_all_still_needed` fails
   on purpose. That is one change by one owner, not a drive-by;
3. and HARD RULE 3 says a fund finding goes in SECURITY-FINDINGS prominently rather than being
   quietly patched. It is [FINDING 17](SECURITY-FINDINGS.md#finding-17), E-36 is raised from medium
   to high, and the false comment that hid it is deleted.

---

## 7. For the lead: is this something a stranger would trust with money?

**No. Not yet, and not because of the poker.**

The poker is in good shape and I checked rather than assumed: the shuffle is genuinely verifiable
(both auditors reproduced real hands from the spec alone, one of them predicting the turn and river
from a commitment recorded while only the flop existed), the settlement oracle finds no
disagreements, conservation holds through every hostile sequence either auditor or this pass could
construct, the deposit anti-replay held against every attack tried, and both auditors drained real
tables to exactly zero. Wave 6 closed every defect the first auditor named; I drove the two that
move money myself on the module built from this source, and took the other two on their authors'
own gates.

What a stranger is being asked to trust is not the poker. It is custody, and custody has one
unfixed hole big enough to swallow a whole table: **one controller call, on a button labelled
"recovery", destroys 100% of the seated chips and nothing can give them back.** That is not a
theoretical risk model; it is four measured ICP on this tree this afternoon, with every invariant
in the project reporting green while it happened.

### The shortest path to yes

In this order, because the first two compose into permanent loss and the rest do not:

1. **Make `reset_table` and `admin_reinit_table` incapable of destroying money.** Refuse unless
   chips and escrow are both zero, or return every chip to escrow first and leave `BALANCES`
   strictly untouched. Both functions — fixing one is how this survived four waves. *Small; the
   bodies are three lines each.*
2. **Make an outstanding pot stake visible and self-serve.** `cash_out`'s `Ok = 0` should read
   `Ok = 0, 298_000_000 still in a stuck pot`; surface it in `get_balance` or the table view; have
   `withdraw`'s refusal name `abandon_stuck_hand`. *Strings only; touches no money path.*
3. **Add an on-chain timer** so a table settles without a browser tab, and **a cycles floor with
   monitoring**, because a frozen canister rejects every withdrawal at once and nothing on chain
   would notice. *The dependency is already in `Cargo.toml`.*
4. **Close FINDING 17** — the one-line predicate change plus its register entry, as §6 describes.
5. **Commit the reproducible-build work and merge it to the default branch**, and put the
   money-safety suite and the settlement oracle into CI (docs/DEFECTS.md H-23). Today every
   fund-safety result in these documents comes from a harness no CI job invokes, and a stranger
   following the README's own Step 1 lands on a tree that cannot perform the build.
6. **Fix E-52 and E-53 together**: give the alpha banner a higher stacking context than `.toast`,
   add a shots scenario that raises a toast on every view, and port `resolve_controller_identity`
   into `tools/shots`. Until E-53 is fixed the pixel gate cannot run at all, so E-52's fix could
   not be gated even if it were made.

Items 1 and 2 are hours of work between them and they are the difference between "an unaudited
alpha with known bugs" — which the product says honestly and prominently, at every viewport I could
measure — and "an unaudited alpha that can delete your balance". **Do 1 and 2 and the honest answer
to a friend changes from "no" to "not yet, and here is exactly why".** It does not change to "yes"
until 5 lands, because until then nobody can check that the code holding the money is the code in
this repo.

---

## Reproducing everything in this document

```
cd /Users/josh/Desktop/cleardeck
./scripts/dev.sh doctor
./scripts/dev.sh test                 # green; wasm sha256 printed on every sub-run
./scripts/dev.sh settlement
./scripts/dev.sh fuzz-default
./scripts/dev.sh known-defects
./scripts/dev.sh hygiene
./scripts/dev.sh selftest

# seam mutations, on a cp -Rc copy; lib.rs asserted byte-identical afterwards
python3 $SCRATCH/seam_mutations_w6.py $SCRATCH/seam6

# the coherence probes: the reproducers for FINDING 17, FINDING 07 and the fund lock.
# IN THE REPO, because a finding whose only reproducer lives in a scratch directory
# is an anecdote. NOT wired into any gate -- see the file's own header for why, and
# for what to change when each finding is fixed.
cd tests/money_safety
CLEARDECK_TABLE_WASM=../../target/wasm32-unknown-unknown/release/table_canister.wasm \
  cargo test --test wave6_coherence -- --nocapture --test-threads=1

# the notice measurement, against the deployed asset canister
node $SCRATCH/notice_probe.mjs http://<frontend-id>.localhost:8077   # contrast + occlusion
node $SCRATCH/toast2.mjs       http://<frontend-id>.localhost:8077   # E-52, deterministic

# REG-09 goes red with the defect restored
#   in a cp -Rc copy: DISCONNECT_TIMEOUT_SECS 90 -> 30, and
#   is_in_hand -> !p.has_folded && p.status == PlayerStatus::Active
cargo test --test regressions reg09
```
