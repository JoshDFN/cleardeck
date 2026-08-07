# ClearDeck security findings

Findings that touch real user funds. Each entry states what was actually executed, so a
reader can tell a demonstrated defect from a suspected one.

> ## 🚨 WAVE 11 RECONCILIATION, 2026-08-06 — THE SEVENTH CROSS-AGENT DEFECT, AND THE TWO THINGS THE FIFTH AUDITOR SAYS COST MONEY
>
> Four agents edited `src/table_canister/src/lib.rs` this wave. All four changes are present,
> none undoes another, and the reconciliation is written up in [WAVE-09.md](WAVE-09.md).
> Three things came out of it.
>
> **[FINDING 43](#finding-43) is new and it is the seventh instance of this project's signature
> shape** — CORRECT TOTALS, WRONG RECIPIENT, EVERY INVARIANT SILENT — and the purest one yet:
> no e8 moves, every number is arithmetically correct, and the public solvency instrument calls
> a player's money at the shared main account a **surplus**, in the same reply that names it as
> money held for somebody the canister cannot name. `total_liability()` (the guard) counts it;
> `get_solvency()`'s `owed` does not, while `held` does. Executed, with a committed marker.
>
> **[FINDING 41](#finding-41) was live and is now closed.** The OISY deposit path fetched its
> transfer destination from the same uncertified query [FINDING 40](#finding-40) is about, forty
> lines below the code that had stopped trusting it, and paid it automatically. The existing
> gate compared the 64-hex form, which that path never touches, so it was green with the defect
> present.
>
> **Nothing the fifth auditor says costs money is new.** Both were already filed and both were
> unscheduled: [FINDING 31](#finding-31) (a deposit at the advertised minimum is a silent 100%
> loss — raised `high` → `critical`) and [FINDING 23](#finding-23) (one controller key erases
> every balance; the auditor executed it — raised `critical` → `fund-theft`). The severities are
> now what was measured rather than what the filer guessed, so they sort to the top of
> `./scripts/register-stats.sh`.

> ## 🚨 WAVE 11, 2026-08-06 — THE DEPOSIT SURFACE: ONE ADDRESS, AND IT IS YOURS
>
> Three auditors raised the deposit path. All four items were driven on the current tree before
> anything was changed, and **two of the four were still live on a tree whose register already
> said FIXED.**
>
> | | | reproduced on this tree? | now |
> |---|---|---|---|
> | [FINDING 34](#finding-34) high | `get_deposit_address()` returned the canister's **MAIN account** -- the same 64 characters to every player -- under the name "deposit address" | **YES.** alice, bob and the main account were one string. 3 ICP sent to it landed where `claim_external_deposit()` truthfully answered *"your deposit address is empty"* | **FIXED.** It returns the legacy 64-hex spelling of the caller's OWN deposit account: the same account `get_deposit_subaccount()` names. Executed: money sent to the address a player is given is credited to that player and withdrawn by them |
> | [FINDING 40](#finding-40) high | that address was served over an **uncertified query**, and one substituted reply is a completed theft | **YES.** alice paid bob's address, bob swept 4.9999 ICP, **every money invariant silent** -- the seventh instance of this project's signature shape, and this time with no canister bug in it at all | **FIXED** by taking the canister out of the trust path: the client derives the address from its own principal and never asks. Gated by running the real frontend module in node against the real canister |
> | [FINDING 06](#finding-06) high | *"`notify_deposit` can never credit a deposit"* | **NO -- it credits.** Fixed in wave 2 and re-verified here; reverting BUG A or BUG B separately each turns a gate red | **CONFIRMED FIXED.** Kept, because it is the only door to money at the shared main account; it stopped calling itself a deposit path and now states the one case it cannot help with |
> | [FINDING 11](#finding-11) low | dust below the fee | **NO -- closed in wave 8.** Re-driven through the address a player is actually given | **CONFIRMED.** And a real gap was found next to it: its only gate was in **no target anybody runs**. Wired |
>
> **Why an address defect survived eleven waves of instruments.** A 64-hex address is only
> spendable through the ICP ledger's LEGACY `transfer` method. **Nothing in this project had ever
> called it.** Every test funds a deposit subaccount with `icrc1_transfer` to
> `Account { owner, subaccount }` -- so every instrument was exercising the addressing scheme no
> player is given, and agreed the deposit path was sound. The new target is the first thing here
> that spends the way a player spends.

> ## 🚨 WAVE 11, 2026-08-06 — THE HEADERS NOW SAY WHAT IS TRUE, AND ONE NEW FUND FINDING
>
> **Twelve findings had headers that did not say FIXED. Nine of those twelve were wrong**, in one
> direction or the other:
>
> | finding | header said | actually |
> |---|---|---|
> | [FINDING 15](#finding-15) | nothing | **FIXED in wave 6** — the fund lock, ~420 ICP, re-verified by an auditor who could not reproduce it |
> | [FINDING 17](#finding-17) | nothing | **FIXED in wave 6** — gated by `wave6_coherence::probe4` |
> | [FINDING 02](#finding-02) / [05](#finding-05) / [06](#finding-06) / [08](#finding-08) / [09](#finding-09) | nothing | **FIXED in wave 2** — the status was in a `**Status:**` line further down |
> | [FINDING 18](#finding-18) | nothing | **FIXED in wave 7** ([E-57](DEFECTS.md#e-57)) |
> | [FINDING 03](#finding-03) | nothing | **FIXED in wave 11**, by another owner, in this wave |
>
> And two headers said FIXED over a `**Status:**` line that still read **OPEN**
> ([FINDING 21](#finding-21), five waves; [FINDING 28](#finding-28)). Both corrected.
>
> ### [FINDING 39](#finding-39) is NEW and it is a fund finding
>
> **The canister ends a hostile sequence owing 4,000,000 e8s it does not hold**, on the exact wasm
> `./scripts/dev.sh test` builds and reports green on. It was filed in wave 4 as
> [E-41](DEFECTS.md#e-41) — with a reproducer, and with a note saying it belonged in this file as
> soon as the mechanism was known. The mechanism is still not known; the write-up is here now
> because the register pass would not mark the entry `FIXED` without running it, and it is red.
>
> ### The gate of the fourth and fifth cross-agent defects is run by nothing
>
> `stall_agreement` (M13, [FINDING 25](#finding-25)), `solvency`
> ([FINDING 35](#finding-35)/[36](#finding-36)), `deposit_subaccount_anchor`
> ([FINDING 28](#finding-28)/[11](#finding-11)) and `fund_reachability` (M9) are named by no make
> target, no `dev.sh` command and no CI job: **[H-45](DEFECTS.md#h-45)**. All four were run by hand
> in this pass — three green, and `fund_reachability` **RED** ([H-48](DEFECTS.md#h-48)). Every
> `FIXED` row below whose gate column says `NOT RUN` is one of these.
>
> **The list grew to six while this pass was writing it down**: `oldest_cluster` (the gate
> [FINDING 22](#finding-22) is now closed by) and `deposit_surface` were both added, unwired, by
> other owners in the same wave. 46 tests in six files, run by nothing.
>
> ### Five findings have no DEFECTS.md id and are scheduled from [THE FINDINGS](#the-findings)
>
> 22, 23, 31, 32, 38. [FINDING 23](#finding-23) is `critical` and has been open since wave 7.

> ## 🚨 STATUS, WAVE 11, 2026-08-06: THE OLDEST CLUSTER IS TRUE AGAIN — FIVE WERE ALREADY DEAD AND SAID OTHERWISE
>
> **FINDING 02, 05, 08, 09 and 17 were CLOSED in waves 2, 7 and 8 and their headers never said
> so.** A reader scanning this file saw five open fund defects that are not open. That is not a
> documentation problem: it is how six findings sat unscheduled for a whole wave, because a
> register you cannot trust at a glance is a register nobody triages from.
>
> Each of the five is now **re-asked on the current tree and gated**, one test per finding, in
> `tests/money_safety/tests/oldest_cluster.rs`:
>
> ```text
> cd tests/money_safety && cargo test --test oldest_cluster -- --test-threads=1 --nocapture
> ```
>
> | finding | verdict on this tree | what goes red if it returns |
> |---|---|---|
> | [02](#finding-02) `state.pot` is the payout authority | **REFUTED, structurally.** `build_side_pots_logged` / `apply_side_pots` have NO caller left in the canister | `finding02_the_state_pot_authority_has_no_caller_in_the_canister` |
> | [05](#finding-05) `leave_table` orphans a stake | **REFUTED, in play.** Layer amounts unchanged across the departure | `finding05_and_08_...`, door `leave_table` |
> | [08](#finding-08) `cash_out` after a timeout | **REFUTED, in play**, by the disconnect route itself | `finding05_and_08_...`, door `timeout_then_cash_out` |
> | [09](#finding-09) the engine logs its own inconsistency | **REFUTED.** 32 mid-hand cross-checks of `state.pot` against the payout basis, 0 `BUG:`/`CRITICAL:` lines | `finding09_the_engine_never_reports_a_pot_it_settled_against_anyway` |
> | [17](#finding-17) a hand settles with no live claim | **REFUTED.** The fold-out winner is paid and the folder pays | `finding17_a_fold_out_pays_the_seat_that_holds_the_claim` |
> | [22](#finding-22) a controller can void a live hand | **REAL, REPRODUCED, ANSWERED** — see below | `finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played` |
>
> ### And the instrument had a hole exactly where this cluster lives
>
> The settlement oracle is the only thing in the project that asks *who was PAID* rather than
> *do the totals balance*. Every vacating scenario in it called `leave_table` and only fell back
> to `cash_out` if that failed — and `leave_table` never fails for a seated player, so
> **`cash_out` and `check_timeouts` had never executed inside the oracle at all**, on 53 hands
> across four benches. FINDING 08's whole point is that the state is reachable by a DISCONNECT.
> `suite::timed_out_seat_cashes_out_mid_hand` and
> `settlement::a_seat_folded_by_its_own_clock_and_cashed_out_mid_hand_settles_by_the_rules`
> close it: a seat folded by its own action clock, cashed out mid-hand over a short all-in, and
> compared seat by seat and person by person against what the rules of poker owe.

> ## 🚨 STATUS, WAVE 10, 2026-08-06: THE CANISTER CAN NOW SAY IT CANNOT PAY — AND ON MAINNET IT CANNOT, BY 2.00 ICP
>
> [FINDING 35](#finding-35) is **CLOSED**: the main account has an observation record, anybody
> (including an anonymous caller) can take a reading, `get_solvency()` publishes what the
> canister owes against what it holds with the age of every reading and `null` where a reading
> has never been taken, and a withdrawal that fails for want of canister funds now says so
> instead of returning the ledger's debug string.
>
> **The 2.00 ICP shortfall on mainnet table_1 is real and is NOT fixed by this.** Nothing here
> edits a balance and nothing here may: see the "CLOSED" box on FINDING 35 and the
> `no_setter_was_added_to_fix_the_books` gate. The canister's job is to report it truthfully;
> returning the money is an operator action.
>
> | account | canister-side observation record | in `total_liability()` | operator can audit it |
> |---|---|---|---|
> | ICP **main** / ckBTC **main** | `MAIN_CUSTODY` (wave 10) | yes, `main_uncredited_observed()` | yes, and `admin_audit_deposit_custody` reads it |
> | ICP / ckBTC **deposit subaccounts** | `DEPOSIT_CUSTODY` | yes | yes |
>
> [FINDING 36](#finding-36) and [FINDING 37](#finding-37) are closed with it: the harness's
> `CustodyStatus` mirror is complete and has a sum assertion that cannot hold if a field is
> silently dropped again, and `total_liability()` is published as `SolvencyReport.guard_liability`
> so an instrument can check the guard instead of having to trigger it.
>
> ---
>
> ## 🚨 STATUS, WAVE 8 COHERENCE PASS, 2026-08-06: THE CENSUS HAS SIX ACCOUNTS AND THE CANISTER CAN READ FOUR
>
> *(Superseded by the box above; kept because it is the statement of the defect.)*
>
> The banner below says every instrument now measures both kinds of ledger account. **That is
> true of the harness and false of the canister**, and the difference is
> [FINDING 35](#finding-35).
>
> Driven on this tree: `admin_audit_deposit_custody` replies **`(1 audited, 0 held, 0 unaudited)`
> on a canister holding 5 ICP** of a named player's, and `admin_update_config(BTC)` is then
> ACCEPTED, which routes `notify_deposit`, the money's only recovery door, at the wrong
> ledger. No attacker, no trap, no unfinished call: one exchange withdrawal and one config change.
> The flip **is** reversible (measured), but no surface would ever tell an operator to reverse it.
>
> Two more instances of the same shape, both in the instruments:
> [FINDING 36](#finding-36) (the harness's `CustodyStatus` mirror silently drops the field
> carrying FINDING 29's money, under a comment saying it is mirrored in full) and
> [FINDING 37](#finding-37) (`total_liability()`, the last custody guard's only input, has one
> caller, no query and, until this pass, no gate).
>
> **[FINDING 33](#finding-33) is CLOSED** in this pass: one word, reproduced three times first,
> now gated by `tests/money_safety/tests/coherence_w8.rs` inside `./scripts/dev.sh test`.

> ## 🚨 STATUS, 2026-08-06: THE UNBOUNDED ONE WAS DRIVEN, AND IT WAS AS BAD AS IT LOOKED
>
> [FINDING 29](#finding-29) -- *"both deposit paths move real money on the ledger before writing any
> record of intent"* -- was the only wave-7 blocker no reviewer could exercise, and the only one that
> can cost an arbitrary amount. **It has now been reproduced against the real mainnet ICP ledger
> wasm, measured, and closed.**
>
> | | measured, on the module the auditor reviewed |
> |---|---|
> | the loss | 3.0 ICP pulled out of a player's wallet into the canister, **0 credited** |
> | the recovery | **eleven doors tried, player and controller, none returns it.** `notify_deposit` refuses the pull's real block index with *"it was credited to your balance when the pull happened"* -- the exact lie the auditor predicted; `admin_restore_balance` no longer exists |
> | the door nobody had named | **`withdraw()` has the same shape.** Its escrow debit and its `PENDING_WITHDRAWALS` flag are committed at the await, its refund only ever existed in the continuation, and one hour later the player was still told *"a withdrawal is already in progress"*. `withdraw` is the only door from escrow to the ledger, so that was a permanent unbounded fund lock |
> | the trap itself | **NOT forced.** Four mechanisms tried, each with the measured reason it failed, recorded in the finding and printed by the gate. The reproduction is faithful to the consequence, not to the cause -- and the last of the four shows the trap is not the only trigger |
>
> **Fixed** by a durable ledger-intent journal written *before* every irreversible movement, made
> replayable by the ledger's own ICRC-1/ICRC-2 deduplication (`memo` + `created_at_time`), with an
> owner-drivable `resolve_my_ledger_intents()`, persistence as `opt` across upgrades, and a bound
> enforced by refusing to start rather than by forgetting. Gate: **M14 LEDGER/BOOKS COHERENCE**,
> `tests/money_safety/tests/ledger_boundary.rs` (6 tests) plus fault injection on one fuzz run in
> four.

> ## 🚨 STATUS, 2026-08-06: THE CANISTER OWNS TWO KINDS OF LEDGER ACCOUNT, AND EVERY INSTRUMENT NOW MEASURES BOTH
>
> **This canister has always held money in two places and measured one.** Its main account, and
> one deposit subaccount per principal -- `sha256("cleardeck-deposit:" || principal)`, the address
> `get_deposit_subaccount()` publishes to external wallets. `total_liability()`, M1, M9's orphan
> check, the drain's `table_is_really_empty` and all four balance surfaces were anchored to the
> main account alone. Two independent reviewers reached that hole from opposite directions in one
> wave, and it is now closed at the anchor rather than at either symptom.
>
> | | finding | what it cost | status |
> |---|---|---|---|
> | 1 | [FINDING 28](#finding-28) high | a player was told *"No claimable balance. Send ICP to your deposit address first"* **while this canister held their ICP at exactly that address**, and `get_custody_status` -- the surface built to answer "what is this canister holding for me" -- answered `total = 0` | **FIXED 2026-08-06.** The canister now records what the ledger says about each of its deposit subaccounts, persisted across upgrades; `get_custody_status` carries it in a field and in `total`; the refusal states the amount, the fee, that the money is not lost and the exact top-up that recovers it; `get_deposit_custody()`, `refresh_deposit_custody()`, `admin_get_deposit_custody()` and `admin_audit_deposit_custody()` are new |
> | 2 | [FINDING 21](#finding-21) high | the FINDING 20 currency guard read a liability of **zero** on a table holding 5 ICP of a player's money, and permitted the re-denomination that strands it. The wave-7 orphan invariant reported **0** orphaned e8s on a canister holding 7 ICP in its own subaccounts | **FIXED 2026-08-06.** `total_liability()` includes every deposit subaccount the canister has read, **and** the guard refuses while any address it published has never been read -- unknown is not zero, which the arithmetic fix alone would have missed |
> | 3 | [FINDING 11](#finding-11) / [E-12](DEFECTS.md#e-12) low | dust at or below the transfer fee, stranded and silent | **ANSWERED 2026-08-06.** Still immovable on its own (arithmetic), now visible on every surface and **recoverable by topping the same address up past the fee** -- driven end to end |
>
> **What this does not claim.** A deposit subaccount is a pure function of a principal, so the set
> of these accounts is as large as the set of principals and this canister cannot enumerate it. It
> enumerates every principal it holds escrow for, has seated, or has already read, and
> `admin_audit_deposit_custody(also)` takes an operator-supplied list for the rest. The derivation
> is written down in the code next to the anchor ("THE ACCOUNT CENSUS" in
> `src/table_canister/src/lib.rs`) so any address can be checked by hand. The harness asserts the
> limit rather than papering over it.
>
> Gates: `tests/money_safety/tests/deposit_subaccount_anchor.rs` (11 tests). Seven one-at-a-time
> reverts of the seven re-anchorings were driven in a `cp -Rc` copy, each anchor asserted to match
> exactly once and every file asserted byte-identical afterwards. **All seven turn a gate red:**
>
> | revert | gate that goes red |
> |---|---|
> | `total_liability()` drops the deposit-subaccount term | `the_currency_guard_refuses_on_a_table_funded_only_through_a_deposit_subaccount` |
> | the currency guard drops the "unknown is not zero" leg | `the_currency_guard_refuses_while_a_published_address_has_never_been_read` |
> | `claim_external_deposit` restores the original refusal text | `the_claim_refusal_may_not_say_there_is_nothing_when_there_is` |
> | `get_custody_status` stops folding the deposit into `total` | `money_at_a_published_deposit_address_is_visible_on_every_surface` |
> | `check_no_orphaned_custody` re-anchored to `ledger_main` alone | `the_orphan_invariant_is_red_on_money_held_only_in_deposit_subaccounts` |
> | `Snapshot::internal_total` drops the canister's deposit custody | `the_drain_reports_not_empty_while_a_deposit_address_still_holds_money` |
> | `check_deposit_attribution` gutted to return nothing | `the_attribution_leg_is_red_when_the_totals_are_right_and_the_recipients_are_not` |

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
<a id="finding-01"></a>
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
reconciles these findings with the harness and tooling defects found alongside them and gives most
of them an id, a severity and a reproduce command. This file is the evidence; that file is the
order of work. **Five findings have no DEFECTS.md id** — 22, 23, 31, 32 and 38 — and they are marked
`—` in the table below. That is not an oversight to tidy up later: it is exactly how
[FINDING 23](#finding-23), the only item in either file graded `critical, unguarded by design`,
sat unscheduled from wave 7 to now. **THE FINDINGS below is a queue in its own right and those
rows are scheduled from it.**

## How to read this file

Same five-word vocabulary as [DEFECTS.md](DEFECTS.md#how-to-read-this-register), same rules:

`OPEN` · `FIXED` (and the gate column names what would catch it coming back) · `FIXED-NO-GATE` ·
`WONTFIX` · `BY-DESIGN`. The wave is its own column. There is no `PARTIAL`: a finding with one
half landed and one half live is `OPEN` and says which half landed
([FINDING 19](#finding-19) is the example).

`./scripts/register-stats.sh` counts this table and DEFECTS.md's together;
`./scripts/register-stats.sh --check` fails if a status leaves the vocabulary, if a `FIXED` names
no gate, if an anchor does not resolve, if two rows claim the same finding number (**21, 22 and 23
were each written twice by two agents in one wave**), or if a write-up here has no row.

**`NOT RUN` in the gate column means the gate was written, it exists, and no target invokes it.**
Four money-safety targets are in that state and they carry the gates of
[FINDING 25](#finding-25), [FINDING 28](#finding-28), [FINDING 34](#finding-34),
[FINDING 35](#finding-35), [FINDING 36](#finding-36) and [FINDING 11](#finding-11):
[H-45](DEFECTS.md#h-45). All four were run by hand on 2026-08-06 and the results are in the gate
column.

<a id="the-findings"></a>
## THE FINDINGS

**Sorted worst-first: everything `OPEN` before everything closed, by severity inside that.**
| finding | sev | status | wave | gate — what would catch it coming back | DEFECTS.md | one line |
|---|---|---|---|---|---|---|
| [FINDING 39](#finding-39) | high | OPEN | — | — | [E-41](DEFECTS.md#e-41) | **the canister owes 4,000,000 e8s it does not hold**, after a hostile sequence, on the exact wasm `./scripts/dev.sh test` builds. Filed as [E-41](DEFECTS.md#e-41) in wave 4 with no findings write-up and never re-run since; re-driven 2026-08-06 and it is twice the size the entry records. M1 CONSERVATION and M2 LEDGER REALITY, which are never excusable |
| [FINDING 23](#finding-23) | fund-theft | OPEN | — | — | — | `uninstall_code` and `install_code --mode reinstall` are FINDING 07 at full scale and the wave-7 audit does not cover them. **Filed wave 7, untouched since. Raised from `critical` to `fund-theft` in wave 11: the fifth auditor EXECUTED it** — 5 ICP deposited as a player, one `icp canister install --mode reinstall` with the same wasm and no code change, and her balance read 0 while the ledger still held her 5 ICP at the canister's account. Every player-callable recovery told her she had nothing. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 19](#finding-19) | high | OPEN | — | — | [E-54](DEFECTS.md#e-54), [E-55](DEFECTS.md#e-55) | **the clock half is FIXED (wave 7, gated by `timers`); the cycles half is OPEN and the fix made it worse.** Nothing on chain moved the game, so liveness was outsourced to whoever had a browser tab open. The on-chain clock closed that and raised idle burn ~630x, and **nothing tops a canister up**. Open until [FINDING 24](#finding-24) and [FINDING 26](#finding-26) are |
| [FINDING 22](#finding-22) | high | FIXED | 11 | `dev.sh test` -> `oldest_cluster` -> `finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played` + `finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked`; `dev.sh test` -> `admin_custody::admin_reinit_table_mid_hand_returns_the_pot_to_the_players_who_put_it_in` + `admin_custody::a_controller_ending_a_hand_pays_a_vacated_seats_stake_to_its_owner_not_its_new_occupant` (added in the wave-11 reconciliation: the chair-changed-hands seam, asserted PER PRINCIPAL) | [E-78](DEFECTS.md#e-78) | the FINDING 07 fix gave a controller a new power: **void any live hand after reading every hole card**, conserving to the e8 so no invariant could see it. Reproduced on this tree, then narrowed: the door refuses unless nothing can move the hand, closes it through the permissionless `settle_unmovable_hand`, and marks every credit `pot_type="refund:ended-by-controller"` in the permanent record |
| [FINDING 24](#finding-24) | high | OPEN | — | — | [E-55](DEFECTS.md#e-55) | a frozen table answers **nothing**, not "queries only": the mitigation this project documented in four places does not exist, and the freezing reserve is about one hour, not 30 days |
| [FINDING 26](#finding-26) | high | OPEN | — | — | [E-55](DEFECTS.md#e-55) | the 226-day runway assumes nobody is hostile: a free, permissionless ingress flood burns a table 65x faster, collapsing it to about three days |
| [FINDING 31](#finding-31) | critical | OPEN | — | — | — | **FINDING 27 is only half closed.** A deposit of exactly the advertised minimum, made to the address the canister publishes, is still unwithdrawable: the sweep fee comes out of the money, so 20,000 sent becomes 10,000 in escrow and `withdraw` refuses it. Not one of `deposit_floor.rs`'s six tests sends anything to a deposit subaccount. **Raised from `high` to `critical` in wave 11: the fifth auditor re-derived it independently and it is a silent 100% loss at the number the product prints.** The dead band for the external route is `(10_000, 20_000]` e8s and the advertised minimum is the TOP of it — the largest fully unrecoverable deposit is exactly the number on the screen, while the in-app approve route at the identical number is safe. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 32](#finding-32) | high | OPEN | — | — | — | the screenshot harness's NO-RAKE gate cannot go red on any rake this canister is capable of taking. [E-61](DEFECTS.md#e-61) fixed the `rake=NaN` half and `test-rake.mjs` now proves the gate goes red on a 1-e8 rake read from `get_hand`; **the second clause was not re-examined in the wave-11 pass** and this finding's own "what would close it" — drive a rake-taking canister end to end — is still undone. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 38](#finding-38) | high | OPEN | — | — | — | `get_solvency()` counts an open `pull` on BOTH sides on the strength of the reading not yet containing the money, and wave 10 shipped a public button that makes the reading contain it: **one anonymous `refresh_solvency()` turns a real shortfall into a published SURPLUS.** Newest finding, wave-10 critic. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 04](#finding-04) | low | OPEN | — | — | [E-13](DEFECTS.md#e-13) | `detect_straight` prefers the wheel over a higher straight. Latent: unreachable today, a trap for the obvious optimisation. Watched by `dev.sh known-defects` |
| [FINDING 10](#finding-10) | fund-theft | FIXED | 2 | `dev.sh test` → `deposit_replay` (dr00–dr10) | [E-02](DEFECTS.md#e-02) | **the only completed theft in this project.** One real on-ledger transfer credited to escrow twice and the excess withdrawn as real ICP, leaving the attacker 2.9997 ICP up in her own wallet and another player's escrow unbacked |
| [FINDING 16](#finding-16) | fund-theft | FIXED | 5 | `dev.sh test` → `regressions::reg09`; `cargo test --workspace hand_membership` | [E-32](DEFECTS.md#e-32), [E-06](DEFECTS.md#e-06) | player-to-player fund theft, demonstrated: a player who stopped answering kept a claim on the pot without paying for it — a free showdown for money already in, measured at 0.98 big blinds a hand |
| [FINDING 01](#finding-01) | critical | FIXED | 2 | `dev.sh test` → `regressions::reg01`; `settlement disagreements::pinned_e01` | [E-01](DEFECTS.md#e-01) | every showdown that followed post-flop betting paid the winner only the pre-flop pot; the rest was debited from stacks and credited to nobody, withdrawable by nobody including a controller |
| [FINDING 07](#finding-07) | critical | FIXED | 7 | `dev.sh test` → `admin_custody` (13 tests); `wave6_coherence::probe5` | [E-07](DEFECTS.md#e-07) | one controller call destroyed 100% of a funded table's chips. Reproduced live on `table_3` first: 40.00000000 ICP gone, canister still holding it, both players reading `get_balance = 0` |
| [FINDING 15](#finding-15) | critical | FIXED | 6 | `dev.sh test` → `wave6_coherence::probe1`; `invariants::reachability` (M9) on every fuzz step | [E-42](DEFECTS.md#e-42), [E-40](DEFECTS.md#e-40) | **the fund lock.** One unguarded evaluator call locked a funded table with ~420 ICP unreachable through every path a player has. Closed in wave 6 and independently re-verified by an auditor who could not reproduce it; this header said nothing about that until wave 11 |
| [FINDING 33](#finding-33) | critical | FIXED | 8 | `dev.sh test` → `coherence_w8::finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible` | [E-45](DEFECTS.md#e-45) | FINDING 29's netting reopened FINDING 21: while one sweep was unfinished `total_liability()` read ZERO on a canister holding 5 ICP and the currency guard let the flip through, **and the flip was one-way.** One word: `== Pull` under a comment saying `Pull` AND `Sweep` |
| [FINDING 02](#finding-02) | high | FIXED | 2 | `dev.sh test` → `cargo test --workspace payout_tests::e03_*`; settlement oracle; wave 11: `oldest_cluster::finding02_the_state_pot_authority_has_no_caller_in_the_canister` | [E-03](DEFECTS.md#e-03) | `state.pot` was the sole authority for side-pot totals **in both directions**: chips created in one and destroyed in the other, through an `f64` ratio |
| [FINDING 05](#finding-05) | high | FIXED | 2 | `dev.sh test` → `regressions::reg05`/`reg08`; `settlement disagreements::pinned_e05`; wave 11: `oldest_cluster::finding05_and_08_...(door leave_table)` | [E-05](DEFECTS.md#e-05) | `leave_table()` made FINDING 02 Direction A attacker-reachable: vacating a seat mid-hand deleted the record of what that player had put in while the money stayed in the pot |
| [FINDING 06](#finding-06) | high | FIXED | 2 | `dev.sh test` → `deposit_replay::dr01`/`dr02`; wave 11: `deposit_surface::money_at_the_shared_main_account_is_recoverable_by_its_sender_and_by_nobody_else` | [E-04](DEFECTS.md#e-04) | `notify_deposit` could never credit a deposit (two independent decode bugs) and the ICP sent was stranded forever. It was also the only thing standing in front of FINDING 10 — a decode bug doing the job of an access control. **Re-verified in wave 11 after a third auditor raised it again: it credits, and reverting EITHER bug separately turns a gate red.** The endpoint stays because it is the only door to money at the shared main account; what changed is that it no longer describes itself as a deposit path |
| [FINDING 08](#finding-08) | high | FIXED | 2 | `dev.sh test` → `regressions::reg05`/`reg08`; wave 11: `oldest_cluster::finding05_and_08_...(door timeout_then_cash_out); settlement `a_seat_folded_by_its_own_clock_and_cashed_out_mid_hand_settles_by_the_rules`` | [E-05](DEFECTS.md#e-05) | `cash_out` was a second door into FINDING 05's orphaned-stake state |
| [FINDING 09](#finding-09) | high | FIXED | 2 | `dev.sh test` → `invariants::classifier` (an unenumerated self-report is now `SelfReportedFailure`, which nothing can excuse); wave 11: `oldest_cluster::finding09_the_engine_never_reports_a_pot_it_settled_against_anyway` | [E-03](DEFECTS.md#e-03) | FINDING 02 Direction B was reached in ordinary play and the canister said so in its own logs, which nothing was reading |
| [FINDING 12](#finding-12) | high | FIXED | 5 | `dev.sh test` → `regressions::reg09`; `timers` | [E-06](DEFECTS.md#e-06) | the disconnect timeout and the action timeout were both 30 s on `table_1`/`btc_table_1`, so one lull ran the whole board out and settled |
| [FINDING 13](#finding-13) | high | FIXED | 3 | `dev.sh test` → `invariants::principals` (M8); settlement PRINCIPAL column | [E-37](DEFECTS.md#e-37) | the E-05 fix paid a departed player's refunded stake to whoever took their chair. Composed end to end on the real canister, **with every total balancing** |
| [FINDING 14](#finding-14) | high | FIXED | 3 | `dev.sh test` → `invariants::upgrade_across_versions` (M7) | [E-38](DEFECTS.md#e-38) | two agents each added a persisted field that is not a Candid-compatible addition. Fixing only the first makes an upgrade **silently destroy every chip at the table** and report success |
| [FINDING 17](#finding-17) | high | FIXED | 6 | `dev.sh test` → `wave6_coherence::probe4` (an OUTCOME assertion: the fold-out winner must be PAID the pot); wave 11: `oldest_cluster::finding17_a_fold_out_pays_the_seat_that_holds_the_claim` | [E-36](DEFECTS.md#e-36) | a hand could be settled with **no live claim on it** and every stake handed back to the players who folded, so the fold-out winner lost the pot they won. Closed in wave 7; this header said nothing about that until wave 11 |
| [FINDING 18](#finding-18) | high | FIXED | 7 | `dev.sh test` → `invariants::custody` (M10, which reads the canister as the PLAYER rather than as a controller) | [E-57](DEFECTS.md#e-57) | `cash_out` of a stuck hand walked away from your own stake and nothing anywhere said so |
| [FINDING 20](#finding-20) | high | FIXED | 7 | `dev.sh test` → `admin_custody::currency_cannot_be_changed_while_the_canister_owes_anybody_anything` | [E-45](DEFECTS.md#e-45) | a controller could re-denominate a funded table, making every balance unpayable, without ever writing `BALANCES` |
| [FINDING 21](#finding-21) | high | FIXED | 8 | `dev.sh test` → `coherence_w8::finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible` | [E-45](DEFECTS.md#e-45) | the FINDING 20 guard measured the wrong total, so a funded table could still be re-denominated. **Its own `**Status:**` line still read OPEN five waves after the header said FIXED** — corrected in wave 11 |
| [FINDING 25](#finding-25) | high | FIXED | 7 | `stall_agreement` (M13 ONE BELIEF, 138 forked states) — **NOT RUN BY ANY TARGET**, see [H-45](DEFECTS.md#h-45). Run by hand 2026-08-06: 2 passed | [E-59](DEFECTS.md#e-59) | **the fourth cross-agent defect.** After a stall, one permissionless call voided the hand the clock would have played out; the player who was losing had the incentive and `get_custody_status()` told them to press the button |
| [FINDING 27](#finding-27) | high | FIXED | 7 | `dev.sh test` → `deposit_floor` (6 tests) + `const _: () = assert!(min_withdrawal <= min_deposit)` | [E-62](DEFECTS.md#e-62) | the ICP deposit floor was below the withdrawal floor, so money arriving at the product's own documented minimum could never leave. **This closes the ICRC-2 `deposit()` door only** — the subaccount door is [FINDING 31](#finding-31) |
| [FINDING 28](#finding-28) | high | FIXED | 8 | `deposit_subaccount_anchor` (11 tests) — **NOT RUN BY ANY TARGET**, see [H-45](DEFECTS.md#h-45). Run by hand 2026-08-06: 11 passed | — | money at the canister's OWN published deposit address was reported as zero by every balance surface and the error text told the player to send more. **Its own `**Status:**` line still read OPEN after the header said FIXED** — corrected in wave 11 |
| [FINDING 29](#finding-29) | high | FIXED | 8 | `dev.sh test` → `ledger_boundary` (8 tests, M14) + fault injection on one fuzz run in four | [E-69](DEFECTS.md#e-69) | all three money doors moved real money on the ledger **before** writing any record of intent, with no journal and no recovery. Driven: 3.0 ICP pulled out of a player's wallet, 0 credited, eleven doors out of that state all closed |
| [FINDING 30](#finding-30) | high | FIXED | 8 | `dev.sh test` → `invariants::archive` (M12); `record::check_archived_participants` on every fuzz step | [E-66](DEFECTS.md#e-66) | the permanent hand archive was built from the seats as they stood at settlement, so it omitted anyone who left mid-hand and invented anyone who sat down — and the shuffle spec told verifiers to count from it |
| [FINDING 34](#finding-34) | high | FIXED | 11 | `dev.sh test` → `deposit_surface` (10 tests, wired in wave 11); `solvency` for the visibility half | [E-70](DEFECTS.md#e-70) | `get_deposit_address()` published the canister's MAIN account -- the same 64 characters to every player -- under the name "deposit address". **This row said FIXED (wave 10) on a tree where it was still live**: FINDING 35's `MAIN_CUSTODY` made the money VISIBLE to an operator and did nothing about the address being published, which is the half that costs a player their deposit. Reproduced on the current tree in wave 11 (alice, bob and the main account all one string), then closed: the method now returns the legacy 64-hex spelling of the caller's OWN deposit account, and a player sending 3 ICP to the address they are given is credited 2.9999 in their own escrow and withdraws it |
| [FINDING 40](#finding-40) | high | FIXED | 11 | `dev.sh test` → `deposit_surface::a_substituted_address_is_a_completed_theft_and_local_derivation_is_what_prevents_it` + `::the_frontends_own_derivation_agrees_with_the_canister_for_every_principal` | — | the deposit address was served over an **uncertified query**, so one dishonest replica substituting one reply is a completed theft: alice paid bob's address, bob swept 4.9999 ICP, **and every money invariant was silent because no e8 went missing.** Closed by removing the canister from the trust path rather than certifying it: the client DERIVES the address from its own principal and never asks, and the gate runs the real frontend module in node against the real canister, principal by principal. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 41](#finding-41) | fund-theft | FIXED | 11 | `dev.sh test` → `deposit_surface::the_oisy_transfer_destination_is_derived_locally_and_not_fetched` (follows the value the transfer is addressed to) + `::the_frontends_32_byte_derivation_agrees_with_the_canister_for_every_principal` | — | **FINDING 40 WAS HALF CLOSED.** The display path derived the address locally; the OISY path, forty lines up the same file, still called `get_deposit_subaccount()` — the same uncertified query, in its 32-byte spelling — and transferred straight to the account it named, with no address ever shown to the player. Closed in the wave-11 reconciliation: the branch derives `depositSubaccount(sessionPrincipal)` locally, cross-checks the canister's reply and **aborts the transfer** on a disagreement instead of warning about it. The 64-hex gate could not see this and stayed green with the defect present, so the new gate follows the value the wallet is actually paid. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 43](#finding-43) | high | OPEN | — | — | — | **THE SEVENTH CROSS-AGENT DEFECT.** The public solvency instrument calls the money at the shared main account a **surplus**, in the same reply that names it as unattributed. `total_liability()` — the number the currency guard reads — has five terms and the fifth is `main_uncredited_observed()` ([FINDING 35](#finding-35)); `get_solvency()`'s own `owed` has six terms and that is not one of them, while `held` DOES include the main-account balance. The same e8 is an asset and not a liability. Executed 2026-08-06: 1 ICP at the main account gives `owed = 0`, `guard_liability = 100000000`, `unattributed_at_main = Some(100000000)`, verdict `CanPayEveryone`, summary *"it owes 0.0000 ICP … a surplus of 100000000 e8s"*. This is exactly the money [FINDING 34](#finding-34)'s shared address collected for eleven waves, and there is ICP at that account on mainnet today. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 42](#finding-42) | fund-theft | OPEN | — | — | — | the local derivation is rooted in a canister id the client takes from **`lobby.get_tables()`, another uncertified query**, not from "the build's own configuration" as [FINDING 40](#finding-40) states. Substitute the canister id and every derived address moves with it, and the modal's cross-check passes because the substituted canister answers consistently. `ic-config.js` already carries the trusted mainnet id list and no deposit path consults it. No DEFECTS.md id; it is scheduled from this table |
| [FINDING 35](#finding-35) | high | FIXED | 10 | `solvency` (12 tests) — **NOT RUN BY ANY TARGET**, see [H-45](DEFECTS.md#h-45). Run by hand 2026-08-06: 12 passed. Plus `invariants::solvency` on every fuzz step, which `dev.sh test` does run | [E-70](DEFECTS.md#e-70) | **the fifth cross-agent defect.** An observation record for every deposit SUBACCOUNT and none for the MAIN account, so `admin_audit_deposit_custody` replied `(1 audited, 0 held, 0 unaudited)` on a canister holding 5 ICP. **The 2.00 ICP shortfall on mainnet table_1 is real and is not fixed by this**; nothing here edits a balance and `no_setter_was_added_to_fix_the_books` keeps it that way |
| [FINDING 36](#finding-36) | high | FIXED | 10 | `solvency::components_sum_to_total` — **NOT RUN BY ANY TARGET**, see [H-45](DEFECTS.md#h-45). Run by hand 2026-08-06: 12 passed | [E-72](DEFECTS.md#e-72) | the harness's `CustodyStatus` mirror was missing the field carrying FINDING 29's money, under a comment saying it is "Mirrored in FULL on purpose" |
| [FINDING 37](#finding-37) | high | FIXED | 10 | `dev.sh test` → `fuzz` (`invariants::solvency::check_solvency_report_is_coherent` on every step) | [E-73](DEFECTS.md#e-73) | `total_liability()` — the last custody guard's only input — had one caller, no query, no surface and no gate, so the only way to sample it was to attempt the destructive operation it guards |
| [FINDING 03](#finding-03) | medium | FIXED | 11 | CI `candid-check` → `./scripts/check-candid.sh --declarations` (0 structural differences, verified 2026-08-06) | [E-08](DEFECTS.md#e-08) | the committed `table_canister.did` did not describe the deployed code. The `.did` ships on-chain as `candid:service` metadata, so a stale one is a canister lying about itself |
| [FINDING 11](#finding-11) | low | BY-DESIGN | 7 | `dev.sh test` → `deposit_subaccount_anchor::dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up` + `deposit_surface::dust_at_the_published_address_is_visible_and_recovered_by_topping_up_the_same_address` — **both RUN as of wave 11**, [H-45](DEFECTS.md#h-45) instance closed | [E-12](DEFECTS.md#e-12) | dust at or below the transfer fee in a deposit subaccount cannot move alone. The arithmetic cannot be changed; it is visible on every surface and recoverable by topping the same address up. **Wave 11 re-drove it through the 64-hex address a player is actually given, and wired the target: its gate had been outside `dev.sh test` since wave 8** |

Detailed write-ups that predate this file live alongside it:

- `docs/FINDING-01-chip-destruction.md` — CRITICAL, confirmed on a local replica: every
  showdown with post-flop betting paid the winner only the pre-flop pot and permanently destroyed
  the rest. **FIXED 2026-08-04**; that document now opens with the fix, the gate that keeps it
  fixed, and what was reverted in a copy of the repo to prove the gate can fail.
- `docs/BACKEND-FUND-SAFETY-TODO.md`

---

<a id="finding-02"></a>
## FINDING 02 — `state.pot` is the sole authority for side-pot totals, in both directions — STATUS: FIXED (wave 2)

> **WAVE 11 RE-VERIFICATION.** Refuted structurally, which is stronger than any run: the two
> routines this finding is about -- `poker_core::side_pots::build_side_pots_logged` and
> `apply_side_pots`, the ones that reconcile against `state.pot` and let it win unconditionally
> -- have **no caller left in `src/table_canister/src/lib.rs`**. They survive only as the archive
> of what the defect was, pinned by ~1,500 golden vectors. A runtime test can only say the
> mint/destroy did not happen on the hands it played; this says there is no edge from the
> canister to the code that can do it.
>
> ```text
> cd tests/money_safety && cargo test --test oldest_cluster -- finding02 --nocapture
> ```
>
> `finding02_the_state_pot_authority_has_no_caller_in_the_canister` reads the engine source with
> comments stripped and fails on any call to either routine, and requires
> `build_side_pots_from_contributions` to be the builder for BOTH ladders (the displayed
> breakdown and the payout plan). Reverting either call site turns it red. `state.pot` survives
> as a redundant accumulator whose only power is to make the canister complain
> (`report_pot_disagreement`); it can no longer decide what anybody is paid.

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

<a id="finding-03"></a>
## FINDING 03 — the committed `table_canister.did` does not describe the deployed code — STATUS: FIXED (wave 11)

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

<a id="finding-04"></a>
## FINDING 04 — latent: `detect_straight` prefers the wheel over a higher straight — STATUS: OPEN

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

<a id="finding-05"></a>
## FINDING 05 — `leave_table()` makes FINDING 02 Direction A ATTACKER-REACHABLE — STATUS: FIXED (wave 2)

> **WAVE 11 RE-VERIFICATION**, on a ladder built for the purpose: one 2 ICP all-in under three
> 10 ICP stacks, everybody to 5 ICP, so the pot has two layers and a departing deep stake has
> somewhere to move to. A deep seat off the clock calls `leave_table` mid-hand:
>
> ```text
> FINDING 05/08 refuted via leave_table: stake 500000000 stayed in the basis; layer amounts
> [800000000, 900000000] unchanged, eligibility [[0,1,2,3],[1,2,3]] -> [[0,2,3],[2,3]]
> ```
>
> The eligibility changing is correct -- leaving relinquishes the claim. **The layer AMOUNTS not
> changing is the finding**: this defect moved money out of the main pot the short all-in can win
> and into the pot only the deepest stacks can, conserving every total while it did it, which is
> why no conservation check could ever see it. `finding05_and_08_neither_door_out_of_a_seat_moves_a_chip_between_pot_layers`
> in `tests/money_safety/tests/oldest_cluster.rs`, door `leave_table`.

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

<a id="finding-06"></a>
## FINDING 06 (NEW) -- `notify_deposit` can never credit a deposit, and the ICP it was sent is stranded forever — STATUS: FIXED (wave 2)

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

> ## ✅ RE-VERIFIED IN WAVE 11, ON THE CURRENT TREE, BOTH BUGS, BOTH DIRECTIONS.
>
> This finding was re-opened on suspicion in wave 11 -- *"the path cannot succeed, and ICP sent
> that way is stranded"* -- and the suspicion is **wrong on the current tree**. Executed, not read:
>
> ```
> alice sends 1 ICP to the canister's MAIN account, block 2
> notify_deposit(bob,   2) -> Err(not sent from your account)      a stranger cannot claim it
> notify_deposit(alice, 2) -> credited 100000000                   the sender can, in full
> ```
>
> `deposit_surface::money_at_the_shared_main_account_is_recoverable_by_its_sender_and_by_nobody_else`,
> alongside the wave-2 gate `deposit_replay::dr01_notify_deposit_credits_a_real_transfer_exactly_once`.
>
> **BOTH bugs were reverted separately in a `cp -Rc` copy and both turn a gate red.**
>
> | revert | what the canister says | gates that go red |
> |---|---|---|
> | BUG A: `response.candid::<(QueryBlocksResponse,)>()` | `Failed to decode ledger response: CandidDecodeFailed { type_name: "(…QueryBlocksResponse,)", candid_error: "Fail to decode argument 0" }` | `dr01`, `dr00`, `dr04`, `dr07`, `dr09`, and `money_at_the_shared_main_account_…` |
> | BUG B: the `Transfer` arm's type mismatched, so `operation : opt Operation` decodes to **null with no error** | `Transaction is not a transfer` -- the SILENT failure mode this finding warned about | `money_at_the_shared_main_account_…` |
>
> **The endpoint was NOT removed, and here is the argument.** The instruction was to make it work
> or remove it, on the grounds that an endpoint which takes a block index and can never credit it
> is worse than none. It credits. And it is the ONLY door to money at the canister's shared main
> account -- an account that carries no name, so a block index is the only evidence of ownership
> such a transfer ever has. Removing it would strand the ICP sitting there now, including
> everything sent to the shared address [FINDING 34](#finding-34) published. What was removed is
> the claim that it is a *deposit path*: its `.did` said *"Players should first transfer ICP to
> the canister's account, then call this with the block index"*, and it now says it is a recovery
> door, names the two conditions under which it cannot help, and points at
> `get_deposit_address()` + `claim_external_deposit()` as the route.

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

## FINDING 07 -- one controller call destroyed 100% of a funded table's chips — STATUS: FIXED (wave 7)

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

## FINDING 20 (high) -- a controller could re-denominate a funded table, making every balance unpayable — STATUS: FIXED (wave 7)

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

**THE FIX WAS INCOMPLETE, AND IS NOW COMPLETE (2026-08-06).** "Escrow + chips + pot" is not
everything the canister holds. [FINDING 21](#finding-21) walked the surviving hole, demonstrated it
at 5 ICP, and is now closed: `total_liability()` includes every deposit subaccount this canister
has read, and the guard additionally refuses while any address it published has never been read.
Read FINDING 21 for the fix and for why the arithmetic half alone would not have been enough.

<a id="finding-21"></a>
## FINDING 21 (high) -- the FINDING 20 guard measures the wrong total, so a funded table can still be re-denominated — STATUS: FIXED (wave 8)

**Severity:** HIGH. Controller-only, fund LOCK. Recoverable only while the table stays empty.
**Status:** FIXED 2026-08-06 (wave 8), gated by `coherence_w8::finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible`, which `./scripts/dev.sh test` names. *(This line read `OPEN` for five waves after the header said FIXED; corrected in the wave-11 register pass.)* Found by the wave-7 critic while attacking the FINDING 20 fix. Demonstrated
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

### THE FIX (2026-08-06) -- and the part of it that is not arithmetic

Both halves are re-anchored, and a third thing had to be built that neither reviewer named.

**1. `total_liability()` now includes the deposit subaccounts.** It is
`escrow + chips + pot + observed_deposit_total()`, where the last term is what the ledger last told
this canister was sitting in its own subaccounts. The currency guard therefore refuses on a table
whose only money is at a published deposit address. Gate:
`deposit_subaccount_anchor::the_currency_guard_refuses_on_a_table_funded_only_through_a_deposit_subaccount`.

**2. `check_no_orphaned_custody` is anchored to every account.** It is now
`(ledger_main + ledger_deposit_subaccounts) - claims - exemptions`, and `claims` includes the
canister's OWN report of its deposit custody, read from `admin_get_deposit_custody()` and never
computed by the harness from the ledger. That keeps the two sides independent: money in a
subaccount the canister knows about is *owed*; money in one it does not know about is *orphaned*.
Both directions are gated in
`deposit_subaccount_anchor::the_orphan_invariant_is_red_on_money_held_only_in_deposit_subaccounts`,
driven on the finding's own numbers (7 ICP, 0 hits before).

**3. UNKNOWN IS NOT ZERO, and this is the part the arithmetic fix alone would have missed.** A
query cannot call the ledger, so the only way this canister can know what is at one of its
subaccounts is to ask from an update and write the answer down. Re-anchoring the guard to
"what has been written down" leaves the whole attack intact one step earlier: a controller who
flips the currency **before anybody looks** strands the same 5 ICP by the same mechanism, and
`total_liability()` reads zero *correctly*, because the balance is genuinely unknown.

So the guard has a second leg. A currency change is refused while any deposit address the canister
can enumerate has never been read, and the refusal names the remedy
(`admin_audit_deposit_custody`, controller-only, reads the ledger, moves nothing). Measured:

```text
  victim deposits 3 ICP and withdraws all of it   -> the canister owes 0, and knows this principal
  victim's external wallet sends 5 ICP to their deposit address (no message to the canister)
  admin_update_config(currency = BTC)
      -> Err "...1 deposit address(es) this canister published have never been read..."
  admin_audit_deposit_custody([])  -> read 1, observed 500000000, unaudited 0
  admin_update_config(currency = BTC)
      -> Err "...while it still owes players 5.0000 ICP..."
```

Gate: `deposit_subaccount_anchor::the_currency_guard_refuses_while_a_published_address_has_never_been_read`.

**THE ENUMERABILITY LIMIT, WRITTEN DOWN RATHER THAN PAPERED OVER.** A deposit subaccount is
`sha256("cleardeck-deposit:" || principal)` -- a pure function of a principal -- so the set of
these accounts is as large as the set of principals and **this canister cannot enumerate it**.
What it can enumerate is every principal it holds escrow for, has seated, or has already observed.
A principal who derives the address off-chain and funds it without ever calling the canister is
outside that set. Three things follow, all of them deliberate:

* the derivation is written down in the code, in the "THE ACCOUNT CENSUS" comment block at the top
  of the deposit section of `lib.rs`, so an operator can compute the address for any principal and
  check it against the ledger by hand;
* `admin_audit_deposit_custody(also: vec principal)` takes an operator-supplied list, so a
  principal learned from a support ticket or from the ledger's own log can be brought into the
  census permanently -- reading an account writes an entry for it;
* the harness asserts the limit rather than hiding it: in
  `the_canisters_deposit_books_match_the_ledger_per_principal`, an audit with an empty `also` list
  on three funded-but-never-seen principals is required to report `read = 0, found = 0`, because
  "I have not looked" is the honest answer and "the accounts are empty" would be a lie.

<a id="finding-22"></a>
## FINDING 22 (medium/high) -- the FINDING 07 fix gives a controller a new power: void any live hand after reading every hole card — STATUS: FIXED (wave 11)

> ## ✅ ANSWERED AND FIXED 2026-08-06 (wave 11). REPRODUCED ON THIS TREE FIRST.
>
> **Reproduced**, on module `792a9487…`, by `oldest_cluster::finding22_a_controller_can_void_a_live_hand`
> before any change:
>
> ```text
> controller sees: phase=Flop pot=6000000 cards=[(0, Jd/Ac), (1, Th/5d), (2, 6d/3h)]
> admin_return_all_chips_to_escrow -> Ok
> after: phase=Flop pot=0 action_on=2 cards_still_dealt=3
>   ...dae  1998000000 -> 2000000000  (+2000000)
>   ...oae  1998000000 -> 2000000000  (+2000000)
>   ...7ae  1998000000 -> 2000000000  (+2000000)
> invariant violations: 0
> hand 1 in the permanent record: winners: [], community_cards: [], participants: None
> ```
>
> A contested hand on the flop, every hole card readable by the caller, ended by one controller
> call. Every player back on exactly their buy-in, **every invariant silent**, the table left
> mid-street with cards on the board and every stack at zero, and nothing anywhere saying it
> happened.
>
> ### THE TRADE-OFF, STATED
>
> | keep the power | remove the power |
> |---|---|
> | A table can always be recovered. `reset_table` REFUSES while the table holds custody and points at this door; it is the conserving primitive FINDING 07 needed and did not have. | A controller who has read every hole card cannot decide whether a hand happens. |
> | The failure it recovers from is a hand no message can move. If the recovery door can only be opened by a predicate a TRAPPING canister cannot satisfy, then a trapping canister has no privileged escape either — and every door shutting at once over real money is [FINDING 15](#finding-15), which cost about 420 ICP of reachability. | The cost is not principal. It is the equity a player has already paid for: a confederate drawing dead is made whole, the player who was going to win the pot is not. Conservation is exact, so **nothing else in this project can see it happen.** |
>
> ### THE DECISION: KEEP IT, NARROW THE WINDOW, MAKE EVERY USE PERMANENT
>
> **1. The door refuses while the hand can still be played.** The gate is
> `hand_cannot_move_right_now(state, now)` — the action clock has run out, or there is no clock
> at all — and **not** `hand_is_stuck`. That choice is the whole of the FINDING 15 argument
> above: `hand_is_stuck` needs the STALL WITNESS, which `check_timeouts` only records *after*
> `advance_table_clock` returns, so a hand whose resolution path traps cannot accumulate one by
> that route, and the on-chain timer is the very thing that may be missing. `now > expires_at` is
> pure state; no trap, no lost timer and no missing witness can stop it becoming true. The cost
> to an honest operator is bounded by one action timeout.
>
> ```text
> Refusing: hand 1 is being played right now -- seat 2 is on the clock with 6000000 in the
> pot -- and this is a RECOVERY door, not a way to end a hand. [...] Wait for the action
> clock to run out -- at most 30 seconds -- or call check_timeouts, and then try again.
> ```
>
> **2. A hand it DOES reach is closed, through the permissionless routine.**
> `settle_unmovable_hand` — the same function `abandon_stuck_hand` and the on-chain clock use —
> so the money moves identically and the controller is doing what any principal could already do.
> This also fixes the "Related, same function" half below, which was a plain bug: the door used
> to empty the pot and leave `phase`, `action_on` and every hole card untouched, so the table sat
> mid-street with every stack at zero and `reload` refused *"Cannot reload during a hand"*.
>
> **3. Every use is in the permanent record, distinguishably.** A new `HandEnding` carried into
> `record_hand_to_history` writes `HistoryWinnerRecord::pot_type` — a field that already exists
> and already crosses to the archive canister, so **no Candid type changed anywhere**. It was the
> constant `"main"` on every credit of every hand, including hands nobody won. It now says which:
>
> | ending | `pot_type` on every credit |
> |---|---|
> | showdown / fold-out | `main` |
> | `abandon_stuck_hand`, the clock, an exit door | `refund:hand-could-not-be-moved` |
> | the last player left | `refund:nobody-left-to-win-it` |
> | **a controller ended it** | **`refund:ended-by-controller`** |
>
> and the canister says so at `CRITICAL:`, which `documented::TOLERATED_SELF_REPORTS` (empty)
> makes a run-stopping self-report:
>
> ```text
> CRITICAL: hand 1 was ENDED BY A CONTROLLER at phase flop: the recovery door
> (admin_return_all_chips_to_escrow / admin_reinit_table) was called on a live hand that
> nothing could move. 6000000 e8s across 3 stakes returned to the players who put them in;
> NOBODY WON THIS HAND. Every credit is recorded with pot_type="refund:ended-by-controller"
> so the permanent record distinguishes it from a hand that was played out.
> ```
>
> **4. And a record defect found on the way.** `settle_unmovable_hand` never wrote the local
> 100-hand ring, because that write lived inline in `settle_hand`, which it does not call. So
> **every** hand ever refunded — by `abandon_stuck_hand`, by the clock, by an exit door — read
> back from `get_hand_history` as `winners: [], community_cards: []`: a blank record of a hand in
> which real money moved back to real people, while the archive canister had every credit. The
> two records disagreed about every refunded hand. It is now one function,
> `record_local_hand_result`, with two callers.
>
> ### The gates
>
> | gate | where | goes red when |
> |---|---|---|
> | `finding22_the_recovery_door_refuses_a_hand_that_can_still_be_played` | `tests/money_safety/tests/oldest_cluster.rs` | either door ends a contested hand somebody is on the clock for; or a refused call moves a chip, a phase or a card |
> | `finding22_a_hand_a_controller_does_end_is_closed_and_permanently_marked` | same | the recovery stops working on an unmovable hand; the hand is left open; the `CRITICAL:` line or the `refund:ended-by-controller` label goes missing; the hand's credits stop summing to what it collected |
> | `admin_reinit_table_mid_hand_returns_the_pot_to_the_players_who_put_it_in` | `tests/money_safety/tests/admin_custody.rs` | **this test used to PIN the defect** (`admin_reinit_table mid-hand must succeed`); it now drives both halves |
>
> **What is NOT claimed.** A controller can still wait one action timeout and end a hand that has
> stalled, having read the cards. That is the residual, and it is the price of a table that can
> always be recovered. What it can no longer do is end a hand that is being played.

**Severity:** MEDIUM-HIGH. Controller-only. Conserves money to the e8, so **no conservation
invariant in this project can see it.** It is not a loss of principal; it is a loss of the
equity a player has already bought.
**Status:** **FIXED 2026-08-06 (wave 11)**, reproduced on this tree first. Introduced by the
FINDING 07 fix; read below for what the defect WAS.
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
## FINDING 23 (critical, unguarded by design) -- `uninstall_code` and `install_code --mode reinstall` are FINDING 07 at full scale, and the wave-7 audit does not cover them — STATUS: OPEN

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

<a id="finding-08"></a>
## FINDING 08 (NEW) -- `cash_out` is a second door into FINDING 05's orphaned-stake state — STATUS: FIXED (wave 2)

> **WAVE 11 RE-VERIFICATION, BY THE DISCONNECT ROUTE ITSELF.** Same two-layer ladder as
> FINDING 05. The seat on the clock goes quiet, nobody calls anything on its behalf, its own
> action clock folds it (`check_timeouts -> PlayerTimedOut`), and only then does it `cash_out`
> with the hand still live:
>
> ```text
> FINDING 05/08 refuted via timeout_then_cash_out: stake 500000000 stayed in the basis; layer
> amounts [800000000, 900000000] unchanged, eligibility [[0,1,2,3],[1,2,3]] -> [[0,1,3],[1,3]]
> ```
>
> `finding05_and_08_neither_door_out_of_a_seat_moves_a_chip_between_pot_layers`, door
> `timeout_then_cash_out`. The test asserts BOTH doors ran (`doors_exercised == 2`), so a change
> that makes `cash_out` refuse this state silently -- and therefore stops measuring it -- is a
> failure rather than a quieter pass.
>
> ### AND THIS FINDING FOUND A HOLE IN THE ORACLE THAT WAS BIGGER THAN THE FINDING
>
> The settlement oracle -- the only instrument here that asks *who was PAID* rather than *do the
> totals balance* -- **had never executed `cash_out` or `check_timeouts` at all.** Every vacating
> scenario in `tests/settlement/src/suite.rs` was written as *"`leave_table`, and `cash_out` if
> that fails"*, and `leave_table` never fails for a seated player. So across 53 compared hands,
> the door this finding is about was never opened, and the only gates on it (`reg08`, M1b) ask
> whether the pot is still fully ATTRIBUTED -- not whether the right seat was PAID. Those two
> questions come apart exactly where [FINDING 13](#finding-13) lives.
>
> Closed by `suite::timed_out_seat_cashes_out_mid_hand` (now in `run_all`) and
> `a_seat_folded_by_its_own_clock_and_cashed_out_mid_hand_settles_by_the_rules`. Measured:
>
> ```text
> --- seat_timed_out_then_cashed_out_mid_hand | hand #1 | button seat 1 | 6 seats
> board: 8c Td Jc 7d 7s
> seat  hole    contributed  folded  left   engine_delta  oracle_delta  DIFF
>    0  3s 9c            20      no    no            60            60      +0
>    1  Ah 4s            60      no    no           -60           -60      +0
>    2  Ad 5s            60     yes   yes           -60           -60      +0   <- clock folded it, then cash_out
>    3  8s Tc            60      no    no            60            60      +0
> oracle pot layers:
>   layer 0 (0..20]  amount 80   eligible [0, 1, 3]  winners [0]
>   layer 1 (20..60] amount 120  eligible [1, 3]     winners [3]
> collected 200   oracle owed 200   engine paid 200   destroyed 0
> ```
>
> The short all-in takes the main pot the departed stake helped fund, which is the exact
> allocation the defect used to break.

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

<a id="finding-09"></a>
## FINDING 09 (NEW) -- FINDING 02 Direction B is reached in ordinary play, and the canister says so — STATUS: FIXED (wave 2)

> **WAVE 11 RE-VERIFICATION.** Four multi-street hands, three seats, a bet on every street:
>
> ```text
> FINDING 09 refuted: 4 hands, 32 mid-hand cross-checks, 0 BUG:/CRITICAL: lines out of 7 log lines
> ```
>
> The gate asserts both halves and neither alone would be enough. The log half -- no `BUG:` and
> no `CRITICAL:` line -- is what this finding was originally reported by, and on its own it is
> only the absence of a complaint. The other half is the property the complaint was a proxy for:
> at **every** observation while a hand is live, `state.pot` must equal the whole payout basis
> (seated stakes plus departed stakes). `finding09_the_engine_never_reports_a_pot_it_settled_against_anyway`
> also requires more than 10 such observations, so a driver that stops reaching real hands fails
> instead of reporting a quiet run.

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

## FINDING 10 -- a deposit block could be credited twice, and the excess withdrawn — STATUS: FIXED (wave 2)

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
## FINDING 11 (NEW, low) -- dust at or below the transfer fee in a deposit subaccount can never be swept — STATUS: BY-DESIGN (wave 7)

> **SUPERSEDED IN SCOPE BY [FINDING 28](#finding-28).** This finding records the dust half only.
> The third independent auditor showed that money in a deposit subaccount is invisible to **every**
> balance surface at any size, not only below the fee, and that the error text tells the player to
> send more money to the address already holding theirs. Read FINDING 28 first.

**Severity:** LOW. Bounded by the fee (10,000 e8s per player per stuck deposit).
**Status:** The *arithmetic* is unchanged and cannot be changed: an ICRC-1 transfer of an amount at
or below the ledger's own fee is not a transfer any code can make. What was actually wrong was the
other two thirds of it, and both are now closed. Gates:
`deposit_subaccount_anchor::dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up`
and `::the_claim_refusal_may_not_say_there_is_nothing_when_there_is`.
**Where:** `claim_external_deposit`, `src/table_canister/src/lib.rs`

> ## ✅ RE-VERIFIED IN WAVE 11, THROUGH THE DOOR A PLAYER IS ACTUALLY GIVEN — AND ONE REAL GAP WAS FOUND AND CLOSED.
>
> Wave 8's top-up path does close this. Re-driven end to end on the current tree, this time
> through the 64-hex address `get_deposit_address()` hands out and the ledger's LEGACY `transfer`
> endpoint -- not through `icrc1_transfer`, which is the only door the wave-8 gate had ever used:
>
> ```
> alice sends 9,999 e8s (one below the fee) to the address she was given
>   claim_external_deposit -> Err("Nothing was swept, and you are NOT empty-handed. 0.0001 ICP
>     (9999 e8s) of yours is at the deposit address this canister published for you. … sending
>     2 e8s or more to the SAME address makes the whole balance claimable …")
>   get_deposit_custody().observed_amount = 9999,  get_custody_status().total = 9999
> alice tops the SAME address up by 2 ICP
>   claim_external_deposit -> Ok, credited 199_999_999 = (9_999 + 200_000_000) - 10_000
> ```
>
> **The gap was not in the fix, it was in the gate.** `deposit_subaccount_anchor.rs` -- the
> eleven-test target that carries this finding's only evidence -- was cargo-auto-discovered and
> named by **nothing**: not `tests/money_safety/Cargo.toml`, not `scripts/dev.sh test`. FINDING
> 11's gate had been sitting outside every target anybody runs since wave 8, which is
> [H-45](DEFECTS.md#h-45) exactly. It is now named in both, alongside the new
> `deposit_surface::dust_at_the_published_address_is_visible_and_recovered_by_topping_up_the_same_address`,
> which drives the identical arithmetic through the address a player is given rather than the one
> the harness happened to use.

The original:

```rust
if balance <= transfer_fee {
    return Err("No claimable balance. ...");
}
```

Three separate things were wrong and only one of them was physics:

| | what | state now |
|---|---|---|
| 1 | the dust cannot be moved by a transfer | **unchanged, and unchangeable.** An amount at or below the fee cannot pay its own fee |
| 2 | no surface reported it, so it was silently gone | **closed.** It is in `get_custody_status.unswept_deposit` and in `total`, in `get_deposit_custody()`, in `admin_get_deposit_custody()`, and in `total_liability()` |
| 3 | the refusal told the player to send money to an address already holding theirs, and did not say the money was still there | **closed.** The refusal now states the amount, the fee, why no transfer can move it, that it is not lost, and the exact top-up that makes it claimable |

**And it is recoverable, which nobody had checked.** Dust is stuck *on its own*; it is not stuck.
Sending anything to the SAME address that takes the total past the fee makes the whole balance
sweepable, dust included, and the refusal now says so with the exact number. Driven end to end:

```text
  alice sends 9,999 e8s (one below the fee)
    claim_external_deposit -> Err, and get_custody_status.unswept_deposit = 9999, total = 9999
  alice sends 2 ICP to the same address
    claim_external_deposit -> Ok, credited 199_999_999 = (9_999 + 200_000_000) - 10_000
```

The 9,999 e8s came out with the top-up. The honest description of dust is therefore
**"immovable alone, visible always, recoverable by topping up"**, and the one option that was
being taken before -- silent -- is the one that is no longer available.

<a id="finding-12"></a>

## FINDING 12 -- one action timeout ends the hand for everybody, because the disconnect timeout and the action timeout are both 30 seconds — STATUS: FIXED (wave 5)

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
## FINDING 13 (high) -- the E-05 fix pays a departed player's stake to whoever takes their chair — STATUS: FIXED (wave 3)

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
## FINDING 14 (high) -- two agents each added a persisted field that is not a Candid-compatible addition, and one of them destroys every chip at the table SILENTLY — STATUS: FIXED (wave 3)

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
## FINDING 15 (critical), one unguarded evaluator call locks a funded table: every path to a player's own money closes at once — STATUS: FIXED (wave 6)

> ### WAVE-11 REGISTER PASS, 2026-08-06 — this finding was CLOSED IN WAVE 6 and its header said nothing about it for five waves
>
> The heading now reads `STATUS: FIXED (wave 6)`. It did not, for the whole time it was closed:
> the fix landed, an independent auditor re-ran it and could not reproduce it, and the one line a
> reader scans stayed silent. That is the defect this register pass exists to remove, and this is
> the entry it was named after.
>
> **The gate, re-verified 2026-08-06 by running it:** `wave6_coherence::probe1`
> (`the auditor's lock, reached by real silence`) and `invariants::reachability` (M9) on every
> fuzz step, both inside `./scripts/dev.sh test`, which was green on
> `sha256 c05fbdc6e7a46b80d4ace7591819429e09ad32650c5763ed5e904a8906360a45`.
> **Note also [H-48](DEFECTS.md#h-48):** M9's own dedicated file, `fund_reachability.rs`, is red
> and no target runs it. The gates above are not that file and were green.

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
## FINDING 16 -- a player who stops answering keeps a claim on the pot without paying for it — STATUS: FIXED (wave 5)

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
## FINDING 17 (high) -- a hand can be settled with NO live claim on it, and every stake handed back to the players who folded — STATUS: FIXED (wave 6)

**Severity:** HIGH. Not fund theft and not fund loss: conservation is exact and every e8 goes
back to the principal that put it in. What is lost is the POT, by the player who won it, to an
ordinary disconnection. And what is lost more broadly is the property this wave was commissioned
to establish — that the table can no longer be wedged into a state the rules of poker cannot
describe.
> **WAVE-11 REGISTER PASS, 2026-08-06 — CLOSED IN WAVE 6, header corrected now.** The heading
> reads `STATUS: FIXED (wave 6)`. This finding was closed by making participation and eligibility
> ONE function (`live_claims` calls `is_in_hand`; `is_in_hand` requires cards) — see
> [E-36](DEFECTS.md#e-36) — and its header carried no status at all until this pass.
> **The gate, re-verified 2026-08-06 by running it:** `wave6_coherence::probe4`
> (`the fold-out winner is paid the pot`), which is an OUTCOME assertion rather than a totals
> assertion, because the totals were exact while this defect was live. It is named explicitly in
> `./scripts/dev.sh test`, which was green.

**Status (as found):** DEMONSTRATED by execution on the wave-6 module `5e07edf6…`, twice, by two different
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
## FINDING 18 (high) -- `cash_out` of a stuck hand walks away from your own stake, and nothing anywhere says so — STATUS: FIXED (wave 7)

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
## FINDING 19 (high) -- nothing on chain moves the game, so liveness is outsourced to whoever has a browser tab open — STATUS: OPEN

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
## FINDING 24 (high) -- a frozen table answers NOTHING, not "queries only": the mitigation this project has documented in four places does not exist — STATUS: OPEN

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
## FINDING 25 (HIGH -- raised from medium in wave 7) -- after a stall, one permissionless call VOIDS the hand the clock would have played out; the player who was losing is the one with the incentive, and the canister TELLS them to do it — STATUS: FIXED (wave 7)

> ### FIXED 2026-08-06. One predicate, and it is about ATTEMPTS.
>
> Reproduced first, on the same construction, and then closed. The reproduction and the gate are
> the same file: `tests/money_safety/tests/stall_agreement.rs`, **M13 ONE BELIEF**, which forks
> **138 states** and drives each one down two arms -- the clock alone with zero ingress, and the
> same state with one door call sent at the first instant it can be sent.
>
> | measured on 138 states | before | after |
> |---|---|---|
> | rows whose two arms paid different recipients | **118** | **0** |
> | `Err` replies that changed the table anyway | **20** | **0** |
> | hands voided by a door that the clock plays out | not separately counted | **0** |
>
> The 118 carries a caveat and it is stated rather than buried: the first run demanded exact
> recipient equality on every row, including `leave_table` by a seated player, whose mid-hand use
> is an ordinary fold and legitimately changes the outcome. At most 30 of the 138 rows are that
> confound, so at least 88 were the defect. The shipped criterion is narrower and the after-run is
> zero under both.
>
> **What changed.** `clock_should_abandon` is deleted. `hand_is_stuck` is the only predicate, all
> six surfaces read it -- the on-chain clock, `abandon_stuck_hand`, `cash_out`, `leave_table`,
> `get_custody_status`/`get_stuck_hand_status`, `TableView.hand_is_unmovable` -- and it now says:
>
> > a hand is stuck when this canister, **while executing**, has handed it to the ordinary
> > resolution path in at least 3 separate committed messages spanning at least 300 s, and the hand
> > has not moved.
>
> An hour-long stall therefore buys no credit toward abandonment on any door. The evidence is a
> `STALL_WITNESS` keyed on `(hand_number, action_timer.expires_at)`, so it clears itself the instant
> the hand moves, and it is deliberately **not persisted across upgrades** -- an upgrade is exactly
> a period in which the canister was not executing.
>
> **The objection this write-up raised against the fix is answered, not ignored.** The concern was
> that coupling the permissionless escape hatch to timer state rebuilds [FINDING 15](#finding-15)'s
> fund lock, because a canister whose clock never re-armed would have a permanently closed hatch.
> The witness is therefore written from **two** places, not one: `on_clock_tick`, and
> `check_timeouts` -- which is permissionless, has no rate limit, and needs no seat. Three of those
> calls across a grace period open the door with the timer completely dead. The floor described in
> the original write-up ("refuse until the clock has been overdue for the grace period AND the
> canister has been executing for at least one grace period") is what was built, with the second
> clause measured as opportunities given rather than as time elapsed.
>
> **The `Err` that commits is gone too.** `cash_out` and `leave_table` are restructured so every
> refusal is decided before the first mutation, with the boundary marked in the source. `cash_out`'s
> in-a-hand refusal now carries the committed-stake sentence, so [FINDING 18](#finding-18)'s
> guarantee is carried by the refusal instead of by voiding a playable hand.
>
> **What a reader should check for themselves.** Three gates in
> `tests/money_safety/tests/invariants/custody.rs` and three unit tests in `lib.rs` pinned the old
> belief. They are rewritten, not deleted, and each replacement states in its own doc comment what
> it replaced. One of them had asserted that a player who **folded by leaving** could call
> `abandon_stuck_hand` and get her stake back -- E-59 in miniature, living inside the gate for
> FINDING 18.

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

### The fix, and the reason it was not obviously free

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

> **RESOLVED, 2026-08-06 -- and this paragraph is left standing because it is the objection the
> fix had to answer.** The floor is what was built. The part it got wrong is "using a first-sighting
> timestamp that `post_upgrade` re-seeds": the witness is a thread-local and is simply *lost* on
> upgrade, which is stronger and simpler -- an upgrade is a period in which the canister was not
> executing, so its evidence does not survive one, and the door reopens a grace period later. The
> dead-timer worry is answered by writing the witness from `check_timeouts` as well as from the
> tick. `check_timeouts` is permissionless and unrestricted, so the hatch is reachable by any
> player in three calls plus the grace, with the clock completely dead. It is a *slower* hatch,
> never a closed one.

### CRITIC AMENDMENT, 2026-08-06 -- "never a closed one" is false in the one state the hatch exists for

The paragraph above is right about a dead timer and wrong about a dead timer **in FINDING 15's
state**, and the two have to be separated because only the second one loses money.

`check_timeouts` writes the witness like this (`src/table_canister/src/lib.rs`):

```rust
let result = advance_table_clock(now);   // the attempt
note_stall_opportunity(now);             // the evidence
```

Both are in ONE message. FINDING 15's state is the state in which the ordinary resolution path
**traps** -- that is the whole reason an escape hatch exists at all. A trap in
`advance_table_clock` aborts the entire `check_timeouts` message, so `note_stall_opportunity`
never commits, so `opportunities` never reaches `STUCK_HAND_MIN_OPPORTUNITIES`, so
`hand_is_stuck` is false forever and `abandon_stuck_hand` refuses forever. The function's own
comment says so: *"an attempt that TRAPS takes this whole message with it, which is what the
two-message timer path exists to cover."* The two-message split exists only in `on_clock_tick`.

So the reachability of the hatch is:

| resolution path | on-chain timer | hatch |
| --- | --- | --- |
| returns normally | alive | opens (grace + 3 sightings) |
| returns normally | dead | opens via `check_timeouts` -- the paragraph above is right here |
| **traps** | alive | opens (the tick writes the sighting in its own message) |
| **traps** | **dead** | **never opens.** Every stake in that pot is unrecoverable by any caller. |

Under the wall-clock predicate this fix replaced, the bottom row opened. It is a narrow row --
it needs both a trapping engine and a lost 30 s interval timer -- but it is the exact
intersection of the two conditions this hatch was built for, and it is now the only row the
hatch does not cover.

**The one-line shape of the fix**: split `check_timeouts` the way `on_clock_tick` is already
split -- call `note_stall_opportunity(now)` FIRST, in a message that cannot trap, and dispatch
`advance_table_clock` behind it. `note_stall_opportunity` clears the witness whenever the hand
moved, so counting the opportunity before the attempt rather than after costs nothing on a
healthy table.

Not fixed here; this pass is a review, and the change belongs to the owner of that file.

### CRITIC AMENDMENT, 2026-08-06 -- what the M13 gate's green does and does not say

Re-run of `tests/money_safety/tests/stall_agreement.rs` against the shipped module,
`138 states compared / 0 voided / 0 recipient disagreements / 0 Err-replies that changed state`,
865 s -- confirmed. What the row table also shows, and what the summary line does not:

* `abandon_stuck_hand` replied `Err("This hand can still progress...")` on **42 of 42** rows;
* `cash_out` replied `Err` on **42 of 42** rows;
* the `voided` column reads `0/0` on **all 138** rows.

No door opened anywhere in the gate. The agreement it proves is agreement between *doing
nothing* and *being refused*, which is the correct answer for those 138 states but is not
evidence that the two paths agree when the predicate is TRUE -- the gate contains no such state.
`settle_unmovable_hand`, the exit-door settle and `NobodyLeftToWinIt` are consequently pinned at
unit level only.

That also leaves `fn voided()` -- which scrapes the canister log for the literal
`"ABANDONED as unmovable"` and carries the strongest property in the file -- with **no positive
control in the green run**. Change that log string and every row still reads `0/0`, every
assertion still passes, and the `one_belief` property becomes permanently vacuous with nothing
to notice. A reverted-predicate build does produce `voided=true` (measured: 95 of 138 rows), so
the check is not dead today; it is unguarded.

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

### WHAT WAS ACTUALLY BUILT, 2026-08-06

Point by point against the five above.

**1. The advice string.** `committed_stake_sentence` no longer says *"once the action clock has been
expired for 5 minutes"*. It says the hand becomes refundable *"once this canister has watched it
fail to move for 5 minutes"*, and that the canister does the refunding itself, so
`abandon_stuck_hand` is named as **the same door, open to anyone, not a faster one**. The field it
is derived from, `committed_is_stuck`, reads the one predicate.

**2. `cash_out` and `leave_table`.** Both read the one predicate, so neither can settle a hand the
clock is about to play out. `cash_out` refuses during a stall -- correctly, the hand can progress --
and its refusal now carries the committed-stake sentence, which is where FINDING 18's guarantee
lives in this state.

**3. The `Err` that commits.** Both functions do their seat lookup and their in-a-hand refusal
before the first mutation, with the boundary marked in the source
(`---- FROM HERE ON NOTHING RETURNS Err ----`). Measured on 138 states: **20 → 0**.

**4. The two arms.** They agree. Recipient disagreements: **118 → 0**.

**5. The gate.** `tests/money_safety/tests/stall_agreement.rs`, **M13 ONE BELIEF**. It is the first
thing in this repository to send an **UPDATE** through `advance_time_only`'s window, and
`World::advance_time_only`'s own doc comment now says that is the point. Per scenario it forks one
control row plus three doors × (every seat + one principal who has never sat down); over 12
scenarios that is **138 forked states**, each compared against the clock-alone arm from a state the
gate first asserts is identical in both worlds. Three properties per state:

  1. no door voided a hand the clock plays out -- read off the canister's own
     `ABANDONED as unmovable` log line, so it also binds `leave_table`, whose mid-hand use is an
     ordinary fold and legitimately changes the outcome;
  2. for the doors that are not ordinary game actions, **every principal ends with the same e8s in
     both arms** -- not the same total, the same amount each;
  3. no `Err` reply changed the table.

A second, cheap test in the same file, `m13b_no_surface_calls_a_playable_hand_dead`, gates point 1
above directly and needs no arms at all: in the stall window it reads `get_stuck_hand_status`,
`get_custody_status` and `TableView.hand_is_unmovable` **as each player**, and fails if any of them
says the hand is dead -- including if the advice string contains *"no longer be moved"* or
*"nobody can win it"*. Those are queries, which execute no round, so they can only ever be caught
by a test that opens the window; they were the sentence the losing player was shown.

**One thing this fix deliberately widened.** The on-chain clock may now act on the
live-hand-with-no-clock arm, which `clock_should_abandon` refused. That is not the automatic door
getting bolder: under the one predicate that arm also requires a full grace period of witnessed
failure, where it used to be abandonable by a caller the instant anybody looked at it. Both doors
are now strictly more conservative on that arm than the manual door was.

**A consequence worth stating plainly.** On a healthy canister, a hand is now almost never
"stuck": the ordinary resolution path folds the expired seat within a tick, so the state cannot
persist. `settle_unmovable_hand`, the exit-door settle and the `NobodyLeftToWinIt` branch are
therefore defence-in-depth for a broken engine rather than paths ordinary play reaches. That is
correct -- they are FINDING 15 escape hatches -- but it means **the harness can no longer construct
a genuinely unmovable hand end to end**, and those branches are pinned at unit level in
`stuck_hand_tests` rather than through PocketIC. A future wave that wants an integration gate on
them needs a fault-injection route into the engine.

---

<a id="finding-26"></a>
## FINDING 26 (high) -- the 226-day runway assumes nobody is hostile: a free, permissionless ingress flood burns a table 65x faster, collapsing it to about three days — STATUS: OPEN

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
## FINDING 27 (high) -- the ICP deposit floor is BELOW the withdrawal floor, so money that arrives at the product's own documented minimum can never leave — STATUS: FIXED (wave 7)

> **CLOSED.** The two floors are one number per currency, the relation
> `min_withdrawal <= min_deposit` is asserted **at compile time** so the old value is a build
> failure rather than a failing test, and a caller's whole remaining balance can always leave at
> any size the ledger can move. Six PocketIC tests drive it through the real canister
> (`tests/money_safety/tests/deposit_floor.rs`, in `./scripts/dev.sh test`). The evidence is at the
> end of this entry; everything above it is the finding as the auditor left it.

**Severity:** HIGH. Fund LOCK, no malice at any step, reachable by following the application's own
instructions. Bounded at 99,999 e8s per player per table, but unrecoverable and silent.
**Status:** CLOSED 2026-08-06. Found by the THIRD independent auditor, 2026-08-05, reproduced live
on the running replica. Recorded by the wave-7 coherence pass; it appeared in no document before.
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

### THE FIX, 2026-08-06, and the half of it the auditor's framing does not reach

**1. The relation is now a compile-time assertion, not a number somebody has to remember.**
The deposit floor was an anonymous literal inside `deposit()` (`if currency == Currency::BTC
{ 1_000 } else { 20_000 }`), which is *why* nothing in the tree could compare it against
`ICP_MIN_WITHDRAWAL_AMOUNT`. It is a named constant per currency now, with a
`Currency::min_deposit()` beside the existing `min_withdrawal()`, and:

```rust
const _: () = assert!(ICP_MIN_WITHDRAWAL_AMOUNT <= ICP_MIN_DEPOSIT_AMOUNT, "FLOOR INVARIANT ...");
const _: () = assert!(ICP_MIN_WITHDRAWAL_AMOUNT >  ICP_TRANSFER_FEE,       "FLOOR INVARIANT ...");
```

Restoring `100_000` produces `error[E0080]: evaluation panicked: FLOOR INVARIANT BROKEN: the ICP
withdrawal floor is above the ICP deposit floor... This is FINDING 27.` — measured, on a copy of
the tree. `ICP_MIN_WITHDRAWAL_AMOUNT` is now `20_000`, equal to the deposit floor; the direction
matters, because raising the DEPOSIT floor to 100,000 would have made new deposits safe and left
the auditor's 80,000 e8s exactly where it was.

**2. Equal floors are not enough, and this is the part worth reading.** The finding above names the
band `[20,000, 100,000)` and the fix that closes it. But escrow balances are not only made of
deposits. An odd-chip split, a partial withdrawal, or a `claim_external_deposit` sweep that paid
the ledger fee out of the amount can each leave a balance *below any floor at all* — the finding
says so itself, one paragraph in ("the band is reachable by ordinary play"), and then proposes a
fix that does not reach it. A consistent pair of floors would have closed the door the auditor came
through and left the pot's door open, which is this project's recurring shape: the instrument was
built by people who knew where the money was supposed to have come from.

So the floor is a statement about the smallest REQUEST, never about the smallest balance that can
leave. `withdraw` waives it for the one request that cannot be a mistake — **the caller's whole
remaining balance**, at any size the ledger can move — and the refusal below that names the network
fee rather than a policy number a player cannot act on:

```text
Minimum withdrawal is 0.0002 ICP. Your whole remaining balance can always be withdrawn in
one call whatever its size, as long as it is more than the 0.0001 ICP network fee -- you
have 0.00015 ICP.
```

**3. What is measured** (`tests/money_safety/tests/deposit_floor.rs`, six tests, real canister and
real ICP ledger on PocketIC, in the default `./scripts/dev.sh test` gate):

| | |
|---|---|
| the auditor's exact sequence | deposit exactly 20,000 e8s, withdraw it, and it lands on the LEDGER. The round trip costs exactly three ledger fees (approve, `transfer_from`, `icrc1_transfer`) and **no principal** |
| the whole dead band | 20,000 / 20,001 / 50,000 / 99,999 / 100,000 all go in and all come out |
| below the floor | 40,000 in, 25,000 out, then the 15,000 residue — under the floor, over the fee, nothing to do with a deposit — swept whole |
| the waiver is narrow | a PARTIAL request under the floor is still refused, and the refusal names the way out |
| the honest boundary | a residue equal to the transfer fee cannot move, because the LEDGER cannot move it, and the refusal says so instead of quoting a policy minimum |
| no other refusal weakened | more-than-you-have, zero, and the 60-second cooldown all still bind after a sweep |

**4. The mirrors.** `MIN_WITHDRAWAL` in `WithdrawModal.svelte` is `20_000n`, and the modal mirrors
the sweep as well as the floor — a client-side floor without the waiver would have rebuilt this
finding one layer up, where no canister-side gate can see it. Its MAX button had to be made exact
for that to work: `Number('0.00012345') * 100_000_000` is `12344.999999999998`, which floors to one
unit short, and one unit short of the whole balance is not a sweep. `ui_limits.rs` reads the
relation off the canister source and fails if either modal drifts.

<a id="finding-28"></a>
## FINDING 28 (high) -- money at the canister's OWN published deposit address is reported as zero by every balance surface, and the error text tells the player to send more — STATUS: FIXED (wave 8)

**Severity:** HIGH. Fund LOCK below the transfer fee, total invisibility above it.
**Status:** FIXED 2026-08-06 (wave 8), gated by `tests/money_safety/tests/deposit_subaccount_anchor.rs` — **which no target runs ([H-45](DEFECTS.md#h-45))**; run by hand on 2026-08-06, 11 passed. *(This line read `OPEN` after the header said FIXED; corrected in the wave-11 register pass.)* Found by the third independent auditor, 2026-08-05, reproduced live. This is the
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

### THE FIX (2026-08-06)

The one structural cause and the five symptoms, each with what it says now.

**The cause: a query cannot see the ledger.** Money arrives at a deposit subaccount with *no
message to this canister at all* -- the ledger changes and the canister is not told -- and an IC
query cannot call another canister to find out. Every balance surface is a query. So the canister
now keeps a written record, `DEPOSIT_CUSTODY`, of what the ledger last said about each of its
deposit subaccounts, taken by the updates that can ask (`claim_external_deposit`,
`refresh_deposit_custody`, `admin_audit_deposit_custody`), persisted across upgrades because it is
a liability record, and reported with its own timestamp because a stale figure presented as
current is the same lie in a different font.

| symptom | now |
|---|---|
| `claim_external_deposit` -> *"No claimable balance. Send ICP to your deposit address first"* while holding the money | states the amount, the fee, why no transfer can move it, that **it is not lost**, and the exact top-up that makes it claimable. It also writes the balance down BEFORE deciding anything, so the refusal path can no longer throw away the one fact no query can discover |
| `get_custody_status()` -> `total = 0, advice = ""` | new field `unswept_deposit`, folded into `total`, with `unswept_deposit_observed_at_ns` -- `null` means NEVER ASKED, documented as not meaning "empty" -- and advice naming `claim_external_deposit()` |
| `get_balance()` -> `0` | **unchanged, deliberately, and now documented.** It is the WITHDRAWABLE figure, and it already excludes chips at the table for the same reason: both are one player-callable step away from escrow. Reporting unswept deposits here would offer a client a "withdraw everything" amount that `withdraw` must then refuse. `withdraw`'s own refusals now carry the deposit sentence, exactly as FINDING 18 made them carry the pot sentence |
| `admin_get_balance` / `admin_get_all_balances` -> `0` | **signatures unchanged** -- Candid will not let a tuple grow an element without breaking every existing reader, verified, and `tests/settlement` is one of them -- so the deposit half is a new controller surface, `admin_get_deposit_custody()`, returning the per-principal amounts, their observation times, and **the list of addresses never read**. Both old methods' doc comments now state their scope and name it |
| no instrument measured it | `total_liability()`, `check_no_orphaned_custody`, M1/M2 and `DrainReport::table_is_really_empty` are all re-anchored -- see FINDING 21 |

**New surfaces, all local, none privileged except the audit:**
`get_deposit_custody()` (query, caller-scoped), `refresh_deposit_custody()` (update, any principal,
their own address only, moves no money), `admin_get_deposit_custody()` (query, controller),
`admin_audit_deposit_custody(also)` (update, controller, reads the ledger, moves no money).

**Measured on the fixed module:**

```text
  bob's wallet sends 5 ICP to the address get_deposit_custody() published
    icrc1_balance_of(that account)     -> 500000000
    get_custody_status()               -> unswept_deposit_observed_at_ns = null
                                          ("never asked", NOT "empty")
  bob calls refresh_deposit_custody()  -> Ok, observed 500000000, sweepable
    get_custody_status()               -> unswept_deposit = 500000000, total = 500000000,
                                          advice names claim_external_deposit()
    admin_get_deposit_custody()        -> total 500000000, attributed to bob, 0 unaudited
  bob calls claim_external_deposit()   -> Ok, escrow 499990000, unswept back to 0
```

and the dust case, which is where the original sentence was worst:

```text
  bob sends exactly the fee, 10,000 e8s
  claim_external_deposit -> Err "Nothing was swept, and you are NOT empty-handed. 0.0001 ICP
     (10000 e8s) of yours is at the deposit address this canister published for you. ... at or
     below the ICP ledger's transfer fee (10000 e8s), so no transfer can move it on its own ...
     IT IS NOT LOST AND IT IS NOT FORGOTTEN ... sending 1 e8s or more to the SAME address makes
     the whole balance claimable ..."
  get_custody_status -> unswept_deposit = 10000, total = 10000
```

**Gates:** `tests/money_safety/tests/deposit_subaccount_anchor.rs`, ten tests. The two that convict
the original defect directly are `money_at_a_published_deposit_address_is_visible_on_every_surface`
and `the_claim_refusal_may_not_say_there_is_nothing_when_there_is`; the rest are one per
re-anchored instrument, phrased as properties.

**The leg that answers the standing lesson.** Every check above could pass on a canister that
attributes alice's deposit to bob: correct totals, wrong recipients, every invariant silent -- the
signature this project has produced four times.
`the_canisters_deposit_books_match_the_ledger_per_principal` compares the canister's books against
the ledger **per principal**, for three actors including one who sent nothing, and asserts each
player's own caller-scoped surface agrees with the ledger for THEIR address.

<a id="finding-29"></a>
## FINDING 29 (high) -- all three money doors move real money on the ledger before writing any record of intent, with no journal and no recovery — STATUS: FIXED (wave 8)

**Severity:** HIGH, **unbounded**. The only finding in the wave-7 audit that can cost an arbitrary
amount, and the only one no reviewer had been able to drive.
**Status:** **REPRODUCED, MEASURED, AND CLOSED.** Read from the code by the third independent
auditor, who said plainly that they could not force the trap on the local replica. It has now been
driven at all three doors, its unrecoverability has been measured door by door, and a durable
ledger-intent journal makes the same fault survivable.
**Where:** `deposit()`, `claim_external_deposit()` and -- the auditor did not name this one --
`withdraw()`, in `src/table_canister/src/lib.rs`.
**Gate:** `tests/money_safety/tests/ledger_boundary.rs` (6 tests), M14 LEDGER/BOOKS COHERENCE in
`tests/money_safety/src/fault.rs`, plus fault injection on one fuzz run in four.

### The defect, stated precisely

An IC message is atomic only **up to each await**. `deposit()` runs as two separate executions:

```text
  execution A   entry .. ic0.call_perform(icrc2_transfer_from) .. return Pending
                ^ every state change A made is COMMITTED here
  -- the ledger executes. REAL MONEY MOVES. This cannot be undone. --
  execution B   the reply callback: claim the block, credit BALANCES
                ^ if B traps, exactly B's writes are discarded. A's stand.
```

Before this fix, **A wrote nothing**. So a discarded B left money inside the canister that nothing
in the canister accounted for, belonging to somebody whose name had never been written down.

### It was driven, and the money was gone

Reproduced without instrumenting the canister and without a mock ledger, against the real mainnet
ICP ledger wasm on PocketIC. The state a trapped tail leaves behind *is*, by the atomicity rule, the
committed state at the await point with the ledger movement standing -- so the harness walks the
call to its last await, takes a **canister snapshot** there, lets the call finish so the ledger
really performs the movement, and loads the snapshot back. That discards exactly execution B.
Mechanism and its limits: `tests/money_safety/src/fault.rs`.

Measured on module `cb26fb9495fbe2084590ff087245968f33bb78460b3c2c977d06c002e632c105` (the module
the auditor reviewed):

```text
deposit(3 ICP)              ledger main 0 -> 300_000_000    escrow 0    ORPHANED 300_000_000
claim_external_deposit()    subaccount 200_000_000 -> 0, main 0 -> 199_990_000, escrow 0
withdraw(2 ICP)             escrow -200_000_000, wallet +199_990_000, pending flag STUCK
```

**Every door out of the deposit state, and what each one said.** This is the complete sweep, not a
selection:

| caller | call | reply |
|---|---|---|
| victim | `claim_external_deposit()` | *"No claimable balance. Send ICP to your deposit address first."* |
| victim | `notify_deposit(5)` -- the real block the pull wrote | *"This block is an ICRC-2 pull performed by this canister on your behalf (the deposit() flow). **It was credited to your balance when the pull happened** and cannot be credited again."* |
| victim | `withdraw(1 ICP)` | *"Insufficient balance. Have: 0.0000 ICP"* |
| victim | `cash_out()` / `leave_table()` | *"Not at table"* |
| controller | `admin_return_all_chips_to_escrow()` | no chips; nothing moves |
| controller | `admin_restore_balance(victim, 1 ICP)` | **method does not exist** (deliberately deleted) |

`alice` started with 1,000,000,000,000 e8s and ended with 999,699,980,000. **300,000,000 e8s of a
player's money, inside the canister, reachable by nobody.** The auditor's prediction that
`notify_deposit`'s sentence would be a lie in exactly this state is confirmed verbatim.

### `withdraw()` is the same shape and the auditor did not name it

`withdraw` debits escrow and sets `PENDING_WITHDRAWALS` **before** the await, so both are committed
at the await point and a discarded continuation does not undo them. Measured: the money reached the
player, and one hour later an unrelated withdrawal was still refused with *"A withdrawal is already
in progress"*. `withdraw` is the only door from escrow to the ledger, so **a discarded withdraw
continuation was a permanent, unbounded fund lock** (M9), on top of a refund path that only ever
existed inside the continuation that had just been discarded.

### The literal trap was NOT forced, and here is exactly what was tried

The reproduction above is faithful to the **consequence**. It does not prove a trap is reachable on
mainnet. Four mechanisms were tried and all four failed; recorded so nobody repeats them
(`TRAP_FORCING_ATTEMPTS` in `tests/money_safety/src/fault.rs` prints them on every run):

| attempt | measured reason it did not work |
|---|---|
| `wasm_memory_limit` squeeze | set to the canister's exact `memory_size` (6,946,875 bytes), and to +1 and +64 KiB. All three deposits completed and credited: the limit only bites on `memory.grow`, and neither half of `deposit()` grows the heap |
| out-of-cycles in the callback | a whole deposit costs **12,565,638** cycles net, but making the outbound call **reserves 42,109,265,417** for the response and the callback executes out of that reservation. Any balance small enough to starve B is too small for `call_perform` to succeed at all, so no money moves |
| upgrade inside the window | `install_code` on a canister with an open call context does drop the callback, but the window is not addressable: the ledger movement and the credit land in the **same** round, and the 2.4 MB module needs a chunk upload that costs more rounds than the window is wide |
| a fault-injecting ledger stub | a callee cannot make its caller's callback trap. The nearest thing it can do is reply undecodably, and ic-cdk 0.19 turns a decode failure into `Err((CanisterError, ..))`, **not** a trap (`api/call.rs::decoder_error_to_reject`) |

**The trap is not the only trigger, and that is the more important point.** The last row reaches the
identical end state through `deposit()`'s ordinary `Err` branch: a call-level error is not evidence
that nothing happened -- the ledger may well have executed and the reply been lost or undecodable --
and the old code returned a tidy error message and forgot. So did an upgrade with a call in flight,
which is an ordinary operational act. **The fix is required whether or not a trap is reachable.**

### The fix: a durable ledger-intent journal, and the ledger's own deduplication

`src/table_canister/src/lib.rs`, section "THE LEDGER-INTENT JOURNAL".

1. **Nothing performs an irreversible ledger movement unless an entry naming the OWNER, the AMOUNT
   and the exact WIRE ARGUMENTS has already been committed.** The write happens before the await, so
   it is committed at the await and survives a discarded continuation.
2. **The retry is safe because the LEDGER decides, not our bookkeeping.** A discarded continuation
   never learned the block index, so the journal does not try to look it up: it re-issues the
   identical transaction. ICRC-1/ICRC-2 deduplicate on the whole transaction including `memo` and
   `created_at_time`, so a movement that already happened comes back `Duplicate { duplicate_of }` --
   a positive answer carrying the block index the first attempt never saw -- and one that never
   happened is simply performed. Exactly-once, by construction. The intent stores the memo and the
   created_at_time and every attempt reproduces them byte for byte.
3. **Retirement is the once-only token.** `take_ledger_intent` removes the entry and the credit
   happens in the same message with no await between, exactly like `claim_deposit_block`. A
   concurrent resolver finds the entry gone and credits nothing.
4. **`resolve_my_ledger_intents()` and `resolve_ledger_intent(id)`** -- any principal, their own
   entries; a controller, anyone's. This is the resume path.
5. **The lease is an efficiency guard and is deliberately short (30 s).** Two callers driving one
   entry can neither double-move it (the ledger deduplicates) nor double-credit it (the removal is
   atomic). A long lease would be a second lock for a discarded continuation to get stuck behind,
   which is the defect wearing a hat.
6. **Persisted as `opt` in `PersistentState`**, with `next_intent_id` restored monotonically (an id
   is the ledger memo; reusing one would make two different movements look like one transaction)
   and leases dropped on the way in (their holder was a message in a module that no longer exists).
   An upgrade is one of the events that creates these entries, so losing them there would
   reintroduce the defect at the moment it is most likely to fire.
7. **Bounded** -- 4 open entries per principal, 512 in the canister -- and the bound is enforced by
   **refusing to start**, never by dropping an entry. Dropping an entry is the forgetting this
   finding is about.
8. **`transfer_tokens()` was deleted, not left unused.** It sent `created_at_time: None` and
   `memo: None`, which is a transaction the ledger cannot deduplicate, so a retry of it is a second
   real payment. A fund canister with an un-deduplicable transfer helper in it is one call site away
   from having the defect back.
9. **Both lying sentences now consult the journal.** `notify_deposit`'s *"it was credited when the
   pull happened"* and `claim_external_deposit`'s *"send ICP to your deposit address first"* each
   name the open entry and `resolve_my_ledger_intents()` instead. `get_custody_status` carries a new
   `unfinished_ledger_ops` field in the record and in `total`, so this fourth place a player's money
   can be is visible on the surface whose whole job is to say where their money is.

**The honest limit.** Automatic resolution works only while the ledger still deduplicates, i.e.
inside its transaction window. Past `retry_deadline_ns` (20 h, inside the ICP ledger's 24 h window)
the canister **refuses** to re-issue, because a re-issue outside the window would move the money a
second time. The entry stays, visible, naming the owner and the amount, and resolving it then needs
an operator to reconcile against the ledger. That is worse than automatic recovery and far better
than the state before this change, in which there was no record at all.

### The gate

**M14 LEDGER/BOOKS COHERENCE**, `tests/money_safety/src/fault.rs`:

> Money that has moved on the ledger is money the canister's own books must either HOLD or NAME.
> There is no third state.
>
> `ledger_main + every deposit subaccount == escrow + chips + pot + uncredited raw transfers +
> open ledger-intent journal`

Both directions are checked and they are different failures: an **orphan** (the canister holds money
nothing accounts for -- FundDestruction, because there is no admin crediting path) and a **short**
(the books promise more than the canister holds -- FundCreation, which is what a discarded *withdraw*
continuation produces). It runs on every fuzz step, and one fuzz run in four injects a real
discarded continuation and then makes the OWNER recover it with a player-only call.

Six tests, all green, each asserting a property rather than an example:

| test | property |
|---|---|
| `m14_deposit_continuation_discarded_is_accounted_for_and_recoverable` | accounted for, named to the right principal, `get_custody_status` names the recovery method, the owner recovers it, a second resolve does not credit twice, and the money reaches her wallet |
| `m14_sweep_continuation_discarded_is_accounted_for_and_recoverable` | the same, and the retry refusal may no longer say *"send ICP to your deposit address first"* while holding it |
| `m14_withdraw_continuation_discarded_does_not_lock_the_player_out` | the debit is written down, the player is whole modulo ledger fees, and she can withdraw again |
| `m14_journal_survives_an_upgrade` | a real `--mode upgrade` keeps the entry, drops the stale lease, and the money is still recoverable on the far side |
| `m14_each_victim_drains_their_own_money_after_a_fault` | **the recipient dimension.** Three faults, three doors, three people; each recovers and drains their own with player-only calls, and no victim is more than a few ledger fees down. A journal that named the wrong owner would keep every total right and fail here |
| `m14_past_the_dedup_window_the_retry_is_refused_and_the_record_kept` | **the one place where doing the helpful thing would move money twice.** Past the ledger's transaction window the retry is refused, nothing is credited, and the entry stays |
| `m14_the_journal_is_bounded_by_refusing_to_start_not_by_forgetting` | the cap refuses new deposits rather than dropping entries, the refusal names the way out, and resolving frees capacity |
| `m14_instrument_self_check` | the reading is non-zero with the journal ignored, so the gate is not vacuously green; also prints the four failed trap attempts and the five places the gate cannot see |

### Where this gate cannot see

Written down rather than left to be rediscovered; `NOT_COVERED` in
`tests/money_safety/src/fault.rs` prints it on every run of the self-check.

* **`IntentOutcome::Unknown`** -- the branch taken when the ledger CALL fails rather than the ledger
  refusing, which leaves the entry open because nobody knows whether the money moved. Unreachable
  from this harness for the same reason a trap is: the real ledger neither rejects nor replies
  undecodably. **Verified by reading only.**
* **A payout the ledger definitively refuses** (`settle_intent`'s refund path for a `Payout`). The
  canister's ledger balance always covers its escrow -- that is M2 holding -- so a well-formed
  payout is never refused here. **Verified by reading only.**
* **Two resolvers in the same round.** Exactly-once is measured sequentially. The concurrent
  argument (the journal removal is atomic, the ledger deduplicates the movement) is sound but is
  not measured.
* **Deposit subaccounts of principals that are not harness actors** -- the same unenumerability
  limit [FINDING 21](#finding-21) documents, inherited.
* **The trap itself**, as above.

### One thing this fix found in somebody else's work

Netting the journal against the deposit-custody observation ([FINDING 28](#finding-28)) is not
cosmetic. A `sweep` moves money **between two accounts this canister already owns**, and the
observation that says "X is at this deposit subaccount" is taken before the sweep and reset in the
continuation. With both terms summed and the continuation discarded, the canister counted the same
e8s twice -- and netting by the sweep amount alone still left the burned transfer fee being claimed
by books after it had ceased to exist anywhere. `observed_deposit_total()` now nets open sweeps
**gross of fee**, and `journalled_incoming_total()` carries them instead, so exactly one term holds
the money at any instant. M14 was red until both halves were right.

> **THE PARAGRAPH ABOVE IS NOT TRUE OF THE CODE, AND THE GAP IS A CRITICAL, OPEN, FUND-LOCKING
> DEFECT. See [FINDING 33](#finding-33).** `journalled_incoming_total()` does *not* carry open
> sweeps: its body is `.filter(|i| i.kind == LedgerIntentKind::Pull)`. The comment above it says
> "`Pull` AND `Sweep`"; the filter says `Pull`. So while a sweep is unfinished the money is
> subtracted from `observed_deposit_total()` and added to **neither** term, `total_liability()`
> reads ZERO on a funded canister, and the FINDING 20 / FINDING 21 currency guard lets a
> re-denomination through -- after which the flip cannot be undone. Reproduced independently
> twice, from opposite directions, and shown to be a **regression introduced by this wave**: the
> same probe is REFUSED against a build with only the pre-await intent write reverted. M14 does
> not see it because M14 reads its own `pull + sweep` total in `fault.rs`, not the canister's.
> **Two measures of the same money, each correct inside its own file, disagreeing from above
> both: the standing signature, for the sixth time.**

<a id="finding-30"></a>
## FINDING 30 (high) -- the permanent hand archive is built from the seats as they stand at settlement, so it omits anyone who left mid-hand and invents anyone who sat down — STATUS: FIXED (wave 8)

**Severity:** HIGH. Not a fund loss. It falsifies the one durable artifact behind the product's
central claim.
**Status:** **FIXED 2026-08-06.** Reproduced first, on the real table canister and the real archive
canister under PocketIC, then fixed at the source and gated three ways. The fix and its proof are
at the bottom of this entry.
Found by the third independent auditor, 2026-08-05, on four archived hands read
by hand from the running instance.
**Where:** `record_hand_to_history` (`src/table_canister/src/lib.rs`), now
`hand_participants` in the same file.

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

**It breaks the shuffle specification's own instruction.** SHUFFLE-SPEC §4 SAID to derive the
player count from the hand's own record -- it no longer does, see the fix below: *"Count it from the hand's own record -- every seat the
history shows with cards, plus any that folded."* Do that for archived hand 10 and you get P = 2
instead of 3, and you reproduce the board `9c Kc Qh / 6c / 4h` instead of the real
`Qh Ts 6c / 4h / 3s`. **For any hand somebody left, the published record is not sufficient to
verify that hand** -- which is the whole of "Provably Fair".

`get_hand_history` on the table canister has the identical omission, so the client shows the same
thing.

**The gate this owes**, and it is a property rather than an example: for every archived hand,
`sum(ending_chips - starting_chips)` reconciles against `total_pot`, and the recorded seat set
equals the dealt-in set the shuffle actually consumed.

---

### REPRODUCED, 2026-08-06, on the real table and the real archive

`tests/money_safety/tests/invariants/archive.rs` installs the table canister under test and the
`history` canister built from this tree, wires them together, and plays the auditor's sequence:
four players dealt in, one leaves after putting 2,000,000 e8s into the pot, and in one arm another
principal buys the empty chair before the hand settles. Then it reads the archive.

The record the archive held, before the fix, verbatim from the run:

```text
--- leaver, chair re-occupied: archived hand_id 1 (table hand 1) ---
  total_pot 8000000  rake 0  showdown true
  dealt in: THE RECORD DOES NOT SAY (no such field)
  players named by the record:
      seat 0 toldy-vthks-...-aae  pos=Seat 0  start=200000000 end=200000000 won=0
      seat 1 lpoz5-mcd63-...-oae  pos=BTN     start=200000000 end=198000000 won=0
      seat 2 weeos-hluch-...-7ae  pos=SB      start=200000000 end=206000000 won=8000000
      seat 3 f6m43-ks6kd-...-rae  pos=BB      start=200000000 end=198000000 won=0
```

`toldy-…` is the intruder. He bought seat 0 after the deal, was never dealt a card, put in nothing,
and the permanent record lists him as a player with the DEPARTED player's starting stack of
200,000,000 e8s -- because `STARTING_CHIPS` is keyed by seat. `74yuz-…`, who was dealt in and paid
2,000,000 into that pot, is not named at all. The record's own player deltas sum to +2,000,000
against a `total_pot` of 8,000,000: **it does not balance.**

**And the board comes out wrong, measured.** The archived seed is
`96edd81596bc467971d030ca90cb36fd6fe8e027107679271a6e23c46ac794ee`. Running the outsider verifiers
-- the Python and the JavaScript one, neither of which shares a line with the canister --

```text
verify_shuffle.py <seed> --players 4   ->  flop 9d 4d 8c   turn 2h   river Js   # what was dealt
verify_shuffle.mjs <seed> --players 3  ->  flop 6h 5h 9d   turn 8c   river 2h   # what SHUFFLE-SPEC
                                                                                # section 4 told a
                                                                                # verifier to compute
```

Three is what you get by counting the players in the old record. A verifier following the
specification would have concluded the table dealt a board that does not follow from its own seed.

Every conservation invariant in the money-safety harness is GREEN on that sequence, in both arms.
That is the fourth instance of the standing signature: correct totals, wrong recipients, every
invariant silent -- here it is the RECORD's recipients rather than the money's.

### THE FIX

**One builder, from the settlement basis.** `hand_participants` replaces the seat walk. It unions
two sources, neither of which is the seat vector:

* `hand_stakes(state)` -- THE PAYOUT BASIS, the same list `plan_payouts` pays out of, which carries
  the OWNER of every stake including stakes whose seat has since been vacated. Everyone who put
  money in the hand is in it, and their money is attributed to them.
* `DEALT_IN` -- a new record written by the deal loop in `start_new_hand`, as it deals, listing
  every seat that took cards and the principal it took them for, in deck order. Everyone who was
  dealt in is in it, including a player who folded pre-flop without putting in a chip.

A principal in neither is not in the hand and does not appear. The list is built ONCE and cloned
into both the archive record and the table's own `get_hand_history` ring, so the two cannot drift
apart the way they had.

**The record now states what a verifier needs.** `HandHistoryRecord.dealt_in` is the ordered list;
`P` for SHUFFLE-SPEC section 4 is its length. Per player the record adds `dealt_in`, `contributed`
and `left_mid_hand`. Every new field is `opt` on both canisters, deliberately: the archive is
append-only and holds records written before the fix, and a bare field would make `stable_restore`
reject every future upgrade of the one canister in this project that must never be reinstalled.
`null` reads as *"the table that wrote this record did not record that fact"*, which is the truth
for the auditor's four hands and is what a verifier should be told instead of a fabricated `false`.

**The specification was wrong too, and is fixed.** SHUFFLE-SPEC section 4 told a verifier to count
`P` out of the player list. It now says to READ `dealt_in`, states that a `null` `dealt_in` means
the board cannot be reproduced, and carries the measured 4-vs-3 board above as the reason.
`check_recorded_hand` on the archive prints the exact verifier command with that hand's `P` already
filled in -- and refuses to print one for a record that does not state `P`, rather than suggesting
a command that would silently produce the wrong board.

**The action log had the same defect and is fixed with it.** Action records resolved their
principal from `state.players[seat]` at settlement, so a departed player's actions were filed under
whoever took the chair (or under `anonymous` if it was empty) -- the auditor's *"none of her actions
recorded"*. They now resolve through `DEALT_IN`.

**Two consequences inside the archive that follow for free, and matter.** `insert_hand` indexes
`hands_by_player` and accumulates `player_stats` from the player list it is given. So before the
fix, a player who left mid-hand **could not find that hand in their own history at all** and their
loss was not in their statistics, while a principal who bought the chair got a hand they never
played added to their `hands_played`. Both follow the participant list, so both are now right.

### AFTER, same sequence, same harness

```text
--- leaver, chair re-occupied: archived hand_id 1 (table hand 1) ---
  total_pot 8000000  rake 0  showdown true
  dealt in (4 players, deal order):
      k=0 seat 0 74yuz-2axoe-...-dae      k=1 seat 1 lpoz5-mcd63-...-oae
      k=2 seat 2 weeos-hluch-...-7ae      k=3 seat 3 f6m43-ks6kd-...-rae
  players named by the record:
      seat 0 74yuz-...  start=200000000 end=198000000 won=0        contributed=2000000
                                                       dealt_in=true  left_mid_hand=true
      seat 1 lpoz5-...  start=200000000 end=198000000 won=0        contributed=2000000
      seat 2 weeos-...  start=200000000 end=206000000 won=8000000  contributed=2000000
      seat 3 f6m43-...  start=200000000 end=198000000 won=0        contributed=2000000
```

The intruder is gone. The departed player is named, with what she paid and the fact that she left.
`sum(contributed) = sum(amount_won) = total_pot = 8,000,000` and `sum(ending - starting) = 0`.
Reproducing the hand from **the archived record alone** -- the seed and `P = 4`, nothing else --
gives `9d 4d 8c / 2h / Js`, which is the board in the record.

### THE GATES

| gate | what it fails on | where |
|---|---|---|
| 7 tests in `cargo test --test invariants -- archive::` | the leaver missing; the intruder present; the record not balancing; `P` absent; the board not reproducing from the archived `P`; the deal record lost across a mid-hand upgrade; the table's copy and the archive's copy disagreeing. Also asserts the OLD `P` gives a *different* board, so it cannot pass vacuously | `tests/money_safety/tests/invariants/archive.rs`, run by `./scripts/dev.sh test` |
| **M12 ARCHIVE FIDELITY**, on every hand the fuzzer settles | leg A: the record must say who played, state who was dealt in, add up, and name nobody who neither took a card nor put in a chip. leg B: what the record says each person contributed must equal what the harness watched them stake, reconstructed step by step from `total_bet_this_hand` and `departed_stakes` and never from the record. leg C: `P` must be stated and everyone it names must be in the player list | `tests/money_safety/src/invariants/record.rs` |
| `Severity::FalseRecord` | never excusable, no id, no magnitude -- it carries zero e8s by construction, which is exactly why every money instrument was green | `tests/money_safety/src/documented.rs` |

Coverage, from `make fuzz-default` on this build: **14 of 14 settled hands checked, 12
cross-checked against the watched stakes, and 3 of them with a mid-hand departure** -- the shape the
defect needs. The count of departures is printed on every run, because a run with none has not
tested this and a gate that quietly declines to measure is indistinguishable from one that passes.

**Reverted, the gates go red.** With `hand_participants` put back on the seat vector and
everything else unchanged, in a `cp -Rc` copy — `cargo test --test invariants -- archive::` gives
`4 passed; 3 failed`:

```text
the_archive_does_not_name_a_player_who_bought_the_chair_mid_hand
    FALSE RECORD: toldy-… bought seat 0 AFTER the deal and was never dealt a card,
    and the permanent record of hand 1 names him as a player.
the_archive_names_the_player_who_left_mid_hand
    the record must say she put in the 2000000 e8s the live table said she had in the pot
    before she left        left: 0   right: 2000000
the_archived_record_balances
    the record's own contributions must add up to the pot it records
                           left: 6000000   right: 8000000
```

and `cargo test --test fuzz` goes from `0 blocking finding(s)` to `3 blocking finding(s)` on the
first default seed alone, shrinking
`M12_ARCHIVE_FIDELITY:record_balances|FalseRecord|HandComplete`.

The four tests that stay green under that mutation are the ones that gate the DEAL record and the
table-versus-archive comparison, neither of which the mutation touches. That is the correct
behaviour and is the reason there are seven tests and not one: each fails on a different thing, so
which ones go red says what broke.

### What is NOT fixed by this

* **The records already in an archive stay wrong.** Nothing can edit them and nothing should:
  the archive is append-only. They read `dealt_in = null`, which is how a verifier can tell.
  No hand has ever been played on mainnet, so the only such records are on local instances.
* **The frontend's generated bindings (`src/declarations/history/history.did.js`) are not
  regenerated**, so the new fields do not reach the UI yet. The interface files are updated;
  the JS binding is a separate owner's file. [DEFECTS.md E-67](DEFECTS.md#e-67).
* **The un-archived backlog is still heap-only.** `UNRECORDED_HANDS` is not in `PersistentState`,
  so an upgrade before `flush_unrecorded_hands` runs destroys the proofs it holds.
  [DEFECTS.md E-68](DEFECTS.md#e-68).
* **`left_mid_hand` is false for the commonest way of leaving a hand.** Found by the wave-8 critic,
  2026-08-06, driving the real table and the real archive on PocketIC. The flag is derived from
  `state.departed_stakes()`, and `record_departed_stake` returns early when `contributed == 0`
  (`src/table_canister/src/lib.rs`), so it writes nothing for a player who was dealt in, folded
  pre-flop **before putting a chip in**, and then left. Measured: UTG folds for free, calls
  `leave_table`, the hand plays on and settles, and the archived record for her reads
  `dealt_in=Some(true) contributed=Some(0) left_mid_hand=Some(false)` — a statement about a person
  that is not true, in the one field added to say whether they left.

  **Not a verifiability defect and not a fund defect:** she is named, `P` is right, the board and
  every hole card still reproduce from the record, and the record balances. What it costs is the
  claim the field makes, plus one thing that is easy to miss: `ArchiveCoverage.hands_with_a_departure`
  in `tests/money_safety/src/invariants/record.rs` counts `left_mid_hand == Some(true)`, so the
  number printed on every fuzz run to prove the gate reached FINDING 30's shape **under-counts by
  exactly the departures this misses**. The two candidate fixes are to record a zero-contribution
  departure as well, or to derive the flag from "dealt in and no longer seated" rather than from the
  stake ledger.

  A neighbouring case that looks the same and is **not** a defect, checked and ruled out: when the
  last seated player leaves, `leave_table` settles the hand *before* vacating the chair, so the
  record correctly says they had not left at settlement time.

<a id="finding-31"></a>
## FINDING 31 (high) -- FINDING 27 IS ONLY HALF CLOSED: a deposit of exactly the advertised minimum, made to the address the canister publishes, is still unwithdrawable — STATUS: OPEN

**Severity:** HIGH. Fund LOCK. No malice at any step. Reachable by following the application's own
printed instructions, at the exact number the application prints.
> **RE-DRIVEN 2026-08-06 (wave-11 register pass). STILL OPEN, verbatim.** Run against the wasm
> `./scripts/dev.sh test` had just built (`sha256 c05fbdc6…`), in a scratch copy, using only
> helpers already in `tests/money_safety/src/world.rs`:
>
> ```
> sent  10001 -> claim Ok(1)     -> escrow      1 -> withdraw REFUSED
> sent  11000 -> claim Ok(1000)  -> escrow   1000 -> withdraw REFUSED
> sent  15000 -> claim Ok(5000)  -> escrow   5000 -> withdraw REFUSED
> sent  19999 -> claim Ok(9999)  -> escrow   9999 -> withdraw REFUSED
> sent  20000 -> claim Ok(10000) -> escrow  10000 -> withdraw REFUSED   <-- THE ADVERTISED MINIMUM
> sent  20001 -> claim Ok(10001) -> escrow  10001 -> withdraw OK (block 3)
> ```
>
> The refusal is verbatim: *"Minimum withdrawal is 0.0002 ICP. Your whole remaining balance can
> always be withdrawn in one call whatever its size, as long as it is more than the 0.0001 ICP
> network fee — you have 0.0001 ICP."* The whole-balance waiver the FINDING 27 fix added is the
> sentence that does not apply, at the one value it needs to.

**Status:** OPEN. Found by the wave-8 critic while verifying the FINDING 27 fix. Reproduced end to
end against the real table canister and the real ICP ledger on PocketIC.
**Where:** `src/table_canister/src/lib.rs` `claim_external_deposit` (`sweep_amount = balance -
transfer_fee`) against the whole-balance waiver in `withdraw`
(`sweeping_whole_balance = amount == balance_now && amount > currency.transfer_fee()`).

**How it is reached.** Not through `DepositModal.svelte` — that modal's "Your Deposit Address" is
the player's OWN NNS account id (`computeAccountId(principal)`), not a canister subaccount, so the
UI does not currently drive this path. It is reached through the canister's public API, which is
this project's own documented second deposit method: `CLAUDE.md`, "Secure Deposit Patterns on ICP
/ 2. Subaccount-based Deposits (External Wallets — `claim_external_deposit()`)". `20_000` is the
number `ICP_MIN_DEPOSIT_AMOUNT` states, the number `DepositModal.svelte` prints as **"Minimum
deposit: 0.0002 ICP"** on its other branch, and the number `deposit_floor.rs` calls
`ADVERTISED_MINIMUM_DEPOSIT`. The third auditor reached the sibling finding
([FINDING 28](#finding-28)) by sending real money to exactly this address.

### This is the STATUS banner's own insight, landing again

The canister owns two kinds of ledger account. FINDING 27 was measured, fixed and gated on **one**
of them -- the ICRC-2 `deposit()` door, where the wallet pays the fee and the full amount lands in
escrow. `tests/money_safety/tests/deposit_floor.rs` drives that door six ways and is correct about
it. **Not one of its six tests sends anything to a deposit subaccount.** On the other door the fee
comes out of the money, and that changes the arithmetic the whole fix rests on.

### Driven, not argued

Real `table_canister` wasm built from this tree, real ICP ledger, PocketIC. One actor, one table.
Send to `(canister, sha256("cleardeck-deposit:" || principal))` -- the address
`get_deposit_subaccount()` publishes -- then `claim_external_deposit()`, then `withdraw(whole
escrow)`:

```text
sent  10001 to the published deposit address -> escrow      1 -> withdraw REFUSED
sent  11000 to the published deposit address -> escrow   1000 -> withdraw REFUSED
sent  15000 to the published deposit address -> escrow   5000 -> withdraw REFUSED
sent  19999 to the published deposit address -> escrow   9999 -> withdraw REFUSED
sent  20000 to the published deposit address -> escrow  10000 -> withdraw REFUSED   <-- THE ADVERTISED MINIMUM
sent  20001 to the published deposit address -> escrow  10001 -> withdraw OK
```

The player's wallet is down 30,000 e8s (20,000 principal + the 10,000 fee their own wallet paid to
reach the address) and the canister permanently holds 10,000 e8s of theirs. The refusal reads:

```text
Minimum withdrawal is 0.0002 ICP. Your whole remaining balance can always be withdrawn in
one call whatever its size, as long as it is more than the 0.0001 ICP network fee -- you
have 0.0001 ICP.
```

That message is true and it is not actionable: the balance is *equal to* the fee, and the waiver
requires *strictly greater*. Topping the ESCROW balance up is not possible without another deposit,
which restarts the same arithmetic.

### Two separate causes, both needed for the trap

1. **`claim_external_deposit()` enforces no deposit floor at all.** `ICP_MIN_DEPOSIT_AMOUNT` is
   checked in `deposit()` and nowhere on this path, so the canister accepts 10,001 e8s at a
   published address and turns it into 1 e8 of escrow. The whole dead band
   `(transfer_fee, transfer_fee * 2]` -- 10,001 through 20,000 -- becomes unwithdrawable escrow.
2. **The advertised minimum sits inside that band, by exactly one e8.** The effective minimum on
   this door is `min_deposit + transfer_fee` if a player expects the round trip to behave like the
   other door, and `transfer_fee * 2 + 1` at the absolute floor. The product states `min_deposit`.

   The coincidence that makes this land exactly on the advertised number is worth writing down:
   `ICP_MIN_DEPOSIT_AMOUNT` is `20_000` and `ICP_TRANSFER_FEE` is `10_000`, so
   `min_deposit - fee == fee` **exactly**, and the waiver's test is a strict `>`. One e8 either way
   and there is no trap. ckBTC does not have it: `BTC_MIN_DEPOSIT_AMOUNT` is `1_000` against a
   `10` sat fee, so `1_000 - 10 = 990` is comfortably above `BTC_MIN_WITHDRAWAL_AMOUNT` of `11`.
   The ICP door is trapped and the BTC door is not, from the same code, which is why no test that
   parameterises over currency would have found it either.

### Why the compile-time invariant did not catch it

The `const _: () = assert!(ICP_MIN_WITHDRAWAL_AMOUNT <= ICP_MIN_DEPOSIT_AMOUNT, ...)` at
`src/table_canister/src/lib.rs:110` is real -- restoring the old `100_000` is a build failure,
verified in a `cp -Rc` copy. But it relates the two floors of the **main-account** door only.
Nothing in it knows that the subaccount door charges the ledger fee out of the deposit, so the
relation it enforces is not the relation that governs the money on this path. The invariant is
anchored to one of the two account kinds, exactly like the instruments the STATUS banner describes.

### What would close it

Any one of these is sufficient and they are not equivalent:

* refuse a claim that would credit less than `min_withdrawal`, and say so with the amount, the fee,
  and the top-up that fixes it (the FINDING 11 pattern already in this file); or
* charge the sweep fee to the canister rather than to the player, so the escrow credit equals what
  arrived; or
* state `min_deposit + transfer_fee` beside the deposit address, and assert that relation at
  compile time the way rule 1 already is.

Whichever is chosen, the gate has to send money to a **deposit subaccount** -- no test in
`deposit_floor.rs` does, which is why six green tests and a compile-time invariant sat on top of a
live reproduction of the finding they were written to close.

### The blind spot, in the gate's own words

`deposit_floor.rs` does not merely omit this door. Its comment on
`a_balance_below_the_floor_can_still_be_swept_whole` **names** it:

> *"Escrow balances are not only made of deposits: a partial withdrawal leaves the remainder, an
> odd-chip split leaves a few e8s, and `claim_external_deposit` credits the swept amount minus a
> ledger fee. Every one of those can leave a balance under the floor..."*

and then simulates it with `fund_escrow(40_000)` + `withdraw(25_000)`, producing a residue of
15,000 -- a value the sweep recovers. The real path at the advertised minimum produces 10,000, the
single value the sweep refuses, and the test immediately below it
(`a_residue_at_the_transfer_fee_is_refused_in_words_that_explain_it`) pins that refusal as correct
behaviour. The suite knows the door exists, models it with a friendly number, and files the hostile
number as expected. `ui_limits.rs` contains no reference to a subaccount at all.

### Reproducer

`cp -Rc src tests Cargo.toml Cargo.lock` into a scratch copy, add a test that calls
`world.transfer_to_deposit_subaccount(alice, 20_000)`, then `world.claim_external_deposit(alice)`,
then `world.withdraw(alice, world.get_balance(alice))`. Every helper it needs already exists in
`tests/money_safety/src/world.rs`.

<a id="finding-32"></a>
## FINDING 32 (high) -- the screenshot harness's NO-RAKE gate cannot go red on any rake this canister is capable of taking: both of its clauses are tautologies on canister-produced data — STATUS: OPEN

**Severity:** HIGH. Not a fund loss on its own. It is the absence of the only gate that guards the
product's headline claim against the PERMANENT ARCHIVE, which is the artifact "provably fair"
rests on.
**Status:** OPEN. Demonstrated by the wave-8 critic by building a canister that takes a 1% house
rake and running the project's own gate function against the record that canister archived.
**Where:** `tools/shots/lib/chain-agreement.mjs`, `assertHandHistoryAgreement` (the two `rake`
clauses) and `foldArchivedHand`; against `src/table_canister/src/lib.rs`
`record_hand_to_history` (`let total_pot: u64 = winners.iter().map(|w| w.amount).sum();` and
`rake: 0,`).

### The two clauses, and why neither can fire

The gate asserts exactly two things about a rake:

```js
if (hist.rake !== null && hist.rake !== 0) { /* RAKE TAKEN */ }
if (hist.totalPot !== hist.awarded + hist.rake) { /* pot != awarded + rake */ }
```

In the canister that produces those records:

* `rake` is the **literal `0`** at `src/table_canister/src/lib.rs` in the `HandHistoryRecord`
  the table sends to the archive. It is not computed from the settlement. No code path anywhere
  can make it non-zero, so clause 1 is dead.
* `total_pot` is **derived from the same winners list the gate sums**:
  `let total_pot: u64 = winners.iter().map(|w| w.amount).sum();`. So `totalPot == awarded` by
  construction and `rake == 0` by construction, and clause 2 is `x !== x + 0`. Dead.

A rake does not make either clause true. It makes the recorded `total_pot` **smaller**, and the
gate compares that smaller number against itself.

### Driven end to end

A 1% house rake was added to `plan_payouts` in a `cp -Rc` copy: each pot share is reduced by
`amount / 100` and the remainder is credited to a house principal's escrow, not reported as a
winner. This is what a real rake looks like, and it passes `PayoutPlan::conserves()` because
`awarded == collected` still holds.

Real wasm, real ICP ledger, real archive canister, PocketIC, four seats, one hand:

```text
pot collected      = 8,000,000 e8s
HOUSE ESCROW       =    80,000 e8s      <-- 1%, withdrawable by the house
archived total_pot = 7,920,000
archived rake      =         0
awarded_total      = 7,920,000
GATE CHECK 1  rake != 0                  -> GREEN
GATE CHECK 2  total_pot != awarded+rake  -> GREEN   (7,920,000 vs 7,920,000 + 0)
M3 check_no_rake (conservation form)     -> 0 violations
internal_total before = 2,400,000,000    after = 2,400,000,000
```

The record was then fed through the project's own exported `foldArchivedHand` and the gate's own
two clauses, verbatim:

```text
foldArchivedHand -> {"handId":1,"totalPot":7920000,"rake":0,"awarded":7920000,"structural":[]}
gate problems    -> []
NO-RAKE GATE: GREEN  <-- the house took 80000 e8s
```

`tools/shots/test-rake.mjs` passes all 22 of its cases on this tree. Its "a rake of 1 e8 is
caught" and "a balanced 5% rake is still caught" cases construct a `HandHistoryRecord` with a
non-zero `rake` field **by hand**. No canister in this repository can emit that record, so those
cases prove the clause is well-formed and prove nothing about whether it guards anything.

### The standing lesson, fifth data point

Correct totals, wrong recipient, every invariant silent. `check_no_rake` is the conservation form
-- escrow + chips + pot before versus after -- and a rake credited to a house account **at the
same table** is inside that total, so M3, the invariant literally named NO RAKE, is green. The
attribution gate declines rather than fires, in these words:

```text
DECLINED: the observed stakes sum to 248000000 but the canister's own record awards 245520000:
the harness did not see the whole hand, so it cannot say who was owed what.
(A rake or a destroyed chip is M3's question, not this one.)
```

M8 hands the rake question to M3, and M3 cannot see this rake. The gate that names the property
defers to the gate that cannot measure it.

### What DOES catch it, and this is real

`cd tests/money_safety && cargo test --test invariants` against the raked wasm: **53 passed, 10
failed**, including `archive::the_archived_record_balances` ("the record's own contributions must
add up to the pot it records: left 8000000, right 7920000"), `m6_a_pot_is_awarded_exactly_once`,
four `m8_*` attribution tests and both `seam_*` tests. `check_hand_payout_total` in
`src/fuzz.rs` compares hand-history `awarded` against `wagered_last_hand` and would fire too.
So the project is not defenceless against a rake. But every one of those lives in the Rust suite
behind `./scripts/dev.sh test` (>10 minutes), and none of them is the gate that reads the
permanent archive a stranger would check.

### What would close it

`rake: 0` and `total_pot = sum(winners)` are the defect, not the JS. Two changes, both in the
canister:

* record `total_pot` as **`plan.collected`** -- what the hand took off the players -- rather than
  as the sum of what it paid out; and
* record `rake` as `plan.collected - plan.awarded_to_players`, computed, so the field means
  something.

Then clause 2 becomes a real cross-check between two independently derived numbers and the gate
can fire. Until then the JS clauses should be treated as unasserted, and
`tools/shots/test-rake.mjs` should carry a case that reads a record the CANISTER produced -- the
probe used here needs `install_archive` and about forty lines.

<a id="finding-33"></a>
## FINDING 33 (critical) -- FINDING 21 IS REOPENED BY FINDING 29'S NETTING: while one sweep is unfinished, `total_liability()` reads ZERO on a canister holding 5 ICP, and the currency guard lets the flip through — STATUS: FIXED (wave 8)

> ### THE FIX, AND WHY IT TOOK THREE REPRODUCTIONS
>
> `journalled_incoming_total()`'s filter is now `.filter(|i| i.kind.credits_on_success())`
> instead of `.filter(|i| i.kind == LedgerIntentKind::Pull)`, one word, making the code do
> what the comment sitting directly above it already claimed. `credits_on_success()` is the
> SAME predicate `unfinished_ledger_ops_for()` uses for the player-facing surface, so the guard
> and the surface now read one definition of "arriving money" and cannot disagree again.
>
> Measured on the fixed tree, real mainnet ICP ledger wasm on PocketIC, 2 ICP into bob's
> published deposit subaccount, `fault::trap_claim_external_tail` to discard the sweep
> continuation:
>
> ```
> after the discarded continuation: ledger_main 199990000  subaccounts 0  books 0  journalled 199990000
> journal: [{id 1, who bob, kind "sweep", amount 199990000}]
> get_custody_status(bob).total  -> 199990000
> admin_update_config(BTC)       -> REFUSED "Refusing to change this table's currency from ICP
>                                   to BTC while it still owes players 1.9999 ICP. ..."
> ```
>
> Before the fix, on the identical state, the same call replied
> `ACCEPTED currency now BTC`, and the flip back was then **REFUSED** ("still owes players
> 9990 sats"), so 1.9999 ICP of bob's ended permanently locked in a currency the canister holds
> none of. That one-way trap is measured in the two reproductions below.
>
> **GATED:** `tests/money_safety/tests/coherence_w8.rs::finding_33_an_open_sweep_keeps_the_currency_guard_shut_and_the_flip_is_reversible`,
> named explicitly in `./scripts/dev.sh test`. Reverting the filter to `== LedgerIntentKind::Pull`
> turns it red. **This is the first gate anywhere in the tree that reads what the currency
> guard reads.** See [FINDING 37](#finding-37) for why that is its own finding.
>
> **What the fix does NOT close:** the same guard is still blind at the canister's MAIN account,
> for the same structural reason and at a different address. That is
> [FINDING 35](#finding-35), which is OPEN and was driven on this tree after the fix landed.

Found by the wave-8 critic while falsifying the FINDING 28 / FINDING 21 fix. **The fix is real
and its own gates are real. This is a new hole opened underneath it by the interaction of two
changes made in the same wave.**

### The arithmetic

`total_liability()` is `escrow + table + observed_deposit_total() + journalled_incoming_total()`.

* `observed_deposit_total()` deliberately **subtracts** open sweeps:
  `observed.saturating_sub(open_sweep_total())`, where `open_sweep_total()` is
  `amount + fee` per open `Sweep` intent. Its comment says the sweep is "counted here instead
  of there, exactly once".
* `journalled_incoming_total()`'s own comment says it counts "`Pull` AND `Sweep`, and no double
  count ... so the sweep is counted here instead of there".
* **Its code is `.filter(|i| i.kind == LedgerIntentKind::Pull)`.**

So while a sweep intent is open the money is subtracted from one term and added to neither.
The comment describes the intended design; the filter does not implement it.

### Driven, against the real mainnet ICP ledger wasm

`tests/money_safety/tests/critic_w8_probe.rs`, using the project's own
`fault::trap_claim_external_tail` (the FINDING 29 injector: the ledger transfer stands, the
post-await continuation is discarded, the intent stays open -- and `IntentOutcome::Unknown`
from any ledger call rejection reaches the same state without a trap).

```
AFTER TRAP: ledger_main=499990000 deposit_subaccounts=0 escrow_total=0 chips=0 pot=0
            canister_deposit_by_principal={victim: 500000000}
get_custody_status(victim): escrow=0 unswept_deposit=0 total=499990000
admin_update_config(BTC) -> Ok(TableConfig { ..., currency: BTC })
```

The canister is holding **4.9999 ICP that is the victim's**, and `admin_update_config` accepted
the flip to BTC -- the exact re-denomination FINDING 20 and FINDING 21 exist to refuse. After
the flip, `claim_external_deposit` and `withdraw` look for that ICP on the ckBTC ledger, where
this canister holds nothing.

`admin_get_deposit_custody` reports `total = 0` at the same instant. `get_custody_status`
reports `total = 499990000`, because it reads `unfinished_ledger_ops_for()`, which uses
`kind.credits_on_success()` -- `Pull | Sweep`. **The player-facing surface is right and the
guard is wrong**, which is why nothing that looks at a surface can find this.

### Attribution: one word

Changing `journalled_incoming_total()`'s filter to `.filter(|i| i.kind.credits_on_success())`
-- making the code do what its comment already claims -- makes the guard hold, with no other
edit:

```
admin_update_config(BTC) -> Err("Refusing to change this table's currency from ICP to BTC
  while it still owes players 4.9999 ICP. ...")
```

### What the harness already knows, and why it did not fire

The money-safety invariants are **not** blind here -- `check_conservation`,
`check_no_orphaned_custody` and `check_deposit_attribution` all fire loudly on this state
(`money_belongs_to_nobody`, delta 499990000). **No test puts the canister in it.** The state is
reachable from the fuzzer's fault injection, but the recovery path closes the intent before the
point-in-time check runs. This is not the standing "every invariant silent" shape; it is the
narrower and more ordinary one: a correct instrument that nothing aims at the state.

### Severity

Critical rather than high: it is an unbounded, controller-reachable custody change on money the
canister is holding, it needs no attacker beyond one unfinished ledger call, and the guard it
defeats is the last one standing between a funded table and a re-denomination.

### INDEPENDENTLY REPRODUCED, and it is WORSE THAN "the flip is allowed": the flip is ONE-WAY

Confirmed 2026-08-06 by a second reviewer, arriving from the FINDING 29 side rather than the
FINDING 21 side, with a different amount and a different actor. The value this adds is a fact
the entry above does not state: **the flip cannot be undone, so the money is not merely
mis-guarded, it is permanently locked.**

Measured on the fixed tree, real mainnet ICP ledger wasm on PocketIC, 2.0 ICP into bob's
deposit subaccount, `fault::trap_claim_external_tail` to discard the sweep continuation:

```
custody:  ledger_main 199990000  subaccounts 0  books 0  journalled(pull+sweep) 199990000
journal:  [{id 1, who bob, kind "sweep", amount 199990000}]
admin_update_config(BTC)              -> ACCEPTED -> currency now BTC
  AFTER THE FLIP, bob resolves        -> Ok(["ledger operation 1: ... "])  entry STILL OPEN,
                                         attempts 2, lease released -- the retry addressed the
                                         ckBTC ledger, which is not there
  AFTER THE FLIP, claim_external_deposit() -> Err("Could not ask the mxzaz-hqaaa-aaaar-qaada-cai
                                         ledger what is at your deposit address: DestinationInvalid")
  AFTER THE FLIP, withdraw(1 ICP)     -> Err("Maximum withdrawal per transaction is 0.1000 BTC")
  ICP still held by the canister      -> main 199990000 + subacc 0 = 199990000
  flip BACK to ICP                    -> REFUSED: "Refusing to change this table's currency from
                                         BTC to ICP while it still owes players 9990 sats."
```

The last line is the trap door. Once the table is BTC, the same netting that read ZERO on the
way in reads a small NON-ZERO on the way out -- `observed(200000000) - open_sweep_gross(sweep
amount + the **BTC** fee)` -- so the guard that let the flip through now refuses to let it back.
The canister ends holding 1.9999 ICP of a player's money, denominated in a currency it holds
none of, with every door closed and no controller method to reopen them.

Two more measurements that narrow the defect to `Sweep` exactly, so the one-word fix above can
be trusted not to be papering over a wider hole:

* **an open `Pull` correctly blocks the flip** -- `journalled_incoming_total()` counts it:
  `REFUSED: "... while it still owes players 3.0000 ICP"`.
* **an open `Payout` is blocked, but only by accident** -- the refusal that fires is the
  *unaudited deposit address* rule, not the liability rule. A payout is in no liability term
  either. It happens not to matter today because the payout's escrow debit is already committed
  and the money has already left, but nothing states that, and the guard is not the reason it
  is safe.

The `Sweep` case is the one that defeats **both** guards at once, which is why it is the one
that gets through: `claim_external_deposit()` READ the subaccount before sweeping, so the
account is audited and the unaudited-accounts rule is satisfied; and the netting then makes its
audited balance read zero, so the liability rule is satisfied too.

### And this hole was OPENED by the wave-8 change, not merely missed by it

The same probe was run against a build with one surgical revert -- the ledger-intent write moved
back into the post-await continuation, everything else in the tree identical:

```
pre-journal-ordering build:  admin_update_config(BTC) -> REFUSED ("still owes players 2.0000 ICP")
fixed build:                 admin_update_config(BTC) -> ACCEPTED -> currency now BTC
```

Before this wave, the stale deposit observation of 200000000 was still counted by
`observed_deposit_total()` and the guard held for the wrong reason. The netting removed that
term and `journalled_incoming_total()`'s `Pull`-only filter failed to add it back. This is a
regression with a clean before/after, not a pre-existing gap.

<a id="finding-34"></a>
## FINDING 34 (high) -- `get_deposit_address()` publishes the MAIN account under the name "deposit address", and money sent there is invisible on every surface of the FIXED build while `claim_external_deposit()` replies "your deposit address is empty" — STATUS: FIXED (wave 11)

> ## ✅ CLOSED IN WAVE 11. IT WAS STILL LIVE WHEN THE WAVE OPENED, ON THE TREE WHOSE HEADER ALREADY SAID FIXED.
>
> **Reproduced first, on the current tree, before anything was changed.** The header above said
> `STATUS: FIXED (wave 10)`; the canister said otherwise:
>
> ```
> get_deposit_address() as alice -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
> get_deposit_address() as bob   -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
> the canister's MAIN account    -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
>
> alice sends 3 ICP to the address she was given, with the ledger's LEGACY `transfer`:
>   icrc1_balance_of(table, deposit_subaccount(alice)) = 0
>   claim_external_deposit() -> Err("Your deposit address is empty: this canister asked the
>     ICP ledger just now and it holds 0. Send ICP to ... and call this again.")
> ```
>
> **What it is now.** ONE account, two spellings, and the spelling depends on who is asking:
>
> | | |
> |---|---|
> | `get_deposit_subaccount()` | `sha256("cleardeck-deposit:" \|\| principal)` -- the ICRC-1 spelling of `(this canister, those bytes)` |
> | `get_deposit_address()` | `account_identifier(this canister, those same bytes)` -- the legacy 64-hex spelling of the **same account** |
>
> Measured after the fix, same sequence: alice `f03c8d73…9f92`, bob `909621…96a3`, main account
> `731114…7d2f` -- three different accounts. Alice's 3 ICP lands at
> `icrc1_balance_of(table, deposit_subaccount(alice)) = 300000000`, **0 in the main account**,
> `claim_external_deposit()` credits her `299990000`, and she withdraws all of it.
>
> **The load-bearing fact was proved and not assumed.** The whole fix rests on the ICP ledger
> resolving the 64-hex account identifier and the ICRC-1 `Account { owner; subaccount }` to ONE
> balance. That is a fact about the ledger, so it is executed against the real mainnet ledger
> module in `the_hex_address_and_the_icrc1_account_are_one_account_on_the_real_ledger`. If it
> were false, publishing the hex form of a subaccount would send every player's money to an
> account `claim_external_deposit()` cannot reach -- a worse defect than the one being fixed.
>
> **THE HARNESS HAD NEVER EXECUTED THE DOOR A HEX ADDRESS IMPLIES.** Every existing test funds a
> deposit subaccount with `icrc1_transfer` to `Account { owner, subaccount }`. A 64-hex address
> is only spendable through the ICP ledger's LEGACY `transfer`, which nothing in this project
> had ever called. That is why an address defect survived eleven waves of instruments that all
> agreed the deposit path was sound: **they were testing the addressing scheme nobody is
> given.** `World::legacy_transfer_to_address` is the missing door and every address test now
> goes through it.
>
> **The two cases where there is no address now say so** instead of returning the shared account:
> an anonymous caller (`claim_external_deposit()` refuses anonymous callers, so money at the
> anonymous principal's deposit account could never be swept by anybody) and a ckBTC table (the
> ckBTC ledger has no account-identifier form at all). The return type is `text`, so the contract
> is stated in the `.did`: **a reply is payable iff it is 64 lowercase hex characters**, and every
> refusal begins `"NO ADDRESS: "`.
>
> **What happens to money already at the shared account.** It is accounted for and it is
> recoverable, with one limit that is now stated everywhere rather than discovered:
>
> | | |
> |---|---|
> | accounted for | `get_solvency().main_account` + `main_uncredited_observed()` (FINDING 35), and `refresh_main_account_custody()` reads it on demand |
> | recoverable | `notify_deposit(block_index)` -- **executed**: alice's 1 ICP at the shared account credited in full, and bob refused for the same block |
> | **NOT recoverable, and now said out loud** | a transfer whose `from` is not the caller's own principal account -- an exchange withdrawal, a custodial wallet -- cannot be attributed to anybody. The `from` check is what stops a stranger claiming your block; the price is that a withdrawal straight from an exchange to the shared account is unrecoverable by you, by a controller and by anybody else. `notify_deposit`'s refusal now says exactly that instead of "only the sender can claim their deposit", which reads like a retry might work |
>
> **The commonest wrong call stopped lying.** A player who sends to their own deposit address and
> then reaches for `notify_deposit` used to be told *"Transfer was not to this canister"* -- false,
> and it sends somebody hunting for a transfer sitting safely at their own address. It now names
> the door that works, and `notify_deposit_does_not_tell_a_player_their_own_deposit_address_is_not_this_canister`
> executes both halves.
>
> **Gate:** `tests/money_safety/tests/deposit_surface.rs` (10 tests), wired into
> `./scripts/dev.sh test` and named in `tests/money_safety/Cargo.toml`. Reverting
> `get_deposit_address()` to `compute_account_identifier(&canister_id(), None)` in a `cp -Rc`
> copy turns **7 of the 10 red**, including the executed one:
> *"the 64-hex address and (table, deposit_subaccount(alice)) must be ONE account: left 0,
> right 300000000"*.


Found by the wave-8 critic. FINDING 28 was closed at the per-player subaccount. **The canister
publishes two different addresses and only one of them was re-anchored.**

* `get_deposit_subaccount()` -> `sha256("cleardeck-deposit:" || principal)`, per player. This is
  the one FINDING 28 fixed, and the one `DepositModal.svelte` uses.
* `get_deposit_address()` -> `compute_account_identifier(canister_id(), None)`: the **main
  account**, the same 64-hex string for every player. Its `.did` comment is "Get the canister's
  account for deposits". Nothing in the frontend source calls it; it is a public query on a
  live canister with "deposit address" in its name.

### Driven on the FIXED build (`tests/money_safety/tests/critic_w8_player.rs`)

```
get_deposit_address()     -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
get_deposit_subaccount()  -> 5fce1ee947ccaa60b0ae9937c67d22ad68efbdbf112e3cc7a94d70e106b3927a
LEDGER: main=500000000 deposit_subaccounts=0
  get_balance()            -> 0
  get_custody_status()     -> escrow=0 unswept_deposit=0 total=0 advice=""
  admin_get_deposit_custody total -> 0
  claim_external_deposit() -> Err("Your deposit address is empty: this canister asked the ICP
    ledger just now and it holds 0. Send ICP to (canister 7tjcv-..., subaccount
    get_deposit_subaccount()) and call this again.")
```

**This is FINDING 28's sentence, at the other published address, on the build that closed
FINDING 28.** The refusal is now emphatic and specific ("this canister asked the ICP ledger
just now and it holds 0") and it is false: the canister holds 5 ICP that arrived at the address
its own `get_deposit_address()` method handed out, and it directs the player to send *more*
money to a different address.

The money is not lost -- `notify_deposit(block_index)` credits it -- but no surface says so and
the one error message the player will see points away from the recovery. A player who did not
keep the block index has no door at all.

### The narrower half, also true

Before anyone calls `refresh_deposit_custody()`, a genuine subaccount deposit reads
`get_custody_status() -> total=0, advice=""`. The machine-readable honesty signal
(`unswept_deposit_observed_at_ns = null`) is correct and the human-readable `advice` field --
the one the product puts on screen -- is the **empty string**. The claim "no surface tells a
player they have nothing while the canister holds their money" does not hold until `advice`
says "this canister has never looked at your deposit address; call `refresh_deposit_custody()`".

### Related, not yet driven

`observed_deposit_total()` sums `DepositObservation::amount` across **every** observation
regardless of `DepositObservation::ledger`, although that field exists (by its own comment) so
that "a table that has ever been ICP and is now BTC can hold value in both at once" can be told
apart. `ledger` is read only for display and persistence, never in a sum. Reachable once
FINDING 33 lets a funded table change currency; the result is ICP e8s and ckBTC sats added into
one number and formatted as the current currency.


---

<a id="finding-35"></a>
## FINDING 35 (high). THE FIFTH CROSS-AGENT DEFECT: the canister has an observation record for every deposit SUBACCOUNT and none for its MAIN account, so money at `get_deposit_address()` is invisible to every surface INCLUDING the operator's audit tool, and the currency guard will let a controller close its only recovery door — STATUS: FIXED (wave 10)

> ## CLOSED, and there is a REAL 2.00 ICP instance of it on mainnet right now
>
> Measured on `kieex-haaaa-aaaaj-qor3q-cai` (table_1) on 2026-08-06, immediately after the six
> backend canisters were upgraded to the reproducible build:
>
> ```text
>     escrow claimed   940,640,001 e8s
>     chips at table             0
>     pot                        0
>     ledger main account holds  740,640,001 e8s
>     ------------------------------------------
>     SHORTFALL        200,000,000 e8s  (2.00 ICP)
> ```
>
> Every published deposit subaccount was audited: `(5 audited, 0 held, 0 unaudited)`. The money
> is not hiding there. The escrow list shows one principal holding 740,620,001 and a second
> holding exactly 200,000,000 — the signature of the old deposit double-credit
> ([FINDING 10](#finding-10)), which is closed in the deployed code and left this residue.
>
> **THE DISCREPANCY HAS NOT BEEN ERASED AND MUST NOT BE.** No method was added that lets a
> controller change a player's balance; `admin_restore_balance` was deleted once already, and
> `tests/money_safety/tests/solvency.rs::no_setter_was_added_to_fix_the_books` is now a
> source-level gate that fails if one comes back under any of five plausible names. Editing a
> balance would not return the money to anybody. It would only stop the canister saying it was
> missing.
>
> **What is closed is that the canister could not SEE it or SAY it.** It can now, on five
> surfaces, and it says "I do not know" rather than "solvent" when it has not looked.
>
> | | before | now |
> |---|---|---|
> | main-account observation record | none | `MAIN_CUSTODY` (`MainAccountObservation`), persisted `opt` at the top level of `PersistentState` |
> | who can take a reading | nobody | **anybody**, incl. anonymous: `refresh_solvency()`, `refresh_main_account_custody()` |
> | player-readable answer | none | `get_solvency()` query: OWES, HOLDS, signed difference, verdict, age of every reading |
> | `get_custody_status` | ten correct fields, none of which could say the money is not there | `canister_solvency` + `canister_shortfall_e8s`, and the shortfall sentence FIRST in `advice` |
> | withdrawal that fails for want of canister funds | `InsufficientFunds { balance: Nat(740640001) }` | a named refusal that says the canister is short, by how much, that the escrow is refunded, and which public method proves it |
> | `admin_audit_deposit_custody` | iterated subaccounts only; answered `(1 audited, 0 held, 0 unaudited)` on a canister holding 5 ICP | reads the MAIN account first and logs `CANNOT PAY EVERYONE — …` |
> | `total_liability()` | four terms, none for the main account | fifth term `main_uncredited_observed()`, plus an UNKNOWN-IS-NOT-ZERO leg covering the main account |
> | harness | no leg could see it: the harness can always read the ledger itself | `invariants::solvency`, four legs, new never-excusable `Severity::InsolvencyUnreported`, per fuzz step |
>
> Driven end to end on the real mainnet ICP ledger wasm on PocketIC, reproducing the mainnet
> shape locally (2.00 ICP leaves the canister's main account with no message to it):
>
> ```text
> HEALTHY -> CanPayEveryone owed=940640001 held=Some(940640001)
> 2.00 ICP left the canister's main account at block 6
> LEDGER main=740630001 escrow_total=940640001 (short by 200010000)
> VIOLATION owes_more_than_it_holds_across_every_account [FundCreation] ... SHORT 200010000 e8s
> SOLVENCY -> CannotPayEveryone  owed=940640001 held=Some(740630001) diff=Some(-200010000)
> alice withdrew 500000000; bob is owed 440640001 and the canister now holds 240630001
> WITHDRAW REFUSAL -> Err("THIS TABLE COULD NOT PAY YOU BECAUSE THIS CANISTER IS SHORT, not
>   because there is anything wrong with your request. It asked the ICP ledger to send you
>   4.4064 ICP out of its main account and the ledger answered that the account holds only
>   2.4063 ICP (240630001 e8s). Your escrow has been put back in full ... this canister owes
>   4.4064 ICP (440640001 e8s) across every player and is short by 2.0001 ICP (200010000 e8s).
>   It has just written that reading down, so get_solvency() will show it to anybody who asks ...
>   This is not something a controller can fix by editing a balance: there is deliberately no
>   method here that can.")
> ```
>
> Note the sequence in the last three lines. **alice asked first and was paid in full out of
> money that was never all there; bob, who did nothing wrong, is the one the ledger turned
> away.** That is what a shortfall does, and it is why the canister has to say so before anybody
> reaches the front of the queue.
>
> ### The one property everything else rests on, and it is asserted, not argued
>
> A deposit-subaccount reading is stale-LOW by construction. A main-account reading is not: money
> arrives there with no message, and this canister spends out of it. So the one-directional
> property had to be BUILT, out of an asymmetry:
>
> * an adjustment that **lowers** the written-down balance (a confirmed payout) is applied
>   ALWAYS, even at the risk of double-subtracting a movement the reading already reflected;
> * an adjustment that **raises** it (a confirmed pull, sweep or notified deposit) is applied
>   ONLY when the movement provably happened after the reading — its ledger timestamp, or the
>   intent's `created_at_time`, is strictly greater than `observed_at_ns`.
>
> `check_written_down_holdings_are_not_overstated` asserts it on every snapshot, and
> `the_written_down_main_balance_is_never_higher_than_the_ledger` drives it through a pull, a
> sweep, a payout and an unannounced arrival:
>
> ```text
> after the first reading                ledger_main=   500000000 claimed=Some(500000000)
> after a pull, no new reading           ledger_main=   800000000 claimed=Some(800000000)
> after a sweep, no new reading          ledger_main=   999990000 claimed=Some(999990000)
> after a payout, no new reading         ledger_main=   699990000 claimed=Some(699990000)
> after an unannounced arrival           ledger_main=   799990000 claimed=Some(699990000)   <- LOW
> ```
>
> ### AND THE FIRST VERSION OF THAT CHECK WAS WRONG, in the way this repository is always wrong
>
> It asserted `main_account <= ledger_main` **unconditionally**, on the strength of the argument
> above, written in a comment. The argument has an exception and the exception is reachable and
> this project has a whole harness for it. Adjustments are applied at `settle_intent`, i.e. when
> the ledger's answer comes back — so a payout the ledger has ALREADY EXECUTED and whose
> continuation was discarded ([FINDING 29](#finding-29)) has left the chain and not yet left the
> record. Measured:
>
> ```text
> BEFORE: main=Some(800000000) held=Some(800000000) diff=Some(0) payouts_in_flight=0
> DURING: main=Some(800000000) (LEDGER says 600000000) held=Some(800000000) diff=Some(0)
>                                                            payouts_in_flight=200000000
> AFTER:  main=Some(600000000) (LEDGER says 600000000) held=Some(600000000) diff=Some(0)
> ```
>
> **What makes it harmless is not the size of the gap. It is that the same amount is on the OWED
> side for exactly as long.** A payout debits the main account by exactly `intent.amount`
> (`amount - fee` to the player, `fee` burnt out of the same account) and exactly `intent.amount`
> is `payouts_in_flight` until the instant the record is adjusted, so
> `(main + A) - (owed + A) == main - owed`: the published DIFFERENCE, which is what the verdict
> is computed from, is **identical to the e8** before, during and after. The same cancellation
> holds for a sweep, between the deposit term and the journal.
>
> The correct statement, which is what is asserted now:
>
> * a per-account figure may be stale-high by **at most what the journal already names**;
> * `held` may not be stale-high **at all** (`canister_claims_to_hold_more_than_the_ledger_holds`);
> * so a false alarm is possible and a false all-clear is not.
>
> Driven by `a_payout_in_flight_moves_the_parts_and_not_the_answer`, which asserts the check is
> silent, that the record really is running ahead of the chain at that moment (otherwise the
> bound is untested), and that the answer does not move.
>
> ### A PRIVACY REGRESSION the first version of this surface shipped with
>
> `get_solvency()` is a public query, and its first version returned
> `deposit_accounts_never_observed : vec principal`. `deposit_account_census()` is every principal
> holding escrow at this table, every principal seated, and every principal already observed —
> so an **anonymous caller could enumerate the players.** That list had been controller-only
> (`admin_get_deposit_custody`) and the new surface handed it to anybody.
>
> An unknown has to be VISIBLE without being ENUMERABLE. The field is now:
>
> * `deposit_accounts_never_observed_count : nat64` — always the true total, public. This is
>   what the verdict branches on, so the answer does not depend on who is asking.
> * `deposit_accounts_never_observed : vec principal` — **all of them for a controller, and your
>   own principal (only if it is unread) for anybody else**, which is the rule
>   `get_all_ledger_intents` already uses.
>
> The summary prose is scoped the same way. Asserted in
> `a_never_observed_main_account_reads_as_unknown_and_never_as_solvent`: the count is non-zero,
> the anonymous list is empty, the anonymous prose does not contain the player's principal, and
> the same query as that player names their own address.
>
> **What IS newly public, deliberately:** the aggregate `escrow`, `chips_at_table`, `pot` and
> `owed`. A solvency claim that does not state what is owed cannot be checked by the person it
> is made to, and this canister's whole problem was making unfalsifiable claims. No per-principal
> amount is exposed.
>
> ### A hole found while closing it, which nothing above would have caught
>
> A reading is taken on ONE ledger. `Currency::ledger_canister()` selects between the ICP and
> ckBTC ledgers, and a table that has changed currency owns a main account on each. Counting an
> ICP reading as evidence about the ckBTC account would let the canister answer "I can pay
> everyone" in satoshis it has never looked for — a false all-clear, the one direction the whole
> design forbids. `observed_main_entry()` therefore FILTERS by the live ledger, and
> `observed_deposit_gross()` does the same on the held side. `DepositObservation::ledger`
> already recorded this and nothing enforced it.
>
> ### What is NOT closed by this
>
> * **The 2.00 ICP on mainnet is still missing.** This makes it visible and quotable; it does not
>   return it. That is an operator action, and the honest options are funding the canister or
>   telling the affected principal.
> * The reading is only as fresh as the last `refresh_solvency()`. Every surface states its age
>   and none of them presents it as current.
> * A currency change now requires a reading of the main account **on the ledger in force**, in
>   both directions. On mainnet that is one public call; on a replica with no ckBTC ledger
>   installed it cannot be taken, and `the_currency_guard_refuses_while_the_main_account_has_never_been_read`
>   asserts that outcome rather than leaving it as a surprise.

Found by the wave-8 coherence pass, deliberately, by asking the one question that is above all
five agents rather than inside any of them: **which of the six accounts in "THE ACCOUNT CENSUS"
does the CANISTER have an instrument for?**

### The one sentence

The wave's organising insight was: *money arrives at an account with no message to the canister,
and an IC query cannot call the ledger, so the canister must ASK from an update and WRITE THE
ANSWER DOWN.* That insight was implemented for accounts **(2)** and **(4)**, the per-player
deposit subaccounts, and for nothing else. Accounts **(1)** and **(3)**, the MAIN accounts where
essentially all of the money actually sits, got **no observation record, no reader, and no term in
any guard**. `refuse_currency_change_while_funded` is a synchronous function, so it can never read
a ledger; it can only read what somebody wrote down; and nobody writes down the main account.

The census's own rule is *"an instrument that measures fewer than all of these accounts is not
measuring this canister's custody."* Its own **MEASURED BY** column for account (1) names
`total_liability()`, but `total_liability()` measures the CLAIMS on the main account, never its
BALANCE. The only thing in this repository that reads the main account is the money-safety
harness, which does not ship.

### Driven, on the FIXED tree, after FINDING 33 was closed

`tests/money_safety/tests/coherence_w8.rs::finding_35_reproduction_money_at_get_deposit_address`.
Real mainnet ICP ledger wasm on PocketIC. alice is an ordinary player: she deposits, is audited,
sends one transfer to the address the canister publishes, then cashes out and leaves.

```
get_deposit_address() -> 73111429194d272dbdb27aa4439e172caeb5618a22c76a6786c81da205247d2f
withdraw(all 300000000) -> Ok(4)
admin_audit_deposit_custody([alice]) -> Ok((1, 0, 0))   (read, observed_total, still_unaudited)
LEDGER main=500000000 subaccounts=0
  get_balance()             -> 0
  get_custody_status total  -> 0   advice=""
  admin_get_deposit_custody -> 0
  admin_update_config(BTC)  -> ACCEPTED currency now BTC
  notify_deposit(3) while BTC -> Err("Failed to query ckBTC ledger: ... mxzaz-hqaaa-aaaar-qaada-cai")
  admin_update_config(ICP) back -> ACCEPTED currency now ICP
  notify_deposit(3) after flip back -> Ok(500000000)
```

Read the third line again. **`admin_audit_deposit_custody` reports "1 account audited, 0 held,
0 unaudited" on a canister holding 5 ICP of alice's**, because it iterates deposit subaccounts and
the money is not in one. The fourth auditor measured the same reply, `(4 audited, 0 held,
0 unaudited)` on a table 1 ICP over-funded, and it reproduces unchanged on the build that closed
FINDING 28. The operator's all-clear is structurally incapable of seeing this money.

### Two halves, and the second is the new one

**(a) INVISIBILITY.** This half is [FINDING 34](#finding-34) and is confirmed here: every surface
reports zero, and the one error message a player sees (`claim_external_deposit`) is emphatic,
specific and false, and points them at a different address.

**(b) THE GUARD IS BLIND THERE TOO, and that is new.** `total_liability()` is
`escrow + table + observed_deposit_total() + journalled_incoming_total()`. Uncredited money at the
MAIN account is in **none** of the four terms, and the "UNKNOWN IS NOT ZERO" leg only enumerates
deposit **subaccounts**, so an audited, drained table with 5 ICP sitting in its main account reads
a liability of exactly zero and `admin_update_config(BTC)` is accepted. After the flip
`notify_deposit`, the door FINDING 34 correctly identifies as the *only* remaining recovery --
routes to the ckBTC ledger and fails, and `claim_external_deposit` follows it.

This is FINDING 21 and FINDING 33 at a third account, reached with **no attacker, no trap, no
fault injection and no unfinished call**: one exchange withdrawal and one routine config change.

### The honest mitigation, measured

**The flip is REVERSIBLE here**, unlike FINDING 33's: `admin_update_config(ICP)` back is ACCEPTED
and `notify_deposit(3)` then returns `Ok(500000000)`. So this is not a permanent loss; it is a
door that a controller can close and reopen. The reason it is still HIGH rather than medium:

* no player-facing surface, at any point, says the money exists;
* no operator surface, at any point, says the money exists, `admin_audit_deposit_custody` says
  the opposite, in the affirmative;
* so **nothing in the system would ever tell the operator to flip it back**. The recovery exists
  and is unreachable by anyone acting on the canister's own reports;
* and it needs the player to have kept the ledger block index, which the product never asks them
  to do.

### The fix, stated so it cannot be half-done again

One `DepositObservation` for the main account, written by the same three callers that write the
subaccount ones (`claim_external_deposit`, a `refresh_*`, `admin_audit_deposit_custody`), plus a
fifth term in `total_liability()`: `observed_main_uncredited = observed_main.saturating_sub(escrow
+ table + open payouts)`. The "UNKNOWN IS NOT ZERO" leg then has to cover the main account too --
a canister that has never read its own main account must refuse a currency change, exactly as it
refuses one with an unread subaccount today. And `get_deposit_address()` should either be deleted
from the interface or return the caller's own derived subaccount, which is the fourth auditor's
own recommendation and is one line.

---

<a id="finding-36"></a>
## FINDING 36 (instrument, high), the money-safety harness's `CustodyStatus` mirror is missing the field that carries FINDING 29's money, under a comment that says it is "Mirrored in FULL on purpose" — STATUS: FIXED (wave 10)

> **CLOSED.** `tests/money_safety/src/table_api.rs::CustodyStatus` now declares
> `unfinished_ledger_ops` (and the two fields FINDING 35 added), and the fix is NOT "add the
> missing field" -- a mirror can silently lose the next one exactly the same way. The fix is
> `CustodyStatus::components_sum_to_total()`:
>
> ```text
> escrow + chips_at_table + committed_in_pot + unswept_deposit + unfinished_ledger_ops == total
> ```
>
> That identity **cannot hold unless every component field is declared**, because Candid drops
> what the mirror does not name while `total` keeps counting it. It is asserted in
> `tests/money_safety/tests/solvency.rs` on both a healthy and an insolvent table, so a
> silently narrowed record fails a test instead of quietly reading nine fields of ten.
> `invariants/custody.rs`'s narrow mirror has its `total` doc corrected: it says which two
> fields are in the total and not above it, and why that mirror is deliberately narrow.

Found by the wave-8 coherence pass, by writing `cs.unfinished_ledger_ops` in a probe and having
the compiler refuse it.

```
error[E0609]: no field `unfinished_ledger_ops` on type `money_safety::table_api::CustodyStatus`
  = note: available fields are: `escrow`, `chips_at_table`, `committed_in_pot`,
          `committed_is_stuck`, `abandonable_in_ns` ... and 4 others
```

The canister's `CustodyStatus` has ten fields. `tests/money_safety/src/table_api.rs`'s mirror has
nine: `unfinished_ledger_ops`, the field the FINDING 29 agent added to carry money that has
moved on the ledger and not been booked, is absent. **Candid record subtyping drops unknown
fields silently**, so nothing failed, nothing warned, and every harness assertion written against
the custody surface this wave has been reading a truncated record.

The mirror's own doc comment, written by a different agent in the same wave:

> **Mirrored in FULL on purpose.** A partial mirror here would let the field be deleted from the
> canister without a single test noticing, which is precisely how the deposit subaccounts stayed
> outside every instrument for seven waves.

The exact mechanism that comment exists to prevent is what happened to it, in the wave it was
written, to the field carrying the money [FINDING 33](#finding-33) was about. This is why
FINDING 33's blindness could not be seen from the harness side: `total` was right, the component
was invisible, every gate green.

The second mirror, `tests/money_safety/src/invariants/custody.rs::CustodyStatus`, is explicitly
and correctly documented as narrow, but its `total` field is now described as *"Everything above,
added up"*, and `total` includes two fields that are no longer above it.

**Fix:** add `pub unfinished_ledger_ops: u64` to the `table_api.rs` mirror; correct the
`invariants/custody.rs` doc for `total`; and add one assertion somewhere that
`escrow + chips_at_table + committed_in_pot + unswept_deposit + unfinished_ledger_ops == total`,
which is the only thing that makes a full mirror load-bearing rather than decorative.

---

<a id="finding-37"></a>
## FINDING 37 (instrument, high), `total_liability()` is the only number the project's last custody guard reads, and it has one caller, no query, no surface and no gate: the only way to sample it is to attempt the destructive operation it guards — STATUS: FIXED (wave 10)

> **CLOSED, by the one query this finding asked for.** `get_solvency()` publishes
> `guard_liability` -- `total_liability()` itself -- alongside every term it is built from, so
> the guard can be SAMPLED without attempting the destructive operation it protects.
>
> The number alone would not be enough; a published figure nobody checks is the same instrument
> in a different font. `invariants::solvency::check_solvency_report_is_coherent` runs on every
> snapshot the harness takes, including every fuzz step, and asserts term by term against
> figures it computes independently:
>
> * `escrow` against `admin_get_all_balances`, `chips_at_table` against `admin_get_table_chips`,
>   `pot` against the table state, `unswept_deposits` against `admin_get_deposit_custody`;
> * `owed` against the sum of its own published terms;
> * `held` against `main_account + deposit_subaccounts + pulls_in_flight - sweep_fees_in_flight`;
> * `difference_e8s` against `held - owed`;
> * and `guard_liability` against `owed - payouts_in_flight + unattributed_at_main`, which is
>   the relation that makes the report and the guard the same instrument.
>
> A term that goes missing from the guard now fails a test instead of waiting for somebody to
> flip a currency and find out.

The structural reason FINDING 21, FINDING 33 and FINDING 35 are three instances of one defect.

```
$ grep -n "total_liability()" src/table_canister/src/lib.rs
4919: fn total_liability() -> u64 {
4954:     let owed = total_liability();          <-- the ONLY caller
```

No `#[ic_cdk::query]` returns it. Nothing in `tests/money_safety` reads it. Every gate in the
project computes its own total from the ledger and the books and compares those two, which is a
good instrument for conservation and says **nothing whatever** about the arithmetic the guard
performs. So:

* the harness can be entirely correct and green while the guard reads zero on a funded canister
  (FINDING 33, measured);
* a term can be missing from the guard for an entire account and no test can notice
  (FINDING 35, measured);
* and the only way anybody has ever discovered either is to call `admin_update_config` with a
  different currency and see what happens.

**Partly addressed:** `tests/money_safety/tests/coherence_w8.rs` is now the first test in the
project that aims at the guard, and it is in `./scripts/dev.sh test`. That is one state, not an
instrument.

**The real fix is one query.** `get_custody_accounting()` returning the four (soon five) terms
separately and their sum, so the harness can assert term-by-term against its own independently
computed figures on every fuzz step. A guard whose number no instrument can read is a guard
nobody can gate, and this project has now shipped three defects inside exactly that gap.

---

<a id="finding-38"></a>
## FINDING 38 (high) -- `get_solvency()` counts an open `pull` on BOTH sides on the strength of the reading not yet containing the money, and wave 10 shipped a public button that makes the reading contain it: one anonymous `refresh_solvency()` turns a real, correctly-reported shortfall into a published SURPLUS — STATUS: OPEN

> **What was executed.** `tests/money_safety` on PocketIC with the real mainnet ICP ledger wasm,
> against the wave-10 build (`table_canister.wasm` sha256
> `1c2da9f96b50efa669a2130bf649852bc9a9dbcc45512f4e2b22d2b8be7c9fad`). Probe source and full log
> are in the critic scratch (`critic_w10.rs::critic_a_public_refresh_during_an_in_flight_pull_invents_a_surplus`).
>
> ```text
> STEP 1  truly short          verdict=CannotPayEveryone diff=-100010000 owed=800000000 held=699990000
> STEP 2  FINDING 29 injection alice's 2 ICP deposit LANDS at the main account, continuation
>                              discarded, pull intent stays OPEN, alice NOT credited
>                              ledger_main 699990000 -> 899990000
> STEP 3  stale reading        verdict=CannotPayEveryone diff=-100010000     <-- still correct
> STEP 4  ONE PUBLIC refresh_solvency(), called by an unrelated player
>                              verdict=CanPayEveryone   diff=+99990000
>                              owed=1000000000 held=1099990000 pulls_in_flight=200000000
>                              LEDGER holds 899990000 against a true liability of 1000000000
>                              => the canister is STILL SHORT 100010000 e8s
> ```
>
> Not one of the seven new solvency legs fired. `check_insolvency_is_reported` returned **0
> violations**, because `harness_owed()` excludes open intents by design while the ledger balance
> it is compared against already contains the money. `check_written_down_holdings_are_not_overstated`
> is silent by construction: its bound is `ledger_holdings + payouts_in_flight + pulls_in_flight`
> and `held` lands on it exactly.

### The arithmetic

`held = main_account + deposit_subaccounts + pulls_in_flight - sweep_fees_in_flight`, and the
justification in `open_pull_total()` is stated in the code:

> *"If the pull happened, the canister holds it (in the main account, **not yet in the reading**)
> and owes it; if it did not, it neither holds nor owes it."*

That parenthesis is a claim about the AGE of the reading. `refresh_solvency()` is public,
unpermissioned, callable by an anonymous principal, and its entire purpose is to make the reading
current -- at which point `main_account` already contains the pulled money and
`pulls_in_flight` adds it a second time. The overstatement is exactly the open pull amount.

### Why the window is durable, not a race

An intent whose continuation was discarded (FINDING 29 -- an upgrade mid-call is one of the
things that discards one, and the six backend canisters were upgraded on 2026-08-06) stays open
until somebody calls `resolve_my_ledger_intents()`. Past `INTENT_RETRY_WINDOW_NS` (20h)
`lease_ledger_intent` refuses to re-issue it at all and the entry is **kept forever**, pending
operator reconciliation. For that entire time every fresh reading produces the inflated answer.

### The mirror, also demonstrated

`critic_a_public_refresh_during_an_in_flight_payout_invents_a_shortfall`: a canister holding
600000000 e8s against a true liability of 600000000 e8s (difference 0, `CanPayEveryone` on the
stale reading) answers **`CannotPayEveryone ... it is SHORT 200000000 e8s`** after one public
`refresh_solvency()` while a payout sits in the same window. `check_point_in_time` -> 0
violations. `shortfall_sentence_from()` puts that sentence FIRST in every player's
`get_custody_status().advice`, so a free, unpermissioned call broadcasts a false insolvency
notice to the whole table.

### The suggested shape of the fix

Count an in-flight `pull` on the OWED side only, and never add it to `held`. In the
"money has not arrived" case the canister then cries poor by the pull amount, which is the safe
direction and one `resolve_my_ledger_intents()` from being cleared; in the "money has arrived"
case it is exactly right. The same asymmetry that governs `MainAccountObservation::credited_since`
(apply a raise only when it provably post-dates the reading, apply a lowering always) is the rule
this term does not follow.

### And a gate that would have caught it

Every existing solvency leg is one-directional: they convict a canister that claims MORE than the
ledger holds and are silent on one that claims less, and none of them compares the published
`difference_e8s` against a difference the harness computes from the ledger plus the journal. A
leg asserting `difference_e8s <= ledger_holdings - (escrow + chips + pot + unswept + pulls that
have already landed)` goes red on step 4 above.

---

<a id="finding-39"></a>
## FINDING 39 (high) — the canister ends a hostile sequence owing 4,000,000 e8s it does not hold, and it has been doing so since wave 4 — STATUS: OPEN

**Severity:** HIGH. Not a theft and not a lock: an **unbacked liability**. Every balance surface
reads correctly, every player can withdraw, and the money runs out before the last one does. M1
CONSERVATION and M2 LEDGER REALITY are the two invariants this project describes as never
excusable, and both are red.

**Status:** OPEN. Filed as [DEFECTS.md E-41](DEFECTS.md#e-41) by the wave-4 attribution pass,
which wrote *"It should be recorded in `docs/SECURITY-FINDINGS.md` as soon as the mechanism is
known, because a canister that owes more than it holds is one withdrawal away from a player's
funds being unbacked."* The mechanism is still not known and the write-up was never made. This is
it, seven waves later, because the wave-11 register pass could not mark the entry `FIXED` without
running it.

**Where:** unknown. First observed at `check_timeouts`; not root-caused, not minimal.

### Re-driven on today's build, not carried forward

The module is the one `./scripts/dev.sh test` had just built and reported green on, by hash:

```
MONEY-SAFETY: wasm under test sha256=c05fbdc6e7a46b80d4ace7591819429e09ad32650c5763ed5e904a8906360a45
              bytes=2833632 toolchain=1.90.0

cd tests/money_safety
MONEY_FUZZ_SEEDS=212967420072193,212967420072194 MONEY_FUZZ_STEPS=400 MONEY_FUZZ_SHRINK=0 \
  cargo test --test fuzz -- --nocapture
```

```
seed 0xc1b1576c1101 finished: 6 hands, 9 upgrades, 0 blocking finding(s)
seed 0xc1b1576c1102 finished: 5 hands, 5 upgrades, 7 blocking finding(s), worst stranded 3175980000 e8s

M2_LEDGER_REALITY:canister_is_short  — canister is SHORT: it holds
  ledger_main=6200030000 + deposit_subaccounts=0 = 6200030000
  but it owes escrow=5805030000 + chips=399000000 + pot=0 + unswept_deposits=0 = 6204030000

M1_CONSERVATION:ledger_equals_owed   — delta=-4000000
  (chips CREATED: the canister owes money it does not hold)

M2_LEDGER_REALITY:owes_more_than_it_holds_across_every_account —
  "It is SHORT 4000000 e8s: somebody's withdrawal is going to fail at the ledger,
   and it will be whoever asks last."

test result: FAILED. 0 passed; 1 failed.  93.41 s
```

### Three things this measurement changes about the register entry

1. **It is not fixed.** Seven waves of fund work — E-32, E-36, E-05/E-37, the ledger-intent
   journal, the solvency surface — have gone past it and it still reproduces.
2. **It is twice the size the entry records.** E-41 says 2,000,000 e8s, "exactly the big blind of
   the 6-max ICP table the seed runs". It is **4,000,000** on this build. The entry's arithmetic
   coincidence is either wrong now or was always a coincidence.
3. **A new leg fires that did not exist when E-41 was filed**, and it is about who can see it:

   > `refresh_solvency_is_not_available_to_an_ordinary_caller` — *the LEDGER says this canister is
   > SHORT 4000000 e8s and an ordinary player cannot even take the reading that would reveal it:
   > `refresh_solvency()` answered `Err("You have asked this canister to re-read its own main
   > account 30 times in the last minute, which is the limit…")`. A solvency check a player has to
   > ask an operator for is not a solvency check.*

   The wave-10 solvency work ([FINDING 35](#finding-35)) built the surface that makes this
   visible, and the rate limit on it is reachable in ordinary hostile play. That is a real,
   separate observation about the newest instrument in the project and it is recorded here rather
   than filed as its own finding because it was produced by this same run.

### Why nobody saw it, which is the part worth reading

**No target runs this configuration.** `./scripts/dev.sh test` fuzzes **1 seed × 40 steps**;
`make fuzz-default` runs the fuzzer's own defaults, **3 seeds × 220 steps**; the reproducer is
**seeds …193,…194 × 400 steps**. `make fuzz` — the long target — runs nine seeds including
**…194** at 600 steps, and nothing in this repository's recorded history shows it being run since
wave 4. So the project's own long fuzz target is very likely red and has been for seven waves,
in the same shape as [H-45](DEFECTS.md#h-45): the gate exists, it is not part of anything, and
nobody had to answer for it.

**And the two-seed form is [H-26](DEFECTS.md#h-26).** Seed …194 finds nothing on its own; it finds
this only when …193 has run before it in the same process. A run is documented as *"a pure
function of `(seed, config, actor_names, steps)`"* and is not, which also means the shrinker's
reduction cannot be trusted and no minimal reproducer exists.

### CRITIC RE-DRIVE, 2026-08-06: `make fuzz` IS red, alone, and it is 100x this size

The paragraph above says the long target is *"very likely red"*. It was measured. Same wasm
(`sha256 c05fbdc6…`), one seed, no second seed in the process, at exactly the step count
`./scripts/dev.sh fuzz` uses:

```
cd tests/money_safety
MONEY_FUZZ_SEEDS=212967420072194 MONEY_FUZZ_STEPS=600 MONEY_FUZZ_SHRINK=0 cargo test --test fuzz

  steps=150  green      steps=320  green      steps=400  green
  steps=250  green      steps=360  green      steps=600  FAILED (52.35 s)
```

```
M9_FUND_REACHABILITY:money_left_behind_after_drain|FundsUnreachable|HandComplete|+
  (seed 0xc1b1576c1102, 600 ops)
  after every legal player-side exit was driven to exhaustion, the canister still owes
  5234010001 of the 10340070002 e8s it started with, and no player call can move it.

  round 0: withdraw 5233990001 refused: … it asked the ICP ledger to send you 52.3399 ICP
           out of its main account and the ledger answered that the account holds only
           48.3399 ICP (4833990001 e8s) …
```

Three things follow, and none of them are in the entry above.

1. **The shortfall is 400,000,000 e8s (4 ICP), not 4,000,000.** One hundred times the number
   this finding is titled with, from the same seed, at the step count the project's own long
   target ships.
2. **It presents as M9 FUND REACHABILITY, not only as M1/M2.** 5,234,010,001 e8s, **more than
   half the 103.4 ICP on the table**, survives a full drain: every cash-out, every withdrawal,
   every sweep, ten rounds of them. That is [FINDING 15](#finding-15)'s property, red, on a
   build whose fast gate is green.
3. **`make fuzz` is a red target in the Makefile today**, not a target that would go red under
   a two-seed invocation nobody types. Anybody who runs the command the help text advertises
   gets this.

The 400-step two-seed form and this are the same underlying defect seen at two magnitudes; the
seed-order dependence ([H-26](DEFECTS.md#h-26)) is confirmed here in the other direction, since
seed …194 alone is green at 400 steps and red at 600.

### What would close it

Root-cause it. The entry's own suggestion is still the right next step: seat a player, let a
`check_timeouts` run a hand out while a seat is `Disconnected` or being vacated, and watch
`ledger_main` against `escrow + chips + pot` across the settlement. Then pin it as a named
regression in `tests/money_safety/tests/regressions.rs`, which **is** in the fast gate, so that
closing it does not depend on anybody remembering to run a nine-seed sweep.

Until then: **this canister can owe more than it holds, on mainnet, today, with no attacker
privilege**, and the only reason the register did not say so is that the entry was filed with a
reproducer nobody ran.


---

<a id="finding-40"></a>
## FINDING 40 (high) -- the deposit address is served over an UNCERTIFIED query, so one dishonest replica substituting one reply is a completed theft that every money invariant in this project calls correct — STATUS: FIXED (wave 11)

> ## ✅ CLOSED IN WAVE 11, BY REMOVING THE CANISTER FROM THE TRUST PATH RATHER THAN HARDENING IT.
>
> **Demonstrated first.** The third auditor's sequence, executed on the current tree in
> `tests/money_safety/tests/deposit_surface.rs::a_substituted_address_is_a_completed_theft_and_local_derivation_is_what_prevents_it`:
>
> ```
> alice asks for her address; a dishonest reply gives her -> 909621844171852630f0c7c21b4b2a2372ee6ebbf01b3e577572d31acbec96a3
>                                                            (that is BOB's deposit address)
> alice pays it: 5 ICP
> bob claim_external_deposit() -> 499990000        alice's 5 ICP, in bob's escrow
> alice get_balance()          -> 0
> ```
>
> **Every money instrument in this project is silent, and correctly so.** Not one e8 went
> missing: M1 conserves, M2 holds, M3 takes no rake, M8's attribution agrees that the money in
> bob's escrow arrived at bob's address. This is the project's signature failure shape --
> *correct totals, wrong recipient, every invariant silent* -- for the seventh time, and here it
> is not even a canister bug. **The canister behaves perfectly at every step.** The defect is
> that the answer to "what is my address" travelled over an ordinary query.
>
> ### Why a query is not evidence
>
> On the IC an update call goes through consensus and its reply is certified. **A query does
> not.** One replica answers it alone, out of its own memory, and the reply carries no signature
> the client checks. So a single dishonest replica -- one node in a subnet, no majority, no
> canister compromise -- can answer `get_deposit_address()` with another player's address. The
> resulting transfer is a perfectly ordinary deposit by the wrong person, and it is
> indistinguishable from an honest one by anybody, including the canister, because by the time
> the money moves the substitution has already happened outside it.
>
> Ownership of a deposit subaccount is nothing but `sha256("cleardeck-deposit:" || principal)`.
> That is not a weakness -- it is what makes the money attributable at all -- but it does mean
> the ONLY thing standing between a player and the wrong account is that they were told the right
> 64 characters.
>
> ### The two candidate fixes, and why the second one is stronger
>
> | | what it does | what is left |
> |---|---|---|
> | **certified query** | `set_certified_data` over the derivation, reply carries a certificate, client verifies it against the IC root key | the canister is still in the trust path. The client must implement certificate verification correctly, must actually reject an unverifiable reply rather than falling back, and the certified value has to be maintained by the canister across upgrades. It hardens a call that does not need to exist |
> | **client derives locally** ✅ | the address is a pure function of `(table canister id, your principal)`, both of which the client already holds -- the canister id from the build's own configuration, the principal from the signed-in identity. The client computes it and never asks | **nothing to substitute.** There is no reply, so there is no reply to tamper with. The failure mode moves from "a replica lied" to "our own arithmetic is wrong", which is a thing a test can check exhaustively and a lying replica cannot influence |
>
> **Chosen: local derivation.** It is strictly stronger, because it deletes the trust
> relationship instead of auditing it, and it is smaller: one pure module, no certificate
> plumbing, no upgrade-time state.
>
> ### What shipped
>
> * **`src/cleardeck_frontend/src/lib/depositAddress.js`** -- the derivation, with no network
>   call at all, on purpose. `deriveDepositAddress(tableCanisterId, principal)`.
> * **`DepositModal.svelte` derives, then CHECKS.** It calls `get_deposit_address()` for one
>   reason only: to detect a substituted reply or a drift between this build and the canister.
>   The comparison NEVER prefers the canister's answer. On disagreement the modal shows **no
>   address at all** and says why -- because at that point the client cannot tell which of the two
>   was tampered with, and showing either is guessing with the player's money.
> * **340 lines of hand-rolled SHA-224 and CRC-32 came OUT of the modal in the same change.**
>   They computed the player's **own wallet** account under the heading *"Your Deposit Address --
>   Send ICP to this address to fund your poker account"*. Money sent there does not fund the
>   poker account; it funds the wallet, and a second step nobody was told about was still
>   required. The modal now shows the one address that is the player's, at the table, with a
>   Claim Deposit button on it.
>
> ### The gate, and the thing it protects against that a doc cannot
>
> A client that derives locally is safer than one that asks **only if its arithmetic is right**.
> A wrong local derivation sends money to an account `claim_external_deposit()` cannot reach,
> which is worse than trusting a query. So the gate does not compare two copies of a constant:
> **it runs the real `depositAddress.js` in node and compares its output with the real canister
> on the replica, principal by principal**
> (`the_frontends_own_derivation_agrees_with_the_canister_for_every_principal`).
>
> Driven in a `cp -Rc` copy: deleting ONE character from the derivation's domain separator
> (`"cleardeck-deposit:"` -> `"cleardeck-deposit"`) turns that test red **and nothing else**:
>
> ```
> test the_frontends_own_derivation_agrees_with_the_canister_for_every_principal ... FAILED
> assertion `left == right` failed: depositAddress.js and the canister disagree about
> 74yuz-2axoe-…-dae's deposit address.
> test result: FAILED. 8 passed; 1 failed
> ```
>
> ### What this does NOT claim
>
> It does not claim the substitution is now impossible for every client. Any third-party
> integration that calls `get_deposit_address()` and trusts the reply is still exposed, which is
> why the `.did` now says so at the method, in words, with the derivation written out next to it.
> It does not make the theft detectable after the fact either: a misdirected transfer is a
> well-formed deposit by the wrong person and no canister-side change can tell the difference.
> **The only defence is not needing to ask, and that is what shipped.**

---

<a id="finding-41"></a>

## FINDING 41 (fund-theft) -- FINDING 40 IS HALF CLOSED: the OISY deposit path still asks the canister where to send the money, over the same uncertified query, and pays the answer automatically — STATUS: FIXED (wave 11 reconciliation), FOUND BY THE WAVE-11 CRITIC

> ### FIXED IN THE WAVE-11 RECONCILIATION, and the gate had to be rebuilt to see it
>
> The OISY branch now derives its destination with no network call:
>
> ```js
> const sessionPrincipal = await (await auth.getAgent()).getPrincipal();
> const depositSubaccountBytes = depositSubaccount(sessionPrincipal);
> ```
>
> The SESSION principal, not the OISY wallet principal, because the session principal is the one
> that calls `claim_external_deposit()` afterwards and is credited. The canister is still asked,
> for one purpose only -- catching a drift between the two derivations -- and a disagreement
> **aborts the transfer** rather than warning about it, because the very next statement moves
> real money. That is a stricter rule than the display path's, which shows a warning, and it is
> stricter for a reason: there is no player in the loop to read a warning between the check and
> the payment.
>
> **Two gates, because the existing one was structurally blind.**
> `the_frontends_own_derivation_agrees_with_the_canister_for_every_principal` compares the
> 64-HEX form -- the form the panel renders and the OISY path never touches -- so it was green
> with this defect present. Added:
> `the_frontends_32_byte_derivation_agrees_with_the_canister_for_every_principal` (the spelling
> an ICRC-1 wallet is actually paid in) and
> `the_oisy_transfer_destination_is_derived_locally_and_not_fetched`, which reads the component,
> finds the name the transfer is addressed to, and requires that name to be assigned from the
> local `depositSubaccount(...)`. It goes red the moment a payment destination comes back over
> the wire again.
>
> **[FINDING 42](#finding-42) is still OPEN and is the other half.** The canister id the
> derivation is rooted in still arrives over `lobby.get_tables()`. This fix removes the table
> canister from the trust path of both the displayed and the paid address; it does not remove
> the lobby.

> **Where it was:** `src/cleardeck_frontend/src/lib/components/DepositModal.svelte:555` and the
> transfer at `:565-590`. Present at `HEAD` (`:577`) and unchanged by the wave-11 deposit work,
> which edited the same file.
>
> [FINDING 40](#finding-40) says, in its own words: *"**nothing to substitute.** There is no
> reply, so there is no reply to tamper with."* That is true of the address the modal
> **displays**. It is false of the address the modal **pays**.
>
> ```js
> // DepositModal.svelte, the OISY branch of handleDeposit()
> statusMessage = 'Getting your deposit address...';
> const depositSubaccount = await tableActor.get_deposit_subaccount();   // <-- THE QUERY
> ...
> const destination = {
>   to: { owner: canisterPrincipal, subaccount: [depositSubaccount] },   // <-- PAID DIRECTLY
>   amount: amountSmallest,
> };
> await wallet.transfer({ ...(isBTC ? { params: destination } : { request: destination }), ... });
> const claimResult = await tableActor.claim_external_deposit();
> ```
>
> `get_deposit_subaccount()` is a `query` (`table_canister.did`), answered by one replica,
> carrying no certificate this client checks — the same property FINDING 40 is about, in the
> 32-byte spelling instead of the 64-hex one. The canister's own `.did` already says so at the
> method: *"AND A QUERY IS NOT EVIDENCE: like `get_deposit_address()`, one replica answers it and
> signs nothing you check. Derive it yourself."* This call site does not.
>
> ### Why this is worse than the path that was fixed
>
> On the display path the player sees 64 characters and could, in principle, compare them.
> Here **no address is ever shown**. The client asks, receives 32 bytes, and instructs the
> player's wallet to pay them. A dishonest replica returning
> `sha256("cleardeck-deposit:" || bob)` sends alice's ICP or ckBTC to bob's deposit account, and
> the next line — `claim_external_deposit()` — is called **as alice**, so it fails with "your
> deposit address is empty" while bob sweeps at leisure. The canister-side half of that sequence
> is already executed and green in
> `tests/money_safety/tests/deposit_surface.rs::a_substituted_address_is_a_completed_theft_and_local_derivation_is_what_prevents_it`;
> the only new element here is that the client volunteers the substitution.
>
> Every money invariant stays silent, for the seventh time, because no e8 goes missing.
>
> ### The fix is one line, and it is already written
>
> `src/cleardeck_frontend/src/lib/depositAddress.js` exports `depositSubaccount(principal)`,
> which is exactly this value derived with no network call. The OISY branch should call it, and
> should treat the canister's reply the way `checkAgainstCanister()` treats the hex one: as
> evidence of drift, never as the source of the destination.
>
> ### What would close it
>
> A gate in the shape of the one FINDING 40 shipped: run the module in node, derive the
> subaccount, and compare it byte for byte with what the OISY branch would transfer to. The
> existing derivation gate compares the 64-hex form only, so it is green today with this defect
> present — which is the standing lesson again: **the gate tests the door that was fixed.**

---

<a id="finding-42"></a>

## FINDING 42 (fund-theft) -- the local derivation is rooted in a canister id that arrives over ANOTHER uncertified query, so FINDING 40's substitution is still one reply away — STATUS: OPEN, FOUND BY THE WAVE-11 CRITIC

> **Where:** `src/cleardeck_frontend/src/routes/+page.svelte:307`, `:364-367`, `:1154`;
> `src/lobby_canister/lobby_canister.did:61`.
>
> [FINDING 40](#finding-40)'s decision table justifies local derivation with: *"the address is a
> pure function of `(table canister id, your principal)`, both of which the client already holds
> — **the canister id from the build's own configuration**, the principal from the signed-in
> identity."* `depositAddress.js` repeats the sentence in its header comment.
>
> **The client does not hold it from the build's configuration.** It holds it from the lobby:
>
> ```js
> // routes/+page.svelte
> let lobbyTables = await lobby.get_tables();          // :307   lobby .did: `query`
> ...
> currentTableInfo = tableInfo;                        // :364
> tableActor = createTableActorProxy(canisterId);      // :367   from the same reply
> ...
> <DepositModal tableCanisterId={currentTableInfo?.canister_id?.[0]} ... />   // :1154
> ```
>
> `get_tables : () -> (vec TableInfo) query;` is an ordinary query with exactly the property
> FINDING 40 is about. Substitute one `canister_id` in that reply and:
>
> * the derived address becomes `account_identifier(ATTACKER_CANISTER, sha256("cleardeck-deposit:" || alice))`,
>   an account only the attacker's canister can sweep;
> * the modal's cross-check **passes**, because `tableActor` was built from the same substituted
>   id, so the substituted canister is asked and answers consistently;
> * `accountIdentifierHex(tableCanisterId, null)` — the "is this the old shared account?" branch —
>   is also computed from the substituted id, so none of the three `checkAgainstCanister()`
>   outcomes fires.
>
> The derivation removed the table canister from the trust path and left the lobby canister in
> it. That is a real reduction in exposure and it is not the property the register records.
>
> ### This is cheap to close on mainnet, and it is already half-built
>
> `src/cleardeck_frontend/src/lib/ic-config.js` carries `MAINNET_CANISTER_IDS` and
> `isMainnetCanisterId()`, with a comment saying they exist so a build can check what it is
> wired to. No deposit path consults them. On a mainnet build, a table canister id that is not
> in that frozen list should not be handed a deposit address at all — the same "show nothing"
> judgement `checkAgainstCanister()` already makes for a disagreeing reply.
>
> ### What would close it
>
> The `MAINNET_ID_LIST` check above, plus a test that feeds `deriveAndVerifyDepositAddress` a
> canister id that is not in the list and asserts no address is shown. Until then FINDING 40's
> "nothing to substitute" should read "nothing to substitute **on the table canister**".

---

<a id="finding-43"></a>
## FINDING 43 (high) -- THE SEVENTH CROSS-AGENT DEFECT: the solvency instrument calls the money at the shared main account a SURPLUS, in the same reply that names it as money held for somebody the canister cannot name — STATUS: OPEN, FOUND IN THE WAVE-11 RECONCILIATION

**Status:** OPEN. Reproduced by an executable, red-on-purpose marker on the current tree.

> ### CORRECT TOTALS, WRONG RECIPIENT, EVERY INVARIANT SILENT
>
> Six consecutive waves have produced a cross-agent defect with that signature. This is the
> seventh, and it is the purest instance so far: **no e8 moves, nothing is destroyed, every
> number in the reply is arithmetically correct, and the reply attributes a player's money to
> the house.**

### What was executed

```
cd tests/money_safety
cargo test --test deposit_surface -- --ignored --nocapture \
  the_solvency_verdict_counts_money_at_the_main_account_as_surplus
```

alice sends 1 ICP to the canister's shared main account through the ICP ledger's legacy
`transfer` -- the door FINDING 34's published address sent every player to for eleven waves --
and then anybody calls the public, money-moving-nothing `refresh_solvency()`:

```
MONEY-SAFETY: wasm under test sha256=1fc6ef7d43729df1e4ab6c506c9c16d4bcdeeb9b74675a391c2e0e4cf2843c6f
verdict              = CanPayEveryone
owed                 = 0
guard_liability      = 100000000
held                 = Some(100000000)
difference_e8s       = Some(100000000)
unattributed_at_main = Some(100000000)
summary              = This canister can pay everyone it owes, on the readings it has: it owes
                       0.0000 ICP (0 e8s) and holds 1.0000 ICP (100000000 e8s), a surplus of
                       100000000 e8s. Every account it can enumerate has been read. ...
```

The canister is holding one ICP of alice's money. It says it owes **nothing**, and calls her ICP
a **surplus** -- while, three fields higher in the same record, reporting
`unattributed_at_main = 1 ICP`, whose own doc comment reads *"money held for somebody this
canister cannot yet name"*.

### The mechanism: two totals of one liability, in one reply

`src/table_canister/src/lib.rs`:

```rust
fn total_liability() -> u64 {          // THE GUARD reads this. Five terms.
    escrow_total()
        .saturating_add(table_claims())
        .saturating_add(observed_deposit_total())
        .saturating_add(journalled_incoming_total())
        .saturating_add(main_uncredited_observed())     // <-- FINDING 35's term
}

let owed = escrow                       // THE INSTRUMENT computes this. Six terms.
    .saturating_add(chips_at_table)
    .saturating_add(pot.max(committed_stake))
    .saturating_add(unswept_deposits)
    .saturating_add(unfinished_incoming)
    .saturating_add(payouts_in_flight);                 // <-- and NOT main_uncredited_observed()

let held = main.balance_now() + deposit_subaccounts + pulls_in_flight - sweep_fees_in_flight;
let difference_e8s = held - owed;
let verdict = ... difference_e8s ...
```

`held` counts the main-account balance on the ASSET side. `owed` does not count it on the
LIABILITY side. So every e8 sitting at the main account is added once and subtracted never, and
the difference reports it as surplus. The report even publishes both totals side by side --
`owed` and `guard_liability` -- disagreeing, with the verdict computed from the smaller one.

`guard_liability` was added in wave 10 by [FINDING 37](#finding-37) *"so an instrument can check
the guard instead of having to trigger it"*. Nothing checks it against `owed`.

### Why this is a cross-agent defect and not an oversight

Wave 10 added the fifth term to `total_liability()` because [FINDING 35](#finding-35) showed a
table sitting on 5 ICP reporting that it owed nobody anything. The public `SolvencyReport` was
written in the same wave and was never given that term. Both agents were right about their own
surface; the two surfaces disagree, and the disagreement is invisible because **each one is
internally consistent**. `invariants::solvency` checks the report against itself
(`components_sum_to_total`), so it cannot see it either.

### Why it matters more than an ordinary instrument bug

This is the surface the deposit screen renders. `DepositModal.svelte` calls
`readTableSolvency()` on mount and shows the answer *before* a player commits real money, which
was the whole point of building it. A player looking at a table holding money it cannot
attribute is told the table has a surplus.

The fifth auditor hit the same reading from the other direction: immediately after a controller
`reinstall` wiped a player's 5 ICP claim, the instrument reported
`verdict = CanPayEveryone` and *"a surplus of 500000000 e8s"*, because wiping `escrow_total()`
lowers `owed`. **Money taken FROM a player improves this score.**

### What would close it

Add `main_uncredited_observed()` to `owed`. It cannot double count: it is defined as
`main_balance.saturating_sub(escrow_total() + table_claims() + open_payout_total())`, i.e. the
residual after exactly the terms `owed` already carries, and it floors at zero, so it can never
manufacture a shortfall. On the probe above it makes `owed = 100000000`, `difference_e8s = 0`,
and the verdict stays `CanPayEveryone` -- correctly, because the canister does hold what it owes.

It was NOT fixed in this reconciliation, deliberately: `get_solvency` is another owner's
surface, [FINDING 38](#finding-38) is a second open defect in the same arithmetic, and a wrong
correction here makes the instrument lie in the other direction. The marker is committed
`#[ignore]`d so it is red only when somebody asks for it, in the shape
`dev.sh known-defects` already established: **a suite that is red by design teaches everyone to
ignore red.**
