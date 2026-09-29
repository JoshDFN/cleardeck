# ClearDeck codebase review, 2026-09-28

Task 1790621714 ("Review entire codebase, how do we make the best possible poker software"),
executed by claude-seat-1 on main at cc84eb6. Read-only: no code changed in this task. The
work it produced is the 44 queue tasks listed in section 4; every gap below names the task
that closes it. Pointers are `path:line` on cc84eb6 and were read, not guessed.

The question was judged against what the best on-chain real-money poker software would be,
on eight lenses: game correctness and rules coverage, fairness and provability, money safety
and custody, performance and cost, player experience, operations, test strategy, and code
health. Five parallel read-only sweeps covered the Rust canisters, the SvelteKit frontend,
the test tiers, the scripts and CI, and the registers; this document is the synthesis.

The registers were the starting point. `./scripts/register-stats.sh` at cc84eb6 reports
docs/DEFECTS.md at 220 entries with 61 OPEN (1 fund-theft, 7 high, 38 medium, 15 low) and
docs/SECURITY-FINDINGS.md at 45 with 6 OPEN (1 fund-theft, 4 high, 1 low). Where a gap below
is already a register row it says so; the review's job was to find what the registers do
not hold, and to turn the open rows that have no task into tasks.

## 1. What is strong

This is not a typical AI-generated poker codebase. The parts that touch money and fairness
have been driven adversarially for fourteen waves, and it shows.

- **Settlement is derived from contributions alone.** `hand_stakes` and `plan_payouts`
  (`src/table_canister/src/lib.rs:9110`, `:9491`) are pure and host-testable, refund any
  unwinnable layer, and `apply_payouts` (`:9727`) refuses any plan that does not pay out
  exactly what it collected. One membership predicate (`is_in_hand`, `:7909`) decides
  participation and eligibility, with `hand_membership.rs` enumerating every reachable
  `Player` shape. The TDA 47-A incomplete all-in rule is pinned in both directions.
- **The ledger-intent journal** (`:2081-2600`): an intent is written before every
  irreversible ledger call, exactly-once is decided by the ledger's own dedup on memo plus
  `created_at_time`, credit is atomic take-then-credit, leases expire, ids are monotonic
  across upgrades. Deposit anti-replay uses a single writer and a monotonic watermark
  instead of forgetting. No method can set or mint a balance, and a gate keeps it so.
- **Upgrade safety in the table canister**: `pre_upgrade` traps on a failed save
  (`:13548`), `post_upgrade` panics on a failed restore and on a table digest mismatch,
  every added persisted field is `opt` (FINDING 14), a live hand's seed and deal order
  survive an upgrade, and `tests/money_safety` upgrades from a previous release.
- **The shuffle**: width-independent Fisher-Yates over a SHA-256 chain with rejection
  sampling (`src/poker_core/src/shuffle.rs`), a normative spec with a worked example,
  wasm32 golden vectors replayed by two standard-library reimplementations, and a browser
  verifier that imports nothing and calls nothing. The house's own hash checks are
  labelled as proving nothing. The no-peeking work proved vetKD does not help and measured
  its own design's costs instead of asserting them.
- **The test estate**: the real table wasm against the real pinned ICP ledger on PocketIC,
  a shrinking fuzzer that has convicted a planted 1% skim, an independent settlement
  oracle, an exhaustive evaluator differential, a screenshot harness whose token census and
  occlusion model are a real pixel gate, and `scripts/test-suites.list` as an executable
  inventory where every cargo target must be run by something.
- **Register discipline**: a closed five-word status vocabulary, every FIXED naming its
  gate, and a command that fails when that stops being true. docs/DECLARED-VS-STORED.md
  names every declared-versus-stored pair and its guard.
- **The frontend's money surfaces**: the poll is queries only (E-92) and its per-tab burn
  is pinned to the poll period; the optimistic echo reconciles against the next verified
  view; query signatures are verified; the deposit address is derived locally from
  build-pinned ids; the cashier states fees, floors and the sweep arithmetic in copy the
  harness recomputes from the ledger fee; equity is computed only at the all-in moment or
  showdown, so no live decision is informed by it.
