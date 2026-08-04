# ClearDeck defect register

**One prioritised list.** Wave 1 produced five independent bodies of work (the `poker_core`
extraction, a differential evaluator harness, a PocketIC money-safety harness, a screenshot
harness and a reference corpus) plus five adversarial critiques. Several of them found the
same bug from different directions. This file reconciles all of it into one register, so wave 2
has a single queue to work from.

- Anything with fund impact also has a full write-up in **[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)**.
  This file is the index and the priority order; that file is the evidence.
- Nothing here has been patched. Wave 1 deliberately froze engine behaviour so ground truth
  could be established first. **Do not fix a defect without a test that fails before the fix.**
- Reproduce anything with `make test` (fast) or the exact command on each entry.

## ID scheme

| prefix | domain |
|---|---|
| `E-` | engine / canister. Real money. |
| `T-` | build, deploy or configuration of the app itself. |
| `H-` | the harnesses. A harness defect means a result you cannot trust. |
| `D-` | documentation and evidence: a claim the artifact does not support. |

`Status` is one of **executed** (reproduced by running code), **code-read** (read but not run),
or **fixed-in-wave-1**.

---

## Priority queue

| # | sev | status | where | one line |
|---|---|---|---|---|
| [E-01](#e-01) | **critical** | executed | `determine_winners` / `advance_to_next_street` | every showdown after post-flop betting pays only the pre-flop pot; the rest is destroyed permanently |
| [E-02](#e-02) | **fund-theft** (latent, gated by E-04) | code-read | `periodic_cleanup`, `deposit`, `notify_deposit` | a real ledger transfer can be credited twice, which is withdrawable ICP created from nothing |
| [E-03](#e-03) | high | executed | `calculate_side_pots` → `poker_core::side_pots` | `state.pot` overrides the players' actual contributions in both directions: mints in one, destroys in the other |
| [E-04](#e-04) | high | executed | `notify_deposit` | can never credit a deposit (two independent decode bugs) and the ICP sent is stranded forever |
| [E-05](#e-05) | high | executed | `leave_table`, `cash_out`, `check_timeouts` | a seat vacated mid-hand orphans its stake, moving contested money into the deepest stack's exclusive pot. Three doors, one bug |
| [E-06](#e-06) | high | executed | `check_timeouts` + `count_players_can_act` | on `table_1`/`btc_table_1` the disconnect and action timeouts are both 30 s, so one lull runs the whole board out and settles |
| [E-07](#e-07) | high | executed | `admin_reinit_table` | the documented recovery tool strands every seated player's chips |
| [T-01](#t-01) | high | executed | `src/cleardeck_frontend`, root `.env` | a bare `npm run build` wires the bundle to the **MAINNET** fund-holding canisters |
| [H-01](#h-01) | high | executed | `tests/money_safety/src/wasms.rs:132-137` | the money-safety harness prefers a pre-existing wasm with no freshness check, so it is not a regression gate |
| [H-02](#h-02) | high | executed | `tests/money_safety/src/invariants.rs:682-696` | a `BUG:`/`CRITICAL:` log line from the canister can never fail a fuzz run |
| [H-03](#h-03) | high | executed | `tests/money_safety/src/invariants.rs` severity model | violations are classified by sign, so a live house rake passes as a "documented" finding |
| [E-08](#e-08) | medium | executed | `src/table_canister/table_canister.did` | the published Candid does not describe the deployed code (241 diff lines) |
| [E-09](#e-09) | medium | executed | `poker_core::{evaluate_hand, evaluate_five_cards}` | no input validation: duplicate cards, wrong card counts and short boards produce plausible impossible hands instead of trapping |
| [E-10](#e-10) | medium | code-read | `PENDING_WITHDRAWALS`, `LAST_WITHDRAWAL` | not in `PersistentState`, so the withdrawal cooldown resets on every upgrade |
| [E-11](#e-11) | medium | code-read | `withdraw` across an upgrade | an upgrade mid-withdraw drops the reply callback, so the refund branch can never run |
| [H-04](#h-04) | medium | executed | `src/table_canister/src/lib.rs` seam | 7 of 7 seam mutations survive with the whole suite green, including a constant-seed shuffle |
| [H-05](#h-05) | medium | executed | `tools/differential/src/checks/degenerate.rs` | the "what the references say" strings are hard-coded; the references were never called, and two are factually wrong |
| [H-09](#h-09) | medium | executed | `tools/shots` lobby scenes | a ~230 px loading band renders simultaneously with the loaded lobby; no scene gates on it |
| [H-11](#h-11) | medium | code-read | `tests/money_safety/src/table_api.rs:80-108` | the fuzzer only ever runs a 30 s action timeout at `table_1` stakes; `table_2`/`table_3` shapes are never exercised |
| [H-12](#h-12) | medium | n/a | all harnesses | nothing proves the RIGHT player won. There is no independent settlement oracle |
| [H-14](#h-14) | medium | executed | `docs/DESIGN-BAR.md` §1, bars 1/2/7/10/15 | the felt geometry is mis-measured, so three of four bars would reject correct work |
| [T-03](#t-03) | medium | executed | `src/lib/ic-config.js:21` | `LOCAL_HOST` hardcodes port 4943; this project's gateway is 8077 |
| [T-04](#t-04) | medium | executed | `.icp/cache/networks/local/state` | the local network state has no checkpoint height common to all five subnets, so it cannot be resumed |
| [T-07](#t-07) | medium | executed | local id mapping | `frontend` has never been created on the local network |
| [E-12](#e-12) | low | executed | `claim_external_deposit` | dust at or below the transfer fee in a deposit subaccount can never be swept |
| [E-13](#e-13) | low | executed | `poker_core::hand::detect_straight` | prefers the wheel over a better straight; unreachable today, a trap for the obvious optimisation |
| [E-14](#e-14) | low | code-read | `leave_table` | missing the `check_rate_limit()?` that `player_action` has |
| [E-15](#e-15) | low | executed | `poker_core::side_pots::level_pot` | the `partial_contributions` term is provably always zero: dead code on a fund path |
| [T-02](#t-02) | low | executed | "Verify Code" / table rows in the app | a locally-wired build still hands the user a mainnet `-e ic` command via a Copy button |
| [T-05](#t-05) | low | code-read | every deploy path | `btc_table_1` is never registered in the lobby |
| [T-06](#t-06) | low | executed | `canister_ids.json` vs `.icp/data/mappings/ic.ids.json` | two mainnet id lists that can drift |
| [H-06](#h-06) | low | executed | `src/table_canister/tests/money_safety.rs` | 2 of the 6 tests at the money-safety path do not test the canister |
| [H-10](#h-10) | low | executed | `tools/differential` | `cargo test --release -- --ignored` exits non-zero **by design**; it was published as a repro command |
| [H-13](#h-13) | low | executed | two `rng.rs` files | SplitMix64 is implemented twice, with different `below()` semantics |
| [H-15](#h-15) | low | executed | `tools/differential` | the third reference evaluator is OFF unless `CLEARDECK_PHE_PYTHON` is set |
| [H-07](#h-07) | — | fixed-in-wave-1 | `tools/shots/run.mjs:119` | an unverified scene wrote the canonical PNG filename |
| [H-08](#h-08) | — | fixed-in-wave-1 | `tools/shots/package.json` | `@dfinity/*` were undeclared dependencies |
| [D-01](#d-01) | — | executed | `tools/differential/README.md` | `rs_poker`'s lineage is OMPEval, not "its own tables", and 5.0.0 is two months old |
| [D-02](#d-02) | — | executed | wave-1 claim lists | several published numbers are unsupported; itemised below |

### What is NOT here

No demonstrated way for one player to take another player's escrow, and no demonstrated way to
withdraw more than deposited plus won. **E-02 is the one fund-theft-class entry and it is not
reachable today** — E-04 blocks the path. It becomes reachable the moment E-04 is fixed, which is
why the two must be fixed in the same change.

`state.pot` being over-collateralised (E-03 Direction A) would also create chips, but no
attacker-reachable way to force that direction has been demonstrated. E-05 forces the *other*
direction, which redistributes already-collected money rather than creating it.

---

## Engine

<a id="e-01"></a>
### E-01 — critical — every showdown destroys all post-flop money

**Where** `advance_to_next_street` (`PreFlop`→`Flop` transition) and `determine_winners`,
`src/table_canister/src/lib.rs`. Full write-up: [FINDING-01](FINDING-01-chip-destruction.md) and
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 01.

**What is wrong** `calculate_side_pots` is called unconditionally when the flop is dealt, which
freezes `state.side_pots` at the pre-flop total. Nothing recomputes it. At showdown the winner is
paid out of that frozen breakdown, so every chip wagered on the flop, turn and river is debited
from stacks and credited to nobody. The tokens stay inside the canister; `withdraw` pays only
against the caller's own escrow and there is no administrative withdrawal, so the loss is
**permanent and unrecoverable, including by a controller**.

Present at `HEAD`, byte-identical to `d461846`, so the mainnet tables are expected to contain it.
(Confirming that needs a mainnet module hash, deliberately not read under the local-only rule.)

**Found by** three independent routes, which is why this is first: a live local-replica hand
(FINDING 01), the money-safety harness's M1 conservation invariant, and the M1b payout-basis leg.

**Reproduce**
```
cd tests/money_safety && cargo test --test regressions -- reg01_a_showdown_with_post_flop_betting_destroys_the_post_flop_money --nocapture
```
Host-speed version with no replica: `cargo test -p table_canister --test money_safety -- m1b_pot_breakdown`

---

<a id="e-02"></a>
### E-02 — FUND-THEFT (latent, gated behind E-04) — a real transfer can be credited twice

**Where** `periodic_cleanup` and `deposit`, `src/table_canister/src/lib.rs`.
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 10.

**What is wrong** Two mechanisms, both of which turn one on-ledger transfer into two credits of
escrow, which is directly withdrawable ICP created from nothing:

1. `VERIFIED_DEPOSITS` is the only defence against crediting a deposit block twice, and
   `periodic_cleanup` bounds it by **forgetting the oldest block indices** once it exceeds
   10,000 entries. A forgotten block passes the already-processed check again.
2. `deposit()` (the ICRC-2 pull) never records the block index of the transfer it just made.
   That transfer's `from` is the caller's account and its `to` is the canister's account, which
   is exactly what `notify_deposit` verifies; `notify_deposit` ignores the `spender` field that
   would distinguish a `transfer_from`. So after any successful `deposit(a)`, calling
   `notify_deposit` on that block credits `a` a second time.

**Why it is not exploitable today** E-04 makes `notify_deposit` fail before it can credit
anything. That is a decode bug standing in for an access control, not a defence.

**Fix order** E-04 and E-02 must land in the **same change**. Fixing E-04 alone opens this.
Replace the pruning with a monotonic `min_unclaimable_block` watermark (bounds memory without
forgetting that a block was spent) and record the block index of every `deposit()`.

**Reproduce** The gate that will fail the moment it becomes possible already exists:
```
cd tests/money_safety && cargo test --test invariants -- m6_a_deposit_block_is_credited_at_most_once
```
It sweeps every block index after an ICRC-2 deposit.

---

<a id="e-03"></a>
### E-03 — high — `state.pot` overrides the players' actual contributions, in both directions

**Where** `calculate_side_pots` → `poker_core::side_pots::build_side_pots_logged`
(ported bug-for-bug from `ceacc37` lines 3521-3556).
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 02 and FINDING 09.

**What is wrong** After splitting the pot by bet level, the routine reconciles against
`state.pot` and never against the sum of the players' `total_bet_this_hand`. `state.pot` wins
unconditionally. Too high → the excess is appended to the highest bet level, i.e. chips appear
in the pot only the deepest stacks can win. Too low → every side pot is scaled down (through an
`f64` proportional-capping branch) and chips are destroyed.

**Reachability** Direction B is reached in ordinary play. During a 600-step randomised sequence
the canister printed, unprompted:
```
BUG: Side pots (400000000) exceed total pot (0). Capping to pot amount.
```
4 ICP of real contributions capped to a pot of zero, in a sequence of nothing but public update
calls, time jumps and upgrades. **The engine detects its own accounting inconsistency, logs it,
and settles anyway.** A detected inconsistency on a fund-custody path must trap.

Direction A's precondition is reachable via E-05.

**Merged from** FINDING 02 (mechanism, from the extraction wave) and FINDING 09 (reachability,
from the fuzzer). Same defect.

**Open** the minimal op sequence that drives `state.pot` to 0 while contributions remain has not
been isolated. That is the one outstanding piece of evidence on this entry.

---

<a id="e-04"></a>
### E-04 — high — `notify_deposit` can never credit, and the ICP it was sent is stranded forever

**Where** `notify_deposit`, `src/table_canister/src/lib.rs`.
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 06.

**What is wrong** Two independent bugs, both must be fixed:

* **BUG A** `response.candid::<(QueryBlocksResponse,)>()` — in `ic-cdk` 0.19 `candid::<R>()` is
  `decode_one::<R>()`, so this asks for one value of type `record { 0 : QueryBlocksResponse }`.
  The reply is one `QueryBlocksResponse`, a named record. It can never match.
* **BUG B** `AccountIdentifier` is declared as `record { hash : blob }`; the ledger's did says
  `type AccountIdentifier = blob`. Under Candid's `opt` rule the mismatched variant arm makes
  `operation : opt Operation` decode to **null** with no error, so with only BUG A fixed every
  legitimate transfer returns `"Transaction is not a transfer"`. BUG B's failure mode is silent,
  so fixing A alone replaces a clear error with a misleading one.

The ICP the user transferred stays in the canister and cannot be withdrawn by anyone, including
a controller — the same terminal state as E-01.

**Blast radius** not the frontend happy path (the app uses `claim_external_deposit`), but the
Candid interface still publishes `get_deposit_address()` + `notify_deposit(block_index)`, so any
wallet, script or integration following the published interface loses the money it sends.

**Reproduce**
```
cd tests/money_safety && cargo test --test regressions -- reg02_notify_deposit_cannot_decode_the_real_ledger_and_strands_the_money --nocapture
```
Runs against the real mainnet ICP ledger wasm (sha256 `a47a915e…`) installed at
`ryjl3-tyaaa-aaaaa-aaaba-cai`, the exact id the canister hardcodes. Not a mock.

---

<a id="e-05"></a>
### E-05 — high — a seat vacated mid-hand orphans its stake. Three doors, one bug

**Where** `leave_table` (`src/table_canister/src/lib.rs:2659-2687`), `cash_out`, and
`check_timeouts`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 05 and FINDING 08.

**What is wrong** Setting `state.players[seat] = None` mid-hand removes the seat from
`collect_contributions` while that player's money stays in `state.pot`. The side pots therefore
sum to less than `state.pot`, E-03 Direction A fires, and the orphaned amount is appended to the
**highest** bet level — the pot only the deepest stacks can win.

Executed, with the real pre-refactor code. Seat 0 all-in for 50, seats 1 and 2 in for 200 each,
`state.pot = 450`:
```
all seats present         pot[0] = 150  eligible [0,1,2]    pot[1] = 300  eligible [1,2]
seat 1 FOLDS, keeps seat  pot[0] = 150  eligible [0,2]      pot[1] = 300  eligible [2]   <- correct
seat 1 VACATES the seat   pot[0] = 100  eligible [0,2]      pot[1] = 350  eligible [2]
```
Nothing about the money changed between the last two lines, only whether the seat was vacated.
The honest all-in short stack's winnable main pot fell by 50 and the 50 moved into a pot only
seat 2 can win. Two principals under one operator make this a repeatable collusion edge.

**Three doors, and this is why 05 and 08 are one entry**
1. `leave_table` — a plain `#[ic_cdk::update]` with no phase guard, no turn requirement and no
   rate limit.
2. `cash_out` — its guard (`"Cannot cash out while in a hand"`) only refuses players who have
   **not** folded, so a folded player can vacate. FINDING 05 assumed this door was closed.
3. **A plain disconnect.** `check_timeouts` auto-folds a quiet player, after which door 2 opens.
   No deliberate call is needed.

**Reproduce**
```
cd tests/money_safety && cargo test --test regressions -- reg05_vacating_a_seat_mid_hand_orphans_the_leavers_stake reg08_a_timed_out_player_can_cash_out_mid_hand_and_orphan_their_stake --nocapture
cargo test -p table_canister --test money_safety -- m1b_vacating_a_seat_mid_hand
```

**Note** this is a **redistribution**, so M1..M6 conservation invariants are blind to it. Only
M1b's attribution leg sees the precondition. See H-12.

---

<a id="e-06"></a>
### E-06 — high — one action timeout ends the hand for everybody

**Where** `check_timeouts` + `count_players_can_act` + `advance_to_next_street`.
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 12.

**What is wrong** `DISCONNECT_TIMEOUT_NS` is a hardcoded 30 s local constant inside
`check_timeouts`, and it marks players `Disconnected` **before** looking at the action timer.
`count_players_can_act` requires `status == Active`. `table_1` and `btc_table_1` are deployed with
`action_timeout_secs = 30` (`icp.yaml`), so the two thresholds are **equal**: the moment anybody
times out, every player who has not heartbeated in 30 s is already `Disconnected`,
`count_players_can_act` drops below 2, and `run_out_board` deals the turn and the river and goes
straight to showdown in the same message — settling out of the stale pre-flop breakdown (E-01).

```
before   phase Flop, pot 206000000, 2 players live
advance 31s; check_timeouts() -> PlayerTimedOut(1)
after    phase HandComplete, 5 community cards (turn AND river dealt, nobody acted)
         200000000 e8s destroyed by the forced settlement
```

`table_2` (45 s) and `table_3` (60 s) have headroom. `table_1` and `btc_table_1` have none. The
defect is that representing a dropped connection also silently ends the hand for players who are
perfectly well connected.

**Reproduce**
```
cd tests/money_safety && cargo test --test regressions -- reg09_one_timeout_with_no_heartbeats_runs_the_whole_board_out_and_settles --nocapture
```

---

<a id="e-07"></a>
### E-07 — high — `admin_reinit_table` strands every seated player's chips

**Where** `admin_reinit_table` → `init_table_state`.
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 07.

**What is wrong** The function documented as "for recovery after upgrade issues" builds a brand
new `TableState` with empty seats. Every seated player's chips and the whole pot are dropped:
not returned to escrow, not withdrawable, gone.
```
before   escrow 800000000   chips 400000000   ledger_main 1200000000
after    escrow 800000000   chips         0   ledger_main 1200000000
M1       +400000000 chips DESTROYED
```
Controller-only, so not an attacker path, but it means the recovery tool is the thing that loses
the funds. Needs an owner decision: return chips to escrow first, or refuse while anyone is
seated.

**Reproduce** `cargo test --test regressions -- reg06_admin_reinit_table_strands_every_seated_players_chips`

---

<a id="e-08"></a>
### E-08 — medium — the published Candid does not describe the deployed code

**Where** `src/table_canister/table_canister.did`, referenced by all four table canisters in
`icp.yaml`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 03. Pre-existing.

241 diff lines. Substantively: `Player` is missing `sitting_out_since : opt nat64`;
`ActionRecord` is missing `phase : text` and `amount : nat64`; and the `Result_N` aliases are
numbered differently, so `Result_1`..`Result_5` in the committed file denote different types than
the code's — a generated client will mis-decode several method results.

**Reproduce**
```
make wasm
candid-extractor target/wasm32-unknown-unknown/release/table_canister.wasm > /tmp/real.did
diff src/table_canister/table_canister.did /tmp/real.did
```
The file is hand-maintained, which is why it drifted. It should be generated in CI with the build
failing on any diff — but regenerating also reformats it and drops its hand-written comments, so
that needs a deliberate decision, not a drive-by.

---

<a id="e-09"></a>
### E-09 — medium — `poker_core` validates no input at all

**Where** `poker_core::evaluate_hand` and `poker_core::evaluate_five_cards`.

**What is wrong** Neither function checks card count or distinctness. Four manifestations, all
the same missing boundary check, all confirmed by direct call:

| input | returns | should |
|---|---|---|
| `Ah Ah Kh Qh Jh 2c 3d` (6 distinct physical cards) | `Flush([14,14,13,12,11])` — a five-card flush built from **four** cards | refuse |
| `Ah Ah Ah Ah Ah` | `Flush([14,14,14,14,14])` | refuse |
| `2h 3h 4h 5h 9h 6c 7c` into `evaluate_five_cards` (7 cards) | `StraightFlush(7)` — no such hand exists in those cards | refuse |
| `Ah Kd` (2 cards) into `evaluate_hand` | `HighCard([])`, which compares **equal** between players and less than every real hand | refuse |

**Why it matters** A dealing or shuffling defect is laundered into a plausible-looking winning
hand and the pot is awarded, instead of trapping before any money moves. And per H-05, **no
reference evaluator in the differential harness validates its own input either**, so the fix wave
cannot treat "a reference would have caught it" as a safety net. The validation has to be at the
`poker_core` boundary, loudly, before the pot is awarded.

**Reproduce** These four are pinned as `#[ignore]`d markers that FAIL until fixed:
```
make known-defects
```

**Merged from** four of the five differential "latent" findings.

---

<a id="e-10"></a>
### E-10 — medium — the withdrawal cooldown resets on every upgrade

`PENDING_WITHDRAWALS` and `LAST_WITHDRAWAL` are not fields of `PersistentState`, so neither
survives an upgrade. Not a fund-loss path on its own (the balance is debited before the transfer),
but a rate limit that an operator action clears is not a rate limit. Code-read; not executed.

<a id="e-11"></a>
### E-11 — medium — an upgrade mid-withdraw drops the refund path

An upgrade that lands while a `withdraw` is awaiting the ledger drops the reply callback along
with the heap, so the `Err` branch that refunds escrow can never run. If the transfer had failed,
the user's escrow stays debited with nothing delivered. **Not claimed as demonstrated:** the
harness cannot yet hold a ledger reply open across an `install_code`. Building that capability is
the test wave-2 needs here.

<a id="e-12"></a>
### E-12 — low — dust at or below the fee is stranded in a deposit subaccount

`claim_external_deposit` refuses when `balance <= transfer_fee` and nothing else can sweep a
subaccount, so anything at or below 0.0001 ICP sent to a deposit address is stuck. Bounded by the
fee. The error message ("send ICP to your deposit address first") is misleading when they already
have.

<a id="e-13"></a>
### E-13 — low — `detect_straight` prefers the wheel over a better straight

`detect_straight(&[14,6,5,4,3,2]) == Some(5)`, should be `Some(6)`. The A-2-3-4-5 check runs before
the descending window scan. Unreachable today: `evaluate_five_cards` is the only caller and always
passes exactly five ranks, and five distinct ranks cannot contain both a wheel and a higher
straight. The trap is that passing all seven cards is an obvious-looking optimisation and would
silently misrank. **Found twice independently** — FINDING 04 (extraction wave) and the
differential harness — which is the same defect, not two.

<a id="e-14"></a>
### E-14 — low — `leave_table` has no rate limit

Missing the `check_rate_limit()?` that `player_action` has, on a function that (per E-05) has fund
consequences. Code-read.

<a id="e-15"></a>
### E-15 — low — dead code on a fund path

`poker_core::side_pots::level_pot`'s `partial_contributions` term is **provably always zero**:
every bet value is itself in the deduped `bet_levels` list, so no bet can lie strictly between two
consecutive levels. Found by mutation testing (deleting the term leaves the suite green, and the
mutant was then proved equivalent). Dead arithmetic in the side-pot split invites a future reader
to trust it.

---

## Build, deploy and configuration

<a id="t-01"></a>
### T-01 — high — a bare `npm run build` wires the bundle to MAINNET

**Where** `src/cleardeck_frontend/src/lib/canisters.js` reads
`VITE_CANISTER_ID_LOBBY || CANISTER_ID_LOBBY`, `vite.config.js` dotenv-loads the repo-root `.env`,
and that `.env` holds the **mainnet** ids:
```
CANISTER_ID_LOBBY='kpfcd-kyaaa-aaaaj-qor3a-cai'
CANISTER_ID_HISTORY='kggj7-4qaaa-aaaaj-qor2q-cai'
```
So a developer who builds and serves the app locally is pointing a dev UI at the live canisters
that custody real ICP and ckBTC, with no warning anywhere in the output.

**Mitigation that exists** `tools/shots/lib/frontend-build.mjs` sets the local ids explicitly and
then **greps the built bundle** for the local lobby id, aborting if it is absent.
`make local-up` delegates to that function, so there is exactly one correct way to build.

**Wave 2** make the failure mode loud in the app itself: refuse to boot, or show a permanent
banner, when the bundle's ids are mainnet ids and the origin is not a mainnet hostname.

<a id="t-02"></a>
### T-02 — low — a locally-wired build still offers a mainnet `-e ic` command

Even with local wiring, the built bundle contains seven mainnet id occurrences across five ids,
including two `navigator.clipboard.writeText("icp canister status qrhly-eaaaa-aaaaj-qousa-cai -e ic")`
payloads and a "Tables (all)" row listing `kieex`/`lfkaz`/`lclgn`. A Copy button that hands a user
a mainnet command from a local dev build is a footgun; the ids should come from the same runtime
config as the wiring.

<a id="t-03"></a>
### T-03 — medium — the app hardcodes gateway port 4943

`src/lib/ic-config.js:21` — `export const LOCAL_HOST = 'http://127.0.0.1:4943'`, and `auth.js`
hardcodes the same port for the local Internet Identity origin. This project's gateway is pinned
to **8077** in `icp.yaml` (because 8000 belongs to another project on this machine). The
screenshot harness works around it with a transparent 4943→8077 reverse proxy, which is a shim,
not a fix. Make the port configurable at build time from the same place the canister ids come
from.

<a id="t-04"></a>
### T-04 — medium — the local network state cannot be resumed

`make doctor` reads the checkpoint directories and reports:
```
4f77f65704ed: 7ef4, 7fe7
52130e56a053: 7f58
52c17c301666: 7ef4
682936da41e0: 7ef4
cd735df1522b: 7f58, 7fee
```
There is **no height present on all five subnets** (7ef4 on three, 7f58 on the other two), which
is exactly the precondition for the launcher's "The state of subnet … is incomplete" refusal.
Resuming will cost the local ICP ledger, the five funded identities and every deployed local
canister.

`make local-up` refuses to start in this condition and requires an explicit `--reset` (which
deletes the state) rather than discovering the problem as a panic. **The replica has been down for
this entire wave**, which is why zero screenshots exist.

<a id="t-05"></a>
### T-05 — low — `btc_table_1` is never registered in the lobby

`scripts/deploy-mainnet.sh` calls `init_microstakes_tables(table_1, table_2, table_3)` and carries
a `# NOTE: verify whether btc_table_1 needs a separate lobby registration call.` `make local-up`
now states this explicitly instead of leaving it as a comment.

<a id="t-06"></a>
### T-06 — low — two lists of mainnet canister ids

`canister_ids.json` (legacy dfx shape) and `.icp/data/mappings/ic.ids.json` (what icp-cli reads)
both list all seven mainnet canisters. They agree today. `ic.ids.json` is authoritative — it is
what `-e ic` resolves and what the mainnet guards in `scripts/dev.sh` and
`tools/shots/lib/ids.mjs` now read. `canister_ids.json` should be deleted or generated.

<a id="t-07"></a>
### T-07 — medium — the frontend asset canister has never been created locally

`frontend` is absent from `.icp/cache/mappings/local.ids.json`, so `node tools/shots/run.mjs
--skip-deploy` throws today. `make local-up` creates it.

---

## Harnesses

A harness defect is not a cosmetic problem. It is the difference between a green run that means
something and a green run that means nothing.

<a id="h-01"></a>
### H-01 — high — the money-safety harness is not a regression gate

**Where** `tests/money_safety/src/wasms.rs:132-137`.

```rust
let shared = repo_root().join("target/wasm32-unknown-unknown/release").join("table_canister.wasm");
if shared.exists() {
    return std::fs::read(&shared).expect("table_canister.wasm unreadable");
}
```
Resolution step 2 returns whatever wasm is lying at that path, with **no freshness, mtime or hash
check** — contradicting the module's own doc comment ("must be built from the checked-out source,
so the harness tests the code in the tree and not a stale artifact"). Proven by the critic: with a
10x-credit fund-theft bug planted in `lib.rs` and a pristine wasm at that path, all 10 invariant
tests report `ok`; deleting that one file makes the identical source go 7/10 red.

**Partial mitigation, in place now** `scripts/dev.sh` builds the wasm, exports
`CLEARDECK_TABLE_WASM` (resolution step **1**, which wins), and prints the sha256 as part of every
`make test` / `make fuzz` run:
```
    wasm under test: e571570fc5ea3f5e9cb27d938d4ec233b05b22e8973291c0282c808aea84f386
```
That makes the documented path a real gate. It does **not** fix a bare
`cd tests/money_safety && cargo test`.

**Wave 2** delete resolution step 2, or always build and hard-fail if the built wasm's sha256
differs from what is installed; and print the sha256 as the first line of every run so no result
can be read without knowing what produced it.

<a id="h-02"></a>
### H-02 — high — a canister that reports its own inconsistency cannot fail a fuzz run

**Where** `check_self_reported_inconsistency`, `tests/money_safety/src/invariants.rs:682-696`;
`Severity::is_documented_defect`, same file `:104-106`; `tests/fuzz.rs:149`.

Every log line matching `BUG:`/`CRITICAL:` is hardcoded to `Severity::BreakdownDrift`;
`is_documented_defect()` returns true for `BreakdownDrift`; and `tests/fuzz.rs` only asserts that
`reproducers` (built from *non*-documented severities) is empty. So a self-reported inconsistency
is structurally incapable of failing a run — which directly undercuts the claim that "the harness
now treats any `BUG:`/`CRITICAL:` log line as an invariant violation in its own right".

Worse, the same branch swallows `pre_upgrade`'s `CRITICAL: Failed to save state to stable memory`
— a **total-fund-loss-on-upgrade** event — into the same non-blocking bucket. Reproduced: the log
line appears and the run reports `test result: ok`.

**Wave 2** split the pattern. `CRITICAL:` and any `BUG:` line the register does not name must be
blocking. Only the specific, enumerated E-03 warning may be documented, and only until E-03 is
fixed.

<a id="h-03"></a>
### H-03 — high — violations are classified by sign, so a house rake passes

**Where** `Severity::is_documented_defect`, `tests/money_safety/src/invariants.rs:104-106`:
`matches!(self, Severity::FundDestruction | Severity::BreakdownDrift)`.

The classifier decides blocking-vs-documented by the **direction** of the money delta, not by
cause. The critic planted a 1% house rake in `determine_winners` — an operator skimming every pot
— and `cargo test --test fuzz` returned `test result: ok. 1 passed; 0 failed`, recording it only
as `documented M1_CONSERVATION:…FundDestruction x143 worst_delta=40000`. Dropping a side pot
passes the same way.

So **"zero blocking findings" is fully compatible with a live rake and with silent pot
destruction.** It rules out the negative-delta direction only. The no-rake property — one of the
four things this project promises users — is not currently gated by anything except the one
hand-written `m3_no_rake_holds_without_post_flop_money_and_fails_with_it` test.

**Wave 2** any unexplained non-zero delta must block. Documented status must be keyed to a named
register entry with an expected magnitude, not to a sign.

<a id="h-04"></a>
### H-04 — medium — the seam is not mutation-tested

The `poker_core` extraction was mutation-tested (23 of 26 `poker_core` mutants convict; the three
survivors were each proved equivalent). The **seam the extraction wrote** was not. Seven of seven
seam mutations survive with the whole suite green:

1. `calculate_side_pots` made a no-op with `if true { return; }`
2. `state.pot / 2` passed as the pot
3. the pots written into a throwaway `Vec`
4. `&[]` passed for the contributions
5. the `BUG: Side pots` warnings dropped
6. `evaluate_hand` shadowed by a stub returning `RoyalFlush` for every player
7. `shuffle_deck` shadowed so every hand is dealt from the constant seed `b"CONSTANT"`

**The canister can stop using the extracted engine entirely, or deal every hand from a fixed deck,
and the suite still reports green.** `src/table_canister/tests/integration_test.rs` is comments
only.

**Wave 2** minimum bar, all three at canister level driving the real state machine:
(a) seat players, run pre-flop **and** post-flop betting to showdown, assert paid-out chips equal
money collected — this alone catches E-01; (b) assert `state.side_pots` sums to `state.pot` after
`calculate_side_pots`; (c) assert the E-05 fold-but-stay-seated vs vacate split is identical.
A `pocket-ic` v11.0.0 binary is already available at `$(dfx cache show)/pocket-ic`.

<a id="h-05"></a>
### H-05 — medium — the differential harness fabricates its oracle for degenerate inputs

**Where** `tools/differential/src/checks/degenerate.rs:117, :174, :211`.

The `reference_says` field is a **hard-coded string literal**. The references are never called.
Reproduce by running `make diff-full` and reading the output — these exact strings appear:
```
ref_says=[both references reject a hand containing the same card twice]
ref_says=[6 distinct physical cards were supplied; both references reject the input outright]
ref_says=[no reference will evaluate fewer than five cards; ...]
```
All three are wrong about reference A. Measured by direct call in both release and debug builds,
`rs_poker` silently ranks `Ah Ah Ah Ah Ah` as `StraightFlush(0)`, `Kh Kh Kd Kd Qs` as
`FourOfAKind(155)`, `Ah Ah Kh Qh Jh 2c 3d` as `StraightFlush(0)` and the two-card hand `Ah Kd` as
`HighCard(2140)`. `phevaluator` returns the out-of-range sentinel `0` for duplicates rather than
raising. Only `poker` 0.7.0 errors. Five of the twelve degenerate findings are pushed
unconditionally with no reference call at all, and eight carry a hard-coded reference claim.

**The engine defects themselves (E-09) are real and correctly described.** It is the differential
evidence attached to them that is fabricated.

**Wave 2** call both references and report the measured result; emit a finding only when there is
something to report. And note the corrected conclusion: **no evaluator in this harness validates
its own input**, so E-09 must be fixed at the `poker_core` boundary and not delegated.

<a id="h-06"></a>
### H-06 — low — two of the six host-level money-safety tests do not test the canister

`the_ledger_anchored_invariants_are_not_checked_here` only asserts a `Cargo.toml` exists, and
`a_consistent_table_satisfies_every_host_checkable_invariant` checks the oracle against itself.
When the critic broke the real `collect_contributions`, only 1 of the 6 went red.

<a id="h-07"></a>
### H-07 — FIXED IN WAVE 1 — an unverified scene wrote the canonical PNG filename

`tools/shots/run.mjs` called `shoot()` unconditionally after `scene.verify()`. Only a *thrown*
exception diverted to `FAILED-*.png`; a soft `verified:false` still wrote
`table-showdown-desktop.png` into `artifacts/screens/<sha>/` **and** mirrored it into
`artifacts/screens/latest/`, where anyone browsing PNGs reads it as proof of a state that was
never asserted. `table-allin` and `table-preflop` both stabilise against a live 45 s/60 s action
clock, so this was the realistic failure path, not a hypothetical.

Fixed here: a verified scene gets the canonical name; anything else is written as
`UNVERIFIED-<scene>-<viewport>.png` so the filename carries the caveat even when nobody opens
`INDEX.md`.

<a id="h-08"></a>
### H-08 — FIXED IN WAVE 1 — undeclared dependencies in the screenshot harness

`tools/shots/package.json` declared only `playwright`; every `@dfinity/agent`, `@dfinity/identity`
and `@dfinity/principal` import resolved by Node walking **up** to the repo-root `node_modules`.
An `npm ci` inside `tools/shots`, or a pruned root, broke all 20 modules. Fixed: the three
packages are declared at `^3.4.3` (matching the frontend) and installed locally.

<a id="h-09"></a>
### H-09 — medium — an unguarded layout race in the scenes being photographed

At t=1 s the lobby renders the `Loading tables…` spinner block **and** the populated/empty lobby
simultaneously; by t=3 s the spinner is gone. That block is ~230 px of vertical layout, so whether
it appears in a given PNG is a coin flip on timing. `FREEZE_CSS` kills animation, not this state
overlap. No scene waits for `.loading-state` to clear.

**Wave 2** every scene should assert `.loading-state` is absent before shooting.

<a id="h-10"></a>
### H-10 — low — a published repro command that exits non-zero by design

`cd tools/differential && cargo test --release -- --ignored` is 1 passed / 5 **failed**. The five
failures are the E-09 and E-13 markers, each with `un-ignore when the fix wave lands` in its
reason string: honest in the code, misleading in a claim list. `make known-defects` now inverts
them properly — it succeeds while the defects are present, distinguishes "ran 0 tests" from
"fixed", and shouts when one goes green.

<a id="h-11"></a>
### H-11 — medium — the fuzzer never exercises the deployed `table_2` / `table_3` shapes

`tests/money_safety/src/table_api.rs:80-108`: `six_max_icp()` uses **`table_1`'s** blinds
(0.01/0.02) with `max_players: 6`, and `heads_up_icp()`/`six_max_with_ante()` both derive from it.
So every fuzz run uses `action_timeout_secs: 30`. That happens to be the E-06 danger value, but it
means the harness structurally cannot see the 45 s/60 s headroom that distinguishes `table_2` and
`table_3`, nor their 0.05/0.10 and 0.10/0.20 blind levels, nor 9 seats. Three copies of the table
config now exist (`icp.yaml`, `tools/shots/lib/config.mjs`, `table_api.rs`) and only `icp.yaml` is
authoritative.

<a id="h-12"></a>
### H-12 — medium — nothing proves the right player won

The most important gap in wave 1, stated plainly. M1..M6 are conservation and settlement
properties: they are blind to a pure **redistribution** between players. If the engine pays the
wrong player the total is unchanged and every invariant still holds. That is exactly the shape of
E-05, and the only reason the harness sees it at all is M1b's attribution leg flagging the
precondition rather than the misallocation.

The differential harness proves the **evaluator** ranks hands correctly (2,598,960 five-card hands
against three lineages, zero disagreements). Nothing connects that to the **payout**: which seats
are eligible for which pot, how ties chop, how odd chips are assigned.

**Wave 2** an independent settlement oracle: given hole cards, board and contributions, compute
what each seat is owed, and compare against what the engine actually paid. This is the single
highest-value harness that does not exist.

<a id="h-13"></a>
### H-13 — low — SplitMix64 implemented twice

`tools/differential/src/rng.rs` (`SplitMix64`) and `tests/money_safety/src/rng.rs` (`Rng`) share
the same core constants but differ in `below()`: rejection sampling versus raw modulo, and panic
versus 0 on `n == 0`. Not consolidated in this pass on purpose — unifying `below()` would change
the money-safety fuzzer's stream and invalidate every recorded seed. If they are merged, both
variants must be kept under distinct names so existing seeds still reproduce.

<a id="h-14"></a>
### H-14 — medium — `docs/DESIGN-BAR.md` would reject correct work

The felt geometry in §1 is mis-measured, because the measurer's bounding box caught the `.table`
DIV (rail plus the pod row above it) rather than the green ellipse. Independently re-measured with
a hue-mask + largest-connected-component method, gated on an ellipse-area sanity check:

| client | document says | measured | check |
|---|---|---|---|
| PokerNow | 912×570, aspect 1.60 | **944×472, aspect 2.00** | blob is 96.5% of π·a·b (82.7% against the claimed box — impossible for a solid ellipse) |
| PokerStars | aspect ~1.40 | **547×250, aspect ≥2.19** | |
| GGPoker | aspect ~1.10-1.32 | **678×312, aspect 2.17** | the source figure claimed a felt filling 99.8% of the window height |
| WPT Global | aspect 1.80-2.27 | **833×400, aspect 2.08** | |

So the headline finding ("felt aspect ratio splits the field") is backwards: **every leader is a
~2:1 stadium.** As written, bar 1 rejects 3 of 4 reference clients and bar 2 rejects 4 of 4.
Also: the palette percentages are 8-colour quantisations of whole image files, and for three
third-party captures 16-21% of the file is the review site's white page gutter, not the client.
Bar 10 ("two typefaces total") contradicts the CSS it cites, which loads five families including a
dedicated `Suits` icon webfont — so the bar tells a builder to render suits as unicode, precisely
what PokerNow chose not to do. Bar 15 ("three of four leaders show live all-in equity") is
evidenced for two, and §8 of the same document disclaims WPT's all-in reference.

**Wave 2** do not build the ClearDeck-vs-bar scorecard until §1 rows 3-5 and bars 1/2/7/10/15 are
re-derived with a validated measurer. Measuring ClearDeck against inverted bars is worse than not
measuring it. Also move the capture scripts into the corpus and index them: the only numbers the
document calls "exact" came from ad-hoc scripts that were never shipped, which is why nobody
caught the felt error.

<a id="h-15"></a>
### H-15 — low — the third reference evaluator is off by default

`cargo run --release` in `tools/differential` prints `# auditing the two references against
phevaluator …` and then `NOTE adjudicator skipped: CLEARDECK_PHE_PYTHON not set`. The headline
"three independently-implemented mature evaluators" needs that variable. `make phe-venv` now
installs it (verified working on Python 3.14.3) and `make diff-full` sets it, so the default path
runs all three.

---

## Documentation and evidence

<a id="d-01"></a>
### D-01 — `rs_poker`'s provenance is overstated

`tools/differential/README.md` calls reference A's evaluator "its own perfect-hash tables,
generated from scratch in the crate's `build.rs`". That `build.rs` says, line 4: *"The algorithm is
reimplemented from zekyll's OMPEval (MIT)."* The lineage is OMPEval. It is still genuinely
independent of the Cactus-Kev/treys lineage that reference B ports, so the independence argument
survives — but 5.0.0 was published 2026-06-09, so the crate's 88k lifetime downloads belong to nine
years of history, not to these two-month-old tables. "Mature, widely used" is doing more work than
the evidence supports for reference A specifically. `phevaluator` corroborates, which is why H-15
matters.

<a id="d-02"></a>
### D-02 — unsupported numbers in the wave-1 claim lists

Recorded so they are not carried forward as fact:

* "the shrinker reduced a 400-op failing sequence to 5 and 12 ops" — the builder's own quoted
  output says `400 -> 10` and `-> 5`, and the run is in no surviving artifact. The shrinker does
  work (independently verified: 120 ops → 5 across 9 signatures); the 400/12 figures do not exist.
* "all other workspace targets: 23, 28, 3, 40, 0, 0 passed" — `table_canister`'s own
  `tests/unit_tests.rs` runs **7** tests, reported as 0. The workspace total is **107 passed, 0
  failed, 0 ignored**, verified by `make test`.
* "258 of 438 corpus files are real gameplay" — one is a `meta.json`, 34 more are lobbies,
  cashiers, registration and HUD overlays, 200 are 700 ms frames of a single session, and 23 files
  are exact byte duplicates. Real-gameplay **table** captures from a client other than PokerNow: 8
  files, 4 of which were found contaminated or mislabelled.
* "the only mainnet id left in the bundle is display text inside one modal" — there are seven
  occurrences across five ids in three different places. See T-02.
* "the replica's on-disk state is very likely NOT resumable" was published as an unverified panic.
  The read-only precondition is now confirmed (T-04): no checkpoint height is common to all five
  subnets. The panic itself is still not reproduced, and does not need to be.
* "3 of 26 poker_core mutants survive" was published as 1 survivor. All three were subsequently
  proved equivalent, so the suite is *stronger* than was demonstrated — but the survivor list that
  was published was not the real one, and one of the survivors is E-15.

---

## Fix ordering

1. **E-04 + E-02 together.** Never E-04 alone: it opens a chips-from-nothing path.
2. **E-01**, with the H-04(a) conservation test written first so the fix is proven.
3. **E-03 + E-05 together.** Compare against `total_bet_this_hand` and trap on mismatch, and stop
   letting a vacated seat erase its stake. Either fix alone leaves the other door open.
4. **H-01, H-02, H-03** before trusting any further green run — otherwise the fixes above are
   being validated by a suite that cannot fail.
5. **E-06**, then **E-07**, then **E-09**.
