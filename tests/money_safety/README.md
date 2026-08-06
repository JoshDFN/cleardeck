# ClearDeck money-safety harness

Six named money invariants, executed against the **real** table canister wasm
talking to the **real** mainnet ICP ledger wasm, on PocketIC.

```
cd tests/money_safety

cargo test --test invariants   -- --test-threads=2   # one test per invariant, M1..M6
cargo test --test regressions  -- --test-threads=2   # minimal reproducers for what was found
cargo test --test fuzz                               # seeded hostile-sequence fuzzer
```

There is also `src/table_canister/tests/money_safety.rs`, which carries the
host-checkable subset (the payout-basis arithmetic) and runs under a plain
`cargo test --workspace` with no replica.

## Why this is a detached crate

`pocket-ic` pulls in tokio, reqwest and rustls. None of that belongs in the
lockfile of a canister that custodies real ICP and ckBTC on mainnet, and it would
slow every build of the fund-holding crates. The crate therefore carries an empty
`[workspace]` table, its own `Cargo.lock` and its own `target/`, and is **not**
picked up by `cargo test --workspace`.

## The invariants

| | Name | Statement |
|---|---|---|
| **M1** | CONSERVATION | `icrc1_balance_of(table, None) + sum(icrc1_balance_of(table, deposit_subaccount(p)))` `== escrow + chips + pot + unswept_deposit_custody + the harness's own two allowances`, exactly. No chips created, none destroyed. **Both kinds of account the canister owns are on the left**; anchoring it to the main account alone is what let a canister hold 5 ICP for a player at an address it had published and satisfy M1 (FINDING 21, FINDING 28). |
| **M1b** | POT BREAKDOWN | `side_pots` sums to `pot`; and mid-hand `pot` equals the sum of the seated players' `total_bet_this_hand`, so every e8 in the middle is attributable. |
| **M2** | LEDGER REALITY | The canister's real ledger balance is never less than what it owes. It can never be short. |
| **M3** | NO RAKE | Over a completed hand, chips awarded == chips wagered. The house takes exactly zero. |
| **M4** | NO NEGATIVE / NO OVERFLOW | Nothing wraps; `saturating_*` never hides a real deficit; hostile amounts are rejected, not clamped. |
| **M5** | UPGRADE DURABILITY | A real `--mode upgrade` preserves every balance, every chip stack and the in-progress hand. |
| **M6** | NO DOUBLE PAY | A pot is awarded once; a withdrawal is paid at most once; a deposit block or allowance is credited at most once. |
| **M8** | PRINCIPAL ATTRIBUTION | The money reached the right PERSON, not merely the right seat and the right total. Includes `check_deposit_attribution`, which compares the canister's deposit-subaccount books against the ledger **one principal at a time**, because every other check here compares sums and a canister that books alice's deposit against bob's address satisfies all of them. |
| **M9** | FUND REACHABILITY | For any state a sequence of legal calls can reach, there is a sequence of legal calls by each funded player that returns that player's balance to the ledger. |
| **M14** | LEDGER/BOOKS COHERENCE | Money that has moved on the ledger is money the canister's books must either HOLD or NAME. There is no third state. `ledger_main + every deposit subaccount == escrow + chips + pot + uncredited raw transfers + open ledger-intent journal`. See `src/fault.rs`, `tests/ledger_boundary.rs` and docs/SECURITY-FINDINGS.md FINDING 29. |

**M14 is the second odd one out, and it is the only invariant here whose subject is a state
ordinary play cannot produce.** M1 is evaluated between messages, and between messages the ledger
and the books always agreed. The gap opens *inside* one message: `deposit`,
`claim_external_deposit` and `withdraw` each move real money and settle the books in the post-await
continuation, so a continuation that does not run leaves the movement standing and the book entry
undone. That state was worth 3.0 ICP of a player's money with eleven closed doors out of it, and
nothing in this harness could see it because nothing in this harness could produce it.

`src/fault.rs` produces it, without instrumenting the canister and without a mock ledger: walk the
call to its LAST await, take a canister snapshot there, let the call finish so the ledger really
performs the movement, load the snapshot back. By the IC's atomicity rule that discards exactly the
final continuation, which is what a trap discards. The same file records the four attempts at
forcing a literal trap and the measured reason each failed — read it before trying a fifth.

One fuzz run in four injects a fault this way and then makes the OWNER recover it with a
player-only call, so M14 is exercised by randomised sequences and not only by fixtures.

