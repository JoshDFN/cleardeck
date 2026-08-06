# `tools/differential`: ClearDeck's differential hand-evaluator harness

Compares ClearDeck's real poker engine (`src/poker_core`) against **two independent
mature reference evaluators** (three when the opt-in third is enabled), and reports
every disagreement.

**Operating rule.** A disagreement between `poker_core` and a mature reference is a
bug in `poker_core` until proven otherwise. ClearDeck is only convicted when *both*
references agree against it (`describe::require_unanimity`): a disagreement raised by
only one reference means the other sided with ClearDeck, so the two references
disagree with each other and the finding is reported as **inconclusive** rather than
charged to ClearDeck. The sub-rank *detail* check is the one documented exemption:
`rs_poker` exposes no kicker detail, so `poker` is the sole authority there, and those
findings are `Cosmetic` (wrong hand name shown, pot still correct) rather than
convictions.

## The references

| role | crate / package | lineage | licence |
|------|-----------------|---------|---------|
| A (always) | `rs_poker` 5.0.0 | its own perfect-hash tables, generated from scratch in the crate's `build.rs`; native 5/6/7-card lookup | Apache-2.0 |
| B (always) | `poker` 0.7.0 | Rust port of the `treys` Python package: Cactus Kev prime products + Senzee perfect hash | MIT |
| C (opt-in) | `phevaluator` (Python, C extension) | Henry Lee's perfect-hash tables, a third lineage. At ~900k hands/second it is cheap enough to run over the **whole** five-card space, not just a sample | Apache-2.0 |

Reference C is opt-in only because `cargo run --release` must work on a machine with
no Python; when `CLEARDECK_PHE_PYTHON` is set it becomes a full third reference and
also audits that A and B really are independent.

A fourth, library-free oracle rides along in the five-card sweep: the textbook
combinatorial count of hands per category (1,302,540 high cards, 1,098,240 one
pairs, ..., 40 straight flushes). It would catch a whole-category
misclassification even if both references were wrong in the same direction.

Reference B is a port of `treys`, so `treys` itself is deliberately **not** used as
the third oracle: two ports of the same tables would not be independent.

## What is checked

1. **All `C(52,5) = 2,598,960` five-card hands** through the real
   `poker_core::evaluate_five_cards`:
   * hand **category**, via an explicit cross-library name mapping
     (`src/category.rs`), never an opaque integer;
   * **sub-rank detail** (which pair, which trips, which straight high) against
     reference B's `Eval::classify()`: this is what catches "right category,
     wrong rank inside it";
   * ClearDeck's extra `HandRank::RoyalFlush` variant coinciding *exactly* with
     both references' ace-high straight flush;
   * the complete pairwise **ordering**.
2. **The ordering check is exhaustive, not sampled.** `src/classes.rs` buckets every
   hand by our class in our class order and requires (a) each bucket to map to a
   single reference class and (b) the reference class to strictly increase across
   buckets. Those two local conditions imply the global one, so a clean run is a
   proof over all `C(2598960, 2)` ≈ 3.4e12 hand pairs in `O(n)`.
   Violations are classified by what they would do to a pot:
   | kind | meaning |
   |------|---------|
   | `FalseTie` | we tie two hands the reference separates: pot chopped when one player should win it outright |
   | `FalseSplit` | we separate two hands the reference ties: pot awarded outright when it should be chopped |
   | `Inversion` | we order two hands backwards: **the losing hand is paid** |
3. **Millions of seeded random seven-card hands** through
   `poker_core::evaluate_hand`, the function `table_canister::determine_winners`
   actually calls. Reference A ranks seven cards with a single native lookup rather
   than a 21-subset loop, so agreement is not circular. The same ordering proof is
   applied to the sample, making the pairwise coverage quadratic in the sample size.
4. **Optionally all `C(52,7) = 133,784,560` seven-card hands** (`--exhaustive-sevens`,
   ~20 minutes, constant memory). It reuses the class correspondence the five-card
   sweep *proved*, and refuses to run if that sweep was partial or dirty.
5. **The literal `sign(our_cmp(a,b)) == sign(ref_cmp(a,b))` check** over random hand
   pairs, in two draw modes: two independent five-card hands, and the realistic
   showdown where two players share one five-card board. Redundant with (2) by
   construction and kept deliberately: it is an independent implementation of the
   same question, so it also guards the class-ordering machinery itself.
6. **Degenerate inputs** the references cannot represent at all: boards shorter than
   five cards, six or seven cards handed to the five-card entry point, and duplicate
   cards.
