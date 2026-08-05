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
| [E-01](#e-01) | **critical** | **FIXED-IN-WAVE-2** (with E-03 and E-05) | `determine_winners` / `advance_to_next_street` | every showdown after post-flop betting paid only the pre-flop pot and destroyed the rest permanently; the payout basis is now rebuilt from the players' contributions at payout time |
| [E-02](#e-02) | **fund-theft** | **FIXED-IN-WAVE-2** (demonstrated as theft first) | `periodic_cleanup`, `deposit`, `notify_deposit`, `VERIFIED_DEPOSITS` | one real ledger transfer credited twice and the excess WITHDRAWN as real ICP; closed by a monotonic watermark plus a bounded record with one writer |
| [E-37](#e-37) | high | **FIXED** (demonstrated first, then fixed) | `Stake`, `plan_payouts`, `apply_payouts` (`principal_of` deleted) | the E-05 fix paid a departed player's refunded stake to whoever took their chair. Composed end to end on the real canister: `-2000000` from the player who left, `+2000000` to the stranger in her chair, every total balancing. The owner now travels with the stake and `M8_PRINCIPAL_ATTRIBUTION` + a PRINCIPAL column in the settlement oracle gate it. [FINDING 13](SECURITY-FINDINGS.md) |
| [E-38](#e-38) | high | **FIXED** (both fields, in one change) | `PersistentState::deposit_watermark`, `TableState::departed_stakes` | two agents each added a non-`opt` persisted field. As shipped an upgrade was REJECTED; fixing only the first makes the upgrade SILENTLY destroy every chip at the table (re-measured on the wave-3 fixture: 594000000 seated e8s and three players' hole cards, gone, upgrade reported successful). Both are now `opt`; M7 upgrades `801aa79` state into the current wasm with every e8, stack, card and the anti-replay record intact, and `pre_upgrade` traps rather than proceeding after a failed save. [FINDING 14](SECURITY-FINDINGS.md) |
| [T-08](#t-08) | **high** | **FIXED-IN-WAVE-3** | `PokerTable.svelte` pot header | the table's headline POT was displayed at **2×** during every betting round, and disagreed with the pot-odds strip on the same screen. The headline is now `get_pot()` unmodified, the two legs shown beside it are a decomposition that sums back to it, and the pot-odds strip spells out the same figure. Gated by the screenshot harness on every table scene |
| [T-09](#t-09) | medium | **FIXED-IN-WAVE-3** | `PokerTable.svelte` showdown | the villain's revealed hand rendered as two blank cards and the winning hand as `0`: an unwrapped Candid `opt` in two places. Both are unwrapped now (`revealedHole()` and `handRankWords()`), and a revealed pair is lifted clear of its own plate so it can be read |
| [H-16](#h-16) | high | **FIXED-IN-COHERENCE-PASS** | `tests/money_safety/src/world.rs`, `wasms.rs` | `World::upgrade` reused `self.table_wasm`, so every "survives an upgrade" assertion was new-wasm-to-itself. `upgrade_to_module_under_test` + `previous_release_table_canister()` + M7 now walk `801aa79` state into the current wasm |
| [H-17](#h-17) | high | **FIXED-IN-COHERENCE-PASS** | `scripts/dev.sh` `cmd_test` | `tests/deposit_replay.rs` -- the E-02 fund-theft reproducer and its ten regressions -- was named by no make target for the whole wave |
| [H-18](#h-18) | high | executed | `deposit_replay.rs` `dr02` | the named regression on the fund-theft primitive never presents the deposit block: the rate limiter skips it and the loop does not retry |
| [H-19](#h-19) | medium | executed | `tests/money_safety/src/documented.rs` | `register_entries_are_all_still_needed`, documented as the thing that stops a stale tolerance surviving a fix, does not exist |
| [H-20](#h-20) | medium | **FIXED-IN-COHERENCE-PASS** | `invariants/relational.rs` + `documented.rs` | the payout fix moved a live defect's self-report from `CRITICAL:` to `WARNING:`, which the detector does not match; 296 of them went unreported in one fuzz run |
| [H-21](#h-21) | low | executed | `tests/money_safety/src/wasms.rs` | the harness's `cargo build` inherits `RUSTUP_TOOLCHAIN`, which overrides `rust-toolchain.toml`; a different toolchain produced a different module hash from identical source |
| [H-22](#h-22) | medium | executed | `tests/settlement` re-deal search | the exact-deal search has no attempt cap, so a deck that stops varying HANGS the suite instead of failing it; contained by a timeout in `cmd_test`, not fixed |
| [E-39](#e-39) | medium | **FIXED-IN-COHERENCE-PASS** | `leave_table` | reduced `state.pot` via `return_uncalled_bet` without the paired `refresh_side_pots`, so the side pots a player is SHOWN stopped summing to the pot. Found by `make fuzz` at its DEFAULT 9 seeds; the wave that caused it ran 3 |
| [H-23](#h-23) | **high** | executed | `.github/workflows/ci.yml` | CI runs neither the money-safety suite nor the settlement oracle. Every fund-safety result in these documents comes from a harness no CI job invokes |
| [E-03](#e-03) | high | **FIXED-IN-WAVE-2** (with E-01 and E-05) | `calculate_side_pots` → `poker_core::side_pots` | `state.pot` overrode the players' actual contributions in both directions, minting in one and destroying in the other through an `f64` ratio; it can no longer move a chip |
| [E-04](#e-04) | high | **FIXED-IN-WAVE-2** (with E-02, in that order) | `notify_deposit` | could never credit a deposit (two independent decode bugs) and the ICP sent was stranded forever |
| [E-05](#e-05) | high | **FIXED-IN-WAVE-2** (with E-01 and E-03) | `leave_table`, `cash_out`, `check_timeouts` | a seat vacated mid-hand orphaned its stake, moving contested money into the deepest stack's exclusive pot; the stake is now recorded independently of seat occupancy |
| [E-06](#e-06) | high | executed | `check_timeouts` + `count_players_can_act` | on `table_1`/`btc_table_1` the disconnect and action timeouts are both 30 s, so one lull runs the whole board out and settles |
| [E-07](#e-07) | high | executed | `admin_reinit_table` | the documented recovery tool strands every seated player's chips |
| [T-01](#t-01) | — | **FIXED-IN-WAVE-2** | `src/cleardeck_frontend` build, root `.env` | the build now refuses to run without an explicit target network, loads the repo-root `.env` only for `-e ic`, and aborts if a local build resolves a mainnet id |
| [H-01](#h-01) | — | **FIXED-IN-WAVE-2** | `tests/money_safety/src/wasms.rs` | the stale-artifact path is gone: the harness always builds, prints the sha256 as the first line of every run, and verifies the installed module hash |
| [H-02](#h-02) | — | **FIXED-IN-WAVE-2** | `tests/money_safety/src/invariants.rs` | `CRITICAL:` and any unenumerated `BUG:` line is now `SelfReportedFailure`, which nothing can excuse |
| [H-03](#h-03) | — | **FIXED-IN-WAVE-2** | `tests/money_safety/src/documented.rs` | tolerance is now a NAMED register keyed on (invariant, check, direction, magnitude); a rake is caught by `awarded_equals_payout_basis` |
| [E-08](#e-08) | medium | executed | `src/table_canister/table_canister.did` | the published Candid does not describe the deployed code (241 diff lines) |
| [E-09](#e-09) | medium | executed | `poker_core::{evaluate_hand, evaluate_five_cards}` | no input validation: duplicate cards, wrong card counts and short boards produce plausible impossible hands instead of trapping |
| [E-10](#e-10) | medium | code-read | `PENDING_WITHDRAWALS`, `LAST_WITHDRAWAL` | not in `PersistentState`, so the withdrawal cooldown resets on every upgrade |
| [E-11](#e-11) | medium | code-read | `withdraw` across an upgrade | an upgrade mid-withdraw drops the reply callback, so the refund branch can never run |
| [H-04](#h-04) | low | **MOSTLY-FIXED-IN-WAVE-2** (re-measured in wave 3) | `src/table_canister/src/lib.rs` seam | 6 of the 7 seam mutations die at canister level; the 7th (dropping the self-report line) has no reachable trigger in honest play. Re-run against the wave-3 payout rewrite: still **6 of 7**, same survivor |
| [H-05](#h-05) | — | **FIXED-IN-WAVE-2** | `tools/differential/src/checks/reference_probe.rs` | both references are now called per probe and the measured verdict is reported; every probe emits a finding only while the engine still accepts the input |
| [H-09](#h-09) | — | **FIXED-IN-WAVE-2** | `+page.svelte` `<main>`, `tools/shots` | spinner and lobby are now a real either/or; `data-lobby-state` is asserted by the lobby scene |
| [H-11](#h-11) | medium | code-read | `tests/money_safety/src/table_api.rs:80-108` | the fuzzer only ever runs a 30 s action timeout at `table_1` stakes; `table_2`/`table_3` shapes are never exercised |
| [H-12](#h-12) | medium | n/a | all harnesses | nothing proves the RIGHT player won. There is no independent settlement oracle |
| [H-14](#h-14) | medium | executed | `docs/DESIGN-BAR.md` §1, bars 1/2/7/10/15 | the felt geometry is mis-measured, so three of four bars would reject correct work |
| [T-03](#t-03) | low | **PARTLY-FIXED-IN-WAVE-2** | `src/lib/ic-config.js`, `vite.config.js` | the port is now build-time configurable (`VITE_LOCAL_GATEWAY_PORT`); `auth.js` and `oisy.js` still hardcode 4943 |
| [T-04](#t-04) | medium | executed | `.icp/cache/networks/local/state` | the local network state has no checkpoint height common to all five subnets, so it cannot be resumed |
| [T-07](#t-07) | — | **FIXED-IN-WAVE-2** | local id mapping | `frontend` is deployed on the local network and serves the app; 17 of 18 screenshots captured through it |
| [E-12](#e-12) | low | executed | `claim_external_deposit` | dust at or below the transfer fee in a deposit subaccount can never be swept |
| [E-13](#e-13) | low | executed | `poker_core::hand::detect_straight` | prefers the wheel over a better straight; unreachable today, a trap for the obvious optimisation |
| [E-14](#e-14) | low | code-read | `leave_table` | missing the `check_rate_limit()?` that `player_action` has |
| [E-15](#e-15) | low | executed | `poker_core::side_pots::level_pot` | the `partial_contributions` term is provably always zero: dead code on a fund path |
| [E-16](#e-16) | — | **fixed-in-wave-2** | `poker_core::shuffle` (was `lib.rs:2314`) | `(draw as usize) % (i+1)` truncated to 32 bits on wasm32, so NO third party could reproduce a deal from the revealed seed |
| [E-30](#e-30) | high | **FIXED-IN-WAVE-2, CORRECTED IN COHERENCE PASS** | `player_action`, the `AllIn` arm | an all-in that raises by less than a full min-raise reopened the betting. The first fix implemented "closed if facing anything at all", which is not the rule and is a regression on CUMULATIVE short all-ins (TDA 47-A). Now `amount_owed < min_raise`; pinned by `coherence_regressions.rs` |
| [E-31](#e-31) | high | **FIXED-IN-WAVE-2, CORRECTED IN COHERENCE PASS** | `player_action` timer check | an expired action timer was refused but never resolved, so the table wedged. Resolving it before the whose-turn check then let a message composed on the flop be APPLIED on the turn; such an action is now refused while the table still unwedges |
| [E-32](#e-32) | high | executed | `is_betting_round_complete` + `check_timeouts` | a `Disconnected` seat is skipped by the betting round yet stays live in the hand: a free showdown for money already in |
| [E-33](#e-33) | medium | code-read | `join_table` / `start_new_hand` | no post-or-wait-for-the-big-blind rule, so a player can cycle in and out taking free non-blind hands |
| [E-34](#e-34) | medium | code-read | `start_new_hand` blind assignment | no dead-button rule: when a seat between the button and the blinds empties, a player is skipped for the big blind |
| [E-35](#e-35) | medium | **FIXED-IN-WAVE-2** | `determine_winners` odd-chip rule | a chopped pot gave its WHOLE remainder to one seat; with three or more winners the rules give one chip each, clockwise from the button. Found by the settlement oracle (D-04) |
| [E-36](#e-36) | medium | executed | `join_table` + `sit_in` + `apply_player_action` | a player who takes an empty chair MID-HAND and calls `sit_in()` is given the action and can bet into a hand they hold no cards in. They can never win that money |
| [T-02](#t-02) | — | **FIXED-IN-WAVE-2** | "Verify Code" panel | the copy payload now comes from the same config as the wiring; each mainnet id appears exactly once in the bundle, as display text |
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

**This paragraph is out of date and is kept because what replaced it matters.** It used to read:
"No demonstrated way for one player to take another player's escrow, and no demonstrated way to
withdraw more than deposited plus won."

Both now exist as executed reproducers. On 2026-08-04 E-04 was fixed before E-02, exactly the
ordering this document forbids, and E-02 became live in the working tree. It was then carried
through to a **completed theft**: one 3 ICP deposit credited twice and 6 ICP withdrawn, leaving the
attacker 2.9997 ICP up in her own on-ledger wallet and another player's escrow unbacked. See
[E-02](#e-02) and [SECURITY-FINDINGS.md FINDING 10](SECURITY-FINDINGS.md#finding-10). Both are now
fixed.

`state.pot` being over-collateralised (E-03 Direction A) would also create chips, but no
attacker-reachable way to force that direction was ever demonstrated. E-05 forced the *other*
direction, which redistributes already-collected money rather than creating it. **Both are fixed
as of 2026-08-04: `state.pot` is no longer the payout basis in either direction, and a vacated
seat's stake stays in the basis.** The redistribution E-05 caused -- 20 chips out of an honest
short all-in's main pot and into the deepest stack's, with every chip conserved -- was measured
end to end by the settlement oracle before the fix and is now zero on all 17 of its hands.

---

## Engine

<a id="e-01"></a>
### E-01 — critical — every showdown destroyed all post-flop money — FIXED

**Status** **FIXED 2026-08-04**, together with [E-03](#e-03), [E-05](#e-05) and
[E-35](#e-35). Fixing any one of them alone leaves another door into the same mistake, which
is why they were done as one change.

**Where** `advance_to_next_street` (`PreFlop`→`Flop` transition) and `determine_winners`,
`src/table_canister/src/lib.rs`. Full write-up: [FINDING-01](FINDING-01-chip-destruction.md) and
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 01.

**What was wrong** `calculate_side_pots` was called unconditionally when the flop was dealt,
which froze `state.side_pots` at the pre-flop total. Nothing recomputed it: both
`determine_winners` and `run_out_board` only rebuilt it *if it was empty*, and from the flop
onwards it never was. At the showdown the winner was paid out of that frozen breakdown and
`state.pot = 0` discarded the rest, so every chip wagered on the flop, turn and river was
debited from stacks and credited to nobody. The tokens stayed inside the canister; `withdraw`
pays only against the caller's own escrow and there is no administrative withdrawal, so the loss
was **permanent and unrecoverable, including by a controller**.

Present at `HEAD` at the time, byte-identical to `d461846`, so the mainnet tables are expected to
contain it. (Confirming that needs a mainnet module hash, deliberately not read under the
local-only rule.) **Nobody has ever played on mainnet, so no user has lost money to it.**

**Found by** three independent routes: a live local-replica hand (FINDING 01), the money-safety
harness's M1 conservation invariant, and its M1b payout-basis leg. A fourth, the settlement
oracle, later reproduced it as D-01 and D-02.

**The fix** the payout basis is now **built from the players' contributions at payout time,
every time**, and `state.side_pots` is display-only state that no payout reads.

* `plan_payouts(state)` (pure, host-testable) layers `hand_contributions(state)` by all-in depth
  through `poker_core::build_side_pots_from_contributions`, which takes no `total_pot` argument
  at all, then awards each layer to the best eligible hand.
* `apply_payouts` is the only place chips are awarded, and it **traps rather than settle** a plan
  whose awards do not equal what the hand collected. The no-rake property makes that exact to
  the e8.
* One routine settles both endings. A hand that ended by fold used to be paid from `state.pot`
  and a showdown from the frozen `state.side_pots`; there is now a single basis and a single
  applier, so the two cannot disagree.
* `state.side_pots` is refreshed after every action (`advance_game`) so a reader of
  `get_table_state` never sees it disagree with `pot`, and `finish_hand` clears it.
* Uncalled bets are returned when the betting round closes rather than left in a pot only the
  bettor is eligible for. See the note at the end of [E-05](#e-05).

**Proof (before → after)**

| gate | before the fix | after |
|---|---|---|
| settlement oracle, 17 deliberate hands | 6 disagreements in 4 classes, 240 chips destroyed on the headline hand | **0 disagreements, 0 destroyed, 0 misdirected** |
| the FINDING 01 hand (blinds 1M/2M, 30M flop bet and call, checks to showdown) | winner paid 4,000,000 of a 64,000,000 pot; 60,000,000 destroyed | winner paid **64,000,000**; destroyed **0** |
| `reg09` forced settlement by one timeout | 200,000,000 destroyed | 0 destroyed (E-06 itself is still live: the hand is still ended early) |

**Reproduce (the fix)**
```
cd tests/money_safety && cargo test --test regressions -- reg01 --nocapture
cargo test -p table_canister --lib -- payout_tests::e01
make settlement                      # the oracle, against the real canister on PocketIC
```

**The markers that fired and have been inverted.** All of these were written to pass only while
E-01 was live and now gate the fixed property:

* `reg01_a_showdown_with_post_flop_betting_destroys_the_post_flop_money` →
  `reg01_a_showdown_with_post_flop_betting_pays_out_every_e8`, and it now uses the finding's own
  numbers (64,000,000 pot).
* `m1b_pot_breakdown_goes_stale_the_moment_there_is_post_flop_money` →
  `m1b_pot_breakdown_tracks_the_pot_through_post_flop_betting`.
* `seam_a_showdown_with_post_flop_betting_accounts_for_every_e8` now asserts
  `awarded == collected`, exactly as its own comment instructed.
* `pinned_e01_the_engine_still_destroys_post_flop_money` →
  `pinned_e01_post_flop_money_is_no_longer_destroyed` (settlement oracle).
* The three E-01 entries in `tests/money_safety/src/documented.rs` were **deleted**. That
  register is now empty, so nothing on the money path is tolerated. Its own doc says fixing a
  defect means deleting the entry.
* `src/table_canister/tests/money_safety.rs` `m1b_a_stale_side_pot_breakdown_understates_the_pot`
  is unchanged and still passes: it pins what the ARCHIVE routine does, which is now the record
  of the defect rather than the payout path.

<a id="e-02"></a>
### E-02 — FUND-THEFT — a real transfer was credited twice and the excess withdrawn (FIXED)

**Status** **DEMONSTRATED end to end, then FIXED**, both 2026-08-04, in the same change. This is
the one fund-theft entry in this document, and it is the only one that was carried all the way to
real ICP leaving the canister.

**Where** `notify_deposit`, `deposit`, `periodic_cleanup`, `VERIFIED_DEPOSITS`,
`src/table_canister/src/lib.rs`. The fix is the `DEPOSIT ANTI-REPLAY` section of the same file.
Full write-up, transcript and audit of the other deposit doors:
[SECURITY-FINDINGS.md FINDING 10](SECURITY-FINDINGS.md#finding-10).

**What was wrong** Two mechanisms, both turning one on-ledger transfer into two escrow credits:

1. `periodic_cleanup` bounded `VERIFIED_DEPOSITS` by **forgetting the oldest block indices** once
   it exceeded 10,000 entries. A forgotten index passed the already-processed check again. The
   memory bound was necessary; the mechanism turned *spent* into *unknown*.
2. `deposit()` (the ICRC-2 pull) never recorded the block index of the transfer it had just made.
   That block's `from` is the caller's account and its `to` is the canister's account, exactly
   what `notify_deposit` verifies, and `notify_deposit` ignored `spender`, the one field that
   identifies a `transfer_from`. Two ordinary calls, no privilege, ordinary stakes.

**How it went live, and what that proves** E-04's decode bugs were what made this unreachable, and
E-04 was fixed first. That is a decode bug standing in for an access control, and the moment it was
lifted the theft was reachable. The harness caught it on the next run; the wasm sha256 and the
`DOUBLE CREDIT` line are in SECURITY-FINDINGS.md.

**The theft, executed** `dr00_theft_one_icrc2_deposit_credited_twice_then_withdrawn`:

```
bob   deposits 20 ICP (honest)                    canister holds 23 ICP
alice deposits  3 ICP (honest)   -> block 5       escrow(alice) 3 ICP
alice notify_deposit(5)                           escrow(alice) 6 ICP, ledger balance UNCHANGED
alice withdraw(6 ICP)            -> Ok            wallet 10000 ICP -> 10002.9997 ICP
                                                  bob's escrow 20 ICP, canister holds 17 ICP
```

**The fix, and the invariant it guarantees** Stated in words at the code:

> for every ledger block index B, this canister credits escrow for B **at most once over the entire
> lifetime of its state**.

* `claim_deposit_block` is the single writer of the record and the single place the rule lives. All
  three doors call it and must see `Ok(())` before touching `BALANCES`, with no `await` between the
  claim and the credit.
* Bounding raises a **monotonic `DEPOSIT_WATERMARK`** past exactly the indices it drops, so a
  dropped index is *permanently refused* rather than forgotten. Both the record and the watermark
  are in `PersistentState`, so upgrades carry them.
* `deposit()` records the block its own pull wrote.
* `notify_deposit` refuses a block whose `spender` is this canister's own account. That is the
  defence covering pulls made before the recording existed, which matters because the mainnet
  canisters already hold state.
* New public query `get_deposit_replay_state() -> (watermark, recorded_block_count)`. **Not yet in
  `src/table_canister/table_canister.did`** (different owner; see E-08).

**The cost, deliberately accepted** A raw transfer whose block index has fallen below the watermark
is refused even if it was never credited. It takes 10,000 later recorded deposits to get there, the
ICP is still on the ledger in the canister's account, and the error message says so and quotes the
block index. Refusing a late deposit is recoverable; crediting one twice is not.

**Still open, and not defended in code, both written up in FINDING 10:**
* `install_code --mode reinstall` erases the record and the watermark, making historical blocks
  creditable again ("the reinstall hazard").
* This fix INTRODUCES a griefing vector: `deposit()` now records a block index per call and has no
  rate limit, so ~1 ICP of ledger fees pushes the watermark past ~10,000 indices and blocks a
  waiting depositor from claiming their own raw transfer. Griefing, not theft: the money stays in
  the canister and is operator-recoverable. Recommended follow-up: rate-limit `deposit()`.

**Reproduce**
```
cd tests/money_safety
cargo test --test deposit_replay                          # the 9 gates, incl. the theft itself
cargo test --test deposit_replay -- dr00 --nocapture      # the theft, transcript printed
cargo test --test deposit_replay -- --ignored dr08        # the bound at the shipped 10,000
                                                          # (executed, green, ~20 min)
```
`dr00` builds the vulnerable module itself from the current source (`E02_REVERSALS`), so the
reproducer cannot rot into a story about a lost binary. If the fix is refactored so a reversal no
longer applies, the test fails and names the hunk.

**Note for whoever runs the suite** `tests/money_safety/tests/deposit_replay.rs` is a new test
binary and is **not** in `scripts/dev.sh`'s `test` target yet, which runs `--test invariants`,
`--test regressions` and `--test fuzz` by name. Add `cargo test --test deposit_replay` there.

---

<a id="e-03"></a>
### E-03 — high — `state.pot` overrode the contributions in both directions — FIXED

**Status** **FIXED 2026-08-04**, together with [E-01](#e-01), [E-05](#e-05) and [E-35](#e-35).

**Where** `calculate_side_pots` → `poker_core::side_pots::build_side_pots_logged`
(ported bug-for-bug from `ceacc37` lines 3521-3556).
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 02 and FINDING 09.

**What was wrong** After splitting the pot by bet level, the routine reconciled against
`state.pot` and never against the sum of the players' `total_bet_this_hand`. `state.pot` won
unconditionally. Too high → the excess was appended to the highest bet level, i.e. chips
appeared in the pot only the deepest stacks could win. Too low → every side pot was scaled down
through an `f64` proportional-capping branch and chips were destroyed. e8s exceed 2^53, so that
`f64` could not even represent a large pot exactly.

**Reachability, as demonstrated** Direction B was reached in ordinary play. During a 600-step
randomised sequence the canister printed, unprompted:
```
BUG: Side pots (400000000) exceed total pot (0). Capping to pot amount.
```
4 ICP of real contributions capped to a pot of zero, in a sequence of nothing but public update
calls, time jumps and upgrades. **The engine detected its own accounting inconsistency, logged
it, and settled anyway.** Direction A's precondition was reachable via E-05.

**The fix** the payout path no longer has a `total_pot` argument to be overridden by.

* `poker_core::build_side_pots_from_contributions(&[Contribution])` is the payout basis. It
  takes the contributions and nothing else, so `sum(pots) == sum(contributions)` holds by
  construction and there is no reconciliation branch and no float anywhere on the path.
  `every_layering_conserves_every_chip` asserts it over a randomised sweep and
  `conservation_holds_at_e8_magnitudes_where_f64_would_not` does it at 2^53+1.
* `state.pot` is now treated as a **redundant accumulator**: it is cross-checked against the
  contributions on every refresh and every settlement, and a disagreement is reported as a
  `CRITICAL: pot accounting disagreement` line, which the money-safety classifier can never
  excuse. It cannot move a chip.
* **Why the mismatch reports rather than traps, deliberately.** Every write to `state.pot` in
  the file is paired with a write to a seat's `total_bet_this_hand`, and E-05's fix closed the
  one way they could drift, so the check should never fire. If it ever does, the contributions
  are the account that corresponds to chips actually taken from stacks, so settling from them is
  conservation-exact whatever `state.pot` says. Trapping instead would leave the pot unsettled,
  and an unsettled pot is the one state from which money genuinely cannot be recovered — the
  terminal state of FINDING 01. The plan's own arithmetic IS trapped on: `apply_payouts` refuses
  to credit anybody if awards do not equal what was collected.
* The old routine is kept in `poker_core::side_pots` as an **archive**, because ~1,500 golden
  vectors (466 of them tripping the capping branch) pin its exact pre-refactor behaviour and
  that record is what makes the fix auditable. Nothing calls it, and
  `the_archive_builder_and_the_payout_builder_disagree_exactly_where_e03_said` states the
  difference between the two as a test so it cannot quietly become the payout basis again.

**Reproduce (the fix)**
```
cargo test -p poker_core --lib -- side_pots
cargo test -p table_canister --lib -- payout_tests::e03
```
`e03_an_overstated_pot_cannot_mint_a_chip` corrupts `state.pot` upwards by 100 and
`e03_an_understated_pot_cannot_destroy_a_chip` sets it to 0 — the exact shape the fuzzer
produced — and both assert the payout is unchanged.

**Markers updated** the `side_pots_sum_to_pot` and `canister_reports_its_own_inconsistency`
entries in `tests/money_safety/src/documented.rs` were deleted, and
`the_one_enumerated_bug_line_is_documented` became
`the_formerly_enumerated_bug_line_now_blocks_too`: the tolerated-self-report list is empty, so
if that `BUG:` line ever appears again it stops the run.

**The one piece of evidence still open** the minimal op sequence that drove `state.pot` to 0
while contributions remained was never isolated. It no longer has a payout consequence, but it
is a real bookkeeping question about the betting path and it is now watched by the `CRITICAL:`
line rather than answered.

<a id="e-04"></a>
### E-04 — high — `notify_deposit` could never credit, and the ICP sent was stranded — FIXED

**Status** **FIXED 2026-08-04**, together with [E-02](#e-02) and in that order. Fixing this alone
is what opened the E-02 theft path, and it did in fact happen for the length of one wave: see
E-02 and the status block at the top of
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md#finding-10).

**Where** `notify_deposit`, `src/table_canister/src/lib.rs`.
[SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 06.

**What was wrong** Two independent bugs, both now fixed:

* **BUG A** `response.candid::<(QueryBlocksResponse,)>()` — in `ic-cdk` 0.19 `candid::<R>()` is
  `decode_one::<R>()`, so this asks for one value of type `record { 0 : QueryBlocksResponse }`.
  The reply is one `QueryBlocksResponse`, a named record. It can never match.
* **BUG B** `AccountIdentifier` is declared as `record { hash : blob }`; the ledger's did says
  `type AccountIdentifier = blob`. Under Candid's `opt` rule the mismatched variant arm makes
  `operation : opt Operation` decode to **null** with no error, so with only BUG A fixed every
  legitimate transfer returns `"Transaction is not a transfer"`. BUG B's failure mode is silent,
  so fixing A alone replaces a clear error with a misleading one.

The ICP the user transferred stayed in the canister and could not be withdrawn by anyone,
including a controller — the same terminal state as E-01.

**The fix** `response.candid::<QueryBlocksResponse>()` (one value, not a one-element tuple), and
`type AccountIdentifier = Vec<u8>` throughout the inline ledger types, matching
`type AccountIdentifier = blob` in the ledger's did. `Approve.allowance_e8s` is `candid::Int`, the
shape the harness had already proved decodes the real reply. Nothing else on the path changed.

**Blast radius** not the frontend happy path (the app uses `claim_external_deposit`), but the
Candid interface still publishes `get_deposit_address()` + `notify_deposit(block_index)`, so any
wallet, script or integration following the published interface loses the money it sends.

**Reproduce (the fix)**
```
cd tests/money_safety && cargo test --test deposit_replay -- dr01 --nocapture
```
`dr01_notify_deposit_credits_a_real_transfer_exactly_once` asserts the first `notify_deposit`
SUCCEEDS (that is this entry) and that a second credits nothing, before and after a real upgrade
(that is E-02). Against the real mainnet ICP ledger wasm (sha256 `a47a915e…`) installed at
`ryjl3-tyaaa-aaaaa-aaaba-cai`, the exact id the canister hardcodes. Not a mock.

**The wave-1 marker fired and has been inverted.**
`reg02_notify_deposit_cannot_decode_the_real_ledger_and_strands_the_money` was written to pass only
while this defect existed, and it failed the moment the decode was fixed
(`tests/regressions.rs:178`, `notify_deposit unexpectedly returned Ok(...)`). Its owner replaced it
with `reg02_notify_deposit_can_read_the_real_ledger_and_never_fails_to_decode`, which gates the
stronger property: no `notify_deposit` rejection may ever again be a decode failure, checked for a
real Transfer block and for a block that is not a deposit for this canister.
`cargo test --test regressions` is green.

---

<a id="e-05"></a>
### E-05 — high — a seat vacated mid-hand orphaned its stake — FIXED

**Status** **FIXED 2026-08-04**, together with [E-01](#e-01), [E-03](#e-03) and [E-35](#e-35).
This was the attacker-reachable door into E-03's minting direction.

**Where** `leave_table`, `cash_out` and (as the door-opener) `check_timeouts`,
`src/table_canister/src/lib.rs`. [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md) FINDING 05 and
FINDING 08.

**What was wrong** Setting `state.players[seat] = None` mid-hand removed the seat from
`collect_contributions` while that player's money stayed in `state.pot`. The side pots therefore
summed to less than the pot, E-03 Direction A fired, and the orphaned amount was appended to the
**highest** bet level — the pot only the deepest stacks can win.

Executed, with the real pre-refactor code. Seat 0 all-in for 50, seats 1 and 2 in for 200 each,
`state.pot = 450`:
```
all seats present         pot[0] = 150  eligible [0,1,2]    pot[1] = 300  eligible [1,2]
seat 1 FOLDS, keeps seat  pot[0] = 150  eligible [0,2]      pot[1] = 300  eligible [2]   <- correct
seat 1 VACATES the seat   pot[0] = 100  eligible [0,2]      pot[1] = 350  eligible [2]
```
Nothing about the money changed between the last two lines, only whether the seat was vacated.
The settlement oracle then measured the same thing end to end on the real canister (D-03): 20
chips moved from an honest short all-in to the deepest stack, **with every chip conserved**, so
no conservation invariant could see it. Two principals under one operator make that a repeatable
collusion edge.

**Three doors, and this is why 05 and 08 are one entry**
1. `leave_table` — a plain `#[ic_cdk::update]` with no phase guard, no turn requirement and no
   rate limit.
2. `cash_out` — its guard (`"Cannot cash out while in a hand"`) only refuses players who have
   **not** folded, so a folded player can vacate.
3. **A plain disconnect.** `check_timeouts` auto-folds a quiet player, after which door 2 opens.
   No deliberate call is needed.

**The fix, and the model it commits to** a stake is recorded **independently of seat occupancy**.

`TableState::departed_stakes: Vec<DepartedStake { hand_number, seat, principal, contributed }>`
(`#[serde(default)]`, so pre-existing state restores as empty, which is correct). Every door out
of an occupied seat calls `record_departed_stake` before vacating, and `hand_contributions`
— the payout basis — is `collect_contributions(players)` plus the departed stakes for the
current hand, carried with `has_folded = true`.

*Money in the pot belongs to the hand, not to the chair.* Once it is in, the only thing leaving
the table changes is that the player can no longer WIN it, which is exactly what folding does.
That is why a departed stake counts towards the bet levels and never appears in an eligibility
list, and it is what makes the fold-and-stay and fold-and-leave cases pay out identically —
asserted directly as `e05_folding_and_leaving_pay_out_identically`.

**The alternative model, and why it was rejected.** Keeping a ghost `Player` in the seat until
the hand ends would need no new field, but it makes an empty chair look occupied to
`join_table`, `count_active_players`, the blind assignment and the UI, and every one of those
reads would then have to know about a state that is neither present nor absent. A stake ledger
is inert: only the payout path reads it.

Entries are tagged with the hand they belong to and ignored for any other hand, cleared by
`start_new_hand` and by `finish_hand`, and pruned on write, so the vector is bounded by the
number of seats. `principal` is carried so that money nobody at the table can win can be
refunded to a player who has already left, which `apply_payouts` does by crediting escrow.

**Reproduce (the fix)**
```
cd tests/money_safety && cargo test --test regressions -- reg05 reg08 --nocapture
cargo test -p table_canister --lib -- payout_tests::e05
make settlement    # pinned_e05_a_seat_vacated_before_the_pots_are_built_no_longer_redistributes
```

**Markers updated** `reg05_vacating_a_seat_mid_hand_orphans_the_leavers_stake` →
`reg05_a_vacated_seats_stake_stays_in_the_payout_basis`; `reg08`'s tail now asserts the stake is
recorded rather than orphaned; the E-05 entry in `documented.rs` was deleted; and M1b's
attribution leg was taught the whole basis (`TableState::payout_basis_total`) so that it
measures `pot` against seated **plus** departed stakes. Against the seated set alone it would
now flag every departure as an orphan and hide a real one.

**Uncalled bets, which this fix made load-bearing.** The excess of a bet nobody covered is now
returned when the betting round closes, and `leave_table` returns the leaver's own uncalled
excess before vacating. In legal play this changes **no** seat's net position — the engine
previously left the excess in a solo side pot that only the bettor was eligible for, so the
money reached the same player, which
`returning_an_uncalled_bet_changes_no_seats_net_position` asserts directly. What it changes is
that the displayed pot no longer includes money that was never in play, and that a player who
leaves while holding an uncovered bet takes it with them instead of leaving it in a pot they can
no longer win.

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
### E-09 — medium — `poker_core` validates no input at all — **FIXED IN WAVE 2**

**Fix** `src/poker_core/src/hand.rs`. `validate_five_cards` / `validate_hand_input` /
`validate_distinct` are the boundary; `evaluate_five_cards` and `evaluate_hand` keep their
`-> HandRank` signature and now **panic**, which in a canister is a trap that rejects the
message and rolls back every state change it made. `try_evaluate_*` return the rejection as a
`Result<HandRank, HandInputError>` for callers that want to decide for themselves, and
`evaluate_*_unchecked` preserves the old unvalidated behaviour, public **only** so the
off-chain probes in `tools/differential` can still measure the defect without catching a panic.

**Why a trap and not a `Result` on the money path.** `evaluate_hand` is called from
`determine_winners`, i.e. with a pot of real ICP/ckBTC on the table. The alternative to trapping
is returning a `HandRank` for a hand that does not exist, which the caller cannot distinguish
from a real one — that pays the wrong player, irreversibly. A trap moves **no** money:
`state.pot` and every `total_bet_this_hand` survive the rollback, so the funds stay fully
attributable and a controller can settle or refund deliberately. The inputs are never
attacker-chosen (they come from the canister's own shuffled deck), so the trap is not a griefing
vector: it can only fire when the deal is already corrupt, i.e. when nothing the engine would
answer is trustworthy. The cost is that a corrupt deck now wedges that hand instead of settling
it wrongly. Deliberate: a stuck hand is recoverable, a wrong payout is not.

Accepted at `evaluate_hand`: two hole cards plus a **3-, 4- or 5-card** board, every card
distinct. At `evaluate_five_cards`: exactly **five** distinct cards. Nothing else.

**Tests** 16 in `src/poker_core/src/hand.rs`'s `validation_tests`, including all four rows of the
table below as both `try_*` rejections and `#[should_panic]` traps, plus
`a_real_deck_and_every_real_deal_out_of_it_validates` (all 46 seven-card windows of a real deck)
so the check provably cannot reject a legal hand. Reverting the two `validate_*?` lines turns 12
of the 25 `poker_core` lib tests red.
```
cargo test -p poker_core --lib validation_tests
```

**The `tools/differential` side is done, by the harness agent, in the same wave.** With
`evaluate_*` trapping, five tests in that crate went red because they deliberately feed the engine
impossible input. They now wrap the misuse probes in `std::panic::catch_unwind` (`ask_engine` in
`src/checks/degenerate.rs`) and assert rejection through `try_evaluate_*` in
`tests/fast_subset.rs`, so the harness reports the measured refusal instead of a fabricated
reference claim — that is the H-05 fix and this fix meeting in the middle. Verified together:
`(cd tools/differential && cargo test)` is 43 passed / 6 ignored, and
`./scripts/dev.sh known-defects` reports **4 defect(s) now fixed** with only E-13's marker still
red. `DEFECT_MARKERS` in `scripts/dev.sh` needed no edit; the marker names are unchanged.

`evaluate_hand_unchecked` / `evaluate_five_cards_unchecked` are consequently referenced only by
`poker_core`'s own tests now, where they prove the four defects were real by still reproducing
them. They are on no money path.

**Side effect** E-13 (`detect_straight` prefers the wheel on >5 ranks) is now **unreachable
through `evaluate_five_cards`**, which refuses more than five cards. E-13 itself is unchanged and
its marker is still red.

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

<a id="e-16"></a>
### E-16 — **FIXED IN WAVE 2** — critical — the provably-fair shuffle could not be verified by anyone

**Where** `src/poker_core/src/shuffle.rs` (was `src/table_canister/src/lib.rs:2314` at `ceacc37`).

`let j = (random_value as usize) % (i + 1);` — `random_value` is a `u64` and `usize` is **32 bits
on `wasm32-unknown-unknown`**, so the canister silently truncated the draw while every native
reimplementation, native test and reader's mental model used all 64 bits. The deck a player was
dealt could not be reproduced from the revealed seed by anybody, so the product's central claim was
false and any honest verifier would have concluded the table was rigged. Fairness was never
affected (the residual bias was ~52/2³², identical for every seat) and nobody had ever played on
mainnet. Full write-up: **[FINDING-02-shuffle-not-verifiable.md](FINDING-02-shuffle-not-verifiable.md)**.

**Fixed by** making the arithmetic width-independent (`n = i as u64 + 1`, and the only narrowing
cast is a value already reduced below 52), adding rejection sampling so the reduction is unbiased,
writing the algorithm down normatively in **[SHUFFLE-SPEC.md](SHUFFLE-SPEC.md)**, regenerating the
2,000 `S` golden vectors by executing the shuffle **inside a wasm32 module**, and adding
`src/poker_core/tests/wasm32_golden.rs` plus the CI job `wasm32-tests` so the fixture is replayed
on the target that runs the money.

**Why it survived** the golden vectors and every test ran on the host only. Demonstrated
concretely: with the original shuffle restored in a scratch tree, `cargo test -p poker_core --test
golden_vectors` is 4/4 GREEN while the new wasm32 replay convicts it on 2000 of 2000 vectors.

**Proof of the fix** two hands dealt on the real `table_canister` wasm under PocketIC with the real
ICP ledger, then reproduced — hole cards and full board — from the revealed seed alone by two
independent verifiers written from the spec, in Python and JavaScript
(`src/poker_core/tests/verify/`). The pre-fix arithmetic reproduces 0 of 9 and 0 of 11 of those
dealt cards.

**Still open, and NOT part of this fix:** the browser still calls the canister's own
`verify_shuffle` query, which only recomputes a hash (`ShuffleProof.svelte:73`), and
`ShuffleProof.svelte:322` still asserts verifiability without demonstrating it. Shipping the
client-side verifier — re-deriving the player's own cards in the browser — is the remaining work,
and the two reference implementations above are the algorithm it needs.

---

## Betting rules — correctness of play

**ID band note.** These are numbered E-30 upward, not E-17 upward, deliberately: wave 2 ran several
agents in the same tree at once and a sequential number would have collided. Renumber at leisure.

These are the defects a poker player would call cheating even though the money-conservation
invariants in `tests/money_safety` cannot see them: a rules deviation **redistributes** chips
rather than creating or destroying them, so M1..M6 all still hold. See H-12.

The tests for all of them are in **`src/table_canister/tests/betting_rules.rs`**, run by
`cargo test --workspace` and therefore by `./scripts/dev.sh test` step 1. They drive
`table_canister::apply_player_action`, `::is_betting_round_complete` and
`::resolve_expired_action_timer` — the real functions, not a re-implementation. `player_action` is
now exactly `check_rate_limit()` + `msg_caller()` + `time()` + the `TABLE` borrow +
`apply_player_action`, and `advance_game` / `advance_to_next_street` / the timer path take `now` as
an argument instead of reading `ic_cdk::api::time()`, which is what makes the state machine
host-testable at all.

<a id="e-30"></a>
### E-30 — **FIXED IN WAVE 2** — high — an incomplete all-in raise reopened the betting

**Where** `src/table_canister/src/lib.rs`, `player_action` → `apply_player_action`, the
`PlayerAction::AllIn` arm (around line 3165 at `ceacc37`).

**What was wrong** The arm set `should_reset_acted = true` on **any** all-in above the current bet,
and `should_reset_acted` clears `has_acted_this_round` for every other live player. So a short
all-in that raised by less than a full min-raise handed a brand-new raise to players who had
already acted.

```
FLOP, three-handed, min-raise 100
  seat 0 (5,000) bets 100          -> current_bet 100, min_raise 100, seat 0 has acted
  seat 1 (150)   shoves all-in     -> current_bet 150; the raise is 50, i.e. INCOMPLETE
  BEFORE: seat 0's has_acted_this_round was cleared, and Raise(300) was ACCEPTED
  AFTER:  seat 0 keeps has_acted_this_round, and Raise(300) is REFUSED
```

**The rule, and the source** Robert's Rules of Poker (Ciaffone), Section 3 "Betting and Raising":
if a player goes all-in for less than the amount needed for a full raise, the betting is **not**
reopened for players who have already acted. TDA 2022 Rule 41 (Raises) and Illustration Addendum 3
say the same, and add the other half: such a player may only **call or fold**. A player who has
**not** yet acted keeps a full option, and their minimum raise is measured off the last **full**
bet or raise, not off the incomplete shove.

**What the fix does**
1. `should_reset_acted` is set only when the all-in is a full raise
   (`raise_amount >= min_raise_in_force`, read before `min_raise` is updated). An incomplete all-in
   still raises the amount owed, still records a `last_aggressor`, and still costs the big blind
   their free check — it just does not reopen anyone's raise.
2. The other half of the rule is now enforced, which it never was: a closed player attempting
   `Raise` is refused, and so is `AllIn` when their stack would put them above the current bet.
   Shoving for the amount owed or less is a call for less and stays legal.

   **No new state was needed.** A full bet or raise clears `has_acted_this_round` for every other
   live player, so a player who still carries `has_acted_this_round == true` while facing a bet
   larger than their own has by construction only been raised into by incomplete all-ins since they
   acted. `player_has_acted && player_current_bet < state.current_bet` **is** "closed to raising".
   Posting a blind is not an action, so the blinds are never closed by it.
3. An over-shove by a closed player is **refused, not silently shrunk to a call**. Quietly
   resizing somebody's wager is not a safe default in a canister that holds funds, and the
   rejection names the amount they may call.

`state.min_raise` was already correct for this case before the fix (an incomplete all-in does not
become the new increment), and that is now pinned by
`the_minimum_raise_over_an_incomplete_all_in_is_measured_off_the_last_full_raise`.

**Reproduce / proof** 8 tests in section 1 of `tests/betting_rules.rs`. Restoring the unconditional
`should_reset_acted = true` turns 3 of the 24 red, including
`an_incomplete_all_in_raise_does_not_reopen_the_betting_to_a_player_who_already_acted`; removing
just the two legality guards turns the same 3 red.
```
cargo test -p table_canister --test betting_rules
```

<a id="e-31"></a>
### E-31 — **FIXED IN WAVE 2** — high — an expired action timer wedged the table

**Where** `src/table_canister/src/lib.rs`, `player_action` (around line 3024 at `ceacc37`) and
`check_timeouts`.

**What was wrong** `player_action` *refused* an action whose timer had passed —
`Err("Action timer has expired. Your turn was forfeited.")` — and changed nothing else. Only
`check_timeouts` ever resolved a timeout, and **nothing inside the canister calls
`check_timeouts`**: there is no `ic-cdk-timer` driving it, the frontend polls it. So once the clock
passed, `state.action_on` stayed pointed at a seat that could no longer act and no message could
move the hand.

Hit immediately in manual play: both players ended up `Disconnected` and `start_new_hand` refused
with `"Need at least 2 active players with chips"` until they were explicitly sat back in.

**The correct behaviour, and why** A forfeited action must **resolve** the hand state. The
alternative is a table that no message can move, and on a canister that is worse than any single
mis-ruled action: the pot is frozen with real funds in it and the only remaining tool is
`admin_reinit_table`, which E-07 shows strands every seated player's chips. Refusing an action
without resolving it is not a conservative choice, it is the unrecoverable one.

**What the fix does** One shared `resolve_expired_action_timer(state, now) -> Option<u8>` is now
the only implementation of "a clock ran out", used by both `check_timeouts` and
`apply_player_action`. In `apply_player_action` it runs **before** the whose-turn check, so **any**
player's message unwedges the table, not only the message of the player who timed out. The
forfeited seat is folded, `advance_game` runs, and the next player gets a full clock starting now
(they must not be charged for the idle period). The late action is then rejected with a message
that says the clock ran out, distinct from "Not your turn".

`now > timer.expires_at` is unchanged, so an action arriving exactly on the expiry instant is still
good — pinned, so a later change cannot quietly start eating live actions.

**Deliberately NOT changed: a forfeited action is a FOLD, even when checking was free.** Every
online room (PokerStars, GGPoker, partypoker, PokerNow) auto-**checks** when there is nothing to
call and folds only when there is, and folding a hand that could have seen the next street for
nothing destroys equity the player never had to give up. It is left alone here because it changes
which hands reach showdown, and `tests/money_safety/tests/regressions.rs` REG-08 pins the current
behaviour (`"the quiet player must have been auto-folded"` on a flop where `current_bet == 0`).
**Recommended follow-up, owner decision, one condition:** timeout → check when
`player.current_bet >= state.current_bet`, fold otherwise; REG-08's fixture needs a bet in front of
the quiet player for the assertion to keep meaning what it says.

**Reproduce / proof** 7 tests in section 2 of `tests/betting_rules.rs`. Restoring the
refuse-only behaviour turns 5 of the 24 red. Behaviour preservation at the canister level was
checked against the real wasm on PocketIC: `reg08_a_timed_out_player_can_cash_out_mid_hand_and_
orphan_their_stake` and `reg09_one_timeout_with_no_heartbeats_runs_the_whole_board_out_and_settles`
both still pass, i.e. the extraction did not change what `check_timeouts` does.

<a id="e-32"></a>
### E-32 — high — a `Disconnected` seat is skipped by the betting round yet stays live in the hand

**Where** `is_betting_round_complete`, `count_active_players` and `find_next_active_seat` (all
require `status == PlayerStatus::Active`) versus `determine_winners` (evaluates every player with
`!has_folded`). Set up by the hardcoded 30 s `DISCONNECT_TIMEOUT_NS` in `check_timeouts`.

**Status: characterised, NOT fixed.** Fixing it decides who is eligible for which pot, which is the
payout path another agent owns in this wave.

**What is wrong** A player marked `Disconnected` mid-street blocks nothing and is offered nothing:
the betting round closes without them ever being asked to call. But they are **not folded**, so at
showdown their hand is evaluated and they stay eligible for every pot level they had already paid
into. They keep the chips they would have had to call **and** their equity in the money already in.

```
FLOP, three-handed, everyone paid 20 pre-flop (pot 60)
  seat 0 bets 200
  seat 2 stops heartbeating -> check_timeouts marks it Disconnected after 30 s
  seat 1 calls 200
observed: phase Turn, seat 2 has_folded=false, chips=5000 (never called),
          total_bet_this_hand=20 (still eligible for the 60 main pot), pot 460
required: seat 2 folded, the 20 forfeited
```

**Why it matters** It is deliberately reachable — stop heartbeating for 30 s while somebody else is
on the clock, let the street close, then `heartbeat()` back to `Active` — and it takes value from
the other players: you decline a bet, keep the chips, and keep the pot equity. It also fires
accidentally on a real network blip, in which case it takes value from the honest players at the
table.

Applies to `table_2` (45 s clock) and `table_3` (60 s), where the 30 s disconnect threshold fires
**before** the action clock, and only to a player who is not the one on the clock (whose own timer
would eventually fold them). On `table_1` / `btc_table_1` the two thresholds are equal, which is
E-06 instead. So E-06 and E-32 are complementary: **every deployed table has one or the other.**

**The rule** Robert's Rules of Poker, Section 1 #12: a player called upon to act who fails to act
has a folded hand. There is no "skip the player but keep them live" state in poker.

**Reproduce**
```
cargo test -p table_canister --test betting_rules -- \
  characterises_a_disconnected_player_being_skipped_yet_left_live_in_the_hand
```
That test asserts TODAY'S wrong answer, so the behaviour cannot change silently. It goes red the
moment the defect is fixed, and its body says what the correct state is.

**Also worth an entry in [SECURITY-FINDINGS.md](SECURITY-FINDINGS.md)** — it is a way to take
another player's money, not the canister's. Not written there by the finding agent because that
file was open in another agent's working set at the time.

<a id="e-33"></a>
### E-33 — medium — no post-or-wait-for-the-big-blind rule, so free hands can be farmed

**Where** `join_table` sets `status = Active` for anyone seating between hands, and `start_new_hand`
deals to every `Active` seat with no check on whether that seat has yet paid a blind in the current
rotation.

Robert's Rules of Poker, Section 2 "Button and Blinds": a player entering a game must either post
the equivalent of the big blind or wait for the big blind to reach them. Neither exists here.

The abuse is a cycle, not a single hand: seat in a position that will not be a blind next hand,
play the hand for free, `leave_table` before the blind reaches you, and re-enter. Whether a given
empty seat lands you in a blind is deterministic from the seat index and the current button
(`find_next_active_seat_with_chips` walks forward from the button), so it can be chosen rather than
guessed. No funds leave escrow; positional value is taken from the players who are paying blinds.

**Not executed** — code-read. `start_new_hand` is `async` and calls `raw_rand`, so it is not
host-testable; demonstrating this needs the PocketIC harness.

<a id="e-34"></a>
### E-34 — medium — no dead-button rule: a player can be skipped for the big blind

**Where** `start_new_hand`:
```rust
state.dealer_seat = find_next_active_seat_with_chips(state, state.dealer_seat);
// then SB and BB are DERIVED by walking forward from the new button
state.small_blind_seat = find_next_active_seat_with_chips(state, state.dealer_seat);
state.big_blind_seat   = find_next_active_seat_with_chips(state, state.small_blind_seat);
```
Deriving both blinds from the button each hand is correct only while the field is stable. Standard
cash-game rule (Robert's Rules, Section 2): the **big blind advances exactly one live player per
hand**; the button may be dead, and no player may be skipped for the big blind or pay it twice in a
row.

Worked example, four seats, seat 2 busts and is removed:
```
hand 1   dealer 1, SB 2, BB 3
hand 2   engine: dealer 3, SB 0, BB 1
         rules:  BB advances 3 -> 0; SB is dead or the button is dead
```
Seat 0 posts the small blind and is **never** the big blind in that rotation, and seat 1 pays the
big blind a hand early having just held the button. Both are EV transfers.

**Not executed** — code-read, but the seat arithmetic above follows directly from
`find_next_active_seat_with_chips` and needs no fixture to check.

### Smaller audit notes, no entry of their own

* **`advance_to_next_street` picks the post-flop first actor with `find_next_active_seat`, which
  requires `status == Active`.** Heads-up this is right (the button is the small blind, so the big
  blind acts first post-flop, exactly as the rules require) and multiway it is right (first live
  seat left of the button). But it is also the mechanism behind E-32: a `Disconnected` live seat is
  never given the action.
* **Heads-up blind posting and pre-flop order are correct.** `active_count == 2` puts the small
  blind on the button and starts the action there; post-flop the big blind acts first. Also correct
  when a three-handed table drops to two through a sit-out, because `active_count` is computed from
  Active-with-chips seats.
* **A short big blind is handled correctly.** `state.current_bet` stays at the FULL
  `config.big_blind` even when the blind was posted for less, so the amount to call does not shrink,
  and `min_raise` stays at one big blind. A player short of the blind is always left `is_all_in` by
  the posting, so `bb_has_option` is correctly cleared.
* **Dead branch:** the `Check` arm's `(is_bb_option && state.current_bet <= player_current_bet)`
  clause can only ever be an equality, which the preceding clause already covers — `bb_has_option`
  is false whenever anyone has bet above the blind. Same shape as E-15: defensive-looking code on a
  betting path that provably never runs.
* **`Bet` and `Raise` have no all-in exception** (`Bet` demands `>= big_blind`, `Raise` demands a
  full increment). Correct, because `AllIn` is the exception — but a player whose whole stack is
  below one big blind gets `"Minimum bet is ..."` from `Bet` and has to know to send `AllIn`
  instead. API sharp edge, not a rules deviation.
* **Hole cards are dealt two-at-a-time in seat-vector order**, not one at a time starting left of
  the button. Statistically harmless, but the deal order is part of what a
  "provably fair" claim covers and it is written down nowhere;
  [SHUFFLE-SPEC.md](SHUFFLE-SPEC.md) should state it now that the shuffle is normative.
* **First-hand button:** `find_next_active_seat_with_chips(state, 0)` starts at seat **1**, so on a
  fresh table with seats 0..2 occupied the first button is seat 1, not seat 0. Cosmetic.

<a id="e-35"></a>
### E-35 — medium — a chopped pot gave all its odd chips to one seat — FIXED

**Status** **FIXED 2026-08-04**, with [E-01](#e-01), [E-03](#e-03) and [E-05](#e-05).
**Found by the settlement oracle**, which is the only instrument in the repo that could see it:
it conserves every chip exactly, so no M1..M6 invariant can. Reported there as **D-04**.

**Where** `determine_winners` and `first_clockwise_from_dealer`,
`src/table_canister/src/lib.rs`.

**What was wrong**
```rust
let pot_share = side_pot.amount / pot_winners.len();
let remainder = side_pot.amount % pot_winners.len();
let amount = if seat == remainder_seat { pot_share + remainder } else { pot_share };
```
The WHOLE remainder went to a single seat. With two winners the remainder is at most one chip
and the rule coincides with the real one, which is why it went unnoticed. With three or more it
can be two or more chips, and then it is wrong: rooms distribute odd chips **one each**, walking
clockwise from the button (Robert's Rules of Poker for flop games; the TDA rules put odd chips
with the player(s) in the earliest position, one at a time when there is more than one).

Observed on the real canister, on a three-way tie whose smallest layer held 8 chips: 2 each and
two over. The engine gave both odd chips to seat 2; the rules give one to seat 2 and one to seat
4. ClearDeck's chips are e8s, so those are real, withdrawable chips.

**Why it matters more than 1 e8** a pot was being divided by a rule the game does not have, on
the code path that also decides who gets the other 99.99% of the money.

**The fix** `poker_core::split_pot_clockwise(amount, winners, dealer_seat, num_seats)` places
the chips that do not divide one at a time, in clockwise order from the seat after the button,
and always pays out exactly `amount`. `first_clockwise_from_dealer` is deleted:
"which ONE of these seats gets the whole remainder" is not a question the rules ask.

**Reproduce (the fix)**
```
cargo test -p poker_core --lib -- split
cargo test -p table_canister --lib -- payout_tests::e35 payout_tests::a_two_way_chop
cd tests/settlement && cargo test --test disagreements -- golden_d04
```
`splitting_always_pays_out_the_whole_pot` sweeps every amount 0..200 x five winner sets x six
button positions and asserts the shares differ by at most one chip and sum to the pot exactly.

**Markers updated** `pinned_odd_chips_still_all_go_to_one_seat` →
`pinned_odd_chips_now_go_one_each_clockwise`, which additionally asserts that no winner of a
chopped layer is more than one chip clear of another. `golden_d04*` are unchanged: they are the
record of what the defect was.

<a id="e-36"></a>
### E-36 — medium — a mid-hand arrival can bet into a hand it holds no cards in

**Status** **executed**, found 2026-08-04 by the money-safety fuzzer at 600 steps (seed
`0xc1ea2dec0003`, 6-max with an ante) while validating the E-01/E-03/E-05 payout fix. **Not
fixed:** it belongs to the seating and betting path (`join_table`, `sit_in`,
`apply_player_action`), which had a different owner in this wave.

**What happens**
```
seat 2 is in a live hand with 1,500,000 committed
seat 2 calls leave_table          -> the chair is empty, the stake stays in the pot
another player calls join_table(2) -> seated SittingOut, no hole cards, total_bet 0
that player calls sit_in()         -> status becomes Active
```
From that point `find_next_active_seat` will give them the action, because it asks only for
`status == Active && !has_folded && !is_all_in` and never for cards. `apply_player_action` does
not check `hole_cards` either, so they can call, bet or raise into a hand they were never dealt
into. **They cannot win any of it:** the payout path only pays a seat that holds cards, so their
money goes to the players who are actually in the hand.

It is a loss for whoever does it rather than a way to take somebody else's money, and it needs
two deliberate calls plus somebody else leaving mid-hand. That is why it is medium and not high.

**How it was found, and what it says about the payout path** the fix for E-05 records a departed
seat's stake against its seat number, so this sequence makes ONE seat carry two stakes in one
hand -- a state the fix's author had reasoned was unreachable. It is handled: both stakes stay in
the payout basis (dropping either would destroy chips), the new occupant is excluded from the
claimants because they hold no cards, and money nobody can claim is carried to a layer that can
be settled. The fuzzer's conservation invariants were clean through it (0 e8s stranded across 6
hands and 4 upgrades); what it flagged was the engine's own log line.

**Reproduce**
```
cd tests/money_safety
MONEY_FUZZ_STEPS=600 cargo test --test fuzz -- --nocapture     # seed 0xc1ea2dec0003
```
The line to look for is
`WARNING: seat 2 carries both a live stake and a departed stake in hand 1`.

**Suggested fix** (for the owner of the seating path): refuse the action to a seat with no hole
cards while a hand is live -- either by keeping a mid-hand arrival out of `find_next_active_seat`
until the next hand regardless of `sit_in`, or by rejecting `player_action` from a seat holding
no cards with a clear message. The second is the narrower change and it also closes the same hole
for any future path that sets `status = Active` mid-hand.

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

**FIXED IN WAVE 2.** Three layers, in order of strength:

1. `src/cleardeck_frontend/build/network-env.mjs` + `vite.config.js`. The unconditional
   `dotenv.config({ path: '../../.env' })` is gone. The build now **requires** an explicit
   `DFX_NETWORK` / `ICP_NETWORK` of `local` or `ic`, loads the repo-root `.env` **only** for
   an `ic` build, and aborts with an actionable message if a `local` build resolves an id
   that appears in `.icp/data/mappings/ic.ids.json` (the project's own committed mainnet
   mapping, so the denylist is never a hand-maintained copy). Throwing from the vite config
   aborts before a byte is emitted.
2. `canisters.js` `resolveCanisterId()` refuses a mainnet id on a non-mainnet build and
   refuses a missing id on any build, at runtime, for a bundle built by another toolchain.
   `assertTableIdAllowed()` applies the same rule to table ids handed over by the lobby.
3. `ic-config.js` compiles the target network into the bundle (`VITE_ICP_NETWORK`), so
   `isMainnet()` no longer has to infer the network from `window.location.hostname`.

Verified by building four ways: a bare `npm run build` **fails** ("does not know which
network it is for"); `DFX_NETWORK=local` with the mainnet lobby/history ids **fails**
("these are MAINNET ClearDeck canisters"); `DFX_NETWORK=local` with no ids **fails** ("the
bundle would render fine and talk to nothing"); the wired local path **succeeds** and the
bundle contains the local lobby id. `ICP_NETWORK=ic` still resolves the mainnet ids from
`.env` (production path preserved), and CI's `DFX_NETWORK=ic` + placeholder ids still works
because dotenv never overrides an explicit value.

<a id="t-02"></a>
### T-02 — low — a locally-wired build still offers a mainnet `-e ic` command

Even with local wiring, the built bundle contains seven mainnet id occurrences across five ids,
including two `navigator.clipboard.writeText("icp canister status qrhly-eaaaa-aaaaj-qousa-cai -e ic")`
payloads and a "Tables (all)" row listing `kieex`/`lfkaz`/`lclgn`. A Copy button that hands a user
a mainnet command from a local dev build is a footgun; the ids should come from the same runtime
config as the wiring.

**FIXED IN WAVE 2.** Both `navigator.clipboard.writeText('icp canister status qrhly-… -e ic')`
payloads are gone. The command is now `statusCommandFor()` from `ic-config.js`, which emits
`-e ic` on a mainnet build and `-e local` with the local canister id otherwise, so the copy
payload can never disagree with what the bundle actually talks to. A local build also shows a
line saying so. The panel's legitimate display of the mainnet ids is untouched and now reads
them from `MAINNET_CANISTER_IDS` instead of retyped literals.

Measured in the built local bundle: `-e ic` appears **once**, as prerendered display text in
the "Verify with:" note (documentation of how to audit the live deployment, not a payload),
and each of the seven mainnet ids appears **exactly once**, in the display-only constant.
Before: seven occurrences across five ids in three places, two of them clipboard payloads.

<a id="t-03"></a>
### T-03 — medium — the app hardcodes gateway port 4943

`src/lib/ic-config.js:21` — `export const LOCAL_HOST = 'http://127.0.0.1:4943'`, and `auth.js`
hardcodes the same port for the local Internet Identity origin. This project's gateway is pinned
to **8077** in `icp.yaml` (because 8000 belongs to another project on this machine). The
screenshot harness works around it with a transparent 4943→8077 reverse proxy, which is a shim,
not a fix. Make the port configurable at build time from the same place the canister ids come
from.

**PARTLY FIXED IN WAVE 2.** `ic-config.js` now derives `LOCAL_HOST` from
`VITE_LOCAL_GATEWAY_PORT` (default 4943), and `vite.config.js` uses the same value for its
dev-server `/api` proxy target — so the port is configurable at build time from the same
place the canister ids come from, which is what this entry asked for.

Still open: `auth.js:116` hardcodes `…localhost:4943` for the local Internet Identity origin
and `oisy.js` hardcodes it in three places. Those files belong to other waves. The screenshot
harness therefore still runs its 4943→8077 reverse proxy, and deliberately keeps the default
at 4943 so the page and the agent share one origin (moving the agent to 8077 while the page
is served from 4943 would introduce a cross-origin problem the shim currently avoids).

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

**FIXED IN WAVE 2.** The asset canister is deployed on the local network and serves the real
app; every screenshot in `artifacts/screens/latest/` was taken against it, through the
gateway, with the local canister ids compiled into the bundle. Note the id is not stable
across a `local-up --reset`: the reset destroys local state and every canister id is
reassigned, so it must always be read from `.icp/cache/mappings/local.ids.json`.

<a id="h-01"></a>
### H-01 — high — the money-safety harness is not a regression gate

> **FIXED IN WAVE 2.** `wasms.rs` now has ONE resolution path: it always runs
> `cargo build -p table_canister --target wasm32-unknown-unknown --release` into the shared target
> dir, reads that file, and prints its sha256 on fd 2 (bypassing `cargo test`'s per-test output
> capture) as the first line of every run:
> ```
> MONEY-SAFETY: wasm under test sha256=32ed2748…c955ac bytes=2272713 path=…/table_canister.wasm
> ```
> `$CLEARDECK_TABLE_WASM` no longer SELECTS the artifact; if it names a different file the run
> hard-fails on a sha256 mismatch. After `install_canister` and after every `upgrade_canister`,
> `World` reads back `canister_status().module_hash` and hard-fails unless it equals that sha256,
> so "the bytes the harness built" and "the code the replica executed" are the same claim.
> Proven the same afternoon: another agent had `lib.rs` in a non-compiling state and the harness
> refused to run (`building table_canister for wasm32-unknown-unknown FAILED`) instead of silently
> reporting green against the 4-hour-old artifact still sitting at that path.

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

> **FIXED IN WAVE 2.** `check_self_reported_inconsistency` now splits the pattern. Exactly one
> line — `documented::REGISTERED_SELF_REPORT`, the `BUG: Side pots (…` warning of E-03 — stays
> `BreakdownDrift` and non-blocking. Every other `BUG:` line and EVERY `CRITICAL:` line becomes
> `Severity::SelfReportedFailure`, which `documented::classify` can never excuse, so it stops the
> run. Covered by `a_critical_log_line_blocks`, `an_unenumerated_bug_log_line_blocks`,
> `the_one_enumerated_bug_line_is_documented` and `ordinary_log_lines_produce_nothing` in
> `tests/money_safety/tests/invariants/classifier.rs`, plus
> `seam_honest_play_produces_no_self_reported_failure`, which states that honest play produces
> nothing to block on so a future failure is a real change.

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

> **FIXED IN WAVE 2.** `Severity::is_documented_defect()` is gone. `tests/money_safety/src/
> documented.rs` holds a NAMED register: each entry carries a defect id from this file, the exact
> `(invariant, check)` pair it excuses, the direction(s) its mechanism can produce and a magnitude
> bound. Anything unlisted blocks — including every `FundDestruction` in a check nobody
> registered — and `FundCreation`, `DoublePay`, `DurabilityLoss`, `RakeTaken` and
> `SelfReportedFailure` are never excusable whatever the register says.
>
> A rake is now caught by a check that survives E-01: `check_awarded_equals_payout_basis` asserts
> the engine paid out ALL of whatever basis it chose (`sum(side_pots)` at the last live
> observation, or `pot`). E-01 makes the basis WRONG; it does not make the engine keep part of it,
> so `awarded < basis` is a rake even while E-01 is live. Severity `RakeTaken`, never excusable.
>
> **Still true, and deliberately not claimed otherwise:** `awarded < collected` cannot separate a
> small rake from E-01's own destruction, because E-01 *is* a rake in every observable respect. The
> two E-01 register entries carry a loose bound (the whole world's money) and say so in their `why`.
> Fixing E-01 means DELETING those entries, at which point every destruction blocks.

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

> **MOSTLY FIXED IN WAVE 2.** Canister-level tests now drive the real state machine through
> PocketIC: `tests/money_safety/tests/invariants/seam.rs`, wired into the `invariants` target that
> `scripts/dev.sh test` already runs, with the hand driver and the per-hand measurements in
> `tests/money_safety/src/scenario.rs`. `src/table_canister/tests/integration_test.rs` is no longer
> comments only: it fails if that suite is deleted or a named test disappears.
>
> Re-running all seven mutations against the same engine baseline (a clean `git archive` of repo
> `801aa79`), one mutation at a time. Three conditions, because the answer depends on whether the
> harness compiled the mutant at all — which is H-01:
>
> | mutation | wave-1 harness + STALE artifact (the H-01 condition) | wave-1 harness, mutant compiled | THIS wave |
> |---|---|---|---|
> | 1. `calculate_side_pots` made a no-op | SURVIVED | died | **died** |
> | 2. `state.pot / 2` passed as the pot | SURVIVED | died | **died** |
> | 3. pots written into a throwaway `Vec` | SURVIVED | died | **died** |
> | 4. `&[]` passed for the contributions | SURVIVED | died | **died** |
> | 5. the `BUG: Side pots` warnings dropped | SURVIVED | SURVIVED | **SURVIVED** |
> | 6. `evaluate_hand` stubbed to `RoyalFlush` | SURVIVED | SURVIVED | **died** |
> | 7. `shuffle_deck` from the constant seed `b"CONSTANT"` | SURVIVED | SURVIVED | **died** |
> | | 0 of 7 caught | 4 of 7 | **6 of 7** |
>
> Each cell is `cargo test --workspace` plus `cd tests/money_safety && cargo test --test invariants`.
> A cell is "died" only when a target reported `test result: FAILED`; a pocket-ic sandbox crash does
> not count and was re-run.
>
> **Read column 1 first.** It is wave 1's documented 7-of-7 result, reproduced exactly: with a
> PRISTINE `target/wasm32-unknown-unknown/release/table_canister.wasm` already at that path and no
> wasm32 build in the run, the money-safety suite reported `10 passed; 0 failed` for every one of the
> seven mutations, and the artifact's sha256 was byte-identical before and after each. The suite was
> never testing the mutant. That is H-01, and it is why H-01 had to be fixed before any of this
> could be measured.
>
> Column 2 is the same wave-1 harness once it IS given the mutant, and it is a fairer baseline for
> judging what the new tests add: the pre-existing
> `m1b_pot_breakdown_goes_stale_the_moment_there_is_post_flop_money` already asserts
> `!side_pots.is_empty()` on the flop, so mutations 1-4 were catchable all along. What no test could
> see was mutation 6 (a stubbed evaluator) or mutation 7 (a fixed deck) — the two that decide who
> wins and what is dealt.
>
> **WAVE-3 RE-RUN, after the FINDING 13 / FINDING 14 rewrite of the payout path.** The wave-2
> mutations targeted functions the rewrite changed, so each was translated again to its nearest
> current site, keeping what the mutation MEANS. One at a time, restored between each, every
> anchor asserted to match EXACTLY ONCE, and a mutation that fails to compile is reported as
> `NOT-APPLIED` rather than counted as a kill. Gate per mutation: `cargo test --workspace` plus
> `cd tests/money_safety && cargo test --test invariants`. Script:
> `$SCRATCH/seam_mutations.py`, run against a `cp -Rc` copy of the tree; the repo's own `lib.rs`
> was untouched and asserted byte-identical afterwards.
>
> | mutation (wave-3 site) | verdict | what convicted it |
> |---|---|---|
> | 1. `refresh_side_pots` made a no-op | **died** | `seam_a`, `seam_b` |
> | 2. every stake halved in `plan_payouts` | **died** | `cargo test --workspace` (20 of 22) |
> | 3. the breakdown written into a throwaway `Vec` | **died** | `seam_a`, `seam_b` |
> | 4. `&[]` passed as the contributions | **died** | `cargo test --workspace` (14 of 22) |
> | 5. the self-report line dropped | **SURVIVED** | — (equivalent mutant, see below) |
> | 6. `evaluate_hand` stubbed to `RoyalFlush` | **died** | `cargo test --workspace` (8 of 22) |
> | 7. every hand dealt from `b"CONSTANT"` | **died** | `seam_c`, `seam_honest_play_produces_no_self_reported_failure` |
> | | **6 of 7** | unchanged from wave 2, same survivor |
>
> Killed by: mutations 1-4 by `seam_b_side_pots_sum_to_pot_at_the_moment_calculate_side_pots_runs`
> and `seam_a_showdown_with_post_flop_betting_accounts_for_every_e8`; mutation 6 by
> `seam_d_the_recorded_showdown_ranks_are_the_ranks_evaluate_hand_returns` (`the canister says seat 0
> held RoyalFlush, but ... evaluate_hand ... gives HighCard([14, 13, 10, 9, 8])`); mutation 7 by
> `seam_c_the_deck_is_different_every_hand`.
>
> The survivor is mutation 5, dropping the `BUG: Side pots (…) exceed total pot (…)` log line.
> It is behaviour-preserving on every reachable input: the warning only fires when the built side
> pots exceed `state.pot`, and honest play cannot reach that (E-05's orphaned stake makes the pots
> SMALLER than the pot, not larger). It is therefore an equivalent mutant at canister level rather
> than a coverage hole, and the blocking classification added for H-02 means that if the condition
> ever does arise the run fails. Killing it properly needs a unit test on
> `poker_core::apply_side_pots`, which belongs with `poker_core`, not with the seam.

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

> **FIXED IN WAVE 2.** `tools/differential/src/checks/reference_probe.rs` calls both references on
> whatever cards it is given and reports what came back, capturing an `Err` or a panic as a
> refusal (`rs_poker` carries internal `debug_assert!`s, so a call that ranks in `--release` can
> abort in a debug `cargo test`). Every P3/P5/P7 finding now quotes the measured verdict, asks the
> third reference too when `CLEARDECK_PHE_PYTHON` is set, and states the corrected conclusion
> in the finding itself when neither reference refused.
>
> The three facts this entry records are pinned as a test
> (`no_reference_in_this_harness_validates_its_own_input`) and re-confirmed in both debug and
> release: `rs_poker` → `StraightFlush(0)` for `Ah Ah Ah Ah Ah`, `FourOfAKind(155)` for
> `Kh Kh Kd Kd Qs`, `HighCard(2140)` for `Ah Kd`; only `poker` 0.7.0 errors.
>
> Each probe also asks `poker_core` inside a panic guard and emits a finding ONLY while the engine
> still accepts the input, so `degenerate_probe_set_reports_exactly_the_defects_that_are_still_live`
> is now an equality assertion. That is what caught the E-09 validation landing: four of the five
> ids disappeared on their own.

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

1. ~~**E-04 + E-02 together.** Never E-04 alone: it opens a chips-from-nothing path.~~
   **DONE, 2026-08-04, and the reason it was written is now on the record.** E-04 landed first,
   E-02 went live for the length of one wave, and the harness caught it. The theft was then executed
   end to end (real ICP out of the canister into an attacker's wallet) before the fix, so this entry
   is no longer a warning about a hypothetical. See [E-02](#e-02).
2. ~~**E-01**, with the H-04(a) conservation test written first so the fix is proven.~~
   **DONE, 2026-08-04.** The conservation gates existed first (H-04(a) `seam_a`, `reg01`, M1) and
   all three were red on the old code and green on the new. See [E-01](#e-01).
3. ~~**E-03 + E-05 together.** Compare against `total_bet_this_hand` and trap on mismatch, and stop
   letting a vacated seat erase its stake. Either fix alone leaves the other door open.~~
   **DONE, 2026-08-04, as one change with E-01 and [E-35](#e-35)** -- four doors into one mistake,
   and the instruction that either fix alone leaves the other door open turned out to be exactly
   right: reverting E-05 alone in a copy of the repo destroys 60 chips and makes the engine print
   `CRITICAL: pot accounting disagreement`, and reverting E-01 alone makes it refuse to settle at
   all. The payout basis is now the players' contributions, `state.pot` is a cross-checked
   redundant accumulator that cannot move a chip, and a vacated seat's stake is recorded
   independently of the seat. See [E-03](#e-03) and [E-05](#e-05).
4. **H-01, H-02, H-03** before trusting any further green run — otherwise the fixes above are
   being validated by a suite that cannot fail.
5. **E-06**, then **E-07**, then **E-09**.

---

## Found by the wave-2 coherence pass

Seven pieces were built in parallel by agents who could not see each other's work. These are the
defects that only exist because of that, plus the ones a reviewer named and nobody landed. All
measured against table canister wasm
`395696359b6d1a6e185fca9e2e051bbb6c8413b62e1545aae55c66d361d8342f`.

<a id="e-37"></a>
### E-37 — high — a departed player's refunded stake is paid to whoever took their chair — FIXED

> **FIXED 2026-08-04 (wave 3), after first being DEMONSTRATED.** Wave 2 filed this as high
> because the two ingredients had each been reached but never composed in one hand. They compose
> in five ordinary API calls, and the composition now runs against the real canister on every
> `./scripts/dev.sh test`:
>
> ```
> M8:   74yuz-2axoe-...-bqsze-dae   -2000000     (alice, who left with 0.02 ICP in the pot)
> M8:   f6m43-ks6kd-...-xks4f-rae   +2000000     (the stranger who took her chair)
> ```
>
> **The fix** the owner travels with the money. `Stake { seat, owner, amount, relinquished }` is
> the payout basis, `Payout::principal` is a plain `Principal` sourced from the stake or from the
> live claim that won the layer, and `principal_of(state, seat)` is **deleted** — there is no
> longer any function in `lib.rs` that turns a seat index into a payee. `apply_payouts` credits
> that principal: their stack if they are in that seat, otherwise their escrow. `push_winner`
> aggregates by `(seat, principal)` and only attaches hole cards when the occupant IS the payee.
>
> **The gates** `payout_tests::finding13_*` (4 host tests),
> `M8_PRINCIPAL_ATTRIBUTION` in `tests/money_safety/src/invariants/attribution.rs`,
> `principals::m8_*` against the real canister, and a PRINCIPAL column in the settlement oracle
> that `Bench::gate` fails on. All four are RED against the pre-fix build. Full write-up:
> [SECURITY-FINDINGS.md FINDING 13](SECURITY-FINDINGS.md).

Introduced by the E-05 fix. `principal_of()` resolves a payout's owner from the **seat**, looking
`state.players[seat]` up first and only falling back to `departed_stakes` when the chair is empty.
So a `Refund` of a departed stake at a chair somebody else has since taken names the **new
occupant**, and `apply_payouts` adds the money to that stranger's stack. The
`(Some(occupant), Some(owed)) if occupant != owed` arm written to prevent this is **dead code**:
both sides of the comparison come from the same seat.

Every chip is conserved, so no conservation invariant can see it, and the settlement oracle diffs
per **seat** — which is paid the right amount — so it cannot see it either. That is the whole
point: this is a class of defect the oracle was built without.

**Reproduced independently in this pass.** Host test against the real `plan_payouts` /
`determine_winners`, no replica:

```
C3: alice's stake is owed to <alice>; the plan names Some(<stranger>)
C3: stranger chips 7 -> 57; alice escrow 0 -> 0
plan.conserves() == true
```

Reproducer: `payout_tests::c3_a_departed_stake_is_paid_to_whoever_took_the_chair` in
`$SCRATCH/c3repo`; `principal_of` there is byte-identical to the shipped tree (diff is empty).
Both ingredients are individually reachable through the public API against the real canister in a
single 1,200-step fuzz run (`refunded … to the escrow of …`, and 296 `WARNING: seat … carries both
a live stake and a departed stake` lines from E-36).

**Fix** carry the owner with the money instead of re-deriving it from the seat, and add one gate
that asserts on **principals**. Full write-up: [SECURITY-FINDINGS.md FINDING 13](SECURITY-FINDINGS.md).

<a id="e-38"></a>
### E-38 — high — two persisted fields that are not Candid-compatible additions, one of them silent — FIXED

> **FIXED 2026-08-04 (wave 3).** Both fields are `opt` as of one change:
> `PersistentState::deposit_watermark: Option<u64>` and
> `TableState::departed_stakes: Option<Vec<DepartedStake>>`. The gate is
> `m7_state_written_by_the_previous_release_survives_the_upgrade_exactly`, which builds `801aa79`
> from git, creates escrow + seated chips + a live hand on it, and REQUIRES the upgrade to
> succeed with every balance, stack, hole card, board card and the anti-replay record intact.
> Executed both regressions: reverting the watermark gets the upgrade REJECTED; reverting
> `departed_stakes` gets it ACCEPTED with 594,000,000 e8s of seated chips silently destroyed.
> `pre_upgrade` now TRAPS on a failed `stable_save` rather than proceeding. Full write-up:
> [SECURITY-FINDINGS.md FINDING 14](SECURITY-FINDINGS.md).

`PersistentState::deposit_watermark: u64` (added by the E-02 fix) and
`TableState::departed_stakes: Vec<DepartedStake>` (added by the E-05 fix). Both carry
`#[serde(default)]` and both comments claim that makes them backward-compatible. **Candid does not
honour `serde(default)`** — only `opt` may be added to a record and still read older state.

* the first is a top-level field, so `stable_restore` fails and the upgrade is **REJECTED**. Loud,
  and therefore safe.
* the second is nested inside `opt TableState`, and Candid decodes an `opt` whose inner value
  cannot be read as **null**. So it makes the whole table arrive as `None`, `post_upgrade` re-inits
  from config, and **every seated player's chips and the live pot are destroyed silently.**

The first mistake hides the second. **Fixing only the first — the remedy the deposit reviewer
recommends and proved works — turns a rejected upgrade into silent chip destruction.** Measured
with candid 0.10.20, the exact version the canister links:

```
SHIPPED  (u64 watermark + vec departed_stakes): REFUSED  ... field_name: Named("deposit_watermark")
HALF-FIX (opt watermark + vec departed_stakes): DECODED  { ..., table_state: None }   <-- silent
OPT-BOTH (opt watermark + opt departed_stakes): DECODED  { ..., table_state: Some(...) }
```

**Executed against the real canister, not only inferred from a decode probe.**
`m7_an_upgrade_from_the_previous_release_never_silently_loses_funds` builds the `801aa79` table
canister from source, installs it on PocketIC with the real ICP ledger, seats two players, and
performs a real `install_code --mode upgrade` to the module under test:

* as shipped: `Subtyping error: field deposit_watermark is not optional field`, upgrade REFUSED,
  escrow 600000000 and seated chips 400000000 both intact;
* with ONLY `deposit_watermark` changed to `Option<u64>`: upgrade **accepted**, and
  `an accepted cross-version upgrade DESTROYED SEATED CHIPS: 400000000 -> 0`. Escrow survived, so
  the loss looks partial and plausible rather than obviously catastrophic.

**Partly mitigated in this pass, not fixed.** `PersistentState` now also carries a flat-scalar
`opt` digest of the table (`table_was_present`, `table_pot_at_save`, `table_seated_chips_at_save`)
and `post_upgrade` panics if the digest says a table was saved and `table_state` came back null, or
if the restored pot or seated-chip total disagrees with the digest. Flat `opt` scalars are the one
shape Candid cannot silently drop, so that turns **any** future nested-field mistake into a
rejected upgrade instead of destroyed chips.

The guard's limit, stated plainly: it can only fire when the SAVED state carries the digest, and
`801aa79` state does not. It does nothing for the old-to-current upgrade above (M7 is what catches
that) and everything for upgrades from this commit onwards.

**The fix itself — both fields to `opt`, in one change — is NOT done.** H-16 is now done (M7).
See [SECURITY-FINDINGS.md FINDING 14](SECURITY-FINDINGS.md).

<a id="t-08"></a>
### T-08 — high — FIXED IN WAVE 3 — the table displayed the pot at twice its size during every betting round

`state.pot` already includes the live `current_bet`s, and the table header added them again:
the screen read `POT 220.40 (110.20 + 110.20 betting)` while the pot-odds strip on the *same
screen* read `Call 69.80 to win 110.20`. Found by the frontend reviewer, in a scene that recorded
`verified: true` and quoted the wrong number in its own notes.

A client that custodies real ICP and shows the pot at 2× is a defect on its own terms. It is also
the strongest argument for the harness change in the same area: every screenshot scene asserts
**presence** of an element, never **agreement** with `get_pot()`.

**Fix (wave 3).** The canister raises `state.pot` in the same statement that raises a player's
`current_bet` — blinds at `src/table_canister/src/lib.rs:2742`/`2754`, Call `3270`, Bet `3289`,
Raise `3323`, AllIn `3356` — so the chain's pot is the WHOLE pot and the client had nothing to add.
`PokerTable.svelte` now renders `totalPot = pot`, labelled `Total pot`. The two figures a player
actually wants are published as a `.pot-breakdown` decomposition (`X collected + Y betting`) which
must sum back to `get_pot()`, and the pot-odds strip carries a `.pot-odds-explanation`
(`Call X to win Y`) quoting the same pot.

**How it stays fixed.** The harness change this defect argued for exists and now has three
assertions pointed at this component: `pot (headline) vs get_pot()`, `pot breakdown sums to
get_pot()` with its `betting` leg equal to `sum(current_bet)`, and `pot-odds "to win Y" vs
get_pot()`. Re-running `./scripts/dev.sh shots` against the defect reproduces it as a hard failure
— that is how it was found again on 2026-08-05: *pot (headline) vs get_pot(): DISAGREES: screen
"0.40" means 39500000 e8s..40500000 e8s, canister says 20000000 e8s (screen is 2.000x the chain)*.
The pre-flop scene went from 6 money figures with one mismatch to 8 money figures, all agreeing.

<a id="t-09"></a>
### T-09 — medium — FIXED IN WAVE 3 — the showdown rendered an unwrapped Candid `opt` twice

`table_3.did.d.ts` declares `'hole_cards': [] | [[Card, Card]]`, so `hole_cards[0]` is the whole
tuple and `hole_cards[1]` is `undefined`; `PokerTable.svelte` passed both to `<Card>`, so
the villain's revealed hand was two blank rectangles. Separately `formatHandRank` did
`Object.keys(handRank)[0]` on an `opt` array, producing the literal string `0` where the winning
hand's name belongs. The hand-history modal on the same table prints `(Pair)` correctly, so the
data is there.

**Fix (wave 3).** `revealedHole(player)` unwraps `Option<(Card, Card)>` properly (and tolerates a
flattened `vec Card` shape, so a Candid change degrades to a correct render rather than back to two
blanks); `handRankWords()` unwraps the rank option. Confirmed on the real showdown capture: the
villain's hand rendered as one blank white card and one empty slot before, and as its actual two
cards after. A second defect surfaced by the fix was fixed with it — a revealed pair was clipped by
its own nameplate, legible only down to the suit pips, so face-up cards now lift clear of the plate
(`.player-cards.shown`) while face-down ones still tuck behind it.

<a id="h-16"></a>
### H-16 — high — the harness never upgrades across a version boundary

`World::upgrade` reuses `self.table_wasm`, so every "survives an upgrade" assertion in the suite
installs the new wasm and upgrades it to **itself**. Same Candid type on both sides of the wire, so
a record-field addition is never tested as an addition. That is exactly why E-38 shipped, and why
E-10 (`PENDING_WITHDRAWALS` not persisted at all) has never been demonstrated either way.

**FIXED IN THIS PASS.** `wasms::previous_release_table_canister()` builds the `801aa79` table
canister by `git archive`-ing that ref into `target/money-safety/` and compiling it there (a
fixture nobody can rebuild rots into a story about a lost file), `World::reinstall_module` and
`World::upgrade_to_module_under_test` walk state across the version boundary, and
`m7_an_upgrade_from_the_previous_release_never_silently_loses_funds` asserts the property that
actually matters: **either the upgrade is refused, or every e8 is still there afterwards.**

It is green today (refused) and green on a correct fix (accepted, funds intact), and RED on the
dangerous middle case. Proven to convict: with only `deposit_watermark` made `opt`, M7 reports
`an accepted cross-version upgrade DESTROYED SEATED CHIPS: 400000000 -> 0`.

A second face of the same gap, found while writing it: this harness's own `TableState` mirror is
version-locked. `world.snapshot()` cannot even READ the previous release, failing with
`wire_type: null, expect_type: vec record {...}` on `departed_stakes`. M7 therefore measures with
scalar queries (`escrow_all`, `admin_get_table_chips`, `get_pot`). Any future cross-version test
has the same constraint.

<a id="h-17"></a>
### H-17 — high — FIXED IN THIS PASS — the fund-theft gate was outside the gate

`tests/money_safety/tests/deposit_replay.rs` holds the E-02 theft reproducer and the ten
regressions that keep it shut. It is a cargo **auto-discovered** test target, and for the whole of
wave 2 no `scripts/dev.sh` target named it: `cmd_test` ran `--test invariants`, `--test
regressions` and `--test fuzz` only. The project's only proven fund-theft primitive had its
regression suite run by nobody.

Now named explicitly in `cmd_test`, so a rename fails the gate rather than silently removing it.

<a id="h-18"></a>
### H-18 — high — `dr02` never presents the deposit block it is named after

`MAX_DEPOSIT_VERIFICATIONS_PER_MINUTE` is 5. `dr02`'s sweep loop advances the clock by 61 s when it
is rate-limited but then moves to `b + 1` **without retrying `b`**. In that fixture the ICRC-2
deposit block is deterministically 5 — the 6th iteration, the one that gets rate-limited. So the
named regression on the double-credit primitive skips the only index that matters.

Proved by reversal by the deposit reviewer: with both defences removed, `dr02` still reported `ok`;
adding a four-line retry to the identical build made it fail with `DOUBLE CREDIT (E-02) … Deposit
block was 5`. `dr09` is the only real gate on that mechanism today.

**Fix** retry the same index after advancing the clock. Four lines.

<a id="h-19"></a>
### H-19 — medium — the register's own safety mechanism does not exist

`tests/money_safety/src/documented.rs` documents `register_entries_are_all_still_needed` as the
test that fails once a register entry stops being hit, "so a fix cannot quietly leave stale
tolerance behind". `grep -rn register_entries_are_all_still_needed tests/` returns nothing.

Currently harmless — the register is `&[]`, so there is nothing stale to police — and that is
exactly when to add it, before the first entry goes back in.

<a id="h-20"></a>
### H-20 — medium — FIXED IN THIS PASS — a live defect's self-report was moved out of the detector's view

`check_self_reported_inconsistency` matches `BUG:` and `CRITICAL:` only. The payout fix wrote
E-36's dual-stake condition as `WARNING: seat {} carries both a live stake and a departed stake`,
which matches neither, so 296 occurrences of a real open defect executing against the real
canister were reported as `0 documented finding(s)` in one 1,200-step fuzz run.

This is the H-03 pathology the register exists to prevent: tolerance living in a string the
classifier ignores, where nothing can police it. Fixed by matching `WARNING:` as well and naming
that one line in `TOLERATED_SELF_REPORTS`, so it is counted, visible, and deleted when E-36 is
fixed.

<a id="h-21"></a>
### H-21 — low — the harness's build of the canister is toolchain-sensitive

`wasms.rs::build_table_canister` shells out to `cargo` with the **inherited environment**. A
parent `cargo` exports `RUSTUP_TOOLCHAIN`, and that overrides `rust-toolchain.toml` even though the
child's `current_dir` is the repo root. Running the harness from a crate that resolves to a
different toolchain built module `7babe62428943bf9…` from source that `./scripts/dev.sh wasm`
compiles to `395696359b6d1a6e…`.

It agrees whenever the Makefile is the caller, and the harness does verify that the installed
module hash equals what it built, so H-01 is genuinely closed. But "the sha256 this harness prints
is the sha256 that gets deployed" is currently a property of the invocation, not of the code.
**Fix** `.env("RUSTUP_TOOLCHAIN", "1.90.0")` on that `Command`, or read the channel from
`rust-toolchain.toml`.

<a id="e-30-correction"></a>
### E-30 — correction — the incomplete-all-in rule as first implemented was not the rule

The fix set `action_is_closed_to_raising = player_has_acted && player_current_bet <
state.current_bet`, i.e. "closed if facing anything at all". Both rulebooks say "closed only if
**not** facing at least a full bet or raise", and both spell out the cumulative case:

* TDA 2022 Rule **47-A**: "An all-in wager (or cumulative multiple short all-ins) totaling less
  than a full bet or raise will not reopen betting for players who have already acted and are not
  facing at least a full bet or raise when the action returns to them."
* Robert's Rules, no-limit: "Multiple all-in wagers, each of an amount too small to qualify as a
  raise, still act as a raise and reopen the betting if the resulting wager size to a player
  qualifies as a raise."

The comment block above the guard states this correctly and the line below it implemented something
else. The cited rule number was also wrong (41, not 47-A). **Corrected in this pass** to
`amount_owed < state.min_raise`, which is the rule as written: `state.min_raise` is deliberately
NOT advanced by an incomplete all-in, so it still holds the last full raise increment, and
`amount_owed` is what the player faces when the action returns.

Note this was a **regression** against `ceacc37` on the cumulative case specifically: the old code
set `should_reset_acted` unconditionally, which happens to be right when the shorts add up to a
full raise. Net across cases the wave-2 fix was still better; it was differently wrong, not simply
better.

`two_successive_incomplete_all_ins_still_do_not_reopen_the_betting` asserts the **false** general
rule by name, and its fixture is one chip short of discriminating (seat 0 owes 90 against a 100
increment), so it passes under both the wrong rule and the right one. Renaming and re-fixturing it
is outstanding.

<a id="e-31-correction"></a>
### E-31 — correction — resolving the stale timer before the whose-turn check let a wager cross streets

The E-31 fix moved `resolve_expired_action_timer` **before** the whose-turn check, which is right —
it is what lets any player's message unwedge the table. But the action then continued to be
*applied*, so a message sent out of turn on the flop could be accepted and applied on the turn:
measured by the betting reviewer as seat 0 pre-firing `Bet(500)` out of turn on the flop and
landing it on the turn (`Ok(())`, phase `Turn`, `current_bet` 500, pot 700) — 500 chips committed
to a street whose card the player had not seen. Before the fix it was refused with `Not your turn`.

**Corrected in this pass**: the resolver still runs first, so the table still unwedges, but if
resolving the stale timer changed the street the action is **refused** rather than applied. The
player re-sends against the board they can actually see.

<a id="h-22"></a>
### H-22 — medium — the settlement oracle's exact-deal search is unbounded, so a degenerate deck HANGS it

`tests/settlement` reaches its hard cases by rewinding the canister to a snapshot and re-dealing
until the deck gives it the hand a scenario needs (97 attempts in a normal run). The search has no
attempt cap. If the deck ever stops varying, the search can never succeed and the test **spins
forever** instead of failing.

Reached during this pass's mutation sweep, by the mutation that deals every hand from the constant
seed `b"CONSTANT"` — which is exactly the mutation the wave added `seam_c_the_deck_is_different_every_hand`
to catch. `invariants` convicted it in 22 s. `tests/settlement` and `tests/disagreements` then hung:
`settlement-…` alive with a log file that had not grown in 20 s, `disagreements-…` the same over
45 s. Both had to be killed by hand for the sweep to continue.

In CI this is worse than a failure, because a hang looks like a slow job and burns the whole
runner budget. **Fix** cap the re-deal attempts (a few hundred), and fail with "could not reach
scenario X in N attempts; the deck may not be varying" — which is itself a useful signal, since a
non-varying deck is a fund-safety defect.

Note for reading the mutation table: S7's `settlement` and `disagreements` cells are `HUNG`, not
`convicted`. `invariants` is what killed that mutant.

---

<a id="wave-3-queue"></a>
## Wave 3 queue — ranked, with reasoning

Written by the wave-2 coherence pass. Ranked by **what a player loses if it is not fixed**, then by
what unblocks the most other work. The lead's question was whether the engine and trust work is
solid enough to move to look-and-feel. **Answer: not quite. Two items first. They are both small.**

### Do these two before anything else — BOTH DONE (wave 3)

Both landed together, as one design, because they are coupled: FINDING 13's fix works by carrying
an owner in persisted state, and FINDING 14 is about persisted state not surviving an upgrade.

**1. ~~E-38 / FINDING 14 — make both persisted fields `opt`, in ONE change.~~ DONE.**
`PersistentState::deposit_watermark: Option<u64>` and
`TableState::departed_stakes: Option<Vec<DepartedStake>>`, in one change, because fixing the first
alone is measured to accept the upgrade and destroy every seated chip while reporting success.
`m7_state_written_by_the_previous_release_survives_the_upgrade_exactly` was rewritten so a
REJECTED upgrade is now a FAILURE rather than an acceptable outcome: it builds `801aa79` from git,
puts escrow, seated stacks and a LIVE hand on it, and requires the upgrade to succeed with every
balance, stack, hole card, board card, deck cursor, shuffle commitment and the deposit anti-replay
record intact — then plays the restored hand out and requires it to settle conserving. Both
regressions were executed: the watermark reverted gets the upgrade rejected, `departed_stakes`
reverted gets it accepted with 594,000,000 e8s of chips and three players' hole cards silently
gone. `m7b` covers the field `801aa79` cannot write, by round-tripping a departed stake and its
owner through a same-version upgrade.

*The scenario-specific `notify_deposit` failure the previous draft of this item flagged is
explained and closed: it is FINDING 06 in the PREVIOUS release, not a regression. `801aa79`
declares the ledger's `AccountIdentifier` as a record with a `hash` field where the real ledger
returns a bare `blob`, so the OLD build cannot decode `query_blocks` at all. The fixture now
transfers before the upgrade and claims after it, which is a better test anyway: it shows the
anti-replay record surviving a second upgrade.*

**`pre_upgrade` now TRAPS on a failed `stable_save`** instead of logging and letting the upgrade
proceed. Proceeding either gets rejected by `post_upgrade` anyway, or silently restores an OLDER
snapshot — rolling back balances, chips AND `verified_deposits`/`deposit_watermark`, which
re-opens the E-02 replay window on blocks already credited. The argument is written out in
[SECURITY-FINDINGS.md FINDING 14](SECURITY-FINDINGS.md#the-pre_upgrade-decision-it-traps-now-and-here-is-the-argument).

**2. ~~E-37 / FINDING 13 — carry the owner with the stake, and add ONE gate that asserts on
principals.~~ DONE, and the defect was DEMONSTRATED first.** The two ingredients compose in five
ordinary API calls, which wave 2 had not shown: a player leaves mid-hand, a stranger takes the
chair and calls `sit_in()` (E-36) which keeps `count_active_players` above one while every real
player walks out, and the hand then settles with no live claim on any layer. Observed on the real
canister: `-2000000` from the departed player, `+2000000` to the stranger, every total balancing.

The fix carries the owner: `Stake { seat, owner, amount, relinquished }` is the payout basis,
`Payout::principal` is a plain `Principal` sourced from the stake or from the live claim that won
the layer, and `principal_of(state, seat)` is **deleted** — there is no function left in `lib.rs`
that can turn a seat index into a payee.

The gate matters as much as the fix, and this is where the previous draft of this item was right:
every measurement instrument in this repo was seat-indexed or aggregate. There are now four that
are not, and all four are red against the pre-fix build:

* `payout_tests::finding13_*` — the pure plan and its application, on principals, host-side.
* `M8_PRINCIPAL_ATTRIBUTION` (`tests/money_safety/src/invariants/attribution.rs`) — per-principal
  `escrow + chips` delta against what the rules owe that PERSON, plus the corollary that needs no
  oracle at all: **a principal who staked nothing in a hand cannot come out of it richer.**
* `principals::m8_*` — the composed sequence against the real canister.
* a PRINCIPAL column in the settlement oracle, folded into `HandComparison::agrees` so
  `Bench::gate` fails on it, plus two chair-swap scenarios that exercise it.

### Then, in this order

**3. T-08 — the pot is displayed at 2×.** Cheap, and it is the one defect on this list a player
would notice in the first thirty seconds. Pair it with the harness change that makes it stay fixed:
every screenshot scene that shows money must assert the rendered number equals `get_pot()` on the
same table at the same moment, instead of asserting an element exists. That single policy change is
what turns wave 3's screenshots from decoration into evidence, and it is a prerequisite for
trusting anything the look-and-feel loops report. **T-09** (blank villain cards, winning hand
rendered as `0`) is the same afternoon's work and the same unwrapped `opt`.

**4. E-36 — a player who takes a chair mid-hand is dealt the action.** They can bet into a hand
they hold no cards in and can never win that money. It is also half of E-37's exploit path, and
fixing it deletes the one entry now in `TOLERATED_SELF_REPORTS`.

**5. H-18 — make `dr02` actually present the deposit block.** Four lines. The named regression on
the project's only fund-theft primitive is vacuous today; `dr09` is carrying it alone.

**6. E-06 and E-07.** One lull runs the whole board out and settles (both timeouts are 30 s at
`table_1`), and the documented recovery tool strands every seated player's chips. Neither is new,
both are real, and E-07 is what the E-31 fix had to argue around.

**7. H-22 — cap the settlement oracle's re-deal search.** Now that the oracle is in the default
gate, its ability to hang instead of fail is a CI-budget hazard. The timeout wrapper in
`cmd_test` contains the damage; the cap is the fix, and "the deck may not be varying" is a useful
failure message in its own right.

**8. Put a PocketIC job in CI.** Today `.github/workflows/ci.yml` runs the wasm build,
`cargo test --workspace`, the wasm32 golden replay, Candid drift and the frontend build. It runs
**neither the money-safety suite nor the settlement oracle**. Every fund-safety result in this
document comes from a harness no CI job invokes. `./scripts/dev.sh test` now runs both, so the CI
change is one job that calls it.

**9. H-21 — pin the toolchain in `wasms.rs`'s `cargo build`.** One `.env()` call. Makes "the sha256
the harness attests to is the sha256 that gets deployed" a property of the code rather than of how
you happened to invoke it.

**10. E-30's test name.** `two_successive_incomplete_all_ins_still_do_not_reopen_the_betting`
asserts a rule that is false, and its fixture is one chip short of discriminating so it passes
either way. The rule itself is fixed and pinned by
`tda_47a_cumulative_short_all_ins_must_reopen_the_betting_to_seat_0`; what is left is a name that
will actively mislead the next reader.

### What can start in parallel right now

The look-and-feel work on **layout, type, colour, motion and the felt geometry** has no dependency
on any of the above and can begin immediately. What must NOT start before items 1-3 is anything
that changes **what numbers the client displays or how money is presented**, because the harness
cannot currently tell a right number from a wrong one, and shipping a prettier client that shows
the pot at 2× is worse than shipping the current one.

<a id="h-23"></a>
### H-23 — high — CI runs none of the fund-safety harnesses

`.github/workflows/ci.yml` has five jobs: build the canisters for wasm32, `cargo test --locked
--workspace`, the `poker_core` wasm32 golden replay (plus the outsider verifiers), Candid interface
drift, and the frontend build. That is a genuinely good set — the wasm32 job in particular is what
would have caught FINDING-02 — but note what is absent:

* the money-safety suite (`invariants`, `regressions`, `deposit_replay`, `fuzz`), and
* the settlement oracle.

**Every fund-safety claim in `WAVE-02.md` and `SECURITY-FINDINGS.md` comes from a harness that no
CI job invokes.** They run when a human remembers to run them. The wave-2 evidence that this
matters: `deposit_replay` was in no target at all (H-17) and nobody noticed for a whole wave, and
the settlement oracle — the only gate that convicts a wrong-seat payment — was behind its own
`make settlement`.

Both are PocketIC-based, so they need a job that can fetch the pocket-ic binary and the pinned ICP
ledger wasm. `./scripts/dev.sh test` now runs both, so the CI change is one job that calls it.

<a id="e-39"></a>
### E-39 — medium — `leave_table` reduced `state.pot` without rebuilding `state.side_pots`

FIXED IN THIS PASS. Found by running `./scripts/dev.sh fuzz` at its shipped settings, which the
wave that introduced the defect had not done: the payout work reported "3 seeds x 600 steps, 0
blocking findings", and the default is **9** seeds x 600. Seed 5 is red.

```
money-fuzz: seed 0xc1b1576c1105 finished: 11 hands, 4 upgrades, 2 blocking finding(s),
            0 documented finding(s), worst stranded 2000000 e8s
money-fuzz: shrinking NEW violation M1b_POT_BREAKDOWN:side_pots_sum_to_pot|BreakdownDrift|PreFlop
```

with the sequence right above it in the canister log:

```
returned uncalled bet of 196000000 to seat 1 in hand 2
seat 1 left hand 2 with 2000000 in the pot; the stake stays in the payout basis
```

**Cause.** The payout fix added an uncalled-bet return to `leave_table`. `return_uncalled_bet`
REDUCES `state.pot`. Both of the other two `return_uncalled_bet` call sites are immediately followed
by `refresh_side_pots`; this one was not, so `state.side_pots` kept the layering it had been built
against the larger pot and stopped summing to it.

**No money moved wrongly.** `plan_payouts` rebuilds the layering from `hand_contributions` and never
reads `state.side_pots`, which is exactly the property E-03's fix was for. What was wrong is the
breakdown a player is SHOWN: `get_table_view` returns `state.side_pots`, so the side pots on screen
did not add up to the pot on screen. The money-safety harness classifies it `BreakdownDrift` and
blocks, which is right — an engine whose two accounts of the pot disagree has to be believed about
neither until you know why.

**Fix** `refresh_side_pots(state)` after `record_departed_stake`, ordered that way deliberately: run
before the stake is recorded and the rebuild would orphan it, which is E-05 again.

**Two lessons worth more than the fix.** First, `./scripts/dev.sh fuzz` at its DEFAULT settings is
the gate, and a wave that runs a third of it and reports zero findings has not run the gate. Second,
depth matters as much as breadth: at `MONEY_FUZZ_STEPS=1200` seed 1 alone reported **6** blocking
findings against the pre-fix build, so the reviewer who claimed the fuzz is red at 1,200 steps was
right, and right about the default too.

---

## Found by the wave-3 coherence pass

Wave 3 restyled four surfaces and rewrote the payout path, in parallel, blind to each other.
Everything below was found by **walking the whole app once** in a real browser against the real
local canisters — land, sign in, deposit, sit, play a hand to showdown, open the history, verify
the shuffle, withdraw — which no single builder did, plus one measurement tool applied identically
to every surface. Reproduce with the commands on each entry.

| # | sev | status | where | one line |
|---|---|---|---|---|
| [T-10](#t-10) | **high** | **FIXED IN THIS PASS** | `PokerTable.svelte` `$effect` | `JSON.stringify` on a Candid `nat64` threw **19 uncaught TypeErrors in one ordinary hand**, killing the action log and starving the effects the fairness panel and hand history run on |
| [T-11](#t-11) | **high** | **FIXED IN THIS PASS, now gated** | `+page.svelte` `.current-table-name` | the largest string on every table screen quoted the LOBBY's stale blinds: `6-Max - 0.01/0.02` on a table charging 0.05/0.10 and `9-Max - 0.01/0.02` on one charging 0.10/0.20. 18 scenes were filed "agrees with chain: yes" around it |
| [E-40](#e-40) | **high** | open | `record_hand_to_history` (`lib.rs:871`) | the only unguarded `evaluate_hand` call left in the canister. Observed **trapping on the live local canister** during ordinary browser play: `IMPOSSIBLE HAND … got 0 community`. A trap here cannot settle the hand |
| [T-14](#t-14) | **high** | open | `tools/shots/lib/frontend-build.mjs` `buildEnvFor` | the deployed local frontend points its agent at **127.0.0.1:4943** while the gateway is on 8077, so the app only works behind the screenshot harness's own shim. Opened in a plain browser it shows a raw fetch stack trace and "The lobby canister is reporting no tables" |
| [H-24](#h-24) | **high** | open | `tools/shots/scenarios/shuffleproof.mjs` | the fairness scene asserts `.proof-item >= 2`, which is true **before** the verification runs. Both shipped shuffleproof PNGs show rungs 3 and 4 grey and "Re-deriving your cards locally", filed as verified |
| [T-16](#t-16) | **high** | open | the whole client at 390×844 | the mobile playing surface is **19.7–21.1% of the screen against PokerNow's 52.1% on the identical device**, and two of the six mobile captures do not contain a poker table at all |
| [D-03](#d-03) | medium | **partly fixed** (vocabulary landed) | every component `<style>` block | 16 border radii, 28 font sizes, 9 greens, 6 ambers, 11 greys, 8 panel tints, 8 panel strokes; four buttons in one header row with three heights, two radii, two font sizes and two accent families |
| [T-15](#t-15) | medium | **partly fixed** (first consumer) | `lib/utils.js` `formatTokenAmount` | the "canonical money layer" written this wave, documented at length, was imported by **nobody**. Seven copies of "divide by 1e8", not one |
| [T-12](#t-12) | low | **FIXED IN THIS PASS** | `PokerTable.svelte:769` | the MAIN pot was labelled `Side 1`, and on a single-layer pot it printed `SIDE 1 0.40` directly under `TOTAL POT 0.40` |
| [T-13](#t-13) | low | **FIXED IN THIS PASS** | 3 of 4 dialogs | Escape closed `HowItWorks` and silently did nothing in `DepositModal`, `WithdrawModal` and `HandHistory`; each carried a keydown handler on a `tabindex="-1"` backdrop that nothing can focus |
| [H-25](#h-25) | medium | open | `./scripts/dev.sh known-defects` | one marker, for one low-severity defect. Twelve open engine defects in this register have none |
| [T-18](#t-18) | **high** | **FIXED IN THIS PASS** | `WithdrawModal.svelte:74` | the withdrawal confirmation printed the ledger **block index** as an ICP amount, and never stated the fee. Measured: withdrawing 1 ICP at block 1130 said "0.0000 ICP sent to your wallet" |
| [T-17](#t-17) | low | **FIXED IN THIS PASS** | `HandHistory.svelte` download button | the per-hand JSON export threw on the same BigInt class as T-10; fixed here, but it was never on any screen the harness photographs |

<a id="t-10"></a>
### T-10 — high — FIXED IN THIS PASS — one uncaught BigInt threw 19 times a hand and starved three features

`PokerTable.svelte` keyed the action feed on

```js
const key = `${lastAction.seat}-${lastAction.timestamp}-${JSON.stringify(lastAction.action)}`;
```

`lastAction.action` is a Candid variant carrying `nat64` amounts, which `@dfinity/agent` decodes to
`BigInt`. **`JSON.stringify` throws on a BigInt.** The statement is inside an `$effect`, so the
throw is uncaught, the effect dies, and nothing is ever pushed into `actionFeed`.

**Measured on the real canisters, one ordinary heads-up hand at `table_2`** (Playwright counting
`page.on('pageerror')`): **7 uncaught `TypeError: Do not know how to serialize a BigInt` before any
click, 12 more from a single click on `Call 0.10`, 19 in total**, and `document.querySelectorAll('.feed-item')`
returned **0** with the `.feed-empty` "Waiting for action…" state showing throughout.

This is the mechanism behind a competitive deficit another agent recorded as a design choice.
The A/B row "5 of 5 action lines carry no amount, and PokerNow interleaves the streets" is not a
missing feature — the code writes both. With the throw removed and nothing else changed, the same
hand produces:

```
▲ 02:16  Seat 2 raised to 0.20
→ 02:16  Flop
☎ 02:16  You called 0.10
```

amounts and street markers included.

**Fix.** `actionKey()` builds the identity from the variant tag plus its amount, as strings, and
never touches JSON. After the fix, on the same hand shape: **0 page errors before the click, 0
after**, three feed items with amounts.

**Reproduce** (before/after, same canisters, bundle served from disk so nothing is deployed):
`SHOTS_SERVE_DIST=src/cleardeck_frontend/dist node …` — the harness's own A/B mode.

**Not the whole story of the fairness panel.** With the table quiet (both seats sat out after a
completed hand) the panel auto-verifies in **505 ms untouched, 0 page errors** — measured twice,
with and without a BigInt-safe `JSON.stringify` shim, 502 ms and 505 ms, so on THAT state the
throw was not what stopped it. What stopped the shipped screenshot is H-24, below.

<a id="t-11"></a>
### T-11 — high — FIXED IN THIS PASS — the table header priced the table 5× and 10× wrong

`init_microstakes_tables` (`src/lobby_canister/src/lib.rs:258–330`) writes `1_000_000 / 2_000_000`
into **all three** ICP table records and bakes those blinds into the NAME string, while `icp.yaml`
initialises `table_2` at `5_000_000 / 10_000_000` and `table_3` at `10_000_000 / 20_000_000`.
Verified directly on the chain:

```
lobby  get_tables      -> "9-Max - 0.01/0.02", "6-Max - 0.01/0.02", "Heads Up - 0.01/0.02", all 1_000_000/2_000_000
table_2 get_table_view -> small_blind = 5_000_000   big_blind = 10_000_000
table_3 get_table_view -> small_blind = 10_000_000  big_blind = 20_000_000
```

The lobby LIST already refuses to quote that record: it renders the STAKES column from the table
contract and flags the row "⚠ record differs". The **table page did not**. `+page.svelte` rendered
`{currentTableInfo.name}` verbatim, so the biggest teal string on the screen read `6-Max -
0.01/0.02` seven hundred pixels from blind discs reading 0.10.

**Why no gate caught it.** `.current-table-name` was on every table scene and in no scraper. The
evidence agent's own critic proved this dynamically by rewriting the pill to `9.99/19.98` and
watching the run still print "14 money figures on screen all equal the canister's" and file the
canonical PNG.

**Fix (client).** The header keeps the FORMAT half of the lobby name — that part is true — and
reads the blinds from the contract that will charge them, through `formatTokenAmount()`. Before the
view arrives it shows the format alone rather than an unchecked number. Measured after the fix,
against `table_2`: pill `6-Max · 0.05/0.10`, chain `5_000_000 / 10_000_000`.

**Fix (gate).** `dom-scrape.mjs` now reads `.current-table-name`; `chain-agreement.mjs` asserts any
blinds it quotes against `view.config`. The pre-flop scene went 8 → 10 money figures and
table-facing-bet 14 → 16. A `headerstakes` fault-injection target was added and **fires**:

```
SHOTS_INJECT_DRIFT=headerstakes ./scripts/dev.sh shots --scenes table-preflop --viewports desktop
  ✗ CHAIN DISAGREEMENT (2): table header pill "6-Max · 0.10/0.20" small blind: DISAGREES:
    screen "0.10" … canister says 5000000 e8s (screen is 2.000x the chain)
  written as UNVERIFIED-table-preflop-desktop.png
```

**Still open, and deliberately not fixed here.** The LOBBY ROW NAME is the same lie on a different
surface, and the lobby scene is UNVERIFIED because of it. Fixing the client there would turn the
only red scene green and hide a live backend defect. The right fix is in the lobby canister:
`init_microstakes_tables` must take the configs it is registering, or read them from the table
canisters, instead of hardcoding table_1's. Until it does, the red scene is doing its job.

<a id="e-40"></a>
### E-40 — high — the one unguarded `evaluate_hand` left, and it traps on the live canister

`poker_core::evaluate_hand` was deliberately made to **trap** on a board that is not 3–5 cards
(E-09, wave 2). Wave 3's payout rewrite guarded two of the three call sites in the canister:

```rust
// lib.rs:4090  rank_claims
if !(3..=5).contains(&state.community_cards.len()) { return Vec::new(); }

// lib.rs:4503  the winner record
PayoutReason::PotShare { .. } => shown
    .filter(|_| state.community_cards.len() >= 3)
    .map(|cards| evaluate_hand(&cards, &state.community_cards)),
```

The third has no guard at all:

```rust
// lib.rs:871  record_hand_to_history
let show_cards = went_to_showdown && !p.has_folded;
…
final_hand_rank: if show_cards {
    p.hole_cards.as_ref().map(|cards| evaluate_hand(cards, &state.community_cards))
} else { None },
```

**Observed, not theorised.** During the coherence walk, four consecutive polls against the live
`4zfnl-5t777-77775-aaadq-cai` returned:

```
[ERROR] Failed to load table state: RejectError: The replica returned a rejection error:
  Reject code: 5
  Reject text: Error from Canister 4zfnl-5t777-77775-aaadq-cai: Canister called `ic0.trap`
  with message: 'Panicked at 'IMPOSSIBLE HAND: evaluate_hand needs a 3-, 4- or 5-card board
  (flop/turn/river), got 0 community'
```

The frontend's poll loop opens with `await tableActor.check_timeouts()`, an **update**, so a trap
on that path rolls the whole call back: the hand cannot settle and the chips stay in it. Both
guarded sites and every code path that deals a board make the empty case look unreachable —
`advance_to_next_street` and `run_out_board` both advance the phase even when their
`deck_index + N < deck.len()` guard refuses to push a card, which is the shape that gets there —
but the honest statement is that **it was reached on a real canister and I did not isolate the
minimal sequence.** A scripted heads-up timeout fold-out (the nearest shape) does not reproduce it:
that path settles through `end_hand_single_winner`, which passes `went_to_showdown = false`.

**What to do.** Guard the third site exactly like the other two — `.filter(|_| state.community_cards.len() >= 3)`.
It is a history record, not a payout, so degrading it to `None` costs nothing and a trap there
costs a wedged table. Then reproduce the reachability with the fuzzer and add the sequence as a
regression, because a hand-history writer that can abort a settlement is a fund-availability
defect even though it moves no money to the wrong place.

**Reproduce the observation**, not the minimal case: drive a browser through a hand at `table_2`
while a second identity acts from Node, with a mid-hand `join` from the browser — the walk script
is `$SCRATCH/w3-coherence/walk3.mjs`.

<a id="t-14"></a>
### T-14 — high — the app a human opens is not the app the gate tests

`ic-config.js` resolves the local agent host from `VITE_LOCAL_GATEWAY_PORT`, defaulting to 4943.
`buildEnvFor` in `tools/shots/lib/frontend-build.mjs` — the only wired build path — **does not set
it**, so the bundle that is deployed to the local asset canister hardcodes `http://127.0.0.1:4943`
while this project's gateway is pinned to **8077** in `icp.yaml`. T-03 was recorded as fixed by
"it now comes from the build environment"; the build environment never supplies it.

The screenshot harness papers over this with a reverse proxy that listens on 4943 and forwards to
8077 (`tools/shots/lib/proxy.mjs`, whose own header says the frontend source could not be edited
because "a later wave redesigns it" — wave 3 was that wave). **So every screenshot ever taken of
this app was taken through a shim no user has.**

What a person actually gets, opening `http://<frontend-id>.localhost:8077/` in Chrome — measured:

* an unstyled toast covering a third of the viewport, containing a raw stack trace with bundle
  paths and line numbers: `Failed to fetch HTTP request: TypeError: Failed to fetch at window.fetch
  (…/_app/immutable/chunks/CjFkTVZV.js:1:1669) at requestFn …`;
* `0 tables · 0 of 0 seats taken`, and an empty state reading **"The lobby canister is reporting no
  tables."** The lobby canister is reporting three. The client blames the chain for its own
  misconfiguration, on the one screen whose whole argument is "you can check this yourself".

**Fix.** Set `VITE_LOCAL_GATEWAY_PORT: String(GATEWAY_PORT)` in `buildEnvFor`. Note the
consequence before doing it: `run.mjs` would then be serving the page from `127.0.0.1:4943` while
the agent calls `localhost:8077`, i.e. cross-origin, so the shim's "same origin removes CORS from
the equation" property is lost and `run.mjs` needs to serve the app from the gateway origin
instead. That is a harness change, not a one-liner, which is why it is left for wave 4 rather than
done here. **Also fix the empty state**: distinguish "the canister answered with no tables" from
"the request never completed", and never print a bundle stack trace to a player.

<a id="h-24"></a>
### H-24 — high — the fairness gate certifies a panel that has not verified anything

```js
await page.waitForSelector('.proof-item', { timeout: 30_000 });
…
verified: panel >= 1 && items >= 2 && revealed >= 1,
```

Rungs 1 and 2 (`commitment published`, `seed revealed`) render straight from the canister's
`get_shuffle_proof()`, so `.proof-item >= 2` is satisfied **before the browser has computed
anything**. The scene never looks at the verdict. Both shipped artifacts show the consequence:
`shuffleproof-desktop.png` and `-mobile.png` have rungs 1–2 filled green, rungs 3–4 grey outline,
the subtitle stuck on "Re-deriving your cards locally", no deck grid, no derived-vs-dealt pair and
no tally — and they are filed under the canonical, verified filename.

The verification itself is real. On a quiet table it reaches its verdict in **505 ms untouched**
(`.headline.good`, "Verified on your machine…"), and the shutter simply fires first: `settle()`
waits for fonts and two animation frames, which is tens of milliseconds.

**Fix.** Wait for and assert `.headline.good`, plus the "N of N cards" tally, plus a 52-cell deck
grid. Then add the negative control the rest of this harness already has: flip a byte in the
revealed seed and require the scene to go UNVERIFIED. Counting `.proof-item` is exactly the
"presence, not agreement" failure T-08 was raised against, surviving in the one panel whose entire
purpose is agreement.

<a id="t-16"></a>
### T-16 — high — the phone is where this client loses, and it loses by more than 2×

One tool, the same hue-mask/largest-component measurer used on the reference corpus, pointed at
our captures and at PokerNow's real portrait capture at the **identical 390×844**:

| capture | felt | % of screen width | % of height | **% of screen area** |
|---|---|---|---|---|
| PokerNow `mobile-portrait-1` | 313×548 | 80.3% | 64.9% | **52.1%** |
| ClearDeck `table-showdown-mobile` | 217×319 | 55.6% | 37.8% | **21.1%** |
| ClearDeck `table-empty-mobile` | 217×316 | 55.6% | 37.5% | **20.9%** |
| ClearDeck `table-allin-mobile` | 217×299 | 55.6% | 35.5% | **19.7%** |

Ours is **38–40% of PokerNow's playing surface on the same phone.** Every other phone reference
in the corpus is at least 1.7× ours.

Worse, two of the six mobile table captures **do not show a poker table**. In
`table-preflop-mobile.png` and `table-facing-bet-mobile.png` the pot readout, all five board slots
and four of the six pods are above the top of the frame; the measurer reads `tallest_col_rel=0.000`,
i.e. the mask is flush against the top edge, and rows ~275–490 of the 844 are empty. Both were
filed VERIFIED, because the DOM assertions never look at the picture.

**The cheap half of the fix costs no protected text.** The four-notice disclaimer block is rendered
**twice** on every page — `+page.svelte:666` (top banner) and `+page.svelte:835` (footer) — 183 px
each on desktop, 235 px on the phone. Rule 2 of this project's brief says a notice may never be
weakened and may always be made more prominent; deleting the SECOND, redundant copy is a judgement
call about prominence and belongs to the lead, not to a coherence pass, so it was not done here.
It is the single largest recoverable block of phone real estate and it is worth an explicit
decision. **The other half is a real reflow**: the header is 120 px and the felt is given whatever
is left after both.

<a id="d-03"></a>
### D-03 — medium — there was no design system, so four agents each invented one

`src/cleardeck_frontend/src/index.scss` was a 57-line reset with no tokens. Everything visual lives
in thirteen per-component `<style>` blocks. Counted across them:

| axis | distinct values | detail |
|---|---|---|
| `border-radius` | **16** | 2 3 4 5 6 7 8 9 10 11 12 14 16 18 20 999 |
| `font-size` | **28** | 8 → 44 px, including 8.5 9.5 10.5 11.5 12.5 13.5 14.5 |
| "our" green | **9** | `#00d4aa` `#2ecc71` `#4ecdc4` `#7ee2b8` `#4ade80` `#22c55e` `#00b894` `#49a16e` `#7bf0d8` |
| "our" amber | **6** | `#fbbf24` `#f59e0b` `#f1c40f` `#f0b429` `#d97706` `#e9ff63` |
| muted grey | **11** | no scale between them |
| panel tint | **8** | `rgba(255,255,255,0.02 … 0.09)` |
| panel stroke | **8** | `rgba(255,255,255,0.05 … 0.15)` |

`PokerTable.svelte` does not use the brand teal **at all** — its positives are `#49a16e` and
`#7ee2b8` — so the felt and the header are two different products' greens, side by side, in every
screenshot.

Measured with `getComputedStyle` on the four buttons that sit in **one row** of the table header:

| button | font | radius | height | accent |
|---|---|---|---|---|
| `Lobby` | 14px/400 | 10px | 42 | white 5% |
| `History` | 13px/500 | 10px | 38 | **indigo** `rgb(99,102,241)` |
| `Verify Fair` | 13px/500 | 10px | 38 | **teal** `rgb(0,212,170)` |
| `ShadowDragon71` | 14px/500 | **8px** | 46 | teal |

Three heights, two radii, two sizes, two weights, two accent families, for four controls that all
mean "open a thing".

Dialog chrome, same method: panel radius **20 px** (deposit) / **16 px** (fairness) / **14 px**
(lobby panes); title **18px/700** (deposit) vs **16px/650** (hand replay) — 650 is not a weight on
any scale.

**Partly fixed.** `index.scss` now carries the vocabulary: surface, ink, accent, money, type,
rhythm, radius and motion tokens, each set to the **majority existing value** on its axis so
adopting one moves the fewest pixels. Thirteen stylesheets were deliberately NOT rewritten to use
them — that is a redesign, and this was a coherence pass. One rule is enforced globally because it
is invisible until it is wrong: money figures are now `tabular-nums`, keyed off the class names the
components already use (`.chips`, `.pot-amount`, `.bet-amount`, `.side-pot-amount`, `.stakes-value`,
`.balance-amount`), so `11.90` and `50.00` line up in adjacent seat pods.

<a id="t-15"></a>
### T-15 — medium — the canonical money layer has no consumers

`lib/utils.js` opens with a 37-line comment naming the six divergent copies of "divide by 1e8 and
round" and declaring `formatTokenAmount()` the canonical one. `grep -rn formatTokenAmount
src/cleardeck_frontend/src` returned exactly **one** hit outside `utils.js` — a comment in
`+page.svelte` telling the reader to import it. **No component imports `utils.js` at all.** Wave 3
did not reduce seven copies to one; it made a seventh.

The disagreement is live and visible in one screenshot: the deposit modal renders the wallet
balance `0.0006 ICP` and `Minimum deposit: 0.0002 ICP (Network fee: 0.0001 ICP)` at four decimals,
over a table balance reading `0.00 ICP` at two. That is wave 2's "50.00 next to 0.0000", moved.

**Partly fixed**: the header stakes pill (T-11) is now the first real consumer. **Not** fixed: the
rule itself needs stating before the rest can adopt it, because the deposit modal's four decimals
are correct — the ledger fee is `0.0001` and a two-decimal render would print a fee of `0.00`. The
honest unification is *two* documented precisions, not one function: **felt and lobby money is 2 dp
(4 dp below 0.01), wallet-and-fee money is 4 dp**, and every surface picks one and says which.

<a id="t-12"></a>
### T-12 — low — FIXED IN THIS PASS — the main pot was labelled "Side 1"

`build_side_pots_from_contributions` returns the **main** pot at index 0. `PokerTable.svelte`
rendered `Side {i + 1}`, so index 0 was named with the wrong poker word, and because the block is
rendered whenever `sidePots.length > 0` a perfectly ordinary single-layer flop printed
`SIDE 1 0.40` immediately under `TOTAL POT 0.40` — the same number twice, one of the two
mislabelled. Now `Main` / `Side 1` / `Side 2`. Verified live: `sidePotLabels: ["Main"]` on a
single-layer pot of 0.40.

The **duplication** is not fixed. Hiding the block at one layer would be right, but
`chain-agreement.mjs` asserts `dom.sidePots.length === truth.sidePots.length`, so suppressing it
would fail the gate that another agent owns. Wave 4: render the breakdown only when it says
something the headline does not, and relax the count assertion to match.

<a id="t-13"></a>
### T-13 — low — FIXED IN THIS PASS — three of four dialogs ignored Escape

`HowItWorks.svelte` binds `onkeydown` on `<svelte:window>` and Escape works from anywhere.
`DepositModal`, `WithdrawModal` and `HandHistory` each put the same handler on their backdrop:

```svelte
<div class="modal-backdrop" onkeydown={(e) => e.key === 'Escape' && onClose()}
     role="button" tabindex="-1" aria-label="Close modal"></div>
```

A `tabindex="-1"` element is not in the tab order and nothing ever focuses it, so the handler can
never fire. Three copies of dead keyboard code. Measured: Escape left all three open, and the
backdrop then **swallowed the next click on any header button** — which is how it was found, as a
Playwright timeout on "History" while the deposit backdrop was still up.

Fixed by giving all four the same contract: `<svelte:window onkeydown>` closes it, a backdrop click
closes it, the close button closes it. Verified live: `depositEsc.escapeClosed: true`,
`historyEsc.escapeClosed: true`.

<a id="h-25"></a>
### H-25 — medium — `known-defects` watches one defect

```
==> known-defect markers (expected RED until wave 2 fixes them)
    red   defect_detect_straight_returns_the_best_straight
==> result
    1 of 1 engine defects still present
```

One marker, for **E-13**, the lowest-severity entry in this register ("unreachable today"). E-06
(one timeout ends the hand for everybody), E-07 (`admin_reinit_table` strands chips), E-32
(a `Disconnected` seat stays live in the hand), E-33, E-34, E-36 and now E-40 have none. The rule
in the brief — "when you fix a defect, update its marker so `make known-defects` stays meaningful"
— cannot bite on defects that never got a marker. The target reads as a green tick over an almost
empty set.

<a id="t-17"></a>
### T-17 — low — FIXED IN THIS PASS — the hand-export button threw on the same BigInt class

`HandHistory.svelte`'s "download this hand" builds a payload containing `amount_e8s` and `won_e8s`
straight off the Candid records — `nat64`, i.e. `BigInt` — and called bare `JSON.stringify`. Same
throw as T-10, in a control no scene photographs. Now uses a BigInt replacer that writes e8s as
decimal strings, which is what a `nat64` is on the wire.

Worth noting as a pattern rather than a bug: `+page.svelte` already carries a private
`safeStringify()` doing exactly this, `HandHistory` did not know, and `PokerTable` had a third
variant of the problem. Three components, three answers, two of them wrong.

<a id="t-18"></a>
### T-18 — high — FIXED IN THIS PASS — the withdrawal receipt stated the wrong amount of money

`withdraw` is `-> Result<u64, String>` and the `u64` is the **ledger block index**
(`src/table_canister/src/lib.rs:1894`, `Ok(block)` at `:1991`). `WithdrawModal.svelte:74` did

```js
success = `Withdrawal successful! ${formatWithUnit(result.Ok)} sent to your wallet.`;
```

so the confirmation ran a block index through the ICP formatter. Measured end to end on the real
local ledger: withdrawing **1 ICP** returned block **1130**, and the old line renders 1130 e8s at
four decimals as **"0.0000 ICP sent to your wallet"**. A player who withdrew a whole ICP was told
they received nothing.

Two things were wrong on the same line. `transfer_tokens` sends `amount - fee`
(`lib.rs:1038`), so the wallet receives **less** than the amount withdrawn, and no screen said so:
the modal's only mention was "A small network fee applies."

**Fix.** Both real figures, plus the block index labelled as a block index. Measured after, on the
same path against the same ledger:

```
escrow 35.0000 -> 34.0000 ICP   (delta 100_000_000 e8s, exactly the 1 ICP requested)
"Withdrew 1.0000 ICP. 0.9999 ICP reached your wallet after the 0.0001 ICP network fee.
 Ledger block #1130."
```

**Still ungated.** No screenshot scene reaches the withdrawal *confirmation* — `deposit` is a
scene, withdraw is not — so nothing in the harness would catch this coming back. Wave 4: a
`withdraw` scene that asserts the receipt's amount against the escrow delta and its fee against
`ICP_TRANSFER_FEE`.
