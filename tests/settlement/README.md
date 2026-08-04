# The ClearDeck settlement oracle

An **independent** answer to one question the rest of the test suite cannot answer:
**did the right player get paid?**

```
make settlement          # everything, including the PocketIC comparison runs
make settlement-fast     # the rules + the golden reproducers, no replica, seconds
```

## Why this exists

Wave 1 established two things and left the gap between them uncovered.

* The hand evaluator is correct. All 2,598,960 five-card hands, three independent
  evaluator lineages, zero disagreements.
* Chips are conserved. Nothing is minted.

Neither says the right seat got the money. **A payout that hands the main pot to the
worst hand at the table conserves every total and satisfies every M1..M6 money
invariant.** Between those two proofs sits `determine_winners`: per-side-pot
eligibility, tie detection, chopping, odd-chip assignment. It was covered by nothing.

This crate closes that gap and is designed to be believed when it disagrees with the
engine.

## How independence is maintained

The oracle (`src/oracle.rs`) derives payouts from the rules of the game:

1. an uncalled bet is returned to the player who made it, because nobody covered it;
2. what remains is split into layered pots, one per distinct all-in depth;
3. a player is eligible for a layer only if they covered it in full;
4. among the eligible players who did not give up their claim, the best five-card
   hand wins the layer; equal hands chop it;
5. chips that do not divide are awarded **one each, clockwise from the button**.

It does **not** call `build_side_pots`, `apply_side_pots`, or anything lifted from
`determine_winners`. A port of the engine's logic would be a mirror, not an oracle:
it would agree with every bug.

The one thing borrowed from the engine's crate is `poker_core::evaluate_hand`, for
step 4's "best five-card hand". That is deliberate and safe: ranking is the part wave
1 proved exhaustively, and a critic could not construct a `poker_core` ranking bug the
differential harness failed to convict. Ranking is settled. Payment is not.

The oracle also self-checks: `settle` panics if its own arithmetic fails to owe out
exactly what was collected. An instrument that silently loses chips cannot convict an
engine of losing chips.

### The odd-chip rule, stated

When a pot does not divide evenly, the leftover chips go **one each, walking clockwise
from the button**, so the first eligible winner to the button's left takes the first
odd chip. That is what rooms do: Robert's Rules of Poker gives the odd chip to the
first player clockwise from the button in flop games, and the TDA rules put odd chips
with the player(s) in the earliest position, distributed one at a time when there is
more than one. It matters here because ClearDeck's chips are e8s, so a three-way chop
of a pot that is not a multiple of three leaves real, withdrawable chips to assign.

The engine credits the **whole** remainder to a single seat. For a two-way chop the
remainder is at most one chip and the rules coincide. For three or more winners they
do not — see D-04 below.

## How the hard cases are reached on purpose

The task was explicit that the interesting settlements must be reached deliberately,
not hoped for. Two mechanisms make that possible against the real canister, with no
engine modification and no faucet.

**Exact stacks, via a snapshot baseline.** `buy_in(seat, amount)` takes an exact
amount, so the all-in ladder is chosen rather than observed. A canister snapshot of
the seated, between-hands table is restored before every scenario, so stacks, escrow,
hand number and the button all come back identical.

**The deal we want, via rewind-and-re-deal.** The shuffle comes from `raw_rand` and no
test can choose it. But after `start_new_hand` the deck is fully determined and a
controller can read it, so the harness knows every seat's hole cards and the exact
five-card board that will come *before any chip has moved* (`src/deal.rs`). If the
deal is not the one a scenario needs — say an exact three-way tie — the snapshot is
restored and the hand is dealt again. The subnet's randomness is not canister state,
so each re-deal is a fresh shuffle. That turns "an exact three-way chop" from a
one-in-a-few-hundred accident into a search of a few hundred single-message deals.

Every prediction is checked against the board the engine actually dealt
(`HandComparison::build`), so a wrong prediction fails loudly instead of silently
steering the search.

## Trap 1: the wasm

An earlier harness resolved the table wasm as "use the release artifact if it exists,
otherwise build", and with a stale-but-present file had therefore never compiled the
canister it claimed to test. This crate does three things instead of one:

1. **always builds** from the checked-out source, unless `$CLEARDECK_TABLE_WASM`
   explicitly overrides it;