7. **The third reference over the whole five-card space** when
   `CLEARDECK_PHE_PYTHON` is set: `ours-vs-C`, `A-vs-C` and `B-vs-C`, each the same
   complete ordering proof. If A or B conflicts with C, ClearDeck's own comparison
   against that reference is downgraded to *inconclusive* rather than used to
   convict.

## Running it

```sh
cd tools/differential

# CI subset: a few seconds, fixed seeds, plus a regression case per known defect.
cargo test

# Full run: exhaustive five-card sweep + 5M random seven-card hands + 4M pairs.
cargo run --release

# Add the exhaustive seven-card sweep (~20 min).
cargo run --release -- --exhaustive-sevens

# Turn the third oracle on (audits that references A and B are independent).
python3 -m venv .venv && .venv/bin/pip install phevaluator
CLEARDECK_PHE_PYTHON=$PWD/.venv/bin/python cargo run --release
```

```sh
# Does the harness have teeth? Breaks a COPY of poker_core three ways and requires
# the harness to convict each time. Never writes to the repository.
./mutation-check.sh
```

Flags: `--sevens N`, `--pairs N`, `--adjudicator-sample N`, `--skip-exhaustive`,
`--exhaustive-sevens`.

Report location, in precedence order: `$CLEARDECK_DIFF_REPORT` >
`$SCRATCH/differential-report.json` > `./differential-report.json`.

Exit status: `0` if the engine agreed with both references everywhere the run
looked, `1` on any fund-impacting disagreement, `2` on a harness error.

## Hooking it into CI

`.github/workflows/ci.yml` runs `cargo test --locked --workspace` from the repo root,
which does **not** reach this crate: it is a separate workspace on purpose (see below).
Add a step to the existing `test` job (owner of that file needs to apply this):

```yaml
      - name: Differential hand-evaluator harness (fast subset)
        working-directory: tools/differential
        run: cargo test --locked
```

Two seconds in a debug build. Do not add `cargo run --release` to CI: the exhaustive
sweeps are a deliberate manual cost.

## Validating the harness itself (`mutation-check.sh`)

"No disagreements" is worthless unless the harness would have found one. The script
copies `src/poker_core` and this crate into a temp dir, injects three engine bugs into
the copy, and requires a `DISAGREE_FUND_IMPACTING` verdict for each:

| mutation | what the harness reported |
|----------|---------------------------|
| `FullHouse(trips, pair)` built as `FullHouse(pair, trips)` | 12 ordering **inversions** + 3,744 sub-rank detail mismatches. Minimal repro `A = 2c 2d Ac Ad Ah` vs `B = 2c 2d 2h 3c 3d` |
| one-pair kickers truncated from three to two | our class count collapses 7462 -> 5317, 585 **false ties**. Minimal repro `A = 2c 2d 3c 5c 6c` vs `B = 2c 2d 4c 5c 6c` |
| `HandRank` variant order swapped so `Flush` outranks `FullHouse` | 1 cross-category **inversion**. Minimal repro `A = Kc Kd Ac Ad Ah` vs `B = 2c 3c 4c 5c 7c` |

It also asserts afterwards that the repository's `poker_core/src/hand.rs` is
byte-identical to how it found it.

Seeds are fixed constants in `src/lib.rs` and never derived from the clock: a report
whose repro cases cannot be replayed is worthless.

## Why this crate is outside the repo workspace

`Cargo.toml` carries its own `[workspace]` stanza. The reference evaluators and
`serde_json` are heavyweight host-only dev dependencies, and letting them into the
root `Cargo.lock` would put them in the dependency graph the reproducible `wasm32`
canister build resolves against. `poker_core` is reached by *path*, so there is no
vendored copy of the engine to drift out of sync with the deployed one.

## Findings

### What was actually run (2026-08-04, `poker_core` @ `ceacc37` + the `poker_core` extraction)

