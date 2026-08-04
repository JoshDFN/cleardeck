# ClearDeck security findings

Findings that touch real user funds. Each entry states what was actually executed, so a
reader can tell a demonstrated defect from a suspected one.

> ## ⛔ READ THIS FIRST — WHAT IS AT STAKE RIGHT NOW
>
> The mainnet table canisters custody **real ICP and real ckBTC**. Three of the findings below
> are confirmed by execution to cause **permanent, unrecoverable loss of real user funds**, and
> one is a **chips-from-nothing (fund-theft class) primitive** that is currently blocked only by
> another bug.
>
> **1. Money is being destroyed in normal play, today.** FINDING 01: every showdown that follows
> any post-flop betting pays the winner only the pre-flop pot. Everything wagered on the flop,
> turn and river is debited from stacks and credited to nobody. The tokens stay inside the
> canister and **cannot be withdrawn by anyone, including a controller** — `withdraw` pays
> strictly against the caller's own escrow and no administrative withdrawal exists. FINDING 06
> and FINDING 07 reach the same terminal state by other routes. On the deployed `table_1` and
> `btc_table_1` a 30-second lull is enough to trigger it (FINDING 12).
>
> **2. FUND-THEFT CLASS, latent — FINDING 10.** A single on-ledger transfer can be credited to a
> player's escrow **twice**, which is withdrawable ICP created from nothing. Two mechanisms:
> `periodic_cleanup` bounds `VERIFIED_DEPOSITS` by *forgetting the oldest block indices*, and
> `deposit()` never records the block index of the transfer it just made, so that block satisfies
> every check `notify_deposit` performs. It is **not exploitable today** only because FINDING 06
> makes `notify_deposit` fail before it can credit anything — a decode bug standing in for an
> access control. **Fixing FINDING 06 without fixing FINDING 10 in the same change opens a live
> path to mint withdrawable ICP.** The FINDING 10 write-up below assesses it as low-today /
> medium-after; it is flagged here as fund-theft class because the post-fix impact is direct
> creation of withdrawable funds, and because the fix ordering is what protects it.
>
> **3. Any seated player can move contested money into the deepest stack's pot** with one public
> update call, or by simply disconnecting (FINDING 05 and FINDING 08). No special privilege, no
> extreme values, ordinary stakes.
>
> Nothing here has been patched. Wave 1 deliberately froze engine behaviour so that ground truth
> could be established first, and every reproducer is written to FAIL when its defect is fixed, so
> a fix cannot land without updating this document in the same change.
>
> **Do not fix FINDING 06 on its own. See the fix ordering in
> [DEFECTS.md](DEFECTS.md#fix-ordering).**

**The single prioritised queue for all of this is [docs/DEFECTS.md](DEFECTS.md)**, which
reconciles these findings with the harness and tooling defects found alongside them and gives each
one an id, a severity and a reproduce command. This file is the evidence; that file is the order of
work.

| here | DEFECTS.md | severity |
|---|---|---|
| FINDING 01 | [E-01](DEFECTS.md#e-01) | critical |
| FINDING 02 + FINDING 09 | [E-03](DEFECTS.md#e-03) | high |
| FINDING 03 | [E-08](DEFECTS.md#e-08) | medium |
| FINDING 04 | [E-13](DEFECTS.md#e-13) | low |
| FINDING 05 + FINDING 08 | [E-05](DEFECTS.md#e-05) | high |
| FINDING 06 | [E-04](DEFECTS.md#e-04) | high |
| FINDING 07 | [E-07](DEFECTS.md#e-07) | high |
| FINDING 10 | [E-02](DEFECTS.md#e-02) | **fund-theft (latent)** |
| FINDING 11 | [E-12](DEFECTS.md#e-12) | low |
| FINDING 12 | [E-06](DEFECTS.md#e-06) | high |

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

---

# Findings from the money-safety harness

Everything below was produced by `tests/money_safety/`, which runs the REAL table
canister wasm against the REAL mainnet ICP ledger wasm, installed at
`ryjl3-tyaaa-aaaaa-aaaba-cai` (the exact canister id the table canister hardcodes as
`ICP_LEDGER_CANISTER`), on PocketIC. The ledger is pinned:

```
URL     https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz
sha256  a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec
```

It is not a mock. It enforces real ICRC-2 allowances, the real 10,000 e8s fee and real
balances, and the harness refuses to run against a module whose hash does not match, because
a permissive ledger would silently void every result here. See
`tests/money_safety/README.md` for how to run it and for the definition of each invariant.

```
cd tests/money_safety
cargo test --test invariants  -- --test-threads=2    # one hand-written test per invariant
cargo test --test regressions -- --test-threads=2    # the reproducers referenced below
cargo test --test fuzz                               # the hostile-sequence fuzzer
```

The host-checkable subset (the payout-basis arithmetic, pinned against the real
`collect_contributions` and `poker_core::apply_side_pots`) also runs with no replica at all
under `cargo test -p table_canister --test money_safety`.

Every reproducer below is written to FAIL if the defect is silently fixed, so a fix cannot land
without updating this document in the same change.

## FINDING 06 (NEW) -- `notify_deposit` can never credit a deposit, and the ICP it was sent is stranded forever

**Severity:** HIGH. Permanent, unrecoverable loss of real user funds through a live,
advertised deposit path. Not on the frontend's happy path (the frontend uses
`claim_external_deposit`), so the blast radius is any wallet, script or integration that uses
the `get_deposit_address()` + `notify_deposit(block_index)` flow the Candid interface still
publishes.
**Status:** CONFIRMED by execution against the real ledger wasm. Reproducer:
`reg02_notify_deposit_cannot_decode_the_real_ledger_and_strands_the_money` in
`tests/money_safety/tests/regressions.rs`.
**Where:** `notify_deposit`, `src/table_canister/src/lib.rs`

A user transfers ICP to the canister's account identifier and calls `notify_deposit(block)`.
Real output from the harness:

```
icrc1_transfer 5.0 ICP -> canister main account            block 2
icrc1_balance_of(table, None) = 500000000                  the canister really holds it
notify_deposit(2) -> Err("Failed to decode ledger response:
    CandidDecodeFailed { type_name:
    \"(table_canister::notify_deposit::{{closure}}::QueryBlocksResponse,)\",
    candid_error: \"Fail to decode argument 0\" }")
get_balance() = 0                                          credited nothing
withdraw(1 ICP) -> Err                                     cannot get it back
icrc1_balance_of(table, None) = 500000000                  still in the canister
```

`withdraw` pays strictly against the caller's own escrow balance, and there is no
administrative withdrawal function anywhere in the canister, so that 5 ICP can never be
taken out by anyone, including a controller. It is stranded exactly as in FINDING 01.

There are **two independent bugs** on this path, and both must be fixed. Each is isolated by
the reproducer, which decodes the same reply bytes twice:

### BUG A: the reply is decoded as a one-element tuple instead of one value

```rust
// src/table_canister/src/lib.rs, notify_deposit
Ok(response) => match response.candid::<(QueryBlocksResponse,)>() {
```

In `ic-cdk` 0.19, `Response::candid::<R>()` is `decode_one::<R>()`. Asking for
`(QueryBlocksResponse,)` therefore asks the decoder for ONE value whose Candid type is
`record { 0 : QueryBlocksResponse }`. The reply holds one value of type
`QueryBlocksResponse`, a record with named fields. It can never match. Proven:

```
decode_one::<(QueryBlocksResponse,)>(reply) = Err(Subtyping error: record {
    457_361_687 : table1; 2_817_142_406 : table3; 3_385_467_812 : nat64;
    3_527_769_489 : nat64; 4_171_053_571 : table16; } is not a tuple type)
decode_one::< QueryBlocksResponse  >(reply) = Ok(...)
```

("is not a tuple type" is the decoder saying the reply's single value is a named record, not the
one-element tuple the canister asked for.)

The correct call is `response.candid::<QueryBlocksResponse>()`, or
`response.candid_tuple::<(QueryBlocksResponse,)>()`.

### BUG B: `AccountIdentifier` is declared as a record; the ledger returns a blob

```rust
// src/table_canister/src/lib.rs, notify_deposit
struct AccountIdentifier { hash: Vec<u8> }        // record { hash : blob }
```

`rs/ledger_suite/icp/ledger.did` says `type AccountIdentifier = blob;`. Because the variant
arm types do not match, `operation : opt Operation` decodes to **null** under Candid's opt
rule. No error, no trap. Real output, decoding the same bytes with the canister's own
declared types after fixing BUG A only:

```
Block { transaction: Transaction { memo: 0, icrc1_memo: None,
        operation: None,                       <-- the transfer silently vanished
        created_at_time: TimeStamp { .. } } }
```

so with only BUG A fixed, `notify_deposit` would reach its next branch and return
`"Transaction is not a transfer"` for every legitimate transfer. Decoding the identical
bytes with `AccountIdentifier = blob` yields the real transfer, amount 500000000, `to` a bare
32-byte blob.

### Fix order

Fix both at once, then re-run the reproducer, which is written to FAIL when the defect is
gone so the change cannot land without updating this document. Note that BUG B's failure mode
is silent, so fixing only BUG A would replace a clear error with a misleading one.

## FINDING 07 (NEW) -- `admin_reinit_table` strands every seated player's chips

**Severity:** HIGH, controller-only. Permanent, unrecoverable loss.
**Status:** CONFIRMED by execution. Reproducer:
`reg06_admin_reinit_table_strands_every_seated_players_chips`.
**Where:** `admin_reinit_table` -> `init_table_state`, `src/table_canister/src/lib.rs`

`admin_reinit_table` is documented as being "for recovery after upgrade issues". It calls
`init_table_state(config)`, which builds a brand new `TableState` with empty seats. Every
seated player's chips, and the whole pot, are dropped: not returned to escrow, not
withdrawable, gone. Real output, two players seated with 2 ICP each:

```
before   escrow 800000000   chips 400000000   pot 0   ledger_main 1200000000
after    escrow 800000000   chips         0   pot 0   ledger_main 1200000000
M1       delta = +400000000 (chips DESTROYED: money stranded in the canister that nobody owns)
```

This needs an owner decision rather than a drive-by patch: either return every seated
player's chips to escrow first, or refuse while anyone is seated. Either way it should not be
possible for a recovery tool to be the thing that loses the funds.

## FINDING 08 (NEW) -- `cash_out` is a second door into FINDING 05's orphaned-stake state

**Severity:** HIGH. Same impact as FINDING 05; a separate entry point that the FINDING 05
write-up explicitly assumed was closed.
**Status:** CONFIRMED by execution. Reproducers:
`reg08_a_timed_out_player_can_cash_out_mid_hand_and_orphan_their_stake` (the disconnect route,
end to end against the real canister) and `reg05_vacating_a_seat_mid_hand_orphans_the_leavers_stake`
in `tests/money_safety/tests/regressions.rs`, plus
`m1b_vacating_a_seat_mid_hand_moves_the_stake_into_the_deepest_stacks_pot` in
`src/table_canister/tests/money_safety.rs`, which pins the reallocation at host speed.

Real output from the disconnect route (3 players, 0.02 ICP each in the pot):

```
check_timeouts() -> PlayerTimedOut(2)          the quiet player is auto-folded
cash_out()       -> Ok(198000000)              198000000 chips returned to escrow MID-HAND
M1b: pot=6000000 but the seated players' total_bet_this_hand sums to 4000000 (delta 2000000)
```
**Where:** `cash_out`, `src/table_canister/src/lib.rs`

FINDING 05 contrasts `leave_table` with `cash_out` and says `cash_out` "guards the same
removal with `Cannot cash out while in a hand`". That guard is narrower than it looks:

```rust
return state.players.iter().flatten()
    .any(|p| p.principal == caller && !p.has_folded);
```

It only refuses players who have **not folded**. A player who folds, or who is auto-folded by
`check_timeouts` after the action timer expires, can then `cash_out` mid-hand, which sets
`state.players[i] = None` and vacates the seat exactly as `leave_table` does. Their
`total_bet_this_hand` disappears from `collect_contributions` while their money stays in
`state.pot`, and `poker_core::apply_side_pots` appends the orphaned amount to the highest bet
level, that is, to the pot only the deepest stacks can win.

Two consequences worth stating separately:

* A player does not need to call anything to reach the folded state. Simply going quiet until
  the timer expires and somebody calls `check_timeouts` folds them. So the state is reachable
  by a **disconnect**, not only by a deliberate call.
* Note also that `cash_out` returns the leaver's chips to escrow, from where they can be
  withdrawn, while their pot contribution is being reallocated behind them.

The exact reallocation, produced by running the real `collect_contributions` and
`poker_core::apply_side_pots`:

```
seat 0 all-in for 50, seats 1 and 2 in for 200 each, pot = 450

all seats present        pot[0] = 150  eligible [0,1,2]   pot[1] = 300  eligible [1,2]
seat 1 FOLDS, keeps seat pot[0] = 150  eligible [0,2]     pot[1] = 300  eligible [2]
seat 1 VACATES the seat  pot[0] = 100  eligible [0,2]     pot[1] = 350  eligible [2]
                                 ^ the honest all-in short stack's winnable main pot
                                   fell by 50, and the 50 moved into the pot only the
                                   deepest stack can win
```

## FINDING 09 (NEW) -- FINDING 02 Direction B is reached in ordinary play, and the canister says so

**Severity:** HIGH. Upgrades FINDING 02 from "mechanism confirmed, reachability not
demonstrated" to reachable, by the engine's own log line.
**Status:** CONFIRMED. Observed in randomised play by the fuzzer against the real canister.
**Where:** `poker_core::side_pots::build_side_pots_logged` reached via
`calculate_side_pots`, `src/table_canister/src/lib.rs`

FINDING 02 records that the capping branch mints or destroys chips whenever `state.pot`
disagrees with the sum of `total_bet_this_hand`, and that an attacker-reachable path to force
the divergence had not been demonstrated. During a 600-step randomised sequence the canister
printed, unprompted:

```
[Canister 7tjcv-pp777-77776-qaaaa-cai] BUG: Side pots (400000000) exceed total pot (0). Capping to pot amount.
```

That is 4 ICP of real contributions being capped to a pot of zero, that is, every side pot set
to zero, in a sequence containing nothing but public update calls, time jumps and upgrades.
The engine detects its own accounting inconsistency, prints it, and settles anyway.

Two things follow:

1. A detected inconsistency on a fund-custody path must trap, not warn. As it stands the
   canister has already told its operators, in its own logs, that it paid out of a basis it
   knew was wrong.
2. The harness now treats any canister log line containing `BUG:` or `CRITICAL:` as an
   invariant violation in its own right
   (`check_self_reported_inconsistency`, M1b `canister_reports_its_own_inconsistency`), so
   this cannot recur unnoticed.

The precise op sequence that drives `state.pot` to 0 while contributions remain has not been
isolated to a minimal reproducer yet. That is the one piece of outstanding work on this
finding.

## FINDING 10 (NEW, latent) -- pruning `VERIFIED_DEPOSITS` makes an old deposit block re-claimable

**Severity:** LOW today because FINDING 06 makes `notify_deposit` fail before it can ever
write to `VERIFIED_DEPOSITS`. MEDIUM the moment FINDING 06 is fixed, and it is a
chips-from-nothing primitive, so it must be fixed in the SAME change.
**Status:** code-read, not executed (it needs 10,000 successful deposits, which FINDING 06
makes impossible today).
**Where:** `periodic_cleanup`, `src/table_canister/src/lib.rs`

`VERIFIED_DEPOSITS` is the only thing that stops a deposit block being credited twice.
`periodic_cleanup` caps it:

```rust
if deposits.len() > 10_000 {
    let mut keys: Vec<u64> = deposits.keys().copied().collect();
    keys.sort();
    let to_remove = deposits.len() - 10_000;
    for key in keys.into_iter().take(to_remove) {
        deposits.remove(&key);          // oldest block indices forgotten
    }
}
```

Once a block index has been forgotten, `notify_deposit` for that block passes the
already-processed check again and credits the same on-ledger transfer a second time. The
memory bound is legitimate; the mechanism for enforcing it is not. A monotonically increasing
`min_unclaimable_block` watermark, rejecting anything at or below it, bounds memory without
forgetting that a block was spent.

Related, and also worth fixing in the same change: `deposit()` (the ICRC-2 pull) never records
the block index of the transfer it just made, so once FINDING 06 is fixed, that transfer's
block would satisfy every check `notify_deposit` performs. Its `from` is the caller's account
and its `to` is the canister's account, which is exactly what `notify_deposit` verifies, and
it ignores the `spender` field that distinguishes a `transfer_from`. That is a direct
double-credit of the same money. It is not exploitable today only because of FINDING 06;
`m6_a_deposit_block_is_credited_at_most_once` in
`tests/money_safety/tests/invariants.rs` sweeps every block index after an ICRC-2 deposit and
is written to fail the moment it becomes possible.

## FINDING 11 (NEW, low) -- dust at or below the transfer fee in a deposit subaccount can never be swept

**Severity:** LOW. Bounded by the fee (10,000 e8s per player per stuck deposit).
**Status:** CONFIRMED incidentally by the harness: a transfer of exactly the fee into a
deposit subaccount leaves it unclaimable.
**Where:** `claim_external_deposit`, `src/table_canister/src/lib.rs`

```rust
if balance <= transfer_fee {
    return Err("No claimable balance. ...");
}
```

There is no path that sweeps a subaccount balance at or below the fee, so anything a user
sends to their deposit address that does not exceed 0.0001 ICP is stranded there. The error
message tells the user to "send ICP to your deposit address first", which is misleading when
they already have.

## FINDING 12 (NEW) -- one action timeout ends the hand for everybody, because the disconnect timeout and the action timeout are both 30 seconds

**Severity:** HIGH. It removes the remaining betting rounds from every player at the table
without anyone choosing to check down, and it settles the hand out of the stale pre-flop
breakdown, which is FINDING 01. So a 30-second lull both takes away the turn and the river and
destroys whatever was already wagered post-flop.
**Status:** CONFIRMED by execution against the real canister. Reproducer:
`reg09_one_timeout_with_no_heartbeats_runs_the_whole_board_out_and_settles` in
`tests/money_safety/tests/regressions.rs`.
**Where:** `check_timeouts` + `count_players_can_act` + `advance_to_next_street`,
`src/table_canister/src/lib.rs`
**Found by:** the harness, incidentally. The first version of the FINDING 08 reproducer used a
30-second timeout and the hand kept ending before the folded player could `cash_out`.

`check_timeouts` marks every player whose `last_seen` is older than `DISCONNECT_TIMEOUT_NS`
(30 seconds, hardcoded inside the function) as `Disconnected` BEFORE it looks at the action
timer:

```rust
const DISCONNECT_TIMEOUT_NS: u64 = 30 * 1_000_000_000;
for player in state.players.iter_mut().flatten() {
    if player.status == PlayerStatus::Active && now > player.last_seen + DISCONNECT_TIMEOUT_NS {
        player.status = PlayerStatus::Disconnected;
```

`count_players_can_act` requires `status == PlayerStatus::Active`:

```rust
!p.has_folded && !p.is_all_in && p.status == PlayerStatus::Active
```

`table_1` and `btc_table_1` are deployed with `action_timeout_secs = 30` (see `icp.yaml`), so
the two thresholds are EQUAL. `last_seen` is only refreshed by `heartbeat` and by taking an
action, so the moment anybody times out, every player who has not sent a heartbeat in the last
30 seconds is already `Disconnected`, `count_players_can_act` drops below 2, and
`advance_to_next_street` calls `run_out_board`, which deals the turn and the river and goes
straight to a showdown in the same message.

Real output, three players, 1 ICP bet and called on the flop, then a 31-second lull with no
heartbeats:

```
before      phase Flop, pot 206000000, 2 players live (not folded, not all-in)
advance 31s; check_timeouts() -> PlayerTimedOut(1)
after       phase HandComplete, community_cards 5 (turn AND river dealt, nobody acted)
            every seated player marked Disconnected
            200000000 e8s destroyed by the forced settlement
```

Note what the destroyed amount is: the whole post-flop pot, settled out of the pre-flop
`side_pots` breakdown. `table_2` (45 s) and `table_3` (60 s) have some headroom between the two
thresholds, but `table_1` and `btc_table_1` have none.

The frontend heartbeats, so this needs the heartbeat to be failing or the tab to be
backgrounded. That is not an exotic condition: it is what a dropped connection looks like, and
it is the condition the `Disconnected` status exists to represent. The defect is that
representing it also silently ends the hand for players who are perfectly well connected.

`DISCONNECT_TIMEOUT_NS` being a hardcoded local constant, rather than derived from
`config.action_timeout_secs`, is what makes this a configuration trap rather than a bug an
operator could tune around.

## What the harness PROVED HOLDS

Stating the negatives matters as much as the defects. Over 9 seeded hostile sequences of 600
steps each (5,400 real IC messages, 78 completed hands, 95 real canister upgrades) across three
table shapes (heads-up, 6-max, 6-max with an ante), plus the hand-written per-invariant tests.
Full machine-readable output: `money-fuzz-report.json` (written to
`target/money-safety/money-fuzz-report.json` by default). Zero blocking findings, zero minimal
reproducers, 16 distinct documented-defect signatures, worst single-run stranded amount
606,500,000 e8s (6.065 ICP).

* **M2 LEDGER REALITY held everywhere.** The canister was never short: at no observed point
  did it owe more than its real ledger balance. Every discrepancy found was in the safe
  direction (money stranded inside the canister), never the dangerous one.
* **No chips were ever created from nothing.** No violation classified `FundCreation` or
  `DoublePay` survived. There is no demonstrated way for one player to take another's escrow,
  or to withdraw more than they deposited plus won.
* **M5 UPGRADE DURABILITY held across 60+ real `--mode upgrade` upgrades**, including
  mid-hand upgrades with money in the pot, hole cards dealt and an action timer running.
  Escrow, chip stacks, the deck, the community cards, the hand number and the timer all
  survived byte-identically, and hands remained playable afterwards.
* **The withdrawal reentrancy guard works.** Two `withdraw` calls for the same principal
  submitted into the SAME round produce exactly one payment:
  `A=Ok(block) B=Err("A withdrawal is already in progress")`. The escrow debit, the ledger
  debit and the amount delivered all match one withdrawal, to the e8, including the real fee.
* **Hostile amounts are refused, not clamped.** `u64::MAX` to `deposit`, `withdraw`, `buy_in`
  and `reload` all return a clean `Err` and change nothing. Buy-ins outside `[min, max]`,
  withdrawals above escrow, raises below the min-raise, raises above the stack, acting out of
  turn and taking an occupied seat are all refused.
* **The ICRC-2 deposit path and the withdraw path are ledger-exact.** `deposit(a)` moves
  exactly `a` into the canister and credits exactly `a`; the depositor pays the two real
  ledger fees. `withdraw(a)` debits escrow by `a`, moves `a` out of the canister, and delivers
  `a - fee`.
* **A hand with no post-flop money pays out exactly.** Chips awarded equals chips wagered, to
  the e8, so the no-rake property is real; FINDING 01 is a defect in the payout basis, not a
  rake.

## What these invariants CANNOT catch

A green run is easy to over-read, so the boundary is stated here as well as in the harness
README.

M1 through M6 are conservation and settlement properties. They are blind to a pure
**redistribution** between players. If the engine pays the wrong player, the total is unchanged
and every invariant above still holds. That is exactly the shape of FINDING 05 and FINDING 08,
and the only reason the harness sees those at all is M1b's attribution leg, which flags the
precondition rather than the misallocation.

Catching a wrong payout needs a different oracle: an independent settlement model that, given
the hole cards, the board and the contributions, computes what each seat is owed, compared
against what the engine actually paid. That model does not exist yet. "No blocking findings"
therefore means no chips were created, none were double-paid, and nothing was lost across an
upgrade. It does not mean the right player won.

Two further boundaries:

* The ckBTC table (`btc_table_1`, `Currency::BTC`) is not covered. Its ledger is
  `mxzaz-hqaaa-aaaar-qaada-cai` and its deposit path goes through the ckBTC minter, neither of
  which is installed in the harness. Everything above is ICP.
* FINDING 06 makes `notify_deposit` dead, so the deposit-block leg of M6 can currently only
  prove that nothing is credited twice while nothing is credited at all. That is why FINDING 10
  must be fixed in the same change as FINDING 06 and not after it.

## Two code facts noted while building the harness, not yet demonstrated

* `PENDING_WITHDRAWALS` and `LAST_WITHDRAWAL` are **not** fields of `PersistentState`, so
  neither survives an upgrade. The visible effect is that the 60-second withdrawal cooldown
  resets on every upgrade. It is not a fund-loss path on its own (the balance is debited
  before the transfer), but it does mean a rate limit can be cleared by an operator action.
* An upgrade that lands while a `withdraw` is awaiting the ledger drops the reply callback
  along with the heap, so the `Err` branch that refunds the escrow can never run. If the
  transfer had failed, the user's escrow stays debited with nothing delivered. Deliberately
  not claimed as demonstrated: the harness cannot yet hold a ledger reply open across an
  `install_code`.
