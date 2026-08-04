# ClearDeck security findings

Findings that touch real user funds. Each entry states what was actually executed, so a
reader can tell a demonstrated defect from a suspected one.

Detailed write-ups that predate this file live alongside it:

- `docs/FINDING-01-chip-destruction.md` — CRITICAL, confirmed on a local replica: every
  showdown with post-flop betting pays the winner only the pre-flop pot and permanently
  destroys the rest.
- `docs/BACKEND-FUND-SAFETY-TODO.md`

---

## FINDING 02 — `state.pot` is the sole authority for side-pot totals, in both directions

**Severity:** HIGH. Mints chips from nothing in one direction, destroys them in the other,
and skews the split toward the deepest stack in both.
**Status:** mechanism CONFIRMED by executing the real pre-refactor code. Attacker-reachable
path to force the divergence NOT demonstrated — see "What is and is not proven".
**Where:** `calculate_side_pots`, `src/table_canister/src/lib.rs` @ `ceacc37` lines
3521-3556; now `poker_core::side_pots::build_side_pots_logged` in
`src/poker_core/src/side_pots.rs`, ported bug-for-bug on purpose.

After splitting the pot by bet level, the routine compares the sum of the side pots against
`state.pot` and forces the former to match the latter. It never compares against the players'
actual `total_bet_this_hand`. `state.pot` wins unconditionally.

### Direction A — `state.pot` too high: chips are created and handed to one player

```
if total_side_pots < state.pot {
    let remaining = state.pot.saturating_sub(total_side_pots);
    if let Some(last_pot) = side_pots.last_mut() {
        last_pot.amount = last_pot.amount.saturating_add(remaining);   // minted
    }
}
```

The entire excess is added to the LAST side pot, which by construction is the highest bet
level, i.e. the pot only the DEEPEST stacks are eligible for. Real output, produced by
running the original code (`src/poker_core/tests/golden_vectors.txt`):

```
contributions  seat1=9223372036854775807  seat2=1  seat3=1  seat4=10(folded)
               seat5=1  seat6=1  seat7=50            actual total = 9223372036854775871
state.pot      18446744073709551615
side pots out  7#1,2,3,5,6,7 ; 27#1,7 ; 80#1,7 ; 18446744073709551501#1
                                                    ^ ~9.2e18 chips that nobody wagered,
                                                      eligible: seat 1 only
```

Chips are the canister's claim on real ICP/ckBTC held on the ledger, so minted chips become
a withdrawable balance. The same thing happens at ordinary stakes — nothing about it needs
extreme values:

```
contributions  seat0=50_000_000  seat1=100_000_000  seat2=100_000_000   actual total = 250_000_000
state.pot      350_000_000                                             overstated by 1 ICP
side pots out  150000000#[0,1,2] ; 200000000#[1,2]
                                  ^ 1 ICP that nobody wagered, payable to seats 1 and 2
```

### Direction B — `state.pot` too low: chips are destroyed and the split is skewed

The capping branch scales every pot but the last by an `f64` ratio and gives the last pot
the remainder. Real output:

```
contributions  seat0=10(folded)  seat1=100  seat2=10  seat3=1     actual total = 121
state.pot      60
uncapped       4#1,2,3 ; 27#1,2 ; 90#1
side pots out  1#1,2,3 ; 13#1,2 ; 46#1      -> 61 chips destroyed
warning        BUG: Side pots (121) exceed total pot (60). Capping to pot amount.
```

Note the skew, which is not just a rounding artefact: the main pot (seats 1, 2, 3 eligible)
drops from 4/121 = 3.3% of the money to 1/60 = 1.7%, while the last pot (seat 1 only) rises
from 74% to 77%. Capping systematically moves money from pots that many players can win into
the pot only the deepest stack can win.

With `state.pot == 0` every side pot becomes 0 and all wagered money vanishes.

### Two further defects in the same branch

- **`f64` precision.** `(side_pot.amount as f64 * ratio) as u64` has a 53-bit mantissa, so
  any pot above 2^53 e8s (~90,071,993 ICP) is silently rounded. Not currently reachable at
  real stakes, but it is arithmetic on a payout path.
- **The warning is only a log.** The routine detects its own inconsistency, prints
  `BUG: Side pots (...) exceed total pot (...)`, and then pays out anyway. A detected
  accounting inconsistency on a fund-custody path should trap, not warn.