**M9 is the odd one out and it is the reason the list needed a ninth entry.** M1 to
M8 all ask whether the arithmetic is right. None of them asks whether the player
can still get the money out, and a table can satisfy every one of them while being
permanently frozen with funded seats. That is not hypothetical: an independent
auditor reached exactly that state in ordinary play, over about 420 ICP, and this
harness was silent — correctly, because not one chip had gone missing. **Chips
conserved inside a canister nobody can withdraw from is not safety.**
docs/SECURITY-FINDINGS.md FINDING 15. M9 is checked two ways
(`src/invariants/reachability.rs`):

* `check_no_settlement_trap`, on every fuzz step at zero extra message cost — no
  update on the settlement path may TRAP while the table holds money, because a
  trap rolls the message back, so that door is shut for that state permanently,
  and `check_timeouts` / `player_action` / `leave_table` all run settlement while
  `withdraw` / `cash_out` refuse during a hand. One trap closes all five;
* `drain`, at the end of every fuzz run and inside each reproducer — actually take
  every player's money out with player-only calls and check it lands on the
  ledger. `final_internal_total` in the fuzz report is now the answer to *"how
  much could not be got out"*.

Its severity, `FundsUnreachable`, is the one severity with a **zero delta**:
nothing has gone missing, which is precisely why everything else is quiet.

The anchor is the **ledger**, not the canister's own bookkeeping.
`icrc1_balance_of(table)` is a fact about the outside world; the escrow map and the
chip stacks are the canister's claims about it. An invariant that compared the
canister against itself would be vacuous.

M1's identity is exact because every path that moves money across the canister
boundary moves the ledger balance and the internal total by the same amount:

```
deposit(a)                main +a                escrow +a
claim_external_deposit    main +(sub - fee)      escrow +(sub - fee)
withdraw(a)               main -a                escrow -a      (a-fee out, fee burnt)
notify_deposit(b)         main unchanged now, +a earlier by a raw transfer
```

`uncredited_raw_deposits` accounts for the last line and for nothing else, which
is what makes "the canister holds more than it owes" a detectable defect rather
than an accounting excuse.

## What the harness runs against

* **Table canister**: built from the checked-out source
  (`cargo build -p table_canister --target wasm32-unknown-unknown --release`),
  or `$CLEARDECK_TABLE_WASM`.
* **ICP ledger**: the real mainnet module, installed at
  `ryjl3-tyaaa-aaaaa-aaaba-cai`, the exact id the table canister hardcodes as
  `ICP_LEDGER_CANISTER`. Pinned:

  ```
  URL     https://download.dfinity.systems/ic/6dcfafb491092704d374317d9a72a7ad2475d7c9/canisters/ledger-canister.wasm.gz
  sha256  a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec
  ```

  It is fetched on first use into `target/money-safety/` (gitignored) and the hash
  is verified before installation. `./fetch-ledger.sh` does the same thing by
  hand; `$ICP_LEDGER_WASM` overrides the path.

  It is **not** a mock. It enforces real ICRC-2 allowances, the real 10 000 e8s
  fee and real balances, and `harness_runs_the_real_ledger_at_the_hardcoded_canister_id`
  proves that before any other test is believed.
* **PocketIC server**: `$POCKET_IC_BIN`, else `$(dfx cache show)/pocket-ic`.
  Written against server 11.0.0 / crate `pocket-ic 11.0.0`.

Everything is local. The harness never touches mainnet and has no `-e ic` path.

## The fuzzer

`generate(seed, steps, config, actors)` emits a self-contained op sequence up
front, so a failing run is an artifact that can be shrunk and replayed rather than
a seed that has to be regenerated. Determinism comes from SplitMix64 plus
PocketIC's deterministic execution.

The alphabet covers every money-moving update method, plus:

* time jumps that land just past `action_timeout`, `AUTO_DEAL_DELAY_NS`,
  `RELOAD_TIMEOUT_SECS`, `SITTING_OUT_KICK_SECS` and `WITHDRAWAL_COOLDOWN_NS`;
* real `--mode upgrade` upgrades at arbitrary points, including mid-hand;
* same-round concurrent submissions (two `withdraw`s for one principal; one
  `player_action` from three principals at once);
* a player who simply stops responding for the rest of the run;
* deliberately illegal input: raises below the min-raise and above the stack,
  acting out of turn, taking an occupied seat, `u64::MAX` amounts, buy-ins outside
  `[min, max]`, withdrawing more than escrow, re-entrant withdrawals, and
  re-claimed deposit blocks.