2. **freshness-checks** whatever it ends up with against the mtime of every Rust
   source and manifest that feeds it, and refuses to run if any input is newer.
   `tests/settlement.rs::a_stale_wasm_is_refused` exercises the guard;
3. **asserts the replica's module hash** for the installed canister equals the sha256
   of the bytes it handed over. `World::new` does it on every instance, and
   `the_installed_module_is_the_one_the_harness_built` states it as its own claim.

The sha256 is printed once per test process, so the identity of the binary under test
is in the output next to the findings.

## What is measured

For every seat: the net change in what the canister owes that principal across the
hand, counted as `escrow + seated chips`. Counting it that way rather than as a change
in `Player::chips` is deliberate — a player who vacates a seat mid-hand has their
stack moved into escrow, so `chips` alone would read as a total loss where nothing was
lost. The oracle's prediction for the same quantity is `owed - contributed`.

Oracle inputs, all from `get_table_state`: `total_bet_this_hand`, `hole_cards`,
`community_cards`, `has_folded`, `dealer_seat`. Deliberately **not** inputs:
`state.pot`, `state.side_pots`, or the engine's `Winner` list. Those are recorded as
evidence about the engine and printed in the report, but the oracle never reads them.

### Auditing the oracle's own input

Contributions come from the engine's bookkeeping, so a defect that *understated*
`total_bet_this_hand` would have the oracle measuring against a lie. `state.pot` is a
second, independent accumulator — every bet increments both — so the harness records
the highest `state.pot` it observes and flags any hand where it exceeds the sum of every
seat's `total_bet_this_hand`. The check is one-sided on purpose: a hand that runs the
board out and pays in a single message has zeroed the pot again before the harness can
look, so a lower observed peak means nothing. A higher one is exactly the E-03
Direction A precondition, and the coverage report counts it.

It reads zero on every hand in the current suite, including D-03 — where the engine's
`state.pot` (480) agrees with the harness's contributions (480) and it is the engine's
*internal* `collect_contributions` (420) that is short. That is the mechanism, confirmed
from two directions.

## What it found, and what happened next

Four disagreements, each with a permanent minimal reproducer in
`tests/disagreements.rs`. Every one is stated with exact hole cards, the board, per-seat
contributions, what the engine paid, what was owed, and the delta per seat.

> **STATUS: all four are FIXED (wave 2), and the oracle is now QUIET.** D-01 and D-02
> were docs/DEFECTS.md E-01, D-03 was E-05, D-04 became E-35, and E-03 was fixed with
> them. On the current tree `make settlement` reports **0 disagreements over 17
> deliberate hands, 0 chips destroyed, 0 chips misdirected**, with every coverage
> class still reached. The counts per class before and after:
>
> | class | before | after |
> |---|---|---|
> | hands settled | 17 | 17 |
> | DISAGREEMENTS | **6** | **0** |
> | chips destroyed | 240 on the headline hand | **0** |
> | chips misdirected | 21 | **0** |
>
> Two things changed in this crate as a result, and they are the important part:
>
> 1. **Every hand is now GATED, where it runs.** `Bench::gate` fails the test that ran
>    a hand whose per-seat deltas differ from the oracle, printing the whole
>    reproducer, and the only way past it is an explicit label in
>    `Bench::allow_disagreement` (which is empty). This closes the gap the crate's own
>    critic found: a conserving wrong-seat payout was planted on a path the suite hits
>    four times per run and all 37 tests stayed green, because the class and randomised
>    tests asserted nothing per hand.
> 2. **The `pinned_*` markers were inverted**, and they now pin the per-seat DIFF
>    vector at all zeroes rather than merely `!agrees`. The `golden_*` tests are
>    unchanged: they are the record of what each defect was.

| id | defect | shape | conserves? |
|----|--------|-------|-----------|
| D-01 | E-01 | every showdown pays only the pre-flop pot; all post-flop money is destroyed | no, destroys |
| D-02 | E-01 | an uncalled river/flop bet is destroyed rather than returned | no, destroys |
| D-03 | E-05 | a seat vacated before the pots are built moves a short stack's main pot into the deep pot | **yes** |
| D-04 | **new** | when a chopped pot leaves more than one odd chip, all of them go to one seat | **yes** |