- **Dead fallback, orphaned pot.** The `else` branch that reads
  "No existing side pot - find any non-folded player to create a pot for" can never fire:
  it is only reached when the eligible list is empty, which at the lowest bet level means
  every contributor folded, in which case its own `any_eligible` list is empty too. The
  actual behaviour when every contributor has folded is that the routine returns NO side
  pots at all and the whole pot is orphaned — pinned by 37 golden vectors, e.g.
  `contributions seat0=100(folded) seat1=1000000(folded), pot=1000100 -> no side pots`.
  Not reachable in normal play (the last player standing does not fold, and
  `end_hand_single_winner` pays `state.pot` directly), but the fallback gives a false
  impression of being handled.

### What is and is not proven

- PROVEN: given a `state` whose `pot` disagrees with the sum of `total_bet_this_hand`, the
  real code mints or destroys chips as shown. 1,500 side-pot vectors were produced by
  executing the original `calculate_side_pots(&mut TableState)`; 466 of them trip the capping
  branch, 223 of those with a non-zero pot. Replayed on every `cargo test --workspace` by
  `src/poker_core/tests/golden_vectors.rs`.
- PROVEN ELSEWHERE: `state.pot` and `state.side_pots` DO diverge in real play —
  `docs/FINDING-01-chip-destruction.md` records `pot = 64,000,000` against
  `side_pots = [4,000,000]` on a local replica.
- NOT PROVEN: that an attacker can steer `state.pot` above the true sum of contributions.
  Every `state.pot` mutation found (antes at lib.rs:2394, blinds at 2408/2420, calls/bets/
  raises/all-ins at 2808/2830/2860/2876) increments the pot in the same statement that
  increments that player's `total_bet_this_hand`, so the two should stay equal within a hand.
  Auditing that they cannot drift — across `saturating_*` clamping, upgrades,
  `admin_reinit_table`, and hands abandoned mid-street — is outstanding work.

### Recommended handling

Do not patch this branch on its own. The reconciliation exists precisely because the two
figures are not trusted to agree, and deleting it would surface whatever real accounting bug
it has been masking. The order that works:

1. Land the conservation invariant (chips in == chips out, exact, because ClearDeck takes no
   rake) as an assertion after every action in randomised play.
2. Make the payout basis the players' `total_bet_this_hand` — the money actually collected —
   and demote `state.pot` and any precomputed breakdown to display state.
3. Then make a detected mismatch trap instead of logging, so it can never silently pay.

---

## FINDING 03 — the committed `table_canister.did` does not describe the deployed code

**Severity:** MEDIUM. Pre-existing; not introduced by the `poker_core` extraction.
**Status:** CONFIRMED by extracting Candid from the built wasm with `candid-extractor` and
diffing against the committed file. 241 diff lines.
**Where:** `src/table_canister/table_canister.did`

Reproduce:

```
cargo build --package table_canister --target wasm32-unknown-unknown --release
candid-extractor target/wasm32-unknown-unknown/release/table_canister.wasm > /tmp/real.did
diff src/table_canister/table_canister.did /tmp/real.did
```

Substantive drift, beyond harmless field/type reordering:

- `Player` is missing `sitting_out_since : opt nat64`.
- `ActionRecord` is missing `phase : text` and `amount : nat64`.
- The `Result_N` aliases are numbered differently, so `Result_1` .. `Result_5` in the
  committed file denote different types than the code's. A client generated from the
  committed file will mis-decode several method results.

This is not merely a stale artefact. `icp.yaml` points all four table canisters at this file
(`candid: src/table_canister/table_canister.did`, lines 54/64/74/84), so it is the interface
the deployed canisters advertise. Any tool that trusts published metadata — a wallet, a block
explorer, a generated client — is working from a description that does not match the code.

The `.did` is hand-maintained rather than generated, which is why it drifted. It should be
generated from the wasm in CI and the build should fail on any diff. Note that generating it
would also reformat the file and drop its hand-written comments, so the switch needs a
deliberate decision rather than a drive-by regeneration.

---

## FINDING 04 — latent: `detect_straight` prefers the wheel over a higher straight

**Severity:** LOW today (not reachable), MEDIUM as a trap for future callers.
**Status:** CONFIRMED by direct call. Pinned by
`detect_straight_prefers_the_wheel_over_a_higher_straight_when_both_are_present` in
`src/poker_core/tests/unit_tests.rs`.
**Where:** `poker_core::hand::detect_straight` (moved verbatim from lib.rs @ `ceacc37`:2485)

The A-2-3-4-5 check runs before the descending window scan, so with six or more distinct
ranks containing both a wheel and a higher straight, the wheel wins:

```
detect_straight(&[14, 6, 5, 4, 3, 2])    == Some(5)    // should be Some(6)
detect_straight(&[14, 7, 6, 5, 4, 3, 2]) == Some(5)    // should be Some(7)
```

