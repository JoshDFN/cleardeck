# Wave 1 — what exists, what it proves, what it does not

ClearDeck is a provably-fair Texas Hold'em implementation running entirely on the Internet
Computer. Its mainnet canisters custody **real ICP and real ckBTC**. Wave 1 built no features and
fixed no bugs on purpose. Its whole job was to establish ground truth: to make the engine's real
behaviour observable, so that wave 2 can change it safely.

This document is the honest state of that effort. It is deliberately unflattering where the
evidence is thin.

> The prioritised list of everything found is **[DEFECTS.md](DEFECTS.md)**. The fund-impact evidence
> is **[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)**. This file explains what the machinery is and
> what you may and may not conclude from a green run.

---

## 1. The honest state of the engine

**The engine destroys real user money in ordinary play, and the mainnet canisters are expected to
contain the bug.**

Every showdown that follows any post-flop betting pays the winner only the pre-flop pot.
`calculate_side_pots` is called unconditionally when the flop is dealt, which freezes the payout
breakdown; nothing recomputes it. Everything wagered on the flop, turn and river is debited from
stacks and credited to nobody, and because `withdraw` pays strictly against the caller's own escrow
and no administrative withdrawal exists, **those tokens can never leave the canister — not even for
a controller.** Reproduced on a live local replica, and again against the real ICP ledger wasm on
PocketIC. The Rust is byte-identical to commit `d461846`, so mainnet is expected to be affected;
confirming that needs a mainnet module hash, which was deliberately not read.

Three further confirmed routes reach the same terminal state (a stranded balance nobody can
withdraw): the published `notify_deposit` path, which can never credit a deposit at all; the
`admin_reinit_table` recovery tool; and, on the deployed `table_1` and `btc_table_1`, a
thirty-second lull, which runs the whole board out and settles.