- **Honest disclosure.** The custody model, the operator's powers and the five protected
  notices are stated on the deposit screen and in the README, and a hygiene gate refuses
  their weakening.

## 2. The gaps, ranked

Ranked by what a real-money player would lose first. Each entry: what is wrong, the
evidence, the change, and the task id. "Register" says whether a row already existed.

### Gap 1. Custody and liveness are one operator decision away, and the decision has no date

The dominant risk to a player's money is not a bug. It is that one controller key can
reinstall or uninstall a funded table (FINDING 23, E-84, fund-theft, open; measured at
40 ICP of claims destroyed by one command, and FINDING 23c shows the same verb pays the
ledger balance to the operator), that two different engines are live on mainnet today
(E-102, open, high: four instances of one package report different module hashes, so one
runs a build predating money fixes), that nothing tops a table up (E-55, open, high;
`cycles-monitor.yml:9-11`: "NOTHING IN THIS REPOSITORY TOPS ANYTHING UP"; the freezing
reserve is about 1.2 hours and a frozen canister refuses every update and, per FINDING 24,
every query), and that a free ingress flood can burn a table 65x faster (FINDING 26, open;
`check_timeouts` at `lib.rs:11011` has no rate limit).

The guardian that closes the first of these is built, gated by nine tests and not
deployed (`icp.yaml:96-122`). Its build script silently drops `--locked`
(`scripts/build-guardian.sh:57`, `:60`). There is no runbook: nothing in the repository
says what the operator does when a hand is stuck, a withdrawal is stuck, the ledger is
down, a table is out of cycles, an upgrade is bad, or the key is compromised; the only
rollback text (`.github/workflows/README.md:64`) offers the snapshot restore that
README.md:190 calls a fund-rollback hazard. Alerts open a GitHub issue twice a day and
nobody is paged. `scripts/deploy-mainnet.sh` has no dirty-tree refusal, no mandatory
snapshot, upgrades six canisters in one loop, and verifies with `grep -q "Heads Up"`
(`:115`, `:126-129`, `:185-190`); E-102's own row says "git status --porcelain must be
empty. this is how it broke."

Register: FINDING 19, 23, 24, 26; E-55, E-84, E-102; D-07. None had a task.

Tasks: 1790632451 (runbook), 1790632453 (deploy-mainnet.sh hardening), 1790632483
(guardian build `--locked` and CI reproduction), 1790632499 (operator: fleet redeploy from
one build), 1790632498 (operator: guardian handover), 1790632482 (operator: freezing
reserve and capped top-up). The per-caller limit on `check_timeouts` is in 1790632445.

### Gap 2. One ckBTC deposit can be credited twice

`verify_ckbtc_deposit` accepts any transfer whose `to.owner` is this canister
(`lib.rs:4728`) and whose `from.owner` is the caller (`:4733`); it never reads
`to.subaccount`. The ICP door compares the full 32-byte account identifier, so a transfer
to a deposit subaccount is refused there and routed to `claim_external_deposit`. The
ckBTC door has no such refusal, and `claim_external_deposit` (`:3556-3700`) sweeps the
caller's subaccount on whatever ledger the table uses. Send X to your own subaccount, call
`notify_deposit(block)`, be credited X; the sats are still there, so the sweep credits X
minus the fee again. Anti-replay does not help because the sweep is a new block. This is
on `btc_table_1`, which holds real ckBTC. SECURITY-FINDINGS.md:2103 already records that
`verify_ckbtc_deposit` is executed by no test; T-35 records that the local stack has no
ckBTC ledger. Not reproduced in this task (no ckBTC ledger locally); the read is
unambiguous.

Register: not recorded. Task: 1790632429 (security-high).

### Gap 3. A pot won by folds shows the winner's cards to the table