| check | volume | result |
|-------|--------|--------|
| exhaustive five-card, category + detail + complete ordering, vs `rs_poker` | 2,598,960 hands | 7462 classes both sides; 0 false ties / 0 false splits / 0 inversions |
| same, vs `poker` | 2,598,960 hands | 7462 classes both sides; 0 / 0 / 0 |
| same, vs `phevaluator` (third lineage) | 2,598,960 hands | 7462 classes both sides; 0 / 0 / 0 |
| reference-vs-reference cross-checks | 3 x 2,598,960 | 0 conflicts, so nothing is inconclusive |
| per-category counts vs textbook combinatorics | 2,598,960 hands | exact match, incl. 4 royal + 36 other straight flushes |
| exhaustive **seven**-card via the proven class map | 133,784,560 hands | 0 mismatches vs either reference; 4824 distinct classes, exactly the combinatorial expectation |
| random seven-card, seed `0xC1EA2DEC0002` | 5,000,000 hands | 0 category / detail / ordering disagreements |
| pairwise sign check, seed `0xC1EA2DEC0001` | 2,000,000 independent pairs + 2,000,000 shared-board showdowns | 0 sign mismatches |
| degenerate-input probes | 17 probes | 12 hits, 5 root causes, all `Latent` (table below) |

Verdict: `AGREE_WITH_LATENT_MISUSE_FINDINGS`, **0 fund-impacting classes, 0 inconclusive**.
The default run takes 71 s and peaks at 230 MB; adding `--exhaustive-sevens` costs about
20 minutes more and no extra memory.

Reproduce with:

```sh
cd tools/differential
SCRATCH=/path/to/scratch CLEARDECK_PHE_PYTHON=/path/to/venv/bin/python \
  cargo run --release -- --exhaustive-sevens
```

### Nothing wrong with the ranking itself

Over all 2,598,960 five-card hands, ClearDeck's `evaluate_five_cards` produces exactly
7462 equivalence classes, in exactly the reference order, with exactly the reference
category and sub-rank detail, against all three references. The class-order proof
reported zero false ties, zero false splits and zero inversions, i.e. the ordering
agrees over **every** pair of five-card hands, not a sample. Per-category counts match
the textbook combinatorics exactly, including 4 royal flushes and 36 other straight
flushes. `evaluate_hand` agrees likewise on seven-card hands.

**No fund-impacting defect was found in the hand evaluator.** Every finding below is a
missing-input-validation defect on a path no live caller reaches today.

### Latent findings, in descending order of how easy they are to reach

| id | reproducer | what happens |
|----|-----------|--------------|
| `degenerate/duplicate-card-reaches-evaluate_hand` | hole `Ah Ah`, board `Kh Qh Jh 2c 3d` | `evaluate_hand`, the function `determine_winners` calls, returns `Flush([14,14,13,12,11])`: a five-card flush built from **four** physical cards. It checks neither card count nor distinctness, so a dealing or shuffling defect becomes a wrong winner instead of a trap. |
| `degenerate/evaluate_five_cards-fabricates-a-hand-from-more-than-five-cards` | `2h 3h 4h 5h 9h 6c 7c` | `evaluate_five_cards` is `pub` and takes `&[Card]`. With more than five cards it tests `is_flush` on one suit and `is_straight` over **all** ranks independently, and returns `StraightFlush(7)`, a hand that is not in the cards (both references say flush). |
| `degenerate/fewer-than-five-cards-returns-empty-highcard` | hole `Ah Kd`, empty board | `combinations(cards, 5)` returns empty below five cards, and `evaluate_hand` falls back to `HandRank::HighCard(vec![])`, which is `Ord`-EQUAL for every player and `Ord`-LESS than every real hand. A showdown on a short board would chop instead of erroring. `run_out_board` guards each deal with `deck_index < deck.len()`, so a 52-card deck makes this unreachable: it is a missing trap, not a live bug. |
| `degenerate/duplicate-cards-are-silently-ranked` | `Ah Ah Ah Ah Ah` | `evaluate_five_cards` returns `Flush([14,14,14,14,14])` for five copies of one card; `Ah Ah Ah Ah Kh` returns `FourOfAKind(14, 13)`. |
| `degenerate/detect_straight-prefers-the-wheel` | `detect_straight(&[14,6,5,4,3,2])` | returns `Some(5)`, the **weakest** straight present, because the wheel is tested before the descending window scan. Unreachable from `evaluate_hand` (which only ever passes five ranks) but it is what makes the `evaluate_five_cards` misuse above so easy to hit. |

## The `characterises_*` tests are not a specification

`tests/fast_subset.rs` contains pairs of tests per known defect:

* `characterises_defect_*`: green today, asserting an answer this harness has
  already shown to be **wrong**, so the behaviour cannot change silently. Each
  carries a `FIX WAVE:` note.
* `defect_*_is_fixed`: `#[ignore]`d, asserting what the fix wave owes.

When a fix lands, delete the `characterises_*` half and un-ignore the other. Never
copy an expectation out of a `characterises_*` test.