One **fund-theft-class** primitive exists and is currently blocked by another bug: a single
on-ledger transfer can be credited to escrow twice. It is not exploitable today only because
`notify_deposit` fails before it can credit anything. **Fixing that decode bug on its own opens a
live path to mint withdrawable ICP.** See [E-02](DEFECTS.md#e-02).

What *is* sound, and was checked hard: **the hand evaluator.** All 2,598,960 five-card hands agree
with two independently-lineaged reference evaluators on category, sub-rank and complete ordering,
with zero disagreements, and the ordering agreement is a proof over all ~3.4 × 10¹² hand pairs
rather than a sample. Also sound: the ICRC-2 deposit path and the withdraw path are ledger-exact
to the e8 including the real 10,000 e8s fee; the withdrawal reentrancy guard works; hostile amounts
(`u64::MAX`, sub-min raises, out-of-turn, occupied seats) are refused rather than clamped; and state
survived 60+ real `--mode upgrade` upgrades including mid-hand.

**What nobody has established: whether the right player gets paid.** No harness connects the proven
evaluator to the actual payout — pot eligibility, tie chopping, odd-chip assignment. See §5.

---

## 2. What exists now

```
src/poker_core/                  NEW. 554 lines. The pure poker engine, extracted from
                                 table_canister so tests can reach it. Zero canister deps.
src/table_canister/src/lib.rs    4,470 lines (was 4,824). Still the state machine, the money and
                                 the ledger calls. Now `pub use`s poker_core's types; its
                                 extracted Candid is byte-identical to the baseline.
src/poker_core/tests/            40 unit tests + 15,688 committed golden vectors replayed on
                                 every run.

tools/differential/              NEW. Differential hand-evaluator harness. Own cargo workspace so
                                 the reference evaluators never enter the canister lockfile.
tools/shots/                     NEW. 2,377 lines / 20 modules. Screenshot harness: real UI, real
                                 canisters, 9 scenes x 2 viewports. HAS NEVER RUN (see §4).
tests/money_safety/              NEW. Money-safety harness: named invariants M1..M6 + a hostile
                                 sequence fuzzer, against the REAL table wasm and the REAL
                                 mainnet ICP ledger wasm on PocketIC. Own cargo workspace.
src/table_canister/tests/money_safety.rs
                                 The host-checkable subset of those invariants, no replica needed;
                                 this one IS in `cargo test --workspace`.

Makefile, scripts/dev.sh         NEW. One entry point for all of the above.
docs/DEFECTS.md                  NEW. The single prioritised defect register.
docs/DESIGN-BAR.md               NEW. 474 lines, 21 numbered look-and-feel bars. Partly WRONG —
                                 see H-14 in DEFECTS.md. Do not score against it yet.
```

Verified repo state, all reproduced in this pass:

| check | result |
|---|---|
| `cargo test --workspace` | **107 passed, 0 failed, 0 ignored** |
| `cargo build -p table_canister --target wasm32-unknown-unknown --release` | green; sha256 `e571570f…` |
| files wave 1 would add to git | **74 paths, 757 KiB, zero binaries, zero images** |
| third-party copyrighted images committed | **none** — the 438-file reference corpus lives outside the repo, in the session scratchpad |
| `README.md` vs `ceacc37` | **unchanged** |
| `src/cleardeck_frontend` vs `ceacc37` | **unchanged** |
| disclaimer / 18+ / jurisdiction / no-rake | **present and unchanged in both README.md and the app**, checked by `make hygiene` |

No test anywhere in the workspace is `#[ignore]`d. Six tests in `tools/differential` are: five are
known-defect markers written to FAIL until the engine is fixed (run them with `make known-defects`,
which inverts them), and one needs the full five-card sweep and runs under `make diff-full`.

### One place to run everything

```
make help            what all of this is
make doctor          toolchain, replica liveness, deployed ids, wasm identity   (read-only)
make local-up        bring the local stack up end to end                        (idempotent)
make test            FAST gate. No replica needed. 31 s measured.
make fuzz            LONG. 9 seeds x 600 hostile steps against a real replica.
make diff-full       exhaustive evaluator differential, all three references.
make shots           screenshot the real UI against the real local canisters.
make known-defects   the markers that are RED on purpose; shouts when one gets fixed.
make hygiene         no large/binary files; player-protection notices intact.
make selftest        prove the mainnet guard refuses every hostile argument shape.
make check           selftest + hygiene + test. What CI should run.
```

All logic is in `scripts/dev.sh`; the `Makefile` is a thin front door. Every `icp` invocation goes
through one wrapper that appends `-e local`, scrubs inherited `ICP_ENVIRONMENT`/`ICP_NETWORK`, and
refuses any argument naming a canister listed in `.icp/data/mappings/ic.ids.json`. That denylist is
**not** duplicated: `tools/shots/lib/ids.mjs` reads the same file, so adding a mainnet canister to
the project protects it in both places automatically. `make selftest` proves the guard fires — 17
refusals, 3 correct allows, verified.

---

## 3. What each harness proves — and what it does not

### 3.1 `tools/differential` — the hand evaluator

**Proves.** All 2,598,960 five-card hands run through the real `poker_core::evaluate_five_cards`
and checked against two independently-lineaged mature evaluators (`rs_poker` 5.0.0, lineage
OMPEval; `poker` 0.7.0, a Rust port of treys' Cactus-Kev + Senzee tables) on hand category (via an
explicit cross-library name map, never an opaque integer), sub-rank detail, and the **complete
ordering**. The ordering check is not sampled: hands are bucketed by our class in our class order
and each bucket must map to one reference class with the reference class strictly increasing, which
makes a clean run a proof over all C(2598960,2) ≈ 3.4 × 10¹² pairs in O(n). Plus a library-free
oracle: the textbook combinatorial count per category, which would catch a whole-category error
even if both references were wrong the same way. Plus 5,000,000 random seven-card hands through
`evaluate_hand` (the function `determine_winners` actually calls) and 4,000,000 explicit
`sign(our_cmp) == sign(ref_cmp)` pairs including realistic shared-board showdowns.

Reproduced in this pass: **zero disagreements, 7,462 five-card classes matching exactly,
`fund_impacting_classes=0`, verdict `AGREE_WITH_LATENT_MISUSE_FINDINGS`.** The harness has teeth:
`mutation-check.sh` breaks a copy of `poker_core` three ways and requires a conviction each time.

**Does not prove.**
* Nothing about **payouts**. It ranks hands. It says nothing about which seat is eligible for which
  pot, how ties chop, or how odd chips are assigned.
* The five "latent" degenerate-input findings are real defects ([E-09](DEFECTS.md#e-09)) but the
  *differential evidence* attached to them is fabricated: the `reference_says` strings are
  hard-coded literals and the references were never called. Two of the three are factually wrong —
  `rs_poker` happily ranks `Ah Ah Ah Ah Ah` and even a two-card hand. **No evaluator in this
  harness validates its own input**, so E-09 must be fixed at the `poker_core` boundary and cannot
  be delegated to a reference. See [H-05](DEFECTS.md#h-05).
* By default only **two** references run. The third lineage (`phevaluator`) needs
  `CLEARDECK_PHE_PYTHON`; `make diff-full` now sets it up (verified working, zero conflicts), but
  a bare `cargo run --release` skips it and says so.
* The exhaustive seven-card sweep (all C(52,7)) is opt-in and was **not** run in this pass.

### 3.2 `tests/money_safety` — the money

**Proves.** The real table canister wasm executes against the **real mainnet ICP ledger wasm**
(sha256 `a47a915e…`, pinned, and the harness refuses to run against any other module) installed at
`ryjl3-tyaaa-aaaaa-aaaba-cai` — the exact canister id the table canister hardcodes. So the
production deposit and withdraw paths run unmodified, with real ICRC-2 allowances and the real
10,000 e8s fee. Not a mock. Invariants M1..M6 are named assertion functions in one place, each with
a hand-written test, driven additionally by a seeded hostile-sequence fuzzer with a
delta-debugging shrinker over every money-moving update, time jumps past each engine deadline, real
mid-hand `--mode upgrade` upgrades, same-round concurrent submissions, silent players and
deliberately illegal input.

Reproduced in this pass: **10 invariant tests and 6 regression reproducers pass** against a
freshly built wasm. The long run (9 seeds × 600 steps = 5,400 real IC messages, 78 hands, 95
upgrades) reported zero blocking findings and produced seven new confirmed defects.

**Does not prove — and this is the most over-readable green light in the project.**
* **Blind to redistribution.** M1..M6 are conservation and settlement properties. If the engine
  pays the wrong player, the total is unchanged and every invariant still holds. That is exactly the
  shape of [E-05](DEFECTS.md#e-05).
* **A green fuzz run is compatible with a live house rake.** The violation classifier decides
  blocking-vs-documented by the **sign** of the delta. A planted 1% rake in `determine_winners`
  produced `test result: ok`, recorded only as a documented `FundDestruction`. Dropping a side pot
  passes the same way. See [H-03](DEFECTS.md#h-03).
* **The canister reporting its own inconsistency cannot fail a run.** Every `BUG:`/`CRITICAL:` log
  line is hardcoded to a non-blocking severity — including `pre_upgrade`'s
  `CRITICAL: Failed to save state to stable memory`, which is total fund loss on upgrade. See
  [H-02](DEFECTS.md#h-02).
* **It is not yet a regression gate.** Left to itself the harness reads whatever wasm is lying in
  `target/`, with no freshness check. With a 10x-credit theft bug in the source and a pristine wasm
  at that path, all 10 invariant tests report `ok`. `make test` and `make fuzz` work around this by
  building the wasm, passing it explicitly and printing its sha256; a bare
  `cd tests/money_safety && cargo test` does not. See [H-01](DEFECTS.md#h-01).
* **ckBTC is entirely uncovered.** `btc_table_1` uses `mxzaz-hqaaa-aaaar-qaada-cai` and the ckBTC
  minter, neither of which is installed. Everything measured is ICP.
* **Only one table shape is ever exercised**: `table_1`'s blinds and a 30 s action timeout.
  `table_2` (45 s, 0.05/0.10) and `table_3` (60 s, 0.10/0.20, 9 seats) are never instantiated. See
  [H-11](DEFECTS.md#h-11).

### 3.3 `src/poker_core` — the extraction

**Proves.** The pure engine was moved, not rewritten: extracted programmatically by brace-matching
from `git show ceacc37:…/lib.rs` with only `pub` prefixes added, so no derive, field name, variant
name or variant order could drift in transcription. `table_canister`'s extracted Candid is
byte-identical (same SHA1) before and after. Behaviour is pinned by 15,688 committed golden vectors
generated from the original code and replayed on every test run, plus a ~5.9M-case differential
against verbatim baseline code with zero divergences. Mutation-tested: 23 of 26 `poker_core`
mutants convict, and the three survivors were each proved to be genuinely equivalent mutants (one
of which is [E-15](DEFECTS.md#e-15), dead code in the side-pot split).

**Does not prove.** The **seam** — the code the extraction *wrote* — is not tested at all. Seven of
seven seam mutations survive with the whole suite green: `calculate_side_pots` made a no-op,
`state.pot / 2` passed as the pot, `&[]` passed for the contributions, `evaluate_hand` shadowed to
return a royal flush for every player, and `shuffle_deck` shadowed so **every hand is dealt from
the constant seed `b"CONSTANT"`**. The canister can stop using the extracted engine altogether and
the suite still reports green. `src/table_canister/tests/integration_test.rs` is comments only.
See [H-04](DEFECTS.md#h-04).

The side-pot maths was ported **bug-for-bug on purpose**, including the `f64` proportional-capping
branch and the early return that leaves stale pots untouched. `poker_core` is not a clean-room
implementation and must not be read as one.

### 3.4 `tools/shots` — the screenshots

**Proves nothing yet. There are zero screenshots.** The shared local replica's gateway on :8077 has
been dead for this entire wave and the harness deliberately does not restart it (other agents share
the replica). `artifacts/screens/` contains a README and a third-party asset cache. No PNG, no
video, no frame burst.

What is built and independently verified without a replica: all 20 modules load; Playwright 1.61.1
launches the cached chromium; stills come out at exactly 1440×900 and 390×844; the 4943→8077 port
shim starts, forwards and closes; the mainnet guards refuse `-e ic` and every ClearDeck mainnet id;
the frontend builder wires the **local** canister ids into the bundle and aborts if the local lobby
id is absent from the built JavaScript; and all 9 scenes are scripted end to end with setup driving
real on-chain state, staging driving the real UI, and verification cross-checking the DOM against a
fresh `get_table_view`. Nothing is mocked. Authentication uses the app's own local-dev login
(`Dev Login → auth.devLogin`, a deterministic Ed25519 identity) because the running network's
descriptor says `"ii": false`, so scripted Internet Identity is impossible here.

Two things must happen before it can run: the replica must come back ([T-04](DEFECTS.md#t-04) — the
on-disk state has no checkpoint height common to all five subnets, so resuming it will cost the
local ledger and the funded identities), and the `frontend` asset canister must be created
([T-07](DEFECTS.md#t-07)). `make local-up` does both.

One hole was closed in this pass: a scene whose on-chain verification came back `false` used to
write the **canonical** PNG filename anyway, into both the run directory and `latest/`, where a
reader browsing PNGs would take it as proof. Unverified scenes now write `UNVERIFIED-*.png`.
See [H-07](DEFECTS.md#h-07).

### 3.5 The reference corpus and `docs/DESIGN-BAR.md`

**Proves.** PokerNow was captured live by driving real Chrome at dpr 2 against a real public
play-money game and through PokerNow's own tutorial, and measured for real numbers rather than
impressions: shipped CSS animation durations, the two-typeface impression, and a responsive scaling
law derived and verified across four viewports. 438 files with 100% provenance coverage in
`INDEX.json`.

**Does not prove — and is actively misleading in places.** The felt geometry for all four reference
clients is mis-measured: the measurer's bounding box caught the `.table` DIV (rail plus the pod row
above it) instead of the green ellipse. Re-measured independently with a hue-mask +
largest-connected-component method gated on an ellipse-area sanity check, PokerNow's felt is
944×472 (aspect **2.00**), not 912×570 (aspect 1.60) — and every one of the four leaders is a ~2:1
stadium, so the document's headline finding ("felt aspect ratio splits the field") is backwards.
**As written, bar 1 rejects 3 of 4 reference clients and bar 2 rejects 4 of 4**, so using the
document to judge ClearDeck work would reject correct work. Palette percentages are 8-colour
quantisations of whole image files, and for three third-party captures 16-21% of the file is the
review site's page gutter rather than the poker client. Two further bars are unsupported by the
document's own citations. Details and the corrected figures: [H-14](DEFECTS.md#h-14).

The corpus itself lives outside the repo (session scratchpad), and the scripts that produced the
only numbers the document calls "exact" were never shipped with it — which is why nobody caught the
felt error.

---

## 4. What wave 1 did NOT do

* **Nothing was fixed.** Every engine defect listed in [DEFECTS.md](DEFECTS.md) is still present at
  `HEAD`, by instruction. Behaviour was frozen so that ground truth could be established first.
* **Nothing was deployed anywhere.** No mainnet call of any kind. No canister was even installed on
  the local replica during this wave — it has been down throughout.
* **No screenshot exists.** Zero of the 18 required PNGs.
* **The right-player-wins question is untouched.** No settlement oracle was built.
* **No UI change.** `src/cleardeck_frontend` is byte-identical to `ceacc37`.
* **The disclaimer, the 18+ notice, the jurisdiction warning and the no-rake property are
  untouched** in both README.md and the app, verified by diff against `ceacc37`. `make hygiene`
  now enforces that on every run.

---

## 5. The gap that matters most

Two independent harnesses now watch the money and the evaluator, and neither can see the thing a
poker site is most likely to get wrong: **paying the wrong player.**

The differential harness proves `evaluate_hand` ranks hands correctly, over the entire five-card
space, against three lineages. The money-safety harness proves chips are conserved and the ledger
is never short. Between those two proofs sits `determine_winners` — pot eligibility per side pot,
tie detection, chopping, odd-chip assignment — and it is covered by nothing. A payout to the wrong
seat conserves every total and passes every invariant. [E-05](DEFECTS.md#e-05) is exactly that shape,
and the only reason it was found at all is that its *precondition* happens to trip an attribution
check.

The missing piece is an independent settlement oracle: given hole cards, board and per-seat
contributions, compute what each seat is owed, and compare against what the engine actually paid,
over randomised hands. That is the first thing wave 2 should build, before any fix lands — because
until it exists, a fix to the side-pot code cannot be shown to have made things better rather than
differently wrong.