D-03 and D-04 are the two that justify the crate. Both conserve every chip: total in
equals total out, seat by seat the money is simply in the wrong hands. `M1..M6` are
conservation invariants, so neither is visible to them. `docs/DEFECTS.md` E-05 says as
much — "this is a redistribution, so M1..M6 conservation invariants are blind to it.
Only M1b's attribution leg sees the precondition" — and that is the distinction this
crate closes: M1b sees the *precondition*, this oracle names the *wrong seat and the
exact number of chips*.

### D-03, in full

```
board Kd Tc 3d 2c Ad          button seat 1
seat  hole    contributed  folded  left    engine   oracle    DIFF
   0  As 8d            20      no    no       +40      +60     -20
   1  6h 8h            60     yes   yes       -60      -60      +0
   2  4d 9h           200      no    no      -200     -200      +0
   3  7s Th           200      no    no      +220     +200     +20
collected 480   engine paid 480   destroyed 0

engine side pots:  60 eligible [0,2,3]   420 eligible [2,3]
oracle pot layers: 80 eligible [0,2,3]   120 + 280 eligible [2,3]
```

Seat 1 folded with 60 committed and left the table while the betting round was still
open — before `calculate_side_pots` had run for the hand at all. `collect_contributions`
reads the seat vector, seat 1 is no longer in it, so the pots are built from 420 while
`state.pot` says 480, and the reconciliation branch appends the missing 60 to the pot
only the deep stacks can win. Seat 0's main pot is 60 where it should be 80.

Twenty chips moved from the honest short all-in to the deepest stack, with perfect
conservation. Two principals under one operator make that a repeatable collusion edge,
and it needs no deliberate call: `check_timeouts` auto-folds a quiet player and
`cash_out` then lets them out.

### D-04, the new finding

`determine_winners` divides a chopped pot with

```rust
let pot_share = side_pot.amount / pot_winners.len();
let remainder = side_pot.amount % pot_winners.len();
let amount = if seat == remainder_seat { pot_share + remainder } else { pot_share };
```

so the whole remainder goes to one seat. Observed on a three-way tie where the
smallest layer held 8 chips: 2 each and two over. The engine gave both odd chips to
seat 2; the rules give one to seat 2 and one to seat 4.

The amount is one chip, 1 e8s. The finding is not the amount. It is that a pot is
being divided by a rule the game does not have, on the code path that also decides who
gets the other 99.99% of the money, and that nothing else in the suite can see it
because it conserves exactly.

## Proof that it can convict a wrong-seat payout

`tests/settlement.rs::the_oracle_convicts_a_conserving_wrong_seat_payout` drives eight
multi-way all-in showdowns with real side-pot ladders and **no** post-flop betting, so
E-01 is not in play and every chip the canister collects is paid out. In the tree as it
stands it is a control: the engine settles all eight correctly and the test asserts
that.

Three payout bugs were then planted in a copy of the repo under `$SCRATCH` (never in
the tree) and the same eight hands re-run with `SETTLEMENT_EXPECT_WRONG_SEAT=1`. Each
plant **conserves every chip** — the probe asserts `destroyed == 0` on every hand, and
it held for all three — so none of them is visible to any conservation invariant.

| plant | patch | hands convicted | largest per-seat error |
|-------|-------|-----------------|------------------------|
| **worst hand wins** | `eligible_hands…rank).max()` → `.min()`, and `sort_by(b.1.cmp(&a.1))` → `sort_by(a.1.cmp(&b.1))` | **8 of 8** | 400 chips |
| **rotate the winner one seat** | each pot paid to the next seat along that pot's eligible list | **8 of 8** | 400 chips |
| **eligibility ignored** | `eligible_hands` no longer filtered by `side_pot.eligible_players`, so a short all-in can be paid from a pot it never covered | **6 of 8** | 600 chips |

Example, from the worst-hand plant. Nothing created, nothing destroyed, everything to
the wrong seat:

```
--- probe_all_in_2_3_4 | board Ad Jh 2h Ac 3d
seat  hole    contributed   engine   oracle    DIFF
   2  Ts Qh            80      -80      -80      +0
   3  Kd 4s           160     -160     +240    -400
   4  7s Th           400     +240     -160    +400
collected 640   engine paid 640   destroyed 0
```

The plants are reproducible: `$SCRATCH/plant.py <worst-hand|rotate-winner|ignore-eligibility> <repo-copy> <pristine-lib.rs>`.

## Coverage actually reached

Derived from the observed records, never from a scenario's intent, so a scenario that
aimed at a three-way pot and produced a two-way one cannot count as three-way
coverage. From the clean `make settlement` run (17 deliberate hands):