`get_table_view` treats `HandComplete` as showdown (`lib.rs:12690`) and reveals every
unfolded seat (`:12711-12713`). A fold-out goes `end_hand_single_winner` (`:9983`) to
`finish_hand` (`:9965`, sets `HandComplete`), and hole cards are cleared only by the next
deal (`:7599`). So the last player standing shows their hand to everyone until the next
hand starts. The archive is right (`push_winner`, `:9863-9872`, withholds the cards); the
live view contradicts the engine's own "takes the pot without showing a hand" rule. At a
real-money table this is a strategic leak every hand that ends by folds.

Register: **[E-107](DEFECTS.md#e-107)** (FIXED, task 1790632438; before it, DEFECTS.md's
"mucked cards stay hidden" covered folded seats, not the winner). Task: 1790632438.

### Gap 4. The clock can be armed on a seat that cannot act, and then fold it

After the posts, the deal sets `action_on` with a helper that asks `will_be_dealt_in`
(chips > 0) and returns the from-seat when nobody qualifies (`lib.rs:7681`, `:7968-7982`),
then arms the timer unconditionally (`:7686-7691`). A seat with fewer chips than its blind
is dealt in and left all-in by the post. When every dealt seat is all-in after blinds and
antes (heads-up, two short stacks), nothing calls `run_out_board`; the clock points at the
big blind, expires, and `resolve_expired_action_timer` folds that seat with no
`can_still_act` check (`:11254-11257`), so the small blind takes the contested layer.
Conservation holds, so the money invariants are silent: correct totals, wrong recipient,
this project's signature shape. Read, not reproduced; the recommended change (run out the
board when fewer than two can act; never fold a seat that cannot act) is right either way.

