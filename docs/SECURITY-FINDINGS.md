# ClearDeck security findings

Findings that touch real user funds. Each entry states what was actually executed, so a
reader can tell a demonstrated defect from a suspected one.

> ## 🚨 STATUS, 2026-08-04: FINDING 10 WAS LIVE IN THE WORKING TREE, WAS DEMONSTRATED AS THEFT, AND IS NOW CLOSED
>
> **The thing this document warned about happened, was proved to be theft rather than a bug, and
> was then fixed.** Read this in order; the sequence is the point.
>
> **1. It went live.** FINDING 06 (the `notify_deposit` decode bug) was fixed in the working tree
> before FINDING 10 was, which is exactly the ordering [DEFECTS.md](DEFECTS.md#fix-ordering)
> forbids. The harness caught it immediately (wave 2, harness-gate agent, working tree, NOT
> mainnet):
>
> ```
> cd tests/money_safety && cargo test --test invariants -- --nocapture \
>   m6_a_deposit_block_is_credited_at_most_once
>
> MONEY-SAFETY: wasm under test sha256=32ed27488505b5f37591119ea0ad9fdef8d25d778ce4bd4d488c8b47a1c955ac
> src/table_canister/src/lib.rs sha256=ed987cf1461b8e6d2d7ccc0b78a7a86e2390f1964fea003f6eeae7c1fd277d4f
>
> DOUBLE CREDIT: notify_deposit re-credited money that was already credited by another
> path: [(4, 400000000)]. Escrow went 700000000 -> 1100000000 with NO new money on the ledger.
> ```
>
> **2. It was carried through to a completed theft.** A double credit on its own is a bug. It
> becomes theft when the invented balance leaves the canister as real ICP, and it does:
> `dr00_theft_one_icrc2_deposit_credited_twice_then_withdrawn` in
> `tests/money_safety/tests/deposit_replay.rs` deposits **3 ICP once** and ends with the attacker
> **2.9997 ICP richer in her own on-ledger wallet**, funded out of another player's escrow. Full
> transcript and the exploit in two calls: **[FINDING 10](#finding-10) below.**
>
> **3. It is closed.** `src/table_canister/src/lib.rs` now carries a single, documented
> anti-replay record (the `DEPOSIT ANTI-REPLAY` section) with a stated invariant: *for every
> ledger block index B, this canister credits escrow for B at most once over the entire lifetime
> of its state.* Memory is still bounded, but the bound raises a monotonic watermark past whatever
> it drops instead of forgetting it, so "forgotten" now means "permanently refused". Nine tests in
> `tests/money_safety/tests/deposit_replay.rs` gate it, including the theft reproducer itself,
> which is rebuilt from the current source on every run.
>
> **What is NOT closed:** an `install_code --mode reinstall` erases the anti-replay record along
> with every balance, after which historical blocks really sent to this canister's account become
> creditable again. See "the reinstall hazard" under [FINDING 10](#finding-10).
>
> ## ⛔ READ THIS FIRST — WHAT IS AT STAKE RIGHT NOW
>
> The mainnet table canisters custody **real ICP and real ckBTC**. Three of the findings below
> are confirmed by execution to cause **permanent, unrecoverable loss of real user funds**, and
> one is a **chips-from-nothing (fund-theft class) primitive** that was blocked only by
> another bug (**superseded by the status block above: it went live, was demonstrated as a
> completed theft, and has since been closed. Item 2 below is kept for the history**).
>
> **1. ~~Money is being destroyed in normal play, today.~~ FIXED 2026-08-04. FINDING 01, and
> with it FINDING 02 and FINDING 05.** Every showdown that followed any post-flop betting used to
> pay the winner only the pre-flop pot, and everything wagered on the flop, turn and river was
> debited from stacks and credited to nobody — inside the canister, **withdrawable by nobody,
> including a controller**. The payout basis is now built from the players' own contributions at
> payout time, `state.pot` and the stored side-pot breakdown can no longer move a chip, and the
> engine refuses to settle a hand whose awards do not equal what it collected. The exact hand from
> this document (a 64,000,000 pot that paid 4,000,000) now pays 64,000,000, and an independent
> settlement oracle driving the real canister agrees with it on all 17 of its deliberate hands,
> having previously convicted four separate payout defects. See
> [DEFECTS.md E-01](DEFECTS.md#e-01), [E-03](DEFECTS.md#e-03), [E-05](DEFECTS.md#e-05) and
> [E-35](DEFECTS.md#e-35), and `FINDING-01-chip-destruction.md`.
>
> **FINDING 07 still reaches the same terminal state by another route** (`admin_reinit_table`
> strands every seated player's chips), and **FINDING 12 still ends a hand early** on a
> 30-second lull on `table_1` / `btc_table_1` — it just no longer destroys the pot when it does.
>
> **2. FUND-THEFT, DEMONSTRATED, NOW FIXED. FINDING 10.** A single on-ledger transfer could be
> credited to a player's escrow **twice** and the excess **withdrawn as real ICP**. Two mechanisms:
> `periodic_cleanup` bounded `VERIFIED_DEPOSITS` by *forgetting the oldest block indices*, and
> `deposit()` never recorded the block index of the transfer it just made, so that block satisfied
> every check `notify_deposit` performs. It was unreachable only because FINDING 06 made
> `notify_deposit` fail before it could credit anything, a decode bug standing in for an access
> control. Both were fixed in the same change, in that order, as the fix ordering required. See the
> status block above and [FINDING 10](#finding-10).
>
> **3. ~~Any seated player can move contested money into the deepest stack's pot~~ FIXED
> 2026-08-04. FINDING 05 and FINDING 08.** It took one public update call, or simply
> disconnecting: no special privilege, no extreme values, ordinary stakes. Vacating a seat
> mid-hand deleted the record of what that player had put in while the money stayed in the pot,
> and the difference was appended to the pot only the deepest stacks could win. A stake is now
> recorded independently of seat occupancy (`TableState::departed_stakes`), so leaving the table
> changes only whether the player can WIN the money, exactly as folding does. The settlement
> oracle measured the redistribution end to end before the fix (20 chips out of an honest short
> all-in's main pot, with every chip conserved) and measures zero now. See
> [DEFECTS.md E-05](DEFECTS.md#e-05).
>
> Wave 1 deliberately froze engine behaviour so that ground truth could be established first, and
> every reproducer is written to FAIL when its defect is fixed, so a fix cannot land without
> updating this document in the same change. Wave 2 is fixing them: each finding's own **Status**
> line below says whether it is still open. **FINDING 06 and FINDING 10 are FIXED**, together, in
> that order. See the status block above.
>
> **The FINDING 06 / FINDING 10 fix ordering has been discharged.** It is preserved in
> [DEFECTS.md](DEFECTS.md#fix-ordering) because the reason it existed is the clearest worked
> example in this repo of why a decode bug is not an access control.

**The single prioritised queue for all of this is [docs/DEFECTS.md](DEFECTS.md)**, which
reconciles these findings with the harness and tooling defects found alongside them and gives each
one an id, a severity and a reproduce command. This file is the evidence; that file is the order of
work.

| here | DEFECTS.md | severity |
|---|---|---|
| FINDING 01 | [E-01](DEFECTS.md#e-01) | critical — **FIXED 2026-08-04** |
| FINDING 02 + FINDING 09 | [E-03](DEFECTS.md#e-03) | high — **FIXED 2026-08-04** |
| FINDING 03 | [E-08](DEFECTS.md#e-08) | medium |
| FINDING 04 | [E-13](DEFECTS.md#e-13) | low |
| FINDING 05 + FINDING 08 | [E-05](DEFECTS.md#e-05) | high — **FIXED 2026-08-04** |
| FINDING 06 | [E-04](DEFECTS.md#e-04) | high |
| FINDING 07 | [E-07](DEFECTS.md#e-07) | high |
| FINDING 10 | [E-02](DEFECTS.md#e-02) | **fund-theft (demonstrated, FIXED)** |
| FINDING 11 | [E-12](DEFECTS.md#e-12) | low |
| FINDING 12 | [E-06](DEFECTS.md#e-06) | high |

Detailed write-ups that predate this file live alongside it:

- `docs/FINDING-01-chip-destruction.md` — CRITICAL, confirmed on a local replica: every
  showdown with post-flop betting paid the winner only the pre-flop pot and permanently destroyed
  the rest. **FIXED 2026-08-04**; that document now opens with the fix, the gate that keeps it
  fixed, and what was reverted in a copy of the repo to prove the gate can fail.
- `docs/BACKEND-FUND-SAFETY-TODO.md`

---

## FINDING 02 — `state.pot` is the sole authority for side-pot totals, in both directions

> **STATUS: FIXED 2026-08-04.** `state.pot` is no longer any authority for the payout: the
> basis is built from the players' contributions (`poker_core::build_side_pots_from_contributions`,
> which has no `total_pot` parameter to be overridden by and no float anywhere), and `state.pot` is
> a cross-checked redundant accumulator. See [DEFECTS.md E-03](DEFECTS.md#e-03) for the fix, the
> proof, and why the mismatch is reported as `CRITICAL:` rather than trapped. Read below for what
> the defect WAS.


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

> **STATUS: FIXED 2026-08-04.** A vacated seat's stake is now recorded independently of seat
> occupancy (`TableState::departed_stakes`) and stays in the payout basis, so all three doors
> (`leave_table`, a post-fold `cash_out`, and a plain disconnect via `check_timeouts`) leave the
> money exactly where folding does. Gated by `reg05` and `reg08` in
> `tests/money_safety/tests/regressions.rs` and by the settlement oracle's `pinned_e05_*`. See
> [DEFECTS.md E-05](DEFECTS.md#e-05). Read below for what the defect WAS.


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
**Status:** CONFIRMED by execution against the real ledger wasm, then **FIXED** on 2026-08-04
together with FINDING 10, in the required order. Both BUG A and BUG B below are fixed in
`src/table_canister/src/lib.rs`; the gate that a legitimate transfer is now creditable *and*
creditable only once is `dr01_notify_deposit_credits_a_real_transfer_exactly_once` in
`tests/money_safety/tests/deposit_replay.rs`.
**Where:** `notify_deposit`, `src/table_canister/src/lib.rs`

> **The old reproducer fired, and has been inverted.** The wave-1 pin
> `reg02_notify_deposit_cannot_decode_the_real_ledger_and_strands_the_money` was written to pass
> only while this defect existed, and it duly failed the moment the decode was fixed, at
> `tests/regressions.rs:178` with `notify_deposit unexpectedly returned Ok(...)`. Its owner has
> since replaced it with `reg02_notify_deposit_can_read_the_real_ledger_and_never_fails_to_decode`,
> which asserts the stronger property: **no rejection from `notify_deposit` may ever again be a
> decode failure**, for a real Transfer block and for a block that is not a deposit for this
> canister. That second half is what a "does it credit?" test cannot see, because a wrong
> `Operation` shape decodes to `None` under Candid's `opt` rule and is reported as
> "not a transfer". `cargo test --test regressions` is green.

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

> **STATUS: FIXED 2026-08-04, with FINDING 02.** The routine that logged
> `BUG: Side pots (...) exceed total pot (...)` and settled anyway is off the payout path, and the
> money-safety harness no longer tolerates that line: `documented::TOLERATED_SELF_REPORTS` is
> empty, so any `BUG:`/`CRITICAL:` line from the canister now fails the run. See
> [DEFECTS.md E-03](DEFECTS.md#e-03). Read below for what the defect WAS.


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

<a id="finding-10"></a>

## FINDING 10 -- a deposit block could be credited twice, and the excess withdrawn (FUND THEFT, FIXED)

**Severity:** **FUND THEFT.** Not "a bug that mints chips": a completed theft. Real ICP left the
canister to an attacker's own ledger wallet in excess of everything she ever deposited, funded out
of another player's escrow.
**Status:** **DEMONSTRATED end to end, then FIXED**, both on 2026-08-04, in the same change and in
the required order. Demonstrated against the **real mainnet ICP ledger wasm** (sha256
`a47a915e…`) installed at `ryjl3-tyaaa-aaaaa-aaaba-cai`, the exact canister id the table canister
hardcodes, on PocketIC. Not a mock and not a simulation of the ledger.
**Where:** `notify_deposit`, `deposit`, `periodic_cleanup`, `VERIFIED_DEPOSITS`,
`src/table_canister/src/lib.rs`. Fix: the `DEPOSIT ANTI-REPLAY` section of the same file.
**Reproducers:** `tests/money_safety/tests/deposit_replay.rs` (ten tests, `dr00`…`dr09`; `dr08` is
`#[ignore]`d for runtime, and has been executed and passed).

### The two mechanisms

Both turn one on-ledger movement into two escrow credits. `VERIFIED_DEPOSITS` was the only thing
standing in the way, and each mechanism defeated it in a different way.

**Mechanism (a): `periodic_cleanup` FORGOT block indices.**

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

Once an index was forgotten, `notify_deposit` for that block passed the already-processed check
again and credited the same transfer a second time. The memory bound was legitimate, because an unbounded map eventually makes `pre_upgrade` fail to
serialise, which bricks a canister with the funds inside. The mechanism for enforcing it, though,
turned *spent* into *unknown*.

**Mechanism (b): `deposit()` never recorded the block its own pull wrote.** `deposit(n)` performs
`icrc2_transfer_from(from = caller, to = canister)` and credits `n`, discarding the block index the
ledger returned. That block's `from` is the caller's account and its `to` is the canister's
account, which is *exactly* the pair `notify_deposit` verifies, and `notify_deposit` ignored
`spender`, the one field that says a `transfer_from` did it. So `notify_deposit(<that block>)`
credited the same `n` again. This leg needs no pruning, no 10,000 deposits and no waiting: **two
ordinary calls, no privilege, ordinary stakes.**

Neither was reachable while FINDING 06 stood, because `notify_deposit` could not decode the
ledger's reply at all. That is **a decode bug standing in for an access control**, and on
2026-08-04 FINDING 06 was fixed first, which made mechanism (b) live in the working tree. The
harness caught it in one run (transcript in the status block at the top of this file).

### The theft, executed

`dr00_theft_one_icrc2_deposit_credited_twice_then_withdrawn`. Two players, real ICP ledger. Bob is
an ordinary honest player; his deposit is what actually gets stolen.

```
bob    deposit  20 ICP  (honest)                canister holds 23 ICP
alice  deposit   3 ICP  (honest)  -> block 5    escrow(alice) = 3 ICP

alice  notify_deposit(5)                        escrow(alice) = 6 ICP
DR-00 double credit: block 5 credited twice. escrow 300000000 -> 600000000
      while the canister's ledger balance stayed at 2300000000
                                                ^ NOT ONE e8 OF NEW MONEY ARRIVED

alice  withdraw(6 ICP)                          -> succeeds
DR-00 THEFT: alice's own ledger wallet 1000000000000 -> 1000299970000
      (profit 299970000 e8s) having deposited 300000000 once

DR-00 SHORTFALL: bob's escrow is 2000000000 but the canister holds only 1700000000
```

Alice ends **2.9997 ICP richer in her own on-ledger wallet**: one whole deposit, less the three
ledger fees the round trip costs her (`icrc2_approve`, `icrc2_transfer_from`, and the withdrawal
transfer). Bob's escrow says 20 ICP and the canister holds 17, so **bob can no longer be paid what
the canister says he owns.** That is the line between a bug and theft, and it was crossed.

Reproduce:

```
cd tests/money_safety
cargo test --test deposit_replay -- dr00 --nocapture
```

The test builds the vulnerable module **itself**, from the current source, by applying two
reversals (`E02_REVERSALS` in that file) that remove exactly the two defences the fix added, and
installs it with a real `install_code --mode reinstall`. So the reproducer does not depend on a
saved binary and cannot rot into a story: if the fix is ever refactored such that a reversal no
longer applies, the test fails and says which hunk, and a reader has to re-establish by hand that
the hole is still shut.

### The fix, and the invariant it guarantees

`src/table_canister/src/lib.rs`, the `DEPOSIT ANTI-REPLAY` section. The invariant is stated there
in words, at the code:

> For every ledger block index B, this canister credits escrow for B **at most once over the
> entire lifetime of its state.**

Four parts:

1. **One record, one writer.** `claim_deposit_block(block_index, who)` is the only writer of
   `VERIFIED_DEPOSITS` and the only place the rule is expressed. All three doors,
   `notify_deposit`, `deposit` and `verify_ckbtc_deposit`, must call it and see `Ok(())` before they
   touch `BALANCES`, with **no `await` between the claim and the credit**, so the two cannot come
   apart.
2. **A monotonic watermark instead of forgetting.** A block index is refused if it is
   `< DEPOSIT_WATERMARK` **or** present in `VERIFIED_DEPOSITS`. When the record exceeds its bound,
   `bound_verified_deposits` raises the watermark past **exactly** the indices it is about to drop
   and then drops them. Memory is still bounded; "forgotten" now means "permanently refused".
   Both the set and the watermark are in `PersistentState`, so an upgrade carries them over.
3. **`deposit()` records the block its pull wrote**, before crediting.
4. **`notify_deposit` refuses a block whose `spender` is this canister's own account.** Such a
   block can only have been written by our own `deposit()` pull, and this is the defence that
   covers pulls made *before* part 3 existed. That matters, because the mainnet canisters already
   hold state. The same check is on the ckBTC door (`verify_ckbtc_deposit`), which had it missing
   in the first draft of this fix: `to.owner == canister` and `from.owner == caller` are both true
   of a `transfer_from`, so without the `spender` test a pre-fix `deposit()` on a BTC table would
   have stayed replayable. `dr00` would not have caught that, because it runs on an ICP table.

### What the fix costs, stated plainly

A raw transfer whose block index has fallen below the watermark can never be credited **even
though it was never credited**. Reaching that state takes `MAX_VERIFIED_DEPOSITS` (10,000) later
deposits recorded before the sender ever calls `notify_deposit`. The trade is deliberate and
one-directional. A refused late deposit is **recoverable**: the ICP is still on the ledger in this
canister's account, and the error message says so and quotes the block index. A double credit is
**not** recoverable, because the invented balance leaves as somebody else's money.
`dr07` pins both halves of that behaviour, and `dr08` pins them **at the shipped
`MAX_VERIFIED_DEPOSITS` of 10,000**: 10,001 real `icrc2_transfer_from` calls through the real ICP
ledger, 20 minutes of PocketIC, wasm sha256 `a7d1243e…`. Executed and green. The canister's own log
line and the two refusals:

```
DR-08 after 10001 ICRC-2 deposits: watermark=0 recorded=10002
[canister] deposit replay protection: watermark raised to 6
           (dropped 2 of 10002 recorded block indices; they remain permanently uncreditable)
DR-08 after the bound ran: watermark=6 recorded=10000

DR-08 already credited block 2 refused: Deposit block 2 is below this table's deposit
  replay-protection watermark (6) and can no longer be credited automatically. Your transfer is
  still on the ledger in this canister's account. Contact the table operator and quote block
  index 2. (Only reachable if more than 10000 later deposits were recorded before you claimed
  this one.)
DR-08 never claimed block 3 refused: <same, block 3>
```

Both refusals then survive a real `install_code --mode upgrade`, and the watermark is asserted never
to move down. Block 2 was the credited raw transfer and block 3 the never-claimed one; the two
indices the bound dropped were 2 and 5, so the floor landed at 6 and covers both.

`get_deposit_replay_state() -> (watermark, recorded_block_count)` was added so an operator or a
user can see whether a given block index is still claimable without guessing.

### The reinstall hazard: OPEN, not fixed

`install_code --mode reinstall` erases `VERIFIED_DEPOSITS` and `DEPOSIT_WATERMARK` along with every
balance. After that the watermark is 0 and **every historical block that really was sent to this
canister's main account becomes creditable again.** Each such block still only credits its own
`from`, so total credits cannot exceed total deposits ever made. But the canister's holdings have
since been reduced by withdrawals and payouts, so the replay would leave it owing more than it
holds, i.e. a shortfall paid out of later depositors' money.

This is **not** defended in code, deliberately: a reinstall already destroys all escrow, which is a
strictly worse event, `post_upgrade` already panics rather than let a bad restore through, and
`CLAUDE.md` already forbids reinstall on production canisters. Defending it properly needs a floor
seeded from the ledger's chain length at install time, which `init` cannot do (no `await`), so it
would need a controller-only monotonic setter. **If a production table canister is ever
reinstalled, the anti-replay floor must be re-seeded before deposits are re-enabled.**

A *fresh* canister is safe against the ledger's tens of millions of pre-existing blocks for a
different and stronger reason, which does not depend on the watermark at all: `notify_deposit`
requires the block's `to` to equal this canister's own account identifier, and no block written
before this canister existed can name it.

### A coverage gap, not a defect: the ckBTC door has no harness at all

`grep -ri ckbtc tests/money_safety` returns nothing. The harness installs the real ICP ledger and
only the ICP ledger, and `TableConfig` in `tests/money_safety/src/table_api.rs` has no BTC shape, so
**`verify_ckbtc_deposit` is executed by no test in this repository.** Everything asserted about the
ckBTC door here is by code inspection and by the fact that it shares `claim_deposit_block` with the
ICP door. `btc_table_1` is deployed on mainnet.

Closing it means pinning a real ckBTC ledger wasm (and, for the native-BTC path, the ckBTC minter)
the way `wasms.rs` pins the ICP ledger, and giving `TableConfig` a BTC constructor. That is a
harness change, in a file with a different owner, and is the largest remaining untested surface on
the money paths.

### A residual weakness this fix INTRODUCES: the watermark can be pushed up on purpose

Stated plainly because it is new, and it is new because of the fix, not despite it.

`deposit()` now records a block index on every call, and `deposit()` has **no rate limit at all**.
That was already true before this change, and it is contrary to this project's own stated rule
("All user-facing update calls should be rate-limited to prevent DoS", `CLAUDE.md`). Before the
fix, `VERIFIED_DEPOSITS` only grew through `notify_deposit`, which *is* rate limited to 5 per
minute per caller. So an attacker can now fill the record as fast as the ledger will take
transfers:

* 10,001 minimum deposits (20,000 e8s each) push `DEPOSIT_WATERMARK` past roughly 10,000 block
  indices. The deposits themselves are withdrawable again, so the real cost is the ledger fee:
  **about 1 ICP.**
* Effect: any raw transfer whose block index is now below the watermark can no longer be credited
  by `notify_deposit`. The victim's ICP is **not taken**: it sits in the canister's account, it is
  operator-recoverable, and the error message quotes the block index. But a user who sent ICP and
  waited is now blocked from claiming it themselves.

It is griefing, not theft, and it is bounded by that: nothing about it lets the attacker withdraw
anyone else's money. It is not defended here because every option costs something a security fix
should not spend silently:

* **Rate-limit `deposit()`.** The cleanest fix: 30/minute per caller would be invisible to a real
  player and would turn "minutes" into hours. It needs a new rate-limit map and therefore a new
  persisted field, and it touches the frontend's primary deposit path. This is the recommended
  follow-up.
* **Raise `MAX_VERIFIED_DEPOSITS`.** Raises the attacker's cost linearly (200,000 makes it ~20
  ICP) at ~12 MB of heap and a bigger `pre_upgrade` blob. Changing the canister's memory
  characteristics by 20x as a side effect of a replay fix is not a change to make without measuring
  `pre_upgrade` on a real replica.
* **Stop letting `deposit()`-recorded indices drive the watermark.** Correct in principle, because
  a block written by our own pull is refused by the `spender` check whether or not it is in the
  record. It needs a per-entry flag, which changes the persisted shape of `verified_deposits`, and
  it splits "one record, one writer", the property that makes the fix auditable.

### The other two deposit doors: audited, findings below

Asked of each: can one on-ledger movement become two escrow credits, and can a concurrent pair of
calls each credit the same movement?

**`deposit()` (ICRC-2 approve + `transfer_from`).** Was the live theft primitive via mechanism (b);
fixed by parts 3 and 4 above. On concurrency it was already safe, and for a reason outside this
canister: two concurrent `deposit()` calls against one allowance are separated by the **ledger's**
allowance accounting, not by anything here. `dr06` shows the second returning
`Insufficient allowance. You approved 0.0000 ICP but tried to deposit 5.0000 ICP`. A new
interleaving that the fix itself *introduces* is covered by `dr09`: a `notify_deposit` naming the
block a concurrent `deposit()` is about to write can pass the cheap pre-flight check and resume
after that block has been recorded. Across five round-offsets, one movement produced exactly one
credit every time: once refused by the `spender` check, which does not depend on timing at all,
and four times by the recorded block index.

**`claim_external_deposit()` (subaccount sweep).** **No double-credit hole found, and nothing was
changed.** It credits `balance - fee` where `balance` is read from the ledger for the caller's own
derived subaccount, so there is no block index to replay and the amount is not caller-supplied. Two
concurrent claims over one arrival are separated, again, by the **ledger**: both read the same
balance, the first sweep empties the subaccount, and the second fails with
`Sweep transfer failed: InsufficientFunds { balance: Nat(0) }` and credits nothing (`dr05`,
observed). The sweep does write a ledger block whose `to` **is** the canister's main account, which
is half of what `notify_deposit` wants. Its `from`, though, is
`account_identifier(canister, deposit_subaccount(caller))`, not the caller's own account, so the
sender check refuses it (`dr03`, asserted against the real block).

Two things about this door are worth recording even though they are not double-credit holes:

* It relies on the ledger rejecting the second sweep rather than on its own state. That is sound
  here, but it is one `icrc1_transfer` failure-mode change away from not being sound, and unlike
  the other two doors it has no durable record of its own. It is the door to re-audit if the
  sweep is ever changed to transfer a *fixed* amount rather than the balance it just read.
* If a second real deposit arrives between the first sweep and the second claim's transfer, the
  second claim can sweep it using the **stale** amount it read earlier. That credits at most what
  actually moved (`dr05` asserts `escrow <= moved`), so it is not creation. But the accounting is
  approximate in a place where it does not need to be.

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

---

## FINDING 13 (high) -- the E-05 fix pays a departed player's stake to whoever takes their chair -- **DEMONSTRATED, THEN FIXED**

> ### STATUS 2026-08-04, wave 3: escalated from "reachable" to **DEMONSTRATED**, then closed.
>
> Wave 2 filed this as high rather than as executed misdirection, because the two ingredients had
> each been reached by the fuzzer but had never been observed **composed in one hand**. They
> compose. The sequence below is five ordinary public API calls, needs no privilege, and was run
> against the real table canister on PocketIC with the real ICP ledger. Observed, with the real
> principals the run produced:
>
> ```
> M8: stake 2000000 e8s left in the pot by 74yuz-2axoe-...-bqsze-dae;
>     the chair was then taken by f6m43-ks6kd-...-xks4f-rae
> M8:   74yuz-2axoe-at4sh-2qtsk-etfcs-ek6kq-nkw7x-d2qef-juk6h-bqsze-dae   -2000000
> M8:   f6m43-ks6kd-wlgjj-l3da3-lu2nl-gxcrc-dyipl-ynbak-nad3v-xks4f-rae   +2000000
>
> M8_PRINCIPAL_ATTRIBUTION violated:
>   delta=2000000 phase=HandComplete -- principal f6m43-... staked NOTHING in this hand and
>   came out of it +2000000 e8s richer.
> ```
>
> **0.02 ICP of one player's money credited to another player, through the public API, with every
> total balancing.** The amount is the blind at the harness's table; nothing about the mechanism
> bounds it -- the stake could be a full stack.
>
> **The fix** the owner now travels with the money. `Payout::principal` is a plain `Principal`
> taken from the [`Stake`] that generated the payout (or from the live claim that won the layer),
> `principal_of(state, seat)` is **deleted**, and there is no longer any function in `lib.rs` that
> can turn a seat index into a payee. `apply_payouts` credits `payout.principal`: their stack if
> they are sitting in that seat, otherwise their escrow.
>
> **The gates** four, and all four go RED against the pre-fix build:
>
> | gate | where | asserts |
> |---|---|---|
> | `payout_tests::finding13_*` (4 tests) | `src/table_canister/src/lib.rs` | the pure plan NAMES the owner; applying it credits the owner's escrow; two stakes at one seat reach two different owners; a pot share names the card-holder |
> | `M8_PRINCIPAL_ATTRIBUTION` | `tests/money_safety/src/invariants/attribution.rs` | per-principal `escrow + chips` delta vs what the rules owe that PERSON, plus the oracle-free corollary: **a principal who staked nothing cannot gain** |
> | `m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair` | `tests/money_safety/tests/invariants/principals.rs` | the composed sequence, end to end, on the real canister |
> | `a_chair_that_changes_hands_mid_hand_pays_the_person_and_not_the_chair` | `tests/settlement/tests/settlement.rs` | the settlement oracle now has a PRINCIPAL column and `Bench::gate` fails on it |
>
> The settlement oracle's report on the pre-fix build is the clearest single statement of the
> class:
>
> ```
> Per-SEAT DIFF      [(0, -2), (1, 0), (2, 0), (3, 0)]
> Per-PRINCIPAL DIFF [("2f6yp-...-bqe", +2), ("6ui2b-...-oae", -2)]
> ```
>
> Everything below is the original write-up, kept because it is the evidence.

Found 2026-08-04 by the wave-2 review, in the code the wave-2 payout fix introduced. **Every
conservation invariant passes while this happens**: the plan awards exactly what it collected,
the table's total value does not change, no chip is destroyed, and the canister logs no
`CRITICAL:` line. What is wrong is WHO HAS THE MONEY.

**Where** `src/table_canister/src/lib.rs`

* `principal_of()` (line ~4102) resolves a payout's principal by looking the **seat** up first
  and only falls back to `departed_stakes` when the chair is **empty**:

  ```rust
  fn principal_of(state: &TableState, seat: u8) -> Option<Principal> {
      if let Some(p) = state.players.get(seat as usize).and_then(|p| p.as_ref()) {
          return Some(p.principal);          // <-- the CURRENT occupant
      }
      state.departed_stakes.iter()
          .find(|d| d.hand_number == state.hand_number && d.seat == seat)
          .map(|d| d.principal)
  }
  ```

* All three `Payout` construction sites (lines ~4000, ~4052, ~4080) take their principal from
  it. So a `PayoutReason::Refund` of a **departed** stake at a chair that has since been
  re-occupied names the **new occupant**, not the player the money belongs to.

* `apply_payouts()` (line ~4185) has an arm written for exactly this case:

  ```rust
  (Some(occupant), Some(owed)) if occupant != owed => { /* pay `owed`'s escrow */ }
  ```

  It is **dead code**. `owed` came from `principal_of`, which returned `occupant`, so the two
  sides of the comparison are the same value read from the same seat. Control falls through to
  `(Some(occupant), _)`, which does `p.chips += payout.amount` -- the departed player's stake
  is added to the stranger's stack.

**The sequence** every step is an ordinary public API call:

```
seat 1 (alice) is in a live hand with 50 in the pot
alice calls leave_table()      -> record_departed_stake(1, alice, 50); players[1] = None
                                  alice's remaining STACK goes to her escrow; the 50 stays
a stranger calls join_table(1)  -> seated SittingOut, no hole cards   (docs/DEFECTS.md E-36)
the hand settles with no live claim on that layer
                                -> plan_payouts refunds 50 to seat 1, named to the STRANGER
                                -> apply_payouts credits the STRANGER's stack
```

**Reproducer** a host test driving the REAL `plan_payouts` / `determine_winners` over a real
`TableState`, no replica needed. Observed output:

```
C3: alice's stake is owed to <alice>; the plan names Some(<stranger>)
C3: stranger chips 7 -> 57;  alice escrow 0 -> 0
plan.conserves() == true      // awarded == collected, exactly
```

Full test: `payout_tests::c3_a_departed_stake_is_paid_to_whoever_took_the_chair`, kept in the
reviewer's scratch copy at
`$SCRATCH/c3repo/src/table_canister/src/lib.rs`.

**Reachability of each ingredient, against the real canister**

* the refund-to-a-departed-player branch: reached by the money-safety fuzzer at 1200 steps,
  which logged `refunded 2500000 to the escrow of toldy-...` and
  `refunded 2000000 to the escrow of weeos-...`.
* a chair re-occupied mid-hand: reached by the same fuzzer, which logged 296
  `WARNING: seat 2 carries both a live stake and a departed stake` lines (E-36).

The two were **not** observed composed in one hand, so this is filed as high rather than as
demonstrated fund theft. Both ingredients are individually reachable through the public API
with no special privilege.

> **WAVE 3 CLOSED THAT GAP: they compose, in five calls.** Now a permanent test
> (`m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair`):
>
> ```text
> alice(0) bob(1) carol(2) are dealt in; all three post to the flop
> alice   leave_table()  -> departed stake (seat 0, owner alice) stays in the pot;
>                           TWO active players remain, so the hand does NOT end
> dave    join_table(0)  -> takes alice's CHAIR mid-hand: SittingOut, no hole cards
> dave    sit_in()       -> Active (E-36), so count_active_players now counts him
> bob     leave_table()  -> carol + dave remain, so the hand STILL does not end
> carol   leave_table()  -> only dave is left and he holds no cards, so the hand settles
>                           with no live claim on any layer: every stake is refunded
> ```
>
> The refund of seat 0 is alice's money, and pre-fix it was credited to dave's stack. The reason
> the two ingredients compose is the step nobody had written down: **`leave_table` only ends the
> hand when `count_active_players` drops to one, and a mid-hand arrival who calls `sit_in()`
> counts** -- so the newcomer keeps the hand alive while every real player walks out of it.

**Second variant, same root cause** two departed stakes at ONE seat owed to two DIFFERENT
principals (alice leaves seat 1; a stranger takes it, bets via E-36, then leaves too).
`principal_of`'s `.find()` returns the FIRST match, so the second player's refund is credited
to the first player's escrow.

**Suggested fix** carry the owner with the money instead of re-deriving it from the seat. The
seat is not the identity: `Contribution` / `DepartedStake` already know whose chips these are,
so `Payout.principal` should be populated from the stake that generated it, and a refund of a
departed stake should always go to that stake's `principal`'s **escrow**, never to the chair.
That also makes the `occupant != owed` arm reachable, which is what it was written for.

### THE FIX AS LANDED (wave 3)

Exactly that, plus one thing the suggestion did not say: make the mistake **unrepresentable**
rather than merely corrected.

* **`Stake { seat, owner, amount, relinquished }`** is the new payout basis (`hand_stakes`).
  `poker_core::Contribution` stays seat-keyed and stays the input to the pot LAYERING, which is
  genuinely a seat question; ownership is not, so it is carried alongside instead of being
  looked up. `hand_contributions` is now `hand_stakes` with the owners dropped, and its doc
  comment says it may only be used where ownership is irrelevant.
* **`Payout::principal` is a plain `Principal`, not an `Option`.** A refund takes it from
  `Stake::owner`; a pot share takes it from the live claim that won the layer, which by
  construction is the player sitting in that seat holding those cards. The
  `(None, None) -> trap` arm is gone because there is nothing left that can produce it.
* **`principal_of(state, seat)` is DELETED, not fixed.** There is now no function in `lib.rs`
  that turns a seat index into a payee, so a future edit cannot reach for the wrong helper.
* **`apply_payouts` matches on the owner, not on the chair.** If the payout's principal is
  sitting in that seat, the credit goes into their stack; otherwise into their escrow -- which
  is where a departing player's stack went when they left, so it is the account they can
  withdraw from. The log line for the occupied-by-somebody-else case is deliberately written
  WITHOUT a `CRITICAL:`/`WARNING:` prefix: nothing is inconsistent, the engine is paying the
  right person, and the money-safety classifier treats every self-reported `CRITICAL:`/
  `WARNING:` line as a finding that stops the run.
* **`push_winner` aggregates by `(seat, principal)`, not by seat**, and only attaches hole cards
  when the seat's occupant IS the principal being credited. Otherwise the hand history reports
  one player's money -- and one player's cards -- under another player's name. That is the
  reporting face of the same defect, and the frontend reads that list.

**Second variant** is closed by the same change: two `DepartedStake`s at one seat produce two
`Stake`s with two owners, and each is refunded to its own owner
(`finding13_two_departed_stakes_at_one_seat_each_reach_their_own_owner`).

**Why no existing gate catches it** the settlement oracle compares per-SEAT deltas, and the
seat is paid the right amount -- it is the principal behind the seat that is wrong. The
money-safety invariants are conservation-based and this conserves exactly. The builder's own
2,000-case sweep in `every_settlement_pays_out_exactly_what_it_collected` never populates
`departed_stakes`, so it cannot construct the state at all. A gate for this has to assert on
PRINCIPALS, not on seats.

---

## FINDING 14 (high) -- two agents each added a persisted field that is not a Candid-compatible addition, and one of them destroys every chip at the table SILENTLY -- **FIXED**

> ### STATUS 2026-08-04, wave 3: both fields are `opt`, and a real cross-version upgrade test is the gate.
>
> **What landed, in one change** (either half alone is worse than neither, which is the whole
> shape of this finding):
>
> * `PersistentState::deposit_watermark: Option<u64>` -- `None` restores as a floor of 0, and the
>   restore only ever RAISES the floor, so no decode outcome can re-open a closed E-02 window.
> * `TableState::departed_stakes: Option<Vec<DepartedStake>>`, read through
>   `departed_stakes()` / `departed_stakes_mut()` / `clear_departed_stakes()` so no call site has
>   to care whether it is `None` or `Some(vec![])`.
>
> **The gate**: `m7_state_written_by_the_previous_release_survives_the_upgrade_exactly` in
> `tests/money_safety/tests/invariants/upgrade_across_versions.rs`. It builds `801aa79` from git
> (`git archive` into `target/money-safety/`, then `cargo build`), installs it on PocketIC with
> the real ICP ledger, creates escrow for three principals through the real ICRC-2 deposit path,
> seats them, deals a hand and stops on the flop, then does a real
> `install_code --mode upgrade` to the module under test and requires the upgrade to **succeed**
> with every one of these unchanged: per-principal escrow, every seat's principal/chips/
> contribution/fold flag/**hole cards**, the pot, the whole betting state, the board, the deck and
> the deck cursor, the side-pot breakdown and the shuffle commitment. It then claims a ledger
> block transferred before the upgrade, upgrades AGAIN, and requires the replay to still be
> refused -- so the anti-replay record is shown to survive an upgrade -- and finally plays the
> restored hand to completion and requires it to settle conserving.
>
> Observed on the fixed tree:
>
> ```
> M7: on 5e12d25bf4d2 -> escrow 900000000 across 3 principals, seated chips 594000000,
>     pot 6000000, hand 1 in phase Flop with board 3
> M7: cross-version upgrade 5e12d25bf4d2 -> 5996594741af SUCCEEDED with every e8, every stack,
>     every card and the anti-replay record intact, and the restored hand settled cleanly.
> ```
>
> **It goes RED on either regression, by two different routes. Both executed:**
>
> Revert `deposit_watermark` to `u64` -- the upgrade is REJECTED:
>
> ```
> Panicked at 'CRITICAL: Failed to restore state from stable memory:
>   "Custom(Fail to decode argument 0 ... Subtyping error: field deposit_watermark is not
>    optional field)". Upgrade REJECTED to protect user funds.'
> the cross-version upgrade was REJECTED, so the module under test CANNOT BE DEPLOYED over
> existing state.
> ```
>
> Revert `departed_stakes` to `Vec` -- the upgrade is ACCEPTED and the table is wiped:
>
> ```
> assertion `left == right` failed: an accepted upgrade changed the SEATS: ... HOLE CARDS.
>   left: []
>  right: [(0, <alice>, 198000000, 2000000, false, Some((Tc, Kc))),
>          (1, <bob>,   198000000, 2000000, false, Some((Th, 3c))),
>          (2, <carol>, 198000000, 2000000, false, Some((Ts, Ah)))]
> ```
>
> 594,000,000 e8s of seated chips gone, upgrade reported successful. Note what the second case
> proves about the digest guard: `table_was_present` restores as `None` from `801aa79` state, so
> **the guard cannot fire for this upgrade at all**. It protects the NEXT non-`opt` addition, not
> this one. The test, not the guard, is what catches it.
>
> **`pre_upgrade` now TRAPS on a failed `stable_save`** instead of logging and proceeding. See
> "The pre_upgrade decision" at the end of this finding for the argument.

Found 2026-08-04 by the wave-2 coherence pass, reconciling `src/table_canister/src/lib.rs`
after three agents edited different regions of it. Neither agent could see the other's field.
The two together are worse than either alone, which is exactly the class of defect a coherence
pass exists to find.

**Where** `src/table_canister/src/lib.rs`

| field | added by | Candid type | where it sits |
|---|---|---|---|
| `PersistentState::deposit_watermark` | the E-02 deposit anti-replay fix | `nat64` | top level of the persisted record |
| `TableState::departed_stakes` | the E-05 payout-basis fix | `vec record {...}` | **nested inside** `PersistentState::table_state`, which is `opt TableState` |

Both carry `#[serde(default)]` and both comments claim that makes them
backward-compatible. **Candid does not honour `serde(default)`.** Only `opt`, `reserved` and
`null` may be added to a record and still read state written before the field existed. A bare
`nat64` or `vec` may not.

### What each one does on `install_code --mode upgrade` from state written before wave 2

Measured by encoding the pre-wave-2 record shape with `candid 0.10.20` -- the exact version the
canister links -- and decoding it as the shipped shape. Probe:
`$SCRATCH/candid-upgrade` (`cargo run`), observed output:

```
SHIPPED  (u64 watermark + vec departed_stakes): REFUSED  ...
    wire_type: nat64, expect_type: nat64, field_name: Named("deposit_watermark")
HALF-FIX (opt watermark + vec departed_stakes): DECODED  NewHalf { ..., table_state: None }
OPT-BOTH (opt watermark + opt departed_stakes): DECODED  NewOpt { ..., table_state: Some(...) }
```

Read the middle line. That is the whole finding.

### Executed end to end against the real canister

Not only a decode probe. `m7_an_upgrade_from_the_previous_release_never_silently_loses_funds`
(`tests/money_safety/tests/invariants/upgrade_across_versions.rs`, added by this pass and wired
into the `invariants` target the gate already runs) builds the `801aa79` table canister from
source, installs it on PocketIC with the real ICP ledger, seats two players with real money, and
then does a genuine `install_code --mode upgrade` to the module under test.

**On the tree as shipped** the upgrade is refused, with the exact error:

```
M7: on 5e12d25bf4d2 -> escrow 600000000, seated chips 400000000, pot 0
Panicked at 'CRITICAL: Failed to restore state from stable memory:
  "Custom(Fail to decode argument 0 ... Subtyping error: field deposit_watermark is not
   optional field)". Upgrade REJECTED to protect user funds.'
M7: the cross-version upgrade was REFUSED, which is the SAFE outcome, and nothing was lost.
```

**With ONLY `deposit_watermark` changed to `Option<u64>`** -- the exact remedy the deposit
reviewer recommends, applied on its own, nothing else touched -- the same test reports:

```
M7: on 5e12d25bf4d2 -> escrow 600000000, seated chips 400000000, pot 0
assertion `left == right` failed: an accepted cross-version upgrade DESTROYED SEATED CHIPS:
  400000000 -> 0
```

**4 ICP of seated chips destroyed, the upgrade reported as successful, and no error anywhere.**
Escrow survived (600000000, a separate top-level field), which is what makes the loss look
partial and plausible rather than obviously catastrophic. That is the finding, executed.

* **As shipped**, the top-level `nat64` makes the entire `stable_restore` fail, `post_upgrade`
  panics, and the upgrade is **REJECTED**. That is loud and it is safe: the old code stays and
  nothing is lost. The reviewer of the deposit work reproduced this against the real canister on
  PocketIC (`Subtyping error: field deposit_watermark is not optional field`) and correctly
  called the fix undeployable.

* **The obvious remedy for that -- change only `deposit_watermark` to `Option<u64>` -- turns a
  rejected upgrade into SILENT DESTRUCTION OF EVERY CHIP AT THE TABLE.** `table_state` is
  `Option<TableState>`, and Candid's rule for `opt t` is that a value which cannot be read as
  `t` decodes as **null**, not as an error. With `departed_stakes` still a bare `vec`, the whole
  `TableState` becomes unreadable, so `table_state` silently arrives as `None` -- and
  `post_upgrade` then takes its `else if let Some(config)` branch and calls
  `init_table_state(config)`. Every seated player's `chips`, the live `pot`, the `side_pots`,
  the hole cards and the shuffle commitment are gone, replaced by a fresh empty table. Escrow
  `balances` survive, because they are a separate top-level field, so **the loss is exactly the
  chips players had bought in with** and nothing in the log says so.

This is the FINDING 01 failure mode (chips that exist and can never be claimed) reached through
the deploy path instead of through a hand, and it is reached by applying the fix the previous
reviewer recommended. Whoever lands that one-line change must land the other half in the same
commit.

**Not currently exploitable and not currently a live loss**, for two reasons that are both
circumstantial: nobody has played on mainnet, so there are no chips at any table to destroy; and
the shipped tree fails loudly rather than silently. It is filed high because the safe state is
an accident of the *other* agent's mistake, and the first person to fix that mistake removes the
accident.

### Why no test could see it

Every "survives an upgrade" assertion in `tests/money_safety` upgrades the new wasm **to
itself**: `World::upgrade` reuses `self.table_wasm`. Same type on both sides of the wire, so the
addition is never tested as an addition. There is no `upgrade_from(previous_release_wasm)`.
Nothing in the repo installs a pre-wave-2 module and upgrades it.

### The fix, in the order it must be applied

1. `deposit_watermark: Option<u64>` **and** `departed_stakes: Option<Vec<DepartedStake>>`, in
   one change. Either alone is worse than neither.
2. `World::upgrade_from(old_wasm)` plus one test that installs the `801aa79` table canister,
   buys chips in, upgrades to the current wasm and asserts seated chips, pot and escrow all
   survive. Without that test the next persisted field repeats this exactly.
3. A general guard, because (2) only protects fields that exist today: persist a redundant
   flat-scalar digest of the table (`opt bool` present, `opt nat64` pot, `opt nat64` seated
   chips) alongside `table_state`, and make `post_upgrade` panic when the digest says a table
   was saved and `table_state` came back `None`. Flat `opt` scalars cannot themselves be
   silently dropped, so that converts any future nested-field mistake from silent chip
   destruction into a rejected upgrade. **This guard is implemented as of this pass** (see
   `PersistentState::table_was_present` / `table_pot_at_save` / `table_seated_chips_at_save`).

   **Read its limit carefully.** The guard can only fire when the SAVED state contains the
   digest, and state written by `801aa79` does not. So it does nothing for the specific
   old-to-current upgrade above -- which is why M7, not the guard, is what catches that -- and
   everything for every upgrade from this commit onwards. It buys the next agent a rejected
   upgrade instead of destroyed chips; it does not retro-fit safety onto state already written.

**All three steps are done as of wave 3.** Step 1 landed as one change; step 2 is M7, rewritten
so that a REJECTED upgrade is now a FAILURE rather than an acceptable outcome (see the status
box at the top of this finding); step 3 is the digest guard, kept, with its limit stated.

Two more things landed with them:

* **`m7b_a_departed_stake_and_its_owner_survive_an_upgrade`.** M7 cannot cover
  `departed_stakes` at all, because `801aa79` has no such field to write. So a second test
  creates a departed stake on the module under test, upgrades to the same module, and requires
  the stake -- and above all its OWNER -- to come back identical, then settles the hand and
  requires the money to reach that owner. This is the coupling between the two findings: the
  FINDING 13 fix works by carrying an owner in persisted state, so that state has to survive an
  upgrade or settlement has nobody to pay.
* **The harness mirrors track the wire.** `tests/money_safety/src/table_api.rs` and
  `tests/settlement/src/table_api.rs` both declare `departed_stakes: opt vec`. Candid lets a
  non-`opt` wire value be read into an `opt` field, so a mirror declared `opt` can decode BOTH
  the old and the new canister -- which is what makes it possible to observe a previous release
  at all. A mirror declared `vec` cannot decode the new one, and that was a second face of H-16.

### The pre_upgrade decision: it TRAPS now, and here is the argument

`pre_upgrade` used to end like this:

```rust
if let Err(e) = ic_cdk::storage::stable_save((state,)) {
    ic_cdk::println!("CRITICAL: Failed to save state to stable memory: {:?}", e);
    // Log but don't panic - allow upgrade to proceed
    // This is safer than trapping which could brick the canister
}
```

**The comment is backwards on a canister that custodies funds, and the change was made
deliberately rather than as a tidy-up.**

* A trap in `pre_upgrade` aborts the **upgrade**. The old code keeps running with its heap
  intact. Nothing is bricked; an *install* is refused, which is a state a human can act on.
* Proceeding after a failed save has exactly two outcomes and both are worse.
  * If stable memory is empty, `post_upgrade`'s `stable_restore` fails and it panics anyway --
    the same refusal, minus the accurate reason, and with the operator told the wrong thing
    about where the failure was.
  * If stable memory still holds an **older snapshot** from a previous upgrade, `stable_restore`
    SUCCEEDS and the canister silently rolls back to it. Every escrow balance, every chip and
    every hand since that snapshot is gone -- and `verified_deposits` and `deposit_watermark`
    roll back with them, which **re-opens the E-02 replay window on ledger blocks that were
    already credited**. A silent rollback of the anti-replay record is a fund-theft primitive,
    reached by a deploy.

So the trap message states the escrow total it was about to save and says the old code is still
running. It cannot be driven from outside on PocketIC -- there is no way to make `stable_save`
fail on demand -- so what is pinned instead is that the decision has not been quietly reverted:
`pre_upgrade_refuses_rather_than_proceeding_after_a_failed_save` reads `lib.rs`, requires
`ic_cdk::trap(` inside `pre_upgrade`, and fails if the string `allow upgrade to proceed` comes
back.

### Related, same root cause, lower stakes

`docs/DEFECTS.md` E-10 (`PENDING_WITHDRAWALS` / `LAST_WITHDRAWAL` are not persisted at all) is
the same blind spot seen from the other side: nobody has ever exercised an upgrade across a
version boundary, so nothing about persistence is known to work.