```
     17  hands settled                          8  exact tie (a pot chopped)
     15  showdown reached                       3  3-way chop
     12  3+ players to showdown                 4  odd chip had to be placed
      5  4+ players to showdown                13  uncalled bet returned
     10  2 simultaneous all-ins                 3  folded money above a short all-in
      8  3 simultaneous all-ins                 4  money wagered after the flop
      3  4+ simultaneous all-ins                2  seat vacated mid-hand
     10  all-ins at 2+ different depths         2  fold-out, no showdown
      8  all-ins at 3+ different depths
     13  2+ pot layers (a real side pot)        6  DISAGREEMENTS
     10  3+ pot layers                          0  hands the oracle could not rule on
                                                0  hands where the oracle's input was
                                                   itself suspect
```

The main test asserts a non-zero count for every one of those classes: a zero means the
harness is not measuring what it claims, which must fail loudly rather than pass as a
clean run.

## Layout

```
src/oracle.rs      THE ORACLE: what the rules of poker say each seat is owed
src/observe.rs     watch one hand on the real canister, compare against it
src/deal.rs        read the deal, predict the board the deck will produce
src/drive.rs       exact stacks, stated betting policies, the E-05 vacate window
src/scenarios.rs   the bench: snapshot baseline + rewind-and-re-deal search
src/suite.rs       the named scenarios, shared by every caller
src/coverage.rs    what was ACTUALLY reached, derived from the records
src/wasms.rs       build + freshness-check + identify the module under test
src/world.rs       PocketIC, real ICP ledger, real table canister wasm
src/table_api.rs   Candid mirror of the canister surface (from the code, not the .did)
src/ledger.rs      real ICP ledger plumbing
src/cards.rs       card notation, so a reproducer reads like a hand history

tests/oracle_rules.rs   21 hand-written rule tests. READ THIS FIRST.
tests/settlement.rs     the comparison harness against the real canister
tests/disagreements.rs  D-01..D-04, golden + pinned against the canister
```

If you are reviewing this crate, read `tests/oracle_rules.rs` first. Every case states
the cards in ordinary notation, the money, and the answer the rules of poker give. If
any of those is wrong, every finding downstream is worthless.

## Running it

```
cd tests/settlement

cargo test --test oracle_rules                       # the rules. milliseconds.
cargo test --test disagreements -- golden            # the four reproducers. milliseconds.
cargo test --test settlement -- --test-threads=1 --nocapture
cargo test --test disagreements -- pinned --test-threads=1 --nocapture
```

`--test-threads=1` because each test owns a PocketIC instance, which is a replica
process. PocketIC instances are dropped when the process exits; if a run is killed
mid-way, check for orphaned `pocket-ic` processes before starting another.

`SETTLEMENT_RANDOM_HANDS=<n>` sets the length of the randomised sweep (default 24).
`SETTLEMENT_RECORD_ONLY=1` turns the per-hand gate into a recorder, so a run against an
engine that HAS a payout defect reports the full count per class instead of failing at
the first bad hand. That is how the before/after table above was measured, against a
copy of the tree with the four fixes reverted. Never set it in CI.
`SETTLEMENT_SKIP_FRESHNESS=1` disables the staleness guard and says so loudly; it
exists only so a reader can deliberately test a module that is not the current tree.

## For whoever changes the payout code next

This crate is a measuring instrument, not a fix — but since wave 2 it is also the
acceptance gate for the payout path. When you change that path:

* every hand you run through `Bench` must agree with the oracle, seat by seat.
  `Bench::gate` fails at the hand, not at the end of the run, and prints the cards, the
  money and the per-seat delta. If a defect genuinely has to ship, add its label to
  `Bench::allow_disagreement` AND an entry to `docs/DEFECTS.md` in the same change; do
  not delete the scenario.
* `tests/disagreements.rs` `pinned_*` tests pin four defects as FIXED, per seat. If one
  of them fails, a payout defect is back.
* the `golden_*` tests encode the engine's *old* payout as a constant. Keep them: they
  are the record of what the defect was. Their oracle-side assertions must not change.
* `the_engine_settles_every_class_of_hand_the_way_the_rules_of_poker_do` asserts zero
  disagreements, zero destroyed and zero misdirected over the whole deliberate suite.
  That assertion is the definition of done, and it was inverted from
  `disagreements > 0` when the fix landed.
