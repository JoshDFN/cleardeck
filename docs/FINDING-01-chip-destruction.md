# FINDING 01 — CRITICAL: every showdown destroys all post-flop money

**Status:** confirmed by reproduction on a local replica, 2026-08-04
**Severity:** CRITICAL — permanent, unrecoverable loss of real user funds
**Affects:** every hand that reaches a showdown after any post-flop betting
**Present in:** `src/table_canister/src/lib.rs` at `HEAD`, byte-identical to commit
`d461846` — the last commit before the icp-cli modernization pass, which touched no Rust.
The mainnet table canisters are therefore expected to contain this bug. Confirming that
requires reading a mainnet module hash, which has deliberately **not** been done under the
local-replica-only constraint of this work.

## What happens

At a showdown, the winner is paid only the money that was in the pot **before the flop**.
Every chip wagered on the flop, turn and river is deleted from the game. It is debited from
the players' stacks and credited to nobody.

The tokens themselves stay inside the table canister on the ledger. Because the only
outbound transfer path is `withdraw`, which pays out strictly against the caller's own
escrow balance (`src/table_canister/src/lib.rs:1610`), and because there is no
administrative withdrawal function anywhere in the canister, **those tokens can never be
withdrawn by anyone, including a controller.** The loss is permanent.

## Reproduction (local replica, heads-up table_1)

```
after blinds        pot =  3,000,000   side_pots = []
preflop completes   pot =  4,000,000   side_pots = [4,000,000]   <-- frozen here
flop: bet 30M, call pot = 64,000,000   side_pots = [4,000,000]   <-- did not grow
turn, river: checks pot = 64,000,000   side_pots = [4,000,000]
showdown            winner had a Straight and was paid 4,000,000
                    60,000,000 e8s (0.6 ICP) destroyed
```

A second, independent run with a 20M flop bet destroyed 40,000,000 e8s. Chip conservation
across the hand:

```
TOTAL BEFORE = 1,000,000,000   (escrow 300M+300M, chips 200M+200M)
TOTAL AFTER  =   960,000,000
DELTA        =   -40,000,000
```

Probe scripts: `conserve.py` and `sidepot_probe.py` (session scratchpad). These will be
folded into the permanent money-safety regression suite as invariant **M1 (conservation)**.

## Root cause

Three lines conspire.

1. **`advance_to_next_street`, `src/table_canister/src/lib.rs:3381`** — on the
   `PreFlop` → `Flop` transition, `calculate_side_pots(state)` is called
   **unconditionally**, not only when someone is all-in:

   ```rust
   GamePhase::PreFlop => {
       // Calculate side pots before dealing flop (in case of all-ins)
       calculate_side_pots(state);
   ```

   So from the flop onwards `state.side_pots` is *always* non-empty, on every single hand,
   holding exactly the pre-flop contributions.

2. **`determine_winners`, `src/table_canister/src/lib.rs:3634`** — only rebuilds the pots
   when the list is empty, which it never is:

   ```rust
   if state.side_pots.is_empty() {
       calculate_side_pots(state);
   }
   ```

   The same stale guard exists in `run_out_board` at line 3312.

3. **`determine_winners`, `src/table_canister/src/lib.rs:3796`** — after paying out from
   the stale breakdown, it discards the real pot without reconciling:

   ```rust
   state.pot = 0;
   state.side_pots.clear();
   ```

`calculate_side_pots` itself contains a reconciliation step that would have caught this
(`if total_side_pots < state.pot { last_pot.amount += remaining }`, line 3528) — but it
only runs at the moment the pots are built, when the two figures still agree.

## Why it was not noticed

- Hands that end in a **fold** are paid correctly. `end_hand_single_winner`
  (line 3558) awards `state.pot` directly and ignores `side_pots`. Most heads-up hands end
  this way.
- Hands that reach a showdown with **no post-flop betting** are also correct, because the
  pot never grew past the pre-flop figure.
- Only *showdowns with post-flop betting* are wrong, and they are wrong quietly: the
  winner's stack simply goes up by less than the pot they were shown. There is no error,
  no log, no trap.
- The test suite could not have caught it. `src/table_canister/tests/unit_tests.rs` does
  not test the canister at all — it re-implements its own private copies of the card
  types, the shuffle, the evaluator and a differently-shaped `calculate_side_pots`, and
  asserts against those. All 34 tests pass and none of them touch this code path.
- `README.md` lists "Large pots may have rounding issues (e8s precision)" under Known
  Issues. This is not a rounding issue. It is total loss of the post-flop pot.

## The fix (not yet applied)

Do not fix this in isolation. The correct sequence is:

1. Land the money-safety harness so **M1 (conservation)** is asserted after every action
   in randomised play. That turns this from a bug someone spotted into a bug the suite
   cannot let back in.
2. Then fix: build the side pots from `total_bet_this_hand` **at payout time**, and treat
   any pre-computed breakdown as advisory display state only. Concretely, make
   `determine_winners` and `run_out_board` always recompute, and add a hard post-condition
   that the sum of awarded chips equals the pot that was collected. The no-rake property
   makes this exact: chips awarded must equal chips wagered, to the e8.
3. Keep the intermediate `calculate_side_pots` call only if the UI needs a live side-pot
   display, and have it write to a separate display-only field so it can never be mistaken
   for the payout basis.

## Related defects found in the same reading

- `determine_winners` never returns an **uncalled bet** before the showdown. The excess is
  left in a pot only the bettor is eligible for, so the money reaches the right player but
  the displayed pot is inflated and the all-in moment diverges from every real client.
- An all-in raise smaller than a full min-raise still resets `has_acted_this_round` for
  every other player (`player_action`, line 3165), reopening the betting. Under standard
  rules an incomplete all-in raise does not reopen action to players who have already
  acted.
- `player_action` rejects an action after the timer expires (line 3024) but does not fold
  the player or advance the hand. If nothing calls `check_timeouts`, the table wedges.
- `pre_upgrade` logs and swallows a `stable_save` failure (line 4543) and lets the upgrade
  proceed. Already tracked in `docs/BACKEND-FUND-SAFETY-TODO.md`.