Register: [E-106](DEFECTS.md#e-106), FIXED by task 1790632440 (security-high); the reproduction is `tests/betting_rules.rs` section 6.

### Gap 5. Leaving from the seat on action hands a closed street to a player who already matched

`leave_table` marks the leaver folded and then moves the clock to the next seat
(`lib.rs:8226-8242`) without asking `is_betting_round_complete` (`:8735-8782`), which
`advance_game` does. A bets, B calls, C leaves on action: the street is closed but the
clock lands on A, whose only legal replies are Check or Fold; if A is away, `:11254` folds
A out of a pot they fully matched.

Register: **[E-108](DEFECTS.md#e-108)** (FIXED, task 1790632441). Task: 1790632441.

### Gap 6. A payout intent past its window locks the owner out of every withdrawal, and no door closes it

The journal keeps an intent 20 h (`lib.rs:2194`); past that the lease refuses it with
"needs an operator to reconcile" (`:2880-2892`). `withdraw` refuses while any open Payout
intent exists for the caller (`:4893-4925`) and escrow was already debited before the
await (`:5028-5035`). No entry point can close an intent after the deadline. A discarded
continuation (an upgrade mid-call, as on 2026-08-06 per FINDING 38) plus a day without
`resolve_my_ledger_intents` freezes that player's whole escrow. FINDING 29 and 38 describe
"kept forever, pending operator reconciliation" as intended; nothing records that no door
exists.

Register: not recorded as a gap. Task: 1790632442.

### Gap 7. A seated player can be reloaded, signed out or expired without warning, and a dropped connection looks live

`auth.js:73` creates the auth client with no `idleOptions`, so the library's default idle
manager logs out and reloads the page after ten minutes without pointer or key input
(`node_modules/@dfinity/auth-client/lib/esm/index.js:184-187`): a player waiting for a
table to fill, or holding a phone still, is reloaded while their stack stays seated and
the clock runs. The header's Sign out (`WalletButton.svelte:542-545`) has no seated guard,
and after it the poll continues anonymously so the dock becomes the spectator dock while
the seat is folded on chain. The 7-day delegation is checked only at init
(`auth.js:80-104`) and expiry surfaces as a failed poll and a bare toast
(`+page.svelte:651-656`). A failed poll only logs (`:648-656`), the client clock keeps
ticking from the last sample (`PokerTable.svelte:882-886`), the optimistic echo renders as
fact for 12 s (`lib/optimistic.js:220`), and `ConnectionStatus.svelte` is imported by no
file. The heartbeat is an update from every tab with an actor, seated or not
(`+page.svelte:782-790`; the canister answers `Err("Not at table")` at `lib.rs:12344`).

Register: not in UI-WAVE.md's 76 findings. Tasks: 1790632457 (session hygiene),
1790632460 (reconnecting state and seated-only heartbeat).

### Gap 8. Every call builds a fresh agent, so every query pays a subnet-key round trip first

`canisters.js:139-184` builds an `HttpAgent` inside every proxied call (`:201`, `:253`).
With query signature verification on (correct), the subnet-key cache is per agent
instance, so each poll (two queries per 500 ms, `+page.svelte:566-569`) is four round
trips, and the same overhead sits on `player_action`. `get_shuffle_proof` changes once per
hand and is fetched every tick. docs/RESPONSIVENESS.md:222 measured 258 ms
action-to-settled on loopback; mainnet carries the avoidable round trip on top.

Register: not recorded. Task: 1790632458.

### Gap 9. The fund canister holds every hole card in plaintext, the sealed dealer has no date, and the panel does not say so

The seed, the deck and every hole card live in `TableState` (`lib.rs:7509`, `:7570`,
`:7639`), readable by the controller through `get_table_state` (`:12661-12672`, a query,
no on-chain trace), a snapshot, or an upgrade, and by node operators. FINDING 22's fix
only stops a controller ending a hand while the clock runs. The no-peeking spike proved
the only reachable improvement is a separate dealer canister with an empty controller
list, wrote it (36 tests), and left it undeployed with a six-step migration
(docs/NO-PEEKING-FEASIBILITY.md section 10.8). Separately, revealing the whole seed after
every hand publishes every folded and unshown hand to anyone who runs the verifier: a
strategic leak a conventional room does not have. `ProofLimits.svelte:12-45` lists five
limits and omits both of these; `HowItWorks.svelte:104` and `:135` still say the
commitment is published before the deal, which D-06 (open) already says is untrue.

Register: FINDING 22 fixed with the residual stated; D-06 open; H-53 fixed; the mid-hand
read and the folded-card leak have no row. Tasks: 1790632470 (sealed dealer step 1),
1790632468 (proof limits and the two sentences).

### Gap 10. No read is certified

No `set_certified_data` or `data_certificate` exists in the table or history canister.
Balance (`lib.rs:5100`), table view (`:12674`), shuffle proof (`:12856`), hand history
(`:12864`) and archive records are plain queries. T-43 turned query signature
verification on, which binds a reply to one replica's key; a dishonest replica can still
hand a player a consistent fake that `shuffle-verify.js` passes, or a wrong balance.
Queued 1790622948 item 4 returns the post-action view from the update; the certified read
path does not exist.

Register: T-43 fixed, T-44 open (deposit warnings only). Task: 1790632469
(architecture).

### Gap 11. The lobby and the archive upgrade into an empty state on a failed save, and the backlog is lost

`src/lobby_canister/src/lib.rs:921-925` and `src/history_canister/src/lib.rs:1435-1438`
log and proceed when `stable_save` fails ("allow upgrade to proceed with potential data
loss"), while the table traps. The history canister is append-only with no cap
(`:599`), so the first upgrade whose serialisation exceeds the instruction limit silently
empties the fairness archive. In the table, `UNRECORDED_HANDS` is a bare thread_local
(`lib.rs:1043`) absent from `PersistentState`, capped at 64 with the oldest dropped
(`:1257-1263`); an upgrade during an archive outage destroys up to 64 records and the
status then reports zero.

Register: E-68 open (the upgrade half); docs/BACKEND-FUND-SAFETY-TODO.md item 2 (open,
unscheduled). Tasks: 1790632447, 1790632448.

### Gap 12. Ingress is free: no inspect_message, unbounded persisted maps, and a watermark that can void a journalled sweep

No canister has `#[ic_cdk::inspect_message]`, so anonymous and malformed updates are paid
for before the inline checks refuse them (docs/BACKEND-FUND-SAFETY-TODO.md item 1, open,
unscheduled). `DEPOSIT_CUSTODY` (`lib.rs:3969`) gains a persisted entry for any caller of
three ledger-querying updates, including at amount 0, with no rate limit and no pruning
(`:3985-3996`, `:786-860`), and its census is quadratic (`:4082-4106`); the code's own
comment (`:2158-2163`) says an unbounded persisted map bricks the canister once
`pre_upgrade` traps on size. `BALANCES` is never pruned. `deposit()` has no rate limit
(FINDING 10's own recommended follow-up, never filed), and `settle_intent`'s Moved branch
treats a watermark refusal as "already credited" (`:2468-2481` under the comment at
`:2463-2466`), which is true for the verified-deposits branch and false for the watermark
branch: a ledger-confirmed sweep can lose its credit and its record. `check_timeouts` has
no rate limit (FINDING 26).

Register: FINDING 10 residual, FINDING 26; the map growth and the watermark interaction
have no row. Tasks: 1790632445 (inspect_message, limits, CLAUDE.md residue), 1790632444
(bounded custody map), 1790632443 (watermark versus journal, deposit rate limit).

### Gap 13. Two admin roots

`is_controller` accepts a persisted `CONTROLLERS` list OR a real controller
(`lib.rs:739-751`); any controller can add to the list (`:5613-5633`). README.md:216-223
says a guardian handover closes every `require_controller` method, but the handover test
(`controller_custody.rs:1121-1131`) never populates the list, so a principal added before
handover keeps every admin door after it.

Register: not recorded. Task: 1790632449.

### Gap 14. Solvency is observational only

A CannotPayEveryone verdict (`lib.rs:6684`, `:7024-7031`) changes no behaviour; deposits
and buy-ins keep being accepted first come first served. The safe direction (refuse NEW
money on a fresh in-update shortfall reading, never auto-pause withdrawals) is cheap.

Register: T-44 open. Task: 1790632450.

### Gap 15. Every CI gate is advisory, the frontend's tests never run in CI, and the frontend build gate is red

Branch protection is not enabled (`.github/workflows/README.md:28-33`, checked
2026-08-09), so a red fund-safety job blocks nothing; docs/WAVE-11.md:18-22 called this
the single most valuable next item. `ci.yml:498-525` only builds the frontend; the 42
vitest files (446 tests) and svelte-check run nowhere but a developer's shell. No clippy
or rustfmt anywhere. `security.yml:32` audits one of four lockfiles and `:39` is
`npm audit || true`. `npm run build:mainnet` is red at HEAD (T-46, open, high): the
bundle verifier's matcher predates the `buildValue` closure.

Register: T-46 open, H-23 fixed with a caveat. Tasks: 1790632455 (T-46), 1790632495 (CI
jobs), 1790632496 (operator: required checks).

### Gap 16. The fast gate is 32 minutes

`make test` is about 1,894 s warm (`scripts/dev.sh:944-954`), 1,610 of them the
money-safety step, whose 22 PocketIC targets run strictly serially though each isolates
its own server. A five-minute tier for per-commit use, with `make test` kept as the merge
gate, is carvable from the existing rows.

Register: none. Task: 1790632456.

### Gap 17. The browser verifier guesses the player count the record already states

`ShuffleProof.svelte:105-116` derives the dealt-in count from participants and uses it as
a preference; `HandReplayer.svelte:256` does the same; the record carries `dealt_in` in
deck order (`lib.rs:545-570`) and the bindings declare it. SHUFFLE-SPEC section 4 says
deriving P from participants is the E-66 mistake. On a preflop fold-out the panel says
"ambiguous" when the record is not.

Register: E-67 open, probably stale after D-11's regeneration. Task: 1790632467.

### Gap 18. The archive's integrity rests on its writer

Records are stored as sent with no chaining (`history_canister/src/lib.rs:722-800`); a
rewritten record is undetectable unless a reader kept a copy, and `tools/archive` has no
root to compare between fetches. `insert_hand` (`:760-782`) will store a record with an
empty `revealed_seed` that can never be completed (unreachable today because reveal
precedes record on all three endings; one reordering makes it reachable).

Register: none. Task: 1790632472.

### Gap 19. Rules coverage is NLHE cash only, and two money-edged rules are still open

E-33 (no post-or-wait for the big blind, so a player can cycle in and out taking free
non-blind hands) and E-34 (no dead-button rule, so a seat emptying between the button and
the blinds skips a player for the big blind) have been open since wave 2 with no task.
Ratholing is free (`buy_in` checks only min and max, `lib.rs:5257-5261`). There is no
straddle, no run-it-twice, no showdown order or muck, no seat change (E-83), no blind
levels or tournaments, and the time bank replaces the clock rather than extending it and
is never replenished (`:12548-12563`). Four smaller residue items: a leaver's uncalled bet
is returned even when another seat could still call it (`:8171-8181`); a folded seat can
`show_cards` mid-hand (`:12593`); `init` prints and ignores a failed `validate_config`
(`:7353-7355`); the view's legality hints disagree with the engine (`:12767` ignores the
closed-action rule at `:8408`).

The queue already holds the canister half of the decision loop (1790622948: auto-check,
auto time bank, three-timeout sit-out, post-action view) and buy-in with rebuy
(1790622949). Whether to add straddle, run-it-twice and tournaments is a product decision
this review does not make; the rules with a money edge are filed.

Register: E-33, E-34, E-83 open. Tasks: 1790632473 (blind rules and rathole), 1790632474
(residue), 1790632475 (time bank, after 1790622948).

### Gap 20. Multi-tabling has no home in either the money layer or the client

Escrow is per table canister (`lib.rs:671`; `join_table` buys in from this canister's
escrow at `:8020-8058`), so moving a bankroll between tables costs a withdrawal (fee,
60 s cooldown, 100 ICP cap) and a deposit. In the client, `+page.svelte` (2,671 lines)
owns one non-reactive actor, one poll, one heartbeat and one clock policy for one table,
and `PokerTable.svelte` (2,370 lines) takes 22 props; a second table is a second browser
tab, which doubles gaps 7 and 8.

Register: none. Tasks: 1790632476 (escrow between trusted tables, architecture),
1790632464 (table session module, architecture).

### Gap 21. The ICRC-2 approval never expires and an interrupted deposit leaves it open

`deposit-icrc2.js:93-107` approves with no `expires_at` and no `expected_allowance`;
`deposit-flow.js:129-143` then calls `deposit`. A failure or a closed tab between the two
leaves the player having paid the approve fee with an indefinite allowance the UI never
reads back or offers to finish or revoke. Fund safety holds; the money is neither visible
nor recoverable from the screen.

Register: none. Task: 1790632461.

### Gap 22. One 934 KB chunk and no error boundary

The app is a single 300 KB gzip chunk with the cashier, the replayer, the proof panel,
the equity engine and the OISY wallet all loaded before the lobby paints; there is no
`+error.svelte`, no `<svelte:boundary>`, no unhandled-rejection handler, so a thrown
render error blanks the app with a seated stack.

Register: UI-WAVE section 6 item 5 records the file sizes, not the bundle or boundary.
Task: 1790632462.

### Gap 23. The test estate has holes between its tiers

All 42 frontend test files are pure-function tests; nothing mounts ActionBar, BetSizer or
the cashier modals, so the arming, hotkey, sizer and cooldown rules are verified only by
the replica harness (about 15 minutes with probes). In Rust there are no property tests
over the betting round, the cycles-per-hand figure is printed rather than asserted
(`cycles_runway.rs:619`), and the fuzzer only ever runs table_1's shape (H-11, open).

Register: H-11 open. Tasks: 1790632463 (happy-dom component tier), 1790632480 (proptest,
cycles budget, three-table scenario).

### Gap 24. The table canister is one 15,715-line file

328 functions, 39 update and 36 query entry points, 44 statics, four identical rate-limit
maps, with banner comments already naming the seams (money and custody `:1893-7335`;
deal, betting and settlement `:7460-10137`; stuck hand and clock `:10137-11900`; queries
`:12654-12936`; upgrade `:13493`). Gaps 3, 4 and 5 were easy to make because the seams
are not enforced. The split moves the module hash, so it should ride the E-102 redeploy.

Register: none. Task: 1790632477 (architecture).

### Gap 25. The orientation documents are stale in ways a newcomer will act on

AI_HANDOFF.md says `lib.rs` is about 4,500 lines, names an API path the code does not use,
a recipe icp.yaml does not use, and gives mainnet commands with a funded identity that the
overlay forbids a seat to run. tests/README.md says unit tests are not yet implemented
against 53 wired suites. Eleven WAVE files sit beside two registers of 19,000 lines with
no index. Queued 3090626845 covers only the README quick-start snippet.

Register: D-07 open. Task: 1790632479.

### Gap 26. A screen reader cannot follow a hand

Seven `aria-live` regions exist, all in the cashier and lobby; none on the table. The
clock is `aria-hidden` (`ActionBar.svelte:190`) with no hidden readout. Two svelte-check
a11y warnings remain in the verify dialog (`+page.svelte:1429`, `:1431`).

Register: none. Task: 1790632466.

### Gap 27. The collusion detector accuses the best player

`tools/archive/lib/collusion.mjs` signal 2's null is also the definition of "B is better
than A's other opponents"; on 800 real hands it named the best player with the same
verdict as a scripted chip dump (T-42, open, high, no task).

Register: T-42 open. Task: 1790632481.

### Gap 28. Drift monitoring compares self-reported metadata, not hashes

`deployed-drift.yml` runs `check-deployed.sh`, which compares the `git:revision` a
canister reports about itself (`verify-build.sh:446-450`: "a canister can claim any
revision it likes"). The script that reads module hashes needs dfx, which the
modernisation dropped and no workflow installs. The one gate that would have caught E-102
could not run.

Register: E-102's row names the gate as red. Task: 1790632454.

## 3. What this review did not do

- No reproduction on a replica. Every engine and money finding is from reading the cited
  lines; gaps 2 and 4 in particular deserve a reproducer as the first step of their task.
- No mainnet read of any kind (overlay rule 1). The E-102 and cycles statements are the
  registers' and the workflows' own.
- No product decisions: straddle, run-it-twice, tournaments, seat change and blind
  levels are named in gap 19 and left to the owner.
- Per-lens severities were normalised into one ranking by the reviewer; the register's
  own severities are unchanged.

## 4. The tasks filed

Forty-four tasks, producer claude-seat-1. `touches:` names code areas only. Operator-lane
tasks carry `lane: operator` and give the operator's command with the identity left as a
placeholder. `after:` ids are real minted ids.

| id | class | gap | title |
|---|---|---|---|
| 1790632429 | security-high | 2 | The ckBTC deposit door checks the owner and not the subaccount |
| 1790632440 | security-high | 4 | A deal that leaves fewer than two seats able to act runs the board out |
| 1790632499 | security-high, operator, after 1790632453 | 1 | The six mainnet backends run one reproducible build |
| 1790632498 | security-high, operator, after 1790632451 and 1790632483 | 1 | The guardian takes the controller seat |
| 1790632482 | security-high, operator | 1 | A wide freezing reserve and a capped cycles top-up |
| 1790632438 | security-medium | 3 | A pot won by folds must not show the winner's hole cards |
| 1790632441 | security-medium | 5 | leave_table by the seat on action goes through advance_game |
| 1790632442 | security-medium | 6 | A payout intent past its window no longer locks withdraw; a reconciliation door |
| 1790632443 | security-medium | 12 | settle_intent credits a watermark-refused sweep; deposit() rate limit |
| 1790632444 | security-medium | 12 | DEPOSIT_CUSTODY is bounded and its census linear |
| 1790632445 | security-medium | 12 | inspect_message, check_timeouts limit, CLAUDE.md residue |
| 1790632447 | security-medium | 11 | Lobby and history trap on a failed stable_save |
| 1790632448 | security-medium | 11 | The un-archived backlog survives an upgrade |
| 1790632449 | security-medium | 13 | The CONTROLLERS list cannot outlive a guardian handover |
| 1790632450 | security-medium | 14 | A confirmed shortfall closes the deposit and buy-in doors |
| 1790632453 | security-medium | 1 | deploy-mainnet.sh refuses a dirty tree, requires a snapshot, staged rollout |
| 1790632483 | security-medium | 1 | build-guardian.sh never falls back to an unlocked build |
| 1790632469 | architecture | 10 | A certified digest for the hand, the balance and the archive |
| 1790632470 | architecture | 9 | The sealed dealer's first gate |
| 1790632476 | architecture | 20 | Escrow moves between trusted tables |
| 1790632464 | architecture | 20 | A table session module |
| 1790632477 | architecture | 24 | lib.rs split into modules along its banners |
| 1790632457 | feature | 7 | A seated player is never reloaded, signed out or expired without warning |
| 1790632460 | feature | 7 | A reconnecting table; heartbeat only from a seated, visible tab |
| 1790632458 | feature | 8 | One HttpAgent per identity; the proof fetched once per hand |
| 1790632461 | feature | 21 | The ICRC-2 approval expires; an open allowance is shown |
| 1790632462 | feature | 22 | Cashier, replayer, proof and equity load on demand; an error boundary |
| 1790632466 | feature | 26 | A screen reader can follow a hand |
| 1790632467 | feature | 17 | The verifier reads dealt_in |
| 1790632468 | feature | 9 | The fairness panel states the two missing limits; D-06 sentences |
| 1790632472 | feature | 18 | The archive publishes a running Merkle root |
| 1790632473 | feature | 19 | Post-or-wait for the big blind, a dead button, a rathole rule |
| 1790632475 | feature, after 1790622948 | 19 | The time bank extends the clock and is replenished |
| 1790632451 | hygiene (docs) | 1 | docs/RUNBOOK.md |
| 1790632454 | hygiene | 28 | Drift monitoring compares certified module hashes |
| 1790632455 | hygiene | 15 | build:mainnet is green again (T-46) |
| 1790632495 | hygiene, after 1790632455 | 15 | CI runs vitest, svelte-check, clippy, rustfmt, cargo audit |
| 1790632496 | hygiene, operator, after 1790632495 | 15 | Required status checks on main |
| 1790632456 | hygiene | 16 | make test-quick |
| 1790632463 | hygiene | 23 | A component test tier under happy-dom |
| 1790632474 | hygiene | 19 | Engine residue: uncalled bet on leave, show_cards, init, legal-actions helper |
| 1790632479 | hygiene (docs) | 25 | AI_HANDOFF.md and tests/README.md say what is true |
| 1790632480 | hygiene | 23 | Property tests, a cycles budget, a three-table scenario |
| 1790632481 | hygiene | 27 | The collusion detector's skill-adjusted null |

Already queued before this review and cited rather than duplicated: 1790622948 (the
canister half of the decision loop), 1790622949 (buy-in and rebuy), 1790622950 (sampled
sounds), 1790622951 (landscape phone), 1790622952 (lobby proof rail), 1790622955
(photograph the street word), 1790622957 (own domain and branded sign-in, operator),
1790622958 (SB and BB labels), 3090622953 (colour literals), 3090622954 (harness gaps),
3090626845 (README quick start).

## 5. If only five things get done

1. Deploy the guardian and redeploy the fleet from one build (gap 1): 1790632453,
   1790632483, 1790632499, 1790632498, with the runbook 1790632451 first.
2. Close the ckBTC double credit (gap 2): 1790632429.
3. Fix the three engine defects (gaps 3, 4, 5): 1790632438, 1790632440, 1790632441.
4. Stop reloading, signing out and blinding seated players (gap 7): 1790632457,
   1790632460.
5. Make the gates bite (gap 15): 1790632455, 1790632495, 1790632496.
