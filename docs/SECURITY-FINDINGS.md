# ClearDeck security findings

Findings that touch real user funds. Each entry states what was actually executed, so a
reader can tell a demonstrated defect from a suspected one.

> ## 🚨 STATUS, 2026-08-05, AFTER WAVE 6: A SECOND INDEPENDENT AUDITOR STILL SAYS NO
>
> A second blind auditor was run **after** this wave's fixes, on the running system, with the
> same instructions as the first: treat every document in this project as a marketing claim
> until verified. Its verdict, verbatim and unsoftened:
>
> > **"NO — I would not tell a friend their money is safe here."**
>
> The first auditor's verdict, for comparison, was *"No. I would not tell a friend their money
> is safe here, and the reason is not the poker."* **The reason is still not the poker.** Wave 6
> closed what the first auditor found — the fund lock is gone, the free-showdown cheat is gone,
> the fairness endpoint no longer accuses honest players — and the second auditor confirmed the
> money arithmetic held through every hostile sequence it could construct, drained two tables to
> exactly zero, and reproduced the shuffle from the spec alone.
>
> **These are the open fund-safety findings, in the order a stranger's money is at risk:**
>
> | | finding | what it costs | status |
> |---|---|---|---|
> | 1 | [FINDING 07](#finding-07) **CRITICAL** | one controller call destroys 100% of a table's seated chips, irreversibly, and **every invariant is silent** | **FIXED 2026-08-05.** Reproduced live on the replica first (**40.00000000 ICP destroyed by one `reset_table`**, canister still holding it, both players reading `get_balance = 0`, no restore path), then fixed and re-verified live: `reset_table` REFUSES while the table holds custody, `admin_reinit_table` returns every chip to its owner's escrow, `init_table_state` traps rather than rebuild over money. Total claims now unchanged to the e8 across both calls. Gated by **`tests/money_safety/tests/admin_custody.rs`**, including a sweep over EVERY controller-callable method and a census that fails when a new one appears unaudited |
> | 1b | [FINDING 20](#finding-20) high | found while auditing the surface for FINDING 07's shape: a controller could change `TableConfig::currency` on a funded table. Currency selects the LEDGER withdrawals are paid from, so every balance becomes unpayable without a single write to `BALANCES` | **FIXED 2026-08-05** in the same change |
> | 2 | [FINDING 18](#finding-18) high | `cash_out` of a stuck hand walks away from your stake and tells you nothing. Composes with FINDING 07 into permanent loss with no malice | **FIXED 2026-08-05.** The exit doors now SETTLE an unmovable hand before they vacate the seat, so `Ok = 0` became `Ok = 298_000_000`; a live hand cannot outlive its last player; `get_custody_status`, the table view and `withdraw`'s refusals all name the stake and `abandon_stuck_hand`. Gated by **M10 CUSTODY VISIBILITY** |
> | 3 | [FINDING 19](#finding-19) high | no on-chain timer: nothing moves the game. The dead window was not 5.5 minutes, it was **unbounded** — a live pot sat untouched for 20 simulated minutes with no client attached | **CLOCK FIXED 2026-08-05** (30 s to self-resolve, 210 s to release every seat, no caller). **The CYCLES half is OPEN and now WORSE**: see below |
> | 3b | [FINDING 19 §cycles](#finding-19) **high** | no cycles monitoring and **no top-up anywhere in the tree**. A canister below its freezing threshold rejects `deposit`, `withdraw`, `cash_out` and `abandon_stuck_hand` at once — total custody failure, no attacker. The new clock raises idle burn **~630x** (0.00007 → 0.0442 T/day), cutting the runway at 10 T cycles from centuries to **226 days** | **OPEN, and made worse on purpose by the fix above.** `get_cycle_status` now makes it visible; nothing tops it up |
> | 4 | [FINDING 17](#finding-17) high | a hand can settle with NO live claim on it and refund every stake to the players who FOLDED. The fold-out winner loses the pot to an ordinary disconnection | **FIXED 2026-08-05.** Reproduced first (52,000,000 e8 pot, three seats, all three back on exactly their buy-in), then closed by making participation and eligibility ONE function: `live_claims` now CALLS `is_in_hand`, and `is_in_hand` no longer accepts a seat that holds no cards. Re-measured: the fold-out winner is paid 202,000,000 against a 200,000,000 buy-in and the folder pays for it. Gated by **M11 OUTCOME** on every fuzz step, `probe4`, and the predicate table in `src/table_canister/tests/hand_membership.rs` |
>
> None of these is fund THEFT. Numbers 1, 2 and 4 were fund LOSS or fund MISDIRECTION and are
> all closed.
> **Number 1 was the largest unfixed fund-loss path in the project and had survived four waves.**
> See docs/WAVE-06.md for the full accounting and the shortest path to a different answer.
>
> **What FINDING 07's fix does NOT do is undo the damage already done.** The local `table_3` on
> this machine holds **120.00000000 ICP that belongs to nobody** — 80 destroyed by the auditor's
> run and 40 by the reproduction that opened this wave. No code change returns it: there is no
> restore path, and adding one would be a mint. That number was invisible to every instrument in
> the project until this wave; it is now what M9's new `money_belongs_to_nobody` check reports.

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
> ~~**FINDING 07 still reaches the same terminal state by another route** (`admin_reinit_table`
> strands every seated player's chips)~~ **FIXED 2026-08-05, see the status block above and
> [FINDING 07](#finding-07).** **FINDING 12 still ends a hand early** on a 30-second lull on
> `table_1` / `btc_table_1` — it just no longer destroys the pot when it does.
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
| FINDING 07 | [E-07](DEFECTS.md#e-07) | critical — **FIXED 2026-08-05** |
| FINDING 20 | [E-45](DEFECTS.md#e-45) | high — **FIXED 2026-08-05** |
| FINDING 10 | [E-02](DEFECTS.md#e-02) | **fund-theft (demonstrated, FIXED)** |
| FINDING 11 | [E-12](DEFECTS.md#e-12) | low |
| FINDING 12 | [E-06](DEFECTS.md#e-06) | high |
| [FINDING 16](#finding-16) | [E-32](DEFECTS.md#e-32) + [E-06](DEFECTS.md#e-06) | **player-to-player fund theft (demonstrated, FIXED)** |

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

<a id="finding-07"></a>

## FINDING 07 -- one controller call destroyed 100% of a funded table's chips -- **FIXED 2026-08-05, REPRODUCED LIVE FIRST**

> ## ✅ CLOSED IN WAVE 7. It was the largest unfixed fund-loss path in the project and it had survived four waves.
>
> **What it was.** `reset_table` and `admin_reinit_table` had byte-identical bodies:
>
> ```rust
> fn reset_table(config: TableConfig)        -> { require_controller()?; validate_config(&config)?; init_table_state(config); }
> fn admin_reinit_table(config: TableConfig) -> { require_controller()?; validate_config(&config)?; init_table_state(config); }
> ```
>
> `init_table_state` builds a brand-new `TableState` with empty seats, so **every seated
> player's chips ceased to exist**: not returned to escrow, not withdrawable, not recoverable.
> `admin_restore_balance` was deliberately removed
> (`// REMOVED: admin_restore_balance - unnecessary attack surface`), so a well-meaning
> controller could not undo it either. The Candid described `admin_reinit_table` as *"for
> recovery after upgrade issues"* — the interface pointed an honest operator at the button that
> deletes everybody's money. A second independent auditor found it from the outside, with no
> access to these documents, and ranked it their number-one blocker; they left `table_3` holding
> **80.00000000 ICP against total claims of 0**, both players reading `get_balance = 0`.
>
> ### REPRODUCED FIRST, on the running local replica, module `5e07edf6…`
>
> `table_3`, min buy-in 20 ICP, `cd-alice` and `cd-bob` deposited and seated. Every figure below
> is `icrc1_balance_of` against the real local ICP ledger and the canister's own
> `admin_get_all_balances` / `admin_get_table_chips` / `get_pot`:
>
> ```
> T0 start                    ledger_main=8000000000   claims=0             ORPHANED=8000000000
>                             (the auditor's 80 ICP, already destroyed, already unrecoverable)
> T1 after two 20 ICP deposits ledger_main=12000000000 claims=4000000000    ORPHANED=8000000000
> T2 seated                   ledger_main=12000000000  chips=4000000000     ORPHANED=8000000000
> reset_table                 -> Ok
> T3 after reset_table        ledger_main=12000000000  chips=0  claims=0    ORPHANED=12000000000
> admin_reinit_table          -> Ok      (nothing left to destroy)
> alice cash_out              -> Err("Not at table")
> alice withdraw(20 ICP)      -> Err("Insufficient balance. Have: 0.0000 ICP, requested: 20.0000 ICP")
> -----------------------------------------------------------------------------------------
> DESTROYED BY ONE CALL, unrecoverable by anybody:   4000000000 e8s   (40.00000000 ICP)
> ```
>
> One correction to the wave-6 write-up, measured rather than assumed: **escrow is NOT zeroed.**
> `init_table_state` never wrote `BALANCES`. In the auditor's run escrow read zero because the
> players had already converted it into chips by sitting down, and it is the CHIPS and the POT
> that were destroyed. Verified directly: a player with 0.6 ICP in escrow and no seat survives
> both calls with the balance intact and can still withdraw it. This matters, because it is why
> the fix keys on table custody rather than on escrow.
>
> ### AND EVERY INVARIANT IN THE PROJECT WAS SILENT
>
> M9's drain reported `fully_drained`, because `internal_total` counts what the canister SAYS it
> owes and after the reset it said it owed nothing. M2 was silent because holding MORE than you
> owe is not insolvency. **An attack that changes what is owed cannot be caught by an instrument
> anchored to what is owed.** That is the deepest part of this finding and it is what the new
> invariant below is for.
>
> ### THE FIX
>
> The two doors are no longer the same function, and the destructive helper guards itself.
>
> 1. **`reset_table` is a CONFIG operation and refuses while the table holds custody.** There is
>    no legitimate reason a config reset touches custody. The refusal names the recovery call.
>    It does NOT require escrow to be empty: `BALANCES` is a separate map this path provably
>    never writes, and that claim is a test
>    (`admin_custody::reset_table_never_touches_escrow`), not an assertion.
> 2. **`admin_reinit_table` is a RECOVERY operation and conserves.** It returns every seated
>    stack and every stake in the live hand to the escrow of the player who OWNS it — through
>    `hand_stakes`, the same payout basis settlement uses, so a stake left behind by a player who
>    already walked away reaches THEM and not whoever took their chair ([FINDING 13](#finding-13))
>    — and only then rebuilds the table.
> 3. **`admin_return_all_chips_to_escrow` is a new, standalone, conserving primitive.** It is
>    what the refusal in (1) points at, and it reports what it moved. It is not the
>    `admin_restore_balance` mistake: that could MINT escrow from nothing, this can only MOVE
>    money the table already holds, and it refuses — changing nothing — if `state.pot` and the
>    attributed stakes disagree, because paying out only the attributed part would destroy the
>    difference. (The escape from that state is `abandon_stuck_hand`, which is public.)
> 4. **`init_table_state` TRAPS if the table it is about to replace holds custody.** The guard is
>    at the helper as well as at both doors, because the reason this survived four waves is that
>    the destructive step was a shared helper and writing a second door was one copy-paste.
>
> ### VERIFIED LIVE, same replica, same script, module rebuilt from this tree
>
> ```
> T2 seated                   chips=4000000000   TOTAL_CLAIMS=54060000000
> reset_table            -> Err("Refusing: reset_table rebuilds the table, and this table is
>                               holding 4000000000 in seated chips and 0 in the pot for real
>                               players. ... Call admin_return_all_chips_to_escrow first ...")
> admin_reinit_table     -> Ok
> T4 after both doors         chips=0            TOTAL_CLAIMS=54060000000   <-- UNCHANGED, to the e8
> get_balance    alice 0 -> 2000000000 ;  bob 2000000000 -> 4000000000
> alice withdraw(20 ICP) -> Ok ; ledger_main and claims each fall by exactly 2000000000
> ```
>
> **What the fix cannot do:** un-destroy what was already destroyed. `table_3` on this machine
> still holds **120.00000000 ICP that belongs to nobody** — 80 from the auditor, 40 from the
> reproduction above — and no code change can return it, because there is no restore path and
> adding one would be a mint. That number is now visible: it is what the new invariant reports.
>
> **It composed with [FINDING 18](#finding-18)** and no longer can: a stake left in a stuck hand
> by a player who cashed out is recoverable by `abandon_stuck_hand`, and `reset_table` can no
> longer take it away in the meantime.

**Severity:** **CRITICAL**, controller-only. Was permanent, unrecoverable loss.
**Status:** **FIXED 2026-08-05.** Reproduced live on the replica first, fixed, re-verified live.
The reproducer that used to PIN the destruction is now the gate that convicts it returning:
`reg06_admin_reinit_table_returns_every_seated_chip_to_its_owner` (was
`reg06_admin_reinit_table_strands_every_seated_players_chips`) and
`wave6_coherence::probe5` (was `#[ignore]`d because it recorded the defect; the `#[ignore]` is
gone with the pin).
**Where:** `reset_table`, `admin_reinit_table`, `init_table_state`,
`src/table_canister/src/lib.rs`. See [DEFECTS.md E-07](DEFECTS.md#e-07).

### The instrument that was missing, and now exists

`M9 FUND REACHABILITY` asked *"can each player reach the balance the canister admits to"*. The
attack works by changing what the canister admits to. So M9 has a third leg, anchored to the
ledger rather than to the canister's books:

> **The canister's ledger balance may never exceed total claims by more than the deposits it has
> not been asked to credit yet. Money inside the canister that belongs to NOBODY is a defect,
> not a surplus.**

* `reachability::check_no_orphaned_custody`, on every snapshot, wired into `check_point_in_time`
  so the fuzzer and every invariants test evaluate it.
* `DrainReport::orphaned_e8s()` / `table_is_really_empty()`, so the end-of-run drain — the thing
  the auditor and the fuzzer both read — stops taking `owed_after == 0` for an answer.
* A new severity, `OrphanedCustody`, added to the **never-excusable** list. It is deliberately
  not a `FundDestruction`, which the documented register is allowed to excuse.

Driven against a module with the guards removed, it reports exactly the finding:

```
[M9_FUND_REACHABILITY] money_belongs_to_nobody OrphanedCustody delta=400000000
  the ledger says this canister holds 4000000000 e8s and the canister says it owes 3600000000
  (escrow 3600000000 + chips 0 + pot 0) ... That leaves 400000000 e8s inside a canister that
  owes them to NOBODY
  BLOCKS BECAUSE: this severity is never excusable
```

### THE WHOLE CONTROLLER SURFACE, AUDITED

The question asked of every controller-callable method: *can this admin action reduce what the
canister owes players without paying them?*

| method | kind | verdict |
|---|---|---|
| `reset_table` | update | **WAS THE DEFECT.** Now refuses while the table holds chips or a pot |
| `admin_reinit_table` | update | **WAS THE DEFECT.** Now returns every chip to its owner's escrow first |
| `admin_return_all_chips_to_escrow` | update | NEW. Conserving by construction; escrow only goes up; ledger untouched |
| `admin_update_config` | update | Never replaced `TableState`, but could re-denominate a funded table: **[FINDING 20](#finding-20)**, fixed |
| `add_controller` / `remove_controller` | update | Privilege only, no custody. Note `is_controller()` also honours the real IC controller list, so `remove_controller` cannot lock the operator out |
| `set_history_canister` | update | Writes `HISTORY_ID`. No custody. Can misdirect the fairness archive, which is [FINDING 19](#finding-19)'s territory, not this one |
| `set_dev_mode` | update | Permanently returns `Err` for every caller. No custody, and no gate needed |
| `flush_unrecorded_hands` | update | Not controller-gated on purpose (a player must be able to make their own evidence durable). No custody |
| `admin_get_balance`, `admin_get_all_balances`, `admin_get_table_chips` | query | Read-only |

`post_upgrade` is not on this list because it is not callable, but it is the other path that can
replace `TableState`, and it already refuses rather than rebuild over money — that guard is
[FINDING 14](#finding-14).

**This table is a test, not a promise.**
`admin_custody::census_every_controller_gated_method_is_classified_here` parses
`src/table_canister/src/lib.rs` for every function that calls `require_controller()` and fails if
one appears that has not been audited here, or if a method recorded as deliberately open acquires
a gate. `admin_custody::sweep_no_admin_call_reduces_what_is_owed` then drives every one of the
updates against a funded, seated table and asserts, after each call, that
`owed_before - owed_after <= ledger_before - ledger_after`: liability may only fall by money that
actually left the canister.

### One bug in the first version of the fix, found by deploying it rather than by testing it

`Player::total_bet_this_hand` is cleared only by `start_new_hand`. Between hands, every seat
still reports the finished hand's bets while `state.pot` is back to zero. The first
`table_custody` read the stakes unconditionally, double-counted them, and the conservation guard
then refused every ordinary post-hand table:

```
admin_reinit_table -> Err("Refusing to touch this table: at phase complete state.pot says 0
                          but the stakes that can be attributed to an owner sum to 2000000000")
```

**Every PocketIC test written for the fix was green**, because not one of them had played a hand
before calling the admin path. It was caught by deploying the module to the local replica and
calling the method on a real table. `admin_custody::the_admin_doors_work_on_a_table_that_has_
already_played_a_hand` is the gate that now convicts it, and it was verified to convict it by
re-introducing the mutation in a copy of the tree.

<a id="finding-20"></a>

## FINDING 20 (high) -- a controller could re-denominate a funded table, making every balance unpayable -- **FIXED 2026-08-05**

**Severity:** HIGH, controller-only, reversible. Fund LOCK, not fund loss.
**Status:** Found while auditing the controller surface for FINDING 07's shape. Fixed in the
same change. Reproducer / gate:
`admin_custody::currency_cannot_be_changed_while_the_canister_owes_anybody_anything`.
**Where:** `reset_table`, `admin_reinit_table`, `admin_update_config` — all three take a whole
`TableConfig`. See [DEFECTS.md E-45](DEFECTS.md#e-45).

`TableConfig::currency` is not a display setting. `Currency::ledger_canister()` is the ledger
`transfer_tokens` pays out of, and `min_withdrawal`, `transfer_fee` and `format_amount` all
follow it. A controller could flip an ICP table to BTC while escrow balances existed, at which
point every `withdraw` would be attempted against the ckBTC ledger, where this canister holds
nothing. No `BALANCES` entry changes, no invariant in the project measures the currency, and
every player's money is unreachable until somebody flips it back. The reverse flip on a canister
holding both assets is worse: sat-denominated balances become withdrawable as ICP e8s.

It is the same family as FINDING 07 — an admin action that makes what the canister owes
unpayable, without paying it — reached by a different mechanism, which is exactly why the audit
had to be of the whole surface rather than of the one function the auditor named.

A second, quieter half was fixed with it: `admin_update_config` wrote `state.config` but not
`TABLE_CONFIG`, and `get_table_currency()` — which `withdraw` and `transfer_tokens` read — reads
`TABLE_CONFIG`. A config change therefore left the table charging blinds by one record and
paying withdrawals by another. Both are now written together.

**The fix:** `refuse_currency_change_while_funded` on all three doors. A currency change is
allowed only when escrow + chips + pot is exactly zero.

**THE FIX IS INCOMPLETE.** "Escrow + chips + pot" is not everything the canister holds. See
FINDING 21, which walks the surviving hole and demonstrates it.

<a id="finding-21"></a>
## FINDING 21 (high) -- the FINDING 20 guard measures the wrong total, so a funded table can still be re-denominated -- **OPEN**

**Severity:** HIGH. Controller-only, fund LOCK. Recoverable only while the table stays empty.
**Status:** OPEN. Found by the wave-7 critic while attacking the FINDING 20 fix. Demonstrated
on PocketIC against the REAL ICP ledger wasm on the fixed module
(`sha256=50a3c2e1d925e84622b90bc224f13a2255f59bba67a020e8f605eb6cf8cb8b46`).
**Where:** `total_liability()` and `refuse_currency_change_while_funded()` in
`src/table_canister/src/lib.rs`; `check_no_orphaned_custody` in
`tests/money_safety/src/invariants/reachability.rs`.

The canister holds money in **two** kinds of ledger account, not one:

  * its main account, `icrc1_balance_of(table, None)`, and
  * one deposit subaccount per player, `sha256("cleardeck-deposit:" || principal)`, which is
    the address `get_deposit_address()` tells external-wallet users to send to.

`total_liability()` is `escrow + chips + pot`. Money that has ARRIVED in a deposit subaccount
and has not yet been swept in by `claim_external_deposit()` is in neither term. The FINDING 20
guard therefore reads a liability of **zero** on a table that is holding a player's deposit,
and permits the currency change it exists to refuse.

Measured, on the fixed module:

```text
  victim sends 5 ICP to their own deposit subaccount (the documented flow)
    ledger_main=0   deposit_subaccounts=500000000   claims=0
    invariant violations: 0
  admin_update_config(currency = BTC)      -> Ok            <-- the guard sees liability 0
  victim claim_external_deposit()          -> Err("Failed to query balance: ... mxzaz-hqaaa-aaaar-qaada-cai")
  victim get_balance()                     -> 0
    ledger_main=0   deposit_subaccounts=500000000   claims=0
    invariant violations: 0
```

`claim_external_deposit` reads `get_table_currency()` to pick the ledger it queries and sweeps
from, so after the flip it looks for the player's 5 ICP on the **ckBTC** ledger. The ICP is
still sitting in an account only this canister can move, and the canister no longer has any code
path that looks at it. It is recoverable by flipping the currency back -- but only while the
table's escrow, chips and pot are all still zero; once one ckBTC player funds the re-denominated
table, `refuse_currency_change_while_funded` blocks the flip back and the ICP is permanent.

**The new orphan invariant cannot see it either, and that is the second half of the finding.**
`check_no_orphaned_custody` computes `ledger_main - claims - uncredited_raw`. Deposit
subaccounts appear in neither side of that subtraction, so a canister holding 7 ICP in its own
subaccounts and owing nobody anything reports **zero** orphaned e8s:

```text
  canister holds 700000000 e8s in deposit subaccounts, 0 in main, and owes 0
  money_belongs_to_nobody hits: 0
```

The check is a re-severity of M1's positive direction and inherits M1's anchor exactly. It is a
real check -- it fires on an uncredited transfer into the main account, verified -- but "the
canister may never hold money it owes to nobody" is not what it measures. It measures "the
canister's MAIN ACCOUNT may never hold money it owes to nobody."

**What the fix has to be:** the liability guard and the orphan check must both be anchored to
every account the canister owns, main plus every deposit subaccount, not to `ledger_main` alone.
Until then FINDING 20 is half-closed and the wave-7 orphan invariant has a blind spot of exactly
the shape FINDING 07 had.

<a id="finding-22"></a>
## FINDING 22 (medium/high) -- the FINDING 07 fix gives a controller a new power: void any live hand after reading every hole card -- **OPEN**

**Severity:** MEDIUM-HIGH. Controller-only. Conserves money to the e8, so **no conservation
invariant in this project can see it.** It is not a loss of principal; it is a loss of the
equity a player has already bought.
**Status:** OPEN, introduced by the FINDING 07 fix.
**Where:** `admin_return_all_chips_to_escrow` and `admin_reinit_table` in
`src/table_canister/src/lib.rs`.

Before the fix, a controller who wanted to end a live hand could only do it by destroying
everybody's money (FINDING 07) -- an action with no beneficiary. After the fix, both doors work
mid-hand and hand every wager back to the player who made it:

```text
  live hand, phase = Turn, pot = 1004000000 e8s
    seat 0  A(d) 8(d)   committed 502000000
    seat 1  Q(h) 9(d)   FOLDED
    seat 2  6(c) 4(h)   committed 502000000
  get_table_state (controller-only) returns every hole card above.
  admin_return_all_chips_to_escrow -> Ok(3000000000)
  after: phase = Turn, pot = 0, every stack 0, escrow +10 ICP each
  invariant violations: 0
```

A controller can read the cards and then decide whether the hand happens. A confederate who is
drawing dead is made whole; the player who was going to win the pot loses the equity they paid
for. `admin_reinit_table` does the same thing on a live pre-flop hand and logs a line that says
"No chip was destroyed", which is true and beside the point.

`table_custody` does not filter on `Stake::relinquished`, so a stake **a player has already
folded** is returned to them as well.

This is the wave-6 lesson repeating: a trap was fixed and a silent refund-everyone path was
created in its place, it conserves perfectly, and nothing noticed. Any guard for it has to be a
rule about WHEN the recovery door may be used (a live hand is not a recovery situation until it
is provably stuck -- `hand_is_stuck` already exists and `abandon_stuck_hand` already uses it),
not a rule about arithmetic.

**Related, same function:** `admin_return_all_chips_to_escrow`'s documentation says it
"abandon[s] the hand in progress". It does not. It zeroes stacks, pot, side pots and departed
stakes and leaves `phase`, `action_on` and every hole card exactly as they were. The table is
left mid-hand with cards on the board and every stack at zero; `reload` refuses ("Cannot reload
during a hand") until the zombie hand finishes.

<a id="finding-23"></a>
## FINDING 23 (critical, unguarded by design) -- `uninstall_code` and `install_code --mode reinstall` are FINDING 07 at full scale, and the wave-7 audit does not cover them -- **OPEN**

**Severity:** CRITICAL. Controller-only, irreversible, 100% of the canister's funds.
**Status:** OPEN. The reinstall half is acknowledged in the wave-7 handover; the `uninstall_code`
half is not, and neither appears in
`admin_custody::census_every_controller_gated_method_is_classified_here`, whose subject is
methods that call `require_controller()` inside the canister.

Measured on the fixed module:

```text
  seated, funded:              ledger=4000000000  escrow=3600000000  chips=400000000  claims=4000000000
  install_code --mode reinstall -> Ok
                               ledger=4000000000  escrow=0  chips=0  claims=0
    [M1_CONSERVATION]      ledger_equals_owed FundDestruction delta=4000000000
    [M9_FUND_REACHABILITY] money_belongs_to_nobody OrphanedCustody delta=4000000000

  uninstall_code                -> Ok
    canister has no code, no balances, and still holds 4000000000 e8s on the ledger.
```

`init_table_state`'s new trap cannot see either: on a reinstall the heap is fresh, so `TABLE` is
`None` and there is no custody to refuse; on an uninstall no canister code runs at all.
`post_upgrade`'s FINDING 14 digest guard only covers `--mode upgrade`. These are the two calls
that actually destroy a mainnet table, and the in-canister audit surface cannot reach them --
which means the control has to be operational (a deploy script that refuses a non-`upgrade`
mode against a canister whose books are non-zero) and has to be tested as such.

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

<a id="finding-11"></a>
## FINDING 11 (NEW, low) -- dust at or below the transfer fee in a deposit subaccount can never be swept

> **SUPERSEDED IN SCOPE BY [FINDING 28](#finding-28) (2026-08-06).** This finding records the dust
> half only. The third independent auditor showed that money in a deposit subaccount is invisible to
> **every** balance surface at any size, not only below the fee, and that the error text tells the
> player to send more money to the address already holding theirs. Read FINDING 28 first.

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

<a id="finding-12"></a>

## FINDING 12 -- one action timeout ends the hand for everybody, because the disconnect timeout and the action timeout are both 30 seconds -- **CLOSED IN WAVE 5, GATE INVERTED IN WAVE 6**

> **STATUS 2026-08-05: CLOSED, AND THE REPRODUCER NOW GATES THE FIX.**
>
> Two independent things had to change and both did:
>
> 1. **participation no longer reads `status`.** `count_players_can_act` and the other three
>    betting-round sites ask `is_in_hand` / `can_still_act`, the same predicate `live_claims`
>    asks, so marking a seat `Disconnected` removes it from nothing (docs/DEFECTS.md E-32);
> 2. **`DISCONNECT_TIMEOUT_SECS` is 90**, longer than every deployed action clock (30 / 45 /
>    60 s), so the two thresholds cannot race on any table. The ACTION clock is always first.
>
> Re-measured on module `5e07edf6…`, the identical sequence this finding describes:
>
> ```
> before      phase Flop, board 3, pot 206000000, 2 players live
> advance 31s; check_timeouts() -> PlayerTimedOut(1)
> after       phase Turn, board 4, pot 206000000, 2 players live, 0 e8s destroyed
>             seat0 Active   seat1 Active folded   seat2 Active     <- nobody Disconnected
> nobody ever answers again; each clock expires in turn
> FINAL       phase HandComplete, board 4, fold-out, seat 0 paid the whole 206000000
>             0 e8s destroyed across the whole sequence
> ```
>
> The board is **4 cards, not 5**: the hand ends by fold-out on the street it actually reached,
> rather than by a five-card showdown nobody bet into. `reg09` was inverted from pinning the
> defect to gating the fix and renamed `reg09_one_timeout_folds_only_the_seat_on_the_clock`; it
> goes RED again if either half of the fix is reverted. The original finding follows unchanged.

**Severity:** HIGH. It removes the remaining betting rounds from every player at the table
without anyone choosing to check down, and it settles the hand out of the stale pre-flop
breakdown, which is FINDING 01. So a 30-second lull both takes away the turn and the river and
destroys whatever was already wagered post-flop.
**Status:** CONFIRMED by execution against the real canister; **CLOSED in wave 5, gate inverted
in wave 6.** Reproducer, now a gate: `reg09_one_timeout_folds_only_the_seat_on_the_clock` in
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

<a id="finding-13"></a>
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

<a id="finding-14"></a>
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

---

<a id="finding-15"></a>
## FINDING 15 (critical), one unguarded evaluator call locks a funded table: every path to a player's own money closes at once

> ### STATUS 2026-08-05: REPRODUCED ON THE RUNNING REPLICA, ROOT CAUSE FOUND, FIXED, AND NOW GATED BY A NEW INVARIANT
>
> **The wave-5 write-up below named the wrong call site.** It read the current source and picked
> the one unguarded `evaluate_hand`, in `record_hand_to_history`. That was a reasonable guess and
> it was wrong, which matters, because the guess would have produced a one-line change that fixed
> nothing. The canister's own log says where the trap actually was:
>
> ```text
> $ icp canister logs table_2
> Panicked at 'IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or 5-card board (flop/turn/river),
>   got 0 community cards (2 cards in total)', src/poker_core/src/hand.rs:187:65
> Canister Backtrace:
>   poker_core::hand::evaluate_hand
>   table_canister::plan_payouts
>   table_canister::settle_hand
>   table_canister::end_hand_single_winner      <-- the FOLD-OUT path, not the showdown path
>   canister_update leave_table
> ```
>
> and a second one, identical, under `canister_update player_action`. Three of the four local
> tables have this in their logs. **Read the log before believing the read.**
>
> **Reproduced end to end**, on the running local replica, against the byte-exact deployed module
> `0x5298915c…`, on an isolated canister so no other agent's fixture was touched. Full transcript
> under "Reproduced, live" below. Every door shut at once, exactly as reported.
>
> **The real cause is not the evaluator.** It is that the engine ended a hand as
> "everybody else folded" while **two seats still held live claims and there was no board to rank
> them by**. `count_active_players`, which decides that the hand is over, counted only seats whose
> `status == Active`; `live_claims`, which decides who may win the pot, counts every seat that has
> not folded. A player whose client stops sending heartbeats leaves the first set and stays in the
> second. So the fold-out settlement ran with an unrankable board, and the evaluator did the only
> thing it had been told to do about an impossible input: it trapped.
>
> **What was fixed, and by whom.** The cause -- participation and eligibility asked of one
> predicate, so a missed heartbeat can never take a seat out of the hand -- was fixed in the same
> wave by the agent working E-32/E-06; see the "WHO IS IN THE HAND" block in
> `src/table_canister/src/lib.rs`. This entry owns the rest:
>
> 1. **no trapping evaluator is reachable from any update entry point.** All three sites
>    (`rank_claims`, `push_winner`, `record_hand_to_history`) call `poker_core::try_evaluate_hand`
>    and handle the rejection; `evaluate_hand` is no longer imported into `table_canister` at all,
>    and a test greps for it on every run.
> 2. **a hand that nobody can move can always be ended, by anybody.** `abandon_stuck_hand()` takes
>    no privilege, refuses unless the action clock has been expired for five minutes (or the hand
>    has no clock at all, which is immovable by construction), and can only ever produce one
>    outcome: every stake back to the player who put it in. There is nothing to win by calling it.
> 3. **"Cannot withdraw while in a hand" now means what it says.** That refusal is lifted once the
>    hand is provably stuck. A withdrawal pays out of escrow and cannot touch the pot; a cash-out
>    leaves the stake behind in the payout basis.
> 4. **the false safety argument is gone.** `apply_payouts` used to justify its trap with *"which
>    is recoverable (players can still leave the table with their stacks)"*. They cannot:
>    `leave_table` runs the same code. The comment now says so.
>
> **The gate.** `M9 FUND REACHABILITY` in `tests/money_safety`: *for any state a sequence of legal
> calls can reach, there is a sequence of legal calls by each funded player that returns that
> player's balance to the ledger.* Two halves, both wired into the fuzzer's per-step loop and its
> end-of-run: no update on the settlement path may TRAP while the table holds money, and at the end
> of every run every player's money is actually taken out to the ledger and checked to arrive.
> Reverting the fix takes it RED, with the original trap, in `tests/money_safety/tests/fund_reachability.rs`.
>
> **Still true and still not fixed:** the deployed module is not the source
> ([T-33](DEFECTS.md#t-33)). Everything above is a statement about this tree.

### What was executed

Local `table_2` (`4zfnl-5t777-77775-aaadq-cai`), deployed module `0x5298915c…`. The auditor was
playing an ordinary hand. **One seated player's client stopped sending heartbeats for thirty
seconds while a pre-flop hand was live.** From that moment on:

| call | result |
|---|---|
| `check_timeouts` | IC0503 trap, `IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or 5-card board (flop/turn/river), got 0 community cards` |
| `player_action` | the same trap |
| `leave_table` | the same trap |
| `withdraw` | `Err("Cannot withdraw while in a hand")` |
| `cash_out` | `Err("Cannot cash out while in a hand")` |

Unreachable at that moment: **38,000,000,000 e8s of escrow, 3,985,000,000 e8s of seated chips and a
15,000,000 e8s pot, about 420 ICP.** Not one path a player has was open. It cleared only because
the auditor happened to try `sit_out`, which does not advance the game and which no error message
suggests.

### Why it is a lock and not a hiccup

A trap rolls the message back. The state is therefore **unchanged** after the failure, so the next
call takes the identical path and traps identically. The client polls `check_timeouts`; that is the
call that fails. The UI offers fold, leave, cash out and withdraw; those are exactly the calls that
fail. From the player's chair this is indistinguishable from the operator having taken the deposit.

### Reproduced, live, on the running replica (2026-08-05)

Not replayed in a harness: executed against the **byte-exact deployed module**, on the running
local replica, on a canister created for the purpose (`5tkpr-7d777-77775-aaaeq-cai`, module
`0x5298915c64ea3b3851eb8511243c6d04adb99dac29d6e770867979583ad15e04`, the identical module
`table_1`, `table_2`, `table_3` and `btc_table_1` are all running) so that no other agent's
fixture was disturbed. The module bytes were taken from the replica's own state directory and
hashed before installing, so this is the code that locked the auditor's table and not a rebuild of
it. Script: `$SCRATCH/repro2.sh`.

Three players, 40 ICP deposited each, seated for 10 ICP. One hand dealt. **Carol's client stops
sending heartbeats** (`sit_out` reaches the identical state in one call: `status` leaves `Active`,
`has_folded` stays `false`, the two hole cards stay in the seat). Then ordinary play: the player on
the clock folds.

```
4. ordinary play continues. The player on the clock folds.
  bob fold -> reject code CanisterError … Canister called `ic0.trap` with message:
              'Panicked at 'IMPOSSIBLE HAND: ev…

5. EVERY DOOR EVERY STILL-SEATED PLAYER HAS
  check_timeouts           (variant { NoAction })
  player_action (bob)      TRAP  'IMPOSSIBLE HAND: evaluate_hand needs a 3-…
  player_action (alice)    (variant { Err = "Not your turn" })
  leave_table (alice)      TRAP  'IMPOSSIBLE HAND: evaluate_hand needs a 3-…
  leave_table (bob)        TRAP  'IMPOSSIBLE HAND: evaluate_hand needs a 3-…
  cash_out (alice)         (variant { Err = "Cannot cash out while in a hand" })
  cash_out (bob)           (variant { Err = "Cannot cash out while in a hand" })
  cash_out (carol)         (variant { Err = "Cannot cash out while in a hand" })
  withdraw (alice)         (variant { Err = "Cannot withdraw while in a hand" })
  withdraw (bob)           (variant { Err = "Cannot withdraw while in a hand" })
  withdraw (carol)         (variant { Err = "Cannot withdraw while in a hand" })
```

Nine of the ten calls a player can make are refused or trapped, and the tenth
(`check_timeouts`) changes nothing. Every seat is funded. This is the auditor's report, in full,
on the module they were given.

Two details worth keeping, because both are non-obvious and both matter to the fix:

- **It heals in one direction only, and by accident.** When the disconnect sweep later marks
  *every* remaining seat silent, `count_active_players` falls to **0** rather than 1, the fold-out
  branch is not taken, the board runs out, and the hand settles normally. So the lock is exactly
  the window where that count is **1** while two seats still hold cards. That is why the auditor's
  `sit_out` cleared it: it moved the count from 1 to 1-with-fewer-claims.
- **`check_timeouts` returning `NoAction` is not safety.** It is the polling call, it reported no
  problem, and the table was frozen.

### The mechanism, corrected

The wave-5 write-up below this line reads the source and blames `record_hand_to_history`. **That
is wrong.** The trap was in `plan_payouts` -> `rank_claims`, reached from
`end_hand_single_winner`, which is the FOLD-OUT path and not the showdown path. The canister log
says so verbatim (see the status block at the top of this finding). The version of `rank_claims`
in the deployed module has no board-length guard at all; the guard now in the source was added
after that module was built, which is why the auditor's replay against current source settled.

**And the guard was never the real answer anyway.** A guard makes the trap stop; it does not stop
the engine arriving at a settlement the rules cannot describe. With the guard and without the
cause fix, the same state settles by refunding *every* funder -- **including the players who
folded**. Measured on the wave-5 HEAD engine, three seats, 10 chips each in, one folded, one
disconnected:

```
live claims (not folded, holding cards): 2
collected=30 awarded=30 payouts=[Refund 10 -> seat 0, Refund 10 -> seat 1, Refund 10 -> seat 2]
after: pot=0  alice=100  bob(FOLDED)=100  carol=100
```

Alice won the hand and got nothing; Bob folded and got his bet back. Conservation is exact, M1 to
M8 are all silent, and the hand has simply been un-played. **That is the shape of defect this
project keeps producing: perfectly conserved money in the wrong place.** It is reachable on the
wave-5 engine in **762 of 4,000** randomised endpoint-level sequences and in **0 of 4,000** after
the cause fix.

### The three call sites, as they were

`poker_core::evaluate_hand` traps by design on an impossible input, that is E-09's fix, and the
block comment above it argues for the trap and offers `try_evaluate_hand` to any caller that would
rather handle the rejection. Three call sites in `table_canister` reach it from the settlement path.
**All three now use `try_evaluate_hand`; `evaluate_hand` is not imported into the canister at
all.** As they were:

```rust
// src/table_canister/src/lib.rs:4098  , guarded
if !(3..=5).contains(&state.community_cards.len()) { return Vec::new(); }

// src/table_canister/src/lib.rs:4505  , guarded
.filter(|_| state.community_cards.len() >= 3)

// src/table_canister/src/lib.rs:871   , NOT GUARDED, inside record_hand_to_history
final_hand_rank: if show_cards {
    p.hole_cards.as_ref().map(|cards| evaluate_hand(cards, &state.community_cards))
} else {
    None
},
```

`determine_winners` calls `record_hand_to_history(state, &winners, true)` unconditionally
(`:4631`), and `show_cards = went_to_showdown && !p.has_folded`. So **any** route to a showdown
with fewer than three community cards traps, and both street-advance routines make that state
reachable, because each sets the next phase whether or not it managed to deal a card:

```rust
// advance_to_next_street, GamePhase::PreFlop
if state.deck_index + 3 < state.deck.len() {   // deal the flop …
    …
}
state.phase = GamePhase::Flop;                 // … but advance regardless
```

`run_out_board` (`:3576-3610`) has the same shape at every street: `if state.deck_index <
state.deck.len()` around each push, and an unconditional phase assignment after it. A hand whose
deck cannot supply cards therefore walks PreFlop → Flop → Turn → River → Showdown with an **empty
board** and then traps in the history recorder, permanently.

This composes with [FINDING 12](#finding-12) and DEFECTS E-43: a missing heartbeat drops a seat from
`count_players_can_act`, which is what closes the betting round early and hands control to
`run_out_board` in the first place.

### THE FIX AS LANDED (2026-08-05)

The order matters and it is the order the auditor asked for: cause first, boundary second, and a
door that does not depend on either being right.

**1. The cause. A missed heartbeat is no longer an absence of obligation.** (Landed in the same
wave by the E-32/E-06 work; see the "WHO IS IN THE HAND" block in
`src/table_canister/src/lib.rs`.) Participation and eligibility are now one predicate,
`is_in_hand`, so `count_active_players` and `live_claims` cannot disagree about who is still in
the hand. A seat that stops responding is still offered the action, its clock runs, and it is
folded by `resolve_expired_action_timer` if it does not act. The fold-out settlement can therefore
no longer be entered with two live claims and no board. Measured: **762 of 4,000** randomised
sequences reached that state before, **0 of 4,000** after.

**2. The boundary. No trapping evaluator is reachable from any update entry point.** All three
sites use `poker_core::try_evaluate_hand`:

| site | what a rejection now does |
|---|---|
| `rank_claims` | drops that seat from the ranking and logs `CRITICAL:`. The money it was contesting is refunded to the seats that funded it by `plan_payouts`'s carry rule. |
| `push_winner` | records the winner with no hand description. It has already been decided who is owed what; a display field may not undo that. |
| `record_hand_to_history` | writes `null` for `final_hand_rank`. An archive field may not roll back a completed payout. |

`evaluate_hand` is no longer imported into `table_canister` at all, so the rule is greppable
rather than remembered, and
`no_trapping_evaluator_is_reachable_from_an_update_entry_point` in
`tests/money_safety/tests/fund_reachability.rs` runs that grep on every test run. **Guarding each
site by hand is exactly the discipline that failed here** -- wave 3 guarded two of three -- so the
fix is a rule and not three guards.

**3. The door that depends on neither. `abandon_stuck_hand()`.**

> A recovery path only a controller can take is not a recovery path, it is a support queue.

No privilege check: any principal may call it. It refuses unless the hand is **provably**
immovable, and "provably" means a fact about the state rather than a guess about the cause:

* the action clock has been expired for five minutes (every ordinary call resets it, and the
  longest configured `action_timeout_secs` plus the time bank is 90 s, so this is not reachable by
  a slow player); **or**
* a live hand has no action clock at all, which is immovable by construction, because
  `resolve_expired_action_timer` is the only thing that can move a hand nobody is acting on.

The only outcome it can produce is `refund_every_stake`: each player gets back exactly what they
put into this hand, through `apply_payouts`, so it is held to the same conservation post-condition
as a real settlement. **Nobody wins, so there is nothing to gain by forcing it**, whatever cards
you were holding. It logs `CRITICAL:` when it fires, because reaching it means the settlement path
failed and that must never be quiet. `get_stuck_hand_status()` is a **query**, deliberately: it has
to answer when every update fails.

**4. "Cannot withdraw while in a hand" now means what it says.** Both guards are lifted once
`hand_is_stuck`. This is not weakening them: while the hand can progress they refuse exactly as
before, and that is pinned by
`m9_the_in_a_hand_refusal_lifts_once_the_hand_cannot_progress`. A withdrawal pays out of escrow and
cannot touch the pot; a cash-out calls `record_departed_stake`, so the stake stays in the payout
basis and only the uncontested stack behind leaves.

**5. A false safety argument, deleted.** `apply_payouts` justified its deliberate trap with *"which
is recoverable (players can still leave the table with their stacks and the code can be fixed and
the message retried)"*. **They cannot.** `leave_table` runs the same code and traps too. The
post-condition is kept -- paying out a plan that does not add up is worse than anything -- and the
argument is replaced with the true one: it is safe to trap there only because it is unreachable
from any plan that can now be built, and safe to be wrong about that only because
`abandon_stuck_hand` does not run through it.

### The invariant this repository did not have — M9, now built

Every money invariant here asks whether the arithmetic is right: chips conserved, escrow ≥
liabilities, the right principal paid. **None of them asked whether a player can still get their
money out.** A table can satisfy every invariant in `tests/money_safety` while being permanently
frozen with funded seats, and that is the state the auditor reached in ordinary play.

> **M9 FUND REACHABILITY.** For any state a sequence of legal calls can reach, there exists a
> sequence of legal calls by each funded player that returns that player's balance to the ledger.

`tests/money_safety/src/invariants/reachability.rs`. Two halves, because either alone is a hole:

* **`check_no_settlement_trap`, on every step of the fuzzer, at zero extra message cost.** No
  update on the settlement path may TRAP while the canister holds money. Blunt, and exactly right
  for this canister: a trap is not a failed call here, it is a closed door. The IC rolls the
  message back, so the state that caused it is still there and the next call takes the identical
  path -- and the doors are not independent, because `check_timeouts`, `player_action` and
  `leave_table` all run settlement while `withdraw` and `cash_out` refuse during a hand. Severity
  `FundsUnreachable`, never excusable by the documented-defect register, and it is the one
  severity with a **zero `delta_e8s`**: nothing has gone missing, which is precisely why every
  other invariant is silent.
* **`drain`, at the end of every fuzz run and in each hand-written reproducer.** Actually take
  every player's money out -- `check_timeouts`, then `abandon_stuck_hand`, then `cash_out`, then
  `withdraw`, using only calls a player can make, with time advanced between rounds because
  waiting is a legal move -- and check it lands on the ledger. `final_internal_total` in the fuzz
  report is now the answer to *"how much could not be got out"*, not *"how much was left lying
  about"*. A structural check only fails on the failure modes somebody imagined; a drain fails on
  all of them.

**It has teeth, demonstrated by reverting the fix.** Restoring the deployed shape of `rank_claims`,
the pre-fix `is_in_hand`, and a `hand_is_stuck` that always says false (a copy of the tree in
`$SCRATCH/revert`) takes it from 6/6 green to **5 of 6 red**, with the original trap:

```
M9 VIOLATED: player_action TRAPPED with 6000000000 e8s at the table. A trap rolls the message
back, so this door is shut permanently for this state … 'IMPOSSIBLE HAND: evaluate_hand needs a
3-, 4- or 5-card board (flop/turn/river), got 0 community cards (2 cards in total)',
src/poker_core/src/hand.rs:187:65
```

The one test that stays green under the revert is the checker's own self-test
(`m9s_per_step_check_can_actually_fail`), which must pass either way or the invariant cannot be
trusted to fire.

### Why this outranks its own severity

The auditor replayed the exact frozen state against the **current source** in its own host harness
and it settled correctly. So the binary holding money and the source offered for inspection are not
the same program, and the one holding money is worse
([DEFECTS.md T-33](DEFECTS.md#t-33)). A verifiable shuffle inside an unverifiable binary buys a
player very little.

---

<a id="finding-16"></a>
## FINDING 16 -- a player who stops answering keeps a claim on the pot without paying for it (PLAYER-TO-PLAYER FUND THEFT, DEMONSTRATED, FIXED)

**Status: FIXED in wave 5.** Recorded here rather than only in
[DEFECTS.md E-32](DEFECTS.md#e-32) because it moves real user funds and this file is
where fund-loss findings belong. Everything below was executed by the wave-5 CRITIC
against real installed modules on PocketIC -- not read out of the source, and not
taken from the wave report it was checking.

### What it was

`PARTICIPATION` in the betting round was decided by `status == PlayerStatus::Active`
(`find_next_active_seat`, `count_players_can_act`, `is_betting_round_complete`,
`count_active_players`). `ELIGIBILITY` for the pot was decided by `live_claims`,
which asks only for hole cards and `!has_folded`. A seat that stopped sending
heartbeats for 30 seconds was marked `Disconnected`, dropped out of the first set,
and stayed in the second: **skipped by the betting round while still eligible for
every pot layer it had already paid into.**

No conservation invariant, pot total, or settlement oracle can see this. Every chip
adds up. They just end up with the wrong player.

### Reproduced, pre-fix, on an installed module

Pre-fix module `sha256=8d66080fe9984cf32157e77f49b098eaad6dea8dc6e43fc36c3b9f4e2219fd6b`
(the wave-5 tree with only the E-32 fix sites reverted to their `ad40c53` shape).
Three seats, 2 ICP each, `action_timeout_secs` longer than the heartbeat threshold so
that going quiet is the ONLY thing that happens. The attacker plays the pre-flop
normally, then closes the tab:

```
hand 3: flop reached, attacker paid_in=2000000. after a 35 s silence
        check_timeouts -> NoAction, attacker status=Disconnected folded=false
        phase=Flop action_on=1
hand 3: COMPLETE. board=5 showdown_seats=[0, 1, 2] attacker in showdown=true
        attacker won=6000000 attacker total_bet=2000000 folded=false
```

**6,000,000 e8s collected on a 2,000,000 e8 stake, with one action for the whole
hand and no message at all after the flop.** 4,000,000 e8s of that was the other two
players' money. Two more hands in the same six-hand run reached a five-card showdown
the same way. The edge is measured over 20,000 real deals by
`the_free_showdown_was_worth_about_a_big_blind_a_hand` in
`src/table_canister/tests/betting_rules.rs`: **0.98 big blinds per exploited spot**,
against a certain `-1 BB` for folding.

`sit_out()` was the same hole with no waiting: one message from a player facing a
bet, and the street closed behind them while they kept their cards, their claim and
every chip.

### The same mismatch also settled a hand on a THREE-CARD BOARD, silently

The other direction is worse than "the pot is given away without a showdown". With a
`Disconnected` seat still holding cards, `count_active_players` reached 1 while TWO
seats had a live claim, so `end_hand_single_winner` fired -- and `rank_claims` ranks
any board of 3 to 5 cards. Observed on the pre-fix module:

```
phase=HandComplete
  seat 0 status=Disconnected folded=false cards=true chips=204000000 total_bet=2000000
  seat 1 status=Active       folded=false cards=true chips=198000000 total_bet=2000000
  seat 2 status=Active       folded=true  cards=true chips=198000000 total_bet=2000000
  ended with board=3 winners=[(0, 6000000)] showdown=[]
```

Seat 1 was an honest, connected, unfolded player with 2,000,000 e8s in the pot. The
hand was decided against them on the flop, the turn and the river were never dealt,
**and it was recorded as a fold-out with an empty showdown list, so neither hand was
ever published.** That is a decided poker hand no player could audit.

### It also locked the honest disconnected player out of their own decision

Pre-fix, the street advanced past the quiet seat, so `state.action_on` was never
theirs again. Measured on the same module, for a player who reconnects:

```
==== CRITIC returning-player driver, build=prefix ====
the third seat is Disconnected while facing a 20000000 bet
phase=Turn action_on=2 (disconnected seat is 1) folded=false
the disconnected seat reaches for the time bank -> Err("Not your turn")
heartbeat -> Ok(()) ; and its call lands -> Err("Not your turn")
```

They could not call, could not fold, and could not reach the time bank they had paid
for -- while remaining liable to be paid out on a hand nobody let them play.

### What the fix does, verified on the fixed module

Fixed module `sha256=5e07edf6fdfdb60f96471dcfc6adab226d1c14746d12b291d952c4eeec1a969c`.
Participation is now asked of the same predicate as eligibility (`is_in_hand` /
`can_still_act`). `Disconnected` no longer moves money. Same drivers, same seats:

* the six-hand attack run reaches **0** free five-card showdowns, and the attacker
  ends `-4,000,000` e8s -- exactly the blinds they forfeited by not acting;
* `sit_out()` mid-hand is deferred to the end of the hand and the seat is still
  offered the action;
* a genuinely `Disconnected` seat (95 s of silence, past the new 90 s threshold) is
  offered the action, keeps its cards, and **the hand stays in progress**;
* the same seat can reach `use_time_bank()` **and** land a `Call` when it comes back.

The one path out of a hand for a player who will not act is now their own action
clock, which folds them, exactly as Robert's Rules of Poker requires.

### What is NOT closed by this fix

1. **A forfeited action is a FOLD even when checking would have cost nothing.**
   Measured on BOTH modules: on a flop where `current_bet == 0` and the seat on the
   clock owes 0, the clock expiring produces `folded=true` and forfeits their whole
   stake in the pot. Every online room checks you down in that spot. This is the last
   place a network blip still costs an honest player a pot they could have had for
   free. It is pinned by `reg08` in `tests/money_safety`, so closing it means
   changing that test in the same commit.
2. **[E-36](DEFECTS.md#e-36) is still live and now has a price.** A seat that takes
   an empty chair mid-hand and calls `sit_in()` becomes `Active` holding no cards,
   `is_in_hand` accepts it, and it is offered the action on every street. Measured on
   the fixed module: it bet 20,000,000 e8s on the flop, the turn AND the river --
   **60,000,000 e8s (0.6 ICP) into a hand it could never win**, in a single hand.
   Nothing is destroyed (the internal total is unchanged to the e8 and `plan_payouts`
   refuses to pay a cardless claim), but the money is transferred to the other
   players. Closing it means requiring `hole_cards.is_some()` outright.
3. **`reg09` and FINDING 12 above still describe E-06 as live.** `reg09` now fails
   with `left: Turn, right: HandComplete`, which is its own doc comment's definition
   of "fixed", so `./scripts/dev.sh test` is RED on one test until the pair is
   updated together.

**Reproduce:** the two drivers used above are
`$SCRATCH/critic_e32.rs` and `$SCRATCH/critic_gaps.rs`, dropped into
`tests/money_safety/tests/` of a `cp -Rc` copy and run with
`CRITIC_BUILD=prefix|fixed cargo test --test critic_e32 -- --test-threads=1 --nocapture`.
The pre-fix module is produced by reverting the six E-32 sites in
`src/table_canister/src/lib.rs` to their `ad40c53` shape.

---

<a id="finding-17"></a>
## FINDING 17 (high) -- a hand can be settled with NO live claim on it, and every stake handed back to the players who folded

**Severity:** HIGH. Not fund theft and not fund loss: conservation is exact and every e8 goes
back to the principal that put it in. What is lost is the POT, by the player who won it, to an
ordinary disconnection. And what is lost more broadly is the property this wave was commissioned
to establish — that the table can no longer be wedged into a state the rules of poker cannot
describe.
**Status:** DEMONSTRATED by execution on the wave-6 module `5e07edf6…`, twice, by two different
sequences. **RE-REPRODUCED on `306caef4…` and FIXED 2026-08-05.**
**Where:** `is_in_hand` + `count_active_players` -> `end_hand_single_winner` -> `plan_payouts`'s
"nobody can win ANY of this money" branch, `src/table_canister/src/lib.rs`.
**Related:** docs/DEFECTS.md [E-36](DEFECTS.md#e-36), raised from medium to high by this finding
and closed with it.

> ## FIXED 2026-08-05 — AND THE FIX IS THAT THERE IS NOW ONE PREDICATE, NOT TWO THAT AGREE
>
> **Reproduced first**, on the module wave 6 shipped (`306caef4…`), by `probe4`:
>
> ```text
> PROBE4 pot=52000000 buy_in=200000000
> PROBE4 FINAL phase=HandComplete pot=0
>     seat0 chips=200000000 bet=2000000 folded=true  cards=true
>     seat1 chips=200000000 bet=2000000 folded=true  cards=true   <- WON, paid nothing
>     seat2 chips=200000000 bet=0       folded=false cards=false
> ```
>
> **The fix**, in `src/table_canister/src/lib.rs`:
>
> ```rust
> pub fn is_in_hand(p: &Player) -> bool {
>     !p.has_folded && p.hole_cards.is_some()
> }
> ```
>
> and — this matters more than the deleted disjunct — **`live_claims` now CALLS `is_in_hand`
> instead of restating it.** The two cannot drift apart by an edit to one of them, because there
> is only one of them. `count_active_players` is `live_claims(...).len()` computed without
> building the vector, and `src/table_canister/tests/hand_membership.rs` asserts that equality
> over every reachable `Player` shape.
>
> The same sequence on `34f9d194…` after the fix:
>
> ```text
> PROBE4 FINAL phase=HandComplete pot=0
>     seat0 chips=198000000 bet=2000000 folded=true  cards=true   <- folded, and paid for it
>     seat1 chips=202000000 bet=2000000 folded=false cards=true   <- WON, and was PAID
>     seat2 chips=200000000 bet=0       folded=false cards=false  <- staked nothing, won nothing
> ```
>
> Note seat 1: the hand now settles the instant `alice` folds, so `bob`'s clock never runs at
> all. The defect was never about the clock; the clock was only how a decided hand was allowed
> to keep going.
>
> **What else the disjunct was doing.** `can_still_act` is `is_in_hand && !is_all_in`, so
> `find_next_active_seat` no longer offers the action to a cardless seat and
> `apply_player_action`'s whose-turn check refuses it. That closes E-36's first half: a mid-hand
> arrival can no longer bet into a hand it was never dealt into, so one chair can no longer
> carry two owners' live stakes, so the `WARNING:` line `hand_stakes` writes for that state can
> no longer fire. **Its `TOLERATED_SELF_REPORTS` entry and the `E-36` entry in
> `documented::REGISTER` were both DELETED in the same change, which empties the register.**
> The engine still contains the log line, deliberately, as an untolerated tripwire: emitting it
> now STOPS a money-safety run instead of being counted.
>
> ### The gates, and what each convicts
>
> | gate | where | goes red when |
> |---|---|---|
> | `predicate_table` | `src/table_canister/tests/hand_membership.rs` | any seat shape where `is_in_hand` and `live_claims` disagree, or where `count_active_players != live_claims(...).len()` |
> | `is_in_hand_and_live_claims_are_one_predicate` | same | the same disagreement with a cardless seat sitting BESIDE a card-holder, which one seat at a time cannot see |
> | `a_cardless_seat_is_never_offered_the_action` | same | E-36's first half returns |
> | `probe4` | `tests/money_safety/tests/wave6_coherence.rs` | on the real module: the fold-out winner must end AHEAD of its buy-in and the folder BEHIND |
> | **M11 OUTCOME** | `tests/money_safety/src/invariants/outcome.rs`, every fuzz step | a live hand is observed with one claimant or none; a hand is won by somebody who was not holding a claim; a refund-everyone settlement happens on a hand that had one |
>
> Reverting the one-line predicate turns 5 of the 6 `hand_membership` tests red, `probe4` red
> with *"FINDING 17 HAS RETURNED: seat 1 won a 52000000 e8 pot by fold-out and came out of the
> hand on 200000000"*, and M11 red against the real canister with *"the hand is STILL LIVE with
> 52000000 e8s in it and exactly ONE seat holding a claim"*.
>
> **Why the sixth stays green, and why that is the whole lesson.**
> `the_decided_hand_pays_the_seat_that_holds_the_claim` — the one that drives `plan_payouts`
> directly — PASSES with the defect restored. `plan_payouts` was never wrong. Every instrument
> aimed at the payout function was aimed at the wrong function: the defect was upstream, in what
> the engine believed the state to be when it decided to settle. That is why M11's sharpest leg
> is STRUCTURAL and per-step rather than a payout assertion.

### The disagreement that is supposed to be closed

Wave 6's headline fix (FINDING 16 / E-32) was that PARTICIPATION and ELIGIBILITY must be the
same question. Participation is now:

```rust
pub fn is_in_hand(p: &Player) -> bool {
    !p.has_folded && (p.hole_cards.is_some() || p.status == PlayerStatus::Active)
}
```

Eligibility is `live_claims`, which requires `hole_cards.is_some()`. **A seat that is `Active`
and holds no cards is in the first set and not the second.** That is the identical mismatch, at
a fifth site, in the same wave that closed the other four. The source comment beside it says the
disjunct *"changes nothing; it holds no cards, so `live_claims` still refuses to pay it"* — which
is true of the payout and false of everything upstream of it, and is the sentence that stopped
anyone looking.

`count_active_players(state) == 1` is what triggers `end_hand_single_winner`. The ONE seat it
counts can be the cardless one. `live_claims` is then EMPTY, so `plan_payouts` takes the branch
added in this same wave for corrupt states — *"hand every seat back exactly what it put in"* —
and the hand is un-played.

### Executed, sequence A: nobody needs to cooperate

`tests/money_safety/tests/wave6_coherence.rs::probe4`, in the repo:

```
alice v bob heads-up; a real 52,000,000 e8 pot (0.52 ICP)
carol calls join_table(2) mid-hand              -> Ok      (an ordinary call)
carol calls sit_in()                            -> Ok      (an ordinary call)
alice folds                                     -> Ok      (ordinary poker; BOB HAS WON)
bob's client drops; his own action clock expires
check_timeouts()                                -> PlayerTimedOut(1)

FINAL phase=HandComplete pot=0
  seat0 chips=200000000  bet=2000000  folded=true   <- got their blind back
  seat1 chips=200000000  bet=2000000  folded=true   <- WON the hand, paid nothing
  seat2 chips=200000000  bet=0        cards=false
```

No trap. No `CRITICAL:` line. No `WARNING:`. Exact conservation. M1 through M9 silent and the
M9 drain clean. Every player ends on precisely their buy-in.

### Executed, sequence B: both card-holders fold

`$SCRATCH/cd6/…::wave6_coherence.rs::probe2` reaches the same settlement with a smaller pot and no timeout at
all, which is what makes it cheap to trigger deliberately.

### Why this is worse than it looks, and why it is not a regression

It is **not a regression**: before wave 6 the same state was reached and the money was
*destroyed* rather than refunded (`plan_payouts` returned early and the next `start_new_hand`
zeroed `state.pot`). The refund is a real improvement.

What changed is REACHABILITY. Before wave 6 a seat that stopped answering was SKIPPED by the
betting round; now it is offered the action and **folded by its own clock**, which is correct
poker and is the fix. But it means "every seat holding cards has folded" is now reachable by
ordinary disconnection, where before it needed everyone to fold on purpose. The fix to E-32
increased the traffic through the one door E-36 leaves open.

### What it says about the instruments

This is the second time in this project that a defect has been invisible to every gate because
the gates all ask arithmetic questions. FINDING 15 added M9 FUND REACHABILITY for exactly that
reason — and M9 is a TRAP detector plus a DRAIN detector, and this is neither. The money is
reachable, the arithmetic is exact, and the outcome is still wrong. **The only instrument in the
repo that could convict it is the independent settlement oracle in `tests/settlement`, which
asks what each seat is OWED by the rules of poker rather than whether the totals balance — and
it still does not run inside the fuzz loop and still has no CI job (docs/DEFECTS.md H-23).**

### What the fix cost elsewhere, and what that revealed

**Eighteen betting-rules tests went red, and they were right to.** `flop_table` in
`src/table_canister/tests/betting_rules.rs` built every seat with `hole_cards: None` while
`deck_index` was already advanced past `2 * n` hole cards -- the fixture's own accounting said
those players had been dealt in and the seats said they had not. It compiled, and it passed,
*because* `is_in_hand` accepted a cardless `Active` seat. So the file that contains
`every_seat_the_engine_waits_for_is_a_seat_that_can_win_the_pot` was asserting the rules of
poker against a table of seats that could not win anything. The same fixture defect was in
`coherence_regressions.rs`. Both now deal from the deck the index is counted against.

**Two money-safety tests were PINNING the defect and had to be inverted**, which is exactly the
hazard docs/WAVE-06.md names:

* `m8_a_departed_stake_is_paid_to_its_owner_and_not_to_whoever_took_the_chair` asserted that
  every principal ended the hand LEVEL -- the refund-everyone outcome. It now asserts the
  poker-correct one: the two players who left forfeit their stakes to the seat that still held a
  claim, and `dave` -- who took a chair and staked nothing -- gets nothing. The FINDING 13
  property is now asserted while real money is moving instead of while everything is flat.
* `m8_a_chair_with_two_owners_credits_each_of_them_under_their_own_name` BUILT the two-owner
  chair on purpose, by making `dave` bet into a hand he held no cards in. That is now
  impossible, so the hand is rewritten as
  `a_mid_hand_arrival_is_never_dealt_the_action_and_can_never_stake_the_hand`: it drives the
  identical sequence and asserts `dave` is never on the clock, is refused every action he sends,
  never acquires a stake, and that the engine never emits the dual-stake `WARNING:`.
  `push_winner`'s aggregation by `(seat, principal)` is kept as defence in depth.

---

<a id="finding-18"></a>
## FINDING 18 (high) -- `cash_out` of a stuck hand walks away from your own stake, and nothing anywhere says so

**Severity:** HIGH. Recoverable in principle, permanently lost in combination with
[FINDING 07](#finding-07), and invisible on every endpoint a leaving player would look at.
**Status:** DEMONSTRATED by an independent auditor on the running local canisters, REPRODUCED by
the wave-6 coherence pass, re-reproduced from scratch on the shipped module `306caef4…` at the
start of wave 7, and **FIXED 2026-08-05**. The fix, the measurements on both modules and the
gate are at the end of this section; everything above it is the original write-up, unchanged.
**Where:** `cash_out` (and the identical guard in `withdraw`), via `hand_is_stuck`,
`src/table_canister/src/lib.rs`.

FINDING 15's fix lifted the "cannot cash out while in a hand" refusal once `hand_is_stuck` — a
correct and necessary change, because that refusal was the second half of the lock. But the
guard is the only thing that ever told a player their money was committed. With it lifted, the
seat is vacated and **the stake stays in the pot**, correctly (`record_departed_stake` keeps it
in the payout basis so it is not destroyed), and the player is told nothing at all:

```
PROBE3 stuck_hand_status = { is_stuck: true, hand_in_progress: true, refundable_pot: 3000000 }
PROBE3 cash_out(alice) -> Ok(198000000)   balance 1800000000 -> 1998000000
PROBE3 withdraw(alice, 1998000000) -> Ok
PROBE3 cash_out(bob)   -> Ok(199000000)   balance 1800000000 -> 1999000000
PROBE3 withdraw(bob,   1999000000) -> Ok
PROBE3 AFTER cash_out: phase=PreFlop pot=3000000 seats: (none)
PROBE3 internal_total still owed = 3000000
```

Both players have left, both have withdrawn everything the canister will admit to owing them,
`get_player_count` is 0, and **the canister is sitting on a live pre-flop hand with a
3,000,000 e8 pot and no players in it.** The auditor's transcript is the same shape with bigger
numbers: `cash_out -> Ok = 0` while 2.98 ICP of theirs was in the pot and `get_balance` returned
0.

It IS recoverable: `abandon_stuck_hand()` refunds every stake to the principal that put it in,
and the probe confirms a clean full drain afterwards. But **nothing on the withdrawal path
points at it.** `cash_out`'s reply is a bare `Ok(n)`; `get_balance` omits it; `withdraw`'s error
messages never name it; the table view does not carry it. The one endpoint that does know,
`get_stuck_hand_status`, has to be asked by a client that already knows to ask.

**Fix**, in the auditor's own order of priority:

1. have `cash_out` and `leave_table` report the amount still committed — `Ok = 0` should read
   `Ok = 0, 298_000_000 still in a stuck pot, call abandon_stuck_hand to recover it`;
2. surface it in `get_balance` or the table view, so a client cannot fail to show it;
3. name `abandon_stuck_hand` explicitly in `withdraw`'s refusal path.

None of these touches the money path. All three are strings.

---

### THE FIX AS LANDED (wave 7, 2026-08-05)

**Re-reproduced first, on the module wave 6 shipped** (`306caef4…`, built from `4af6dc6`), with
the auditor's own numbers, by `tests/money_safety/tests/invariants/custody.rs`:

```text
AUDITOR/before: phase=PreFlop pot=300000000 stuck=true bob_stake=298000000
    seat0 chips=296000000 bet=2000000    seat1 chips=0 bet=298000000
AUDITOR/cash_out(bob) -> Ok(0)   get_balance -> 0
    get_balance() -> 0 (escrow only)
    get_custody_status() is not answerable on this build: CanisterMethodNotFound
    get_table_view() carries no custody figure on this build

ORPHAN/after: cash_out alice -> 198000000, bob -> 199000000;
              phase=PreFlop pot=3000000 seats: (none)

DEPARTED: leave_table(alice) -> Ok(198000000); her stake still in hand 1 = 2000000
    every surface she can read: zero
```

**Not the change the auditor asked for, and deliberately so.** Item 1 above cannot be done:
`cash_out` returns `variant { Ok : nat64; Err : text }` and a `nat64` carries no sentence.
Widening it to a record breaks every checked-in client IDL, including the one `+page.svelte`
decodes `Number(result.Ok)` from. So **the state was removed rather than annotated**:

* **`cash_out` and `leave_table` SETTLE an unmovable hand before they vacate the seat**
  (`settle_unmovable_hand`). A stuck hand cannot be won by anybody, so the only lawful outcome
  is the one `abandon_stuck_hand` already produces, and the leaver's stake comes back into their
  stack and goes with them. `Ok = 0` becomes `Ok = 298_000_000`, which is the whole truth rather
  than a number with a caveat attached. **No new power**: any principal can already call
  `abandon_stuck_hand` on a stuck hand, so this is one call folded into another.
* **The last player out turns the lights off.** A departure that empties the table settles a
  live hand the same way. Nobody holds cards; nobody can win it.
* **`get_custody_status()`** (new caller-scoped query) reports `escrow`, `chips_at_table`,
  `committed_in_pot`, `committed_is_stuck`, `abandonable_in_ns`, `total`, and an `advice` string
  that names `abandon_stuck_hand`.
* **`TableView.my_committed_in_pot` / `hand_is_unmovable`**, so the call every client already
  polls carries it. Non-zero with `my_seat = null` IS this finding.
* **`withdraw`'s refusals**, both of them, carry the same sentence from the same function, with
  the exact e8 figure.

The same sequence on the fixed module:

```text
CRITICAL: hand 1 was ABANDONED as unmovable at phase preflop BY AN EXIT DOOR ...
          300000000 e8s across 2 stakes returned to the players who put them in,
          including the leaver's
AUDITOR/cash_out(bob) -> Ok(298000000)   get_balance -> 298000000
AUDITOR/withdraw(298000000) -> wallet 999701980000 -> 999999970000
AUDITOR/after: phase=HandComplete pot=0
ORPHAN/after: phase=HandComplete pot=0 seats: (none)

DEPARTED/withdraw refusal: Insufficient balance. Have: 19.9800 ICP, requested: 20.0000 ICP.
  0.0200 ICP (2000000 e8s) of yours is still committed to hand 1, and that hand can no longer
  be moved by any message, so nobody can win it. Call abandon_stuck_hand() -- any principal
  may -- and every stake goes back to whoever put it in, including yours, into your
  withdrawable balance.
```

**The way this fix could have re-created [FINDING 15](#finding-15), and the guard against it.**
`settle_unmovable_hand` runs INSIDE the two exit doors, and `apply_payouts` traps on a plan that
does not conserve. A trap rolls the message back, so a bad plan would have taken the player's
exit with it and shut the door they were walking through: the exact shape of the fund lock this
project spent a wave removing. The plan is therefore checked before it is acted on, and a
non-conserving one is declined and logged rather than settled. The player still leaves.

**The gate: M10 CUSTODY VISIBILITY** (`tests/money_safety/src/invariants/custody.rs`), evaluated
on every fuzz step and by four tests in the `invariants` target:

> If a principal has a positive stake in a pot, at least one surface **that principal can read**
> must say so.

Every other reader in the money-safety harness is a controller, which is exactly how this stayed
invisible: M1 balanced, M9's drain reported `fully_drained` (correctly, because the money WAS
reachable), and the only endpoint that knew was one no client would think to call. M10 reads the
canister as the player and counts a missing method, a rejected call and an undecodable reply all
as "reports nothing", so it convicts the pre-fix module instead of failing to build against it.

**What the gate does not prove.** This tree also carries the on-chain clock from
[FINDING 19](#finding-19), which refunds a stuck hand within 30 s by itself. A fixture that
merely advances time can therefore be satisfied by the clock rather than by the exit door, so
the custody tests advance time WITHOUT executing a round and assert on the canister's own log
line, `BY AN EXIT DOOR`. The gate fails if the exit-door code is removed even while the clock
remains.

**Still not closed by this fix:** the composition with [FINDING 07](#finding-07). A stake that is
now visible and recoverable is still destroyed by `reset_table`, and that is that finding's to
answer.

---

<a id="finding-19"></a>
## FINDING 19 (high) -- nothing on chain moves the game, so liveness is outsourced to whoever has a browser tab open -- **THE CLOCK IS FIXED 2026-08-05. THE CYCLES HALF IS OPEN AND THIS FIX MADE IT WORSE.**

**Severity:** HIGH for a fund-holding canister.
**Status:** the timer half is **CLOSED**, measured before and after with every client closed.
The cycles half is **OPEN** and is now the more urgent of the two, because the fix burns cycles
continuously. Reproducer and gates: `tests/money_safety/tests/timers.rs`. Register entries:
[E-54](DEFECTS.md#e-54), [E-55](DEFECTS.md#e-55), [E-56](DEFECTS.md#e-56).

### What it was

`ic-cdk-timers = "1"` was declared at `Cargo.toml:22` and `set_timer` appeared **nowhere** in
`src/`:

```
$ grep -rn "set_timer" src/          # no output
```

Every clock in this engine -- the action timer, the disconnect threshold, the sitting-out
auto-kick, the reload timer, the stuck-hand grace -- was evaluated only inside a message
somebody else sent. `check_timeouts` is an ordinary update call.

### The dead window was worse than the auditor measured

The auditor reported `abandonable_in_ns = 169_194_575_000` at t+160 s and called it a
**5.5-minute minimum dead window**. That is the time until `abandon_stuck_hand` would be
*accepted*, and that method needs a caller too. Driven to the end on the pre-fix module, with
nothing but subnet ticks and queries after the clients close:

```
  t+  60s  phase=PreFlop pot=3000000 stuck=false abandonable_in=Some(269)s seated=2
  t+ 360s  phase=PreFlop pot=3000000 stuck=true  abandonable_in=None       seated=2
  t+1200s  phase=PreFlop pot=3000000 stuck=true  abandonable_in=None       seated=2
  RESULT: hand resolved at None, seats released at None
```

**Twenty simulated minutes, a live pot, and the dead window is unbounded, not 5.5 minutes.**
Both seats were also still marked `Active` after twenty minutes of total silence: the
disconnect threshold had never been evaluated either, because nothing evaluated anything.

### After the fix, same setup, same silence

```
  t+  30s  HAND RESOLVED ITSELF, phase=HandComplete
      seat0 chips=201000000 folded=false   <- won the pot on the fold-out
      seat1 chips=199000000 folded=true    <- folded by its own clock
  t+ 210s  every seat released, chips back in escrow
  internal total unchanged at 4000000000 e8s
```

* **30 s** to resolve the hand: exactly the table's action clock.
* **210 s** to release every seat: exactly `DISCONNECT_TIMEOUT_SECS` (90) +
  `SITTING_OUT_KICK_SECS` (120). The chips are then in escrow, reachable by an ordinary
  `withdraw`.
* No e8 created or destroyed anywhere along the way.

**Mid-hand upgrade, gated separately.** Timers do not survive upgrades: the CDK task queue is
heap and the system global-timer field is cleared. `post_upgrade` re-arms last, against the
state it actually restored; after an upgrade taken with a live hand the canister reports
`clock_watchdog_armed = true` and `next_wake_at` 29 s out, aimed at the *restored* action clock,
and the hand still resolves itself in 30 s with no client attached.

**`check_timeouts` is still callable** so the system works if a timer is ever lost, and both
paths call one function, `advance_table_clock`. That is gated behaviourally, not by inspection:
`the_clock_and_check_timeouts_cannot_drift` drives two identical tables on a real replica, one
by clock alone with zero ingress and one by `check_timeouts` alone, and requires the same
money-carrying end state.

### The fact that decided the design

`ic-cdk-timers` 1.0 runs every callback behind a bounded-wait self-call, explicitly to catch
traps at the call boundary, and `global_timer.rs::do_timer` step 7 reads:

> If a repeated timer is successfully **dispatched** (irrespective of the timer's own success),
> reschedule it.

So a **repeating** timer is rescheduled before its callback runs and survives a trapping
callback. A **self-rescheduling one-shot** does not: the re-arm is in the same message as the
work, so one trap ends the clock permanently with nothing on chain to notice. On a canister
whose settlement path has trapped before ([FINDING 15](#finding-15)) that is decisive, so the
shape is a repeating 30 s watchdog (the part that cannot die) plus a one-shot aimed at the exact
next deadline (the part that is precise, and free to arm because only a *fire* costs a message).

This was not theoretical. The first build of the tick trapped on **every** tick on a `RefCell`
double-borrow, and only the watchdog's dispatch-time rescheduling kept the clock alive to be
noticed. Full write-up: [E-56](DEFECTS.md#e-56).

### The defect this fix introduced, and why it belongs in a security file

The clock's first build **refunded every stake in a playable hand**. It treated "the action
clock expired more than five minutes ago" as "nothing can move this hand" and voided the hand
before ever trying the ordinary timeout path. The money-safety fuzzer convicted it on its second
seed.

Those two are the same claim only while the clock is running. They come apart after any stall in
which the canister does not execute -- and the sharpest way to get one is **to run out of
cycles**. So as first written, this fix and the open cycles finding below composed: a table that
froze for want of cycles and was then topped up would have voided its hand on the way back up,
with a `CRITICAL:` line saying it was unmovable when it was merely stale.

The grace is now measured from **when the canister first saw the clock overdue**, so a stall
buys no credit toward abandonment. Trap-safety is preserved by putting the sighting and the work
in separate messages: the sighting commits, the work may trap and roll back alone, and the grace
keeps running. Gated by `a_hand_stalled_past_its_grace_is_played_out_not_voided`, which is
verified red against the reverted fix -- the fuzzer alone is **not** enough, it stays green.
Full write-up: [E-56](DEFECTS.md#e-56).

### The unexplored area the auditor named next: CYCLES. STILL OPEN, AND NOW MORE URGENT.

A canister below its freezing threshold **rejects every update call**: `deposit`, `withdraw`,
`cash_out`, `player_action`, `reload` and `abandon_stuck_hand` all stop at once. That is a total
custody failure with no attacker and no in-application remedy, and queries keep answering, so
the UI would go on showing balances the canister can no longer pay out.

**The fix above makes it arrive sooner, by a factor of ~630.** Measured, empty table, one
simulated hour on a PocketIC 13-node application subnet:

| | idle burn | runway at 1 T | at 10 T | at 50 T |
|---|---|---|---|---|
| before the clock | 0.00007 T/day | ~39 years | ~390 years | ~1,950 years |
| after the clock | **0.0442 T/day** | **22 days** | **226 days** | **1,130 days** |

Per tick: **15,361,360 cycles** at a 30 s watchdog, 15,386,002 at 60 s -- exactly linear, so
`CLOCK_WATCHDOG_SECS` prices the whole design: `86400 / period * 15.4M` cycles per day. For
comparison, the bare repeating interval this finding originally suggested costs 485 T/year per
idle table at 1 s.

The freezing *reserve* is unaffected -- measured at 2,128,884,000 cycles, i.e. 30 days of
storage-only burn -- because the reserve is computed from idle resource consumption, not from
message execution. The clock does not raise the threshold; it drains the balance toward it.

**What was added:** `get_cycle_status()`, a query open to anybody including anonymous callers,
because the number that predicts a total custody failure must not be privileged. It reports
`liquid_balance` (the figure that actually reaches zero), a burn rate **measured on the running
instance** rather than taken from a price list, `runway_days`, and `clock_ticks` -- the only
externally visible evidence that the clock is alive. A canister up for minutes with
`clock_ticks == 0` has lost its clock.

**What was deliberately NOT built, and what it would need.** No top-up mechanism exists
anywhere in this tree, and `get_cycle_status` must not be read as one:

1. **A funding path.** A `deposit_cycles` endpoint is trivial; deciding *who pays* is not.
   These are per-table canisters, so the bill scales with the lobby.
2. **A wallet or minting arrangement** the tables can pull from, plus an actor authorised to
   move value into them -- an additional privileged actor on a canister where
   [FINDING 07](#finding-07) is still open, which is exactly why not to bolt one on in a hurry.
3. **An off-chain watcher** polling `get_cycle_status` across every table, alerting well above
   zero (`runway_days` under ~60), because once a canister is frozen the remedy needs someone
   awake.
4. **A decision about degraded operation.** Refusing new buy-ins while still honouring
   withdrawals is a better failure mode than serving both until everything stops at once.
   Nothing in the engine expresses that today.

Until at least 1 and 3 exist, the honest answer to *"can I always get my money out"* is still
**no**, and the reason still has nothing to do with poker.

---

<a id="finding-24"></a>
## FINDING 24 (high) -- a frozen table answers NOTHING, not "queries only": the mitigation this project has documented in four places does not exist

> **RENUMBERED IN WAVE 7.** This finding and the two below were written as 21, 22 and 23 by the
> critic of the timers work, in the same wave that the critic of the admin/custody work wrote its
> own 21, 22 and 23. Six findings shared three numbers and three `<a id>` anchors, so every
> internal link resolved to whichever duplicate the reader reached first. The cycles/timers set was
> moved to 24-26; the admin/custody set kept 21-23 because it appears first in this file. See
> docs/WAVE-07.md §7.

**Severity:** HIGH. No funds are destroyed and none can be stolen. What is destroyed is the
*only* remedy [FINDING 19 §cycles](#finding-19) / [E-55](DEFECTS.md#e-55) offers, at the exact
moment it is needed.

**Status:** EXECUTED on the running system by an adversarial reviewer of the wave-6 clock work,
2026-08-05. Module under test `cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105`
(the wave's final build), PocketIC 13-node application subnet, which runs the **real** IC
execution environment.

### The claim that is false

Four places in this project say the same thing, and it is the load-bearing sentence of the
cycles remediation plan:

| where | what it says |
|---|---|
| `docs/DEFECTS.md` E-55 | *"Queries keep answering, so the UI would continue to show balances it can no longer pay out."* |
| `src/table_canister/src/lib.rs`, `get_cycle_status` | *"A query, so it costs the caller nothing and **keeps working when the update path does not**."* |
| `src/table_canister/src/lib.rs`, `CustodyStatus` | *"It is a QUERY, so it is free and it **still answers when the update path is refusing everything**."* |
| [FINDING 19](#finding-19) §cycles item 3 | an off-chain watcher **polling `get_cycle_status`** is the proposed alarm |

A frozen canister on the Internet Computer rejects **query calls too**. Not degraded, not
stale: rejected at the boundary, by the replica, before any canister code runs.

### Reproduced

A funded table, one player with 10.00000000 ICP in escrow, driven below its freezing threshold
(by raising `freezing_threshold`, which is the same state a table reaches by burning its
balance down, and is reversible so the same instance also proves recovery):

```
  BEFORE freezing: get_table_state=ANSWERED get_cycle_status=ANSWERED
  --- FROZEN ---
  QUERY  get_cycle_status       -> REJECTED: Canister ... is unable to process query calls
                                   because it's frozen. Please top up the canister with
                                   cycles and try again.
  QUERY  get_table_state        -> REJECTED: (same)
  QUERY  get_stuck_hand_status  -> REJECTED: (same)
  UPDATE withdraw               -> REJECTED: Canister ... is out of cycles
  UPDATE check_timeouts         -> REJECTED: Canister ... is out of cycles
  UPDATE abandon_stuck_hand     -> REJECTED: Canister ... is out of cycles
  after 300s frozen: balance 99987160901499 -> 99987160655019 (delta 246,480 -- storage only,
                                   the clock does not run while frozen)
  --- AFTER TOP-UP ---
  get_balance=1000000000   withdraw -> Ok
  internal_total=0  ledger_main=0
```

The update rejections carry `CanisterOutOfCycles` and happen at ingress submission, so the
message is never even accepted into the pool.

### Why this is worse than what E-55 describes, not better

E-55's picture is a canister that lies: it shows balances it cannot pay. The reality is a
canister that **disappears**. Every consequence gets worse:

* **The alarm cannot fire from the endpoint built to raise it.** `get_cycle_status` returns
  `runway_days` right up until the moment it stops returning anything. An off-chain watcher
  built the obvious way -- poll, read `runway_days`, alert if low -- gets a transport-level
  reject and, unless it was written to treat *unreachability itself* as the alarm, reports
  nothing at all. The endpoint added this wave cannot report the state it exists for.
* **A player cannot even see what they are owed.** `get_balance`, `get_custody_status` and
  `get_stuck_hand_status` all go dark, so the surface added for
  [FINDING 18](#finding-18) -- "what is this canister holding for *me*" -- is unavailable in
  precisely the incident where somebody would ask.
* **The failure is indistinguishable from a subnet problem** to anybody outside, which is how
  an operator loses hours before looking at cycles at all.

### The freezing reserve is not 30 days any more, it is about one hour

Measured on the same build, from outside the canister:

| | value |
|---|---|
| `reserved_for_freezing` | 2,129,748,540 cycles |
| observed burn (external, one simulated hour) | 1,843,363,680 cycles/hour = 44,240,728,320/day |
| **what the reserve is worth at that burn** | **1.16 hours** |

The freezing threshold is `freezing_threshold_seconds x idle resource consumption`, and idle
resource consumption counts memory and compute allocation only -- it knows nothing about a
timer that sends messages. E-55 already records that the reserve is unchanged by the clock.
The consequence it does not draw is that the reserve's *protective value* fell by the same
~630x as everything else: the buffer between "running" and "gone" is now about an hour of
burn, so `runway_days` from `get_cycle_status` is the **only** usable warning, and per the
section above it stops being readable at zero.

### What this changes about the remediation

Item 3 of [FINDING 19](#finding-19)'s list is still right but is not sufficient as written. The
watcher must alarm on **failure to reach the canister**, not only on a low `runway_days` it
manages to read, and the alarm threshold has to be far enough above zero to leave a human time
to act, because the reserve buys about an hour and not 30 days.

### Reproducer

`tests/money_safety/tests/timers.rs` has no test for this. The probe used was written outside
the tree (adversarial review, scratch copy) and is straightforward to port: raise
`freezing_threshold` on the table canister via `pic.update_canister_settings`, then attempt one
query and one update and assert on the rejections. It belongs in the tree, because the sentence
it falsifies is repeated in four places and is the basis of the cycles plan.

---

<a id="finding-25"></a>
## FINDING 25 (HIGH -- raised from medium in wave 7) -- after a stall, one permissionless call VOIDS the hand the clock would have played out; the player who was losing is the one with the incentive, and the canister TELLS them to do it

**Severity:** **HIGH**, raised from MEDIUM by the wave-7 coherence pass. Total money is conserved
exactly. What moves is the *outcome*: a pot that one player had won on a fold-out is handed back,
and the player who chooses that is the player who was about to lose it. This is the same shape as
[FINDING 17](#finding-17) -- a hand settled with the stakes returned rather than the winner paid --
reached by a different door.

Three facts found after the original write-up move this out of MEDIUM, and all three are measured
in the amendment at the end of this section:

  1. `get_custody_status()` **instructs the losing player to press the button**, in the canister's
     own words, at exactly the moment the canister's own clock is about to fold them;
  2. `cash_out()` and `leave_table()` -- both wired to this predicate **in the same wave** by a
     different agent -- are strictly better for the attacker than `abandon_stuck_hand()`: they
     recover the stake **and** take the whole stack out of a live hand in one call;
  3. a principal who has **never sat at the table** can settle the hand and receives an error reply
     saying it did nothing.

**Status:** EXECUTED, 2026-08-05, adversarial review of the wave-6 clock work. **Re-reproduced and
extended 2026-08-06 by the wave-7 coherence pass**, independently, from the other end (looking for
a seam between agents rather than attacking the timer work). Same module,
`cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105`.

**RENUMBERED IN WAVE 7** from FINDING 22; see docs/WAVE-07.md §7.

### The residual half of E-56

[E-56](DEFECTS.md#e-56) defect 2 is the correct observation that *"past its grace"* is a claim
about the wall clock while *"nothing can move this hand"* is a claim about attempts, and that
they come apart after any stall in which the canister does not execute -- a subnet halt, a
canister frozen for want of cycles and then topped up, or simply a controller stopping the
canister for longer than five minutes to upgrade it. The fix measures the grace from when the
canister **first saw** the clock overdue.

That fix was applied to `clock_should_abandon`, the door the timer uses. It was **not** applied
to `hand_is_stuck`, the door `abandon_stuck_hand` uses, which still reads

```rust
Some(ref t) => now > t.expires_at.saturating_add(STUCK_HAND_GRACE_NS),
```

`abandon_stuck_hand` is permissionless by design, and that design is right. The consequence is
that after any stall longer than the five-minute grace, the manual door is open **instantly**,
before the clock has had a single opportunity, and it stays open until somebody's message wins
the race with the timer.

### Reproduced: the same stall, two money outcomes

Two identical tables, heads-up, 200.00000000 ICP each, a live pre-flop hand with a 3,000,000 e8
pot. Both are stalled for 3,600 s with **no execution at all** -- the exact construction
`timers.rs::a_hand_stalled_past_its_grace_is_played_out_not_voided` uses. Then:

```
  ARM A (no caller at all -- the clock alone, i.e. the builder's own gate):
      abandon lines = 0     stacks = [201000000, 199000000]
      -> seat 1's clock ran out, seat 1 was folded, seat 0 WON the pot.

  ARM B (identical, except seat 1 calls abandon_stuck_hand while the canister comes back up):
      abandon lines = 1     refunded = 3000000     stacks = [200000000, 200000000]
      -> "CRITICAL: hand 1 was ABANDONED as unmovable at phase preflop: no message could
          advance it. 3000000 e8s across 2 stakes returned to the players who put them in;
          nobody won the hand."
```

Nothing was unmovable in arm B either. It is the same hand, in the same state, that arm A plays
out. Seat 1 -- the seat that loses 1,000,000 e8s in arm A -- recovers it in arm B by sending one
message that anybody is allowed to send, and the log records the hand as unmovable when it was
merely stale.

Eight concurrent `abandon_stuck_hand` calls during the same recovery were also tried: exactly
one is accepted, the refund happens once, and `internal_total` is unchanged. **There is no
double-refund and no arithmetic defect here.** The defect is that the door opens at all.

### Why the gate does not see it

`a_hand_stalled_past_its_grace_is_played_out_not_voided` sends no ingress after setup, so it
only ever exercises arm A. Its assertion message states a property of the *system* -- *"A stall
is not a stuck hand: the seat whose clock expired should have been folded and the hand played
out"* -- which the system does not have. Adding one `abandon_stuck_hand` call to that test turns
it red.

### The fix, and the reason it is not obviously free

Making `hand_is_stuck` agree with `clock_should_abandon` means the manual door also has to be
told when the canister first saw the clock overdue, i.e. it has to read `CLOCK_STUCK_SINCE`.
That couples the permissionless escape hatch to timer state, and the escape hatch exists
precisely because the timer might be dead -- a canister whose clock never re-armed after an
upgrade would then have a *permanently closed* escape hatch, which is
[FINDING 15](#finding-15)'s fund lock rebuilt. The safe shape is probably a floor rather than a
substitution: refuse until the clock has been overdue for the grace period **and** the canister
has been executing for at least one grace period, using a first-sighting timestamp that
`post_upgrade` re-seeds. That is a design decision, not a patch, which is why it is recorded
here rather than changed.

---

### WAVE-7 AMENDMENT — three doors, not one, and a signpost pointing at them

Reproduced independently by the coherence pass, which was not attacking the timer work but looking
for a state on which the wave's four agents disagree. It found exactly one, and this is it. Probes:
`$SCRATCH/w7_coherence.rs`, run against the same module.

**1. The canister instructs the losing player to void the hand.** `get_custody_status()` is a
surface the custody-visibility agent added **this wave**, and it derives `committed_is_stuck` from
`hand_is_stuck` -- the raw predicate -- not from the rule the timer agent established. Measured at
the moment of the stall, before anything has executed:

```text
get_stuck_hand_status: is_stuck=true hand_in_progress=true refundable_pot=3000000

LOSER (the seat on the clock): committed=1000000 stuck=true
    advice: 0.0100 ICP (1000000 e8s) of yours is still committed to hand 1, and that hand can
    no longer be moved by any message, so nobody can win it. Call abandon_stuck_hand() -- any
    principal may -- and every stake goes back to whoever put it in, including yours, into your
    withdrawable balance.
```

Every clause after "committed to hand 1" is false. Driven by the clock alone, with **zero ingress
messages**, the same hand resolves in **2 rounds**: the seat on the clock is folded and the other
player wins the pot. The sentence the player is shown is contradicted by the canister's own timer a
few seconds later.

`abandon_stuck_hand`'s own doc comment says *"There is nothing to win by calling it, whatever cards
you were holding."* Measured: 2,000,000 e8s.

**2. `cash_out` and `leave_table` are on the same predicate, and they are the better weapon.**
Both were given a `settle_unmovable_hand` call at the top **in this wave**, as the fix for
[FINDING 18](#finding-18), and both ask `hand_is_stuck`. `abandon_stuck_hand` only recovers the
stake; `cash_out` recovers the stake and removes the stack from a live hand:

```text
cash_out(the seat on the clock) -> Ok(200000000)
  phase=HandComplete pot=0 (was 3000000)
  escrow: loser=2000000000  winner=1800000000
```

The player who was about to be folded out leaves with every e8 they arrived with, and the opponent
who had won the hand is level. No "recovery method" needs to be called.

**3. An `Err` reply that commits a settlement.** The settle runs *before* the seat lookup, and
returning `Err` from an ic-cdk update is a normal reply, not a rollback:

```text
mallory (never at the table) cash_out -> Err("Not at table")
  phase PreFlop -> HandComplete    pot 3000000 -> 0
```

Any principal at all can void any stalled hand and be told the call failed.

**4. The two arms, side by side, with a real pot.** One state, two doors:

```text
=== ARM: the clock alone, ZERO ingress
  pot=4000000  phase=PreFlop  on_clock=alice  stakes=[2000000, 2000000]
  clock resolved it after 2 rounds
  RESULT phase=HandComplete   alice=+0        bob=+4000000    sum=4000000

=== ARM: abandon_stuck_hand, called by alice
  abandon_stuck_hand(alice) -> Ok(4000000)
  RESULT phase=HandComplete   alice=+2000000  bob=+2000000    sum=4000000
```

Correct totals. Wrong recipients. Every conservation invariant silent.

**5. Why no gate in this project can reach it.** `World::advance()` is
`pic.advance_time(d); pic.tick();` -- it **always lets a round run**, so the on-chain clock always
wins the race and the window never opens. `World::advance_time_only` is the only thing that opens
it, it appears in exactly three places in the tree (all in
`tests/money_safety/tests/invariants/custody.rs`), and **all three use it for QUERIES only**. No
test in this repository has ever sent an UPDATE through the window. That is the gate this fix owes:
`advance_time_only`, then an update, on all three doors. See [E-59](DEFECTS.md#e-59).

**What the fix must also cover.** The floor described above has to be applied to `hand_is_stuck`
itself, not only to `abandon_stuck_hand`, because `cash_out`, `leave_table`, `get_custody_status`,
`get_stuck_hand_status` and `TableView.hand_is_unmovable` all read it. Fixing the one method named
in the original write-up would leave two money doors and three player-facing surfaces on the old
rule.

---

<a id="finding-26"></a>
## FINDING 26 (high) -- the 226-day runway assumes nobody is hostile: a free, permissionless ingress flood burns a table 65x faster, collapsing it to about three days

**Severity:** HIGH, and it is **not** caused by the wave-6 clock. It is the number E-55's
runway table is missing, and it changes the cycles finding from "a predictable date" to "a date
an attacker picks".

**Status:** EXECUTED, 2026-08-05, adversarial review. Same module, `cb26fb94...`.

### What was run

Two tables, identical configuration, identical number of replica rounds, 600 simulated seconds
each. One is left alone. On the other, one seated player calls `check_timeouts` -- a
permissionless update with no rate limit and no argument -- five times per simulated second.

```
  idle   600s:            307,227,280 cycles,  20 clock ticks
  attack 600s:         20,038,661,478 cycles,  20 clock ticks, 3,000 ingress calls
  amplification: x65.2                          attacker cost: 0
```

The clock is not the mechanism: both arms ran the same 20 ticks, so the timer accounts for
~1.5% of the attacked burn. The cost is **ingress induction plus execution, which the IC
charges to the canister, not to the caller**. Extrapolated, that is 2.886 T/day, so a table
holding 10 T cycles has **3.5 days** of runway rather than 226, and the attacker pays nothing
and needs no funds, no seat and no privilege. All 3,000 calls were accepted; none was rate
limited.

### Why it matters here specifically

Combined with [FINDING 24](#finding-24) above, the end state is a table that stops answering
queries and updates at a time of an attacker's choosing, with every player's escrow intact but
unreachable until somebody tops it up. Nothing in this tree tops anything up, and the
observability added this wave goes dark at the same instant.

`CLAUDE.md` in this repo already states the intended policy -- *"All user-facing update calls
should be rate-limited to prevent DoS"*, with named budgets for player actions, deposits,
heartbeats and withdrawals. `check_timeouts`, `abandon_stuck_hand`, `get_*` updates and the
other permissionless entry points are outside it. Rate limiting is not by itself a fix (a
rejected ingress message is still charged to the canister, only more cheaply), but the gap
between the stated policy and the code is worth closing before the cycles work is designed,
because whatever budget a top-up mechanism is sized against has to survive this.

---

<a id="finding-27"></a>
## FINDING 27 (high) -- the ICP deposit floor is BELOW the withdrawal floor, so money that arrives at the product's own documented minimum can never leave -- **OPEN**

**Severity:** HIGH. Fund LOCK, no malice at any step, reachable by following the application's own
instructions. Bounded at 99,999 e8s per player per table, but unrecoverable and silent.
**Status:** OPEN. Found by the THIRD independent auditor, 2026-08-05, reproduced live on the
running replica. Recorded here by the wave-7 coherence pass; it appeared in no document before.
**Where:** `src/table_canister/src/lib.rs:1889` (`min_deposit`) against `:66`
(`ICP_MIN_WITHDRAWAL_AMOUNT`); mirrored into
`src/cleardeck_frontend/src/lib/components/DepositModal.svelte` and `WithdrawModal.svelte`.

```rust
// deposit(), line 1889
let min_deposit = if currency == Currency::BTC { 1_000 } else { 20_000 };
// line 66
const ICP_MIN_WITHDRAWAL_AMOUNT: u64 = 100_000;
```

Any ICP escrow balance in **[20,000, 100,000)** e8s is unreachable by every door:

```text
cd-carol deposits exactly the advertised minimum, 20,000 e8s
  withdraw(20_000)      -> Err "Minimum withdrawal is 0.0010 ICP"
  withdraw(100_000)     -> Err insufficient balance
  buy_in(0, 20_000)     -> Err "Minimum buy-in is 2.0000 ICP"
  get_custody_status    -> total = 20_000, escrow = 20_000, advice = ""
```

`table_1` on the auditor's instance ended holding **80,000 e8s belonging to two real players**,
with no method any player or controller can call that will move it.

**It is not only the documented minimum.** The band is reachable by ordinary play: any player who
loses down to a balance under 0.001 ICP is in it. The deposit minimum merely makes it reachable on
the first action a new player takes.

**BTC is the other way round** (deposit floor 1,000 sats, withdrawal floor 11 sats) and has no
trap, which is why nobody noticed. `WithdrawModal.svelte` reasons through this exact trap *for BTC*
-- *"Enforcing 1,000 client-side would TRAP DUST... Choosing the higher number costs a player their
remaining balance"* -- without noticing the canister does it for ICP.

**The fix is one number and a gate.** The deposit floor must be at least the withdrawal floor, and
`ui_limits.rs` must assert the relation `min_deposit >= min_withdrawal` per currency rather than
only mirroring each into the modals.

<a id="finding-28"></a>
## FINDING 28 (high) -- money at the canister's OWN published deposit address is reported as zero by every balance surface, and the error text tells the player to send more -- **OPEN**

**Severity:** HIGH. Fund LOCK below the transfer fee, total invisibility above it.
**Status:** OPEN. Found by the third independent auditor, 2026-08-05, reproduced live. This is the
same account the wave-7 critic reached from the opposite direction in
[FINDING 21](#finding-21); [FINDING 11](#finding-11) records the dust half only.
**Where:** `claim_external_deposit` (`src/table_canister/src/lib.rs:2008`), `get_balance`,
`get_custody_status`, `admin_get_balance`, `admin_get_all_balances`.

```text
cd-bob transfers 10,000 e8s to the address get_deposit_subaccount() gave him
  icrc1_balance_of(that account)  -> 10_000
  claim_external_deposit()        -> Err "No claimable balance. Send ICP to your deposit
                                     address first. Use get_deposit_subaccount() to get
                                     your address."
  get_custody_status()            -> total = 0, escrow = 0, advice = ""
  get_balance()                   -> 0
```

**This is [FINDING 18](#finding-18) in a second account.** FINDING 18 was written because a player
was told they had nothing while money of theirs sat in the pot; the fix built
`get_custody_status()` to answer "what is this canister holding for *me*". It answers **zero** for
money sitting at the address the canister itself published to the player, and the reply the player
does get instructs them to send **more** money to the address already holding theirs.

**Why every instrument agrees with the lie.** The canister owns two kinds of ledger account and
every measurement in this project is anchored to one of them:

| instrument | anchor | sees the subaccounts? |
|---|---|---|
| M1 `ledger_equals_owed` | `icrc1_balance_of(table, None)` | no |
| M9 `check_no_orphaned_custody` | `ledger_main - claims - uncredited_raw` | no |
| `total_liability()` (the FINDING 20 guard) | `escrow + chips + pot` | no |
| `DrainReport::table_is_really_empty` | `ledger_main` | no |
| all four balance surfaces | `BALANCES` | no |

`World::snapshot()` already computes `ledger_deposit_subaccounts` and **nothing compares it to
anything**. See docs/WAVE-07.md §5: re-anchoring to every account the canister owns is the single
highest-leverage fix in the project, because it closes this, half of FINDING 21, and the M9 hole at
once.

<a id="finding-29"></a>
## FINDING 29 (high) -- both deposit paths move real money on the ledger before writing any record of intent, with no journal and no recovery -- **OPEN, NEVER EXERCISED**

**Severity:** HIGH, **unbounded**. The only finding in the wave-7 audit that can cost an arbitrary
amount, and the only one no reviewer has been able to drive.
**Status:** OPEN. Read from the code by the third independent auditor, who states plainly that they
could not force the trap on the local replica. **NOT REPRODUCED BY ANYBODY.** Recorded at full
severity anyway, because the shape is the textbook one and the recovery paths were deliberately
deleted.
**Where:** `deposit()` (`src/table_canister/src/lib.rs:1876`) and `claim_external_deposit()`
(`:2008`).

Both perform an irreversible ledger movement and credit `BALANCES` in the **post-await
continuation**. If that continuation traps -- instruction limit, memory, any panic in the tail --
the ledger movement stands and the credit is rolled back with the message. There is no journal
written before the call and no resume path afterwards.

There is also no manual repair:

* `notify_deposit` categorically refuses any block whose spender is the canister, with *"This block
  is an ICRC-2 pull performed by this canister on your behalf (the deposit() flow). It was credited
  to your balance when the pull happened and cannot be credited again"* -- a sentence that would be
  **false** in exactly this state;
* `admin_restore_balance` was deliberately removed (*"If balance recovery is needed, redeploy with a
  migration in post_upgrade"*), and removing it was right for its own reason (it could mint escrow
  from nothing) but it means the last in-band repair is gone.

**What settling it needs**, in the auditor's own words and repeated here because it is the next
thing somebody should build: a PocketIC harness that forces a trap in the post-await continuation
(instruction-limit exhaustion, or a fault-injecting ledger stub that returns `Ok` and then makes the
tail panic), and then asks whether ANY player- or controller-callable method can recover the funds.
Everything that needs is local; nothing about it needs mainnet.

The fix is the standard one and does not depend on the answer: write intent to stable state
**before** the ledger call, make the continuation idempotent against that record, and give the
record a resume path any principal can drive for their own entry.

<a id="finding-30"></a>
## FINDING 30 (high) -- the permanent hand archive is built from the seats as they stand at settlement, so it omits anyone who left mid-hand and invents anyone who sat down -- **OPEN**

**Severity:** HIGH. Not a fund loss. It falsifies the one durable artifact behind the product's
central claim.
**Status:** OPEN. Found by the third independent auditor, 2026-08-05, on four archived hands read
by hand from the running instance.
**Where:** `record_hand_to_history` (`src/table_canister/src/lib.rs:1157`).

The player list comes from `state.players`, not from `hand_stakes` -- the payout basis that
[FINDING 13](#finding-13) exists because of. The payout path was taught that a seat is not a person;
the archive path was not.

```text
archived hand_id 4 (table_2)
  players: cd-attacker at seat 0, position "SB", starting_chips 5_000_000_000,
           ending_chips 5_000_000_000
           -- he bought into that chair AFTER THE FLOP and was never dealt in
  alice, who was dealt in and lost 300_000_000 e8s: absent, and none of her actions recorded
  total_pot 900_000_000 against listed player deltas of +300_000_000 -- the record does not balance
```

**It breaks the shuffle specification's own instruction.** SHUFFLE-SPEC §4 says to derive the
player count from the hand's own record: *"Count it from the hand's own record -- every seat the
history shows with cards, plus any that folded."* Do that for archived hand 10 and you get P = 2
instead of 3, and you reproduce the board `9c Kc Qh / 6c / 4h` instead of the real
`Qh Ts 6c / 4h / 3s`. **For any hand somebody left, the published record is not sufficient to
verify that hand** -- which is the whole of "Provably Fair".

`get_hand_history` on the table canister has the identical omission, so the client shows the same
thing.

**The gate this owes**, and it is a property rather than an example: for every archived hand,
`sum(ending_chips - starting_chips)` reconciles against `total_pot`, and the recorded seat set
equals the dealt-in set the shuffle actually consumed.