Not reachable today: `evaluate_five_cards` is the only caller and is only ever invoked on
exactly five cards, from `combinations(all_cards, 5)`, and five distinct ranks cannot contain
both a wheel and a higher straight. The trap is that calling `evaluate_five_cards` or
`detect_straight` with all seven cards is an obvious-looking optimisation, and it would
silently misrank hands. Left unfixed in this wave by instruction (behaviour is frozen while
ground truth is being established); the test documents it so a "fix" is a deliberate act.

---

## FINDING 05 — `leave_table()` makes FINDING 02 Direction A ATTACKER-REACHABLE

**Severity:** HIGH. Any seated player can, with one public update call and no special
privilege, move contested money out of the pot a short stack is eligible for and into the
pot only the deepest stack can win.
**Status:** CONFIRMED. Reproduced by executing the real pre-refactor `calculate_side_pots`
(code sliced verbatim out of `git show ceacc37:src/table_canister/src/lib.rs`).
**Found by:** adversarial review of the poker_core extraction wave. FINDING 02 states that an
attacker-reachable path to force `state.pot > sum(total_bet_this_hand)` was NOT demonstrated.
This is that path.
**Where:** `leave_table`, `src/table_canister/src/lib.rs:2659-2687` feeding
`poker_core::side_pots::build_side_pots_logged` (`src/poker_core/src/side_pots.rs:137-142`)

`leave_table` is a plain `#[ic_cdk::update]` with **no phase guard, no turn requirement and
no rate limit**. Mid-hand it marks the player folded and then removes the seat entirely:

```rust
// src/table_canister/src/lib.rs:2681-2687
if was_in_hand {
    if let Some(ref mut p) = state.players[seat] { p.has_folded = true; }
}
state.players[seat] = None;          // <-- contribution disappears from the accounting
```

`state.pot` still holds that player's money, but `collect_contributions` enumerates
`state.players`, so the vacated seat contributes nothing. The side pots therefore sum to LESS
than `state.pot`, and Direction A fires: the entire abandoned amount is appended to the LAST
side pot, which by construction is the highest bet level, i.e. the pot only the deepest stacks
are eligible for.

Note the contrast with `cash_out`, which guards the same removal with
`"Cannot cash out while in a hand"` (lib.rs:1913). `leave_table` has no such guard.

### Executed reproduction

Seat 0 is an honest short stack all-in for 50. Seat 1 bets 200. Seat 2 (deep) bets 200.
Everyone's chips were really collected, so `state.pot = 450` in every case below.

```
A) all three seats occupied (before the call)
   pot[0] = 150  eligible [0, 1, 2]
   pot[1] = 300  eligible [1, 2]

C) seat 1 merely FOLDS and keeps its seat  <-- correct behaviour
   pot[0] = 150  eligible [0, 2]
   pot[1] = 300  eligible [2]

B) seat 1 calls leave_table()              <-- state.players[1] = None
   pot[0] = 100  eligible [0, 2]      main pot shrank by 50
   pot[1] = 350  eligible [2]         deep stack's exclusive pot grew by 50
```

Between C and B nothing about the money changed, only whether the seat was vacated. The
honest all-in player's winnable main pot fell from 150 to 100, and the 50 moved into a pot
that only seat 2 can win.

### Why it matters

Seat 1's contribution at the 50 bet level is money seat 0 was entitled to contest. Vacating
the seat removes seat 0's claim on it and hands it to the deepest stack. Two principals under
one operator (trivial to obtain) therefore make this a repeatable edge: the confederate posts
into the pot and leaves, and the money reappears in the attacker's exclusive side pot instead
of the pot the victim could have won. In the numbers above the colluding pair's net result
improves by 50 and seat 0's expected value falls by the same 50.

This is not the `f64` capping branch and needs no extreme values; it is ordinary heads-up-plus
-one play at any stake.

### Suggested direction (not applied — this wave freezes behaviour)

Two independent fixes, both worth having:

1. Stop letting a vacated seat erase its stake. Either keep the seat (`has_folded = true`,
   status `Left`) until the hand completes, as `cash_out` effectively requires, or move the
   departing player's `total_bet_this_hand` into a recorded "abandoned" contribution so the
   bet-level split still sees it.
2. Make the reconciliation in `build_side_pots` compare against the players' actual
   `total_bet_this_hand` rather than trusting `state.pot`, and trap on a mismatch instead of
   silently topping up the last pot (see FINDING 02 and FINDING 03).

Also: `leave_table` is missing the `check_rate_limit()?` that `player_action` has.