After every step the point-in-time invariants (M1, M1b, M2, M4) are evaluated; M3
is evaluated at each hand boundary; M5 is evaluated across each upgrade.

### Known defects do not stop the run

The engine has documented fund-safety defects (see `docs/SECURITY-FINDINGS.md`).
If the fuzzer aborted on the first one it would trip inside the first hand and
explore nothing. So violations are classified by direction:

* `FundDestruction` / `BreakdownDrift`: money stranded inside the canister, and
  the stale `side_pots` breakdown that causes it. **Counted and reported**, run
  continues. Every one of these is already characterised.
* `FundCreation` / `DoublePay` / `DurabilityLoss`: the canister owes more than it
  holds, something was settled twice, or an upgrade lost state. **Not** known
  defects. The run shrinks the sequence to a minimal reproducer and fails.

### Tuning

```
MONEY_FUZZ_SEEDS=1,2,3    which seeds to run
MONEY_FUZZ_STEPS=600      ops per run
MONEY_FUZZ_SHRINK=60      candidate runs the shrinker may spend
MONEY_FUZZ_REPORT=<path>  where to write money-fuzz-report.json
```

Every step is a real IC message against a real replica: budget roughly a second
per 10 steps, plus ~2 s per PocketIC instance. The shrinker starts a fresh
instance per candidate, so a shrink budget of 60 is about two minutes.

## Reading a false positive correctly

`check_hand_payout_total` (M3 in its sharp "awarded == collected" form) reads
"collected" as the sum of the *seated* players' `total_bet_this_hand`. That is
exact only if no seat was vacated during the hand. `leave_table`, and `cash_out`
after a fold, delete the seat **and** the record of what that player put in, so
after one of those the sum understates what was collected and the check would
report money appearing from nowhere, when the truth is FINDING 05's orphaned
stake. The fuzzer therefore applies the sharp form only across hands whose seated
set did not change, and relies on `check_no_rake` (value conservation, which needs
no attribution) for the rest.

Comparing the endpoints is not sufficient either, and the fuzzer proved it: it
produced `LeaveTable{actor:2}` mid-hand followed by `FundAndSeat{actor:2}`, which
leaves the seated set identical at the two endpoints while the departure still
erased that player's contribution. `HandWatch::seat_churn` therefore tracks every
step of the hand, not just its ends.

Both of those were traps the harness fell into while it was being built, and both
are the reason the report contains no `FundCreation` findings: the measurement was
wrong, the engine was not creating chips. That distinction is the whole job here,
so it is recorded rather than quietly fixed.

## What these invariants CANNOT catch

Worth stating plainly, because a green run is easy to over-read.

M1 through M6 are conservation and settlement properties. **They were also blind to
a table that cannot be emptied**, which is what M9 exists for and what this section
said nothing about until an outsider locked 420 ICP and every one of them stayed
green. Read the rest of this section with that in mind: the list of things a green
run does not mean has been wrong before, in the direction of being too short.

M1 through M6 are blind to a pure
**redistribution** between players: if the engine pays the wrong player, the total
is unchanged and every invariant here still holds. That is exactly the shape of
FINDING 05 and FINDING 08 (a vacated seat's stake moving from the pot the short
stack could win into the pot only the deepest stack can win), and the only reason
the harness sees those at all is M1b's attribution leg, which flags the
precondition rather than the misallocation.

Catching a wrong payout needs a different oracle: an independent settlement model
that, given the hole cards, the board and the contributions, computes what each
seat is owed, and is compared against what the engine actually paid. That model
does not exist yet. Until it does, "no blocking findings" means no chips were
created, none were double-paid and nothing was lost across an upgrade. It does not
mean the right player won.

Two further boundaries:

* The ckBTC table (`btc_table_1`, `Currency::BTC`) is not covered. Its ledger is
  `mxzaz-hqaaa-aaaar-qaada-cai` and its deposit path goes through the ckBTC minter,
  neither of which is installed here. Everything in this harness is ICP.
* `notify_deposit` is currently dead (FINDING 06), so the deposit-block
  double-credit leg of M6 can only prove that nothing is credited twice while
  nothing is credited at all. `m6_a_deposit_block_is_credited_at_most_once` sweeps
  every block index after an ICRC-2 deposit specifically so that it starts failing
  the moment FINDING 06 is fixed without FINDING 10 being fixed with it.
